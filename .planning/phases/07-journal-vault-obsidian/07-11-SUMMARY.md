---
phase: 07-journal-vault-obsidian
plan: 11
subsystem: journal git runner and Telegram journal mail
tags: [git, env, pathspec, branch, ssh, sqlite, telegram, gap-closure]
gap_closure: true
gap_ids: [WR-04, WR-09, WR-10, IN-07]
requires:
  - phase: 07-journal-vault-obsidian
    provides: "gitsync runner (07-05, 07-09), JournalMail scan (07-07)"
provides:
  - "journal git commands immune to inherited GIT_* repository variables, with literal pathspecs"
  - "HEAD verified/repaired to journal.branch before commit and push"
  - "user ssh command kept and hardened instead of replaced"
  - "mail scan returns already-claimed rows when a later row errors"
affects: [07-10 (compiles state.rs against GitStop/Published/Prepared, unchanged)]
key-files:
  modified:
    - agent-brain/src/journal/gitsync.rs
    - agent-brain/src/telegram/detect.rs
  created:
    - agent-brain/tests/journal_gitmail.rs
decisions:
  - "wrong-branch repair uses plain `switch <branch>` when the local branch already exists (keeps unpushed commits), `checkout -B <branch> origin/<branch>` only when it does not, and an unborn branch plus `read-tree --empty` when the remote has no such branch"
  - "GIT_SSH alone (a program that may not take -o) is left untouched; GIT_SSH_COMMAND is not set for it"
status: complete
metrics:
  tasks: 2
  completed: 2026-11-10
commits: 3
plan_head_before: 7de1dedf4daa92a7fbdd9558f31a0265cd8513ea
plan_head_after: a5fa0f2ad317e7055028da6228b435e71b5053dc
actuals:
  tokens: 21000
  tasks: 2
  commits: 3
---

# Phase 7 Plan 11: git runner hardening and mail claim Summary

The journal git runner can no longer be redirected by the environment or push the wrong branch, keeps the user's ssh command, and the Telegram mail scan no longer drops claimed rows on a later database error.

## What changed

**Task 1 - `agent-brain/src/journal/gitsync.rs` (commit fb120f4)**
- WR-04: `command()` now `env_remove`s `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_COMMON_DIR`, `GIT_OBJECT_DIRECTORY`, `GIT_NAMESPACE`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_PREFIX` and sets `GIT_LITERAL_PATHSPECS=1`. The existing variables (`GIT_TERMINAL_PROMPT`, `GIT_ALLOW_PROTOCOL`, author/committer, the `-c` overrides) are unchanged.
- IN-07: new `ssh_command()` builds `GIT_SSH_COMMAND` as the user's `GIT_SSH_COMMAND` or `core.sshCommand` (read only for clone/fetch/push/pull/ls-remote) plus the fixed hardening options; with nothing configured the fixed command is unchanged; `GIT_SSH` alone is left untouched.
- WR-10: new `ensure_branch()` runs first in `sync_existing`. HEAD not on `refs/heads/<branch>` (other branch, detached, unborn): clean tracked tree -> repaired; otherwise `Stopped(GitStop::Conflict { detail })` with a fixed Vietnamese text without paths. `commit_and_push` additionally refuses to commit or push (returns `CommittedNotPushed`) when HEAD is not the journal branch. No new variants or signature changes.

**Task 2 - `agent-brain/src/telegram/detect.rs` (commit 11e9867), tests (commit a5fa0f2)**
- WR-09: `JournalMail::scan` already claimed per row; the loss was the `?` on a later `UPDATE`. A database error after at least one claimed row now returns the rows collected so far (the failing row is not claimed, so the next scan retries it); an error on the first row is still an `Err`.
- `agent-brain/tests/journal_gitmail.rs`: 11 tests.

## Verification (real output)

- `cargo test --offline --locked --lib journal::gitsync`: `2 passed` (includes the new `ssh_command` unit test).
- `--test journal_gitsync --test journal_gitmail`: `11 passed` (gitmail) and `16 passed` (the 16 existing gitsync tests).
- `--test journal_gitmail --test journal_serve`: `11 passed; 0 failed` and `7 passed; 0 failed`.
- Full suite `cargo test --offline --locked --no-fail-fast`: `92 binaries, 1870 passed, 0 failed`.
- Acceptance greps: `GIT_LITERAL_PATHSPECS` and `GIT_DIR` present in gitsync.rs; no added `--force`, `+refs`, `reset`, `--amend`, `stash`, `-A`, `--all`. `git diff --name-only 7de1ded..HEAD` lists only the three declared files; Cargo files unchanged.
- rustfmt: `gitsync.rs` and `journal_gitmail.rs` are rustfmt-clean (`rustfmt --check --edition 2021`). `detect.rs` was already not clean on the base (diffs at lines ~77, 285, 299, 738+, none in the lines changed here); left as is.

### Do the guard tests fail without the fix?

Ran the 11 tests with `gitsync.rs` and `detect.rs` reverted to base 7de1ded: 8 failed (`export_ignores_inherited_git_dir`, `clone_on_another_branch_is_repaired...`, `dirty_clone_on_another_branch_stops...`, `detached_head_is_repaired`, `users_ssh_command_is_kept...`, `core_ssh_command_is_kept...`, `git_ssh_program_alone_is_left_untouched`, `mail_scan_returns_claimed_rows...`). Three passed on the base:
- `mail_scan_with_the_very_first_row_failing_...`: regression guard for the unchanged first-row `Err` behaviour, intentionally passes on both.
- `unborn_head_on_a_stray_branch_is_repaired`: the base `integrate` already repairs an unborn HEAD once a remote branch exists (after fetch). Kept as a regression guard.
- `marked_files_with_glob_names_...`: first version passed on the base because git matches an existing exact path before treating `*`/`[` as a glob (`add` and `rm --cached` of an exact, existing name are safe). Reworked so it covers the case where it matters: a generated name `Dự án/c*.md` that is gone and untracked, next to a tracked `Dự án/cc.md`. With only `GIT_LITERAL_PATHSPECS` removed it fails (`cc.md` deleted from the commit); with the fix it passes.

## Deviations from Plan

1. **[Rule 1 - safety] Repair uses `switch <branch>` when the local branch already exists.** The plan names `checkout -B <branch> origin/<branch>`, which would reset a local `<branch>` that holds unpushed commits. `checkout -B` is used only when the local branch is missing; when the remote has no such branch, HEAD is pointed at a new unborn `<branch>` and the index emptied with `read-tree --empty` (working-tree files untouched) so the first commit does not carry the old branch's tree.
2. **Branch check runs before `check_dirty`** so a dirty tree on the wrong branch yields the required `Conflict` rather than `Dirty`. A tree with tracked changes on the wrong branch is never repaired, even if those changes are marked generated files (conservative).
3. **Extra guard in `commit_and_push`** (HEAD must be the journal branch) beyond the plan, as belt and braces for T-07-11-03; the signature is unchanged.
4. **Per-row un-claim not implemented.** Nothing in `scan` can fail after the claim (`clip` and building `Outgoing::Message` are infallible), so there is no un-claim path to write. A claimed row dropped by `Outbox::push` when the 200-item queue overflows (the second half of WR-09) is not fixed: `Outbox::push` is infallible and lives outside this plan's files. Left as a known gap; the review's alternative (mark `sent_at` from the sender after `sendMessage`) needs changes in `outbox.rs`/`detect.rs::run`.
5. **`ssh_command` is `pub(crate)` and unit-tested in `gitsync.rs`**, while the user-visible behaviour is tested through the real binary with a fake ssh script, so no public accessor was added.

## Known limitations

- A clone left by an interrupted `clone --no-checkout` (empty index) on another branch counts as having tracked changes (every file shows as deleted), so it stops with the `Conflict` notice instead of repairing; the user fixes it by hand. This stays safe (nothing pushed).
- The first full-suite attempt (two overlapping full runs on the same machine) reported 1 failed test in a 21-test binary I did not capture; a clean single full run afterwards was 1870 passed, 0 failed. Treated as load flakiness, not reproduced.

## Known Stubs

None.

## Threat Flags

None. The new `git config --get core.sshCommand` call runs with the repository variables scrubbed and no shell.

## Self-Check: PASSED

- agent-brain/tests/journal_gitmail.rs: FOUND
- commits fb120f4, 11e9867, a5fa0f2: present on the branch (3 commits from base 7de1ded)
