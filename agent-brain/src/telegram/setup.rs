//! `agent-brain telegram setup` and `agent-brain telegram test` (D-08, D-09).
//!
//! Setup reads the bot token (no echo on a terminal), checks it with getMe,
//! prints a 6-digit code and stores the chat ID of the private chat that sends
//! that code, in a 0600 `telegram.json`. The token is never printed, logged or
//! put on any argv; Telegram is reached only through `transport` (curl `-K -`).

use super::config::{self, TelegramConfig};
use super::transport::{self, Body, CallError, Transport};
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Lifetime of a pairing code (10 minutes).
pub const CODE_SECS: i64 = 600;
/// Wrong 6-digit codes accepted before setup gives up.
pub const MAX_WRONG: u32 = 5;

const SHORT_CALL_SECS: u64 = 20;

/// 6 decimal digits from `/dev/urandom`, uniform by rejection sampling
/// (4_294_000_000 is the largest multiple of 1_000_000 below 2^32).
pub fn new_code() -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open("/dev/urandom")?;
    loop {
        let mut buf = [0u8; 4];
        f.read_exact(&mut buf)?;
        let v = u32::from_le_bytes(buf);
        if v < 4_294_000_000 {
            return Ok(format!("{:06}", v % 1_000_000));
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PairStep {
    Ignore,
    Wrong,
    TooMany,
    Paired(i64),
}

pub struct Pairing {
    pub code: String,
    pub deadline_ms: i64,
    pub wrong: u32,
}

fn is_six_digits(s: &str) -> bool {
    s.len() == 6 && s.bytes().all(|b| b.is_ascii_digit())
}

impl Pairing {
    pub fn new(code: String, now_ms: i64) -> Pairing {
        Pairing {
            code,
            deadline_ms: now_ms.saturating_add(CODE_SECS * 1000),
            wrong: 0,
        }
    }

    pub fn expired(&self, now_ms: i64) -> bool {
        now_ms >= self.deadline_ms
    }

    /// Only a `message` from a private chat whose sender is the chat itself
    /// counts (D-09/D-10); groups, channels and callbacks are ignored.
    pub fn on_update(&mut self, update: &Value) -> PairStep {
        let Some(msg) = update.get("message") else {
            return PairStep::Ignore;
        };
        let chat = msg.get("chat");
        if chat.and_then(|c| c.get("type")).and_then(|t| t.as_str()) != Some("private") {
            return PairStep::Ignore;
        }
        let Some(chat_id) = chat.and_then(|c| c.get("id")).and_then(|i| i.as_i64()) else {
            return PairStep::Ignore;
        };
        let from_id = msg
            .get("from")
            .and_then(|f| f.get("id"))
            .and_then(|i| i.as_i64());
        if from_id != Some(chat_id) {
            return PairStep::Ignore;
        }
        let Some(text) = msg.get("text").and_then(|t| t.as_str()) else {
            return PairStep::Ignore;
        };
        let text = text.trim();
        let candidate = match text.strip_prefix("/start ") {
            Some(rest) => rest.trim(),
            None => text,
        };
        if !is_six_digits(candidate) {
            return PairStep::Ignore;
        }
        if candidate == self.code {
            return PairStep::Paired(chat_id);
        }
        self.wrong = self.wrong.saturating_add(1);
        if self.wrong >= MAX_WRONG {
            PairStep::TooMany
        } else {
            PairStep::Wrong
        }
    }
}

fn fail(msg: &str) -> i32 {
    crate::term::emit(msg);
    1
}

/// `check_curl`'s text names `serve`; here the command to re-run is setup/test.
fn curl_ok(curl: &str, rerun: &str) -> Result<(), String> {
    transport::check_curl(curl).map_err(|e| e.replace("agent-brain serve", rerun))
}

/// Text for a failed call. `CallError` texts never contain the token
/// (descriptions are redacted by the transport).
fn call_failed(e: &CallError) -> String {
    match e {
        CallError::Network(_) | CallError::BadResponse(_) => {
            format!("Không tới được Telegram: {}", e.text())
        }
        CallError::Api { code, .. } if *code == 401 || *code == 404 => {
            format!("Telegram từ chối token ({code}) — kiểm tra lại token từ @BotFather")
        }
        CallError::Api { code: 409, .. } => {
            "Bot đang được đọc ở nơi khác (409) — tắt webhook hoặc tiến trình đang dùng bot rồi chạy lại".to_string()
        }
        _ => e.text(),
    }
}

/// Next offset after the update IDs in a `getUpdates` result.
fn next_offset(result: &Value, current: i64) -> i64 {
    result
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|u| u.get("update_id").and_then(|i| i.as_i64()))
                .fold(current, |acc, id| acc.max(id.saturating_add(1)))
        })
        .unwrap_or(current)
}

fn sorted_updates(result: &Value) -> Vec<Value> {
    let mut ups: Vec<Value> = result.as_array().cloned().unwrap_or_default();
    ups.sort_by_key(|u| u.get("update_id").and_then(|i| i.as_i64()).unwrap_or(0));
    ups
}

pub fn cmd_setup() -> i32 {
    // (a) curl and API base before anything else.
    let curl = transport::curl_bin();
    if let Err(e) = curl_ok(&curl, "agent-brain telegram setup") {
        return fail(&e);
    }
    let base = match transport::api_base() {
        Ok(b) => b,
        Err(e) => return fail(&e),
    };

    // (b) One getUpdates reader per bot: refuse while serve runs, and hold the
    // lock while pairing so serve cannot start meanwhile.
    let data_dir = crate::config::data_dir();
    let mut lock = match crate::dashboard::lock::acquire(&data_dir) {
        Ok(l) => l,
        Err(crate::dashboard::lock::LockError::Held { pid, .. }) => {
            let pid = pid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "?".to_string());
            return fail(&format!(
                "agent-brain serve đang chạy (pid {pid}) — dừng serve (Ctrl+C) trước khi ghép nối Telegram, rồi chạy lại serve."
            ));
        }
        Err(crate::dashboard::lock::LockError::Io(e)) => {
            let path = data_dir.join(crate::dashboard::lock::LOCK_FILE);
            return fail(&format!("Không tạo được file khóa {}: {e}", path.display()));
        }
    };
    // The lock file still names the last serve; name this process instead so
    // a serve started meanwhile does not point at a stale pid.
    let _ = lock.write_info(std::process::id(), 0, crate::db::now_ms());
    let code = pair(&curl, base, &data_dir);
    drop(lock);
    code
}

fn pair(curl: &str, base: String, data_dir: &std::path::Path) -> i32 {
    // (c) Token.
    let raw = match super::prompt::read_secret("Token bot (từ @BotFather): ") {
        Ok(t) => t,
        Err(e) => return fail(&format!("Không đọc được token: {e}")),
    };
    let token = raw.trim().to_string();
    drop(raw);
    if token.is_empty() {
        return fail("Chưa nhập token — chạy lại agent-brain telegram setup");
    }
    if !config::token_shape_ok(&token) {
        return fail("Token không đúng dạng <số>:<chuỗi> — lấy lại token từ @BotFather");
    }

    // (d) getMe.
    let stop = Arc::new(AtomicBool::new(false));
    let tx = Transport::new(
        token.clone(),
        base,
        curl.to_string(),
        data_dir.join("telegram-upload"),
        stop,
    );
    let me = match tx.call("getMe", &Body::Json(json!({})), SHORT_CALL_SECS) {
        Ok(v) => v,
        Err(e) => return fail(&call_failed(&e)),
    };
    let username = me
        .get("username")
        .and_then(|u| u.as_str())
        .unwrap_or("?")
        .to_string();

    // (e) Drain: anything sent before the code is printed never counts.
    let mut offset = match tx.call(
        "getUpdates",
        &Body::Json(json!({"offset": -1, "timeout": 0})),
        SHORT_CALL_SECS,
    ) {
        Ok(v) => next_offset(&v, 0),
        Err(e) => return fail(&call_failed(&e)),
    };

    // (f) Code.
    let code = match new_code() {
        Ok(c) => c,
        Err(e) => return fail(&format!("Không tạo được mã ghép nối (/dev/urandom): {e}")),
    };
    let mut pairing = Pairing::new(code, crate::db::now_ms());
    crate::term::emit_out(&format!(
        "Mở Telegram, nhắn cho @{username} đúng mã sau (hết hạn sau 10 phút): {}",
        pairing.code
    ));

    // (g) Wait for the code.
    let base_ms = transport::base_backoff_ms();
    let mut failures: u32 = 0;
    let chat_id = 'wait: loop {
        let now = crate::db::now_ms();
        if pairing.expired(now) {
            return fail("Mã đã hết hạn (10 phút) — chạy lại agent-brain telegram setup");
        }
        let left_secs = ((pairing.deadline_ms - now) / 1000).max(0) as u64;
        let timeout = transport::LONG_POLL_SECS.min(left_secs);
        let mut body = json!({"timeout": timeout, "allowed_updates": ["message"]});
        if offset > 0 {
            body["offset"] = json!(offset);
        }
        match tx.call("getUpdates", &Body::Json(body), timeout + 10) {
            Ok(v) => {
                failures = 0;
                offset = next_offset(&v, offset);
                for up in sorted_updates(&v) {
                    match pairing.on_update(&up) {
                        PairStep::Paired(chat) => break 'wait chat,
                        PairStep::TooMany => {
                            return fail("Quá nhiều mã sai — chạy lại agent-brain telegram setup")
                        }
                        PairStep::Wrong => crate::term::emit(&format!(
                            "Mã sai ({}/{MAX_WRONG}) — đang chờ mã đúng.",
                            pairing.wrong
                        )),
                        PairStep::Ignore => {}
                    }
                }
            }
            Err(e) => {
                failures = failures.saturating_add(1);
                match e.retry_delay_ms(failures, base_ms) {
                    Some(ms) => {
                        let left_ms = (pairing.deadline_ms - crate::db::now_ms()).max(0) as u64;
                        std::thread::sleep(std::time::Duration::from_millis(ms.min(left_ms)));
                    }
                    None => return fail(&call_failed(&e)),
                }
            }
        }
    };

    // (h) Save, confirm in Telegram, confirm the offset, confirm here.
    let path = config::path();
    let cfg = TelegramConfig {
        token,
        chat_id,
        paired_at: crate::db::now_ms(),
    };
    if let Err(e) = config::save(&path, &cfg) {
        return fail(&e.message);
    }
    if let Err(e) = tx.call(
        "sendMessage",
        &Body::Json(json!({
            "chat_id": chat_id,
            "text": "Đã ghép nối agent-brain với chat này. Bạn sẽ nhận tin khi task cần bạn duyệt hoặc xử lý.",
        })),
        SHORT_CALL_SECS,
    ) {
        crate::term::emit(&format!(
            "Không gửi được tin xác nhận tới Telegram: {}",
            e.text()
        ));
    }
    if offset > 0 {
        let _ = tx.call(
            "getUpdates",
            &Body::Json(json!({"offset": offset, "timeout": 0})),
            SHORT_CALL_SECS,
        );
    }
    crate::term::emit_out(&format!(
        "Đã ghép nối với chat {chat_id} (@{username}). Đã lưu {} (quyền 0600). Khởi động lại agent-brain serve để bật thông báo.",
        path.display()
    ));
    0
}

fn hostname() -> String {
    let mut buf = [0u8; 256];
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if rc != 0 {
        return "máy này".to_string();
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let name = String::from_utf8_lossy(&buf[..end]).trim().to_string();
    if name.is_empty() {
        "máy này".to_string()
    } else {
        name
    }
}

/// Sends one test message to the paired chat. Same 0600 / validity refusals
/// as serve (`config::load`).
pub fn cmd_test() -> i32 {
    let path = config::path();
    let cfg = match config::load(&path) {
        Ok(c) => c,
        Err(config::LoadError::Missing) => {
            return fail("Chưa ghép nối Telegram — chạy agent-brain telegram setup")
        }
        Err(e) => return fail(&e.message().unwrap_or_default()),
    };
    let curl = transport::curl_bin();
    if let Err(e) = curl_ok(&curl, "agent-brain telegram test") {
        return fail(&e);
    }
    let base = match transport::api_base() {
        Ok(b) => b,
        Err(e) => return fail(&e),
    };
    let tx = Transport::new(
        cfg.token.clone(),
        base,
        curl,
        crate::config::data_dir().join("telegram-upload"),
        Arc::new(AtomicBool::new(false)),
    );
    let text = format!(
        "Tin thử từ agent-brain trên máy {}: kết nối Telegram hoạt động.",
        hostname()
    );
    match tx.call(
        "sendMessage",
        &Body::Json(json!({"chat_id": cfg.chat_id, "text": text})),
        SHORT_CALL_SECS,
    ) {
        Ok(_) => {
            crate::term::emit_out(&format!("Đã gửi tin thử tới chat {}.", cfg.chat_id));
            0
        }
        Err(e) => fail(&call_failed(&e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(chat_id: i64, chat_type: &str, from_id: i64, text: &str) -> Value {
        json!({
            "update_id": 1,
            "message": {
                "message_id": 10,
                "chat": {"id": chat_id, "type": chat_type},
                "from": {"id": from_id, "is_bot": false},
                "text": text
            }
        })
    }

    fn pairing() -> Pairing {
        Pairing::new("123456".to_string(), 1_000_000)
    }

    #[test]
    fn new_code_is_six_digits() {
        for _ in 0..1000 {
            let c = new_code().unwrap();
            assert_eq!(c.len(), 6, "{c}");
            assert!(c.bytes().all(|b| b.is_ascii_digit()), "{c}");
        }
    }

    #[test]
    fn exact_code_from_private_chat_pairs() {
        let mut p = pairing();
        assert_eq!(
            p.on_update(&msg(5555, "private", 5555, "123456")),
            PairStep::Paired(5555)
        );
        let mut p = pairing();
        assert_eq!(
            p.on_update(&msg(5555, "private", 5555, "  123456 \n")),
            PairStep::Paired(5555)
        );
    }

    #[test]
    fn start_with_code_pairs_and_bare_start_is_ignored() {
        let mut p = pairing();
        assert_eq!(
            p.on_update(&msg(5555, "private", 5555, "/start 123456")),
            PairStep::Paired(5555)
        );
        let mut p = pairing();
        assert_eq!(
            p.on_update(&msg(5555, "private", 5555, "/start")),
            PairStep::Ignore
        );
        assert_eq!(p.wrong, 0);
    }

    #[test]
    fn group_chat_and_foreign_sender_are_ignored() {
        let mut p = pairing();
        assert_eq!(
            p.on_update(&msg(-100123, "group", 5555, "123456")),
            PairStep::Ignore
        );
        assert_eq!(
            p.on_update(&msg(-100123, "supergroup", 5555, "000000")),
            PairStep::Ignore
        );
        assert_eq!(
            p.on_update(&msg(5555, "private", 6666, "123456")),
            PairStep::Ignore
        );
        assert_eq!(p.wrong, 0);
    }

    #[test]
    fn non_messages_and_other_text_are_ignored() {
        let mut p = pairing();
        let cb = json!({"update_id": 2, "callback_query": {"data": "123456"}});
        assert_eq!(p.on_update(&cb), PairStep::Ignore);
        for text in ["hello", "12345", "1234567", "12345a", ""] {
            assert_eq!(
                p.on_update(&msg(5555, "private", 5555, text)),
                PairStep::Ignore,
                "{text}"
            );
        }
        let no_text = json!({"update_id": 3, "message": {
            "chat": {"id": 5555, "type": "private"}, "from": {"id": 5555}}});
        assert_eq!(p.on_update(&no_text), PairStep::Ignore);
        assert_eq!(p.wrong, 0);
    }

    #[test]
    fn five_wrong_codes_abort() {
        let mut p = pairing();
        for n in 1..=4 {
            assert_eq!(
                p.on_update(&msg(5555, "private", 5555, "000000")),
                PairStep::Wrong,
                "attempt {n}"
            );
        }
        assert_eq!(
            p.on_update(&msg(5555, "private", 5555, "000000")),
            PairStep::TooMany
        );
        assert_eq!(p.wrong, MAX_WRONG);
    }

    #[test]
    fn code_expires_after_ten_minutes() {
        let now = 1_000_000;
        let p = Pairing::new("123456".to_string(), now);
        assert_eq!(p.deadline_ms, now + CODE_SECS * 1000);
        assert!(!p.expired(now));
        assert!(!p.expired(now + 599_999));
        assert!(p.expired(now + 600_000));
    }

    #[test]
    fn next_offset_moves_past_every_update() {
        let v = json!([{"update_id": 7}, {"update_id": 3}, {"update_id": 9}]);
        assert_eq!(next_offset(&v, 0), 10);
        assert_eq!(next_offset(&json!([]), 4), 4);
        assert_eq!(next_offset(&json!([{"update_id": 2}]), 5), 5);
    }
}
