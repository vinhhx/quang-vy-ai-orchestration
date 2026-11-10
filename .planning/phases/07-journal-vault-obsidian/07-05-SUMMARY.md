---
phase: 07-journal-vault-obsidian
plan: 05
subsystem: journal
status: complete
tags: [journal, git, obsidian, failure-policy, D-10, D-22]
requires: ["07-02"]
provides:
  - "writer::apply stale marked-file deletion, foreign list, .gitignore bootstrap"
  - "gitsync::prepare sync of an existing clone (dirty check/restore, fetch, ff-only, rebase + abort)"
  - "gitsync::commit_and_push: add/rm --cached split, one fetch+rebase+push retry on a rejected push"
  - "state::record_outcome failure policy: push_fail_streak, pushfail/recovered/conflict/dirty/notrepo/secret/foreign mails"
affects: ["07-07 (delivers journal_mail rows)", "07-03 (Today.md rendering meets the foreign check)"]
tech-stack:
  added: []
  patterns:
    - "git integration without history rewrite: merge --ff-only / rebase of unpushed commits / rebase --abort"
    - "fetch-failure marker in .git so an offline cycle with nothing to push still counts as a failed push"
    - "FNV-1a dedupe keys on journal_mail for one mail per cause"
key-files:
  created:
    - agent-brain/tests/journal_gitsync.rs
  modified:
    - agent-brain/src/journal/writer.rs
    - agent-brain/src/journal/gitsync.rs
    - agent-brain/src/journal/state.rs
decisions:
  - "An offline fetch leaves a category marker in .git; with nothing to push, commit_and_push then reports CommittedNotPushed(category) instead of UpToDate, so the streak is not reset while the remote is unreachable"
  - "Restore of marked generated files uses `checkout HEAD --pathspec-from-file` (index and work tree), so a crash between `git add` and `git commit` is also recovered"
  - "Removed paths are staged with `rm --cached --ignore-unmatch` (not `add`), so a deleted file that was never committed (e.g. written by --no-push) cannot fail the whole staging step"
  - "Stopped and SecretBlocked do not count as push failures; LocalOnly leaves the streak unchanged; PushFailed leaves `blocked` unchanged"
metrics:
  duration: "~45 min"
  completed: 2026-11-10
actuals:
  tokens: 15700
  tasks: 3
  commits: 3
plan_head_before: cad033c827859cf9837ab8a19642903e0dfa9389
plan_head_after: 8c8296d695cee682185a95f2db5748cef86d2a4a
---

# Phase 7 Plan 05: Vault git safety and D-10 failure policy Summary

The exporter now owns exactly the marked files in `Today.md`, `Nhật ký/` and `Dự án/`. It syncs an existing vault clone without rewriting published history: it fast-forwards, rebases only its own unpushed commits, aborts on conflict, commits locally while offline and catches up later. It turns failures into one Telegram-ready mail per cause, plus a single push-failure alert after N failures and a recovery notice.

## What was built

**Task 1, writer (D-22)** in `agent-brain/src/journal/writer.rs`:
- After writing the map, `apply` walks only `Today.md`, `Nhật ký/` and `Dự án/`. It does not follow symlinks (a symlinked root is skipped entirely) and skips every dot-name.
- It deletes a regular file that is not in the map only when `has_marker` is true. Each deleted file goes into `removed`, and empty directories under `Nhật ký/` are pruned. The root folder itself is kept.
- It creates `.gitignore` (`.obsidian/`, `.trash/`, LF) only when no `.gitignore` exists, including a symlink, and lists it in `changed`.
- `.obsidian/`, `.git/` and any other root are never read for deletion. The 07-02 path validation, the marker check before overwrite, and temp+rename are unchanged.

**Task 2, gitsync (D-10, D-22)** in `agent-brain/src/journal/gitsync.rs`:
- `prepare` on an existing clone runs in this order:
  1. Remove a stale `.git/index.lock` (older than `git_timeout_seconds`).
  2. Abort a leftover rebase.
  3. Dirty check with `status --porcelain=v1 -z --untracked-files=no`. Marked generated files, and deleted managed files whose `HEAD` version carries the marker, are restored with `checkout HEAD --pathspec-from-file`. Anything else returns `Stopped(Dirty)`.
  4. `fetch origin <branch>`. If it fails, `prepare` records the category in `.git/agent-brain-journal-fetch-failed` and returns `Ready`.
  5. Integrate: `rev-list --left-right --count`, then `merge --ff-only` when only behind, or `rebase origin/<branch>` when diverged. A failed rebase collects `diff --name-only --diff-filter=U -z`, runs `rebase --abort` and returns `Stopped(Conflict { detail: paths })`.
  6. With an unborn `HEAD`, it runs `checkout -B <branch> origin/<branch>`.
- `commit_and_push`:
  - Stages existing paths with `add` and deleted paths with `rm --cached --ignore-unmatch`. Both go through NUL-separated `--pathspec-from-file`.
  - Pushes `HEAD:refs/heads/<branch>`. If the push is `Rejected`, it does one fetch, integrates and pushes again.
  - With nothing to push but a failed fetch, it returns `CommittedNotPushed`.
  - It never forces a push, hard-resets or amends. The acceptance grep is empty.

**Task 3, state (D-10, D-20)** in `agent-brain/src/journal/state.rs`:
- `record_outcome` keeps the 07-02 keys and adds `push_fail_streak`, `push_fail_since_ms`, `push_alert_sent` and `blocked`.
- The push-failure threshold comes only from `cfg.push_fail_notify_after`.
- Mail kinds, texts and dedupe keys follow the plan's `<interfaces>` exactly: `pushfail:{since}`, `recovered:{since}`, `conflict:{fnv}`, `dirty:{fnv}`, `notrepo:{day}`, `secret:{fnv}`, `foreign:{fnv}`.
- Path lists are capped at 10 entries, followed by `… và {k} tệp khác`. FNV-1a 64 is a private function.
- All writes for one report run in one transaction (`unchecked_transaction` when the connection is in autocommit mode, otherwise the caller's transaction).

## Tests added

- Unit tests:
  - `journal::writer::tests`: `stale_marker_file_is_deleted`, `marker_less_file_is_kept`, `obsidian_dir_untouched`, `gitignore_bootstrapped_only_when_absent`, `symlink_in_managed_root_not_followed`
  - `journal::gitsync::tests::classify_examples` (renamed from `classify_categories` and extended)
  - `journal::state::tests`: `alert_once_after_n_failures`, `recovery_mail_after_alert`, `no_recovery_mail_without_alert`, `cause_mails_once_per_cause`, `secret_mail_has_paths_not_values`, `foreign_mail_dedupes`, `state_survives_reopen`, `fnv_is_stable`
- Integration tests in `tests/journal_gitsync.rs`: `nonempty_remote_keeps_human_files`, `human_file_at_generated_path_is_not_overwritten`, `offline_commit_then_catch_up`, `remote_moved_fast_forwards`, `diverged_history_rebases_local_commits`, `conflict_aborts_without_overwrite`, `dirty_clone_stops_without_overwrite`, `not_a_repo_dir_is_refused`, `rejected_push_retries_once_after_rebase` (extra), `secret_blocked_export_mails_user`

## Verification (real output)

```
cargo test --offline --locked --lib journal::writer -- --test-threads=1
  test result: ok. 7 passed; 0 failed   (the 5 plan tests ... ok)
cargo test --offline --locked --lib journal::gitsync -- --test-threads=1
  running 1 test / journal::gitsync::tests::classify_examples ... ok
cargo test --offline --locked --lib journal::state -- --test-threads=1
  running 10 tests / test result: ok. 10 passed; 0 failed
cargo test --offline --locked --test journal_gitsync --test journal_export -- --test-threads=1
  journal_export: test result: ok. 5 passed; 0 failed
  journal_gitsync: test result: ok. 10 passed; 0 failed
cargo test --offline --locked --lib journal -- --test-threads=1
  test result: ok. 38 passed; 0 failed
cargo build --offline --locked --all-targets   -> no warnings, no errors
git diff --exit-code a03d39d -- Cargo.toml Cargo.lock   -> no diff
rustfmt --check --edition 2021 on writer.rs, gitsync.rs, state.rs, tests/journal_gitsync.rs -> clean
```

Acceptance greps:
- gitsync `--force|reset --hard|push -f|--amend|--autostash|force-with-lease`: nothing.
- `rebase` matches both the rebase onto `origin/<branch>` and `rebase --abort`; `merge --ff-only` and `HEAD:refs/heads/` are present.
- state: `push_fail_notify_after` on 3 lines; `streak >= <digit>`: nothing; kind literals: 30 lines; `stderr|repo_url|clone_dir`: nothing.
- writer: `has_marker` on 3 or more lines, plus `obsidian` and `remove_file`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] An offline cycle with nothing new reset the push-failure streak**
- **Found during:** Task 2/3 design.
- **Issue:** `prepare` returns `Ready` when `fetch` fails. If nothing new is committed, `HEAD == origin/<branch>` makes `commit_and_push` return `UpToDate`, which `record_outcome` treats as success. A 30-minute serve loop with no new data would then never reach N failures while offline.
- **Fix:** `prepare` writes the fetch-failure category to `.git/agent-brain-journal-fetch-failed`. It is cleared on a successful fetch or push. When there is nothing to push and the marker is present, `commit_and_push` returns `CommittedNotPushed(category)`.
- **Files modified:** `gitsync.rs`. **Commit:** b758aa6.

**2. [Rule 1 - Bug] `git add` of a removed path that was never committed fails the whole staging step**
- **Fix:** Deleted paths are staged with `rm -q --cached --ignore-unmatch --pathspec-from-file`, and existing paths with `add`. **Commit:** b758aa6.

**3. [Rule 2 - Correctness] Restore from `HEAD` instead of from the index**
- **Issue:** The plan said `checkout -- <paths>`.
- **Fix:** `checkout HEAD --pathspec-from-file=- --pathspec-file-nul` restores both the index and the work tree. A crash between `git add` and `git commit` therefore no longer leaves staged content behind. A deleted managed file is also treated as the exporter's own when its `HEAD` version has the marker. Added files (`A`) that carry the marker are left for the writer. **Commit:** b758aa6.

**4. [Rule 2] Leftover rebase state is aborted before syncing**
- **Fix:** A `.git/rebase-merge` or `.git/rebase-apply` left by an interrupted run (Pitfall 16) is cleared with `rebase --abort` before the dirty check. **Commit:** b758aa6.

**5. 07-02 unit test `writes_skips_identical_and_leaves_foreign` updated**
- **Why:** `changed` now also lists the bootstrapped `.gitignore` in a fresh directory, which is the behaviour this plan requires. **Commit:** 3154799.

**6. Extra test `rejected_push_retries_once_after_rebase`**
- **Why:** The CLI scenarios never reach the push-rejected retry, because `prepare` always fetches first. This library-level test moves the remote between `prepare` and the push. To keep library git calls on the sandbox config, it sets `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_NOSYSTEM` in the test process.

### Plan/environment notes

- **`cargo fmt --check` crate-wide is not gating.** The base repository already has about 100 unformatted files, so formatting was not run on the crate. Instead, `rustfmt --check --edition 2021` passes on every file this plan touched.
- **Merge-conflict and fast-forward failures with no unmerged paths** (for example, untracked files in the way) report `Stopped(Conflict { detail: "origin/<branch>" })`. The mail then names the branch, never a path or stderr.
- **`human_file_at_generated_path_is_not_overwritten`** finds out whether `Today.md` is rendered (it is only after 07-03 merges) by running a `--no-push` export in a second sandbox. It asserts the warning only in that case; the remote `Today.md` must stay byte-identical either way. The warning is printed on stderr by `term::emit`, so the test checks stdout and stderr combined.
- `journal/mod.rs` was not edited (07-07 owns it). No Cargo changes. STATE.md and ROADMAP.md were not touched.

## Known Stubs

None.

## Threat Flags

None. All new surface (clone sync, deletions, alert texts) is covered by T-07-19..T-07-25. The `.git/agent-brain-journal-fetch-failed` marker holds only a category word.

## Self-Check: PASSED

- FOUND: agent-brain/src/journal/writer.rs, gitsync.rs, state.rs, agent-brain/tests/journal_gitsync.rs
- FOUND commits: 3154799, b758aa6, 8c8296d (ancestors of HEAD)
