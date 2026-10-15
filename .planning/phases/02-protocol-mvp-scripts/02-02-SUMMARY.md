---
phase: 02-protocol-mvp-scripts
plan: 02
subsystem: protocol-files
tags: [python, stdlib, markdown, contract-fixtures, path-validation]

requires:
  - phase: 02-protocol-mvp-scripts
    provides: "02-01 brain and tests packages"
provides:
  - "brain/planfile.py: .plan.md / .qa-report.md render, parse, validate, meta, plan_files, has_blockers, parse_qa_verdict"
  - "spec §4.2 / §4.3 templates verbatim"
  - "tests/contract/plan.json (18 cases) and qa_report.json (8 cases), shared with Phase 3"
affects: [02-03, 02-05, 02-06, 03-rust-cli-core]

actuals:
  tasks: 1
  commits: 2

tech-stack:
  added: []
  patterns:
    - "language-neutral {version: 1, cases: [...]} fixtures run under subTest(id=...)"
    - "brain-meta JSON comment on line 2 with '>' escaped as \\u003e"

key-files:
  created:
    - brain/planfile.py
    - brain/templates/plan.md
    - brain/templates/qa-report.md
    - tests/contract/plan.json
    - tests/contract/qa_report.json
    - tests/test_planfile.py
  modified: []

key-decisions:
  - "Executed by Antigravity (agy, gemini-3.8-flash-high); reviewed by Claude"

patterns-established:
  - "TDD commit pair: test(02-02) fails on its own, feat(02-02) makes it pass"

requirements-completed: []

duration: 15min
completed: 2026-10-15
status: complete
---

# Phase 2 Plan 02: `.plan.md` / `.qa-report.md` File Contract Summary

**One parser and validator for the plan and QA report files, with the spec templates copied verbatim and 26 language-neutral fixture cases that Phase 3's Rust tests will reuse.**

## Performance

- **Duration:** 15 min (agy run 908 s)
- **Completed:** 2026-10-15
- **Tasks:** 1
- **Files modified:** 6 (all new)

## Accomplishments

- `brain/templates/plan.md` matches spec §4.2 line for line; `brain/templates/qa-report.md` uses the spec §4.3 QA bullets as headings.
- `brain/planfile.py` implements every function in the plan's interface list with the stated signatures.
- Path checks for the File column: absolute, `~`, `..`, `.git/`, protocol files, empty cells, and (with a root) symlinks that leave the repo. NFD paths are normalized to NFC.
- The meta comment survives a request containing `-->`; the QA verdict parser returns a verdict only when exactly one of Đạt / Cần sửa appears.

## Task Commits

1. **Task 1 (tests first):** `2bee0c3` test(02-02): add planfile contract fixtures, templates, and tests
2. **Task 1 (implementation):** `03c8a6a` feat(02-02): implement planfile parser, validator, and skeleton renderers

## Files Created/Modified

- `brain/planfile.py` - contract implementation
- `brain/templates/plan.md`, `brain/templates/qa-report.md` - spec templates
- `tests/contract/plan.json` (18 cases), `tests/contract/qa_report.json` (8 cases) - shared fixtures
- `tests/test_planfile.py` - runs every fixture case plus direct renderer, meta and NFC tests

## Decisions Made

None beyond the plan.

## Deviations from Plan

None.

## Review (Claude)

Re-run by the reviewer:

- `python3 -m unittest discover -s tests -t . -v`: 9 tests, OK (3 from 02-01, 6 new).
- All five acceptance-criteria commands pass; plan.json has 18 cases (12 required), every id and expect present.
- Test-first order verified: at `2bee0c3` alone, `tests.test_planfile` fails; with `03c8a6a` it passes.
- Templates compared with `files/02-phase1-giao-thuc-mvp.md` §4.2 and §4.3: verbatim.
- Scope: exactly the six files in `files_modified`. No secrets, no installs, no real profile paths.
- Extra probing of the File column outside the fixtures: NFD, backticks, spaces, `.git` inside paths and `..` that resolves back into `.git/` are all handled.

Finding (a gap in the plan's specification, not an execution error):

- **`validate_plan` accepts File cells that name the whole repo or a glob.** `.`, `./` and `src/..` normalize to `.`; `src/*`, `**` and `src/[ab].py` pass unchanged; none produces an error, and `plan_files` returns them. Plan 02-05 builds Dev's `Write(./<file>)` rules and the guard hook's write allowlist from `plan_files`, so one such cell in an LLM-written plan would let Dev write anywhere (ROLE-06). Fix: new codes `not_a_file` and `glob_path`, plus six contract cases. Scheduled in `.planning/fixes/02-01-review-fixes.plan.md` together with the two 02-01 findings, before plan 02-03.

## Issues Encountered

None.

## User Setup Required

None.

## Next Phase Readiness

- Finding **resolved** in `298dea1` (6 contract cases) and `1230a35` (`not_a_file`, `glob_path` codes in `validate_plan`), reviewed by Claude; each test commit fails on its own and its fix commit passes. Next: plan 02-03 (Lead protocol enforcement).
- ROLE-01, ROLE-03, ROLE-04 and ROLE-06 have their file-level checks here; they are marked complete only once the role commands enforce them (plans 02-03 and 02-05).
