---
phase: 05-embedded-dashboard
plan: 04
subsystem: dashboard
tags: [rust, axum, process-supervision, pdeathsig, api, guards]

requires:
  - phase: 05-embedded-dashboard
    provides: "05-02/05-03 serve, gate, SSE, lock, shutdown path, test helpers"
provides:
  - "jobs::{OUTPUT_LINES, JobSpec, JobView, JobManager::{new, start, get, list, reserved, live_pids, shutdown, kill_all_now}}: ab-spawner thread owns every fork, process_group(0), PR_SET_PDEATHSIG(SIGTERM) + getppid check, waiter threads, 200-line output ring, per-project reservations"
  - "AppState::jobs; router merges api_tasks/api_runs/api_projects/api_stats inside the auth gate"
  - "POST /api/tasks (202 | 409 ambiguous/none/busy/dirty/branch_exists | 422 | 503), GET /api/preflight, GET /api/slug, GET /api/jobs/{id} (+task_id, run_id)"
  - "api_tasks::{MAX_REQUEST_CHARS, NewTask (deny_unknown_fields), LeadPreflight, Refusal, resolve_project, busy_reason, lead_preflight}"
  - "empty routes() in api_runs (05-05), api_stats (05-06), api_projects (05-07)"
  - "D-08 shutdown: SIGINT to every dashboard-started supervisor, wait kill_grace_secs + 5 s, kill_tree stragglers; second signal → kill_all_now + exit 130"
affects: [05-05, 05-06, 05-07, 05-09, 05-10, 05-13]

actuals:
  tasks: 2
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Runs are child `agent-brain run <role> ... -- <project> <request>` processes (argument vector, no shell), never in-process supervise()"
    - "Guards reuse the CLI's own texts (Fallback Ask, dirty tree, branch exists, stacking, E-06)"

key-files:
  created:
    - agent-brain/src/dashboard/jobs.rs
    - agent-brain/src/dashboard/api_tasks.rs
    - agent-brain/src/dashboard/api_runs.rs
    - agent-brain/src/dashboard/api_stats.rs
    - agent-brain/src/dashboard/api_projects.rs
    - agent-brain/tests/dashboard_run.rs
    - agent-brain/tests/dashboard_kill.rs
  modified:
    - agent-brain/src/dashboard/mod.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, --dangerously-skip-permissions approved for Phase 5); reviewed by Claude"
  - "Child-process runs (rated costly, flagged): CLI parity for exit codes, DB writes and refusals; crash safety via PDEATHSIG from one never-exiting spawner thread"

patterns-established:
  - "Every dashboard start goes resolve_project → lead_preflight (busy → dirty → branch_exists) → jobs.start, all inside blocking()"

requirements-completed: [DASH-07]

duration: 25min
completed: 2026-10-17
status: complete
---

# Phase 5 Plan 04: Giao việc on the Server Summary

**A project id plus a Vietnamese request posted to `/api/tasks` starts Lead as a child `agent-brain run`. Its log streams over the 05-02 SSE path and the task stops at planned, with nothing chained. The start is refused, with the CLI's own wording, when:**
- **the project is ambiguous or unknown;**
- **the working tree is dirty;**
- **the task branch already exists;**
- **the project is busy;**
- **a second personal-profile run would start.**

**The profile always comes from `projects_map.json`. When `serve` stops or crashes, its runs stop too and leave no processes behind.**

## Performance

- **Duration:** about 25 min (agy run 800 s, plus review)
- **Completed:** 2026-10-17
- **Tasks:** 2 (both TDD: tests committed first)
- **Files:** 8

## Accomplishments

- **JobManager:** one `ab-spawner` std thread forks every child. Each child gets its own process group, `PR_SET_PDEATHSIG(SIGTERM)`, and a `getppid` check in `pre_exec`. A waiter thread per job reads stdout and stderr into a 200-line ring, waits for the child (so no zombies), records the exit code (128 + signal when killed by one) and releases the project reservation.
- **Start path:** `POST /api/tasks` handles everything in `blocking`:
  1. check the request is non-empty and ≤ 20,000 chars;
  2. slug from `routing::slugify`;
  3. `routing::resolve` must give exactly one project, otherwise 409 with candidates `{id, name_vi, path, profile}` and the Fallback Ask header;
  4. `require_runnable`;
  5. the preflight (busy, then dirty, then branch exists);
  6. `jobs.start` with `["run","lead","--name",slug,"--",id,request]`.
- **Busy rules:** live runs from the DB (pid and start time checked with `procs::alive`) or a dashboard reservation make a project busy. A personal-profile run or reservation blocks any other personal project, with the exact CLI E-06 text.

## Task Commits

1. `4f57a35` test(05-04): tests for giao việc happy path, slug and job status (agy)
2. `547ea5d` feat(05-04): giao việc via child agent-brain run with job manager (agy)
3. `a378971` test(05-04): tests for preflight, busy rules, resolve gate and serve kill (agy)
4. `cd48f9e` feat(05-04): preflight, busy rules and resolve gate for giao việc (agy)

## Deviations from Plan

None.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- **Scope:** the 4 commits touch exactly the 8 declared files. `tests/common/mod.rs` is unchanged.
- **Tests:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1` exits 0 with 195 passed and 0 failed. That is the 178 before plus 17 new: 12 in `dashboard_run`, 3 in `dashboard_kill` and 2 unit tests in `dashboard::jobs`. All named behaviours are OK, including `dash04_assign_lead_from_api`, `dash07_ambiguous_or_unknown_project_never_runs`, `d04_project_busy_refused`, `d17_second_personal_run_refused` and all three `d08_*` tests. agy's report of "all 96 tests" was wrong; the reviewer count above is authoritative.
- **No leftovers:** a `ps` check before and after the full run shows no `fake_claude*`, python sleep or `serve` process left behind.
- **Other checks:** the Python suite is OK and `cargo fmt --check` exits 0.
- **Grep gates:**
  - `PR_SET_PDEATHSIG` and `process_group(0)` are present, and so is the `"--"` separator.
  - No `Command::new("sh"|"bash")` and no `"checkout"|"switch"|"stash"|"reset"|"commit"` string appears in `src/dashboard`.
  - The E-06 text and `deny_unknown_fields` are present.
- **Code read:** `jobs.rs`, `api_tasks.rs` and the `mod.rs` shutdown diff were read in full against Pattern 3/6/7 and the HTTP contract.
- **Secrets:** the pattern scan only hit test task names like `task-p1`, which match the `sk-` pattern. No real secrets.

**Non-blocking notes:**

- **Shutdown race:** a child spawned in the gap between `start()` releasing the manager lock and inserting the job can miss `shutdown()`'s SIGINT sweep. PDEATHSIG still sends it SIGTERM when `serve` exits, so D-08 holds.
- **Spawn failures labelled `busy`:** a failed spawn ("Không chạy được agent-brain" / "Binary đã bị thay thế") is answered 409 `kind: busy`, which the plan's "start refusal → 409 busy" wording allows. The UI shows the server text verbatim, so the user still sees the real reason.
- **Requests during shutdown grace:** while the job-shutdown grace runs, the HTTP server still answers. New starts are refused with 503 by `JobManager`.

## Next Phase Readiness

Wave 5 (05-05 task states and Kill, 05-06 statistics, 05-07 projects API) can start. Each plan owns a disjoint route module, created empty here.
