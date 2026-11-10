---
phase: 07-journal-vault-obsidian
plan: 10
subsystem: journal
tags: [gap-closure, code-review, safety, git, obsidian, vault, config]
requires: [07-02, 07-05, 07-09]
provides:
  - "ExportOutcome::Refused(String): clock guard (now more than 1 day behind brain.db) and mass-deletion guard (>= 10 marked files and more than journal.max_remove_fraction), both before any write/commit/push"
  - "agent-brain journal export --allow-removals (CLI only; skips the deletion guard once, never the clock guard)"
  - "ExportOutcome::Failed: an export Err is recorded (last_export_ms, journal_state, one mail per episode) and still returned to the caller"
  - "episode-scoped alert dedupe keys (alert_episode_ms, foreign_episode_ms in journal_state)"
  - "config keys journal.max_remove_fraction (0.05..=1.0, default 0.5) and journal.attention_max_days (1..=365, default 14)"
  - "journal::known_secrets(cfg, config_dir, dashboard_token): the serve loop's secret set"
  - "tests/journal_safety.rs (12 tests)"
affects: [agent-brain/src/journal/mod.rs, agent-brain/src/journal/state.rs, agent-brain/src/journal/collect.rs, agent-brain/src/journal/writer.rs, agent-brain/src/journal/config.rs, agent-brain/src/journal/render.rs, agent-brain/src/main.rs]
key-files:
  created:
    - agent-brain/tests/journal_safety.rs
  modified:
    - agent-brain/src/journal/mod.rs
    - agent-brain/src/journal/state.rs
    - agent-brain/src/journal/collect.rs
    - agent-brain/src/journal/writer.rs
    - agent-brain/src/journal/config.rs
    - agent-brain/src/journal/render.rs
    - agent-brain/src/main.rs
key-decisions:
  - "The loop keeps calling run_export_once (unchanged signature, struct literal in loop_.rs untouched); the CLI calls run_export_once_with(p, ExportOptions { allow_removals }). ExportParams gained no field, so loop_.rs and tests/support need no edit"
  - "The deletion guard runs twice: before gitsync::prepare (as planned) and again right after prepare returns Ready, still before writer::apply, so a fresh clone of a full remote with an empty brain.db is also refused"
  - "Failed is a separate variant (not Refused): it carries no reason and its mail text is fixed (state::FAILED_MAIL_TEXT)"
  - "'Closed' task statuses = crate::tasks::SUCCESS_STATUSES (answered, qa_pass). The repo has no closed/cancelled task status; none was invented"
  - "max_remove_fraction is stored as permille (u32) because JournalConfig derives Eq"
  - "Foreign-file alerts use their own episode (foreign_episode_ms), ended by an export that wrote files and found no foreign file; tying them to the blocked episode would re-mail on every successful export"
  - "notrepo keeps its per-day key (notrepo:{day}); it already recurs"
metrics:
  tasks: 3
  completed: 2026-11-10
status: complete
gap_ids: ["WR-01", "WR-02", "WR-03", "WR-07", "WR-11", "WR-12", "IN-01", "IN-02", "IN-03", "IN-09"]
requirements: [JRN-01, JRN-02, JRN-03, JRN-04, JRN-07]
requirements-completed: [JRN-01, JRN-02, JRN-03, JRN-04, JRN-07]
actuals:
  tokens: 10900
  tasks: 3
  commits: 3
plan_head_before: 7de1dedf4daa92a7fbdd9558f31a0265cd8513ea
plan_head_after: 787f250e145ccf960e8072525cb746ed000444bc
---

# Phase 7 Plan 10: export guards, failure recording, episode alerts and config checks Summary

An export now refuses to run with a clock behind brain.db or to delete most of the vault, records its own errors with backoff, re-alerts recurring problems once per episode, never writes through a symlinked directory, and rejects relative clone dirs and credential URLs. A new 12-test file pins each fix, and every test was shown failing without its fix.

## What changed per finding

| Finding | Fix | Where |
|---|---|---|
| WR-01 | Clock guard: `newest_activity_ms` (max of runs.started_at/ended_at, tasks.approved_at, plan_approvals.approved_at). `now_ms` more than 1 day earlier gives `Refused(REFUSED_CLOCK)` before collect. Deletion guard: `writer::planned_removals` (read-only, same listing and marker test as `apply`). Refused when removals >= 10 and removals*1000 > `max_remove_permille`*existing. It runs after the secret gate, before `prepare`, and again after `prepare`. CLI prints the fixed reason and exits 1 | mod.rs, writer.rs, collect.rs, config.rs |
| WR-01 override | `journal export --allow-removals` -> `ExportOptions { allow_removals: true }`, CLI only. It skips only the deletion guard | main.rs, mod.rs |
| WR-02 | `slug_ok` rejects `..` and a trailing `.`. `run_export_once_with` records an `Err` as `ExportOutcome::Failed` (last_export_ms, blocked=failed, mail `failed:{episode}`), then returns the `Err` | collect.rs, mod.rs, state.rs |
| WR-03 | Dedupe keys `conflict:{ep}:{hash}`, `dirty:{ep}:{hash}`, `secret:{ep}:{hash}`, `refused:{ep}:{hash}`, `failed:{ep}`, `foreign:{fep}:{hash}`. `on_success` clears `alert_episode_ms` | state.rs |
| WR-07 | Carry-over skips tasks with a success status and items older than `attention_max_days`. The day's own list is unchanged | collect.rs, config.rs |
| WR-11 | Each existing ancestor between clone_dir and the target must be a real directory (`symlink_metadata`). A symlinked ancestor, or a target that is a symlink or not a regular file, is reported `foreign` and not written. Temp file opened with `create_new`; a stale temp (or a planted symlink) is removed, not followed | writer.rs |
| WR-12 | `clone_dir` must be absolute after `~` expansion. The error names `journal.clone_dir` | config.rs |
| IN-09 | `https://`/`ssh://` userinfo with `:` is rejected. The message points to SSH keys or a credential helper and never echoes the URL | config.rs |
| IN-01 | `du-an-{id}` fallback loops `-2`, `-3`, ... until free | collect.rs |
| IN-02 | `Hôm qua` only when the previous day with entries is `today - 1`, else `Trước đó`. The golden `Today.md` is unchanged (its previous day is yesterday) | render.rs |
| IN-03 | `known_secrets(cfg, config_dir, dashboard_token)` builds the same set as `inputs::collect_secrets`: env, dashboard token, Telegram token next to the map file, engine keys. A test asserts equality with `collect_secrets` for the same inputs | mod.rs |

## Verification (real output)

Task 1 verify (`--lib journal`, then build warnings):
```
test result: ok. 67 passed; 0 failed; 0 ignored; 0 measured; 726 filtered out; finished in 0.41s
0
```
Existing journal integration tests after Task 1 (cargo runs the targets in alphabetical order): journal_export 5, journal_gitsync 16, journal_render 10, journal_sanitize 2, journal_serve 7, journal_tim 4, all `ok`, 0 failed.

Task 2 verify (`--lib journal --test journal_render --test journal_export`; the trailing `journal` filter also applies to the two integration targets):
```
test result: ok. 72 passed; 0 failed; 0 ignored; 0 measured; 726 filtered out; finished in 0.54s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.94s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.15s
```
Because of that filter I also ran the targets without it: journal_export 5, journal_gitsync 16, journal_render 10, journal_serve 7, all passed, 0 failed. Build: 0 warnings.

Task 3 verify:
```
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.49s
```
Full suite (`--no-fail-fast`, summed over `test result` lines), run three times while the 07-11 and 07-12 worktrees were building and testing on the same machine:
```
run 1: passed 1876 failed 1
run 2: no FAILED / panicked line in any target (that run captured only failure lines, not counts)
run 3: passed 1877 failed 0 targets 92
```
1877 = 1858 (the base, per the plan) + 7 new lib tests (state 2, writer 2, config 3) + 12 in journal_safety. Run 1 recorded only the summed counts, so the name of its one failed test was not captured. Run 2 and run 3 had no failure with the same code (no source change between the runs). I count it as a timing flake under the load of three parallel worktrees, but I did not identify the test.

Acceptance greps: `Refused` appears in mod.rs and state.rs. `allow_removals` appears in main.rs and mod.rs and nowhere in loop_.rs. `attention_max_days` appears in config.rs and collect.rs, `is_absolute` in config.rs, and `Trước đó` in render.rs. Cargo.toml and Cargo.lock are unchanged (absent from `git diff --stat 7de1ded HEAD`). rustfmt is clean on every touched journal file and on the new test file. main.rs has 2 rustfmt diffs from the base (lines 467 and 524), not in the lines I changed.

## Each guard test fails without its fix

I made one temporary source mutation per fix with a scratch script, ran `--test journal_safety`, then restored with `git checkout -- <files>`. With all 13 mutations applied, 12 of 12 tests failed, each on its own assertion:

- wiped_db: export pushed 16 files (exit 0, expected 1)
- clock: export pushed (exit 0)
- dotted_slugs: `từ chối ghi đường dẫn không hợp lệ: Dự án/a..b.md`
- export_error: `last_export_ms` None
- dirty / conflict recurrence: 1 mail, expected 2
- carry-over: `[1, 2, 3]`, expected `[2]`
- symlink: `foreign` empty
- clone_dir: relative path accepted
- fallback key: `du-an-9`, expected `du-an-9-2`
- label: `Hôm qua` for Jan 10
- secrets: "dashboard token missing"

A second run applied only the IN-09 and WR-07 closed-status mutations, to show the parts that the first run masked. The credential URL was accepted, and carry-over gave `[2, 3]` (the closed task was carried).

## Deviations from Plan

1. **[Rule 2] Deletion guard also after `prepare`.** The plan puts the guard before `gitsync::prepare`. The guard runs there, and runs again right after `prepare` returns Ready, before `writer::apply`. A fresh clone (or a pull) can bring in the remote's files, and a local-only check misses an empty DB on a new machine. Nothing is written by the exporter before either check.
2. **No `ExportParams` field; `run_export_once_with` instead.** loop_.rs and tests/support build `ExportParams` with struct literals and are owned by other plans. An extra field would have forced edits there. `run_export_once(p)` keeps its signature and means "no override".
3. **Test (c) expectation.** The plan says a slug `a..b`/`foo.` exports as a `du-an-{id}` page. The existing fallback chain is slug -> `slugify(name_vi)` -> `du-an-{id}` (07-02), so the pages are `ab.md` and `foo.md`. The test asserts that, plus `du-an-{id}` when the name also slugifies to nothing (direct `project_file_key` calls). The point of WR-02 (no `..` path, export succeeds) is what fails without the fix.
4. **Test (d) uses a broken brain.db, not an unwritable clone dir.** `DROP TABLE plan_approvals` gives a deterministic `Err` at library level, root-independent and before any git work. With WR-11, a non-directory ancestor is now reported foreign rather than erroring. The plan allowed "e.g." here.
5. **Test (e) conflict at library level, dirty clone end to end.** The CLI-level recurrence test uses a dirty `.gitignore` (same episode mechanism, no offline/online remote dance). The conflict recurrence uses `state::record_outcome` with a real `GitStop::Conflict`.
6. **IN-03: the dashboard token is not available to the CLI.** The token is generated per `serve` run (`auth::new_token`) and never written to the data dir, so `journal export` passes `None`. The shared function is `journal::known_secrets`. `inputs::collect_secrets` (inputs.rs, outside this plan's files) still has its own body. The new test `cli_and_loop_scan_the_same_secrets` asserts that both return the identical set for the same inputs. **Follow-up:** have `inputs::collect_secrets` delegate to `journal::known_secrets(&st.cfg, map_dir, Some(&st.token))` so both share one body.
7. **WR-11 scope, slightly wider:** a target that is itself a symlink or not a regular file is also reported foreign (before, a directory there made the whole export `Err`, and a symlinked target was read through).
8. **Task 1 touched writer.rs and config.rs** (`planned_removals`, `max_remove_fraction`), which the plan lists under Task 2. Both are in `files_modified`.

## Not done / follow-ups (outside this plan's files)

- README (agent-brain/README.md, Nhật ký section) does not yet mention `journal.max_remove_fraction`, `journal.attention_max_days` or `journal export --allow-removals`. README is not in this plan's files.
- `inputs::collect_secrets` delegation (deviation 6).
- The serve loop (`loop_.rs`) prints `Nhật ký: lỗi vòng lặp: ...` for the returned `Err` as before. Backoff now comes from `last_export_ms`.

## Known Stubs

None.

## Threat Flags

None. No new endpoint, auth path or schema. Two `journal_state` keys were added (`alert_episode_ms`, `foreign_episode_ms`) in the existing key/value table.

## Commits

- f9b2470 fix(07-10): guard mass deletion and clock skew, record export failures once, episode-scoped alert keys (WR-01..03)
- 430311d fix(07-10): attention age cap, symlink-safe writer, absolute clone_dir, no credentials in repo_url, key collision, yesterday label (WR-07/11/12, IN-01/02/09)
- 787f250 test(07-10): safety tests for the export guards and config validation

## Self-Check: PASSED

- FOUND: agent-brain/tests/journal_safety.rs, agent-brain/src/journal/{mod,state,writer}.rs
- FOUND: f9b2470, 430311d, 787f250 (`git log 7de1ded..HEAD`)
- STATE.md, ROADMAP.md and REQUIREMENTS.md not modified
