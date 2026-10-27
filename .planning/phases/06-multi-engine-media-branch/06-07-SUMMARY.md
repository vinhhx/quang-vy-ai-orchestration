---
phase: 06-multi-engine-media-branch
plan: 07
subsystem: engine-quota
status: complete
tags: [rust, quota, rate-limit, autopilot, dev-continuation, fake-claude]

requires:
  - phase: 06-multi-engine-media-branch
    provides: "06-05 engine seam (engine::resolve_for_run, plan_for, EnginePlan.stream/display, execute_role_staged ordering)"
provides:
  - "error::EXIT_QUOTA (5)"
  - "quota::{OUTCOME_QUOTA, QuotaHit, text_matches, detect_claude, detect_agy, stderr_tail, stop_detail, user_message, step_name}"
  - "stream::Summary rate_limit_rejected, rate_limit_reset_at_ms, rate_limit_type, using_overage, api_error_status, terminal_reason, last_retry_error, last_retry_status, result_seen()"
  - "execute_role_staged turns a quota or rate-limit failure of a claude-format run into outcome quota, metrics_json.quota {engine, signal, reset_at}, runs.error `quota: <detail>` and exit 5"
  - "autopilot::decide StopReason::HetQuota (het_quota, word `hết quota`), RunFact.engine, quota-aware build_facts / effective_rounds, D2 quota rule before the failed-run rule"
  - "Dev continuation mode (ENG-03): a re-run after a quota stop continues on the partial diff"
affects: [06-09, 06-10, 06-11, 06-13, 06-14]

actuals:
  tokens: 17300
  tasks: 3
  commits: 6
plan_head_before: eeb252e2e0ec59167598e1a1d88a3d729f821323
plan_head_after: d42312fbc54068c32e5445540c1cd659da31ef8e

tech-stack:
  added: []
  patterns:
    - "One pure detector per stream family at the run seam; a successful final result always wins over earlier rate-limit events"
    - "A quota run keeps the round: dropped from the autopilot facts unless it is the last run and the driver is not resuming"
    - "Continuation is derived from the latest Dev run outcome, no new flag; the old session is never resumed"

key-files:
  created:
    - agent-brain/src/quota.rs
    - agent-brain/tests/engine_quota.rs
  modified:
    - agent-brain/src/stream.rs
    - agent-brain/src/error.rs
    - agent-brain/src/lib.rs
    - agent-brain/src/roles/mod.rs
    - agent-brain/src/roles/dev.rs
    - agent-brain/src/roles/lead.rs
    - agent-brain/src/autopilot/decide.rs
    - agent-brain/src/autopilot/view.rs
    - agent-brain/src/autopilot/mod.rs

key-decisions:
  - "Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6)."
  - "The quota stop returns BrainError(user_message, EXIT_QUOTA) and relies on main to print it once on stderr, instead of an extra emit that would print the line twice."
  - "`.handover.md` is context, not a deliverable: during a continuation it is ignored by the post-run scope check and left out of the automatic commit."
  - "detect_agy is written and unit-tested here but not wired; plan 06-09 calls it from the agy run seam."

requirements-completed: [ENG-02, ENG-03]

coverage:
  - id: D1
    description: "A quota or rate-limit failure of a claude-format run stops the step: outcome quota, runs.error, metrics_json.quota {engine, signal, reset_at ms}, the Vietnamese message and exit 5; no retry and no second run row"
    requirement: ENG-02
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_quota.rs#eng02_dev_quota_stops"
        status: pass
      - kind: unit
        ref: "agent-brain/src/quota.rs#rejected_without_overage_stops_with_reset_time"
        status: pass
    human_judgment: false
  - id: D2
    description: "Detector boundaries: a successful final result is never quota (recovered 429, rejected event with success), allowed_warning and overage-covered rejections do not stop, 429/402/blocking_limit/limit text/api_retry/stderr do, 529 Overloaded does not; resetsAt seconds become integer milliseconds (fraction truncated)"
    requirement: ENG-02
    verification:
      - kind: unit
        ref: "agent-brain/src/quota.rs#success_result_after_rejected_event_is_not_quota"
        status: pass
      - kind: unit
        ref: "agent-brain/src/quota.rs#overloaded_529_is_not_quota"
        status: pass
      - kind: unit
        ref: "agent-brain/src/stream.rs#rate_limit_fields"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_quota.rs#eng02_retry_then_success_not_quota"
        status: pass
    human_judgment: false
  - id: D3
    description: "Autopilot stops with het_quota (own reason, detail line, checked before the failed-run rule); quota runs never consume a round; a re-run of the stopped step keeps its round for the driver and for a manual Lead run"
    requirement: ENG-02
    verification:
      - kind: unit
        ref: "agent-brain/src/autopilot/decide.rs#d2_quota_stops_het_quota"
        status: pass
      - kind: unit
        ref: "agent-brain/src/autopilot/decide.rs#quota_run_dropped_on_resume"
        status: pass
      - kind: unit
        ref: "agent-brain/src/autopilot/decide.rs#effective_rounds_skip_quota"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_quota.rs#eng02_autopilot_stops_het_quota"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_quota.rs#quota_run_does_not_consume_round"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_quota.rs#manual_lead_round_skips_quota"
        status: pass
    human_judgment: false
  - id: D4
    description: "Dev continuation after a quota stop: dirty plan files, protocol files and .handover.md are accepted, anything else is still refused; the input, allowed tools, guard read access and metrics_json.continuation carry the continuation; no session is resumed"
    requirement: ENG-03
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_quota.rs#eng03_rerun_continues_partial_diff"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_quota.rs#eng03_dirty_outside_plan_refused"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_quota.rs#eng03_handover_included"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_quota.rs#no_quota_dirty_still_refused"
        status: pass
    human_judgment: false
  - id: D5
    description: "Quota detection matches the shapes a REAL Claude Code or DeepSeek usage-limit stop produces, and agy quota text is recognised"
    requirement: ENG-02
    verification:
      - kind: unit
        ref: "agent-brain/src/quota.rs#detect_agy_rules"
        status: pass
    human_judgment: true
    rationale: "No real Claude Code usage-limit result, DeepSeek 402/429 rendering or agy quota stop was ever captured (research A1-A3, A5; 06-01 E-09 is `fail`, 06-02 is open). Every rule is tested only against Rust-side fakes built from documented field names. The raw lines stay in `events`, so the first real stop must be compared with the rules (fixtures from 06-01 are added in 06-11)."

duration: 70min
completed: 2026-10-27
---

# Phase 6 Plan 07: Quota stop, het_quota and Dev continuation Summary

**A quota or rate-limit failure of a claude-format run is now detected at the run seam (multi-signal, a successful final result always wins), recorded as outcome `quota` with `metrics_json.quota {engine, signal, reset_at}` and exit code 5; autopilot stops with `het_quota` and never switches engine; quota runs keep their round; and a Dev re-run continues the partial diff on the same worktree.**

Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6).

## What was built

- **Task 1 (tracer, `d6fcf66`)**: `error::EXIT_QUOTA = 5`. `stream::Summary` records `rate_limit_event` (rejected flag, `rateLimitType`, `resetsAt` seconds -> integer ms, last `isUsingOverage`), `system/api_retry` (`error`, `error_status`) and the `result` error fields (`api_error_status`, `terminal_reason`), plus `result_seen()`. New `quota.rs`: `detect_claude` applies, in order, success-result-wins, rejected event without overage, error result with HTTP 429/402, `blocking_limit`, limit/balance text in an error result, last `api_retry` of `rate_limit`/`billing_error` without a result, and a limit phrase in the stderr tail with a non-zero exit and no result; 529 `Overloaded` matches nothing. `execute_role_staged` runs the detector after `summary.finish()` for claude-format runs that were not killed or timed out, stores `quota: <detail>` in `runs.error`, sets outcome `quota` and merges `metrics_json.quota`, then returns `BrainError(user_message, EXIT_QUOTA)` so the role code after the call (commit, scope check) never runs. Tracer gate: the three `<verify>` commands re-run end-to-end before expanding.
- **Task 2 (`b3322fc`)**: `StopReason::HetQuota` (`het_quota`), `RunFact.engine` and `RunFact::is_quota()`; the D2 quota branch sits right after the `running` branch, so it precedes the failed-run rule. `build_facts` drops quota runs except a last one when not resuming; `effective_rounds` skips earlier quota runs in the plan, implement, fix-code-review and qa counts. View word `hết quota` and the stored detail line; `child_error_stop` ignores `EXIT_QUOTA`. `manual_round` needed only the new `engine` field on its synthetic fact.
- **Task 3 (`d42312f`)**: `cmd_dev` computes the continuation read-only (current branch -> task row -> latest Dev run outcome `quota`). When true, dirty paths in the plan's file set (NFC-normalised), `PROTOCOL_FILES` and `.handover.md` are accepted and any other dirty path is refused with the existing message listing only those paths. Both implement and fix runs append `Tiếp tục việc dở của lượt trước (dừng vì hết quota): …`, add `Bash(git diff *)` and `Bash(git status *)`, and, when `.handover.md` exists, `Read(./.handover.md)` plus the absolute rule and guard read access (never write access); stage metrics carry `continuation: true` (kept next to `fix_for` / `named_files`). `quota::detect_agy` (pure, unit-tested) is ready for plan 06-09.

## Verification

- `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1`: exit 0, 54 test binaries ok, no failures (436 lib tests, engine_quota 9/9).
- Task verify commands: `--test engine_quota` (9 passed), `--lib -- quota:: stream:: error::` (32 passed at Task 1), `contract_stream` + `cli_stream` + `cli_dev` (19), `--lib autopilot` (47), `engine_quota autopilot_driver cli_autopilot lead_rounds autopilot_kill` all ok.
- `cargo fmt --manifest-path agent-brain/Cargo.toml --check`: clean. `git diff --exit-code 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock`: no diff (no crate added; `regex` already a dependency).
- Acceptance greps: `EXIT_QUOTA` in error.rs (1 line) and roles/mod.rs; `quota::detect_claude` 1 line in roles/mod.rs; `pub mod quota;` in lib.rs; `git diff 574b6cd -- tests/contract/stream.json` empty; `HetQuota` 5 lines and `het_quota` 3 lines in decide.rs; the D2 awk check (quota branch before `last_run.status == "failed"`) exits 0; `"hết quota"` once in view.rs; the continuation sentence once and `Bash(git diff *)` once in dev.rs; `pub fn detect_agy` once in quota.rs.

## TDD note

Each task started with a `test(06-07)` commit of its CLI tests, run RED against the code of the previous task before any production edit: Task 1 `eng02_dev_quota_stops` failed on exit code (1, not 5); Task 2 `eng02_autopilot_stops_het_quota` failed (stop reason was not `het_quota`) and `manual_lead_round_skips_quota` failed (round 2, not 1); Task 3 `eng03_rerun_continues_partial_diff` and `eng03_handover_included` failed (re-run refused, exit 1). For Task 3 the production change had already been written, so its diff was saved, reverted for the RED run and re-applied. Unit tests live next to the code they test, so their RED was a compile failure and was not committed separately. `eng02_retry_then_success_not_quota`, `quota_run_does_not_consume_round` and `no_quota_dirty_still_refused` passed before the change by design (guards for unchanged behaviour). The plan prescribes one `feat` commit per task; the three RED commits are extra, hence 6 commits instead of 3.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The quota message was to be emitted and also returned, which prints it twice**
- **Found during:** Task 1
- **Issue:** The plan says to emit the user message and return `Err(BrainError::new(message, EXIT_QUOTA))`. `main` already prints `e.message` on stderr for every error, so both would show the same line twice.
- **Fix:** The seam returns the error only; the message reaches stderr once, which is what `eng02_dev_quota_stops` asserts (`Hết quota engine claude`, `agent-brain không tự đổi engine`).
- **Files modified:** agent-brain/src/roles/mod.rs
- **Commit:** d6fcf66

**2. [Rule 2 - Missing critical functionality] `.handover.md` would have broken the continuation**
- **Found during:** Task 3
- **Issue:** The plan accepts a dirty `.handover.md` but not what happens after the run. The post-run scope check would report it as a protocol violation (ROLE-06) and the automatic commit would add it to the task branch.
- **Fix:** When the file exists during a continuation, it is removed from the changed-path set of the scope check and from the paths of the automatic commit. `eng03_handover_included` asserts no violation and that the file is never tracked; it also checks the guard policy gives read but not write access.
- **Files modified:** agent-brain/src/roles/dev.rs
- **Commit:** d42312f

**3. [Rule 1 - Bug, in the test] `eng03_dirty_outside_plan_refused` could not create the out-of-plan file**
- **Found during:** Task 3 (after its RED commit)
- **Issue:** The fake claude runs the guard hook, which blocks a write outside the plan, so `ngoai-plan.txt` was never created and the second run was not refused.
- **Fix:** The first run of that test uses the fake's `ignore_hooks` so the tree really is dirty outside the plan.
- **Files modified:** agent-brain/tests/engine_quota.rs
- **Commit:** d42312f

**Total deviations:** 3 auto-fixed (1 spec correction, 1 missing correctness, 1 test bug). **Impact:** none on the planned behaviour; deviation 2 widens the continuation to cover the handover file's post-run handling.

Minor plan inaccuracies, no action needed: `RunFact` had two struct literals to update (the test in decide.rs and `manual_round` in lead.rs), not the four-plus-one the plan counted.

## Auth gates

None.

## Known Stubs

None that feed UI data. `quota::detect_agy` is complete and tested but not called yet: plan 06-09 wires the agy run seam to it (planned). The dashboard and Telegram do not map `quota` / `het_quota` yet; plans 06-10, 06-13 and 06-14 own that.

## Threat Flags

None beyond the plan's threat model. T-06-24: the detector only stops (exit 5), nothing retries or switches engine, tests assert one process and one run row, and autopilot stops with `het_quota`. T-06-25: the continuation accepts only plan files, protocol files and `.handover.md`; the post-run scope check still unions committed and uncommitted changes. T-06-26: raw lines stay in `events`; `metrics_json.quota` and `runs.error` keep the evidence. T-06-27: a successful final result always wins and 529 is excluded, both with negative unit tests.

## Issues Encountered

- No real Claude Code, DeepSeek or agy quota capture exists (06-01 E-09 is `fail`, 06-02 is open); detection is built only on documented field names and Rust-side fakes, as the plan states. See coverage D5.
- `cargo fmt` reformatted `engine_quota.rs` and `quota.rs` after edits; it touched no file outside this plan's scope (the crate was fmt-clean before).

## Self-Check: PASSED

- FOUND: agent-brain/src/quota.rs, agent-brain/tests/engine_quota.rs, agent-brain/src/stream.rs, agent-brain/src/autopilot/decide.rs, agent-brain/src/roles/dev.rs
- FOUND commits (ancestors of HEAD; `git rev-list --count eeb252e..HEAD` = 6): a2f7d41, d6fcf66, 8767733, b3322fc, 775cc92, d42312f
- Acceptance criteria of all three tasks re-run and passing; plan-level `<verification>` re-run and passing.
