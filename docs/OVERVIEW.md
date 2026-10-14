# ClawTerm

A Rust desktop client that turns tabs into persistent OpenClaw agent conversations instead of shell sessions.

**Status:** Prototype.

## Switch conversations

Use separate tabs for separate agents and keep each conversation attached to its session.

## Choose the model

Select a gateway-advertised model per session without editing the gateway configuration.

## Pick up where you left off

Persist tab bindings locally and reload conversation history from the gateway.

## How it fits together

Desktop tab → Session binding → Gateway WebSocket → Agent response.

## Current boundaries

Prototype desktop client, not a PTY terminal or sandbox. Current gateway compatibility must be verified separately. Conversations and model execution remain gateway responsibilities.

## Get started

See the [README](../README.md) for prerequisites, setup, configuration, and development checks.

## Further reading

- [Technical guide](../docs/technical-guide.md)
- [Third-party notices](../THIRD_PARTY_NOTICES.md)
- [Gateway client](../crates/clawterm-gateway/src/client.rs)

## Landing page

The companion [static landing page](site/index.html) is self-contained HTML/CSS with local assets and no analytics, external fonts, or JavaScript dependencies. Serve `docs/site/` from a static host to render it. GitHub file view shows its source. No hosting or repository-visibility change is implied by committing these files.
