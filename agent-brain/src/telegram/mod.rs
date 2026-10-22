//! Telegram bot inside `agent-brain serve` (Phase 05.3, D-01/D-04/D-05/D-06).
//!
//! Outbound long polling only: this module never opens a listening socket.
//! All traffic goes through `transport::Transport` (system curl, token on stdin).

pub mod bot;
pub mod callback;
pub mod compose;
pub mod config;
pub mod detect;
pub mod intake;
pub mod outbox;
pub mod prompt;
pub mod render;
pub mod setup;
pub mod transport;

use crate::dashboard::AppState;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Telegram channels whose health is tracked separately.
/// `Poll` = the `ab-tg` thread (getMe, getUpdates); `Send` = the outbox thread `ab-tg-send`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Poll,
    Send,
}

impl Channel {
    fn bit(self) -> u8 {
        match self {
            Channel::Poll => 1,
            Channel::Send => 2,
        }
    }
}

/// One outage = one warning line + one recovery line, however the channels interleave.
/// Each channel owns one bit of the mask; every transition is a single atomic op.
#[derive(Debug, Default)]
pub struct Health {
    mask: AtomicU8,
}

impl Health {
    pub fn new() -> Health {
        Health {
            mask: AtomicU8::new(0),
        }
    }

    /// Marks `ch` failing. Prints the warning (and returns true) only when no
    /// channel was failing before, i.e. an outage starts.
    pub fn failed(&self, ch: Channel, what: &str) -> bool {
        let prev = self.mask.fetch_or(ch.bit(), Ordering::SeqCst);
        if prev == 0 {
            crate::term::emit(&format!(
                "Telegram: không tới được Telegram ({what}) — đang thử lại; thông báo được giữ lại, các run và autopilot vẫn chạy bình thường."
            ));
            true
        } else {
            false
        }
    }

    /// Marks `ch` healthy. Prints the recovery line (and returns true) only when
    /// `ch` was the last failing channel, i.e. the outage ends.
    pub fn ok(&self, ch: Channel) -> bool {
        let prev = self.mask.fetch_and(!ch.bit(), Ordering::SeqCst);
        if prev == ch.bit() {
            crate::term::emit_out("Telegram: đã kết nối lại");
            true
        } else {
            false
        }
    }

    pub fn ok_quiet(&self, ch: Channel) {
        self.mask.fetch_and(!ch.bit(), Ordering::SeqCst);
    }

    pub fn is_failing(&self) -> bool {
        self.mask.load(Ordering::SeqCst) != 0
    }
}

pub struct BotCtx {
    pub st: Arc<AppState>,
    pub cfg: config::TelegramConfig,
    pub tx: transport::Transport,
    pub health: Health,
    pub stop: Arc<AtomicBool>,
    pub outbox: outbox::Outbox,
    pub registry: std::sync::Mutex<outbox::Registry>,
}

pub struct BotHandle {
    stop: Arc<AtomicBool>,
}

impl BotHandle {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Sleeps `ms` in 100 ms slices; returns true as soon as `stop` is set.
pub fn sleep_or_stop(stop: &AtomicBool, ms: u64) -> bool {
    let mut left = ms;
    loop {
        if stop.load(Ordering::SeqCst) {
            return true;
        }
        if left == 0 {
            return false;
        }
        let slice = left.min(100);
        std::thread::sleep(Duration::from_millis(slice));
        left -= slice;
    }
}

/// Starts the bot when `telegram.json` is present and valid. Without the file
/// nothing is printed and `None` is returned (serve behaves as before, D-06).
pub fn start(st: Arc<AppState>) -> Option<BotHandle> {
    let path = config::path();
    let cfg = match config::load(&path) {
        Ok(c) => c,
        Err(config::LoadError::Missing) => return None,
        Err(e) => {
            if let Some(msg) = e.message() {
                crate::term::emit(&format!("Telegram: lỗi — {msg}"));
            }
            return None;
        }
    };
    let base = match transport::api_base() {
        Ok(b) => b,
        Err(e) => {
            crate::term::emit(&format!("Telegram: lỗi — {e}"));
            return None;
        }
    };
    let curl = transport::curl_bin();
    if let Err(e) = transport::check_curl(&curl) {
        crate::term::emit(&format!("Telegram: lỗi — {e}"));
        return None;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let upload_dir = crate::config::data_dir().join("telegram-upload");
    let _ = std::fs::remove_dir_all(&upload_dir);
    let tx = transport::Transport::new(cfg.token.clone(), base, curl, upload_dir, stop.clone());
    let ctx = Arc::new(BotCtx {
        st,
        cfg,
        tx,
        health: Health::new(),
        stop: stop.clone(),
        outbox: outbox::Outbox::new(),
        registry: std::sync::Mutex::new(outbox::Registry::default()),
    });

    // Notifications do not wait for getMe: the sender backs off while
    // Telegram is unreachable and the detector only queues.
    outbox::spawn(ctx.clone());
    detect::spawn(ctx.clone());

    // curl children carry PDEATHSIG, which fires when the spawning THREAD exits:
    // curl is only spawned from this long-lived thread.
    let spawned = std::thread::Builder::new()
        .name("ab-tg".to_string())
        .spawn(move || poll_thread(ctx));
    if let Err(e) = spawned {
        crate::term::emit(&format!("Telegram: lỗi — không tạo được luồng bot: {e}"));
        // The sender and the detector are already running and nobody holds a
        // `BotHandle` to stop them.
        stop.store(true, Ordering::SeqCst);
        return None;
    }
    Some(BotHandle { stop })
}

fn poll_thread(ctx: Arc<BotCtx>) {
    let base_ms = transport::base_backoff_ms();
    let mut failures: u32 = 0;
    loop {
        if ctx.stop.load(Ordering::SeqCst) {
            return;
        }
        match ctx
            .tx
            .call("getMe", &transport::Body::Json(serde_json::json!({})), 20)
        {
            Ok(me) => {
                ctx.health.ok_quiet(Channel::Poll);
                let username = me.get("username").and_then(|u| u.as_str()).unwrap_or("?");
                crate::term::emit_out(&format!("Telegram: đã kết nối (@{username})"));
                // The `/task` / `/huy` command menu (INTAKE-01): once, a failure is ignored.
                let _ = ctx.tx.call(
                    "setMyCommands",
                    &transport::Body::Json(serde_json::json!({"commands": [
                        {"command": "task", "description": "Giao việc mới cho agent-brain"},
                        {"command": "huy", "description": "Huỷ yêu cầu giao việc đang mở"},
                    ]})),
                    20,
                );
                // Same thread: curl's PDEATHSIG stays tied to a long-lived thread.
                bot::run_poll(&ctx);
                return;
            }
            Err(e) if e.is_unauthorized() => {
                crate::term::emit(
                    "Telegram: lỗi — Telegram từ chối token (401) — chạy lại agent-brain telegram setup",
                );
                ctx.stop.store(true, Ordering::SeqCst);
                return;
            }
            Err(transport::CallError::Stopped) => return,
            Err(e) => {
                failures = failures.saturating_add(1);
                match e.retry_delay_ms(failures, base_ms) {
                    Some(ms) => {
                        ctx.health.failed(Channel::Poll, &e.text());
                        if sleep_or_stop(&ctx.stop, ms) {
                            return;
                        }
                    }
                    None => {
                        // Retrying is pointless (e.g. 404 for a malformed or deleted
                        // token): stop the sender and the detector too, or they keep
                        // queueing events that are rejected one by one.
                        crate::term::emit(&format!("Telegram: lỗi — {}", e.text()));
                        ctx.stop.store(true, Ordering::SeqCst);
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_tracks_channels_separately() {
        let h = Health::new();
        assert!(h.failed(Channel::Send, "x"));
        assert!(!h.failed(Channel::Send, "x"));
        // A poll success does not end the send outage.
        assert!(!h.ok(Channel::Poll));
        assert!(h.is_failing());
        // Outage already announced.
        assert!(!h.failed(Channel::Poll, "y"));
        // Poll still failing.
        assert!(!h.ok(Channel::Send));
        assert!(h.is_failing());
        assert!(h.ok(Channel::Poll));
        assert!(!h.is_failing());
        assert!(!h.ok(Channel::Poll));
        // A new outage.
        assert!(h.failed(Channel::Poll, "z"));
        h.ok_quiet(Channel::Poll);
        assert!(!h.is_failing());
    }

    #[test]
    fn health_single_send_outage_recovers() {
        let h = Health::new();
        assert!(h.failed(Channel::Send, "x"));
        assert!(h.ok(Channel::Send));
        assert!(!h.is_failing());
    }

    #[test]
    fn sleep_or_stop_returns_early() {
        let stop = AtomicBool::new(true);
        let t = std::time::Instant::now();
        assert!(sleep_or_stop(&stop, 10_000));
        assert!(t.elapsed() < Duration::from_millis(500));
        let go = AtomicBool::new(false);
        assert!(!sleep_or_stop(&go, 120));
    }
}
