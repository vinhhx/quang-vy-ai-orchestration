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

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 07-xx-xx | — | — | JRN-01 | — | Day file: groups, one callout per task per day, 4 attention categories, midnight boundary 00:00 +07 | golden-file | `cargo test ... --test journal_export golden_day` | ❌ W0 | ⬜ pending |
| 07-xx-xx | — | — | JRN-02 | — | All five file kinds, no weekly file, second export = identical bytes and no new commit | idempotency | `cargo test ... --test journal_export idempotent` | ❌ W0 | ⬜ pending |
| 07-xx-xx | — | — | JRN-03 | T-07 sanitizer | No path/URL/key/Markdown-injection in the vault; a planted secret blocks commit and push | unit + integration | `cargo test ... --lib journal::sanitize` ; `--test journal_export secret_blocks_commit` | ❌ W0 | ⬜ pending |
| 07-xx-xx | — | — | JRN-04 | T-07 git | Empty and non-empty remote, push failure keeps local commit, conflict aborts without overwrite, alert once after N | fake-git-remote | `cargo test ... --test journal_gitsync` ; `--lib journal::state` | ❌ W0 | ⬜ pending |
| 07-xx-xx | — | — | JRN-05 | — | Digest once per day, only after a successful push, percent-encoded github.com link | unit + fake Telegram | `cargo test ... --lib journal::digest` ; `--test journal_telegram digest_once` | ❌ W0 | ⬜ pending |
| 07-xx-xx | — | — | JRN-06 | T-07 fts | `/tim` folds diacritics, hostile queries never error, no plan/QA/diff text in results, strangers get no reply | unit + fake Telegram | `cargo test ... --lib journal::fts` ; `--test journal_telegram tim` | ❌ W0 | ⬜ pending |
| 07-xx-xx | — | — | JRN-07 | — | Config absent = feature off, ranges rejected, migration 12 on a v11 DB keeps data, a DB with pre-existing orphan events still upgrades | unit | `cargo test ... --test db_migration` ; `--lib config::tests::journal` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky. Rows are refined by the planner into real task IDs.*

---

## Wave 0 Requirements

- [ ] `agent-brain/src/journal/` module skeleton with `#[cfg(test)]` units (config, sanitize, state, digest, fts)
- [ ] `agent-brain/tests/support/journal.rs` fixture builder and `agent-brain/tests/fixtures/journal/golden/` (synthetic data only, no real task titles)
- [ ] `agent-brain/tests/journal_export.rs`, `journal_gitsync.rs`, `journal_telegram.rs`
- [ ] FK-check fix in `db.rs` with a test in `tests/db_migration.rs`; update the `SCHEMA_VERSION` assertion and the `setMyCommands` assertion
- [ ] `scripts/phase7-live-check.sh` (schema dry-run on a copy, modelled on `phase6-live-check.sh`)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| First real `journal export --no-push` diff against the demo, then a real push to the private repo | JRN-01, JRN-02, JRN-04 | Needs the real brain.db, the real remote and the user's eyes on the format | Run `agent-brain journal export --no-push`, review the diff in `~/.local/share/agent-brain/journal`, then export with push |
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
