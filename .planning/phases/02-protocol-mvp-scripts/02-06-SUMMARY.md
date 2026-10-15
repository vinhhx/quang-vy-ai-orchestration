---
phase: 02-protocol-mvp-scripts
plan: 06
subsystem: task-log-and-docs
tags: [python, stdlib, stream-json, task-log, installer, docs]

requires:
  - phase: 02-protocol-mvp-scripts
    provides: "02-01 runner (execute_role); 02-02 planfile; 02-05 guard.jsonl"
  - phase: 01-verify-assumptions
    provides: "spikes/s1/raw/s1.jsonl real stream-json capture; spike-results S1, S4, S5, S8"
provides:
  - "brain/streamlog.py: stream-json summary (turns before first edit, reads outside plan, denials), tolerant of malformed lines"
  - "brain/tasklog.py: per-task log in the spec §7 shape, record_run hook in execute_role, `python3 -m brain.tasklog check <dir> --min N` gate"
  - "brain/install.sh (symlink into BRAIN_BIN_DIR, --uninstall), brain/README.md, tests/protocol-manual.md, protocol-log/nghiem-thu-p1.md"
affects: [02-07, 03-rust-cli-core, 04-session-token-optimization]

actuals:
  tasks: 3
  commits: 4

tech-stack:
  added: []
  patterns:
    - "task log never stores raw stream content (sentinel test)"
    - "runner hook wrapped so a logging failure never changes a run's outcome"

key-files:
  created:
    - brain/streamlog.py
    - brain/tasklog.py
    - brain/templates/tasklog.md
    - brain/install.sh
    - brain/README.md
    - tests/protocol-manual.md
    - protocol-log/nghiem-thu-p1.md
    - tests/test_streamlog.py
    - tests/test_tasklog.py
    - tests/fixtures/stream/dev_two_reads_then_edit.jsonl
    - tests/fixtures/stream/parallel_reads.jsonl
    - tests/fixtures/stream/dev_blocked.jsonl
    - tests/fixtures/stream/malformed_lines.jsonl
  modified:
    - brain/agent.py

key-decisions:
  - "Executed by Antigravity (agy, gemini-3.8-flash-high); reviewed by Claude"
  - "Manual adds a required guard.jsonl check after the first live brain dev (stop before P1-08 if empty) and the re-plan warning (reviewer's additions)"

patterns-established:
  - "Each CLI assumption A-xx is listed with its Phase 1 evidence and the config key to change if it is wrong"

requirements-completed: []

duration: 15min
completed: 2026-10-15
status: complete
---

# Phase 2 Plan 06: Task Log, Installer and Docs Summary

**Every role run now writes a spec §7 task log line set: Dev's turns before its first edit, whether it read outside the plan, and blocked calls. There's a 5-task gate check, a one-command installer, a README, and a Vietnamese live-acceptance manual for P1-01..P1-10.**

## Performance

- **Duration:** 15 min (agy run 868 s)
- **Completed:** 2026-10-15
- **Tasks:** 3
- **Files modified:** 14

## Accomplishments

- `streamlog.summarize` counts assistant turns before the first edit, lists reads outside the plan, counts tool use and permission denials, and survives malformed lines. It passes on the real Phase 1 capture `spikes/s1/raw/s1.jsonl` (the test ran, not skipped).
- `tasklog.record_run` is called at the end of `execute_role` on normal exit, timeout and Ctrl+C, inside a try so a log failure never changes the run. Blocked calls = `permission_denials` + `block` lines in `guard.jsonl`. Writes are atomic. Raw stream content never reaches the log (sentinel test).
- `python3 -m brain.tasklog check protocol-log --min 5` is the phase gate for "5 real tasks".
- `brain/install.sh` symlinks `brain` into `BRAIN_BIN_DIR` (default `~/.local/bin`), prints the PATH hint, refuses to overwrite a non-symlink, and `--uninstall` removes it.
- `tests/protocol-manual.md`: preparation (scan, hand-edit names/aliases/profile, `~/.claude-orca` session warning), one section per P1-01..P1-10, the required `guard.jsonl` check after the first `brain dev`, the re-plan warning, 5 real tasks, and troubleshooting. `protocol-log/nghiem-thu-p1.md` has the ten acceptance rows, all `chưa chạy`.

## Task Commits

1. **Task 1:** `df74be2` feat(02-06): streamlog parser for turn counting and read/edit extraction
2. **Task 2:** `4e562b0` feat(02-06): task log per-task recording, gate check, and runner hook
3. **Task 3:** `a09a29c` docs(02-06): installer, user README, live manual, and acceptance table
4. **Reviewer docs fix:** `73aa8e8` docs(02-06): correct Phase 1 status of CLI assumptions A-03, A-04, A-06, A-07

## Files Created/Modified

- `brain/streamlog.py`, `brain/tasklog.py`, `brain/templates/tasklog.md` (spec §7 block verbatim)
- `brain/agent.py` - one guarded `record_run` call at each exit of `execute_role`
- `brain/install.sh`, `brain/README.md`, `tests/protocol-manual.md`, `protocol-log/nghiem-thu-p1.md`
- `tests/test_streamlog.py`, `tests/test_tasklog.py`, `tests/fixtures/stream/*.jsonl`

## Decisions Made

None beyond the plan and the two reviewer-requested manual additions.

## Deviations from Plan

- Tasks 1 and 2 were committed as one commit each (tests and implementation together) instead of a separate failing-test commit first. The tests exist and pass; the test-first order cannot be shown from history. Not worth rewriting.

## Review (Claude)

Re-run by the reviewer:

- Full suite: 103 tests, OK; `test_real_s1_fixture` ran and passed.
- All twelve acceptance-criteria commands pass, including the verbatim §7 template diff, the empty-dir gate exiting 1, `bash -n` on the installer, every command and exit code 130 in the README, P1-01..P1-10 in the manual and ten rows in the acceptance table.
- Installer verify run in a temp `BRAIN_BIN_DIR`: install, `brain --help`, uninstall all work. Nothing was installed into the real `~/.local/bin`.
- `brain/agent.py` diff: only the guarded `record_run` calls.
- No personal paths or secrets in the diff (README examples use `/home/user/...` placeholders).

Finding (fixed by the reviewer as a docs-only change, `73aa8e8`): **the README overstated what Phase 1 confirmed.** It marked A-03, A-04, A-06 and A-07 "Đã xác nhận". Per `spikes/spike-results.md`: S1/S4 used a single `--allowedTools` value only (A-03 partial); Read/Grep/Glob permissions were never tested (A-04 not verified); the personal profile was never run live and E-07 was not tried (A-06 partial); no `--model` alias or `--strict-mcp-config` was passed (A-07 not verified). The table now says so and points each one at the live check that settles it.

## Issues Encountered

None.

## User Setup Required

None yet. Plan 02-07's live run starts with `bash brain/install.sh`.

## Next Phase Readiness

- Plan 02-07: Task 1 (integration preflight, assumption reconciliation) is automated; Task 2 is the user's live acceptance P1-01..P1-10 and 5 real tasks with their own Claude accounts.
- ROLE-07 (task log) is also owned by 02-07's 5-task gate, so it stays open until then.
