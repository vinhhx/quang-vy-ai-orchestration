---
phase: 05-embedded-dashboard
plan: 03
subsystem: dashboard
tags: [rust, flock, single-instance, auth, sse, tests]

requires:
  - phase: 05-embedded-dashboard
    provides: "05-02 serve tracer: gate, SSE, pump, startup/shutdown, test helpers"
provides:
  - "dashboard::lock::{LOCK_FILE, ServeLock, LockError, acquire, ServeLock::write_info}: flock LOCK_EX|LOCK_NB on <data_dir>/serve.lock (0600, 0700 dir), JSON {pid, port, started_at} only"
  - "cmd_serve refuses a second instance before any DB side effect, pointing at the first pid/port without the token"
  - "signal handlers registered before the URL lines (reviewer-requested fix from the 05-02 review)"
  - "tests: d10_port_in_use_exits_1, d10_config_port_used_without_flag, d14_second_serve_refused, shutdown_with_open_sse_is_prompt, dash01_url_line_and_shell, dash02_listens_on_loopback_only, dash03_requests_without_token_are_refused, dash03_security_headers, dash03_new_token_each_start, dash05_resume_with_last_event_id, d12_external_writer_rows_arrive_live, sse_unknown_run_is_404, lock::test_lock_lifecycle"
affects: [05-04, 05-12, 05-13]

actuals:
  tasks: 2
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Single instance = kernel flock on a CLOEXEC file; the file stays on disk, the lock dies with the process"

key-files:
  created:
    - agent-brain/src/dashboard/lock.rs
    - agent-brain/tests/dashboard_http.rs
  modified:
    - agent-brain/src/dashboard/mod.rs
    - agent-brain/tests/dashboard_stream.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, --dangerously-skip-permissions approved for Phase 5); reviewed by Claude"
  - "auth.rs, sse.rs and pump.rs untouched: every refusal-matrix and SSE edge test passed against the 05-02 code"

patterns-established:
  - "Tests that spawn a second server wait with wait_until + try_wait and clean up by recorded PID"

requirements-completed: [DASH-02, DASH-03]

duration: 20min
completed: 2026-10-17
status: complete
---

# Phase 5 Plan 03: Serve Hardening Summary

**Only one `serve` can run, and a second one points at the first without revealing the token. A taken port fails clearly, the config port is honoured, and every stop signal ends `serve` promptly even with an SSE stream open. Tests prove the socket is loopback-only, and that the token gate, the Host/Origin/Content-Type refusals and the security headers hold on every path. SSE resumes from Last-Event-ID, follows rows written by another process, and returns 404 for unknown runs.**

## Performance

- **Duration:** about 20 min (agy run 728 s, plus review)
- **Completed:** 2026-10-17
- **Tasks:** 2 (both TDD: tests committed first)
- **Files:** 4

## Accomplishments

- **Lock file:** `lock.rs` creates the data dir with mode 0700 when it's missing. It opens `serve.lock` with mode 0600 (std's CLOEXEC is kept) and takes a non-blocking exclusive `flock`. When the lock is already held, it reads pid and port leniently and returns `Held`. `write_info` truncates the file and writes only `{"pid","port","started_at"}`.
- **Startup order:** config → port → `lock::acquire` → exe → bind → `write_info` → `open_ctx_with`. A held lock refuses with exit 1 before the DB is opened.
- **Signal handlers:** SIGINT, SIGTERM and SIGHUP are created before the URL lines are printed. A failure to create one emits a Vietnamese message and exits 1.

## Task Commits

1. `ab8448f` test(05-03): single instance, port and shutdown tests (agy)
2. `6660540` feat(05-03): single-instance lock for agent-brain serve (agy)
3. `cacaf3e` test(05-03): gate matrix, loopback and SSE edge tests (agy)

## Deviations from Plan

- **Reviewer-requested change:** the signal handlers are now registered before the URL lines. This was carried over from the 05-02 review; `mod.rs` was already in Task 1's files.
- No `fix(05-03)` commits: no test exposed a defect in `auth.rs`, `sse.rs` or `pump.rs`.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- **Scope:** the 3 commits touch exactly the 4 files. `auth.rs`, `sse.rs` and `pump.rs` are unchanged.
- **Tests:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1` exits 0 with 178 passed and 0 failed, which is the 165 before plus 13 new. All 13 named 05-03 tests are OK, and the 05-02 tracer still passes. agy ran the process-spawning tests twice with no flakes.
- **Other checks:** the Python suite is OK, `cargo fmt --check` exits 0, and there are no non-loopback addresses in `src/dashboard`.
- **Assertions spot-checked:**
  - d14: the second serve's stdout, its stderr and `serve.lock` all lack the token, and stderr names the first pid and port.
  - dash02: `/proc/net/tcp` LISTEN entries on the port are `0100007F`, and `tcp6` has none.
  - d12: rows arrive in order 1, 2, 3 and then `end`.
- **Secrets:** a pattern scan of the diff is clean.

**Non-blocking notes:**

- **d12 timing bound:** the plan's behaviour says `end` arrives "within 3 s of the last write", but the test only bounds it with a 10 s read timeout plus order checks. Without the pump hints, the stream would sleep 15 s and miss `end`, so the test still proves live following, just with a looser bound than specified.
- **Lock info window:** a second `serve` that starts between the first one's `acquire` and its `write_info` (just before the bind) reads stale or empty info and prints `?` or an old pid. It is still refused, so D-14 holds.

## Next Phase Readiness

Plan 05-04 (assigning work on the server: child runs, `POST /api/tasks`, preflight, busy rules) can start. `mod.rs` keeps the marked spot for the job-manager shutdown.
