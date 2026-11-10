//! Phase 7 plan 07: the journal inside `agent-brain serve` (D-09, D-13, D-20) and its Telegram
//! delivery (JRN-04, JRN-05). Every test runs a real `serve` against the fake Telegram API and a
//! bare git remote in the sandbox; the clock comes from `AGENT_BRAIN_JOURNAL_NOW_MS`. Nothing
//! contacts api.telegram.org or github.com; all texts are invented.

#![allow(dead_code, unused_imports)]

mod common;
#[allow(dead_code)]
#[path = "support/journal.rs"]
mod jsupport;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use agent_brain::journal::NOW_ENV;
use common::{wait_until, Sandbox, Serve};
use jsupport::*;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tg::{write_config, FakeTelegram, PAIRED_CHAT};

const CONNECTED: &str = "Telegram: đã kết nối (@ab_fake_bot)";
const WEB_BASE: &str = "https://github.com/example-owner/example-vault";
const DAY_LINK: &str = "https://github.com/example-owner/example-vault/blob/main/Nh%E1%BA%ADt%20k%C3%BD/2026/01/2026-01-15.md";

fn stdout_of(srv: &Serve) -> String {
    srv.stdout.lock().unwrap().clone()
}

fn stderr_of(srv: &Serve) -> String {
    std::fs::read_to_string(&srv.stderr_path).unwrap_or_default()
}

/// Waits for the connected line and for the first long poll (after the start-up drain).
fn wait_polling(tg: &FakeTelegram, srv: &Serve) {
    assert!(
        wait_until(Duration::from_secs(10), || stdout_of(srv)
            .contains(CONNECTED)),
        "no connected line; stdout:\n{}\nstderr:\n{}",
        stdout_of(srv),
        stderr_of(srv)
    );
    assert!(
        wait_until(Duration::from_secs(10), || tg
            .calls("getUpdates")
            .iter()
            .any(|r| r["json"]["timeout"].as_i64().unwrap_or(0) > 0)),
        "no long poll; records:\n{:#?}\nstderr:\n{}",
        tg.records(),
        stderr_of(srv)
    );
}

fn text_of(rec: &Value) -> &str {
    rec["json"]["text"].as_str().unwrap_or("")
}

/// `sendMessage` records to the paired chat whose text satisfies `pred`.
fn messages(tg: &FakeTelegram, pred: impl Fn(&str) -> bool) -> Vec<Value> {
    tg.calls("sendMessage")
        .into_iter()
        .filter(|r| r["json"]["chat_id"] == json!(PAIRED_CHAT) && pred(text_of(r)))
        .collect()
}

/// Waits for the first `sendMessage` to the paired chat satisfying `pred`.
fn wait_message(
    tg: &FakeTelegram,
    srv: &Serve,
    secs: u64,
    what: &str,
    pred: impl Fn(&str) -> bool,
) -> Value {
    let mut found = None;
    wait_until(Duration::from_secs(secs), || {
        found = messages(tg, &pred).into_iter().next();
        found.is_some()
    });
    match found {
        Some(r) => r,
        None => panic!(
            "no sendMessage {what}; records:\n{:#?}\nstdout:\n{}\nstderr:\n{}",
            tg.records(),
            stdout_of(srv),
            stderr_of(srv)
        ),
    }
}

fn insert_mail(conn: &Connection, kind: &str, key: &str, text: &str) {
    conn.execute(
        "INSERT INTO journal_mail (kind, dedupe_key, text, created_at) VALUES (?1, ?2, ?3, 1)",
        params![kind, key, text],
    )
    .unwrap();
}

fn db(sb: &Sandbox) -> Connection {
    let conn = Connection::open(&sb.db_path).unwrap();
    conn.busy_timeout(Duration::from_secs(5)).unwrap();
    conn
}

fn mail_sent_at(sb: &Sandbox, key: &str) -> Option<i64> {
    db(sb)
        .query_row(
            "SELECT sent_at FROM journal_mail WHERE dedupe_key = ?1",
            params![key],
            |r| r.get::<_, Option<i64>>(0),
        )
        .unwrap_or(None)
}

fn mail_count(sb: &Sandbox, where_sql: &str) -> i64 {
    db(sb)
        .query_row(
            &format!("SELECT count(*) FROM journal_mail WHERE {where_sql}"),
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
}

fn state_value(sb: &Sandbox, key: &str) -> Option<String> {
    db(sb)
        .query_row(
            "SELECT value FROM journal_state WHERE key = ?1",
            params![key],
            |r| r.get::<_, String>(0),
        )
        .ok()
}

fn loop_ticks(sb: &Sandbox) -> i64 {
    state_value(sb, agent_brain::journal::loop_::KEY_LOOP_TICKS)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn stop(mut srv: Serve) {
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// Task 2: delivery of journal_mail rows
// ---------------------------------------------------------------------------

#[test]
fn mail_delivered_once() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let text = "Nhật ký: thử hộp thư";
    {
        let conn = agent_brain::db::open(&sb.db_path).unwrap();
        insert_mail(&conn, "pushfail", "pushfail:1", text);
    }

    let srv = tg.serve(&sb, &[]);
    wait_message(&tg, &srv, 10, "with the queued text", |t| t == text);
    assert_eq!(messages(&tg, |t| t == text).len(), 1);
    assert!(
        wait_until(Duration::from_secs(5), || mail_sent_at(&sb, "pushfail:1")
            .is_some()),
        "sent_at not set"
    );
    stop(srv);

    let srv2 = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv2);
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(
        messages(&tg, |t| t == text).len(),
        1,
        "delivered again after a restart; stderr:\n{}",
        stderr_of(&srv2)
    );
    stop(srv2);
}

#[test]
fn stale_digest_dropped() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    sb.write_config(json!({"journal": {"utc_offset_minutes": 420}}));
    let text = "Nhật ký Thứ Tư 14/01/2026: thử";
    {
        let conn = agent_brain::db::open(&sb.db_path).unwrap();
        insert_mail(&conn, "digest", "digest:2026-01-14", text);
    }

    let now = local_ms(2026, 1, 15, 10, 0, 0).to_string();
    let srv = tg.serve(&sb, &[(NOW_ENV, now.as_str())]);
    wait_polling(&tg, &srv);
    assert!(
        wait_until(Duration::from_secs(10), || mail_sent_at(
            &sb,
            "digest:2026-01-14"
        )
        .is_some()),
        "stale digest row not claimed; stderr:\n{}",
        stderr_of(&srv)
    );
    std::thread::sleep(Duration::from_secs(3));
    assert!(
        messages(&tg, |t| t.contains("14/01/2026: thử")).is_empty(),
        "a late digest was sent: {:#?}",
        tg.calls("sendMessage")
    );
    assert!(
        stderr_of(&srv).contains("bỏ digest"),
        "no drop warning; stderr:\n{}",
        stderr_of(&srv)
    );
    stop(srv);
}

#[test]
fn mail_inserted_while_running_is_delivered() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let text = "Nhật ký: đã đẩy lại được lên repo vault";
    {
        let conn = agent_brain::db::open(&sb.db_path).unwrap();
        insert_mail(&conn, "recovered", "recovered:1", text);
    }
    wait_message(&tg, &srv, 10, "with the recovered text", |t| t == text);
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(messages(&tg, |t| t == text).len(), 1);
    assert!(mail_sent_at(&sb, "recovered:1").is_some());
    stop(srv);
}
