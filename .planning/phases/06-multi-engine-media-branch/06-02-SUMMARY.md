---
phase: 06-multi-engine-media-branch
plan: 02
subsystem: testing
tags: [agy, antigravity, headless, spike, fake-agy, permissions, stream-json]

requires:
  - phase: 03-pipeline
    provides: "spikes/lib/env.sh, S4 kill technique, S7 agy 1.3.5 evidence"
provides:
  - "tests/fakes/fake_agy.py scenario-driven fake agy reused by plan 06-09"
  - "spikes/s7b/run.sh and verify.sh probes for agy 1.4.7 (S7B_OFFLINE self-test)"
  - "Real agy 1.4.7 evidence: soft-deny, accept-edits, sandbox, print-timeout, bad model, SIGTERM, process tree"
  - "Dev permission decision: accept-edits (D-26)"
  - "tests/fixtures/stream/agy_tool_run.jsonl and agy_error.jsonl real-shape fixtures"
affects: [06-09, agy engine arm, ENG-06]

actuals:
  tokens: 14500
  tasks: 3
  commits: 4
plan_head_before: 2a49c4bf93a063f40fd647af0743cda244b95ae8
plan_head_after: faa52aa

tech-stack:
  added: []
  patterns:
    - "Probe scripts with an offline self-test against a scenario-driven fake"
    - "Every agy call: timeout + </dev/null + stdout to file + explicit --print-timeout"

key-files:
  created:
    - tests/fakes/fake_agy.py
    - spikes/s7b/run.sh
    - spikes/s7b/verify.sh
    - spikes/s7b/NOTES.md
    - spikes/s7b/raw/ (31 scrubbed evidence files)
    - tests/fixtures/stream/agy_tool_run.jsonl
    - tests/fixtures/stream/agy_error.jsonl
  modified: []

key-decisions:
  - "agy Dev uses --mode accept-edits (narrowest mode that wrote the file; sandbox soft-denied)"
  - "Plan 06-09 must expect init.permission_mode request-review for Dev even with --mode accept-edits"
  - "Plan 06-09 must detect --print-timeout expiry from stderr; status is SUCCESS and exit is 0"

requirements-completed: [ENG-06]

duration: 3 sessions (Task 1 earlier, Tasks 2-3 continuation)
completed: 2026-10-27
status: complete
---

# Phase 6 Plan 02: agy 1.4.7 probe Summary

**Real agy 1.4.7 headless evidence (soft-deny, accept-edits writes files, timeout and error shapes, clean process tree), a scenario-driven fake agy, and the decision that agy Dev runs with `--mode accept-edits`.**

## Performance

- **Completed:** 2026-10-27
- **Tasks:** 3 (tracer, human-action, decision)
- **Files created:** 4 authored plus 33 evidence/fixture files

## Accomplishments

- `tests/fakes/fake_agy.py`, `spikes/s7b/run.sh` and `verify.sh` pass `S7B_OFFLINE=1 bash spikes/s7b/run.sh` (Task 1, earlier run).
- The user's real probe on logged-in agy 1.4.7 (09:23:30Z to 09:24:52Z, exit 0, no authentication message, P5 skip not run) is committed scrubbed, with `scan-secrets: OK`.
- `spikes/s7b/NOTES.md` (Vietnamese) records the table of results, the decision `accept-edits`, and the three facts 06-09 needs.

## Task Commits

1. **Task 1 (tracer): fake agy and probe scripts** - `68dc9d9` (test)
2. **Task 2: probe evidence (scrubbed)** - `72cfae3` (docs); 31 files under `spikes/s7b/raw/` plus the two fixtures, nothing else
3. **Task 3: probe notes and Dev permission decision** - `faa52aa` (docs)

**Plan metadata:** the SUMMARY commit follows this file. The orchestrator owns STATE.md, ROADMAP.md and REQUIREMENTS.md, so they were not touched.

## Verify lines (from `spikes/s7b/raw/s7b-verify.txt`)

| Probe | permission_mode | file | status | exit | stderr head |
|-------|-----------------|------|--------|------|-------------|
| default | request-review | no | SUCCESS | 0 | `jetski: no output produced — a tool required the "write_file" permission that headless mode cannot prompt for, so it was auto-denied.` |
| accept_edits | request-review | yes | SUCCESS | 0 | none |
| sandbox | request-review | no | SUCCESS | 0 | same soft-deny text |
| timeout | request-review | no | SUCCESS | 0 | `[agy] print timeout after 5s with turn in progress; returning partial output` |
| badmodel | none | no | ERROR | 1 | `error: invalid model selection (--model "khong-co-model-nay" --effort "")…` |
| procs_kill | none | no | ERROR (`context canceled`) | 1 | `error: context canceled` |

Tool step keys: `conversation_id,state,step_index,step_type,tool_info,tool_name`. Process counters: `leftover_after_exit=0`, `survivors_after_kill=0`, `survivors_after_sweep=0`.

## Decision

`## Quyết định` = `accept-edits` (user reply "accept-edits"). Implications for plan 06-09:

- `init.permission_mode` stays `request-review` under `--mode accept-edits`; expect `request-review` for Dev and confirm the flag took effect only through the git diff scope and empty-diff checks (D-05).
- `--print-timeout` expiry returns `SUCCESS`, exit 0, empty response; detect it from the stderr line, not status or exit.
- No post-exit process sweep is needed (all counters 0); keep the S4 technique as a safety net.
- Fake/real difference: real tool steps carry `tool_name` and `tool_info`, not the `tool` object that `fake_agy.py` emits. 06-09 should align the fake or the parser.
- `--sandbox` cannot edit files (soft-denied), so it is not a Dev option.

## Deviations from Plan

None - plan executed exactly as written. Two procedural notes:

- No per-plan commit ledger file existed for 06-02 (Task 1 ran in an earlier session), so `plan_head_before` is the parent of Task 1's commit (`68dc9d9^`). `commits: 4` counts this plan's own commits (68dc9d9, 72cfae3, faa52aa, and the SUMMARY commit); `git rev-list` over that range would also count other plans' interleaved commits.
- The new probe output includes extra file kinds beyond the plan's list (`s7b-procs.jsonl`, `s7b-procs-kill.*`, `*.stderr.txt`); all sit under `spikes/s7b/raw/` and were committed by name.

## Issues Encountered

None. An unexpected extra stderr text in `badmodel` (list of available models) is the agy error body; no secret content (scan-secrets OK, no home path or email).

## Known Stubs

None.

## Threat Flags

None. The raw evidence keeps `~`-relative paths only; T-06-04 to T-06-06 mitigations held (scratch repo outside this repo, no skip flag run, scrubbed output, scan-secrets OK).

## User Setup Required

None beyond the agy login the user already has.

## Next Phase Readiness

Plan 06-09 can implement the agy engine arm against the recorded shapes. Open: agy quota text (A5) and `--dangerously-skip-permissions` behaviour on 1.4.7 are unobserved.

## Self-Check: PASSED

- FOUND: tests/fakes/fake_agy.py, spikes/s7b/run.sh, spikes/s7b/verify.sh, spikes/s7b/NOTES.md, spikes/s7b/raw/s7b-verify.txt, tests/fixtures/stream/agy_tool_run.jsonl, tests/fixtures/stream/agy_error.jsonl
- FOUND commits: 68dc9d9, 72cfae3, faa52aa

---
*Phase: 06-multi-engine-media-branch*
*Completed: 2026-10-27*
