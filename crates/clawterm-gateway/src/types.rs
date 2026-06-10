use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One configured agent from `agents.list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Effective primary model ref ("provider/id") when reported.
    #[serde(default)]
    pub model: Option<Value>,
}

impl AgentInfo {
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

/// One model from `models.list` (view=configured).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub available: Option<bool>,
}

impl ModelInfo {
    /// Canonical "provider/id" ref accepted by `sessions.patch {model}`.
    pub fn model_ref(&self) -> String {
        match &self.provider {
            Some(p) if !p.is_empty() => format!("{}/{}", p, self.id),
            _ => self.id.clone(),
        }
    }
}

/// Session row subset from `sessions.describe` / `sessions.create`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    #[serde(default)]
    pub key: Option<String>,
    #[serde(rename = "sessionId", default)]
    pub session_id: Option<String>,
}

/// A display-normalized transcript entry from `chat.history`.
#[derive(Debug, Clone)]
pub struct HistoryMessage {
    pub role: String,
    pub text: String,
}

/// Extract readable text out of a transcript/chat `message` payload, which may
/// carry content as a plain string or as an array of `{type:"text",text}` parts.
pub fn message_text(message: &Value) -> String {
    match &message["content"] {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| {
                if p["type"].as_str() == Some("text") || p["text"].is_string() {
                    p["text"].as_str().map(str::to_owned)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}
