# Plan branch-name-check-no-git: the binary must start without `git` on PATH (2026-11-10)

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, branch `main`.

## Bug (found by `scripts/build-release.sh`, step "stats --json in an empty environment")
`config.rs:82-90` validates `merge.default_base_branch` with `gitops::valid_branch_name` (`src/gitops.rs:595`), which spawns `git check-ref-format --branch`. The embedded default config (`assets/config.default.json`) contains `"merge": {"default_base_branch": "dev"}`, so EVERY start validates it. With no `git` on PATH (`env -i HOME=... PATH=/nonexistent agent-brain stats --json`) the spawn fails, the check returns false, and the binary refuses to start with `Cấu hình merge.default_base_branch phải là tên nhánh git hợp lệ`. That breaks DIST-01 (a static binary that runs with nothing else installed) and makes every start depend on an external program. The dashboard task poll (`dashboard/api_tasks.rs:1892-1893`) also spawns two git processes per poll through the same function.

## Fix
1. `src/gitops.rs`: add `pub fn valid_branch_name_syntax(name: &str) -> bool`, pure Rust, no process. Same rules as before plus git's `check-ref-format` rules for a branch short name: not empty; not `@`; no leading `-`; no whitespace or control characters (including DEL); none of `~ ^ : ? * [ \`; no `..`; no `@{`; no `//`; not starting or ending with `/`; not ending with `.`; no path component that starts with `.` or ends with `.lock`. Unicode letters are allowed. Keep `valid_branch_name` with its current behaviour (syntax check AND `git check-ref-format`), implemented as `valid_branch_name_syntax(name) && <git check>` so the git call only happens for names that pass the cheap syntax check.
2. `src/config.rs` (the `merge.default_base_branch` validation) and the dashboard poll path (`src/dashboard/api_tasks.rs` where `merge_state` is prepared, lines around 1892) call `valid_branch_name_syntax`. `src/workspace.rs` keeps `valid_branch_name` (git is available whenever a merge really runs).
3. Tests:
   - unit tests in `gitops.rs`: a table of valid names (`dev`, `main`, `task/sua-app`, `release/1.2`, `feature/đăng-nhập`) and invalid ones (`""`, `@`, `-x`, `a b`, `a..b`, `a~1`, `a^b`, `a:b`, `a?b`, `a*b`, `a[b`, `a\b`, `a//b`, `/a`, `a/`, `a.`, `a.lock`, `x/.hidden`, `x/y.lock/z`, `a@{1}`, `a\tb`); plus a test that agrees with `git check-ref-format --branch` on this table when git is present;
   - a config test: the default embedded config loads while `PATH` points at an empty directory (set `PATH` for the process under test, or call the validation directly) and `merge.default_base_branch` values `bad name` and `a..b` are still rejected;
   - a regression test `agent-brain/tests/dist_empty_env.rs`: run the built binary (`env!("CARGO_BIN_EXE_agent-brain")`) with `env_clear()`, `HOME=<tempdir>`, `PATH=/nonexistent`, args `stats --json`; assert exit success and that stdout contains `"total"` (this is the check `scripts/build-release.sh` runs).
4. Do not change `Cargo.toml`/`Cargo.lock`, the merge flow logic, or any file outside: `agent-brain/src/gitops.rs`, `agent-brain/src/config.rs`, `agent-brain/src/dashboard/api_tasks.rs`, `agent-brain/tests/dist_empty_env.rs`.

## Verify (run, paste the results in the summary)
```bash
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib gitops
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib config
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dist_empty_env
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test task_merge_push --test dashboard_actions
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
bash scripts/build-release.sh
```
The last command must end without any `Lỗi:` line (it builds the static binary and runs the empty-environment checks). Expected: all green; the suite was 1902 passed, 0 failed before this fix.

## Commit
`fix(agent-brain): branch name syntax check without spawning git, so the binary starts without git on PATH`. Stage files by name only; no `git add -A`, no `--no-verify`. End with the `Co-Authored-By` line required by the environment.
