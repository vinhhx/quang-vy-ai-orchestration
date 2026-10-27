---
phase: 06-multi-engine-media-branch
plan: 18
subsystem: dashboard-screens
tags: [dashboard, alpine-csp, human-engine, stats, overview, hum-01, hum-02, eng-04, eng-07, set-05]
requires:
  - phase: 06-multi-engine-media-branch
    provides: "06-16 task detail `human` payload, human_done / human_return actions and routes, waiting_human autopilot state, overview human_mode / human_name / quota_detail; 06-17 task screen team / engine / quota slots and the strip exclusions; 06-13 stats engines[] and qa_by_dev_engine[]; 06-08 AB.fmtWait / fmtPercent / engineText / setStaffNames and the cho_nguoi state keys"
provides:
  - "Human step callout on the task screen (waiting / rejected) with Xong and Trả lại, copy buttons and the D-25 push-failure block"
  - "Tổng quan: Đang chạy hint for people, blue cho_nguoi banner with ◐, quota reason line from quota_detail"
  - "Thống kê: Theo engine card and the QA Cần sửa rate by Dev engine card"
  - "Smoke cases task-human, stats, overview-human; js-check sections task-human, stats-engines, overview-human; keys scenario task-human"
affects: [06-19]
tech-stack:
  added: []
  patterns:
    - "The callout reads only the task-detail payload, so Xong / Trả lại work when /pipeline fails"
    - "Rows of the two stats cards are built in getters (engineRows, rateRows) so templates hold no expressions"
    - "The keys scenario puts the smoke repo on the task branch through AB_SMOKE_REPO because Xong is enabled only there, and restores it in a finally"
key-files:
  created: []
  modified:
    - agent-brain/assets/dashboard/screens/task.html
    - agent-brain/assets/dashboard/js/task.js
    - agent-brain/assets/dashboard/screens/overview.html
    - agent-brain/assets/dashboard/js/overview.js
    - agent-brain/assets/dashboard/screens/stats.html
    - agent-brain/assets/dashboard/js/stats.js
    - scripts/dashboard-smoke.sh
    - scripts/dashboard-js-check.mjs
    - scripts/dashboard-keys.mjs
key-decisions:
  - "The callout sits after the pipeline card (before the D-03 callouts) as the plan says, so Tab reaches Xong after the flow region and the disabled strip buttons never take focus first"
  - "Rejected body is composed client-side from report_paths (.qa-report -> `QA kết luận cần sửa.`, .code-review -> `Review code kết luận sửa lại.`) because the 06-16 payload carries no body string"
  - "Copy messages (`Đã sao chép tên nhánh` / `đường dẫn` / `lệnh`) show inside the callout instead of the pipeline-card message line"
  - "The autopilot badge for waiting_human is handled in task.js (app.js is owned by 06-08 and its autoBadge has no such state)"
  - "app.css needed no new class: build-css.sh output is byte-identical, `--check` prints `app.css khớp nguồn`"
requirements-completed: [HUM-01, HUM-02, ENG-04, ENG-07, SET-05]
status: complete
duration: ~75 min
completed: 2026-10-27
commits: 3
plan_head_before: a2ea3690aa51bcf2b30b86bf9024afffba49885d
plan_head_after: 18796bc83169400306806fce42673c02b339ce3c
actuals:
  tokens: 40000
  tasks: 3
  commits: 3
---

# Phase 6 Plan 18: Human callout, Tổng quan delta and Thống kê engine cards Summary

**The task screen now shows who works on a step, for how long, the branch and the exact push command when the push failed, and lets the user pull their work in (`Xong`) or hand it back (`Trả lại`) with no dialog; Tổng quan shows people at work and quota stops correctly; Thống kê shows tokens, cost and QA Cần sửa rate per engine, with unpriced engines reading `chưa có giá` instead of `≈ $0`.**

Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6).

## What was built

**Task 1 (tracer) `765129e`.** `task.js` gains the human getters (`hasHuman`, `humanWaiting`, `humanRejected`, `humanHeading`, `humanBody`, `humanWaitText`, `humanContact`, `humanPushFailed`, `humanReportPaths`, `humanDoneAction`, `humanReturnAction`, `humanButtons`, helpers, tint and glyph), `humanDone()` / `humanReturn()` (POST `/human/done|return` with `{rev}`, `busyAction` disables every task action meanwhile, success and refusal messages per UI-SPEC), `copyBranch()` / `copyReportPaths()` / `copyPushCommand()`, `syncHumanName()` (feeds `AB.setStaffNames` from the payload so engine texts read `người Hà`), the `waiting_human` autopilot badge (`Autopilot: chờ Hà làm`, `cho_nguoi`) and the Xong / Trả lại primary slot below Duyệt plan, the quota re-run and Chạy thêm 1 vòng. `task.html` has the `data-human-callout` (`role="status"`) block with heading row, body, branch block + `Sao chép tên nhánh`, contact line, push-failure block, report block, copy message, wait time and the button with its helper. The smoke gets fixture task 4 (`doi-nguoi`, Hà waiting 3 days 2 hours, push ok) and task 5 (Dev runs on claude, deepseek and a person, each followed by a QA verdict), and the case `task-human` (`OK task-human`).

**Task 2 `9884fb6`.** `overview.js` / `overview.html`: hint `Agent hoặc nhân sự đang làm việc`, `bannerClass` / `bannerHasGlyph` / `bannerGlyph` methods (blue `cho_nguoi` text tint and ◐ without pulse for `human_mode == "waiting"`), reason line from `stop_detail` (autopilot) or `quota_detail` (manual quota stop in Chờ bạn). `stats.js` / `stats.html`: `engineRows` (bar = share of the largest in + out, `vào {X} · ra {Y}`, `≈ $C · n run` or `chưa có giá · n run`, `người` row without bar: `không có token` / `{n} lượt do người làm`, cache figures in `title`), `rateRows` / `hasRates` / `noRates` (`33%`, `{k} / {n} lần QA`, bar width = rate), `barStyle`, and the two cards after `Theo vai trò` with titles, helpers, footnote and the empty QA line. Smoke: DOM assertions for `stats` (`OK stats-engines`) and `overview` (`OK overview-human`) plus a retry for the stats virtual-time race.

**Task 3 `18796bc`.** js-check sections `task-human`, `stats-engines`, `overview-human` (115 new checks, 1284 total): headings (waiting, second round, fix step), bodies, `Đã chờ 3 ngày 2 giờ.`, wait counting between polls, tints, glyphs, primary order, in-flight labels and all-actions-disabled, refusal under the button, POST bodies, no dialogs, push failure, copy with and without clipboard, badge, staff names, engine and rate rows (33 %, 0 %, 100 %, unpriced, person), empty and error states, filter change keeping figures, overview hint, banner tint, reason line. Keys scenario `task-human` (Tab from the flow region reaches `Xong` without landing in the strip; Enter on `Sao chép tên nhánh` shows `Đã sao chép tên nhánh`; task 4 unchanged afterwards). `build-css.sh` left `app.css` byte-identical.

## Verification (all re-run on the final tree)

- `bash scripts/dashboard-smoke.sh overview assign task task-stop task-quota task-human stats projects board settings --keys` passes (`OK overview-human`, `OK task-human`, `OK stats-engines`, keys `OK task-human` among 15 scenarios).
- `bash scripts/dashboard-ui-lint.sh` (113 interactive tags), `node scripts/dashboard-js-check.mjs` (1284 checks), `bash scripts/build-css.sh --check` (`app.css khớp nguồn`), `cargo test --test dashboard_assets` (6 passed).
- Banned-class / `x-html` / inline-style grep over the three screens and their JS is clean; no dialog markup for Xong / Trả lại; `data-human-callout` appears once in task.html; `/human/done` and `/human/return` appear in task.js; `name: 'task-human' | 'stats-engines' | 'overview-human'` count is 3.
- Smokes ran only against sandbox configs built by the script in a temp dir; the serve on 127.0.0.1:7437 was never touched. `agent-brain/tools/tailwindcss` was symlinked (gitignored) to the main checkout copy; build-css.sh verified its sha256. The full Rust suite was not re-run: no Rust file changed in this plan.

## Deviations from Plan

**1. [Rule 3 - Blocking] Smoke fixture task 5 added for the stats cards.**
- The plan says to give "the stats fixtures" runs with the three engines and QA verdicts. Putting them on tasks 1-3 would change the existing pipeline / ledger assertions of those tasks, and on task 4 would muddy the human-wait fixture, so a fifth task (`do-thong-ke`) carries them. Deepseek shows 3 runs because task 3's quota run also used deepseek.

**2. [Rule 3 - Blocking] The keys scenario switches the smoke repo's branch.**
- `human_done` is enabled only when the repo is on the task branch (06-16), and a disabled button cannot take focus. The scenario runs `git checkout task/nguoi-lam` through `AB_SMOKE_REPO` (exported by the smoke) and restores `task/sua-app` in a `finally`. It never presses Xong or Trả lại. The plain `task-human` smoke case runs on `task/sua-app` and asserts the server's off-branch reason under Xong instead.

**3. [Rule 2 - Missing critical] Rejected body derived from `report_paths`.**
- UI-SPEC calls the rejected body a "server string", but the 06-16 payload has no such field. The text is built client-side from the attached report (`.qa-report.md` / `.code-review.md`) with a neutral fallback when neither is present.

**4. [Planner-default copy] `Sao chép lệnh` message.** UI-SPEC gives no message for the push-command copy button; it reads `Đã sao chép lệnh`.

**5. [Structure] Callout placement vs. tab order.** The callout is after the pipeline card (as the plan's must-haves say), and the strip lives inside the pipeline card, so DOM order puts the strip first. While a person works every strip button is disabled (06-16), so Tab from the flow region still reaches Xong first; the keys scenario asserts that no strip control takes focus before it.

**6. [Scope] `app.js` untouched.** The `waiting_human` badge text and key live in task.js (`autoBadge` getter) because app.js is owned by 06-08.

**Total deviations:** 2 blocking fixture / harness adjustments, 1 missing-critical, 3 copy / structure notes. **Impact:** none on the plan's must-haves.

## Not exercised here (needs a person)

- UI-SPEC backstops that need a real browser look: contrast of the amber / blue / dashed words in light and dark themes, `Ế/Ộ/Ữ` names not clipped with `leading-normal`, and the `w-36` engine line wrapping at 1024 px with a long name. The classes follow the recipes and js-check keeps long names whole, but no screenshot review was done.
- `Xong` / `Trả lại` success paths are asserted in js-check (request bodies, messages) and by the Rust tests of 06-16; no browser run presses them, because that would start a real `human done|return` job.
- The `ring-rose-600` / `dark:ring-rose-400` invalid-field ring named in UI-SPEC is used by the Settings editors (06-14), not by these screens; `app.css` already matches the source.

## Known Stubs

None.

## Threat Flags

None beyond the plan's threat model. T-06-67: every new string (names, notes, branch, report paths, push command, server reasons) renders through `x-text`; no `x-html`, no inline style, CSP unchanged (grep-checked). T-06-68: the command shown is the server's `human.push.command`, copied verbatim. T-06-69: unpriced engines read `chưa có giá` (js-check and smoke assert no `≈ $0`). T-06-SC: no dependency added; Cargo files untouched.

## Self-Check: PASSED

- FOUND: agent-brain/assets/dashboard/screens/task.html, js/task.js, screens/overview.html, js/overview.js, screens/stats.html, js/stats.js, scripts/dashboard-smoke.sh, scripts/dashboard-js-check.mjs, scripts/dashboard-keys.mjs
- FOUND commits: 765129e, 9884fb6, 18796bc (measured count since `a2ea369`: 3)
