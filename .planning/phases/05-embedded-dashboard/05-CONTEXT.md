# Phase 5: Embedded Dashboard - Context

**Gathered:** 2026-10-17
**Status:** Ready for planning

<domain>
## Phase Boundary

A web dashboard served by the existing `agent-brain` binary, which stays one static file (DASH-01..DASH-12). From the browser the user can:

- see tasks and live logs;
- assign work in Vietnamese and walk it through Lead → Dev → QA;
- Kill a run;
- read token statistics;
- manage the project list.

The server listens only on `127.0.0.1` and needs an access token. The CSS works offline. The dashboard reuses the Phase 3 core (routing, roles, runner, kill, SQLite, stats, projects) and does not rewrite its logic.

Not in this phase (spec `files/05-phase4-dashboard.md` §2): multiple users or logins, remote or internet access, and new orchestration behaviour that the CLI doesn't have.

</domain>

<decisions>
## Implementation Decisions

### Carried forward (not re-discussed)
- **D-00a:** The stack is Axum + SSE, with HTML + Tailwind + Alpine.js embedded in the binary, served from the same origin as the API (spec §2, §3.1; DASH-01).
- **D-00b:** Security comes from listening only on `127.0.0.1` plus a token generated at startup and printed in the terminal (DASH-02, DASH-03).
- **D-00c:** The API surface is spec §4.2. Live logs work like this: `stream.rs` writes to SQLite `events` and also publishes on a broadcast channel; SSE carries the channel. When a page reloads, it reads the old events from SQLite first and then continues on SSE (DASH-05).
- **D-00d:** The Phase 3 rules all still apply:
  - `projects_map.json` is the source of truth (03 D-05);
  - the DB is `~/.local/share/agent-brain/brain.db` (03 D-06);
  - a task is a `task/<slug>` branch (03 D-07);
  - task IDs are short integers (03 D-08);
  - the S4 kill method with a configurable grace period (03 D-09..D-12);
  - one static musl x86_64 binary (03 D-04).
- **D-00e:** The CSS is Tailwind, built ahead of time and embedded. Nothing loads from a CDN at runtime (DASH-09).

### Pipeline from the UI
- **D-01:** After Lead finishes a task assigned from the UI, the task **pauses in a "Chờ duyệt plan" state** (waiting for plan review). The UI renders `.plan.md`, and Dev starts only when the user presses **"Chạy Dev"**. After Dev, **"Chạy QA"** works the same way. Nothing chains automatically.
- **D-02:** **The UI only shows `.plan.md`; it cannot edit it.** To change the plan, the user edits the file in their own editor. The task screen shows the file path with a copy button and picks up a new `<task>: plan` commit once it lands. The UI never writes plan commits itself.
- **D-03:** When Dev stops with **Vướng mắc** (blocked, exit 4) or QA concludes **Cần sửa** (needs fixes), the task screen highlights that section or the QA report. It offers **exactly the next steps the CLI already prints**, such as re-plan through Lead or rerun Dev, as buttons. There is no new automation and no automatic retry.
- **D-04:** **One run per project at a time**, whether it was started from the UI or the terminal. While the project has a run in progress, the start buttons are disabled and the reason is shown. Each task is a git branch in the same working tree, so two concurrent runs would fight over checkout. Nothing is queued.
- **D-05:** **The project picker is an autocomplete field.** Typing a name, alias or accent-free name shows matching projects below the field, using the CLI's matching (`routing::normalize` / `resolve`). The user then selects one. **A task can only start once exactly one project is selected**, so an ambiguous name can never start a run (DASH-07).
- **D-06:** **Base branch and dirty working tree follow the CLI rules, shown clearly in the UI.** A dirty working tree means the task is refused, with the reason given. A non-main base branch is allowed, but the CLI's stacking warning is shown before the task starts. The UI never switches branches.
- **D-07:** **The task name (slug) is pre-filled from the request**, generated the same way as the CLI default, and **the user can edit it** before starting.
- **D-08:** **When the server stops** (Ctrl+C on `serve`, or a crash), **the runs it started stop too.** They go through the same path as Ctrl+C on a CLI run (SIGTERM → grace → SIGKILL, status `killed`), and the existing stale-run reconciliation handles a crash. No run is left going on its own.

### Starting & reaching it
- **D-09:** **`agent-brain serve`** is a new subcommand that runs in the foreground of a terminal. It prints the URL and token at startup, and Ctrl+C stops it along with its runs (D-08). The other CLI commands are unchanged.
- **D-10:** **The port has a fixed default and can be changed.** It binds `127.0.0.1:<default>`, for example 7878; the exact default is up to the researcher and planner. It can be overridden with `--port` and a config key such as `dashboard_port`. If the port is already in use, the server prints a clear error and exits with a non-zero code.
- **D-11:** **The token is passed in the URL once and then kept in a cookie.** The terminal prints `http://127.0.0.1:<port>/?token=…`. The first visit sets an `HttpOnly` cookie restricted to the same site, then redirects to the URL without the token. **A new token is generated at every start**, so old cookies stop working. Every API and SSE request without a valid token is refused (DASH-03).
- **D-12:** **Runs started from the terminal (`agent-brain run …`) appear live in the dashboard and can be killed from it.** Both write the same SQLite tables, so the UI tails their `events` from the DB. **This means the live view cannot rely only on the in-process broadcast; it must also follow new `events` rows in SQLite.** Killing from the UI uses the same path as `agent-brain kill N`.
- **D-13:** **The server only prints the URL and never opens a browser itself** (no xdg-open).
- **D-14:** **Only one `serve` can run at a time.** A second `serve` finds the port taken, or finds the running instance, and refuses with a message pointing to it.

### Profile switcher
- **D-15:** **The profile switcher only filters the view.** Its options are Tất cả (all), work and personal. It filters the task lists, the statistics and the project picker. **A new task always runs on the profile set for its project in `projects_map.json`;** the UI cannot override it, and the Projects screen does not edit `profile`. This satisfies DASH-08 by filtering what you see.
- **D-16:** **The switcher starts on "Tất cả"** and remembers the user's choice per browser in local storage.
- **D-17:** **A second personal-profile run is refused**, as the CLI does (rule E-06: never two personal runs at once). The start button is disabled and shows the reason, for example "Profile personal đang có run chạy" (a personal-profile run is already in progress). Nothing is queued.

### Look, language, Figma
- **D-18:** **There is no Figma design, so DASH-10 does not apply.** `/gsd-ui-phase 5` can still be used to make a UI-SPEC before planning.
- **D-19:** **The interface text is Vietnamese**, matching the CLI messages and the protocol: Giao việc (assign work), Vướng mắc (blocked), Đạt / Cần sửa (pass / needs fixes), Chờ duyệt plan (waiting for plan review). Technical terms stay as they are: task, run, token, Kill, Lead/Dev/QA.
- **D-20:** **The style is a friendly app** rather than a dense developer tool: cards, generous spacing, and a visual pipeline.
- **D-21:** **The theme follows the system** (`prefers-color-scheme`). There's also a manual light/dark toggle, remembered per browser.
- **D-22:** **The log pane shows readable steps.** Each event is one row with the same content as the CLI line: time, an icon for tool / assistant / result / system, and short text. Commands and paths are monospace. The pane scrolls automatically and **pauses** when the user scrolls up, with a way to jump back to live. Clicking a row shows its raw JSON (`events.raw_json`).
- **D-23:** **The pipeline is a horizontal stepper** with three steps, Lead → Dev → QA. Each step shows its state: chờ (waiting), đang chạy (running), xong (done), Vướng mắc, Cần sửa or killed. It also shows that run's tokens and duration. The action buttons (Chạy Dev, Chạy QA, Kill, and the D-03 next steps) sit on the current step.

### Claude's Discretion
- The exact default port, the cookie name and attributes, the token length, and how the server finds an already-running instance (D-10, D-11, D-14).
- How terminal-started runs are followed in SQLite (polling interval or notifications) and how that merges with the in-process broadcast (D-12).
- How the offline Tailwind build fits into the static musl build and `scripts/build-release.sh` with no network, for example committing the built CSS or using a standalone Tailwind binary at dev time (DASH-09). The researcher must check this against the "offline, locked" build rule.
- How each screen is split into files, keeping "one screen per file" as spec §7 asks; the Alpine.js structure; the charting approach for Thống kê (the stats screen), which must be embedded and offline.
- How the server runs roles: in the same process through `roles::execute_role` / `runner::supervise`, or as a child `agent-brain run` process. The choice must keep the S4 kill guarantees and D-08.
- The Projects screen (DASH-12) details: editing aliases writes to `projects_map.json` with the existing `projects::save_map`, and "quét lại" (rescan) calls `projects::scan`. How to handle conflicts with hand edits and duplicate aliases should mirror `agent-brain projects scan`.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope and acceptance
- `files/05-phase4-dashboard.md`: the source spec. Screens §4.1, API §4.2, live-log flow §4.3, tasks §5, test cases P4-01..P4-10 §6, risks §7.
- `.planning/REQUIREMENTS.md`: DASH-01..DASH-12, plus the out-of-scope table (remote access, Tailwind CDN).
- `.planning/ROADMAP.md`, the "Phase 5: Embedded Dashboard" section: 5 success criteria.

### Behaviour the UI must match (Phase 3 core)
- `.planning/phases/03-rust-cli-core/03-CONTEXT.md`: D-04..D-16 (static build, data paths, task = branch, task IDs, kill and Ctrl+C semantics, status/inspect/stats).
- `.planning/phases/03-rust-cli-core/03-VERIFICATION.md`: what was proven live (kill tree, profiles, routing, Fallback Ask).
- `agent-brain/README.md`: commands, exit codes 0/1/2/3/4/130, config keys, profiles, kill semantics.
- `agent-brain/LIVE-CHECKS.md`: the live kill method on claude 2.1.301 (python sleep), reused for P4-06.
- `spikes/spike-results.md` S4: the kill method (record descendants, SIGTERM, sweep). The Bash tool's shell runs in its own PGID and SID.

### Project rules
- `CLAUDE.md`: Claude plans and reviews, agy executes, and how agy is called.
- `.planning/PROJECT.md` Key Decisions: "Nhúng UI vào binary, localhost + token" (embed the UI in the binary, localhost + token) and "Giao thức trước, giao diện sau" (protocol before interface).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `routing::normalize`, `routing::resolve`, `projects::select_project`: accent-insensitive matching and Fallback Ask, for the autocomplete (D-05) and DASH-07.
- `projects::load_map` / `save_map` / `scan` / `find_duplicates` / `projects_of`: the Projects screen (DASH-12).
- `roles::cmd_run` / `roles::execute_role` / `roles::build_command`, `runner::supervise(req, &mut dyn RunObserver)`: start Lead/Dev/QA. **The `RunObserver` trait is the natural point to publish each event to the SSE broadcast.**
- `runner::cmd_kill`, `procs::kill_tree` / `descendants` / `signal_group`: the Kill button (DASH-06, D-12).
- `runner::reconcile_stale_runs`: marks runs `lost` on startup or after a crash (D-08).
- `stats::query_report` / `query_stats` / `parse_since`: the Thống kê screen and the profile filter (DASH-08, DASH-11).
- `fastpath::cmd_status` logic: `GET /api/projects/:id/status` (0 tokens).
- `db.rs`: tables `projects`, `tasks`, `runs`, `events(run_id, seq, ts, kind, raw_json)`, plus `insert_event`, `get_task`, `find_task_by_branch`, `set_task_status`, for history and catch-up reads (DASH-05).
- `stream.rs` `StreamEvent` / `classify` / `feed` and the `term.rs` line formatter: one readable row per event (D-22) with the same wording as the CLI.
- `planfile.rs`: parse `.plan.md` / `.qa-report.md` to render the plan, the Vướng mắc section and the QA conclusion (D-01, D-03).

### Established Patterns
- All user-facing messages are Vietnamese (`BrainError` messages, `term.rs`).
- Exit codes: 0, 1, 2 (Fallback Ask), 3 (protocol violation), 4 (Vướng mắc), 130 (stopped). The UI maps them to step states (D-23).
- Dependencies are pinned exactly (`=x.y.z`) and builds are `--offline --locked`. Adding Axum (and tokio's `net` feature) means a vendored or fetched lockfile update, which is a user checkpoint, as in 03-01.
- Tests use temp HOMEs and `tests/fakes/fake_claude.py`, never the real CLI.

### Integration Points
- `main.rs` `Commands`: add `Serve { port }` (D-09).
- `config.rs`: add a key for the dashboard port (D-10).
- `tokio` is currently built without `net`. Axum and the HTTP server need it, which affects the static musl build and `scripts/build-release.sh` (DIST-01 must still pass).
- The terminal and the dashboard share `brain.db`, so concurrent writers need SQLite busy-timeout/WAL behaviour, which the researcher should check (D-12).

</code_context>

<specifics>
## Specific Ideas

- The project picker should feel like a search box. The user said: "name suggest input and select in list find in list".
- The friendly style must still make it practical to watch agents: readable log rows with raw JSON on click, plus a stepper with tokens and duration on each step.
- The task screen should give the same next-step guidance the CLI prints after a run, so moving between the UI and the terminal is seamless.

</specifics>

<deferred>
## Deferred Ideas

- Editing `.plan.md` in the UI, and a "re-plan with a note" box: not chosen. Plans are reviewed in the user's own editor (D-02).
- Queueing tasks per project or per profile: not chosen; the UI refuses instead (D-04, D-17).
- Running in the background as a daemon (`serve --daemon`) and opening the browser automatically: not chosen (D-09, D-13).
- Changing a project's profile, or overriding the profile for one task, from the UI: not chosen (D-15).
- Detecting outside interactive Claude sessions on `~/.claude-orca` before a personal-profile run (they blocked E-06 in Phase 3): a possible CLI and UI safeguard, but a new capability, so noted for the backlog.

</deferred>

---

*Phase: 05-embedded-dashboard*
*Context gathered: 2026-10-17*
