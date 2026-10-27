---
phase: 06-multi-engine-media-branch
plan: 05
subsystem: engine-routing
status: complete
tags: [rust, engine-seam, teams, pricing, cli, fake-claude]

requires:
  - phase: 06-multi-engine-media-branch
    provides: "06-03 teams/staff/engine catalog (Engine, teams::resolve, TaskTeam, HumanWait, projects::team_of)"
  - phase: 06-multi-engine-media-branch
    provides: "06-04 schema v4 (tasks.team_json, task_team_json, set_task_team_if_null, insert_run_with_engine)"
provides:
  - "engine::{ResolvedEngine, resolve_for_run, StreamFormat, EnginePlan, plan_for, claude_argv_parts}"
  - "execute_role_staged decides the engine at one seam, stores runs.engine and metrics_json.team, prints `engine {engine}` in the header"
  - "run lead --team and auto --start --team; project default team; mac-dinh fallback"
  - "pricing::{engine_price, cost_for_engine, is_priced}; engine-first estimate_usd"
affects: [06-07, 06-09, 06-10, 06-11, 06-12, 06-13, 06-19]

actuals:
  tokens: 11600
  tasks: 3
  commits: 3
plan_head_before: 66426b30a0e4718b208acf096022bb5529b86174
plan_head_after: d67893d1f5ff59daec8db871f8fa5cb2fdb6ca5b

tech-stack:
  added: []
  patterns:
    - "One seam per run: resolve_for_run (task row -> engine) then plan_for (engine -> argv/env/label) before any run row or spawn"
    - "Children resolve the team from the task row; only `run lead` and `auto --start` take --team"
    - "Engine-first pricing: a non-claude run is priced only from pricing.<engine>, unpriced = 0"

key-files:
  created:
    - agent-brain/tests/engine_routing.rs
  modified:
    - agent-brain/src/engine.rs
    - agent-brain/src/roles/mod.rs
    - agent-brain/src/roles/lead.rs
    - agent-brain/src/main.rs
    - agent-brain/src/autopilot/mod.rs
    - agent-brain/src/pricing.rs
    - agent-brain/tests/cli_workflow.rs

key-decisions:
  - "Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6)."
  - "plan_for refuses deepseek and agy with `Engine {x} chưa được bật trong bản agent-brain này` and refuses người outside the Dev human path; plans 06-09, 06-11 and 06-12 fill those arms without changing the seam."
  - "engine_price also returns None for `người` (a person is never priced), matching is_priced."
  - "A wait of any mode (waiting or rejected) blocks every role except the waiting role (D-29 groundwork)."
  - "metrics_json.team is merged on every run, so metrics_json is never NULL any more (JSON null team for a legacy task)."

requirements-completed: [ENG-01, TEAM-03]

coverage:
  - id: D1
    description: "Every role run goes through resolve_for_run and plan_for; runs.engine and metrics_json.team are stored; the log header shows `engine claude`"
    requirement: ENG-01
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#team_flag_routes_and_records"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#legacy_task_runs_claude"
        status: pass
    human_judgment: false
  - id: D2
    description: "An all-claude team produces the same fake-claude record (argv, cwd, claude_config_dir, present_vars) as a legacy task with no team"
    requirement: ENG-01
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#all_claude_team_matches_legacy_record"
        status: pass
    human_judgment: false
  - id: D3
    description: "run lead --team and auto --start --team record the team before the Lead starts; project default and mac-dinh fallback; unknown explicit team refused before any branch or task row; --team refused for other roles and for auto without --start"
    requirement: TEAM-03
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#project_default_team"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#explicit_unknown_team_refused"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#replan_keeps_team"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#auto_start_records_team"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#auto_start_unknown_team_refused"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#team_flag_only_lead"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#auto_team_only_with_start"
        status: pass
    human_judgment: false
  - id: D4
    description: "A task whose team vanished from teams.json, or that waits for a person, refuses the next step before any run row or spawn"
    requirement: TEAM-03
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#missing_team_refused_before_run"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#waiting_marker_blocks_other_roles"
        status: pass
    human_judgment: false
  - id: D5
    description: "Engine-first pricing: non-claude runs are priced only from pricing.<engine> (unpriced = 0), claude keeps the family lookup and the stored result cost"
    requirement: ENG-01
    verification:
      - kind: unit
        ref: "agent-brain/src/pricing.rs#engine_first_pricing"
        status: pass
      - kind: unit
        ref: "agent-brain/src/pricing.rs#estimate_non_claude_unpriced_is_zero"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#claude_cost_unchanged"
        status: pass
    human_judgment: false

duration: 45min
completed: 2026-10-27
---

# Phase 6 Plan 05: Engine seam, task team and engine-first pricing Summary

**One seam (`resolve_for_run` -> `plan_for`) now decides every role run's engine from the task's stored team, records `runs.engine` and `metrics_json.team`, shows `engine claude` in the header, and tasks get their team at creation through `run lead --team`, `auto --start --team`, the project `team` key or `mac-dinh`; non-claude runs are priced only from `pricing.<engine>`.**

Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6).

## What was built

- **Task 1 (tracer, `d719792`)**: `engine.rs` gained `ResolvedEngine`, `resolve_for_run`, `StreamFormat`, `EnginePlan`, `plan_for`, `claude_argv_parts`. `execute_role_staged` calls the seam after the live-driver check and before any run row, uses `ep.argv` / `ep.env`, prints `... · engine {} · model {}`, and `Observer::on_spawn` calls `insert_run_with_engine` and merges `{team}` with the stage metrics in one call. `cmd_lead` resolves the team right after the workflow (before the dry-run branch, so an unknown explicit team creates no branch or task row), emits the project-default fallback note, and stores the team with `set_task_team_if_null` after `set_task_workflow_if_null`. `--team` is refused for non-lead roles. Tracer gate: `<verify>` re-run end-to-end (engine_routing 5/5, the seven regression suites, fmt) before expanding.
- **Task 2 (`0399f2e`)**: `AutoOpts.team`; `auto --team` without `--start` is refused; `auto --start --team T` resolves T before any spawn and forwards `--team T` to the Lead child before `--`. `stage_args` is untouched (children read the team from the task row). Tests for legacy NULL team, missing team, waiting marker and the all-claude adjacency record.
- **Task 3 (`d67893d`)**: `pricing::{engine_price, cost_for_engine, is_priced}`; `estimate_usd` uses `pricing.<engine>` for non-claude runs (no entry -> 0.0); `RunFinish.cost_usd` is `cost_for_engine(...)`, falling back to the result cost for claude and 0 otherwise.

## Verification

- `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --no-fail-fast -- --test-threads=1`: exit 0, 52 test binaries ok (404 lib tests, engine_routing 13).
- `cargo fmt --manifest-path agent-brain/Cargo.toml --check`: clean.
- `git diff --exit-code 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock`: no diff (no crate added).
- Acceptance greps: `engine::resolve_for_run` and `engine::plan_for` 2 lines in roles/mod.rs; `· engine {} · model {}` count 1; `set_task_team_if_null` in lead.rs sits after `upsert_task(` and before the Lead `build_command`; dev.rs, qa.rs, review.rs untouched; `"--team"` appears once in `cmd_auto`, `stage_args` has no `team`; the three pricing functions and `cost_for_engine` in roles/mod.rs present.

## TDD note

Task 1 and Task 3 were test-first (Task 1: all five tests run RED against the old binary before any production edit; Task 3: unit tests written before the pricing functions, so RED was a compile failure). In Task 2 the `autopilot/mod.rs` and `main.rs` change was written before its tests; the tests then passed on the first run, so RED was not observed for Task 2. The plan prescribes one `feat` commit per task, so there are no separate `test(...)` commits. The adjacency, missing-team and waiting tests compare real fake-claude records and run rows, so they would fail if the seam altered claude argv/env or ran a refused step.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `cli_workflow::note_limits` asserted `metrics_json IS NULL`**
- **Found during:** Task 3 full-suite run
- **Issue:** The plan requires `metrics_json.team` on every run, so a run with no note now has `{"team": ...}` instead of NULL; the old assertion encoded "no metrics at all" where the intent is "no note stored".
- **Fix:** The test now parses `metrics_json` (empty object when NULL) and asserts there is no `note` key.
- **Files modified:** agent-brain/tests/cli_workflow.rs (outside the plan's `files_modified`; a direct consequence of the required `metrics_json.team` behavior)
- **Commit:** d67893d

**Total deviations:** 1 auto-fixed (1 bug-in-test). **Impact:** none on behavior; the tracked behavior (blank note stores no note) is still asserted.

## Auth gates

None.

## Known Stubs

None. `plan_for` deliberately refuses `deepseek`, `agy` and `người` (plan intermediate state, filled by 06-09, 06-11, 06-12); that is a refusal with a clear message, not a stub feeding UI data.

## Threat Flags

None. The new surface (`--team` flag, team resolution from the task row) is covered by T-06-14 to T-06-17 in the plan's threat model: claude keeps `profile::build_env` (adjacency test), unknown teams are refused before any branch or row, a missing team refuses instead of switching engine, non-claude runs are never priced as Anthropic models.

## Issues Encountered

- `cargo test` stops at the first failing test binary without `--no-fail-fast`; the `note_limits` failure hid the later suites until it was fixed.
- Plan 06-02 (agy probe) is still open and 06-01's E-09 is `fail`; neither is a dependency of this plan and nothing here touches agy or DeepSeek.

## Self-Check: PASSED

- FOUND: agent-brain/tests/engine_routing.rs, agent-brain/src/engine.rs, agent-brain/src/pricing.rs
- FOUND commits: d719792, 0399f2e, d67893d (ancestors of HEAD; `git rev-list --count 66426b3..HEAD` = 3)
- Acceptance criteria of all three tasks re-run and passing; plan-level `<verification>` re-run and passing.
