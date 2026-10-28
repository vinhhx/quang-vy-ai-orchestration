//! Read-only status of the Telegram bot for the dashboard (Phase 06.2 plan 02).
//!
//! A process-global store, written by `telegram::start`, `poll_thread` and the `Health`
//! transitions, read by `GET /api/telegram`. It never holds the bot token in a field that
//! is serialized: the only token-derived data kept is a redaction list used to scrub the
//! failure messages before they are stored.

use serde::Serialize;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// `telegram.json` is missing: the bot is not set up.
    Off,
    /// The file is present but unusable, or Telegram rejected the token.
    Error,
    /// Bot threads spawned, no successful `getMe` yet.
    Starting,
    /// `getMe` succeeded and `Health` is not failing.
    Running,
    /// `Health::is_failing()`.
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    pub state: State,
    /// Vietnamese reason, only for `error` / `degraded`.
    pub message: Option<String>,
    pub bot_username: Option<String>,
    pub chat_id: Option<i64>,
    pub paired_at: Option<i64>,
    pub last_ok_at: Option<i64>,
    pub outbox_pending: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Base {
    Off,
    Error,
    Active,
}

type Probe = Box<dyn Fn() -> usize + Send + Sync>;

struct Inner {
    base: Base,
    failing: bool,
    message: Option<String>,
    bot_username: Option<String>,
    chat_id: Option<i64>,
    paired_at: Option<i64>,
    last_ok_at: Option<i64>,
    outbox_probe: Option<Probe>,
    redact: Vec<String>,
}

impl Inner {
    fn new() -> Inner {
        Inner {
            base: Base::Off,
            failing: false,
            message: None,
            bot_username: None,
            chat_id: None,
            paired_at: None,
            last_ok_at: None,
            outbox_probe: None,
            redact: Vec::new(),
        }
    }

    fn scrub(&self, text: &str) -> String {
        let mut out = text.to_string();
        for secret in &self.redact {
            if !secret.is_empty() {
                out = out.replace(secret.as_str(), "<ẩn>");
            }
        }
        out
    }
}

pub struct Store {
    inner: Mutex<Inner>,
}

impl Default for Store {
    fn default() -> Self {
        Store::new()
    }
}

impl Store {
    pub fn new() -> Store {
        Store {
            inner: Mutex::new(Inner::new()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// No `telegram.json`.
    pub fn set_off(&self) {
        *self.lock() = Inner::new();
    }

    /// Unusable file, failed pre-flight check, or a token Telegram rejected.
    pub fn set_error(&self, message: &str) {
        let mut g = self.lock();
        let message = g.scrub(message);
        g.base = Base::Error;
        g.failing = false;
        g.message = Some(message);
    }

    /// Bot threads are up. `token` is only used to scrub later messages.
    pub fn set_starting(&self, chat_id: i64, paired_at: i64, token: &str, probe: Probe) {
        let mut redact = vec![token.to_string()];
        if let Some((_, secret)) = token.split_once(':') {
            redact.push(secret.to_string());
        }
        *self.lock() = Inner {
            base: Base::Active,
            chat_id: Some(chat_id),
            paired_at: Some(paired_at),
            outbox_probe: Some(probe),
            redact,
            ..Inner::new()
        };
    }

    /// First successful `getMe`.
    pub fn set_connected(&self, username: &str) {
        let mut g = self.lock();
        if g.base == Base::Active {
            g.bot_username = Some(username.to_string());
        }
    }

    /// A channel started failing; `message` is the Vietnamese outage text.
    pub fn note_failed(&self, message: &str) {
        let mut g = self.lock();
        if g.base != Base::Active {
            return;
        }
        let message = g.scrub(message);
        g.failing = true;
        g.message = Some(message);
    }

    /// A call succeeded at `now_ms`; `healthy` is `!Health::is_failing()` right after it.
    pub fn note_ok(&self, now_ms: i64, healthy: bool) {
        let mut g = self.lock();
        if g.base != Base::Active {
            return;
        }
        g.last_ok_at = Some(now_ms);
        if healthy {
            g.failing = false;
            g.message = None;
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let g = self.lock();
        let state = match g.base {
            Base::Off => State::Off,
            Base::Error => State::Error,
            Base::Active if g.failing => State::Degraded,
            Base::Active if g.bot_username.is_none() => State::Starting,
            Base::Active => State::Running,
        };
        let message = match state {
            State::Error | State::Degraded => g.message.clone(),
            _ => None,
        };
        Snapshot {
            state,
            message,
            bot_username: g.bot_username.clone(),
            chat_id: g.chat_id,
            paired_at: g.paired_at,
            last_ok_at: g.last_ok_at,
            outbox_pending: g.outbox_probe.as_ref().map(|p| p()).unwrap_or(0),
        }
    }
}

static STORE: OnceLock<Store> = OnceLock::new();

pub fn store() -> &'static Store {
    STORE.get_or_init(Store::new)
}

pub fn snapshot() -> Snapshot {
    store().snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "4242:fake-token-for-tests";

    fn started() -> Store {
        let s = Store::new();
        s.set_starting(55, 7, TOKEN, Box::new(|| 3));
        s
    }

    #[test]
    fn default_is_off() {
        let snap = Store::new().snapshot();
        assert_eq!(snap.state, State::Off);
        assert_eq!(snap.message, None);
        assert_eq!(snap.bot_username, None);
        assert_eq!(snap.outbox_pending, 0);
    }

    #[test]
    fn starting_then_running_then_degraded_then_running() {
        let s = started();
        let snap = s.snapshot();
        assert_eq!(snap.state, State::Starting);
        assert_eq!(snap.chat_id, Some(55));
        assert_eq!(snap.paired_at, Some(7));
        assert_eq!(snap.outbox_pending, 3);
        s.note_ok(100, true);
        s.set_connected("ab_bot");
        let snap = s.snapshot();
        assert_eq!(snap.state, State::Running);
        assert_eq!(snap.bot_username.as_deref(), Some("ab_bot"));
        assert_eq!(snap.last_ok_at, Some(100));
        assert_eq!(snap.message, None);
        s.note_failed("không tới được Telegram");
        let snap = s.snapshot();
        assert_eq!(snap.state, State::Degraded);
        assert_eq!(snap.message.as_deref(), Some("không tới được Telegram"));
        // A success while another channel still fails keeps the outage on screen.
        s.note_ok(200, false);
        assert_eq!(s.snapshot().state, State::Degraded);
        assert_eq!(s.snapshot().last_ok_at, Some(200));
        s.note_ok(300, true);
        let snap = s.snapshot();
        assert_eq!(snap.state, State::Running);
        assert_eq!(snap.message, None);
        // The username survives an outage.
        assert_eq!(snap.bot_username.as_deref(), Some("ab_bot"));
    }

    #[test]
    fn failure_before_getme_is_degraded_not_starting() {
        let s = started();
        s.note_failed("x");
        assert_eq!(s.snapshot().state, State::Degraded);
        s.note_ok(1, true);
        assert_eq!(s.snapshot().state, State::Starting);
    }

    #[test]
    fn error_keeps_message_and_ignores_later_health_calls() {
        let s = started();
        s.set_error("Telegram từ chối token (401)");
        s.note_ok(1, true);
        s.note_failed("late");
        let snap = s.snapshot();
        assert_eq!(snap.state, State::Error);
        assert_eq!(
            snap.message.as_deref(),
            Some("Telegram từ chối token (401)")
        );
    }

    #[test]
    fn health_calls_without_a_bot_do_nothing() {
        let s = Store::new();
        s.note_failed("x");
        s.note_ok(1, true);
        s.set_connected("nobody");
        assert_eq!(s.snapshot(), Store::new().snapshot());
    }

    #[test]
    fn messages_never_carry_the_token() {
        let s = started();
        s.note_failed(&format!("curl: lỗi tại {TOKEN} và fake-token-for-tests"));
        let snap = s.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        assert!(!json.contains(TOKEN), "{json}");
        assert!(!json.contains("fake-token-for-tests"), "{json}");
        assert!(json.contains("<ẩn>"), "{json}");
    }

    #[test]
    fn serializes_state_as_lowercase_word() {
        let s = started();
        let v = serde_json::to_value(s.snapshot()).unwrap();
        assert_eq!(v["state"], "starting");
        for (state, word) in [
            (State::Off, "off"),
            (State::Error, "error"),
            (State::Starting, "starting"),
            (State::Running, "running"),
            (State::Degraded, "degraded"),
        ] {
            assert_eq!(serde_json::to_value(state).unwrap(), word);
        }
    }
}
