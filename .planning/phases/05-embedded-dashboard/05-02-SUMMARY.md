---
phase: 05-embedded-dashboard
plan: 02
subsystem: dashboard
tags: [rust, axum, sse, auth, sqlite, tracer]

requires:
  - phase: 05-embedded-dashboard
    provides: "05-01 axum/futures-util locked, Alpine CSP vendored, first app.css"
provides:
  - "`agent-brain serve [--port N]`: binds 127.0.0.1 only, prints URL + 64-hex token, stops on SIGINT/SIGTERM/SIGHUP with exit 0 and `Đã dừng dashboard.`"
  - "config key dashboard_port (default 7437, config::DEFAULT_DASHBOARD_PORT)"
  - "dashboard::{AppState, cmd_serve, router, json_error, blocking}"
  - "auth gate: Host allow-list (421), token handshake (303 + HttpOnly SameSite=Strict cookie), cookie check (401), Origin (403) and JSON Content-Type (415) on writes, CSP/nosniff/no-referrer/cache headers on every response"
  - "assets: include_bytes! table for /, /app.css, /vendor/alpine.csp.min.js; no filesystem access"
  - "pump: ab-pump thread on PRAGMA data_version, hints per run id + 0, reconcile_stale_runs every 20 ticks"
  - "SSE GET /api/runs/{id}/stream: cursor = events.seq, Last-Event-ID, status/end events, derived row text (no raw_json)"
  - "test helpers in tests/common/mod.rs: HttpResp, http_raw, wait_until, tree_procs, ProcGuard, Serve, Sandbox::serve, Sandbox::fake_claude_tree"
affects: [05-03, 05-04, 05-05, 05-06, 05-07, 05-08, 05-10, 05-12]

actuals:
  tasks: 1
  commits: 1

tech-stack:
  added: []
  patterns:
    - "Broadcast channel carries change hints only; every SSE connection reads rows from SQLite by its own cursor (terminal- and dashboard-started runs share one path)"
    - "One auth middleware layered after routes and fallback covers every path"

key-files:
  created:
    - agent-brain/src/dashboard/mod.rs
    - agent-brain/src/dashboard/auth.rs
    - agent-brain/src/dashboard/assets.rs
    - agent-brain/src/dashboard/pump.rs
    - agent-brain/src/dashboard/sse.rs
    - agent-brain/assets/dashboard/index.html
    - agent-brain/tests/dashboard_stream.rs
  modified:
    - agent-brain/src/main.rs
    - agent-brain/src/lib.rs
    - agent-brain/src/config.rs
    - agent-brain/assets/config.default.json
    - agent-brain/assets/dashboard/app.css
    - agent-brain/tests/common/mod.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, --dangerously-skip-permissions approved for Phase 5); reviewed by Claude"
  - "Default port 7437, cookie agent_brain_token (session, HttpOnly; SameSite=Strict; Path=/), token 32 bytes /dev/urandom → 64 hex, never on disk"
  - "Reviewer clarification passed to agy: the plan cited 'the exact CSP string in must_haves', which moved to 05-03 at the checker split; the 05-03 / research string was used verbatim"

patterns-established:
  - "Dashboard HTTP tests use the raw TcpStream client and Sandbox::serve; no HTTP client crate"

requirements-completed: []

duration: 25min
completed: 2026-10-17
status: complete
---

# Phase 5 Plan 02: Serve Tracer Summary

**`agent-brain serve` binds 127.0.0.1 and gates every path behind a per-start token cookie. It serves the embedded shell, CSS and Alpine, and streams over SSE the rows a run started from the terminal writes to SQLite, live and in order, then `end`. Ctrl+C stops it with exit 0.**

## Performance

- **Duration:** about 25 min (agy run 1146 s, plus review)
- **Completed:** 2026-10-17
- **Tasks:** 1 (tracer)
- **Files:** 13

## Accomplishments

- **CLI and config:** `Commands::Serve { port }` is dispatched before `app::open_ctx()`, in the same place as `Guard`. `dashboard_port` defaults to 7437, and a value above 65535 is refused with a Vietnamese error.
- **Startup order:** load config → port → `current_exe` → bind `Ipv4Addr::LOCALHOST` → `open_ctx_with` (validate, migrate, reconcile) → token → channels → pump → router → print the two lines. The `AddrInUse` message gives the `--port` and `dashboard_port` remedy.
- **Shutdown:** the first of SIGINT, SIGTERM or SIGHUP sets the watch and the pump stop flag, then starts graceful shutdown with a 3-second hard cap. `mod.rs` marks the spot where 05-03 adds the lock and 05-04 adds the job-manager shutdown.
- **SSE stream:** reads the run row before the next page of events, so no row committed before the final status can be missed. It emits `status` again only when status, outcome or tokens change, and emits `end` once the run is no longer running and the last page wasn't full. It waits for a hint, a `Lagged` error, 15 s, or shutdown.
- **Row format:** frames carry `{seq, ts, time, kind, ev, text}`, where `text` is the CLI's `term::event_text`. The raw JSON never goes into a frame.

## Task Commits

1. `2fb605c` feat(05-02): agent-brain serve tracer — token gate, embedded shell, live SSE (agy)

## Deviations from Plan

None by the executor. The plan said "the exact CSP string in must_haves", but that string moved to 05-03 when the plan was split. The reviewer passed the 05-03 / research string to agy, and `auth::CSP` matches it verbatim.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- **Scope:** `2fb605c` touches exactly the 13 declared files.
- **Verify commands:** `--test dashboard_stream` passes, with `tracer_terminal_run_streams_live_through_gate ... ok`. `--lib dashboard` runs 8 unit tests: auth ct_eq, cookie_value, host/origin allow-lists and new_token, plus sse row_kind and row_json init/thinking_tokens.
- **Plan verification:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1` exits 0 (20 test binaries, 165 tests, 0 failed). The Python suite is OK, `cargo fmt --check` exits 0, and `build-css.sh --check` prints "app.css khớp nguồn".
- **Acceptance greps:** `cmd_serve` 1 and `Ipv4Addr::LOCALHOST` 1. There are no matches for `0.0.0.0`, `UNSPECIFIED` or `Ipv6Addr`, for `xdg-open` or `webbrowser`, or for `unsafe-` in auth.rs. `"dashboard_port": 7437` appears once.
- **Code read:** `auth.rs`, `mod.rs`, `sse.rs`, `pump.rs` and `assets.rs` were read in full against the interfaces block and Pattern 1/2/4/8. `events.ts` is in milliseconds (`db::now_ms`), which matches `from_timestamp_millis` in `row_json`.
- **Secrets:** a pattern scan of the diff is clean (the only hit is the commit author line in git metadata).

**Non-blocking notes, carried forward:**

- **Signal handlers (for 05-03, which edits `mod.rs` and tests shutdown):** SIGINT, SIGTERM and SIGHUP are registered only when the final `select!` first polls, just after the URL is printed. A signal in that microsecond window would end the process with the default action, not exit 0. Registering them before printing closes the window.
- **Pump reopen:** after a DB error, `pump.rs` reopens the connection at once rather than sleeping a tick first. It can't spin hot, because each query is preceded by a tick sleep.
- **Placeholder shell (for 05-08):** `index.html` uses `font-bold` and has no `color-scheme` meta, both of which the UI-SPEC rules out. Plan 05-08 replaces the shell.

## Next Phase Readiness

Plan 05-03 (single-instance lock and the refusal and lifecycle matrix) can start. The gate, SSE and startup code it tests are in place.
