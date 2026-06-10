//! Local persistence of the tab layout. The conversations themselves live in
//! the gateway (session store + transcripts); we only remember which sessions
//! our tabs were bound to so a restart can reattach and replay history.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedTab {
    pub agent_id: String,
    pub session_key: String,
    #[serde(default)]
    pub model_ref: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedState {
    pub tabs: Vec<PersistedTab>,
    #[serde(default)]
    pub active_tab: usize,
}

fn state_path() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("clawterm").join("state.json"))
}

pub fn load() -> PersistedState {
    let Some(path) = state_path() else {
        return PersistedState::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save(state: &PersistedState) {
    let Some(path) = state_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(raw) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path, raw);
    }
}
