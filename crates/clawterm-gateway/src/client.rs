use anyhow::{anyhow, bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio_tungstenite::tungstenite::Message;

use crate::config::GatewayConfig;
use crate::types::{message_text, AgentInfo, HistoryMessage, ModelInfo};

const PROTOCOL_MIN: u32 = 3;
const PROTOCOL_MAX: u32 = 4;
const RPC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Streaming state of a chat run, mirroring the gateway `chat` event union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatState {
    Delta,
    Final,
    Aborted,
    Error,
}

/// One broadcast `chat` event scoped to a session.
#[derive(Debug, Clone)]
pub struct ChatEvent {
    pub state: ChatState,
    pub session_key: String,
    pub run_id: String,
    pub seq: i64,
    /// Incremental text for `Delta` events.
    pub delta_text: Option<String>,
    /// When true, `delta_text` replaces the accumulated text instead of appending.
    pub replace: bool,
    /// Full text of the cumulative/final assistant message when present.
    pub message_text: Option<String>,
    pub error_message: Option<String>,
}

/// Non-chat gateway happenings the UI may care about.
#[derive(Debug, Clone)]
pub enum GatewayEvent {
    Chat(ChatEvent),
    Connected,
    Disconnected(String),
}

struct Shared {
    pending: Mutex<HashMap<String, oneshot::Sender<Result<Value>>>>,
}

/// Async client for one gateway WebSocket connection.
///
/// Cheap to clone; all clones share the underlying connection.
#[derive(Clone)]
pub struct GatewayClient {
    tx: mpsc::UnboundedSender<Message>,
    shared: Arc<Shared>,
    events: broadcast::Sender<GatewayEvent>,
}

impl GatewayClient {
    /// Connect + handshake. Resolves once `hello-ok` is received.
    pub async fn connect(cfg: &GatewayConfig) -> Result<Self> {
        let (ws, _) = tokio_tungstenite::connect_async(&cfg.url)
            .await
            .with_context(|| format!("connecting to gateway at {}", cfg.url))?;
        let (mut sink, mut stream) = ws.split();

        // The gateway speaks first with a `connect.challenge` event.
        let challenge = tokio::time::timeout(RPC_TIMEOUT, stream.next())
            .await
            .context("timed out waiting for connect.challenge")?
            .ok_or_else(|| anyhow!("gateway closed during handshake"))??;
        let challenge: Value = serde_json::from_str(challenge.to_text()?)?;
        if challenge["event"] != "connect.challenge" {
            bail!("expected connect.challenge, got: {challenge}");
        }

        let connect_id = uuid::Uuid::new_v4().to_string();
        let connect_req = json!({
            "type": "req",
            "id": connect_id,
            "method": "connect",
            "params": {
                "minProtocol": PROTOCOL_MIN,
                "maxProtocol": PROTOCOL_MAX,
                // `gateway-client` + `backend` is the device-less loopback path;
                // shared-token auth carries the trust.
                "client": {
                    "id": "gateway-client",
                    "displayName": "ClawTerm",
                    "version": env!("CARGO_PKG_VERSION"),
                    "platform": std::env::consts::OS,
                    "mode": "backend"
                },
                "role": "operator",
                // admin is needed by sessions.patch (per-session model override)
                "scopes": ["operator.read", "operator.write", "operator.admin"],
                "auth": { "token": cfg.token },
                "locale": "en-US",
                "userAgent": format!("clawterm/{}", env!("CARGO_PKG_VERSION")),
            }
        });
        sink.send(Message::Text(connect_req.to_string())).await?;

        let hello = tokio::time::timeout(RPC_TIMEOUT, stream.next())
            .await
            .context("timed out waiting for hello-ok")?
            .ok_or_else(|| anyhow!("gateway closed during handshake"))??;
        let hello: Value = serde_json::from_str(hello.to_text()?)?;
        if hello["ok"] != true {
            bail!("gateway connect rejected: {}", hello["error"]);
        }

        let (tx, mut out_rx) = mpsc::unbounded_channel::<Message>();
        let shared = Arc::new(Shared {
            pending: Mutex::new(HashMap::new()),
        });
        let (events, _) = broadcast::channel(4096);

        // Writer task.
        tokio::spawn(async move {
            while let Some(msg) = out_rx.recv().await {
                if sink.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Reader task: route responses to pending RPCs, fan out events.
        let shared2 = shared.clone();
        let events2 = events.clone();
        tokio::spawn(async move {
            while let Some(frame) = stream.next().await {
                let frame = match frame {
                    Ok(Message::Text(t)) => t,
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => continue,
                };
                let Ok(msg) = serde_json::from_str::<Value>(&frame) else {
                    continue;
                };
                match msg["type"].as_str() {
                    Some("res") => {
                        let id = msg["id"].as_str().unwrap_or_default().to_owned();
                        if let Some(waiter) = shared2.pending.lock().await.remove(&id) {
                            let result = if msg["ok"] == true {
                                Ok(msg["payload"].clone())
                            } else {
                                Err(anyhow!("gateway error: {}", msg["error"]))
                            };
                            let _ = waiter.send(result);
                        }
                    }
                    Some("event") => {
                        if msg["event"].as_str() == Some("chat") {
                            if let Some(ev) = parse_chat_event(&msg["payload"]) {
                                let _ = events2.send(GatewayEvent::Chat(ev));
                            }
                        }
                    }
                    _ => {}
                }
            }
            let _ = events2.send(GatewayEvent::Disconnected("connection closed".into()));
        });

        let client = Self { tx, shared, events };
        let _ = client.events.send(GatewayEvent::Connected);
        Ok(client)
    }

    /// Subscribe to gateway events (chat stream + connection lifecycle).
    pub fn subscribe(&self) -> broadcast::Receiver<GatewayEvent> {
        self.events.subscribe()
    }

    /// Issue one RPC request and await its response payload.
    pub async fn rpc(&self, method: &str, params: Value) -> Result<Value> {
        let id = uuid::Uuid::new_v4().to_string();
        let (otx, orx) = oneshot::channel();
        self.shared.pending.lock().await.insert(id.clone(), otx);
        let frame = json!({"type": "req", "id": id, "method": method, "params": params});
        self.tx
            .send(Message::Text(frame.to_string()))
            .map_err(|_| anyhow!("gateway connection closed"))?;
        match tokio::time::timeout(RPC_TIMEOUT, orx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err(anyhow!("gateway connection closed mid-request")),
            Err(_) => {
                self.shared.pending.lock().await.remove(&id);
                Err(anyhow!("rpc {method} timed out"))
            }
        }
    }

    /// Live agent roster from the gateway (never hardcoded).
    pub async fn list_agents(&self) -> Result<Vec<AgentInfo>> {
        let payload = self.rpc("agents.list", json!({})).await?;
        let agents = payload["agents"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(agents
            .into_iter()
            .filter_map(|a| serde_json::from_value(a).ok())
            .collect())
    }

    /// Picker-sized configured model catalog.
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let payload = self.rpc("models.list", json!({"view": "configured"})).await?;
        let models = payload["models"].as_array().cloned().unwrap_or_default();
        Ok(models
            .into_iter()
            .filter_map(|m| serde_json::from_value(m).ok())
            .collect())
    }

    /// Ensure a session exists for `key` bound to `agent_id`.
    pub async fn create_session(&self, key: &str, agent_id: &str) -> Result<Value> {
        self.rpc("sessions.create", json!({"key": key, "agentId": agent_id}))
            .await
    }

    /// Send a user message into a session; streaming output arrives via `subscribe()`.
    /// Returns the gateway `runId`.
    pub async fn chat_send(&self, session_key: &str, message: &str) -> Result<String> {
        let payload = self
            .rpc(
                "chat.send",
                json!({
                    "sessionKey": session_key,
                    "message": message,
                    "idempotencyKey": uuid::Uuid::new_v4().to_string(),
                }),
            )
            .await?;
        Ok(payload["runId"].as_str().unwrap_or_default().to_owned())
    }

    /// Abort the active run for a session.
    pub async fn chat_abort(&self, session_key: &str) -> Result<()> {
        self.rpc("chat.abort", json!({"sessionKey": session_key}))
            .await?;
        Ok(())
    }

    /// Pin this session to `model_ref` ("provider/id"), or clear with `None`.
    /// This is a per-session override stored in the gateway session row —
    /// it does not touch openclaw.json and needs no gateway restart.
    pub async fn set_session_model(&self, key: &str, model_ref: Option<&str>) -> Result<Value> {
        self.rpc(
            "sessions.patch",
            json!({"key": key, "model": model_ref}),
        )
        .await
    }

    /// Display-normalized transcript for a session.
    pub async fn chat_history(&self, session_key: &str) -> Result<Vec<HistoryMessage>> {
        let payload = self
            .rpc("chat.history", json!({"sessionKey": session_key}))
            .await?;
        let rows = payload["messages"]
            .as_array()
            .or_else(|| payload["history"].as_array())
            .cloned()
            .unwrap_or_default();
        Ok(rows
            .iter()
            .filter_map(|m| {
                let role = m["role"].as_str()?.to_owned();
                let text = message_text(m);
                if text.is_empty() {
                    None
                } else {
                    Some(HistoryMessage { role, text })
                }
            })
            .collect())
    }

    /// Full session row (`sessions.describe`), e.g. to read the model override.
    pub async fn describe_session(&self, key: &str) -> Result<Value> {
        self.rpc("sessions.describe", json!({"key": key})).await
    }

    /// Delete a session row + transcript.
    pub async fn delete_session(&self, key: &str) -> Result<()> {
        self.rpc("sessions.delete", json!({"key": key})).await?;
        Ok(())
    }
}

fn parse_chat_event(payload: &Value) -> Option<ChatEvent> {
    let state = match payload["state"].as_str()? {
        "delta" => ChatState::Delta,
        "final" => ChatState::Final,
        "aborted" => ChatState::Aborted,
        "error" => ChatState::Error,
        _ => return None,
    };
    let msg_text = if payload["message"].is_object() {
        let t = message_text(&payload["message"]);
        (!t.is_empty()).then_some(t)
    } else {
        None
    };
    Some(ChatEvent {
        state,
        session_key: payload["sessionKey"].as_str()?.to_owned(),
        run_id: payload["runId"].as_str().unwrap_or_default().to_owned(),
        seq: payload["seq"].as_i64().unwrap_or_default(),
        delta_text: payload["deltaText"].as_str().map(str::to_owned),
        replace: payload["replace"].as_bool().unwrap_or(false),
        message_text: msg_text,
        error_message: payload["errorMessage"].as_str().map(str::to_owned),
    })
}
