---
phase: 07-journal-vault-obsidian
plan: 01
subsystem: database
tags: [sqlite, migration, fts5, foreign-keys, rust]
requires: []
provides:
  - "db::apply_pending_migrations: FK check compares before/after counts, only an increase aborts"
  - "migration 12 / SCHEMA_VERSION = 12: journal_state, journal_mail (+ journal_mail_unsent), journal_fts (FTS5)"
affects: [07-02, 07-05, 07-06, 07-07, 07-08]
tech-stack:
  added: []
  patterns: ["tolerate pre-existing FK damage, fail on new damage inside the IMMEDIATE migration transaction"]
key-files:
  created: []
  modified:
    - agent-brain/src/db.rs
    - agent-brain/tests/db_migration.rs
key-decisions:
  - "D-21 implemented: no repair, no deletion, no file copy in db::open; pre-existing violations only emit a warning"
requirements-completed: [JRN-07]
status: complete
commits: 2
plan_head_before: 55436415b4655a0c401afe54bf89809023d3ebc7
plan_head_after: b8f9253eece017834e0ee7544391747c50b3f852
actuals:
  tokens: 14000
  tasks: 2
  commits: 2
duration: ~25min
completed: 2026-11-10
---

# Phase 7 Plan 01: FK-tolerant migrations and migration 12 Summary

**`db::open` now upgrades a brain.db that already holds orphan `events` rows (it fails only when a migration adds violations), and migration 12 adds `journal_state`, `journal_mail` and the FTS5 table `journal_fts` (`SCHEMA_VERSION` = 12).**

## Tasks

| Task | Name | Commit |
| ---- | ---- | ------ |
| 1 | FK check tolerates pre-existing violations, still fails on new ones | 2d64a81 |
| 2 | Migration 12 (journal tables), SCHEMA_VERSION 12, test updates | b8f9253 |

## What changed

- `agent-brain/src/db.rs`
  - New private `foreign_key_violations` and `apply_pending_migrations(tx, migrations, from)`: counts `pragma_foreign_key_check` before, runs `migrations[from..]` setting `user_version` after each, counts again. `after > before` returns `Nâng cấp CSDL thất bại: vi phạm khóa ngoại tăng từ {before} lên {after} sau khi nâng lược đồ` (the transaction is dropped, so it rolls back). `before > 0` and no increase emits one `Cảnh báo: ...` line through `crate::term::emit`.
  - `open` calls it instead of the inline loop; foreign_keys OFF, IMMEDIATE transaction, re-read of `user_version`, commit and foreign_keys ON are unchanged.
  - Migration 12 appended with the exact shapes of the plan (`journal_state`, `journal_mail` + index, `journal_fts` with `tokenize='unicode61 remove_diacritics 0'`); no CHECK on `journal_mail.kind`.
- `agent-brain/tests/db_migration.rs`: helpers `seed_with_orphan_events`, `build_v10`, `build_v11`, `build_v11_with_orphans`; tests `v10_db_with_orphan_events_upgrades`, `v11_db_upgrades_to_v12_keeping_data`, `v11_db_with_orphan_events_upgrades_to_v12`, `v12_reopen_runs_no_migration`; the eight literal version asserts changed 11 -> 12.
- Unit tests in `db::tests`: `preexisting_fk_violations_do_not_block_migration`, `migration_that_adds_fk_violation_fails` (checks the message prefix, `từ 0 lên 1`, that the insert and `user_version` are rolled back).

## Verification (real output)

- RED confirmed first: `v10_db_with_orphan_events_upgrades` failed with the old `vi phạm khóa ngoại sau khi dựng lại bảng runs`; after the version asserts moved to 12, 10 tests failed until migration 12 existed.
- `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib db::tests -- --test-threads=1`: `6 passed; 0 failed`.
- `cargo test ... --test db_migration --test lib_foundation --test task_origin -- --test-threads=1`: `db_migration` 34 passed, `lib_foundation` 13 passed, `task_origin` 11 passed, 0 failed.
- `cargo test ... --lib` (whole lib suite): `718 passed; 0 failed`.
- `git diff --exit-code 5543641 -- agent-brain/Cargo.toml agent-brain/Cargo.lock`: no diff (no dependency added).

## Deviations from Plan

1. **[Rule 3 - Blocking] `cargo fmt --check` cannot be clean repo-wide.** On the base commit `cargo fmt --check` already reports diffs in ~95 files (including pre-existing hunks of `db.rs` and `db_migration.rs`). Running `cargo fmt` as the plan says reformatted 96 files; I reverted all of them (per-path `git checkout -- agent-brain/src agent-brain/tests` after saving my work) and re-applied only my changes in rustfmt style. Verified that no `cargo fmt --check` hunk falls inside the lines I added; the remaining diffs are pre-existing. The Task 2 verify command `cargo fmt --check` therefore still exits non-zero for reasons outside this plan's scope (out of scope; a repo-wide format pass belongs elsewhere).
2. **Verify-text mismatch (test output interleaving).** The plan's `fails_when` looks for the literal `v10_db_with_orphan_events_upgrades ... ok` (and the `preexisting_...` / `v11_..._orphan_...` names). Because the specified `crate::term::emit` warning writes straight to stderr (not captured by the test harness), cargo's stdout line becomes `test name ... Cảnh báo: ...` followed by `ok` on the next line. With stderr discarded (`2>/dev/null`) the lines read `test v10_db_with_orphan_events_upgrades ... ok`, `test v11_db_with_orphan_events_upgrades_to_v12 ... ok`, `test db::tests::preexisting_fk_violations_do_not_block_migration ... ok`. Behavior is as the plan specifies; only the grep-style check needs stderr dropped.
3. **Plan commit ledger not persisted.** The worktree sandbox refused writing to the shared `.git/worktrees/...` path, so `commits`/`plan_head_before` come from `git rev-list --count 5543641..HEAD` = 2 (base `5543641...` was verified at start). Both values are measured, not narrated.

No auth gates. No stubs. No new network/auth/file surface (threat flags: none).

## Threat model

T-07-01 and T-07-02 mitigated and covered by `v10_db_with_orphan_events_upgrades`, `v11_db_with_orphan_events_upgrades_to_v12`, `migration_that_adds_fk_violation_fails`. T-07-SC: Cargo files unchanged.

## Notes for later plans

- 07-08 README must carry the section the warning refers to: `Nâng cấp lên Phase 7 (lược đồ 12)` (backup before upgrade; older binaries refuse a v12 DB).
- Existing orphan rows are never touched by `db::open`; any cleanup is a documented manual step.

## Self-Check: PASSED

- agent-brain/src/db.rs: FOUND (`fn apply_pending_migrations`, `CREATE VIRTUAL TABLE journal_fts USING fts5`, `SCHEMA_VERSION: i64 = 12`)
- agent-brain/tests/db_migration.rs: FOUND (`fn v10_db_with_orphan_events_upgrades`, `fn v11_db_upgrades_to_v12_keeping_data`)
- Commits 2d64a81 and b8f9253 are ancestors of HEAD.
