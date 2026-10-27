---
phase: 06-multi-engine-media-branch
plan: 01
subsystem: testing
tags: [e09, deepseek, claude-cli, stream-json, spike, secret-scan, offline-self-test]

requires:
  - phase: 01-spikes
    provides: spikes/lib/env.sh, scrub.py, scan-secrets.sh and the S7 run.sh/verify.sh style
provides:
  - spikes/e09/run.sh: E-09 probe A (real) + B (invalid token), retry knobs, scrub + leak gate, fixture copy, E09_OFFLINE self-test
  - spikes/e09/verify.sh: name=value observations per probe, never prints the key
  - spikes/e09/NOTES.md: D-02 outcome recorded as `fail` (E-09 skipped by the user, not run)
affects: [06-11 deepseek engine, 06-03 config defaults for engines.deepseek.models]

actuals:
  tokens: 4200
  tasks: 3
  commits: 2
plan_head_before: 2a49c4bf93a063f40fd647af0743cda244b95ae8
plan_head_after: 894ff8ba98a29142ffaaa341c93dc42d91a95bb3

tech-stack:
  added: []
  patterns:
    - "Token reaches the claude child only through the ANTHROPIC_AUTH_TOKEN environment variable (indirect expansion inside a subshell), never on argv"
    - "Probe output staged outside the repo, scrubbed, leak-checked with grep -F, then copied; scan-secrets.sh runs before any fixture is written"
    - "Offline self-test against tests/fakes/fake_claude.py with a planted key, including a negative test of the leak gate"

key-files:
  created:
    - spikes/e09/run.sh
    - spikes/e09/verify.sh
    - spikes/e09/NOTES.md
  modified: []

key-decisions:
  - "D-02 recorded as `fail` because the user skipped the real E-09 probe (skip-e09, 2026-10-27); this is a not-run gate, not a measured failure, so plan 06-11 waits for E-09 to be run or for a dsh-vs-drop decision"

patterns-established:
  - "E-09 real run: export DEEPSEEK_API_KEY in the user's own shell, then bash spikes/e09/run.sh"

# Not copied from the plan's requirements field on purpose: this plan only produced the evidence tooling, and the
# real E-09 evidence does not exist, so ENG-01 and ENG-05 are NOT satisfied by this plan.
requirements-completed: []

coverage:
  - id: D1
    description: "E-09 probe pipeline (probes A/B/C, verify, scrub, leak gate, fixture copy) proven offline against the fake claude with a planted key"
    requirement: "ENG-05"
    verification:
      - kind: integration
        ref: "E09_OFFLINE=1 bash spikes/e09/run.sh (prints E09 offline OK, git status unchanged)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Real E-09 observations through DeepSeek's Anthropic-format endpoint, scrubbed raw evidence and the two stream fixtures"
    requirement: "ENG-01"
    verification: []
    human_judgment: true
    rationale: "Needs the user's own DeepSeek key; the user skipped the probe, so nothing was observed and no evidence or fixture exists"

duration: 25min
completed: 2026-10-27
status: complete
---

# Phase 6 Plan 01: E-09 DeepSeek gate Summary

**The E-09 probe tooling (real probe A, no-billing invalid-token probe B, scrub, `grep -F` leak gate, fixture copy) is built and proven offline against the fake claude with a planted key, but the real probe was skipped by the user, so D-02 is recorded as `fail` (not run) and plan 06-11 stays blocked.**

Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6).

## Performance

- **Duration:** about 25 min (includes the user's checkpoint round trip)
- **Tasks:** 3 (Task 1 done, Task 2 skipped by the user, Task 3 recorded as `fail`)
- **Files created:** 3 (`spikes/e09/run.sh`, `spikes/e09/verify.sh`, `spikes/e09/NOTES.md`)

## Accomplishments

- `spikes/e09/run.sh`: one bounded `timeout 180` claude call path (`e09_call`), stdin from `/dev/null`, outputs to files. Probe A uses `ANTHROPIC_BASE_URL=https://api.deepseek.com/anthropic` with the key only in the `ANTHROPIC_AUTH_TOKEN` environment of the child. Probe B uses the fixed token `invalid-key-for-e09-probe`. Probe A is retried once with `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1 CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING=1` only if it fails. Optional probe C via `E09_EXTRA_MODEL`. Exit codes 0 / 1 / 2 (missing key) / 3 (leak gate).
- `spikes/e09/verify.sh`: prints `a.*`, `a2.*`, `b.*`, `c.*` observation lines (`stream_json_parses`, `init_seen`, `api_key_source`, `init_model`, `tool_use_write`, `tool_result`, `result_seen`, `result_is_error`, `api_error_status`, `terminal_reason`, `usage_input_gt0`, `usage_output_gt0`, `model_usage_keys`, `total_cost_usd`, `result_text_head`, `exit_code`, `file_ok`, `key_absent`); the key is only tested with `grep -qF`.
- Offline self-test (`E09_OFFLINE=1`): runs the whole pipeline against `tests/fakes/fake_claude.py` with the planted key `e09-offline-planted-0123456789` in a temp dir, asserts ten observation lines, runs a negative test that the leak gate detects a planted key, prints `E09 offline OK`, and leaves `git status --porcelain` unchanged.
- `spikes/e09/NOTES.md`: `## Quyết định` = `fail`, with the note that E-09 was skipped by the user on 2026-10-27 (not run, not a measured failure).

## Task Commits

1. **Task 1 (tracer): probe scripts + offline self-test** - `77037f8` (test)
2. **Task 2: user runs the real probe** - skipped by the user (`skip-e09`), no commit
3. **Task 3: decision** - `894ff8b` (docs): NOTES.md with `fail`

SUMMARY commit follows (`docs(06-01): complete E-09 DeepSeek gate plan`).

## Verification

- `E09_OFFLINE=1 bash spikes/e09/run.sh` printed `E09 offline OK`; `git status --porcelain` unchanged (PASS).
- `bash -n` on both scripts (PASS).
- `env -u DEEPSEEK_API_KEY bash spikes/e09/run.sh` exited 2 with `Thiếu DEEPSEEK_API_KEY` (PASS).
- Task 1 acceptance greps: both scripts executable; exactly one non-comment `timeout 180 "$E09_CLAUDE_BIN"` line and it contains `</dev/null`; `export ANTHROPIC_AUTH_TOKEN` present; `scan-secrets.sh` and `invalid-key-for-e09-probe` referenced; no `echo`/`printf` of the key variable (PASS).
- Task 3 acceptance: `grep -c "## Quyết định" spikes/e09/NOTES.md` is 1 and the file names `fail`; `bash spikes/lib/scan-secrets.sh` exit 0 (`scan-secrets: OK`) after NOTES.md was written (PASS).
- Task 2 acceptance and plan-level "after Task 2" checks (fixtures exist, `a.key_absent=yes`): NOT RUN, the probe was skipped by the user.

## Decisions Made

- D-02 outcome: `fail` because E-09 was not run. Plan 06-11 cannot proceed until E-09 is run (`bash spikes/e09/run.sh` with `DEEPSEEK_API_KEY` exported in the user's own shell) or the user decides dsh vs drop. agy, teams, quota, Settings and the human engine are unaffected (D-02). ENG-01's phase gate (a `tiet-kiem` task with Dev on DeepSeek) cannot pass as written until then.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Scratch git identity email would trip the secret scan**
- **Found during:** Task 1
- **Issue:** The plan's `-c user.email=e09@example.com` would be flagged by `spikes/lib/scan-secrets.sh`, whose email allow-list only covers `@example.invalid`.
- **Fix:** `run.sh` uses `e09@example.invalid`.
- **Files modified:** `spikes/e09/run.sh`
- **Commit:** `77037f8`

**2. [Rule 2 - Missing critical functionality] Probe output is staged outside the repo before anything is written under `spikes/e09/raw`**
- **Found during:** Task 1
- **Issue:** Running the probes directly into `spikes/e09/raw` would leave unscrubbed output (possibly containing the key) inside the repo if the leak gate tripped, contradicting D-03 ("before anything is written under spikes/e09/raw").
- **Fix:** `run.sh` writes to a staging dir under `SPIKE_WORK_ROOT`, scrubs, runs the `grep -F` gate, copies to `spikes/e09/raw`, runs `scan-secrets.sh`, and deletes the copied files and exits 3 if that fails; fixtures are copied last and re-checked.
- **Files modified:** `spikes/e09/run.sh`
- **Commit:** `77037f8`

### Plan outcomes that differ from the plan

**3. Task 2 skipped by the user.** The user replied `skip-e09` to the Task 2 checkpoint (the plan's own resume signal for "E-09 not run"). Consequently no `spikes/e09/raw/*` files and no `tests/fixtures/stream/deepseek_e09_ok.jsonl` or `deepseek_e09_auth_error.jsonl` were produced, and no placeholder fixtures were created. Plan 06-11's parser test, which keys on `deepseek_e09_ok`, has no fixture to use until E-09 is run.

**4. Task 3 recorded as `fail` without a user choice among the three options.** Per the plan's resume-signal text ("skip-e09 ... Task 3 then records fail") and the coordinator's instruction, the decision is `fail`, documented in NOTES.md as not run rather than measured.

**5. `scan-secrets.sh` output wording.** The plan expects it to "print CLEAN"; the script prints `scan-secrets: OK` on a clean host scan (`CLEAN` only appears with `--container`). Exit status 0 was used as the pass condition.

**Total deviations:** 2 auto-fixed (1 bug, 1 missing critical), 3 plan-outcome differences. **Impact:** the tooling is complete and tested; the D-02 gate evidence does not exist, so plan 06-11 is blocked.

## Authentication Gates

Task 2 was the planned human-action gate (the user's own DeepSeek key). The user did not run it. The executor never held, read or asked for the key.

## Issues Encountered

- The worktree isolation guard refused compound shell commands that name git or run scripts through constructs it could not verify (for example `bash -c '... git status ...'` from the plan's verify line). The same checks were run as plain, separate commands with equivalent results.

## Known Stubs

None.

## Threat Flags

None. The only network-facing path is the user-run probe to the DeepSeek endpoint, already covered by T-06-01 through T-06-03 in the plan's threat model.

## Next Phase Readiness

- Blocked: plan 06-11 (DeepSeek engine) until E-09 is run for real or the user decides dsh vs drop. To run: `export DEEPSEEK_API_KEY=...` in your own shell, then `bash spikes/e09/run.sh` from the repo root (optionally with `E09_EXTRA_MODEL=<model id>`), paste the printed lines, then record `pass-alias` or `pass-explicit` in `spikes/e09/NOTES.md`.
- Ready: every other Phase 6 plan (agy, teams, quota, Settings, human engine) is independent of this outcome.
- Requirements ENG-01 and ENG-05 are not satisfied by this plan; `requirements-completed` is left empty on purpose.

## Self-Check: PASSED

- FOUND: `spikes/e09/run.sh`, `spikes/e09/verify.sh`, `spikes/e09/NOTES.md`
- FOUND commits: `77037f8`, `894ff8b`
- NOT PRODUCED (by design, user skipped Task 2): `spikes/e09/raw/*`, `tests/fixtures/stream/deepseek_e09_ok.jsonl`, `tests/fixtures/stream/deepseek_e09_auth_error.jsonl`
