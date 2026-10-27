---
phase: 06-multi-engine-media-branch
plan: 13
subsystem: dashboard-stats-pipeline
status: complete
tags: [rust, axum, stats, pipeline, engines, sqlite]

requires:
  - phase: 06-multi-engine-media-branch
    provides: "06-03 engines catalog and runs.engine, 06-04 insert_run_with_engine / insert_human_run, 06-05 engine-first pricing and is_priced, 06-07 quota outcome, 06-10 teams on tasks and the het_quota step state"
provides:
  - "stats::EngineStats, stats::QaRate, query_by_engine(conn, profile, since_ms, pricing), query_qa_by_dev_engine(conn, profile, since_ms)"
  - "GET /api/stats fields engines[{engine, runs, input, output, cache_read, cache_write, cost, priced}] and qa_by_dev_engine[{engine, qa_fix, qa_total}]"
  - "pipeline::FlowCard.engine / engine_planned, LedgerRow.engine / human, LedgerTotal.human_runs, engine_text_of(), set_planned_engines(), state words het_quota and cho_nguoi"
affects: [06-17, 06-18]

actuals:
  tokens: 60000
  tasks: 3
  commits: 3
plan_head_before: 2f330a266f3b0737b6996887967675f0af628379
plan_head_after: 202b24db16b78ddfba47964e804e93e38bdb7470

tech-stack:
  added: []
  patterns:
    - "Stats rows are produced by one bound-parameter SQL per breakdown, with the same (profile, since) filter as the existing totals"
    - "Planned card engines are filled after derive_response by set_planned_engines, so the FlowFacts test literals stay unchanged"

key-files:
  created: []
  modified:
    - agent-brain/src/stats.rs
    - agent-brain/src/dashboard/api_stats.rs
    - agent-brain/src/dashboard/pipeline.rs
    - agent-brain/src/dashboard/api_tasks.rs
    - agent-brain/tests/dashboard_stats.rs
    - agent-brain/tests/db_stats.rs
    - agent-brain/tests/dashboard_pipeline.rs

key-decisions:
  - "Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6)."
  - "A person's ledger row is left out of total.cost_unknown as well as out of total.duration_ms: a person has no price, so the missing cost is not an unknown one. The plan only named the minutes."
  - "engine_text_of accepts the staff id from metrics_json only when it passes staff::validate_id; otherwise a human run reads as the bare engine người."
  - "A card's engine comes from the last run matching the card (any cycle), so a card that only has runs from an earlier cycle still names the engine that ran it."
  - "set_planned_engines maps card keys to roles (plan -> lead, plan_review, code and fix -> dev, code_review, qa) and also accepts the stage spellings from the plan text; an unknown key is left without an engine."

requirements-completed: [ENG-04, ENG-07, HUM-03]

coverage:
  - id: D1
    description: "GET /api/stats splits tokens and cost by engine with a priced flag; unpriced engines are flagged, not shown as free"
    requirement: ENG-04
    verification:
      - kind: integration
        ref: "agent-brain/tests/dashboard_stats.rs#eng04_engines_split_and_priced"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/dashboard_stats.rs#eng04_engines_follow_filters"
        status: pass
      - kind: unit
        ref: "agent-brain/src/stats.rs#query_by_engine_groups_and_sorts"
        status: pass
    human_judgment: false
  - id: D2
    description: "QA Cần sửa rate by the Dev engine under test as integer counts, human runs included, boundaries per the probe"
    requirement: ENG-07
    verification:
      - kind: integration
        ref: "agent-brain/tests/db_stats.rs#eng07_qa_rate_by_dev_engine"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/db_stats.rs#eng07_rate_boundaries_and_filters"
        status: pass
      - kind: unit
        ref: "agent-brain/src/stats.rs#qa_rate_attribution"
        status: pass
    human_judgment: false
  - id: D3
    description: "Pipeline cards and ledger rows carry the engine (actual or planned), human rows read người:{staff} with no tokens or cost, the total counts human runs, quota reads hết quota"
    requirement: HUM-03
    verification:
      - kind: integration
        ref: "agent-brain/tests/dashboard_pipeline.rs#pipeline_engines_on_cards_and_ledger"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/dashboard_pipeline.rs#pipeline_legacy_planned_claude"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/dashboard_pipeline.rs#pipeline_human_run_row_and_total"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/dashboard_pipeline.rs#pipeline_quota_state"
        status: pass
      - kind: unit
        ref: "agent-brain/src/dashboard/pipeline.rs#ledger_engine_and_human_rows"
        status: pass
    human_judgment: false
---

# Phase 6 Plan 13: Engines in stats and on the pipeline Summary

**Per-engine token/cost split with a `priced` flag, QA "Cần sửa" rate by Dev engine (people included), and engine fields on every pipeline card and ledger row, including planned engines from the task's team.**

Executor: a Claude gsd-executor subagent in an isolated worktree, in place of agy (the user chose this for Phase 6).

## Accomplishments

- Task 1 (tracer, `def1a7d`): `stats::query_by_engine` groups runs by `runs.engine`, flags `priced` through `pricing::is_priced`, sorts by input + output descending then engine name. `GET /api/stats` gains `engines`. The HTTP tests were run RED first (no `engines` key), then green. Existing total/by_project/by_role/by_day/by_profile fields are untouched, and the existing assertions in `dashboard_stats.rs` have no removed lines.
- Task 2 (`bb229b8`): `stats::query_qa_by_dev_engine` attributes each `qa:pass` / `qa:fix` verdict to the engine of the latest Dev run of the same task with a smaller run id (`d.id < q.id`), so a person's run whose `started_at` is the start of the wait is still attributed correctly. QA runs with no earlier Dev run, and verdicts other than pass/fix, are not counted; an engine with no verdict has no row. `GET /api/stats` gains `qa_by_dev_engine` as integer counts.
- Task 3 (`202b24d`): `FlowCard.engine` / `engine_planned`, `LedgerRow.engine` / `human`, `LedgerTotal.human_runs`; `engine_text_of`, `set_planned_engines`, and the `het_quota` / `cho_nguoi` words in `state_word`. `get_pipeline` builds the role to engine map from the task's team (`claude` for every role when the task has no team or the team is no longer in teams.json) and calls `set_planned_engines` after `derive_response`. Only the `get_pipeline` handler was touched in `api_tasks.rs`; `taskstate.rs` was not touched (06-10 already maps `quota` to `het_quota`).

## Verification

- `cargo test --offline --locked --test dashboard_stats --test db_stats`: pass (10 + 8).
- `cargo test --offline --locked --lib stats`: pass (4).
- `cargo test --offline --locked --test dashboard_pipeline --test dashboard_actions --test task_team`: pass (22 + 14 + 17).
- Full suite `cargo test --offline --locked -- --test-threads=1`: no failures, including `contract_guard`.
- `cargo fmt --check`: clean. `git diff 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock`: empty.
- Acceptance greps for all three tasks pass (`query_by_engine`, `"engines"`, `query_qa_by_dev_engine`, `d.id < q.id`, `"qa_by_dev_engine"`, `engine_planned` count 10, `set_planned_engines` in both files, the two state-word arms).

## Deviations from Plan

None - plan executed exactly as written. Two small choices the plan left open are listed under key-decisions (human rows excluded from `cost_unknown`; staff id validated before use).

## Known Stubs

None.

## Threat Flags

None. T-06-52: all new SQL uses bound parameters `?1` / `?2`. T-06-53: costs are the stored engine-first `runs.cost_usd`, unpriced engines carry `priced: false`, covered by tests. T-06-54: only a validated staff id enters pipeline text. T-06-SC: no dependency added.

## Notes for later plans

- 06-17 / 06-18 read `engines[].priced` and `qa_by_dev_engine[]` (compute `round(100 * qa_fix / qa_total)` in the UI) and the pipeline `engine` / `engine_planned` / `human` / `human_runs` fields.
- `cho_nguoi` has a word but nothing produces that state yet; it is for the human engine plan (06-12), which is not merged.

## Self-Check: PASSED

- Files: stats.rs, api_stats.rs, pipeline.rs, api_tasks.rs, dashboard_stats.rs, db_stats.rs, dashboard_pipeline.rs all modified and committed.
- Commits: def1a7d, bb229b8, 202b24d are ancestors of HEAD; `git rev-list --count 2f330a2..HEAD` was 3 before this summary.
