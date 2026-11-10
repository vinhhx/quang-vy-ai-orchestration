# Summary: task-merge-push (Merge & push after QA)

Plan: `.planning/quick/task-merge-push.plan.md`. Executed in worktree
`/mnt/data/IDE/QuangVy-Orchestration/.claude/worktrees/agent-a88e98f1484e08572`, branch
`worktree-agent-a88e98f1484e08572`, base commit `a03d39d`.

## Result

- `agent-brain task merge <id> --push` does the fetch, base checkout (created from origin when it is missing, fast-forwarded when behind), `merge --no-ff` (protocol files left out), push without force, and `git branch -d`. It reads the A/B/C state from git, so it is safe to run again. `task merge <id>` without the flag works as before.
- Dashboard: a `merge_push` control (`Merge & push`, or `Push lại` in state B), `POST /api/tasks/{id}/merge-push`, and a confirmation dialog on the task screen.
- Telegram: the `QA đạt` message has a `Merge & push` button. The first press only asks. `Xác nhận` runs the shared control once and sends the result as one message. `Hủy` changes nothing.
- Never `--force`, `+refspec`, `reset --hard`, `clean`, `branch -D` or a shell. Errors shown to the user have the remote URL, the repo path and URL credentials scrubbed.

## Files changed (`git diff --name-only a03d39d..HEAD`)

- `agent-brain/src/gitops.rs`: new functions only: `git_net`, `remote_url`, `rev_parse`, `ahead_behind`, `log_subjects`, `valid_branch_name`, plus unit tests.
- `agent-brain/src/config.rs`, `agent-brain/assets/config.default.json`: the `merge` block (`MergeConfig`) and its tests.
- `agent-brain/src/workspace.rs`: `MergeState`, `merge_state`, `merge_push_base`, `MergePushOutcome`, `merge_push_task`, `merge_push_message`, `merge_busy_cli`, `cmd_merge_push`. The conflict handling of `merge_task` moved into the shared helpers `merge_conflicts` and `resolve_protocol_conflicts`; the messages are unchanged.
- `agent-brain/src/main.rs`: the `--push` flag on `task merge` (additions only).
- `agent-brain/src/dashboard/taskstate.rs`: `build_merge_push` and its test.
- `agent-brain/src/dashboard/api_tasks.rs`: the route, `merge_busy_reason`, the `merge_push` control in `load_task_state`, the `run_control_as` arm, `ControlOutcome::MergePushed`, `absent_control`, and `merge_base` in the task JSON.
- `agent-brain/src/telegram/callback.rs`: ops `MergeAsk` (`g`), `MergeConfirm` (`G`), `MergeCancel` (`h`).
- `agent-brain/src/telegram/render.rs`: the button on the QA pass message, plus `merge_confirm_text` and `merge_result_text`.
- `agent-brain/src/telegram/bot.rs`: `on_merge_push` and the `MergePushed` arm in `apply_outcome`.
- `agent-brain/assets/dashboard/js/task.js`: the button, dialog and result message. `task.html` was not changed because the generic dialog is reused. No new Tailwind classes, so `app.css` was not rebuilt.
- `agent-brain/README.md`: one CLI table row and the section `### Merge & push sau QA đạt` (additions only).
- Tests: `agent-brain/tests/task_merge_push.rs` (new, 13 tests), `agent-brain/tests/telegram_merge_push.rs` (new, 1 test), `agent-brain/tests/dashboard_actions.rs` (+2 tests), `agent-brain/tests/telegram_notify.rs` (one assertion updated, see deviations).

## Commits

| sha | subject |
|---|---|
| bd45a5e | feat(agent-brain): git_net with timeout, remote/rev helpers, branch name check |
| 4d42576 | feat(agent-brain): merge config block (default base, git timeout, delete branch) |
| fd55ff8 | feat(agent-brain): task merge --push (fetch, merge --no-ff, push, delete branch) |
| 7dbff22 | feat(agent-brain): Merge & push control on the dashboard and Telegram |
| ad32d0b | feat(agent-brain): Merge & push button and confirmation dialog on the task screen |
| e2c3ebd | test(agent-brain): task merge --push against a local bare origin |
| d5f2e0a | test(agent-brain): dashboard merge_push control and route |
| 092833e | test(agent-brain): Telegram Merge & push asks, runs once, ignores strangers |
| 6d876a4 | docs(agent-brain): README row and section for task merge --push |
| f5240ce | fix(agent-brain): git_net keeps the user's own ssh command |
| 1e3971b | test(agent-brain): base created from origin; QA pass message has Merge & push |

The summary itself is committed as `docs(planning): summary for task merge and push`.

## Verification (run in the worktree, with the worktree root in place of the repo path)

| Command | Result |
|---|---|
| Baseline (section 0), `cargo test --no-fail-fast --offline --locked` at a03d39d | **passed 1719 failed 0** |
| `cargo test ... --test task_merge_push` | ok. 13 passed; 0 failed |
| `cargo test ... --test dashboard_actions` | ok. 16 passed; 0 failed (baseline 14) |
| `cargo test ... --test telegram_merge_push` | ok. 1 passed; 0 failed |
| `cargo test ... --lib` | ok. 724 passed; 0 failed (baseline 716) |
| `cargo test ... --no-fail-fast` summed | **passed 1743 failed 0** (+24) |
| `bash scripts/dashboard-ui-lint.sh` | UI lint OK: 324 thẻ tương tác (exit 0) |
| `bash scripts/build-css.sh --check` | app.css khớp nguồn (exit 0) |
| `node scripts/dashboard-js-check.mjs` | JS check OK: 1469 checks (exit 0) |

A secret-pattern grep over `git diff a03d39d..HEAD` found nothing. `spikes/lib/scan-secrets.sh` only scans `spikes/`, so it does not apply here.

## Deviations from the plan

1. **`agent-brain/src/telegram/bot.rs` was changed.** It is not named in "Phạm vi file", but it is where callback presses are handled and where `apply_outcome` must match every `ControlOutcome`. The plan allows "file khai báo kiểu nút nếu callback nằm riêng".
2. **`agent-brain/tests/telegram_notify.rs` (`qa_pass_notifies_summary`) was changed.** It asserted that the QA pass keyboard is exactly `["Mở lại task"]`. The feature adds `Merge & push` in front, so this was the only test that failed in the full run. The expectation is now `["Merge & push", "Mở lại task"]` with callback prefixes `g:1:` and `o:1:`.
3. **`run_control_as` instead of `run_task_action`.** `merge_push` is a control (`view.controls`, like `cleanup_worktree`), so it lives in `run_control_as`, the same function the dashboard route and the Telegram confirm press call. `run_task_action` only starts role jobs.
4. **Name check comes first.** The branch names are validated before the status check, so no git command runs with an unchecked name (test 12). The status check needs git when the task is already in state C. Refusal order after that: status (with the C exception), busy, dirty tree, missing branch.
5. **`MergePushOutcome` has an extra field `push_failed`.** It sets the CLI exit code (exit 1 only when refused or the push failed) and the dashboard JSON `push_failed`.
6. **`build_merge_push(task, busy, git_state, base)` has an extra `base` argument,** used for the reason `Đã merge vào <base> và push`.
7. **Busy source.** The dashboard uses `merge_busy_reason`: the task's own live run or driver, then `api_tasks::busy_reason`. The CLI uses `merge_busy_cli`: the task's own run or driver, then `claim::busy_reason_ws`, the same mode-aware check `run` and `auto` use.
8. **No `-f` on `git rm --cached`.** Stripping protocol files uses `git rm -q --cached -- <path>` without `-f`. The index matches the working tree after the merge, so `-f` is not needed, and test 11 can then reject `-f` in every command. Conflict resolution through the shared `merge_task` helper still uses `git rm -q -f` (old behaviour). It is not reached in the never-force scenario.
9. **`git_net` refinement** (commit f5240ce). It also leaves ssh alone when `GIT_SSH` or `core.sshCommand` is set, because the `GIT_SSH_COMMAND` variable would otherwise override the user's configured ssh.
10. **Process lock.** `merge_push_task` holds a process-wide mutex, so two presses at once (dashboard and Telegram) never run git side by side in one repo. This is in addition to the plan's per-press checks.
11. **State C without the task branch.** When the task branch is gone, state C (or B) is found from the `[task #<id>]` mark in the first-parent subjects of `origin/<base>` (or `<base>`). This lets "already done" work again after the branch was deleted.
12. **No local origin and state B.** A second run without origin reports "đã merge từ trước" and keeps the task branch, because it was never pushed.
13. **The README commit uses the `docs(agent-brain):` prefix,** and the ssh refinement uses `fix(agent-brain):`. The plan named only `feat`/`test`, but these prefixes are more accurate.
14. **Tailwind binary.** `scripts/build-css.sh --check` needs `agent-brain/tools/tailwindcss`, which is gitignored and was missing in the fresh worktree. I copied the binary from the main checkout (read-only use; its sha256 is checked by the script). It is untracked and ignored, and nothing was committed.

## Notes on tests

- Timeout: `gitops::tests::git_net_timeout_kills_the_process_group` runs a slow git alias (`exec sleep 30`) with a 1 s limit. It checks the error `quá 1s`, an elapsed time under 10 s, and that the recorded process is gone. A `pre-receive` sleep was not needed.
- Never force (test 11): a `git` wrapper on PATH records every command line over a rejected push and its retry. No line contains `-f`, `--force*`, `+…`, `-D`, `--hard`, `--delete`, `reset` or `clean`. The push line is exactly `push origin refs/heads/dev:refs/heads/dev`, and the delete is `branch -d -- task/sua-app`.
- Busy (test 6, dashboard): a live `sleep` process registered as a running run, the same method as `tests/runner_kill.rs`.
- Telegram: a single scenario covers the button on QA pass, the stranger chat being ignored, the first press only asking (no git), `Hủy`, `Xác nhận` running once, and an old confirm press being refused with no second merge.

## Open points

- Before a merge-push run, the dashboard control does not check for a dirty tree, an ahead/diverged base or a network problem (it is built from local refs only, every poll). Those cases are refused when the button is pressed, with the reason shown.
- In shared mode the repo is left on the base branch after a merge-push. Another task's next run in the same project has to check out its own branch; existing role logic handles or refuses that.
- In worktree mode the busy check is the mode-aware one (`max_parallel_tasks`). Merging in the main repo does not touch task worktrees, but the task's own worktree keeps the branch (a note says to use `Dọn worktree` first).
- Not done (outside the plan): marking the task in the DB after the push, and pushing tags.
