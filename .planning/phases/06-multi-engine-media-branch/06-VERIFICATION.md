---
phase: 06-multi-engine-media-branch
verified: 2026-11-10T06:30:00Z
status: human_needed
score: 14/16 must-haves verified
covered_files:
  - ".planning/phases/06-multi-engine-media-branch/06-01-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-01-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-02-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-02-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-03-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-03-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-04-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-04-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-05-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-05-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-06-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-06-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-07-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-07-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-08-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-08-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-09-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-09-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-10-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-10-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-11-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-11-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-12-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-12-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-13-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-13-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-14-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-14-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-15-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-15-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-16-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-16-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-17-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-17-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-18-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-18-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-19-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-19-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-20-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-20-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-21-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-21-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-22-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-22-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-23-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-23-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-24-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-24-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-25-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-25-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-26-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-26-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-27-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-27-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-28-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-28-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-29-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-29-SUMMARY.md"
  - ".planning/phases/06-multi-engine-media-branch/06-30-PLAN.md"
  - ".planning/phases/06-multi-engine-media-branch/06-30-SUMMARY.md"
  - "agent-brain/assets/dashboard/js/task.js"
  - "agent-brain/src/dashboard/api_tasks.rs"
  - "agent-brain/src/departments.rs"
  - "agent-brain/src/engine.rs"
  - "agent-brain/src/human.rs"
  - "agent-brain/src/teams.rs"
  - "agent-brain/src/telegram/bot.rs"
  - "agent-brain/src/telegram/render.rs"
  - "agent-brain/src/telegram/summary.rs"
  - "agent-brain/tests/departments.rs"
  - "agent-brain/tests/engine_routing.rs"
  - "agent-brain/tests/human_engine.rs"
  - "agent-brain/tests/task_team.rs"
  - "agent-brain/tests/telegram_quota.rs"
  - "scripts/dashboard-js-check.mjs"
covered_digest: "v3:sha256:f4582249879e1a9a4af877b115187c38512fff0e6128fdcbee172b8afcf39764"
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 11/16
  gaps_closed:
    - "HUM-02 / CR-01: human done is retry-safe after a failure past the merge (06-27)"
    - "D-40 / ENG-11 / WR-04: one readiness policy (chưa kiểm runs, marked) at TeamReadiness::refusal, project_pin, member_usable, and the mark on quota re-run choices (06-28, 06-29)"
    - "D-16 / WR-03: phong-ban stops with a message instead of falling through to claude when a department has members but none usable (06-30)"
  gaps_remaining: []
  regressions: []
advisory:
  - finding: "phong-ban still falls back to claude when departments.json is unreadable (members empty plus loaded.errors) or when config.json cannot be loaded (warn then claude) (06-GAPS-REVIEW WR-01)"
    category: other
    reason: "Residual of D-16 outside the original gap text (the gap asked for the stop when members exist but none usable, which is closed and tested). The 06-30 plan flagged the config-load case as an explicit assumption. A hand-edit typo in departments.json turns phong-ban roles into silent claude runs. Resolved by treating a non-empty loaded.errors with no members as a stop, mirrored in TeamReadiness::refusal"
    evidence_status: "code read (engine.rs resolve_inner has_members computed from loaded.members_of only); no failing test provided"
  - finding: "The chưa kiểm mark on quota re-run choices reads only the team's own engine, so a phong-ban team whose picked department member is on an unverified endpoint is not marked at choice time (06-GAPS-REVIEW WR-02)"
    category: other
    reason: "Display limitation documented in 06-29 flagged_assumptions; the run header and metrics_json.unverified still mark the run once it starts. pick_from does not prefer verified members"
    evidence_status: "code read (teams.rs unverified_engine uses team.engine_for); no failing test provided"
  - finding: "api_departments.rs:184 member_readiness still uses `s.readiness == \"san_sang\"`, so the Phòng ban page shows a chưa kiểm member as not ready while the seam picks it (06-GAPS-REVIEW WR-03)"
    category: other
    reason: "Display only, no run choice is offered there (06-28 flagged_assumptions keep it deliberately so the E-09 reason stays visible). Confirmed by reading line 184"
    evidence_status: "code read; not a bind point"
human_verification:
  - test: "ENG-01 phase gate: run one real task with team tiet-kiem (Lead and QA on claude, Dev on an anthropic endpoint: the company gateway or DeepSeek's official endpoint)"
    expected: "Each run header shows the right `engine ...`; runs.engine and Thống kê split tokens and cost per engine; the gateway/DeepSeek Dev run edits files under tool use; the header carries `chưa kiểm E-09` until evidence exists (LIVE-CHECKS section 8, P6-02)"
    why_human: "Needs the user's key and a real network call. spikes/e09/NOTES.md records `Quyết định: not-run` for vcadm-deepseek and deepseek; 06-01 recorded the skipped D-02 gate; there is no spikes/e09/raw and no anthropic_e09_* fixture. Only fake-engine tests and scripts/phase6-live-check.sh exercised the mixed team."
  - test: "ENG-11 / D-34: run spikes/e09/run.sh per endpoint and per model (gateway first), then add `verified` to the engine entry"
    expected: "a.stream_json_parses, a.tool_use_write, a.result_seen, a.usage_input_gt0, a.usage_output_gt0, a.file_ok, a.key_absent all yes, and b.result_is_error=yes b.api_error_status=401 (06-20 Task 3/4 still open)"
    why_human: "Needs VCADM_GATEWAY_KEY / DEEPSEEK_API_KEY; the decision is the user's"
  - test: "Quota stop on a real claude run and on a real endpoint (P6-03), then re-run on another team from the dashboard; include a `tiet-kiem` re-run whose Dev endpoint is chưa kiểm"
    expected: "Banner `Hết quota engine X` with `Chạy lại bước này bằng đội hình...`; the `tiet-kiem` option reads `Bước Code sẽ chạy bằng: deepseek — chưa kiểm` and is selectable; nothing runs by itself; the new engine continues from .plan.md and the existing diff. The detector was calibrated on assumed shapes (quota.rs header: real Claude/DeepSeek/agy quota results were never captured; research A1-A3, A5)"
    why_human: "Cannot burn real quota in a test; WR-02 false-positive risk is only observable on real transcripts"
  - test: "agy as Dev on a small real task (P6-04)"
    expected: "Only plan files change, or the run reports `agy không sửa file nào...`; permission mode accept-edits as decided at the 06-02 probe"
    why_human: "Needs the user's agy login (the 06-02 probe was run on agy 1.4.7 and its evidence is committed, but the engine path through agent-brain with real agy is unrecorded)"
  - test: "Human Dev through a second clone (P6-05): human run, push, Xong, Trả lại"
    expected: "`Xong` brings the second clone's commits in by fast-forward; scope check as for Dev; rejected result returns to the same person. Retry-after-failure is now covered by tests with a SQLite trigger; confirm once on a real remote that pressing Xong again after an interrupted attempt prints `đã được gộp ở lần bấm Xong trước` and completes"
    why_human: "Needs a second clone and a real remote"
  - test: "Key sweep over every key_env on real runs (P6-06)"
    expected: "Only `yes` lines; no key in stream.jsonl, events.raw_json, stderr.txt, runs.error, committed files"
    why_human: "Needs real keys; the automated sweep used planted keys"
  - test: "Settings, Giao việc, task and Thống kê in light and dark themes at 1024 / 768 / 480 px (P6-07, SET-05)"
    expected: "Engine rows match config order with honest readiness; no clipped text; contrast acceptable; the re-run dialog and Telegram-independent quota banner show the `chưa kiểm` mark legibly. 06-UI-REVIEW.md scored 15/24 (banned off-scale classes) and dashboard-ui-lint.sh does not enforce the banned list"
    why_human: "Visual judgement; the only recorded approval is the user's `ok` to the UI-SPEC delta copy (06-24 Task 2); the 06-29 mark reuses the approved `{id} — {từ}` form with no separate approval"
  - test: "dsh as a black box on a scratch personal repo with Kill while running (P6-09)"
    expected: "No `ab-dsh-` process left in the container, changed files root-owned as documented, git still works. The kill-wrapper probe is recorded ok (spikes/s6/raw/s6-kill-wrapper.txt, A18); --uid-probe (A19) is not recorded"
    why_human: "Needs the real container"
  - test: "claude-max refused on a work project with the SPK-10 text and running on a personal project (P6-10)"
    expected: "Refusal text `chỉ dùng cho dự án profile personal`; personal run uses ~/.claude-max (D-45 relaxes this for claude_config_dir entries, pin_profile entries still apply)"
    why_human: "Needs real profiles"
  - test: "Telegram quota buttons with the second confirm tap (P6-08, optional)"
    expected: "Pick a team, second tap confirms, then the step re-runs through the shared action; a chưa kiểm team's button reads `Đội hình tiet-kiem (deepseek — chưa kiểm)` and the confirm text names `deepseek — chưa kiểm`"
    why_human: "Needs the real bot"
  - test: "Keyboard Tab order on the task screen (scripts/dashboard-keys.mjs step gate-buttons)"
    expected: "From [data-flow-region] one Tab reaches #ab-act-approve. Red with and without the Phase 6 change; a Phase 5 keyboard check, not a Phase 6 requirement. Escalated in 06-VALIDATION.md; stale expectation vs real regression undecided"
    why_human: "Decide whether to fix the script or the layout (suspect commit a63fc17 queue sidebar)"
---

# Phase 6: Multi-engine Teams (Media moved to 06.1) Verification Report

**Phase Goal:** Mỗi vai trò (Lead/Dev/QA và hai vai trò review) chạy được bằng engine khác nhau (claude theo profile dự án, DeepSeek, Antigravity, nhân sự) qua đội hình đặt tên, cấu hình trên màn Cài đặt; hết quota thì dừng chờ người dùng và đổi được đội hình, không đổi ngầm. (Nhánh Media chuyển sang Phase 06.1.)
**Verified:** 2026-11-10T06:30:00Z
**Status:** human_needed
**Re-verification:** Yes, after gap closure (plans 06-27..06-30)

## Summary

All three gaps of the previous report (gaps_found, 11/16) are closed in the code and pinned by tests that I re-ran, not taken from the SUMMARYs. No automated truth fails any more. The status is `human_needed` because the phase gate itself (ENG-01: one real mixed-engine task), the E-09 evidence, a real quota stop, real agy, a real second clone, the real key sweep and a visual look at the built screens have never been done by a person; those eleven items are carried over unchanged (item 5 and item 3 gained a sentence for the new behavior).

Evidence I ran myself on HEAD 6642d8a (no source modified): `--test human_engine` 17 passed, `--test engine_routing` 24, `--test departments` 22, `--test task_team` 19, `--test telegram_quota` 10, `--test engine_guard` 10, lib `engine:: teams:: human::` 41, all with 0 failed; `dashboard-ui-lint.sh` OK (324), `build-css.sh --check` (`app.css khớp nguồn`), `dashboard-js-check.mjs` OK (1469 checks). The orchestrator's full-suite run (1719/0) and `dashboard-smoke.sh task-quota` OK are accepted, not re-run.

## Gap Re-check

| Gap | Previous finding | Code evidence now | Test evidence | Result |
|-----|------------------|-------------------|---------------|--------|
| 1. HUM-02 / CR-01 | `merge --ff-only FETCH_HEAD` ran before the DB commit; a retry hit `remote_sha == head` and said "chưa có commit mới nào" | `human.rs:294 already_merged(base, head, remote)` = `head != base && remote != base && is_ancestor(base, remote) && is_ancestor(remote, head)`; `done()` (304-417) skips the merge when `merged_before`, prints `Nhánh {branch} đã được gộp ở lần bấm Xong trước — ghi nhận tiếp.`; offline path reads `refs/remotes/origin/<branch>` and prints `dùng các commit đã gộp ở lần bấm Xong trước`; the merge now uses the checked sha (`merge --ff-only &remote_sha`, `FETCH_HEAD` read once at 359). Plain local commits are never counted (`remote != base`, ancestry both ways) | `done_retry_after_failure_past_merge` (a SQLite `BEFORE INSERT ON runs WHEN NEW.engine='người'` trigger aborts the run insert after the merge; asserts HEAD = staff tip, marker kept, no run, then second Xong gives exactly one run with `human.head`/`human.base`, marker cleared, `dev_done`, no "chưa có commit mới nào"), `done_retry_offline_after_merge`, `done_retry_takes_newer_push`, `done_local_commit_not_taken_as_staff_work`; 17/17 in human_engine pass, including the old refusals | CLOSED |
| 2. ENG-11 / D-40 / WR-04 | `chưa kiểm` runs at the seam but is refused at quota re-run, project pin and department member | `engine.rs:113 readiness_allows_run` (true for `san_sang` and `chua_kiem` only); used at `teams.rs:365` (refusal), `engine.rs:576` (project_pin Engine branch), `departments.rs:576` (member_usable); `thieu_key`/`thieu_lenh` stay refused; profile guard still evaluated first; `teams.rs:398 unverified_engine`; `api_tasks.rs:1568-2193` `quota_unverified` and `quota.unverified_teams`; `telegram/render.rs:321 team_button_label` (`Đội hình tiet-kiem (deepseek — chưa kiểm)`), `bot.rs:1330` confirm text; `task.js:1039` dialog mark | `quota_rerun_unverified_engine_runs` (the user's real shape: `deepseek` entry without `verified`, key set; ready_teams lists `tiet-kiem`; re-run returns 202, Dev run `engine=deepseek`, `succeeded`, `metrics.unverified=true`, one history entry; planted key absent from task, job, runs), `unverified_endpoint_pinned_runs_marked`, `phong_ban_member_on_unverified_endpoint_is_picked`, `quota_unverified_team_button_marked`, `quota_payload_marks_unverified_team`, lib `readiness_allows_run_policy` and `c3_unverified_engine_*`; the `thiếu key` refusal tests stay green | CLOSED |
| 3. D-16 / WR-03 | `Ok(None) => {}` and the `Err` arm fell through to claude silently | `engine.rs:696-770`: `has_members` from `loaded.members_of(dept)`; `Ok(None)` with members returns `BrainError` `... có thành viên nhưng không ai sẵn sàng cho vai {role} (độ khó ...) — không tự chạy bằng engine khác ...`; `Err(e)` returns `không chọn được thành viên ... — không tự chạy bằng engine khác`; no members keeps the team engine (documented phong-ban rule) | `phong_ban_unusable_members_stop_dev` (no dev run row, fake claude not invoked, exit 1), `phong_ban_department_branches_agree_with_team_readiness` (seam and `TeamReadiness::refusal` agree for dev and qa), existing pick tests green | CLOSED (residual advisory below) |

## Goal Achievement

### Observable Truths

Roadmap Success Criteria 1, 2, 3 and 5 (criterion 4 moved to Phase 06.1), plus the plan-level truths for ENG-08..11, TEAM, SET, HUM and QUOTA. Truths 2-11, 13 and 14 were carried over from the initial verification with a regression check (existence, wiring, listed tests still pass in the targeted runs above); truths 12, 15, 16 were fully re-verified.

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SC1: a `tiet-kiem` task runs Lead and QA on claude and Dev on DeepSeek/gateway, the log names the right engine, stats split tokens per engine (one real mixed task end to end) | ? UNCERTAIN | Wired and tested with fakes (`engine::plan_for`, `runs.engine`, header `engine ...`, `stats.rs GROUP BY runs.engine`, `scripts/phase6-live-check.sh`). No real endpoint run: E-09 is `not-run`, the 06-01 gate was skipped by the user. Human item 1 |
| 2 | SC2: on quota the run stops with outcome `quota` and exit 5, the autopilot stops `hết quota`, the user re-runs with another team, nothing is switched automatically; the new engine continues from `.plan.md` and the existing diff | ✓ VERIFIED | `quota.rs`, `roles/mod.rs` `OUTCOME_QUOTA`/`EXIT_QUOTA`, `quota_rerun` (`api_tasks.rs`), `tests/engine_quota.rs`, `telegram_quota.rs` (10 pass), `task_team.rs` (19 pass). Caveat: detector never calibrated on a real stop (human item 3), WR-02 false positives |
| 3 | SC3: the key of every engine never appears in committed files, `stream.jsonl`, `events.raw_json`, `stderr.txt`, `runs.error`, job output | ✓ VERIFIED | `secrets.rs`, per-run `EnginePlan.redact`, `tests/engine_secrets.rs`; the new quota-rerun and Telegram tests additionally assert a planted DEEPSEEK key is absent from task JSON, job output, runs and bot requests. Real-key sweep not run (human item 6) |
| 4 | SC5: Antigravity is a real headless engine per S7 and QA's "Cần sửa" rate is recorded per Dev engine | ✓ VERIFIED | `engine::agy_plan`, `tests/engine_agy.rs`, `stats::query_qa_by_dev_engine`; real agy 1.4.7 probe evidence committed |
| 5 | Each of the five roles resolves its engine from the task's stored team and records it; a team missing from teams.json refuses the step | ✓ VERIFIED | `engine::resolve_inner`, `tests/engine_routing.rs` (24 pass), D-46 honoured |
| 6 | ENG-08/09/10: user-declared engine list, dsh container black box, profile guard at every bind point | ✓ VERIFIED | `config.rs`, `dsh.rs`, `engine::profile_refusal`, `teams::team_profile_refusal`; `engine_guard` 10 pass; profile guard still runs before readiness at all three bind points (06-28) |
| 7 | ENG-11: an `anthropic` endpoint without evidence shows `chưa kiểm`, the run is marked, the probe script takes a key variable and endpoint label | ✓ VERIFIED (mechanism) | `entry_status` ChuaKiem, `EnginePlan.unverified`, `spikes/e09/run.sh`; policy is now consistent (truth 16). The evidence itself is absent (human item 2) |
| 8 | TEAM-01, SET-02..04: loaders, built-ins, Settings CRUD with atomic write and 409 conflict, staff in use cannot be deleted | ✓ VERIFIED | `teams.rs`, `staff.rs`, `settings_store.rs`, `tests/dashboard_settings.rs`; lib teams tests pass |
| 9 | TEAM-02/TEAM-03: team stored durably with the task (schema v4), history, change refused while a run or human step is live, team picked from the project default, Giao việc, Telegram, CLI | ✓ VERIFIED | `db.rs` migration, `set_task_team_cas`, `task_team.rs` 19 pass; real-db schema dry run recorded 2026-10-28 |
| 10 | SET-01: Cài đặt has an Engine section listing configured engines with local-only readiness and no key value | ✓ VERIFIED | `GET /api/engines`, `engine::readiness`, `dashboard_settings::engines_readiness_no_secret` |
| 11 | SET-05: Settings and the changed screens pass the Phase 5 UI gates and are looked at by the user at 1024/768/480 px | ? UNCERTAIN | ui-lint, build-css check and JS check green (re-run); `task-quota` smoke OK (orchestrator); full `--keys` smoke red on the Phase 5 `gate-buttons` check (escalated); no recorded look at the built screen; 06-UI-REVIEW.md 15/24. Human items 7 and 11 |
| 12 | HUM-01..03: `người:<id>` waits with a pushed branch, `Xong` fast-forwards and continues and a failure part-way can be retried, `Trả lại` returns it, the run is recorded and counted in stats | ✓ VERIFIED | Truth was FAILED (CR-01); now `done()` is retry-safe, see Gap 1; 17/17 human_engine tests pass, `human_dashboard` was green in the orchestrator's full run |
| 13 | QUOTA-01: stopped-on-quota step re-run from the dashboard and Telegram through the same action; `Thử lại sau` keeps the engine | ✓ VERIFIED | `quota_rerun` shared; `telegram_quota` 10 pass; the offered team list now includes `chưa kiểm` teams, marked |
| 14 | ENG-04/ENG-07: Thống kê splits tokens and cost per engine id, flags unpriced engines, "Cần sửa" by Dev engine | ✓ VERIFIED | `stats.rs`, `pricing.rs`, `tests/dashboard_stats.rs` (full suite green) |
| 15 | The engine is never switched silently (D-16): a role whose configured members cannot run stops with a message | ✓ VERIFIED | Was FAILED (WR-03); now Gap 3. Residual: broken departments.json / unloadable config (advisory) |
| 16 | D-40: an unverified endpoint runs and is marked, consistently at the run seam, project pin, department member and quota re-run | ✓ VERIFIED | Was FAILED (WR-04); now Gap 2 |

**Score:** 14/16 truths verified (2 uncertain, both need a person; 0 failed; 0 present-but-behavior-unverified)

### Intentional Deviations (user decisions after the phase was planned; not gaps)

Unchanged from the initial report: D-46 (QA may use any engine; only `người` and `dsh` are Dev-only), D-45 (`claude_config_dir` not locked to a profile), opt-in `auto_resume_quota` / `restore_team_after_quota`, `POST /api/engines/models` writes `models` only, D-47..D-49 (idea projects, Telegram plan approval, skill mode), Settings grown to six sections. No `overrides:` entries were added. D-40 is applied, not amended (06-28 prohibition).

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `agent-brain/src/human.rs` | retry-safe `done()` with `already_merged` | ✓ VERIFIED | contains `fn already_merged`, merges the checked sha; wired from the CLI and dashboard Xong |
| `agent-brain/src/engine.rs` | `readiness_allows_run`, stricter department branch | ✓ VERIFIED | `pub fn readiness_allows_run` (113), `không tự chạy bằng engine khác` (737, 752) |
| `agent-brain/src/teams.rs` | `refusal` on shared predicate, `unverified_engine` | ✓ VERIFIED | lines 365, 398 |
| `agent-brain/src/departments.rs` | `member_usable` on the shared predicate | ✓ VERIFIED | line 576 |
| `agent-brain/src/dashboard/api_tasks.rs` | `quota_unverified`, `quota.unverified_teams` | ✓ VERIFIED | 1568, 1874, 2179-2193 |
| `agent-brain/src/telegram/render.rs`, `bot.rs` | marked team button and confirm text | ✓ VERIFIED | `team_button_label`, `quota_unverified_engine` used at render.rs:567 and bot.rs:1330 |
| `agent-brain/assets/dashboard/js/task.js`, `scripts/dashboard-js-check.mjs` | dialog mark and its check | ✓ VERIFIED | task.js:1039; JS check 1469 OK |
| New tests (`human_engine`, `task_team`, `engine_routing`, `departments`, `telegram_quota`) | named tests exist and pass | ✓ VERIFIED | all named tests present and passing in the targeted runs |
| `.planning/phases/06-.../06-UAT.md` | end-of-phase UAT record | ✗ MISSING | not written; real-engine results unrecorded (routes to human verification, not a code gap) |
| Other artifacts of the initial report | engine/quota/secrets/dsh/stats/settings/db | ✓ VERIFIED | regression-checked by the targeted suites; no change |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `human::done` | `gitops::is_ancestor` / DB transaction | `already_merged`, then `insert_human_run` + `set_task_team_cas` in one tx | WIRED | order unchanged but now safe to repeat |
| `TeamReadiness::refusal` | `engine::readiness_allows_run` | readiness policy | WIRED | teams.rs:365 |
| `project_pin` | `engine::readiness_allows_run` | readiness policy | WIRED | engine.rs:576 |
| `departments::member_usable` | `engine::readiness_allows_run` | readiness policy | WIRED | departments.rs:576 |
| `quota_teams` / `quota_rerun` | `TeamReadiness::ready_names` / `refusal` | unchanged call sites, new policy; refusal re-evaluated at request time | WIRED | `quota_rerun_refuses_team_not_ready` green |
| `api_tasks quota_view` | task.js dialog and Telegram render | `unverified_teams` / `quota_unverified_engine` | WIRED | tests above |
| `resolve_inner` | `departments::pick_from` | `members_of(dept)` decides stop vs fall-through | WIRED | engine.rs:703-770 |
| `roles/mod.rs` seam | `resolve_for_run_with` | `?` stops the step before any run row | WIRED | `phong_ban_unusable_members_stop_dev` asserts no run row |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| Quota dialog / Telegram button mark | `quota.unverified_teams`, `quota_unverified` | `TeamReadiness::unverified_engine` from `engine::readiness` statuses of the real config and env | Yes (asserted in `quota_payload_marks_unverified_team`) | FLOWING |
| Human run row after retry | `metrics.human.head/base` | `head_sha` after merge or already-merged HEAD, `p.wait.base` | Yes (asserted) | FLOWING |
| Other initial-report traces (stats engine split, Settings engines, quota banner, task team line) | | | | FLOWING (unchanged) |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Xong retry after failure past merge, offline retry, newer push, local commit refused | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test human_engine -- --test-threads=1` | 17 passed, 0 failed | ✓ PASS |
| Pinned and department chưa kiểm run marked; phong-ban stops; branches agree | `--test engine_routing` / `--test departments` | 24 / 22 passed | ✓ PASS |
| Quota re-run with chưa kiểm Dev; payload mark | `--test task_team` | 19 passed | ✓ PASS |
| Telegram marked buttons and two-tap confirm | `--test telegram_quota` | 10 passed | ✓ PASS |
| Profile guard still decides first | `--test engine_guard` | 10 passed | ✓ PASS |
| Policy unit tests | `--lib -- engine:: teams:: human::` | 41 passed | ✓ PASS |
| UI gates | `dashboard-ui-lint.sh`, `build-css.sh --check`, `dashboard-js-check.mjs` | OK 324 / `app.css khớp nguồn` / OK 1469 | ✓ PASS |
| Full suite, `dashboard-smoke.sh task-quota` | (orchestrator) | 1719/0; `OK task-quota` | ✓ PASS (accepted, not re-run) |
| Full `dashboard-smoke.sh ... --keys` | (orchestrator, previous run) | red at `gate-buttons` | ✗ Phase 5 check, escalated, unchanged |

### Probe Execution

| Probe | Command | Result | Status |
|-------|---------|--------|--------|
| `spikes/e09/run.sh` offline, `scripts/phase6-live-check.sh` | not re-run | SUMMARY claims only; gap-closure plans did not touch them | not re-run by verifier |

### Requirements Coverage

Every ID declared by the 30 PLAN frontmatters (ENG-01..11, TEAM-01..03, SET-01..05, HUM-01..03, QUOTA-01; the gap plans add no new IDs: HUM-02, ENG-11, ENG-10, QUOTA-01, ENG-02) is defined in `.planning/REQUIREMENTS.md` and mapped to Phase 6. No orphaned Phase 6 requirement; MEDIA-01..03 belong to Phase 06.1. The planner registered no extra HUM-*/SET-*/QUOTA-* IDs beyond those.

| Requirement | Source Plans | Status | Evidence |
|-------------|--------------|--------|----------|
| ENG-01 | 01, 05, 11, 17, 19 | ? NEEDS HUMAN | fakes only; no real mixed run (item 1) |
| ENG-02 | 07, 19, 25, 29, 30 | ✓ SATISFIED | truths 2, 15; no silent switch on quota or department team |
| ENG-03 | 07, 19 | ✓ SATISFIED | `engine_quota::eng03` |
| ENG-04 | 13, 19, 26 | ✓ SATISFIED | truth 14 |
| ENG-05 | 01, 19, 21 | ✓ SATISFIED | truth 3 |
| ENG-06 | 02, 09, 19 | ✓ SATISFIED | truth 4 |
| ENG-07 | 03, 13, 18, 19 | ✓ SATISFIED (as amended by D-46) | `stats.rs` |
| ENG-08 | 11, 19, 22, 26 | ✓ SATISFIED | truth 6 |
| ENG-09 | 19, 20, 22, 23, 25 | ✓ SATISFIED | truth 6; real-container check outstanding (item 8) |
| ENG-10 | 11, 19, 22, 28 | ✓ SATISFIED | truth 6; guard order preserved in 06-28 |
| ENG-11 | 11, 19, 20, 24, 26, 28, 29 | ✓ SATISFIED (mechanism and policy); E-09 evidence is a human item | truths 7, 16 |
| TEAM-01 | 03 | ✓ SATISFIED (as amended by D-46) | truth 8 |
| TEAM-02 | 04, 10, 17, 19 | ✓ SATISFIED | truth 9 |
| TEAM-03 | 05, 10, 14, 15 | ✓ SATISFIED | truth 9 |
| SET-01 | 06, 08, 19, 24, 26 | ✓ SATISFIED | truth 10 |
| SET-02..04 | 03, 06, 08 | ✓ SATISFIED | truth 8 |
| SET-05 | 08, 15, 17, 18, 19, 24 | ? NEEDS HUMAN | truth 11 |
| HUM-01 | 12, 16, 18 | ✓ SATISFIED | truth 12 |
| HUM-02 | 12, 16, 18, 27 | ✓ SATISFIED | truth 12, Gap 1 |
| HUM-03 | 04, 12, 13 | ✓ SATISFIED | truth 12 |
| QUOTA-01 | 10, 14, 17, 29 | ✓ SATISFIED | truth 13 |

Bookkeeping for the orchestrator (not code findings): every Phase 6 requirement checkbox in `REQUIREMENTS.md` is still `[ ]` and the traceability table still says `Pending`; ROADMAP progress for Phase 6 and the new 06-27..06-30 plans should be updated; REQUIREMENTS.md TEAM-01/ENG-07 and SET-01 (four sections) still carry pre-D-46 / pre-six-section wording.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `agent-brain/src/engine.rs` | ~703-770 | phong-ban fall-through to claude when departments.json is unreadable (empty members plus errors) or config.json cannot be loaded | ⚠️ Warning (advisory, WR-01 of 06-GAPS-REVIEW) | silent claude run after a hand-edit typo; outside the closed gap text |
| `agent-brain/src/teams.rs` | ~398-410 | mark reads only the team's own engine, not the picked department member | ⚠️ Warning (advisory, WR-02) | phong-ban unverified member unmarked at choice time |
| `agent-brain/src/dashboard/api_departments.rs` | 184 | `readiness == "san_sang"` literal not moved to `readiness_allows_run` | ⚠️ Warning (advisory, WR-03) | Phòng ban page shows chưa kiểm member as not ready |
| `agent-brain/src/runner.rs`, `quota.rs`, `stats.rs`, `api_settings.rs`, UI sources | | WR-01, WR-02, WR-05, WR-06 and UI-review debt of the initial report | ⚠️ Warning | untouched by the gap plans; triage |
| `agent-brain/src/telegram/render.rs` | ~437 | hidden-teams line says "(engine thiếu key hoặc thiếu lệnh)" but the count also includes profile and department refusals | ℹ️ Info | wording (IN-02 of 06-GAPS-REVIEW) |
| `agent-brain/tests/departments.rs` | end | env vars set without an RAII guard | ℹ️ Info | test hygiene (IN-04) |

No `TBD`/`FIXME`/`XXX` in the gap-closure source files (grep of human, engine, teams, departments, api_tasks, telegram render/bot). 06-GAPS-REVIEW.md: 0 critical, 3 warnings, 4 info; none contradicts a must-have of plans 06-27..06-30 beyond the flagged assumptions.

### Human Verification Required

See the `human_verification` list in the frontmatter (eleven items). The decisive ones are item 1 (real `tiet-kiem` task, the phase gate), item 2 (E-09 per endpoint) and item 3 (a real quota stop and a `chưa kiểm` re-run). None of P6-02..P6-10 has a recorded result; there is no `06-UAT.md`.

### Gaps Summary

No code gaps remain. The three previous gaps (CR-01, WR-04, WR-03) are closed with behavior-level tests that fail without the fixes (a trigger-forced failure after the merge, a real-shape `deepseek` entry without `verified`, a department with only unusable members). Three advisory warnings from the gap-closure review are recorded and may be scheduled as a follow-up plan; none blocks the phase goal. The phase moves to `human_needed`: the user must run the real-engine checks and record the outcome in a `06-UAT.md` before the phase can be called passed.

---

_Verified: 2026-11-10T06:30:00Z_
_Verifier: Claude (gsd-verifier)_
