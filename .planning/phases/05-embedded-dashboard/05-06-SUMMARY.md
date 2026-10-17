---
phase: 05-embedded-dashboard
plan: 06
subsystem: dashboard
tags: [rust, sqlite, chrono, stats, api]

requires:
  - phase: 05-embedded-dashboard
    provides: "05-04 empty api_stats routes, AppState, blocking/json_error"
provides:
  - "stats::{DayStats, ProfileStats, day_key, query_by_day, query_by_day_in, query_by_profile}: day buckets in Rust with chrono (Local in production, injected tz in tests), per-profile totals; query_report untouched"
  - "GET /api/stats?profile=&since= → StatsReport JSON + by_day + by_profile; invalid profile or since → 400 with the CLI message"
affects: [05-11, 05-13]

actuals:
  tasks: 2
  commits: 4

tech-stack:
  added: []
  patterns:
    - "New breakdowns are new functions; the CLI's StatsReport shape is never changed"

key-files:
  created:
    - agent-brain/tests/dashboard_stats.rs
  modified:
    - agent-brain/src/stats.rs
    - agent-brain/src/dashboard/api_stats.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, --dangerously-skip-permissions approved for Phase 5); reviewed by Claude"

patterns-established:
  - "Profile and since filters use nullable SQL parameters (?1 IS NULL OR ...), never string-built SQL"

requirements-completed: [DASH-11]

duration: 20min
completed: 2026-10-17
status: complete
---

# Phase 5 Plan 06: Statistics API Summary

**`GET /api/stats` returns the CLI's statistics report plus token totals by day and by profile, filtered by the profile switcher and an optional `since` window. Day buckets follow local midnight. `agent-brain stats` is unchanged.**

## Performance

- **Duration:** about 20 min (agy run 711 s, plus review)
- **Completed:** 2026-10-17
- **Tasks:** 2 (both TDD: tests committed first)
- **Files:** 3

## Accomplishments

- **New stats functions:** `stats.rs` gains only additions, with no removed or changed lines.
  - `day_key` turns a millisecond timestamp into `YYYY-MM-DD` in a given time zone, falling back to UTC for an ambiguous local time.
  - `query_by_day_in` buckets every run row by its `started_at` day.
  - `query_by_profile` groups by `runs.profile`.

  All three use the same nullable profile and since filters as `query_report`, so the breakdowns add up to the same total.
- **Stats endpoint:** `api_stats.rs` checks `profile` with `profile::check_profile` and `since` with `stats::parse_since`; either failing returns 400 with the CLI message. It then merges `query_report` with `by_day` and `by_profile` inside `blocking`.

## Task Commits

1. `6ae5946` test(05-06): add tests for stats by day and by profile (agy)
2. `b044a54` feat(05-06): stats by day and by profile (agy)
3. `7ae2575` test(05-06): add tests for GET /api/stats (agy)
4. `a0c4ba6` feat(05-06): GET /api/stats (agy)

## Deviations from Plan

None.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- **Scope:** the 4 commits touch exactly the 3 declared files. `git diff` of `stats.rs` contains only `+` lines.
- **Tests:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --no-fail-fast -- --test-threads=1` exits 0 with 223 passed and 0 failed (215 before plus 8 new). These include `dash11_stats_breakdowns`, `dash08_stats_profile_filter`, `day_key_midnight`, `query_by_day_in_midnight`, `stats_bad_params_are_400` and `stats_requires_cookie`. The Python suite is OK and `cargo fmt --check` exits 0.
- **Schema:** the token and cost columns of `runs` are `NOT NULL DEFAULT 0` (db.rs schema v1), so reading them as `i64` and `f64` cannot hit NULL.
- **Secrets:** the pattern scan is clean.

**Pre-existing flaky test, not caused by this plan:**

- **What failed:** the reviewer's first full run (fail-fast) stopped at `contract_guard::guard_contract_cases`, a Phase 3 test from 03-07, with `write stdin: BrokenPipe` at tests/contract_guard.rs:202.
- **Cause:** in some contract cases `agent-brain guard` exits before reading its stdin, so the test's `write_all` races the child's exit.
- **Evidence it's a flake:** it then passed 8 times in a row alone, and the `--no-fail-fast` full run is green. Neither the test nor `guard.rs` has changed in Phase 5.
- **Possible fix:** ignore EPIPE on the write in the test, or have `guard` drain stdin first. Recorded in STATE.md as a concern.

## Next Phase Readiness

05-07 (projects API: suggestions, list, git status, alias editing, rescan) is the last plan of wave 5.
