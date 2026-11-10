---
phase: 06-multi-engine-media-branch
plan: 29
subsystem: ui
tags: [quota-rerun, telegram, dashboard, d-40, eng-11, quota-01, eng-02, unverified]

requires:
  - phase: 06-28
    provides: "TeamReadiness::unverified_engine, manual_dev_quota_with_env, readiness_allows_run"
provides:
  - "quota.unverified_teams field in GET /api/tasks/{id}"
  - "TaskState.quota_unverified and TaskState::quota_unverified_engine"
  - "team_button_label formatting with chưa kiểm mark on Telegram"
  - "Telegram quota rerun confirmation marking unverified engine"
  - "Dashboard rerun dialog teamDlgStepLine marking unverified teams"
  - "06-UI-SPEC Delta 2026-11-10 documentation"
affects: [06-VERIFICATION]

actuals:
  tokens: 4854
  tasks: 2
  commits: 2

tech-stack:
  added: []
  patterns:
    - "Transparent UI warning marks for unverified endpoints without blocking selection"
    - "Consistent mark copy across Telegram and Web UI"

key-files:
  created: []
  modified:
    - agent-brain/src/dashboard/api_tasks.rs
    - agent-brain/src/telegram/render.rs
    - agent-brain/src/telegram/bot.rs
    - agent-brain/src/telegram/summary.rs
    - agent-brain/tests/telegram_quota.rs
    - agent-brain/assets/dashboard/js/task.js
    - scripts/dashboard-js-check.mjs
    - agent-brain/tests/task_team.rs
    - .planning/phases/06-multi-engine-media-branch/06-UI-SPEC.md

key-decisions:
  - "Exact copy 'Đội hình {name} ({engine} — chưa kiểm)' on Telegram button and 'Bước {step} sẽ chạy bằng: {engine} — chưa kiểm' in confirmations and dialog lines"
  - "quota.unverified_teams computed at snapshot time from TeamReadiness::unverified_engine, keeping ready_teams ordering"
  - "Telegram hidden teams message drops 'hoặc chưa kiểm' clause because unverified teams are now offered and marked"

patterns-established:
  - "Unverified execution transparency at quota rerun selection point"

requirements-completed: [ENG-11, QUOTA-01, ENG-02]

coverage:
  - id: D1
    description: "Telegram quota button and confirmation mark unverified engine"
    requirement: ENG-11
    verification:
      - kind: integration
        ref: "agent-brain/tests/telegram_quota.rs#quota_unverified_team_button_marked"
        status: pass
      - kind: unit
        ref: "agent-brain/src/telegram/render.rs#team_button_label_marks_unverified"
        status: pass
    human_judgment: false
  - id: D2
    description: "Task API exposes quota.unverified_teams"
    requirement: QUOTA-01
    verification:
      - kind: integration
        ref: "agent-brain/tests/task_team.rs#quota_payload_marks_unverified_team"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/task_team.rs#quota_payload_and_rerun_manual"
        status: pass
    human_judgment: false
  - id: D3
    description: "Dashboard dialog option line marks unverified teams"
    requirement: ENG-02
    verification:
      - kind: automated_ui
        ref: "scripts/dashboard-js-check.mjs"
        status: pass
    human_judgment: false
  - id: D4
    description: "UI gates and smoke pass"
    requirement: ENG-02
    verification:
      - kind: automated_ui
        ref: "scripts/dashboard-ui-lint.sh"
        status: pass
      - kind: automated_ui
        ref: "scripts/build-css.sh --check"
        status: pass
      - kind: e2e
        ref: "scripts/dashboard-smoke.sh task-quota"
        status: pass
    human_judgment: false

duration: 38min
completed: 2026-11-10
status: complete
---

# Phase 6 Plan 29: Quota Re-run Unverified Engine Transparency (ENG-11, QUOTA-01, ENG-02) Summary

**Both Telegram quota buttons/confirmations and the dashboard re-run dialog transparently mark teams that run on unverified endpoints with the approved "chưa kiểm" notice without blocking choice or leaking secret credentials.**

## Performance

- **Duration:** 38 min
- **Started:** 2026-11-10T15:50:30+07:00
- **Completed:** 2026-11-10T16:28:30+07:00
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments

- Completed the visible portion of Gap 2 of `06-VERIFICATION.md` (`ENG-11`, `QUOTA-01`, `ENG-02`): after plan 06-28 allowed unverified engines to run, plan 06-29 ensures that all user-facing selection surfaces explicitly disclose unverified status using the locked `chưa kiểm` label.
- Server-side task snapshot: `TaskState` now computes `quota_unverified: Vec<(String, String)>` and provides `quota_unverified_engine(&self, team: &str) -> Option<&str>`; `quota_view` includes `"unverified_teams": Vec<String>` in `GET /api/tasks/{id}`.
- Telegram Bot:
  - `team_button_label`: formats buttons as `Đội hình {name} ({engine} — chưa kiểm)` when the engine has no E-09 evidence.
  - Confirmation prompt: `Bước {step} sẽ chạy bằng: {engine} — chưa kiểm` on first tap.
  - Hidden teams notice: parenthetical updated to `(engine thiếu key hoặc thiếu lệnh) — không hiện.` (removing "hoặc chưa kiểm" since unverified teams are no longer hidden).
- Dashboard:
  - `teamDlgStepLine(t)` in `task.js`: appends ` — chưa kiểm` when the team is in `quota.unverified_teams`.
- UI Gates & Regression Suite:
  - `dashboard-ui-lint.sh`, `build-css.sh --check`, and `dashboard-js-check.mjs` all passed.
  - `dashboard-smoke.sh task-quota` passed with `OK task-quota`.
  - Full Rust test suite (716 unit tests and 41 integration test binaries) passed with 0 failures.

## Task Commits

1. **Task 1 (tracer): Telegram marks a re-run team that will run unverified, on the button and in the confirmation** - `cae9d1d` (feat)
2. **Task 2: the dashboard re-run dialog shows the same mark; UI gates and the UI-SPEC delta** - `4809790` (feat)

## Files Created/Modified

- `agent-brain/src/dashboard/api_tasks.rs` - Added `TaskState.quota_unverified` field, `quota_unverified_engine` method, loader computation, and `unverified_teams` in `quota_view`.
- `agent-brain/src/telegram/render.rs` - Added `team_button_label` helper, updated `buttons_for` to use it, updated hidden-teams copy in `text_for`, added `quota_unverified` to `make_state`, added unit test `team_button_label_marks_unverified`.
- `agent-brain/src/telegram/bot.rs` - Appended ` — chưa kiểm` to `engine` before calling `render::rerun_confirm_text` when the team runs an unverified engine.
- `agent-brain/src/telegram/summary.rs` - Added `quota_unverified: Vec::new()` to `test_task_state` fixture.
- `agent-brain/tests/telegram_quota.rs` - Refactored `setup_quota` into `setup_quota_env`, added `quota_unverified_team_button_marked` integration test.
- `agent-brain/assets/dashboard/js/task.js` - Updated `teamDlgStepLine` to append ` — chưa kiểm` for teams in `quota.unverified_teams`.
- `scripts/dashboard-js-check.mjs` - Added JS check `rerun: chưa kiểm mark on an unverified team`.
- `agent-brain/tests/task_team.rs` - Added `quota_payload_marks_unverified_team` test and updated `quota_payload_and_rerun_manual` expectation with `unverified_teams: []`.
- `.planning/phases/06-multi-engine-media-branch/06-UI-SPEC.md` - Appended section `## Delta 2026-11-10 — dấu chưa kiểm khi chạy lại sau hết quota (gap 06-29, D-40)`.

## Decisions Made

- Applied exact copy `{name} ({engine} — chưa kiểm)` without creating new readiness words or modifying existing CSS classes.
- Verified that planted API keys (`PLANTED_DS_KEY`) never leak into Telegram API records or task JSON.

## Deviations from Plan

None - plan executed strictly as specified.

### Note on Rustfmt Drift

As specified in the plan, `api_tasks.rs`, `render.rs`, `bot.rs`, `summary.rs`, and `telegram_quota.rs` have pre-existing rustfmt drift from earlier phases. New code was written strictly in rustfmt style by hand without formatting whole files. `cargo fmt --manifest-path agent-brain/Cargo.toml --check` flags the expected pre-existing files across the repository:
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

## Verification Results

- `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test telegram_quota --test task_team -- --test-threads=1`: 29 passed, 0 failed.
- `bash scripts/dashboard-ui-lint.sh && bash scripts/build-css.sh --check && node scripts/dashboard-js-check.mjs`: OK (324 interactive cards, app.css matches source, 1469 JS checks).
- `bash scripts/dashboard-smoke.sh task-quota`: `OK task-quota` (Smoke OK: task-quota).
- `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1`: 716 unit tests, 41 test binaries passed.
- `git diff --exit-code 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock`: clean (exit code 0).
- `git diff --exit-code 82a1ba8 -- agent-brain/assets/dashboard/app.css agent-brain/assets/dashboard/screens`: clean (exit code 0).

## Next Phase Readiness

- Gap 2 of `06-VERIFICATION.md` is now completely closed across both execution (06-28) and user-facing transparency (06-29).
- Ready for plan 06-30.

---
## Self-Check: PASSED
- FOUND: agent-brain/src/dashboard/api_tasks.rs
- FOUND: agent-brain/src/telegram/render.rs
- FOUND: agent-brain/src/telegram/bot.rs
- FOUND: agent-brain/src/telegram/summary.rs
- FOUND: agent-brain/tests/telegram_quota.rs
- FOUND: agent-brain/assets/dashboard/js/task.js
- FOUND: scripts/dashboard-js-check.mjs
- FOUND: agent-brain/tests/task_team.rs
- FOUND: .planning/phases/06-multi-engine-media-branch/06-UI-SPEC.md
- FOUND commit: cae9d1d
- FOUND commit: 4809790
