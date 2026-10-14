# ClawTerm technical guide

An agentic-OS terminal: every tab is a live conversation with an
[OpenClaw](https://docs.openclaw.ai) agent. Warp's UX — tabs, blocks, a
per-tab model picker — but instead of a PTY shell, each tab is bound to one
OpenClaw agent session streaming over the local gateway.

```
┌─ ClawTerm ──────────────────────────────────────────────────┐
│ [ALINA] [FORGE] [+]                            ● connected  │
│ agent: FORGE ▾   model: anthropic/claude-opus-4-7 ▾         │
├─────────────────────────────────────────────────────────────┤
│ you ❯  Hi FORGE — which agent are you?                      │
│ FORGE ❮  I'm FORGE, the coder agent.                        │
├─────────────────────────────────────────────────────────────┤
│ ❯ message this agent — Enter to send                        │
└─────────────────────────────────────────────────────────────┘
```

## How to run

Prereqs: Rust (stable), a running OpenClaw gateway on the same machine
(`openclaw gateway`), and `~/.openclaw/openclaw.json` containing
`gateway.auth.token`.

```bash
cargo run -p clawterm-app
```

The app reads the gateway port + token from `~/.openclaw/openclaw.json`
(override with `OPENCLAW_GATEWAY_URL` / `OPENCLAW_GATEWAY_TOKEN`). The agent
roster and model catalog are fetched live from the gateway at startup — nothing
is hardcoded.

Transport smoke test (no GUI, verifies the gateway round trip):

```bash
cargo run -p clawterm-gateway --example smoke
```

### Using it

- **Tabs**: `Ctrl+T` new tab, `Ctrl+W` close tab, `Ctrl+Tab` next tab, or use
  the `+`/`x` buttons. Each tab is one gateway session named
  `agent:<agentId>:clawterm-<uuid>`.
- **Agent**: pick from the `agent` dropdown, or type `:agent forge` (id or
  display name). Switching agents rebinds the tab to a fresh session under the
  new agent.
- **Model**: pick from the `model` dropdown, or type
  `:model anthropic/claude-opus-4-7` (`:model default` clears). This issues a
  `sessions.patch {model}` — a **per-session override** stored in the gateway
  session row. It never edits `openclaw.json` and needs no gateway restart.
  Verify with `/status` in the tab: the gateway reports
  `Session selected: <model> · Reason: session override`.
- **Slash commands** pass through to the gateway (`/status`, `/new`,
  `/compact`, …) since they are interpreted by the chat runner server-side.
- **Persistence**: tab layout (agent, session key, model override) is saved to
  `~/.local/share/clawterm/state.json`. Conversations themselves live in the
  gateway's session store; on restart ClawTerm reattaches to the same sessions
  and replays history via `chat.history`.

## Architecture

Two crates, one boundary: *the gateway owns all conversation state; the app
owns only presentation and the tab↔session binding.*

```
crates/clawterm-gateway     library: OpenClaw gateway WebSocket client
  src/config.rs             reads port + token from ~/.openclaw/openclaw.json
  src/client.rs             protocol v4 framing: RPCs + broadcast chat events
  src/types.rs              AgentInfo / ModelInfo / history projection
  examples/smoke.rs         live end-to-end transport test

crates/clawterm-app         binary: egui/eframe desktop app
  src/backend.rs            tokio thread owning the connection; Command/Event
                            channels bridge to the UI thread
  src/app.rs                tabs, transcript blocks, input line, pickers
  src/state.rs              ~/.local/share/clawterm/state.json (tab layout)
```

### Gateway protocol (discovered, not assumed)

ClawTerm speaks the OpenClaw gateway WebSocket protocol (v4) on
`ws://127.0.0.1:18789`:

- **Handshake**: the gateway sends `connect.challenge`; the client responds
  with a `connect` request carrying the shared token. ClawTerm connects as
  `client.id: "gateway-client", mode: "backend"` — the device-less loopback
  path — with scopes `operator.read/write/admin` (admin is required by
  `sessions.patch`).
- **Per tab**: `sessions.create {key, agentId}` → `chat.send {sessionKey,
  message, idempotencyKey}` → broadcast `chat` events
  (`state: delta|final|aborted|error`, token deltas in `deltaText`) filtered
  by `sessionKey` → rendered as blocks.
- **Roster / models**: `agents.list`, `models.list {view: "configured"}`.
- **Model picker**: `sessions.patch {key, model: "provider/id"}`.
- **History**: `chat.history {sessionKey}` (display-normalized by the gateway).

Protocol references: `docs/gateway/protocol.md` and the TypeBox schemas
shipped with the `openclaw` package (`packages/gateway-protocol`).

## The Warp design-reference boundary

ClawTerm uses Warp only as a product-design reference: tabs as first-class
sessions, a block-based transcript, a per-tab model picker, and an
input-at-bottom layout.

No Warp source code or crate is copied, linked, or bundled. The current public
`warpdotdev/Warp` repository does not publish the application source under an
MIT license, so ClawTerm makes no claim that Warp is an MIT software
dependency. Rendering and input are implemented with egui/eframe and
ClawTerm-owned Rust code.

See `THIRD_PARTY_NOTICES.md` for the design-reference notice and dependency
provenance boundary.

## License

MIT — see [LICENSE](../LICENSE).
