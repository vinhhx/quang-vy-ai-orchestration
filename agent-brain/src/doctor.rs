//! `agent-brain doctor`: a watchdog that is independent of the run supervisor.
//!
//! The supervisor already stops a run that returned its result without exiting
//! (`finish_after_result`) or went silent (`run_stall_minutes`), but both live inside the
//! supervising process: if it hangs or dies, or if something else is wrong (an orphan autopilot
//! driver, a full disk, `serve` not running), nobody notices. `scan` looks at brain.db and the
//! run folders from outside and returns `Finding`s. It only reports; the one exception is a run
//! that is marked `running` while its process is gone (`run-dead`), which `heal_dead_runs`
//! marks `lost`. It never kills a run, changes `auto_state` or deletes a file.
//!
//! The same scan feeds the CLI (`agent-brain doctor`) and the serve detector
//! (`telegram::detect::DoctorAlerts`). `Dedup` keeps one alert per finding key and repeats it
//! only after `realert_minutes`. Messages carry ids, project slugs and roles only: no path, no
//! token and no text of the user's request.

use crate::autopilot::claim;
use crate::config::Config;
use crate::db::{self, DriverRow, RunRow};
use crate::error::BrainResult;
use crate::procs::{self, ProcId};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Exit code: no finding.
pub const EXIT_CLEAN: i32 = 0;
/// Exit code: at least one finding.
pub const EXIT_FINDINGS: i32 = 1;
/// Exit code: the command itself failed (no database, no Telegram setup for `--notify`, ...).
pub const EXIT_ERROR: i32 = 2;

/// A run with no recorded process younger than this is just being spawned, not dead.
const SPAWN_GRACE_MS: i64 = 2 * 60_000;
/// The most findings listed in one Telegram message; the rest is counted.
pub const MAX_PER_MESSAGE: usize = 5;
/// Name of the dedup state file in the data directory.
pub const STATE_FILE: &str = "doctor-state.json";

const TAIL_CHUNK: u64 = 16 * 1024;
const TAIL_MAX_CHUNKS: u64 = 8;

const HEAL_HINT: &str = "Sửa: chạy agent-brain doctor --heal.";
const HEALED_NOTE: &str = "Đã đánh dấu lost.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// `run-dead`, `result-no-exit`, `run-silent`, `run-long`, `driver-stale`, `queue-stuck`,
    /// `disk-low` or `serve-down`.
    pub kind: &'static str,
    /// Stable id of the finding (`run-silent:312`), used to avoid sending it twice.
    pub key: String,
    pub task_id: Option<i64>,
    pub run_id: Option<i64>,
    /// One Vietnamese sentence with a hint of the command to use.
    pub text: String,
}

/// Every threshold of one scan, in plain numbers so a test can set them without a `Config`.
#[derive(Debug, Clone)]
pub struct DoctorOpts {
    pub result_no_exit_minutes: u64,
    pub silent_minutes: u64,
    /// `run_stall_minutes` of the config: when above 0 the silence limit is this plus 5.
    pub run_stall_minutes: u64,
    pub run_max_minutes: u64,
    pub driver_stale_minutes: u64,
    pub queue_stuck_minutes: u64,
    /// Free space below this many GB is a finding; 0 = off.
    pub disk_min_gb: u64,
    pub realert_minutes: u64,
    /// Free space measured by the caller (`disk_free_gb`); `None` = unknown, no finding.
    pub free_gb: Option<u64>,
    /// Look at `serve.pid` / `serve.lock` in `data_dir` (`--check-serve`).
    pub check_serve: bool,
    pub data_dir: PathBuf,
}

impl DoctorOpts {
    /// Thresholds of `cfg.doctor`; `free_gb` stays unmeasured and `check_serve` off.
    pub fn from_config(cfg: &Config) -> DoctorOpts {
        let d = &cfg.doctor;
        DoctorOpts {
            result_no_exit_minutes: d.result_no_exit_minutes,
            silent_minutes: d.silent_minutes,
            run_stall_minutes: cfg.run_stall_minutes,
            run_max_minutes: d.run_max_minutes,
            driver_stale_minutes: d.driver_stale_minutes,
            queue_stuck_minutes: d.queue_stuck_minutes,
            disk_min_gb: d.disk_min_gb,
            realert_minutes: d.realert_minutes,
            free_gb: None,
            check_serve: false,
            data_dir: crate::config::data_dir(),
        }
    }

    /// The silence limit of a live run, in minutes.
    pub fn silent_limit_minutes(&self) -> u64 {
        if self.run_stall_minutes > 0 {
            self.run_stall_minutes + 5
        } else {
            self.silent_minutes
        }
    }

    pub fn realert_ms(&self) -> i64 {
        (self.realert_minutes as i64).saturating_mul(60_000)
    }
}

fn minutes_ms(m: u64) -> i64 {
    (m as i64).saturating_mul(60_000)
}

fn proc_of(pid: Option<i64>, start: Option<i64>) -> Option<ProcId> {
    match (pid, start) {
        (Some(p), Some(s)) if p > 0 && s >= 0 => Some(ProcId {
            pid: p as i32,
            start: s as u64,
        }),
        _ => None,
    }
}

/// Slug of the project behind `projects.id`, cached for one scan.
#[derive(Default)]
struct Names(HashMap<i64, String>);

impl Names {
    fn project(&mut self, conn: &Connection, id: i64) -> String {
        self.0
            .entry(id)
            .or_insert_with(|| {
                conn.query_row("SELECT slug FROM projects WHERE id = ?1", [id], |r| r.get(0))
                    .unwrap_or_else(|_| "?".to_string())
            })
            .clone()
    }
}

fn describe_run(run: &RunRow, project: &str) -> String {
    match run.task_id {
        Some(t) => format!("Run {} ({}, task {t}, dự án {project})", run.id, run.role),
        None => format!("Run {} ({}, dự án {project})", run.id, run.role),
    }
}

fn kill_hint(task_id: Option<i64>) -> String {
    match task_id {
        Some(t) => format!(" Gợi ý: agent-brain kill {t}."),
        None => String::new(),
    }
}

fn mtime_ms(path: &Path) -> Option<i64> {
    let m = std::fs::metadata(path).ok()?.modified().ok()?;
    let d = m.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(d.as_millis() as i64)
}

/// Whether the last line of `stream.jsonl` is the engine's final `result` event
/// (`"type":"result"` for claude, `"event":"result"` for agy). Reads only the end of the file
/// (16 KiB at a time, at most 128 KiB when the last line is long); never the whole file.
pub fn last_event_is_result(path: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(len) = f.metadata().map(|m| m.len()) else {
        return false;
    };
    let mut buf: Vec<u8> = Vec::new();
    let mut read = 0u64;
    loop {
        let chunk = TAIL_CHUNK.min(len - read);
        if chunk == 0 {
            return false;
        }
        read += chunk;
        let mut part = vec![0u8; chunk as usize];
        if f.seek(SeekFrom::Start(len - read)).is_err() || f.read_exact(&mut part).is_err() {
            return false;
        }
        part.extend_from_slice(&buf);
        buf = part;
        if let Some(end) = buf.iter().rposition(|b| !b.is_ascii_whitespace()) {
            let start = buf[..end].iter().rposition(|b| *b == b'\n').map(|i| i + 1);
            if start.is_some() || read >= len {
                let line = &buf[start.unwrap_or(0)..=end];
                return match serde_json::from_slice::<serde_json::Value>(line) {
                    Ok(v) => {
                        v.get("type").and_then(|t| t.as_str()) == Some("result")
                            || v.get("event").and_then(|t| t.as_str()) == Some("result")
                    }
                    Err(_) => false,
                };
            }
        } else if read >= len {
            return false;
        }
        if read >= TAIL_CHUNK * TAIL_MAX_CHUNKS {
            return false;
        }
    }
}

fn scan_run(
    conn: &Connection,
    now_ms: i64,
    opts: &DoctorOpts,
    run: &RunRow,
    names: &mut Names,
    out: &mut Vec<Finding>,
) {
    let who = describe_run(run, &names.project(conn, run.project_id));
    let age_ms = now_ms - run.started_at;
    let alive = proc_of(run.pid, run.pid_start).map(procs::alive);
    match alive {
        Some(false) => {
            out.push(Finding {
                kind: "run-dead",
                key: format!("run-dead:{}", run.id),
                task_id: run.task_id,
                run_id: Some(run.id),
                text: format!("{who} vẫn ghi \"running\" nhưng tiến trình đã chết. {HEAL_HINT}"),
            });
            return;
        }
        None => {
            if age_ms > SPAWN_GRACE_MS {
                out.push(Finding {
                    kind: "run-dead",
                    key: format!("run-dead:{}", run.id),
                    task_id: run.task_id,
                    run_id: Some(run.id),
                    text: format!(
                        "{who} vẫn ghi \"running\" nhưng không có tiến trình nào được ghi nhận sau {} phút. {HEAL_HINT}",
                        age_ms / 60_000
                    ),
                });
            }
            return;
        }
        Some(true) => {}
    }

    // The process is alive: look at what the engine wrote.
    let stream = run
        .run_dir
        .as_deref()
        .map(|d| Path::new(d).join("stream.jsonl"));
    let stream_mtime = stream.as_deref().and_then(mtime_ms);
    let result_seen = stream_mtime.is_some()
        && stream.as_deref().map(last_event_is_result).unwrap_or(false);
    if result_seen {
        let idle = now_ms - stream_mtime.unwrap_or(now_ms);
        if idle > minutes_ms(opts.result_no_exit_minutes) {
            out.push(Finding {
                kind: "result-no-exit",
                key: format!("result-no-exit:{}", run.id),
                task_id: run.task_id,
                run_id: Some(run.id),
                text: format!(
                    "{who} đã báo kết quả cuối {} phút trước nhưng tiến trình chưa thoát.{}",
                    idle / 60_000,
                    kill_hint(run.task_id)
                ),
            });
        }
    } else {
        let last = stream_mtime.unwrap_or(run.started_at);
        let silent = now_ms - last;
        if silent > minutes_ms(opts.silent_limit_minutes()) {
            out.push(Finding {
                kind: "run-silent",
                key: format!("run-silent:{}", run.id),
                task_id: run.task_id,
                run_id: Some(run.id),
                text: format!(
                    "{who} im lặng {} phút (engine không ghi gì).{}",
                    silent / 60_000,
                    kill_hint(run.task_id)
                ),
            });
        }
    }
    if age_ms > minutes_ms(opts.run_max_minutes) {
        out.push(Finding {
            kind: "run-long",
            key: format!("run-long:{}", run.id),
            task_id: run.task_id,
            run_id: Some(run.id),
            text: format!(
                "{who} đã chạy {} phút (quá {} phút).{}",
                age_ms / 60_000,
                opts.run_max_minutes,
                kill_hint(run.task_id)
            ),
        });
    }
}

/// A run that still counts as alive: its process runs, or it was spawned a moment ago.
fn run_is_live(run: &RunRow, now_ms: i64) -> bool {
    match proc_of(run.pid, run.pid_start) {
        Some(p) => procs::alive(p),
        None => now_ms - run.started_at <= SPAWN_GRACE_MS,
    }
}

fn scan_drivers(
    conn: &Connection,
    now_ms: i64,
    opts: &DoctorOpts,
    running: &[RunRow],
    out: &mut Vec<Finding>,
) -> BrainResult<()> {
    let mut stmt = conn.prepare(
        "SELECT t.id, p.slug, p.profile, t.auto_state, t.auto_pid, t.auto_pid_start, t.updated_at
         FROM tasks t JOIN projects p ON p.id = t.project_id
         WHERE t.auto_state IN ('running', 'stopping') ORDER BY t.id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                DriverRow {
                    task_id: r.get(0)?,
                    project_slug: r.get(1)?,
                    profile: r.get(2)?,
                    auto_pid: r.get(4)?,
                    auto_pid_start: r.get(5)?,
                },
                r.get::<_, String>(3)?,
                r.get::<_, i64>(6)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (driver, state, updated_at) in rows {
        if claim::row_alive(&driver) {
            continue;
        }
        if running
            .iter()
            .any(|r| r.task_id == Some(driver.task_id) && run_is_live(r, now_ms))
        {
            continue;
        }
        let idle = now_ms - updated_at;
        if idle <= minutes_ms(opts.driver_stale_minutes) {
            continue;
        }
        let t = driver.task_id;
        out.push(Finding {
            kind: "driver-stale",
            key: format!("driver-stale:{t}"),
            task_id: Some(t),
            run_id: None,
            text: format!(
                "Task {t} (dự án {}): autopilot ở trạng thái {state} nhưng không có tiến trình điều khiển hay run nào đang chạy ({} phút). Gợi ý: agent-brain auto {t} để chạy tiếp, hoặc agent-brain kill {t}.",
                driver.project_slug,
                idle / 60_000
            ),
        });
    }
    Ok(())
}

fn scan_queue(
    conn: &Connection,
    now_ms: i64,
    opts: &DoctorOpts,
    out: &mut Vec<Finding>,
) -> BrainResult<()> {
    for q in db::waiting_actions(conn)? {
        let waited = now_ms - q.created_at;
        if waited <= minutes_ms(opts.queue_stuck_minutes) {
            continue;
        }
        let project: Option<(String, String)> = conn
            .query_row(
                "SELECT p.slug, p.profile FROM tasks t JOIN projects p ON p.id = t.project_id WHERE t.id = ?1",
                [q.task_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        let Some((slug, profile)) = project else {
            continue;
        };
        if claim::project_busy_reason(conn, &slug, &profile, Some(q.task_id), false)?.is_some() {
            continue;
        }
        out.push(Finding {
            kind: "queue-stuck",
            key: format!("queue-stuck:{}", q.id),
            task_id: Some(q.task_id),
            run_id: None,
            text: format!(
                "Task {} (dự án {slug}): hành động xếp hàng {} chờ {} phút dù dự án không bận. Gợi ý: kiểm tra agent-brain serve đang chạy (agent-brain doctor --check-serve) hoặc chạy lại bước đó bằng tay.",
                q.task_id,
                q.action_id,
                waited / 60_000
            ),
        });
    }
    Ok(())
}

/// Pid written by `serve`: `serve.pid` (plain number or JSON with `pid`) or the JSON `serve.lock`
/// that `agent-brain serve` writes.
fn serve_pid(dir: &Path) -> Option<i32> {
    for name in ["serve.pid", crate::dashboard::lock::LOCK_FILE] {
        let Ok(text) = std::fs::read_to_string(dir.join(name)) else {
            continue;
        };
        let text = text.trim();
        let pid = text.parse::<i64>().ok().or_else(|| {
            serde_json::from_str::<serde_json::Value>(text)
                .ok()?
                .get("pid")?
                .as_i64()
        });
        if let Some(p) = pid.filter(|p| *p > 0 && *p <= i32::MAX as i64) {
            return Some(p as i32);
        }
    }
    None
}

fn scan_serve(
    conn: &Connection,
    opts: &DoctorOpts,
    running: &[RunRow],
    out: &mut Vec<Finding>,
) -> BrainResult<()> {
    let Some(pid) = serve_pid(&opts.data_dir) else {
        return Ok(());
    };
    if procs::proc_id(pid).map(procs::alive).unwrap_or(false) {
        return Ok(());
    }
    let drivers: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE auto_state = 'running'",
        [],
        |r| r.get(0),
    )?;
    if running.is_empty() && drivers == 0 {
        return Ok(());
    }
    out.push(Finding {
        kind: "serve-down",
        key: "serve-down".to_string(),
        task_id: None,
        run_id: None,
        text: format!(
            "agent-brain serve không chạy (pid {pid} đã chết) trong khi còn {} run đang chạy và {drivers} task autopilot chạy. Gợi ý: khởi động lại agent-brain serve.",
            running.len()
        ),
    });
    Ok(())
}

/// Scans brain.db and the run folders at `now_ms`. Pure with respect to the caller's thresholds
/// (`opts`); reads only, never changes the database.
pub fn scan(conn: &Connection, now_ms: i64, opts: &DoctorOpts) -> BrainResult<Vec<Finding>> {
    let mut running = db::running_runs(conn)?;
    running.sort_by_key(|r| r.id);
    let mut names = Names::default();
    let mut out = Vec::new();
    for run in &running {
        scan_run(conn, now_ms, opts, run, &mut names, &mut out);
    }
    scan_drivers(conn, now_ms, opts, &running, &mut out)?;
    scan_queue(conn, now_ms, opts, &mut out)?;
    if opts.disk_min_gb > 0 {
        if let Some(free) = opts.free_gb {
            if free < opts.disk_min_gb {
                out.push(Finding {
                    kind: "disk-low",
                    key: "disk-low".to_string(),
                    task_id: None,
                    run_id: None,
                    text: format!(
                        "Ổ đĩa chứa dữ liệu agent-brain chỉ còn {free} GB trống (ngưỡng {} GB). Gợi ý: dọn các task đã merge (agent-brain task cleanup --merged) và thư mục runs cũ.",
                        opts.disk_min_gb
                    ),
                });
            }
        }
    }
    if opts.check_serve {
        scan_serve(conn, opts, &running, &mut out)?;
    }
    Ok(out)
}

/// The only repair: marks every `run-dead` finding's run `lost` (when it is still `running`) and
/// rewrites its text to say so. A finding whose run changed state in the meantime is dropped.
/// Returns how many runs were marked. Nothing else is ever touched.
pub fn heal_dead_runs(conn: &Connection, findings: &mut Vec<Finding>) -> BrainResult<usize> {
    let mut healed = 0;
    let mut keep = Vec::with_capacity(findings.len());
    for mut f in findings.drain(..) {
        if f.kind != "run-dead" {
            keep.push(f);
            continue;
        }
        let Some(run_id) = f.run_id else {
            keep.push(f);
            continue;
        };
        if db::mark_lost_if_running(conn, run_id)? {
            healed += 1;
            f.text = f.text.replace(HEAL_HINT, HEALED_NOTE);
            keep.push(f);
        }
    }
    *findings = keep;
    Ok(healed)
}

/// One line for the terminal: `[kind] sentence`.
pub fn format_finding(f: &Finding) -> String {
    format!("[{}] {}", f.kind, f.text)
}

/// The Telegram message for `findings` (one message): a header, at most `MAX_PER_MESSAGE` lines
/// and `… và N cảnh báo nữa` for the rest.
pub fn alert_text(findings: &[Finding]) -> String {
    let mut lines = vec![format!(
        "agent-brain doctor: {} cảnh báo mới",
        findings.len()
    )];
    for f in findings.iter().take(MAX_PER_MESSAGE) {
        lines.push(format!("- {}", format_finding(f)));
    }
    if findings.len() > MAX_PER_MESSAGE {
        lines.push(format!(
            "… và {} cảnh báo nữa (chạy agent-brain doctor để xem đủ)",
            findings.len() - MAX_PER_MESSAGE
        ));
    }
    lines.join("\n")
}

/// Which findings were already reported, and when: a finding is reported the first time and again
/// after `realert_minutes`; a key that is no longer found is forgotten (it is new when it comes
/// back). Kept in memory by serve and in `doctor-state.json` by the CLI.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dedup {
    #[serde(default)]
    sent: BTreeMap<String, i64>,
}

impl Dedup {
    pub fn new() -> Dedup {
        Dedup::default()
    }

    /// The state file, or an empty state when it is missing or unreadable (the worst case is one
    /// repeated alert).
    pub fn load(path: &Path) -> Dedup {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Atomic write, mode 0600 (temp file, then rename).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() && !dir.exists() {
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(dir)?;
            }
        }
        let tmp = path.with_extension(format!("tmp{}", std::process::id()));
        let body = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let res = (|| {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            f.write_all(&body)?;
            f.sync_all()?;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
            std::fs::rename(&tmp, path)
        })();
        if res.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        res
    }

    /// The findings to report now (new, or last reported `realert_ms` ago or more), marked as
    /// reported at `now_ms`. Keys that are not in `findings` are forgotten.
    pub fn select(&mut self, findings: &[Finding], now_ms: i64, realert_ms: i64) -> Vec<Finding> {
        let present: HashSet<&str> = findings.iter().map(|f| f.key.as_str()).collect();
        self.sent.retain(|k, _| present.contains(k.as_str()));
        let mut due = Vec::new();
        for f in findings {
            let send = match self.sent.get(&f.key) {
                None => true,
                Some(at) => now_ms - at >= realert_ms,
            };
            if send {
                self.sent.insert(f.key.clone(), now_ms);
                due.push(f.clone());
            }
        }
        due
    }

    pub fn is_empty(&self) -> bool {
        self.sent.is_empty()
    }

    pub fn len(&self) -> usize {
        self.sent.len()
    }
}

/// Available space in whole GB of the file system holding `dir` (its nearest existing parent when
/// it does not exist yet), from `df -Pk`. `None` when `df` fails.
pub fn free_gb_of(dir: &Path) -> Option<u64> {
    let mut d = dir.to_path_buf();
    while !d.exists() {
        d = d.parent()?.to_path_buf();
    }
    let out = std::process::Command::new("df")
        .arg("-Pk")
        .arg(&d)
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_df_available_kb(&String::from_utf8_lossy(&out.stdout)).map(|kb| kb / 1024 / 1024)
}

/// "Available" column (KiB) of `df -Pk` output.
pub fn parse_df_available_kb(df_output: &str) -> Option<u64> {
    df_output
        .lines()
        .nth(1)?
        .split_whitespace()
        .nth(3)?
        .parse()
        .ok()
}

/// The smaller free space (GB) of the data directory and the worktree directory.
pub fn disk_free_gb() -> Option<u64> {
    [
        crate::config::data_dir(),
        crate::workspace::worktrees_dir(),
    ]
    .iter()
    .filter_map(|d| free_gb_of(d))
    .min()
}

struct Sender {
    tx: crate::telegram::transport::Transport,
    chat_id: i64,
}

impl Sender {
    /// The paired bot from `telegram.json`, or the Vietnamese reason it cannot be used.
    fn open() -> Result<Sender, String> {
        use crate::telegram::{config, transport};
        let cfg = match config::load(&config::path()) {
            Ok(c) => c,
            Err(config::LoadError::Missing) => {
                return Err(
                    "Chưa ghép nối Telegram — chạy agent-brain telegram setup rồi chạy lại doctor --notify"
                        .to_string(),
                )
            }
            Err(e) => return Err(e.message().unwrap_or_default()),
        };
        let curl = transport::curl_bin();
        transport::check_curl(&curl)
            .map_err(|e| e.replace("agent-brain serve", "agent-brain doctor --notify"))?;
        let base = transport::api_base()?;
        let tx = transport::Transport::new(
            cfg.token,
            base,
            curl,
            crate::config::data_dir().join("telegram-upload"),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        Ok(Sender {
            tx,
            chat_id: cfg.chat_id,
        })
    }

    fn send(&self, text: &str) -> Result<(), String> {
        use crate::telegram::transport::Body;
        self.tx
            .call(
                "sendMessage",
                &Body::Json(serde_json::json!({"chat_id": self.chat_id, "text": text})),
                20,
            )
            .map(|_| ())
            .map_err(|e| format!("Không gửi được tin Telegram: {}", e.text()))
    }
}

/// `agent-brain doctor [--json] [--notify] [--heal] [--check-serve]`. Exit code: 0 no finding,
/// 1 findings, 2 the command failed. Handled before `app::open_ctx`, whose own
/// `reconcile_stale_runs` would mark dead runs `lost` without being asked (`--heal`).
pub fn cmd_doctor(json: bool, notify: bool, heal: bool, check_serve: bool) -> i32 {
    let cfg = match crate::config::load_config() {
        Ok(c) => c,
        Err(e) => {
            crate::term::emit(&e.message);
            return EXIT_ERROR;
        }
    };
    let sender = if notify {
        match Sender::open() {
            Ok(s) => Some(s),
            Err(msg) => {
                crate::term::emit(&msg);
                return EXIT_ERROR;
            }
        }
    } else {
        None
    };
    let db_path = crate::config::db_path();
    if !db_path.is_file() {
        crate::term::emit("Không đọc được CSDL: chưa có brain.db (chưa chạy agent-brain lần nào?)");
        return EXIT_ERROR;
    }
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            crate::term::emit(&format!("Không đọc được CSDL: {}", e.message));
            return EXIT_ERROR;
        }
    };
    let mut opts = DoctorOpts::from_config(&cfg);
    opts.check_serve = check_serve;
    opts.free_gb = disk_free_gb();
    let now = db::now_ms();
    let mut findings = match scan(&conn, now, &opts) {
        Ok(f) => f,
        Err(e) => {
            crate::term::emit(&format!("Không đọc được CSDL: {}", e.message));
            return EXIT_ERROR;
        }
    };
    if heal {
        if let Err(e) = heal_dead_runs(&conn, &mut findings) {
            crate::term::emit(&format!("Không sửa được run chết: {}", e.message));
            return EXIT_ERROR;
        }
    }

    if let Some(sender) = sender {
        let state_path = crate::config::data_dir().join(STATE_FILE);
        let mut dedup = Dedup::load(&state_path);
        let due = dedup.select(&findings, now, opts.realert_ms());
        if !due.is_empty() {
            if let Err(msg) = sender.send(&alert_text(&due)) {
                // Not saved: the findings stay "new" and the next run tries again.
                crate::term::emit(&msg);
                return EXIT_ERROR;
            }
        }
        if let Err(e) = dedup.save(&state_path) {
            crate::term::emit(&format!("Không ghi được trạng thái doctor: {e}"));
            return EXIT_ERROR;
        }
    }

    if json {
        match serde_json::to_string_pretty(&findings) {
            Ok(s) => crate::term::emit_out(&s),
            Err(e) => {
                crate::term::emit(&format!("Không tạo được JSON: {e}"));
                return EXIT_ERROR;
            }
        }
    } else {
        for f in &findings {
            crate::term::emit_out(&format_finding(f));
        }
    }
    if findings.is_empty() {
        EXIT_CLEAN
    } else {
        EXIT_FINDINGS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn df_output_is_parsed() {
        let out = "Filesystem     1024-blocks      Used Available Capacity Mounted on\n/dev/sda2        455000000 251000000 181000000      59% /mnt/data\n";
        assert_eq!(parse_df_available_kb(out), Some(181_000_000));
        assert_eq!(parse_df_available_kb("Filesystem only\n"), None);
        assert_eq!(parse_df_available_kb(""), None);
    }

    #[test]
    fn free_gb_of_a_missing_dir_uses_its_parent() {
        let dir = tempfile::tempdir().unwrap();
        let got = free_gb_of(&dir.path().join("not/yet/there"));
        assert!(got.is_some());
    }

    #[test]
    fn result_event_is_found_in_the_tail_even_when_the_line_is_long() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("stream.jsonl");
        let long = "x".repeat(40_000);
        let body = format!(
            "{{\"type\":\"system\"}}\n{{\"type\":\"result\",\"result\":\"{long}\"}}\n"
        );
        std::fs::write(&p, body).unwrap();
        assert!(last_event_is_result(&p));
        std::fs::write(&p, "{\"event\":\"result\",\"status\":\"SUCCESS\"}\n").unwrap();
        assert!(last_event_is_result(&p));
        // An event after the result means the engine is still talking.
        std::fs::write(&p, "{\"type\":\"result\"}\n{\"type\":\"assistant\"}\n").unwrap();
        assert!(!last_event_is_result(&p));
        std::fs::write(&p, "").unwrap();
        assert!(!last_event_is_result(&p));
        assert!(!last_event_is_result(&dir.path().join("missing.jsonl")));
    }
}
