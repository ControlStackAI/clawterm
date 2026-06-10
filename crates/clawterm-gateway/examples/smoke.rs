//! Live smoke test against the local OpenClaw gateway:
//! roster, models, session create, chat round trip with streaming,
//! per-session model override verified via /status.
//!
//! Run: cargo run -p clawterm-gateway --example smoke

use clawterm_gateway::{load_gateway_config, ChatState, GatewayClient, GatewayEvent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = load_gateway_config()?;
    let client = GatewayClient::connect(&cfg).await?;
    println!("connected to {}", cfg.url);

    let agents = client.list_agents().await?;
    println!(
        "agents: {:?}",
        agents
            .iter()
            .map(|a| format!("{}={}", a.id, a.display_name()))
            .collect::<Vec<_>>()
    );

    let models = client.list_models().await?;
    println!(
        "models: {:?}",
        models.iter().map(|m| m.model_ref()).collect::<Vec<_>>()
    );

    let key = "agent:scout:clawterm-smoke";
    client.create_session(key, "scout").await?;
    println!("session ready: {key}");

    let mut events = client.subscribe();
    let run_id = client
        .chat_send(key, "Reply with exactly: SMOKE-OK")
        .await?;
    println!("run started: {run_id}");

    loop {
        match events.recv().await? {
            GatewayEvent::Chat(ev) if ev.session_key == key => match ev.state {
                ChatState::Delta => {
                    print!("{}", ev.delta_text.unwrap_or_default());
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
                state => {
                    println!(
                        "\n[{state:?}] message={:?} err={:?}",
                        ev.message_text, ev.error_message
                    );
                    break;
                }
            },
            GatewayEvent::Disconnected(why) => anyhow::bail!("disconnected: {why}"),
            _ => {}
        }
    }

    // Model override round trip.
    client
        .set_session_model(key, Some("anthropic/claude-opus-4-7"))
        .await?;
    println!("patched session model to anthropic/claude-opus-4-7");
    let mut events = client.subscribe();
    client.chat_send(key, "/status").await?;
    loop {
        match events.recv().await? {
            GatewayEvent::Chat(ev) if ev.session_key == key => {
                if ev.state != ChatState::Delta {
                    let text = ev.message_text.unwrap_or_default();
                    let line = text
                        .lines()
                        .find(|l| l.contains("Session selected") || l.contains("override"))
                        .unwrap_or("(no override line found)");
                    println!("status: {line}");
                    break;
                }
            }
            GatewayEvent::Disconnected(why) => anyhow::bail!("disconnected: {why}"),
            _ => {}
        }
    }

    client.delete_session(key).await?;
    println!("cleaned up {key}");
    Ok(())
}
