---
phase: 07-journal-vault-obsidian
plan: 09
subsystem: journal
tags: [git, gap-closure, no-push, obsidian, vault]
requires: [07-02, 07-05, 07-08]
provides:
  - "gitsync::pending_paths: generated, marker-bearing paths that are untracked or differ from HEAD"
  - "same-clone first-run flow (--no-push, review, push) commits everything in one commit"
affects: [agent-brain/src/journal/gitsync.rs, agent-brain/src/journal/mod.rs]
key-files:
  modified:
    - agent-brain/src/journal/gitsync.rs
    - agent-brain/src/journal/mod.rs
    - agent-brain/tests/journal_gitsync.rs
    - scripts/phase7-live-check.sh
    - agent-brain/README.md
decisions:
  - "pending_paths also accepts an untracked .gitignore that is byte-identical to writer::GITIGNORE_BOOTSTRAP (see deviation 1)"
metrics:
  tasks: 3
  completed: 2026-11-10
status: complete
gap_ids: [G-07-NOPUSH-THEN-PUSH]
requirements: [JRN-02, JRN-04]
actuals:
  tasks: 3
  commits: 3
plan_head_before: 567ee978f98f33e0e605f29df4c45438af0bb2fe
---

# Phase 7 Plan 09: no-push then push on the same clone Summary

Closes gap G-07-NOPUSH-THEN-PUSH: after `agent-brain journal export --no-push`, the next `agent-brain journal export` on the same clone now commits and pushes every generated file that is not yet committed, in one commit, and a third run makes no commit.

## What was done

1. **Task 1 (test, cbe4ea5)** - six tests in `agent-brain/tests/journal_gitsync.rs`: `no_push_then_push_commits_pending_files`, `no_push_then_push_is_idempotent`, `no_push_then_push_on_non_empty_vault`, `no_push_removed_stale_then_push_commits_deletion`, `pending_never_stages_foreign_or_obsidian`, `no_push_stages_nothing`. RED proof, run before the fix: `no_push_then_push_commits_pending_files ... FAILED` with the CLI printing `Nhật ký: không có thay đổi.` (the defect from 07-08), 4 more new tests also failed, `no_push_stages_nothing` passed. Result line: `11 passed; 5 failed`.
2. **Task 2 (fix, cce0b1e)** - `gitsync::pending_paths(ctx, dir, generated)`: one `git status --porcelain=v1 -z --untracked-files=all`, keeps paths in `generated` that carry the marker and are `??` or differ from HEAD (`A`/`M`/`T` in either column); never `.obsidian/`, never deleted paths, empty list on git error. `export_with_tz` unions them into `paths` and `written.changed` only when `p.push` is true, so the commit subject count and `Nhật ký: đã đẩy <n> tệp` equal the files really staged. No signature change, staging still `add --pathspec-from-file`.
3. **Task 3 (docs, b499d13)** - `scripts/phase7-live-check.sh` checks A and B now run on one clone and one vault (the separate `vault-a`/`clone-a` workaround is gone; A also asserts nothing is staged). README first-run step 2 (one line) now says to run `agent-brain journal export` on the same clone; the "delete the clone" instruction is removed. No other README line changed.

## Verification (real output)

- `cargo test ... --test journal_gitsync --test journal_export --test journal_serve`: `test result: ok. 5 passed` (journal_export), `ok. 16 passed` (journal_gitsync: 10 existing + 6 new), `ok. 7 passed` (journal_serve), 0 failed.
- `cargo test ... --lib journal`: `65 passed; 0 failed`.
- `bash scripts/phase7-live-check.sh`: checks A..G `LIVE OK`, final line `Phase 7 live check OK`.
- Full suite `cargo test --offline --locked --no-fail-fast`: `passed 1858 failed 0` (1852 before + 6 new).
- Forbidden-command grep (`"-A"`, `"--all"`, `--force`, `reset`, `--amend`, `stash`) on gitsync.rs and mod.rs: no match.
- `git diff --stat 567ee97..HEAD`: only the five declared files; Cargo.toml and Cargo.lock unchanged.
- `rustfmt --check --edition 2021` clean on gitsync.rs, mod.rs and the test file after formatting my added lines (the three files were clean on base).

## Deviations from Plan

**1. [Rule 1 - Bug] Bootstrap `.gitignore` also needs to be staged**
- **Found during:** Task 2 (first green run: `assert_clean` showed `?? .gitignore`).
- **Issue:** On a fresh vault `writer::apply` creates `.gitignore` (content `.obsidian/\n.trash/\n`, no marker, not in `generated`). A `--no-push` run reports it as changed; the following push run sees it as unchanged, so it stayed untracked and the clone was not clean. A direct push run (no earlier `--no-push`) does commit it, so the same-clone flow would differ.
- **Fix:** `pending_paths` additionally returns `.gitignore` only when it is untracked (`??`) and byte-identical to `writer::GITIGNORE_BOOTSTRAP`. A user-edited or tracked-and-modified `.gitignore` is never returned (the existing `dirty_clone_stops_without_overwrite` test still passes). Test 1 asserts `.gitignore` is in the commit.
- **Files:** agent-brain/src/journal/gitsync.rs, agent-brain/tests/journal_gitsync.rs
- **Commit:** cce0b1e

The plan's `pending_paths` contract said "only paths in `generated`"; this is the one exception and it is narrowly bounded as above.

**2. Commit ledger (0c)** was not written to the git dir; `commits: 3` and `plan_head_before` were taken from `git log` against the worktree base SHA (3 commits: cbe4ea5, cce0b1e, b499d13, plus this SUMMARY commit).

## Known Stubs

None.

## Threat Flags

None. T-07-09-01..03 mitigated as planned (marker + generated + not `.obsidian/` filter; explicit pathspec only; count taken after the union and compared in tests with `git show --name-only`). No dependency added.

## Self-Check: PASSED

- cbe4ea5, cce0b1e, b499d13 are ancestors of HEAD.
- STATE.md, ROADMAP.md, REQUIREMENTS.md not modified.
