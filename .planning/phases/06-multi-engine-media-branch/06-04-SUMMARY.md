---
phase: 06-multi-engine-media-branch
plan: 04
subsystem: database
tags: [sqlite, rusqlite, migration, schema-v4, team, human-run]

requires:
  - phase: 05.4-telegram-task-intake
    provides: schema v3 (tasks.origin) and the v2 test builder
provides:
  - "tasks.team_json column (MIGRATIONS[3], SCHEMA_VERSION 4)"
  - "db::task_team_json, set_task_team_if_null, set_task_team_cas (NULL-safe, bump updated_at)"
  - "db::insert_run_with_engine (insert_run delegates with claude)"
  - "db::HumanRun and insert_human_run (finished run, engine người, no tokens, never running)"
affects: [06-05, 06-10, 06-12, 06-13, 06-19]

actuals:
  tokens: 9500
  tasks: 3
  commits: 4
plan_head_before: 2a49c4bf93a063f40fd647af0743cda244b95ae8
plan_head_after: dae9b4d062e834d6616f4894a2b8bed599ed2226

tech-stack:
  added: []
  patterns:
    - "NULL-safe compare-and-swap with SQLite `IS ?` bound to Option<&str>"
    - "Human work stored as a finished run row, never a running one"

key-files:
  created: []
  modified:
    - agent-brain/src/db.rs
    - agent-brain/tests/db_migration.rs
    - agent-brain/tests/task_origin.rs

key-decisions:
  - "Task 1 decision: option-column, chosen by the user on 2026-10-27 before dispatch (new tasks.team_json column, brain.db schema 4)"
  - "Engine name strings stay literals in db.rs ('claude' in insert_run, 'người' in insert_human_run); plan 06-05 passes Engine::as_db_str() into insert_run_with_engine"
  - "Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6)"

requirements-completed: [TEAM-02, HUM-03]

coverage:
  - id: D1
    description: "tasks.team_json exists after migration 4; v2 and v3 databases upgrade in place with rows kept and team_json NULL"
    requirement: TEAM-02
    verification:
      - kind: unit
        ref: "agent-brain/tests/db_migration.rs#v3_db_gains_team_json"
        status: pass
      - kind: unit
        ref: "agent-brain/tests/db_migration.rs#v2_db_jumps_to_v4"
        status: pass
      - kind: unit
        ref: "agent-brain/tests/db_migration.rs#fresh_db_has_team_json_at_current_version"
        status: pass
    human_judgment: false
  - id: D2
    description: "Team writes are once-only (set_task_team_if_null) or compare-and-swap (set_task_team_cas, NULL-safe) and bump updated_at"
    requirement: TEAM-02
    verification:
      - kind: unit
        ref: "agent-brain/tests/db_migration.rs#team_if_null_writes_once"
        status: pass
      - kind: unit
        ref: "agent-brain/tests/db_migration.rs#team_cas_rules"
        status: pass
      - kind: unit
        ref: "agent-brain/tests/db_migration.rs#team_json_missing_task"
        status: pass
    human_judgment: false
  - id: D3
    description: "insert_run_with_engine stores the given engine; insert_run keeps its signature and stores claude"
    verification:
      - kind: unit
        ref: "agent-brain/tests/db_migration.rs#insert_run_with_engine_stores_engine"
        status: pass
    human_judgment: false
  - id: D4
    description: "insert_human_run stores a finished run (engine người, succeeded, usage none, zero tokens, NULL model/pid) that reconcile_stale_runs never touches"
    requirement: HUM-03
    verification:
      - kind: unit
        ref: "agent-brain/tests/db_migration.rs#insert_human_run_shape"
        status: pass
      - kind: unit
        ref: "agent-brain/tests/db_migration.rs#human_run_not_reconciled"
        status: pass
      - kind: unit
        ref: "agent-brain/tests/db_migration.rs#insert_human_run_stores_outcome_as_given"
        status: pass
    human_judgment: false
  - id: D5
    description: "A database one version ahead of SCHEMA_VERSION is still refused and left unchanged"
    verification:
      - kind: unit
        ref: "agent-brain/tests/db_migration.rs#newer_version_is_refused_and_unchanged"
        status: pass
    human_judgment: false

duration: 25min
completed: 2026-10-27
status: complete
---

# Phase 6 Plan 04: Schema v4 team_json, run engine and human runs Summary

**brain.db schema v4 adds `tasks.team_json` with once-only and compare-and-swap writers, runs record their real engine, and a person's finished work is stored as a `succeeded` run with engine `người` and no running row.**

## Performance

- **Duration:** about 25 min
- **Started:** 2026-10-27T01:28Z (approximate; not captured at start)
- **Completed:** 2026-10-27T01:53Z
- **Tasks:** 3 (Task 1 was a pre-answered decision)
- **Files modified:** 3

## Task 1 decision

Task 1 decision: option-column, chosen by the user on 2026-10-27 before dispatch. The `tasks.team_json` column (brain.db schema 4) is the storage for the task team, its history and the human-wait marker. The change is one-way for the live database (still v2 on 2026-10-23); plan 06-19 owns the backup, the dry run on a copy and the README upgrade note. Nothing in this plan opened the live brain.db.

## Executor

Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6). CLAUDE.md exempts explicit user requests from the agy hand-off rule. Files were staged by name, no `--no-verify`, no secrets, and every test uses a temp dir.

## Accomplishments

- `MIGRATIONS[3]` (`ALTER TABLE tasks ADD COLUMN team_json TEXT;`) and `SCHEMA_VERSION = 4`; v2 and v3 databases upgrade in place, rows kept, `team_json` NULL on old rows.
- `task_team_json`, `set_task_team_if_null` (`team_json IS NULL`) and `set_task_team_cas` (`team_json IS ?4`, NULL-safe), both writers bump `tasks.updated_at`; a refused write leaves `updated_at` untouched.
- `insert_run_with_engine` binds the engine; `insert_run` is a one-line delegate with `"claude"`, so `NewRun` and the six test files that build it are unchanged.
- `HumanRun` and `insert_human_run`: one INSERT, engine `người` (verified against an NFC escape literal), status `succeeded`, `usage_source = 'none'`, no model / pid / pgid / pid_start / run_dir / session_id, never `running`.

## Task Commits

TDD tasks produced a RED then a GREEN commit each:

1. **Task 1: team storage decision** - no commit (answered by the user before dispatch)
2. **Task 2 (tracer): migration 4 and team helpers**
   - RED `f9d8809` (test)
   - GREEN `7068c85` (feat)
3. **Task 3: insert_run_with_engine and insert_human_run**
   - RED `0d352de` (test)
   - GREEN `dae9b4d` (feat)

**Plan metadata:** committed after this file (docs: complete plan)

## TDD Gate Compliance

Both TDD tasks have `test(06-04)` before `feat(06-04)`. RED semantic assessment: the RED commits add inert signature stubs in `db.rs` (returning `Ok(None)` / `Ok(false)`, or delegating to `insert_run`, or inserting a minimal wrong row) so the target tests compile and fail on the planned assertions, not on compile errors. Task 2 RED: `v3_db_gains_team_json`, `v2_db_jumps_to_v4`, `fresh_db_has_team_json_at_current_version`, `team_if_null_writes_once`, `team_cas_rules` failed on assertions (user_version 3 vs 4, stub returning false). `team_json_missing_task` passes under the stub because a missing task is also `None`/`false` there; it guards the real implementation against erroring. Task 3 RED: all four new tests failed on assertions (engine `claude` vs `deepseek`, engine and shape of the stub row). The plan's `tdd.md` machine classifier (`check tdd-red-evidence`) does not support cargo's libtest report format, so RED evidence is the unmodified `cargo test` output reviewed by hand. No REFACTOR commit.

## Files Created/Modified

- `agent-brain/src/db.rs` - MIGRATIONS[3], SCHEMA_VERSION 4, team helpers, `insert_run_with_engine`, `HumanRun`, `insert_human_run`
- `agent-brain/tests/db_migration.rs` - `build_v3`, `fresh_task_db`, and the new tests listed in the coverage block (plus `fresh_db_has_team_json_at_current_version` and `insert_human_run_stores_outcome_as_given`); `v2_db_gains_origin_column` now follows `SCHEMA_VERSION`
- `agent-brain/tests/task_origin.rs` - `cli_upgrades_v2_db` now follows `SCHEMA_VERSION`

## Decisions Made

- Option-column for team storage (user decision, above).
- Engine strings stay literals in `db.rs` because `engine.rs` lands in parallel plan 06-03.
- `insert_human_run` binds the engine as an SQL literal `'người'`; the test compares to an NFC escape (`ng\u{01b0}\u{1edd}i`) so a decomposed form would fail.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Two existing tests hard-coded schema version 3**
- **Found during:** Task 2 (bumping `SCHEMA_VERSION` to 4)
- **Issue:** `v2_db_gains_origin_column` (in `db_migration.rs`) asserted `user_version == 3` and `SCHEMA_VERSION == 3`; `cli_upgrades_v2_db` (in `task_origin.rs`) asserted `ver == 3`. Both fail by construction once migration 4 exists.
- **Fix:** Both now assert against `agent_brain::db::SCHEMA_VERSION`. The plan listed only `db.rs` and `db_migration.rs`; `task_origin.rs` was outside the declared files, but the change is a one-line assertion forced by the version bump.
- **Files modified:** `agent-brain/tests/db_migration.rs`, `agent-brain/tests/task_origin.rs`
- **Verification:** both suites pass; full suite green
- **Committed in:** `7068c85`

---

**Total deviations:** 1 auto-fixed (Rule 1), plus the TDD stub commits described above.
**Impact on plan:** No scope creep. One file outside the declared list (`task_origin.rs`) touched by a single assertion line.

## Issues Encountered

None. The known `contract_guard::guard_contract_cases` BrokenPipe flake did not occur; the full suite passed on the first run.

## Verification

- `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test db_migration --test lib_foundation -- --test-threads=1`: 20 + 13 passed, includes `v3_db_gains_team_json ... ok`
- `cargo test ... --lib`: 383 passed
- `cargo test ... --test db_migration`: 24 passed, includes `insert_human_run_shape ... ok`
- `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1`: exit 0, no FAILED
- `cargo fmt --manifest-path agent-brain/Cargo.toml --check`: exit 0
- `git diff --exit-code 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock`: no diff
- `git diff 574b6cd -- dashboard_kill.rs cli_review.rs dashboard_stream.rs dashboard_http.rs dashboard_stats.rs`: empty (NewRun unchanged)
- Acceptance greps: one `ADD COLUMN team_json TEXT`, one `SCHEMA_VERSION: i64 = 4`, three team fns, `team_json IS ?` and `team_json IS NULL` each present once, three of `insert_run_with_engine` / `insert_human_run` / `HumanRun`

## Known Stubs

None. The RED-commit stubs were replaced in the GREEN commits.

## Threat Flags

None. No new endpoint, auth path or file access; the schema change is the one covered by T-06-11.

## User Setup Required

None - no external service configuration required. Before a Phase 6 binary first opens the live `~/.local/share/agent-brain/brain.db`, back it up (plan 06-19).

## Next Phase Readiness

- Plans 06-05, 06-10, 06-12 and 06-13 can use the contract names from `<interfaces>` as written.
- `STATE.md`, `ROADMAP.md` and `REQUIREMENTS.md` were not touched (orchestrator-owned in worktree mode); TEAM-02 and HUM-03 are still shared with sibling plans, so the orchestrator decides when to mark them complete.

## Self-Check: PASSED

- `agent-brain/src/db.rs`, `agent-brain/tests/db_migration.rs`, `agent-brain/tests/task_origin.rs`: present and committed
- Commits `f9d8809`, `7068c85`, `0d352de`, `dae9b4d`: present on the branch, 4 commits since `plan_head_before`

---
*Phase: 06-multi-engine-media-branch*
*Completed: 2026-10-27*
