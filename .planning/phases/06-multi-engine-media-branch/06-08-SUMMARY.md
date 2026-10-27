---
phase: 06-multi-engine-media-branch
plan: 08
subsystem: ui
tags: [alpine-csp, tailwind, dashboard, settings, teams, workflows, staff, engines]

requires:
  - phase: 06-multi-engine-media-branch
    provides: "06-06 Settings API (GET /api/teams /api/staff /api/engines /api/workflows, POST save/delete routes)"
provides:
  - "Cài đặt screen (#/settings): teams, workflows, staff editors and read-only engine readiness"
  - "Shared vocabulary for later UI plans: five state keys, AB.fmtWait, AB.engineText, AB.setStaffNames, AB.fmtPercent"
  - "settings smoke case, JS-check settings section, rebuilt app.css"
affects: [06-09, 06-10, 06-11, assign, task, stats, overview]

actuals:
  tokens: 120000
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Alpine CSP: flat draft fields plus kind-parametrised methods (kind = team | workflow | staff); no arrow functions, ternaries, template literals or negation in templates"
    - "Engine rows are built from a field whitelist, so an unexpected payload field (a key value) can never reach the DOM"
    - "Conflict-safe writes: POST {version, create, entry}; 409 conflict reloads the list and keeps the draft; invalid file disables writes"

key-files:
  created:
    - agent-brain/assets/dashboard/screens/settings.html
    - agent-brain/assets/dashboard/js/settings.js
  modified:
    - agent-brain/assets/dashboard/index.html
    - agent-brain/assets/dashboard/js/app.js
    - agent-brain/src/dashboard/assets.rs
    - agent-brain/tests/dashboard_assets.rs
    - agent-brain/assets/dashboard/app.css
    - scripts/dashboard-smoke.sh
    - scripts/dashboard-js-check.mjs

key-decisions:
  - "One editor block per section placed above the list, the edited row hidden, instead of swapping the editor into the row (avoids duplicating the form markup and ids)"
  - "New workflow defaults to stages plan, gate:you, implement, qa (like nhanh), no loops, manual, 3 / 60 / 20; the spec gave only the limit defaults"
  - "Delete dialog for a team also shows a server-derived open-task line (used_by.open_tasks) because a deleted team refuses the next step of open tasks"
  - "settings.js keeps its own engine-name helper; AB.engineText exists for the other screens"

patterns-established:
  - "kind-keyed state prefix map (team / w / s) for generic save, delete, refusal and conflict handling"
  - "data-*-row hooks (team, workflow, staff, engine) and data-settings-status for tests"

requirements-completed: [SET-01, SET-02, SET-03, SET-04, SET-05]

duration: 95min
completed: 2026-10-27
status: complete
---

# Phase 6 Plan 08: Cài đặt screen Summary

**Cài đặt (#/settings) with inline teams, workflows and staff editors on the conflict-safe 06-06 routes, a whitelisted read-only Engine readiness list, and the shared state words and formatters (fmtWait, engineText, setStaffNames, fmtPercent) later screens use.**

Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6).

## Performance

- **Duration:** about 95 min
- **Started:** 2026-10-27T02:20:00Z (approx.)
- **Completed:** 2026-10-27T03:54:00Z
- **Tasks:** 3 (tracer, auto, auto)
- **Files modified:** 9 (2 created)

## Accomplishments

- Sixth nav item `Cài đặt`, route `#/settings` (title `Cài đặt · agent-brain`), assets table entries, sub-nav with `sessionStorage` memory, status line, load / stale / error states.
- Team, workflow and staff editors: exact UI-SPEC copy, client validation mirrored from the server (D-24 human options only in Dev, D-27 maxlength 40), one editor per section, Nhân bản naming, 422 refusal callout (max 10 lines, focus moves), 409 conflict keeps the draft, invalid file disables every write control, three alertdialog delete confirms with destructive confirm.
- Read-only Engine section (four rows, readiness badge, `Kiểm tra lại`), no form control; planted key value test passes.
- app.js: five state keys in all five maps, `AB.fmtWait`, `AB.engineText`, `AB.setStaffNames`, `AB.fmtPercent`; app.css rebuilt and `app.css khớp nguồn`.

## Task Commits

1. **Task 1 (tracer): route, shell, team list** - `bf7486c` (feat)
2. **Task 2: editors, dialogs, states, Engine section, JS-check section** - `121ab09` (feat)
3. **Task 3: shared vocabulary, assets test list, app.css** - `522ad55` (feat)

Executor close-out: SUMMARY commit follows (docs). Measured commits on the branch since the plan base: 3 (`plan_head_before` eeb252e, `plan_head_after` 522ad55 before the docs commit).

## Verification (all re-run after the last task commit)

- `bash scripts/dashboard-smoke.sh overview assign task task-stop stats projects board settings --keys` passes (prints `OK settings-shell`, `Keys OK ...`).
- `bash scripts/dashboard-ui-lint.sh` (104 tags), `bash scripts/build-css.sh --check` (`app.css khớp nguồn`), `node scripts/dashboard-js-check.mjs` (1014 checks, 135 new in `settings`, plus vocab and formatter checks) pass.
- `cargo test --offline --locked --test dashboard_assets --test assets_parity -- --test-threads=1` passes; `git diff 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock` is empty.
- Extra, not committed: a throwaway headless-Chrome run on a private sandbox serve (temp HOME / XDG / DB, never the user's config or serve; both processes stopped by recorded PID) drove create, edit, conflict, copy, delete, workflow, staff, engine and invalid-file flows (121 of 124 checks; the 3 misses were sandbox artefacts: the fake `claude_bin` made deepseek read `thiếu lệnh` instead of `thiếu key`) and confirmed no sideways scroll at 1280 / 768 / 480 px and that `md:grid-cols-2` / `sm:grid-cols-3` collapse to one column.

## Deviations from Plan

**1. [Plan inconsistency] `data-loaded` value** - the plan text says `data-loaded` is `true`, but the existing contract (projects, board, smoke `data-loaded="yes"`) uses `loadedAttr` = `yes` / `no`. Used `yes`; the settings smoke asserts `yes`.

**2. [Plan inconsistency] assets.rs grep count** - acceptance says `grep -c "screens/settings.html\|js/settings.js"` is 2; each Asset entry has the path on a `path:` line and again in `include_bytes!`, so the real count is 4 for the two entries. Both entries exist and the asset tests pass.

**3. [Rule 3 - Blocking] tailwind binary missing in the worktree** - `agent-brain/tools/tailwindcss` is gitignored and absent from a fresh worktree. Symlinked it to the user's checkout copy (not a download or install); `build-css.sh` verified the pinned sha256. The symlink is ignored by git.

**4. [Rule 2 - Missing critical] Team delete dialog open-task line** - added `{n} task chưa xong đang dùng đội hình này; sau khi xóa, các task đó cần Đổi đội hình trước khi chạy bước kế tiếp.` below the server usage line, because the engine seam refuses a step whose team no longer exists. Wording is mine, not in the UI-SPEC.

**5. [Planner-silent choice] workflow editor defaults and staff file-error callout** - new workflow stages default to plan, gate:you, implement, qa; the staff file-error callout heading is `staff.json có lỗi` (no closing line). Engine fetch failure shows the muted line `Không kiểm tra được. Bấm Kiểm tra lại để thử lại.` (wording not specified).

**Total deviations:** 5 (2 plan inconsistencies, 1 blocking fix, 1 critical addition, 1 gap-fill). **Impact:** none on scope; all acceptance criteria met in substance.

## Issues Encountered

- Early shell commands were refused by the worktree guard when a heredoc terminator appeared inside the heredoc body or a `bash -c` string; solved by writing helper scripts through the Write tool and running plain commands.

## Known Stubs

None. Intermediate state noted: between commits 2 and 3 the engine badges used the neutral fallback class until the new state keys landed; final tree is complete.

## Threat Flags

None. No new endpoints or trust boundaries; T-06-28 (x-text only, lint grep clean), T-06-29 (field whitelist plus planted-key check), T-06-30 (conflict keeps draft, invalid file blocks writes) and T-06-SC (no dependency added, Cargo files unchanged) are mitigated as planned.

## User Setup Required

None.

## Next Phase Readiness

- Later UI plans can use `AB.fmtWait`, `AB.engineText`, `AB.setStaffNames` (settings.js already calls it after loading staff) and `AB.fmtPercent`, and the five state keys.
- Backstops for the end-of-phase browser check: header nav wrap with six items at 768 / 480 px (screenshots looked fine), dark-mode contrast of the invalid-field ring, leaving `#/settings` with an open draft discards it (no guard, as planned).

## Self-Check: PASSED

Files exist (settings.html, settings.js, this summary); commits `bf7486c`, `121ab09`, `522ad55` are ancestors of HEAD.
