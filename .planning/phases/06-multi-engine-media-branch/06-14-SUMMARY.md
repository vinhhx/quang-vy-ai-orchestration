---
phase: 06-multi-engine-media-branch
plan: 14
subsystem: telegram
tags: [telegram, quota, teams, callback-grammar, intake]

requires:
  - phase: 06-multi-engine-media-branch
    provides: "06-07 quota detection (quota::stop_detail, step_name); 06-10 shared api_tasks::quota_rerun / StartRequest.team"
provides:
  - "Telegram quota message with one button per team and a mandatory second confirm tap (D-17, D-28)"
  - "Strict team callback grammar t:/u: plus v:/l: ops"
  - "Team step in Telegram /task (TEAM-03)"
affects: [telegram, dashboard-quota]

plan_head_before: 2f330a266f3b0737b6996887967675f0af628379
plan_head_after: aa9e97fa0899912d5d234b78c20860f0446a419a

actuals:
  tokens: 60000
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Buttons carry indexes into a server-held list plus a 4-hex version of that list; ver and rev re-checked on every press"
    - "Confirmation screen is untracked by the detector; cancel restores and re-tracks the quota message"

key-files:
  created:
    - agent-brain/tests/telegram_quota.rs
  modified:
    - agent-brain/src/telegram/callback.rs
    - agent-brain/src/telegram/detect.rs
    - agent-brain/src/telegram/render.rs
    - agent-brain/src/telegram/bot.rs
    - agent-brain/src/telegram/intake.rs
    - agent-brain/src/telegram/outbox.rs
    - agent-brain/tests/telegram_intake.rs

key-decisions:
  - "Quota event uses the dashboard's own rule (latest run of any role has outcome quota, nothing running, no live driver) so a shown button is never refused by quota_rerun"
  - "The confirmation message is taken out of the detector registry; Không chạy lại re-inserts the quota message when the quota stop still holds"
  - "Team pick/confirm re-check rev, then the team-list version, then the index; a changed list re-renders the buttons instead of acting"

requirements-completed: [QUOTA-01, TEAM-03]

duration: 2h (with one interruption)
completed: 2026-10-27
status: complete
---

# Phase 6 Plan 14: Telegram quota team re-run and /task team step Summary

**A quota stop reaches Telegram as one message with a button per team, re-run only after a second confirming tap through the dashboard's own `quota_rerun`; `/task` gains a team step; all 05.3/05.4 trust rules still hold.**

Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6).

## Accomplishments

- `EventKind::Quota` (key `quota:{run_id}`) wins over the autopilot Stop event and is never reported as RunFailed.
- Quota message `Hết quota engine {X} · task {id} {slug}` with the stop detail, a step line, up to 8 `Đội hình {tên}` buttons and `Thử lại sau`; `Còn {k} đội hình khác — chọn trên dashboard.` when there are more teams.
- First tap on a team only edits the message into `Chạy lại bước {bước} bằng đội hình {tên}?` with `Chạy lại bước này` / `Không chạy lại`; only the second tap calls `api_tasks::quota_rerun(…, ActionSource::Telegram)`, which records team history `by: telegram`.
- Stale team list (`Danh sách đội hình đã đổi — chọn lại.` plus fresh buttons) and stale rev (existing stale handling) never run anything; `Không chạy lại` restores the quota message; `Thử lại sau` writes nothing.
- `/task` team step: `Mặc định ({team dự án})` then up to 8 teams (`iD:{flow}:{index}`); the confirm screen shows `Đội hình: {tên} ({summary})`; the request carries `team: f.team_name()`.
- No `Xong` / `Trả lại` button exists on Telegram.

## Task Commits

1. **Task 1 (tracer): quota message and two-tap re-run** - `98216be` (feat)
2. **Task 2: stale list/rev, cancel, postpone, trust rules** - `6607815` (feat)
3. **Task 3: `/task` team step** - `aa9e97f` (feat)

TDD note: the tracer test file was written first and observed failing on the planned assertion (the bot sent the old `Run lỗi` message instead of the quota message) before implementation. The plan commits test and implementation together per task, so there are no separate `test(06-14)` commits.

## Files Created/Modified

- `agent-brain/src/telegram/callback.rs` - `Op::TeamCancel`/`RetryLater`, `TeamOp`, `TeamCb`, `encode_team`, `decode_team`, `team_list_ver`, `MAX_TEAM_BUTTONS`, `IntakeOp::Team`, unit tests
- `agent-brain/src/telegram/detect.rs` - `EventKind::Quota`, `quota_run`
- `agent-brain/src/telegram/render.rs` - quota text/buttons, confirm and done texts, `team_names`
- `agent-brain/src/telegram/bot.rs` - `on_team`, `on_quota_side`, `show_quota`, `restore_quota`, `press_of`
- `agent-brain/src/telegram/intake.rs` - `Stage::Team`, team fields, `team_screen`, `pick_team`, `team_line`
- `agent-brain/src/telegram/outbox.rs` - one match arm for the new event kind
- `agent-brain/tests/telegram_quota.rs` - 8 tests (new)
- `agent-brain/tests/telegram_intake.rs` - flow helpers tap the default team; 3 new team tests

## Decisions Made

See key-decisions. The detector's quota rule mirrors the dashboard's `quota_run` rather than excluding inspect runs, so Telegram never offers a re-run the shared function would refuse.

## Deviations from Plan

**1. [Rule 3 - Blocking] outbox.rs match arm**
- **Found during:** Task 1
- **Issue:** `kind_word` in `outbox.rs` matches `EventKind` exhaustively; the new variant would not compile. `outbox.rs` is not in the plan's file list.
- **Fix:** added `EventKind::Quota => "hết quota"` (terminal log word only).
- **Files modified:** `agent-brain/src/telegram/outbox.rs`
- **Commit:** `98216be`

**2. [Plan adaptation] Telegram-origin test cannot see `TELEGRAM_APPROVE_REFUSED` on a manual quota task**
- **Found during:** Task 2
- **Issue:** the terminal `run dev` that causes the quota stop already records the plan approval, so a forged `a:` press is answered `Plan hiện tại đã được duyệt …` instead of the gate-time refusal text.
- **Fix:** `quota_rerun_telegram_origin_not_approval` accepts either refusal (alert), asserts `approved_commit` is unchanged by the forged press and by the whole re-run, and that the re-run works. The gate-time `TELEGRAM_APPROVE_REFUSED` text stays asserted by the existing `forged_approve_refused` in `telegram_intake` (still passing).

**3. [Plan adaptation] Detector unit tests**
- The plan lists detect unit tests; `TaskState` cannot be built cheaply in a unit test, so the Quota event, its precedence over the stop, and unchanged RunFailed behavior are covered by the fake-Telegram suites (`telegram_quota`, `telegram_notify::run_failed_notifies`). The autopilot `het_quota` precedence over Stop has no dedicated test.

**Total deviations:** 1 auto-fixed (Rule 3), 2 plan adaptations. **Impact:** none on behavior or scope.

## Issues Encountered

An API session-limit interruption paused the run after Task 1 was written and its tracer passed; work resumed from the intact worktree. One of my own unit tests (`new_ops_decode`) wrongly expected a 3-part `x:` code; fixed in the test.

## Known Stubs

None.

## Threat Flags

None. All new surface is Telegram callback_data, covered by T-06-55..58 (strict decoder, ver and rev re-checked, second tap, existing authz).

## Verification

- `cargo test --test telegram_quota --test telegram_intake --test telegram_actions --test telegram_notify --test telegram_serve --test telegram_outage --test telegram_setup -- --test-threads=1`: all pass (quota 8, intake 37, actions 20, notify 7, serve 11, outage 6, setup 12)
- `cargo test --lib`: 447 passed
- `cargo fmt --check`: clean; `git diff 574b6cd -- Cargo.toml Cargo.lock`: empty

## Self-Check: PASSED

Files exist (`tests/telegram_quota.rs`, `06-14-SUMMARY.md`); commits `98216be`, `6607815`, `aa9e97f` are in HEAD history.
