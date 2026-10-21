//! Telegram updates: long poll, chat allowlist and button actions (Phase 05.3,
//! D-01/D-02/D-06/D-10/D-12/D-15).
//!
//! Outbound only: `getUpdates` long polling through the curl transport. Every
//! update is checked against the paired chat ID before anything else is read
//! (`authz`); strangers get no reply and no callback answer. Buttons run the
//! dashboard's own `run_control`, which re-reads the task and checks the `rev`
//! the message was rendered with, so a stale button never acts. No update can
//! create a task or start new work (task intake is Phase 05.4).

use super::callback::{self, Callback, Op};
use super::outbox::{Outgoing, Tracked};
use super::render;
use super::transport::{backoff_ms, base_backoff_ms, Body, CallError, LONG_POLL_SECS};
use super::{sleep_or_stop, BotCtx, Channel};
use crate::dashboard::api_tasks::{load_task_state, run_control, ControlOutcome};
use crate::error::BrainResult;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// One terminal line per unknown chat / sender ID per this many seconds (D-10).
pub const STRANGER_LOG_SECS: u64 = 600;
/// Most unknown IDs remembered for the log limit (stranger spam cannot grow memory).
const STRANGER_CAP: usize = 1024;

/// The single reply to any text from the paired chat (D-01).
pub const HELP_TEXT: &str =
    "Bot chỉ nhận nút bấm trên tin thông báo — không giao việc mới qua Telegram.";
pub const INVALID_BUTTON: &str = "Nút không hợp lệ.";
const UNAVAILABLE_TEXT: &str = "Dashboard đang dừng — không nhận việc mới";
const INTERNAL_TEXT: &str = "Lỗi nội bộ — xem terminal đang chạy agent-brain serve";

/// Telegram's limit for an `answerCallbackQuery` text.
const ANSWER_MAX: usize = 200;
const ANSWER_MAX_SECS: u64 = 10;
const REPLY_MAX_SECS: u64 = 20;
const DRAIN_MAX_SECS: u64 = LONG_POLL_SECS + 10;
const POLL_MAX_SECS: u64 = LONG_POLL_SECS + 10;

#[derive(Debug, PartialEq, Eq)]
pub enum Authz {
    Paired,
    Stranger(i64),
    Skip,
}

fn id_at(v: &Value, outer: &str) -> Option<i64> {
    v.get(outer)
        .and_then(|o| o.get("id"))
        .and_then(Value::as_i64)
}

/// Who sent `update` (D-10). Checked before anything else of the update is read.
///
/// - `message`: Paired only when `chat.id` and `from.id` both equal `paired`;
/// - `callback_query`: Paired only when `message.chat.id` and `from.id` both equal `paired`;
/// - a message without chat or sender, a callback without its message, its
///   chat, its sender or its data, and every other update type: Skip;
/// - otherwise Stranger(the first differing ID, chat ID first).
pub fn authz(update: &Value, paired: i64) -> Authz {
    if let Some(msg) = update.get("message") {
        let chat = match id_at(msg, "chat") {
            Some(c) => c,
            None => return Authz::Skip,
        };
        if chat != paired {
            return Authz::Stranger(chat);
        }
        let from = match id_at(msg, "from") {
            Some(f) => f,
            None => return Authz::Skip,
        };
        if from != paired {
            return Authz::Stranger(from);
        }
        return Authz::Paired;
    }
    if let Some(cq) = update.get("callback_query") {
        let msg = match cq.get("message") {
            Some(m) if m.is_object() => m,
            _ => return Authz::Skip,
        };
        if !cq.get("data").is_some_and(Value::is_string) {
            return Authz::Skip;
        }
        let chat = match id_at(msg, "chat") {
            Some(c) => c,
            None => return Authz::Skip,
        };
        if chat != paired {
            return Authz::Stranger(chat);
        }
        let from = match id_at(cq, "from") {
            Some(f) => f,
            None => return Authz::Skip,
        };
        if from != paired {
            return Authz::Stranger(from);
        }
        return Authz::Paired;
    }
    Authz::Skip
}

/// State of the poll loop (plan 05.3-06 adds the pending note).
#[derive(Debug, Default)]
pub struct PollState {
    /// Next `getUpdates` offset: the last handled update_id + 1.
    pub offset: Option<i64>,
    /// When each unknown ID was last logged.
    pub strangers: HashMap<i64, Instant>,
}

impl PollState {
    pub fn new() -> PollState {
        PollState::default()
    }
}

/// True when the stranger line for `id` should be printed now (at most once per
/// `STRANGER_LOG_SECS` per ID; entries older than that are forgotten).
fn stranger_should_log(strangers: &mut HashMap<i64, Instant>, id: i64, now: Instant) -> bool {
    let window = Duration::from_secs(STRANGER_LOG_SECS);
    strangers.retain(|_, at| now.saturating_duration_since(*at) < window);
    if strangers.contains_key(&id) || strangers.len() >= STRANGER_CAP {
        return false;
    }
    strangers.insert(id, now);
    true
}

/// The updates of a `getUpdates` answer with their ids, ascending by update_id.
/// Entries without a numeric update_id are left out.
fn sorted_updates(result: &Value) -> Vec<(i64, Value)> {
    let mut out: Vec<(i64, Value)> = result
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|u| {
                    u.get("update_id")
                        .and_then(Value::as_i64)
                        .map(|id| (id, u.clone()))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|(id, _)| *id);
    out
}

/// D-06: forgets every update that arrived while serve was not running.
/// `offset -1` returns only the newest update; the next `getUpdates` with
/// `Some(newest + 1)` confirms it. `None` when there was nothing to drop.
pub fn drain_offset(ctx: &BotCtx) -> Result<Option<i64>, CallError> {
    let v = ctx.tx.call(
        "getUpdates",
        &Body::Json(json!({"offset": -1, "timeout": 0})),
        DRAIN_MAX_SECS,
    )?;
    Ok(sorted_updates(&v)
        .last()
        .map(|(id, _)| id.saturating_add(1)))
}

enum Flow {
    Retry,
    Exit,
}

struct ErrState {
    failures: u32,
    conflict_reported: bool,
    last_error: Option<String>,
}

/// Handles a failed `getUpdates` (drain or long poll): backs off, never a hot loop.
/// Health is reported only on `Channel::Poll` (the outbox owns `Channel::Send`).
fn on_poll_error(ctx: &BotCtx, e: &CallError, es: &mut ErrState, base_ms: u64) -> Flow {
    if let CallError::Stopped = e {
        return Flow::Exit;
    }
    if e.is_unauthorized() {
        crate::term::emit(
            "Telegram: lỗi — Telegram từ chối token (401) — chạy lại agent-brain telegram setup",
        );
        ctx.stop.store(true, Ordering::SeqCst);
        return Flow::Exit;
    }
    es.failures = es.failures.saturating_add(1);
    let wait = if matches!(e, CallError::Api { code: 409, .. }) {
        if !es.conflict_reported {
            es.conflict_reported = true;
            crate::term::emit(
                "Telegram: bot đang được đọc ở nơi khác (409) — tắt webhook hoặc tiến trình agent-brain khác rồi chạy lại serve",
            );
        }
        backoff_ms(es.failures, base_ms)
    } else {
        match e.retry_delay_ms(es.failures, base_ms) {
            Some(ms) => {
                ctx.health.failed(Channel::Poll, &e.text());
                ms
            }
            None => {
                let text = e.text();
                if es.last_error.as_deref() != Some(text.as_str()) {
                    crate::term::emit(&format!("Telegram: lỗi — {text}"));
                    es.last_error = Some(text);
                }
                backoff_ms(es.failures, base_ms)
            }
        }
    };
    if sleep_or_stop(&ctx.stop, wait) {
        Flow::Exit
    } else {
        Flow::Retry
    }
}

/// The `ab-tg` loop after getMe: drain once (D-06), then long-poll until stop.
pub fn run_poll(ctx: &Arc<BotCtx>) {
    let base_ms = base_backoff_ms();
    let mut state = PollState::new();
    let mut es = ErrState {
        failures: 0,
        conflict_reported: false,
        last_error: None,
    };

    loop {
        if ctx.stop.load(Ordering::SeqCst) {
            return;
        }
        match drain_offset(ctx) {
            Ok(off) => {
                ctx.health.ok(Channel::Poll);
                state.offset = off;
                break;
            }
            Err(e) => {
                if let Flow::Exit = on_poll_error(ctx, &e, &mut es, base_ms) {
                    return;
                }
            }
        }
    }
    es.failures = 0;
    es.last_error = None;

    loop {
        if ctx.stop.load(Ordering::SeqCst) {
            return;
        }
        let mut body = json!({
            "timeout": LONG_POLL_SECS,
            "allowed_updates": ["message", "callback_query"],
        });
        if let Some(off) = state.offset {
            body["offset"] = json!(off);
        }
        match ctx.tx.call("getUpdates", &Body::Json(body), POLL_MAX_SECS) {
            Ok(v) => {
                es.failures = 0;
                es.last_error = None;
                ctx.health.ok(Channel::Poll);
                for (id, update) in sorted_updates(&v) {
                    if ctx.stop.load(Ordering::SeqCst) {
                        return;
                    }
                    // Never handle an update twice.
                    if state.offset.is_some_and(|off| id < off) {
                        continue;
                    }
                    // Advance first: an update that fails is never retried.
                    state.offset = Some(id.saturating_add(1));
                    handle_update(ctx, &mut state, &update);
                }
            }
            Err(e) => {
                if let Flow::Exit = on_poll_error(ctx, &e, &mut es, base_ms) {
                    return;
                }
            }
        }
    }
}

/// The pressed message of a callback query.
#[derive(Debug, Clone)]
pub(crate) struct Press {
    pub id: String,
    pub message_id: i64,
    pub text: String,
}

/// Handles one update. `authz` runs first, before any other field is read.
pub fn handle_update(ctx: &Arc<BotCtx>, state: &mut PollState, update: &Value) {
    match authz(update, ctx.cfg.chat_id) {
        Authz::Skip => return,
        Authz::Stranger(id) => {
            if stranger_should_log(&mut state.strangers, id, Instant::now()) {
                crate::term::emit(&format!("Telegram: bỏ qua tin từ chat lạ {id}"));
            }
            return;
        }
        Authz::Paired => {}
    }

    // Same precedence as `authz`: a message first, then a callback query.
    if let Some(msg) = update.get("message") {
        if let Some(text) = msg.get("text").and_then(Value::as_str) {
            on_text(ctx, state, text);
        }
        return;
    }
    if let Some(cq) = update.get("callback_query") {
        on_callback(ctx, state, cq);
    }
}

/// A text from the paired chat. Nothing creates a task (D-01); plan 05.3-06
/// adds the note prompt of `Trả về Lead` here.
fn on_text(ctx: &Arc<BotCtx>, _state: &mut PollState, _text: &str) {
    reply(ctx, HELP_TEXT);
}

fn on_callback(ctx: &Arc<BotCtx>, state: &mut PollState, cq: &Value) {
    let id = match cq.get("id").and_then(Value::as_str) {
        Some(i) if !i.is_empty() => i.to_string(),
        _ => return,
    };
    let msg = cq.get("message");
    let message_id = msg
        .and_then(|m| m.get("message_id"))
        .and_then(Value::as_i64);
    let data = cq.get("data").and_then(Value::as_str).unwrap_or("");
    let (message_id, cb) = match (message_id, callback::decode(data)) {
        (Some(m), Some(cb)) => (m, cb),
        _ => {
            answer(ctx, &id, INVALID_BUTTON, false);
            return;
        }
    };
    let press = Press {
        id,
        message_id,
        text: msg
            .and_then(|m| m.get("text"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    };
    match cb.op {
        Op::Approve => act(ctx, &press, &cb, "Duyệt plan", "approve"),
        Op::MoreRound => act(ctx, &press, &cb, "Chạy thêm 1 vòng", "auto_more_round"),
        Op::End => act(ctx, &press, &cb, "Dừng hẳn", "auto_end"),
        Op::ReturnLead | Op::NoNote | Op::CancelNote => on_return_lead(ctx, state, &press, &cb),
        Op::KillAsk | Op::KillConfirm | Op::KillCancel => on_kill(ctx, &press, &cb),
    }
}

/// `Trả về Lead` and its note prompt (plan 05.3-06).
fn on_return_lead(ctx: &Arc<BotCtx>, _state: &mut PollState, press: &Press, _cb: &Callback) {
    answer(ctx, &press.id, INVALID_BUTTON, false);
}

/// `Kill` with its confirmation (plan 05.3-06).
fn on_kill(ctx: &Arc<BotCtx>, press: &Press, _cb: &Callback) {
    answer(ctx, &press.id, INVALID_BUTTON, false);
}

/// Runs a control through the dashboard's own function (D-02), bound to the
/// `rev` the message was rendered with (D-15, 05.2 D-28).
fn act(ctx: &Arc<BotCtx>, press: &Press, cb: &Callback, label: &str, control: &str) {
    // The detector must not edit the pressed message while the bot handles it.
    let removed = ctx
        .registry
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove_by_message(press.message_id);
    let outcome = run_control(&ctx.st, cb.task_id, control, Some(&cb.rev), None);
    apply_outcome(ctx, press, cb, label, control, outcome, removed);
}

/// Puts the pressed message back under the detector's watch when nothing was
/// done, unless a newer message for the task is already tracked.
fn reinsert(ctx: &BotCtx, removed: Option<Tracked>) {
    if let Some(t) = removed {
        let mut reg = ctx.registry.lock().unwrap_or_else(|e| e.into_inner());
        if reg.get(t.task_id).is_none() {
            reg.insert(t);
        }
    }
}

fn audit(label: &str, task_id: i64, result: &str) {
    crate::term::emit_out(&format!("Telegram: {label} task {task_id} — {result}"));
}

/// The pressed message's text with `suffix` appended.
fn with_suffix(original: &str, suffix: &str) -> String {
    format!("{}{suffix}", render::clip(original, render::MAX_TEXT))
}

pub(crate) fn apply_outcome(
    ctx: &Arc<BotCtx>,
    press: &Press,
    cb: &Callback,
    label: &str,
    control: &str,
    outcome: BrainResult<ControlOutcome>,
    removed: Option<Tracked>,
) {
    let task_id = cb.task_id;
    let (short, what) = match outcome {
        Ok(ControlOutcome::Approved {
            plan_commit,
            job_id,
            ..
        }) => {
            let sha7: String = plan_commit.chars().take(7).collect();
            let mut what = format!("Đã duyệt plan {sha7}");
            if job_id.is_some() {
                what.push_str(" · autopilot chạy tiếp");
            }
            ("Đã duyệt plan".to_string(), what)
        }
        Ok(ControlOutcome::Started { job_id }) => {
            let what = if control == "auto_more_round" {
                "Đã giao thêm 1 vòng".to_string()
            } else {
                format!("Đã bắt đầu job {job_id}")
            };
            (what.clone(), what)
        }
        Ok(ControlOutcome::Ended { .. }) => {
            let what =
                "Đã dừng hẳn autopilot. Bật autopilot trên dashboard để chạy tiếp.".to_string();
            (what.clone(), what)
        }
        Ok(ControlOutcome::StopRequested) => {
            let what = "Đã gửi yêu cầu dừng".to_string();
            (what.clone(), what)
        }
        Ok(ControlOutcome::Stale) | Ok(ControlOutcome::NotFound(_)) => {
            // The message stays untracked: it loses its buttons.
            stale(ctx, press, label, task_id);
            return;
        }
        Ok(ControlOutcome::Refused { reason, .. }) => {
            answer(ctx, &press.id, &reason, true);
            reinsert(ctx, removed);
            audit(label, task_id, &reason);
            return;
        }
        Ok(ControlOutcome::Unavailable) => {
            answer(ctx, &press.id, UNAVAILABLE_TEXT, true);
            reinsert(ctx, removed);
            audit(label, task_id, UNAVAILABLE_TEXT);
            return;
        }
        Ok(ControlOutcome::Invalid(msg)) => {
            answer(ctx, &press.id, &msg, true);
            reinsert(ctx, removed);
            audit(label, task_id, &msg);
            return;
        }
        Err(e) => {
            crate::term::emit(&format!(
                "Telegram: lỗi nội bộ khi xử lý nút ({})",
                e.message
            ));
            answer(ctx, &press.id, INTERNAL_TEXT, true);
            reinsert(ctx, removed);
            audit(label, task_id, INTERNAL_TEXT);
            return;
        }
    };

    answer(ctx, &press.id, &short, false);
    let suffix = render::result_suffix(&what, true, crate::db::now_ms());
    edit(
        ctx,
        press.message_id,
        &with_suffix(&press.text, &suffix),
        render::no_keyboard(),
    );
    audit(label, task_id, &what);
}

/// A stale press (D-15): nothing was executed; the user learns the task's
/// current state and the message loses its buttons.
pub(crate) fn stale(ctx: &Arc<BotCtx>, press: &Press, label: &str, task_id: i64) {
    let now_line = match crate::db::open(&ctx.st.db_path) {
        Ok(conn) => match load_task_state(&ctx.st, &conn, task_id) {
            Ok(Ok(s)) => render::state_line(&s),
            Ok(Err(_)) => format!("không còn task {task_id}"),
            Err(e) => {
                crate::term::emit(&format!("Telegram: lỗi đọc task {task_id}: {}", e.message));
                "không đọc được trạng thái task".to_string()
            }
        },
        Err(e) => {
            crate::term::emit(&format!("Telegram: lỗi mở CSDL: {}", e.message));
            "không đọc được trạng thái task".to_string()
        }
    };
    let sentence = format!("Task đã đổi trạng thái: {now_line}");
    answer(ctx, &press.id, &sentence, false);
    edit(
        ctx,
        press.message_id,
        &with_suffix(&press.text, &format!("\n\n{sentence}")),
        render::no_keyboard(),
    );
    audit(label, task_id, &sentence);
}

/// Answers a callback query (stops the button's spinner). Failures — e.g. a
/// 400 "query is too old" — are ignored.
pub(crate) fn answer(ctx: &BotCtx, callback_id: &str, text: &str, alert: bool) {
    let body = json!({
        "callback_query_id": callback_id,
        "text": render::clip(text, ANSWER_MAX),
        "show_alert": alert,
    });
    let _ = ctx
        .tx
        .call("answerCallbackQuery", &Body::Json(body), ANSWER_MAX_SECS);
}

/// Replaces the text and buttons of a sent message in the paired chat. Goes
/// through the outbox, which retries during an outage and treats "message is
/// not modified" as success.
pub(crate) fn edit(ctx: &BotCtx, message_id: i64, text: &str, keyboard: Value) {
    ctx.outbox.push(Outgoing::Edit {
        message_id,
        text: text.to_string(),
        keyboard,
    });
}

/// Sends `text` to the paired chat (one attempt; a failure is dropped).
pub(crate) fn reply(ctx: &BotCtx, text: &str) {
    let body = json!({
        "chat_id": ctx.cfg.chat_id,
        "text": text,
    });
    let _ = ctx
        .tx
        .call("sendMessage", &Body::Json(body), REPLY_MAX_SECS);
}
