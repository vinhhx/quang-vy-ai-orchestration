---
phase: 05-embedded-dashboard
plan: 05
subsystem: dashboard
tags: [rust, axum, task-state, pipeline, kill, plan-view]

requires:
  - phase: 05-embedded-dashboard
    provides: "05-04 JobManager, api_tasks routes, resolve_project/busy_reason, empty api_runs"
provides:
  - "taskstate::{GitFacts, TaskFacts, StepView, ActionView, TaskView, step_state, derive}: pure derivation of steps (cho|dang_chay|xong|vuong_mac|can_sua|vi_pham|killed|loi), banner, group (running|waiting|attention|done), current step and actions (run_dev|run_qa|relead|kill) with the CLI's refusal texts"
  - "GET /api/tasks?profile=&limit= (profile filter, busy facts), GET /api/tasks/{id} (plan + QA report as data, worktree or branch source, plan commit, dirty flag, runs, job)"
  - "POST /api/tasks/{id}/dev|qa|relead → 202 job | 409 not_allowed/busy"
  - "POST /api/runs/{id}/kill via runner::cmd_kill; GET /api/runs/{id}; GET /api/runs/{id}/events/{seq}/raw"
  - "api_tasks::busy_reason_for_task (skips the task's own run/job so its running step shows Kill)"
  - "roles/lead.rs: Lead exit 3 records runs.outcome = protocol_violation"
affects: [05-08, 05-10, 05-13]

actuals:
  tasks: 3
  commits: 6

tech-stack:
  added: []
  patterns:
    - "Step state is derived from runs + tasks.status + git facts on every read; nothing is stored"
    - "Plan/QA files are read from the worktree when the task branch is checked out, else `git show <branch>:<file>` with the branch from the DB"

key-files:
  created:
    - agent-brain/src/dashboard/taskstate.rs
    - agent-brain/tests/dashboard_pipeline.rs
  modified:
    - agent-brain/src/dashboard/mod.rs
    - agent-brain/src/dashboard/api_tasks.rs
    - agent-brain/src/dashboard/api_runs.rs
    - agent-brain/src/roles/lead.rs
    - agent-brain/tests/cli_lead.rs
    - agent-brain/tests/dashboard_kill.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, --dangerously-skip-permissions approved for Phase 5); reviewed by Claude"
  - "Lead exit-3 outcome write accepted as the one CLI-side behaviour change of Phase 5 (research Open Question 1)"

patterns-established:
  - "Every dashboard action is an enabled ActionView derived from the CLI's own preconditions; the server only starts `agent-brain run` children and kills through runner::cmd_kill"

requirements-completed: [DASH-06]

duration: 30min
completed: 2026-10-17
status: complete
---

# Phase 5 Plan 05: Task Pipeline and Kill Summary

**The API now drives a task from Lead to Dev to QA step by step:**
- **Derived states:** each step's state, banner, group and next actions are worked out from the CLI's own records. The refusal texts match the CLI word for word.
- **Plan and QA as data:** both are returned as data, and a plan commit made in the user's editor is picked up on the next read.
- **Kill:** it stops runs started from the dashboard and from a terminal through `runner::cmd_kill`, with no survivors.
- **Lead violations:** a Lead protocol violation is now recorded in the DB.

## Performance

- **Duration:** about 30 min (agy run 1218 s, plus review)
- **Completed:** 2026-10-17
- **Tasks:** 3 (all TDD: tests committed first)
- **Files:** 8

## Accomplishments

- **Task states:** `taskstate.rs` is pure, with no rusqlite, gitops, process or clock calls (checked by grep). It has 7 unit tests covering the step, banner, group, current-step and action tables.
- **Task detail:** `GET /api/tasks/{id}` reads `.plan.md` and `.qa-report.md` from the worktree when the task branch is checked out, and otherwise with `git show <tasks.branch>:<file>`. It finds the `<slug>: plan` commit on the task branch and computes whether code changed after the plan. Validation errors are reported only for the worktree source.
- **Starts:** `POST /api/tasks/{id}/dev|qa|relead` rebuilds the facts, then starts `["run","dev","--",project]`, `["run","qa","--",project]` or `["run","lead","--name",slug,"--",project,title]` only when the matching action exists and is enabled.
- **Kill:** opens a fresh context and calls `runner::cmd_kill(task_id)`, the same path as `agent-brain kill N`. A task-less run gets 409 "không thuộc task nào", and a finished run gets 409 "không còn chạy".

## Task Commits

1. `f1d7427` test(05-05): assert lead protocol violation sets outcome in DB (agy)
2. `3a402d2` feat(05-05): derive task steps and next actions; record lead protocol violations (agy)
3. `7e14a6f` test(05-05): add dashboard pipeline and task management tests (agy)
4. `156c13e` feat(05-05): task list/detail and dev/qa/relead from the dashboard (agy)
5. `fc407b3` test(05-05): add dashboard kill and raw log tests (agy)
6. `d1f124f` feat(05-05): kill and raw log endpoints (agy)

## Deviations from Plan

- **A deliberate CLI behaviour change** (required by the plan notes; agy listed it too):
  - **File:** `agent-brain/src/roles/lead.rs`.
  - **Change:** when Lead edits files outside `.plan.md` (exit 3), it now calls `db::set_run_outcome(.., "protocol_violation")` right before returning `EXIT_PROTOCOL`.
  - **Reason:** parity with Dev and QA, so a Lead violation from a terminal run shows in the dashboard as "Vi phạm giao thức".
  - **Unchanged:** exit codes (3) and every CLI message. This is the only CLI-side behaviour change in Phase 5. `tests/cli_lead.rs` asserts the new outcome.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- **Scope:** the 6 commits touch exactly the 8 declared files. `mod.rs` gains only `pub mod taskstate;`, and `lead.rs` gains one line.
- **Tests:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1` exits 0 with 215 passed and 0 failed. That's the 195 before plus 20 new: 7 taskstate, 8 pipeline and 5 kill tests. The Python suite is OK and `cargo fmt --check` exits 0. A `ps` check found no leftover fake, sleep or serve processes.
- **Grep gates:**
  - `pub fn derive(` appears once.
  - The purity grep finds 0 hits, and `protocol_violation` appears once in lead.rs.
  - `taskstate::derive` appears 3 times in api_tasks and `runner::cmd_kill` once in api_runs.
  - There are no `fs::write`, `commit_paths` or `File::create` calls in api_tasks, and no `libc::kill`, `SIGKILL` or `signal_group` in api_runs or api_tasks.
- **Code read:** `taskstate.rs` (derive and builders) was read in full. Each refusal text was compared with the CLI source: dev.rs:91 dirty, qa.rs:68 dirty, lead.rs:87 dirty, the branch and plan texts. The git reads in `api_tasks.rs` use argument vectors with `tasks.branch` from the DB.
- **Secrets:** the pattern scan is clean.

**Non-blocking notes:**

- **A stuck `open` task:** a task whose Lead `timed_out` gets no action. The plan's list (failed, killed, lost, violation) leaves out `timed_out`, so the UI offers no "Chạy lại Lead" there, but the CLI can still re-plan. Worth adding when the UI plans are revised.
- **Duplicated code:** the git-facts and plan-reading block appears twice in `api_tasks.rs`, once in the detail handler and once in the action handler (around lines 794 and 1079). It works, but a later edit could change one copy and not the other.

## Next Phase Readiness

05-06 (statistics) and 05-07 (projects API) can run next. They own disjoint files (`stats.rs`/`api_stats.rs` and `routing.rs`/`api_projects.rs`).
