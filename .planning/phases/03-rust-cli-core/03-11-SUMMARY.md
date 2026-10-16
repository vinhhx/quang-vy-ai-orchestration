---
phase: 03-rust-cli-core
plan: 11
subsystem: roles-dev-qa
tags: [rust, dev, qa, guard-hook, pipeline, protocol]

requires:
  - phase: 03-rust-cli-core
    provides: "03-10 execute_role, display_argv, tests/support/protocol.rs; 03-07 guard + write_hook_settings; 03-06 parse_qa_verdict"
provides:
  - "`agent-brain run dev`: plan-file allowlist (Read/Edit/Write rules, relative and absolute), DEV_DISALLOWED_TOOLS as Phase 2, guard hook = the binary itself via current_exe + --settings, post-run git check (path outside plan → exit 3, nothing reverted), Vướng mắc → commit and exit 4, timeout → no commit, plan commands never executed"
  - "`agent-brain run qa`: report-only QA over <plan commit>..HEAD, verdict Đạt / Cần sửa, code edit → exit 3, no verdict → exit 1, Cần sửa prints next steps"
  - "Full lead → dev → qa pipeline through the binary (ROLE-05)"
affects: [03-12, 03-13]

actuals:
  tasks: 2
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Dev sandbox = tool rules + guard hook + post-run git diff check (three layers, as Phase 2)"

key-files:
  created:
    - agent-brain/tests/cli_dev.rs
    - agent-brain/tests/cli_qa.rs
  modified:
    - agent-brain/src/roles/dev.rs
    - agent-brain/src/roles/qa.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, gemini-3.8-flash-high); reviewed by Claude"

patterns-established:
  - "Role code runs git only through gitops; no Command::new in roles/dev.rs"

requirements-completed: [CLI-03]

duration: 15min
completed: 2026-10-16
status: complete
---

# Phase 3 Plan 11: Dev and QA Parity Summary

**Dev and QA now run through the binary with Phase 2's protections: Dev can touch only the plan's files (tool rules, the `agent-brain guard` hook and a post-run git check), stops with Vướng mắc, and QA only writes its report; a whole lead → dev → qa pipeline passes end to end.**

## Performance

- **Duration:** 15 min (agy run 861 s)
- **Completed:** 2026-10-16
- **Tasks:** 2
- **Files:** 4

## Task Commits

1. `f1c1446` test(03-11): add failing dev parity tests
2. `f92e456` feat(03-11): dev role parity
3. `19be51b` test(03-11): add failing qa and pipeline tests
4. `f6a061b` feat(03-11): qa role parity and pipeline

## Deviations from Plan

None.

## Review (Claude)

Re-run by the reviewer:

- Acceptance: `cli_dev` 13 ok (≥ 13), including `p1_07_dev_edits_only_plan_files`, `p1_08_dev_blocked_writes_vuong_mac`, `dev_guard_blocks_read_outside_plan`, `dev_backstop_exit_3_when_hooks_ignored`, `dev_never_runs_plan_commands`; `cli_qa` named tests 7/7 incl. `role_05_pipeline_lead_dev_qa`; `write_hook_settings(` and `current_exe` in dev.rs; no `Command::new` in dev.rs; `find_commit_by_subject(` in qa.rs; `cargo fmt --check` clean.
- Full cargo suite passes twice (170 tests across 19 binaries); Python suite OK; no `FAKE_CLAUDE` or test-only logic in `src/`.
- Code read: Dev's allowed and disallowed tools match `brain/role_dev.py` (same six disallowed tools; Read/Edit/Write per plan file plus absolute rules); QA finds `<task>: plan` by subject and diffs from it.
- Scope: the plan's four files.

## Issues Encountered

None.

## Next Phase Readiness

03-12 builds the static release binary and the live-check material; 03-13's live run exercises the real guard hook with the real CLI.
