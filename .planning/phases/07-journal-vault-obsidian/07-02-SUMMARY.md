---
phase: 07-journal-vault-obsidian
plan: 02
subsystem: journal
tags: [rust, git, obsidian, fts5, sqlite, config, tracer]
requires:
  - "07-01: migration 12 (journal_state, journal_mail, journal_fts with display column), SCHEMA_VERSION 12"
provides:
  - "agent_brain::journal module with every public contract listed in 07-02-PLAN 'New contracts'"
  - "CLI agent-brain journal export [--no-push]"
  - "Config.journal (config.json section journal; absent = off), per-project journal_group in projects_map.json"
  - "journal.lock exclusivity, journal_state export keys, journal_fts rebuild per export"
affects: [07-03, 07-04, 07-05, 07-06, 07-07, 07-08]
tech-stack:
  added: []
  patterns:
    - "own git runner with wall-clock timeout + procs::kill_tree, fixed author via env, -c gpgsign/hooks/autocrlf overrides"
    - "SafeText newtype: render only accepts sanitized text"
    - "collect in one read transaction; FTS rebuild in one IMMEDIATE transaction"
key-files:
  created:
    - agent-brain/src/journal/mod.rs
    - agent-brain/src/journal/config.rs
    - agent-brain/src/journal/model.rs
    - agent-brain/src/journal/collect.rs
    - agent-brain/src/journal/render.rs
    - agent-brain/src/journal/sanitize.rs
    - agent-brain/src/journal/writer.rs
    - agent-brain/src/journal/gitsync.rs
    - agent-brain/src/journal/state.rs
    - agent-brain/src/journal/fts.rs
    - agent-brain/tests/support/journal.rs
    - agent-brain/tests/journal_export.rs
  modified:
    - agent-brain/src/lib.rs
    - agent-brain/src/config.rs
    - agent-brain/src/main.rs
key-decisions:
  - "ALLOWED_PUNCT excludes '!' (must-have truth lists '!' among removed characters; the action text listed it as kept)"
  - "--no-push writes files into the clone and stages nothing (LocalOnly), so nothing is left staged"
  - "Local git failures in commit_and_push (add/commit) surface as CommittedNotPushed(Other) -> ExportOutcome::PushFailed, since Published has no other error variant"
  - "Model.project_slugs = file keys of projects with at least one day entry, sorted"
requirements-completed: [JRN-01, JRN-02, JRN-03, JRN-04, JRN-07]
status: complete
commits: 3
plan_head_before: c10969387c9342516df2e195d2c3135efa1ada49
plan_head_after: f04c52573aa494bf00deafebe7f75a7ac1ed1a05
actuals:
  tokens: 28300
  tasks: 3
  commits: 3
duration: ~27min
completed: 2026-11-10
---

# Phase 7 Plan 02: Journal export tracer Summary

**`agent-brain journal export` now reads brain.db in one read transaction, renders day files in the D-17 demo format (frontmatter, marker, foldable callouts per task), blocks on known secret values, writes them with marker ownership and temp+rename, and commits/pushes them as `agent-brain journal` through its own timeout-bounded git runner; exports are exclusive (`journal.lock`), recorded in `journal_state`, indexed into `journal_fts`, and the `journal` config section is fully validated with github.com link helpers.**

## Tasks

| Task | Name | Commit |
| ---- | ---- | ------ |
| 1 (tracer) | `journal export` renders a day file from brain.db and pushes it to a bare remote | f79de9f |
| 2 | Exports are exclusive, recorded in journal_state, indexed into journal_fts | f36e87f |
| 3 | journal config fully validated, journal_group fallback, github.com link helpers | f04c525 |

## What changed

- `src/journal/mod.rs`: `NOW_ENV`, `now_ms`, `LOCK_FILE`, `JournalLock` + `acquire_lock` (flock `LOCK_EX|LOCK_NB`, dir 0700, file 0600), `ExportParams`, `ExportOutcome`, `ExportReport`, `run_export_once` (lock → collect → render → `scan_generated` gate → `prepare` → `writer::apply` → `fts::rebuild` → `commit_and_push` → `state::record_outcome`), `known_secrets`, `cmd_export` (Vietnamese one-line results; PushFailed/Stopped/SecretBlocked exit 1 with category or `{path} ({rule})`, never values or raw stderr).
- `src/journal/config.rs`: `GroupDef`, `JournalTz`, `JournalConfig` (+ `Default`, `tz()`, `default_group()`), `parse` (typed keys, ranges, strict `HH:MM`, repo_url scheme allow-list, branch charset, groups 1-10 with id/label rules, author, `web_base_url` https), lenient `journal_group_of` (one `term::emit` warning per (project, id) per process), `web_base`, `blob_url`, `percent_encode_path`.
- `src/journal/model.rs`: `MARKER`, `JOURNAL_DIR`, `PROJECTS_DIR`, `TODAY_PATH`, path fns, `weekday_vi`, `ProjectRef`, `AttentionKind` (`label`, `category`), `StatusKind`, `DayStatus`, `TaskDay`, `AttentionItem`, `Model`, `DaySummary`, `summarize`.
- `src/journal/collect.rs`: `collect`, `day_bounds_ms`, `local_day`, `project_file_key`, `ROLE_LABELS`. Reads only `projects` (via `list_project_display_map`), `tasks(id, project_id, slug, title, status, approved_at)`, latest `proposals.title`, and runs via `db::runs_for_task`; status of the day's last run through `dashboard::taskstate::step_state`; rounds through `effective_rounds`.
- `src/journal/render.rs`: `render_all` (day files only), `fmt_duration`.
- `src/journal/sanitize.rs`: `SafeText` (+ `cut`), `sanitize_text`, `cut_chars`, `ALLOWED_PUNCT`, `SecretHit`, `scan_generated`.
- `src/journal/writer.rs`: `WriteReport`, `apply`, `is_managed_path`, `has_marker`; path checks (empty, absolute, `..`, backslash, NUL, `.obsidian/`, `.git/`, non-NFC, non-managed) before the first write.
- `src/journal/gitsync.rs`: `GitCtx`, `GitOut`, `run_git` (+ `run_git_input`), `FailCategory`, `classify`, `GitFailure`, `GitStop`, `Prepared`, `prepare` (clone `--no-checkout -- <url> <dir>`, `checkout -B` or `symbolic-ref` for an empty remote, NotARepo for a non-empty non-repo dir), `Published`, `commit_and_push` (`add --pathspec-from-file=- --pathspec-file-nul`, `diff --cached --quiet`, `push origin HEAD:refs/heads/<branch>`).
- `src/journal/state.rs`: key consts, `get`, `set`, `enqueue_mail`, `record_outcome`.
- `src/journal/fts.rs`: `FtsRow`, `fold`, `rows_from_model`, `rebuild`.
- `src/lib.rs`: `pub mod journal;`. `src/config.rs`: 4 lines (doc, field, parse line, literal field). `src/main.rs`: `Commands::Journal`, `JournalCommands::Export { no_push }`, dispatch arm.
- Tests: `tests/support/journal.rs` (bare_remote, local_ms, journal_section, setup_projects, seed_task, seed_proposal, seed_run, export, clone_dir, checkout, remote_commits); `tests/journal_export.rs` (5 tests); unit tests in `journal::{tests, collect, config, fts, gitsync, model, render, sanitize, state, writer}`.

Rendered tracer day file (from the test, verbatim):

```text
---
date: 2026-01-15
tasks: 1
tags: [nhat-ky]
---
<!-- Tệp sinh tự động từ agent-brain — không sửa tay, lần xuất sau sẽ ghi đè. -->
# Thứ Năm 15/01/2026 — 1 việc (1 xong)

## Video (1)
> [!success]- ✓ du-an-a — Dựng video giới thiệu sản phẩm mẫu
> **Trạng thái:** xong · 34 phút
> **Dự án:** [[Dự án/du-an-a|du-an-a]] · **Nhóm:** Video · **Số bước chạy:** 3 · **Vòng:** 1
> **Vai đã chạy:** Lead, Dev, QA
>
> Dựng video giới thiệu sản phẩm mẫu thêm phụ đề tiếng Việt cho từng cảnh

---
Tháng: [[2026-01]]
```

## Verification (real output)

- `cargo test ... --test journal_export -- --test-threads=1` (stderr dropped): `export_rebuilds_fts_index ... ok`, `export_without_journal_config_refuses ... ok`, `second_export_makes_no_commit ... ok`, `title_without_proposal_uses_cut_request ... ok`, `tracer_export_pushes_one_day_file ... ok`; `test result: ok. 5 passed; 0 failed`.
- `cargo test ... --lib journal -- --test-threads=1`: 25 tests incl. `scan_finds_known_value_and_redacts_path`, `day_boundary_is_local_midnight`, `lock_is_exclusive`, `busy_export_touches_nothing`, `rebuild_is_idempotent`, `folded_match_finds_vietnamese`, `record_outcome_sets_export_keys`, `enqueue_mail_dedupes`, and the 8 `journal::config::tests` (`absent_section_is_off`, `ranges_rejected`, `repo_url_injection_rejected`, `branch_rejected`, `groups_validated`, `journal_group_lenient`, `blob_url_percent_encodes`, `web_base_from_remote_forms`); `test result: ok. 25 passed; 0 failed`.
- `cargo test ... --lib journal::config`: `8 passed; 0 failed`. `cargo test ... --lib config::tests`: `42 passed; 0 failed` (34 existing `config::tests` incl. the default-config parses + 8 `journal::config::tests`; `assets/config.default.json` untouched).
- Whole lib suite: `751 passed; 0 failed`. `--test lib_foundation --test dashboard_settings --test db_migration`: 19, 34, 13 passed, 0 failed.
- `cargo build --tests`: no warnings, no errors.
- `git diff --exit-code a03d39d -- agent-brain/Cargo.toml agent-brain/Cargo.lock`: no diff.
- RED runs: Task 2 failed to compile on missing `acquire_lock` and `export_rebuilds_fts_index ... FAILED` before the implementation; Task 3 failed to compile on missing `blob_url`/`web_base`/`percent_encode_path`.
- Acceptance greps: `pub mod journal;` 1 line; `journal::config::parse` 1 line; `journal::cmd_export` 1 line; forbidden patterns (`planfile|protocol_paths|run_dir|workdir|claude_config`, `crate::(engine|runner|roles|fastpath)::|gitops::`) print nothing in `src/journal/`; `pathspec-from-file` 1 line, `GIT_ALLOW_PROTOCOL` 1 line; `step_state` in collect.rs; `grep -c journal src/config.rs` = 4; `LOCK_EX` present; `DELETE FROM journal_fts` 1, `routing::normalize` 1, `INSERT OR IGNORE INTO journal_mail` 1; `pub fn blob_url` 1, `pub fn percent_encode_path` 1, `Cấu hình journal\.` count 14; `Hỗ trợ bộ phận khác` present.

## Deviations from Plan

1. **[Rule 3 - Blocking] `cargo fmt` / `cargo fmt --check` repo-wide.** `cargo fmt --check` is not clean on the base (100 files differ now, all pre-existing; none of them is a journal file and no hunk touches the lines this plan added to `config.rs`/`main.rs`/`lib.rs`). I did not run `cargo fmt` on the crate; every new file was formatted with `rustfmt --edition 2021 <file>` and passes `rustfmt --check`. The Task 3 verify clause `cargo fmt --check` therefore still exits non-zero for out-of-scope reasons.
2. **[Rule 1 - Spec conflict] `!` is not in `ALLOWED_PUNCT`.** The action text keeps `!`, while the must-have truth (and RESEARCH step 4) lists `!` among the removed characters. I followed the must-have (stricter); `ALLOWED_PUNCT = ".,;:()&+-_'\"?"` plus `…`. 07-03/07-04 read the constant, so they stay consistent.
3. **TDD order for Task 1.** The tracer tests and the implementation were written in the same pass; the first test run was already green, so Task 1 has no recorded failing run. Tasks 2 and 3 had real RED runs (above).
4. **`--no-push` stages nothing.** Instead of "stage then reset", `commit_and_push(push=false)` returns `LocalOnly` without touching the index (same observable result: files in the clone, nothing staged, nothing committed).
5. **Local git failures map to `CommittedNotPushed(Other)`.** A failing `git add`/`git commit` has no dedicated `Published` variant in the contract; it is reported as `CommittedNotPushed(GitFailure{Other})` → `ExportOutcome::PushFailed` (exit 1, label only).
6. **Small additive choices (contract names unchanged):** extra public helpers `run_git_input`, `GitOut::ok`, `SafeText::cut`/`is_empty`, `sanitize::cut_chars`, `collect::ROLE_LABELS`, `JournalLock`/`acquire_lock`; `GitCtx` fields are public; `collect` skips run days after `today` (clock-skew guard); `branch` also rejects a leading/trailing `/` and `//`; group ids are limited to 40 chars and labels to 60; `Model.project_slugs` holds projects with at least one entry.
7. **Plan commit ledger:** written to the worktree git dir this time; `commits`/`plan_head_before` above come from it (`git rev-list --count c109693..HEAD` = 3 before the SUMMARY commit).

## Intentional extension points (owned by later plans, not stubs)

- `Model.attention` is always empty and there is no `Cần anh xử lý` section (07-03); `render_all` emits day files only (Today/month/year/project pages are 07-03).
- `gitsync::prepare` ignores `generated` and does no fetch/rebase/dirty handling; `WriteReport.removed` is always empty (07-05).
- `state::record_outcome` ignores `cfg` (push-failure streaks and mails are 07-05).
- `sanitize_text` has no pattern rules (URLs, paths, key shapes) yet (07-04).

## Threat model

T-07-04 (`scan_generated` before any write, known-value redaction in `sanitize_text`), T-07-05 (allow-list, `title_without_proposal_uses_cut_request`), T-07-06 (`repo_url_injection_rejected`, `branch_rejected`, `--` before the URL, `GIT_ALLOW_PROTOCOL`), T-07-07 (`StrictHostKeyChecking=yes`, `BatchMode=yes`, `GIT_TERMINAL_PROMPT=0`), T-07-08 (slug regex/slugify/`du-an-{id}`, writer path checks), T-07-09 (marker check, foreign list), T-07-10 (`lock_is_exclusive`, `busy_export_touches_nothing`), T-07-11 (`-c commit.gpgsign=false -c core.autocrlf=false -c core.hooksPath=/dev/null`, author via env), T-07-SC (no Cargo change) are all mitigated in code. No new surface outside the threat register (threat flags: none).

No auth gates.

## Self-Check: PASSED

- FOUND: agent-brain/src/journal/{mod,config,model,collect,render,sanitize,writer,gitsync,state,fts}.rs, agent-brain/tests/support/journal.rs, agent-brain/tests/journal_export.rs
- FOUND (ancestors of HEAD): f79de9f, f36e87f, f04c525
