---
phase: 06-multi-engine-media-branch
plan: 16
subsystem: dashboard
tags: [human-engine, nguoi, dashboard, xong, tra-lai, pipeline, hum-01, hum-02]
requires:
  - phase: 06-10
    provides: "task team document, change_team refusal while a person waits, quota view"
  - phase: 06-12
    provides: "team_json.wait marker, EXIT_WAITING, agent-brain human done|return, autopilot waiting_human"
  - phase: 06-13
    provides: "LedgerRow.engine/human, FlowCard.engine, state_word(cho_nguoi)"
provides:
  - "GET /api/tasks/{id} field `human` (mode, staff, step, round, branch, since_ms, waiting_ms, report_paths, push) and actions human_done (Xong) / human_return (Trả lại)"
  - "POST /api/tasks/{id}/human/done and /human/return through one shared human_action that starts the 06-12 CLI as a job"
  - "taskstate: waiting_human autopilot state, banners Chờ {tên} làm / Chờ bạn · trả lại cho {tên}, strip gating while a person works"
  - "pipeline: apply_human_wait synthesizes a cho_nguoi ledger row and card without a run; overview items carry human_mode, human_name, quota_detail"
affects: [06-18, 06-19]
tech-stack:
  added: []
  patterns:
    - "a waiting person is shown from the marker only: the pipeline row is synthesized (run_id 0), never inserted, never counted in total"
    - "dashboard actions for the human step are derived into view.actions in load_task_state, so the shared action function and the payload cannot disagree"
key-files:
  created:
    - agent-brain/tests/human_dashboard.rs
  modified:
    - agent-brain/src/dashboard/taskstate.rs
    - agent-brain/src/dashboard/api_tasks.rs
    - agent-brain/src/dashboard/pipeline.rs
key-decisions:
  - "Xong refusal order: mode (rejected) then live run / driver then branch; Trả lại: mode then busy"
  - "Rejected mode: group waiting, banner Chờ bạn · trả lại cho {tên} (planner decision from the plan)"
  - "auto_start and auto_more_round controls are also disabled while a marker exists (extra to the plan, same reason text as the strip)"
  - "The synthesized row's kind reuses the existing ledger vocabulary: code, sửa theo QA, sửa theo review"
  - "Manual steps have no wait.round, so the synthesized row's round follows the Dev rows already on the ledger (+1)"
patterns-established:
  - "apply_human_wait also sets PipelineResponse.current_card and the legacy cards.code so every view of the pipeline agrees"
requirements-completed: [HUM-01, HUM-02]
duration: 55min
completed: 2026-10-27
status: complete
commits: 3
plan_head_before: f6ac28fd5489d4d7e17711dfcb7974632042cf03
plan_head_after: 54e6acdb67f95725bfaa499f96198bbfb9362de9
actuals:
  tokens: 15700
  tasks: 3
  commits: 3
---

# Phase 6 Plan 16: Human step on the dashboard (server side) Summary

**The task payload, actions, banners, autopilot state and pipeline now describe a waiting or rejected person step: `Xong` and `Trả lại` are token-gated dashboard actions that run `agent-brain human done|return` as jobs through one shared `human_action`, nothing else starts while a person works, and the pipeline shows a synthesized "chờ người" row without inventing a run.**

## What was built

- **Task 1 (tracer)** `80f09ed`: `taskstate.rs` gains `AutoFacts.human_mode/human_name`, `build_human_done` / `build_human_return`, the `waiting_human` auto state, the banners and groups for both modes, and the strip gating (`run_dev`, `run_qa`, `relead` refused with `Đang chờ {tên} làm — bấm Xong khi họ báo xong.` or `Kết quả của {tên} chưa đạt — bấm Trả lại để {tên} làm lại.`). `api_tasks.rs` passes the marker into `AutoFacts`, appends the two actions to `view.actions` when a wait exists (on-branch reason, live run / driver reason), adds `human_view` and the `human` field of the task detail, `human_action` and the routes `POST /api/tasks/{id}/human/done|return` (body `{rev}`, `control_response`).
- **Task 2** `7d6dd94`: unit tests `rejected_banner`, `human_strip_gating`, `waiting_human_state`; integration tests `rejected_mode_and_return`, `strip_disabled_while_waiting`, `autopilot_waiting_human_state` (real `auto --start` + `approve` reaches `waiting_human`) and `xong_refused_off_the_task_branch`. The production code for these paths was written with Task 1 (same functions and files), so this commit is the tests, as in 06-12.
- **Task 3** `54e6acd`: `pipeline::apply_human_wait` (synthesized ledger row `run_id: 0`, `human: true`, `engine: "người:{id}"`, state `cho_nguoi`, verdict `chờ người`, `started_at = since_ms`, `duration_ms` = waiting time; card state `cho_nguoi`, result `Chờ {tên} làm`; `total` untouched), called from `get_pipeline` after `set_planned_engines`. `list_tasks` reads each task's `team_json` and the staff list once, feeds the `AutoFacts` (banner and group match the detail) and adds `human_mode`, `human_name`, `quota_detail` to every item.

## Verification

- `cargo test --offline --locked --test human_dashboard -- --test-threads=1`: 9 passed (plan asked for 8 across the three tasks).
- `--lib dashboard::taskstate`: 37 passed; `--lib dashboard::pipeline`: 46 passed (includes `human_wait_synthesized_row`).
- `--test dashboard_actions --test dashboard_autopilot --test dashboard_pipeline --test dashboard_http`: all pass (14, 36, 22, 9).
- Full suite `cargo test --offline --locked -- --test-threads=1`: 59 test binaries `ok`, no FAILED / panicked lines (no contract_guard flake).
- `cargo fmt --check` clean; `git diff --exit-code 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock` empty.
- Acceptance greps: `pub fn human_action` / `pub fn human_view` (2 lines), both routes (2), `pub fn apply_human_wait` (1) and one `apply_human_wait` use in api_tasks.rs, `"waiting_human"` and `bấm Xong khi họ báo xong` in taskstate.rs; no file under `src/telegram/` is touched by any `(06-16)` commit.

## Deviations from Plan

### Auto-fixed / added

**1. [Rule 2 - Missing critical functionality] auto_start / auto_more_round controls gated while a person works**
- **Found during:** Task 1 (writing `waiting_human_state`)
- **Issue:** with the auto state `waiting_human` the controls list offered an enabled `Bật autopilot`; pressing it while the task waits for a person would start a driver on a task whose step is held by someone else (T-06-62: actions while a person works).
- **Fix:** `derive_with` disables `auto_start` and `auto_more_round` with the same strip text whenever a marker exists; `POST /api/tasks/{id}/auto` is therefore refused (tested).
- **Files modified:** agent-brain/src/dashboard/taskstate.rs
- **Commit:** 80f09ed

**2. [Plan alignment] Task 2 production code landed in the Task 1 commit**
- `auto_state_of`, the rejected banner / group and the strip gating sit in the same functions as the waiting banner, so they were written once; Task 2 is the tests that pin them (unit + integration).

**3. [Clarification] Fix-row kind text**
- The plan says the fix-stage row's kind is `sửa`; the ledger already names these `sửa theo QA` / `sửa theo review` (`derive_inner`), so the synthesized row reuses them (label `Vòng {n} · sửa theo QA · người {tên}`), keeping one vocabulary. It still sits on the `fix` card (a fix for QA) or the `code` card (a fix for code review).

**4. [Additions beyond the plan, same files]**
- `apply_human_wait` also sets `current_card` (the waiting card) and the legacy `cards.code` so the old and new pipeline views agree; one extra integration test (`xong_refused_off_the_task_branch`).
- `quota_detail` in the overview test uses a lead run flipped to `quota` through SQL (reason line `Run 1 (lead) …`), not a real Dev quota stop.

No other deviations. The executor choice (gsd-executor in a worktree, no agy) was made by the user for this phase.

## Auth gates

None.

## Known Stubs

None.

## Threat Flags

None beyond the plan's threat model. T-06-61 (routes only on the token-gated dashboard, no Telegram path, `rev` + enabled-action checks) and T-06-62 (run_dev / run_qa / relead / change_team / autopilot start refused while a marker exists, server side) are covered by tests; T-06-63 accepted (staff notes only in the payload of the user's own dashboard); T-06-SC: no dependency change.

## Limits

- The `Xong` / `Trả lại` payload only reflects this task's live run or driver; when another job holds the project, the job manager refuses the press with kind `busy` (409) at start time rather than in the payload.
- The UI (buttons, callout, `… (job {n})` copy) belongs to plan 06-18; this plan only provides the server contract.

## Self-Check: PASSED

- FOUND: agent-brain/tests/human_dashboard.rs, agent-brain/src/dashboard/taskstate.rs, agent-brain/src/dashboard/api_tasks.rs, agent-brain/src/dashboard/pipeline.rs
- FOUND commits: 80f09ed, 7d6dd94, 54e6acd
