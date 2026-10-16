---
phase: 03-rust-cli-core
plan: 06
subsystem: plan-contract
tags: [rust, planfile, qa-report, contract-tests, byte-parity]

requires:
  - phase: 03-rust-cli-core
    provides: "03-01 embedded assets; 03-03 planfile meta; 03-04 roles prompt constants"
provides:
  - "planfile.rs: parse_plan, validate_plan (ordered error codes, outside_root, NFD paths), plan_files, has_blockers, parse_qa_verdict (Đạt / Cần sửa / none), QA skeleton"
  - "contract_planfile.rs: all 24 plan.json cases and 8 qa_report.json cases, Python meta-line round trip"
  - "assets_parity.rs: embedded prompts, templates and default config locked to the Phase 2 originals"
affects: [03-10, 03-11]

actuals:
  tasks: 2
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Byte-equality tests keep the embedded Phase 2 prompts from drifting (orchestrator decision 3)"

key-files:
  created:
    - agent-brain/tests/contract_planfile.rs
    - agent-brain/tests/assets_parity.rs
  modified:
    - agent-brain/src/planfile.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, gemini-3.8-flash-high); reviewed by Claude"

patterns-established:
  - "Contract tests collect every case's mismatch and fail once with the full list"

requirements-completed: [CLI-03]

duration: 11min
completed: 2026-10-16
status: complete
---

# Phase 3 Plan 06: Plan and QA Report Contract Summary

**The `.plan.md` / `.qa-report.md` contract is ported and passes all 24 plan cases and 8 QA cases of the shared fixtures, Python-written meta lines read back unchanged, and the embedded prompts, templates and default config are locked byte for byte to Phase 2.**

## Performance

- **Duration:** 11 min (agy run 679 s)
- **Completed:** 2026-10-16
- **Tasks:** 2
- **Files:** 3

## Task Commits

1. `65042c0` test(03-06): add failing plan contract tests
2. `4c4c4cd` feat(03-06): port plan parsing and validation
3. `ec6f40c` test(03-06): add qa contract and assets parity tests
4. `16e5865` feat(03-06): qa verdict and asset parity

## Deviations from Plan

None.

## Review (Claude)

Re-run by the reviewer on a `git archive` snapshot of `16e5865`:

- Acceptance: `plan_contract_cases` + `plan_fixture_has_24_cases` 2/2; `prompts_equal_phase2`, `templates_equal_phase2`, `default_config_matches_phase2` 3/3; `qa_report_contract_cases`, `qa_skeleton_matches_python_shape`, `reads_python_meta_line` 3/3; `pub fn validate_plan(` and `outside_root:` present; `cargo fmt --check` clean; full cargo suite passes; `brain/` and `tests/` untouched.
- Test strength: `plan_contract_cases` compares `valid`, the exact ordered `error_codes`, `files` and `has_blockers` per case, materialises the symlink-escape layout, and fails with the full mismatch list.
- Scope: exactly the plan's three files.

## Issues Encountered

None.

## Next Phase Readiness

03-10 (Lead parity) uses `validate_plan` for plan validation and re-plan; 03-11 uses `parse_qa_verdict` for QA.
