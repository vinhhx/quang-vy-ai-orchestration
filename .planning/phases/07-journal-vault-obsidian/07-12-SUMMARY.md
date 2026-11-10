---
phase: 07-journal-vault-obsidian
plan: 12
subsystem: journal
tags: [sanitizer, fts, telegram, gap-closure, code-review]
requires: [07-04, 07-06, 07-09]
provides:
  - "sanitizer treats any dotted token with an alphabetic last label (2+ letters) as a host, except version numbers, single-letter labels and one-dot file names"
  - "single-slash text (15/01, 24/7, CI/CD, và/hoặc) survives; real paths are still redacted; the allow-list keeps an inner `/` only"
  - "fts::rows_from_model_cut: FTS display/title use the vault's callout_title_chars cut"
  - "/tim reply stops at a result boundary with a '… và N kết quả khác' trailer"
affects: [agent-brain/src/journal/sanitize.rs, agent-brain/src/journal/fts.rs, agent-brain/src/telegram/journal_cmd.rs]
key-files:
  created:
    - agent-brain/tests/journal_text.rs
  modified:
    - agent-brain/src/journal/sanitize.rs
    - agent-brain/src/journal/fts.rs
    - agent-brain/src/telegram/journal_cmd.rs
decisions:
  - "WR-05 host rule and its known false positives are documented in a code comment next to RE_HOST (see below)"
  - "FTS rows are cut with a new rows_from_model_cut(model, chars); rows_from_model keeps its signature and uses the default cut (see deviation 1)"
metrics:
  tasks: 2
  completed: 2026-11-10
status: complete
gap_ids: [WR-05, WR-06, WR-08, IN-08]
requirements: [JRN-03, JRN-06]
requirements-completed: [JRN-03, JRN-06]
actuals:
  tokens: 9000
  tasks: 2
  commits: 3
plan_head_before: 7de1dedf4daa92a7fbdd9558f31a0265cd8513ea
plan_head_after: b749cb7b614a7f6d549ad48036521608d4ba1318
---

# Phase 7 Plan 12: sanitizer rules, `/tim` text and reply budget Summary

Closes code-review findings WR-05, WR-06, WR-08 and IN-08: internal hosts of any alphabetic TLD are redacted, ordinary single-slash text is no longer erased, `/tim` indexes and returns only the title text the vault shows, and a long `/tim` reply ends at a result boundary with a trailer instead of being cut mid-URL.

## What was done

1. **Task 1 (fix, 19fcb97) - `sanitize.rs`.**
   - WR-05: `RE_HOST` is now `(?i)\b(?:[a-z0-9-]+\.)+[a-z]{2,}(?::\d{1,5})?\b` plus a closure (`is_host_match`) that keeps one-dot file names with an extension from `FILE_EXTENSIONS` (`node.js`, `report.pdf`, `main.rs`) unless a `:port` follows. Version numbers (`v1.2`, `2.5.1`) and single-letter labels (`e.g`, `i.e`, `a.b`) never match, because the last label must be 2+ letters.
   - WR-06: `RE_REL_PATH` still finds any word with an inner `/`; `is_path_token` decides. A word is kept only when it has exactly one `/` between two short sides (letters up to 6, or digits up to 4: `CI/CD`, `I/O`, `A/B`, `và/hoặc`, `24/7`, `50/50`, `15/01`) or is a three-number date (`15/01/2026`). Everything else with a `/` is a path: two or more separators, a side with a dot or hyphen or other characters (`a/b.php`, `task/sua-loi-dang-nhap`, `./a`, `C:/x`), or a long side. Words starting with `/`, `~` or a drive letter are still caught earlier by the absolute, home and Windows rules.
   - The allow-list used to turn every `/` into a space, so `15/01` could not survive. It now keeps a `/` only when both neighbours are letters or digits; a leading, trailing or doubled `/` is never kept. Because only words judged "not a path" can have an inner `/` at that point, no path or URL shape gets through.
   - Known false positives of the host rule (documented next to `RE_HOST`): `ASP.NET`, `Dr.Smith`, two sentences stuck together without a space (`xong.Sau`), multi-dot file names (`jquery.min.js`). Known false negatives: one-dot domains whose TLD equals a listed file extension (`intranet.sh`, `corp.rs`), and short pairs such as `src/app` (both sides at most 6 letters). Over-redaction is the safe side (D-03), so a wider list was not pursued.
2. **Task 2 (fix, f6eea13) - `fts.rs`, `journal_cmd.rs`.**
   - WR-08: `rows_from_model_cut(model, title_chars)` builds `display` and `title` from `t.title.cut(title_chars)`, the exact text of the callout header and project page. `rows_from_model(model)` keeps its signature and delegates with the default `callout_title_chars`.
   - IN-08: `format_reply` builds each result (date line and link) first, then picks the largest number of whole results that fits `MAX_TEXT` including the trailer. The header counts the listed results; the last line is `… và N kết quả khác` (N = results found but not listed). Nothing is cut inside a result or URL; the final `clip` in `on_tim` stays as a safety net and no longer triggers for results.
3. **Task 2 (test, b749cb7) - `tests/journal_text.rs`**, three end-to-end tests with the fake Telegram and a real `journal export --no-push`:
   - `tim_does_not_index_title_text_the_vault_cuts`: proposal title with `tukhoaxanhla` after character 70; neither the vault nor `/tim` shows it and `/tim tukhoaxanhla` is "not found".
   - `tim_reply_stops_at_a_result_boundary`: 22 tasks with 400-character titles, `search_max_results` 20; reply under 4096 characters, header count equals the number of complete links, last line `… và N kết quả khác` with listed + N = 20.
   - `ordinary_slash_text_survives_and_real_paths_and_hosts_do_not_leak`: `Cấu hình CI/CD cho bản phát hành ngày 15/01` is in the vault and in the `/tim CI` reply unchanged; a title with `https://intranet.example.co/x?token=abc`, `/home/someone/app/config.php` and `git.example.co` leaks into no vault file, no digest text (built from the same collector/summary) and no `/tim` reply.

## Verification (real output)

- `cargo test ... --lib journal::sanitize`: `test result: ok. 22 passed; 0 failed` (14 existing + 8 new; the new ones are `hosts_of_any_alphabetic_tld_are_redacted`, `non_hosts_survive`, `single_slash_text_survives`, `real_paths_are_still_redacted`, `date_survives_beside_path_and_url`, `slash_never_survives_at_the_edge_or_doubled`, `relaxed_rules_keep_secret_and_code_handling`, `gate_accepts_slash_words_in_day_text`).
- `cargo test ... --test journal_sanitize --test journal_render`: `ok. 10 passed` (journal_render, `golden_files_match` among them) and `ok. 2 passed` (journal_sanitize). Golden files are unchanged: no fixture was touched. Note: the plan's verify command passes `journal::sanitize` as a name filter, which also filters the two integration binaries to 0 tests; they were therefore run again without the filter.
- `cargo test ... --lib journal::fts` `ok. 7 passed` (new `rows_use_the_vault_title_cut`); `--lib journal_cmd` `ok. 3 passed` (new `format_reply_stops_at_a_result_boundary`).
- `cargo test ... --test journal_text --test journal_tim`: `ok. 3 passed` (journal_text) and `ok. 4 passed` (journal_tim, the 4 existing tests unchanged and green).
- Full suite: `cargo test ... --no-fail-fast`, run twice: first run `suites 92 passed 1870 failed 1`, second run `suites 92 passed 1869 failed 2`. The failures are all in `tests/telegram_actions.rs`, which does not touch journal code: `running_run_adds_kill_button` fails in every run (also when run alone, and also with the prebuilt `telegram_actions` binary of the main checkout run read-only, which was not built from this worktree), `press_without_text_keeps_tracked_text` failed once under the load of three parallel builds and passed in the main-checkout run. I judged both unrelated to this plan (out of scope, not fixed); this means the plan's `failed 0` criterion for the full suite is NOT met literally and a human should look at `running_run_adds_kill_button` (a Telegram kill-button/edit sequence test) separately. All journal suites (`journal_text` 3, `journal_tim` 4, `journal_sanitize` 2, `journal_render` 10, `journal_export`, `journal_gitsync`, `journal_serve`) and the lib unit tests passed. No compiler warning from the changed files (`cargo check --lib --test journal_text` prints nothing).
- `rustfmt --check --edition 2021` is clean for `sanitize.rs`, `fts.rs`, `journal_cmd.rs` (all three were clean on the base) and for `tests/journal_text.rs` (new file, formatted by hand because rustfmt would also reformat the shared `tests/common` and `tests/support` modules through its `mod` declarations; those were not touched).
- `git diff --name-only 7de1ded..HEAD` lists only the four declared files; Cargo files unchanged; no dependency added.

### Each test can fail

- `tim_does_not_index_title_text_the_vault_cuts`: with `fts.rs` temporarily indexing the full title (`let shown = t.title.clone()`), it failed with `reply shows tukhoaxanhla: Nhật ký — 1 kết quả cho: soat ... vào tukhoaxanhla rồi kiểm tra lại toàn bộ cuoicungtitle xong`. Restored afterwards.
- `tim_reply_stops_at_a_result_boundary`: with the budget check in `format_reply` disabled (`<= usize::MAX`), it failed because the final `clip` cut the last URL mid-way (`https://github.com/example-owner-with-a-long-name/example-vault-with-a`). Restored afterwards.
- Sanitizer unit tests: the old rules cannot pass them by construction (the old TLD list had no `co`, `ai`, `me`, `tech`, `site`, `uk`, `de`; the old `RE_REL_PATH` matched every inner `/`; the old allow-list stripped every `/`). I did not re-run them against the old code.
- `ordinary_slash_text_survives_and_real_paths_and_hosts_do_not_leak`: its survival half fails on the old sanitizer by the same reasoning (`15/01` became `(đường dẫn)`); not run against the old code.

## Deviations from Plan

### Auto-fixed / adjusted

**1. [Rule 3 - Blocking] The FTS row builder does not receive the journal config**
- **Found during:** Task 2
- **Issue:** The plan says to read `callout_title_chars` "from the journal config that is already passed to the FTS row builder". `fts::rows_from_model(model)` takes only the model; the single call site is `agent-brain/src/journal/mod.rs:175` (`fts::rebuild(p.conn, &fts::rows_from_model(&model))`), and `mod.rs` is owned by plan 07-10 (not in this plan's `files_modified`, edited in parallel).
- **Fix:** Added `fts::rows_from_model_cut(model, title_chars)` and kept `rows_from_model(model)` (public signature unchanged, as required) delegating with `JournalConfig::default().callout_title_chars` (60). With the default configuration, which the tests use, `/tim` now matches the vault exactly.
- **Remaining follow-up for the orchestrator:** for a non-default `journal.callout_title_chars`, the index still uses 60 characters while the vault uses the configured value. To make it exact, change the call in `mod.rs` to `fts::rows_from_model_cut(&model, cfg.callout_title_chars)` after plans 07-10 and 07-12 are merged (one line; `cfg` is `p.cfg` in `export_with_tz`). With a configured value below 60 the index would show more than the vault; with a value above 60 it shows less. The test `tim_reply_stops_at_a_result_boundary` sets `callout_title_chars` to 200 and passes either way.
- **Files modified:** `agent-brain/src/journal/fts.rs`
- **Commit:** f6eea13

**2. [Rule 1 - Bug] Slash survival needed an allow-list change**
- **Found during:** Task 1
- **Issue:** The plan only mentions the path rule, but `ALLOWED_PUNCT` excludes `/`, so even a kept `15/01` came out as `15 01`.
- **Fix:** `sanitize_text` keeps a `/` only when both neighbours are letters or digits (see What was done). `ALLOWED_PUNCT` itself is unchanged.
- **Commit:** 19fcb97

**3. [Choice within the plan] `/tim` header wording on truncation**
- The header states the number of listed results (not the total found), and the trailer carries the rest, so "the header count matches the listed results". When nothing is cut the reply is byte-identical to before (existing `format_reply_lines` test unchanged). "N" counts only results that the search returned (the search is limited to `search_max_results`), not every match in the index.

**4. [Test scope] FTS rows for request-derived titles**
- When a task has no proposal title the title is the request text; the vault shows the request in the callout body up to `title_max_chars` but the header only up to `callout_title_chars`. Following the plan, the index now covers only the header cut, so a word that sits only in characters 61-140 of such a request is no longer found by `/tim` although the body shows it. This is the conservative direction of D-15 and matches the plan and the review's first suggested fix.

None other. No architectural change, no new dependency, no config key, no change to the Cargo files, STATE.md, ROADMAP.md or REQUIREMENTS.md.

## Known Stubs

None.

## Threat Flags

None. Mitigations T-07-12-01 (any-TLD host rule, unit tests with uncommon TLDs), T-07-12-02 (explicit path shapes, mixed-input and end-to-end tests) and T-07-12-03 (FTS rows use the vault's cut, test (a)) are implemented; T-07-12-SC: no dependency added.

## Self-Check: PASSED

- Files exist: `agent-brain/tests/journal_text.rs`, `agent-brain/src/journal/sanitize.rs`, `agent-brain/src/journal/fts.rs`, `agent-brain/src/telegram/journal_cmd.rs`.
- Commits exist on the branch: 19fcb97 (sanitizer), f6eea13 (fts and /tim reply), b749cb7 (tests); `git rev-list --count 7de1ded..HEAD` was 3 before this SUMMARY commit.
- STATE.md, ROADMAP.md and REQUIREMENTS.md untouched.
