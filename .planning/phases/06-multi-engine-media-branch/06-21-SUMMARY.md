---
phase: 06-multi-engine-media-branch
plan: 21
subsystem: engines
tags: [rust, agent-brain, secrets, redaction, gateway-keys, eng-05]
requires:
  - phase: 06-multi-engine-media-branch
    provides: "EnginePlan.redact, EnginePlan.db_engine, Engine::Named, anthropic kind with per-run key injection (06-11)"
provides:
  - "agent_brain::secrets: REDACTED, MIN_SECRET_BYTES, redact_line, redact_str, redact_file, SECRET_SCAN_MAX_BYTES, scan_for_secrets"
  - "runner::RunRequest.redact: every stdout line scrubbed before stream.jsonl, the observer (events.raw_json, terminal, dashboard job output); stderr.txt rewritten on every exit path"
  - "post-run key scan at the run seam: outcome protocol_violation, metrics_json.secret_leak, exit 3, nothing committed"
  - "fake claude scenario op echo_env and scenario key sleep_after"
affects: [06-22, 06-23, 06-25, 06-26]

plan_head_before: b3e08c82ef4750f6048ced23a84dbbe67dd1ff5f
plan_head_after: 7880ce2d8e68f68dc6f44e3a28103654eaeab734

actuals:
  tokens: 13600
  tasks: 3
  commits: 5

tech-stack:
  added: []
  patterns:
    - "one per-run secret list (the run's own EnginePlan.redact) drives every filter, so two runs on different engines never see each other's key"
    - "filter before write: lines are scrubbed in the runner, ahead of the file, the observer and the drain loop; the one sink the child writes itself (stderr.txt) is rewritten once the tree is gone, with a drop guard for error paths"
    - "scan the project, not only agent-brain's own output: changed_paths plus diff_names(base, HEAD) are read for the key before the role may commit"

key-files:
  created:
    - agent-brain/src/secrets.rs
    - agent-brain/tests/engine_secrets.rs
  modified:
    - agent-brain/src/lib.rs
    - agent-brain/src/runner.rs
    - agent-brain/src/roles/mod.rs
    - tests/fakes/fake_claude.py

key-decisions:
  - "Patterns are the secret and its JSON-escaped spelling (when different), longest first; a secret under 8 bytes is ignored exactly as the plan says"
  - "If stderr.txt cannot be rewritten the file is removed and the failure is reported through on_notice: an absent log beats a log that holds a key"
  - "The scan runs for every run that ended by itself (succeeded or failed), skips killed and timed-out runs, and returns before the role code can commit; the error text is paths only and is re-redacted"
  - "engines.rs is untouched; the scan lives in secrets.rs and roles/mod.rs"

patterns-established:
  - "Test hygiene: failure messages go through safe() and name files or table.column only, never a planted value"

requirements-completed: [ENG-05]

coverage:
  - id: D1
    description: "a gateway Dev run whose model echoes its key leaves it in no run-dir file, no database column, no CLI output; stream.jsonl, stderr.txt and events.raw_json carry [đã ẩn] instead"
    requirement: ENG-05
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_secrets.rs#eng05_key_absent_everywhere"
        status: pass
      - kind: unit
        ref: "agent-brain/src/secrets.rs#secrets::tests::redact_line_replaces_every_occurrence"
        status: pass
    human_judgment: false
  - id: D2
    description: "runs.error and the quota message are built from redacted text"
    requirement: ENG-05
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_secrets.rs#eng05_quota_detail_redacted"
        status: pass
    human_judgment: false
  - id: D3
    description: "a killed run, a timed-out run and a dashboard-started run leave no key on disk or in the job output"
    requirement: ENG-05
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_secrets.rs#eng05_killed_run_redacted"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_secrets.rs#eng05_timeout_run_redacted"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_secrets.rs#eng05_job_output_redacted"
        status: pass
    human_judgment: false
  - id: D4
    description: "with two gateway keys exported, key B is in no sink of a run on gw, on claude or on agy, and no child env holds a key variable"
    requirement: ENG-05
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_secrets.rs#eng05_two_keys_isolated"
        status: pass
    human_judgment: false
  - id: D5
    description: "an engine without secrets pays nothing: a claude stream.jsonl equals the fake's stdout byte for byte; stdout-file mode refuses a non-empty redact list"
    requirement: ENG-05
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_secrets.rs#claude_stream_unchanged_without_secrets"
        status: pass
    human_judgment: false
  - id: D6
    description: "an agent that copies a key into a file (uncommitted or committed) cannot get it committed: protocol_violation, secret_leak paths, exit 3, message with paths only; the dry run prints no key"
    requirement: ENG-05
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_secrets.rs#eng05_key_written_to_plan_not_committed"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_secrets.rs#eng05_dev_file_with_key_not_committed"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_secrets.rs#eng05_committed_key_detected"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_secrets.rs#dry_run_has_no_key"
        status: pass
      - kind: unit
        ref: "agent-brain/src/secrets.rs#secrets::tests::scan_rules"
        status: pass
    human_judgment: false
  - id: D7
    description: "Against a REAL gateway the redaction holds for the real claude stream (key formats, partial lines, tool results that quote the environment)"
    verification: []
    human_judgment: true
    rationale: "Every test uses the fake claude and planted values; only the user-run E-09 of plan 06-20 sees a real gateway response"

duration: 45min
completed: 2026-10-28
status: complete
---

# Phase 6 Plan 21: Engine key redaction and the pre-commit key scan Summary

**Each run scrubs only its own engine's key (the secret and its JSON-escaped spelling) from every stdout line before it is written or shown, rewrites `stderr.txt` on every exit path, and refuses to commit a file that contains a key (outcome `protocol_violation`, `metrics_json.secret_leak`, exit 3).**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-10-28T01:55Z (approximate; first commit 02:11Z)
- **Completed:** 2026-10-28T02:40Z
- **Tasks:** 3 (1 tracer, 2 auto; all `tdd="true"`)
- **Files modified:** 6 (2 created)

## Accomplishments

- `secrets.rs`: `redact_line` (byte level, secrets of at least 8 bytes, JSON-escaped spelling too, longest first), `redact_str`, `redact_file` (temp file in the same directory, mode 0600, rename; no-op for an empty list, a missing file or a clean file), `scan_for_secrets` (regular files up to 1 MiB, input order, paths only).
- `runner.rs`: `RunRequest.redact`; lines are scrubbed in the main loop and the drain loop before `stream.jsonl`, `obs.on_line` (so `events.raw_json`, the terminal and the dashboard job output); `stderr.txt` is rewritten after the child tree is gone, with a `Drop` guard for error returns; stdout-file mode refuses a non-empty list with `Engine ghi stdout ra file không được mang key — lỗi cấu hình engine`. With an empty list the line is passed through uncopied.
- `roles/mod.rs`: passes `ep.redact` into the one `RunRequest`; `runs.error` and the quota exit message are re-redacted; after a run that ended by itself, `changed_paths` plus `diff_names(base, HEAD)` are scanned before any role code commits.
- Fake claude: `echo_env` op (stdout event, stderr line or file, optional `append`) and scenario key `sleep_after` (stay alive after the events, for kill and timeout tests).
- `tests/engine_secrets.rs`: 11 integration tests walking every run-dir file and every database column for the planted values.

## Task Commits

1. **Task 1 (tracer): redact engine keys before every sink**
   - RED `5d988a9` (test)
   - GREEN `6e4468a` (feat)
2. **Task 2: redaction under kill, timeout, job output and two keys** - `871adc0` (test; no production change was needed)
3. **Task 3: scan for engine keys before committing**
   - RED `60bb867` (test)
   - GREEN `7880ce2` (feat)

**Plan metadata:** the `docs(06-21)` commit that adds this file.

## TDD Gate Compliance

`test(06-21)` precedes `feat(06-21)` for tasks 1 and 3; task 2 is `test(06-21)` only (the plan allows this when `runner.rs` does not change).

- **RED, task 1:** `eng05_key_absent_everywhere` failed on `key found in run dir files: [... stderr.txt, ... stream.jsonl]`, the planned assertion. `eng05_quota_detail_redacted` failed on `runs.error not redacted`, but for a test-design reason: the scenario carried `api_error_status: 429`, so the detail was `HTTP 429` and never quoted the text. The scenario was corrected after GREEN (the `api_error_status` field removed so the `blocking_limit` text path is used) and the test was then verified by mutation: with the line filter disabled both tests fail on `key found in run dir files: [... stream.jsonl]`. Evidence is cargo's plain-text report; `gsd_run check tdd-red-evidence` does not support it, so the assessment is by inspection.
- **RED, task 3:** the three leak tests failed with `left: 0 right: 3` (exit 0 and the file was committed), the planned assertion. `dry_run_has_no_key` already held (the key lives only in the child env, never in argv) and is not claimed as RED.
- **Task 2:** all five tests passed on the first run because task 1 already implemented the behaviour. Teeth were checked by mutation: replacing the secret list in `scrub_stderr_file` with an empty one makes `eng05_key_absent_everywhere`, `eng05_killed_run_redacted`, `eng05_timeout_run_redacted` and `eng05_job_output_redacted` fail on `key found in run dir files: [... stderr.txt]`.
- **Not split:** the `secrets::tests::*` unit tests were added in the GREEN commits (a unit test of a function that does not exist cannot compile).

## Files Created/Modified

- `agent-brain/src/secrets.rs` - redaction and scan helpers with unit tests (new)
- `agent-brain/src/lib.rs` - `pub mod secrets;`
- `agent-brain/src/runner.rs` - `RunRequest.redact`, line filter, `StderrGuard`, `scrub_stderr_file`, file-mode refusal
- `agent-brain/src/roles/mod.rs` - `redact: ep.redact.clone()`, re-redacted error texts, the post-run scan
- `tests/fakes/fake_claude.py` - `echo_env`, `sleep_after`
- `agent-brain/tests/engine_secrets.rs` - 11 tests (new)

## Decisions Made

- JSON-escaped spelling of a secret is also a pattern: a stream line is JSON, so a key containing a quote or backslash would otherwise survive.
- A failed `stderr.txt` rewrite deletes the file and reports it, rather than leaving a log with a key.
- `agent-brain/src/engine.rs` is untouched (acceptance criterion: no commit of this plan touches it).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fake claude sleeps before it prints, so "echo then sleep" was impossible**
- **Found during:** Task 1 (planning the kill and timeout tests)
- **Issue:** the existing `sleep` scenario key runs before any event is printed; a kill or timeout would then leave nothing on stdout to redact.
- **Fix:** added scenario key `sleep_after` (sleep after the events, before the result). Same file the plan already lists.
- **Files modified:** `tests/fakes/fake_claude.py`
- **Commit:** `5d988a9`

**2. [Rule 2 - Missing critical] Defence beyond the plan text**
- **Found during:** Task 1
- **Issue:** the plan names `runs.error`; the quota exit message printed on the CLI is built from the same detail, and a failed `stderr.txt` rewrite or an error return could still leave a key on disk.
- **Fix:** `quota::user_message` output goes through `redact_str`; `StderrGuard` (drop guard) rewrites `stderr.txt` on error returns; a failed rewrite removes the file; JSON-escaped spelling added to the patterns.
- **Files modified:** `agent-brain/src/runner.rs`, `agent-brain/src/roles/mod.rs`, `agent-brain/src/secrets.rs`
- **Commit:** `6e4468a`

**3. [Plan scope] One extra test and one weaker agy check**
- `eng05_committed_key_detected` (the agent commits the key itself; proves the `diff_names(base, HEAD)` half of the scan) is beyond the plan's list.
- The fake agy cannot echo its environment, so for the agy run `eng05_two_keys_isolated` asserts the recorded env NAMES (no `GW_KEY`, `GW_B_KEY`, `ANTHROPIC_AUTH_TOKEN`) plus key B in no sink; the fake claude runs do echo key B's variable.

**Total deviations:** 3 (1 Rule 3, 1 Rule 2, 1 scope note). **Impact:** none on the plan's contract.

## Issues Encountered

- The first full-suite run in the foreground hit the 600 s tool cap; it was re-run in the background and finished with every result line `ok` (489 lib tests, 60 result lines, 0 failed). The `contract_guard` BrokenPipe flake did not occur.
- `cargo fmt --check` clean; `Cargo.toml` and `Cargo.lock` identical to `574b6cd`; `spikes/lib/scan-secrets.sh` prints `scan-secrets: OK`; `git grep -E "sk-[A-Za-z0-9]{20,}"` finds nothing.

## Known Stubs

None.

## Known limits (stated, not hidden)

- A secret under 8 bytes is not redacted (plan rule); a file over 1 MiB is not scanned (plan rule).
- The scan does not run for a killed or timed-out run, and does not run when the run stopped on quota (nothing is committed on a quota stop anyway).
- Research A10 (`--disallowedTools "Read(//proc/**)"`) is not added; the post-run scan is the guard against a key copied into a file.

## Threat Flags

None. Register mitigations: T-06-42 (`eng05_key_absent_everywhere`, `eng05_job_output_redacted`), T-06-43 (`eng05_killed_run_redacted`, `eng05_timeout_run_redacted`, `eng05_quota_detail_redacted`), T-06-44 (`eng05_key_written_to_plan_not_committed`, `eng05_dev_file_with_key_not_committed`, `eng05_committed_key_detected`), T-06-79 (`eng05_two_keys_isolated`), T-06-SC (no dependency added).

## User Setup Required

None.

## Next Phase Readiness

- 06-23 can assert that a dsh run carries no key against the same `redact` plumbing; an empty list stays free.
- No blockers.

## Self-Check: PASSED

- Files present: `secrets.rs`, `engine_secrets.rs`, `runner.rs`, `fake_claude.py`, this SUMMARY.
- Commits in history (listed by `git log b3e08c8..HEAD`): `5d988a9`, `6e4468a`, `871adc0`, `60bb867`, `7880ce2`.
- Verify commands re-run after the last code commit: `engine_secrets` 11/11, `secrets::` unit tests 6/6, full suite every result line `ok`, `cargo fmt --check` clean, Cargo files identical to `574b6cd`, `scan-secrets: OK`, `engine.rs` untouched by this plan.
