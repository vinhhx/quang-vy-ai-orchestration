# Phase 6: Multi-engine & Media Branch - Pattern Map

Scoped re-map for the D-30 … D-44 work of 2026-10-28 (replacement 06-11, new 06-20 … 06-24, revised 06-19). Analogs are current code written by executed Phase 6 plans. The pre-code version is in git history.

**Mapped:** 2026-10-28
**Files analyzed:** 41 (new or modified)
**Analogs found:** 40 / 41 (all analog paths verified git-tracked; paths are relative to repo root, `agent-brain/` prefix kept)

Line numbers are for the current tree and drift as 06-11 lands; re-grep before relying on them after an earlier plan in the chain.

## File Classification

| New/Modified File | Plan | Role | Data Flow | Closest Analog | Match |
|---|---|---|---|---|---|
| `agent-brain/src/config.rs` (engine list parse) | 06-11 | config | transform | same file `parse_engines` :129-251 + tests :913-1012 | exact |
| `agent-brain/assets/config.default.json` | 06-11 | config | static | same file `engines` block | exact |
| `agent-brain/src/engine.rs` (`Engine::Named`, `EngineDef`, `readiness`, `plan_for`) | 06-11 | service | request-response | same file, agy arm `agy_plan` :377-436 | exact |
| `agent-brain/src/teams.rs` (parse, summary, QA rule, `team_profile_refusal`) | 06-11 / 06-22 | model/validator | transform | same file `validate_entry` :195-233 | exact |
| `agent-brain/src/pricing.rs` | 06-11 | utility | transform | same file `engine_price`/`is_priced` :38-71 | exact |
| `agent-brain/src/roles/mod.rs` (seam: `String` engine, refusal backstop) | 06-11 | service | request-response | same file :428-445, :486-510, :592-672 | exact |
| `agent-brain/src/roles/dev.rs` (`kind_of` gate, `dsh:empty`) | 06-11 / 06-23 | service | batch | same file :575-612 (`is_agy`) | exact |
| `agent-brain/src/profile.rs` (`pin_profile` env) | 06-11 | utility | transform | same file `build_env` :67-100 | exact |
| `agent-brain/src/dashboard/api_settings.rs` (`get_engines`, save refusal) | 06-11 / 06-22 | route | request-response | same file :811-826, :355-375 | exact |
| `agent-brain/src/dsh.rs` (stub in 06-11, full in 06-23) | 06-11 / 06-23 | service | streaming (text) | `engine.rs` `agy_plan` + `quota.rs` `detect_agy` | role-match |
| `agent-brain/src/lib.rs` | 06-11 | config | static | same file (module list) | exact |
| `docs/config.engines.example.json` + loader test | 06-11 | config/test | static | same file (already array-shaped; update to D-41) | exact |
| `tests/fakes/fake_claude.py` (`echo_env`) | 06-11 / 06-21 | test fake | event-driven | same file :40-66 (record block) | exact |
| `agent-brain/tests/engine_routing.rs`, `dashboard_settings.rs` | 06-11 / 06-22 | test | request-response | same files (`engines_readiness_no_secret` :998) | exact |
| `spikes/e09/{run.sh,verify.sh,NOTES.md}` | 06-20 | script | batch | same files | exact |
| `spikes/s6/run.sh` (`--kill-wrapper`) | 06-20 | script | batch | same file `--kill` variants | exact |
| `agent-brain/src/runner.rs` (`redact`; later stop hook) | 06-21 / 06-23 | service | streaming | same file `RunRequest` :162-172, `cmd_kill` :55-160 | exact |
| `agent-brain/tests/engine_secrets.rs` | 06-21 | test | request-response | `tests/engine_agy.rs` `agy_env_and_argv_hygiene` :266 | role-match |
| `agent-brain/src/roles/lead.rs`, `autopilot/mod.rs`, `dashboard/api_tasks.rs`, `telegram/intake.rs` (bind points) | 06-22 | controller | request-response | `api_tasks.rs` `start_new_task` :670 + `change_team` :1403 | exact |
| `agent-brain/tests/{task_team,telegram_quota,cli_autopilot}.rs` | 06-22 | test | request-response | `tests/engine_routing.rs` | role-match |
| `agent-brain/assets/dashboard/js/{settings,assign}.js`, `screens/*.html`, `scripts/dashboard-js-check.mjs`, `scripts/dashboard-smoke.sh` | 06-24 | component | request-response | `settings.js` `engineRows` :1540-1575 | exact |
| `.planning/phases/06-.../06-UI-SPEC.md` (delta, D-44) | 06-24 | doc | static | same file | exact |
| `agent-brain/src/stream.rs`, `term.rs`, `dashboard/sse.rs` (text stream) | 06-23 | service | streaming | same files, agy branch | role-match |
| `agent-brain/src/quota.rs` (`detect_text`) | 06-23 | utility | transform | same file `detect_agy` :60-110 | exact |
| `agent-brain/src/autopilot/{decide,view}.rs`, `dashboard/taskstate.rs` (`dsh:empty`) | 06-23 | service | transform | same files (`agy:empty`) | exact |
| `tests/fakes/fake_docker.py`, `tests/fakes/fake_dsh.py` | 06-23 | test fake | event-driven | `tests/fakes/fake_agy.py` | exact |
| `agent-brain/tests/engine_dsh.rs` | 06-23 | test | request-response | `tests/engine_agy.rs` | exact |
| `scripts/phase6-live-check.sh` | 06-19 | script | batch | `scripts/board-live-check.sh` | role-match |
| `agent-brain/README.md` (§5, §11, §14) | 06-19 | doc | static | same file | exact |
| `agent-brain/LIVE-CHECKS.md` (§ 8, P6-01 … P6-10) | 06-19 | doc | static | same file § 7 Phase 5.5 | exact |

## Pattern Assignments

### `agent-brain/src/engine.rs` (service, request-response) — 06-11 core

**Analog:** itself, the agy arm. Replace the closed enum with an id-bearing one and dispatch on kind.

**Current closed set to replace** (lines 11-26, 28-56):
```rust
pub const ENGINE_ORDER: [&str; 4] = ["claude", "deepseek", "agy", "người"];
pub enum Engine { Claude, Deepseek, Agy, Human(String) }
// parse: trim + NFC, then match literal names; "người:" prefix via crate::staff::validate_id
pub fn as_db_str(&self) -> &'static str   // must become String/&str (S1, S2)
```
Keep the `trim().nfc().collect()` and `HUMAN_PREFIX` + `staff::validate_id` shape in `parse`; add `Named(String)` for a valid slug.

**Dispatch seam to extend** (`plan_for`, 309-340): `match &resolved.engine { Engine::Claude => Ok(EnginePlan{...}), Engine::Agy => agy_plan(cfg, role, claude_argv, base_env, timeout), Engine::Deepseek => Err(...), Engine::Human(_) => Err(...) }`. New: dispatch on `cfg.engines.kind_of(id)` to `anthropic_plan` / `agy_plan` (parameterised by the entry) / `dsh::plan` (stub returning `"Engine dsh chưa được bật"`; 06-11 leaves it so 06-21/06-23 never edit `plan_for`). Every error returns before any run row exists.

**EnginePlan** (290-303): change `db_engine: &'static str` to `String`; add `StreamFormat::Text` for dsh (283-287 enum is `Claude, Agy`).

**Env hygiene to generalise (D-32, ENG-05)** (`agy_plan` 415-421):
```rust
env.retain(|k, _| !(k.starts_with("ANTHROPIC_") || k == "CLAUDE_CONFIG_DIR"
    || k == "CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR" || k == "CLAUDE_CODE_SUBAGENT_MODEL"
    || k == &cfg.engines.deepseek.key_env));
```
Becomes `|| cfg.engines.all_key_envs().contains(k)`. The `anthropic` arm strips the OTHER entries' key variables and sets `ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` from the entry's `key_env` value (read from env at spawn, never stored).

**Readiness** (169-215): iterate catalog. Per-entry check pattern to copy: `command_status(name, cmd, path_var, DESC)` then, for key kinds, `env.get(&key_env).map_or(false, |v| !v.trim().is_empty())` else `Readiness::ThieuKey` with `reason: "Chưa đặt biến môi trường {key_env}."`. Never copy an env value. Add `kind`, `label`, `evidence` fields additively; D-40 `chưa kiểm` = no E-09 evidence; dsh container not running reuses `thiếu lệnh` with a reason (D-44). Default catalog order must stay `claude, deepseek, agy, người` (asserted at `tests/dashboard_settings.rs:1018-1028`).

**Append only (06-21):** `scan_for_secrets`, no `plan_for` change.

---

### `agent-brain/src/config.rs` (config, transform) — 06-11

**Analog:** `parse_engines` (129-251), error texts asserted at tests 971-1004.

**Style to keep:** key_env validator (`valid_key_env`, :111-118, uppercase/digit/_), per-field Vietnamese errors `"Cấu hình engines.{id}.{field} không hợp lệ (…)"`, `command` non-empty check, `print_timeout_margin_secs` bound 0..=3600 (:225-236). Legacy object shape errors stay byte-identical (`"Cấu hình engines.khac không hợp lệ (chỉ deepseek, agy)"`). New: accept array; reject `claude` id (D-41, "remove it" text), `command` is an executable string only, `pin_profile`, `allowed_profiles` (`work`/`personal`), dsh discrete fields (container, dsh profile). Struct shape: `EnginesCfg { list: Vec<EngineDef> }` with `get`, `kind_of`, `all_key_envs`; keep a legacy-object test as back-compat proof and keep `engines_defaults` equality (:913-932).

---

### `agent-brain/src/teams.rs` (validator, transform) — 06-11 / 06-22

**Analog:** `validate_entry` role loop (204-233).
```rust
let engine = match raw.as_str().and_then(Engine::parse) { Some(e) => e, None => { errors.push(field_error(role, format!("Engine không hợp lệ cho {role}: {shown} (dùng …)"))); continue; } };
if role == "qa" && engine != Engine::Claude { errors.push(field_error(role, "QA phải dùng engine claude.")); continue; }
if engine.is_human() && role != "dev" { errors.push(field_error(role, "Engine người chỉ dùng cho vai trò dev.")); continue; }
```
Copy this `field_error` + `continue` shape for the D-37 rule (dsh only for dev: refuse for lead/plan-review/code-review/qa with a Vietnamese message, in the loader and on save) and the D-39 rule (pinned profile not for QA; the QA rule stays exact `Claude`). Validation is syntax-only at load (unknown id = warning, not error; same as the `warnings.push("Nhân sự … không còn…")` branch at :227-231). Update the "Engine không hợp lệ" text (no longer a fixed list). `Team::summary` sort (54-87) and `engine_text` (131-140) become catalog-ordered.

**`team_profile_refusal` (06-22)**: one pure function, modelled on `resolve` (377-418) returning `BrainResult`/`Option<String>`; takes team, catalog, project profile. D-42: at save time refuse only when already bound (project default `team` in `projects_map.json` or an open task); otherwise at use.

---

### Bind points for the guard (controller, request-response) — 06-22

**Analog (closest to the pattern, copy shape):** `dashboard/api_tasks.rs:667-678`
```rust
let (teams_loaded, _staff) = crate::teams::load();
let team = match crate::teams::resolve(&teams_loaded, req.team, crate::projects::team_of(&map, &project.id).as_deref()) {
    Ok(r) => r,
    Err(e) => return Ok(StartOutcome::Invalid(e.message)),
};
```
Add the refusal right after `resolve`, returning `StartOutcome::Invalid(msg)` (the live check expects HTTP 422 on `POST /api/tasks`). Same site list: `roles/lead.rs:101-110` (`resolve(...)?` then emit note; return `Err(BrainError::error(msg))`), `autopilot/mod.rs:620` (`resolve(&teams_loaded, Some(requested), None)?`), `api_tasks.rs:1403 change_team` (add after the `state.teams.get(team).is_none()` check as `TeamOutcome::Invalid`/`Refused { reason, kind }`), `api_tasks.rs:1715 quota_rerun`, `api_settings.rs:355 save_team_blocking` (use `refusal(TEAMS_HEADING, vec![field_err(...)])` as at :370-372), Telegram `/task` goes through `start_new_task` (intake.rs 597, 1163, 1172). Run-time backstop at the `roles/mod.rs` seam after `resolve_for_run`, before `plan_for`.

---

### `agent-brain/src/profile.rs` (utility, transform) — 06-11 Task 3

**Analog:** `build_env` (67-100): clones base env, strips `NESTED_SESSION_VARS`, applies profile `unset_env`, sets or removes `CLAUDE_CONFIG_DIR` with `~` expansion. `pin_profile` reuses the SAME builder by passing the pinned profile name instead of `project.profile` (do not duplicate the `~` logic). Call site: `roles/mod.rs:431-435`. Refuse (not strip) when the project profile is not allowed (D-36/D-38).

---

### `agent-brain/src/roles/mod.rs` (service, request-response) — 06-11, 06-21, 06-23 (serial)

**Seam** (427-445): `resolve_for_run` -> `build_env` -> `plan_for` -> run dir -> header `"[agent-brain] {} · {} · profile {} ({}) · engine {} · model {}"` (add the D-40 unverified mark here and `metrics_json.unverified`). Observer field `db_engine: &'static str` (486) -> `String`; passed to `insert_run_with_engine(&self.ctx.conn, &new_run, self.db_engine)` (510; already `&str`).

**Post-run branches to extend** (606-672): `agy_stream` flag, `agy_print_timeout_hit` -> `TimedOut`, non-SUCCESS -> `Failed`, then `if ep.stream == StreamFormat::Claude && !Killed/TimedOut { detect_claude } else if agy_stream && ... { detect_agy } else { None }`. `anthropic` reuses the Claude branch unchanged; dsh adds a `StreamFormat::Text` branch calling `quota::detect_text`. Cost fallback `if ep.db_engine == ENGINE_CLAUDE {cost} else {0.0}` (667) -> "native" test (claude or anthropic without `base_url`).

---

### `agent-brain/src/roles/dev.rs` (service, batch) — 06-11 gate, 06-23 `dsh:empty`

**Analog:** 575-612.
```rust
let is_agy = crate::db::get_run(&ctx.conn, role_run.run_id)?
    .map(|r| r.engine == crate::engine::ENGINE_AGY).unwrap_or(false);
// ... .planning/ and sensitive_path violations when is_agy ...
if is_agy && all_changed_nfc.is_empty() {
    crate::term::emit_out(AGY_EMPTY_MESSAGE);
    crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "agy:empty")?;
    return Ok(crate::error::EXIT_BLOCKED);
}
```
Change the predicate to `matches!(ctx.cfg.engines.kind_of(&r.engine), Some(Agy | Dsh))`; outcome string per kind (`agy:empty`, `dsh:empty`). Add `dsh:empty` in `autopilot/decide.rs` (~294, ~935-945), `dashboard/taskstate.rs:110`, `autopilot/view.rs:347`, next to each `agy:empty`.

---

### `agent-brain/src/dsh.rs` (service, streaming text) — stub 06-11, full 06-23

**Analog:** `agy_plan` (engine.rs 377-436) for plan building, `detect_agy` (quota.rs ~60-110) for the detector. Build `argv = ["docker", "exec", "-w", cwd, container, "dsh", "--profile", dsh_profile, prompt]` from discrete config fields; `stdout_file: true` like agy (stdout is the stream file, never a pipe); stdin `/dev/null` is already the runner behaviour. Plain text: no usage (`usage_source none`). `detect_text(stderr_tail/outcome)` copies the `agy_regex` + `first_matching_line` + `QuotaHit { signal, detail, reset_at_ms: None }` shape (quota.rs `one_line`, `first_matching_line`). D-37: Dev only. No analog for the in-container stop hook (see No Analog).

---

### `agent-brain/src/runner.rs` (service, streaming) — 06-21 redact, 06-23 stop hook

**Analog:** `RunRequest` (162-172: `argv, cwd, env, stderr_path, stream_path, timeout, kill_grace, stdout_file`). Add `redact: Vec<String>` (06-21; secrets = values of the run's engine `key_env` variables) applied to anything written to disk, and an optional stop hook (06-23). Kill path analog: `cmd_kill` (55-160): `procs::alive(root)`, `db::mark_kill_requested`, `procs::kill_tree(root, Duration::from_secs(ctx.cfg.kill_grace_secs), &force)`, poll `get_run` for up to 2 s. The container spec is persisted in `runs.metrics_json.dsh` and read back by `cmd_kill` from `RunRow.metrics_json` (db.rs `RunRow.metrics_json`, `UPDATE runs SET metrics_json` helper at db.rs:871). Never kill by process name (CLAUDE.md, S4).

---

### `agent-brain/src/pricing.rs` (utility, transform) — 06-11

**Analog:** `engine_price` 38-47 (`claude`/`người` -> `None`, else `table.get(engine)`), `is_priced` 64-71, `estimate_usd` 77-89 (`r.engine == ENGINE_CLAUDE` family lookup). Native = `claude` or an `anthropic` entry with no `base_url`; price gateway/agy/dsh by id. Callers to keep compiling: `stats.rs:356`, `autopilot/decide.rs:390`, `api_tasks.rs:2140`. A no-usage engine (dsh) needs a hint so the UI does not read "chưa có giá" as "no tokens" (stats.rs `query_by_engine` is generic; only `api_stats.rs:43` calls).

---

### `agent-brain/src/dashboard/api_settings.rs` (route, request-response) — 06-11 payload

**Analog:** `get_engines` (815-826):
```rust
let res = blocking(move || {
    let staff = crate::staff::load();
    let engines = crate::engine::readiness(&st.cfg, &crate::profile::current_env(), staff.staff.len());
    Ok(json!({ "engines": engines }))
}).await;
match res { Ok(v) => json_response(StatusCode::OK, v), Err(e) => internal_read(&e.message) }
```
Add `kind`, `label`, `description`, `evidence` to `EngineStatus`; D-43 warning `engine chưa khai báo` for `deepseek` missing from the list is computed in `teams_json` (154-230), which has `st.cfg`. Never emit an env value (test `engines_readiness_no_secret`, `tests/dashboard_settings.rs:998`).

---

### `agent-brain/assets/dashboard/js/settings.js`, `assign.js` (component, request-response) — 06-24

**Analog:** `engineRows` (settings.js 1540-1575): builds rows from a fixed `ENGINE_ORDER` (:32) + `ENGINE_DESC` (:33-45) and `FIXED_ENGINES` (:45, used in `roleOptions` 901-907), special-casing `name === 'deepseek'` for key/model rows. Rewrite to loop `this.engines` from `GET /api/engines` (server sends `kind`, `label`, `description`), keep the synthetic `người` row, show `Biến key:` (NAME only) for every key-bearing entry. Keep the `READINESS_WORDS` lookup and `chua_kiem` default (:1549) and the unchanged `engineBadgeClass`/`engineGlyph` helpers. `assign.js:5` `ENGINE_ORDER = ['claude','deepseek','agy']` and the readiness lines at :218-240: iterate the API order; `chua_kiem` is a soft hint, not "bước này sẽ dừng với lỗi" (:757-760). `settings.js:618` QA rule `this.tQa !== 'claude'` stays. Gates: `scripts/dashboard-js-check.mjs`, `scripts/dashboard-smoke.sh`, `scripts/dashboard-ui-lint.sh`, `scripts/build-css.sh --check`. First task = UI-SPEC delta + checkpoint (D-44).

---

### `tests/fakes/fake_claude.py` (test fake, event-driven) — 06-11 / 06-21

**Analog:** record block 44-56:
```python
record_path = os.environ.get("FAKE_CLAUDE_RECORD")
if record_path:
    record = {"argv": sys.argv[1:], "cwd": os.getcwd(), "claude_config_dir": os.environ.get("CLAUDE_CONFIG_DIR"),
              "present_vars": present_vars, "settings": settings_val, "blocked": blocked}
    with open(record_path, "a", encoding="utf-8") as f: f.write(json.dumps(record) + "\n")
```
Extend the record additively (e.g. names-only `env_names`, `base_url` presence as in `fake_agy.py`'s `env_names` record). `echo_env` scenario key (06-21) makes the fake print the env values into its output so the redaction/leak scan can be proven.

---

### `tests/fakes/fake_docker.py`, `tests/fakes/fake_dsh.py` (test fake, event-driven) — 06-23

**Analog:** `tests/fakes/fake_agy.py` (docstring 2-26, `parse_argv` 49-80): scenario file via `FAKE_AGY_SCENARIO`, record via `FAKE_AGY_RECORD` (one JSON line per call with argv/cwd/env_names), keys `lines`, `raw_lines`, `delay_ms`, `final_newline`, `exit_code`, `stderr`, `ops` (writes files), `sleep_s`, `spawn_child`, `pidfile` (`"{own pid} {child pid}"`). Use `FAKE_DSH_*` / `FAKE_DOCKER_*` names. `fake_docker.py` accepts `exec -w CWD CONTAINER dsh --profile P PROMPT` and `exec CONTAINER ps`/`kill` for the marker-file kill check; no real Docker in any automated test. Plain-text output, no usage.

---

### `agent-brain/tests/engine_dsh.rs` (test, request-response) — 06-23

**Analog:** `tests/engine_agy.rs` (769 lines).
```rust
mod common;
#[allow(dead_code)] #[path = "support/protocol.rs"] mod protocol;
use common::{RunOut, Sandbox};
fn fake_agy() -> PathBuf { common::repo_root().join("tests").join("fakes").join("fake_agy.py").canonicalize().expect(..) }
fn agy_config(sb: &Sandbox) { sb.write_config(json!({"engines": {"agy": {"command": fake_agy()...}}})); }
struct AgyEnv { scenario, record }  // vars() -> FAKE_AGY_SCENARIO / FAKE_AGY_RECORD; records() parses JSON lines
fn write_doi_agy(sb, roles) { sb.write_teams(json!({"doi-agy": {"description": .., "roles": roles}})); }
fn setup_project(sb, profile) -> PathBuf   // make_repo + project_entry + write_map (lines 114-125)
fn run_row(sb, role) -> RunRow             // SELECT engine,status,outcome,usage_source,...,metrics_json FROM runs (158-185)
```
Tests to mirror: `agy_lead_tracer` (189), `agy_env_and_argv_hygiene` (266), `agy_dev_edits_in_scope` (411), `agy_dev_empty_diff_suspicious` (498 -> `dsh:empty` NOT hit when the fake edits), `agy_quota_stops` (541), `agy_kill_s4` (678-769: `spawn_child` + `pidfile`, `sb.spawn_env(["run", ...])`, `sb.run(&["kill", &task_id])`, assert `status == "killed"`, assert recorded PIDs are gone). dsh config: `engines` array with a `kind: "dsh"` entry whose `docker` points at `fake_docker.py`, container `fake-dsh`, `allowed_profiles: ["personal"]`; assert `usage_source none`, no `.planning/` blocks, refusal on a work project.

---

### `agent-brain/tests/{engine_routing,dashboard_settings,task_team,telegram_quota,cli_autopilot}.rs` (tests) — 06-11 / 06-22

**Analog:** `tests/engine_routing.rs` header (1-17) and helpers (`setup_auto_project`:57, `assert_ok`:74, `team_json`:82, `run_of`:104, `write_doi_claude`:117, test `team_flag_routes_and_records`:126). Add `tiet_kiem_routes_lead_dev_qa` on a custom id (06-11 tracer) and `eng10_*` refusal tests (06-22) in the same style; `dashboard_settings.rs:998-1040` for the readiness payload, additively. Update literal `deepseek` assertions where the enum changes; keep legacy-shape tests (config.rs 971-1004) as back-compat proof.

---

### `docs/config.engines.example.json` (config, static) — 06-11

**Analog:** itself. It already has a `_comment` and array entries `claude`, `claude-max` with `command` as an argv array and `env.CLAUDE_CONFIG_DIR`. D-41 changes it to the FINAL shape: `command` = executable only, `pin_profile` instead of `env.CLAUDE_CONFIG_DIR`, no `claude` entry (reserved id), dsh entry with discrete fields and `allowed_profiles: ["personal"]` (D-38), `key_env` NAMES only. Add a test that loads it through the real parser (pattern: `config.rs` tests that parse JSON text via `parse_engines`).

---

### `spikes/e09/{run.sh,verify.sh,NOTES.md}`, `spikes/s6/run.sh` (script, batch) — 06-20

**Analog:** `spikes/e09/run.sh` itself. Conventions to keep: `set -u`, `. "$(dirname "${BASH_SOURCE[0]}")/../lib/env.sh"`, `die()` helper, env knobs (`E09_CLAUDE_BIN`, `E09_BASE_URL`, `E09_MODEL`, `E09_EXTRA_MODEL`), `E09_OFFLINE=1` self-test using `tests/fakes/fake_claude.py` and a planted key (`KEY="e09-offline-planted-0123456789"`), stage output outside the repo, scrub, leak-check, then copy; exit codes 0/1/2 (missing key)/3 (leak gate). New: parameterise per endpoint (gateway via `E09_BASE_URL`/`E09_MODEL`, key-variable name as a parameter, key only in the env). `spikes/s6/run.sh`: keep `guard_scr`, `docker exec "$C" ... </dev/null`, "no name-based signalling", marker/PID approach; `--kill-wrapper` and optional `--uid` variants are user-run. Fixtures `tests/fixtures/stream/anthropic_e09_*` are user output, never written by an agent.

---

### `scripts/phase6-live-check.sh` (script, batch) — 06-19 Task 1

**Analog:** `scripts/board-live-check.sh` (442 lines): `set -euo pipefail`; `SCRIPT_DIR`/`REPO_ROOT` via `cd -P`; arg loop with `--keep` and exit 2 on unknown; `BIN="${AB_BIN:-$REPO_ROOT/agent-brain/target/debug/agent-brain}"` with a build hint; tool check loop (`python3 curl`); isolated `TMP="$(mktemp -d)"` with HOME, XDG_*, `AGENT_BRAIN_DB`, `AGENT_BRAIN_CONFIG` inside it; serve in the background with the PID recorded and stopped by exact PID (no pkill); cookie through curl stdin (`-K -`), never on argv; `fetch NAME QUERY` writes `$TMP/out/NAME.json` + `.code`; final line `"... live check OK"`. Phase 6 additions per the research table: array-shape config (`gw` anthropic with fake claude, `agy` fake, `dsh` with `fake_docker.py`, `claude-max` with `pin_profile`), two planted keys exported and swept by grep, assertions listed in "06-19 Revision" (engines `claude, gw, claude`; dsh `usage_source none`; quota then re-run on `mac-dinh`; `claude-max` refused with 422 on the work project `live-p6`, accepted on a personal scratch project; `/api/stats` rows per id). The "E-09 failed -> skip" truth is deleted.

---

### `agent-brain/LIVE-CHECKS.md` and `README.md` (doc) — 06-19 Task 2

**Analog (LIVE-CHECKS):** § 7 Phase 5.5 and § 6 Phase 5.4 (`## N. Nghiệm thu Phase X`, then `### P5.4-00: Chuẩn bị môi trường cô lập` followed by numbered `### P5.4-0N: ...` scenarios, ending `### Dọn dẹp`). There is no `## 8` yet: add `## 8. Nghiệm thu Phase 6` with `### P6-00 … P6-10` (11 headings; acceptance grep `^### P6-(0[0-9]|10)`). P6-02 uses the endpoint that passed E-09; add P6-09 (dsh black box, kill, no `ab-dsh-` marker, root-owned file check) and P6-10 (`claude-max` refused on work, runs on personal).

**Analog (README):** sections at `## 5. Cấu hình` (91; keys table 101-197, workflows 211, autopilot 252), `## 11. An toàn dữ liệu` (335), `## 14. Dashboard web` (376). README currently has no engines text: add the array shape, kinds, fields, `key_env` NAME only, `allowed_profiles`/`pin_profile`, both shapes, built-in `tiet-kiem`/`deepseek` note (D-43), a minimal `config.json` to §5; dsh blast radius (root, `/mnt/data`, docker.sock, root-owned files) to §11; Settings "Engine" list to §14. Exit-code table (§4) unchanged.

## Shared Patterns

### Fail before any run row
**Source:** `agent-brain/src/roles/mod.rs:427-445` + `engine.rs plan_for` returns `BrainResult`.
**Apply to:** unknown engine id, profile refusal, dsh stub, `claude` reserved id at run time. Return `BrainError::error("<Vietnamese message>")` before `insert_run`; never switch engines silently (D-16).

### Secrets: names only, strip all key variables, scrub output
**Source:** `engine.rs:415-421` (env retain), `config.rs:111-118` (`valid_key_env`), `EnginePlan.redact`.
**Apply to:** anthropic, agy, dsh arms; `get_engines` payload; README/example. Test against planted keys (`tests/engine_agy.rs:266`, `spikes/e09/run.sh` planted key).

### Vietnamese field-level validation errors
**Source:** `teams.rs:204-233` (`field_error(role, "...")` then `continue`), `config.rs` `"Cấu hình engines.<x> không hợp lệ (...)"`.
**Apply to:** D-37, D-39, D-41 refusals and all new config fields.

### Test sandbox
**Source:** `tests/common/mod.rs` `Sandbox` (`write_config`, `write_teams`, `write_map`, `make_repo`, `project_entry`, `run_env`, `spawn_env`, `db`, `root`).
**Apply to:** every new Rust integration test; fakes are launched by absolute path via `common::repo_root().join("tests").join("fakes")`.

### Process safety
**Source:** CLAUDE.md "How to call agy", `runner.rs cmd_kill`, `tests/engine_agy.rs:678-769`.
**Apply to:** dsh stop hook and kill test: record descendants, SIGTERM the recorded PID, sweep recorded descendants; no `pkill`/`killall`.

## No Analog Found

| File / part | Role | Data Flow | Reason |
|---|---|---|---|
| dsh in-container stop hook (marker/PID file, `docker exec C kill`) in `runner.rs` | service | event-driven | Runner only kills host process trees; use `spikes/s6/run.sh --kill` marker approach and RESEARCH "The dsh kind" |
| `docs/config.engines.example.json` loader test | test | static | No existing test loads a `docs/` file; follow the `config.rs` parse tests and add a `repo_root()`-relative read |

## Metadata

**Analog search scope:** `agent-brain/src`, `agent-brain/tests`, `agent-brain/assets/dashboard/js`, `tests/fakes`, `spikes/e09`, `spikes/s6`, `scripts`, `docs`, `agent-brain/{README,LIVE-CHECKS}.md`
**Tracked-source check:** `engine.rs`, `tests/fakes/fake_agy.py`, `docs/config.engines.example.json`, `spikes/s6/run.sh` confirmed by `git ls-files`; no `.gsd` mirror paths used.
**Pattern extraction date:** 2026-10-28
