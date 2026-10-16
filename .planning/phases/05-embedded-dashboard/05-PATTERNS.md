# Phase 5: Embedded Dashboard - Pattern Map

**Mapped:** 2026-10-17
**Files analyzed:** 27 new/modified (Rust 15, front-end assets 10, scripts/tests 2 groups)
**Analogs found:** 17 / 27 (front-end assets, auth, SSE, spawner and lock have no in-repo analog; use RESEARCH.md Patterns 1-4, 8, 9)

All analog paths below were confirmed git-tracked (`git ls-files agent-brain`). Line numbers come from reading the files this session; RESEARCH.md line cites for `db.rs`/`stats.rs`/`lead.rs` may be off by a few lines, so trust the numbers here.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `agent-brain/Cargo.toml` (modify) | config | n/a | itself (lines 9-20, exact-pin style `=x.y.z`) | exact |
| `agent-brain/src/lib.rs` (modify: `pub mod dashboard;`) | config | n/a | itself (lines 3-20) | exact |
| `agent-brain/src/main.rs` (modify: `Commands::Serve`) | controller (CLI dispatch) | request-response | `Commands::Guard` pre-`open_ctx` branch, `main.rs:130-133` | exact |
| `agent-brain/src/config.rs` (modify: `dashboard_port`) | config | transform | `kill_grace_secs` handling, `config.rs:15-27`, `149-153`, `215` | exact |
| `agent-brain/assets/config.default.json` (modify) | config | n/a | itself, `"kill_grace_secs": 3` | exact |
| `src/dashboard/mod.rs` (serve, AppState, router, shutdown) | controller/service | request-response | `main.rs` error-to-exit mapping 135-145, 181-191; `app.rs` Ctx | role-match |
| `src/dashboard/auth.rs` | middleware | request-response | none (RESEARCH Pattern 2 + Code Examples) | none |
| `src/dashboard/assets.rs` | utility | file-I/O (embedded) | `config.rs:6` `include_str!` | partial |
| `src/dashboard/lock.rs` | utility | file-I/O | none (RESEARCH Pattern 1) | none |
| `src/dashboard/pump.rs` | service | event-driven | `runner::reconcile_stale_runs` (runner.rs ~32-52) + `db::open` | partial |
| `src/dashboard/sse.rs` | controller | streaming | none (RESEARCH Pattern 4) | none |
| `src/dashboard/jobs.rs` | service | event-driven | `tests/runner_kill.rs` spawn + `procs::*`; `runner::cmd_kill` | partial |
| `src/dashboard/taskstate.rs` | utility | transform (pure) | `tasks.rs` (task lookup) + `fastpath.rs` | role-match |
| `src/dashboard/api_projects.rs` | controller | CRUD | `projects::select_project` 143-176, `save_map`, `cmd_scan` 641-732 | role-match |
| `src/dashboard/api_tasks.rs` / `api_runs.rs` | controller | request-response/CRUD | `runner::cmd_kill` 55-130, `db::get_task`, `running_runs` | role-match |
| `src/dashboard/api_stats.rs` | controller | request-response | `stats::query_stats` 179-186 | exact |
| `src/routing.rs` (modify: `suggest`) | utility | transform | `routing::resolve` 73-122 | exact |
| `src/stats.rs` (modify: `query_by_day`, `query_by_profile`) | service | CRUD (read) | `stats::query_report` 72-176 | exact |
| `src/roles/lead.rs` (optional one-liner `set_run_outcome`) | service | CRUD | `dev.rs` / `qa.rs` `set_run_outcome` calls | exact |
| `assets/dashboard/index.html`, `screens/*.html`, `js/*.js`, `app.css`, `vendor/alpine.csp.min.js`, `VENDOR.md` | component | request-response | none in repo | none |
| `dashboard-src/input.css`, `scripts/build-css.sh` | config/script | batch | `scripts/build-release.sh` | role-match |
| `scripts/build-release.sh` (modify: serve smoke) | script | batch | itself lines 38-70 | exact |
| `tests/dashboard_*.rs` | test | request-response | `tests/runner_kill.rs`, `tests/fastpath.rs`, `tests/common/mod.rs` | exact |
| `tests/assets_parity.rs` (extend, DASH-09 static check) | test | transform | itself | exact |

## Pattern Assignments

### `agent-brain/src/main.rs` (modify: add `Serve`)

**Analog:** `agent-brain/src/main.rs`

**Subcommand declaration** (lines 20-81): clap derive, Vietnamese doc comments, one variant per command.
```rust
/// Dừng tiến trình của task
Kill {
    /// ID của task
    task_id: i64,
},
...
Guard {
    /// Đường dẫn tới file chính sách
    #[arg(long)]
    policy: Option<PathBuf>,
},
```
Add `/// Mở dashboard (127.0.0.1)` `Serve { #[arg(long)] port: Option<u16> }`.

**Pre-`open_ctx` early exit** (lines 130-133). Copy for `Serve` so the lock and bind happen before any DB write (RESEARCH Pattern 1):
```rust
if let Commands::Guard { policy } = &cmd {
    let code = guard::cmd_guard(policy.as_deref());
    std::process::exit(code);
}
```
Also add `Commands::Serve { .. } => unreachable!()` to the later `match cmd` (line 148 does the same for `Guard`).

**Error mapping** (lines 135-145 and 181-191): `EXIT_ROUTING` goes to `term::emit_out`, everything else to `term::emit`, then `std::process::exit(e.code)`. A port-in-use error must use this path with exit code 1.

---

### `agent-brain/src/config.rs` (modify: `dashboard_port`)

**Analog:** `kill_grace_secs` end to end.

Struct field (line 25): `pub kill_grace_secs: u64,` -> add `pub dashboard_port: u16,`.
Parse with default (lines 149-153):
```rust
let kill_grace_secs = obj
    .get("kill_grace_secs")
    .and_then(|n| n.as_u64())
    .unwrap_or(3);
```
Struct literal (lines 193-216, `kill_grace_secs,` at 215): add the new field. Also add `"dashboard_port": 7437` to `assets/config.default.json` (keys are alphabetical there). Precedence flag > config > default. Check `tests/assets_parity.rs` and any test that builds `Config { .. }` by literal, since a new field breaks them.

---

### `agent-brain/src/routing.rs` (modify: add `suggest`)

**Analog:** `routing::resolve`, lines 73-122 (same file). `resolve` is exact-equality, so `suggest` is a sibling with substring ranking.

Reuse the per-project match loop shape (lines 88-107):
```rust
let norm_q = normalize(stripped_query);
for p in projects {
    if normalize(&p.id) == norm_q { ... }
    else if normalize(&p.name_vi) == norm_q { ... }
    else { for alias in &p.aliases { if normalize(alias) == norm_q { ... } } }
}
```
`suggest(query, &[Project]) -> Vec<(Project, rank)>`: `normalize(q)` contained in `normalize(id|name_vi|alias)`, rank exact > prefix > contains. Pure function, tests go in `tests/contract_routing.rs` style. `slugify` (47-71) is already what `/api/slug` should call.

---

### `agent-brain/src/stats.rs` (modify: `query_by_day`, `query_by_profile`)

**Analog:** `stats::query_report`, lines 72-176; `query_stats` 179-186.

**Filter idiom** (lines 54-65): optional filters as nullable params, never string-built SQL:
```rust
WHERE (?1 IS NULL OR runs.profile = ?1)
  AND (?2 IS NULL OR runs.started_at >= ?2)
...
params![profile, since_ms],
```
Add the new functions beside it and do not change `StatsReport` (it is the CLI's JSON, lines 13-19). Wrapper pattern (lines 179-186): `serde_json::to_value(&report).map_err(|e| BrainError::error(format!("Lỗi serialize JSON: {e}")))`. Day bucketing in Rust with `chrono::Local` (the `chrono` features `clock`,`std` in `Cargo.toml:20` suffice; `term.rs:22-24` already uses `chrono::Local::now()`).

---

### `src/dashboard/api_*.rs` (controllers)

Handlers are async but every rusqlite/git/`std::process` call goes through `spawn_blocking` with a fresh connection or `Ctx`, never a shared `Connection` (RESEARCH Pitfalls 4, 15).

**Open a Ctx** (`app.rs:14-33`):
```rust
pub fn open_ctx_with(cfg: Config, map_path: &Path, db_path: &Path) -> BrainResult<Ctx> {
    let map = crate::projects::load_map(map_path)?;
    let conn = crate::db::open(db_path)?;
    crate::runner::reconcile_stale_runs(&conn)?;
    let projects = crate::projects::projects_of(&map);
    crate::db::sync_projects(&conn, &projects)?;
    Ok(Ctx { cfg, conn, map, projects })
}
```
Handlers call `open_ctx_with(cfg.clone(), &config::map_path(), &config::db_path())` inside `spawn_blocking`. `AppState` holds `Config` (Clone) and paths, not a Ctx.

**Kill handler** wraps `runner::cmd_kill` (`runner.rs:55-130`). Shape to mirror, including the BrainError messages that the UI should surface verbatim:
```rust
pub fn cmd_kill(ctx: &mut Ctx, task_id: i64) -> BrainResult<i32> {
    let task = crate::db::get_task(&ctx.conn, task_id)?;
    if task.is_none() { return Err(BrainError::error(format!("Không tìm thấy task {task_id}"))); }
    let running = crate::db::running_run_for_task(&ctx.conn, task_id)?;
    ...
    crate::db::mark_kill_requested(&ctx.conn, run.id)?;
    let force = std::sync::atomic::AtomicBool::new(false);
    let report = crate::procs::kill_tree(root, Duration::from_secs(ctx.cfg.kill_grace_secs), &force);
```
It blocks up to `kill_grace_secs` plus a few seconds, so `spawn_blocking` is mandatory. It also calls `term::emit` (stderr), which is harmless for the server.

**Project gate for POST** (`projects.rs:143-176`): call `projects::select_project(query, &map)`; on `Err` with `code == EXIT_ROUTING` return HTTP 409 with the candidate list and do not spawn. `BrainError { message, code }` fields are public (`error.rs:9-27`). Map: EXIT_ROUTING -> 409, other BrainError -> 422/500 with `message` as the Vietnamese reason.

**Projects screen writes**: `projects::load_map` (93), `projects::projects_of` (85), `projects::find_duplicates` (597, return as warnings, do not refuse), `projects::save_map` (321, atomic tmp+rename, 0600), then `db::sync_projects`. Rescan mirrors `projects::cmd_scan` (641-732), which warns on duplicates and errors on empty `scan_roots` with the text at ~692-696; in the UI run it as a child `agent-brain projects scan`.

**Stats**: `api_stats.rs` calls `stats::query_stats(&conn, profile, since_ms)` (179) with `stats::parse_since(s, db::now_ms())` (45), and validates the profile like `cmd_stats` does with `crate::profile::check_profile(prof, &ctx.cfg.profiles)?` (stats.rs ~195-197).

**Status endpoint**: model on `fastpath::cmd_status` (`fastpath.rs:6-31`): `select_project`, `project.root()`, `gitops::is_repo_root`, `gitops::current_branch`, `gitops::git(&root, &["status","--short"])`. The CLI prints with `term::emit_out`; the API must return the same facts as JSON and not call `cmd_status` (it prints).

**Plan/QA content**: use `planfile::parse_plan`, `validate_plan`, `has_blockers`, `section_text(text, "Vướng mắc")`, `parse_qa_verdict` (planfile.rs 312, 397, 519, 284, 549), `gitops::find_commit_by_subject` (142), `gitops::changed_paths` (54), `gitops::git(root, &["show", ...])` for non-current branches.

---

### `src/dashboard/jobs.rs` (spawner thread, child `agent-brain run`)

**Analog (partial):** `tests/runner_kill.rs:60-84` builds the exact child command and spawns it; the Sandbox `command()` (`tests/common/mod.rs:241-265`) shows env handling. Production code must `Command::new(current_exe)` with `["run", role, "--name", slug, "--", project, request]` (the `--` is mandatory, RESEARCH Pitfall 6; `main.rs:28-44` defines `Run { role, project, request, name, dry_run, verbose }`).

**Kill and liveness primitives to reuse** (`procs`): `procs::ProcId { pid, start }`, `procs::alive`, `procs::proc_id`, `procs::descendants`, `procs::signal_proc`, `procs::kill_tree(root, grace, &AtomicBool)`. For stragglers on shutdown: `kill_tree(child, Duration::ZERO, &AtomicBool::new(true))`.

No in-repo analog for `PR_SET_PDEATHSIG`, `process_group(0)` or SIGINT forwarding: copy RESEARCH Pattern 3 recipe. Use the same `libc =0.2.190` already in `Cargo.toml:14`.

---

### `src/dashboard/pump.rs`

**Analog (partial):** `runner::reconcile_stale_runs(conn)` (`runner.rs` ~32-52, uses `db::mark_status(conn, run.id, "lost")` and `procs::alive`); call it every ~5 s from the pump. Open the pump's own read connection with `db::open(&db_path)` (`db.rs:43`, sets the WAL/busy-timeout PRAGMAs; do not use a bare `Connection::open`). `data_version` polling and the broadcast of hints: RESEARCH Code Examples "Pump".

---

### `src/dashboard/sse.rs`, `auth.rs`, `lock.rs`, `assets.rs`

No analog in the repo (no HTTP code exists yet). Copy from RESEARCH.md: SSE Pattern 4 (cursor = `events.seq`, `Last-Event-ID`, final `event: end`); Pattern 2 + Code Examples (`ct_eq`, `cookie_value`, `middleware::from_fn_with_state`, Host/Origin checks, CSP header string); Pattern 1 (`flock` single instance); Pattern 8 (`static ASSETS: &[(&str,&str,&[u8])]` via `include_bytes!`). The only in-repo precedent for embedding is `config.rs:6`:
```rust
pub const DEFAULT_CONFIG_JSON: &str = include_str!("../assets/config.default.json");
```

**Event row text for SSE**: use `stream::classify(raw.as_bytes())` then `term::event_text(&ev, false)` (`term.rs:26`); `event_text` returns text with no timestamp, which is what the log pane wants. Do not use `event_line` (`term.rs:173`, stamps the clock). `events` columns: `id, run_id, seq, ts, kind, raw_json` with `UNIQUE(run_id, seq)` (`db.rs:29-30`); `kind` already holds classes like `system/init`, `bad_json`.

---

### `src/dashboard/taskstate.rs` (pure derivation)

**Analog:** `tasks.rs` (task resolution from branch/DB) for the DB access style; the truth tables come from existing writes: `set_task_status` values `planned`, `dev_done`, `blocked`, `qa_fix`, `qa_pass`, `open`; `set_run_outcome` values `dev:blocked`, `qa:fix`, `qa:pass`, `protocol_violation`; run statuses `running|succeeded|failed|timed_out|killed|lost` (`db.rs:18`). `runs.exit_code` is claude's own code, not the CLI's 0/2/3/4/130. Keep it a pure function `(task row, latest run per role, git facts) -> steps + next actions` so it unit-tests without HTTP. DB helpers: `db::get_task` (192), `db::latest_run_for_task` (505), `db::running_runs` (485), `db::running_run_for_task` (494), `db::find_task_by_branch` (216).

Busy rule (D-04/D-17): the CLI enforces only the personal rule, `roles/mod.rs` ~243-246 message `"Profile personal đang có run {} chạy (task {tid_str}) — ..."`. Reuse that wording for the disabled-button reason; add the per-project check in the server from `db::running_runs`.

---

### `tests/dashboard_*.rs` (new integration tests)

**Analog:** `tests/runner_kill.rs` + `tests/common/mod.rs` (Sandbox).

**Test scaffold** (`runner_kill.rs:1-12`, `fastpath.rs:1-30`):
```rust
mod common;
use common::Sandbox;
let sb = Sandbox::new();
let repo = sb.make_repo("web-ban-hang", &[("README.md", "# web-ban-hang\n")], None, None);
let entry = sb.project_entry(&repo, "web-ban-hang", "work");
sb.write_map(serde_json::json!([entry]), &[]);
sb.write_config(serde_json::json!({ "kill_grace_secs": 1 }));
```
Spawn `serve` with `sb.command(&["serve", "--port", "0"], &[])` (`common/mod.rs:241-265` clears the env and sets HOME/XDG/AGENT_BRAIN_DB/AGENT_BRAIN_CONFIG, so tests never touch the real HOME) and `cmd.stdout(Stdio::piped())`, then parse the printed URL for the real port. HTTP via raw `std::net::TcpStream` (no HTTP client dev-dependency; `[dev-dependencies]` only has `tempfile`). Use `fake_claude.py` / `fake_claude_tree.py` (`runner_kill.rs:20-26`, `FAKE_TREE_PIDFILE`, `FAKE_TREE_MODE`, `FAKE_TREE_SLEEP`) for run and kill tests; the survivors check uses `procs::descendants` / `procs::alive` and a `Drop` guard that SIGKILLs leftovers (`runner_kill.rs:28-37`). Sleeps in live tests must be `python3 time.sleep` (MEMORY note).

Add any new Sandbox helper (e.g. `serve()` that waits for the URL line) to `tests/common/mod.rs` rather than duplicating.

---

### `scripts/build-release.sh` (modify) and `scripts/build-css.sh` (new)

**Analog:** `scripts/build-release.sh`. Conventions to copy: `set -euo pipefail`, `SCRIPT_DIR`/`REPO_ROOT` resolution (lines 3-6), Vietnamese error messages on stderr with `exit 1`, the isolated-env smoke pattern (lines 38-70):
```bash
if ! env -i HOME="$TMP/home" PATH=/nonexistent "$TMP/bin/agent-brain" --version >/dev/null 2>&1; then
  echo "Lỗi: kiểm tra --version trong môi trường rỗng thất bại" >&2
  exit 1
fi
```
Add a `serve --port 0` smoke in the same style (start in background, read the URL, curl the token handshake and `/app.css`, SIGINT, assert clean exit). `build-css.sh` must never run in the release path (build stays offline; RESEARCH Pattern 8).

## Shared Patterns

### Vietnamese user-facing errors with exit codes
**Source:** `agent-brain/src/error.rs:1-48`
**Apply to:** all dashboard Rust that can fail (startup, handlers)
```rust
pub const EXIT_OK: i32 = 0; EXIT_ERROR = 1; EXIT_ROUTING = 2; EXIT_PROTOCOL = 3; EXIT_BLOCKED = 4; EXIT_INTERRUPTED = 130;
BrainError::new(message, code) / BrainError::error(message)   // code 1
impl From<std::io::Error> for BrainError   // "Lỗi I/O: {e}"
impl From<rusqlite::Error> for BrainError  // "Lỗi CSDL: {e}" (db.rs:35-39)
```
Return `BrainResult<_>`; HTTP handlers translate `BrainError.message` into the JSON `{"error": "..."}` body.

### DB access
**Source:** `db::open` (`db.rs:43`), `db::now_ms` (39), `db::sync_projects` (111). Always open through `db::open` so WAL and busy timeout match the CLI. One connection per blocking call or per thread; `rusqlite::Connection` is not `Sync`.

### Paths and config
**Source:** `config.rs:71-102` (`config_dir`, `data_dir`, `map_path`, `db_path` with `AGENT_BRAIN_DB` override, `runs_dir`). Put `serve.lock` under `config::data_dir()` (alongside `brain.db`), never a hard-coded `~/.local/share`.

### Pinned dependencies and offline build
**Source:** `agent-brain/Cargo.toml:9-20`: exact `=x.y.z` pins, `default-features = false` where possible (see `chrono`). New lines per RESEARCH "Core": tokio gains `rt-multi-thread`, `net`; add `axum`, `futures-util`. The lockfile fetch is a user checkpoint; the executor must not run `cargo fetch`.

### Never re-implement core logic
`routing::normalize`/`slugify`/`resolve`, `projects::*`, `runner::cmd_kill`, `procs::kill_tree`, `planfile::*`, `stats::*`, `gitops::*`: call them; do not port to JS or duplicate in Rust.

### Time
`db::now_ms()` for ms timestamps; `chrono::Local` for display/bucketing (`term.rs:22-24`).

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `src/dashboard/auth.rs` | middleware | request-response | No HTTP code exists; use RESEARCH Pattern 2 |
| `src/dashboard/sse.rs` | controller | streaming | No streaming server code; RESEARCH Pattern 4 |
| `src/dashboard/lock.rs` | utility | file-I/O | No flock/single-instance code; RESEARCH Pattern 1 |
| `src/dashboard/assets.rs` | utility | embedded files | Only `include_str!` of one JSON exists (`config.rs:6`); RESEARCH Pattern 8 |
| `assets/dashboard/**` (HTML, Alpine CSP JS, Tailwind CSS) | component | request-response | No front-end exists; RESEARCH Patterns 8-9, UI-SPEC if produced |
| PDEATHSIG / SIGINT-forward spawner internals | service | event-driven | Only test-side spawns exist (`tests/runner_kill.rs`); RESEARCH Pattern 3 |

## Metadata

**Analog search scope:** `agent-brain/src`, `agent-brain/src/roles`, `agent-brain/tests`, `agent-brain/assets`, `scripts/`
**Files scanned:** about 20 read or grepped (main, app, lib, config, routing, stats, runner, db, term, projects, fastpath, tasks, error, Cargo.toml, test common, runner_kill, fastpath tests, build-release.sh)
**Pattern extraction date:** 2026-10-17
