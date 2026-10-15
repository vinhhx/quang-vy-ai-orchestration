---
phase: 02-protocol-mvp-scripts
plan: 03
subsystem: lead-role
tags: [python, stdlib, git, protocol-enforcement, tdd]

requires:
  - phase: 02-protocol-mvp-scripts
    provides: "02-01 tracer and runner; 02-02 planfile contract (validate_plan incl. not_a_file / glob_path from the review fix)"
provides:
  - "brain lead: skeleton before the run, ROLE-01 post-run check (exit 3), meta restore, validate_plan gate, refusals, re-plan on the task branch"
  - "Lead prompt: spec lines verbatim plus .gsd fallback, plan rules and a good/bad example"
  - "tests/helpers.plan_text and lead_scenario; fake claude write op keep_meta flag"
affects: [02-05, 02-06, 02-07, 03-rust-cli-core]

actuals:
  tasks: 1
  commits: 4

tech-stack:
  added: []
  patterns:
    - "brain writes the protocol skeleton, the agent fills it, brain validates and commits"
    - "never revert user files: report paths and stop with a documented exit code"

key-files:
  created: []
  modified:
    - brain/role_lead.py
    - brain/prompts/lead.md
    - tests/helpers.py
    - tests/fakes/fake_claude.py
    - tests/test_lead_e2e.py

key-decisions:
  - "Re-plan rule: on the current task branch, an uncommitted .plan.md alone does not count as dirty (it is the file the re-plan regenerates); any other dirty path, or a leftover .plan.md on another branch, still refuses with exit 1"
  - "Executed by Antigravity (agy, gemini-3.8-flash-high); reviewed by Claude"

patterns-established:
  - "Protocol breach → EXIT_PROTOCOL (3) with the offending paths; refusal → exit 1 before any branch or agent side effect"

requirements-completed: []

duration: 15min
completed: 2026-10-15
status: complete
---

# Phase 2 Plan 03: Lead Protocol Enforcement Summary

**`brain lead` now only ever commits a valid `.plan.md` on a fresh or re-planned task branch. Out-of-plan edits stop with exit 3 and are left in place, and every refusal happens before a branch is made or the agent starts.**

## Performance

- **Duration:** 15 min (agy runs: plan 576 s, review fix 292 s)
- **Completed:** 2026-10-15
- **Tasks:** 1 (+ one review fix)
- **Files modified:** 5

## Accomplishments

- Skeleton `.plan.md` (title + brain-meta + five headings) written before the Lead run; meta restored if Lead drops or changes it.
- ROLE-01 post-run check: any changed or untracked path other than `.plan.md` → exit 3, paths listed, nothing committed or reverted.
- `validate_plan` gate: an invalid plan stays uncommitted and brain prints the manual commit command (D-10).
- Refusals with exit 1 before side effects: dirty tree, empty or unknown profile, missing path, not a repo root, task branch existing but not checked out. Ambiguous name: exit 2.
- Re-plan on the current task branch keeps base_branch, base_commit and created_at from the first plan, including after a timeout or a failed validation (review fix).
- Lead prompt: the 6 spec §4.3 lines verbatim, then the `.gsd` → CLAUDE.md/README fallback, plan rules (keep the first two lines, one repo-relative file per row, no `..`, leave Vướng mắc empty, no code edits or tests) and a Tốt / Không tốt example.

## Task Commits

1. **Task 1 (tests first):** `ecd69d8` test(02-03): add lead protocol e2e tests and test helpers
2. **Task 1 (implementation):** `460f732` feat(02-03): enforce lead protocol in role_lead and update prompt
3. **Review fix (tests):** `bb3ce0e` test(02-03): re-plan after a failed Lead run
4. **Review fix:** `3263835` fix(02-03): allow re-plan over brain's own uncommitted .plan.md

## Files Created/Modified

- `brain/role_lead.py` - full Lead protocol in `cmd_lead`
- `brain/prompts/lead.md` - prompt extensions
- `tests/helpers.py` - `plan_text`, `lead_scenario`
- `tests/fakes/fake_claude.py` - `keep_meta` on write ops
- `tests/test_lead_e2e.py` - 15 Lead protocol tests + 4 re-plan tests (19 in the file)

## Decisions Made

- Re-plan dirty-tree rule (see key-decisions), chosen by the reviewer to satisfy both 02-03 must-haves ([edge ROLE-01] and [edge ROLE-05]).

## Deviations from Plan

None in agy's execution. The plan itself was inconsistent; see the review below.

## Review (Claude)

Re-run by the reviewer:

- Full suite: 36 tests, OK. `test_p1_06_lead_only_writes_plan` ok; every acceptance-criteria command passes; the exit-3 test asserts HEAD unchanged and `src/app.py` keeping the fake's edit.
- Test-first order: `ecd69d8` alone fails (6 failures); `bb3ce0e` alone fails (2 failures); each following commit passes.
- Scope: the plan's five files; the review fix touched only `brain/role_lead.py` and added tests (no existing test line removed).
- Prompt checked against spec §4.3: verbatim lines kept; it does not tell Lead to run tests or edit code.

Finding (resolved): **re-plan after a failed run was blocked by brain's own skeleton.** After a timeout, Ctrl+C or a failed `validate_plan`, the skeleton `.plan.md` stayed uncommitted and the next `brain lead` refused with "Working tree có thay đổi chưa commit". This broke the must-have "re-running brain lead on the same task branch regenerates the plan and keeps the original base_commit". The plan's action had said to keep the 02-01 dirty check unchanged, which contradicted that must-have. Fixed by `.planning/fixes/02-03-replan-after-failure.plan.md` (`bb3ce0e`, `3263835`), executed by agy and reviewed: the four new tests cover timeout → re-plan (base_commit kept), failed validation → re-plan, other dirty files still refusing, and a leftover `.plan.md` on another branch still refusing.

Note for users: a re-plan regenerates `.plan.md` from a fresh skeleton, so hand edits to an uncommitted `.plan.md` are replaced. To keep hand edits, commit them with the printed `git -C <root> commit -m "<task>: plan" -- .plan.md` command instead of re-running `brain lead`. Plan 02-06's manual should say this.

## Issues Encountered

None beyond the finding above.

## User Setup Required

None.

## Next Phase Readiness

- Wave 4 (02-04, 02-05, 02-06) can start. They are run one at a time on this branch because they share `tests/helpers.py`.
- ROLE-01 and ROLE-05 are now enforced and tested for Lead at the brain level; they are marked complete after plan 02-05 adds Dev and QA, and live behaviour is checked in 02-07.
