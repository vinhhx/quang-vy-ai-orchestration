//! Merge & push from Telegram: the `QA đạt` message carries the button; the first press only
//! asks, `Xác nhận` runs the dashboard's own `merge_push` control once, `Hủy` changes nothing, an
//! old message cannot run it again and a stranger chat cannot run it at all. The bot talks to the
//! local fake Telegram; `origin` is a local bare repository.

mod common;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use common::{wait_until, Sandbox, Serve};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tg::{write_config, FakeTelegram, PAIRED_CHAT, STRANGER_CHAT};

const CONNECTED: &str = "Telegram: đã kết nối (@ab_fake_bot)";

fn stdout_of(srv: &Serve) -> String {
    srv.stdout.lock().unwrap().clone()
}

fn stderr_of(srv: &Serve) -> String {
    std::fs::read_to_string(&srv.stderr_path).unwrap_or_default()
}

fn text_of(rec: &Value) -> &str {
    rec["json"]["text"].as_str().unwrap_or("")
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
        "no long poll; stderr:\n{}",
        stderr_of(srv)
    );
}

fn wait_message(tg: &FakeTelegram, srv: &Serve, prefix: &str, skip: usize) -> Value {
    let mut found = None;
    wait_until(Duration::from_secs(20), || {
        found = tg
            .calls("sendMessage")
            .into_iter()
            .filter(|r| text_of(r).starts_with(prefix))
            .nth(skip);
        found.is_some()
    });
    found.unwrap_or_else(|| {
        panic!(
            "no sendMessage #{skip} starting with {prefix:?}; records:\n{:#?}\nstderr:\n{}",
            tg.records(),
            stderr_of(srv)
        )
    })
}

fn wait_answer(tg: &FakeTelegram, srv: &Serve, callback_id: &str) -> Value {
    tg.wait_call("answerCallbackQuery", Duration::from_secs(20), |r| {
        r["json"]["callback_query_id"].as_str() == Some(callback_id)
    })
    .unwrap_or_else(|| {
        panic!(
            "no answer for {callback_id}; records:\n{:#?}\nstderr:\n{}",
            tg.records(),
            stderr_of(srv)
        )
    })
}

fn wait_edit(tg: &FakeTelegram, srv: &Serve, message_id: i64, pred: impl Fn(&Value) -> bool) -> Value {
    tg.wait_call("editMessageText", Duration::from_secs(20), |r| {
        r["json"]["message_id"].as_i64() == Some(message_id) && pred(r)
    })
    .unwrap_or_else(|| {
        panic!(
            "no matching edit of {message_id}; records:\n{:#?}\nstderr:\n{}",
            tg.records(),
            stderr_of(srv)
        )
    })
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

fn data_of(rec: &Value, label: &str) -> String {
    buttons(rec)
        .into_iter()
        .find(|(l, _)| l == label)
        .map(|(_, d)| d)
        .unwrap_or_else(|| panic!("no {label:?} button on {rec}"))
}

/// Pushes a press of `data` on message `message_id` from `chat`; returns the callback id.
fn press(tg: &FakeTelegram, chat: i64, message_id: i64, text: &str, data: &str) -> String {
    let up = tg.callback_update(chat, chat, message_id, text, data);
    let id = up["callback_query"]["id"].as_str().unwrap().to_string();
    tg.push_update(up);
    id
}

/// Waits until the bot asked for updates past `update_id` (it was handled or ignored).
fn wait_handled(tg: &FakeTelegram, srv: &Serve, update_id: i64) {
    assert!(
        wait_until(Duration::from_secs(20), || tg
            .calls("getUpdates")
            .iter()
            .any(|r| r["json"]["offset"].as_i64().unwrap_or(0) > update_id)),
        "update {update_id} not consumed; stderr:\n{}",
        stderr_of(srv)
    );
}

fn git_sha(sb: &Sandbox, dir: &Path, rev: &str) -> String {
    sb.git(dir, &["rev-parse", rev]).trim().to_string()
}

fn has_branch(dir: &Path, b: &str, sb: &Sandbox) -> bool {
    sb.git(dir, &["branch", "--list", b]).trim() != ""
}

/// Repo `web` with a bare `origin` and `dev` pushed; task 1 on `task/sua-app` (one commit,
/// checked out), status `dev_done` until the test flips it to `qa_pass`.
fn setup(sb: &Sandbox) -> (PathBuf, PathBuf) {
    let remote = sb.root.join("remote.git");
    std::fs::create_dir_all(&remote).unwrap();
    sb.git(&remote, &["init", "-q", "--bare", "-b", "main"]);
    let repo = sb.make_repo("web", &[("src/a.txt", "a\n")], None, Some(remote.to_str().unwrap()));
    sb.git(&repo, &["checkout", "-q", "-b", "dev"]);
    sb.git(&repo, &["push", "-q", "origin", "main", "dev"]);
    sb.git(&repo, &["checkout", "-q", "-b", "task/sua-app"]);
    std::fs::write(repo.join("src/a.txt"), "a2\n").unwrap();
    sb.git(&repo, &["commit", "-q", "-am", "sua-app: sửa a"]);
    let entry = sb.project_entry(&repo, "web", "work");
    sb.write_map(json!([entry]), &[]);
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let now = agent_brain::db::now_ms();
    conn.execute(
        "INSERT INTO projects (id, slug, path) VALUES (1, 'web', ?1)",
        rusqlite::params![repo.to_str().unwrap()],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, status, created_at, updated_at)
         VALUES (1, 1, 'sua-app', 'Sửa app', 'task/sua-app', 'dev', 'dev_done', ?1, ?1)",
        rusqlite::params![now],
    )
    .unwrap();
    (repo, remote)
}

fn qa_pass(sb: &Sandbox) {
    sb.db()
        .execute(
            "UPDATE tasks SET status = 'qa_pass', updated_at = ?1 WHERE id = 1",
            rusqlite::params![agent_brain::db::now_ms()],
        )
        .unwrap();
}

#[test]
fn merge_push_from_telegram_asks_then_runs_once() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let (repo, remote) = setup(&sb);
    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    qa_pass(&sb);
    let msg = wait_message(&tg, &srv, "QA đạt · task 1", 0);
    let labels: Vec<String> = buttons(&msg).into_iter().map(|(l, _)| l).collect();
    assert_eq!(labels, vec!["Merge & push", "Mở lại task"], "{msg}");
    let m = msg["response"]["result"]["message_id"].as_i64().expect("message id");
    let qa_text = text_of(&msg).to_string();
    let ask = data_of(&msg, "Merge & push");
    assert!(ask.starts_with("g:1:"), "{ask}");
    let origin_before = git_sha(&sb, &remote, "dev");

    // A stranger's press is dropped before anything is read.
    let up = tg.callback_update(STRANGER_CHAT, STRANGER_CHAT, m, &qa_text, &ask);
    let stranger_cb = up["callback_query"]["id"].as_str().unwrap().to_string();
    let uid = tg.push_update(up);
    wait_handled(&tg, &srv, uid);
    assert!(
        tg.calls("answerCallbackQuery")
            .iter()
            .all(|r| r["json"]["callback_query_id"].as_str() != Some(stranger_cb.as_str())),
        "a stranger got an answer"
    );

    // First press: only the question, no git.
    let cb = press(&tg, PAIRED_CHAT, m, &qa_text, &ask);
    assert_eq!(wait_answer(&tg, &srv, &cb)["json"]["text"], "Xác nhận Merge & push?");
    let confirm = wait_edit(&tg, &srv, m, |r| {
        text_of(r) == "Merge task 1 (task/sua-app) vào dev và push?"
    });
    let labels: Vec<String> = buttons(&confirm).into_iter().map(|(l, _)| l).collect();
    assert_eq!(labels, vec!["Xác nhận", "Hủy"], "{confirm}");
    assert_eq!(sb.git(&repo, &["branch", "--show-current"]).trim(), "task/sua-app");
    assert_eq!(git_sha(&sb, &remote, "dev"), origin_before);

    // Hủy: nothing changes; the QA message comes back with its buttons.
    let cb = press(&tg, PAIRED_CHAT, m, text_of(&confirm), &data_of(&confirm, "Hủy"));
    assert_eq!(wait_answer(&tg, &srv, &cb)["json"]["text"], "Đã huỷ — chưa merge");
    let restored = wait_edit(&tg, &srv, m, |r| {
        text_of(r) == qa_text && buttons(r).iter().any(|(l, _)| l == "Merge & push")
    });
    assert_eq!(sb.git(&repo, &["branch", "--show-current"]).trim(), "task/sua-app");
    assert_eq!(git_sha(&sb, &remote, "dev"), origin_before);
    assert!(has_branch(&repo, "task/sua-app", &sb));

    // Ask again, then Xác nhận: runs once.
    let cb = press(&tg, PAIRED_CHAT, m, &qa_text, &data_of(&restored, "Merge & push"));
    wait_answer(&tg, &srv, &cb);
    let confirm_data = data_of(&confirm, "Xác nhận");
    assert!(confirm_data.starts_with("G:1:"), "{confirm_data}");
    let cb = press(&tg, PAIRED_CHAT, m, text_of(&confirm), &confirm_data);
    assert_eq!(wait_answer(&tg, &srv, &cb)["json"]["text"], "Đã merge & push");
    let result = wait_message(&tg, &srv, "Merge & push · task 1", 0);
    let rt = text_of(&result);
    assert!(rt.contains("Đã merge 'task/sua-app' vào 'dev' (") && rt.contains("và push lên origin."), "{rt}");
    assert!(!rt.contains(remote.to_str().unwrap()) && !rt.contains(repo.to_str().unwrap()), "{rt}");
    wait_edit(&tg, &srv, m, |r| {
        text_of(r).starts_with(&qa_text) && text_of(r).contains("✓ Đã merge & push — từ Telegram lúc ")
    });
    let merged = git_sha(&sb, &repo, "dev");
    assert_eq!(git_sha(&sb, &remote, "dev"), merged);
    assert!(!has_branch(&repo, "task/sua-app", &sb));
    let subjects = sb.git(&repo, &["log", "--first-parent", "--format=%s", "dev"]);
    assert_eq!(subjects.matches("[task #1]").count(), 1, "{subjects}");

    // The old confirmation pressed again: refused, nothing runs a second time.
    let cb = press(&tg, PAIRED_CHAT, m, text_of(&confirm), &confirm_data);
    let ans = wait_answer(&tg, &srv, &cb);
    assert_eq!(ans["json"]["show_alert"], true, "{ans}");
    assert!(ans["json"]["text"].as_str().unwrap().contains("Đã merge vào dev và push"), "{ans}");
    let refusal = wait_message(&tg, &srv, "Merge & push · task 1", 1);
    assert!(text_of(&refusal).contains("Không chạy: Đã merge vào dev và push"), "{refusal}");
    assert_eq!(git_sha(&sb, &repo, "dev"), merged);
    assert_eq!(git_sha(&sb, &remote, "dev"), merged);

    assert!(
        stdout_of(&srv).contains("Telegram: Merge & push task 1 — Đã merge 'task/sua-app' vào 'dev'"),
        "{}",
        stdout_of(&srv)
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}
