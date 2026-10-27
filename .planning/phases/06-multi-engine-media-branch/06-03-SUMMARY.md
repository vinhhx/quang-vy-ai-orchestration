---
phase: 06-multi-engine-media-branch
plan: 03
subsystem: engines
tags: [rust, teams, staff, engine-catalog, readiness, config, requirements]

requires:
  - phase: 05.2-workflows
    provides: "workflows::validate_name, key_lines, loader conventions reused by teams/staff"
provides:
  - "engine.rs: Engine enum (parse with NFC, as_db_str, to_config_string), Readiness, EngineStatus, find_command, readiness"
  - "staff.rs: staff.json loader and validator (SET-04 store)"
  - "teams.rs: built-in teams mac-dinh and tiet-kiem, strict teams.json loader, resolve, summary, TaskTeam/TeamChange/HumanWait/PushResult documents"
  - "config.rs: optional engines block (DeepseekCfg, AgyCfg, EnginesCfg), key NAME only"
  - "projects::team_of and Sandbox teams/staff helpers"
  - "REQUIREMENTS.md: TEAM-01..03, SET-01..05, HUM-01..03, QUOTA-01 registered, coverage 122"
affects: [06-04, 06-05, 06-10, 06-11, 06-12, 06-13, 06-16]

actuals:
  tokens: 18000
  tasks: 3
  commits: 3
plan_head_before: 2a49c4bf93a063f40fd647af0743cda244b95ae8
plan_head_after: 6b6425e68c2651a22f8f344b32e475788d44c86b

tech-stack:
  added: []
  patterns:
    - "Team and staff loaders mirror workflows.rs: size cap, line/column JSON error, per-entry `Dòng {n}: ... '{name}': ` prefix, built-ins always present"
    - "FieldError {field, message} so a later Settings writer can reuse the same validators"
    - "Readiness is local only: PATH lookup plus key-variable presence, no env value ever copied"

key-files:
  created:
    - agent-brain/src/engine.rs
    - agent-brain/src/staff.rs
    - agent-brain/src/teams.rs
    - agent-brain/assets/teams.default.json
  modified:
    - agent-brain/src/lib.rs
    - agent-brain/src/config.rs
    - agent-brain/assets/config.default.json
    - agent-brain/src/projects.rs
    - agent-brain/tests/common/mod.rs
    - .planning/REQUIREMENTS.md

key-decisions:
  - "Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6)."
  - "Per-entry error lines use the entry's line (key_lines), not the field's line, matching 'Dòng {l}' in the plan"
  - "teams.json and staff.json entries reject unknown keys (`Khóa lạ: {k}`), like workflows.json; config `engines` rejects unknown engine names but ignores unknown sub-keys, like the rest of config.rs"
  - "A missing deepseek command is reported before a missing key (thieu_lenh wins over thieu_key)"
  - "engines.deepseek.models only accepts the five role keys; an unknown role is a config error"

patterns-established:
  - "Engine::parse is the single gate for engine text (trim, NFC, exact names, validated staff id)"
  - "Team::summary and engine_text are the only producers of the server summary string"

requirements-completed: [TEAM-01, SET-04, ENG-07]

duration: ~25min (start time not recorded; estimate)
completed: 2026-10-27
status: complete

coverage:
  - id: D1
    description: "teams.json loader with built-ins, override, strict validation (QA claude, human only dev, name/description rules), exact Vietnamese errors, resolve"
    requirement: TEAM-01
    verification:
      - kind: unit
        ref: "cargo test --lib teams:: (builtins_only, file_entries_and_override, human_staff_and_warning, entry_errors_keep_others, file_level_errors, resolve_rules, task_team_doc)"
        status: pass
    human_judgment: false
  - id: D2
    description: "staff.json loader (slug id <= 32 bytes, display_name, notes) with line-numbered errors"
    requirement: SET-04
    verification:
      - kind: unit
        ref: "cargo test --lib staff:: (staff_rules)"
        status: pass
    human_judgment: false
  - id: D3
    description: "QA forced to claude and human engine restricted to dev, refused with the exact messages"
    requirement: ENG-07
    verification:
      - kind: unit
        ref: "cargo test --lib teams::tests::entry_errors_keep_others"
        status: pass
    human_judgment: false
  - id: D4
    description: "config.json engines block with defaults and validation; readiness computed locally and never leaking the key value"
    requirement: TEAM-01
    verification:
      - kind: unit
        ref: "cargo test --lib config::tests::engines_defaults config::tests::engines_validation engine::tests::readiness_states"
        status: pass
    human_judgment: false
  - id: D5
    description: "Requirement IDs registered in REQUIREMENTS.md with Traceability rows and coverage 122"
    verification:
      - kind: other
        ref: "Task 3 verify scripts (V1_OK, V2_OK)"
        status: pass
    human_judgment: false
---

# Phase 6 Plan 03: Teams, staff and engine catalog foundations Summary

**Hand-editable teams.json (built-ins mac-dinh and tiet-kiem, QA-claude and human-only-dev rules), separate staff.json, config `engines` block with leak-free local readiness, `projects::team_of`, and twelve new requirement IDs registered.**

Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6).

## Performance

- **Duration:** about 25 min (start time was not recorded; estimate)
- **Completed:** 2026-10-27T01:47Z
- **Tasks:** 3
- **Files modified:** 10 (4 created, 6 modified)

## Accomplishments

- `teams::load_from(text, &staff)` always yields `mac-dinh` and `tiet-kiem` first, then file entries in file order; a same-named entry replaces a built-in with source `override`; invalid entries land in `errors` with `Dòng {n}: đội hình '{name}': ` while valid ones still load.
- `Team::summary` implements the UI-SPEC grammar (claude, deepseek, agy, then people in staff order, `tất cả: {engine}` when one engine holds all five roles); output is deterministic.
- `staff.rs`, `engine.rs` (Engine enum with NFC parsing, readiness, `find_command`) and the `TaskTeam`/`HumanWait` documents give plans 06-04..06-16 one shared set of types.
- `config.json` gains an optional `engines` block (key variable NAME only, https base_url, `^[A-Z_][A-Z0-9_]*$` key_env); a config without it behaves as before.
- `engine::readiness` returns claude, deepseek, agy, người in order; a planted key value never appears in the serialized result (asserted in `readiness_states`).
- REQUIREMENTS.md: TEAM-01..03, SET-01..05, HUM-01..03, QUOTA-01 defined after ENG-07, traced to Phase 6, coverage 122.

## Task Commits

1. **Task 1 (tracer): teams and staff loaders with built-in teams** - `14b5d52` (feat)
2. **Task 2: engines config, local readiness, team_of, Sandbox helpers** - `5b8c55f` (feat)
3. **Task 3: register Phase 6 requirements** - `6b6425e` (docs)

**Plan metadata:** committed with this SUMMARY (docs: complete plan). `plan_head_before: 2a49c4bf93a063f40fd647af0743cda244b95ae8`.

Tests were written together with the implementation in each task commit rather than as separate RED commits: the plan is `type: execute` with one commit per task, and Rust unit tests live in the same files as the code. No separate `test(06-03)` commit exists.

## Files Created/Modified

- `agent-brain/src/engine.rs` - Engine enum, constants, Readiness, EngineStatus, find_command, readiness
- `agent-brain/src/staff.rs` - staff.json loader and validators
- `agent-brain/src/teams.rs` - team loader/validator, resolve, summary, persisted TaskTeam documents
- `agent-brain/assets/teams.default.json` - built-in teams
- `agent-brain/src/lib.rs` - `pub mod engine; staff; teams;`
- `agent-brain/src/config.rs` - DeepseekCfg, AgyCfg, EnginesCfg, `Config.engines`, parse_engines, tests
- `agent-brain/assets/config.default.json` - documents the engines defaults
- `agent-brain/src/projects.rs` - `team_of`
- `agent-brain/tests/common/mod.rs` - `teams_path`, `write_teams`, `staff_path`, `write_staff`
- `.planning/REQUIREMENTS.md` - new section, traceability rows, coverage, footer

## Decisions Made

- Entry-level line numbers (not field lines) in per-entry errors, as the plan states.
- Unknown keys in a team or staff entry are an error; unknown sub-keys inside the config `engines` objects are ignored (the config parser is lenient elsewhere), but an unknown engine name is an error.
- Command-missing takes precedence over key-missing for deepseek.
- No `brain.db` change, no crate added: `Cargo.toml` and `Cargo.lock` are identical to 574b6cd.

## Deviations from Plan

None - plan executed exactly as written.

Environment notes (not plan deviations): the sandbox refused writes into the worktree's git-dir, so the plan commit ledger (`gsd-plan-head-before-06-03`) and the cwd-drift sentinel were not written there. The base SHA (`2a49c4b`, equal to the dispatch base) was kept in the scratchpad and the `commits: 3` count measured with `git rev-list --count 2a49c4b..HEAD` before this SUMMARY commit. The cwd never left the worktree.

## Issues Encountered

None. `cargo test --lib` passed 400 tests; `lib_foundation` (13), `dashboard_workflows` (9) and `cli_workflow` (16) passed; `cargo fmt --check` clean; no compiler warnings.

## Known Stubs

None.

## Threat Flags

None. Mitigations T-06-07 (256 KiB cap), T-06-08 (Engine enum, QA rule, human-only-dev), T-06-09 (readiness leak test), T-06-10 (https + key_env pattern) and T-06-SC (no crate added) are implemented and tested.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Ready for 06-04 (db `tasks.team_json` using `TaskTeam`) and 06-05 (run seam on top of `engine`).
- Plan 06-11 may change `engines.deepseek.models` defaults if the E-09 decision is `pass-explicit`.

## Self-Check: PASSED

- Files exist: engine.rs, staff.rs, teams.rs, teams.default.json (created in Task 1 commit).
- Commits exist on this branch: 14b5d52, 5b8c55f, 6b6425e.
- Plan verification re-run: lib suite green, three integration suites green, fmt clean, Cargo.toml/Cargo.lock identical to 574b6cd, both Task 3 verify scripts pass.

---
*Phase: 06-multi-engine-media-branch*
*Completed: 2026-10-27*
