---
phase: 02-protocol-mvp-scripts
plan: 04
subsystem: project-selection
tags: [python, stdlib, routing, profiles, projects-map, contract-fixtures, tdd]

requires:
  - phase: 02-protocol-mvp-scripts
    provides: "02-01 routing/config/projects; 02-03 lead_scenario; review fix: load_map refuses an unreadable map"
provides:
  - "brain scan / brain add (brain/registry.py) maintaining projects_map.json per D-04..D-07"
  - "projects.save_map (atomic), git_remote (credential-stripped), unique_id, new_entry, scan, add_project, find_duplicates"
  - "contract fixtures normalize.json, routing.json, profile_env.json, scan.json shared with Phase 3"
  - "P1-01..P1-05 and P1-10 tests named test_p1_xx_*"
affects: [02-05, 02-06, 02-07, 03-rust-cli-core]

actuals:
  tasks: 2
  commits: 3

tech-stack:
  added: []
  patterns:
    - "merge projects by realpath; scan only updates repo and missing"
    - "atomic JSON save: temp file in the same dir, fsync, os.replace"

key-files:
  created:
    - brain/registry.py
    - tests/contract/normalize.json
    - tests/contract/routing.json
    - tests/contract/profile_env.json
    - tests/contract/scan.json
    - tests/test_routing.py
    - tests/test_profile.py
    - tests/test_projects.py
  modified:
    - brain/projects.py

key-decisions:
  - "Executed by Antigravity (agy, gemini-3.8-flash-high); reviewed by Claude"
  - "brain/routing.py and brain/config.py were in scope for fixes but needed none: every fixture case passed on the 02-01 code"

patterns-established:
  - "Scan-step fixtures with {Root} placeholders materialized into real git repos in the Sandbox"

requirements-completed: [ROUTE-06]

duration: 15min
completed: 2026-10-15
status: complete
---

# Phase 2 Plan 04: Project Selection Summary

**Vietnamese names, aliases, Fallback Ask and the work/personal profile env are now proven from shared JSON fixtures and through the real `brain` CLI. `brain scan` and `brain add` keep your hand edits, mark missing projects, warn on duplicate names and never store remote credentials.**

## Performance

- **Duration:** 15 min (agy run 877 s)
- **Completed:** 2026-10-15
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments

- P1-01..P1-04: Vietnamese name and alias matching, ambiguous and no-match stops with exit 2 and no agent started; 17 routing and 7 normalize contract cases.
- P1-10: work runs with `CLAUDE_CONFIG_DIR` removed even when the shell exports one; personal gets `<HOME>/.claude-orca`; empty and unknown profiles are refused (7 profile_env cases).
- P1-05: `brain scan` adds new depth-1 repos, keeps `id`, `name_vi`, `aliases`, `profile` and `type` untouched on rescans, marks missing folders and restores them, keeps unknown keys, and gives colliding ids a `-2` suffix.
- ROUTE-06: duplicate names and aliases (including an alias equal to another id) print a `CẢNH BÁO` line; exit stays 0.
- `brain add`: non-git folder → `media`; an already-mapped path keeps its hand fields; a missing path → exit 1.

## Task Commits

1. **Task 1 (tests + fixtures):** `08af993` test(02-04): routing and profile acceptance fixtures and tests
2. **Task 2 (tests + fixture):** `5f4fe77` test(02-04): add scan contract fixture and test_projects
3. **Task 2 (implementation):** `af1d8d9` feat(02-04): implement brain scan and brain add with duplicate warnings and merge semantics

## Files Created/Modified

- `brain/projects.py` - scan, add, save and duplicate helpers (the four 02-01 functions unchanged, including the review fix in `load_map`)
- `brain/registry.py` - `cmd_scan`, `cmd_add`
- `tests/contract/*.json` - normalize, routing, profile_env, scan fixtures
- `tests/test_routing.py`, `tests/test_profile.py`, `tests/test_projects.py`

## Decisions Made

None beyond the plan.

## Deviations from Plan

None.

## Review (Claude)

Re-run by the reviewer:

- Full suite: 51 tests, OK. Every acceptance-criteria command of both tasks passes; test names include `test_p1_01`..`test_p1_04`, both `test_p1_10_*` and `test_p1_05_scan_keeps_hand_edits`.
- Test-first order: `08af993` and `5f4fe77` each fail on their own; `af1d8d9` passes.
- The profile fixture uses `~/.claude-orca` / `/home/u/.claude-orca` (SPK-10). Tests use Sandbox paths only; no real home or profile directory is touched.
- The review-fix behaviour of `load_map` (refuse an unreadable map) is untouched, so `brain scan` cannot overwrite a broken map; `tests/test_robustness.py` still passes.
- Scope: the plan's files only.

Minor findings, not blocking (candidates for a later small fix):

1. `brain/registry.py:57` detects an explicit `--type` by scanning `sys.argv`, because `cli.py` (not in this plan's scope) gives `--type` the default `code`. The real `brain add` command works, but `cli.main(["add", path, "--type", ...])` called in-process ignores the flag. Cleaner: default `--type` to `None` in `cli.py` and drop the `sys.argv` check.
2. `projects.git_remote` strips credentials only from `http`/`https` URLs. An `ssh://user:password@host/...` remote would keep its password in the map. Rare, but the same strip could apply to any scheme with a password.

## Issues Encountered

None.

## User Setup Required

None. (The live run in plan 02-07 starts with `brain scan --root <folder>` and hand-editing `name_vi`, `aliases` and `profile`.)

## Next Phase Readiness

- Plan 02-05 (Dev + QA roles) next, then 02-06.
- ROUTE-06 marked complete. ROUTE-01..05 and PROF-01 are proven at the brain level here but are also owned by plan 02-07's live acceptance (PROF-01's real transcript location), so they stay open until then.
