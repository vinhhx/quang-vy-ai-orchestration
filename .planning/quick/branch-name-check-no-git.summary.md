# Summary: branch-name check without git (2026-11-10)

Worktree: /mnt/data/IDE/QuangVy-Orchestration/.claude/worktrees/agent-aa7da2669a769c8f6
Branch: worktree-agent-aa7da2669a769c8f6 (base 439b00c)

## Files changed
- agent-brain/src/gitops.rs: new `valid_branch_name_syntax` (pure Rust); `valid_branch_name` = syntax check AND `git check-ref-format --branch`; tests `branch_name_syntax_table`, `branch_name_syntax_agrees_with_git`.
- agent-brain/src/config.rs: `merge.default_base_branch` uses `valid_branch_name_syntax`; test `merge_base_branch_validation_needs_no_git`.
- agent-brain/src/dashboard/api_tasks.rs: the merge-state poll uses `valid_branch_name_syntax` for base and task branch.
- agent-brain/tests/dist_empty_env.rs: new regression test (binary, `env_clear`, `HOME=<tempdir>`, `PATH=/nonexistent`, `stats --json`).
- workspace.rs untouched (keeps `valid_branch_name`).

## Commits
- fa8e9fa fix(agent-brain): branch name syntax check without spawning git, so the binary starts without git on PATH

## Verify
- `--lib gitops`: 19 passed, 0 failed
- `--lib config`: 53 passed, 0 failed
- `--test dist_empty_env`: 1 passed, 0 failed
- `--test task_merge_push --test dashboard_actions`: 16 passed + 13 passed, 0 failed
- Full suite, run 1: passed 1904, failed 2 (see open points). Run 2 (log saved): passed 1906, failed 0.
- `bash scripts/build-release.sh`: exit 0, no `Lỗi:` line. Last lines:
  - `P2-09 OK (route A): agent-brain/target/x86_64-unknown-linux-musl/release/agent-brain · 17634560 byte · sha256 8dab4a04fb98dcd57c957a79bf3a454870647dd5481f481650125efe32728c9f`
  - `P4-01 OK: dashboard chạy trong môi trường rỗng (env -i, PATH=/nonexistent) — / và /app.css qua token, 401 khi thiếu token`
  - `agent-brain chạy được khi không có gì cài thêm (env -i, PATH=/nonexistent)`

## Deviations
- Config test does not change `PATH` for the process (process-global, would race with parallel tests that spawn git). It calls the validation directly (default config loads, `bad name` and `a..b` rejected, `release/1.2` accepted). The empty-PATH behaviour is covered end to end by `tests/dist_empty_env.rs`.

## Open points
- The first full-suite run reported 2 failures; I did not capture which tests (output was piped through the awk summary). A second full run was 1906 passed, 0 failed, so they look flaky under load, unrelated to this change (the first run overlapped with nothing else heavy, but I could not confirm). Worth a re-run by the reviewer.
- I did not prove the new regression test fails on the pre-fix code (no stash allowed); the bug was reproduced by `build-release.sh` per the plan.
