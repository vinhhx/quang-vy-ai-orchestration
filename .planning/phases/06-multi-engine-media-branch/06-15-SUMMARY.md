---
phase: 06-multi-engine-media-branch
plan: 15
subsystem: ui
tags: [alpine, dashboard, giao-viet, teams, axum]

requires:
  - phase: 06-multi-engine-media-branch
    provides: "06-06 GET /api/teams, /api/engines, /api/staff; 06-08 AB.engineText / AB.setStaffNames and the settings fixtures; 06-10 POST /api/tasks accepts team"
provides:
  - "GET /api/teams field project_defaults {team, effective, note} (same shape as /api/workflows)"
  - "Giao việc: Đội hình radio fieldset under Quy trình, default = project team, team sent in POST /api/tasks"
  - "Every picker state: loading, failed fetch, invalid teams.json, not-ready engine warning, human-role helper, project default note"
affects: [06-16, 06-17, 06-18, 06-19, telegram, task-screen]

actuals:
  tokens: 9500
  tasks: 2
  commits: 2
plan_head_before: 2f330a266f3b0737b6996887967675f0af628379
plan_head_after: f5453f82e95c2c9e5bd43f03aeeaa9d1980257fd

tech-stack:
  added: []
  patterns:
    - "Second radio fieldset copied from the workflow picker (teamTouched re-selection, no new disabled reason)"
    - "Readiness and staff calls are best-effort: a failure only hides the warnings"

key-files:
  created: []
  modified:
    - agent-brain/src/dashboard/api_settings.rs
    - agent-brain/tests/dashboard_settings.rs
    - agent-brain/assets/dashboard/screens/assign.html
    - agent-brain/assets/dashboard/js/assign.js
    - scripts/dashboard-smoke.sh
    - scripts/dashboard-js-check.mjs

key-decisions:
  - "Project default team comes from the teams payload (project_defaults), so /api/projects is unchanged"
  - "The smoke project gets team doi-thu only after the CLI seeding, so seeded tasks keep running on the default team"
  - "Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6)."

patterns-established:
  - "teamsLoaded gates the request field: team is omitted while loading or after a failed fetch (server default applies)"

requirements-completed: [TEAM-03, SET-05]

coverage:
  - id: D1
    description: "GET /api/teams returns project_defaults with team, effective and note (unknown team falls back to mac-dinh with the server note)"
    requirement: TEAM-03
    verification:
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#teams_project_defaults"
        status: pass
    human_judgment: false
  - id: D2
    description: "Giao việc shows the Đội hình fieldset under Quy trình with options, chips, descriptions, summaries; default is the project's team; teamTouched keeps a manual pick; request carries team"
    requirement: TEAM-03
    verification:
      - kind: unit
        ref: "scripts/dashboard-js-check.mjs section assign-team"
        status: pass
      - kind: automated_ui
        ref: "bash scripts/dashboard-smoke.sh assign settings (OK assign-team)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Picker states (loading, fetch failure, invalid teams.json with max 10 lines, not-ready engine callout, human helper, project default note) and no new disabled reason on Giao việc"
    requirement: SET-05
    verification:
      - kind: unit
        ref: "scripts/dashboard-js-check.mjs section assign-team"
        status: pass
      - kind: other
        ref: "bash scripts/dashboard-ui-lint.sh; bash scripts/build-css.sh --check"
        status: pass
    human_judgment: false
  - id: D4
    description: "Giao việc with ten teams grows with the page, no inner scroll or sideways scroll (UI-SPEC backstop)"
    requirement: SET-05
    verification: []
    human_judgment: true
    rationale: "Browser layout backstop from the plan (verification: backstop); no automated ten-team layout check exists in the smoke or keys scripts"

duration: 35min
completed: 2026-10-27
status: complete
---

# Phase 6 Plan 15: Giao việc team picker Summary

**Giao việc gets an Đội hình radio list under Quy trình, defaulting to the project's team from a new `project_defaults` field of `GET /api/teams`, with readiness and human-role warnings, and sends `team` to `POST /api/tasks`.**

Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6).

## Performance

- **Duration:** about 35 min
- **Tasks:** 2 (1 tracer, 1 auto)
- **Files modified:** 6
- **Diff:** 603 insertions, 1 deletion

## Accomplishments

- `teams_json` adds `project_defaults` per project (raw `team`, `effective` via `teams::resolve`, `note`), mirroring `workflows_json`; test `teams_project_defaults` covers a valid team, an unknown team (note, falls back to `mac-dinh`) and no team.
- assign.js: `loadTeams()` (teams, then engines and staff for readiness words and names), `defaultTeamFor`, `pickTeam` / `onTeamChange` (`teamTouched`), `selectDefaultTeam` on project choose, option getters, readiness lines, warning callout, human and note helper lines, `postBody.team` only when the list loaded.
- assign.html: `Đội hình` fieldset directly after `Quy trình` and before the slug, same radio-row recipe, `data-team-option` on each row; every state from the UI-SPEC.
- Smoke prints `OK assign-team`; JS-check section `assign-team` (49 checks) covers default, re-selection, request field, failed load, invalid file, readiness, human helper, note and unchanged disabled reasons. `app.css` needed no change (`build-css.sh --check` matches).

## Task Commits

1. **Task 1 (tracer): `/api/teams` project defaults, team picker, `team` in the request** - `a230c3a` (feat)
2. **Task 2: picker states, JS-check section, UI gates** - `f5453f8` (feat)

The tracer gate re-ran the full Task 1 verify (settings test, smoke assign settings, UI lint and JS check) before expansion; all passed.

## Files Created/Modified

- `agent-brain/src/dashboard/api_settings.rs` - `project_defaults` in `teams_json`
- `agent-brain/tests/dashboard_settings.rs` - `teams_project_defaults`
- `agent-brain/assets/dashboard/screens/assign.html` - Đội hình fieldset
- `agent-brain/assets/dashboard/js/assign.js` - team state, loaders, getters, `postBody.team`
- `scripts/dashboard-smoke.sh` - project `team` fixture, assign team assertions, `OK assign-team`
- `scripts/dashboard-js-check.mjs` - section `assign-team`

## Decisions Made

- The smoke fixture adds `team: "doi-thu"` to `web-ban-hang` right before `serve` starts, not in the initial map, because the CLI seeding that runs in between would otherwise create the seeded tasks with `doi-thu` (dev on deepseek) and change the task-screen fixtures.
- Readiness lines iterate the fixed engine order (claude, deepseek, agy), deduplicated, and skip human roles.
- The project default note shows under the list only while the selected team equals the project's effective team (same rule as the workflow note).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Plan inconsistency] Smoke cannot assert the `doi-thu` radio checked in a DOM dump**
- **Found during:** Task 1 (smoke `assign` block)
- **Issue:** The plan asks the smoke to assert the `doi-thu` radio is checked. The `assign` smoke is a headless DOM dump of `#/assign` with no project chosen, so the correct default is `mac-dinh` (UI-SPEC: no project means `mac-dinh`). Asserting `doi-thu` would contradict the contract.
- **Fix:** The smoke asserts `mac-dinh` is the single checked team radio, and asserts the project default separately: it reads `GET /api/teams` (with the session cookie) and requires `project_defaults['web-ban-hang']` to be `doi-thu`. The "project default is preselected" behavior is covered by the `assign-team` JS-check section (`choose web` selects `doi-thu`). `dashboard-keys.mjs` is not in the plan's files, so no browser-driven project selection was added.
- **Files modified:** scripts/dashboard-smoke.sh
- **Verification:** `bash scripts/dashboard-smoke.sh assign settings` prints `OK assign-team`
- **Committed in:** a230c3a

**2. [Rule 3 - Blocking] Missing gitignored tailwind binary in the worktree**
- **Found during:** Task 2 (`build-css.sh --check`)
- **Fix:** symlinked `agent-brain/tools/tailwindcss` to the main checkout's copy, as 06-08 did (gitignored, not committed); `build-css.sh` verifies its sha256.

---

**Total deviations:** 2 (1 plan inconsistency, 1 blocking environment)
**Impact on plan:** No scope change; all acceptance criteria and verify commands pass.

## Issues Encountered

None beyond the deviations above. The dev-time `cargo build` and smoke runs used the smoke's own sandbox (temp config, `--port 0`); the user's live config and the running serve on 127.0.0.1:7437 were not touched, and nothing was left running.

## Known Stubs

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `project_defaults` and the picker recipe are ready for the Telegram buttons and the change-team and re-run dialogs that reuse the same option shape.
- The ten-team layout backstop (D4) is a browser check not covered by an automated script; it needs a human or a later layout check.

## Self-Check: PASSED

- Files: all six modified files exist; `agent-brain/assets/dashboard/screens/assign.html` has `name="team"` once, after `name="workflow"`.
- Commits: `a230c3a` and `f5453f8` are ancestors of HEAD.
- Acceptance criteria: `teamTouched` count 4 (>= 2), `project_defaults` count 3 in api_settings.rs, `name: 'assign-team'` count 1, failure copy count 1, `app.css khớp nguồn`.
- Verify: `dashboard_settings` 13 passed including `teams_project_defaults ... ok`; full smoke with `--keys` printed `OK assign-team`, `OK settings-shell` and all key checks; UI lint 105 interactive tags OK; JS check 1063 checks OK; banned-class and `x-html` / inline-style greps found nothing.

---
*Phase: 06-multi-engine-media-branch*
*Completed: 2026-10-27*
