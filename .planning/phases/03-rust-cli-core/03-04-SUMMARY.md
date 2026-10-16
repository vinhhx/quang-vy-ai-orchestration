---
phase: 03-rust-cli-core
plan: 04
subsystem: cli
tags: [rust, clap, tokio, stream-json, sqlite, tracer, tdd]

requires:
  - phase: 03-rust-cli-core
    provides: "03-03 library and storage foundation"
provides:
  - "agent-brain binary: full clap command tree (projects scan/add, run, status, inspect, kill, stats, guard) with exit-code mapping"
  - "`agent-brain run lead` end to end: routing → profile env → task branch → claude in its own process group → stream to run dir + events table → terminal lines → .plan.md commit"
  - "runner::supervise (current-thread tokio, reader task + mpsc + select, timeout, kill_on_drop)"
  - "stream.rs event parsing and run summary; term.rs non-panicking terminal sink and line formats (D-09)"
  - "roles/mod.rs shared role spec, command builder, forbidden permission-flag check"
  - "Entry points for owner plans: fastpath::cmd_status, stats::cmd_stats, guard::cmd_guard (fail-closed exit 2), roles::dev/qa::cmd_dev/cmd_qa"
  - "tests/common/mod.rs Sandbox: env_clear + temp HOME/XDG/AGENT_BRAIN_DB/AGENT_BRAIN_CONFIG, fake claude"
affects: [03-05, 03-06, 03-07, 03-08, 03-09, 03-10, 03-11]

actuals:
  tasks: 2
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Tokio only inside runner.rs, on a current-thread runtime"
    - "Hermetic CLI e2e tests through a Sandbox that clears the environment"

key-files:
  created:
    - agent-brain/src/main.rs
    - agent-brain/src/term.rs
    - agent-brain/src/roles/mod.rs
    - agent-brain/src/roles/lead.rs
    - agent-brain/src/roles/dev.rs
    - agent-brain/src/roles/qa.rs
    - agent-brain/src/fastpath.rs
    - agent-brain/src/stats.rs
    - agent-brain/src/guard.rs
    - agent-brain/tests/common/mod.rs
    - agent-brain/tests/cli_e2e.rs
  modified:
    - agent-brain/src/lib.rs
    - agent-brain/src/stream.rs
    - agent-brain/src/runner.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, gemini-3.8-flash-high); reviewed by Claude"

patterns-established:
  - "Commands owned by later plans answer BrainError::not_available; guard fails closed with exit 2"

requirements-completed: [CLI-01]

duration: 18min
completed: 2026-10-16
status: complete
---

# Phase 3 Plan 04: Tracer `agent-brain run lead` Summary

**`agent-brain run lead "web bán hàng" "thêm nút đăng xuất"` now runs end to end against the fake claude: it routes the Vietnamese name, creates the task branch, runs claude in its own process group, prints one line per stream event, stores the run, its tokens and every event in SQLite, and commits only `.plan.md`.**

## Performance

- **Duration:** 18 min (agy run 1050 s)
- **Completed:** 2026-10-16
- **Tasks:** 2
- **Files:** 14

## Task Commits

1. `5b1e379` test(03-04): add failing run-lead tracer tests
2. `36838c1` feat(03-04): run lead tracer through every layer
3. `341f79f` test(03-04): add refusal-path and entry-point tests
4. `0fab40d` feat(03-04): entry points for owner plans and run-lead refusal paths

## Deviations from Plan

None.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- `cargo test --offline --locked`: 5 unit + 11 cli_e2e + 13 lib_foundation = 29 tests, three runs in a row, all pass. Task 1 grep = 2, Task 2 grep = 9, total cli_e2e ok = 11.
- Every acceptance grep passes: `process_group(0)`, `kill_on_drop(true)`, `new_current_thread`, `read_until`, `reconcile_stale_runs` in runner.rs; tokio used in no other src file; `env_clear` and `AGENT_BRAIN_DB` in the Sandbox; `Cờ quyền không được phép` in roles/mod.rs; the five owner entry points exist and main.rs dispatches status/stats/guard to them; `brain/` and `tests/` untouched. `cargo fmt --check` clean; Python suite OK.
- Isolation: the real `~/.config/agent-brain/` and `~/.local/share/agent-brain/` were not modified by the test runs (no file newer than the run start, no `brain.db` created).
- Code read: runner.rs (spawn with env_clear + built env, stdin null, stderr to file, own process group, reader task + mpsc + select with timeout); the forbidden-flag check runs on both `common_args` and role args before spawning.

Finding, handed to plan 03-09 (owner of runner.rs, which rewrites supervision for the S4 kill):

1. **Every run waits an extra 2 seconds after claude exits.** In `supervise_async`'s drain loop, `Some(line) = rx.recv()` stops matching once the reader closes the channel, so `select!` waits out the full 2-second `drain_deadline` instead of breaking. Measured: the single tracer test takes 2.16 s; with the drain loop breaking on `None` (scratch-copy experiment, not committed) the whole cli_e2e suite runs in 0.75 s and still passes. Fix: match `line = rx.recv()` and break on `None`, keeping the deadline only for a pipe held open by a grandchild (Pitfall 8).

Known tracer-scope limits (stated in the plan, owned by later plans): the timeout signals only the direct child and Ctrl+C/SIGHUP are not handled yet (03-09); plan validation, re-plan, branch rules and `--dry-run` land in 03-10.

## Issues Encountered

None.

## Next Phase Readiness

Wave 4 (03-05, 03-06, 03-07, 03-08) can run: each plan owns separate files per the ownership list.
