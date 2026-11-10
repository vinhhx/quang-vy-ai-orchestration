//! Notification detector (Phase 05.3, D-05/D-06/D-07).
//!
//! Follows the dashboard pump's hints (`AppState.hints`: every brain.db change,
//! also from CLI processes) plus a slow safety rescan, compares a cheap per-task
//! signature, and re-derives the task state only for tasks whose signature
//! changed. Events are deduplicated in memory; the set is seeded at start-up so
//! a serve restart never resends an event that already existed. The detector
//! never calls Telegram: it only pushes `Outgoing` items onto the outbox.
//!
//! Sent messages follow their task (D-11/D-12/D-15): when a tracked message's
//! event is no longer current (the task was handled from the dashboard or a
//! terminal), the message is edited to say so and loses its buttons; while
//! the event is current, its buttons follow the task's runs and rev. A message
//! the bot is acting on is taken out of the registry first, so a Telegram
//! action is never edited twice.

use super::outbox::{Outgoing, Registry, Tracked};
use super::render;
use super::{sleep_or_stop, BotCtx};
use crate::autopilot::decide::GateState;
use crate::dashboard::api_tasks::{load_task_state, TaskState};
use crate::dashboard::AppState;
use crate::db::RunRow;
use crate::error::BrainResult;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::error::TryRecvError;

pub const SAFETY_SCAN_SECS: u64 = 60;

const TICK_MS: u64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    Gate,
    Stop,
    QaPass,
    RunFailed,
    /// The latest step stopped on a quota or rate limit (Phase 6, D-17): the user picks a team.
    Quota,
    Answered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub task_id: i64,
    pub kind: EventKind,
    pub key: String,
    pub is_update: bool,
}

fn settling_run(s: &TaskState) -> bool {
    s.runs
        .iter()
        .any(|r| r.status == "running" && (r.role == "lead" || r.role == "plan-review"))
}

/// The run a quota event is about: the task's latest run (any role, as the dashboard's own
/// `Chạy lại bước này bằng đội hình…` reads it) when it ended with outcome `quota` and nothing
/// is running and no autopilot driver is alive. Only then can the shared re-run act.
pub fn quota_run(s: &TaskState) -> Option<&RunRow> {
    if s.driver_alive || s.runs.iter().any(|r| r.status == "running") {
        return None;
    }
    s.runs
        .iter()
        .max_by_key(|r| (r.started_at, r.id))
        .filter(|r| r.outcome.as_deref() == Some(crate::quota::OUTCOME_QUOTA))
}

/// The one event the task's current state calls for, if any (precedence:
/// quota, stop, gate, QA passed, run failed). A quota stop is reported as such, also when
/// the autopilot stopped for it (`het_quota`), and never as a failed run.
pub fn current_event(s: &TaskState) -> Option<Event> {
    let task_id = s.task.id;
    let ev = |kind, key: String| Some(Event { task_id, kind, key, is_update: false });

    if let Some(run) = quota_run(s) {
        return ev(EventKind::Quota, format!("quota:{}", run.id));
    }

    if s.view.auto.state == "stopped" {
        if let Some(stop) = &s.aj.stop {
            return ev(EventKind::Stop, format!("stop:{}", stop.at_ms));
        }
    }

    // The plan is settled only when the run that wrote or reviewed it has
    // finished, and (autopilot) once the driver has released the task: a live
    // driver is about to write `gate` or `stopped`, and the message must carry
    // the state the user will act on.
    if s.gate == Some(GateState::Waiting) && !s.driver_alive && !settling_run(s) {
        if let Some(pc) = &s.git.plan_commit {
            return ev(EventKind::Gate, format!("gate:{pc}"));
        }
    }

    if s.task.status == "qa_pass" {
        let qa = s
            .runs
            .iter()
            .filter(|r| r.role == "qa")
            .max_by_key(|r| (r.started_at, r.id))
            .map(|r| r.id)
            .unwrap_or(0);
        return ev(EventKind::QaPass, format!("qa:{qa}"));
    }

    if s.task.status == "answered" {
        let last_id = s
            .runs
            .iter()
            .filter(|r| r.role == "analyst" || r.role == "analysis-review")
            .max_by_key(|r| (r.started_at, r.id))
            .map(|r| r.id)
            .unwrap_or(0);
        return ev(EventKind::Answered, format!("answer:{last_id}"));
    }

    let latest = s.runs.iter().max_by_key(|r| (r.started_at, r.id));
    if let Some(r) = latest {
        let auto_busy = matches!(
            s.view.auto.state.as_str(),
            "running" | "stopping" | "stopped"
        );
        if (r.status == "failed" || r.status == "timed_out") && !s.driver_alive && !auto_busy {
            return ev(EventKind::RunFailed, format!("fail:{}", r.id));
        }
    }
    None
}

/// One `(task id, signature)` row per task. The signature changes whenever a
/// fact an event depends on changes in brain.db; run events are not part of it.
pub fn signatures(conn: &Connection) -> BrainResult<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT t.id,
                t.status
                || '|' || COALESCE(t.auto_state, '')
                || '|' || COALESCE(t.auto_stop_at, '')
                || '|' || COALESCE(t.auto_pid, '')
                || '|' || COALESCE(t.approved_commit, '')
                || '|' || COALESCE(t.auto_json, '')
                || '|' || t.updated_at
                || '|' || COALESCE((SELECT MAX(r.id) FROM runs r WHERE r.task_id = t.id), '')
                || '|' || COALESCE((SELECT r.status FROM runs r WHERE r.task_id = t.id
                                    ORDER BY r.started_at DESC, r.id DESC LIMIT 1), '')
         FROM tasks t ORDER BY t.id",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn status_of(sig: &str) -> &str {
    sig.split('|').next().unwrap_or("")
}

#[derive(Default)]
pub struct Detector {
    sigs: HashMap<i64, String>,
    seen: HashSet<(i64, String)>,
    reported: HashSet<i64>,
    /// Tasks whose state could not be read while seeding. Their first successful
    /// read is a pre-restart state: its event is marked seen, not notified.
    unseeded: HashSet<i64>,
}

enum Eval {
    State(Box<TaskState>),
    Gone,
    Failed,
}

/// Edits for the tracked message of `s`'s task, if any (D-15, D-11/D-12).
///
/// - the event `current` is gone or another one: the entry is taken and the
///   message gets `{state} — xử lý ngoài Telegram (dashboard hoặc terminal) lúc
///   HH:MM` without buttons;
/// - the same event with other buttons (a run started or ended, a new rev): only
///   the buttons are replaced, and the entry remembers them.
///
/// The registry lock is taken only here, after the task state was loaded.
pub fn follow_tracked(
    registry: &Mutex<Registry>,
    s: &TaskState,
    current: Option<&Event>,
    now_ms: i64,
) -> Option<Outgoing> {
    let mut reg = registry.lock().unwrap_or_else(|e| e.into_inner());
    let tracked = reg.get(s.task.id)?;
    match current {
        Some(ev) if ev.key == tracked.key => {
            let keyboard = render::keyboard_json(&render::buttons_for(ev, s));
            if keyboard == tracked.keyboard {
                return None;
            }
            let message_id = tracked.message_id;
            let t = reg.take(s.task.id)?;
            reg.insert(Tracked {
                keyboard: keyboard.clone(),
                ..t
            });
            Some(Outgoing::Keyboard {
                message_id,
                keyboard,
            })
        }
        _ => {
            let t = reg.take(s.task.id)?;
            let suffix = render::result_suffix(&render::state_line(s), false, now_ms);
            Some(Outgoing::Edit {
                message_id: t.message_id,
                text: render::with_suffix(&t.text, &suffix),
                keyboard: render::no_keyboard(),
            })
        }
    }
}

impl Detector {
    pub fn new() -> Detector {
        Detector::default()
    }

    fn eval(&mut self, st: &AppState, conn: &Connection, task_id: i64) -> Eval {
        match load_task_state(st, conn, task_id) {
            Ok(Ok(s)) => Eval::State(Box::new(s)),
            Ok(Err(_)) => Eval::Gone,
            Err(e) => {
                if self.reported.insert(task_id) {
                    crate::term::emit(&format!("Telegram: lỗi đọc task {task_id}: {}", e.message));
                }
                Eval::Failed
            }
        }
    }

    /// Records every task's signature and current event as already seen,
    /// without sending anything (restart must not resend, TELE-04).
    pub fn seed(&mut self, st: &AppState, conn: &Connection) -> BrainResult<()> {
        for (task_id, sig) in signatures(conn)? {
            match self.eval(st, conn, task_id) {
                Eval::State(s) => {
                    if let Some(ev) = current_event(&s) {
                        self.seen.insert((ev.task_id, ev.key));
                    }
                    self.sigs.insert(task_id, sig);
                }
                Eval::Gone => {
                    self.sigs.insert(task_id, sig);
                }
                // Not seen yet: without this, the first scan that reads the task would
                // notify an event that existed before the restart (TELE-04).
                Eval::Failed => {
                    self.unseeded.insert(task_id);
                }
            }
        }
        Ok(())
    }

    /// Evaluates tasks whose signature changed (or, when `full`, every task not
    /// yet `qa_pass`) and returns, in order, the edits of their tracked
    /// messages (`follow_tracked`) and a `Notify` for each event not seen
    /// before. A task's moved-on edit always comes before its new notification.
    pub fn scan(
        &mut self,
        st: &AppState,
        conn: &Connection,
        full: bool,
        registry: &Mutex<Registry>,
    ) -> BrainResult<Vec<Outgoing>> {
        let rows = signatures(conn)?;
        let present: HashSet<i64> = rows.iter().map(|(id, _)| *id).collect();
        self.sigs.retain(|id, _| present.contains(id));

        let mut out = Vec::new();
        for (task_id, sig) in rows {
            let changed = self.sigs.get(&task_id) != Some(&sig);
            if !changed && !(full && status_of(&sig) != "qa_pass" && status_of(&sig) != "answered") {
                continue;
            }
            match self.eval(st, conn, task_id) {
                Eval::State(s) => {
                    self.sigs.insert(task_id, sig);
                    let ev = current_event(&s);
                    if let Some(item) =
                        follow_tracked(registry, &s, ev.as_ref(), crate::db::now_ms())
                    {
                        out.push(item);
                    }
                    if let Some(mut ev) = ev {
                        if ev.kind == EventKind::Gate
                            && self
                                .seen
                                .iter()
                                .any(|(tid, k)| *tid == ev.task_id && k.starts_with("gate:") && k != &ev.key)
                        {
                            ev.is_update = true;
                        }
                        if self.seen.insert((ev.task_id, ev.key.clone()))
                            && !self.unseeded.contains(&task_id)
                        {
                            out.push(Outgoing::Notify(ev));
                        }
                    }
                    self.unseeded.remove(&task_id);
                }
                Eval::Gone => {
                    self.sigs.insert(task_id, sig);
                    self.unseeded.remove(&task_id);
                }
                // Signature not recorded: the task is evaluated again next scan.
                Eval::Failed => {}
            }
        }
        Ok(out)
    }
}

/// Longest Telegram text of a failed-intake message (the Bot API limit is 4096).
const INTAKE_TEXT_MAX: usize = 3500;

/// `Hàng chờ #<id> lỗi: <lý do>` for a queue intake that failed.
pub fn intake_failed_text(id: i64, error: Option<&str>) -> String {
    let reason = error
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .unwrap_or("không rõ lý do");
    let text = format!("Hàng chờ #{id} lỗi: {reason}");
    if text.chars().count() <= INTAKE_TEXT_MAX {
        text
    } else {
        let mut out: String = text.chars().take(INTAKE_TEXT_MAX - 1).collect();
        out.push('…');
        out
    }
}

/// Queue intakes from Telegram that went `failed` (plan triage-engine, goal 4): one message per
/// failure, with the reason. A failure is `(id, updated_at)`, so a retried intake that fails
/// again is reported again. The set is seeded at start-up so a serve restart never resends.
#[derive(Debug, Default)]
pub struct IntakeFailures {
    seen: HashSet<(i64, i64)>,
}

fn failed_telegram_intakes(conn: &Connection) -> BrainResult<Vec<(i64, Option<String>, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT id, error, updated_at FROM intake WHERE source = 'telegram' AND status = 'failed' ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

impl IntakeFailures {
    pub fn seed(conn: &Connection) -> BrainResult<IntakeFailures> {
        let seen = failed_telegram_intakes(conn)?
            .into_iter()
            .map(|(id, _, at)| (id, at))
            .collect();
        Ok(IntakeFailures { seen })
    }

    /// Messages for failures not reported yet; forgets intakes that are no longer failed.
    pub fn scan(&mut self, conn: &Connection) -> BrainResult<Vec<Outgoing>> {
        let rows = failed_telegram_intakes(conn)?;
        let mut out = Vec::new();
        let mut now_seen = HashSet::new();
        for (id, error, at) in rows {
            if !self.seen.contains(&(id, at)) {
                out.push(Outgoing::Message {
                    text: intake_failed_text(id, error.as_deref()),
                });
            }
            now_seen.insert((id, at));
        }
        self.seen = now_seen;
        Ok(out)
    }
}

/// `Môi trường đã sẵn sàng · task N slug — autopilot chạy lại lúc HH:MM` (C2).
pub fn env_rerun_text(task_id: i64, slug: &str, project: &str, at_ms: i64) -> String {
    format!(
        "Môi trường đã sẵn sàng · task {task_id} {slug} · dự án {project} — autopilot chạy lại lúc {}",
        crate::autopilot::view::fmt_hhmm(at_ms)
    )
}

/// C2: autopilot reruns the scheduler started after an environment stop
/// (`auto_json.env_rerun_at`): one message per rerun. Seeded at start-up so a serve restart
/// never resends.
#[derive(Debug, Default)]
pub struct EnvReruns {
    seen: HashSet<(i64, i64)>,
}

fn env_rerun_rows(conn: &Connection) -> BrainResult<Vec<(i64, String, String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.slug, p.slug, json_extract(t.auto_json, '$.env_rerun_at')
         FROM tasks t JOIN projects p ON p.id = t.project_id
         WHERE json_extract(t.auto_json, '$.env_rerun_at') IS NOT NULL ORDER BY t.id",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

impl EnvReruns {
    pub fn seed(conn: &Connection) -> BrainResult<EnvReruns> {
        let seen = env_rerun_rows(conn)?
            .into_iter()
            .map(|(id, _, _, at)| (id, at))
            .collect();
        Ok(EnvReruns { seen })
    }

    pub fn scan(&mut self, conn: &Connection) -> BrainResult<Vec<Outgoing>> {
        let mut out = Vec::new();
        let mut now_seen = HashSet::new();
        for (id, slug, project, at) in env_rerun_rows(conn)? {
            if !self.seen.contains(&(id, at)) {
                out.push(Outgoing::Message {
                    text: env_rerun_text(id, &slug, &project, at),
                });
            }
            now_seen.insert((id, at));
        }
        self.seen = now_seen;
        Ok(out)
    }
}

/// `Đề xuất #M «tiêu đề» đang chờ task #N được merge vào <base>.` (B3).
pub fn chain_wait_text(proposal_id: i64, title: &str, error: &str) -> String {
    let wait = error
        .strip_prefix(crate::orchestrator::scheduler::CHAIN_WAIT_PREFIX)
        .map(|rest| format!("chờ task #{rest}"))
        .unwrap_or_else(|| error.to_string());
    let title: String = title.chars().take(120).collect();
    format!("Đề xuất #{proposal_id} «{title}» đang {wait}.")
}

/// B3: approved proposals the scheduler holds until the earlier code task is merged
/// (`proposals.error` = "Chờ task #N được merge vào …"): one message per `(proposal, error)`, so
/// the 5-second scheduler tick never repeats it and a changed wait is reported again. Seeded at
/// start-up so a serve restart never resends.
#[derive(Debug, Default)]
pub struct ChainWaits {
    seen: HashSet<(i64, String)>,
}

fn chain_wait_rows(conn: &Connection) -> BrainResult<Vec<(i64, String, String)>> {
    let pattern = format!(
        "{}% được merge vào %",
        crate::orchestrator::scheduler::CHAIN_WAIT_PREFIX
    );
    let mut stmt = conn.prepare(
        "SELECT id, title, error FROM proposals
         WHERE status = 'approved' AND error LIKE ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([pattern], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

impl ChainWaits {
    pub fn seed(conn: &Connection) -> BrainResult<ChainWaits> {
        let seen = chain_wait_rows(conn)?
            .into_iter()
            .map(|(id, _, error)| (id, error))
            .collect();
        Ok(ChainWaits { seen })
    }

    pub fn scan(&mut self, conn: &Connection) -> BrainResult<Vec<Outgoing>> {
        let mut out = Vec::new();
        let mut now_seen = HashSet::new();
        for (id, title, error) in chain_wait_rows(conn)? {
            let key = (id, error);
            if !self.seen.contains(&key) {
                out.push(Outgoing::Message {
                    text: chain_wait_text(id, &title, &key.1),
                });
            }
            now_seen.insert(key);
        }
        self.seen = now_seen;
        Ok(out)
    }
}

/// Journal mailbox (Phase 7, JRN-05, D-20): delivers every unsent `journal_mail` row to the paired
/// chat at most once. A row is claimed (`sent_at`) before it goes to the outbox, so a restart or a
/// second scan never sends it again; a crash between claim and send loses the message (A11). Rows are
/// claimed one at a time and a database error on a later row returns the rows already claimed (the
/// failing row stays unclaimed for the next scan, WR-09). A
/// `digest` of a day other than today's local day is claimed and dropped, never sent late.
#[derive(Debug, Default)]
pub struct JournalMail;

/// The local day of `now_ms` in the journal's time zone.
fn journal_today(cfg: &crate::journal::config::JournalConfig, now_ms: i64) -> chrono::NaiveDate {
    use crate::journal::collect::local_day;
    use crate::journal::config::JournalTz;
    match cfg.tz() {
        JournalTz::Local => local_day(now_ms, &chrono::Local),
        JournalTz::Fixed(off) => local_day(now_ms, &off),
    }
}

impl JournalMail {
    pub fn scan(
        &mut self,
        conn: &Connection,
        cfg: &crate::journal::config::JournalConfig,
        now_ms: i64,
    ) -> BrainResult<Vec<Outgoing>> {
        let rows: Vec<(i64, String, String, String)> = {
            let mut stmt = conn.prepare(
                "SELECT id, kind, dedupe_key, text FROM journal_mail
                 WHERE sent_at IS NULL ORDER BY id LIMIT 20",
            )?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        let today = journal_today(cfg, now_ms);
        let mut out = Vec::new();
        for (id, kind, key, text) in rows {
            let claimed = match conn.execute(
                "UPDATE journal_mail SET sent_at = ?1 WHERE id = ?2 AND sent_at IS NULL",
                rusqlite::params![now_ms, id],
            ) {
                Ok(n) => n == 1,
                // WR-09: a database error on a later row (e.g. SQLITE_BUSY while the exporter
                // writes) must not discard the rows already claimed: return them now. This row was
                // not claimed (the UPDATE failed), so the next scan retries it and the rest.
                Err(e) if !out.is_empty() => {
                    crate::term::emit(&format!(
                        "Nhật ký: lỗi đọc hộp thư ({e}) — gửi các thư đã nhận, phần còn lại quét lại sau"
                    ));
                    break;
                }
                Err(e) => return Err(e.into()),
            };
            if !claimed {
                continue;
            }
            if kind == "digest" {
                let day = key
                    .strip_prefix("digest:")
                    .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
                if let Some(day) = day.filter(|d| *d != today) {
                    crate::term::emit(&format!(
                        "Nhật ký: bỏ digest ngày {} — quá 24:00, không gửi muộn",
                        day.format("%d/%m/%Y")
                    ));
                    continue;
                }
            }
            out.push(Outgoing::Message {
                text: render::clip(&text, render::MAX_TEXT),
            });
        }
        Ok(out)
    }
}

fn open_db(ctx: &BotCtx) -> Option<Connection> {
    let mut reported = false;
    loop {
        if ctx.stop.load(Ordering::SeqCst) {
            return None;
        }
        match crate::db::open(&ctx.st.db_path) {
            Ok(c) => return Some(c),
            Err(e) => {
                if !reported {
                    crate::term::emit(&format!("Telegram: lỗi mở CSDL: {}", e.message));
                    reported = true;
                }
                if sleep_or_stop(&ctx.stop, 1000) {
                    return None;
                }
            }
        }
    }
}

fn run(ctx: Arc<BotCtx>) {
    // Subscribe before seeding: a change between the seed and the first scan
    // still marks the detector dirty.
    let mut rx = ctx.st.hints.subscribe();
    let conn = match open_db(&ctx) {
        Some(c) => c,
        None => return,
    };

    let mut det = Detector::new();
    let mut last_err: Option<String> = None;
    let mut fails;
    let mut reruns;
    let mut waits;
    loop {
        match det
            .seed(&ctx.st, &conn)
            .and_then(|()| IntakeFailures::seed(&conn))
            .and_then(|f| EnvReruns::seed(&conn).map(|r| (f, r)))
            .and_then(|(f, r)| ChainWaits::seed(&conn).map(|w| (f, r, w)))
        {
            Ok((f, r, w)) => {
                fails = f;
                reruns = r;
                waits = w;
                break;
            }
            Err(e) => {
                if last_err.as_deref() != Some(e.message.as_str()) {
                    crate::term::emit(&format!("Telegram: lỗi đọc CSDL: {}", e.message));
                    last_err = Some(e.message.clone());
                }
                if sleep_or_stop(&ctx.stop, 1000) {
                    return;
                }
            }
        }
    }
    last_err = None;

    // Journal mail queued while serve was stopped goes out now, not after the first 60 s rescan.
    let mut journal_mail = JournalMail;
    match journal_mail.scan(&conn, &ctx.st.cfg.journal, crate::journal::now_ms()) {
        Ok(items) => {
            for item in items {
                ctx.outbox.push(item);
            }
        }
        Err(e) => {
            crate::term::emit(&format!("Telegram: lỗi đọc CSDL: {}", e.message));
            last_err = Some(e.message.clone());
        }
    }

    let mut last_full = Instant::now();
    loop {
        if sleep_or_stop(&ctx.stop, TICK_MS) {
            return;
        }
        let mut dirty = false;
        loop {
            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Lagged(_)) => dirty = true,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Closed) => return,
            }
        }
        let full = last_full.elapsed() >= Duration::from_secs(SAFETY_SCAN_SECS);
        if !dirty && !full {
            continue;
        }
        if full {
            last_full = Instant::now();
        }
        // Each scan pushes what it found on its own; a failed intake scan leaves `fails` as it
        // was, so the intake is reported on a later scan.
        let results = [
            det.scan(&ctx.st, &conn, full, &ctx.registry),
            fails.scan(&conn),
            reruns.scan(&conn),
            waits.scan(&conn),
            journal_mail.scan(&conn, &ctx.st.cfg.journal, crate::journal::now_ms()),
        ];
        let mut failed = false;
        for res in results {
            match res {
                Ok(items) => {
                    for item in items {
                        ctx.outbox.push(item);
                    }
                }
                Err(e) => {
                    failed = true;
                    if last_err.as_deref() != Some(e.message.as_str()) {
                        crate::term::emit(&format!("Telegram: lỗi đọc CSDL: {}", e.message));
                        last_err = Some(e.message.clone());
                    }
                }
            }
        }
        if !failed {
            last_err = None;
        }
    }
}

/// Starts the detector thread `ab-tg-detect`.
pub fn spawn(ctx: Arc<BotCtx>) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("ab-tg-detect".to_string())
        .spawn(move || run(ctx))
        .expect("spawn ab-tg-detect thread")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(items: &[Outgoing]) -> Vec<String> {
        items
            .iter()
            .map(|i| match i {
                Outgoing::Message { text } => text.clone(),
                other => panic!("unexpected item {other:?}"),
            })
            .collect()
    }

    #[test]
    fn failed_telegram_intake_is_reported_once_per_failure() {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("brain.db")).unwrap();
        let old = crate::db::insert_intake(&conn, "telegram", None, "cũ", "normal", true).unwrap();
        crate::db::set_intake_status_error(&conn, old, "failed", Some("trước khi serve chạy"))
            .unwrap();
        let mut fails = IntakeFailures::seed(&conn).unwrap();
        assert!(fails.scan(&conn).unwrap().is_empty(), "seeded failures are not resent");

        let tg = crate::db::insert_intake(&conn, "telegram", None, "việc", "normal", true).unwrap();
        let ui = crate::db::insert_intake(&conn, "ui", None, "việc ui", "normal", true).unwrap();
        crate::db::set_intake_status_error(&conn, ui, "failed", Some("x")).unwrap();
        crate::db::set_intake_status_error(&conn, tg, "failed", Some("Triage (claude) lỗi: hết hạn"))
            .unwrap();
        assert_eq!(
            texts(&fails.scan(&conn).unwrap()),
            vec![format!("Hàng chờ #{tg} lỗi: Triage (claude) lỗi: hết hạn")]
        );
        assert!(fails.scan(&conn).unwrap().is_empty());

        // Retried, then failed again: a new message.
        assert!(crate::db::retry_intake_if_failed(&conn, tg).unwrap());
        assert!(fails.scan(&conn).unwrap().is_empty());
        std::thread::sleep(std::time::Duration::from_millis(5));
        crate::db::set_intake_status_error(&conn, tg, "failed", Some("lần 2")).unwrap();
        assert_eq!(
            texts(&fails.scan(&conn).unwrap()),
            vec![format!("Hàng chờ #{tg} lỗi: lần 2")]
        );
    }

    #[test]
    fn env_rerun_is_reported_once_per_rerun() {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("brain.db")).unwrap();
        conn.execute(
            "INSERT INTO projects (slug, path, profile) VALUES ('p1', '/tmp/p1', 'work')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (project_id, slug, title, branch, created_at, updated_at) VALUES (1, 's1', 't', 'task/s1', 0, 0)",
            [],
        )
        .unwrap();
        let mut reruns = EnvReruns::seed(&conn).unwrap();
        assert!(reruns.scan(&conn).unwrap().is_empty());
        crate::db::set_auto_json(&conn, 1, Some(r#"{"env_rerun_at": 1000}"#)).unwrap();
        let got = texts(&reruns.scan(&conn).unwrap());
        assert_eq!(got.len(), 1);
        assert!(got[0].starts_with("Môi trường đã sẵn sàng · task 1 s1 · dự án p1"), "{got:?}");
        assert!(reruns.scan(&conn).unwrap().is_empty());
        crate::db::set_auto_json(&conn, 1, Some(r#"{"env_rerun_at": 2000}"#)).unwrap();
        assert_eq!(reruns.scan(&conn).unwrap().len(), 1);
        // Seeded at start-up: not resent.
        let mut fresh = EnvReruns::seed(&conn).unwrap();
        assert!(fresh.scan(&conn).unwrap().is_empty());
    }

    #[test]
    fn intake_failed_text_without_reason_and_cut() {
        assert_eq!(intake_failed_text(4, None), "Hàng chờ #4 lỗi: không rõ lý do");
        assert_eq!(intake_failed_text(4, Some("  ")), "Hàng chờ #4 lỗi: không rõ lý do");
        let long = intake_failed_text(4, Some(&"y".repeat(5000)));
        assert_eq!(long.chars().count(), INTAKE_TEXT_MAX);
        assert!(long.ends_with('…'));
    }
}
