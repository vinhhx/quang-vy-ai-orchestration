//! GET /api/telegram (Phase 06.2 plan 02): the read-only status of the Telegram bot.
//! Every test runs its own serve in its own sandbox (own XDG dirs) against the local
//! fake Telegram; nothing reads the user's real config and nothing contacts api.telegram.org.

mod common;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use common::{wait_until, Sandbox, Serve};
use serde_json::{json, Value};
use std::time::Duration;
use tg::{
    telegram_json_path, write_config, write_raw_config, FakeTelegram, BOT_USERNAME, FAKE_TOKEN,
    PAIRED_CHAT,
};

/// The secret half of `FAKE_TOKEN` (after the colon).
fn secret_half() -> &'static str {
    FAKE_TOKEN.split_once(':').unwrap().1
}

fn status_of(srv: &Serve) -> Value {
    let resp = srv.get("/api/telegram");
    assert_eq!(resp.status, 200, "{}", resp.text());
    resp.json()
}

fn assert_no_token(body: &str, what: &str) {
    assert!(
        !body.contains(FAKE_TOKEN),
        "{what}: full token leaked: {body}"
    );
    assert!(
        !body.contains(secret_half()),
        "{what}: token secret half leaked: {body}"
    );
}

fn wait_state(srv: &Serve, want: &str) -> Value {
    let mut last = Value::Null;
    let hit = wait_until(Duration::from_secs(15), || {
        last = status_of(srv);
        last["state"] == want
    });
    assert!(hit, "state never reached {want}; last: {last}");
    last
}

#[test]
fn telegram_off_without_file() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);
    let v = status_of(&srv);
    assert_eq!(v["state"], "off", "{v}");
    assert!(v["message"].is_null(), "{v}");
    assert!(v["bot_username"].is_null(), "{v}");
    assert!(v["chat_id"].is_null(), "{v}");
    assert!(v["paired_at"].is_null(), "{v}");
    assert!(v["last_ok_at"].is_null(), "{v}");
    assert_eq!(v["outbox_pending"], 0, "{v}");
    assert_eq!(v["tasks_today"], 0, "{v}");
    assert_eq!(v["tasks_per_day_limit"], 10, "{v}");
    let path = v["config_path"].as_str().expect("config_path string");
    assert!(path.ends_with("telegram.json"), "{path}");
    assert_eq!(path, telegram_json_path(&sb).display().to_string());
}

#[test]
fn telegram_requires_the_dashboard_token() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);
    assert_eq!(srv.request("GET", "/api/telegram", &[], b"").status, 401);
}

#[test]
fn telegram_error_insecure_file() {
    let sb = Sandbox::new();
    write_config(&sb, PAIRED_CHAT, 0o644);
    let srv = sb.serve(&[]);
    let v = status_of(&srv);
    assert_eq!(v["state"], "error", "{v}");
    let msg = v["message"].as_str().unwrap_or("");
    assert!(!msg.is_empty(), "{v}");
    assert!(msg.contains("chmod 600"), "{msg}");
    assert!(v["bot_username"].is_null(), "{v}");
    assert_no_token(&v.to_string(), "insecure");
}

#[test]
fn telegram_error_invalid_file() {
    let sb = Sandbox::new();
    write_raw_config(&sb, &format!(r#"{{"token":"{FAKE_TOKEN}"}}"#), 0o600);
    let srv = sb.serve(&[]);
    let v = status_of(&srv);
    assert_eq!(v["state"], "error", "{v}");
    assert!(
        v["message"].as_str().unwrap_or("").contains("không hợp lệ"),
        "{v}"
    );
    assert_no_token(&v.to_string(), "invalid");
}

#[test]
fn telegram_running_with_fake_api() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let srv = tg.serve(&sb, &[]);
    let v = wait_state(&srv, "running");
    assert_eq!(v["bot_username"], BOT_USERNAME, "{v}");
    assert_eq!(v["chat_id"], PAIRED_CHAT, "{v}");
    assert_eq!(v["paired_at"], 1, "{v}");
    assert!(v["message"].is_null(), "{v}");
    assert!(v["last_ok_at"].as_i64().unwrap_or(0) > 0, "{v}");
    assert_eq!(v["outbox_pending"], 0, "{v}");
    assert_no_token(&v.to_string(), "running");
}

#[test]
fn telegram_starting_while_getme_is_pending() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    tg.set_behavior(json!([{"method": "getMe", "times": 1, "delay_ms": 4000}]));
    write_config(&sb, PAIRED_CHAT, 0o600);
    let srv = tg.serve(&sb, &[]);
    let v = status_of(&srv);
    assert_eq!(v["state"], "starting", "{v}");
    assert!(v["bot_username"].is_null(), "{v}");
    assert_eq!(v["chat_id"], PAIRED_CHAT, "{v}");
    assert_no_token(&v.to_string(), "starting");
    let v = wait_state(&srv, "running");
    assert_eq!(v["bot_username"], BOT_USERNAME, "{v}");
}

#[test]
fn telegram_degraded_while_telegram_is_unreachable() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    tg.set_behavior(json!([{
        "method": "getMe", "times": 1000, "status": 500, "description": "Internal Server Error"
    }]));
    write_config(&sb, PAIRED_CHAT, 0o600);
    let srv = tg.serve(&sb, &[]);
    let v = wait_state(&srv, "degraded");
    assert!(!v["message"].as_str().unwrap_or("").is_empty(), "{v}");
    assert!(v["bot_username"].is_null(), "{v}");
    assert_no_token(&v.to_string(), "degraded");
}

#[test]
fn telegram_error_when_token_is_rejected() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    tg.set_behavior(json!([
        {"method": "getMe", "times": 1, "status": 401, "description": "Unauthorized"}
    ]));
    write_config(&sb, PAIRED_CHAT, 0o600);
    let srv = tg.serve(&sb, &[]);
    let v = wait_state(&srv, "error");
    assert!(v["message"].as_str().unwrap_or("").contains("401"), "{v}");
    assert_no_token(&v.to_string(), "rejected");
}

#[test]
fn telegram_never_leaks_token() {
    // Valid file, every state the bot passes through, plus the error states: the whole
    // response body is searched for the planted token and for its secret half.
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    tg.set_behavior(json!([{"method": "getMe", "times": 1, "delay_ms": 2000}]));
    write_config(&sb, PAIRED_CHAT, 0o600);
    let srv = tg.serve(&sb, &[]);
    let starting = srv.get("/api/telegram");
    assert_eq!(starting.status, 200);
    assert_no_token(&starting.text(), "starting");
    wait_state(&srv, "running");
    let running = srv.get("/api/telegram");
    assert_no_token(&running.text(), "running");
    for (name, value) in running.json().as_object().unwrap() {
        assert_no_token(&value.to_string(), name);
    }

    // The raw file content is only ever reported as chat_id / paired_at.
    let sb2 = Sandbox::new();
    write_raw_config(
        &sb2,
        &format!(r#"{{"token":"{FAKE_TOKEN}","chat_id":"nope","extra":"{FAKE_TOKEN}"}}"#),
        0o600,
    );
    let srv2 = sb2.serve(&[]);
    let body = srv2.get("/api/telegram").text();
    assert!(body.contains("\"state\":\"error\""), "{body}");
    assert_no_token(&body, "invalid chat_id");
}
