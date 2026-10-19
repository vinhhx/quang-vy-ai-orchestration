---
status: testing
phase: 05-embedded-dashboard
source: [05-VERIFICATION.md]
started: 2026-10-19T05:40:00Z
updated: 2026-10-19T05:40:00Z
---

## Current Test

number: 1
name: "05-01: Alpine CSP was vendored only after you confirmed the package"
expected: |
  You confirmed @alpinejs/csp on npmjs.com and GitHub before it was vendored (05-01 checkpoint reply "approved"); the vendored file's sha256 matches VENDOR.md.
awaiting: user response

## Tests

Each test confirms one judgment-tier prohibition from 05-VERIFICATION.md `flagged_prohibitions` (ADR-550 D4: the LLM-judge verdicts are non-authoritative and need a human sign-off).

### 1. 05-01: Alpine CSP was vendored only after you confirmed the package
expected: You confirmed @alpinejs/csp on npmjs.com and GitHub before it was vendored (05-01 checkpoint reply "approved"); the vendored file's sha256 matches VENDOR.md.
result: [pending]

### 2. 05-08/05-09: no control chooses or overrides a new task's profile
expected: The header profile switcher only filters the views. Giao việc has no profile field (POST /api/tasks takes project, request and name; d15_profile_field_rejected passes), and in the live check new tasks used their project's profile (P4-08).
result: [pending]

### 3. 05-10: no control edits .plan.md or .qa-report.md, or creates a commit
expected: The Task screen shows the plan and the QA report read-only. Its only actions are Kill, Chạy Dev, Chạy QA and Chạy lại Lead.
result: [pending]

### 4. 05-11: no control edits a project's profile, id, path or type
expected: Dự án lets you edit aliases only (PUT body { aliases, etag }; d15_profile_not_editable passes).
result: [pending]

### 5. 05-13: no personal-profile run during the live checks while another Claude session used ~/.claude-orca
expected: Every live-check run (13–17) used profile work, and no personal run is recorded in brain.db. You confirm that no personal run was started by other means during the checks.
result: [pending]

### 6. 05-13: live processes were never stopped with pkill or killall, or by process name
expected: Signals came only from the UI Kill (runner::cmd_kill) and from recorded-PID SIGINT/SIGTERM to the serve and Chrome processes Claude started. The scripts contain no pkill or killall.
result: [pending]

## Summary

total: 6
passed: 0
issues: 0
pending: 6
skipped: 0
blocked: 0

## Gaps
