---
phase: 07-journal-vault-obsidian
plan: 06
subsystem: telegram
tags: [rust, telegram, fts5, sqlite, journal, search]
requires:
  - "07-02: journal_fts filled by every export, journal::fts::{FtsRow, fold, rebuild}, journal::config::blob_url, model::day_path"
provides:
  - "journal::fts::{MAX_QUERY_CHARS, MAX_QUERY_TOKENS, query_tokens, match_query, Hit, search, index_is_empty}"
  - "telegram::journal_cmd::{USAGE_TIM, NOT_FOUND_PREFIX, EMPTY_INDEX, format_reply, on_tim}"
  - "Telegram command /tim <tu khoa> (dispatch arm, setMyCommands entry, help text)"
affects: [07-07, 07-08]
tech-stack:
  added: []
  patterns:
    - "untrusted text -> fold -> split on non-alphanumerics -> quoted FTS5 tokens, bound as one MATCH parameter"
    - "read-only Telegram command handled inline on the poll thread (like /bugs)"
key-files:
  created:
    - agent-brain/src/telegram/journal_cmd.rs
    - agent-brain/tests/journal_tim.rs
  modified:
    - agent-brain/src/journal/fts.rs
    - agent-brain/src/telegram/intake.rs
    - agent-brain/src/telegram/mod.rs
    - agent-brain/src/telegram/bot.rs
    - agent-brain/tests/telegram_intake.rs
key-decisions:
  - "The prefix star is added to the last token only when it has 2+ chars, so `du-an-a` does not also match every `an` of `du-an-b`"
  - "query_tokens is a public helper of fts.rs; on_tim uses it for the folded query shown in replies and the empty-query check"
requirements-completed: [JRN-06]
status: complete
commits: 3
plan_head_before: cad033c827859cf9837ab8a19642903e0dfa9389
plan_head_after: 6f196ba373599946cb5d2405cc0712bc69c8a6ba
actuals:
  tokens: 14000
  tasks: 3
  commits: 3
duration: ~35min
completed: 2026-11-10
---

# Phase 7 Plan 06: /tim over the journal index Summary

**`/tim <tu khoa>` from the paired chat searches only `journal_fts` (Vietnamese-folded, quoted tokens, 100-char / 8-token caps, bound MATCH parameter) and answers with `DD/MM/YYYY · {du an} — {tieu de} · {trang thai}` plus a percent-encoded github.com link to the day file; hostile input never errors, strangers get no reply.**

## Tasks

| Task | Name | Commit |
| ---- | ---- | ------ |
| 1 | safe MATCH builder and search over journal_fts | a36f655 |
| 2 | the /tim command: handler, dispatch, menu, help text | 7452a57 |
| 3 | /tim end to end through serve and the fake Telegram API | 6f196ba |

## What changed

- `journal/fts.rs` (additive; `FtsRow`, `fold`, `rows_from_model`, `rebuild` untouched): `MAX_QUERY_CHARS = 100`, `MAX_QUERY_TOKENS = 8`, `query_tokens`, `match_query`, `Hit`, `search` (`... WHERE journal_fts MATCH ?1 ORDER BY bm25(journal_fts), day DESC LIMIT ?2`, reads `day` and `display` only), `index_is_empty`.
- `telegram/journal_cmd.rs` (new): `USAGE_TIM`, `NOT_FOUND_PREFIX`, `EMPTY_INDEX`, `format_reply`, `on_tim`. No plan/run-dir/QA code is referenced (acceptance grep prints nothing).
- `intake.rs`: arm `"tim" => super::journal_cmd::on_tim(ctx, rest)` after `"reject"`. `mod.rs`: `pub mod journal_cmd;`, `setMyCommands` entry `tim` (last), comment mentions `/tim`. `bot.rs`: one sentence appended to `HELP_TEXT`. Pairing is the existing `authz` before dispatch (unchanged).
- `tests/telegram_intake.rs`: exact command list gets the 7th entry `tim`.
- `tests/journal_tim.rs` (new): fixture with 3 tasks, real `journal export --no-push`, `serve` against the fake Telegram.

## Verification (real output, stderr dropped)

- `cargo test --offline --locked --lib journal::fts -- --test-threads=1`: `match_query_quotes_tokens ... ok`, `hostile_queries_never_error ... ok`, `search_folds_and_prefixes ... ok`, `index_is_empty_reports ... ok` (plus the 2 existing); `test result: ok. 6 passed; 0 failed`.
- `cargo test ... --lib telegram::journal_cmd`: `format_reply_lines ... ok`, `format_reply_without_web_base ... ok`; `2 passed; 0 failed`.
- `cargo test ... --test telegram_intake -- --test-threads=1`: `test result: ok. 48 passed; 0 failed`.
- `cargo test ... --test journal_tim -- --test-threads=1`: `tim_finds_by_folded_words ... ok`, `tim_from_stranger_gets_no_reply ... ok`, `tim_hostile_queries_and_usage ... ok`, `tim_never_returns_text_outside_the_vault ... ok`; `test result: ok. 4 passed; 0 failed`.
- Whole lib suite: `757 passed; 0 failed`. `cargo build --tests`: 0 warnings.
- `git diff --exit-code a03d39d -- agent-brain/Cargo.toml agent-brain/Cargo.lock`: no diff.
- Acceptance greps: `"tim" => super::journal_cmd::on_tim` 1 line in intake.rs; `"command": "tim"` 1 line each in mod.rs and tests/telegram_intake.rs; `/tim` in bot.rs help; `planfile|protocol_paths|run_dir|qa-report|runs_for_task` in journal_cmd.rs prints nothing; `kehoachbimat` and `STRANGER_CHAT` present in journal_tim.rs.
- RED run: Task 1 `search_folds_and_prefixes` failed first with `left: ["2026-01-15", "2026-01-14"]` (before the 2-char prefix rule). Tasks 2/3 were written together with their implementation, so no separate failing run is recorded for them.

## Deviations from Plan

1. **[Rule 1 - Bug in spec] Prefix star only for tokens of 2+ chars.** The contract said the last token always gets `*`. With that, `du-an-a` folds to `"du" "an" "a"*`, and `a*` also matches `an` of `du-an-b`, so the required `du-an-a -> [du-an-a row]` test failed. A one-char last token is now matched exactly; behaviour for every example in the plan (`dang nhap`, `tim`, `kie`, `2026 01 14`) is unchanged. Covered by an extra assertion in `match_query_quotes_tokens` (`du a` -> `"du" "a"`).
2. **[Rule 3 - Blocking] `cargo fmt --check` is not clean on the base repo** (about 100 pre-existing files differ). Not run crate-wide; the new files (`journal_cmd.rs`, `journal_tim.rs`) and `fts.rs` pass `rustfmt --check --edition 2021`, and lines added to existing files follow rustfmt style. The Task 3 `cargo fmt --check` clause therefore still exits non-zero for out-of-scope reasons; the Cargo-diff clause passes.
3. **Small additive choices:** public helper `fts::query_tokens` (used by `on_tim` for the empty-query check and the folded text in replies); a DB error inside the search is also reported as `Lỗi CSDL — xem terminal đang chạy agent-brain serve` with a `term::emit` line (the plan listed this only for `db::open`); in `format_reply` a hit with an unparseable day prints `{day} · {display}` without a link line.
4. `tests/journal_tim.rs` fixes the run directory of task C with a direct `UPDATE runs SET run_dir` because `support/journal.rs::seed_run` (07-02) always inserts `run_dir = None` and is outside this plan's files.

## Known Stubs

None.

## Threat model

T-07-26 (fold + split + quoted tokens, caps, bound parameter, config result limit, `clip`; `hostile_queries_never_error`, `tim_hostile_queries_and_usage`), T-07-27 (existing `authz` before dispatch; `tim_from_stranger_gets_no_reply`, which also proves the bot kept processing), T-07-28 (only `day`/`display` read; `tim_never_returns_text_outside_the_vault` with a word beyond the 140-char cut and a word only in a run-dir plan file), T-07-SC (Cargo files unchanged) are mitigated. Threat flags: none.

No auth gates.

## Self-Check: PASSED

- FOUND: agent-brain/src/telegram/journal_cmd.rs, agent-brain/tests/journal_tim.rs, agent-brain/src/journal/fts.rs
- FOUND (ancestors of HEAD): a36f655, 7452a57, 6f196ba
