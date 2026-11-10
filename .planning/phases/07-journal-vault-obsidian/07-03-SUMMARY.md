---
phase: 07-journal-vault-obsidian
plan: 03
subsystem: journal
tags: [rust, obsidian, journal, attention, golden-tests]
requires:
  - phase: 07-02
    provides: journal model/collect/render skeleton, tracer day file
provides:
  - "journal::collect::classify_as_of (as-of-day status, D-16 precedence)"
  - "approvals as activity, Model.attention per day, carry-over into today"
  - "render: attention section, Today.md, month, year and project files"
  - "golden test fixture (synthetic) with UPDATE_GOLDEN=1 refresh"
affects: [07-06, 07-07]
key-files:
  created:
    - agent-brain/tests/journal_render.rs
    - agent-brain/tests/fixtures/journal/golden/ (10 files)
  modified:
    - agent-brain/src/journal/collect.rs
    - agent-brain/src/journal/render.rs
decisions:
  - "model.rs left unchanged: the 07-02 public types already carry everything the new rules need"
status: complete
commits: 2
plan_head_before: cad033c827859cf9837ab8a19642903e0dfa9389
plan_head_after: d1a145d
actuals:
  tokens: 24000
  tasks: 2
  commits: 2
requirements-completed: [JRN-01, JRN-02]
completed: 2026-11-10
---

# Phase 7 Plan 03: Full journal content Summary

The exporter now produces every file kind of D-02/D-17 (day, Today, month, year, project pages) with the `Cần anh xử lý` attention list first in each day file, derived only from runs, tasks and plan_approvals rows.

## Commits

- `84dd2ab` feat(07-03): D-16 attention rules, approvals, carry-over to today (collect.rs, tests/journal_render.rs)
- `d1a145d` feat(07-03): Today, month, year and project pages + golden tests (render.rs, tests, golden files)

## What was built

- `collect::classify_as_of(&LastRun, approvals, task_status, is_last_overall, day_end_ms)`: pure classification of a task's last run before the end of a day, in the plan's order (quota, env:down, protocol_violation, blocked, qa:fix, killed, running, timed_out, failed/lost, qa:pass, analyst answered, plan gate, success status, active). `LastRun::from_row` adapts `RunRow`. This replaces the 07-02 private `day_status` (which mapped any succeeded step to `xong`).
- `collect()` reads `plan_approvals` once inside the existing read transaction plus `tasks.approved_at`; a day with an approval is an active day of the task; attention per past day comes from that day's task statuses; today's list also carries unresolved earlier tasks (`AttentionItem.day` = day of their last run), sorted by (category, day, task_id).
- `render_all`: attention section between heading and first group (` · từ DD/MM` for carried items); today's file is rendered even when it only has carry-over items; `Today.md`, `Nhật ký/YYYY/MM/YYYY-MM.md`, `Nhật ký/YYYY/YYYY.md`, `Dự án/<slug>.md` per the plan's formats; links only to files in the result set; no weekly file.

## Verification (real output)

- `cargo test --offline --locked --test journal_render --test journal_export -- --test-threads=1`: journal_export 5 passed; journal_render 10 passed (`attention_by_day`, `killed_and_approved_are_not_attention`, `groups_from_journal_group_only`, `approval_days_are_listed`, `golden_files_match`, `day_file_has_attention_first`, `today_embeds_today`, `all_links_resolve`, `no_weekly_file_and_nfc_paths`, `render_is_deterministic`), 0 failed.
- `cargo test --offline --locked --lib journal -- --test-threads=1`: 27 passed, 0 failed, including `journal::collect::tests::quota_precedes_failed_and_violation_precedes_gate` and `journal::render::tests::project_page_folds_old_months`.
- Acceptance greps: `fn classify_as_of` once in collect.rs; `plan_approvals` present; no `planfile|protocol_paths|qa-report|run_dir` in collect.rs/model.rs code; golden dir has no `title:`/`request:`; `Cần anh xử lý` count 1 in the 15/01 golden file; all 10 golden files committed (git status clean).
- All golden files were generated once with `UPDATE_GOLDEN=1`, then read by eye against the plan's formats before committing.

## Deviations from Plan

### Auto-fixed / process deviations

**1. [Process] `cargo fmt --check` not used as a gate.** The base repo is not rustfmt-clean (about 100 files differ), so the plan's `cargo fmt --check` clause was replaced by formatting only my files through `rustfmt --edition 2021 < file` (stdin mode, so no child modules are touched): collect.rs, render.rs, tests/journal_render.rs. Never ran `cargo fmt` on the crate. Golden files are generated text, not Rust.

**2. [Process] RED step not run separately.** I wrote the implementation of Task 1 before running the new tests, so the "confirm they fail on the 07-02 code" step was not executed; the tests were run green. Test expectations come straight from the plan's behavior list (e.g. the six-item 15/01 list, task 11 not carried).

**3. [Plan note] model.rs not modified.** It is listed in `files_modified`, but no change was needed (public types unchanged, as the plan requires).

**4. [Process] Plan commit ledger file not written.** The harness refused writes into the shared `.git/worktrees/...` path, so `plan_head_before` was taken from the known worktree base `cad033c`; `commits: 2` equals `git rev-list --count cad033c..HEAD` before the docs commit.

Otherwise the plan was executed as written.

## Notes for downstream plans

- `Today.md` is always rendered (even with no tasks: "Hôm nay chưa có việc nào."), so the key set of `render_all` always contains it.
- The sanitizer drops `!` and other markup punctuation (07-02 constant `ALLOWED_PUNCT`); fixture titles are plain text, so 07-04's pattern rules cannot change the golden bytes.
- Month/year/project counts: distinct tasks active in the period; `xong` = status on the task's last active day of the period is Done; `Dự án chính` = top 3 projects by task count in the month (count desc, slug asc).
- A succeeded dev run without a `qa:pass` is now `đang làm` (07-02 labelled it `xong`); `xong` requires `qa:pass`, an analyst answer, or a last run with a success task status.

## Known Stubs

None.

## Threat Flags

None. T-07-12 (no plan/report reads) holds per the acceptance grep; T-07-13 by `all_links_resolve`; T-07-14 by the golden and determinism checks on `title:`/`request:`.

## Self-Check: PASSED

- agent-brain/tests/journal_render.rs, 10 golden files, collect.rs and render.rs exist and are committed.
- Commits `84dd2ab` and `d1a145d` are ancestors of HEAD.
