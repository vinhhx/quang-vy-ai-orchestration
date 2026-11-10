---
phase: 06-multi-engine-media-branch
plan: 30
subsystem: engine
tags: [departments, phong-ban, routing, d-16, wr-03, eng-02]

requires:
  - phase: 06-28
    provides: "readiness_allows_run, member_usable on shared predicate"
provides:
  - "Stricter department pick in engine::resolve_inner"
  - "Explicit failure with Vietnamese message when department has members but none usable"
  - "Explicit failure with Vietnamese message when department pick errors"
  - "Documented fallback to team engine when department has no members"
  - "Seam, preview and TeamReadiness::refusal alignment"
affects: [06-VERIFICATION]

actuals:
  tokens: 2612
  tasks: 2
  commits: 2

tech-stack:
  added: []
  patterns:
    - "No silent fall-through to default engine (D-16)"
    - "Consistent readiness and resolution agreement across seam, preview, and refusal"

key-files:
  created: []
  modified:
    - agent-brain/src/engine.rs
    - agent-brain/tests/engine_routing.rs
    - agent-brain/tests/departments.rs

key-decisions:
  - "When a department has members but pick_from returns None, return BrainError with Vietnamese explanation naming department and role instead of falling back to claude (D-16)"
  - "When pick_from returns Err, return BrainError naming department and role instead of logging a warning and continuing on claude (D-16)"
  - "When a department has no members (0 members), keep the documented fall-through to team engine (claude)"

patterns-established:
  - "Zero silent fallback on configured department teams"

requirements-completed: [ENG-02]

coverage:
  - id: D1
    description: "Unusable department members stop the step before execution"
    requirement: ENG-02
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#phong_ban_unusable_members_stop_dev"
        status: pass
    human_judgment: false
  - id: D2
    description: "Department branches agree with preview and TeamReadiness"
    requirement: ENG-02
    verification:
      - kind: integration
        ref: "agent-brain/tests/departments.rs#phong_ban_department_branches_agree_with_team_readiness"
        status: pass
      - kind: unit
        ref: "agent-brain/src/teams.rs#c3_department_team_needs_a_ready_member"
        status: pass
    human_judgment: false
  - id: D3
    description: "Pick errors stop the step instead of falling back"
    requirement: ENG-02
    verification:
      - kind: unit
        ref: "agent-brain/src/engine.rs#resolve_inner"
        status: pass
    human_judgment: false

duration: 25min
completed: 2026-11-10
status: complete
---

# Phase 6 Plan 30: Department Team No Silent Fallback (D-16, WR-03, ENG-02) Summary

**On team phong-ban, configured roles whose departments have members but no usable candidate, or whose pick errors, explicitly fail with clear Vietnamese error messages instead of silently falling back to claude (D-16 / WR-03).**

## Performance

- **Duration:** 25 min
- **Started:** 2026-11-10T16:30:25+07:00
- **Completed:** 2026-11-10T16:58:00+07:00
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Closed Gap 3 of `06-VERIFICATION.md` (`D-16 "không đổi ngầm"`, review finding `WR-03`): removed silent fall-through to claude on team `phong-ban`.
- In `agent-brain/src/engine.rs` (`resolve_inner`):
  - Department file is loaded once via `crate::departments::load_path(&crate::departments::departments_path(), active_cfg)`.
  - Membership is checked: `has_members = dept.is_some_and(|d| !loaded.members_of(d).is_empty())`.
  - When `pick_from` returns `Ok(None)` and `has_members` is true: returns `BrainError` with exact message: `Đội hình {team}: {phòng} có thành viên nhưng không ai sẵn sàng cho vai {role} (độ khó {độ khó}) — không tự chạy bằng engine khác. Sửa engine của thành viên trên trang Phòng ban hoặc đổi đội hình rồi chạy lại.`.
  - When `pick_from` returns `Ok(None)` and `has_members` is false: preserves documented fall-through to team engine (`claude`).
  - When `pick_from` returns `Err(e)`: returns `BrainError` with exact message: `Đội hình {team}: không chọn được thành viên của {phòng} cho vai {role}: {e.message} — không tự chạy bằng engine khác.`.
- Preserved alignment between `resolve_for_run_with`, `preview_for_role` (Lead prompt's Dev engine block), and `TeamReadiness::refusal`.

## Task Commits

1. **Task 1 (tracer): `run dev` on `phong-ban` with only unusable Thi công members stops with a message, nothing runs** - `c5c16f2` (fix)
2. **Task 2: a department pick error stops the step instead of running claude (D-16)** - `06f68c1` (fix)

## Files Created/Modified

- `agent-brain/src/engine.rs` - Updated `resolve_for_run` doc comment; in `resolve_inner` department branch, loaded departments once, checked `has_members`, and replaced silent fall-through and warning with explicit `BrainError`s.
- `agent-brain/tests/engine_routing.rs` - Added `phong_ban_unusable_members_stop_dev` integration test verifying CLI run failure, Lead prompt preview message, zero dev run rows inserted, and zero fake claude invocations.
- `agent-brain/tests/departments.rs` - Added `phong_ban_department_branches_agree_with_team_readiness` integration test verifying error messages on dev, fallback to claude on qa, and identical refusal behavior in `TeamReadiness`.

## Decisions Made

- Strict adherence to D-16: never substitute an engine silently.
- When `has_members` is false (a department genuinely has no members), the documented rule `vai chưa có thành viên chạy claude` remains intact.
- Both `resolve_for_run_with` and `preview_for_role` return the exact same error string.

## Deviations from Plan

None - plan executed strictly as specified with exact Vietnamese error messages.

### Note on Rustfmt Drift

All files modified by plan 06-30 (`agent-brain/src/engine.rs`, `agent-brain/tests/engine_routing.rs`, `agent-brain/tests/departments.rs`) were formatted with `rustfmt` and pass `rustfmt --check` cleanly with 0 diffs.
As instructed, existing formatting drift in untouched files across the repository was left unmodified. Running `cargo fmt --manifest-path agent-brain/Cargo.toml --check` flags the pre-existing files:
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

- `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test departments --test engine_routing -- --test-threads=1`: 46 passed, 0 failed.
- `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib teams:: -- --test-threads=1`: 15 passed, 0 failed.
- `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1`: 716 unit tests, 41 test binaries passed (0 failed).
- `git diff --exit-code 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock`: clean (exit code 0).

## Next Phase Readiness

- All three gaps from `06-VERIFICATION.md` (Gap 1: CR-01 / 06-27, Gap 2: WR-04 / 06-28 & 06-29, Gap 3: WR-03 / 06-30) are now completely resolved, verified, and merged on `main`.
- Phase 06 code changes are fully complete.

---
## Self-Check: PASSED
- FOUND: agent-brain/src/engine.rs
- FOUND: agent-brain/tests/engine_routing.rs
- FOUND: agent-brain/tests/departments.rs
- FOUND commit: c5c16f2
- FOUND commit: 06f68c1
