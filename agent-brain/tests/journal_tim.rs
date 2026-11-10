//! Phase 7 plan 06: `/tim <từ khóa>` end to end through `serve` and the fake Telegram API.
//! The index comes from a real `journal export --no-push`; nothing contacts api.telegram.org.
//! Texts and projects are invented (`du-an-a`, `du-an-b`).

#![allow(dead_code, unused_imports)]

mod common;
#[allow(dead_code)]
#[path = "support/journal.rs"]
mod jsupport;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use common::{wait_until, Sandbox, Serve};
use jsupport::*;
use rusqlite::params;
use serde_json::{json, Value};
use std::time::Duration;
use tg::{write_config, FakeTelegram, PAIRED_CHAT, STRANGER_CHAT};

const CONNECTED: &str = "Telegram: đã kết nối (@ab_fake_bot)";
const WEB_BASE: &str = "https://github.com/example-owner/example-vault";
const DAY_15_LINK: &str = "https://github.com/example-owner/example-vault/blob/main/Nh%E1%BA%ADt%20k%C3%BD/2026/01/2026-01-15.md";
const NOT_FOUND: &str = "Không tìm thấy mục nào trong nhật ký cho: ";

struct Fixture {
    // Kept alive for the whole test: the sandbox owns every temp directory.
    sb: Sandbox,
    tg: FakeTelegram,
    srv: Serve,
}

impl Fixture {
    fn finish(mut self) {
        self.srv.stop(libc::SIGINT, Duration::from_secs(10));
    }
}

fn stdout_of(srv: &Serve) -> String {
    srv.stdout.lock().unwrap().clone()
}

fn stderr_of(srv: &Serve) -> String {
    std::fs::read_to_string(&srv.stderr_path).unwrap_or_default()
}

/// Waits for the connected line and the first long poll, so later updates are delivered.
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

fn sent_to(tg: &FakeTelegram, chat: i64) -> Vec<Value> {
    tg.calls("sendMessage")
        .into_iter()
        .filter(|r| r["json"]["chat_id"].as_i64() == Some(chat))
        .collect()
}

/// Sends `text` from the paired chat and returns the text of the one reply it produces.
fn ask(f: &Fixture, text: &str) -> String {
    let before = sent_to(&f.tg, PAIRED_CHAT).len();
    f.tg.push_update(f.tg.message_update(PAIRED_CHAT, PAIRED_CHAT, "private", text));
    let mut reply = None;
    wait_until(Duration::from_secs(15), || {
        let all = sent_to(&f.tg, PAIRED_CHAT);
        if all.len() > before {
            reply = Some(text_of(&all[before]).to_string());
            true
        } else {
            false
        }
    });
    match reply {
        Some(r) => r,
        None => panic!(
            "no reply to {text:?}; records:\n{:#?}\nstderr:\n{}",
            f.tg.records(),
            stderr_of(&f.srv)
        ),
    }
}

/// Tasks A (du-an-a, 15/01), B (du-an-b, 14/01) and C (du-an-a, 15/01; a long request whose word
/// `bimatsauky` lies after the 140-character cut, and a dev run directory holding a plan file with
/// `kehoachbimat`). The index is built by a real export, then serve runs against the fake.
fn setup() -> Fixture {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    let mut section = journal_section(&remote);
    section["web_base_url"] = json!(WEB_BASE);
    sb.write_config(json!({ "journal": section }));
    let conn = setup_projects(&sb, &[("du-an-a", None), ("du-an-b", None)]);

    let title_a = "Sửa lỗi đăng nhập trang quản trị";
    let a = seed_task(&conn, "du-an-a", "sua-loi-dang-nhap", title_a);
    seed_proposal(&conn, a, title_a);
    seed_run(
        &conn,
        a,
        "dev",
        "succeeded",
        None,
        local_ms(2026, 1, 15, 9, 0, 0),
        Some(local_ms(2026, 1, 15, 9, 20, 0)),
        Some(1),
    );

    let title_b = "Tìm kiếm khách hàng theo số điện thoại";
    let b = seed_task(&conn, "du-an-b", "tim-khach-hang", title_b);
    seed_proposal(&conn, b, title_b);
    seed_run(
        &conn,
        b,
        "dev",
        "succeeded",
        None,
        local_ms(2026, 1, 14, 10, 0, 0),
        Some(local_ms(2026, 1, 14, 10, 30, 0)),
        Some(1),
    );

    let filler = "Rà soát tài liệu hướng dẫn cài đặt môi trường ".repeat(5);
    assert!(filler.chars().count() > 200);
    let request = format!("{filler}bimatsauky");
    let c = seed_task(&conn, "du-an-a", "ra-soat-tai-lieu", &request);
    let run_c = seed_run(
        &conn,
        c,
        "dev",
        "succeeded",
        None,
        local_ms(2026, 1, 15, 11, 0, 0),
        Some(local_ms(2026, 1, 15, 11, 30, 0)),
        Some(1),
    );
    let run_dir = sb.root.join("run-c");
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::write(
        run_dir.join(".plan.md"),
        "# Kế hoạch\nTừ khóa riêng của kế hoạch: kehoachbimat\n",
    )
    .unwrap();
    conn.execute(
        "UPDATE runs SET run_dir = ?1 WHERE id = ?2",
        params![run_dir.to_str().unwrap(), run_c],
    )
    .unwrap();
    drop(conn);

    let out = export(&sb, &["--no-push"], local_ms(2026, 1, 15, 21, 0, 0));
    assert_eq!(out.code, 0, "{}", out.stderr);

    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    Fixture { sb, tg, srv }
}

#[test]
fn tim_finds_by_folded_words() {
    let f = setup();
    let reply = ask(&f, "/tim dang nhap");
    assert!(
        reply.contains("du-an-a — Sửa lỗi đăng nhập trang quản trị"),
        "{reply}"
    );
    assert!(reply.contains(DAY_15_LINK), "{reply}");
    assert!(reply.contains("15/01/2026"), "{reply}");

    let reply = ask(&f, "/tim tim");
    assert!(
        reply.contains("Tìm kiếm khách hàng theo số điện thoại"),
        "{reply}"
    );
    assert!(reply.contains("14/01/2026"), "{reply}");
    f.finish();
}

#[test]
fn tim_never_returns_text_outside_the_vault() {
    let f = setup();
    for word in ["bimatsauky", "kehoachbimat"] {
        let reply = ask(&f, &format!("/tim {word}"));
        assert!(reply.starts_with(NOT_FOUND), "{word}: {reply}");
    }
    f.finish();
}

#[test]
fn tim_hostile_queries_and_usage() {
    let f = setup();
    for q in ["title:sua AND (", "\"unterminated", "*", "NOT x", "50% off"] {
        let reply = ask(&f, &format!("/tim {q}"));
        assert!(!reply.starts_with("Lỗi"), "{q}: {reply}");
    }
    let reply = ask(&f, "/tim");
    assert_eq!(reply, "Cách dùng: /tim <từ khóa>");
    let reply = ask(&f, "/tim dang");
    assert!(
        reply.contains("du-an-a — Sửa lỗi đăng nhập trang quản trị"),
        "{reply}"
    );
    f.finish();
}

#[test]
fn tim_from_stranger_gets_no_reply() {
    let f = setup();
    f.tg.push_update(f.tg.message_update(STRANGER_CHAT, STRANGER_CHAT, "private", "/tim dang"));
    std::thread::sleep(Duration::from_secs(3));
    assert!(
        sent_to(&f.tg, STRANGER_CHAT).is_empty(),
        "stranger got a reply: {:#?}",
        sent_to(&f.tg, STRANGER_CHAT)
    );
    assert!(
        !f.tg
            .calls("sendMessage")
            .iter()
            .any(|r| text_of(r).contains("du-an-a — Sửa lỗi")),
        "journal text sent after a stranger's /tim"
    );
    // The bot is alive and has processed the stranger's update: the paired chat still works.
    let reply = ask(&f, "/tim dang");
    assert!(reply.contains("du-an-a — Sửa lỗi"), "{reply}");
    f.finish();
}
