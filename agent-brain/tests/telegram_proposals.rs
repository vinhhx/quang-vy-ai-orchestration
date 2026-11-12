//! Proposals of Telegram-created intakes are sent to Telegram with `Duyệt đề xuất` / `Từ chối`
//! buttons. Nothing here contacts api.telegram.org: every test talks to the local fake Telegram
//! (fake token, temp dirs).

#![allow(dead_code, unused_imports)]

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use common::{wait_until, Sandbox, Serve};
use protocol::setup_project;
use serde_json::{json, Value};
use std::time::Duration;
use tg::{write_config, FakeTelegram, PAIRED_CHAT, STRANGER_CHAT};

const CONNECTED: &str = "Telegram: đã kết nối (@ab_fake_bot)";
const APPROVE: &str = "Duyệt đề xuất";
const REJECT: &str = "Từ chối";

fn stdout_of(srv: &Serve) -> String {
    srv.stdout.lock().unwrap().clone()
}

fn stderr_of(srv: &Serve) -> String {
    std::fs::read_to_string(&srv.stderr_path).unwrap_or_default()
}

fn wait_polling(tg: &FakeTelegram, srv: &Serve) {
    assert!(
        wait_until(Duration::from_secs(10), || stdout_of(srv).contains(CONNECTED)),
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

fn buttons(rec: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(rows) = rec["json"]["reply_markup"]["inline_keyboard"].as_array() {
        for row in rows {
            for b in row.as_array().into_iter().flatten() {
                out.push((
                    b["text"].as_str().unwrap_or("").to_string(),
                    b["callback_data"].as_str().unwrap_or("").to_string(),
                ));
            }
        }
    }
    out
}

fn button_data(rec: &Value, label: &str) -> String {
    buttons(rec)
        .into_iter()
        .find(|(l, _)| l == label)
        .map(|(_, d)| d)
        .unwrap_or_else(|| panic!("no {label:?} button on {rec}"))
}

fn message_id_of(rec: &Value) -> i64 {
    rec["response"]["result"]["message_id"]
        .as_i64()
        .unwrap_or_else(|| panic!("no message_id in {rec}"))
}

fn empty_keyboard(rec: &Value) -> bool {
    rec["json"]["reply_markup"]["inline_keyboard"] == json!([])
}

/// Presses `data` on message `message_id` from `chat`; returns the callback id.
fn press_data(tg: &FakeTelegram, chat: i64, message_id: i64, text: &str, data: &str) -> String {
    let up = tg.callback_update(chat, chat, message_id, text, data);
    let id = up["callback_query"]["id"].as_str().unwrap().to_string();
    tg.push_update(up);
    id
}

/// Presses button `label` of the sent message `rec` from the paired chat.
fn press(tg: &FakeTelegram, rec: &Value, label: &str) -> String {
    press_data(
        tg,
        PAIRED_CHAT,
        message_id_of(rec),
        text_of(rec),
        &button_data(rec, label),
    )
}

fn answers_for(tg: &FakeTelegram, callback_id: &str) -> Vec<Value> {
    tg.calls("answerCallbackQuery")
        .into_iter()
        .filter(|r| r["json"]["callback_query_id"].as_str() == Some(callback_id))
        .collect()
}

/// The text of the answer to `callback_id`.
fn wait_answer_text(tg: &FakeTelegram, srv: &Serve, callback_id: &str) -> String {
    match tg.wait_call("answerCallbackQuery", Duration::from_secs(10), |r| {
        r["json"]["callback_query_id"].as_str() == Some(callback_id)
    }) {
        Some(r) => r["json"]["text"].as_str().unwrap_or("").to_string(),
        None => panic!(
            "no answerCallbackQuery for {callback_id}; records:\n{:#?}\nstdout:\n{}\nstderr:\n{}",
            tg.records(),
            stdout_of(srv),
            stderr_of(srv)
        ),
    }
}

fn wait_edit(
    tg: &FakeTelegram,
    srv: &Serve,
    message_id: i64,
    pred: impl Fn(&Value) -> bool,
) -> Value {
    match tg.wait_call("editMessageText", Duration::from_secs(10), |r| {
        r["json"]["message_id"].as_i64() == Some(message_id) && pred(r)
    }) {
        Some(r) => r,
        None => panic!(
            "no matching editMessageText for message {message_id}; records:\n{:#?}\nstdout:\n{}\nstderr:\n{}",
            tg.records(),
            stdout_of(srv),
            stderr_of(srv)
        ),
    }
}

fn edits_of(tg: &FakeTelegram, message_id: i64) -> usize {
    tg.calls("editMessageText")
        .into_iter()
        .filter(|r| r["json"]["message_id"].as_i64() == Some(message_id))
        .count()
}

fn wait_proposal_message(tg: &FakeTelegram, srv: &Serve, id: i64) -> Value {
    let prefix = format!("Đề xuất #{id} —");
    match tg.wait_call("sendMessage", Duration::from_secs(15), |r| {
        text_of(r).starts_with(&prefix)
    }) {
        Some(r) => r,
        None => panic!(
            "no proposal message {prefix:?}; records:\n{:#?}\nstdout:\n{}\nstderr:\n{}",
            tg.records(),
            stdout_of(srv),
            stderr_of(srv)
        ),
    }
}

fn proposal_messages(tg: &FakeTelegram, id: i64) -> usize {
    let prefix = format!("Đề xuất #{id} —");
    tg.calls("sendMessage")
        .iter()
        .filter(|r| text_of(r).starts_with(&prefix))
        .count()
}

/// Sets the top-level `telegram_projects` allowlist of projects_map.json.
fn allow_projects(sb: &Sandbox, ids: &[&str]) {
    let mut map = sb.read_map();
    map["telegram_projects"] = json!(ids);
    std::fs::write(sb.map_path(), serde_json::to_string_pretty(&map).unwrap()).expect("write map");
}

/// Project `web-ban-hang` in the allowlist, paired fake Telegram, serve running and polling.
fn world() -> (Sandbox, FakeTelegram, Serve) {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);
    let srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    (sb, tg, srv)
}

/// An intake of `source` in state `proposed` with one `pending` proposal on `conn`; returns the
/// proposal id.
fn insert_on(
    conn: &rusqlite::Connection,
    source: &str,
    title: &str,
    team: &str,
    after: Option<i64>,
) -> i64 {
    let proj = agent_brain::db::project_row_id(conn, "web-ban-hang").unwrap();
    let intake =
        agent_brain::db::insert_intake(conn, source, Some(proj), "yêu cầu", "normal", true)
            .unwrap();
    agent_brain::db::set_intake_status_error(conn, intake, "proposed", None).unwrap();
    let id = agent_brain::db::insert_proposal(
        conn,
        &agent_brain::db::NewProposal {
            intake_id: intake,
            project_id: proj,
            title,
            request: "Dựng trang chủ mới",
            priority: "normal",
            level: "vua",
            team,
            workflow: "mac-dinh",
            reason: Some("Điều phối đề xuất"),
        },
    )
    .unwrap();
    if after.is_some() {
        assert!(agent_brain::db::set_proposal_after_id(conn, id, after).unwrap());
    }
    id
}

fn add_proposal(sb: &Sandbox, source: &str, title: &str, team: &str) -> i64 {
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    insert_on(&conn, source, title, team, None)
}

fn status_of(sb: &Sandbox, id: i64) -> String {
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    agent_brain::db::get_proposal(&conn, id)
        .unwrap()
        .unwrap()
        .status
}

fn quiet() {
    std::thread::sleep(Duration::from_secs(2));
}

#[test]
fn telegram_proposal_is_sent_once_with_two_buttons() {
    let (sb, tg, mut srv) = world();
    let id = add_proposal(&sb, "telegram", "Tạo trang chủ", "mac-dinh");
    let msg = wait_proposal_message(&tg, &srv, id);
    let text = text_of(&msg);
    assert!(text.starts_with(&format!("Đề xuất #{id} — dự án web-ban-hang")), "{text}");
    assert!(text.contains("Tiêu đề: Tạo trang chủ"), "{text}");
    assert!(text.contains("Đội hình: mac-dinh"), "{text}");
    assert!(text.contains("Quy trình: mac-dinh"), "{text}");
    assert!(text.contains("Ưu tiên: normal"), "{text}");
    assert!(text.contains("Lý do: Điều phối đề xuất"), "{text}");
    assert!(!text.contains("Chạy sau"), "{text}");
    assert!(!text.contains(sb.root.to_str().unwrap()), "absolute path in {text}");
    assert_eq!(msg["json"]["chat_id"], PAIRED_CHAT);
    let labels: Vec<String> = buttons(&msg).into_iter().map(|(l, _)| l).collect();
    assert_eq!(labels, vec![APPROVE.to_string(), REJECT.to_string()]);
    assert!(button_data(&msg, APPROVE).starts_with(&format!("w:{id}:")));
    assert!(button_data(&msg, REJECT).starts_with(&format!("z:{id}:")));
    // A later scan does not repeat it.
    quiet();
    assert_eq!(proposal_messages(&tg, id), 1);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn dashboard_proposal_is_not_sent() {
    let (sb, tg, mut srv) = world();
    let ui = add_proposal(&sb, "ui", "Việc dashboard", "mac-dinh");
    let tgram = add_proposal(&sb, "telegram", "Việc telegram", "mac-dinh");
    wait_proposal_message(&tg, &srv, tgram);
    quiet();
    assert_eq!(proposal_messages(&tg, ui), 0, "dashboard proposal sent to Telegram");
    assert_eq!(proposal_messages(&tg, tgram), 1);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn restart_does_not_resend_existing_proposals() {
    let (sb, tg, mut srv) = world();
    let id = add_proposal(&sb, "telegram", "Tạo trang chủ", "mac-dinh");
    wait_proposal_message(&tg, &srv, id);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));

    let mut again = tg.serve(&sb, &[]);
    wait_polling(&tg, &again);
    // A proposal added while serve runs again still goes out; the old one does not.
    let fresh = add_proposal(&sb, "telegram", "Việc mới", "mac-dinh");
    wait_proposal_message(&tg, &again, fresh);
    quiet();
    assert_eq!(proposal_messages(&tg, id), 1, "resent after restart");
    assert_eq!(proposal_messages(&tg, fresh), 1);
    assert_eq!(again.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn approve_button_approves_and_closes_the_message() {
    let (sb, tg, mut srv) = world();
    let id = add_proposal(&sb, "telegram", "Tạo trang chủ", "mac-dinh");
    let msg = wait_proposal_message(&tg, &srv, id);
    let mid = message_id_of(&msg);

    let cb = press(&tg, &msg, APPROVE);
    assert_eq!(wait_answer_text(&tg, &srv, &cb), "Đã duyệt");
    let edit = wait_edit(&tg, &srv, mid, |r| {
        text_of(r).contains("✓ Đã duyệt từ Telegram lúc ")
    });
    assert!(text_of(&edit).starts_with(text_of(&msg)), "{edit}");
    assert!(empty_keyboard(&edit), "{edit}");
    assert_eq!(status_of(&sb, id), "approved");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn reject_button_rejects() {
    let (sb, tg, mut srv) = world();
    let id = add_proposal(&sb, "telegram", "Tạo trang chủ", "mac-dinh");
    let msg = wait_proposal_message(&tg, &srv, id);
    let mid = message_id_of(&msg);

    let cb = press(&tg, &msg, REJECT);
    assert_eq!(wait_answer_text(&tg, &srv, &cb), "Đã từ chối");
    let edit = wait_edit(&tg, &srv, mid, |r| {
        text_of(r).contains("✗ Đã từ chối từ Telegram lúc ")
    });
    assert!(empty_keyboard(&edit), "{edit}");
    assert_eq!(status_of(&sb, id), "rejected");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn second_press_and_dashboard_first_change_nothing() {
    let (sb, tg, mut srv) = world();
    let id = add_proposal(&sb, "telegram", "Tạo trang chủ", "mac-dinh");
    let msg = wait_proposal_message(&tg, &srv, id);
    let mid = message_id_of(&msg);

    let first = press(&tg, &msg, APPROVE);
    assert_eq!(wait_answer_text(&tg, &srv, &first), "Đã duyệt");
    wait_edit(&tg, &srv, mid, |r| text_of(r).contains("✓ Đã duyệt từ Telegram"));
    // Second press on the same (old) buttons, now also trying to reject.
    let second = press(&tg, &msg, APPROVE);
    let ans = wait_answer_text(&tg, &srv, &second);
    assert!(ans.starts_with("Đề xuất đã được xử lý (approved) ở nơi khác"), "{ans}");
    let third = press(&tg, &msg, REJECT);
    let ans = wait_answer_text(&tg, &srv, &third);
    assert!(ans.starts_with("Đề xuất đã được xử lý (approved) ở nơi khác"), "{ans}");
    assert_eq!(status_of(&sb, id), "approved");

    // Handled on the dashboard before the press.
    let other = add_proposal(&sb, "telegram", "Việc khác", "mac-dinh");
    let msg2 = wait_proposal_message(&tg, &srv, other);
    let resp = srv.post_json(&format!("/api/proposals/{other}/reject"), &json!({}));
    assert_eq!(resp.status, 200, "{}", resp.text());
    let cb = press(&tg, &msg2, APPROVE);
    let ans = wait_answer_text(&tg, &srv, &cb);
    assert!(ans.starts_with("Đề xuất đã được xử lý (rejected) ở nơi khác"), "{ans}");
    let edit = wait_edit(&tg, &srv, message_id_of(&msg2), |r| {
        text_of(r).contains("đã xử lý ngoài Telegram")
    });
    assert!(empty_keyboard(&edit), "{edit}");
    assert_eq!(status_of(&sb, other), "rejected");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn old_rev_is_refused() {
    let (sb, tg, mut srv) = world();
    let id = add_proposal(&sb, "telegram", "Tạo trang chủ", "mac-dinh");
    let msg = wait_proposal_message(&tg, &srv, id);
    let mid = message_id_of(&msg);

    // The proposal is edited (title) between sending and pressing.
    sb.db()
        .execute("UPDATE proposals SET title = 'Đã sửa' WHERE id = ?1", [id])
        .unwrap();
    let cb = press(&tg, &msg, APPROVE);
    let ans = wait_answer_text(&tg, &srv, &cb);
    assert!(ans.starts_with("Nút cũ — đề xuất đã đổi"), "{ans}");
    let edit = wait_edit(&tg, &srv, mid, |r| text_of(r).contains("nút cũ"));
    assert!(empty_keyboard(&edit), "{edit}");
    assert_eq!(status_of(&sb, id), "pending");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn approve_refused_when_project_leaves_the_allowlist() {
    let (sb, tg, mut srv) = world();
    let id = add_proposal(&sb, "telegram", "Tạo trang chủ", "mac-dinh");
    let msg = wait_proposal_message(&tg, &srv, id);
    let mid = message_id_of(&msg);

    allow_projects(&sb, &[]);
    let cb = press(&tg, &msg, APPROVE);
    let ans = wait_answer_text(&tg, &srv, &cb);
    assert!(ans.starts_with("Dự án không còn trong danh sách Telegram"), "{ans}");
    quiet();
    assert_eq!(status_of(&sb, id), "pending");
    assert_eq!(edits_of(&tg, mid), 0, "the message must keep its buttons");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn validation_error_keeps_the_buttons() {
    let (sb, tg, mut srv) = world();
    let id = add_proposal(&sb, "telegram", "Tạo trang chủ", "khong-co-doi-nay");
    let msg = wait_proposal_message(&tg, &srv, id);
    let mid = message_id_of(&msg);

    let cb = press(&tg, &msg, APPROVE);
    let ans = wait_answer_text(&tg, &srv, &cb);
    assert!(ans.contains("khong-co-doi-nay"), "{ans}");
    quiet();
    assert_eq!(status_of(&sb, id), "pending");
    assert_eq!(edits_of(&tg, mid), 0);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn stranger_chat_is_ignored() {
    let (sb, tg, mut srv) = world();
    let id = add_proposal(&sb, "telegram", "Tạo trang chủ", "mac-dinh");
    let msg = wait_proposal_message(&tg, &srv, id);
    let mid = message_id_of(&msg);

    let stranger = press_data(
        &tg,
        STRANGER_CHAT,
        mid,
        text_of(&msg),
        &button_data(&msg, APPROVE),
    );
    // A press from the paired chat after it proves the stranger's press was processed (or not).
    let marker = press_data(&tg, PAIRED_CHAT, mid, text_of(&msg), "w:abc");
    wait_answer_text(&tg, &srv, &marker);
    assert!(answers_for(&tg, &stranger).is_empty());
    assert_eq!(status_of(&sb, id), "pending");
    assert_eq!(edits_of(&tg, mid), 0);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn malformed_data_is_an_invalid_button() {
    let (sb, tg, mut srv) = world();
    let id = add_proposal(&sb, "telegram", "Tạo trang chủ", "mac-dinh");
    let msg = wait_proposal_message(&tg, &srv, id);
    let mid = message_id_of(&msg);
    let good = button_data(&msg, APPROVE);
    let rev = good.rsplit(':').next().unwrap().to_string();

    for bad in [
        "w:abc".to_string(),
        format!("w:{id}"),
        format!("w:0:{rev}"),
        format!("z:{id}:{}", &rev[..15]),
        format!("w:{id}:{rev}:extra"),
        format!("w:{id}:{}", rev.to_uppercase().replace('0', "G")),
    ] {
        let cb = press_data(&tg, PAIRED_CHAT, mid, text_of(&msg), &bad);
        let ans = wait_answer_text(&tg, &srv, &cb);
        assert_eq!(ans, agent_brain::telegram::bot::INVALID_BUTTON, "{bad}");
    }
    assert_eq!(status_of(&sb, id), "pending");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn proposal_after_another_shows_the_dependency() {
    let (sb, tg, mut srv) = world();
    // One transaction, so a scan sees both proposals with `after` already set.
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    let first = insert_on(&conn, "telegram", "Bước một", "mac-dinh", None);
    let second = insert_on(&conn, "telegram", "Bước hai", "mac-dinh", Some(first));
    conn.execute_batch("COMMIT").unwrap();
    drop(conn);
    let m1 = wait_proposal_message(&tg, &srv, first);
    let m2 = wait_proposal_message(&tg, &srv, second);
    assert!(!text_of(&m1).contains("Chạy sau"), "{m1}");
    assert!(
        text_of(&m2).contains(&format!("Chạy sau đề xuất #{first}")),
        "{m2}"
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}
