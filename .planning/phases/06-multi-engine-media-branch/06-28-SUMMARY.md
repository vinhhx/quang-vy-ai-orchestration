---
phase: 06-multi-engine-media-branch
plan: 28
subsystem: engine
tags: [engine, quota-rerun, teams, departments, d-40, eng-11, eng-10, unverified]

requires:
  - phase: 06-11
    provides: "teams, quota re-run, and engine routing"
  - phase: 06-26
    provides: "departments and role resolution"
provides:
  - "engine::readiness_allows_run(readiness: &str) -> bool unifying readiness check across quota re-run, project pins, and departments"
  - "TeamReadiness::unverified_engine helper identifying chua_kiem role engines"
  - "Quota re-run accepting unverified (chua_kiem) engines and running them marked (metrics_json.unverified = true)"
  - "Project pins accepting unverified (chua_kiem) engines without falling back to defaults"
  - "Department members (phong-ban) on unverified endpoints being picked and marked"
affects: [06-29, 06-30, 06-VERIFICATION]

actuals:
  tokens: 12418
  tasks: 2
  commits: 2

tech-stack:
  added: []
  patterns:
    - "Single readiness predicate readiness_allows_run for all role-to-engine bind points"
    - "Non-blocking chua_kiem execution marked with runtime warnings and metrics metadata"

key-files:
  created: []
  modified:
    - agent-brain/src/engine.rs
    - agent-brain/src/teams.rs
    - agent-brain/src/departments.rs
    - agent-brain/tests/task_team.rs
    - agent-brain/tests/engine_routing.rs
    - agent-brain/tests/departments.rs

key-decisions:
  - "readiness_allows_run returns true for Readiness::SanSang and Readiness::ChuaKiem, false for all other readiness states (D-40)"
  - "TeamReadiness::refusal uses readiness_allows_run to avoid blocking unverified engines while strictly enforcing thieu key / thieu lenh"
  - "TeamReadiness::unverified_engine detects whether a role in a team is mapped to an unverified engine"
  - "project_pin and departments::member_usable both call readiness_allows_run ensuring consistent behavior across all bind points"

patterns-established:
  - "Shared readiness predicate across routing, quota re-run, and department picking"

requirements-completed: [ENG-11, ENG-10]

coverage:
  - id: D1
    description: "Quota re-run allows unverified engine to run marked"
    requirement: ENG-11
    verification:
      - kind: integration
        ref: "agent-brain/tests/task_team.rs#quota_rerun_unverified_engine_runs"
        status: pass
    human_judgment: false
  - id: D2
    description: "Project pins accept unverified endpoints without silent fallback"
    requirement: ENG-11
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#unverified_endpoint_pinned_runs_marked"
        status: pass
    human_judgment: false
  - id: D3
    description: "Department members on unverified endpoints are selectable and marked"
    requirement: ENG-11
    verification:
      - kind: integration
        ref: "agent-brain/tests/departments.rs#phong_ban_member_on_unverified_endpoint_is_picked"
        status: pass
    human_judgment: false
  - id: D4
    description: "Shared readiness predicate strictly rejects unrunnable engines (thieu key, thieu lenh)"
    requirement: ENG-10
    verification:
      - kind: unit
        ref: "agent-brain/src/engine.rs#readiness_allows_run_policy"
        status: pass
      - kind: unit
        ref: "agent-brain/src/teams.rs#c3_unverified_engine_is_ready_and_marked"
        status: pass
    human_judgment: false

duration: 30min
completed: 2026-11-10
status: complete
---

# Phase 6 Plan 28: Unverified Engine Execution (D-40 / ENG-11) Summary

**`chưa kiểm` Anthropic endpoints (without E-09 evidence) are consistently runnable at all three bind points (quota re-run, project pins, and department members) and execute with the `· chưa kiểm E-09` header and `metrics_json.unverified = true` without blocking, while unrunnable states (`thiếu key`, `thiếu lệnh`) remain strictly refused.**

## Performance

- **Duration:** 30 min
- **Started:** 2026-11-10T14:46:00+07:00
- **Completed:** 2026-11-10T15:16:00+07:00
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- Closed Gap 2 of `06-VERIFICATION.md` (`ENG-11`, review finding `WR-04`): unified all three role-to-engine bind points under `engine::readiness_allows_run(readiness: &str) -> bool`, applying decision `D-40` consistently across the codebase.
- Added `engine::readiness_allows_run`: returns `true` for `san_sang` and `chua_kiem`, and `false` for `thieu_key`, `thieu_lenh`, and all other unknown/empty statuses.
- Updated `TeamReadiness::refusal` in `agent-brain/src/teams.rs`: unverified engines are now considered runnable for quota re-runs, and `TeamReadiness::unverified_engine` was introduced to identify unverified engines for UI badges in plan 06-29.
- Updated `project_pin` (Engine branch) in `agent-brain/src/engine.rs`: pinned endpoints without E-09 verification are accepted and run with the `· chưa kiểm E-09` header and `metrics_json.unverified = true`, instead of being refused with `chưa sẵn sàng`.
- Updated `member_usable` in `agent-brain/src/departments.rs`: department members on unverified endpoints are selectable on `phong-ban` teams and run with unverified metrics flags.
- Preserved strict rejection of genuinely unrunnable engines: endpoints with `thiếu key` or `thiếu lệnh` remain refused at all three bind points.
- Preserved `ENG-10` precedence: profile restrictions (`team_profile_refusal`, `profile_refusal`) continue to be evaluated prior to readiness checks.

## Task Commits

1. **Task 1: chưa kiểm engines are runnable at the quota re-run (D-40)** - `e61a720` (fix)
2. **Task 2: project pins and department members accept chưa kiểm engines (D-40)** - `383e05e` (fix)

## Files Created/Modified

- `agent-brain/src/engine.rs` - Added `readiness_allows_run`, updated `project_pin` Engine branch to use it, added `readiness_allows_run_policy` unit test.
- `agent-brain/src/teams.rs` - Updated `TeamReadiness::refusal` to use `readiness_allows_run`, added `TeamReadiness::unverified_engine` helper, added unit tests `c3_unverified_engine_is_ready_and_marked` and `c3_builtin_tiet_kiem_on_default_deepseek`.
- `agent-brain/src/departments.rs` - Updated `member_usable` to check readiness using `crate::engine::readiness_allows_run`.
- `agent-brain/tests/task_team.rs` - Added `manual_dev_quota_with_env` helper and `quota_rerun_unverified_engine_runs` integration test verifying full quota re-run lifecycle with unverified engines.
- `agent-brain/tests/engine_routing.rs` - Added `unverified_endpoint_pinned_runs_marked` integration test verifying project pin execution and header/metrics marking.
- `agent-brain/tests/departments.rs` - Added `phong_ban_member_on_unverified_endpoint_is_picked` integration test verifying department member selection and unverified marking.

## Decisions Made

- `readiness_allows_run` is the single source of truth for engine execution readiness.
- An unverified engine is never run silently: it always emits the `· chưa kiểm E-09` stderr header and sets `metrics.unverified = true` in the DB run record.
- `TeamReadiness::unverified_engine` inspects the engine corresponding to `role` on the given team and returns `Some(engine_id)` only if its readiness status is `chua_kiem`.

## Deviations from Plan

None - plan executed strictly as specified with exact symbol names `readiness_allows_run`, `unverified_engine`, and `manual_dev_quota_with_env`.

### Note on Rustfmt Drift

All files modified by plan 06-28 (`agent-brain/src/engine.rs`, `agent-brain/src/teams.rs`, `agent-brain/src/departments.rs`, `agent-brain/tests/task_team.rs`, `agent-brain/tests/engine_routing.rs`, `agent-brain/tests/departments.rs`) were formatted with `rustfmt` and pass `rustfmt --check` cleanly with 0 diffs.
As instructed, existing formatting drift in untouched files across the repository was left unmodified. Running `cargo fmt --manifest-path agent-brain/Cargo.toml --check` flags the following pre-existing files:
- `agent-brain/src/answers.rs`
- `agent-brain/src/autopilot/claim.rs`
- `agent-brain/src/autopilot/decide.rs`
- `agent-brain/src/autopilot/mod.rs`
- `agent-brain/src/autopilot/view.rs`
- `agent-brain/src/config.rs`
- `agent-brain/src/container.rs`
- `agent-brain/src/dashboard/api_departments.rs`
- `agent-brain/src/dashboard/api_fixbug.rs`
- `agent-brain/src/dashboard/api_intake.rs`
- `agent-brain/src/dashboard/api_projects.rs`
- `agent-brain/src/dashboard/api_prompts.rs`
- `agent-brain/src/dashboard/api_settings.rs`
- `agent-brain/src/dashboard/api_tasks.rs`
- `agent-brain/src/dashboard/mod.rs`
- `agent-brain/src/dashboard/pipeline.rs`
- `agent-brain/src/dashboard/settings_store.rs`
- `agent-brain/src/dashboard/sse.rs`
- `agent-brain/src/dashboard/taskstate.rs`
- `agent-brain/src/db.rs`
- `agent-brain/src/fastpath.rs`
- `agent-brain/src/fixbug/config.rs`
- `agent-brain/src/fixbug/gsheet.rs`
- `agent-brain/src/fixbug/init.rs`
- `agent-brain/src/fixbug/legacy.rs`
- `agent-brain/src/fixbug/mod.rs`
- `agent-brain/src/fixbug/pull.rs`
- `agent-brain/src/fixbug/trello.rs`
- `agent-brain/src/fixbug/writeback.rs`
- `agent-brain/src/gitops.rs`
- `agent-brain/src/guard.rs`
- `agent-brain/src/inputs.rs`
- `agent-brain/src/lib.rs`
- `agent-brain/src/main.rs`
- `agent-brain/src/orchestrator/mod.rs`
- `agent-brain/src/orchestrator/scheduler.rs`
- `agent-brain/src/orchestrator/triage.rs`
- `agent-brain/src/planfile.rs`
- `agent-brain/src/preflight.rs`
- `agent-brain/src/pricing.rs`
- `agent-brain/src/projects.rs`
- `agent-brain/src/prompts.rs`
- `agent-brain/src/protocol_paths.rs`
- `agent-brain/src/quota.rs`
- `agent-brain/src/roles/analyst.rs`
- `agent-brain/src/roles/dev.rs`
- `agent-brain/src/roles/lead.rs`
- `agent-brain/src/roles/mod.rs`
- `agent-brain/src/roles/qa.rs`
- `agent-brain/src/roles/review.rs`
- `agent-brain/src/telegram/bot.rs`
- `agent-brain/src/telegram/detect.rs`
- `agent-brain/src/telegram/fixbug_cmd.rs`
- `agent-brain/src/telegram/outbox.rs`
- `agent-brain/src/telegram/render.rs`
- `agent-brain/src/telegram/summary.rs`
- `agent-brain/src/workflows.rs`
- `agent-brain/src/workspace.rs`
- `agent-brain/tests/analyst_run.rs`
- `agent-brain/tests/analyst_workflows.rs`
- `agent-brain/tests/assets_parity.rs`
- `agent-brain/tests/autopilot_driver.rs`
- `agent-brain/tests/cli_autopilot.rs`
- `agent-brain/tests/cli_e2e.rs`
- `agent-brain/tests/cli_fix.rs`
- `agent-brain/tests/cli_lead.rs`
- `agent-brain/tests/cli_qa.rs`
- `agent-brain/tests/dashboard_autopilot.rs`
- `agent-brain/tests/dashboard_departments.rs`
- `agent-brain/tests/dashboard_projects.rs`
- `agent-brain/tests/dashboard_prompts.rs`
- `agent-brain/tests/dashboard_settings.rs`
- `agent-brain/tests/dashboard_workflows.rs`
- `agent-brain/tests/db_migration.rs`
- `agent-brain/tests/e2e_findings_fixes_b.rs`
- `agent-brain/tests/e2e_findings_fixes_d2.rs`
- `agent-brain/tests/e2e_findings_fixes_d2_worktree.rs`
- `agent-brain/tests/engine_agy.rs`
- `agent-brain/tests/fixbug_cli_api.rs`
- `agent-brain/tests/fixbug_pull.rs`
- `agent-brain/tests/fixbug_readers.rs`
- `agent-brain/tests/fixbug_trial.rs`
- `agent-brain/tests/intake_api.rs`
- `agent-brain/tests/orchestrator_scheduler.rs`
- `agent-brain/tests/orchestrator_triage.rs`
- `agent-brain/tests/project_read_sources.rs`
- `agent-brain/tests/prompt_layers.rs`
- `agent-brain/tests/protocol_dir.rs`
- `agent-brain/tests/runner_kill.rs`
- `agent-brain/tests/support/fixbug.rs`
- `agent-brain/tests/task_inputs.rs`
- `agent-brain/tests/task_origin.rs`
- `agent-brain/tests/telegram_actions.rs`
- `agent-brain/tests/telegram_intake.rs`
- `agent-brain/tests/telegram_notify.rs`
- `agent-brain/tests/telegram_quota.rs`

## Issues Encountered

- `telegram_actions::running_run_adds_kill_button` exhibited an asynchronous message markup race during the full test suite under high parallel test suite load; re-running the test individually and as part of `telegram_actions` passed with 0 errors.

## Next Phase Readiness

- Gap 2 (`ENG-11` / `D-40`) is closed and verified across routing, quota re-run, and department picking.
- Ready for plan 06-29 (`chưa kiểm` badge and warning UI in quota re-run choices).

---
## Self-Check: PASSED
- FOUND: agent-brain/src/engine.rs
- FOUND: agent-brain/src/teams.rs
- FOUND: agent-brain/src/departments.rs
- FOUND: agent-brain/tests/task_team.rs
- FOUND: agent-brain/tests/engine_routing.rs
- FOUND: agent-brain/tests/departments.rs
- FOUND commit: e61a720
- FOUND commit: 383e05e
