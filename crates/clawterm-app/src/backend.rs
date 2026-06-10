//! Background worker: owns the tokio runtime + gateway connection and bridges
//! it to the egui thread over plain channels.

use clawterm_gateway::{
    load_gateway_config, AgentInfo, ChatState, GatewayClient, GatewayEvent, HistoryMessage,
    ModelInfo,
};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;

/// UI → backend.
#[derive(Debug)]
pub enum Command {
    /// Create (or reattach to) the gateway session backing a tab.
    EnsureSession { tab_id: u64, key: String, agent_id: String },
    /// Send user input into a session.
    Send { key: String, text: String },
    /// Pin/clear the per-session model override (None clears).
    PatchModel { key: String, model_ref: Option<String> },
    /// Reload a session transcript for a restored tab.
    LoadHistory { tab_id: u64, key: String },
    /// Abort the active run in a session.
    Abort { key: String },
}

/// Backend → UI.
#[derive(Debug)]
pub enum Event {
    Ready { agents: Vec<AgentInfo>, models: Vec<ModelInfo> },
    SessionReady { tab_id: u64, key: String },
    Delta { key: String, text: String, replace: bool },
    RunDone { key: String, text: Option<String>, error: Option<String> },
    History { tab_id: u64, key: String, messages: Vec<HistoryMessage> },
    ModelPatched { key: String, model_ref: Option<String> },
    Error { context: String, message: String },
    Disconnected { message: String },
}

/// Sends events to the UI thread and wakes the egui event loop.
#[derive(Clone)]
struct Emitter {
    events: Sender<Event>,
    repaint: Arc<dyn Fn() + Send + Sync>,
}

impl Emitter {
    fn emit(&self, ev: Event) -> bool {
        let ok = self.events.send(ev).is_ok();
        (self.repaint)();
        ok
    }
}

pub fn spawn(
    commands: UnboundedReceiver<Command>,
    events: Sender<Event>,
    repaint: impl Fn() + Send + Sync + 'static,
) {
    let emitter = Emitter {
        events,
        repaint: Arc::new(repaint),
    };
    std::thread::Builder::new()
        .name("clawterm-backend".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(run(commands, emitter));
        })
        .expect("spawn backend thread");
}

async fn run(mut commands: UnboundedReceiver<Command>, emitter: Emitter) {
    let client = match connect().await {
        Ok(c) => c,
        Err(e) => {
            emitter.emit(Event::Error {
                context: "connect".into(),
                message: format!("{e:#}"),
            });
            return;
        }
    };

    match load_catalog(&client).await {
        Ok((agents, models)) => {
            emitter.emit(Event::Ready { agents, models });
        }
        Err(e) => {
            emitter.emit(Event::Error {
                context: "catalog".into(),
                message: format!("{e:#}"),
            });
        }
    }

    // Fan gateway chat events into UI events.
    let mut gw_events = client.subscribe();
    let chat_emitter = emitter.clone();
    let chat_pump = async move {
        loop {
            match gw_events.recv().await {
                Ok(GatewayEvent::Chat(ev)) => {
                    let ui_ev = match ev.state {
                        ChatState::Delta => Event::Delta {
                            key: ev.session_key,
                            text: ev.delta_text.unwrap_or_default(),
                            replace: ev.replace,
                        },
                        ChatState::Final => Event::RunDone {
                            key: ev.session_key,
                            text: ev.message_text,
                            error: None,
                        },
                        ChatState::Aborted => Event::RunDone {
                            key: ev.session_key,
                            text: ev.message_text,
                            error: Some("(run aborted)".into()),
                        },
                        ChatState::Error => Event::RunDone {
                            key: ev.session_key,
                            text: ev.message_text,
                            error: Some(ev.error_message.unwrap_or_else(|| "unknown error".into())),
                        },
                    };
                    if !chat_emitter.emit(ui_ev) {
                        break;
                    }
                }
                Ok(GatewayEvent::Disconnected(why)) => {
                    chat_emitter.emit(Event::Disconnected { message: why });
                    break;
                }
                Ok(GatewayEvent::Connected) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    };

    let cmd_client = client.clone();
    let cmd_emitter = emitter.clone();
    let cmd_pump = async move {
        while let Some(cmd) = commands.recv().await {
            let client = cmd_client.clone();
            let emitter = cmd_emitter.clone();
            // Each command runs independently so a slow agent reply never
            // blocks other tabs.
            tokio::spawn(async move {
                handle_command(&client, cmd, &emitter).await;
            });
        }
    };

    tokio::select! {
        _ = chat_pump => {}
        _ = cmd_pump => {}
    }
}

async fn handle_command(client: &GatewayClient, cmd: Command, emitter: &Emitter) {
    match cmd {
        Command::EnsureSession { tab_id, key, agent_id } => {
            match client.create_session(&key, &agent_id).await {
                Ok(_) => {
                    emitter.emit(Event::SessionReady { tab_id, key });
                }
                Err(e) => {
                    emitter.emit(Event::Error {
                        context: format!("create session {key}"),
                        message: format!("{e:#}"),
                    });
                }
            }
        }
        Command::Send { key, text } => {
            if let Err(e) = client.chat_send(&key, &text).await {
                emitter.emit(Event::RunDone {
                    key,
                    text: None,
                    error: Some(format!("send failed: {e:#}")),
                });
            }
        }
        Command::PatchModel { key, model_ref } => {
            match client.set_session_model(&key, model_ref.as_deref()).await {
                Ok(_) => {
                    emitter.emit(Event::ModelPatched { key, model_ref });
                }
                Err(e) => {
                    emitter.emit(Event::Error {
                        context: format!("model override for {key}"),
                        message: format!("{e:#}"),
                    });
                }
            }
        }
        Command::LoadHistory { tab_id, key } => match client.chat_history(&key).await {
            Ok(messages) => {
                emitter.emit(Event::History { tab_id, key, messages });
            }
            Err(e) => {
                emitter.emit(Event::Error {
                    context: format!("history for {key}"),
                    message: format!("{e:#}"),
                });
            }
        },
        Command::Abort { key } => {
            if let Err(e) = client.chat_abort(&key).await {
                emitter.emit(Event::Error {
                    context: format!("abort {key}"),
                    message: format!("{e:#}"),
                });
            }
        }
    }
}

async fn connect() -> anyhow::Result<GatewayClient> {
    let cfg = load_gateway_config()?;
    GatewayClient::connect(&cfg).await
}

async fn load_catalog(client: &GatewayClient) -> anyhow::Result<(Vec<AgentInfo>, Vec<ModelInfo>)> {
    let agents = client.list_agents().await?;
    let models = client.list_models().await?;
    Ok((agents, models))
}
