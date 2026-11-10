# Phase 7: Journal Vault (Obsidian) - Pattern Map

**Mapped:** 2026-11-10
**Files analyzed:** 27 new/modified
**Analogs found:** 26 / 27 (all paths relative to `agent-brain/` unless noted; all verified git-tracked)

No real task titles or data are quoted here; fixtures in plans must use invented text.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match |
|---|---|---|---|---|
| `src/lib.rs` (+`pub mod journal;` after `inputs`) | config | n/a | same file, alphabetical `pub mod` list (lines ~20-40) | exact |
| `src/journal/mod.rs` (`run_export_once`, `cmd_export`) | service | batch | `src/fixbug/` `cmd_check(&ctx, ..)` style CLI fns | role-match |
| `src/journal/config.rs` (`JournalConfig`, `journal_group_of`) | config | transform | `src/projects.rs:548-563` `preflight_of` + `src/preflight.rs:300-333` `read_lenient` | exact |
| `src/journal/collect.rs` | service | CRUD (read) | `src/stats.rs` `query_by_day_in` (219-232) + `db::runs_for_task` | role-match |
| `src/journal/model.rs`, `render.rs` | utility | transform (pure) | `dashboard::taskstate::step_state` (taskstate.rs:107-137) | role-match |
| `src/journal/sanitize.rs` | utility | transform | `src/secrets.rs` (value-based; patterns are NEW) | partial |
| `src/journal/writer.rs` | utility | file-I/O | `src/gitops.rs` (git + fs helpers) | partial |
| `src/journal/gitsync.rs` | service | request-response (external process) | `src/gitops.rs:1-20` `git_cmd` (env pattern only; do NOT reuse, it is private, no timeout) | partial |
| `src/journal/state.rs` (kv + mailbox) | model | CRUD | `src/db.rs` helpers (`upsert_task` etc.) | role-match |
| `src/journal/fts.rs` | model | CRUD | none for FTS5 (use RESEARCH sec. 3); fold via `routing::normalize` | none |
| `src/journal/digest.rs` | utility | transform | `src/stats.rs:219` `day_key<Tz>` | role-match |
| `src/journal/loop_.rs` | service | event-driven | `src/orchestrator/mod.rs:185-219` `spawn_loop` | exact |
| lock in journal | utility | file-I/O | `src/dashboard/lock.rs:26-60` `acquire` | exact |
| `src/dashboard/mod.rs` (+spawn) | config | event-driven | line 271 `orchestrator::spawn_loop` | exact |
| `src/main.rs` (`Journal` cmd) | route | request-response | `Commands::Fixbug` (main.rs:468-494) | exact |
| `src/config.rs` (`journal` section) | config | transform | `orchestrator` block (1394-1458) + `telegram_max_tasks_per_day` (1461-1471) | exact |
| `src/db.rs` migration 12 + FK fix | migration | CRUD | `MIGRATIONS` tail (db.rs:190-195) + `open` (276-305) | exact |
| `src/telegram/journal_cmd.rs` (`/tim`) | controller | request-response | `src/telegram/fixbug_cmd.rs` | exact |
| `src/telegram/intake.rs` (+`"tim"` arm) | controller | request-response | `on_command` match (711-729) | exact |
| `src/telegram/detect.rs` (+`JournalMail` scan) | service | event-driven | `ChainWaits` (459-500) | exact |
| `src/telegram/mod.rs` setMyCommands (+`tim`), `bot.rs` HELP_TEXT | config | n/a | existing list (mod.rs:230-238) | exact |
| `tests/journal_*.rs` (golden, idempotency, sanitizer, fts, digest) | test | transform | `tests/dashboard_stats.rs` (FixedOffset day tests) | role-match |
| `tests/journal_gitsync.rs` | test | file-I/O | `tests/human_engine.rs:24-30` `bare_remote` | exact |
| `tests/journal_telegram.rs` | test | event-driven | `tests/telegram_notify.rs:1-30` | exact |
| `tests/support/journal.rs` | test util | n/a | `tests/support/telegram.rs`, `tests/support/protocol.rs` | exact |
| `tests/db_migration.rs` (v11->v12, version assert) | test | CRUD | `build_v9` + `v9_db_upgrades_to_v10_keeping_data` (1454-1482) | exact |
| `scripts/phase7-live-check.sh` (repo root) | script | batch | `scripts/phase6-live-check.sh` | exact |

## Pattern Assignments

### `src/journal/loop_.rs` (service, event-driven)
**Analog:** `src/orchestrator/mod.rs:185-219`
```rust
pub fn spawn_loop(st: Arc<AppState>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut shutdown = st.shutdown.clone();
        loop {
            tokio::select! {
                _ = shutdown.changed() => { if *shutdown.borrow() { break; } }
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    let st_clone = st.clone();
                    let res = tokio::task::spawn_blocking(move || { tick(&st_clone) }).await;
                    // errors only: crate::term::emit(&format!("... error: {}", e.message));
```
Journal: tick every ~60 s, `run_export_once` decides due-ness. Wire in `src/dashboard/mod.rs:270-271` next to `let _orchestrator = crate::orchestrator::spawn_loop(state.clone());`.

### Journal lock (`journal/mod.rs`)
**Analog:** `src/dashboard/lock.rs:26-60`: `acquire(dir)` is hard-wired to `LOCK_FILE`; create dir with mode 0o700, `OpenOptions` read/write/create with `.mode(0o600)`, then `libc::flock(LOCK_EX|LOCK_NB)`. Copy or add `acquire_named(dir, name)`; use `journal.lock`.

### `src/journal/config.rs` (`journal_group_of`)
**Analog:** `src/projects.rs:548-563`
```rust
pub fn preflight_of(map: &serde_json::Value, project_id: &str) -> crate::preflight::Preflight {
    let Some(obj) = map.get("projects").and_then(|p| p.as_array()).and_then(|arr| {
            arr.iter().find(|p| p.get("id").and_then(|v| v.as_str()) == Some(project_id))
        }) else { return crate::preflight::Preflight::default(); };
    crate::preflight::read_lenient(obj)
}
```
Lenient rule from `preflight.rs:300-333`: invalid key keeps default and calls `crate::term::emit("Cảnh báo: {key} không hợp lệ — dùng mặc định")`; never error. Missing/invalid `journal_group` -> Công việc (D-08). Do NOT add a field to `Project` (41 struct literals).

### `src/config.rs` (`journal` section)
**Analog:** `config.rs:1394-1458`: `let mut x = XConfig::default(); if let Some(v) = obj.get("x") { if let Some(o) = v.as_object() { if let Some(en) = o.get("enabled") { match en.as_bool() { Some(b) => ..., None => return Err(BrainError::error("Cấu hình orchestrator.enabled phải là boolean")) }`; range style from 1461-1471:
```rust
Some(val) => match val.as_u64() {
    Some(n) if n <= 1000 => n as u32,
    _ => return Err(BrainError::error("Cấu hình telegram_max_tasks_per_day phải là số nguyên từ 0 đến 1000")),
```
Add `pub journal: JournalConfig` to `Config` (1040-1060) and to the single `Ok(Config { ... })` literal (~1501). Absent section = old behaviour (off); add test for that.

### `src/main.rs` (`journal export`)
**Analog:** `Commands` enum (main.rs:21+) with `#[command(subcommand)] subcommand: ...`; dispatch in final match after `open_ctx()`:
```rust
Commands::Fixbug { subcommand } => Ok(match subcommand {
    FixbugCommands::Check { project } => agent_brain::fixbug::cmd_check(&ctx, &project),
```
Add `Commands::Journal { subcommand }` plus `JournalCommands::Export { #[arg(long)] no_push: bool, #[arg(long)] rebuild: bool }` -> `agent_brain::journal::cmd_export(&ctx, ...)`. Doc comments are Vietnamese.

### `src/db.rs` migration 12 + FK-check fix
**Analog:** `MIGRATIONS` tail (db.rs:190-195):
```rust
ALTER TABLE intake ADD COLUMN split INTEGER NOT NULL DEFAULT 1;"#,
    r#"ALTER TABLE tasks ADD COLUMN workdir TEXT;"#,
];
pub const SCHEMA_VERSION: i64 = 11;   // -> 12; append r#"CREATE TABLE journal_state ..., journal_mail ..., CREATE VIRTUAL TABLE journal_fts USING fts5(...)"#
```
FK blocker, `open()` db.rs:283-300:
```rust
let bad: i64 = tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
if bad > 0 { return Err(BrainError::error("Nâng cấp CSDL thất bại: vi phạm khóa ngoại sau khi dựng lại bảng runs")); }
```
Fix: compare violations to those present before the pending migrations (count before the loop; fail only if count grew), or restrict the check to tables touched by the migration. Add a test with seeded orphan `events` rows that upgrades successfully.

### `tests/db_migration.rs` (v11->v12)
**Analog:** `build_v9` (1454-1482): build the old DB by replaying `agent_brain::db::MIGRATIONS[..N]`, set `user_version`, insert rows, then open via `db::open` and assert data kept. Update the line asserting `SCHEMA_VERSION` equal to 11 (about line 976 per RESEARCH; other asserts use `agent_brain::db::SCHEMA_VERSION` so stay valid).

### `src/telegram/journal_cmd.rs` (`/tim`) and `intake.rs` arm
**Analog:** `src/telegram/fixbug_cmd.rs:1-40` (imports, USAGE consts, pure `parse_*` returning `Result<_, String>`, replies via `bot::reply(ctx, &text)`, `clip`/`MAX_TEXT` from `super::render`):
```rust
use super::bot;
use super::render::{clip, MAX_TEXT};
use super::BotCtx;
pub const USAGE_BUGS: &str = "Cách dùng: /bugs <dự án>";
```
Dispatch arm in `intake.rs:711-729`: `"reject" => super::fixbug_cmd::on_reject(ctx, rest),` -> add `"tim" => super::journal_cmd::on_tim(ctx, rest),` (declare `mod journal_cmd;` in telegram/mod.rs). Pairing is enforced earlier by `bot::authz`. Also update setMyCommands (`telegram/mod.rs:230-238`), `HELP_TEXT` (bot.rs:42) and the exact-list assertion in `tests/telegram_intake.rs` (~1953-1971).

### `src/telegram/detect.rs` (`JournalMail` scan)
**Analog:** `ChainWaits` (detect.rs:459-500):
```rust
pub struct ChainWaits { seen: HashSet<(i64, String)> }
impl ChainWaits {
    pub fn seed(conn: &Connection) -> BrainResult<ChainWaits> {...}
    pub fn scan(&mut self, conn: &Connection) -> BrainResult<Vec<Outgoing>> {
        ... out.push(Outgoing::Message { text: chain_wait_text(id, &title, &key.1) });
```
Seed at start-up (line ~546: `.and_then(|(f, r)| ChainWaits::seed(&conn).map(|w| (f, r, w)))`) so a restart never resends. For the mailbox use claim-before-push (UPDATE ... sent=1 then push) instead of an in-memory set. Messages are plain text (no parse_mode).

### `src/journal/sanitize.rs`
`src/secrets.rs` redacts known secret values only; the key-shape/path/URL patterns and final gate `scan_generated()` are new (use `regex`). Fold text with `routing::normalize` (also for FTS).

### `src/journal/gitsync.rs` and `writer.rs`
**Analog (env only):** `gitops.rs:9-20`:
```rust
fn git_cmd(root: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(root);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
```
Own runner adds: timeout, `GIT_SSH_COMMAND` BatchMode, fixed author env (`agent-brain journal`), distinguishing "nothing to commit" via `diff --cached --quiet`.

### `src/journal/digest.rs`, `collect.rs`: day math
**Analog:** `src/stats.rs:219`
```rust
pub fn day_key<Tz: chrono::TimeZone>(ms: i64, tz: &Tz) -> String {
    match tz.timestamp_millis_opt(ms).single() { Some(dt) => dt.date_naive().format("%Y-%m-%d").to_string(), None => ... }
```
Generic over `Tz`; `chrono::Local` in prod, `FixedOffset::east_opt(7*3600)` in tests (as `tests/dashboard_stats.rs`). Also `telegram::intake::local_midnight_ms` (intake.rs:442).

### `tests/journal_gitsync.rs` (bare remote)
**Analog:** `tests/human_engine.rs:24-30`
```rust
fn bare_remote(sb: &Sandbox) -> PathBuf {
    let dir = sb.root.join("remote.git");
    std::fs::create_dir_all(&dir).unwrap();
    sb.git(&dir, &["init", "--bare", "-b", "main"]);
    dir
}
```
`Sandbox::git` (tests/common/mod.rs:389) uses `env_clear`, temp `HOME`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM=1`. A second clone simulates a human edit/conflict.

### `tests/journal_telegram.rs` and `tests/support/journal.rs`
**Analog:** `tests/telegram_notify.rs:1-17`
```rust
mod common;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;
use tg::{write_config, FakeTelegram, PAIRED_CHAT};
```
`FakeTelegram::start(&sb)` launches `<repo root>/tests/fakes/fake_telegram.py` (note: repo-root `tests/fakes`, via `common::repo_root()`, not `agent-brain/tests`). Use `calls("sendMessage")`, `push_update(...)`; stranger chat constant `STRANGER_CHAT` for the unpaired-`/tim` test. Put journal helpers (seed DB rows, bare remote, `AGENT_BRAIN_JOURNAL_NOW_MS` hook) in `tests/support/journal.rs`, included with `#[allow(dead_code)] #[path = "support/journal.rs"] mod journal;`.

### `scripts/phase7-live-check.sh`
**Analog:** `scripts/phase6-live-check.sh`: header comment listing modes (`--keep`, `--schema-dry-run`), `set -euo pipefail`, `SCRIPT_DIR`/`REPO_ROOT`, `BIN="${AB_BIN:-$REPO_ROOT/agent-brain/target/debug/agent-brain}"`, tool check loop (`python3 curl git`), `TMP="$(mktemp -d)"`, `cleanup()` that kills only the recorded `SERVE_PID` (INT, then KILL after 10 s). Isolated HOME/XDG/`AGENT_BRAIN_DB`/`AGENT_BRAIN_CONFIG`; use a local bare repo as the vault remote; schema dry-run on a COPY of brain.db; final line `Phase 7 live check OK`.

## Shared Patterns

- **Errors:** `BrainResult`/`BrainError::error("Vietnamese message")` (`src/error.rs`); background loops only `crate::term::emit(...)`, never panic.
- **Lenient reads of hand-edited JSON:** warn via `term::emit`, keep default (preflight.rs:300-333).
- **No hard code (repo rule):** no project/workflow names in code; each option is config with default = old behaviour, range-checked, with an "absent = old behaviour" test.
- **Tests:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1`; `common::Sandbox` isolates HOME/DB.

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `src/journal/fts.rs` | model | CRUD | No FTS5 usage exists; follow RESEARCH.md section 3 (fold in Rust, `unicode61 remove_diacritics 0`, quoted-token MATCH) |

## Metadata

**Analog search scope:** `agent-brain/src`, `agent-brain/tests`, `tests/fakes`, `scripts/`
**Pattern extraction date:** 2026-11-10
