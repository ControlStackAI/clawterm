# GOAL — Build "ClawTerm": an agentic-OS terminal that is a direct interface to OpenClaw's agents

## Mission (the why — internalize, don't parrot)

Matt is building an Arch + OpenClaw "agentic OS." The primary way a human should interact
with it is the terminal — but instead of typing into a shell, each terminal tab is a live
conversation with an OpenClaw agent. Think "Warp's UX, but every tab is an OpenClaw agent
session." Warp's client is open source (Rust), and a local fork already exists to leverage.

You are building this terminal, called **ClawTerm**. 

## Source material you MUST leverage (don't start from a blank page)

- **Local Warp source (already forked):** `/home/matthew/projects/Personal/warp`
  - It's a Rust/Cargo workspace. Git: origin `github.com/ControlStackAI/warp`, upstream `warpdotdev/warp`.
  - **Dual-licensed: AGPL-3.0 AND MIT** (`LICENSE-AGPL`, `LICENSE-MIT`). **Build ClawTerm under the MIT option.** Preserve required attribution/copyright notices for any code you copy over.
  - Key crates to study and pull from (the input + render + UI layers are what you want):
    - `crates/warp_terminal` — terminal core
    - `crates/warpui`, `crates/warpui_core`, `crates/warpui_extras` — UI layer
    - `crates/input_classifier` — input handling
    - `crates/warp_core`, `crates/warp_cli` — core + CLI
    - `app/` — app shell/entry
  - Pull over the terminal rendering / tab / input UX you need; rewire the input+output layer so a tab streams to/from an OpenClaw agent session instead of a PTY shell.

- **OpenClaw (the agent backend you're connecting to):**
  - Gateway runs locally on `localhost:18789` with token auth. Config + token: `~/.openclaw/openclaw.json` (look for `gateway.auth.token`).
  - Agent roster is defined in `~/.openclaw/openclaw.json` (`agents` / `agents.defaults`). Read it live — do NOT hardcode the roster.
  - **Docs (read these to discover the REAL API surface before designing transport):** `/home/matthew/.npm-global/lib/node_modules/openclaw/docs` — especially session, agent-loop, streaming, and gateway/API references.
  - Per-session model override is a first-class OpenClaw feature (sessions can run with a model override without editing config). Use it for the model picker.

## Core UX (required)

- Each tab = one persistent terminal session bound to one OpenClaw agent.
- Multiple tabs open at once, each can be a different agent (ALINA, FORGE, ATLAS, …).
- Agent and session are switchable within a tab; default is one session per tab.
- A per-tab model picker (like Warp's) selects the LLM for that tab's agent session.
- Typed input is sent to the bound agent; the agent's streamed reply renders in the tab.
- Per-tab session state survives an app restart.

## Hard constraints (do NOT violate — these prevent the fragile build)

1. **Transport:** talk to the OpenClaw gateway's real session/streaming API (`localhost:18789`, token auth). FIRST discover the actual API shape from the OpenClaw source/docs. Do NOT assume an API; do NOT shell out to `openclaw agent` per keystroke.
2. **Model switching = per-SESSION model override** (non-destructive). The picker must NOT rewrite `openclaw.json` and must NOT require a gateway restart on each change.
3. **License:** build under MIT; keep attribution intact for any Warp code reused.
4. **Roster is read live** from OpenClaw config — never hardcoded.

## Explicitly loose (your call — decide and proceed, don't ask)

Language/framework for new glue code, exactly how you fork vs. embed the Warp crates, file/repo
layout, rendering details, build tooling, how the picker/menus look. Pick sensible defaults and move.


## Repo & workflow

- Working directory: `/home/matthew/projects/clawTerm` (you are here).
- **Create the GitHub repo under the `ControlStackAI` org** (private) named `clawterm`, set it as `origin`, and push. Use `gh` (already authed as `mmango7474`, has ControlStackAI access). Default branch `main`. Add an MIT `LICENSE`, a `.gitignore`, and a `README.md`.
- Commit incrementally as each capability lands. Push regularly.

## Definition of Done (the goal is complete only when ALL are TRUE and demonstrated)

- [ ] GitHub repo `ControlStackAI/clawterm` exists, is `origin`, and the working tree is pushed.
- [ ] App builds from a clean clone and launches to a window with working tabs.
- [ ] At least two tabs open simultaneously, each bound to a different OpenClaw agent.
- [ ] Typing in a tab sends to that agent via the gateway API and streams the reply back into the tab.
- [ ] Per-tab model picker changes the active model for that tab's session, verified by the next
      response using the new model — WITHOUT editing `openclaw.json` or restarting the gateway.
- [ ] Per-tab session persists across an app restart (reopen → history/agent/model intact).
- [ ] `README.md` documents architecture, how to run, the Warp-fork boundary, and the MIT licensing.
- [ ] `git log` shows incremental commits; app builds and runs from a fresh clone.

## Process reminders

- Verify every checkbox by actually running the app — never claim done from code-reading.
- If a hard constraint turns out to be impossible, STOP and explain why rather than working around it.
