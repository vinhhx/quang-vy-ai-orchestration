# Deferred items (Phase 6)

## From plan 06-10

- **rustfmt drift in `agent-brain/tests/dashboard_assets.rs`** (`asset_references_resolve`, the `for name in &["overview", ... "settings"]` array, introduced with the Settings screen in plan 06-08). `cargo fmt --manifest-path agent-brain/Cargo.toml --check` fails on it. Not touched by 06-10 (outside its files); a plain `cargo fmt` fixes it.

## From plan 06-23

- **rustfmt drift in `agent-brain/tests/e09_fixtures.rs`** (`e09_fixtures_parse`, the `println!("{name}: ok (in={} out={})", ...)` line at about line 49). `cargo fmt --manifest-path agent-brain/Cargo.toml --check` fails on it at the merge base (`6cc2112`); it is the only file reported. Not touched by 06-23 (outside its files); a plain `cargo fmt` fixes it.

## From plan 06-24

- **`agent-brain/assets/dashboard/js/task.js` `teamDlgReadinessLines` (about line 998)** still uses a fixed `['claude','deepseek','agy']` order, so the change-team and quota re-run dialogs show no `! {id}: {word}` line for other configured engine ids. Outside 06-24's files; needs a follow-up plan.
- **dsh `Model:` line:** the Settings row for a dsh engine omits `Model:` because the server puts the profile name there; the approved delta lists `Model:` without a kind restriction. Re-add if wanted.
