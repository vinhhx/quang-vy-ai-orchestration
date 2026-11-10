# Phase 7 — Code review disposition (07-REVIEW.md)

Each finding of `07-REVIEW.md` and what happened to it (2026-11-10).

| Finding | Severity | Disposition | Where |
|---------|----------|-------------|-------|
| WR-01 mass deletion on clock skew / empty DB | warning | fixed | plan 07-10 (guards, `--allow-removals`, `journal.max_remove_fraction`) |
| WR-02 slug with `..` breaks every export | warning | fixed | plan 07-10 |
| WR-03 permanent alert dedupe keys | warning | fixed | plan 07-10 (episode-scoped keys) |
| WR-04 inherited `GIT_*` vars, non-literal pathspecs | warning | fixed | plan 07-11 |
| WR-05 short TLD allow-list for hosts | warning | fixed | plan 07-12 |
| WR-06 `RE_REL_PATH` mangles dates and `và/hoặc` | warning | fixed | plan 07-12 |
| WR-07 carry-over never expires | warning | fixed | plan 07-10 (`journal.attention_max_days`) |
| WR-08 `/tim` shows more title text than the vault | warning | fixed | plan 07-12 (+ wiring commit `fts::rows_from_model_cut`) |
| WR-09 claimed mail lost on error | warning | partly fixed | plan 07-11 fixes the per-row claim; a claimed row can still be lost when the Telegram outbox overflows its 200-item cap (`outbox.rs`, outside the plan): **open, low risk** |
| WR-10 branch never checked against `journal.branch` | warning | fixed | plan 07-11 |
| WR-11 write through a symlinked directory | warning | fixed | plan 07-10 |
| WR-12 relative `clone_dir` | warning | fixed | plan 07-10 |
| WR-13 test gaps | warning | mostly fixed | new tests in `journal_safety.rs`, `journal_gitmail.rs`, `journal_text.rs`; **open:** no end-to-end test of `JournalTz::Local`, interrupted-rebase and stale-lock scenarios, serve-loop digest retry across a failed push |
| IN-01 fallback key collision | info | fixed | plan 07-10 |
| IN-02 "Hôm qua" label | info | fixed | plan 07-10 |
| IN-03 CLI vs loop secret sets | info | partly fixed | plan 07-10 shares one helper; the dashboard token exists only in the running `serve` process, so the CLI cannot include it: **accepted** |
| IN-04 FK tolerance compares counts | info | open | follow-up: compare violation identity (`db.rs`) |
| IN-05 vault `.gitignore` hiding a managed path | info | open | follow-up |
| IN-06 interrupted rebase aborted unconditionally | info | open | follow-up |
| IN-07 user's ssh command overwritten | info | fixed | plan 07-11 |
| IN-08 `/tim` reply cut mid-result | info | fixed | plan 07-12 |
| IN-09 credentials accepted in `repo_url` | info | fixed | plan 07-10 |
