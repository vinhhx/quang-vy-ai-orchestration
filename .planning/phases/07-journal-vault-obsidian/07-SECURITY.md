---
phase: "7"
slug: "journal-vault-obsidian"
status: verified
threats_open: 0
asvs_level: 1
created: "2026-11-10"
---

# Phase 7 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail. Audited 2026-11-10 by `gsd-security-auditor` against plans 07-01..07-09 (register authored at plan time). Plans 07-10..07-12 (review fixes) add rows 07-10-xx..07-12-xx that are verified at their own execution (see their SUMMARY files).

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| brain.db → vault files, digest, FTS index | every user-typed string passes the sanitizer; only allowlisted columns are read | task requests, titles, status |
| config.json / env → git argv | repo URL, branch and ssh settings reach git commands | URLs, branch names |
| vault clone ↔ remote | exporter commits and pushes; never rewrites history | generated Markdown |
| Telegram chat → `/tim` | only the paired chat and sender may query | search text, result lines |
| script → real data dir | dry run works on a copy only | brain.db copy |

---

## Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation | Status |
|-----------|----------|-----------|----------|-------------|------------|--------|
| T-07-01 | DoS | `db::open` with orphan rows | high | mitigate | violations counted before migrations; fail only if count grows (`db.rs:243-253`); tests `db_migration.rs` | closed |
| T-07-02 | Tampering | FK check weakened | medium | mitigate | after-count comparison kept; `migration_that_adds_fk_violation_fails` | closed |
| T-07-03 | Info disclosure | `journal_fts` content | low | transfer | to T-07-04 (rows built from sanitized text only) | closed |
| T-07-04 | Info disclosure | user text into vault | high | mitigate | `sanitize_text`, `scan_generated` gate before clone/write/commit (`mod.rs:145-175`) | closed |
| T-07-05 | Tampering | Markdown/Obsidian injection | high | mitigate | character allow-list; fixed prefixes; no title in frontmatter | closed |
| T-07-06 | Elevation of privilege | repo_url/branch into git argv | high | mitigate | scheme allow-list, charset checks, `GIT_ALLOW_PROTOCOL`, `--` | closed |
| T-07-07 | Spoofing | SSH host key and prompts | medium | mitigate | `BatchMode=yes`, `StrictHostKeyChecking=yes`, no terminal prompt | closed |
| T-07-08 | Tampering | project slug as file name | high | mitigate | `slug_ok`, fallback key, `check_rel` | closed |
| T-07-09 | Tampering | overwriting a human-edited file | high | mitigate | marker-only management; dirty check | closed |
| T-07-10 | DoS | CLI export racing serve loop | medium | mitigate | `flock` on `journal.lock`; `Busy` outcome | closed |
| T-07-11 | Tampering | user/global git config | medium | mitigate | fixed `-c` overrides, author via env | closed |
| T-07-12 | Info disclosure | attention derivation reading plan/QA text | high | mitigate | DB columns only (role, status, outcome, times) | closed |
| T-07-13 | Tampering | dangling wikilinks | medium | mitigate | links only to rendered files; test fails on dangling link | closed |
| T-07-14 | Tampering | user text in YAML frontmatter | medium | mitigate | frontmatter holds dates, counts, fixed tags only | closed |
| T-07-15 | Info disclosure | paths/URLs/hosts/keys/env names in requests | high | mitigate | sanitizer patterns before cut; end-to-end leak test | closed |
| T-07-16 | Tampering | HTML injection, pasted diffs/code | high | mitigate | cut at diff/fence/PEM; allow-list | closed |
| T-07-17 | Info disclosure | secret echoed by the gate report | high | mitigate | `SecretHit` holds path and rule only; path masked | closed |
| T-07-18 | DoS | regex cost on a 20,000-char request | low | accept | linear-time regex engine; request length capped at 20,000 | closed (accepted) |
| T-07-19 | Tampering | history rewrite | high | mitigate | no force, `--hard`, `--amend`, reset, clean, `-A` anywhere in `journal/` | closed |
| T-07-20 | Tampering | overwriting a pulled or local human change | high | mitigate | dirty stop, ff-only/rebase of own commits, abort on conflict | closed |
| T-07-21 | DoS | git/ssh hang | medium | mitigate | command timeout then kill tree; stale `index.lock` handling | closed |
| T-07-22 | Info disclosure | stderr/URL/path in Telegram alerts | high | mitigate | fixed texts; `GitFailure` carries a category only | closed |
| T-07-23 | DoS | alert spam | medium | mitigate | one alert per streak; `INSERT OR IGNORE` on unique dedupe key | closed |
| T-07-24 | Tampering | deleting user files or `.obsidian/` | high | mitigate | delete only regular, marked, managed files; dot-names skipped | closed |
| T-07-25 | Info disclosure | phone PAT under `.obsidian/` committed | high | mitigate | `.obsidian/` refused and git-ignored; explicit pathspecs | closed |
| T-07-26 | Tampering/DoS | FTS MATCH injection, huge queries | high | mitigate | folded, quoted tokens; caps on length and tokens | closed |
| T-07-27 | Elevation of privilege | `/tim` from an unpaired chat | high | mitigate | `authz` first (chat and sender) | closed |
| T-07-28 | Info disclosure | results beyond the vault | high | mitigate | rows from `SafeText` only; selects day/display only | closed |
| T-07-29 | Repudiation/DoS | double digest or alert | medium | mitigate | unique dedupe key; claim before outbox | closed |
| T-07-30 | Info disclosure/integrity | digest link before push | medium | mitigate | queued only after `Pushed`/`UpToDate`; late digest dropped | closed |
| T-07-31 | DoS | git blocking the async runtime | medium | mitigate | `spawn_blocking`; timeouts; lock | closed |
| T-07-32 | Info disclosure | message to an unpaired chat | high | mitigate | paired chat id only; fixed templates | closed |
| T-07-33 | Tampering/Info disclosure | dry run touching the real brain.db | high | mitigate | copy with `cp -p`; before/after snapshot of the real dir | closed |
| T-07-34 | Info disclosure | real names in docs and fixtures | medium | mitigate | synthetic names only | closed |
| T-07-09-01 | Info disclosure | staging a foreign file or `.obsidian/` | high | mitigate | `pending_paths`: generated + marker only; never `.obsidian/` | closed |
| T-07-09-02 | Tampering | broad staging (`add -A`) | high | mitigate | one `add` with `--pathspec-from-file`; no `-a` on commit | closed |
| T-07-09-03 | Repudiation | commit count vs content | low | mitigate | count taken after the union; tests compare with `git show` | closed |
| T-07-SC (plans 02-09, ×8) | Tampering | dependency installs | high | mitigate | `Cargo.toml`/`Cargo.lock` unchanged since the phase plans | closed |

*Status: open · closed · open — below high threshold (non-blocking)*
*Severity: critical > high > medium > low — only open threats at or above `high` count toward threats_open*

Unregistered flags noted by the auditor (none blocking, all folded into the review fixes): write through a symlinked managed directory (WR-11, plan 07-10), `/tim` shows more title text than the vault (WR-08, plan 07-12), slug containing `..` fails the export (WR-02, plan 07-10).

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-07-01 | T-07-18 | Regex cost on very long requests: the regex crate is linear-time and requests are capped at 20,000 characters at every entry point (dashboard, intake, triage), so worst-case cost is bounded | plan author (07-04), verified by the auditor | 2026-11-10 |

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-11-10 | 46 | 46 | 0 | gsd-security-auditor (sonnet), orchestrated by Claude |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-11-10
