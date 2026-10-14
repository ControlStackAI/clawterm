# GOAL — Build ClawTerm

## Mission

ClawTerm is a desktop interface where each tab is a persistent conversation
with an OpenClaw agent. Its terminal-inspired layout borrows familiar ideas
from modern terminals—tabs, transcript blocks, and command-oriented input—but
it is not a PTY shell.

## Product goals

- Each tab binds to one OpenClaw agent session.
- Multiple tabs may use different agents concurrently.
- Agent and model choices are selectable per tab.
- Responses stream from the OpenClaw gateway into transcript blocks.
- Tab/session bindings survive an application restart.

## Engineering constraints

1. Use the documented OpenClaw gateway WebSocket protocol. Do not shell out to
   a CLI for each message.
2. Apply model changes as per-session overrides. Do not rewrite gateway
   configuration or require a gateway restart.
3. Fetch the agent roster and model catalog from the gateway; do not hardcode
   an installation's agents.
4. Keep the project MIT-licensed. Warp is a UX reference only: no Warp source
   or crate is copied, linked, or bundled in the current implementation.
5. Keep credentials out of fixtures, logs, examples, and committed files.

## Source references

- OpenClaw documentation: <https://docs.openclaw.ai>
- OpenClaw source: <https://github.com/openclaw/openclaw>
- Warp product repository, used only as a design reference:
  <https://github.com/warpdotdev/Warp>

## Release checks

- `cargo fmt --all -- --check`
- `cargo check --locked --workspace`
- `cargo test --locked --workspace`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo build --locked --workspace`
- Protocol compatibility checked against a named OpenClaw release and a local
  synthetic gateway fixture. Synthetic coverage is not live-gateway
  acceptance.
- `Cargo.lock` contains dependency identities, registry sources, and checksums.
- `THIRD_PARTY_NOTICES.md` accurately describes the Warp boundary and Rust
  dependency provenance.

## Acceptance boundary

A source build and synthetic protocol tests establish local release quality.
They do not prove live gateway compatibility, a successful model call,
platform packaging, or end-to-end GUI behavior. Record those separately when
they are actually exercised.
