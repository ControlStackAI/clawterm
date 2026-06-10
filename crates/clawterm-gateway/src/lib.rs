//! OpenClaw gateway client for ClawTerm.
//!
//! Speaks the gateway WebSocket protocol (v4) on the loopback interface:
//! - handshake: wait for `connect.challenge`, send a `connect` request with the
//!   shared gateway token (client id `gateway-client`, mode `backend`, which the
//!   gateway accepts device-less on direct loopback connections)
//! - framing: `{type:"req",id,method,params}` / `{type:"res",id,ok,payload|error}`
//!   / `{type:"event",event,payload}`
//! - chat runs: `chat.send` starts a run; streamed output arrives as broadcast
//!   `chat` events (`state: delta|final|aborted|error`) keyed by `sessionKey`.

mod client;
mod config;
mod types;

pub use client::{ChatEvent, ChatState, GatewayClient, GatewayEvent};
pub use config::{load_gateway_config, GatewayConfig};
pub use types::{AgentInfo, HistoryMessage, ModelInfo, SessionInfo};
