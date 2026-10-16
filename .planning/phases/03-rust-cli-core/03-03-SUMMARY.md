---
phase: 03-rust-cli-core
plan: 03
subsystem: core-library
tags: [rust, rusqlite, routing, gitops, procfs, tdd]

requires:
  - phase: 03-rust-cli-core
    provides: "03-01 crate shell, error module, embedded config"
provides:
  - "config.rs: config_from_value, defaults, validation messages, path resolution (env-based paths proven by 03-04)"
  - "routing.rs: normalize, slugify (48 chars, cut at last hyphen), resolve (None/Ambiguous)"
  - "projects.rs: Project model, load_map (never overwrites a bad file), select_project Fallback Ask (exit 2), require_runnable"
  - "profile.rs: NESTED_SESSION_VARS (11), check_profile, build_env, describe_profile"
  - "gitops.rs: full port of brain/gitops.py (argument arrays, GIT_TERMINAL_PROMPT=0)"
  - "planfile.rs: brain-meta line, read_meta/set_meta, plan skeleton"
  - "db.rs: schema v1 (projects, tasks, runs, events), WAL, FK, 0700/0600, task/run/event API"
  - "tasks.rs: current_task with adoption of Python-made task branches"
  - "procs.rs: /proc stat identity (pid + start time), alive()"
  - "runner.rs: RunStatus, reconcile_stale_runs (no signals), cmd_kill stub"
  - "app.rs: Ctx, open_ctx, open_ctx_with"
affects: [03-04, 03-05, 03-06, 03-07, 03-08, 03-09, 03-10, 03-11]

actuals:
  tasks: 2
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Library-level integration tests in agent-brain/tests/lib_foundation.rs, hermetic (TempDir, explicit env maps)"
    - "Process identity = (pid, /proc starttime); a PID alone is never trusted"

key-files:
  created:
    - agent-brain/src/config.rs
    - agent-brain/src/routing.rs
    - agent-brain/src/projects.rs
    - agent-brain/src/profile.rs
    - agent-brain/src/gitops.rs
    - agent-brain/src/planfile.rs
    - agent-brain/src/stream.rs
    - agent-brain/src/db.rs
    - agent-brain/src/tasks.rs
    - agent-brain/src/procs.rs
    - agent-brain/src/runner.rs
    - agent-brain/src/app.rs
    - agent-brain/tests/lib_foundation.rs
  modified:
    - agent-brain/src/lib.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, gemini-3.8-flash-high); reviewed by Claude"
  - "Reviewer formatted agent-brain/src/error.rs (03-01 file, rustfmt only, 8b79edf) so cargo fmt --check passes for every later plan"

patterns-established:
  - "Reconciliation only updates rows; signals belong to plan 03-09"

requirements-completed: []

duration: 17min
completed: 2026-10-16
status: complete
---

# Phase 3 Plan 03: Library and Storage Foundation Summary

**The Rust library now has every layer the `run lead` tracer needs: config, Vietnamese routing, the project map, profile env, git operations, plan meta, SQLite schema v1 with its API, task adoption, /proc process identity and signal-free stale-run reconciliation, all covered by 13 hermetic integration tests.**

## Performance

- **Duration:** 17 min (agy run 999 s)
- **Completed:** 2026-10-16
- **Tasks:** 2
- **Files:** 14

## Task Commits

1. `ebaff22` test(03-03): add failing library-core tests
2. `e4c8160` feat(03-03): library core — config, routing, projects, profile, gitops, plan meta
3. `573f113` test(03-03): add failing storage and reconcile tests
4. `000b562` feat(03-03): storage, task adoption, process identity, reconciliation

Reviewer commit: `8b79edf` style(03-01): rustfmt error.rs.

## Deviations from Plan

None in agy's execution. `cargo fmt --check` failed only on `error.rs`, a 03-01 file outside this plan's scope; agy left it alone and the reviewer formatted it.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- `cargo test --offline --locked`: 5 unit + 13 integration tests pass, three runs in a row. Task 1 grep = 7, Task 2 grep = 6, total ok = 13.
- All acceptance greps pass: `GIT_TERMINAL_PROMPT` and `OnceLock` present; no shell in gitops; no env-resolving calls in lib_foundation.rs; no `kill`/`killpg`/`SIGTERM`/`SIGKILL` in runner.rs; `PRAGMA user_version`, `params!`, `open_ctx_with` present; no `src/main.rs`; `brain/` and `tests/` untouched. Python suite: 119 OK. `cargo fmt --check` clean after `8b79edf`.
- Code read against the Python reference: `gitops.rs` matches `brain/gitops.py` function by function (same git arguments); `routing.rs` mirrors `brain/routing.py`; `db.rs` applies the migration in one transaction guarded by `user_version`, creates the directory 0700 and the file 0600, sets WAL, `synchronous=NORMAL`, FK on, 5 s busy timeout; `procs.rs` parses `/proc/<pid>/stat` after the last `)`; reconciliation marks rows lost and never signals.
- clippy (not a plan gate): 5 style warnings, no correctness issues (the "identical if blocks" in `resolve` mirrors the Python if/elif and is harmless).

Findings, handed to plan 03-05 (owner of routing.rs and profile.rs, whose parity fixtures test exactly these functions):

1. **normalize drops more marks than Python.** Rust uses `is_combining_mark` (Unicode Mn, Mc, Me); Python drops only category Mn. Reproduced with a probe: `ा` → Rust `""`, Python `"ा"`; `क़ि हिन्दी` → Rust `"क हनद"`, Python `"कि हिनदी"`. Vietnamese diacritics are all Mn, so Vietnamese routing and every slug are unaffected, but `normalize` is not byte-identical for other scripts.
2. **build_env does not normalise the tilde path.** Python applies `os.path.normpath` to `HOME + dir[1:]`; Rust joins without normalising, so a configured `~/.claude-orca/` (trailing slash) or `~/a/../b` gives a different `CLAUDE_CONFIG_DIR` string. Default config is unaffected.

## Issues Encountered

None.

## Next Phase Readiness

Plan 03-04 (tracer) writes `main.rs` and wires these modules end to end; it also proves the env-based config paths that these in-process tests deliberately do not touch.
