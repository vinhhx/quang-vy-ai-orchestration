---
phase: 02-protocol-mvp-scripts
plan: 05
subsystem: dev-qa-roles
tags: [python, stdlib, claude-code-hooks, allowlist, protocol-enforcement, tdd]

requires:
  - phase: 02-protocol-mvp-scripts
    provides: "02-01 runner; 02-02 planfile (validate_plan incl. not_a_file / glob_path); 02-03 Lead protocol and lead_scenario"
provides:
  - "brain/guard.py: PreToolUse hook enforcing per-run read/write allowlists, fail-closed, logging to guard.jsonl"
  - "brain dev: plan-file allowlist (tool rules + guard + post-run git check), Vướng mắc stop (exit 4)"
  - "brain qa: report-only QA over <plan commit>..HEAD, verdict Đạt / Cần sửa"
  - "Dev and QA prompt files; fake claude hook simulation"
affects: [02-06, 02-07, 03-rust-cli-core]

actuals:
  tasks: 3
  commits: 8

tech-stack:
  added: []
  patterns:
    - "three layers for Dev: --allowedTools rules, a PreToolUse guard hook, and a post-run git diff check"
    - "per-run policy.json / settings.json / guard.jsonl in the 0o700 run dir"

key-files:
  created:
    - brain/guard.py
    - brain/role_dev.py
    - brain/role_qa.py
    - brain/prompts/dev.md
    - brain/prompts/qa.md
    - tests/test_guard.py
    - tests/test_dev_e2e.py
    - tests/test_qa_e2e.py
    - tests/test_pipeline_e2e.py
  modified:
    - tests/fakes/fake_claude.py

key-decisions:
  - "Hook settings use Claude Code's nested shape {matcher, hooks: [{type: command, command}]} (review fix; the plan specified a flat shape)"
  - "Executed by Antigravity (agy, gemini-3.8-flash-high); reviewed by Claude"

patterns-established:
  - "The fake claude honours only the settings shape the real CLI reads, so a wrong shape fails the tests"

requirements-completed: [ROLE-05]

duration: 20min
completed: 2026-10-15
status: complete
---

# Phase 2 Plan 05: Dev and QA Roles Summary

**`brain dev` lets Dev read and edit only the plan's files, enforced by tool rules, a guard hook and a post-run git check. It stops with exit 4 on Vướng mắc. `brain qa` writes only `.qa-report.md` and reports Đạt or Cần sửa.**

## Performance

- **Duration:** 20 min (agy runs: plan 879 s, review fix 273 s)
- **Completed:** 2026-10-15
- **Tasks:** 3 (+ one review fix)
- **Files modified:** 10

## Accomplishments

- Guard hook (`brain/guard.py`): stdlib only, no `brain` imports, runs under `python3 -I`. Fail-closed on bad stdin or policy (exit 2). Paths are joined with cwd, NFC-normalized and realpath-resolved, so `src/../other.py` and symlinks out of the repo are blocked. Grep and Glob are blocked when a read allowlist is set. Every decision goes to `guard.jsonl`.
- `brain dev`: requires the task branch named in the plan meta and a clean tree; runs `validate_plan` and refuses errors (including root and glob paths) and an empty file list before starting claude; builds `Read/Edit/Write(./<file>)` rules plus the guard policy from the pre-run plan; after the run any committed or uncommitted path outside the plan → exit 3, nothing reverted; Vướng mắc → commit `<task>: vướng mắc` and exit 4; prints Cách kiểm tra without running it.
- `brain qa`: finds the `<task>: plan` commit, refuses when no code changed after it, writes the QA skeleton, diffs `<plan commit>..HEAD` excluding `.plan.md` and `.qa-report.md`, refuses any change other than the report (exit 3), commits `<task>: qa` and prints the D-16 next steps on Cần sửa.
- Three separate prompt files (ROLE-05); Dev and QA prompts start with the spec §4.3 drafts verbatim.

## Task Commits

1. **Task 1 tests:** `fd305a5` test(02-05): add guard hook tests and fake claude hook simulation
2. **Task 1:** `9f72913` feat(02-05): implement pre-tool-use guard hook and write_hook_settings
3. **Task 2 tests:** `eb79a78` test(02-05): add dev e2e tests for allowlist and vuong mac stop
4. **Task 2:** `026068c` feat(02-05): implement brain dev with allowlist enforcement and commit handling
5. **Task 3 tests:** `721c366` test(02-05): add qa e2e and full pipeline lead-dev-qa tests
6. **Task 3:** `a9582e1` feat(02-05): implement brain qa with report validation and verdict reporting
7. **Review fix tests:** `3bca897` test(02-05): expect Claude Code's nested hook settings shape
8. **Review fix:** `b52dfc5` fix(02-05): write guard hook settings in Claude Code's nested shape

## Decisions Made

- Nested hook settings shape (see key-decisions).

## Deviations from Plan

None in agy's execution. The plan's settings shape was wrong; see the review.

## Review (Claude)

Re-run by the reviewer:

- Full suite: 87 tests, OK. Every acceptance-criteria command of the three tasks passes, including fail-closed `printf '{bad' | python3 -I brain/guard.py --policy /nonexistent` → exit 2, no `brain` import in guard.py, no `subprocess`/`os.system` in role_dev.py, `test_p1_07_*`, `test_p1_08_*`, `test_p1_09_*` and `test_role_05_*` ok.
- Test-first order per task: each test commit fails for its task's tests and the following feat/fix commit passes (guard, dev, qa+pipeline, and the review fix).
- Scope: the plan's ten files; the review fix touched only guard.py, fake_claude.py and test_guard.py.
- Code read in full for guard.py, role_dev.py and role_qa.py.

Finding (resolved): **the guard hook settings used a shape the real Claude Code ignores.** Plan 02-05 specified `{"matcher", "command"}` entries under `hooks.PreToolUse`; Claude Code's settings schema (as used by every hook in this repo's `.claude/settings.local.json`) is `{"matcher", "hooks": [{"type": "command", "command"}]}`. With the flat shape the real CLI would not run the guard, silently losing Dev's read limit (ROLE-02) and the `guard.jsonl` log; the fake accepted the flat shape, so no test failed. Fixed by `.planning/fixes/02-05-hook-settings-shape.plan.md` (`3bca897`, `b52dfc5`): brain writes the nested shape and the fake now runs only nested `type: command` hooks (`test_fake_ignores_flat_hook_shape`).

Not verified against the real CLI (no tokens spent): that `claude -p --settings <file>` with this hook actually runs the guard. Plan 02-07's live run must confirm that `<run_dir>/guard.jsonl` gets lines after a `brain dev` run; the 02-06 manual lists an empty guard log as a troubleshooting case. ROLE-06 stays open until then.

Minor notes, not blocking:

- Dev may edit any part of `.plan.md` (it is on the allowlist so it can write Vướng mắc). The allowlist and post-run check use the pre-run plan, so this cannot widen Dev's file access, but a Dev edit to the Thay đổi table would be committed under the task.
- `Bash(git commit *)` also matches `git commit --amend`; an amend of the plan commit would hide `<task>: plan` from `brain qa`. The post-run file check still applies.

## Issues Encountered

None beyond the finding above.

## User Setup Required

None.

## Next Phase Readiness

- Plan 02-06 (task log, installer, README, manual) next; its manual must include checking `guard.jsonl` after the first live `brain dev`.
- ROLE-05 marked complete. ROLE-02, ROLE-03 and ROLE-04 are also owned by plan 02-07's live acceptance; ROLE-06 waits for the live guard confirmation.
