# Phase 6: Multi-engine Teams - Pattern Map

**Mapped:** 2026-10-26
**Files analyzed:** 36 new/modified (media excluded, moved to 06.1)
**Analogs found:** 34 / 36
**Tracked-source gate:** every analog path below was checked to be in `git ls-files` (all under `agent-brain/`, `tests/`, `scripts/`, `spikes/`). No `.gsd/capabilities` mirror paths are used. Paths are relative to repo root; `agent-brain/src/...` is abbreviated `src/...` only inside excerpts headings.

Decisions that change how analogs are copied: D-24 (human engine Dev-only), D-27 (team name cap 40 = `workflows::validate_name`, not UI-SPEC 48), D-28 (Telegram needs a second confirm tap), D-29 (team change refused while human step waits).

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match |
|---|---|---|---|---|
| `agent-brain/src/engine.rs` (new) | service (EnginePlan, env, readiness) | request-response | `agent-brain/src/profile.rs` + `roles/mod.rs::build_command_with_prompt` | role-match |
| `agent-brain/src/teams.rs` (new) | model/loader | file-I/O | `agent-brain/src/workflows.rs` | exact |
| `agent-brain/src/staff.rs` (new) | model/loader | file-I/O | `agent-brain/src/workflows.rs` | exact |
| `agent-brain/assets/teams.default.json` (new) | config | file-I/O | `agent-brain/assets/workflows.default.json` | exact |
| `agent-brain/src/settings_store.rs` (new) | utility (atomic write, version token) | file-I/O | `agent-brain/src/projects.rs::save_map` (l.335-395) | exact |
| `agent-brain/src/secrets.rs` (new) | utility (redactor, scan) | transform | `agent-brain/src/report.rs::sensitive_path` + `profile.rs` env filter | partial |
| `agent-brain/src/quota.rs` (new) | utility (pure detector) | transform | `agent-brain/src/stream.rs` (`Summary`, `classify`) + `tests/fixtures/stream/*.jsonl` | role-match |
| `agent-brain/src/human.rs` (new) | service | event-driven/git | `agent-brain/src/gitops.rs` + `roles/dev.rs` post-run block | partial |
| `agent-brain/src/error.rs` (mod: `EXIT_QUOTA=5`, `EXIT_WAITING=6`) | config | n/a | same file l.1-6 | exact |
| `agent-brain/src/db.rs` (mod: v4 migration, `NewRun.engine`, team helpers) | model/migration | CRUD | same file (`MIGRATIONS`, `set_task_workflow_if_null`, `insert_run`) | exact |
| `agent-brain/src/runner.rs` (mod: redact, stdout_file) | service | streaming | same file l.215-240, 315-335 | exact |
| `agent-brain/src/roles/mod.rs` (mod: `execute_role_staged` engine seam, header) | service | request-response | same file l.131-178, 400-560 | exact |
| `agent-brain/src/roles/{lead,dev,qa,review}.rs` (mod: resolve team, continuation) | controller (cmd_*) | request-response | `roles/dev.rs` l.84-104, 428-456 | exact |
| `agent-brain/src/stream.rs` (mod: agy branch, quota fields) | utility | streaming/transform | same file l.140-205 | exact |
| `agent-brain/src/pricing.rs` (mod: engine-first lookup) | utility | transform | same file l.1-40 | exact |
| `agent-brain/src/stats.rs` + `dashboard/api_stats.rs` (mod) | service | CRUD (read) | `stats.rs` by_role query l.150-190 | exact |
| `agent-brain/src/projects.rs` (mod: `team_of`) | utility | transform | `projects.rs::workflow_of` l.93-105 | exact |
| `agent-brain/src/autopilot/decide.rs`, `view.rs`, `mod.rs` (mod: `HetQuota`, waiting_human) | service | event-driven | same files (`StopReason`, `reason_word`) | exact |
| `agent-brain/src/dashboard/api_settings.rs` (new) | controller | CRUD | `dashboard/api_workflows.rs` (read) + `projects.rs::save_map` (write) | role-match |
| `agent-brain/src/dashboard/api_workflows.rs` (mod: payload editable shape + version) | controller | request-response | same file | exact |
| `agent-brain/src/dashboard/api_tasks.rs` (mod: `NewTask.team`, `change_team`, `quota_rerun`, `human_done`, `human_return`) | controller | request-response | same file `run_task_action` l.1489+, `ActionSource` l.1764 | exact |
| `agent-brain/src/dashboard/taskstate.rs`, `pipeline.rs` (mod) | service | transform | same files | exact |
| `agent-brain/src/dashboard/assets.rs`, `mod.rs` (mod: routes + ASSETS) | config | request-response | same files l.11-76 / l.71-72 | exact |
| `agent-brain/src/telegram/{callback,detect,render}.rs` (mod) | controller | event-driven | same files (`Op`, `EventKind`, `buttons_for`) | exact |
| `agent-brain/src/main.rs` (mod: `--team`) | config (CLI) | request-response | `--workflow` / `--origin` flag handling | exact |
| `agent-brain/assets/dashboard/screens/settings.html` (new) | component | CRUD | `screens/projects.html` | role-match |
| `agent-brain/assets/dashboard/js/settings.js` (new) | component (Alpine) | CRUD | `js/projects.js` | role-match |
| `agent-brain/assets/dashboard/{index.html,js/app.js}` (mod: nav, router, status maps, `AB.fmtWait` etc.) | component | request-response | same files (`router`, `screenTitles`) | exact |
| `agent-brain/assets/dashboard/{screens/assign,task,stats}.html`, `js/{assign,task,stats}.js` (mod) | component | request-response | themselves | exact |
| `agent-brain/assets/dashboard/app.css` (rebuilt) | config | build | `scripts/build-css.sh` | exact |
| `tests/fakes/fake_agy.py` (new) | test fake | streaming | `tests/fakes/fake_claude.py` | role-match |
| `tests/fakes/fake_claude.py` (mod: record env names) | test fake | streaming | same file | exact |
| `agent-brain/tests/{engine_routing,engine_quota,engine_secrets,engine_agy,human_engine,task_team}.rs` (new) | test | request-response | `tests/task_origin.rs`, `tests/cli_dev.rs`, `tests/telegram_actions.rs` | exact |
| `agent-brain/tests/dashboard_settings.rs` (new) | test | request-response | `tests/dashboard_workflows.rs` | exact |
| `agent-brain/tests/{dashboard_assets,db_migration,dashboard_stats}.rs` (mod) | test | n/a | themselves | exact |
| `agent-brain/tests/fixtures/stream/*.jsonl` (new quota/agy fixtures) | fixture | n/a | `tests/fixtures/stream/dev_blocked.jsonl` (see Pitfall 15: Rust-only, not `tests/contract/stream.json`) | exact |

## Pattern Assignments

### `agent-brain/src/teams.rs` and `staff.rs` (loader, file-I/O)

**Analog:** `agent-brain/src/workflows.rs`

**Constants / built-ins** (l.1-17):
```rust
pub const BUILTIN_WORKFLOWS_JSON: &str = include_str!("../assets/workflows.default.json");
pub const DEFAULT_WORKFLOW: &str = "mac-dinh";
pub const MAX_FILE_BYTES: u64 = 262_144;
```
Copy as `BUILTIN_TEAMS_JSON`, `DEFAULT_TEAM = "mac-dinh"`, same cap.

**Slug rule** (l.138-143): reuse `crate::workflows::validate_name` directly (40-byte cap, D-27). Do not write a second validator.

**Loader shape** (l.781-836): `load_from(text: Option<&str>, cfg) -> Loaded{ items, errors }`; builtins first; size cap message; serde error `Dòng {line}, cột {col}: ... không phải JSON hợp lệ.`; non-object message `Dòng 1: ... phải là một object`; `key_lines(file_text)` for per-entry line numbers; source tag `"override"` when the name matches a builtin else `"file"`; invalid entries push to `errors` while valid ones still load. Extend per-entry validation with: QA must be `claude` (D-06); `người:<id>` only on role `dev` (D-24); unknown staff id is a warning not an error; missing role defaults to `claude`.
Also copy `load_path` (exists/metadata/size check) and `pub fn load(cfg)` wrapper used by `api_workflows.rs` (`crate::workflows::load(&st.cfg)`).
**Test analog:** unit tests at bottom of `workflows.rs` + `tests/cli_workflow.rs`.

### `agent-brain/src/settings_store.rs` (atomic write)

**Analog:** `agent-brain/src/projects.rs::save_map` (l.335-395). Copy verbatim: parent dir create with `0o700`, tmp name `{stem}_{pid}_{nanos}.tmp`, `OpenOptions::new().write(true).create_new(true)` + `.mode(0o600)`, `write_all`, `flush`, `sync_all`, `rename`, `remove_file(&tmp)` on every failure, Vietnamese `BrainError::error(format!("Không thể ghi ..."))`.
```rust
let mut opts = std::fs::OpenOptions::new();
opts.write(true).create_new(true);
use std::os::unix::fs::OpenOptionsExt; opts.mode(0o600);
let mut f = opts.open(&tmp_path)?;
f.write_all(json_str.as_bytes())?; f.write_all(b"\n")?; f.flush()?; f.sync_all()?;
...
if let Err(e) = std::fs::rename(&tmp_path, path) { let _ = std::fs::remove_file(&tmp_path); ... }
```
Delta: add a directory `fsync` after `rename` (D-10), generalize the tmp prefix, and add the version token (hash of raw bytes or `"absent"`) compared under a process-wide `Mutex` just before rename. Operate on `serde_json::Value` (preserve_order) so untouched entries are not rewritten.

### `agent-brain/src/db.rs` (schema v4, NewRun.engine, team CAS)

**Migration vector tail** (l.58-61):
```rust
    "ALTER TABLE tasks ADD COLUMN origin TEXT;",
];
pub const SCHEMA_VERSION: i64 = 3;
```
Append `"ALTER TABLE tasks ADD COLUMN team_json TEXT;",` and set `SCHEMA_VERSION = 4`. Test: copy the shape of `tests/db_migration.rs`. Gate: first task is a `checkpoint:decision` copying `.planning/phases/05.4-telegram-task-intake/05.4-02-PLAN.md` Task 1.

**Helper style** (l.641-647), copy for `set_task_team_if_null` and a CAS `set_task_team_cas(conn, id, old, new)` (`... WHERE id = ?3 AND team_json IS ?4`, also bump `updated_at`):
```rust
pub fn set_task_workflow_if_null(conn: &Connection, task_id: i64, json: &str) -> BrainResult<bool> {
    let changes = conn.execute(
        "UPDATE tasks SET workflow_json = ?1 WHERE id = ?2 AND workflow_json IS NULL",
        params![json, task_id],
    )?;
    Ok(changes == 1)
}
```
**insert_run** (l.311-340): add `pub engine: String` to `NewRun` and replace the literal `'claude'` in the INSERT with `?` (shift params). Callers to update: `roles/mod.rs` Observer `on_spawn` and db tests. Add `insert_human_run` next to it (status `succeeded`, `usage_source='none'`, pid NULLs). `runs.status` CHECK forbids a new status (l.34), hence marker-not-run for the wait.

### `agent-brain/src/roles/mod.rs` (the engine seam)

**Analog:** itself. Three excerpts to modify.

1. Argv builder to keep as the claude-family builder (l.131-178): `cfg.claude_bin -p <input> --output-format stream-json --verbose --model <m> --append-system-prompt <prompt>` then `common_args`, `--max-turns`, `extra_args`, `--allowedTools`, `--disallowedTools`. Add `build_agy_command` beside it; `check_forbidden_args` (l.94-107) must keep refusing `--dangerously-skip-permissions` for claude-family; the agy Dev builder adds its permission flag itself (D-26: probe `--mode accept-edits`/`--sandbox` first), with a unit test that non-Dev agy argv has none.
2. Env + header (l.~425-450):
```rust
let env = crate::profile::build_env(&project.profile, &ctx.cfg.profiles, &crate::profile::current_env())?;
...
crate::term::emit(&format!("[agent-brain] {} · {} · profile {} ({}) · model {}",
    plan.role, project.id, project.profile, p_desc, plan.model));
```
Replace with `engine::env_for(engine, env)` applied AFTER `build_env` (Pitfall 5) and add ` · engine {engine}` to the header (D-14).
3. `RunRequest { argv, cwd, env, stderr_path, stream_path, timeout, kill_grace }` and `Observer::on_spawn` building `NewRun` (add `engine`), `on_line` doing `summary.feed` then `insert_event` then `term::emit` (redaction must precede this, see runner). After `finish_run`, call `quota::detect` and return `Err(BrainError::new(msg, EXIT_QUOTA))`; every `cmd_*` already propagates with `?`.

### `agent-brain/src/runner.rs` (redaction, agy stdout-to-file)

**Env isolation** (l.226-233):
```rust
cmd.env_clear(); cmd.envs(&req.env);
cmd.stdin(std::process::Stdio::null());
cmd.stdout(std::process::Stdio::piped());
cmd.stderr(stderr_file);
cmd.process_group(0); cmd.kill_on_drop(true);
```
**Line sink** (l.322-329): `stream_file.write_all(&line); stream_file.flush(); obs.on_line(&line);`. Add `RunRequest.redact: Vec<String>` and `RunRequest.stdout_file: Option<PathBuf>`; apply redaction to `line` before BOTH calls; for agy spawn with the file as stdout and tail it (poll 100-200 ms, last line without newline, 2 s drain). Kill path (`procs::kill_tree`, `cmd_kill`) untouched (S4). Test analog: `tests/runner_kill.rs`.

### `agent-brain/src/stream.rs` (agy branch + quota fields)

**classify** (l.146-163) currently maps only `type`. Add before the `else`: a line with an `event` key and no `type` becomes `agy/{event}`. `Summary` (l.179-205, private `result_seen`) gains optional tolerant fields `rate_limit_rejected`, `using_overage`, `api_error_status`, `terminal_reason`, `last_retry_error` and an agy format flag reading `result.status/response/usage` (usage mapping per RESEARCH Engine Facts). Also teach `term::event_text` and `dashboard/sse.rs::row_kind` the `agy/*` kinds. Fixtures: `tests/fixtures/stream/dev_blocked.jsonl` style; real agy lines in `spikes/s7/raw/s7-stream.jsonl`; real `allowed_warning` line in `spikes/s1/raw/s1.jsonl` line 5.

### `agent-brain/src/quota.rs` (pure detector)
No exact analog (no detector exists). Use the RESEARCH sketch (`detect_claude`, agy variant) and test it like `tests/contract_stream.rs` does for stream parsing but Rust-only fixtures. Negative fixture is mandatory: `system/api_retry` rate_limit then success must NOT stop.

### `agent-brain/src/roles/dev.rs` (continuation + agy diff check)

**Dirty-tree refusal to relax in continuation mode** (l.86-99): `changed_paths(&proj_path)?` non-empty returns `Working tree có thay đổi chưa commit ...`. When the latest Dev run outcome is `quota`, allow dirty paths inside plan files + protocol files (Pitfall 1).
**Post-run scope check to reuse for agy and human `Xong`** (l.428-456):
```rust
let uncommitted = crate::gitops::changed_paths(&proj_path)?;
all_changed.extend(uncommitted);
let violations: Vec<String> = all_changed_nfc.difference(&allowed_nfc).cloned().collect();
if !violations.is_empty() {
    crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
    return Ok(crate::error::EXIT_PROTOCOL);
}
```
Add an empty-diff check for agy Dev (`agy:empty`) and `report::sensitive_path` for secret-looking paths. Return handling for the human wait: `EXIT_WAITING`.

### `agent-brain/src/gitops.rs` (human `Xong`)
Use the existing `git(root, args)` (l.18, sets `GIT_TERMINAL_PROMPT=0`), `current_branch`, `changed_paths`, `branch_exists`, `commit_paths`. Add `push_branch` (never `--force`, D-25, best effort) and `fetch_branch` + `merge_ff_only` (never `pull`/`reset`, Pitfall 18).

### `agent-brain/src/autopilot/decide.rs` + `view.rs` (HetQuota)

**Enum** (decide.rs l.112-121) and `as_str`:
```rust
#[serde(rename_all = "snake_case")]
pub enum StopReason { HetVong, HetNganSach, ViPhamGiaoThuc, VuongMac, DungBoiBan, RunLoi }
```
Add `HetQuota` => `"het_quota"`; `view.rs::reason_word` (l.7-14) add `StopReason::HetQuota => "hết quota"`. Insert the D2 rule testing `outcome == "quota"` BEFORE the `failed` branch at decide.rs l.826 (`if last_run.status == "failed" || ... "timed_out" || ... "lost"`; the `Decision::Stop(StopInfo{...})` literal there is the template). `build_facts` round counting: drop superseded quota runs (Pitfall 2). Child exit handling `child_error_stop` in `autopilot/mod.rs` (~l.340) ignores `[EXIT_OK, EXIT_PROTOCOL, EXIT_BLOCKED, EXIT_INTERRUPTED]`; add `EXIT_WAITING`.

### `agent-brain/src/dashboard/api_settings.rs` (new routes)

**Route/handler shape analog:** `dashboard/api_workflows.rs` l.1-40:
```rust
pub fn routes() -> Router<Arc<AppState>> { Router::new().route("/api/workflows", get(get_workflows)) }
async fn get_workflows(State(st): State<Arc<AppState>>) -> Response {
    let res = blocking(move || { ... Ok((StatusCode::OK, [(CONTENT_TYPE, "application/json; charset=utf-8")], axum::Json(val)).into_response()) }).await;
    match res { Ok(resp) => resp, Err(e) => { crate::term::emit(&format!("Lỗi nội bộ ...: {e}")); json_error(StatusCode::INTERNAL_SERVER_ERROR, "...") } }
}
```
Imports: `crate::dashboard::{blocking, json_error, AppState}`. Register: `dashboard/mod.rs` l.71-72 `.merge(api_workflows::routes())` add `.merge(api_settings::routes())`; the auth gate layer and `DefaultBodyLimit::max(65536)` are inherited. Writes use POST (not DELETE). Write core = `settings_store` above; refuse with 409 when the loaded file has `errors`. Make `workflows::validate_entry` `pub(crate)` and add `Workflow::to_file_entry()`.
**Test analog:** `tests/dashboard_workflows.rs` (`Sandbox::new`, `sb.serve(&[])`, `srv.get`, `r.json()`); POST helpers in `tests/dashboard_actions.rs`.

### `agent-brain/src/dashboard/api_tasks.rs` (team + four new actions)

**Shared action seam** (`run_task_action` l.1489-1530): load state by `load_task_state`, stale check against `state.rev`, find the action in `state.view.actions`, refuse with `ControlOutcome::Refused { reason, kind }` when disabled. Add `change_team`, `quota_rerun`, `human_done`, `human_return` in this family so dashboard and Telegram share them. Source enum (l.1764): `ActionSource { Dashboard, Telegram }`; Telegram re-run is not plan approval (`TELEGRAM_APPROVE_REFUSED`, l.1770 stays). `task_rev` (l.955-967) must hash `team_json`. `NewTask` has `#[serde(deny_unknown_fields)]` (l.26): add `team: Option<String>` and emit `--team` in `new_task_args` only when set (Pitfall 16; existing argv assertions must hold).

### `agent-brain/src/telegram/{callback,detect,render}.rs`

**Grammar** (callback.rs l.1-30): `{op}:{task}:{rev}`, 1-char op, `MAX_LEN = 64`, ops listed in `enum Op` + `ALL_OPS` (update the array length). Add team pick `t:{task}:{idx}:{ver4}:{rev}` and confirm `u:{...}` (D-28 second tap; Kill ask/confirm/cancel is the precedent: `KillAsk/KillConfirm/KillCancel`).
**Events** (detect.rs l.37-42): `enum EventKind { Gate, Stop, QaPass, RunFailed }` add `Quota` (key `quota:{run_id}`) and exclude `outcome=quota` from `RunFailed`.
**Buttons** (render.rs l.307-344): `buttons_for` matches on `ev.kind`, builds `button("label", Op::X, s, None)` rows gated by `has_control/has_action`. Add a `Quota` arm. Tests: `tests/telegram_actions.rs`, `tests/telegram_notify.rs`, helpers `tests/support/telegram.rs`, fake `tests/fakes/fake_telegram.py`.

### `agent-brain/src/stats.rs` / `api_stats.rs`

**Analog:** `by_role` query (stats.rs l.150-190): `SELECT runs.role, COUNT(*), COALESCE(SUM(...),0)... FROM runs WHERE (?1 IS NULL OR runs.profile = ?1) AND (?2 IS NULL OR runs.started_at >= ?2) GROUP BY runs.role ORDER BY runs.role` mapped to `RoleStats { role, totals: Totals{...} }`. Copy as `by_engine` with `GROUP BY runs.engine`; add the QA-by-Dev-engine SQL from RESEARCH Pattern 9. Test analogs: `tests/db_stats.rs`, `tests/dashboard_stats.rs`.
**pricing.rs** l.8-39: `price_for` substring match falls back to the most expensive entry (Pitfall 6): add engine-first exact lookup returning `Option`.

### `agent-brain/src/projects.rs::team_of`
**Analog:** `workflow_of` (l.93-105): reads `p.get("workflow").and_then(as_str)`, trims, returns `None` when blank. Copy with key `"team"`. No struct change.

### `agent-brain/src/dashboard/assets.rs` (+ tests)
**Pattern** (l.11-76): `Asset { path: "/js/stats.js", mime: "text/javascript; charset=utf-8", body: include_bytes!("../../assets/dashboard/js/stats.js") }`. Add `/js/settings.js` and `/screens/settings.html`. Tests: `tests/dashboard_assets.rs` (no external loads check), `tests/assets_parity.rs`.

### `assets/dashboard/js/settings.js` and `screens/settings.html`
**Analog:** `js/projects.js` (l.1-40) and `screens/projects.html`.
```js
(function () {
  window.AB.onReady('projects', function () {
    return { ...flat state..., init: function () { ...window.AB.onLeave(function () {...}); } };
  });
})();
```
```html
<section data-screen="projects" x-data="projects" :data-loaded="loadedAttr" class="space-y-6">
```
Use `data-screen="settings"`, `x-data="settings"`, `:data-loaded`. Card/Primary/Secondary/callout class strings: copy from `projects.html` l.1-25 (Card `rounded-2xl bg-white p-6 shadow-sm ring-1 ring-slate-200 dark:bg-slate-800 dark:ring-slate-700`, Secondary button string with `focus-visible:outline-2 ...` FOCUS suffix, amber callout `rounded-xl bg-amber-50 p-4 ring-1 ring-amber-300 ...`). Alpine CSP rules: flat fields, no arrows/ternaries/templates. Router registration in `js/app.js` l.395-445: add `settings: 'Cài đặt'` to `screenTitles`, an `if (h === '#/settings')` branch in `parseHash`, and `loadScript('settings')` (file must be self-contained). Serialize edits to `app.js`/`index.html` across plans (Pitfall 17). UI gates: `scripts/dashboard-ui-lint.sh` (scans every `screens/*.html`), `scripts/build-css.sh --check`, `scripts/dashboard-js-check.mjs`, `scripts/dashboard-smoke.sh`.

### `tests/fakes/fake_agy.py` (new) and `fake_claude.py` (mod)
**Analog:** `tests/fakes/fake_claude.py` l.1-80: `FAKE_CLAUDE_RECORD` appends `{argv, cwd, ...}` JSON per call; `FAKE_CLAUDE_SCENARIO` JSON with `events`, `raw_lines`, `exit_code`, `stderr`, `by_role` (role detected from first line of `--append-system-prompt`). Extend the record with env var NAMES and a boolean sentinel check, never values (Pitfall 5). `fake_agy.py` emits `{"event":"init"...}`, `step_update`, `result` lines slowly to stdout, final line without newline, scenario-driven exit code. Test harness: `tests/common/mod.rs` `Sandbox` (`make_repo`, `write_map`, `serve`) and `tests/support/protocol.rs`. Test style analog `tests/task_origin.rs` (scenario helpers like `plan_review_entry`).

## Shared Patterns

### Vietnamese errors with line/column
**Source:** `workflows.rs` l.806-820. Apply to teams/staff loaders and every Settings validation message.

### One shared action function for dashboard, Telegram, CLI
**Source:** `api_tasks.rs::run_task_action` l.1489+. Apply to `change_team`, `quota_rerun`, `human_done`, `human_return`; never add a Telegram-only path.

### Atomic file writes at 0600 with tmp cleanup
**Source:** `projects.rs::save_map` l.335-395. Apply to `teams.json`, `staff.json`, `workflows.json` writes.

### Secrets never in logs
**Source:** env building `profile.rs::build_env` (+ `unset_env` in `assets/config.default.json`) and runner `env_clear`. Apply: key value only read at spawn; redact in runner before file/event/term/job sinks; `Config` holds only the variable name; `secrets::scan_paths` before `commit_paths` for secret-bearing engines; scrub fixtures with `spikes/lib/scan-secrets.sh`.

### Kill and process safety
**Source:** `runner.rs` `process_group(0)`, `kill_on_drop`, `procs::kill_tree`, `cmd_kill`. Apply to deepseek (it is `claude`) and agy unchanged.

### Test sandbox
**Source:** `tests/common/mod.rs` `Sandbox`, fake claude/agy, temp brain.db. Real quota/keys are never used.

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `agent-brain/src/quota.rs` | utility | transform | No quota detection exists; no real quota stream captured (only `allowed_warning`); use RESEARCH sketch plus probes |
| `agent-brain/src/human.rs` (marker + `Xong` fetch) | service | event-driven | No human-wait state or remote fetch exists; compose from `gitops.rs` helpers and the `cmd_dev` post-run block |
| `agent-brain/src/secrets.rs` (redactor) | utility | transform | Only path-based `report::sensitive_path` exists; no value redaction |
| E-09 / agy probe plans (user-run) | probe | n/a | Closest format: `spikes/s7/run.sh`, `spikes/s7/verify.sh`, `spikes/s7/NOTES.md` |

## Metadata

**Analog search scope:** `agent-brain/src`, `agent-brain/tests`, `agent-brain/assets`, `tests/fakes`, `scripts`, `spikes/s7`
**Files scanned:** about 25 read in full or by section, tracked-file list reviewed (about 250 paths)
**Pattern extraction date:** 2026-10-26
