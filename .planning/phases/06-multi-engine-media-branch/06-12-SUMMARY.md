---
phase: 06-multi-engine-media-branch
plan: 12
subsystem: engine
tags: [human-engine, nguoi, git, fast-forward, autopilot, hum-01, hum-02, hum-03]
requires:
  - phase: 06-04
    provides: "team_json wait marker, set_task_team_cas, insert_human_run"
  - phase: 06-05
    provides: "engine seam (resolve_for_run, Engine::Human)"
  - phase: 06-07
    provides: "quota / D2 rules the human run flows through"
  - phase: 06-09
    provides: "report::sensitive_path (pub(crate)), agy Dev diff checks reused as the human scope rules"
provides:
  - "run dev on a nguoi:<id> team waits (marker, no process, no running row) and exits EXIT_WAITING = 6"
  - "agent-brain human done <task_id>: fetch + merge --ff-only, scope check, recorded human run"
  - "agent-brain human return <task_id> (Tra lai) for rejected fix steps"
  - "autopilot state waiting_human and resume after Xong"
affects: [06-16, 06-19]
tech-stack:
  added: []
  patterns:
    - "a human wait is a marker in team_json, never a running run row"
    - "git operations in the human engine are fetch / ls-remote / merge --ff-only / push without force only"
key-files:
  created:
    - agent-brain/src/human.rs
    - agent-brain/tests/human_engine.rs
  modified:
    - agent-brain/src/error.rs
    - agent-brain/src/lib.rs
    - agent-brain/src/main.rs
    - agent-brain/src/roles/dev.rs
    - agent-brain/src/autopilot/mod.rs
key-decisions:
  - "Xong compares the fetched tip with HEAD: equal = no new commit; HEAD not an ancestor of the tip (remote behind or diverged) = diverged text; otherwise merge --ff-only"
  - "A failed fetch is classified as 'branch missing' by an empty git ls-remote, not by parsing git's (localized) error text"
  - "The scope check reads the plan files from .plan.md at wait.base (git show), so a staff commit cannot widen its own allowlist"
  - "Run row, marker clear and dev_done status are written in one SQLite transaction (CAS loss rolls the run back)"
  - "After a protocol violation an autopilot task is resumed without the resume flag (new autopilot::stop_after_human_violation), because cmd_auto resumes with D2 skipped"
patterns-established:
  - "try_push: best-effort, never force, result stored in the marker and echoed with the exact manual command"
requirements-completed: [HUM-01, HUM-02, HUM-03]
duration: 50min
completed: 2026-10-27
status: complete
commits: 3
plan_head_before: ae55af4b6810733f0819c2e86596b86fef2ee3e9
plan_head_after: 496e72c2246d517a032f1c0aca4d5ef95ac0f87d
actuals:
  tokens: 14800
  tasks: 3
  commits: 3
---

# Phase 6 Plan 12: Human Dev step Summary

**A person can do the Dev step end to end: agent-brain pushes the task branch, waits through a `team_json.wait` marker (no process, no `running` row), fetches the pushed branch with `merge --ff-only` on `human done`, applies the agent scope rules, records a finished `nguoi` run and resumes the autopilot in the same process.**

## What was built

- **Task 1 (tracer)** `b403184`: `error::EXIT_WAITING = 6`. New `human.rs` with `valid_task_branch`, `push_command`, `try_push` (checks `origin`, pushes `refs/heads/{b}:refs/heads/{b}`, never `--force`), `start_wait` (staff and branch validation, idempotent re-run prints status and keeps the marker, CAS write of `HumanWait`, the `Chờ ... / Đã đẩy ... / Khi ... báo xong` lines), `done`, `give_back`, `HumanAction`, `cmd_human`. `roles/dev.rs`: after the Dev preconditions and approval record, `resolve_for_run(.., "dev")` returning `Engine::Human(id)` goes to `human::start_wait` (stage `fix` + `fix_for` for `--fix`, else `implement`); the dry run prints `Bước Dev do người làm — không chạy lệnh nào (dry run).` using a read-only task lookup. `main.rs`: `human done|return <task_id>`.
- **Task 2** `fa7554d`: tests for every `Xong` refusal (no marker, `rejected` marker, wrong branch, dirty tree, no origin, branch missing on the remote, no new commit, diverged), all leaving marker, run rows and local HEAD unchanged; scope violations (`src/khac.txt`, `.planning/`, `.github/`) give exit 3, a run with outcome `protocol_violation`, the listed paths and `agent-brain không tự hoàn tác`, with nothing reverted; fix steps start in `rejected` mode with `report_paths` and `Trả lại` switches to `waiting` with a new `since_ms` and a fresh push. The production code for these paths was written together with Task 1 (same file), so this commit is the tests.
- **Task 3** `496e72c`: `run_loop` releases the task as `waiting_human` (exit 0, no stop) when the child exits `EXIT_WAITING`, before the "no run created" check; `child_exit_is_neutral` (used by `child_error_stop`, unit-tested) includes `EXIT_WAITING`. `human done` resumes the autopilot with `cmd_auto` when `wait.autopilot`; after a violation it calls the new `autopilot::stop_after_human_violation`.

## Verification

- `cargo test --offline --locked --test human_engine -- --test-threads=1`: 13 passed (plan asked for 11).
- Task 1 suites (`cli_dev cli_fix engine_routing engine_quota engine_agy`): pass. Task 3 suites (`autopilot_driver cli_autopilot autopilot_branch` + `human_engine`): pass; lib `autopilot` unit tests: 49 passed.
- Full suite `cargo test --offline --locked -- --test-threads=1`: 58 test binaries `ok`, no FAILED / panicked lines (no contract_guard flake).
- `cargo fmt --check` clean; `git diff 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock` empty.
- Acceptance greps: `EXIT_WAITING` const, one `human::start_wait` line in dev.rs, no `--force` / `-f` / `reset` / `pull` / `rebase` literal in human.rs, `"--ff-only"` present, three refusal texts, `give_back`, `sensitive_path`, `"waiting_human"`, `cmd_auto` in human.rs.

## Deviations from Plan

### Auto-fixed / added

**1. [Rule 1 - Bug] Resume after a protocol violation would have re-run Dev instead of stopping**
- **Found during:** Task 3 (extra test `autopilot_violation_is_recorded_as_stop`)
- **Issue:** the plan's flagged assumption was that resuming through `cmd_auto` lets decide's D2 record `vi phạm giao thức`. `cmd_auto` resumes with `resume = true`, which skips D2, so the driver started the Dev step again (a second wait) and the violation was never recorded as a stop.
- **Fix:** `autopilot::stop_after_human_violation` claims the task and drives with `resume = false`; `human done` uses it only for the violation case. `cmd_auto` is still used for the normal case.
- **Files modified:** agent-brain/src/autopilot/mod.rs, agent-brain/src/human.rs
- **Commit:** 496e72c

**2. [Plan alignment] Unit test of the ignore list lives in the crate**
- `child_error_stop` needs a `Ctx`, so the ignore list became `pub(crate) fn child_exit_is_neutral` with a unit test in `src/autopilot/mod.rs` instead of a check in `tests/human_engine.rs`.

**3. [Minor] Two extra tests beyond the plan's list**
- `done_without_origin_refused` (the no-origin `Xong` refusal needs its own repo) and `autopilot_violation_is_recorded_as_stop`.

No other deviations. The executor choice (gsd-executor in a worktree, no agy) was made by the user for this phase.

## Self-Check: PASSED

- FOUND: agent-brain/src/human.rs, agent-brain/tests/human_engine.rs
- FOUND commits: b403184, fa7554d, 496e72c

## Known Stubs

None.

## Threat Flags

None beyond the plan's threat model. T-06-47 (fetch + ff-only, scope / `.planning/` / secret-path checks, validated `task/<slug>` ref), T-06-48 (push never forced, non-fatal), T-06-49 (`insert_human_run` with staff, times, base/head), T-06-50 (marker only, never a `running` row) and T-06-SC (no dependency change) are covered by tests. T-06-51 accepted by design: no Telegram or dashboard buttons were added.

## Limits

- `human done` never touches `.plan.md` content from the staff beyond allowing it in the diff; scope is judged against the plan as of `wait.base`.
- A fix step's allowlist is the plan files only (agent fixes may also touch files a report names); a human fix that edits such a file is reported as a violation.
- The dashboard rows, buttons and `… (job {n})` copy for `Xong` / `Trả lại` belong to plan 06-16.
