---
phase: 06-multi-engine-media-branch
plan: 06
subsystem: api
tags: [axum, rust, dashboard, settings, atomic-write, teams, staff, workflows, engines]

requires:
  - phase: 06-multi-engine-media-branch
    provides: "06-03 teams/staff/engine catalog (teams::validate_entry, staff::validate_entry, engine::readiness) and Sandbox helpers sb.write_teams / sb.write_staff / sb.teams_path()"
provides:
  - "dashboard::settings_store: file_version, read_doc, write_object (process-wide lock, version check, atomic 0600 write, directory fsync), WriteError"
  - "dashboard::api_settings: GET /api/teams, /api/staff, /api/engines; POST /api/{teams,workflows,staff}/{save,delete}; teams_json, staff_json"
  - "GET /api/workflows now carries version and per-workflow edit"
  - "workflows::validate_entry pub, workflows::workflows_path(), Workflow::to_file_entry()"
affects: [06-08, 06-15, 06-17, settings-ui, giao-viec-ui]

actuals:
  tokens: 20500
  tasks: 3
  commits: 6
plan_head_before: 66426b30a0e4718b208acf096022bb5529b86174
plan_head_after: 0e30ec8bbf7e6b77fd1bab869d410168ae5217c8

tech-stack:
  added: []
  patterns:
    - "Version-token optimistic concurrency: GET returns an opaque 16-hex hash of the file bytes (or absent); a write re-reads under one process-wide lock and answers 409 on mismatch"
    - "Never overwrite a file that fails to load (409 invalid_file with the loader's own error lines)"
    - "Order-preserving single-key read-modify-write on serde_json preserve_order maps (insert keeps position, shift_remove on delete)"
    - "Entries are validated by the same loader function the runtime uses, then stored in canonical form (Team::to_file_entry / Workflow::to_file_entry)"

key-files:
  created:
    - agent-brain/src/dashboard/settings_store.rs
    - agent-brain/src/dashboard/api_settings.rs
    - agent-brain/tests/dashboard_settings.rs
  modified:
    - agent-brain/src/dashboard/mod.rs
    - agent-brain/src/dashboard/api_workflows.rs
    - agent-brain/src/workflows.rs

key-decisions:
  - "Version is checked right after the invalid-file check and before the name rules, so a stale client gets 409 conflict rather than a 422 computed from content it has not seen; write_object checks it again under the lock"
  - "Saved entries are stored in canonical form (to_file_entry): all five team roles in ROLES order, trimmed description, workflow entry with all six keys and loops always an object; a whole budget_usd is written as an integer"
  - "Refusal field names for the two hyphenated roles are mapped back to the request's underscore names (plan_review, code_review)"
  - "GET /api/staff returns notes as an empty string (not null) when there are none, so text inputs bind directly"
  - "An unreadable (non-UTF-8 or permission) settings file answers 500 with a fixed text on GET and save; the detail goes to the terminal only"

patterns-established:
  - "api_settings::run_write: every POST handler runs its blocking body on the blocking pool and maps BrainError to a fixed 500 text"
  - "workflows_payload(st) in api_workflows is the single builder for both GET /api/workflows and the write responses"

requirements-completed: [SET-01, SET-02, SET-03, SET-04]

coverage:
  - id: D1
    description: "Teams can be read, created, edited, deleted from the dashboard API with the loader's own rules; built-ins read-only, overrides editable; written atomically at 0600 under one lock"
    requirement: "SET-02"
    verification:
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#teams_get_save_get"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#teams_save_rules"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#teams_update_and_override"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#teams_delete_rules"
        status: pass
      - kind: unit
        ref: "cargo test --lib dashboard::settings_store (version_absent_and_hash, write_conflict_detected, write_creates_0600_and_no_tmp)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Conflicts (stale version -> 409 conflict), invalid hand-edited files (409 invalid_file, never overwritten) and untouched entries preserved byte-for-value in order"
    requirement: "SET-02"
    verification:
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#teams_conflict_keeps_file"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#teams_invalid_file_blocks_writes"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#teams_untouched_entries_preserved"
        status: pass
    human_judgment: false
  - id: D3
    description: "Staff CRUD with the usage refusal (a person used by a team cannot be deleted)"
    requirement: "SET-04"
    verification:
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#staff_crud_and_usage"
        status: pass
    human_judgment: false
  - id: D4
    description: "Workflow save/delete through the same write path; GET /api/workflows gains version and per-workflow edit shape; to_file_entry round-trips through validate_entry"
    requirement: "SET-03"
    verification:
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#workflows_get_has_version_and_edit"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#workflows_save_rules"
        status: pass
      - kind: unit
        ref: "cargo test --lib workflows::tests::to_file_entry_round_trips"
        status: pass
    human_judgment: false
  - id: D5
    description: "GET /api/engines lists local readiness only; no engine write route; no key value in any response; settings routes sit behind the token/Host/Origin gate"
    requirement: "SET-01"
    verification:
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#engines_readiness_no_secret"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/dashboard_settings.rs#settings_routes_gated"
        status: pass
    human_judgment: false

duration: 12min
completed: 2026-10-27
status: complete
---

# Phase 6 Plan 06: Settings API Summary

**Safe dashboard write path for teams, workflows and staff (version token, lock, atomic 0600 write plus directory fsync, loader validation, invalid files never overwritten) and a read-only engine readiness list that never exposes a key value.**

Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6).

## Performance

- **Duration:** 12 min
- **Started:** 2026-10-27T03:03:34Z
- **Completed:** 2026-10-27T03:15:15Z
- **Tasks:** 3 (Task 1 tracer, Tasks 2 and 3 auto, all `tdd="true"`)
- **Files modified:** 6 (3 created, 3 modified)

## Accomplishments

- `settings_store` gives one write primitive for all three files: process-wide lock, version re-check right before writing, temp file in the same directory (`create_new`, mode 0600), flush, `sync_all`, rename, directory `sync_all`, temp removed on any failure.
- Teams: `GET /api/teams` (summary, `default_for`, `used_by`, `builtin_names`, `version`), `POST /api/teams/save` (create and update) and `/delete`, with built-ins read-only, overrides editable and deletable, names immutable, QA/human/name rules coming from `teams::validate_entry`, and untouched entries keeping their exact JSON and order.
- Staff: `GET /api/staff` with `used_in`, `POST /api/staff/save` and `/delete`, delete refused while a team uses the person.
- Workflows: `POST /api/workflows/save` and `/delete` validated by `workflows::validate_entry` (messages stripped of `Dòng N: `, field null); `GET /api/workflows` keeps every Phase 5.2 field and adds `version` and `edit`; the existing `dashboard_workflows` suite passes unchanged.
- `GET /api/engines` returns `engine::readiness` of the serve process env; the test greps the raw body for the planted key value and finds nothing; there is no engines write route.

## Task Commits

1. **Task 1 (tracer): settings store and teams read/save** - RED `20226ce` (test), GREEN `2d8c9d8` (feat)
2. **Task 2: team rules, delete, conflicts, staff** - RED `fbebbff` (test), GREEN `d8801c7` (feat)
3. **Task 3: workflow writes, engine readiness, gate** - RED `eddd8ad` (test), GREEN `0e30ec8` (feat)

**Plan metadata:** committed separately as `docs(06-06): complete settings API plan` (SUMMARY.md only; STATE.md and ROADMAP.md are left to the orchestrator).

## Files Created/Modified

- `agent-brain/src/dashboard/settings_store.rs` - `file_version`, `read_doc`, `write_object`, `WriteError`, atomic write; unit tests `version_absent_and_hash`, `write_conflict_detected`, `write_creates_0600_and_no_tmp`
- `agent-brain/src/dashboard/api_settings.rs` - all new routes, payload builders `teams_json` / `staff_json`, shared 422/409/500 shapes, small unit tests
- `agent-brain/src/dashboard/mod.rs` - `pub mod api_settings; pub mod settings_store;` and `.merge(api_settings::routes())` before the fallback, inside `auth::gate`
- `agent-brain/src/dashboard/api_workflows.rs` - `workflows_json(.., version)`, `edit` per workflow, shared `workflows_payload(st)`
- `agent-brain/src/workflows.rs` - `validate_entry` pub, `workflows_path()`, `Workflow::to_file_entry()`, unit test `to_file_entry_round_trips`
- `agent-brain/tests/dashboard_settings.rs` - 12 HTTP tests

## Decisions Made

See `key-decisions` above. Rationale in short: a stale client should learn about the conflict before being told a name clashes with content it never saw; canonical storage keeps the file readable and loader-stable; underscore field names let the UI map errors onto its own form fields.

## TDD Gate Compliance

All three tasks ran RED then GREEN with separate commits (`test(06-06)` before `feat(06-06)`). Gates present: RED `20226ce`, `fbebbff`, `eddd8ad`; GREEN `2d8c9d8`, `d8801c7`, `0e30ec8`. No REFACTOR commits were needed.

- **RED evidence, Task 1:** `teams_get_save_get` failed on `GET /api/teams` answering 404 (expected 200). Semantic assessment: the target test ran and failed on the planned first assertion.
- **RED evidence, Task 2:** 7 of 8 tests failed on planned assertions (`POST /api/teams/delete` and the staff routes answered 404/405, the update path answered the placeholder refusal, hyphenated field names). `teams_get_save_get` stayed green.
- **RED evidence, Task 3:** `workflows_get_has_version_and_edit` (`version` null), `workflows_save_rules` (`edit` missing) and `engines_readiness_no_secret` (`/api/engines` 404) failed on planned assertions. `settings_routes_gated` already passed: the gate and the 405 on unknown POST paths pre-exist, so it is kept as a regression guard rather than a RED driver.
- `gsd_run check tdd-red-evidence` does not apply to `cargo test` output (no supported report format); the semantic assessment above is by inspection of the real test output.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Loop-name mismatch between the plan text and the code**
- **Found during:** Task 3 (`Workflow::to_file_entry`)
- **Issue:** the plan's interface says `Workflow.loops` holds `plan|implement|qa`, but `validate_entry` stores `plan|build|qa` (the file key is `implement`, the stored loop start is `build`). Reading only `implement` would have dropped the code-review loop from `edit` and from the round trip.
- **Fix:** `to_file_entry` maps `build` (and `implement`, defensively) to `"implement": "code-review"`; the round-trip unit test covers both built-ins and a partial loop set.
- **Files modified:** `agent-brain/src/workflows.rs`
- **Verification:** `to_file_entry_round_trips`, `workflows_get_has_version_and_edit`
- **Committed in:** `0e30ec8`

**2. [Rule 1 - Bug] Gate test used a PUT without an Origin header**
- **Found during:** Task 3 RED run
- **Issue:** my first `settings_routes_gated` sent `PUT /api/engines` without Origin and got the gate's 403 instead of the router's 405; a test bug, not a product bug.
- **Fix:** use `put_json` (cookie plus Origin) so the router's 405 is what is asserted.
- **Files modified:** `agent-brain/tests/dashboard_settings.rs`
- **Committed in:** `eddd8ad`

### Plan-compliant choices worth noting (not deviations)

- Six commits instead of three: TDD RED commits precede each task's feat commit. The plan's commit subjects (`feat(06-06): ...`) are used for the GREEN commits.
- The plan did not specify the `notes` wire form for staff with no notes (empty string chosen) or the 422 message for a duplicate or unknown staff id (`Đã có nhân sự mã '{id}'.`, `Không có nhân sự '{id}' trong staff.json.`).
- `GET /api/workflows` falls back to the old tolerant loader with `version: "unreadable"` if the file cannot be read as UTF-8, so the read path never gets worse than before.

**Total deviations:** 2 auto-fixed (both Rule 1). **Impact on plan:** none on scope; no files outside the plan's list were touched.

## Issues Encountered

None blocking. The worktree sandbox refuses compound git commands, so commits and checks were issued as separate plain commands.

## Verification Results

- `cargo test --offline --locked --test dashboard_settings --test dashboard_workflows --test dashboard_http --test dashboard_projects -- --test-threads=1`: 12 + 9 + 9 + 9 passed, 0 failed
- `cargo test --offline --locked --lib -- --test-threads=1`: 407 passed, 0 failed
- `cargo fmt --check`: clean; `git diff --exit-code 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock`: no diff; `git diff --stat 574b6cd -- agent-brain/tests/dashboard_workflows.rs`: empty; no build warnings
- Acceptance greps: `merge(api_settings::routes())` once and before `.fallback(`; `sync_all` x2, `0o600`, `create_new(true)` present in `settings_store.rs`; `validate_entry` and `to_file_entry` pub; `/api/engines` has no write route

## Known Stubs

None.

## Known Limitations

- Deleting a staff member checks usage against teams that load; a team entry that fails validation for another reason is not counted. (An invalid `teams.json` entry is already surfaced as an error line in the UI, and `teams.json` writes are blocked while it exists.)
- The version token is a std `DefaultHasher` hash, valid only inside one serve process (same trade as `task_rev`), as the plan specified.

## Threat Flags

None. All new surface (settings routes, file writes, engines GET) is covered by T-06-18 to T-06-23 in the plan's threat model: routes merged before the fallback inside `auth::gate`, version-token conflicts and invalid-file refusals, temp 0600 plus fsync plus rename plus directory fsync, no engine write route, readiness exposes the variable name only, 64 KiB body limit unchanged, no crate added.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- The payload contracts in the plan's `<interfaces>` are in place for plans 06-08, 06-15 and 06-17. Notes for them: refusal `errors[].field` for workflows is null; staff `notes` is `""` when absent; write responses carry the fresh list payload plus `ok: true`.
- No blockers.

## Self-Check: PASSED

- Created files exist: `settings_store.rs`, `api_settings.rs`, `tests/dashboard_settings.rs`
- Commits found on HEAD: `20226ce`, `2d8c9d8`, `fbebbff`, `d8801c7`, `eddd8ad`, `0e30ec8`
- All three tasks' acceptance criteria re-run and passing; plan-level verification commands re-run and passing

---
*Phase: 06-multi-engine-media-branch*
*Completed: 2026-10-27*
