# Plan: read-only git calls must not take `.git/index.lock` (GIT_OPTIONAL_LOCKS=0)

## Problem
`git status` (and other read commands) opportunistically take `.git/index.lock` to refresh the
index stat cache. agent-brain runs such reads in the background while an agent run writes to the
same repo:

- serve's Telegram watcher `telegram/detect.rs:200` and outbox `telegram/outbox.rs:281`, and every
  dashboard task view, call `dashboard::api_tasks::load_task_state` → `gitops::changed_paths`
  → `git status --porcelain=v1 -z --untracked-files=all` (`gitops.rs:54`).
- At the same time the CLI run (Dev/Lead via claude, or agent-brain's own plan commit) runs
  `git add` / `git commit`, which fails with
  `fatal: Unable to create '.../.git/index.lock': File exists … remove the file manually to continue.`

Observed as the flaky test `telegram_actions::more_round_from_telegram` (fails at the
`plan_cap_stopped` setup, `tests/telegram_actions.rs:~219`, under parallel `cargo test`), likely
also `press_without_text_keeps_tracked_text`. In real use the same race can break a real Dev
commit while the dashboard or the Telegram bot looks at the task.

## Fix
Git documents `GIT_OPTIONAL_LOCKS=0` for exactly this: background readers skip optional locks;
required locks of write commands are unaffected.

## Changes (only these files)
| # | File | Where | Change |
|---|---|---|---|
| 1 | agent-brain/src/gitops.rs | `pub fn git` (line ~5), `ancestry` (~118), `plan_commit_on_branch` (~156), `is_tracked` (~234) — every production `std::process::Command::new("git")` in the file (not the `#[cfg(test)]` module) | add `cmd.env("GIT_OPTIONAL_LOCKS", "0");` next to the existing `cmd.env("GIT_TERMINAL_PROMPT", "0");`. Prefer one small private helper (e.g. `fn git_cmd(root) -> Command` setting `current_dir` and both env vars) used by all four, matching the file's style; keep behaviour otherwise identical. |
| 2 | agent-brain/src/gitops.rs | `#[cfg(test)] mod tests` | add a deterministic unit test that `git()` passes the env to git: in a temp repo, `git(root, &["-c", "alias.optlocks=!printenv GIT_OPTIONAL_LOCKS", "optlocks"])` returns `"0"` (trimmed). Also a regression test: create `.git/index.lock` in a repo with a modified tracked file, call `changed_paths` → Ok with the file, and the lock file still exists afterwards (status did not touch it). |

Do NOT touch: tests other than the new unit tests, `tests/fakes/*`, Cargo.toml/Cargo.lock, any
other src file, `.planning/STATE.md`, `ROADMAP.md`, `state.json`, untracked `.planning/` files,
`~/.claude-orca`.

Sanity check before editing: `grep -rn 'Command::new("git")' agent-brain/src` — production hits
must be only in gitops.rs (plus `#[cfg(test)]` code in report.rs / telegram/intake.rs). If any other
production call exists, stop and report instead of widening scope.

## Verify
```bash
cd agent-brain
cargo fmt --check
cargo test --offline --locked --lib gitops
cargo test --offline --locked --test telegram_actions
# stress the flaky one: 20 runs of the two suspects in parallel with the full suite load
for i in $(seq 1 10); do cargo test --offline --locked --test telegram_actions -- more_round_from_telegram press_without_text_keeps_tracked_text || exit 1; done
cargo test --offline --locked              # full suite, parallel
cargo test --offline --locked              # second full run
git diff --stat HEAD~2 -- . ':!src/gitops.rs'   # must be empty apart from the summary
```
Acceptance: all pass; `grep -c 'GIT_OPTIONAL_LOCKS' agent-brain/src/gitops.rs` ≥ 2 (helper +
test); every production git `Command` in gitops.rs goes through the env-setting path.

## Commits
1. `test(gitops): failing test for GIT_OPTIONAL_LOCKS on read-only git calls`
2. `fix(gitops): read-only git calls skip optional index.lock (GIT_OPTIONAL_LOCKS=0)`
Stage files by name; no `git add -A`, no `--no-verify`.

## Summary to report
Files changed, commits, verify results with pass/fail counts, any deviation.
