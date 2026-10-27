# Deferred items (Phase 6)

## From plan 06-10

- **rustfmt drift in `agent-brain/tests/dashboard_assets.rs`** (`asset_references_resolve`, the `for name in &["overview", ... "settings"]` array, introduced with the Settings screen in plan 06-08). `cargo fmt --manifest-path agent-brain/Cargo.toml --check` fails on it. Not touched by 06-10 (outside its files); a plain `cargo fmt` fixes it.
