---
status: testing
phase: 06-multi-engine-media-branch
source: [06-VERIFICATION.md]
started: 2026-11-10T10:27:39Z
updated: 2026-11-10T10:27:39Z
---

## Current Test

number: 1
name: ENG-01 phase gate: run one real task with team tiet-kiem (Lead and QA on claude, Dev on an anthropic endpoint: the company gateway or DeepSeek's official endpoint)
expected: |
  Each run header shows the right `engine ...`; runs.engine and Thống kê split tokens and cost per engine; the gateway/DeepSeek Dev run edits files under tool use; the header carries `chưa kiểm E-09` until evidence exists (LIVE-CHECKS section 8, P6-02)
awaiting: user response

## Tests

### 1. ENG-01 phase gate: run one real task with team tiet-kiem (Lead and QA on claude, Dev on an anthropic endpoint: the company gateway or DeepSeek's official endpoint)
expected: Each run header shows the right `engine ...`; runs.engine and Thống kê split tokens and cost per engine; the gateway/DeepSeek Dev run edits files under tool use; the header carries `chưa kiểm E-09` until evidence exists (LIVE-CHECKS section 8, P6-02)
result: [pending]

### 2. ENG-11 / D-34: run spikes/e09/run.sh per endpoint and per model (gateway first), then add `verified` to the engine entry
expected: a.stream_json_parses, a.tool_use_write, a.result_seen, a.usage_input_gt0, a.usage_output_gt0, a.file_ok, a.key_absent all yes, and b.result_is_error=yes b.api_error_status=401 (06-20 Task 3/4 still open)
result: [pending]

### 3. Quota stop on a real claude run and on a real endpoint (P6-03), then re-run on another team from the dashboard; include a `tiet-kiem` re-run whose Dev endpoint is chưa kiểm
expected: Banner `Hết quota engine X` with `Chạy lại bước này bằng đội hình...`; the `tiet-kiem` option reads `Bước Code sẽ chạy bằng: deepseek — chưa kiểm` and is selectable; nothing runs by itself; the new engine continues from .plan.md and the existing diff. The detector was calibrated on assumed shapes (quota.rs header: real Claude/DeepSeek/agy quota results were never captured; research A1-A3, A5)
result: [pending]

### 4. agy as Dev on a small real task (P6-04)
expected: Only plan files change, or the run reports `agy không sửa file nào...`; permission mode accept-edits as decided at the 06-02 probe
result: [pending]

### 5. Human Dev through a second clone (P6-05): human run, push, Xong, Trả lại
expected: `Xong` brings the second clone's commits in by fast-forward; scope check as for Dev; rejected result returns to the same person. Retry-after-failure is now covered by tests with a SQLite trigger; confirm once on a real remote that pressing Xong again after an interrupted attempt prints `đã được gộp ở lần bấm Xong trước` and completes
result: [pending]

### 6. Key sweep over every key_env on real runs (P6-06)
expected: Only `yes` lines; no key in stream.jsonl, events.raw_json, stderr.txt, runs.error, committed files
result: [pending]

### 7. Settings, Giao việc, task and Thống kê in light and dark themes at 1024 / 768 / 480 px (P6-07, SET-05)
expected: Engine rows match config order with honest readiness; no clipped text; contrast acceptable; the re-run dialog and Telegram-independent quota banner show the `chưa kiểm` mark legibly. 06-UI-REVIEW.md scored 15/24 (banned off-scale classes) and dashboard-ui-lint.sh does not enforce the banned list
result: [pending]

### 8. dsh as a black box on a scratch personal repo with Kill while running (P6-09)
expected: No `ab-dsh-` process left in the container, changed files root-owned as documented, git still works. The kill-wrapper probe is recorded ok (spikes/s6/raw/s6-kill-wrapper.txt, A18); --uid-probe (A19) is not recorded
result: [pending]

### 9. claude-max refused on a work project with the SPK-10 text and running on a personal project (P6-10)
expected: Refusal text `chỉ dùng cho dự án profile personal`; personal run uses ~/.claude-max (D-45 relaxes this for claude_config_dir entries, pin_profile entries still apply)
result: [pending]

### 10. Telegram quota buttons with the second confirm tap (P6-08, optional)
expected: Pick a team, second tap confirms, then the step re-runs through the shared action; a chưa kiểm team's button reads `Đội hình tiet-kiem (deepseek — chưa kiểm)` and the confirm text names `deepseek — chưa kiểm`
result: [pending]

### 11. Keyboard Tab order on the task screen (scripts/dashboard-keys.mjs step gate-buttons)
expected: From [data-flow-region] one Tab reaches #ab-act-approve. Red with and without the Phase 6 change; a Phase 5 keyboard check, not a Phase 6 requirement. Escalated in 06-VALIDATION.md; stale expectation vs real regression undecided
result: [pending]

## Summary

total: 11
passed: 0
issues: 0
pending: 11
skipped: 0
blocked: 0

## Gaps
