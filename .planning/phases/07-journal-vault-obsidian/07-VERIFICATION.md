---
phase: 07-journal-vault-obsidian
verified: 2026-11-10T21:45:00Z
status: human_needed
score: 11/12 must-haves verified
covered_files:
  - ".planning/phases/07-journal-vault-obsidian/07-01-PLAN.md"
  - ".planning/phases/07-journal-vault-obsidian/07-01-SUMMARY.md"
  - ".planning/phases/07-journal-vault-obsidian/07-02-PLAN.md"
  - ".planning/phases/07-journal-vault-obsidian/07-02-SUMMARY.md"
  - ".planning/phases/07-journal-vault-obsidian/07-03-PLAN.md"
  - ".planning/phases/07-journal-vault-obsidian/07-03-SUMMARY.md"
  - ".planning/phases/07-journal-vault-obsidian/07-04-PLAN.md"
  - ".planning/phases/07-journal-vault-obsidian/07-04-SUMMARY.md"
  - ".planning/phases/07-journal-vault-obsidian/07-05-PLAN.md"
  - ".planning/phases/07-journal-vault-obsidian/07-05-SUMMARY.md"
  - ".planning/phases/07-journal-vault-obsidian/07-06-PLAN.md"
  - ".planning/phases/07-journal-vault-obsidian/07-06-SUMMARY.md"
  - ".planning/phases/07-journal-vault-obsidian/07-07-PLAN.md"
  - ".planning/phases/07-journal-vault-obsidian/07-07-SUMMARY.md"
  - ".planning/phases/07-journal-vault-obsidian/07-08-PLAN.md"
  - ".planning/phases/07-journal-vault-obsidian/07-08-SUMMARY.md"
  - ".planning/phases/07-journal-vault-obsidian/07-09-PLAN.md"
  - ".planning/phases/07-journal-vault-obsidian/07-09-SUMMARY.md"
  - "agent-brain/README.md"
  - "agent-brain/src/db.rs"
  - "agent-brain/src/journal/collect.rs"
  - "agent-brain/src/journal/config.rs"
  - "agent-brain/src/journal/digest.rs"
  - "agent-brain/src/journal/fts.rs"
  - "agent-brain/src/journal/gitsync.rs"
  - "agent-brain/src/journal/loop_.rs"
  - "agent-brain/src/journal/mod.rs"
  - "agent-brain/src/journal/model.rs"
  - "agent-brain/src/journal/render.rs"
  - "agent-brain/src/journal/sanitize.rs"
  - "agent-brain/src/journal/state.rs"
  - "agent-brain/src/journal/writer.rs"
  - "agent-brain/src/telegram/detect.rs"
  - "agent-brain/src/telegram/journal_cmd.rs"
  - "scripts/phase7-live-check.sh"
covered_digest: "v3:sha256:22a055129989402b58714bd567f165eb7627e49ea52bfc9b95acbb6d8fbacc58"
behavior_unverified: 0
overrides_applied: 0
human_verification:
  - test: "Schema dry run on a copy of the real brain.db (about 645 orphan `events` rows on this machine): stop serve, back up per README section 'Nâng cấp lên Phase 7 (lược đồ 12)', run `bash scripts/phase7-live-check.sh --schema-dry-run`"
    expected: "`Schema v12 dry run OK (từ v11)`, `fk_violations_before=N after=N`, `real_dir_unchanged=yes`"
    why_human: "Reads the real data directory and needs serve stopped at the user's chosen time; the automated live check only proves the same path on a synthetic DB with 2 orphan rows"
  - test: "Install the binary at a serve restart (no task running), set `journal.repo_url` to the private vault repo, run `agent-brain journal export --no-push`, review `git -C ~/.local/share/agent-brain/journal status` and `diff` against the demo format (vault commit 203c7fe), then run `agent-brain journal export` on the SAME clone, then run it a third time"
    expected: "Day files under `Nhật ký/YYYY/MM/`, `Today.md`, month, year and project files; demo files replaced, `.obsidian/` and `master` untouched; exactly one commit by `agent-brain journal` on `main`; the third run makes no commit; group label `Hỗ trợ bộ phận khác` replaces the demo's longer label"
    why_human: "Needs the real brain.db, the real private remote and the user's eyes on the D-17 format. The vault clone is not present on this machine, so byte-level parity with demo commit 203c7fe could not be compared here; the format was checked against the D-17 text and the synthetic golden files only"
  - test: "Set `journal.enabled: true` (and `journal.digest_time` a few minutes ahead if desired), restart serve, wait for the digest, then send `/tim <từ>` from the paired phone chat and one `/tim` from a different chat if available"
    expected: "Exactly one digest in the paired chat with counts per group, the `Cần anh xử lý` list and a github.com link that opens the day file when logged in to GitHub; `/tim` returns dates and links and nothing from plans or QA reports; no reply to the other chat"
    why_human: "Real Telegram bot, real clock at 18:00, real GitHub login for the link"
  - test: "On the Android phone: Obsidian + obsidian-git, HTTPS + fine-grained read-only token, pull only; open `Today.md`, tap the project and day links; pull again after a later export"
    expected: "`Today.md` shows today's file embedded; links open existing notes (no empty notes created); pulls keep working after later exports (no history rewrite)"
    why_human: "Needs a phone and a GitHub token (ROADMAP success criterion 6, first half)"
  - test: "Judgment-tier prohibitions in the PLAN frontmatter carry `status: unverified, flagged: true` (07-01, 07-02, 07-03, 07-04, 07-05, 07-06, 07-07, 07-08, 07-09). This verifier read the code for each (see 'Prohibitions' below) and found no violation, but the verdict is non-authoritative"
    expected: "A human accepts or rejects each flagged prohibition (`unverified-prohibition — human review recommended`)"
    why_human: "ADR-550 D4: judgment-tier prohibitions are never silently passed in a verification report"
---

# Phase 7: Journal Vault (Obsidian) Verification Report

**Phase Goal:** Export the daily work journal from brain.db (source of truth, one way) to a private Obsidian vault repo (`git@github.com:vinhhx/agent-brain-journal.git`); phone read-only; one file per day plus month/year files, `Today.md` and `Dự án` pages (no weekly file); plain-language content in three groups with a "cần anh xử lý" section first; allowlisted fields only, secret scan before push; Telegram end-of-day digest (sent only after a successful push) and `/tim` search via SQLite FTS5; commit and push in batches (about every 30 minutes and at end of day); safe git behaviour (never force, never overwrite human files).
**Verified:** 2026-11-10T21:45:00Z
**Status:** human_needed
**Re-verification:** No, initial verification (no earlier VERIFICATION.md existed)

## Goal Achievement

Verdict: every automated must-have is backed by code that exists, is substantive, is wired, and by tests that exercise the behavior; I re-ran the relevant test binaries and the isolated live check myself. What remains is the manual-only set recorded in 07-VALIDATION.md and 07-08-PLAN.md (real brain.db, real private remote, real Telegram at 18:00, the phone). No must-have failed, so there are no gaps.

### Observable Truths

ROADMAP success criteria (the contract) first, then the locked decisions that the orchestrator asked to be checked.

| #  | Truth | Status | Evidence |
| -- | ----- | ------ | -------- |
| SC1 | `journal export` creates `Nhật ký/YYYY/MM/YYYY-MM-DD.md` (Asia/Ho_Chi_Minh day), `Today.md`, month, year and `Dự án/<project>.md`; a second run on the same data makes no commit; no weekly file | ✓ VERIFIED | `journal/mod.rs::cmd_export` → `run_export_once` → collect → `render::render_all` (render.rs:366-406 emits only day, month, year, project, Today; no weekly path exists in model.rs/render.rs) → `writer::apply` → `gitsync::commit_and_push`. Day bounds are local midnight via `collect::day_bounds_ms`/`local_day`, with a fixed-offset override `utc_offset_minutes` (test `day_boundary_is_local_midnight`). `tests/journal_export.rs::tracer_export_pushes_one_day_file`, `second_export_makes_no_commit`; `tests/journal_render.rs::no_weekly_file_and_nfc_paths`, `render_is_deterministic`, `golden_files_match` (10/10 pass on my run). Live check A-G prints `Phase 7 live check OK` (my run). "On the real brain.db" is a manual item |
| SC2 | Day file has three groups by `journal_group` (undeclared project goes to Công việc), one entry per task per day, and a `cần anh xử lý` section first listing all four categories | ✓ VERIFIED | `config::journal_group_of` reads `projects_map.json` leniently, unknown or missing id falls back to the first group (D-08, D-19; no field added to `Project`). `collect::collect` builds one `TaskDay` per (task, active day). `collect::classify_as_of` covers plan waiting (`PlanWaiting`, via `plan_approvals`), quota and env down (`Quota`, `EnvDown`), QA fix and blocked (`QaFix`, `Blocked`), failed, timeout and protocol_violation. `render_day` writes `## Cần anh xử lý (N)` directly under the heading, before the group sections; carry-over of unresolved older items into today is implemented (`carry`). Golden day file 2026-01-15 shows the six items in category order. Tests `attention_by_day`, `killed_and_approved_are_not_attention`, `groups_from_journal_group_only`, `day_file_has_attention_first` pass |
| SC3 | A task whose request holds a diff, path, internal URL and key-like string leaves nothing in the vault; the secret scanner blocks commit and push on a planted secret and tells the user | ✓ VERIFIED | `sanitize::sanitize_text`: NFC, strip invisible chars, cut at the first diff/code fence/PEM, redact known secret values, then replace URLs, e-mail, `key=value`, Bearer, token shapes, IPs, hosts, Windows/home/absolute/relative paths, env-style names, long blobs, then keep only letters, digits and `ALLOWED_PUNCT`, then cut to `title_max_chars`. The cut happens after redaction. Final gate `sanitize::scan_generated` (known values plus url, abs-path, pem, token-shape, jwt, telegram-token, key-assignment, email) runs in `export_with_tz` before `gitsync::prepare` and before any write; a hit returns `SecretBlocked` so nothing is written, committed or pushed, and `state::record_outcome` queues a `secret` mail carrying only a masked relative path and a rule name. `tests/journal_sanitize.rs::hostile_request_leaves_nothing_in_vault` and `planted_secret_blocks_export` pass on my run (the second asserts the value is absent from stdout/stderr). Collection reads only `tasks(id, project_id, slug, title, status, approved_at)`, `plan_approvals`, `proposals.title` and run rows (role/status/outcome/times); no plan, QA, run-dir or diff read |
| SC4 | In `serve`, the loop pushes about every 30 minutes and at end of day; offline or SSH refusal keeps a local commit and retries; after N consecutive failures a Telegram message; a pull conflict stops and reports without overwriting | ✓ VERIFIED | `dashboard/mod.rs:272` spawns `journal::loop_::spawn_loop`; `loop_::export_due` fires on first run, after `interval_minutes` (default 30, config 1-1440), on day change and once the digest moment is reached; off unless `journal.enabled` and `repo_url` (test `loop_disabled_by_default`). `gitsync::commit_and_push` commits locally first, then pushes `HEAD:refs/heads/<branch>` with no force; a failed fetch writes a `.git/agent-brain-journal-fetch-failed` marker so an offline cycle with nothing new still counts as a failure. `state::on_push_failed` counts the streak and queues one `pushfail` mail at `push_fail_notify_after` (default 3), one `recovered` mail afterwards. `gitsync::integrate`: ff-only when behind, `rebase` of own unpushed commits when diverged, `rebase --abort` and `GitStop::Conflict` on conflict; a dirty tracked non-generated file gives `GitStop::Dirty`. Tests `offline_commit_then_catch_up`, `remote_moved_fast_forwards`, `diverged_history_rebases_local_commits`, `conflict_aborts_without_overwrite`, `dirty_clone_stops_without_overwrite`, `rejected_push_retries_once_after_rebase` (16/16 `journal_gitsync` pass on my run); `journal_serve` 7/7 pass |
| SC5 | At 18:00 (configurable) the paired chat gets exactly one digest (counts per group, attention list, github.com link); `/tim <từ khóa>` returns dates with links from allowlisted content and never plan, QA or diff | ✓ VERIFIED (against the fake Telegram) | `digest::digest_due/digest_text/digest_link`, default `digest_time` 18:00 (configurable `HH:MM`). `loop_::tick_with_tz` calls `state::enqueue_mail("digest", digest:<day>)` only after outcome `Pushed` or `UpToDate` (never on `PushFailed`, `Stopped`, `SecretBlocked`); `journal_mail.dedupe_key` is UNIQUE (`INSERT OR IGNORE`) so one digest per day, also across restarts. `telegram/detect.rs::JournalMail` claims a row (`sent_at`) before queueing it to the outbox and drops a `digest:<day>` whose day is not today (D-20, "bỏ digest … quá 24:00"). `/tim`: `intake.rs:727` dispatches to `journal_cmd::on_tim` (after `bot::authz`, strangers get no reply), registered in `setMyCommands` and `HELP_TEXT`; `fts::search` queries only `journal_fts` (`day`, `display`), folded, quoted tokens (no FTS operators), capped at 100 chars, 8 tokens, `search_max_results`. Index rows come from `rows_from_model` (already sanitized title, slug, group, day, status). Tests `digest_once_after_push`, `no_digest_before_digest_time`, `no_digest_when_push_fails`, `stale_digest_dropped`, `mail_delivered_once` (journal_serve 7/7 on my run); `journal_tim` (`tim_finds_by_folded_words`, `tim_never_returns_text_outside_the_vault`, `tim_hostile_queries_and_usage`, `tim_from_stranger_gets_no_reply`) was green in the orchestrator's full run. The real 18:00 digest and a real `/tim` are manual items |
| SC6 | Obsidian on the phone pulls the vault and shows `Today.md` with today's file embedded; no route or command writes back from the vault into brain.db | ? UNCERTAIN (human) for the phone half; ✓ VERIFIED for the no-write-back half | `render_today` writes `![[YYYY-MM-DD]]` when today has a file (tests `today_embeds_today`, `all_links_resolve`: every wikilink target exists in the output set), links to yesterday, month, year and projects. The clone is only ever written by `writer::apply` and `gitsync`; the vault is read back only for the marker line (`has_marker`, `file_has_marker`), never parsed into the DB. `grep clone_dir` shows no other consumer, the only dashboard reference to `journal` is the `spawn_loop` call, and no CLI subcommand other than `journal export` exists. `.gitignore` bootstrap ignores `.obsidian/` and `.trash/`. The phone pull itself needs a phone: human item 4 |
| D-21 | The FK check no longer blocks migrations because of old orphan rows; it fails only when violations increase; migration 12 exists; no backup or repair in `db::open` | ✓ VERIFIED | `db.rs:227-260` `apply_pending_migrations` counts `pragma_foreign_key_check` before and after, errors with `Nâng cấp CSDL thất bại: vi phạm khóa ngoại tăng từ {before} lên {after}…` only when `after > before`, otherwise warns; nothing is deleted, repaired or copied. `SCHEMA_VERSION = 12`; migration 12 creates `journal_state`, `journal_mail` (dedupe_key UNIQUE, `journal_mail_unsent` index) and FTS5 `journal_fts`. Tests: `db::tests::preexisting_fk_violations_do_not_block_migration`, `migration_that_adds_fk_violation_fails`, `db_migration::v10_db_with_orphan_events_upgrades`, `v11_db_with_orphan_events_upgrades_to_v12` exist; whole suite green per orchestrator. 07-01 was the first task of the phase (commit 2d64a81 precedes b8f9253 and all journal work). README backup step, dry run and rollback present (README lines 65-84) |
| D-20 | Digest only after a successful push; retry later in the day; dropped after 24:00 with a warning | ✓ VERIFIED | See SC5: enqueue only on `Pushed`/`UpToDate`; a failed push leaves the digest un-queued and `export_due` retries every interval; `tick_with_tz` warns once per dropped day (`dropped_warned`), `JournalMail::scan` drops a stale `digest:` row. `no_digest_when_push_fails`, `stale_digest_dropped` pass. `UpToDate` is only returned when HEAD equals `origin/<branch>` and no failed-fetch marker exists |
| D-22 | Only marker files are managed; marker-less files are never written, deleted or staged; `.obsidian/` ignored; only `main` is pushed | ✓ VERIFIED | `writer::apply`: skips (`foreign`) an existing file without the marker, deletes only stale files that carry the marker and sit under `Today.md`, `Nhật ký/`, `Dự án/`; `check_rel` rejects `.obsidian/`, `.git/`, `..`, non-NFC and unmanaged paths; `.gitignore` created only when absent. `gitsync::pending_paths` (07-09) stages untracked or modified generated files only when they have the marker, never `.obsidian/`, and `.gitignore` only when untracked and byte-identical to `GITIGNORE_BOOTSTRAP`. Push is `HEAD:refs/heads/<journal.branch>` (default `main`). Tests `human_file_at_generated_path_is_not_overwritten`, `nonempty_remote_keeps_human_files`, `pending_never_stages_foreign_or_obsidian`, `no_push_stages_nothing` pass |
| 07-09 | `--no-push` then push on the same clone commits every pending generated file in one commit; third run makes none; deletions from an earlier `--no-push` are committed | ✓ VERIFIED | `mod.rs:178-186` extends the path list with `pending_paths` before `commit_and_push`, and `written.changed` (so the CLI count and commit message equal the staged files). Tests `no_push_then_push_commits_pending_files`, `no_push_then_push_is_idempotent`, `no_push_then_push_on_non_empty_vault`, `no_push_removed_stale_then_push_commits_deletion` pass on my run. README first-run step 2 now says "on the same clone" and no `rm -rf` workaround remains; live check A runs `--no-push` then B pushes from the same clone |
| D-17 | Output keeps the demo format: frontmatter (`date`, `tasks`, `tags`), generated-file marker, day heading, `## <group> (<n>)`, foldable callout `> [!success]- ✓ <project> — <title>` with status and duration, project/group/steps line, cut request paragraph, footer, `Today.md` embed, month table plus `## Theo dự án`, project page by month | ✓ VERIFIED against the D-17 text and golden files; demo parity is a human item | `render.rs` `render_day`/`render_callout`/`render_month`/`render_project`/`render_today` and golden `Nhật ký/2026/01/2026-01-15.md`, `Today.md`. Non-done tasks use `warning`/`failure`/`info`/`note` callout types (planner's discretion per D-17). The reference demo (vault commit 203c7fe) is not available on this machine (`~/.local/share/agent-brain/journal` does not exist), so a byte-level format comparison is part of human item 2 |
| Safe git | Never force, never rewrite pushed history, no shared git helpers, no model call | ✓ VERIFIED | `grep` over `src/journal/*.rs` finds no `--force`, `-f`, `reset --hard`, `--amend`, `stash`, `clean`, `git add -A`; the journal has its own runner (`gitsync::run_git`: timeout, process-group kill, fixed identity, `commit.gpgsign=false`, `core.hooksPath=/dev/null`, `GIT_TERMINAL_PROMPT=0`, `BatchMode=yes`) and no `gitops` import; no engine, HTTP or model client is referenced in `src/journal`. `Cargo.toml` and `Cargo.lock` are unchanged since the phase base |

**Score:** 11/12 truths verified (the one open truth, SC6, is verified for the no-write-back half and needs a phone for the other half); 0 present-but-behavior-unverified.

### Deferred Items

None. No gap matched a later phase.

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `agent-brain/src/db.rs` | FK-tolerant migrations, migration 12, `SCHEMA_VERSION = 12` | ✓ VERIFIED | lines 193-260 |
| `agent-brain/src/journal/{mod,config,model,collect,render,sanitize,writer,gitsync,state,fts,digest,loop_}.rs` | Exporter module | ✓ VERIFIED | 12 files, 4,900 lines in total, no stubs, no debt markers (grep clean) |
| `agent-brain/src/telegram/journal_cmd.rs` + `telegram/detect.rs::JournalMail` | `/tim` and mailbox delivery | ✓ VERIFIED | wired in `intake.rs:727`, `telegram/mod.rs:239` (`setMyCommands`), `detect.rs:632,672` |
| `agent-brain/src/main.rs`, `config.rs`, `lib.rs` | `journal export [--no-push]` CLI, `journal` config section | ✓ VERIFIED | `main.rs:177,303,512`; `config.rs:1572` (`journal::config::parse`) |
| `agent-brain/tests/journal_{export,render,sanitize,gitsync,tim,serve}.rs`, `tests/support/journal.rs`, `tests/fixtures/journal/golden/` | Automated verification, synthetic data | ✓ VERIFIED | 6 integration test files, 49 integration tests; golden fixtures use `du-an-a..d` and generic titles |
| `scripts/phase7-live-check.sh` | Isolated live check and schema dry run on a copy | ✓ VERIFIED | my run printed `fk_violations_before=2 after=2`, `real_dir_unchanged=yes`, `Phase 7 live check OK` |
| `agent-brain/README.md` | Upgrade section, CLI row, config table, `/tim` and digest | ✓ VERIFIED | lines 65-84 (upgrade), 109 (CLI row), 451-512 (journal config and first run), 895-901 (Telegram); `git diff --numstat e46ed65 HEAD` shows 103 added, 0 deleted |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | -- | --- | ------ | ------- |
| `main.rs` `Commands::Journal` | `journal::cmd_export` | clap subcommand | WIRED | main.rs:512-513 |
| `dashboard::cmd_serve` | `journal::loop_::spawn_loop` | background task | WIRED | dashboard/mod.rs:272 |
| `loop_::tick` | `run_export_once` → `state::record_outcome` | export, then record | WIRED | loop_.rs:107, mod.rs:224 |
| `loop_::tick_with_tz` | `journal_mail` (digest) | `enqueue_mail` only on `Pushed`/`UpToDate` | WIRED | loop_.rs:108-120 |
| `telegram::detect::run` | `JournalMail::scan` → outbox | startup and every scan | WIRED | detect.rs:632, 672 |
| `intake::on_command("tim")` | `journal_cmd::on_tim` → `fts::search` | after `bot::authz` | WIRED | intake.rs:727 |
| `export_with_tz` | `gitsync::pending_paths` | extends staged list before `commit_and_push` (07-09 key link) | WIRED | mod.rs:180 |
| `export_with_tz` | `sanitize::scan_generated` | gate before prepare/write/commit | WIRED | mod.rs:145 |
| `export_with_tz` | `fts::rebuild` | index rebuilt after each write | WIRED | mod.rs:175 |
| README step 3 | `scripts/phase7-live-check.sh --schema-dry-run` | documented command | WIRED | README line 75 |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| Day/Today/month/year/project files | `Model.days`, `Model.attention` | `collect::collect`: `tasks`, `runs_for_task`, `plan_approvals`, `proposals` in one read transaction | Yes (golden and e2e tests assert the values) | ✓ FLOWING |
| Digest text | `DaySummary` | `model::summarize(&model, today)` from the same export | Yes (`digest_once_after_push` asserts text and link) | ✓ FLOWING |
| `/tim` replies | `journal_fts` rows | `fts::rebuild(rows_from_model)` on every export | Yes (`export_rebuilds_fts_index`, `tim_finds_by_folded_words`) | ✓ FLOWING |
| Push-failure alerts | `journal_state` streak | `state::record_outcome` from the real outcome | Yes (`journal_gitsync`, `secret_blocked_export_mails_user`) | ✓ FLOWING |

### Behavioral Spot-Checks

I did not run `cargo test` or `cargo build` (a full run was in progress). I ran pre-built test binaries compiled after the final commit (21:24, HEAD 21:22) directly, plus the isolated live-check script.

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Git safety, offline catch-up, conflict, no-push-then-push | `target/debug/deps/journal_gitsync-80469359893e4eb0 --test-threads=2` | 16 passed, 0 failed | ✓ PASS |
| Hostile request and secret gate | `journal_sanitize-7b6189dc342e991c --test-threads=1` | 2 passed | ✓ PASS |
| Golden files, attention first, links, determinism, no weekly file | `journal_render-82bdea618271cff5 --test-threads=1` | 10 passed | ✓ PASS |
| Serve loop, digest once after push, none when push fails, stale digest dropped, mail once | `journal_serve-1a7ef98eb0775cf0 --test-threads=1` | 7 passed | ✓ PASS |
| Isolated CLI live check and schema dry run on a copy | `bash scripts/phase7-live-check.sh` | `Phase 7 live check OK` | ✓ PASS |
| Whole suite | orchestrator, after the last merge | 1858 passed, 0 failed (not re-run by me) | ✓ PASS (reported) |

### Probe Execution

The phase declares no `scripts/*/tests/probe-*.sh`. `scripts/phase7-live-check.sh` is its probe equivalent and is covered above.

### Requirements Coverage

Requirement IDs found in PLAN frontmatter: 07-01 JRN-07; 07-02 JRN-01..04, JRN-07; 07-03 JRN-01, JRN-02; 07-04 JRN-03; 07-05 JRN-02..04; 07-06 JRN-06; 07-07 JRN-04, JRN-05; 07-08 JRN-01..07; 07-09 JRN-02, JRN-04. The union is exactly JRN-01..JRN-07, matching the ROADMAP Phase 7 line and the seven rows in REQUIREMENTS.md. No orphaned requirement.

| Requirement | Source Plan(s) | Description | Status | Evidence |
| ----------- | -------------- | ----------- | ------ | -------- |
| JRN-01 | 07-02, 07-03, 07-08 | Day file, local day, one entry per task per day, template text, three groups, attention first with four categories | ✓ SATISFIED | SC1, SC2 |
| JRN-02 | 07-02, 07-03, 07-05, 07-08, 07-09 | Today, month, year, project pages; no weekly file; rebuildable and idempotent; one way | ✓ SATISFIED | SC1, SC6, D-22, 07-09 |
| JRN-03 | 07-02, 07-04, 07-05, 07-08 | Allowlist, cut to about 140 chars, path/URL blocked, secret scan before commit, no push on hit, user told | ✓ SATISFIED | SC3 (`title_max_chars` default 140, `callout_title_chars` 60) |
| JRN-04 | 07-02, 07-05, 07-07, 07-08, 07-09 | Clone at `~/.local/share/agent-brain/journal` (default `data_dir()/journal`), author `agent-brain journal`, serve loop plus manual command, failure policy, conflict stops | ✓ SATISFIED | SC4 |
| JRN-05 | 07-07, 07-08 | Digest at configurable time, once a day, groups, attention, github.com link, paired chat only | ✓ SATISFIED (automated) | SC5; real 18:00 is manual |
| JRN-06 | 07-06, 07-08 | `/tim` over FTS5, allowlisted content only, date plus link, paired chat only | ✓ SATISFIED (automated) | SC5; real chat is manual |
| JRN-07 | 07-01, 07-02, 07-08 | Everything configurable, migration with backup step, no binary swap while tasks run | ✓ SATISFIED | `journal::config::parse` validates every key with a range (repo_url, branch, clone_dir, interval_minutes, digest_time, push_fail_notify_after, title lengths, git timeouts, groups, offsets, web_base_url, search_max_results); `journal_group` read from `projects_map.json`; README upgrade section carries the backup and "do not replace the binary while tasks run" rules |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| (journal module, tests, script) | - | `TBD`, `FIXME`, `XXX`, `TODO`, `HACK`, `todo!`, `unimplemented!` | none found | grep over every file touched by the phase is clean |
| `journal/sanitize.rs` | 123 | The relative-path rule rewrites ordinary `a/b` text (dates such as `15/01/2026`, `và/hoặc`) to `(đường dẫn)` in titles | ℹ️ Info | Documented known limit (07-04 deviation 5); errs on the safe side, only affects wording of titles |
| `journal/render.rs` | 75-78 | When the title is derived from the request, the request paragraph repeats the title | ℹ️ Info | Cosmetic; matches the D-17 callout layout |
| `db.rs` | 249 | The FK guard compares totals (`after > before`), so a migration that fixes one violation and adds another would pass | ℹ️ Info | Accepted by D-21 ("only when the count grows"); no such migration exists |
| whole repo | - | `cargo fmt --check` not clean (about 100 pre-existing files); new journal files are `rustfmt` clean per file | ℹ️ Info | Accepted deviation recorded in 07-01..07-08; not a phase failure |
| `07-02-SUMMARY`, `07-VALIDATION.md` | - | `!` is not in `ALLOWED_PUNCT`, so `!` is dropped from titles; the generated warning callout still uses `!` as an icon | ℹ️ Info | Accepted deviation (stricter than the plan's action text, follows the must-have truth) |

### Prohibitions (ADR-550 D3/D4, judgment tier, non-authoritative)

Flagged `unverified-prohibition — human review recommended`. My code-reading verdict for each is "not violated":

- 07-01: FK check not weakened (db.rs:243-253); no delete or repair of orphans in `db::open`, no backup copy in `open()`.
- 07-02: exporter reads only allowlisted columns and `projects_map.json` (collect.rs header and queries); no model or engine call in `src/journal`; marker-less files are never overwritten (`writer::apply` → `foreign`); no code path reads the vault back into the DB.
- 07-03: attention is derived from `runs`, `tasks`, `plan_approvals` only; no plan file or QA report is read.
- 07-04: gate hits and mails carry masked relative paths and rule names (`mask_path`, `stop_mail`), asserted by `planted_secret_blocks_export`.
- 07-05: no force-push, hard reset, amend or rebase of pushed commits (`integrate` rebases only unpushed own commits); nothing touches `.obsidian/`; marker-less files are never deleted; mails hold categories, rule names and vault-relative paths only.
- 07-06: `/tim` queries only `journal_fts`.
- 07-07: digest queued only after `Pushed`/`UpToDate`; mailbox goes through the paired-chat outbox.
- 07-08: the script copies the real DB only with `cp -p` and opens the copy; README and fixtures use synthetic names (grep for real project and owner names in README and script is clean).
- 07-09: no `git add -A`, no force or history rewrite; staging is explicit `--pathspec-from-file`; marker-less and `.obsidian/` paths excluded (`pending_never_stages_foreign_or_obsidian`).

### Human Verification Required

#### 1. Schema dry run on the real brain.db copy

**Test:** Stop serve, back up per README "Nâng cấp lên Phase 7 (lược đồ 12)", run `bash scripts/phase7-live-check.sh --schema-dry-run`.
**Expected:** `Schema v12 dry run OK (từ v11)`, `fk_violations_before=N after=N` (N about 645), `real_dir_unchanged=yes`.
**Why human:** Touches the real data directory at a time of the user's choosing.

#### 2. First real export, demo diff and push

**Test:** Install the binary at a serve restart with no task running, set `journal.repo_url`, run `agent-brain journal export --no-push`, review `git status` and `git diff` against demo commit 203c7fe, then `agent-brain journal export` on the same clone, then once more.
**Expected:** Five file kinds, demo files replaced, `.obsidian/` and `master` untouched, one commit by `agent-brain journal` on `main`, third run makes no commit, group label shown as `Hỗ trợ bộ phận khác`.
**Why human:** Real data, real remote, and the visual format comparison with the demo (the vault clone is absent here).

#### 3. Digest at 18:00 and `/tim` on the real bot

**Test:** Enable the loop, wait for (or move) `digest_time`, send `/tim <từ>` from the paired chat and, if possible, from another chat.
**Expected:** One digest with group counts, the `Cần anh xử lý` list and a working github.com link; `/tim` returns dates and links only; the other chat gets no reply.
**Why human:** Real Telegram, real clock, real GitHub login.

#### 4. Obsidian on Android

**Test:** obsidian-git over HTTPS with a read-only token, pull, open `Today.md`, tap links, pull again after a later export.
**Expected:** `Today.md` embeds today's file, no empty notes are created, pulls keep working.
**Why human:** Needs a phone.

#### 5. Flagged judgment-tier prohibitions

**Test:** Skim the "Prohibitions" list above.
**Expected:** Accept or reject each flagged item.
**Why human:** Non-authoritative verifier verdicts must not pass silently.

### Bookkeeping notes for the orchestrator (not goal failures)

- `.planning/ROADMAP.md` still shows "7/8 plans executed", 07-08 unchecked, and no line for gap-closure plan 07-09; `.planning/REQUIREMENTS.md` still marks JRN-01..07 as Pending. Update both after the human items are accepted.
- `07-VALIDATION.md` still says `status: draft`, `nyquist_compliant: false`, all rows pending; this verification supplies the evidence needed to flip it.
- `07-09-SUMMARY.md` and `07-08-SUMMARY.md` were checked: the defect found in 07-08 (no-push then push) is closed by 07-09 with the code, tests, README and live check all changed, so the earlier deviation no longer applies.

### Gaps Summary

No gaps. All six ROADMAP success criteria and the locked decisions D-17, D-20, D-21 and D-22 are backed by wired, substantive code and by passing tests that I re-ran (journal_gitsync 16/16, journal_sanitize 2/2, journal_render 10/10, journal_serve 7/7, live check OK; full suite 1858/0 per the orchestrator). The phase stays at `human_needed` only because the manual-only checks (real brain.db migration, real push and demo diff, real 18:00 digest and `/tim`, phone pull) are not done and cannot be done by an automated verifier.

---

_Verified: 2026-11-10T21:45:00Z_
_Verifier: Claude (gsd-verifier)_
