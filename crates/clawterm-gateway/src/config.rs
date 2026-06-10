use anyhow::{Context, Result};
use serde_json::Value;
use std::path::PathBuf;

/// Connection settings for the local OpenClaw gateway.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub url: String,
    pub token: String,
}

fn openclaw_config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("cannot resolve home directory")?;
    Ok(home.join(".openclaw").join("openclaw.json"))
}

/// Read the gateway URL + auth token from `~/.openclaw/openclaw.json`.
///
/// `OPENCLAW_GATEWAY_URL` / `OPENCLAW_GATEWAY_TOKEN` env vars override the file.
pub fn load_gateway_config() -> Result<GatewayConfig> {
    let env_url = std::env::var("OPENCLAW_GATEWAY_URL").ok();
    let env_token = std::env::var("OPENCLAW_GATEWAY_TOKEN").ok();
    if let (Some(url), Some(token)) = (env_url.clone(), env_token.clone()) {
        return Ok(GatewayConfig { url, token });
    }

    let path = openclaw_config_path()?;
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let cfg: Value = serde_json::from_str(&raw)
        .with_context(|| format!("parsing {}", path.display()))?;

    let gateway = &cfg["gateway"];
    let port = gateway["port"].as_u64().unwrap_or(18789);
    let token = env_token
        .or_else(|| gateway["auth"]["token"].as_str().map(str::to_owned))
        .context("gateway.auth.token missing from openclaw.json")?;
    let url = env_url.unwrap_or_else(|| format!("ws://127.0.0.1:{port}"));

    Ok(GatewayConfig { url, token })
}
