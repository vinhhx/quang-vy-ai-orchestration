---
phase: "7"
slug: "journal-vault-obsidian"
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: "2026-11-10"
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution. Source of the map below: `07-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `cargo test` (unit `#[cfg(test)]` + `agent-brain/tests/*.rs`), `tempfile`, fake Telegram (`tests/fakes/fake_telegram.py`), local bare git repos as the vault remote |
| **Config file** | none (`agent-brain/Cargo.toml`); sandbox helper `tests/common/mod.rs` |
| **Quick run command** | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib journal` |
| **Full suite command** | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1` |
| **Estimated runtime** | quick ~30 s once built; full several minutes |

---

## Sampling Rate

- **After every task commit:** Run the quick run command
- **After every plan wave:** Run the targeted `--test journal_export --test journal_gitsync --test journal_telegram --test db_migration --test telegram_intake`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds for the quick command

---

## Per-Task Verification Map

`cargo test ...` below stands for `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked`; every command ends with `-- --test-threads=1`.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 07-01-T1 | 07-01 | 1 | JRN-07 | T-07-01, T-07-02 | Pre-existing orphan `events` rows no longer block a migration; a migration that adds a violation still fails and rolls back (D-21) | unit + integration | `cargo test ... --lib db::tests` ; `cargo test ... --test db_migration` (`v10_db_with_orphan_events_upgrades`) | ❌ W0 | ⬜ pending |
| 07-01-T2 | 07-01 | 1 | JRN-07 | T-07-03 | Migration 12: `journal_state`, `journal_mail` (UNIQUE dedupe_key), `journal_fts` (FTS5); v11 → v12 keeps data; `SCHEMA_VERSION == 12` | integration | `cargo test ... --test db_migration --test lib_foundation --test task_origin` | ❌ W0 | ⬜ pending |
| 07-02-T1 | 07-02 | 2 | JRN-01, JRN-02, JRN-04 | T-07-04..T-07-09, T-07-11 | Tracer: `journal export` → day file (D-17 format, +07:00 day) → bare remote, author `agent-brain journal`, clone in data dir, second run no commit, refused without config, known-value gate | e2e (bare remote) + unit | `cargo test ... --test journal_export` ; `cargo test ... --lib journal` | ❌ W0 | ⬜ pending |
| 07-02-T2 | 07-02 | 2 | JRN-04, JRN-06 | T-07-10 | `journal.lock` exclusive (Busy touches nothing); export keys in `journal_state`; FTS rebuild idempotent and folded | unit + integration | `cargo test ... --lib journal` ; `cargo test ... --test journal_export` (`export_rebuilds_fts_index`) | ❌ W0 | ⬜ pending |
| 07-02-T3 | 07-02 | 2 | JRN-07, JRN-01 | T-07-06 | Config absent = off; ranges; `repo_url`/`branch` injection rejected; groups validated; `journal_group` lenient (→ Công việc); github.com link percent-encoding | unit | `cargo test ... --lib journal::config` ; `cargo test ... --lib config::tests` | ❌ W0 | ⬜ pending |
| 07-03-T1 | 07-03 | 3 | JRN-01 | T-07-12 | D-16 four categories with precedence (quota > failed, violation > gate), gate via approvals, carry-over into today, groups only from `journal_group` | integration (fixture DB) + unit | `cargo test ... --test journal_render` ; `cargo test ... --lib journal` | ❌ W0 | ⬜ pending |
| 07-03-T2 | 07-03 | 3 | JRN-01, JRN-02 | T-07-13, T-07-14 | Golden: Today/day/month/year/project files, attention first, links resolve, no weekly file, NFC paths, deterministic bytes | golden-file | `cargo test ... --test journal_render` ; `cargo test ... --lib journal::render` | ❌ W0 | ⬜ pending |
| 07-04-T1 | 07-04 | 3 | JRN-03 | T-07-15, T-07-16 | Sanitizer corpus: paths, URLs, hosts, e-mails, key shapes, env names, diffs/code, markup removed before the cut; plain text unchanged | unit | `cargo test ... --lib journal::sanitize` | ❌ W0 | ⬜ pending |
| 07-04-T2 | 07-04 | 3 | JRN-03 | T-07-15, T-07-17 | Hostile request leaves nothing in the vault; planted secret (project id = known value) blocks write/commit/push, value never printed | integration (bare remote) | `cargo test ... --test journal_sanitize` | ❌ W0 | ⬜ pending |
| 07-05-T1 | 07-05 | 3 | JRN-02 | T-07-24, T-07-25 | Writer deletes only marked stale files, keeps marker-less files and `.obsidian/`, `.gitignore` only when absent | unit + integration | `cargo test ... --lib journal::writer` ; `cargo test ... --test journal_gitsync` | ❌ W0 | ⬜ pending |
| 07-05-T2 | 07-05 | 3 | JRN-04 | T-07-19, T-07-20, T-07-21 | Offline commit then catch-up push, remote moved (ff/rebase), conflict aborts without overwrite, dirty clone stops, not-a-repo refused, never force | integration (bare remotes) | `cargo test ... --test journal_gitsync` ; `cargo test ... --lib journal::gitsync` | ❌ W0 | ⬜ pending |
| 07-05-T3 | 07-05 | 3 | JRN-04, JRN-03 | T-07-22, T-07-23 | One alert after N failures, recovery notice, one mail per cause, persisted; secret mail has paths not values | unit + integration | `cargo test ... --lib journal::state` ; `cargo test ... --test journal_gitsync` (`secret_blocked_export_mails_user`) | ❌ W0 | ⬜ pending |
| 07-06-T1 | 07-06 | 3 | JRN-06 | T-07-26 | Folded quoted-token MATCH, caps, hostile queries never error, prefix search | unit | `cargo test ... --lib journal::fts` | ❌ W0 | ⬜ pending |
| 07-06-T2 | 07-06 | 3 | JRN-06 | T-07-26 | `/tim` handler, dispatch, `setMyCommands` entry, help text; reply = date · line + github.com link | unit + integration | `cargo test ... --lib telegram::journal_cmd` ; `cargo test ... --test telegram_intake` | ❌ W0 | ⬜ pending |
| 07-06-T3 | 07-06 | 3 | JRN-06 | T-07-27, T-07-28 | `/tim` end to end: folded words, nothing beyond the vault (cut text, plan file), hostile queries, stranger gets no reply | integration (fake Telegram) | `cargo test ... --test journal_tim` | ❌ W0 | ⬜ pending |
| 07-07-T1 | 07-07 | 3 | JRN-05, JRN-04 | T-07-30 | `digest_due`, `export_due`, `digest_text`, link fallback with a fake clock | unit | `cargo test ... --lib journal` | ❌ W0 | ⬜ pending |
| 07-07-T2 | 07-07 | 3 | JRN-05, JRN-04 | T-07-29, T-07-32 | Mailbox delivered once (claim before push), queued-while-stopped rows sent at start, stale digest dropped (D-20) | integration (fake Telegram) | `cargo test ... --test journal_serve mail` ; `cargo test ... --test journal_serve stale_digest_dropped` | ❌ W0 | ⬜ pending |
| 07-07-T3 | 07-07 | 3 | JRN-04, JRN-05 | T-07-29, T-07-30, T-07-31 | Serve loop exports on schedule; one digest after a successful push; none before 18:00 or when the push fails; restart does not resend; off by default | integration (serve + bare remote + fake Telegram) | `cargo test ... --test journal_serve` | ❌ W0 | ⬜ pending |
| 07-08-T1 | 07-08 | 4 | JRN-07 (+ JRN-01..04 on the real binary) | T-07-33 | Isolated live check of the CLI path; schema dry run on a copy (version 12, counts, FK not increased, real dir unchanged) | script | `bash scripts/phase7-live-check.sh` | ❌ W0 | ⬜ pending |
| 07-08-T2 | 07-08 | 4 | JRN-07 | T-07-34 | README upgrade (backup, dry run, rollback), CLI row, config keys, `/tim` and digest — insertions only | doc check | README grep command of 07-08 Task 2 | ❌ W0 | ⬜ pending |
| 07-08-T3 | 07-08 | 4 | JRN-01..JRN-07 | all | Phase gate: full suite, fmt, Cargo unchanged, journal static checks (no plan/run-dir reads, no model calls, no shared git helpers, no history rewrite) | full suite + static | `cargo test ... ` (full) ; static gate command of 07-08 Task 3 | ✅ suite exists | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky.*

---

## Wave 0 Requirements

Each item is created by the task that first needs it (RED step of that task):

- [ ] FK-check fix in `db.rs` with tests in `tests/db_migration.rs`; the eight literal version asserts move to 12 (07-01)
- [ ] `agent-brain/src/journal/` module with `#[cfg(test)]` units: config, sanitize, collect, fts, state (07-02), render (07-03), writer, gitsync (07-05), digest, loop_ (07-07)
- [ ] `agent-brain/tests/support/journal.rs` fixture builder + `tests/journal_export.rs` (07-02)
- [ ] `tests/journal_render.rs` + `tests/fixtures/journal/golden/` (synthetic data only) (07-03)
- [ ] `tests/journal_sanitize.rs` (07-04), `tests/journal_gitsync.rs` (07-05), `tests/journal_tim.rs` + updated `setMyCommands` assertion in `tests/telegram_intake.rs` (07-06), `tests/journal_serve.rs` (07-07)
- [ ] `scripts/phase7-live-check.sh` (isolated live check + schema dry run on a copy, modelled on `phase6-live-check.sh`) (07-08)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Schema dry run on a copy of the real brain.db (645 orphan `events` rows on this machine) | JRN-07 | Reads the real data directory (via `cp -p` only) with serve stopped, at the user's chosen time | Stop serve, back up per README "Nâng cấp lên Phase 7 (lược đồ 12)", run `bash scripts/phase7-live-check.sh --schema-dry-run`; expect `Schema v12 dry run OK (từ v11)`, `fk_violations_before=N after=N`, `real_dir_unchanged=yes` |
| First real `journal export --no-push` diff against the demo, then a real push to the private repo | JRN-01, JRN-02, JRN-04 | Needs the real brain.db, the real remote and the user's eyes on the format | Install the binary at a serve restart (no task running); run `agent-brain journal export --no-push`, review `git -C ~/.local/share/agent-brain/journal diff`, then `agent-brain journal export` |
| Digest at the real 18:00 on the paired chat; `/tim` from the phone | JRN-05, JRN-06 | Real Telegram and real clock | Wait for 18:00 or set `journal.digest_time` a few minutes ahead; send `/tim <từ>` |
| Obsidian on Android: pull the vault, open `Today.md`, check the embed and links; a github.com link opens when logged in | JRN-02, JRN-05 | Needs a phone | Clone with HTTPS + PAT via obsidian-git, pull, open the notes |
| No write path from the vault back to brain.db | JRN-02 | Absence of a route | Confirm no route or command reads the vault |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
