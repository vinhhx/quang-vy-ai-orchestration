---
phase: 03-rust-cli-core
plan: 01
subsystem: infra
tags: [rust, cargo, musl, crate-manifest, error-codes, embedded-assets]

requires:
  - phase: 02-protocol-mvp-scripts
    provides: "brain/__init__.py exit codes, brain/config.json, brain/prompts, brain/templates"
provides:
  - "agent-brain crate shell: Cargo.toml with ten =-pinned crates, committed Cargo.lock (98 packages), all crates in the local cargo cache"
  - "agent_brain::error: BrainError {message, code}, BrainResult, EXIT_OK..EXIT_INTERRUPTED (same values as brain/__init__.py)"
  - "Embedded assets: byte copies of the three prompts and two templates, config.default.json with kill_grace_secs, inspect_model, inspect_max_diff_bytes, timeout_seconds.inspect"
  - "Rust target x86_64-unknown-linux-musl installed (musl-gcc still missing, see Open item)"
affects: [03-03, 03-04, 03-06, 03-12]

actuals:
  tasks: 3
  commits: 2

tech-stack:
  added: [clap 4.6.7, serde 1.0.229, serde_json 1.0.154, rusqlite 0.40.2, tokio 1.53.3, libc 0.2.190, unicode-normalization 0.1.25, regex 1.13.1, chrono 0.4.45, tempfile 3.27.0]
  patterns:
    - "Every later plan builds with cargo --offline --locked"

key-files:
  created:
    - agent-brain/Cargo.toml
    - agent-brain/Cargo.lock
    - agent-brain/src/lib.rs
    - agent-brain/src/error.rs
    - agent-brain/assets/prompts/lead.md
    - agent-brain/assets/prompts/dev.md
    - agent-brain/assets/prompts/qa.md
    - agent-brain/assets/templates/plan.md
    - agent-brain/assets/templates/qa-report.md
    - agent-brain/assets/config.default.json
  modified:
    - .gitignore

key-decisions:
  - "Task 1 executed by Antigravity (agy 1.4.0, gemini-3.8-flash-high); reviewed by Claude"
  - "agy permission mode for Phase 3: --dangerously-skip-permissions, approved by the user on 2026-10-16 (recorded in memory before this run)"
  - "Task 2 (crate legitimacy): user replied \"approved\""
  - "Task 3 network steps (rustup target add, cargo generate-lockfile, cargo fetch, offline test, lockfile commit) were run by Claude at the user's request (\"run them\") after the user's own run left no trace on this machine; sudo apt install musl-tools remains the user's"

patterns-established:
  - "Network and toolchain steps are user checkpoints (or Claude on explicit request), never the executor"

requirements-completed: [DIST-01, CLI-03]

duration: 40min
completed: 2026-10-16
status: complete
---

# Phase 3 Plan 01: Crate Shell Summary

**`agent-brain` is a compilable library crate with exactly the ten audited crates pinned and locked, the Phase 2 exit codes as `BrainError`, and the Phase 2 prompts, templates and config embedded unchanged; every later plan can build offline.**

## Performance

- **Duration:** about 40 min including the user checkpoints (agy run 348 s)
- **Completed:** 2026-10-16
- **Tasks:** 3 (Task 1 executor; Tasks 2-3 checkpoints)
- **Files:** 11

## Accomplishments

- `Cargo.toml`: package `agent-brain` 0.1.0, edition 2021, rust-version 1.85, publish false; the nine runtime crates and dev `tempfile`, all with `=` pins; release profile `strip = true`, `lto = "thin"`, no `panic = "abort"`.
- `Cargo.lock`: 98 packages; the ten direct crates resolve to exactly the pinned versions.
- `src/error.rs`: constants 0/1/2/3/4/130, `BrainError::{new, error, not_available}`, `Display` (message only), `Error`, `From<io::Error>` ("Lỗi I/O: ..."), `BrainResult`; five unit tests.
- Assets: `cmp`-identical copies of `brain/prompts/{lead,dev,qa}.md` and `brain/templates/{plan,qa-report}.md`; `config.default.json` with the exact top-level key set from the plan (no `protocol_log_dir`).
- `.gitignore`: `agent-brain/target/`.

## Task Commits

1. `a640897` feat(03-01): crate manifest, error module with tests, and embedded assets (agy; tests committed with the implementation because nothing could compile before the fetch, which the plan allows)
2. `c00c591` build(03-01): lock agent-brain dependencies (Claude, Task 3)

## Deviations from Plan

- Task 3 was run by Claude instead of the user, at the user's explicit request. The user's first report of "toolchain ready" could not be confirmed (no lockfile, no musl target, no fetched crates on this machine).
- **Open item:** `musl-tools` is not installed (`sudo` needs the user's password). The musl release build of step 6 fails with `failed to find tool "x86_64-linux-musl-gcc"`, so that acceptance criterion is not met yet. It blocks nothing until plan 03-12, whose `scripts/build-release.sh` has the host-gcc fallback (route B). The user should run `sudo apt install musl-tools` before 03-12.

## Review (Claude)

Re-run by the reviewer:

- Both Task 1 `<verify>` commands print `manifest ok` / `assets ok`; all acceptance criteria pass (`agent-brain/target/` in .gitignore, `EXIT_INTERRUPTED`, `not_available`, `pub mod error;`, no `src/main.rs`, `brain/` untouched); Python suite 114 tests OK.
- Code read in full: `error.rs` matches the interfaces contract exactly; `config.default.json` values equal `brain/config.json` plus the four new keys.
- After Task 3: `git ls-files agent-brain/Cargo.lock` lists the file; `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked` → 5 passed; `rustup target list --installed` includes `x86_64-unknown-linux-musl`; `command -v musl-gcc` → not found (open item).
- Scope: Task 1 touched exactly its ten files. `spikes/lib/scan-secrets.sh`: OK.

## Next Phase Readiness

Plan 03-03 (library and storage foundation) can start: all crates are cached and `--offline --locked` builds work on the gnu target.
