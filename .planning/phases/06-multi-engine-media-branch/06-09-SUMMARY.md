---
phase: 06-multi-engine-media-branch
plan: 09
subsystem: engine
tags: [agy, antigravity, headless, stream-json, runner, quota, d-04, d-05]
requires:
  - phase: 06-02
    provides: "agy 1.4.7 probe (spikes/s7b/NOTES.md): Dev permission mode, timeout shape, leftover counts"
  - phase: 06-05
    provides: "engine seam (plan_for, EnginePlan)"
  - phase: 06-07
    provides: "quota::detect_agy, D2 rule"
provides:
  - "agy as a supervised headless engine for lead, dev, plan-review, code-review"
  - "runner stdout-to-file tail mode (RunRequest.stdout_file)"
  - "agy/* stream events and Summary fields (status, response, error, permission_mode, thinking_tokens)"
  - "agy Dev post-run checks: scope, .planning/, secret paths, empty diff (outcome agy:empty)"
affects: [06-11, 06-12, 06-19]
tech-stack:
  added: []
  patterns:
    - "child stdout is a file, tailed by the runner (never a pipe)"
    - "read-only roles answer with the whole protocol file; agent-brain writes it from result.response"
key-files:
  created:
    - agent-brain/tests/engine_agy.rs
  modified:
    - agent-brain/src/engine.rs
    - agent-brain/src/runner.rs
    - agent-brain/src/stream.rs
    - agent-brain/src/roles/mod.rs
    - agent-brain/src/roles/dev.rs
    - agent-brain/src/report.rs
    - agent-brain/src/autopilot/decide.rs
    - agent-brain/src/autopilot/view.rs
    - agent-brain/src/term.rs
    - agent-brain/src/dashboard/sse.rs
    - tests/fakes/fake_agy.py
key-decisions:
  - "AGY_DEV_PERMISSION = [--mode, accept-edits] (06-02 decision)"
  - "agy_expected_permission_mode is request-review for every role, Dev included (init does not reflect accept-edits)"
  - "print-timeout expiry is mapped to timed_out from the stderr line, not from status or exit code"
  - "No post-exit process-group sweep (leftover_after_exit=0 in the probe); runner.rs sweep and procs.rs untouched"
patterns-established:
  - "Tail mode: poll the stream file every 150 ms, split on newline, final unterminated line sent after exit"
requirements-completed: [ENG-06]
duration: 75min
completed: 2026-10-27
status: complete
commits: 3
plan_head_before: 0a85015847f718ae91fd65ecb2e9ecc66bbd7054
plan_head_after: 1a0421740661c3f9afc0eba939bfc07a7d474ff6
actuals:
  tokens: 14000
  tasks: 3
  commits: 3
---

# Phase 6 Plan 09: agy headless engine Summary

**agy runs through the same seam, supervisor and kill as claude: stdout goes straight to `stream.jsonl` and is tailed, `agy/*` events feed status, response and usage, Dev gets only `--mode accept-edits`, and every Dev diff is checked for scope, `.planning/`, secrets and emptiness.**

## What was built

- **Task 1 (tracer)** `ada7205`: `engine::plan_for` agy arm (`-p`, `--output-format stream-json`, `--print-timeout {N}s`, optional `--model`; env stripped of `ANTHROPIC_*`, `CLAUDE_CONFIG_DIR`, `CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR`, `CLAUDE_CODE_SUBAGENT_MODEL` and the DeepSeek key variable), `agy_print_timeout`, `response_file_for`. `RunRequest.stdout_file` opens `stream.jsonl` (0600, truncated) as the child's stdout and tails it (complete lines, last line without a newline after exit). `classify` maps an `event`-keyed object to `agy/<event>`; `Summary` reads init, DONE step usage (partial) and result (status, response, error, usage with thinking folded into output, `thinking_tokens` kept in `metrics_json`). `execute_role_staged` marks a non-SUCCESS result `failed` even when agy exits 0 and writes `.plan.md` / `.plan-review.md` / `.code-review.md` from `result.response` (outer code fence removed). The normal Lead checks then commit the plan (`sua-app: plan`).
- **Task 2** `2616c66`: `AGY_DEV_PERMISSION` appended for role `dev` only; for an agy Dev run any changed path under `.planning/` or matching `report::sensitive_path` (now `pub(crate)`) is a violation even when the plan lists it (exit 3); an empty changed set after SUCCESS is outcome `agy:empty`, exit 4, no commit. `decide.rs`: `agy:empty` stops with `VuongMac` and the detail line, `RunFact::is_ok` false.
- **Task 3** `1a04217`: `quota::detect_agy` runs for agy streams (outcome `quota`, exit 5, `metrics_json.quota.engine = agy`); the `[agy] print timeout after Ns ... returning partial output` stderr line maps a SUCCESS/exit-0 run to `timed_out`; `init.permission_mode` is compared with `agy_expected_permission_mode` and a mismatch is stored in `metrics_json.agy` and printed (`Cảnh báo: agy chạy với permission_mode ...`); term/sse render `agy/init`, `agy/step_update`, `agy/result`; fake agy gained `pidfile` and `permission_mode` keys.

## 06-02 values used (spikes/s7b/NOTES.md)

| Item | Value |
|------|-------|
| `AGY_DEV_PERMISSION` | `["--mode", "accept-edits"]` |
| `agy_expected_permission_mode` | `request-review` for every role (the flag is not reflected in `init`; the diff is the proof) |
| `--print-timeout` expiry | status SUCCESS, exit 0, empty response; only signal is the stderr line, mapped to `timed_out` |
| Post-exit sweep | **Not added.** `leftover_after_exit=0`, `survivors_after_kill=0`, `survivors_after_sweep=0`; `runner.rs` sweep, `RunRequest.sweep_group_after_exit` and `procs.rs` were left unchanged. The S4 kill (recorded PID, descendants, SIGTERM then sweep of the recorded list) still covers a hang (`agy_kill_s4`). |
| Bad model / SIGTERM | status ERROR, exit 1 (`failed`); `context canceled` on SIGTERM |

## Deviations from Plan

### Auto-fixed / added

**1. [Rule 2 - Missing critical functionality] `autopilot/view.rs` shows the `agy:empty` detail**
- **Found during:** Task 2
- **Issue:** `stop_detail` for `VuongMac` always printed the generic "Dev dừng vì vướng mắc" text, so the `agy:empty` detail from `decide.rs` would never reach the terminal or dashboard.
- **Fix:** `VuongMac` with a `detail_line` prints that line; the generic text is unchanged when there is none. Unit test added.
- **Files modified:** agent-brain/src/autopilot/view.rs (not in the plan's file list)
- **Commit:** 2616c66

**2. [Plan alignment with S7b] fake agy tool step shape**
- The orchestrator noted the real tool step uses `tool_name` + `tool_info`; `tests/fakes/fake_agy.py` now emits that shape instead of a `tool` object. The parser does not read tool fields (reads / edits stay NULL for agy runs, per plan).
- **Commit:** 1a04217

**3. [Minor] `tail_done` flag instead of a fixed 2 s tail wait**
- In file mode the tail task does a final read to EOF as soon as the child has exited (flag set before the drain), so the 2 s drain is only an upper bound. Behavior is the same (complete lines, trailing line without newline).

No other deviations.

## Verification

- `cargo test --offline --locked --test engine_agy -- --test-threads=1`: 13 passed.
- lib `stream:: engine::` and `engine:: autopilot::`: pass; `runner_kill`, `cli_stream`, `contract_stream`, `cli_lead`, `cli_dev`, `cli_fix`, `dashboard_stream`: pass.
- `cargo fmt --check` clean; `git diff 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock` empty; no `pkill`/`killall` in the fake or tests.
- Full suite `cargo test --offline --locked -- --test-threads=1`: all 57 test binaries report `ok`, no FAILED / panicked lines (no contract_guard flake this run).

## Self-Check: PASSED

- FOUND: agent-brain/tests/engine_agy.rs, agent-brain/src/engine.rs, tests/fakes/fake_agy.py
- FOUND commits: ada7205, 2616c66, 1a04217

## Known Stubs

None.

## Threat Flags

None beyond the plan's threat model (T-06-31 to T-06-36 are covered: Dev-only flag, scope/`.planning/`/secret checks, empty-diff stop, env stripping, S4 kill without name-based killing).

## Limits

- Real quota text of agy is still unobserved (A5); `detect_agy` keeps its community-evidence rule.
- `run_command` under `accept-edits` was not probed (S7b limit): an agy Dev may not be able to run tests.
- `reads` / `edits` / `turns_before_first_edit` / `reads_outside_plan` stay NULL for agy runs.
