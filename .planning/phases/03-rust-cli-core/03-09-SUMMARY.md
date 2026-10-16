---
phase: 03-rust-cli-core
plan: 09
subsystem: process-control
tags: [rust, signals, process-tree, kill, procfs, tokio]

requires:
  - phase: 03-rust-cli-core
    provides: "03-03 procs identity and reconcile; 03-04 runner supervise; 03-07 partial usage"
provides:
  - "procs::kill_tree: S4 method — record descendants from /proc, SIGTERM group, grace (kill_grace_secs, default 3 s), SIGKILL group + every recorded process still alive, verified sweep; KillReport with survivors"
  - "Signal guards: every target matched by pid + /proc start time, pid > 1, never agent-brain's own pid or process group; killpg only when the recorded claude pid leads its group"
  - "runner: SIGINT (once graceful, twice forces SIGKILL), SIGHUP, SIGTERM and timeout all kill the whole tree; TreeGuard Drop kills on unwind; `agent-brain kill <task_id>` from another process; run rows end killed/timed_out with tokens kept"
  - "Drain loop exits as soon as the stream channel closes (03-04 finding: 2 s tail removed)"
  - "tests/fakes/fake_claude_tree.py: fake claude whose Bash grandchild sleeps in its own session (S4 r2 shape)"
affects: [03-11, 03-12, 03-13]

actuals:
  tasks: 2
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Blocking kill routine runs in spawn_blocking; supervisor select! covers stream, child exit, timeout, signals and kill requests"

key-files:
  created:
    - tests/fakes/fake_claude_tree.py
    - tests/test_fake_claude_tree.py
    - agent-brain/tests/runner_kill.rs
  modified:
    - agent-brain/src/procs.rs
    - agent-brain/src/runner.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, gemini-3.8-flash-high); reviewed by Claude"
  - "The 03-04 drain finding was handed to this plan (owner of runner.rs) and fixed test-first"

patterns-established:
  - "No process is signalled unless agent-brain recorded it (pid + start time)"

requirements-completed: [PROC-01, PROC-02]

duration: 19min
completed: 2026-10-16
status: complete
---

# Phase 3 Plan 09: Safe Process-Tree Kill Summary

**Stopping a run is now safe and complete: `agent-brain kill <task_id>`, Ctrl+C (once or twice), SIGHUP, SIGTERM and timeouts all use the verified S4 method, so no claude, bash or sleep process survives, and a reused PID is never signalled.**

## Performance

- **Duration:** 19 min (agy run 1144 s)
- **Completed:** 2026-10-16
- **Tasks:** 2
- **Files:** 5

## Task Commits

1. `d469039` test(03-09): add failing tree fake and kill_tree tests
2. `83f62f2` feat(03-09): S4 kill_tree and tree-shaped fake claude
3. `408e034` test(03-09): add failing kill and signal tests
4. `075878b` feat(03-09): signal-safe supervisor and kill command

## Deviations from Plan

One addition at the reviewer's request (03-04 review finding), in this plan's runner.rs: the drain loop breaks when the stream channel closes instead of waiting out the 2-second deadline. Tests `fast_run_drain_breaks_immediately` (about 150 ms instead of over 2 s) and `grandchild_holding_stdout_drain_deadline` (a grandchild holding stdout open still ends within the deadline).

## Review (Claude)

Re-run by the reviewer on a `git archive` snapshot of `075878b`:

- `runner_kill -- --test-threads=1`: 12 passed, including all 10 named in the acceptance criteria (P2-02 cooperative and SIGTERM-ignoring, P2-03 SIGINT once/twice, SIGHUP, SIGTERM, timeout, unknown/idle task, reconcile, reused PID).
- After the run, no process from the snapshot (fake claude tree, `sleep 317`) was alive; the only `sleep` processes on the machine were the reviewer's own monitor.
- Static checks: `fake_claude_tree.py` mode 100755 and `start_new_session=True`; `kill_tree` and `getpgrp` in procs.rs; `SignalKind::hangup`, `spawn_blocking`, `impl Drop for TreeGuard` in runner.rs; no `pkill`/`killall` in any of the five files; `cargo fmt --check` clean; `tests.test_fake_claude_tree` OK.
- Code read: `signal_group` / `signal_proc` refuse pid ≤ 1, agent-brain's own pid/group, and any process whose start time differs or is a zombie; `kill_tree` records descendants before the first signal, SIGTERMs the group, polls until the grace ends (or a forced second Ctrl+C), then SIGKILLs the group and every recorded survivor and verifies the sweep.
- Scope: the plan's five files.

## Issues Encountered

None.

## Next Phase Readiness

03-11 runs Dev/QA through the same supervisor; 03-13's live P2-02 check uses a real `sleep 300` through the real CLI.
