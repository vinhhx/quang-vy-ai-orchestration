//! Outgoing Telegram queue and its sender thread `ab-tg-send` (Phase 05.3, D-05/D-10).
//!
//! Every request goes to `ctx.cfg.chat_id`, the paired chat; there is no other
//! destination. A queued item is rendered from the task's state at send time
//! and dropped when the task has moved on (D-05). While Telegram is unreachable
//! the sender backs off and retries the same item; agent work never waits on it.

use super::detect::{current_event, Event, EventKind};
use super::render;
use super::transport::{base_backoff_ms, Body, CallError};
use super::{sleep_or_stop, BotCtx, Channel};
use crate::dashboard::api_tasks::load_task_state;
use rusqlite::Connection;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const DOCUMENT_MAX_SECS: u64 = 60;
const MESSAGE_MAX_SECS: u64 = 30;
const CAPTION_MAX: usize = 1000;
const DB_READ_ATTEMPTS: u32 = 3;
const EDIT_MAX_SECS: u64 = 20;

#[derive(Debug, Clone, PartialEq)]
pub enum Outgoing {
    Notify(Event),
    /// Replaces the text and buttons of a sent message (D-15, plan 05.3-06).
    Edit {
        message_id: i64,
        text: String,
        keyboard: serde_json::Value,
    },
    /// Replaces only the buttons of a sent message.
    Keyboard {
        message_id: i64,
        keyboard: serde_json::Value,
    },
}

/// Bot API method and JSON body for an `Edit` / `Keyboard` item, always on
/// `chat_id` (the paired chat); `None` for a `Notify` item.
pub fn edit_request(chat_id: i64, item: &Outgoing) -> Option<(&'static str, serde_json::Value)> {
    match item {
        Outgoing::Notify(_) => None,
        Outgoing::Edit {
            message_id,
            text,
            keyboard,
        } => Some((
            "editMessageText",
            serde_json::json!({
                "chat_id": chat_id,
                "message_id": message_id,
                "text": text,
                "reply_markup": keyboard,
            }),
        )),
        Outgoing::Keyboard {
            message_id,
            keyboard,
        } => Some((
            "editMessageReplyMarkup",
            serde_json::json!({
                "chat_id": chat_id,
                "message_id": message_id,
                "reply_markup": keyboard,
            }),
        )),
    }
}

/// Telegram's answer to an edit that changes nothing; the message already
/// shows the wanted content, so it counts as success.
pub fn not_modified(e: &CallError) -> bool {
    matches!(e, CallError::Api { code: 400, description, .. } if description.contains("message is not modified"))
}

#[derive(Default)]
pub struct Outbox {
    q: Mutex<VecDeque<Outgoing>>,
    cv: Condvar,
}

impl Outbox {
    pub fn new() -> Outbox {
        Outbox::default()
    }

    pub fn push(&self, item: Outgoing) {
        let mut q = self.q.lock().unwrap_or_else(|e| e.into_inner());
        q.push_back(item);
        self.cv.notify_one();
    }

    /// Next item, or `None` after `ms` milliseconds without one.
    pub fn pop_timeout(&self, ms: u64) -> Option<Outgoing> {
        let deadline = Instant::now() + Duration::from_millis(ms);
        let mut q = self.q.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(item) = q.pop_front() {
                return Some(item);
            }
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            q = match self.cv.wait_timeout(q, deadline - now) {
                Ok((g, _)) => g,
                Err(e) => e.into_inner().0,
            };
        }
    }

    pub fn len(&self) -> usize {
        self.q.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A message the bot sent for a task (the newest one per task).
#[derive(Debug, Clone)]
pub struct Tracked {
    pub task_id: i64,
    pub key: String,
    pub kind: EventKind,
    pub message_id: i64,
    pub text: String,
    /// The `keyboard_json` sent with the message.
    pub keyboard: serde_json::Value,
}

#[derive(Default)]
pub struct Registry {
    by_task: HashMap<i64, Tracked>,
}

impl Registry {
    /// Inserts `t`, replacing an older message of the same task.
    pub fn insert(&mut self, t: Tracked) {
        self.by_task.insert(t.task_id, t);
    }

    pub fn get(&self, task_id: i64) -> Option<&Tracked> {
        self.by_task.get(&task_id)
    }

    pub fn take(&mut self, task_id: i64) -> Option<Tracked> {
        self.by_task.remove(&task_id)
    }

    pub fn remove_by_message(&mut self, message_id: i64) -> Option<Tracked> {
        let task_id = self
            .by_task
            .values()
            .find(|t| t.message_id == message_id)
            .map(|t| t.task_id)?;
        self.by_task.remove(&task_id)
    }

    pub fn task_ids(&self) -> Vec<i64> {
        let mut ids: Vec<i64> = self.by_task.keys().copied().collect();
        ids.sort_unstable();
        ids
    }
}

fn kind_word(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Gate => "chờ duyệt plan",
        EventKind::Stop => "autopilot dừng",
        EventKind::QaPass => "QA đạt",
        EventKind::RunFailed => "run lỗi",
    }
}

/// Opens the sender's connection when it has none; `None` = stop requested.
fn ensure_conn<'a>(ctx: &BotCtx, slot: &'a mut Option<Connection>) -> Option<&'a Connection> {
    let mut reported = false;
    while slot.is_none() {
        match crate::db::open(&ctx.st.db_path) {
            Ok(c) => *slot = Some(c),
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
    slot.as_ref()
}

enum Step {
    /// Item finished (delivered or dropped).
    Done,
    /// Stop requested: the thread exits.
    Stopped,
}

/// Sends one notification: plan document first (gate), then the text replying to it.
///
/// D-05 guarantees, in order of the loop:
/// - before EVERY attempt (document or text) the task state is reloaded and the
///   item is dropped when `current_event` no longer has the item's key;
/// - every retryable failure reports `health.failed(Channel::Send, ..)` and waits
///   `retry_delay_ms(failures, base)`, `failures` counting the consecutive
///   failures of the current request (reset when a request succeeds);
/// - `health.ok(Channel::Send)` is called once per finished item: after the
///   text message is delivered, or when Telegram rejects it for good. A
///   delivered document alone does not end the outage (its text is still to come);
/// - a document already delivered for the item is never sent again;
/// - the `registry` lock is only taken after Telegram answered, never during a call.
fn notify(ctx: &BotCtx, slot: &mut Option<Connection>, ev: &Event) -> Step {
    let base_ms = base_backoff_ms();
    let chat_id = ctx.cfg.chat_id;
    let task_id = ev.task_id;
    let mut failures: u32 = 0;
    let mut db_failures: u32 = 0;
    let mut doc_done = false;
    let mut doc_message: Option<i64> = None;

    loop {
        if ctx.stop.load(Ordering::SeqCst) {
            return Step::Stopped;
        }
        let conn = match ensure_conn(ctx, slot) {
            Some(c) => c,
            None => return Step::Stopped,
        };
        let s = match load_task_state(&ctx.st, conn, task_id) {
            Ok(Ok(s)) => s,
            Ok(Err(_)) => return Step::Done,
            Err(e) => {
                db_failures += 1;
                if db_failures >= DB_READ_ATTEMPTS {
                    crate::term::emit(&format!("Telegram: lỗi đọc task {task_id}: {}", e.message));
                    crate::term::emit(&format!(
                        "Telegram: bỏ tin cho task {task_id} — không đọc được trạng thái task"
                    ));
                    return Step::Done;
                }
                if sleep_or_stop(&ctx.stop, 1000) {
                    return Step::Stopped;
                }
                continue;
            }
        };
        // D-05: the task moved on since the event was queued.
        if current_event(&s).map(|e| e.key) != Some(ev.key.clone()) {
            return Step::Done;
        }
        let r = render::message_for(ev, &s);

        if let (Some((filename, content)), false) = (&r.document, doc_done) {
            let caption = render::clip(
                &format!("Plan task {task_id}: {}", s.task.title),
                CAPTION_MAX,
            );
            let body = Body::Upload {
                fields: vec![
                    ("chat_id".to_string(), chat_id.to_string()),
                    ("caption".to_string(), caption),
                ],
                file_field: "document".to_string(),
                filename: filename.clone(),
                content: content.clone(),
            };
            match ctx.tx.call("sendDocument", &body, DOCUMENT_MAX_SECS) {
                Ok(v) => {
                    doc_message = v.get("message_id").and_then(|m| m.as_i64());
                    doc_done = true;
                    failures = 0;
                    // Re-check the task before the text attempt (D-05).
                    continue;
                }
                Err(CallError::Stopped) => return Step::Stopped,
                Err(e) => {
                    failures = failures.saturating_add(1);
                    match e.retry_delay_ms(failures, base_ms) {
                        Some(ms) => {
                            ctx.health.failed(Channel::Send, &e.text());
                            if sleep_or_stop(&ctx.stop, ms) {
                                return Step::Stopped;
                            }
                            continue;
                        }
                        None => {
                            crate::term::emit(&format!(
                                "Telegram: không gửi được file plan cho task {task_id} — {}",
                                e.text()
                            ));
                            doc_done = true;
                            failures = 0;
                            continue;
                        }
                    }
                }
            }
        }

        let keyboard = render::keyboard_json(&r.keyboard);
        let mut body = serde_json::json!({
            "chat_id": chat_id,
            "text": r.text,
            "reply_markup": keyboard,
        });
        if let Some(mid) = doc_message {
            body["reply_parameters"] = serde_json::json!({
                "message_id": mid,
                "allow_sending_without_reply": true,
            });
        }
        match ctx
            .tx
            .call("sendMessage", &Body::Json(body), MESSAGE_MAX_SECS)
        {
            Ok(v) => {
                let message_id = v.get("message_id").and_then(|m| m.as_i64()).unwrap_or(0);
                ctx.registry
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(Tracked {
                        task_id,
                        key: ev.key.clone(),
                        kind: ev.kind,
                        message_id,
                        text: r.text.clone(),
                        keyboard,
                    });
                ctx.health.ok(Channel::Send);
                crate::term::emit_out(&format!(
                    "Telegram: đã gửi tin {} cho task {task_id}",
                    kind_word(ev.kind)
                ));
                return Step::Done;
            }
            Err(CallError::Stopped) => return Step::Stopped,
            Err(e) => {
                failures = failures.saturating_add(1);
                match e.retry_delay_ms(failures, base_ms) {
                    Some(ms) => {
                        ctx.health.failed(Channel::Send, &e.text());
                        if sleep_or_stop(&ctx.stop, ms) {
                            return Step::Stopped;
                        }
                    }
                    None => {
                        crate::term::emit(&format!(
                            "Telegram: bỏ tin cho task {task_id} — {}",
                            e.text()
                        ));
                        // Telegram answered (a non-retryable API error): the
                        // channel works; a local failure (curl) says nothing.
                        if matches!(e, CallError::Api { .. }) {
                            ctx.health.ok(Channel::Send);
                        }
                        return Step::Done;
                    }
                }
            }
        }
    }
}

/// Sends one `Edit` / `Keyboard` item with the same retry, drop and Health
/// rules as a notification. The item carries its final content, so it is not
/// re-checked against the task state.
fn edit(ctx: &BotCtx, item: &Outgoing) -> Step {
    let (method, body) = match edit_request(ctx.cfg.chat_id, item) {
        Some(r) => r,
        None => return Step::Done,
    };
    let message_id = body.get("message_id").and_then(|m| m.as_i64()).unwrap_or(0);
    let base_ms = base_backoff_ms();
    let mut failures: u32 = 0;
    loop {
        if ctx.stop.load(Ordering::SeqCst) {
            return Step::Stopped;
        }
        match ctx
            .tx
            .call(method, &Body::Json(body.clone()), EDIT_MAX_SECS)
        {
            Ok(_) => {
                ctx.health.ok(Channel::Send);
                return Step::Done;
            }
            Err(CallError::Stopped) => return Step::Stopped,
            Err(e) if not_modified(&e) => {
                ctx.health.ok(Channel::Send);
                return Step::Done;
            }
            Err(e) => {
                failures = failures.saturating_add(1);
                match e.retry_delay_ms(failures, base_ms) {
                    Some(ms) => {
                        ctx.health.failed(Channel::Send, &e.text());
                        if sleep_or_stop(&ctx.stop, ms) {
                            return Step::Stopped;
                        }
                    }
                    None => {
                        crate::term::emit(&format!(
                            "Telegram: bỏ sửa tin {message_id} — {}",
                            e.text()
                        ));
                        if matches!(e, CallError::Api { .. }) {
                            ctx.health.ok(Channel::Send);
                        }
                        return Step::Done;
                    }
                }
            }
        }
    }
}

fn run(ctx: Arc<BotCtx>) {
    let mut conn: Option<Connection> = None;
    loop {
        if ctx.stop.load(Ordering::SeqCst) {
            return;
        }
        let item = match ctx.outbox.pop_timeout(500) {
            Some(i) => i,
            None => continue,
        };
        let step = match &item {
            Outgoing::Notify(ev) => notify(&ctx, &mut conn, ev),
            Outgoing::Edit { .. } | Outgoing::Keyboard { .. } => edit(&ctx, &item),
        };
        if let Step::Stopped = step {
            return;
        }
    }
}

/// Starts the sender thread `ab-tg-send`. curl children carry PDEATHSIG, which
/// fires when the spawning thread exits: this thread lives as long as the bot.
pub fn spawn(ctx: Arc<BotCtx>) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("ab-tg-send".to_string())
        .spawn(move || run(ctx))
        .expect("spawn ab-tg-send thread")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(task_id: i64) -> Event {
        Event {
            task_id,
            kind: EventKind::Gate,
            key: format!("gate:{task_id}"),
        }
    }

    #[test]
    fn outbox_is_fifo_and_times_out() {
        let ob = Outbox::new();
        assert!(ob.pop_timeout(10).is_none());
        ob.push(Outgoing::Notify(ev(1)));
        ob.push(Outgoing::Notify(ev(2)));
        assert_eq!(ob.len(), 2);
        assert_eq!(ob.pop_timeout(10), Some(Outgoing::Notify(ev(1))));
        assert_eq!(ob.pop_timeout(10), Some(Outgoing::Notify(ev(2))));
        assert!(ob.is_empty());
    }

    #[test]
    fn outbox_wakes_a_waiting_sender() {
        let ob = Arc::new(Outbox::new());
        let ob2 = ob.clone();
        let t = std::thread::spawn(move || ob2.pop_timeout(5000));
        std::thread::sleep(Duration::from_millis(50));
        ob.push(Outgoing::Notify(ev(3)));
        assert_eq!(t.join().unwrap(), Some(Outgoing::Notify(ev(3))));
    }

    fn tracked(task_id: i64, message_id: i64) -> Tracked {
        Tracked {
            task_id,
            key: format!("gate:{task_id}"),
            kind: EventKind::Gate,
            message_id,
            text: String::new(),
            keyboard: render::no_keyboard(),
        }
    }

    #[test]
    fn edit_request_targets_the_paired_chat() {
        let edit = Outgoing::Edit {
            message_id: 7,
            text: "t".to_string(),
            keyboard: serde_json::json!({"inline_keyboard": []}),
        };
        assert_eq!(
            edit_request(5555, &edit),
            Some((
                "editMessageText",
                serde_json::json!({
                    "chat_id": 5555,
                    "message_id": 7,
                    "text": "t",
                    "reply_markup": {"inline_keyboard": []}
                })
            ))
        );
        let k =
            serde_json::json!({"inline_keyboard": [[{"text": "Kill", "callback_data": "k:1:x"}]]});
        let keyboard = Outgoing::Keyboard {
            message_id: 7,
            keyboard: k.clone(),
        };
        assert_eq!(
            edit_request(5555, &keyboard),
            Some((
                "editMessageReplyMarkup",
                serde_json::json!({"chat_id": 5555, "message_id": 7, "reply_markup": k})
            ))
        );
        assert_eq!(edit_request(5555, &Outgoing::Notify(ev(1))), None);
    }

    #[test]
    fn not_modified_only_for_the_same_content_answer() {
        let api = |code: i64, d: &str| CallError::Api {
            code,
            description: d.to_string(),
            retry_after: None,
        };
        assert!(not_modified(&api(
            400,
            "Bad Request: message is not modified: specified new message content and reply markup are exactly the same as a current content and reply markup of the message"
        )));
        assert!(not_modified(&api(
            400,
            "Bad Request: message is not modified: specified new message content and reply markup are exactly the same"
        )));
        assert!(!not_modified(&api(
            400,
            "Bad Request: message to edit not found"
        )));
        assert!(!not_modified(&api(500, "message is not modified")));
        assert!(!not_modified(&CallError::Network("x".to_string())));
    }

    #[test]
    fn registry_keeps_one_message_per_task() {
        let mut r = Registry::default();
        r.insert(tracked(2, 10));
        r.insert(tracked(1, 11));
        r.insert(tracked(2, 12));
        assert_eq!(r.task_ids(), vec![1, 2]);
        assert_eq!(r.get(2).map(|t| t.message_id), Some(12));
        assert!(r.remove_by_message(10).is_none());
        assert_eq!(r.remove_by_message(12).map(|t| t.task_id), Some(2));
        assert_eq!(r.take(1).map(|t| t.message_id), Some(11));
        assert!(r.task_ids().is_empty());
    }
}
