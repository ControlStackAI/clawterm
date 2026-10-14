# Third-party notices

## Warp (design reference; no bundled code)

ClawTerm's interaction design (tab-per-session model, block-based transcript,
per-tab model picker) is inspired by the Warp terminal product. Warp and its
marks belong to their respective owner.

No Warp source code or Warp crate is included, copied, or linked in this
repository. The public `warpdotdev/Warp` repository checked during the 2026-10
release review did not publish the application source or an MIT license, so
ClawTerm does not claim an MIT-licensed Warp dependency. This notice
acknowledges the product design reference only. Any future incorporation of
third-party source requires a fresh license review and the source's complete
license notice.

## Rust dependencies

Rust dependency identities, versions, registry sources, and checksums are
recorded in `Cargo.lock`. Direct dependencies include eframe, tokio,
tokio-tungstenite, serde, serde_json, anyhow, uuid, dirs, tracing,
tracing-subscriber, and futures-util. Their license terms and the terms of
their transitive dependencies remain those published by their respective
authors; `Cargo.lock` is provenance data, not a replacement for those terms.
