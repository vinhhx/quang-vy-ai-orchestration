# Phase 5: Embedded Dashboard - Research

**Researched:** 2026-10-17
**Domain:** A loopback-only web dashboard (Axum + SSE + embedded HTML/Tailwind/Alpine.js) inside the existing static musl `agent-brain` binary; live-following SQLite written by other processes; supervising role runs started from a browser.
**Confidence:** HIGH on server stack, auth, SSE/SQLite following, musl build, process supervision, offline CSS (all prototyped and run on this machine, probes in the scratchpad, none committed); MEDIUM on the UI layer (Alpine CSP build verified in headless Chrome for the core features only, not for all five screens); LOW only where tagged `[ASSUMED]`.

> Provenance tags: `[VERIFIED: ...]` = observed this session (a file read with path:lines and a verbatim quote, or a command run with its output shown); `[CITED: url]` = taken from an official document fetched this session; `[ASSUMED]` = training knowledge or a judgement call, listed in the Assumptions Log.
> Nothing in this research touched the repository, `~/.cargo`, or the real `claude`. All experiments ran in `/tmp/claude-1000/-mnt-data-IDE-QuangVy-Orchestration/10436bb8-7059-473d-a616-dc31d8e09270/scratchpad/` with a scratch `CARGO_HOME`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

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

### Deferred Ideas (OUT OF SCOPE)
- Editing `.plan.md` in the UI, and a "re-plan with a note" box: not chosen. Plans are reviewed in the user's own editor (D-02).
- Queueing tasks per project or per profile: not chosen; the UI refuses instead (D-04, D-17).
- Running in the background as a daemon (`serve --daemon`) and opening the browser automatically: not chosen (D-09, D-13).
- Changing a project's profile, or overriding the profile for one task, from the UI: not chosen (D-15).
- Detecting outside interactive Claude sessions on `~/.claude-orca` before a personal-profile run (they blocked E-06 in Phase 3): a possible CLI and UI safeguard, but a new capability, so noted for the backlog.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DASH-01 | Binary opens the dashboard, nothing to install (UI embedded, same origin as API) | `include_bytes!` asset table (no `rust-embed`); musl static build with axum verified; `serve` subcommand; build-release smoke (Standard Stack, Pattern 1, Pattern 8) |
| DASH-02 | Not reachable from another LAN machine (loopback only) | Bind `Ipv4Addr::LOCALHOST` only; verified `ss` shows `127.0.0.1:<port>`; test via `/proc/net/tcp` (Security Domain, Validation) |
| DASH-03 | No token = refused | Token middleware + cookie + Host/Origin checks, verified with curl (401 / 303+Set-Cookie / 421) (Pattern 2) |
| DASH-04 | Assign from UI, live log over SSE, pipeline steps change state | Child-process job manager + DB-following SSE + derived step state machine (Patterns 3, 4, 5) |
| DASH-05 | Reload keeps old logs and continues; SSE reconnects and catches up from SQLite | Per-run cursor = `events.seq`; `Last-Event-ID` resume verified against a separate writer process (Pattern 4) |
| DASH-06 | Kill from UI leaves no stragglers | Reuse `runner::cmd_kill`; supervisor child keeps S4 kill; SIGINT-forward and PDEATHSIG both verified (Pattern 3) |
| DASH-07 | Ambiguous project name lists choices, never auto-runs | New `routing::suggest`; server-side `resolve` gate on POST (Pattern 6) |
| DASH-08 | Profile switch filters stats and tasks; new task uses the project's profile | Filter params on every list endpoint; start path never accepts a profile (Pattern 7) |
| DASH-09 | Offline: styles still render | Committed Tailwind CSS built with the standalone CLI (verified inside a no-network namespace); no external URLs in assets (Pattern 8, Pitfall 12) |
| DASH-10 | Figma match (N/A, D-18) | No design exists; mark N/A in verification; optional `/gsd-ui-phase 5` |
| DASH-11 | Stats by day / profile / project / role; overview running/done/failed | `stats::query_report` covers project and role only; `by_day` and `by_profile` must be added (chrono::Local in musl verified) |
| DASH-12 | Projects screen: list, edit alias, rescan | Optimistic-concurrency alias edit on `save_map`; rescan = child `projects scan`; warn-only duplicates (Pattern 9) |
</phase_requirements>

## Summary

The Phase 3 core is already shaped for this phase: SQLite in WAL mode with a 5 s busy timeout, `events(run_id, seq, ts, kind, raw_json)` with `UNIQUE(run_id, seq)`, a `RunObserver` that inserts one row per stream line, and `cmd_kill`/`procs::kill_tree` that already work across processes (the CLI `kill` kills a run owned by another terminal). The dashboard therefore does not need to sit inside the run; it can treat SQLite as the single source of truth for live data and treat every run as an external process, which is exactly what D-12 forces for terminal-started runs.

Three findings reshape the CONTEXT's suggested wiring, all verified by running code. (1) `runner::supervise` builds its own tokio runtime and `block_on`s; called from an async context it panics ("Cannot start a runtime from within a runtime"), and only works from `spawn_blocking`. It also registers its own SIGINT/SIGTERM/SIGHUP handlers, which would collide with the server's. (2) A child `agent-brain run <role> ...` spawned by `serve` keeps every Phase 3 guarantee for free (same code path, exit codes 0/2/3/4/130, same DB writes, same S4 kill) and, with `PR_SET_PDEATHSIG` plus SIGINT forwarding, stops cleanly on both graceful shutdown and a SIGKILL of the server (both tested: run ends `killed`, no `fake_claude_tree`/`sleep` survivors). (3) Because live following needs SQLite anyway, an in-process "broadcast of events" is redundant: use a broadcast of change hints fed by a `PRAGMA data_version` poller, and let every SSE handler read its own cursor from SQLite. Reload, reconnect via `Last-Event-ID`, and terminal-started runs then share one code path.

The build side is low-risk. Axum 0.8.9 with `default-features = false, features = ["http1","json","query","tokio"]` plus `futures-util` (already in `Cargo.lock`) adds 22 packages and changes no existing locked version; a probe binary linked with axum, tokio `net` and rusqlite built as `static-pie linked` with no `NEEDED` entries through the existing route-A musl command in 2 m 05 s cold. Offline CSS is solved by the Tailwind v4.3.3 standalone CLI at dev time (no Node, built inside a no-network namespace) with the generated `app.css` committed, so `build-release.sh` and `--offline --locked` are untouched. For the UI, the Alpine **CSP build** (`@alpinejs/csp` 3.17.4) lets the server send `script-src 'self'` with no `unsafe-eval`; core features were verified in headless Chrome under that exact policy.

**Primary recommendation:** Build `agent-brain serve` as a thin axum layer: SQLite is the only live data source (a change-hint pump plus per-connection cursors for SSE), every role run is a child `agent-brain run ... --` process owned by a dedicated spawner thread (own process group, `PDEATHSIG`, SIGINT on shutdown), Kill reuses `runner::cmd_kill`, assets are `include_bytes!` files including a committed Tailwind CSS and the vendored Alpine CSP build, and the Cargo/Tailwind/Alpine downloads are user checkpoints exactly like 03-01.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Token check, cookie, Host/Origin, security headers | Dashboard server (axum middleware) | - | Every request, including SSE and static files, must pass one gate; the browser must not be trusted to enforce it |
| Serving UI files | Dashboard server (`include_bytes!` table) | - | One static binary (DIST-01); no disk reads, so no path traversal surface |
| Live log delivery | Dashboard server (SSE handlers reading SQLite) | Storage (SQLite `events`) | D-12: terminal-started runs exist only in SQLite; one path for all runs |
| Detecting new rows from other processes | Dashboard server (pump thread, `data_version`) | Storage | Change hints only; payload always read from the DB |
| Running Lead/Dev/QA | Child `agent-brain run` process | Dashboard server (job manager) | Keeps Phase 3 supervision, signals and DB writes untouched; crash-safe via PDEATHSIG |
| Kill | Core library (`runner::cmd_kill`, `procs::kill_tree`) | Dashboard server (HTTP wrapper) | D-12 "same path as `agent-brain kill N`" |
| Task pipeline state (Lead/Dev/QA step states, next actions) | Dashboard server (pure function over DB + git) | Browser (rendering) | Must equal what the CLI prints; derived from `tasks.status` and `runs.outcome` |
| Project matching / autocomplete | Core library (`routing`) | Dashboard server | Same `normalize` as the CLI; browser only renders the list |
| Alias editing and rescan | Core library (`projects`) | Dashboard server | `projects_map.json` stays the source of truth (03 D-05) |
| Stats aggregation | Core library (`stats`) + new by-day/by-profile queries | Dashboard server | Same SQL semantics as `agent-brain stats`; day bucketing in Rust (`chrono::Local`) |
| Profile filter, theme, auto-scroll pause | Browser (Alpine + localStorage) | - | Pure view state (D-15, D-16, D-21) |
| CSS generation | Dev-time tooling (Tailwind standalone) | Repo (committed `app.css`) | Release build must stay offline and Node-free |

## Project Constraints (from CLAUDE.md)

- Claude plans and reviews; **Antigravity (`agy`) executes**. Every plan must be executable without this conversation: exact files, steps, acceptance criteria, verify commands. Claude does not edit files in a plan's scope while `agy` runs.
- `agy` is invoked as `timeout 45m agy -p "<prompt>" --output-format stream-json --print-timeout 40m </dev/null > "$LOG" 2> "${LOG%.jsonl}.stderr.txt"`; stdin from `/dev/null`, stdout to a file, never the bare `antigravity` binary, no turn-limit flag. Tool permissions are unverified: after every run check the diff for real changes.
- The agy prompt must say: commit each task (no `git add -A`, no `--no-verify`), do not touch `.planning/STATE.md`, `ROADMAP.md`, untracked `.planning/` files, or `~/.claude-orca`.
- Review: `git log`/`git diff` since the pre-run HEAD, re-run the plan's verify commands, check acceptance criteria, scan for secrets (`spikes/lib/scan-secrets.sh`).
- Network and toolchain steps are user checkpoints (or Claude on explicit request), never the executor (03-01 precedent). This applies to `cargo fetch`, the Tailwind binary download and the Alpine npm download in this phase.
- Killing: record descendants, SIGTERM the recorded PID, sweep; never `pkill`/`killall` or kill by process name.
- Memory note: live kill tests must use `python3 -c 'import time; time.sleep(N)'`, not a foreground `sleep N` (claude 2.1.301 blocks standalone sleep).

## Standard Stack

### Core (new Cargo dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | `=0.8.9` | HTTP routing, middleware, SSE, `axum::serve` | Locked by D-00a. crates.io: max_version 0.8.9, published 2026-04-14, not yanked, `rust_version` 1.80 (project MSRV 1.85). Legitimacy OK, 10.46M downloads/week, repo github.com/tokio-rs/axum. `[VERIFIED: crates.io API + package-legitimacy seam]` |
| futures-util | `=0.3.34` with `default-features = false, features = ["alloc"]` | `stream::unfold` to build the SSE stream | Already locked at 0.3.34 in `agent-brain/Cargo.lock`, so no new package; axum itself depends on it. `[VERIFIED: Cargo.lock; probe compiled]` |

Exact lines to add to `agent-brain/Cargo.toml` (tokio line replaces the current one; `rt-multi-thread` adds no packages, verified by lockfile diff):

```toml
tokio = { version = "=1.53.3", features = ["rt", "rt-multi-thread", "net", "process", "io-util", "signal", "time", "macros", "sync"] }
axum = { version = "=0.8.9", default-features = false, features = ["http1", "json", "query", "tokio"] }
futures-util = { version = "=0.3.34", default-features = false, features = ["alloc"] }
```

Current line being replaced `[VERIFIED: agent-brain/Cargo.toml:13]`: `tokio = { version = "=1.53.3", features = ["rt", "process", "io-util", "signal", "time", "macros", "sync"] }`.

Axum's own `tokio` feature already enables `tokio/net`, `tokio/rt` and `tokio/macros` `[VERIFIED: crates.io API features for axum 0.8.9: "tokio": ["dep:hyper-util","dep:tokio","tokio/net","tokio/rt","tower/make","tokio/macros"]]`; listing `net` explicitly in the tokio line keeps the intent visible to reviewers (the CONTEXT flagged it).

No other crate is needed. Deliberately not added: `rust-embed`/`include_dir` (use `include_bytes!`), `axum-extra`/`cookie` (hand-parse one cookie; set one header), `tokio-stream`/`async-stream` (use `futures_util::stream::unfold`), `rand`/`getrandom` (read `/dev/urandom` with `std`), `tower-http`, `tracing`, `notify`, any HTTP client dev-dependency (tests use a raw `std::net::TcpStream`, matching the "no other dependency" rule of 03-01), and a charting library.

### Lockfile delta (what the user checkpoint actually does)

`[VERIFIED: scratch copy of agent-brain, CARGO_HOME scratch, `cargo fetch`, then diff of name+version pairs]` Existing 98 packages unchanged (no removed or changed pair); 22 packages added:

`atomic-waker 1.1.2, axum 0.8.9, axum-core 0.5.6, form_urlencoded 1.2.2, http 1.5.0, http-body 1.1.0, http-body-util 0.1.5, httparse 1.10.1, httpdate 1.0.3, hyper 1.12.0, hyper-util 0.1.21, matchit 0.8.4, mime 0.3.17, percent-encoding 2.3.2, ryu 1.0.23, serde_path_to_error 0.1.20, serde_urlencoded 0.7.1, socket2 0.6.5, sync_wrapper 1.0.2, tower 0.5.3, tower-layer 0.3.3, tower-service 0.3.3`.

Only `axum` and `futures-util` are pinned with `=` in `Cargo.toml`; the other 20 are pinned by the lockfile on the day the user runs the fetch (the 03-01 pattern pinned direct crates only). The versions above are what resolved today; the real fetch may resolve a newer patch of a transitive crate, which is acceptable because `--locked` then freezes it.

**What "vendored or fetched lockfile update" means here:** this repo has no `vendor/` directory and no `.cargo/config.toml` (checked: `agent-brain/.cargo`, `../.cargo`, `~/.cargo/config.toml` all absent); crates live in `~/.cargo/registry` and none of the 22 new ones are there yet (`ls ~/.cargo/registry/cache/*/ | grep axum` empty). So after `Cargo.toml` is edited, `cargo test --offline --locked` cannot work until a human (or Claude on explicit request, as in 03-01 Task 3) runs one network command, then commits the regenerated `Cargo.lock`:

```bash
cargo fetch --manifest-path agent-brain/Cargo.toml          # network; rewrites Cargo.lock keeping the 98 existing versions
cargo test  --manifest-path agent-brain/Cargo.toml --offline --locked
CC_x86_64_unknown_linux_musl=musl-gcc cargo build --manifest-path agent-brain/Cargo.toml --release --offline --locked --target x86_64-unknown-linux-musl
git add agent-brain/Cargo.lock agent-brain/Cargo.toml && git commit -m "build(05-01): lock dashboard dependencies"
```

The executor must not run any of these (03-01 prohibition).

### Front-end tooling (not Cargo; dev-time or vendored)

| Item | Version | Purpose | Provenance |
|------|---------|---------|------------|
| Tailwind CSS standalone CLI | v4.3.3 (`tailwindcss-linux-x64`, 111,749,248 bytes, sha256 `dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a`) | Generates `assets/dashboard/app.css` at dev time; not part of the release build | `[VERIFIED: downloaded from github.com/tailwindlabs/tailwindcss/releases/download/v4.3.3/; sha256 equals the published sha256sums.txt line; ran `--help` and a build]` |
| `@alpinejs/csp` | 3.17.4 (`dist/cdn.min.js`, 72,198 bytes, sha256 `0d18d7f8d7910e2e0212f0f056b12f50bebc3abb7d88d2f7c7cb4c336fe4519a`; tarball integrity `sha512-SlRXmqO6kYhnxlg+99etmuzJtE9Lk4QbKjBHqerXzaMflJqoJXdz/SI3IvHJGZ/vRVyC3bR0SSBz40oY7goBeg==`) | Reactive UI without `unsafe-eval` | `[VERIFIED: npm view + npm pack + sha256sum]`; flagged SUS by the legitimacy seam, see audit |

Tailwind's generated CSS begins with `/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */`; Alpine's file carries no banner, so add a `VENDOR.md` next to the vendored file recording package, version, source URL, hashes and license (MIT) `[VERIFIED: package.json license MIT]`.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Standard Alpine build | `@alpinejs/csp` | Standard build needs `script-src 'unsafe-eval'` (it compiles expressions with `new Function`); CSP build is 72 KB vs 56 KB and forbids arrow functions, template literals, property assignment in handlers and globals in templates, so logic lives in `Alpine.data()` (a good fit for one JS file per screen) `[CITED: alpinejs.dev/advanced/csp]` |
| Child `agent-brain run` | In-process `roles::execute_role` via `spawn_blocking` | In-process works (verified: `supervise` from `spawn_blocking` returns `Succeeded`) but duplicates signal handling with the server, ties run lifetime to the server's memory, loses crash safety, and prints CLI chatter into the server terminal; see Pattern 3 |
| Change-hint pump + cursor reads | In-process hook in `RunObserver::on_line` publishing payloads | Hook cannot see terminal-started runs (D-12), so SQLite following is required anyway; a hook would be a second path to keep consistent |
| Hand-made CSS bars | Chart.js/uPlot | Hand-made bars are ~0 KB, offline, theme-able; a chart library is 50-200 KB of vendored JS to audit for four simple breakdowns |

### Package Legitimacy Audit

Seam run: `gsd_run query package-legitimacy check --ecosystem crates ...` and `--ecosystem npm @alpinejs/csp alpinejs` this session.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| axum | crates.io | 5 yrs (2021-07-22) | 10.46M/wk | github.com/tokio-rs/axum | OK | Approved (also mandated by D-00a) |
| futures-util | crates.io | 8 yrs | 20.6M/wk | github.com/rust-lang/futures-rs | OK | Approved (already in lockfile) |
| axum-core, hyper, hyper-util, tower, tower-layer, tower-service, http, http-body, http-body-util, httparse, httpdate, matchit, mime, percent-encoding, form_urlencoded, serde_urlencoded, serde_path_to_error, sync_wrapper, socket2, atomic-waker, ryu | crates.io | 2-12 yrs | 7.6M-25.9M/wk each | tokio-rs, hyperium, tower-rs, dtolnay, servo, rust-lang, smol-rs repos | OK (all 21) | Approved (transitive, locked) |
| @alpinejs/csp | npm | package since 2024-01-04; latest version 2026-09-21 | 42,643/wk | github.com/alpinejs/alpine | SUS (`too-new`: the latest release is 26 days old; weekly releases) | Flagged: planner must add a `checkpoint:human-verify` before vendoring. [WARNING: flagged as suspicious - verify before using.] The package and repo are long-lived (companion `alpinejs` 728k/wk); the flag reflects release recency only; no `postinstall` script (`scripts` empty) |
| Tailwind standalone binary | GitHub release (not a registry package) | - | - | github.com/tailwindlabs/tailwindcss | n/a (hash-verified) | Approved with pinned sha256; dev-time only, never linked or shipped |

**Packages removed due to SLOP verdict:** none.
**Packages flagged SUS:** `@alpinejs/csp@3.17.4` - planner inserts `checkpoint:human-verify` (user opens npmjs.com/package/@alpinejs/csp and github.com/alpinejs/alpine, confirms the repo link, MIT license, and that 3.17.4 exists and is not deprecated) before the vendoring step. Cheap fallback if the user objects to recency: pin an older 3.x and re-run the headless-Chrome smoke (Pitfall 14).

**Installation (all user checkpoints, none for the executor):**
```bash
# Cargo: see the command block under "Lockfile delta"
# Tailwind (dev tool, gitignored):
mkdir -p agent-brain/tools
curl -sL -o agent-brain/tools/tailwindcss https://github.com/tailwindlabs/tailwindcss/releases/download/v4.3.3/tailwindcss-linux-x64
echo "dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a  agent-brain/tools/tailwindcss" | sha256sum -c - && chmod +x agent-brain/tools/tailwindcss
# Alpine CSP (vendored file, committed):
cd "$(mktemp -d)" && npm pack @alpinejs/csp@3.17.4 && tar -xzf alpinejs-csp-3.17.4.tgz
sha256sum package/dist/cdn.min.js   # must print 0d18d7f8...4519a
cp package/dist/cdn.min.js <repo>/agent-brain/assets/dashboard/vendor/alpine.csp.min.js
```

## Architecture Patterns

### System Architecture Diagram

```
Browser (127.0.0.1:PORT, cookie)                            Terminal user
  |  GET /  ?token=T  ----------------------+                  |
  |  GET /app.css /js/*.js /screens/*.html  |                  |  agent-brain run ... (CLI)
  |  GET/POST /api/*   EventSource /api/runs/N/stream          |  agent-brain kill N
  v                                                            v
+----------------- agent-brain serve (one process, flock) ----------------+       +--- agent-brain run ---+
| axum Router                                                             |       | supervisor (existing)  |
|  middleware: Host allow-list -> token/cookie -> Origin(POST) -> headers |       | writes runs/events     |
|   |-- static: include_bytes! table (index, app.css, js, screens, alpine)|       +----------+-------------+
|   |-- read APIs: projects/tasks/stats/runs (spawn_blocking + read conn)  |                  | spawns (own PGID)
|   |-- SSE /api/runs/{id}/stream: cursor=events.seq; wake on hint        |                  v
|   |-- write APIs: POST tasks|dev|qa|relead, kill, projects PUT|scan      |            claude (S4 kill tree)
|        |                         |                                      |
|        | job manager             | runner::cmd_kill (spawn_blocking)    |
|        v                         v                                      |
|  spawner thread (long-lived) --- Command(current_exe, ["run", role, ..., "--", project, text])
|        process_group(0) + PR_SET_PDEATHSIG(SIGTERM); waiter thread per child
|                                                                          |
|  pump thread: data_version every 250 ms -> new events? -> broadcast<i64> hints (run_id | 0)
|  reconcile_stale_runs every ~5 s (so dead terminal runs stop counting as busy)
+-----------------------------+--------------------------------------------+
                              | SQLite WAL  ~/.local/share/agent-brain/brain.db  (busy_timeout 5 s)
                              | projects_map.json (atomic save_map)   git working trees (task/<slug>)
```

Primary trace (assign from UI): POST `/api/tasks` -> validate project with `routing::resolve` (never guess) -> job reserved (project + profile busy checks) -> spawner thread starts `agent-brain run lead --name SLUG -- PROJECT "REQUEST"` -> child inserts `tasks`/`runs`, then one `events` row per stream line -> pump sees `data_version` change, publishes the run id -> SSE handler wakes, reads `events WHERE run_id=? AND seq>cursor`, emits rows -> child finishes, sets `tasks.status='planned'` -> pump publishes `0` -> SSE emits `status` then `end`; UI stepper shows "Chờ duyệt plan".

### Recommended Project Structure

```
agent-brain/
├── Cargo.toml                       # +axum, futures-util, tokio net/rt-multi-thread
├── assets/dashboard/                # everything here is include_bytes!'d
│   ├── index.html                   # shell: theme.js (head, blocking), alpine (defer), app.js
│   ├── app.css                      # GENERATED by Tailwind, committed
│   ├── VENDOR.md                    # provenance + hashes of vendored files
│   ├── vendor/alpine.csp.min.js     # @alpinejs/csp 3.17.4
│   ├── js/theme.js app.js overview.js task.js assign.js stats.js projects.js
│   └── screens/overview.html task.html assign.html stats.html projects.html
├── dashboard-src/input.css          # Tailwind entry (not embedded)
├── tools/                           # gitignored; tailwindcss binary
├── src/dashboard/
│   ├── mod.rs        # serve(), AppState, router, shutdown orchestration
│   ├── auth.rs       # token, cookie, Host/Origin, security headers
│   ├── assets.rs     # static table + handler (no disk access)
│   ├── lock.rs       # flock single-instance + info file
│   ├── pump.rs       # data_version poller, reconcile, broadcast hints
│   ├── sse.rs        # /api/runs/{id}/stream
│   ├── jobs.rs       # spawner thread, PDEATHSIG, SIGINT forwarding, job map
│   ├── taskstate.rs  # pure: (tasks.status, last runs, plan/git facts) -> steps + next actions
│   └── api_*.rs      # projects, tasks, runs, stats handlers
scripts/build-css.sh                 # download-verify tailwind, build, --check mode
```

`lib.rs` adds `pub mod dashboard;`. Additive helpers in existing modules: `routing::suggest`, `stats::query_by_day`/`query_by_profile`, `config` key `dashboard_port`.

### Pattern 1: `serve` startup order (single instance, no side effects before the lock)

**What:** In `main.rs`, handle `Commands::Serve` **before** `app::open_ctx()` (the way `Guard` is handled at `main.rs:130`), because `open_ctx` opens the DB, reconciles and syncs projects (writes). Order: load config -> flock `serve.lock` -> bind `127.0.0.1:port` -> only then `open_ctx` -> start pump -> print URL.
**Why:** My probe opened the DB before binding; a refused second instance still created a stray `t2.db`. D-14 wants a refusal with no side effects.
**Verified facts:** `flock(LOCK_EX|LOCK_NB)` on a file opened by `std::fs::OpenOptions` (which sets `O_CLOEXEC`) refused a second process with `WouldBlock`, the lock file showed `{"pid":512303}`, and re-locking worked after the holder exited `[VERIFIED: probe4, output shown]`. A second bind returned `ErrorKind::AddrInUse` -> message and exit 1 `[VERIFIED: probe]`.
```rust
// Source: probe4/probe in scratchpad (compiled against libc =0.2.190, tokio =1.53.3)
let f = std::fs::OpenOptions::new().create(true).read(true).write(true).open(&lock_path)?; // O_CLOEXEC by default
if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
    // read {"pid":..,"port":..} from the file and print: "agent-brain serve đã chạy (pid P) tại http://127.0.0.1:PORT/ ..."
}
let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await; // never 0.0.0.0, never "localhost"
```
Keep `f` alive for the whole process; write `{"pid","port","started_at"}` (no token) after a successful bind. The token is never written to disk, so a second `serve` cannot print it; its message points at the first terminal. `--port 0` must be accepted (print the real port from `listener.local_addr()`) so tests can run in parallel.

### Pattern 2: Auth gate (token -> cookie -> redirect), Host and Origin checks

**Defaults to adopt (Claude's discretion items):**

| Item | Choice | Why |
|------|--------|-----|
| Default port | **7437**, overridable by `--port` and config key `dashboard_port` (precedence: flag > config > default) | 7878 (the CONTEXT example) is Radarr's default port `[CITED: docs.seerr.dev/using-seerr/settings/services/ via web search: "Radarr uses 7878"]`; keep below 32768 (Linux ephemeral range). That 7437 is unassigned is `[ASSUMED]` (A1); D-10's clear "port in use" error covers a clash |
| Token | 32 bytes from `/dev/urandom`, hex-encoded (64 chars), new each start | 256-bit, no crate. `[VERIFIED: probe gen_token]` |
| Cookie | `agent_brain_token=<token>; HttpOnly; SameSite=Strict; Path=/` (session cookie: no `Max-Age`; no `Secure` since the origin is plain http) | `[CITED: MDN Set-Cookie]` HttpOnly hides it from JS but it is still sent by fetch/EventSource; Strict = same-site only. Verified `Set-Cookie` and `303 Location: /` with curl |
| Compare | constant-time byte fold (5 lines) | not a crypto primitive; avoids adding `subtle` |
| Host check | allow only `127.0.0.1:<port>` and `localhost:<port>`, else `421` | blocks DNS rebinding; `[VERIFIED: curl -H 'Host: evil.com:17878' -> 421]` |
| Origin check | for every non-GET/HEAD: `Origin` must equal `http://127.0.0.1:<port>` or `http://localhost:<port>`, else `403`; require `Content-Type: application/json` on POST/PUT | CSRF defense in depth beside SameSite |
| Token only accepted on | `GET /` (query `token=`) then redirect to `/` | keeps tokens out of API URLs and logs |
| Headers on every response | `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `Cache-Control: no-store` on `/api/*` and `no-cache` on assets | CSP string verified working with the Alpine CSP build in headless Chrome |

Observed probe results (curl): no cookie -> 401; wrong cookie -> 401; `/?token=<good>` -> `303 See Other`, `location: /`, `set-cookie: agent_brain_token=...; HttpOnly; SameSite=Strict; Path=/`; with cookie -> 200 `[VERIFIED: probe run]`.

Known limit, accept and document: cookies are not port-isolated (`[CITED: RFC 6265 §8.5: "Cookies do not provide isolation by port."]`), so another local web service on `127.0.0.1` would be sent the cookie. Mitigations in place: session cookie, per-start token, Host/Origin checks. Browsers key the cookie to the host name, so the printed URL must use `127.0.0.1` consistently; opening `localhost:PORT` first has no cookie and shows a 401 page telling the user to use the printed URL.

### Pattern 3: Running roles as child `agent-brain run` processes (answers the in-process vs child question)

**Choice: child process.** Evidence:

1. `[VERIFIED: probe3]` `runner::supervise` called directly inside `#[tokio::main]` panics: `Cannot start a runtime from within a runtime`; from `tokio::task::spawn_blocking` it returns `Succeeded`. Source: `runner.rs:183-188` `[VERIFIED: runner.rs:183-188]`:
   ```
   pub fn supervise(req: &RunRequest, obs: &mut dyn RunObserver) -> BrainResult<RunOutcome> {
       let rt = tokio::runtime::Builder::new_current_thread()
       ...
       rt.block_on(supervise_async(req, obs))
   ```
   so in-process is possible but needs a blocking thread per run.
2. `supervise_async` registers its own handlers `[VERIFIED: runner.rs:197-202]`: `signal(SignalKind::interrupt())`, `signal(SignalKind::terminate())`, `signal(SignalKind::hangup())`. In-process these race the server's own shutdown handling. As a child they are the process's only handlers, i.e. exactly the CLI behaviour that Phase 3 verified live.
3. A child gives CLI parity for free: the same pre-flight (dirty tree, branch rules, E-06), the same `tasks.status`/`runs.outcome` writes, the same exit codes, and its stderr/stdout carry the Vietnamese refusal text to show in the UI. `execute_role` and `cmd_lead/dev/qa` print through `term::emit` and need a `&mut Ctx` (a non-`Sync` `Connection`); the child isolates both.
4. **D-08 graceful stop** `[VERIFIED: probe2 graceful]`: the server sends SIGINT to the supervisor child (identical to Ctrl+C), it kills the `fake_claude_tree` tree, the run row ends `killed` (exit_code 143 from the killed claude), `ps` shows no `fake_claude_tree`, no `sleep 317`, no `agent-brain run`.
5. **D-08 crash** `[VERIFIED: probe2 crash]`: with `PR_SET_PDEATHSIG` = SIGTERM set in `pre_exec`, SIGKILLing the server made the supervisor handle SIGTERM, kill its tree, and finalize the DB row `killed`; no survivors after 4 s. In-process cannot do this (a SIGKILLed server leaves `claude` orphaned and the row `running` until reconcile sees a dead PID, and the orphan is not dead).

Spawn recipe (`[VERIFIED: probe2 compiled and ran]`):
```rust
// run only on ONE long-lived std::thread ("spawner"); PDEATHSIG fires when the *spawning thread* exits
let mut cmd = std::process::Command::new(&exe);           // exe = std::env::current_exe() resolved once at startup
cmd.args(["run", "lead", "--name", &slug, "--", &project_id, &request]); // "--" is mandatory, see Pitfall 6
cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
cmd.process_group(0);                                     // own PGID: terminal Ctrl+C does not reach it; the server forwards it
unsafe { cmd.pre_exec(|| if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) }); }
```
A waiter thread per child drains stdout/stderr into a bounded ring (last ~200 lines) and calls `wait()` (no zombies), then records the exit code in the job. Shutdown sequence: stop accepting new jobs -> SIGINT every live supervisor -> wait up to `kill_grace_secs + 5` s -> for stragglers `procs::kill_tree(child, Duration::ZERO, &AtomicBool::new(true))` -> then stop the HTTP server. Handle SIGINT, SIGTERM **and SIGHUP** in the server (closing the terminal must not leave runs; the children are in another process group so they do not get the terminal's SIGHUP themselves).

Kill from the UI: `POST /api/runs/{id}/kill` -> look up `runs.task_id` -> `spawn_blocking(|| runner::cmd_kill(&mut ctx, task_id))` with a fresh `Ctx` from `app::open_ctx_with(...)`. It is the same function the CLI calls (`runner.rs:55`), works on terminal-started runs (it kills the recorded claude PID tree; the owning supervisor sees the exit and records `killed`), and blocks for up to `kill_grace_secs + ~2` s, hence `spawn_blocking`. A run without `task_id` (an `inspect`) is not killable from the UI; list only task runs.

### Pattern 4: Live logs = SQLite cursor per SSE connection, woken by a broadcast of hints (answers D-12 merge)

**Design:** one pump thread owns a read connection (`db::open`) and every 250 ms runs `PRAGMA data_version`; if it changed it reads `SELECT run_id, MAX(id) FROM events WHERE id > ?last GROUP BY run_id`, sends each `run_id` on a `tokio::sync::broadcast::Sender<i64>` (capacity 1024), and sends `0` meaning "runs/tasks changed" (status flips, task status). It also calls `runner::reconcile_stale_runs` every ~5 s. An SSE handler: `subscribe()` first, then loop { read `events WHERE run_id=?1 AND seq>?2 ORDER BY seq LIMIT 500`; emit rows; if none, check the run status; if terminal emit `event: end`; else wait for a hint for this run (or `0`, or lag, or 15 s) }. Because every wake-up re-reads from the DB by cursor, there is nothing to de-duplicate and a lagged receiver simply re-reads.

`[CITED: sqlite.org/pragma.html#pragma_data_version]` "The integer values returned by two invocations of "PRAGMA data_version" from the same connection will be different if changes were committed to the database by any other connection in the interim... unchanged for commits made on the same database connection... same for all database connections, including database connections in separate processes." The pump connection never writes events itself, so it sees every writer, including the in-process kill path and child supervisors.

Latency measured with a separate Python writer inserting every 0.4 s: rows reached the SSE client within about 0.0-0.25 s (log timestamps 33.222 / 33.471 / 33.978 / 34.239 ...), the final status flip produced `event: end` `[VERIFIED: probe, output shown]`. A reconnect with `Last-Event-ID: 3` returned only ids 4-6 and `end` `[VERIFIED]`. No broadcast payload carries log text, so CONTEXT D-00c's "broadcast channel" exists (it carries hints) and D-12 is satisfied by the same code path.

Wire format per log row (`id:` = `events.seq`, per-run cursor; do not use the global rowid):
```
id: 12
data: {"seq":12,"ts":1792172314208,"kind":"tool","text":"tool Bash ls -la"}
```
Build `text` on the server with `stream::classify(raw.as_bytes())` then `term::event_text(&ev, false)` (not `event_line`, which stamps the current time) and derive `kind` in {tool, assistant, tool_result, result, system, error} from `ev.kind`/text prefix; **do not** ship `raw_json` (real data: max 22,284 bytes per event `[VERIFIED: copy of the real brain.db: max(length(raw_json)) = 22284, avg 1791, 185 events / 12 runs]`, tool results can be far larger). Raw JSON is fetched on click from `GET /api/runs/{id}/events/{seq}/raw` and shown pretty-printed via `x-text`.
Other SSE events: `event: status` (data = run status/outcome/tokens, no `id:` so `Last-Event-ID` is untouched) on connect and on change; `event: end` when the run is terminal and drained. `KeepAlive` every 15 s. 42 of 185 real events are `system/thinking_tokens` noise `[VERIFIED: same copy]` (STATE.md also records it): give the log pane a "Ẩn system" toggle, default on.

Why one SSE per task screen and polling for lists: browsers cap HTTP/1.1 connections per origin (about 6, `[ASSUMED]` A4), and each open tab holds its SSE; Overview/Projects/Stats refresh by polling every 3-5 s.

```rust
// Source: probe.rs (axum 0.8.9 + futures-util 0.3.34), trimmed. Verified compiling and streaming.
let rx = st.tx.subscribe();                       // subscribe BEFORE the first backlog read
let s = futures_util::stream::unfold(state, |mut s| async move {
    loop {
        if let Some(ev) = s.buf.pop_front() { return Some((Ok::<Event, Infallible>(ev), s)); }
        if s.done { return None; }
        /* spawn_blocking(read_after(db, run_id, s.last)) -> rows, status; push Event::default().id(seq).data(json) */
        /* if drained and status is terminal: push Event::default().event("end").data(status); s.done = true; */
        /* else await hint: tokio::time::timeout(15s, rx.recv() until id==run_id||0 | Lagged) */
    }
});
Sse::new(s).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
```

### Pattern 5: Task and pipeline state is derived, not stored (D-01, D-03, D-23)

The CLI already records everything needed. Verified values (quotes verbatim):

| Fact | Source `[VERIFIED: file:line]` | Quote |
|------|------|-------|
| Run statuses | `db.rs:18` | `status TEXT NOT NULL CHECK (status IN ('running','succeeded','failed','timed_out','killed','lost')),` |
| Run roles | `db.rs:16` | `role TEXT NOT NULL CHECK (role IN ('lead','dev','qa','inspect'))` |
| events table | `db.rs:29-30` | `CREATE TABLE events (id INTEGER PRIMARY KEY, run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE, seq INTEGER NOT NULL, ts INTEGER NOT NULL, kind TEXT NOT NULL, raw_json TEXT NOT NULL, UNIQUE(run_id, seq));` |
| Task becomes planned after Lead | `lead.rs:252` | `crate::db::set_task_status(&ctx.conn, task_id, "planned")?;` |
| Dev blocked | `dev.rs:347-349` | `crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "dev:blocked")?;` ... `crate::db::set_task_status(&ctx.conn, tid, "blocked")?;` |
| Dev done | `dev.rs:355` | `crate::db::set_task_status(&ctx.conn, tid, "dev_done")?;` |
| Dev/QA protocol violation | `dev.rs:277`, `qa.rs:211` | `crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;` |
| QA needs fixes | `qa.rs:238-240` | `"qa:fix"` ... `crate::db::set_task_status(&ctx.conn, tid, "qa_fix")?;` |
| QA pass | `qa.rs:243-245` | `"qa:pass"` ... `crate::db::set_task_status(&ctx.conn, tid, "qa_pass")?;` |
| New/updated task status | `db.rs:174-181` | `VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7, ?7)` ... `status = 'open',` |
| QA verdict values | `planfile.rs:108` | `pub const QA_VERDICTS: [&str; 2] = ["Đạt", "Cần sửa"];` |
| Exit codes | README table | `0, 1, 2, 3, 4, 130` (`EXIT_OK..EXIT_INTERRUPTED`) |

Note: `runs.exit_code` is **claude's** exit code, not the CLI's 0/2/3/4/130, so D-23's "map exit codes to step states" must come from `tasks.status` + `runs.outcome` + `runs.status`, and (for UI-started jobs) the in-memory job's recorded CLI exit code. Recommended step-state mapping (planner may refine with UI-SPEC): per role, latest run of that role -> `running` => "đang chạy"; none => "chờ"; `succeeded` and no negative outcome => "xong"; `outcome='dev:blocked'` => "Vướng mắc"; `outcome='qa:fix'` => "Cần sửa"; `outcome='protocol_violation'` => "Vi phạm giao thức"; `killed` => "killed"; `failed`/`timed_out`/`lost` => "lỗi".
Task banner: `planned` => "Chờ duyệt plan" (action Chạy Dev); `dev_done` => action Chạy QA; `blocked` => highlight `Vướng mắc` section, actions "Chạy lại Lead (re-plan)" then (after the user edits and commits) "Chạy Dev"; `qa_fix` => highlight QA report, actions "Chạy lại Lead" / "Chạy Dev" (CLI prints: edit `.plan.md` and commit `<task>: plan`, or `run lead "<project>" "<request>" --name <task>`, then `run dev`, `qa.rs:231-237`). Re-running Lead from the UI passes the task's stored request (`tasks.title`) and slug so the CLI takes its re-plan branch (`lead.rs:93`).
Gap to close with one line in `lead.rs` (planner decision, Open Question 1): `lead.rs:211-221` returns `EXIT_PROTOCOL` for files edited outside `.plan.md` but never calls `set_run_outcome`, so a terminal-started Lead protocol violation is invisible to the DB, unlike Dev/QA.

Plan/QA rendering: read `.plan.md` from the project working tree only if the current branch equals `tasks.branch`; otherwise `git show <tasks.branch>:.plan.md` (branch from the DB, never from user input). Parse with `planfile::parse_plan`, `validate_plan`, `has_blockers`, `section_text(.., "Vướng mắc")`; render sections as `x-text` blocks with `white-space: pre-wrap` and the `changes` list as a table; **no markdown renderer and no `x-html`** (also impossible in the CSP build). D-02 pick-up: the detail endpoint returns the `<task>: plan` commit sha (`gitops::find_commit_by_subject`) and a `plan_dirty` flag (`gitops::changed_paths` contains `.plan.md`); the open task screen polls it every 3 s and re-renders when the sha or content hash changes. Dev preconditions from `dev.rs:69-93`: must be on `task/<slug>` and a clean tree, so show those as the disabled-button reason ("Plan có thay đổi chưa commit - commit với message '<task>: plan'").

### Pattern 6: Project picker and the ambiguity gate (D-05, DASH-07)

`routing::resolve` is **exact** normalized equality on id, `name_vi` or an alias (`routing.rs:73-122`), not prefix or substring, so it cannot drive an autocomplete. Add `routing::suggest(query, &projects) -> Vec<(Project, rank)>`: keep entries where `normalize(q)` is contained in `normalize(id|name_vi|alias)`, rank exact > prefix > contains, expose `GET /api/projects/suggest?q=&profile=` (profile filter per D-15), mark `missing` projects as not selectable. The UI posts the selected **project id**; the server still calls `select_project`/`resolve` on POST and answers `409` with the candidate list on `Ambiguous` or `None` **without spawning**. Ids resolve first by exact match (`routing.rs:82-86`), so an id that equals another project's alias is still unambiguous.
Slug (D-07): `GET /api/slug?text=` returns `routing::slugify(text)` (`routing.rs:47`) so the pre-filled and edited names match the CLI default exactly (accent stripping, `đ`, 48-char cap); an empty slug means the user must type one (CLI error: "hãy truyền cờ --name <slug>").

### Pattern 7: Busy rules and profile filter (D-04, D-15, D-17)

Neither D-04 nor the "second personal run" rule is enforced identically in the CLI: `execute_role` enforces only the personal rule, verbatim `[VERIFIED: roles/mod.rs:243-246]`: `"Profile personal đang có run {} chạy (task {tid_str}) — không chạy song song trên cùng thư mục cấu hình (E-06)"`, and merely warns for `work` (`roles/mod.rs:248-251`). There is **no per-project rule in the CLI**. So the server must enforce D-04 itself: before spawning, refuse if `SELECT ... FROM runs JOIN projects ... WHERE status='running'` has a row for that project **or** an in-memory reservation exists for it (covers two quick clicks), and refuse a personal-profile start when any running run has profile `personal` (D-17), returning a Vietnamese reason that the UI shows on the disabled button. Expose the same facts in `GET /api/tasks` (`busy: [{project, run_id, profile}]`) so buttons are disabled before the click. A terminal-side start that races the UI check cannot be prevented without changing the CLI (accepted; the CLI personal rule still protects E-06).
Profile filter: `GET /api/tasks|stats|projects?profile=work|personal` (absent = Tất cả). Tasks filter on `projects.profile`; stats on `runs.profile` (existing `query_report`). No endpoint accepts a profile for starting a task (D-15).

### Pattern 8: Embedded assets and the offline CSS build

- Assets: a `static ASSETS: &[(&str, &str, &[u8])]` of (path, mime, `include_bytes!(...)`) with an exact-match lookup; unknown paths 404; no filesystem access at runtime. A missing file is a **compile error**, which doubles as a release guard.
- Tailwind: `scripts/build-css.sh` (dev-time) runs `agent-brain/tools/tailwindcss -i agent-brain/dashboard-src/input.css -o agent-brain/assets/dashboard/app.css --minify`; `app.css` is committed. `input.css`: `@import "tailwindcss"; @custom-variant dark (&:where(.dark, .dark *)); @source "../assets/dashboard";`. Verified: `unshare -rn ./tailwindcss -i input.css -o out.css --minify` succeeded inside a no-network namespace (so the `@import` resolves from the binary), output 7,188 bytes for a small sample `[VERIFIED: command run, output shown]`.
- `build-release.sh` needs **no Tailwind, Node or network**; `cargo build --offline --locked` just embeds the committed files. Add two cheap assertions to it: a smoke that starts `agent-brain serve --port 0` under the existing `env -i HOME=... PATH=/nonexistent` and checks `/` and `/app.css` respond through the token handshake, then stops it with SIGINT (extends DIST-01), and `scripts/build-css.sh --check` as a dev-only freshness test (rebuild to a temp file and `cmp` with the committed one) that is skipped when `tools/tailwindcss` is absent.
- Dark mode: system by default, manual toggle (D-21): a blocking `<script src="/js/theme.js">` in `<head>` (external file, so CSP stays strict) sets the `dark` class from `localStorage` or `prefers-color-scheme` before first paint.
- Tailwind scans source text, so write full class names in HTML/JS strings; never build `"bg-" + color`.

### Pattern 9: Alpine structure, screens, and the Projects screen

- **Alpine CSP build** + one `Alpine.data(name, factory)` per screen, registered in `document.addEventListener('alpine:init', ...)`; screens are `screens/<name>.html` fragments fetched by `app.js` into a mount node (a hash router: `#/`, `#/task/12`, `#/assign`, `#/stats`, `#/projects`). `[VERIFIED: headless Chrome 'dump-dom' with the exact CSP header, Alpine CSP 3.17.4]`: shell `x-data` fetched `screens/overview.html`, inserted with `innerHTML`, Alpine auto-initialised the fragment; `x-for` rendered rows; `:style="'width:' + pct(r) + 'px'"` (a method call plus string concatenation) produced `style="width:10px"`; a getter bound with `x-text` showed `2`; `x-model` and `@click="add"` bound; `theme.js` ran in `<head>`. No CSP violation prevented rendering. This keeps "one screen per file" (spec §7) as an HTML+JS pair that can be handed to `agy` one screen at a time.
- Template limits to respect `[CITED: alpinejs.dev/advanced/csp]`: no arrow functions, no destructuring, no template literals, no property assignment in handlers (`user.name = 'x'`), no globals (`console`, `window`, `JSON`, `Math`, `parseInt`) and no `x-html` in templates; compute in the `Alpine.data` object (getters/methods).
- All log text, plan text and raw JSON go through `x-text` only (agent output is untrusted).
- Charts: CSS bars. Per-day = flex columns, others = horizontal bars; widths/heights come from a `pct` computed in JS and bound with `:style` string concatenation; numbers via `x-text`. Zero JS libraries, offline, themeable with Tailwind `dark:` classes.
- Log pane (D-22): append rows, track whether the container is scrolled to bottom; auto-scroll only when it is; show "Về cuối (live)" when the user has scrolled up; click a row to fetch raw JSON. Keep at most a few thousand rows in memory (drop the oldest) and virtualize nothing.
- Projects screen (DASH-12): `GET /api/projects` returns the map entries plus an `etag` (mtime-nanos + length, or a hash of the bytes); `PUT /api/projects/{id}` with `{aliases:[...], etag}` -> re-read the file, compare the etag (mismatch => `409` with the fresh list, UI says someone edited the file), mutate only that entry's `aliases` (trim, drop empties, de-duplicate by `routing::normalize`; `serde_json` has `preserve_order` so other keys and ordering survive), compute `projects::find_duplicates` and **return them as warnings without refusing** (this mirrors `cmd_scan`, which only warns: `projects.rs:724-729`), then `projects::save_map` (atomic temp-file + rename, mode 0600, `projects.rs:321-382`) and `db::sync_projects`. `profile` is read-only in the UI (D-15). "Quét lại" runs the child `agent-brain projects scan` and shows its two output lines; with empty `scan_roots` the CLI errors "Chưa có scan_roots - chạy: agent-brain projects scan --root <thư mục>" (`projects.rs:692-696`) and the UI shows that text. The server must call `db::sync_projects` after a scan because `main.rs` syncs only at CLI start. The read-modify-write window between our re-read and rename is milliseconds, the same as the CLI; accept it.

### Pattern 10: Stats (DASH-11)

`stats::query_report` returns `total`, `by_project`, `by_role` only `[VERIFIED: stats.rs:37-43: pub struct StatsReport { pub filter, pub total, pub by_project, pub by_role }]`; day and profile breakdowns do not exist. Add `stats::query_by_day` and `query_by_profile` (new functions; do not change `query_report`'s JSON, which the CLI prints). Bucket days **in Rust**: select `(started_at, profile, token columns)` for the `since` window and group with `chrono::Local`, rather than SQLite `'localtime'`. `[VERIFIED: a musl static binary printed the right local offset: unset TZ -> +07:00 (from /etc/localtime), TZ=America/New_York -> -04:00, env -i -> +07:00]`. Overview counts come from one SQL over `tasks` + latest run (running / waiting for you (`planned`,`dev_done`) / needs attention (`blocked`,`qa_fix`, failed/lost/timed_out/killed/protocol_violation) / done (`qa_pass`)); that grouping is a UX judgement `[ASSUMED]` A2 for the UI-SPEC to confirm.

### Anti-Patterns to Avoid

- **Calling `supervise` or `cmd_*` from an async handler.** Panics (verified). All rusqlite, `git`, `std::process`, `kill_tree` (it `thread::sleep`s) and `cmd_kill` calls go through `spawn_blocking`.
- **Holding a prepared statement or read transaction across an `.await`.** WAL checkpoints cannot complete while a reader is open `[CITED: sqlite.org/wal.html "...if there is always at least one active reader, then no checkpoints will be able to complete and hence the WAL file will grow without bound."]`.
- **Sending the token in any URL other than the first `GET /`.** Keep it out of API paths and logs.
- **Binding `0.0.0.0` or `"localhost"`.** Bind `Ipv4Addr::LOCALHOST` explicitly.
- **Serving files from disk or accepting a path parameter for assets.** Table lookup only.
- **Re-implementing kill, slugify, normalize, plan parsing, atomic map save.** Call the Phase 3 functions.
- **Letting the UI start a task by free text.** Only a server-validated project id starts a run.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP server, routing, body limits, SSE framing | Raw hyper/TCP loop or a custom event-stream writer | `axum` 0.8.9 (`Router`, `middleware::from_fn_with_state`, `Sse`/`KeepAlive`, `DefaultBodyLimit`) | Keep-alive, chunking, HTTP/1.1 edge cases; verified working in the probe |
| Process-tree kill | New kill code for the UI | `runner::cmd_kill` / `procs::kill_tree` | S4-verified (PROC-01/02); D-12 says same path |
| Pipeline/step truth | A second status store | Derive from `tasks.status`, `runs.status`, `runs.outcome` | Single source of truth; terminal and UI agree |
| Project matching, slugs | JS re-implementation | `routing::normalize`/`slugify` via the API | Unicode `đ`/NFD handling must match the CLI byte for byte |
| Plan/QA parsing | A markdown parser | `planfile::parse_plan`, `section_text`, `parse_qa_verdict` | Same validation as the CLI; no HTML injection surface |
| Atomic JSON save | `fs::write` | `projects::save_map` (tmp + rename + 0600) | Never leaves a half-written `projects_map.json` |
| Cryptographic randomness | A PRNG | `/dev/urandom` via `std` | OS CSPRNG, no crate |
| CSS | Hand-written utility CSS | Tailwind standalone CLI + committed `app.css` | D-00e; reproducible with a pinned, hash-checked binary |
| Reactive UI | Vanilla DOM plumbing | Alpine CSP build | Small, CSP-clean, fits per-screen files |
| Charts | A charting library | CSS bars | 4 simple breakdowns; nothing to vendor or audit |

**Key insight:** the dashboard adds a *view and a gate*, not new orchestration. Anything that decides, executes or kills already exists; the risk is in wiring (blocking calls in async, signals, auth), not in logic.

## Common Pitfalls

### Pitfall 1: Axum 0.8 path syntax
**What goes wrong:** The spec writes `/api/runs/:id/stream`; in axum 0.8 routes use `{id}` (`/{single}`, `/{*many}`) `[CITED: tokio.rs/blog/2025-01-01-announcing-axum-0-8-0]`.
**How to avoid:** Use `/api/runs/{id}/stream` (this form compiled and served in the probe). Do not copy the spec's `:id`.
**Warning signs:** Panic at router construction on startup.

### Pitfall 2: `serve` hangs on Ctrl+C because SSE connections never finish
**What goes wrong:** `[VERIFIED: probe with an open SSE client, SIGINT sent: process still alive 4 s later]` axum's graceful shutdown waits for open connections.
**How to avoid:** Put a `tokio::sync::watch<bool>` shutdown flag in `AppState`; every SSE loop also selects on it and ends the stream; additionally wrap the serve future in a hard timeout (~3 s after runs are stopped). Order: stop runs first (Pattern 3), then signal SSE, then let `with_graceful_shutdown` finish.

### Pitfall 3: EventSource reconnects forever after a normal close
**What goes wrong:** `[CITED: HTML Standard, server-sent-events]` "Clients will reconnect if the connection is closed"; only a non-200 status or wrong content type fails it, and `close()` stops it. A server that simply finishes the stream after a run ends gets reconnected in a loop.
**How to avoid:** Emit a final `event: end`; the client handler calls `es.close()`. Auto-reconnect during a live run is the desired DASH-05 behaviour: the browser sends `Last-Event-ID` (non-empty only if an `id:` was received) and the server resumes from that `seq`.

### Pitfall 4: `supervise()` inside an async context
Covered in Pattern 3 (panics). Also never share one rusqlite `Connection` between tasks: it is `Send` but not `Sync`; use a `Mutex<Connection>` for GET handlers or `db::open` per call.

### Pitfall 5: PDEATHSIG is tied to the spawning thread
**What goes wrong:** `[CITED: man7.org PR_SET_PDEATHSIG(2const)]` "The 'parent' in this case is considered to be the thread that created this process" and the signal is sent "when that thread terminates". Spawning from a tokio `spawn_blocking` worker (which exits after idle time) would SIGTERM a healthy run. Also cleared when exec'ing a set-user-ID binary (not relevant here).
**How to avoid:** One dedicated, never-exiting spawner `std::thread` forks every child (as in probe2). Waiter threads only `wait()`.

### Pitfall 6: Request text or project id starting with `-`
**What goes wrong:** `[VERIFIED: run]` `agent-brain run lead --dry-run -dash "--help me"` fails with `error: unexpected argument '-d' found ... tip: to pass '-d' as a value, use '-- -d'`; with `--` it works and the Lead prompt contains `Yêu cầu: --help me`.
**How to avoid:** Always `run <role> [--name SLUG] -- <project> [<request>]`; same for `status -- <project>`. Spawn with an argv array, never through a shell, so there is no shell injection either.

### Pitfall 7: Stale `running` rows block start buttons
**What goes wrong:** `reconcile_stale_runs` runs only at `open_ctx` (`app.rs:24`), i.e. at the start of a CLI process. A terminal run killed with `-9` leaves `running` until some CLI starts.
**How to avoid:** The pump reconciles every ~5 s (it only marks rows whose PID+start-time are dead, `runner.rs:32-53`).

### Pitfall 8: Lock FD leaking into children
`std` opens files `O_CLOEXEC`, so children do not inherit the `flock` FD and the lock dies with the server. Do not `dup` the FD or clear CLOEXEC; if it leaked, a crashed server's lock would be held by an orphaned child.

### Pitfall 9: The binary is replaced while `serve` runs
`std::env::current_exe()` then reports a path ending ` (deleted)` and spawns fail. Resolve `current_exe` once at startup; on spawn failure return a clear Vietnamese error ("Binary đã bị thay thế - khởi động lại `agent-brain serve`").

### Pitfall 10: Event inserts can be dropped under extreme contention
`roles/mod.rs:335` is `let _ = crate::db::insert_event(...)`: if a writer waits past the 5 s busy timeout (`db.rs:93`) the row is silently lost and `seq` has a gap. Practically unreachable (tiny autocommit inserts), but the log pane must tolerate gaps and never assume `seq` is dense. First-ever concurrent start of two processes on a brand-new DB could race the migration transaction (`db.rs:100-106`); not a concern once the DB exists.

### Pitfall 11: Cookie scope surprises
Cookies are per host, not per port (`[CITED: RFC 6265 §8.5]`), and the printed URL host must match the cookie host. Print `127.0.0.1` URLs only; do not set `Secure` (plain http).

### Pitfall 12: Offline "no external URL" tests trip on Tailwind's banner
**What goes wrong:** `[VERIFIED]` the generated CSS contains `https://tailwindcss.com` inside the `/*! ... */` license banner.
**How to avoid:** The DASH-09 static test must target loads, not substrings: forbid `@import url(`, `url(http`, `src="http`, `href="http`, `//cdn`, `<link ... href="http` in `index.html`, `screens/*.html`, `js/*.js` and `app.css`; allow the banner (or strip it with a test-side filter).

### Pitfall 13: `.plan.md` is not always in the working tree
If the project's current branch is another task, the file on disk is that other task's plan. Use `git show <branch>:.plan.md` when `current_branch != tasks.branch`.

### Pitfall 14: Alpine CSP build drift
The CSP build changes between 3.x releases and has the template limits listed in Pattern 9. Pin the exact version, keep logic in `Alpine.data`, and keep a headless-Chrome smoke (`google-chrome --headless=new --no-sandbox --virtual-time-budget=4000 --dump-dom http://127.0.0.1:PORT/...`) as a manual verify step; Chrome, Chromium and Firefox are installed on this machine.

### Pitfall 15: Blocking the runtime
A single un-offloaded `git` call or rusqlite query stalls every SSE keep-alive. Use `rt-multi-thread` (two workers is enough) so a slip is a latency bump rather than a freeze, and still offload everything blocking.

## Runtime State Inventory

Not applicable: this phase is additive (no rename/refactor/migration). Only new runtime state: `~/.local/share/agent-brain/serve.lock` (single-instance lock + `{pid,port,started_at}`), no schema migration (the existing `MIGRATIONS[0]` already has every table and column the dashboard reads).

## Code Examples

### Constant-time token compare and cookie lookup (hand-written, 12 lines)
```rust
// Source: probe.rs, verified against curl (401/303/200)
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}
fn cookie_value<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let raw = h.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|kv| { let (k, v) = kv.trim().split_once('=')?; (k == name).then_some(v) })
}
```

### Auth middleware shape (axum 0.8)
```rust
// Source: probe.rs (compiled with axum =0.8.9, features http1/json/query/tokio)
let app = Router::new()
    .route("/api/runs/{id}/stream", get(stream))
    .layer(middleware::from_fn_with_state(st.clone(), auth))   // async fn auth(State<Arc<AppState>>, Request, Next) -> Response
    .with_state(st);
let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
axum::serve(listener, app).with_graceful_shutdown(shutdown_future).await?;
```

### Pump (change hints from SQLite)
```rust
// Source: probe.rs pump(); uses crate::db::open so PRAGMAs match the CLI
let conn = agent_brain::db::open(&db)?;
loop {
    std::thread::sleep(Duration::from_millis(250));
    let ver: i64 = conn.pragma_query_value(None, "data_version", |r| r.get(0))?;
    if ver == last_ver { continue; }
    last_ver = ver;
    // SELECT run_id, MAX(id) FROM events WHERE id > ?last GROUP BY run_id  -> tx.send(run_id); then tx.send(0)
}
```

### Child spawn (see Pattern 3 for the full recipe and verification)

### Tailwind build command
```bash
agent-brain/tools/tailwindcss -i agent-brain/dashboard-src/input.css -o agent-brain/assets/dashboard/app.css --minify
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| axum routes `/:id`, `/*rest` | `/{id}`, `/{*rest}` | axum 0.8.0 (2025-01) `[CITED: tokio.rs blog]` | Spec's `:id` examples must be rewritten |
| `#[async_trait]` extractors | native async fn in traits | axum 0.8 `[CITED: tokio.rs blog]` | Irrelevant unless custom extractors are written (none planned) |
| Tailwind v3 (PostCSS/Node, `tailwind.config.js`) | v4 CSS-first config, standalone CLI binary | Tailwind 4.x `[ASSUMED]` for the dates; the v4.3.3 binary and the `@import`/`@custom-variant`/`@source` syntax were run this session | No Node needed |
| Alpine standard build with `unsafe-eval` | `@alpinejs/csp` build | Alpine 3.x `[CITED: alpinejs.dev/advanced/csp]` | Strict CSP possible, at the cost of expression limits |
| Tailwind via CDN | prebuilt, committed CSS | REQUIREMENTS out-of-scope table | DASH-09 |

**Deprecated/outdated:** `axum::Server` in favour of `axum::serve` `[ASSUMED]` (the probe used `axum::serve`; the removal version was not checked); the spec's CDN suggestion for Tailwind (spec §3.3 already says to prebuild).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Port 7437 is unassigned/unused on typical dev machines | Pattern 2 | Low: D-10 requires a clear "port in use" error, `--port` and `dashboard_port` override; user may prefer 7878 despite Radarr |
| A2 | Overview grouping (running / waiting for you / needs attention / done) is the right UX mapping of task states | Pattern 10 | Low: UI-SPEC or user can re-bucket; data is derived, nothing stored |
| A3 | One `Mutex<Connection>` (or open-per-call) is enough read throughput for a single user | Pattern 4 | Low: reads are sub-millisecond; switch to a small pool if profiling says otherwise |
| A4 | Browsers allow about 6 HTTP/1.1 connections per origin, so one SSE per open task screen and polling elsewhere is prudent | Pattern 4 | Low: if wrong, tabs could hold more streams; design stays valid |
| A5 | Final release binary grows roughly 0.5-1.5 MB with axum (probe binary alone is 3.66 MB static-pie; baseline agent-brain is 6.12 MB) | Summary | Low: no size requirement exists; measure in the build plan |
| A6 | Alpine CSP 3.17.4 supports every directive the five screens need (verified: `x-data`, `x-for`, `x-text`, `x-model`, `@click` with a method, `:class`, `:style` with concatenation; not tested: `x-show`, `x-transition`, `$refs`, `$watch`, `@keydown`) | Pattern 9 | Medium: a screen may need a different expression style; mitigated by an early UI tracer and the CSP limits being documented |
| A7 | Showing `.plan.md` as sectioned plain text (no markdown render) is acceptable for D-01 "renders `.plan.md`" | Pattern 5 | Low-medium: user may expect rendered markdown; UI-SPEC should confirm |
| A8 | Not storing the token on disk (so a second `serve` cannot reprint it) satisfies D-14's "message pointing to it" | Pattern 1 | Low: if the user wants the URL re-printed, store it 0600 in `serve.lock`; trade-off is a token at rest |

## Open Questions

1. **Record Lead protocol violations in the DB?**
   - What we know: `lead.rs:211-221` returns `EXIT_PROTOCOL` for out-of-scope edits without `set_run_outcome`; Dev/QA record `protocol_violation` (`dev.rs:277`, `qa.rs:211`).
   - What's unclear: whether the planner accepts a one-line core change for parity so terminal-started Lead violations show in the UI.
   - Recommendation: add `crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;` before the return, with a test; otherwise the UI can only show it for UI-started jobs (in-memory exit code 3).
2. **Should the UI show/kill `inspect` runs started from a terminal?**
   - What we know: `runs.role` allows `inspect` and they have no `task_id`, so `cmd_kill` cannot target them.
   - Recommendation: list task runs only in Phase 5; leave `inspect` out.
3. **Request length limit.** CLI has none beyond argv size; recommend 20,000 characters and a 64 KB JSON body limit via `DefaultBodyLimit`.
4. **`system/thinking_tokens` default visibility.** Recommend hidden by default with a toggle (data supports it); confirm in UI-SPEC.
5. **Default port confirmation (A1).** Planner may keep 7437 or switch to the CONTEXT's 7878.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain + musl target | build, DIST-01 | yes | rustc/cargo 1.98.1, `x86_64-unknown-linux-musl` installed | - |
| `musl-gcc` (route A) | static build | yes | `/usr/bin/musl-gcc` | build-release.sh route B (gcc + CFLAGS) |
| Network (crates.io, GitHub, npm) | user checkpoints only | yes (this session) | - | none needed after the checkpoints |
| New crates in `~/.cargo/registry` | `--offline --locked` builds | **no** (22 packages missing) | - | User checkpoint: `cargo fetch` (Standard Stack) |
| Tailwind standalone CLI | regenerating `app.css` | not in repo (downloaded to scratch only) | v4.3.3 | Commit `app.css` once; later edits need the binary or a hand edit |
| Node / npm | `npm pack @alpinejs/csp` (one-time) and optional `node --check` | yes | node v24.21.0 | Download the tarball with curl from the npm registry |
| python3 | fake claude (`tests/fakes/*.py`), writers in tests | yes | 3.12.3 | - |
| git | task branches, `git show` | yes | 2.43.0 | - |
| curl | build-release smoke, manual checks | yes | 8.5.0 | bash `/dev/tcp` |
| Headless browser | UI smoke / UAT | yes | google-chrome, chromium, firefox present | manual browser |
| `unshare -rn` | proving offline CSS build | yes (user namespaces permitted) | - | disable the network manually |
| `claude` CLI | live acceptance only | yes (used by Phase 3 live checks) | 2.1.301 per LIVE-CHECKS | tests use fakes |

**Missing dependencies with no fallback:** none. **Missing with fallback:** the 22 crates (checkpoint) and the Tailwind binary (checkpoint, dev-time only).

## Validation Architecture

(`workflow.nyquist_validation` is not set in `.planning/config.json`, so it is enabled.)

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust 1.98.1) with `tempfile`; existing `tests/common/mod.rs` `Sandbox` (temp HOME, XDG dirs, `AGENT_BRAIN_DB`, `AGENT_BRAIN_CONFIG`, fake claude via `claude_bin`) `[VERIFIED: tests/common/mod.rs]`; Python `tests/fakes/fake_claude.py` and `fake_claude_tree.py` |
| Config file | none beyond `Cargo.toml`; new tests `agent-brain/tests/dashboard_*.rs` |
| Quick run command | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib` plus the touched integration test, e.g. `--test dashboard_http` |
| Full suite command | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked && python3 -m unittest discover -s tests -t . -q` |
| HTTP client in tests | none new: a ~40-line raw `std::net::TcpStream` helper (send request, read until close; for SSE search the raw bytes for `id: N`, chunk framing is irrelevant to substring checks) |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DASH-01 | `serve --port 0` prints URL+token; `GET /` (after handshake) 200, `/app.css` 200 with a Tailwind banner | integration | `cargo test ... --test dashboard_http dash01` | Wave 0 |
| DASH-01 (dist) | binary serves under `env -i`, `PATH=/nonexistent` | script | `bash scripts/build-release.sh` (new smoke step) | extend existing |
| DASH-02 | listening socket is `127.0.0.1` only (parse `/proc/net/tcp`: `0100007F:<port>` state `0A`, no `00000000:<port>`); source has no `0.0.0.0` | integration + grep | `cargo test ... --test dashboard_http dash02` | Wave 0 |
| DASH-03 | no cookie/wrong cookie -> 401 on `/`, `/api/*`, SSE; `?token=` -> 303 + attributes; Host mismatch -> 421; POST with bad Origin -> 403; token differs across two starts | integration | `... dash03` | Wave 0 |
| DASH-04 | POST `/api/tasks` with fake claude Lead scenario (`tests/support/protocol.rs`) -> job -> run row, SSE rows, steps Lead running -> done, task `planned` | integration | `cargo test ... --test dashboard_run dash04` | Wave 0 |
| DASH-05 | SSE resume with `Last-Event-ID`; terminal-started run (test launches `agent-brain run` directly, not via serve) streams live | integration | `... dash05` | Wave 0 |
| DASH-06 | POST kill with `fake_claude_tree.py`: after < 5 s no recorded descendant alive (`procs::alive`), run `killed`; plus server SIGINT and SIGKILL cases (D-08) | integration | `... --test dashboard_kill dash06` | Wave 0 |
| DASH-07 | ambiguous/unknown project POST -> 409 with candidates, zero rows in `runs`, fake-claude record file empty; `suggest` ranking and accent-insensitive match | integration + unit | `... dash07` and `cargo test --lib routing::` | Wave 0 |
| DASH-08 | `?profile=work|personal` filters tasks/stats/projects; start never takes a profile (child env shows the project's `CLAUDE_CONFIG_DIR`); second personal start refused with the CLI text; project busy refused | integration | `... dash08` | Wave 0 |
| DASH-09 | static: no external loads in embedded assets (Pitfall 12 patterns); `app.css` non-empty and starts with the Tailwind banner; manual: UAT with network off | unit + manual | `cargo test ... --test dashboard_assets` | Wave 0 |
| DASH-10 | N/A (D-18) | - | record "not applicable" in VERIFICATION | - |
| DASH-11 | `/api/stats` returns `by_day`, `by_profile`, `by_project`, `by_role` for seeded runs (day buckets via `chrono::Local`, check around midnight with an injected offset); overview counts | unit + integration | `cargo test ... stats::` and `--test dashboard_api` | Wave 0 |
| DASH-12 | list; alias edit preserves other keys and order; etag mismatch -> 409; duplicate -> warning not error; rescan via child; DB synced after save | integration | `... --test dashboard_projects` | Wave 0 |
| UI smoke | headless Chrome `--dump-dom` of the served shell shows the mounted screen (CSP intact) | manual/script | `google-chrome --headless=new --no-sandbox --virtual-time-budget=4000 --dump-dom <url>` | manual |
| JS syntax | every `js/*.js` parses | script | `for f in agent-brain/assets/dashboard/js/*.js; do node --check "$f"; done` | Wave 0 |

Manual acceptance (checkpoints, as in 03-13): P4-04..P4-09 in a real browser against a real `claude`; P4-06 live with `python3 -c 'import time; time.sleep(307)'`; P4-02 from another LAN machine; P4-09 with the network disabled.

### Sampling Rate
- **Per task commit:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib` + the touched `dashboard_*` test.
- **Per wave merge:** full suite command.
- **Phase gate:** full suite green + the manual P4 rows before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `agent-brain/tests/dashboard_http.rs` (+ a shared `tests/common` raw-HTTP helper) - DASH-01/02/03
- [ ] `dashboard_run.rs`, `dashboard_kill.rs`, `dashboard_api.rs`, `dashboard_projects.rs`, `dashboard_assets.rs`
- [ ] Cargo checkpoint (axum, futures-util, tokio features) so any of the above can compile offline
- [ ] `assets/dashboard/` placeholders so `include_bytes!` compiles from the first server plan

## Security Domain

(`security_enforcement` is not set to false in config, so this section is required.)

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | Per-start 256-bit token, one-time URL handoff, constant-time compare; no accounts (out of scope) |
| V3 Session Management | yes | Session cookie `HttpOnly; SameSite=Strict; Path=/`, invalidated by restart (new token); no `Max-Age` |
| V4 Access Control | yes | A single middleware gates every route, including SSE and static assets; Host allow-list; Origin check on writes |
| V5 Input Validation | yes | Project ids validated through `routing::resolve`; slugs through `routing::slugify`; request text length cap; JSON body limit; ids/seqs parsed as integers; argv arrays + `--`, never a shell |
| V6 Cryptography | yes | `/dev/urandom` only; no custom crypto; token never logged and never written to disk |
| V7 Error Handling and Logging | yes | Return Vietnamese messages from `BrainError` only; no stack traces; do not log URLs containing `token=` |
| V8 Data Protection | yes | `brain.db` already 0600 (`db.rs:67-90`); `serve.lock` 0600; `Referrer-Policy: no-referrer`; `Cache-Control: no-store` on APIs |
| V12/V13 API and Web Service | yes | JSON only, `Content-Type` required on writes, no CORS headers (same origin only) |
| V14 Configuration | yes | CSP `script-src 'self'` with no `unsafe-eval`; `frame-ancestors 'none'`; `nosniff` |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| DNS rebinding to `127.0.0.1` | Spoofing / Elevation | Host allow-list (`127.0.0.1:port`, `localhost:port`), 421 otherwise (verified) |
| Cross-site request forgery from any web page | Tampering | SameSite=Strict + Origin check + JSON content type |
| Stored XSS via agent output / plan text | Tampering / Info disclosure | `x-text` only, CSP build forbids `x-html`, `script-src 'self'` with no inline scripts |
| Token leakage (history, Referer, logs) | Info disclosure | One-time `?token=` on `GET /` then 303 to `/`, `Referrer-Policy: no-referrer`, no token logging |
| Cookie sent to another localhost service | Info disclosure | Session cookie, per-start token; RFC 6265 §8.5 limit documented and accepted |
| Command injection through request/slug/project | Elevation | `Command::args` array, `--` separator, server-side resolve/slugify, no shell |
| Path traversal | Info disclosure | No disk serving; fixed asset table; plan files located via DB branch + git, not a client path |
| LAN exposure | Info disclosure / Elevation | Bind `Ipv4Addr::LOCALHOST` only; asserted from `/proc/net/tcp` |
| Orphaned runs after server crash/exit | Denial / Elevation | PDEATHSIG + SIGINT forwarding + SIGHUP handling; verified |
| Resource exhaustion (SSE, huge bodies) | Denial | One SSE per task screen, `DefaultBodyLimit`, bounded log ring and row counts |

Design reality to keep visible in docs: the dashboard can start agents that edit code, so the token is equivalent to shell-level access to the project folders; it is for one user on one machine (spec §2).

## Plan Slicing Suggestion (for the planner; waves not mandatory)

- **05-01 (checkpoints, 03-01 style):** `Cargo.toml` edit (executor, no network) -> `checkpoint:human-verify` on the legitimacy table (note the SUS flag on `@alpinejs/csp`) -> `checkpoint:human-action`: `cargo fetch`, offline test, musl build, commit `Cargo.lock`; and the Tailwind + Alpine download/verify steps with hashes. `/gsd-ui-phase 5` can run before or alongside to produce the UI-SPEC.
- **Wave 1 (tracer):** `serve` subcommand, config `dashboard_port`, lock + bind order, auth/headers, asset table with a placeholder page, graceful shutdown; tests DASH-01/02/03. Proves the musl build and DIST-01 smoke early.
- **Wave 2 (read side):** `routing::suggest`, projects/tasks/stats/runs read APIs, `taskstate`, pump + SSE + raw-event endpoint; tests DASH-05/11 and the read halves of 07/08.
- **Wave 3 (write side):** jobs (spawner thread, PDEATHSIG, SIGINT/SIGHUP, ring buffers), POST tasks/dev/qa/relead, preflight, kill, projects PUT/scan; tests DASH-04/06/07/08/12 and D-08 crash/graceful.
- **Wave 4 (UI):** shell + theme + router + profile switcher + API helper, then one `agy` task per screen (overview, task, assign, stats, projects), then `build-css.sh` and the committed CSS; static tests DASH-09 and `node --check`.
- **Wave 5 (acceptance):** build-release smoke, README (`serve`, config key, security notes), live checkpoints P4-02/04/05/06/07/08/09 with VERIFICATION.

## Sources

### Primary (HIGH confidence)
- Local code read this session: `agent-brain/src/{runner,db,app,config,main,projects,routing,stats,procs,term,stream,planfile,fastpath,tasks,gitops}.rs`, `src/roles/{mod,lead,dev,qa}.rs`, `Cargo.toml`, `Cargo.lock`, `assets/config.default.json`, `tests/common/mod.rs`, `tests/assets_parity.rs`, `tests/runner_kill.rs`, `scripts/build-release.sh`, `.planning/{REQUIREMENTS,STATE}.md`, `files/05-phase4-dashboard.md`, `03-01-PLAN/SUMMARY.md`, `03-RESEARCH.md` (structure), README.
- crates.io API (axum versions, features, dependencies), `package-legitimacy` seam (22 crates + 2 npm), `npm view/pack` (alpine), GitHub release assets and `sha256sums.txt` (Tailwind v4.3.3).
- Experiments in the scratchpad (not committed): `probe` (axum auth/SSE/pump), `probe2` (child spawn, PDEATHSIG, SIGINT), `probe3` (`supervise` from async vs `spawn_blocking`), `probe4` (flock), `probe5` (`chrono::Local` on musl), musl release build (route A, 2 m 05 s cold, `static-pie linked`, no `NEEDED`), Tailwind build in `unshare -rn`, headless-Chrome smoke of Alpine CSP + strict CSP.

### Secondary (MEDIUM confidence, official docs fetched)
- sqlite.org/pragma.html#pragma_data_version; sqlite.org/wal.html
- html.spec.whatwg.org/multipage/server-sent-events.html (reconnect, `Last-Event-ID`, `close()`, non-200 behaviour)
- man7.org PR_SET_PDEATHSIG(2const) (thread semantics)
- alpinejs.dev/advanced/csp (CSP build limits)
- tokio.rs/blog/2025-01-01-announcing-axum-0-8-0 (`{id}` path syntax)
- developer.mozilla.org Set-Cookie; datatracker.ietf.org RFC 6265 §8.5
- docs.rs/axum/0.8.9/axum/response/sse (module listing; method detail not covered by the fetch, verified by compiling instead)

### Tertiary (LOW confidence)
- Web search result for Radarr/Sonarr default ports (docs.seerr.dev, ssdnodes.com): used only to avoid 7878.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - versions read from crates.io/npm/GitHub, lockfile delta computed, musl static build executed.
- Architecture (SQLite-following SSE, child-process runs, auth): HIGH - each piece prototyped and exercised with curl, a separate writer process, fake-claude tree kill, SIGINT and SIGKILL of the parent.
- UI layer (Alpine CSP, fragments, CSS bars): MEDIUM - verified core features in headless Chrome under the final CSP; the full five-screen behaviour is untested (A6).
- Pitfalls: HIGH for those marked VERIFIED (axum shutdown hang, `supervise` panic, `--` requirement, Tailwind banner); MEDIUM for documentation-cited ones.

**Research date:** 2026-10-17
**Valid until:** 2026-11-16 (axum/tower/hyper patch versions and Alpine/Tailwind release weekly; re-check the lockfile delta and the Alpine SUS flag at the 05-01 checkpoint)
