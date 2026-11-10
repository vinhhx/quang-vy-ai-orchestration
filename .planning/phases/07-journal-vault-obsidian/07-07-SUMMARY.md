---
phase: 07-journal-vault-obsidian
plan: 07
subsystem: journal
tags: [rust, serve, telegram, digest, scheduler, sqlite]
requires:
  - "07-02: journal skeleton (run_export_once, JournalConfig, DaySummary, state::{get,set,enqueue_mail}, journal_mail table)"
provides:
  - "journal::digest::{digest_key, digest_moment_ms, digest_due, digest_link, digest_text, queued}"
  - "journal::loop_::{LOOP_TICK_SECS, KEY_LOOP_TICKS, export_due, LoopMem, tick, spawn_loop}"
  - "telegram::detect::JournalMail (claim-before-push, stale digest drop, scan at start)"
  - "serve starts the journal loop (dashboard/mod.rs, one line)"
affects: [07-08]
tech-stack:
  added: []
  patterns:
    - "pure scheduling functions generic over chrono::TimeZone, clock injected by the caller"
    - "mailbox delivery: UPDATE sent_at WHERE sent_at IS NULL, push to outbox only when one row changed"
    - "tokio task + spawn_blocking per tick, same shape as the orchestrator loop, with a shutdown-aware sleep"
key-files:
  created:
    - agent-brain/src/journal/digest.rs
    - agent-brain/src/journal/loop_.rs
    - agent-brain/tests/journal_serve.rs
  modified:
    - agent-brain/src/journal/mod.rs
    - agent-brain/src/telegram/detect.rs
    - agent-brain/src/dashboard/mod.rs
key-decisions:
  - "loop_ticks is bumped at the END of a tick (also when the tick failed), so a grown counter means a whole tick, export included, has finished; the plan listed it near the start of tick"
  - "A digest whose dedupe key does not parse as a date is sent like any other mail; only a parsed day different from today's local day is dropped"
  - "The digest is enqueued under report.today (the day of the export) and the tick's `today` is computed from the same now, so both agree"
requirements-completed: [JRN-04, JRN-05]
status: complete
commits: 3
plan_head_before: cad033c827859cf9837ab8a19642903e0dfa9389
plan_head_after: d63566fe1ff8595db18ef64094f5dd8531986aaa
actuals:
  tokens: 26000
  tasks: 3
  commits: 3
duration: ~75min
completed: 2026-11-10
---

# Phase 7 Plan 07: Journal loop in serve and daily digest Summary

**`agent-brain serve` now exports the vault on a 60 s scheduler (about every `journal.interval_minutes`, at day change, and once after `journal.digest_time`), queues exactly one Telegram digest per local day only after a push that succeeded or found the vault at the remote head, and the Telegram detector delivers every `journal_mail` row to the paired chat at most once (claim `sent_at`, then outbox), dropping late digests.**

## Tasks

| Task | Name | Commit |
| ---- | ---- | ------ |
| 1 | scheduling and digest functions with a fake clock | 6f15cf9 |
| 2 | deliver journal_mail rows to the paired chat at most once; drop stale digests | 171eb79 |
| 3 | serve loop exports on schedule, one digest per day after a successful push | d63566f |

## What changed

- `journal/digest.rs`: `digest_key` (`digest:YYYY-MM-DD`), `digest_moment_ms` (local `digest_time`, DST-gap fallback), `digest_due`, `digest_link` (day file when the day has tasks or attention items, else `Today.md`), `digest_text` (header, per-group counts, `Cần anh xử lý (k):` with up to 10 lines and `… và m mục khác`, or `Không có việc cần anh xử lý.`, link last), `queued`.
- `journal/loop_.rs`: `export_due` (never exported / interval / day change / digest pending with a last export before the digest moment), `LoopMem`, `tick` (reads `journal_state`, prints the once-per-day "bỏ digest … chưa đẩy được trước 24:00" warning, runs `run_export_once(push=true)`, calls `state::enqueue_mail("digest", …)` only in the `Pushed | UpToDate` arm when a digest is pending), `spawn_loop` (ends at once without `journal.enabled`/`repo_url`; tick in `spawn_blocking`; errors only through `term::emit`).
- `telegram/detect.rs`: `JournalMail::scan` (select unsent rows, claim each with `UPDATE … WHERE id = ? AND sent_at IS NULL`, push `Outgoing::Message` only when one row changed; stale `digest` rows are claimed and dropped with `Nhật ký: bỏ digest ngày DD/MM/YYYY — quá 24:00, không gửi muộn`), run once right after seeding and on every scan.
- `dashboard/mod.rs`: one inserted line `let _journal = crate::journal::loop_::spawn_loop(state.clone());` after the orchestrator spawn.
- `journal/mod.rs`: only `pub mod digest;` and `pub mod loop_;`.

## Verification (real output)

- `cargo test ... --lib journal -- --test-threads=1`: `test result: ok. 34 passed; 0 failed` (25 from 07-02 plus 9 new: `digest_due_cases`, `digest_moment_is_local_digest_time`, `digest_text_format`, `digest_text_without_attention_and_overflow`, `digest_text_cuts_long_titles`, `digest_link_falls_back_to_today_md`, `digest_key_is_per_day`, `queued_sees_any_row_with_the_key`, `export_due_cases`; all `... ok`).
- `cargo test ... --test journal_serve -- --test-threads=1`: `test result: ok. 7 passed; 0 failed` (`digest_once_after_push`, `loop_disabled_by_default`, `mail_delivered_once`, `mail_inserted_while_running_is_delivered`, `no_digest_before_digest_time`, `no_digest_when_push_fails`, `stale_digest_dropped`, all `... ok`).
- Task 2 gate before the loop existed: `--test journal_serve mail`: 2 passed (`mail_delivered_once`, `mail_inserted_while_running_is_delivered`); `stale_digest_dropped`: 1 passed; `--test telegram_notify --test telegram_intake`: 48 passed and 11 passed, 0 failed.
- `cargo test ... --test dashboard_http --test orchestrator_scheduler`: 9 passed and 21 passed, 0 failed.
- `git diff --exit-code a03d39d -- agent-brain/Cargo.toml agent-brain/Cargo.lock`: no diff. `git diff --stat -- agent-brain/src/dashboard/mod.rs` (working change before commit): 1 insertion.
- Acceptance greps: `pub mod digest;` and `pub mod loop_;` one line each in `journal/mod.rs`; no `Local::now|SystemTime::now|now_ms()` in `digest.rs`; `Cần anh xử lý` 5 lines in `digest.rs`; `sent_at IS NULL` 2 lines in `detect.rs`; `pub struct JournalMail` 1 line; `journal::loop_::spawn_loop` 1 line in `dashboard/mod.rs`; `spawn_blocking` in `loop_.rs`; `enqueue_mail` 1 line in `loop_.rs`, inside the `Pushed | UpToDate` arm.
- `rustfmt --check --edition 2021` is clean for `digest.rs`, `loop_.rs` and `tests/journal_serve.rs`; the lines added to `detect.rs`, `dashboard/mod.rs` and `journal/mod.rs` follow rustfmt style (their files have pre-existing diffs; no rustfmt hunk touches the added lines).

## Deviations from Plan

1. **[Rule 3 - Blocking] `cargo fmt --check` repo-wide.** Not clean on the base (about 100 pre-existing files differ), so the clause in Task 3's second verify command cannot exit 0 for out-of-scope reasons. I did not run `cargo fmt` on the crate; new files were checked per file with `rustfmt --check --edition 2021`. The other commands of that verify (`dashboard_http`, `orchestrator_scheduler`, Cargo diff) were run separately and pass.
2. **TDD order.** The unit tests, the implementation and the integration tests were written in the same pass for each task; there is no recorded failing RED run for any of the three tasks. Every test was run green and asserts real behaviour (for example `digest_once_after_push` checks the digest text/link, one remote commit and a restart).
3. **Plan commit ledger.** Writing `gsd-plan-head-before-07-07` into the shared `.git/worktrees/...` directory was refused by the worktree guard, so `plan_head_before` is the base SHA from the dispatch (`cad033c`), and `commits: 3` is `git rev-list --count cad033c..HEAD` before the SUMMARY commit.
4. **`loop_ticks` is incremented at the end of `tick`**, not near its start (see key-decisions), so tests that wait for it also wait for the export.
5. **Timeout margins in tests.** `mail_inserted_while_running_is_delivered` waits up to 10 s (plan: 5 s) for the data_version hint because five worktrees build and test concurrently.
6. **Additive items.** `LoopMem` (the in-memory "warned days" set the plan called `mem`), extra unit tests (`digest_moment_is_local_digest_time`, `digest_text_cuts_long_titles`, `digest_key_is_per_day`, `queued_sees_any_row_with_the_key`), and a `Công việc 1` assertion on the digest's group line.

## Known Stubs

None.

## Threat model

- T-07-29: `dedupe_key` UNIQUE + `INSERT OR IGNORE` + claim-before-outbox; `mail_delivered_once` and the restart step of `digest_once_after_push` pass.
- T-07-30: digest queued only in the `Pushed | UpToDate` arm; `no_digest_when_push_fails` and `stale_digest_dropped` pass.
- T-07-31: ticks run in `spawn_blocking`, git bounded by 07-02's runner timeouts, `journal.lock` handles overlap with a CLI export (`Busy` queues nothing).
- T-07-32: messages leave only as `Outgoing::Message` through the bot outbox (paired chat id); texts are 07-05's templates or this plan's digest built from `SafeText` titles, group labels and a github.com link. No local paths, remote URL or raw git output are included.
- T-07-SC: no dependency added, Cargo files unchanged.

Threat flags: none. A11 (at-most-once delivery) is accepted as written: a crash between claim and send loses one message.

## Notes for 07-08 and the orchestrator

- `serve` reads the `journal` config once at start; a config change needs a restart of serve.
- The digest text uses `DaySummary.attention`; until 07-03 fills it, the digest says `Không có việc cần anh xử lý.`.
- `UpToDate` is the plan's own definition of "vault at remote head" (`HEAD == origin/<branch>` after staging, see `gitsync::commit_and_push`).

No auth gates.

## Self-Check: PASSED

- FOUND: agent-brain/src/journal/digest.rs, agent-brain/src/journal/loop_.rs, agent-brain/tests/journal_serve.rs
- FOUND (ancestors of HEAD): 6f15cf9, 171eb79, d63566f
