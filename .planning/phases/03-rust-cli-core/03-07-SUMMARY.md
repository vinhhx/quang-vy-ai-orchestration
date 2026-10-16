---
phase: 03-rust-cli-core
plan: 07
subsystem: stream-guard
tags: [rust, stream-json, terminal, guard-hook, contract-tests, log-02]

requires:
  - phase: 03-rust-cli-core
    provides: "03-02 stream.json/guard.json fixtures; 03-04 tracer stream/term/guard entry points"
provides:
  - "stream.rs: summarize_file with Python parity on all 9 stream.json cases, KPI fields, partial usage (UsageSource::Partial) for runs without a result line"
  - "term.rs: event lines, default truncation vs --verbose, summary line with KPI text, closed-stderr safe"
  - "guard.rs: `agent-brain guard` PreToolUse hook (decide + cmd_guard), fail-closed, NFC + non-strict realpath, guard.jsonl log, write_hook_settings in Claude Code's nested shape"
  - "Tests: contract_stream (2), cli_stream (4: P2-04, P2-05), contract_guard (5, all 21 shared cases), stream/term unit tests"
affects: [03-09, 03-11, 03-13]

actuals:
  tasks: 3
  commits: 8

tech-stack:
  added: []
  patterns:
    - "The hook command is the binary itself (agent-brain guard), never Python"

key-files:
  created:
    - agent-brain/tests/contract_stream.rs
    - agent-brain/tests/cli_stream.rs
    - agent-brain/tests/contract_guard.rs
  modified:
    - agent-brain/src/stream.rs
    - agent-brain/src/term.rs
    - agent-brain/src/guard.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, gemini-3.8-flash-high) in two runs; reviewed by Claude"
  - "Only Read populates `reads` (Python parity), overriding the plan's step text that also listed Grep/Glob"

patterns-established:
  - "When a plan step contradicts the Python reference and the plan's own parity must-have, the reference wins"

requirements-completed: [LOG-01, LOG-02]

duration: 26min
completed: 2026-10-16
status: complete
---

# Phase 3 Plan 07: Stream Records, Terminal Lines and Guard Hook Summary

**claude's stream now becomes trustworthy records: the Rust summary equals the Python one on every shared case (with partial usage for killed runs), each event prints as one terminal line, and `agent-brain guard` enforces the Dev sandbox from inside the binary, passing all 21 shared guard cases.**

## Performance

- **Duration:** 26 min (agy runs 818 s for Tasks 1–2, 699 s for Task 3 + review fix)
- **Completed:** 2026-10-16
- **Tasks:** 3
- **Files:** 6

## Task Commits

1. `0275f7d` test(03-07): add failing stream contract tests
2. `ab476e1` feat(03-07): stream summary parity and partial usage
3. `db10f72` test(03-07): add stream e2e and terminal tests
4. `d8ae19b` feat(03-07): terminal lines and stream e2e
5. `d55b95e` test(03-07): assert only Read populates reads (review fix)
6. `2b46036` fix(03-07): count only Read as a read, like Python (review fix)
7. `1ba8248` test(03-07): add failing guard contract tests
8. `249345f` feat(03-07): guard hook subcommand

## Deviations from Plan

- **Orchestration error (reviewer's):** the first agy prompt said "execute Task 1 and Task 2", so Task 3 (guard) was run in a second agy run. No work was lost or redone.
- **Plan defect fixed:** Task 1 step 2 said "Read/Grep/Glob go to reads", contradicting the plan's must-have of equality with `brain/streamlog.py`, which adds only `Read` to `reads`. A Dev Grep with a `path` would have changed `reads` / `reads_outside_plan` (the KPI) versus Python; no shared fixture has a Grep/Glob call, so the contract test could not catch it. Fixed test-first (`d55b95e`, `2b46036`); Grep/Glob still count in `tool_counts`.

## Review (Claude)

Re-run by the reviewer on `git archive` snapshots of `d8ae19b` and `249345f`:

- Task 1: `stream_contract_cases` + `stream_fixture_visits_all_cases` 2/2; `UsageSource::Partial` present; Python `tests.test_contract_stream` OK in the repo.
- Task 2: `p2_04_runs_row_complete`, `p2_05_bad_json_line_stored_raw`, `closed_stderr_does_not_abort_run`, `default_mode_truncates_verbose_shows_all` 4/4; `turn trước lần sửa đầu` in term.rs.
- Task 3: `guard_contract_cases`, `guard_fixture_ids`, `guard_never_opens_db`, `hook_settings_shape_and_quoting`, `read_free_mode_when_read_allow_none` 5/5; `printf '{bad' | agent-brain guard --policy /nonexistent` exits 2; `write_hook_settings` and the tool matcher present; no `python` in guard.rs.
- Full suite passes; `cargo fmt --check` clean; `test_reads_counts_only_read_not_grep` passes.
- Code read: guard resolves symlinks before comparing, NFC-normalises, blocks Grep/Glob when a read allowlist is set, fails closed on any unreadable input, and writes the nested `{matcher, hooks: [{type: command}]}` settings shape (the Phase 2 02-05 lesson).
- Scope: the plan's six files only; shared fixtures untouched.

## Issues Encountered

None beyond the deviations above.

## Next Phase Readiness

03-09 (kill) relies on partial usage for SIGKILLed runs; 03-11 (Dev) wires `write_hook_settings` and `agent-brain guard` into the Dev run.
