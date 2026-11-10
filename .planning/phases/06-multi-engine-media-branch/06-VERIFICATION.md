---
phase: 06-multi-engine-media-branch
verified: 2026-11-10T02:58:00Z
status: gaps_found
score: 11/16 must-haves verified
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
  - "agent-brain/assets/dashboard/js/settings.js"
  - "agent-brain/assets/dashboard/screens/settings.html"
  - "agent-brain/src/autopilot/mod.rs"
  - "agent-brain/src/config.rs"
  - "agent-brain/src/dashboard/api_settings.rs"
  - "agent-brain/src/dashboard/api_stats.rs"
  - "agent-brain/src/dashboard/api_tasks.rs"
  - "agent-brain/src/dashboard/settings_store.rs"
  - "agent-brain/src/db.rs"
  - "agent-brain/src/dsh.rs"
  - "agent-brain/src/engine.rs"
  - "agent-brain/src/human.rs"
  - "agent-brain/src/pricing.rs"
  - "agent-brain/src/quota.rs"
  - "agent-brain/src/roles/dev.rs"
  - "agent-brain/src/roles/mod.rs"
  - "agent-brain/src/runner.rs"
  - "agent-brain/src/secrets.rs"
  - "agent-brain/src/staff.rs"
  - "agent-brain/src/stats.rs"
  - "agent-brain/src/teams.rs"
  - "agent-brain/src/telegram/callback.rs"
  - "docs/config.engines.example.json"
  - "scripts/phase6-live-check.sh"
  - "spikes/e09/run.sh"
covered_digest: "v3:sha256:1c5f371826e2759e4f7a6e11705700bc5898472c361da66312b0e6d6335b5486"
behavior_unverified: 0
overrides_applied: 0
gaps:
  - truth: "HUM-02: pressing Xong fast-forwards the task branch into the worktree and continues the workflow; a failure part-way can be retried"
    status: partial
    reason: "CR-01 (06-REVIEW.md, confirmed by reading human.rs:357-427). done() runs `git merge --ff-only FETCH_HEAD` BEFORE the fallible work (diff_names, insert_human_run, the set_task_team_cas compare-and-swap) and before the DB transaction. If any of those fail, or the process is killed, the local branch is already advanced but the wait marker stays in team_json and no human run exists. On the next click `remote_sha == head`, so the retry returns the false error 'Nhánh ... chưa có commit mới nào'. The task cannot leave the waiting state through the dashboard or CLI; the only exits are a manual git reset or editing team_json. The happy path is covered by tests/human_engine.rs and passes."
    artifacts:
      - path: "agent-brain/src/human.rs"
        issue: "merge (line 367) happens before the DB commit (lines 416-427); retry check at lines 357-361 treats an already-merged branch as 'no new commit'"
    missing:
      - "Make done() idempotent: compare against p.wait.base and skip the merge/fetch-failure branch when the remote tip is already an ancestor of HEAD and HEAD != base (fix sketched in 06-REVIEW.md CR-01)"
      - "A test that makes the post-merge step fail (or pre-merges the branch) and then retries Xong successfully"
  - truth: "D-40 / ENG-11: an anthropic endpoint without E-09 evidence is not blocked (it runs and is marked chưa kiểm), and the same rule holds at every bind point including the quota re-run"
    status: partial
    reason: "WR-04 (06-REVIEW.md, confirmed by reading engine.rs:564-573 and teams.rs TeamReadiness::refusal). plan_for and the run header run an unverified endpoint (unverified: true). TeamReadiness::refusal, project_pin (Engine branch) and departments::member_usable require readiness == san_sang, so chua_kiem is refused there. entry_status reports chua_kiem for every base_url entry without `verified`. The user's real ~/.config/agent-brain/config.json has the `deepseek` engine with a base_url and no `verified`, so after a Claude quota stop the built-in `tiet-kiem` team (Dev on deepseek) is not offered by the quota team switch and quota_rerun refuses it, while assigning the same team at task creation runs. No test pins this inconsistency in either direction (grep of task_team / telegram_quota / engine_guard finds no chua_kiem case)."
    artifacts:
      - path: "agent-brain/src/teams.rs"
        issue: "TeamReadiness::refusal rejects every readiness other than SanSang, including ChuaKiem"
      - path: "agent-brain/src/engine.rs"
        issue: "project_pin Engine branch requires SanSang; plan_for accepts ChuaKiem"
    missing:
      - "Decide the policy once. If D-40 stands, accept SanSang | ChuaKiem in TeamReadiness::refusal, project_pin and member_usable and show the chưa kiểm mark on the buttons; if not, refuse in plan_for too and amend D-40"
      - "A test for quota_rerun / ready_names with a chua_kiem Dev engine"
  - truth: "The engine is never switched silently (goal: không đổi ngầm; D-16): a role whose configured members/engine cannot run stops with a message"
    status: partial
    reason: "WR-03 (06-REVIEW.md, confirmed by reading engine.rs:696-716 and departments.rs:640). On the phong-ban team, when a department has members but none is usable, departments::pick returns Ok(None); resolve_inner has `Ok(None) => {}` and keeps the team's default engine (claude), possibly on the work profile, with no message. The Err arm warns but also proceeds on claude. Every other bind point refuses this situation through TeamReadiness::refusal. The core Phase 6 quota path does not switch silently (verified); this is the department team added after Phase 6 (departments are not in any 06-*-PLAN.md), but the code lives in the Phase 6 engine seam and contradicts D-16."
    artifacts:
      - path: "agent-brain/src/engine.rs"
        issue: "Ok(None) => {} falls through to the team's claude engine"
    missing:
      - "Return a BrainError naming the department and role when members exist but pick returns Ok(None) or Err; keep the claude fall-through only when the department has no members"
human_verification:
  - test: "ENG-01 phase gate: run one real task with team tiet-kiem (Lead and QA on claude, Dev on an anthropic endpoint: the company gateway or DeepSeek's official endpoint)"
    expected: "Each run header shows the right `engine ...`; runs.engine and Thống kê split tokens and cost per engine; the gateway/DeepSeek Dev run edits files under tool use; the header carries `chưa kiểm E-09` until evidence exists (LIVE-CHECKS section 8, P6-02)"
    why_human: "Needs the user's key and a real network call. spikes/e09/NOTES.md records `Quyết định: not-run` for vcadm-deepseek and deepseek; 06-01 recorded the skipped D-02 gate; there is no spikes/e09/raw and no anthropic_e09_* fixture. Only fake-engine tests and scripts/phase6-live-check.sh exercised the mixed team."
  - test: "ENG-11 / D-34: run spikes/e09/run.sh per endpoint and per model (gateway first), then add `verified` to the engine entry"
    expected: "a.stream_json_parses, a.tool_use_write, a.result_seen, a.usage_input_gt0, a.usage_output_gt0, a.file_ok, a.key_absent all yes, and b.result_is_error=yes b.api_error_status=401 (06-20 Task 3/4 still open)"
    why_human: "Needs VCADM_GATEWAY_KEY / DEEPSEEK_API_KEY; the decision is the user's"
  - test: "Quota stop on a real claude run and on a real endpoint (P6-03), then re-run on another team from the dashboard"
    expected: "Banner `Hết quota engine X` with `Chạy lại bước này bằng đội hình...`; nothing runs by itself; the new engine continues from .plan.md and the existing diff. The detector was calibrated on assumed shapes (quota.rs header: real Claude/DeepSeek/agy quota results were never captured; research A1-A3, A5)"
    why_human: "Cannot burn real quota in a test; WR-02 false-positive risk is only observable on real transcripts"
  - test: "agy as Dev on a small real task (P6-04)"
    expected: "Only plan files change, or the run reports `agy không sửa file nào...`; permission mode accept-edits as decided at the 06-02 probe"
    why_human: "Needs the user's agy login (the 06-02 probe was run on agy 1.4.7 and its evidence is committed, but the engine path through agent-brain with real agy is unrecorded)"
  - test: "Human Dev through a second clone (P6-05): human run, push, Xong, Trả lại"
    expected: "`Xong` brings the second clone's commits in by fast-forward; scope check as for Dev; rejected result returns to the same person. Include a retry after a forced failure to confirm CR-01 once fixed"
    why_human: "Needs a second clone and a real remote"
  - test: "Key sweep over every key_env on real runs (P6-06)"
    expected: "Only `yes` lines; no key in stream.jsonl, events.raw_json, stderr.txt, runs.error, committed files"
    why_human: "Needs real keys; the automated sweep used planted keys"
  - test: "Settings, Giao việc, task and Thống kê in light and dark themes at 1024 / 768 / 480 px (P6-07, SET-05)"
    expected: "Engine rows match config order with honest readiness; no clipped text; contrast acceptable. 06-UI-REVIEW.md scored 15/24 (banned off-scale classes: text-xs x44, font-medium x8, gap-3/p-3/px-3/min-h-10; settings 'Ghi chú prompt' section, assign.html, task.html) and dashboard-ui-lint.sh does not enforce the banned list"
    why_human: "Visual judgement; the only recorded approval is the user's `ok` to the UI-SPEC delta copy (06-24 Task 2), not a look at the built screen"
  - test: "dsh as a black box on a scratch personal repo with Kill while running (P6-09)"
    expected: "No `ab-dsh-` process left in the container, changed files root-owned as documented, git still works. The kill-wrapper probe is recorded ok (spikes/s6/raw/s6-kill-wrapper.txt, A18); --uid-probe (A19) is not recorded"
    why_human: "Needs the real container"
  - test: "claude-max refused on a work project with the SPK-10 text and running on a personal project (P6-10)"
    expected: "Refusal text `chỉ dùng cho dự án profile personal`; personal run uses ~/.claude-max (D-45 relaxes this for claude_config_dir entries, pin_profile entries still apply)"
    why_human: "Needs real profiles"
  - test: "Telegram quota buttons with the second confirm tap (P6-08, optional)"
    expected: "Pick a team, second tap confirms, then the step re-runs through the shared action"
    why_human: "Needs the real bot"
  - test: "Keyboard Tab order on the task screen (scripts/dashboard-keys.mjs step gate-buttons)"
    expected: "From [data-flow-region] one Tab reaches #ab-act-approve. Red with and without the Phase 6 change; a Phase 5 keyboard check, not a Phase 6 requirement. Escalated in 06-VALIDATION.md; stale expectation vs real regression undecided"
    why_human: "Decide whether to fix the script or the layout (suspect commit a63fc17 queue sidebar)"
---

# Phase 6: Multi-engine Teams (Media moved to 06.1) Verification Report

**Phase Goal:** Mỗi vai trò (Lead/Dev/QA và hai vai trò review) chạy được bằng engine khác nhau (claude theo profile dự án, DeepSeek, Antigravity, nhân sự) qua đội hình đặt tên, cấu hình trên màn Cài đặt; hết quota thì dừng chờ người dùng và đổi được đội hình, không đổi ngầm. (Nhánh Media chuyển sang Phase 06.1.)
**Verified:** 2026-11-10T02:58:00Z
**Status:** gaps_found
**Re-verification:** No, initial verification

## Summary

The engine, team, quota, secret-hygiene, agy, dsh, human-engine, stats and Settings machinery exists, is wired and is covered by automated tests (orchestrator evidence: full Rust suite 83 suites, 1704 passed, 0 failed at 6d172c0; JS check 1468; ui-lint, build-css and settings smoke green). I re-checked the code behind the main truths rather than the SUMMARYs, and re-ran the module-filtered lib tests `quota::`, `teams::` and `human::` (35 passed, 0 failed).

Three confirmed defects keep the status at `gaps_found`: CR-01 (human `Xong` not idempotent), WR-04 (a `chưa kiểm` engine runs but is refused at the quota re-run, which hits the user's own `tiet-kiem` team) and WR-03 (silent fall-through to claude on the department team). Independently of those, the phase gate itself (ENG-01: a real mixed-engine task) and every real-engine check were never run, so those items need the user.

## Goal Achievement

### Observable Truths

Roadmap Success Criteria 1, 2, 3 and 5 (criterion 4 moved to Phase 06.1), plus the plan-level truths for ENG-08..11, TEAM, SET, HUM and QUOTA.

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SC1: a `tiet-kiem` task runs Lead and QA on claude and Dev on DeepSeek/gateway, the log names the right engine, stats split tokens per engine (one real mixed task end to end) | ? UNCERTAIN | Wired and tested with fakes: `engine::plan_for` (anthropic branch sets `ANTHROPIC_BASE_URL`/`ANTHROPIC_AUTH_TOKEN` per run, strips every other key var), `runs.engine`, header `engine ...`, `stats.rs:350 GROUP BY runs.engine`; `scripts/phase6-live-check.sh` asserts Lead/Dev/QA = claude/gw/claude and `/api/stats` rows. No real endpoint run: E-09 is `not-run` for both endpoints, 06-01 gate skipped by the user. Human item 1 |
| 2 | SC2: on quota the run stops with outcome `quota` and exit 5, the autopilot stops `hết quota`, the user re-runs with another team, nothing is switched automatically; the new engine continues from `.plan.md` and the existing diff | ✓ VERIFIED | `quota.rs` detectors (agy, text, stream); `roles/mod.rs:1260` sets `OUTCOME_QUOTA`, `EXIT_QUOTA`; `autopilot/view.rs:14` `StopReason::HetQuota => "hết quota"`; `POST /api/tasks/{id}/quota-rerun` (`api_tasks.rs:563`); `tests/engine_quota.rs`, `telegram_quota.rs`, live check task 2 (rerun on `mac-dinh`, one `team_history` entry, no run in between). Caveats: detector never calibrated on a real stop (human item 3) and WR-02 false positives |
| 3 | SC3: the key of every engine never appears in committed files, `stream.jsonl`, `events.raw_json`, `stderr.txt`, `runs.error`, job output | ✓ VERIFIED | `secrets.rs` (`redact_line`, `redact_file`), per-run `EnginePlan.redact`, key read only from env (`anthropic_plan`), `strip_key_envs`, `tests/engine_secrets.rs`, live-check sweep `key_absent[...]=yes` over temp dir, SQL dump and `git log -p`; `spikes/lib/scan-secrets.sh` OK per SUMMARYs. Real-key sweep not run (human item 6) |
| 4 | SC5: Antigravity is a real headless engine per S7 (not manual) and QA's "Cần sửa" rate is recorded per Dev engine | ✓ VERIFIED | Real agy 1.4.7 evidence committed (`spikes/s7b/raw`, decision `--mode accept-edits`); `engine::agy_plan`, `agy_expected_permission_mode`; `tests/engine_agy.rs`; `stats::query_qa_by_dev_engine` (`stats.rs:379`, `GROUP BY dev_engine`) incl. human runs |
| 5 | Each of the five roles resolves its engine from the task's stored team and records it (`runs.engine`, header `engine ...`, `metrics.team`); a team missing from teams.json refuses the step | ✓ VERIFIED | `engine::resolve_inner` (`engine.rs:579-720`): `Team::engine_for(role)`, error `Đội hình '...' không còn trong teams.json`; `db::task_team_json`; `tests/engine_routing.rs`; D-46 (any engine for any role) honoured in `teams.rs:886` |
| 6 | ENG-08/09/10: user-declared engine list (anthropic, agy, dsh; legacy object shape still loads; no route writes command/endpoint/key name), dsh as a container black box killed by marker, profile guard at every bind point | ✓ VERIFIED | `config.rs` engine list, `docs/config.engines.example.json` loaded by a unit test, `dsh.rs` (717 lines, marker + recorded PIDs, no kill by name), `engine::profile_refusal`, `teams::team_profile_refusal`; `tests/engine_dsh.rs`, `engine_guard.rs`, `engine_routing.rs`; kill-wrapper probe recorded ok. Deviation: one write route `POST /api/engines/models` writes only `models` (see Intentional Deviations) |
| 7 | ENG-11: an `anthropic` endpoint without evidence shows `chưa kiểm`, the run is marked, the probe script takes a key variable and endpoint label | ✓ VERIFIED (mechanism) | `entry_status` sets `ChuaKiem`; `EnginePlan.unverified`; `spikes/e09/run.sh` offline self-test; the evidence itself is absent (human item 2). The consistency of the policy across bind points fails, see truth 16 |
| 8 | TEAM-01, SET-02..04: `teams.json`/`staff.json` loaders with size cap and line/column errors, built-ins `mac-dinh`/`tiet-kiem`, Settings CRUD with atomic write, `version` conflict (409), built-ins read-only but copyable, staff in use cannot be deleted | ✓ VERIFIED | `teams.rs`, `staff.rs`, `dashboard/settings_store.rs`, routes `/api/teams`, `/api/teams/save|delete`, `/api/staff...` (`api_settings.rs:24-29`); `tests/dashboard_settings.rs`, `engine_guard.rs`; lib tests `teams::` pass. Caveat WR-06: the engine-models route has no client version and `atomic_write` replaces a symlinked file |
| 9 | TEAM-02/TEAM-03: team stored durably with the task (`tasks.team_json`, schema v4 with backup), change history, change refused while a run or a human step is live; team picked from the project default, Giao việc, Telegram `/task`, CLI `--team` | ✓ VERIFIED | `db.rs:59` migration, `set_task_team_cas`, `tests/db_migration.rs`, `task_team.rs`; schema dry run on the REAL brain.db recorded by the user 2026-10-28: `Schema v4 dry run OK (từ v2)`, `real_dir_unchanged=yes` (06-19 SUMMARY) |
| 10 | SET-01: Cài đặt has an Engine section listing the configured engines with local-only readiness and never shows a key value | ✓ VERIFIED | `get_engines` (`GET /api/engines`), `engine::readiness` (no env value copied), `dashboard_settings::engines_readiness_no_secret`. Section count changed (six sections) by later quick plans, see deviations |
| 11 | SET-05: Settings and the changed screens pass the Phase 5 UI gates and are looked at by the user at 1024/768/480 px | ? UNCERTAIN | ui-lint, `build-css.sh --check`, JS check, settings smoke green (orchestrator). Full `--keys` smoke red on `gate-buttons` (Phase 5 check, escalated). No recorded look at the built screen; 06-UI-REVIEW.md 15/24. Human items 7 and 11 |
| 12 | HUM-01..03: `người:<id>` (Dev only) waits with a pushed branch, `Xong` fast-forwards the branch in and continues, `Trả lại` returns it, the run is recorded with engine `người` and counted in stats | ✗ FAILED (partial) | HUM-01, HUM-03 and the happy path of HUM-02 pass (`tests/human_engine.rs`, `human_dashboard.rs`, stats include human runs). The failure path of `Xong` is not recoverable: CR-01, see gap 1 |
| 13 | QUOTA-01: stopped-on-quota step re-run from the dashboard (banner `Hết quota engine X`, team picker) and Telegram (team buttons + second confirm) through the same action function; `Thử lại sau` keeps the engine | ✓ VERIFIED | `quota_rerun` (`api_tasks.rs:2331`) shared with Telegram `callback.rs`; `tests/telegram_quota.rs`, `dashboard_actions.rs`; live check task 2. Which teams are offered is limited by truth 16 |
| 14 | ENG-04/ENG-07: Thống kê splits tokens and cost per engine id, flags unpriced engines, `usage: none` for dsh; "Cần sửa" by Dev engine | ✓ VERIFIED | `stats.rs:350`, `stats.rs:379-401`, `pricing.rs` engine pricing, `tests/dashboard_stats.rs eng04_*`, `db_stats.rs`; lib test at `stats.rs:701` |
| 15 | The engine is never switched silently (D-16): a role whose configured members cannot run stops with a message | ✗ FAILED (partial) | WR-03: department pick `Ok(None)` falls through to claude with no message (`engine.rs:709`). The quota path is clean. See gap 3 |
| 16 | D-40: an unverified endpoint runs and is marked, consistently at the run seam, project pin, department member and quota re-run | ✗ FAILED (partial) | WR-04: `TeamReadiness::refusal` and `project_pin` require `san_sang`; `plan_for` accepts `chua_kiem`. In the user's real config `deepseek` has no `verified`, so `tiet-kiem` is not offered after a Claude quota stop. See gap 2 |

**Score:** 11/16 truths verified (2 uncertain, 3 failed-partial; 0 present-but-behavior-unverified)

### Intentional Deviations (user decisions after the phase was planned; not gaps)

| Deviation | Source | Effect on the original text |
|-----------|--------|-----------------------------|
| QA may use any engine; only `người` and `dsh` stay Dev-only | D-46 (06-CONTEXT, user 2026-10-29); `.planning/quick/team-any-engine-any-role.plan.md`; `teams.rs:886` | Replaces the ENG-07 "QA must be claude" clause and the QA clause of TEAM-01. REQUIREMENTS.md TEAM-01/ENG-07 and `docs/config.engines.example.json` (`"QA luôn là claude có sẵn (D-39)"`) still carry the old wording and should be updated |
| An engine may carry `claude_config_dir`, not locked to a profile; D-36 applies only to `pin_profile` / `allowed_profiles` | D-45; `.planning/quick/engine-claude-config-dir.plan.md` | Relaxes ENG-10 for `claude_config_dir` entries |
| Opt-in `orchestrator.auto_resume_quota` and `restore_team_after_quota` (default off) | post-phase user decision (quick plans, commit 48fc6b4) | With the flag on, a quota step is retried automatically on the same engine; default off keeps "never auto-switch". Interacts with WR-02 (a false quota would be retried repeatedly) |
| `POST /api/engines/models` writes engine `models` | commit 3e265b3; 06-SECURITY.md AR-06-07 | ENG-08/SET-01 say no route writes engine definitions; this route writes only the model map. Accepted risk, but see WR-06 (no version token) |
| Project type `idea` with a shadow git repo; Telegram plan approval allowed for Telegram tasks; skill mode | D-47, D-48, D-49 | Outside the original Phase 6 requirement text |
| Settings now has six sections and a prompt-notes section | later quick plans (06-UI-REVIEW.md) | Contract said four sections |

No `overrides:` entries were added: each item above is a documented user decision, not a failed must-have.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `agent-brain/src/engine.rs` (2360 lines) | engine catalog, readiness, resolve, plan_for, profile guard | ✓ VERIFIED | Substantive, used by roles seam and dashboards; WR-03/WR-04 defects noted |
| `agent-brain/src/teams.rs` (1328) | teams loader, rules, TeamReadiness | ✓ VERIFIED | lib tests pass; WR-04 in `refusal` |
| `agent-brain/src/quota.rs` (646) | per-engine quota detection | ✓ VERIFIED | wired at `roles/mod.rs:1260`; WR-02 regex breadth |
| `agent-brain/src/human.rs` (528) | human engine wait/done/give_back | ⚠️ PARTIAL | wired; CR-01 in `done()` |
| `agent-brain/src/secrets.rs` (293) | redaction | ✓ VERIFIED | per-run secret list |
| `agent-brain/src/dsh.rs` (717) | docker exec wrapper, marker kill, preflight | ✓ VERIFIED | `tests/engine_dsh.rs`; IN-01/IN-02 minor |
| `agent-brain/src/staff.rs`, `pricing.rs`, `stats.rs` | staff, per-engine pricing and stats | ✓ VERIFIED | |
| `agent-brain/src/dashboard/api_settings.rs`, `settings_store.rs` | write routes with atomic write and version | ✓ VERIFIED | WR-06 caveats |
| `agent-brain/assets/dashboard/js/settings.js`, `screens/settings.html` | Cài đặt screen | ✓ VERIFIED | 2748 / 672 lines, real handlers; UI debt in 06-UI-REVIEW.md |
| `agent-brain/src/db.rs` | schema v4 `tasks.team_json` | ✓ VERIFIED | migration plus real-db dry run |
| `scripts/phase6-live-check.sh`, `spikes/e09/*`, `spikes/s7b/*`, `spikes/s6/*`, `docs/config.engines.example.json` | evidence tooling | ✓ VERIFIED | `spikes/e09/raw` absent by design (E-09 not run) |
| `.planning/phases/06-.../06-UAT.md` | end-of-phase UAT record | ✗ MISSING | not written; real-engine results are unrecorded |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `roles/mod.rs` run seam | `engine::resolve_for_run_with` / `plan_for` | team -> engine per role | WIRED | engine id written to `runs.engine`; unknown id refused before any run row |
| `plan_for` | `secrets::redact_*` | `EnginePlan.redact` | WIRED | every sink scrubbed |
| `roles/mod.rs:1260` | `autopilot` stop `HetQuota` | `OUTCOME_QUOTA`, `EXIT_QUOTA` | WIRED | `decide.rs:313,335` |
| dashboard quota banner / Telegram buttons | `quota_rerun` | shared action | WIRED | `api_tasks.rs:563`, `telegram/callback.rs` |
| Settings routes | `settings_store::atomic_write` + version | write path | WIRED | engines route lacks version (WR-06) |
| `human::done` | DB transaction | merge then commit | PARTIAL | order is wrong, CR-01 |
| `TeamReadiness::refusal` | D-40 policy of `plan_for` | readiness | NOT_CONSISTENT | WR-04 |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| Thống kê engine split | `engines[]` | `stats.rs` SQL `GROUP BY runs.engine` | Yes | FLOWING |
| Settings Engine rows | `/api/engines` | `engine::readiness(cfg, env)` | Yes (local checks) | FLOWING |
| Quota banner | `quota` object | `metrics_json.quota` written at `roles/mod.rs` | Yes | FLOWING |
| Task team line | `team_json` | `db::task_team_json` | Yes | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| quota, teams, human lib units | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib -- quota:: teams:: human::` | 35 passed, 0 failed | ✓ PASS |
| Full Rust suite (orchestrator, not re-run) | `cargo test ... -- --test-threads=1` at 6d172c0 | 83 suites, 1704 passed, 0 failed | ✓ PASS (accepted) |
| UI gates (orchestrator, not re-run) | `dashboard-js-check.mjs` 1468, `dashboard-ui-lint.sh`, `build-css.sh --check`, `dashboard-smoke.sh settings --keys` DOM | green | ✓ PASS (accepted) |
| Full `dashboard-smoke.sh ... --keys` | (orchestrator) | red at `gate-buttons` | ✗ FAIL, Phase 5 check, escalated |
| CR-01 retry dead-end | read `human.rs:357-427` | merge precedes DB commit; retry hits `remote_sha == head` | ✗ CONFIRMED by inspection (no test exists) |
| WR-04 in the user's config | read `~/.config/agent-brain/config.json` engines (ids/kinds/base_url only) | `deepseek` has base_url and no `verified` | ✗ CONFIRMED by inspection |

### Probe Execution

| Probe | Command | Result | Status |
|-------|---------|--------|--------|
| `spikes/e09/run.sh` offline | not re-run | SUMMARY and 06-20: `E09 offline OK` | not re-run by verifier |
| `spikes/s7b`, `spikes/s6` raw evidence | file presence | `spikes/s7b/raw`, `spikes/s6/raw/s6-kill-wrapper.txt` present | present |
| `scripts/phase6-live-check.sh` | not re-run (orchestrator ran the suite) | SUMMARY: nine `LIVE OK:` lines, three runs | not re-run by verifier |

### Requirements Coverage

All 23 IDs claimed by the 26 PLAN frontmatters were checked against `.planning/REQUIREMENTS.md`: every ID in ENG-01..11, TEAM-01..03, SET-01..05, HUM-01..03 and QUOTA-01 is defined there and mapped to Phase 6 in the traceability table. No orphaned Phase 6 requirement. MEDIA-01..03 are mapped to Phase 06.1 and are out of scope.

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|--------------|-------------|--------|----------|
| ENG-01 | 01, 05, 11, 17, 19 | `tiet-kiem` mixed-engine task, log per engine (phase gate) | ? NEEDS HUMAN | automated with fakes; no real run, E-09 not-run |
| ENG-02 | 07, 19, 25 | quota detected, team change proposed, no silent switch | ✓ SATISFIED | truth 2 (WR-02 caveat) |
| ENG-03 | 07, 19 | new engine continues from `.plan.md`/diff/handover | ✓ SATISFIED | `engine_quota::eng03` |
| ENG-04 | 13, 19, 26 | stats split per engine | ✓ SATISFIED | truth 14 |
| ENG-05 | 01, 19, 21 | no key in files or logs | ✓ SATISFIED | truth 3 |
| ENG-06 | 02, 09, 19 | agy per S7 | ✓ SATISFIED | truth 4 |
| ENG-07 | 03, 13, 18, 19 | "Cần sửa" per engine; QA strong by default | ✓ SATISFIED (as amended by D-46) | `stats.rs:379`; default team QA is claude |
| ENG-08 | 11, 19, 22, 26 | user-declared engine list | ✓ SATISFIED | truth 6; engines/models route deviation |
| ENG-09 | 19, 20, 22, 23, 25 | dsh black box | ✓ SATISFIED | truth 6; real-container check outstanding |
| ENG-10 | 11, 19, 22 | profile guard (SPK-10) | ✓ SATISFIED | truth 6; D-45 relaxation |
| ENG-11 | 11, 19, 20, 24, 26 | E-09 evidence per endpoint, `chưa kiểm` | ✗ PARTIAL | mechanism satisfied; evidence absent; policy inconsistent (gap 2) |
| TEAM-01 | 03 | teams.json and built-ins | ✓ SATISFIED (as amended by D-46) | truth 8 |
| TEAM-02 | 04, 10, 17, 19 | durable task team, history | ✓ SATISFIED | truth 9 |
| TEAM-03 | 05, 10, 14, 15 | team selection points | ✓ SATISFIED | truth 9 |
| SET-01 | 06, 08, 19, 24, 26 | Settings Engine section | ✓ SATISFIED | truth 10 |
| SET-02 | 06, 08 | team CRUD | ✓ SATISFIED | truth 8 |
| SET-03 | 06, 08 | workflow CRUD | ✓ SATISFIED | truth 8 |
| SET-04 | 03, 06, 08 | staff | ✓ SATISFIED | truth 8 |
| SET-05 | 08, 15, 17, 18, 19, 24 | UI gates and user look | ? NEEDS HUMAN | truth 11 |
| HUM-01 | 12, 16, 18 | human Dev wait | ✓ SATISFIED | truth 12 |
| HUM-02 | 12, 16, 18 | `Xong` / `Trả lại` | ✗ PARTIAL | CR-01 (gap 1) |
| HUM-03 | 04, 12, 13 | human run recorded | ✓ SATISFIED | truth 12 |
| QUOTA-01 | 10, 14, 17 | quota re-run dashboard and Telegram | ✓ SATISFIED | truth 13 (offer list limited by gap 2) |

Bookkeeping to hand to the orchestrator: every Phase 6 requirement checkbox in `REQUIREMENTS.md` is still `[ ]` and the traceability table still says `Pending`; the ROADMAP Phase 6 block still shows `18/26 plans executed` with 06-20..06-26 unchecked although all have SUMMARYs (06-19 Task 3 and 06-20 Tasks 3-4 are the only open checkpoints). These are not verifier findings against the code.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `agent-brain/src/human.rs` | 367 vs 416-427 | side effect (git merge) before the DB commit | 🛑 Blocker (CR-01, gap 1) | task stuck on a partial failure |
| `agent-brain/src/engine.rs` | 709 | `Ok(None) => {}` silent fall-through to claude | 🛑 Blocker (WR-03, gap 3) | silent engine substitution |
| `agent-brain/src/teams.rs` | 367-377 | readiness policy differs from the run seam | 🛑 Blocker (WR-04, gap 2) | quota re-run refuses an engine that runs |
| `agent-brain/src/runner.rs` | 586, 717 | `rx.recv()` select arm with no `if rx_open` guard | ⚠️ Warning (WR-01) | busy loop after the child closes stdout |
| `agent-brain/src/quota.rs` | 59, 219-239 | bare `rate limit`, `\b429\b`, `\b402\b`, `quota`, `credits` on model text | ⚠️ Warning (WR-02) | ordinary failures become `hết quota` stops; amplified by `auto_resume_quota` |
| `agent-brain/src/stats.rs` | 95-109 | unchecked `*` in `parse_since` | ⚠️ Warning (WR-05) | 500 or empty report on huge `since` |
| `agent-brain/src/dashboard/api_settings.rs`, `settings_store.rs` | 1001, 106 | no version on engine-models save; rename over a symlink | ⚠️ Warning (WR-06) | lost update, symlink replaced |
| UI sources (`settings.html`, `assign.html`, `task.html`) | various | banned off-scale classes, ui-lint does not enforce them | ⚠️ Warning | 06-UI-REVIEW.md 15/24 |
| `docs/config.engines.example.json` | `claude-max` `_note` | stale "QA luôn là claude" after D-46 | ℹ️ Info | docs drift |

No `TBD`/`FIXME`/`XXX` markers in the phase's source files, and no `TODO`/`HACK`/`todo!`/`unimplemented!`. IN-01..IN-06 of 06-REVIEW.md remain `open` (06-REVIEW-DISPOSITION.md: 13 of 13 open); CR-01 and WR-03/04 are carried into the gaps above, the rest stay warnings/info for triage. 06-SECURITY.md reports threats_open 0 with 7 accepted risks (AR-06-04..07 among them).

### Human Verification Required

See the `human_verification` list in the frontmatter (eleven items). The decisive ones are item 1 (real `tiet-kiem` task, the phase gate), item 2 (E-09 per endpoint) and item 3 (a real quota stop). None of P6-02..P6-10 has a recorded result: there is no `06-UAT.md` and LIVE-CHECKS section 8 has no recorded outcomes. Already done and recorded by the user: the agy 1.4.7 probe (06-02), the schema v4 dry run on the real `brain.db` (06-19 Task 3), the kill-wrapper probe (06-20, A18), and approval of the UI-SPEC delta copy (06-24).

### Gaps Summary

Three gaps, each confirmed by reading the code and each with a small fix already described in 06-REVIEW.md:

1. **CR-01, `human done` order of operations (HUM-02).** Merge to git, then write the DB. Make the step idempotent and add a failure-then-retry test.
2. **WR-04, readiness policy (ENG-11 / D-40).** Decide whether `chưa kiểm` endpoints run everywhere or nowhere. Today the run seam accepts them and the quota re-run, project pin and department member checks refuse them, which blocks the `tiet-kiem` re-run in the user's own configuration.
3. **WR-03, silent claude fallback on the department team (D-16).** A department with members but no usable member should stop with a message, not run on claude.

WR-01, WR-02, WR-05, WR-06, IN-01..IN-06 and the UI-review findings are not blockers but should be triaged; WR-02 matters more once `auto_resume_quota` is switched on. After closing the three gaps the phase moves to `human_needed` until the user runs the real-engine checks above. The Phase 5 `gate-buttons` keyboard failure is outside Phase 6 and is tracked in 06-VALIDATION.md.

---

_Verified: 2026-11-10T02:58:00Z_
_Verifier: Claude (gsd-verifier)_
