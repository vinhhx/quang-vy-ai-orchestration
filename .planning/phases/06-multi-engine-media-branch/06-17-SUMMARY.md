---
phase: 06-multi-engine-media-branch
plan: 17
subsystem: dashboard-task-screen
tags: [dashboard, alpine-csp, teams, engines, quota, ui]
requires:
  - phase: 06-multi-engine-media-branch
    provides: "06-10 task detail team / team_history / quota / change_team / quota_rerun and their routes; 06-13 pipeline engine / engine_planned, ledger engine / human, total.human_runs; 06-15 /api/teams, /api/engines, /api/staff; 06-08 AB.engineText / setStaffNames / fmtWait"
provides:
  - "Team line, history and change-team dialog on the task screen"
  - "Engine line on every stage card, engine in ledger labels, new total row and legend wording"
  - "Quota banner in the stop slot and the team re-run dialog"
  - "Smoke cases task-team and task-quota, keys scenarios change-team and task-quota, js-check section task-team-quota"
affects: [06-18 human callout (leaves the slot below the pipeline card and the human_done / human_return exclusions in place)]
tech-stack:
  added: []
  patterns:
    - "One dialog panel with a mode ('change' | 'rerun') instead of two copies; public method names per plan kept as thin entry points"
    - "Pseudo-action object (quota_later) so the banner buttons render through the existing action template"
key-files:
  created: []
  modified:
    - agent-brain/assets/dashboard/screens/task.html
    - agent-brain/assets/dashboard/js/task.js
    - scripts/dashboard-smoke.sh
    - scripts/dashboard-keys.mjs
    - scripts/dashboard-js-check.mjs
key-decisions:
  - "showBanner now means 'the stop slot shows' (quota or autopilot stop); a new showAutoStop keeps the autopilot-only meaning"
  - "Thử lại sau remembers the quota run id client-side (quotaPostponedRun), so a reload or a newer quota stop shows the full banner with no cleanup code"
  - "app.css needed no new utility class; build-css.sh output is byte-identical"
requirements-completed: [TEAM-02, QUOTA-01, ENG-01, SET-05]
status: complete
duration: ~25 min
completed: 2026-10-27
commits: 3
plan_head_before: 2fcbfc081b77370a2e004859d631536d8a5104ec
plan_head_after: 82280f51d232e0147d8b8d5be7cc1d8af56405e4
actuals:
  tokens: 26680
  tasks: 3
  commits: 3
---

# Phase 6 Plan 17: Team, Engine and Quota on the Task Screen Summary

The task screen now shows the task's team (with history and a server-driven `Đổi đội hình` dialog), the engine of every stage card and ledger row, the new total row and legend wording, and a quota stop banner with a safe team re-run dialog and a no-op `Thử lại sau`.

Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6).

## What was built

**Task 1 (tracer) — `c955445`.** `task.js` getters `teamLineText`, `teamMissing`, `teamHistoryLines`, `teamControls`, and the shared team dialog (`openChangeTeam`, `pickDlgTeam`, `confirmChangeTeam`, `closeChangeTeam`) that fetches `/api/teams`, `/api/engines`, `/api/staff` only when opened and POSTs `{team, rev}` to `/api/tasks/{id}/team`. `task.html` has the `data-team-line` block under the workflow line, the `Lịch sử đổi đội hình` list, the amber removed-team hint and the `role="dialog"` panel. The strip excludes `change_team`, `quota_rerun`, `human_done`, `human_return`. Smoke `task` asserts the line, one history item and the button outside the strip (`OK task-team`); keys scenario `change-team` covers Enter, preselected radio, disabled primary with reason, ArrowDown, Escape and focus return, with no POST.

**Task 2 — `cbced37`.** Engine line per flow card (`sr-only` `Engine: ` prefix, `dự kiến ` when planned), ledger label `Vòng {n} · {kind} · {engine}` through `AB.engineText`, human rows `{AB.fmtWait} · không có token` and the human token line, total `{N} lượt chạy · {M} phút · ${X}` plus the human-runs helper, legend entries 2 and 7 reworded (still seven). Quota banner reuses the stop slot (heading, server detail, continuation note, `Chạy lại bước này bằng đội hình…`, `Thử lại sau`, `Dừng hẳn` only with `auto_end`); the re-run dialog has no preselection, a `Bước {step} sẽ chạy bằng: {engine}` line and the `vẫn là {X}` chip, and POSTs `{team, rev}` to `/api/tasks/{id}/quota-rerun`. Primary order is `Duyệt plan` > quota re-run > `Chạy thêm 1 vòng` > first strip action. Smoke fixture task 3 (`tiet-kiem`, Lead on claude, Dev on deepseek stopped on quota), case `task-quota` (`OK task-quota`), keys scenario `task-quota` (Tab reaches the button, Enter opens with focus on the first radio and nothing checked, Escape returns focus, `Thử lại sau` collapses with no state change, reload shows the full banner).

**Task 3 — `82280f5`.** js-check section `task-team-quota` (106 new checks, 1169 total) covering every string in the must-haves and asserting that `Thử lại sau` makes no API call. `scripts/build-css.sh` left `app.css` byte-identical (no new class needed); `build-css.sh --check` prints `app.css khớp nguồn`.

## Verification (all re-run on the final tree)

- `bash scripts/dashboard-smoke.sh overview assign task task-stop task-quota stats projects board settings --keys` passes (`OK task-team`, `OK task-quota`, `OK change-team`, `OK task-quota` keys).
- `bash scripts/dashboard-ui-lint.sh` (109 interactive tags), `node scripts/dashboard-js-check.mjs` (1169 checks), `bash scripts/build-css.sh --check`, `cargo test --test dashboard_assets -- --test-threads=1` (6 passed).
- Banned-class / `x-html` / inline-style grep over `task.html` and `task.js` is clean.
- Acceptance greps: `data-team-line` 1, `Lịch sử đổi đội hình` 2, `/team'` 1, `/quota-rerun` 2, `engine_planned` 2, `lượt chạy claude` 0 in js, smoke and js-check, `data-legend-item` 7, `đang chạy / chờ người` 1, `name: 'task-team-quota'` 1.
- Smokes ran only against sandbox configs built by the script in a temp dir (own `HOME`, `AGENT_BRAIN_CONFIG`, port 0); the running serve on 127.0.0.1:7437 was never touched. `agent-brain/tools/tailwindcss` was symlinked (gitignored) to the main checkout copy; build-css.sh verified its sha256.

## Deviations from Plan

**1. [Rule 3 - Blocking] Old total-row js-check assertions updated in Task 2, not Task 3.**
- Found during: Task 2. The two `lượt chạy claude` expectations in `dashboard-js-check.mjs` fail the moment `totalText` changes, which would leave the Task 2 commit red.
- Fix: updated both to the new text in the Task 2 commit; Task 3 adds the new section.
- Commit: `cbced37`.

**2. [Scope note] Keys scenario `change-team` committed with Task 1.** The plan lists `dashboard-keys.mjs` under Task 2, but the scenario tests the Task 1 dialog, so it landed with that commit. `task-quota` keys landed with Task 2 as planned.

**3. [Rule 2 - Missing critical] Lazy staff fetch for engine texts.** The plan has only the dialogs fetch `/api/staff`. Without more, a human card or ledger row would read `người ha` until a dialog is opened. `ensureStaffNames()` reads `/api/staff` once, only when a pipeline card or ledger row has a human engine, then calls `AB.setStaffNames` and bumps a counter so the texts re-render. Inert when no human engine exists. Commit `cbced37`.

**4. [Planner-default copy] `by: "cli"` history entries read `bạn qua CLI`.** UI-SPEC names only `dashboard` and `telegram`; the server can also write `cli`.

**5. [Structure] One dialog panel, two modes.** The plan describes two team dialogs; both share one markup block (`teamDlgMode` `change` or `rerun`). Specified method names (`openQuotaRerun`, `pickRerunTeam`, `confirmQuotaRerun`, `closeQuotaRerun`, `rerunOpen`, `teamDlgOpen`) exist as entry points. Titles, bodies, buttons, reasons and focus follow the spec per mode.

**Total deviations:** 3 auto-fixed or added (1 blocking, 1 missing-critical, 1 scope placement), 2 copy / structure notes. **Impact:** none on behaviour or on the plan's must-haves.

## Not exercised here (needs 06-12 / 06-16 or a human)

- Human-engine data (`người:{id}` runs, `cho_nguoi` cards, `human_runs` in the total) is built against the documented contract and covered by js-check fixtures only; no serve fixture has human data, since 06-12 and 06-16 are not merged. `người Hà` and the `3 ngày 2 giờ` row were checked through js-check with a staff map set by hand.
- UI-SPEC backstop: a `w-36` card with `người Nguyễn Văn Á` wrapping without widening at 1024 px was not browser-checked (no human fixture). The engine line has `break-words` and the card keeps `min-w-0`; the existing 1024 / 768 / 480 px layout checks on tasks 1 and 2 still pass.
- The real browser POST of `change_team` and `quota_rerun` success paths was not run against the Rust routes; js-check asserts the request bodies and messages, and the Rust side has its own tests (06-10). Browser-level checks stop at open, disabled reason, Escape and focus return, as those must not write.
- Autopilot plus quota (`Dừng hẳn` present, badge `Autopilot: chờ bạn · hết quota`) is covered in js-check against the server stop word; no serve fixture runs a live autopilot stop on quota.

## Known Stubs

None.

## Threat Flags

None. No new route or trust boundary: the two POST routes already exist (06-10), every new string renders through `x-text`, no `x-html`, no inline style, CSP unchanged; every POST carries `rev`, and `Thử lại sau` sends nothing (asserted in js-check and the keys scenario).

## Self-Check: PASSED

- Files modified exist: `task.js`, `task.html`, `dashboard-smoke.sh`, `dashboard-keys.mjs`, `dashboard-js-check.mjs`.
- Commits reachable from HEAD: `c955445`, `cbced37`, `82280f5` (measured count since `2fcbfc0`: 3).
