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

/// Watchdog of `agent-brain doctor` inside serve: every full scan (60 s) it runs
/// `doctor::scan`, marks dead runs `lost` (the only repair) and sends one message with the
/// findings that are new or due again after `realert_minutes`. The state is in memory; a finding
/// that is gone is forgotten, so it is new again when it comes back. Never called on the 500 ms
/// tick, only when `full` and `doctor.enabled`.
#[derive(Debug, Default)]
pub struct DoctorAlerts {
    dedup: crate::doctor::Dedup,
}

impl DoctorAlerts {
    pub fn new() -> DoctorAlerts {
        DoctorAlerts::default()
    }

    /// The serve entry point: does nothing unless this is a full scan (`full`) and
    /// `doctor.enabled`; otherwise measures the disk and runs `scan` now.
    pub fn tick(
        &mut self,
        conn: &Connection,
        cfg: &crate::config::Config,
        full: bool,
    ) -> BrainResult<Vec<Outgoing>> {
        if !full || !cfg.doctor.enabled {
            return Ok(Vec::new());
        }
        let mut opts = crate::doctor::DoctorOpts::from_config(cfg);
        if opts.disk_min_gb > 0 {
            opts.free_gb = crate::doctor::disk_free_gb();
        }
        self.scan(conn, crate::db::now_ms(), &opts)
    }

    /// One message for the findings to report now (at most `MAX_PER_MESSAGE` listed), also
    /// written to the terminal; nothing when there is nothing new.
    pub fn scan(
        &mut self,
        conn: &Connection,
        now_ms: i64,
        opts: &crate::doctor::DoctorOpts,
    ) -> BrainResult<Vec<Outgoing>> {
        let mut findings = crate::doctor::scan(conn, now_ms, opts)?;
        crate::doctor::heal_dead_runs(conn, &mut findings)?;
        let due = self.dedup.select(&findings, now_ms, opts.realert_ms());
        if due.is_empty() {
            return Ok(Vec::new());
        }
        let text = crate::doctor::alert_text(&due);
        crate::term::emit(&text);
        Ok(vec![Outgoing::Message { text }])
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

/// Revision of a proposal: changes when its title, team, workflow, priority, status or
/// `updated_at` change, so a button sent earlier no longer matches once the proposal was edited
/// (on the dashboard) or handled. 16 hex characters, like a task's `rev`.
pub fn proposal_rev(p: &crate::db::ProposalRow) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    p.id.hash(&mut h);
    p.title.hash(&mut h);
    p.team.hash(&mut h);
    p.workflow.hash(&mut h);
    p.priority.hash(&mut h);
    p.status.hash(&mut h);
    p.updated_at.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// `pending` proposals of intakes created from Telegram (the dashboard's own proposals are not
/// sent), with the project slug. Oldest first.
fn pending_telegram_proposals(
    conn: &Connection,
) -> BrainResult<Vec<(crate::db::ProposalRow, String)>> {
    let mut out = Vec::new();
    for (id, slug) in pending_telegram_proposal_ids(conn)? {
        if let Some(p) = crate::db::get_proposal(conn, id)? {
            if p.status == "pending" {
                out.push((p, slug));
            }
        }
    }
    Ok(out)
}

fn pending_telegram_proposal_ids(conn: &Connection) -> BrainResult<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, COALESCE(pr.slug, '') FROM proposals p
         JOIN intake i ON i.id = p.intake_id
         LEFT JOIN projects pr ON pr.id = p.project_id
         WHERE i.source = 'telegram' AND p.status = 'pending' ORDER BY p.id",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// A message with `Duyệt đề xuất` / `Từ chối` for each `pending` proposal of a Telegram-created
/// intake, once. Seeded at start-up (a serve restart never resends, TELE-04); an id that is no
/// longer `pending` is forgotten.
#[derive(Debug, Default)]
pub struct ProposalNotices {
    seen: HashSet<i64>,
}

impl ProposalNotices {
    pub fn seed(conn: &Connection) -> BrainResult<ProposalNotices> {
        let seen = pending_telegram_proposal_ids(conn)?
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        Ok(ProposalNotices { seen })
    }

    pub fn scan(&mut self, conn: &Connection) -> BrainResult<Vec<Outgoing>> {
        let mut out = Vec::new();
        let mut now_seen = HashSet::new();
        for (p, slug) in pending_telegram_proposals(conn)? {
            if !self.seen.contains(&p.id) {
                out.push(Outgoing::Prompt {
                    text: render::proposal_text(&p, &slug),
                    keyboard: render::keyboard_json(&render::proposal_keyboard(&p)),
                });
            }
            now_seen.insert(p.id);
        }
        self.seen = now_seen;
        Ok(out)
    }
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
    let mut proposals;
    loop {
        match det
            .seed(&ctx.st, &conn)
            .and_then(|()| IntakeFailures::seed(&conn))
            .and_then(|f| EnvReruns::seed(&conn).map(|r| (f, r)))
            .and_then(|(f, r)| ChainWaits::seed(&conn).map(|w| (f, r, w)))
            .and_then(|(f, r, w)| ProposalNotices::seed(&conn).map(|n| (f, r, w, n)))
        {
            Ok((f, r, w, n)) => {
                fails = f;
                reruns = r;
                waits = w;
                proposals = n;
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
    let mut doctor_alerts = DoctorAlerts::new();
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
            proposals.scan(&conn),
            journal_mail.scan(&conn, &ctx.st.cfg.journal, crate::journal::now_ms()),
            doctor_alerts.tick(&conn, &ctx.st.cfg, full),
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
    fn telegram_proposals_are_sent_once_and_rev_follows_edits() {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("brain.db")).unwrap();
        conn.execute(
            "INSERT INTO projects (slug, path, profile) VALUES ('p1', '/tmp/p1', 'work')",
            [],
        )
        .unwrap();
        let mk = |source: &str, title: &str| {
            let intake =
                crate::db::insert_intake(&conn, source, Some(1), "việc", "normal", true).unwrap();
            crate::db::insert_proposal(
                &conn,
                &crate::db::NewProposal {
                    intake_id: intake,
                    project_id: 1,
                    title,
                    request: "việc",
                    priority: "normal",
                    team: "dev",
                    workflow: "mac-dinh",
                    reason: Some("vì cần"),
                    level: "vua",
                },
            )
            .unwrap()
        };
        let old = mk("telegram", "cũ");
        let mut notices = ProposalNotices::seed(&conn).unwrap();
        assert!(notices.scan(&conn).unwrap().is_empty(), "seeded proposals are not resent");

        let tg = mk("telegram", "mới");
        let _ui = mk("ui", "từ dashboard");
        let got = notices.scan(&conn).unwrap();
        assert_eq!(got.len(), 1, "{got:?}");
        match &got[0] {
            Outgoing::Prompt { text, keyboard } => {
                assert!(text.starts_with(&format!("Đề xuất #{tg} — dự án p1")), "{text}");
                assert!(text.contains("Tiêu đề: mới"), "{text}");
                let rows = keyboard["inline_keyboard"][0].as_array().unwrap();
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0]["text"], "Duyệt đề xuất");
                assert_eq!(rows[1]["text"], "Từ chối");
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(notices.scan(&conn).unwrap().is_empty());

        // A handled proposal is forgotten; a fresh seed does not resend the open one.
        assert!(crate::db::set_proposal_status_reason(&conn, old, "rejected", None).unwrap());
        assert!(notices.scan(&conn).unwrap().is_empty());
        let mut fresh = ProposalNotices::seed(&conn).unwrap();
        assert!(fresh.scan(&conn).unwrap().is_empty());

        let before = proposal_rev(&crate::db::get_proposal(&conn, tg).unwrap().unwrap());
        assert_eq!(before.len(), 16);
        conn.execute("UPDATE proposals SET title = 'đã sửa' WHERE id = ?1", [tg])
            .unwrap();
        let after = proposal_rev(&crate::db::get_proposal(&conn, tg).unwrap().unwrap());
        assert_ne!(before, after);
    }

    /// Thresholds of the doctor tests: only `run-long` can fire for a live run with a stale start.
    fn doctor_opts(dir: &std::path::Path) -> crate::doctor::DoctorOpts {
        crate::doctor::DoctorOpts {
            result_no_exit_minutes: 3,
            silent_minutes: 1440,
            run_stall_minutes: 0,
            run_max_minutes: 150,
            driver_stale_minutes: 10,
            queue_stuck_minutes: 60,
            disk_min_gb: 0,
            realert_minutes: 60,
            free_gb: None,
            check_serve: false,
            data_dir: dir.to_path_buf(),
        }
    }

    fn doctor_run(conn: &Connection, role: &str, pid_start: i64, age_min: i64) -> i64 {
        let pid = std::process::id() as i64;
        let id = crate::db::insert_run(
            conn,
            &crate::db::NewRun {
                task_id: None,
                project_id: 1,
                role: role.to_string(),
                profile: "work".to_string(),
                model: None,
                pid,
                pgid: pid,
                pid_start,
                base_commit: None,
                run_dir: None,
            },
        )
        .unwrap();
        conn.execute(
            "UPDATE runs SET started_at = ?1 WHERE id = ?2",
            rusqlite::params![crate::db::now_ms() - age_min * 60_000, id],
        )
        .unwrap();
        id
    }

    /// A `running` row whose process is this test process (alive), started `age_min` ago.
    fn live_run(conn: &Connection, age_min: i64) -> i64 {
        let start = crate::procs::proc_id(std::process::id() as i32)
            .expect("own process")
            .start;
        doctor_run(conn, "dev", start as i64, age_min)
    }

    fn doctor_db(dir: &std::path::Path) -> Connection {
        let conn = crate::db::open(&dir.join("brain.db")).unwrap();
        conn.execute(
            "INSERT INTO projects (slug, path, profile) VALUES ('p1', '/tmp/p1', 'work')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn doctor_alerts_send_once_repeat_after_realert_forget_and_cap_at_five() {
        let dir = tempfile::tempdir().unwrap();
        let conn = doctor_db(dir.path());
        let opts = doctor_opts(dir.path());
        let now = crate::db::now_ms();
        let mut alerts = DoctorAlerts::new();
        assert!(alerts.scan(&conn, now, &opts).unwrap().is_empty(), "nothing to report");

        let first = live_run(&conn, 200);
        let got = texts(&alerts.scan(&conn, now, &opts).unwrap());
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].starts_with("agent-brain doctor: 1 cảnh báo mới"), "{}", got[0]);
        assert!(got[0].contains(&format!("[run-long] Run {first} (dev, dự án p1)")), "{}", got[0]);
        assert!(!got[0].contains('/'), "no path in the message: {}", got[0]);

        // Not again inside realert_minutes, again after it.
        assert!(alerts.scan(&conn, now + 30 * 60_000, &opts).unwrap().is_empty());
        assert!(alerts.scan(&conn, now + 59 * 60_000, &opts).unwrap().is_empty());
        let again = texts(&alerts.scan(&conn, now + 61 * 60_000, &opts).unwrap());
        assert_eq!(again.len(), 1, "{again:?}");

        // Gone: forgotten, so the next finding is new.
        crate::db::mark_status(&conn, first, "succeeded").unwrap();
        assert!(alerts.scan(&conn, now + 62 * 60_000, &opts).unwrap().is_empty());
        let second = live_run(&conn, 200);
        let got = texts(&alerts.scan(&conn, now + 63 * 60_000, &opts).unwrap());
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains(&format!("Run {second} ")), "{}", got[0]);

        // Seven new findings in one scan: one message, five listed, the rest counted.
        crate::db::mark_status(&conn, second, "succeeded").unwrap();
        for _ in 0..7 {
            live_run(&conn, 200);
        }
        let got = texts(&alerts.scan(&conn, now + 64 * 60_000, &opts).unwrap());
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].starts_with("agent-brain doctor: 7 cảnh báo mới"), "{}", got[0]);
        assert_eq!(got[0].lines().filter(|l| l.starts_with("- [run-long]")).count(), 5);
        assert!(got[0].contains("… và 2 cảnh báo nữa"), "{}", got[0]);
        assert!(alerts.scan(&conn, now + 65 * 60_000, &opts).unwrap().is_empty());
    }

    #[test]
    fn doctor_alerts_heal_a_dead_run_and_report_it_once() {
        let dir = tempfile::tempdir().unwrap();
        let conn = doctor_db(dir.path());
        // This process's pid with a start time that does not match: the process "is gone".
        let id = doctor_run(&conn, "qa", 1, 0);
        let opts = doctor_opts(dir.path());
        let mut alerts = DoctorAlerts::new();
        let now = crate::db::now_ms();
        let got = texts(&alerts.scan(&conn, now, &opts).unwrap());
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains(&format!("[run-dead] Run {id} (qa, dự án p1)")), "{}", got[0]);
        assert!(got[0].contains("Đã đánh dấu lost"), "{}", got[0]);
        let status: String = conn
            .query_row("SELECT status FROM runs WHERE id = ?1", [id], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "lost");
        assert!(alerts.scan(&conn, now + 1000, &opts).unwrap().is_empty());
    }

    #[test]
    fn doctor_alerts_run_only_on_a_full_scan_and_when_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let conn = doctor_db(dir.path());
        live_run(&conn, 100_000);
        let mut v: serde_json::Value =
            serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap();
        v["doctor"] = serde_json::json!({"disk_min_gb": 0});
        let cfg = crate::config::config_from_value(&v).unwrap();
        let mut alerts = DoctorAlerts::new();
        assert!(alerts.tick(&conn, &cfg, false).unwrap().is_empty(), "not on the 500 ms tick");
        assert_eq!(alerts.tick(&conn, &cfg, true).unwrap().len(), 1);

        v["doctor"] = serde_json::json!({"disk_min_gb": 0, "enabled": false});
        let off = crate::config::config_from_value(&v).unwrap();
        let mut alerts = DoctorAlerts::new();
        assert!(alerts.tick(&conn, &off, true).unwrap().is_empty(), "enabled = false turns it off");
        let status: String = conn
            .query_row("SELECT status FROM runs WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "running", "a disabled watchdog changes nothing");
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
