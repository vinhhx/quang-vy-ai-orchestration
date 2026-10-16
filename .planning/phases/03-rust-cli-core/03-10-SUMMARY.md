---
phase: 03-rust-cli-core
plan: 10
subsystem: roles-lead
tags: [rust, lead, protocol, validate-plan, e-06]

requires:
  - phase: 03-rust-cli-core
    provides: "03-05 routing/profile; 03-06 validate_plan; 03-07 stream/guard; 03-09 supervisor"
provides:
  - "`agent-brain run lead` at Phase 2 parity: validate_plan before commit (invalid plan → exit 1 with codes and manual commit hint, .plan.md left uncommitted), ROLE-01 extra-path violation → exit 3 without reverting, re-plan keeps the meta base commit, existing-but-not-checked-out task branch refused, stacking warning, --dry-run with no side effects"
  - "E-06: a second run on profile personal while a live (pid + start time) personal run exists is refused with exit 1; profile work only warns"
  - "roles/mod.rs: shell_join, display_argv (prompt shown as <assets/prompts/<role>.md>), shared execute_role"
  - "tests/support/protocol.rs: plan_text, lead_scenario, setup_project for 03-11"
affects: [03-11, 03-13]

actuals:
  tasks: 1
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Production code never reads test-harness variables; tests provide valid fake scenarios instead"

key-files:
  created:
    - agent-brain/tests/cli_lead.rs
    - agent-brain/tests/support/protocol.rs
  modified:
    - agent-brain/src/roles/lead.rs
    - agent-brain/src/roles/mod.rs
    - agent-brain/tests/cli_e2e.rs
    - agent-brain/tests/runner_kill.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, gemini-3.8-flash-high) in two runs (plan, review fix); reviewed by Claude"

patterns-established:
  - "When a new validation breaks older tests, fix the tests' fixtures, never special-case the test harness in src/"

requirements-completed: [CLI-03]

duration: 30min
completed: 2026-10-16
status: complete
---

# Phase 3 Plan 10: Lead Parity Through the Binary Summary

**`agent-brain run lead` now enforces the full Phase 2 Lead protocol (plan validation, ROLE-01 violation stop, re-plan, branch rules, dry run) and refuses a concurrent personal-profile run (E-06), with the shared role helpers in place for Dev and QA.**

## Performance

- **Duration:** 30 min (agy runs 1051 s plan, 616 s review fix)
- **Completed:** 2026-10-16
- **Tasks:** 1 (+ one review fix)
- **Files:** 6

## Task Commits

1. `a25c8cb` test(03-10): add failing lead parity tests
2. `0cd7212` feat(03-10): lead protocol parity and E-06 refusal
3. `55c4d9f` test(03-10): make tracer-era lead tests write a valid plan (review fix)
4. `512a4b5` fix(03-10): drop test-harness bypass from lead validation (review fix)

## Deviations from Plan

agy's first run reported one deviation, which the review rejected and fixed (below).

## Review (Claude)

Re-run by the reviewer:

- Acceptance: the nine named `cli_lead` tests pass (9/9); `validate_plan(` in lead.rs, `(E-06)` and `pub fn display_argv(` in roles/mod.rs, `pub fn setup_project(` in tests/support/protocol.rs; `cli_e2e` passes; `cargo fmt --check` clean.
- Code read: the E-06 guard refuses only when the recorded personal run is alive by pid + /proc start time, so a stale DB row cannot block the user.

Finding (resolved): **test-harness bypass in production code.** To keep four tracer-era tests green (`cli_e2e::run_lead_bad_json_line_recorded`, `runner_kill::{fast_run_drain_breaks_immediately, kill_unknown_or_idle_task, grandchild_holding_stdout_drain_deadline}`, whose fakes write no plan), `cmd_lead` dropped the `no_changes` validation error whenever `FAKE_CLAUDE_RECORD` was set and the scenario had no ops. Any caller setting that variable could commit an empty plan. The reviewer confirmed the four dependent tests by removing the block in a scratch copy, then wrote `.planning/fixes/03-10-remove-test-bypass.plan.md`; agy changed only the tests' fakes to write a valid plan (`55c4d9f`, assertions unchanged) and removed the block (`512a4b5`).

After the fix: `grep -rn FAKE_CLAUDE agent-brain/src/` prints nothing; full cargo suite passes (157 tests across 17 binaries, `runner_kill` with one thread 12/12); `cargo fmt --check` clean; Python suite OK.

Planning note: plan 03-10 did not list `cli_e2e.rs` / `runner_kill.rs` although turning on validation necessarily broke their fixtures; the fix plan covered that gap.

## Issues Encountered

None beyond the finding above.

## Next Phase Readiness

03-11 ports Dev and QA on top of `execute_role`, `display_argv` and `tests/support/protocol.rs`.
