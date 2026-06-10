# Third-party notices

## Warp (UX reference)

ClawTerm's interaction design (tab-per-session model, block-based transcript,
per-tab model picker) follows the Warp terminal client, Copyright (C)
2020-2026 Denver Technologies, Inc. (github.com/warpdotdev/warp, studied via
the fork github.com/ControlStackAI/warp).

No Warp source code is included in this repository. Warp's `warpui` /
`warpui_core` crates are MIT-licensed; the remainder of the Warp repository is
AGPL-3.0-only and is intentionally not used. Should ClawTerm later embed the
MIT-licensed `warpui` crates, their copyright notice (Copyright (C) 2020-2026
Denver Technologies, Inc., MIT License) must accompany them.

## Rust dependencies

Direct dependencies (egui/eframe, tokio, tokio-tungstenite, serde, anyhow,
uuid, dirs, tracing, futures-util) are used under their respective MIT /
Apache-2.0 licenses as published on crates.io.
