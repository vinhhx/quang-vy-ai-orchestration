//! Telegram updates: long poll, chat allowlist and button actions (Phase 05.3,
//! D-01/D-02/D-06/D-10/D-12/D-15).
//!
//! Outbound only: `getUpdates` long polling through the curl transport. Every
//! update is checked against the paired chat ID before anything else is read
//! (`authz`); strangers get no reply and no callback answer. Buttons run the
//! dashboard's own `run_control`, which re-reads the task and checks the `rev`
//! the message was rendered with, so a stale button never acts. No update can
//! create a task or start new work (task intake is Phase 05.4).
//!
//! `Trả về Lead` asks for an optional note (D-13): while one prompt is open and
//! not expired, the paired chat's next text message is the note, passed to the
//! dashboard's own `return_lead` / `relead` path as one argv element.

use super::callback::{self, Callback, Op};
use super::detect;
use super::outbox::{Outgoing, Tracked};
use super::render;
use super::transport::{backoff_ms, base_backoff_ms, Body, CallError, LONG_POLL_SECS};
use super::{sleep_or_stop, BotCtx, Channel};
use crate::dashboard::api_runs::{kill_run, KillOutcome};
use crate::dashboard::api_tasks::{
    load_task_state, normalize_note, run_control_as, run_task_action, ActionSource, ControlOutcome,
    TaskState,
};
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
const NOT_APPLICABLE: &str = "Hành động không áp dụng cho trạng thái hiện tại";

/// How long a `Trả về Lead` note prompt stays open (D-13).
pub const NOTE_PROMPT_SECS: i64 = 600;
const NOTE_ASK_ANSWER: &str = "Gửi ghi chú cho Lead hoặc bấm Không ghi chú";
pub const NOTE_TOO_LONG: &str =
    "Ghi chú quá dài (tối đa 500 ký tự) — gửi lại ngắn hơn hoặc bấm Không ghi chú.";
pub const NOTE_EXPIRED: &str = "Hết thời gian (10 phút) — không làm gì.";
pub const NOTE_CANCELLED: &str = "Đã huỷ — không làm gì.";
pub const NOTE_REPLACED: &str = "Đã huỷ — có yêu cầu ghi chú mới.";
pub const NOTE_SENT: &str = "Đã gửi cho Lead.";
const NOTE_STALE: &str = "Task đã đổi trạng thái — không làm gì.";
const PROMPT_FAILED: &str = "Không gửi được yêu cầu ghi chú — bấm lại sau";

/// Telegram's limit for an `answerCallbackQuery` text.
/// Characters of a note shown in the result line of an edit.
const NOTE_EXCERPT: usize = 80;
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

/// The open `Trả về Lead` note prompt (D-13). At most one is open at a time.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingNote {
    pub task_id: i64,
    /// The rev of the pressed message; the return runs only with this rev.
    pub rev: String,
    /// The message whose `Trả về Lead` was pressed.
    pub origin_message_id: i64,
    pub origin_text: String,
    /// The prompt message (with `Không ghi chú` / `Huỷ`).
    pub prompt_message_id: i64,
    pub expires_at_ms: i64,
}

impl PendingNote {
    pub fn expired(&self, now_ms: i64) -> bool {
        now_ms >= self.expires_at_ms
    }
}

/// State of the poll loop.
#[derive(Debug, Default)]
pub struct PollState {
    /// Next `getUpdates` offset: the last handled update_id + 1.
    pub offset: Option<i64>,
    /// When each unknown ID was last logged.
    pub strangers: HashMap<i64, Instant>,
    /// The open note prompt of `Trả về Lead`, if any.
    pub pending: Option<PendingNote>,
}

impl PollState {
    pub fn new() -> PollState {
        PollState::default()
    }
}

/// Takes the open note prompt off `state` when it has expired at `now_ms`.
pub fn take_expired(state: &mut PollState, now_ms: i64) -> Option<PendingNote> {
    if state.pending.as_ref().is_some_and(|p| p.expired(now_ms)) {
        state.pending.take()
    } else {
        None
    }
}

/// D-13: an unanswered prompt ends after `NOTE_PROMPT_SECS` and runs nothing.
fn expire_prompt(ctx: &BotCtx, state: &mut PollState) {
    if let Some(p) = take_expired(state, crate::db::now_ms()) {
        edit(
            ctx,
            p.prompt_message_id,
            NOTE_EXPIRED,
            render::no_keyboard(),
        );
        audit("Trả về Lead", p.task_id, NOTE_EXPIRED);
    }
}

/// The line added under a message while its `Kill` waits for confirmation (D-14).
pub fn kill_confirm_suffix(run_id: i64) -> String {
    format!("\n\nXác nhận Kill run {run_id}?")
}

/// `text` without the confirmation line of run `run_id`, when it ends with it.
fn without_confirm(text: &str, run_id: i64) -> &str {
    text.strip_suffix(kill_confirm_suffix(run_id).as_str())
        .unwrap_or(text)
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
                expire_prompt(ctx, &mut state);
                for (id, update) in sorted_updates(&v) {
                    if ctx.stop.load(Ordering::SeqCst) {
                        return;
                    }
                    expire_prompt(ctx, &mut state);
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

/// A text from the paired chat. Nothing creates a task (D-01). While a note
/// prompt is open and not expired, the text is the note of `Trả về Lead`
/// (D-13); otherwise the help line is the only reply.
fn on_text(ctx: &Arc<BotCtx>, state: &mut PollState, text: &str) {
    let now = crate::db::now_ms();
    let open = state.pending.as_ref().is_some_and(|p| !p.expired(now));
    if !open {
        reply(ctx, HELP_TEXT);
        return;
    }
    match normalize_note(Some(text)) {
        // The prompt stays open: the user may send a shorter note.
        Err(_) => reply(ctx, NOTE_TOO_LONG),
        Ok(note) => perform_return(ctx, state, note, None),
    }
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
    // Telegram sends only `chat`, `message_id` and `date = 0` (no `text`) for a message
    // the bot can no longer read (an old gate message). The text then comes from the
    // tracked entry; if there is none, `text` stays empty and results go out as a new
    // message instead of overwriting the pressed one (`edit_result`).
    let shown = msg
        .and_then(|m| m.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let text = if shown.trim().is_empty() {
        ctx.registry
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .text_of_message(message_id)
            .unwrap_or_default()
    } else {
        shown.to_string()
    };
    let press = Press {
        id,
        message_id,
        text,
    };
    match cb.op {
        Op::Approve => act(ctx, &press, &cb, "Duyệt plan", "approve"),
        Op::MoreRound => act(ctx, &press, &cb, "Chạy thêm 1 vòng", "auto_more_round"),
        Op::End => act(ctx, &press, &cb, "Dừng hẳn", "auto_end"),
        Op::ReturnLead | Op::NoNote | Op::CancelNote => on_return_lead(ctx, state, &press, &cb),
        Op::KillAsk | Op::KillConfirm | Op::KillCancel => on_kill(ctx, &press, &cb),
    }
}

/// A task's state read on a fresh connection.
enum Loaded {
    State(Box<TaskState>),
    Gone,
    Error,
}

fn load_fresh(ctx: &BotCtx, task_id: i64) -> Loaded {
    let conn = match crate::db::open(&ctx.st.db_path) {
        Ok(c) => c,
        Err(e) => {
            crate::term::emit(&format!("Telegram: lỗi mở CSDL: {}", e.message));
            return Loaded::Error;
        }
    };
    match load_task_state(&ctx.st, &conn, task_id) {
        Ok(Ok(s)) => Loaded::State(Box::new(s)),
        Ok(Err(_)) => Loaded::Gone,
        Err(e) => {
            crate::term::emit(&format!("Telegram: lỗi đọc task {task_id}: {}", e.message));
            Loaded::Error
        }
    }
}

fn has_control(s: &TaskState, id: &str) -> bool {
    s.view.controls.iter().any(|c| c.id == id)
}

fn has_action(s: &TaskState, id: &str) -> bool {
    s.view.actions.iter().any(|a| a.id == id)
}

/// Stops the detector from also editing a message the bot is about to edit.
fn untrack(ctx: &BotCtx, message_id: i64) -> Option<Tracked> {
    ctx.registry
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove_by_message(message_id)
}

/// One inline button row with callback data for `cb`'s task, run and rev.
fn button_row(label: &str, op: Op, cb: &Callback) -> Vec<render::Button> {
    vec![render::Button {
        label: label.to_string(),
        data: callback::encode(&Callback {
            op,
            task_id: cb.task_id,
            run_id: cb.run_id,
            rev: cb.rev.clone(),
        }),
    }]
}

/// The text of the note prompt (D-13).
pub fn note_prompt_text(task_id: i64) -> String {
    format!(
        "Ghi chú cho Lead (task {task_id}): gửi tin nhắn kế tiếp làm ghi chú (tối đa 500 ký tự), hoặc bấm Không ghi chú. Hết hạn sau 10 phút."
    )
}

/// `Trả về Lead` and its note prompt (D-13).
fn on_return_lead(ctx: &Arc<BotCtx>, state: &mut PollState, press: &Press, cb: &Callback) {
    match cb.op {
        Op::ReturnLead => ask_note(ctx, state, press, cb),
        Op::NoNote | Op::CancelNote => {
            // Only the buttons of the open prompt, for its task and rev.
            let open = state.pending.as_ref().is_some_and(|p| {
                p.task_id == cb.task_id
                    && p.rev == cb.rev
                    && p.prompt_message_id == press.message_id
                    && !p.expired(crate::db::now_ms())
            });
            if !open {
                answer(ctx, &press.id, INVALID_BUTTON, false);
                return;
            }
            if cb.op == Op::NoNote {
                perform_return(ctx, state, None, Some(&press.id));
            } else if let Some(p) = state.pending.take() {
                edit(
                    ctx,
                    p.prompt_message_id,
                    NOTE_CANCELLED,
                    render::no_keyboard(),
                );
                answer(ctx, &press.id, "Đã huỷ", false);
                audit("Trả về Lead", p.task_id, NOTE_CANCELLED);
            }
        }
        _ => answer(ctx, &press.id, INVALID_BUTTON, false),
    }
}

/// `Trả về Lead` pressed: checks the task, then opens the note prompt.
fn ask_note(ctx: &Arc<BotCtx>, state: &mut PollState, press: &Press, cb: &Callback) {
    let label = "Trả về Lead";
    let s = match load_fresh(ctx, cb.task_id) {
        Loaded::State(s) => s,
        Loaded::Gone => {
            untrack(ctx, press.message_id);
            stale(ctx, press, label, cb.task_id);
            return;
        }
        Loaded::Error => {
            answer(ctx, &press.id, INTERNAL_TEXT, true);
            return;
        }
    };
    if cb.rev != s.rev {
        untrack(ctx, press.message_id);
        stale(ctx, press, label, cb.task_id);
        return;
    }
    if !has_control(&s, "return_lead") && !has_action(&s, "relead") {
        answer(ctx, &press.id, NOT_APPLICABLE, true);
        return;
    }
    // One prompt at a time: a new press replaces the open one.
    if let Some(old) = state.pending.take() {
        edit(
            ctx,
            old.prompt_message_id,
            NOTE_REPLACED,
            render::no_keyboard(),
        );
    }
    let keyboard = render::keyboard_json(&[
        button_row("Không ghi chú", Op::NoNote, cb),
        button_row("Huỷ", Op::CancelNote, cb),
    ]);
    match send_prompt(
        ctx,
        &note_prompt_text(cb.task_id),
        press.message_id,
        keyboard,
    ) {
        Some(prompt_message_id) => {
            state.pending = Some(PendingNote {
                task_id: cb.task_id,
                rev: cb.rev.clone(),
                origin_message_id: press.message_id,
                origin_text: press.text.clone(),
                prompt_message_id,
                expires_at_ms: crate::db::now_ms() + NOTE_PROMPT_SECS * 1000,
            });
            answer(ctx, &press.id, NOTE_ASK_ANSWER, false);
        }
        None => answer(ctx, &press.id, PROMPT_FAILED, true),
    }
}

/// Tells the user `text`: as the callback answer when there is a press,
/// otherwise as a message in the paired chat.
fn tell(ctx: &BotCtx, callback_id: Option<&str>, text: &str, alert: bool) {
    match callback_id {
        Some(id) => answer(ctx, id, text, alert),
        None => reply(ctx, text),
    }
}

/// Runs `Trả về Lead` for the open prompt with `note` (already normalized),
/// through the dashboard's own path (D-02): control `return_lead` (Bạn gate,
/// autopilot stop) or, where the state has none, the `Chạy lại Lead` action.
/// The prompt is closed whatever the outcome.
fn perform_return(
    ctx: &Arc<BotCtx>,
    state: &mut PollState,
    note: Option<String>,
    callback_id: Option<&str>,
) {
    let p = match state.pending.take() {
        Some(p) => p,
        None => return,
    };
    let label = "Trả về Lead";
    let task_id = p.task_id;
    let removed = untrack(ctx, p.origin_message_id);

    let outcome: BrainResult<ControlOutcome> = match load_fresh(ctx, task_id) {
        Loaded::Gone => Ok(ControlOutcome::NotFound(format!(
            "Không tìm thấy task {task_id}"
        ))),
        Loaded::Error => Err(crate::error::BrainError::new(
            "không đọc được trạng thái task",
            1,
        )),
        Loaded::State(s) if s.rev != p.rev => Ok(ControlOutcome::Stale),
        Loaded::State(s) => {
            if has_control(&s, "return_lead") {
                run_control_as(
                    &ctx.st,
                    task_id,
                    "return_lead",
                    Some(&p.rev),
                    note.as_deref(),
                    ActionSource::Telegram,
                )
            } else if has_action(&s, "relead") {
                run_task_action(
                    &ctx.st,
                    task_id,
                    "relead",
                    "lead",
                    Some(&p.rev),
                    note.as_deref(),
                )
            } else {
                Ok(ControlOutcome::Refused {
                    reason: NOT_APPLICABLE.to_string(),
                    kind: "not_allowed",
                })
            }
        }
    };

    let refused = |reason: &str| {
        tell(ctx, callback_id, reason, true);
        reinsert(ctx, removed.clone());
        edit(ctx, p.prompt_message_id, reason, render::no_keyboard());
        audit(label, task_id, reason);
    };
    match outcome {
        Ok(ControlOutcome::Started { .. }) => {
            let what = match note.as_deref() {
                Some(n) => {
                    // Only a short excerpt: the whole note (up to 500 characters) would
                    // crowd the original text out of the edit.
                    let shown: String = n.chars().take(NOTE_EXCERPT).collect();
                    let dots = if n.chars().count() > NOTE_EXCERPT {
                        "…"
                    } else {
                        ""
                    };
                    format!("Đã trả về Lead (ghi chú: «{shown}{dots}»)")
                }
                None => "Đã trả về Lead (không ghi chú)".to_string(),
            };
            let suffix = render::result_suffix(&what, true, crate::db::now_ms());
            edit_result(
                ctx,
                p.origin_message_id,
                &p.origin_text,
                &suffix,
                render::no_keyboard(),
            );
            edit(ctx, p.prompt_message_id, NOTE_SENT, render::no_keyboard());
            if let Some(id) = callback_id {
                answer(ctx, id, "Đã trả về Lead", false);
            }
            // Never the note itself on the terminal: only its length.
            let result = match note.as_deref() {
                Some(n) => format!("đã giao (ghi chú {} ký tự)", n.chars().count()),
                None => "đã giao (không ghi chú)".to_string(),
            };
            audit(label, task_id, &result);
        }
        Ok(ControlOutcome::Stale) | Ok(ControlOutcome::NotFound(_)) => {
            // The origin message stays untracked: it loses its buttons.
            let sentence = format!("Task đã đổi trạng thái: {}", state_now(ctx, task_id));
            edit_result(
                ctx,
                p.origin_message_id,
                &p.origin_text,
                &format!("\n\n{sentence}"),
                render::no_keyboard(),
            );
            edit(ctx, p.prompt_message_id, NOTE_STALE, render::no_keyboard());
            if let Some(id) = callback_id {
                answer(ctx, id, &sentence, false);
            }
            audit(label, task_id, &sentence);
        }
        Ok(ControlOutcome::Refused { reason, .. }) => refused(&reason),
        Ok(ControlOutcome::Invalid(msg)) => refused(&msg),
        Ok(ControlOutcome::Unavailable) => refused(UNAVAILABLE_TEXT),
        Ok(other) => {
            // Not an outcome of return_lead / relead.
            crate::term::emit(&format!(
                "Telegram: kết quả không mong đợi khi trả về Lead task {task_id}: {other:?}"
            ));
            refused(INTERNAL_TEXT);
        }
        Err(e) => {
            crate::term::emit(&format!(
                "Telegram: lỗi nội bộ khi trả về Lead task {task_id} ({})",
                e.message
            ));
            refused(INTERNAL_TEXT);
        }
    }
}

/// Sends the note prompt as a reply to `reply_to`; the prompt's message id,
/// or `None` when Telegram did not take it (one attempt).
fn send_prompt(ctx: &BotCtx, text: &str, reply_to: i64, keyboard: Value) -> Option<i64> {
    let mut body = json!({
        "chat_id": ctx.cfg.chat_id,
        "text": text,
        "reply_parameters": {
            "message_id": reply_to,
            "allow_sending_without_reply": true,
        },
    });
    if keyboard != render::no_keyboard() {
        body["reply_markup"] = keyboard;
    }
    match ctx
        .tx
        .call("sendMessage", &Body::Json(body), REPLY_MAX_SECS)
    {
        Ok(v) => v.get("message_id").and_then(Value::as_i64),
        Err(_) => None,
    }
}

/// True when `run_id` is a running run of the task (T-05.3-27).
fn running_run_of(s: &TaskState, run_id: i64) -> bool {
    s.runs
        .iter()
        .any(|r| r.id == run_id && r.status == "running")
}

/// `Kill` with its confirmation (D-14). The first press only asks; only the
/// confirming press stops anything, and only through the dashboard's
/// `kill_run` → `runner::cmd_kill` (D-02). No other path here stops a process.
fn on_kill(ctx: &Arc<BotCtx>, press: &Press, cb: &Callback) {
    let label = "Kill";
    let task_id = cb.task_id;
    let run_id = match cb.run_id {
        Some(r) => r,
        None => {
            answer(ctx, &press.id, INVALID_BUTTON, false);
            return;
        }
    };
    let base = without_confirm(&press.text, run_id).to_string();

    if cb.op == Op::KillCancel {
        kill_cancel(ctx, press, task_id, &base);
        return;
    }

    let s = match load_fresh(ctx, task_id) {
        Loaded::State(s) => s,
        Loaded::Gone => {
            untrack(ctx, press.message_id);
            stale(ctx, press, label, task_id);
            return;
        }
        Loaded::Error => {
            answer(ctx, &press.id, INTERNAL_TEXT, true);
            return;
        }
    };
    if cb.rev != s.rev || !running_run_of(&s, run_id) {
        untrack(ctx, press.message_id);
        stale(ctx, press, label, task_id);
        return;
    }

    if cb.op == Op::KillAsk {
        let keyboard = render::keyboard_json(&[
            button_row("Kill", Op::KillConfirm, cb),
            button_row("Huỷ", Op::KillCancel, cb),
        ]);
        edit_result(
            ctx,
            press.message_id,
            &base,
            &kill_confirm_suffix(run_id),
            keyboard,
        );
        answer(
            ctx,
            &press.id,
            &format!("Xác nhận Kill run {run_id}?"),
            false,
        );
        return;
    }

    // Op::KillConfirm: the second press.
    let removed = untrack(ctx, press.message_id);
    match kill_run(&ctx.st, run_id) {
        Ok(KillOutcome::Done { ok, message }) => {
            let suffix = render::result_suffix(&message, true, crate::db::now_ms());
            let text = with_suffix(&base, &suffix);
            // The message's event still current (e.g. a stop): it keeps the
            // remaining buttons with the fresh rev, and stays tracked.
            let keyboard = match (removed, load_fresh(ctx, task_id)) {
                (Some(t), Loaded::State(s)) => match detect::current_event(&s) {
                    Some(ev) if ev.key == t.key => {
                        let kb = render::keyboard_json(&render::buttons_for(&ev, &s));
                        reinsert(
                            ctx,
                            Some(Tracked {
                                text: text.clone(),
                                keyboard: kb.clone(),
                                ..t
                            }),
                        );
                        kb
                    }
                    _ => render::no_keyboard(),
                },
                _ => render::no_keyboard(),
            };
            edit_result(ctx, press.message_id, &base, &suffix, keyboard);
            answer(ctx, &press.id, &message, !ok);
            audit(label, task_id, &message);
        }
        Ok(KillOutcome::Refused(msg)) | Ok(KillOutcome::NotFound(msg)) => {
            answer(ctx, &press.id, &msg, true);
            edit_result(
                ctx,
                press.message_id,
                &base,
                &format!("\n\n{msg}"),
                render::no_keyboard(),
            );
            audit(label, task_id, &msg);
        }
        Err(e) => {
            crate::term::emit(&format!(
                "Telegram: lỗi nội bộ khi Kill run {run_id} ({})",
                e.message
            ));
            answer(ctx, &press.id, INTERNAL_TEXT, true);
            let keyboard = removed
                .as_ref()
                .map(|t| t.keyboard.clone())
                .unwrap_or_else(render::no_keyboard);
            reinsert(ctx, removed);
            edit(ctx, press.message_id, &base, keyboard);
            audit(label, task_id, INTERNAL_TEXT);
        }
    }
}

/// `Huỷ` on a Kill confirmation: nothing is stopped; the message gets back the
/// buttons its event allows now, or says the task has moved on.
fn kill_cancel(ctx: &Arc<BotCtx>, press: &Press, task_id: i64, base: &str) {
    let current = match load_fresh(ctx, task_id) {
        Loaded::State(s) => detect::current_event(&s).map(|ev| {
            let kb = render::keyboard_json(&render::buttons_for(&ev, &s));
            (ev, kb)
        }),
        _ => None,
    };
    match current {
        Some((ev, kb)) => {
            // Still the message's event: it keeps following the task with
            // these buttons. The lock is never held during a Telegram call.
            {
                let mut reg = ctx.registry.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(t) = reg.remove_by_message(press.message_id) {
                    if t.key == ev.key {
                        reg.insert(Tracked {
                            keyboard: kb.clone(),
                            ..t
                        });
                    }
                }
            }
            edit(ctx, press.message_id, base, kb);
        }
        None => {
            untrack(ctx, press.message_id);
            let sentence = format!("Task đã đổi trạng thái: {}", state_now(ctx, task_id));
            edit_result(
                ctx,
                press.message_id,
                base,
                &format!("\n\n{sentence}"),
                render::no_keyboard(),
            );
        }
    }
    answer(ctx, &press.id, "Đã huỷ Kill", false);
    audit("Kill", task_id, "Đã huỷ Kill");
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
    let outcome = run_control_as(
        &ctx.st,
        cb.task_id,
        control,
        Some(&cb.rev),
        None,
        ActionSource::Telegram,
    );
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
    render::with_suffix(original, suffix)
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
    edit_result(
        ctx,
        press.message_id,
        &press.text,
        &suffix,
        render::no_keyboard(),
    );
    audit(label, task_id, &what);
}

/// A stale press (D-15): nothing was executed; the user learns the task's
/// current state and the message loses its buttons.
pub(crate) fn stale(ctx: &Arc<BotCtx>, press: &Press, label: &str, task_id: i64) {
    let sentence = format!("Task đã đổi trạng thái: {}", state_now(ctx, task_id));
    answer(ctx, &press.id, &sentence, false);
    edit_result(
        ctx,
        press.message_id,
        &press.text,
        &format!("\n\n{sentence}"),
        render::no_keyboard(),
    );
    audit(label, task_id, &sentence);
}

/// One short line for where the task is now (read on a fresh connection).
fn state_now(ctx: &BotCtx, task_id: i64) -> String {
    match load_fresh(ctx, task_id) {
        Loaded::State(s) => render::state_line(&s),
        Loaded::Gone => format!("không còn task {task_id}"),
        Loaded::Error => "không đọc được trạng thái task".to_string(),
    }
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
///
/// An empty `text` (the message's text is unknown) never reaches
/// `editMessageText` — Telegram rejects it and the message would be left as is;
/// only the buttons are replaced then.
pub(crate) fn edit(ctx: &BotCtx, message_id: i64, text: &str, keyboard: Value) {
    if text.trim().is_empty() {
        ctx.outbox.push(Outgoing::Keyboard {
            message_id,
            keyboard,
        });
        return;
    }
    ctx.outbox.push(Outgoing::Edit {
        message_id,
        text: text.to_string(),
        keyboard,
    });
}

/// Shows `suffix` (a result or a question) on message `message_id`: the message's
/// `original` text with `suffix` appended. When `original` is unknown — Telegram sent no
/// text for a message the bot can no longer read and nothing is tracked — the message is
/// not overwritten with only the suffix: `suffix` goes out as a new reply (with `keyboard`),
/// and the old message just loses its buttons when `keyboard` is empty.
fn edit_result(ctx: &BotCtx, message_id: i64, original: &str, suffix: &str, keyboard: Value) {
    if !original.trim().is_empty() {
        edit(ctx, message_id, &with_suffix(original, suffix), keyboard);
        return;
    }
    let strip = keyboard == render::no_keyboard();
    send_prompt(ctx, suffix.trim(), message_id, keyboard);
    if strip {
        ctx.outbox.push(Outgoing::Keyboard {
            message_id,
            keyboard: render::no_keyboard(),
        });
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const PAIRED: i64 = 5555;

    fn message(chat: Option<i64>, from: Option<i64>) -> Value {
        let mut m = json!({"message_id": 1, "text": "xin chào"});
        if let Some(c) = chat {
            m["chat"] = json!({"id": c, "type": "private"});
        }
        if let Some(f) = from {
            m["from"] = json!({"id": f, "is_bot": false});
        }
        json!({"update_id": 1, "message": m})
    }

    fn cbq(chat: i64, from: i64) -> Value {
        json!({"update_id": 2, "callback_query": {
            "id": "cb-1",
            "from": {"id": from, "is_bot": false},
            "message": {"message_id": 9, "chat": {"id": chat, "type": "private"}, "text": "t"},
            "data": "a:1:0123456789abcdef"
        }})
    }

    #[test]
    fn authz_paired_and_strangers() {
        assert_eq!(
            authz(&message(Some(PAIRED), Some(PAIRED)), PAIRED),
            Authz::Paired
        );
        assert_eq!(
            authz(&message(Some(7777), Some(7777)), PAIRED),
            Authz::Stranger(7777)
        );
        // Chat ID first.
        assert_eq!(
            authz(&message(Some(7777), Some(PAIRED)), PAIRED),
            Authz::Stranger(7777)
        );
        // Adjacency: the paired chat but another sender.
        assert_eq!(
            authz(&message(Some(PAIRED), Some(8888)), PAIRED),
            Authz::Stranger(8888)
        );
        assert_eq!(authz(&cbq(PAIRED, PAIRED), PAIRED), Authz::Paired);
        assert_eq!(authz(&cbq(PAIRED, 8888), PAIRED), Authz::Stranger(8888));
        assert_eq!(authz(&cbq(7777, 7777), PAIRED), Authz::Stranger(7777));
        assert_eq!(authz(&cbq(7777, PAIRED), PAIRED), Authz::Stranger(7777));
    }

    #[test]
    fn authz_skips_incomplete_updates() {
        // Callback without its message.
        let mut no_msg = cbq(PAIRED, PAIRED);
        no_msg["callback_query"]
            .as_object_mut()
            .unwrap()
            .remove("message");
        assert_eq!(authz(&no_msg, PAIRED), Authz::Skip);
        // Callback without data (also from a stranger).
        let mut no_data = cbq(7777, 7777);
        no_data["callback_query"]
            .as_object_mut()
            .unwrap()
            .remove("data");
        assert_eq!(authz(&no_data, PAIRED), Authz::Skip);
        // Other update types.
        let edited = json!({"update_id": 3, "edited_message": {
            "message_id": 1, "chat": {"id": PAIRED}, "from": {"id": PAIRED}, "text": "x"
        }});
        assert_eq!(authz(&edited, PAIRED), Authz::Skip);
        assert_eq!(authz(&json!({"update_id": 4}), PAIRED), Authz::Skip);
        // Message without chat, or without sender.
        assert_eq!(authz(&message(None, Some(PAIRED)), PAIRED), Authz::Skip);
        assert_eq!(authz(&message(Some(PAIRED), None), PAIRED), Authz::Skip);
        // A string chat id is not a number.
        let odd = json!({"message": {"chat": {"id": "5555"}, "from": {"id": PAIRED}}});
        assert_eq!(authz(&odd, PAIRED), Authz::Skip);
    }

    #[test]
    fn stranger_logged_once_per_window() {
        let mut s = HashMap::new();
        let t0 = Instant::now();
        assert!(stranger_should_log(&mut s, 7777, t0));
        assert!(!stranger_should_log(
            &mut s,
            7777,
            t0 + Duration::from_secs(1)
        ));
        assert!(stranger_should_log(
            &mut s,
            8888,
            t0 + Duration::from_secs(1)
        ));
        assert!(!stranger_should_log(
            &mut s,
            7777,
            t0 + Duration::from_secs(STRANGER_LOG_SECS - 1)
        ));
        assert!(stranger_should_log(
            &mut s,
            7777,
            t0 + Duration::from_secs(STRANGER_LOG_SECS)
        ));
    }

    #[test]
    fn updates_sorted_and_ids_required() {
        let v = json!([
            {"update_id": 12, "message": {}},
            {"message": {}},
            {"update_id": 10, "message": {}},
            {"update_id": 11, "callback_query": {}}
        ]);
        let ids: Vec<i64> = sorted_updates(&v).into_iter().map(|(id, _)| id).collect();
        assert_eq!(ids, vec![10, 11, 12]);
        assert!(sorted_updates(&json!({"x": 1})).is_empty());
    }

    fn pending(expires_at_ms: i64) -> PendingNote {
        PendingNote {
            task_id: 1,
            rev: "0123456789abcdef".to_string(),
            origin_message_id: 10,
            origin_text: "Chờ duyệt plan · task 1".to_string(),
            prompt_message_id: 11,
            expires_at_ms,
        }
    }

    #[test]
    fn pending_note_expiry() {
        let p = pending(1_000);
        assert!(!p.expired(999));
        assert!(p.expired(1_000));
        assert!(p.expired(1_001));

        // Only an expired prompt is taken off the poll state.
        let mut st = PollState::new();
        assert_eq!(take_expired(&mut st, 5_000), None);
        st.pending = Some(pending(1_000));
        assert_eq!(take_expired(&mut st, 999), None);
        assert!(st.pending.is_some());
        assert_eq!(take_expired(&mut st, 1_000), Some(pending(1_000)));
        assert!(st.pending.is_none());
    }

    #[test]
    fn kill_confirm_suffix_text() {
        assert_eq!(kill_confirm_suffix(12), "\n\nXác nhận Kill run 12?");
        let shown = format!("Run đang chạy{}", kill_confirm_suffix(3));
        assert_eq!(without_confirm(&shown, 3), "Run đang chạy");
        // Another run's suffix, or none, is left as it is.
        assert_eq!(without_confirm(&shown, 4), shown);
        assert_eq!(without_confirm("Run đang chạy", 3), "Run đang chạy");
    }

    #[test]
    fn note_prompt_names_the_task() {
        assert_eq!(
            note_prompt_text(7),
            "Ghi chú cho Lead (task 7): gửi tin nhắn kế tiếp làm ghi chú (tối đa 500 ký tự), hoặc bấm Không ghi chú. Hết hạn sau 10 phút."
        );
    }

    #[test]
    fn suffix_keeps_original_text() {
        let s = with_suffix("Chờ duyệt plan · task 1", "\n\n✓ x");
        assert_eq!(s, "Chờ duyệt plan · task 1\n\n✓ x");
        let long = "a".repeat(render::MAX_TEXT + 50);
        let s = with_suffix(&long, "\n\nb");
        assert!(s.ends_with("\n…\n\nb"), "{}", &s[s.len() - 10..]);
        assert_eq!(s.chars().count(), render::MAX_TEXT + 3);
    }
}
