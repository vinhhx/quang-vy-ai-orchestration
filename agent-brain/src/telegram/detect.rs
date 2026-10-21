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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub task_id: i64,
    pub kind: EventKind,
    pub key: String,
}

fn settling_run(s: &TaskState) -> bool {
    s.runs
        .iter()
        .any(|r| r.status == "running" && (r.role == "lead" || r.role == "plan-review"))
}

/// The one event the task's current state calls for, if any (precedence:
/// stop, gate, QA passed, run failed).
pub fn current_event(s: &TaskState) -> Option<Event> {
    let task_id = s.task.id;
    let ev = |kind, key: String| Some(Event { task_id, kind, key });

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
                Eval::Failed => {}
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
            if !changed && !(full && status_of(&sig) != "qa_pass") {
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
                    if let Some(ev) = ev {
                        if self.seen.insert((ev.task_id, ev.key.clone())) {
                            out.push(Outgoing::Notify(ev));
                        }
                    }
                }
                Eval::Gone => {
                    self.sigs.insert(task_id, sig);
                }
                // Signature not recorded: the task is evaluated again next scan.
                Eval::Failed => {}
            }
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
    loop {
        match det.seed(&ctx.st, &conn) {
            Ok(()) => break,
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
        match det.scan(&ctx.st, &conn, full, &ctx.registry) {
            Ok(items) => {
                last_err = None;
                for item in items {
                    ctx.outbox.push(item);
                }
            }
            Err(e) => {
                if last_err.as_deref() != Some(e.message.as_str()) {
                    crate::term::emit(&format!("Telegram: lỗi đọc CSDL: {}", e.message));
                    last_err = Some(e.message.clone());
                }
            }
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
