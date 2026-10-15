---
phase: 02-protocol-mvp-scripts
plan: 07
subsystem: integration-and-acceptance
tags: [integration-tests, traceability, live-acceptance]

requires:
  - phase: 02-protocol-mvp-scripts
    provides: "plans 02-01..02-06 and their review fixes"
provides:
  - "tests/test_phase2_integration.py: P1 traceability, handler wiring, README coverage, fixture shape, pipeline → task log"
  - "A-01..A-10 assumption reconciliation against spikes/spike-results.md"
affects: [03-rust-cli-core]

actuals:
  tasks: 1
  commits: 1

key-files:
  created:
    - tests/test_phase2_integration.py
  modified: []

key-decisions:
  - "Task 1 executed by Antigravity (agy, gemini-3.8-flash-high); reviewed by Claude. Tasks 2 and 3 are the user's live checkpoints."

requirements-completed: []

completed: null
status: checkpoint
---

# Phase 2 Plan 07: Integration Preflight and Live Acceptance Summary

**Status: Task 1 done. Waiting at the Task 2 checkpoint, the user's live run of P1-01..P1-10 with real Claude accounts. Task 3 (five real tasks) follows.**

## Task 1: Integration preflight (done)

- Commit: `e5a2605` test(02-07): add phase 2 integration preflight tests
- `tests/test_phase2_integration.py`, five tests, all passing: every P1-01..P1-10 has a `test_p1_<NN>_*` test; every `cli.HANDLERS` entry resolves; the README documents all five commands and exit codes 0, 1, 2, 3, 4, 130; every contract fixture has `version` 1 and unique ids; a sandbox lead → dev → qa run fills the task log (Dev turns before first edit 2, no reads outside the plan, QA row Đạt, the two user lines empty; `check_logs` 1 → 0 after filling them).
- No integration defect was found; no production file changed.
- Reviewer re-run: full suite 108 tests OK; all ten `test_p1_<NN>_` ids present.
- Reviewer docs fix found through this task: `5ed08d5` the README's A-10 now matches the plans (tool names); the E-06 token-rotation risk has its own row.

## CLI assumption reconciliation (A-01..A-10)

From `must_haves.flagged_assumptions` in plans 02-01, 02-05 and 02-06, checked strictly against `spikes/spike-results.md` (confirmed only when a spike exercised exactly that behaviour). No assumption is refuted.

| ID | Assumption | Status | Evidence |
|----|------------|--------|----------|
| A-01 | `claude -p --output-format stream-json` requires `--verbose` | confirmed | S1: without `--verbose` → `Error: When using --print, --output-format=stream-json requires --verbose`, exit 1 |
| A-02 | stream-json lines: `system/init` with session_id, `assistant` with tool_use, `user` with tool_result, final `result` with usage and permission_denials | confirmed | S1 observations and `spikes/s1/raw/s1.jsonl` (also parsed by `test_real_s1_fixture`) |
| A-03 | `--allowedTools`/`--disallowedTools` take several values; `Tool(./path)` and `Bash(cmd *)` syntax; unallowed tools are denied in `-p` and listed in permission_denials | not covered | S1/S4 used a single `Bash(...)` value; `Tool(./path)` rules and a denial were never exercised |
| A-04 | Read/Grep/Glob need no permission, so only the guard hook limits reads | not covered | no spike tested read permissions |
| A-05 | `--settings <file>` loads a PreToolUse hook in `-p`; exit 2 blocks and returns stderr | not covered | only a SessionStart hook was observed in S1 |
| A-06 | Explicit `CLAUDE_CONFIG_DIR=~/.claude` misbehaves, so work unsets it; personal is `~/.claude-orca`, logged in | not covered | S5: E-07 "không thử"; personal profile "chưa kiểm chứng: chạy claude thật với profile cá nhân" (work with the variable unset did run correctly) |
| A-07 | `--model opus/sonnet` aliases, `--append-system-prompt`, `--strict-mcp-config` | not covered | S1 passed none of these flags |
| A-08 | `--max-turns` exists and limits turns | confirmed | S8 variant A: `error_max_turns`, "Reached maximum number of turns (1)" |
| A-09 | SIGTERM to the claude PID ends its Bash-tool children | confirmed | S4 r1: SIGTERM → no leftover `sleep 317` after 5 s |
| A-10 | Tool names `Task`, `MultiEdit`, `NotebookEdit`; disallowing an unknown name is harmless | not covered | S1 init lists the tool names; disallowing an unknown name was not tried |

Separate risk, not an A-xx: **E-06**, two claude processes on `~/.claude-orca` (this orchestrating session and a personal-profile `brain` run) may rotate the OAuth token. Not verified; the manual says to run personal-profile tasks only when no other session uses `~/.claude-orca`.

Five assumptions (A-03..A-07) and A-10 are settled only by the live run. The ones that matter most for enforcement:

- **A-05**: the Dev guard hook must actually run. The manual's required `guard.jsonl` check after the first `brain dev` (P1-07) settles it; an empty log stops the live run.
- **A-03**: Dev's `Read/Edit/Write(./<file>)` rules and the denial behaviour. `permission_denials` in the task log and the post-run git check show it.
- **A-06**: P1-10 checks the work transcript lands under `~/.claude/projects/`.

## Task 2: Live acceptance P1-01..P1-10 (checkpoint, user)

Not started. Precondition met: `spikes/spike-results.md` exists and is approved; the personal profile is `~/.claude-orca` (SPK-10). The user follows `tests/protocol-manual.md` and fills `protocol-log/nghiem-thu-p1.md`.

## Task 3: Five real tasks (checkpoint, user)

Not started. Gate: `python3 -m brain.tasklog check protocol-log --min 5` exits 0.

## Deviations from Plan

- Task 1's assumption table was returned by agy in its final response and written here by the reviewer (agy is not allowed to write under `.planning/`).
