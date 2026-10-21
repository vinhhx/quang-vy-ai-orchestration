//! Telegram notifications (Phase 05.3 plan 03): gate, stop, QA passed and run
//! failed messages, sent while serve runs. Every test talks to the local fake
//! Telegram only (tests/support/telegram.rs); nothing contacts api.telegram.org.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use common::{wait_until, RunOut, Sandbox, Serve};
use protocol::lead_scenario;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;
use tg::{write_config, FakeTelegram, PAIRED_CHAT};

// ---------------------------------------------------------------------------
// Scenario helpers (copied from tests/dashboard_autopilot.rs)
// ---------------------------------------------------------------------------

fn review_report(verdict: &str, blocking: i64, advisory: i64) -> String {
    let mut body = format!(
        "---\nverdict: {verdict}\nblocking: {blocking}\nadvisory: {advisory}\nfiles: []\n---\n# Review: sửa app\n## Mục chặn (blocking)\n"
    );
    for i in 0..blocking {
        body.push_str(&format!("- mục chặn {}\n", i + 1));
    }
    body.push_str("## Góp ý (advisory)\n");
    for i in 0..advisory {
        body.push_str(&format!("- góp ý {}\n", i + 1));
    }
    body
}

fn plan_review_entry(verdict: &str, blocking: i64, advisory: i64) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan-review.md",
        "content": review_report(verdict, blocking, advisory),
    }]})
}

fn lead_plan_entry() -> Value {
    lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])
}

fn by_role(roles: Value) -> Value {
    json!({ "by_role": roles })
}

/// Repo `web-ban-hang` with an entry without `workflow` (server default: mac-dinh).
fn setup_auto_project(sb: &Sandbox, profile: &str) -> PathBuf {
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", profile);
    entry.as_object_mut().unwrap().remove("workflow");
    sb.write_map(json!([entry]), &[]);
    repo
}

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

fn task_auto_state(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT auto_state FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
}

/// `auto --start` in a terminal on mac-dinh: Lead, plan review approves, stops at the gate.
fn auto_start_to_gate(sb: &Sandbox, sc: Value) {
    let res = sb.run(
        &[
            "auto",
            "--start",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(sc),
    );
    assert_ok(&res, "auto --start");
    assert_eq!(task_auto_state(sb, 1).as_deref(), Some("gate"));
}

fn gate_scenario() -> Value {
    by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry("approve", 0, 2)],
    }))
}

// ---------------------------------------------------------------------------
// Telegram helpers
// ---------------------------------------------------------------------------

const CONNECTED: &str = "Telegram: đã kết nối (@ab_fake_bot)";

fn stdout_of(srv: &Serve) -> String {
    srv.stdout.lock().unwrap().clone()
}

fn stderr_of(srv: &Serve) -> String {
    std::fs::read_to_string(&srv.stderr_path).unwrap_or_default()
}

fn wait_connected(srv: &Serve) {
    assert!(
        wait_until(Duration::from_secs(10), || stdout_of(srv)
            .contains(CONNECTED)),
        "no connected line; stdout:\n{}\nstderr:\n{}",
        stdout_of(srv),
        stderr_of(srv)
    );
}

fn text_of(rec: &Value) -> &str {
    rec["json"]["text"].as_str().unwrap_or("")
}

fn first_line(rec: &Value) -> &str {
    text_of(rec).lines().next().unwrap_or("")
}

fn messages_starting(tg: &FakeTelegram, prefix: &str) -> Vec<Value> {
    tg.calls("sendMessage")
        .into_iter()
        .filter(|r| text_of(r).starts_with(prefix))
        .collect()
}

fn wait_message(tg: &FakeTelegram, srv: &Serve, prefix: &str) -> Value {
    match tg.wait_call("sendMessage", Duration::from_secs(15), |r| {
        text_of(r).starts_with(prefix)
    }) {
        Some(r) => r,
        None => panic!(
            "no sendMessage starting with {prefix:?}; records:\n{:#?}\nstdout:\n{}\nstderr:\n{}",
            tg.records(),
            stdout_of(srv),
            stderr_of(srv)
        ),
    }
}

/// `(label, callback_data)` of every inline button, row by row.
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

fn labels(rec: &Value) -> Vec<String> {
    buttons(rec).into_iter().map(|(l, _)| l).collect()
}

fn is_rev_data(data: &str, prefix: &str) -> bool {
    match data.strip_prefix(prefix) {
        Some(rev) => rev.len() == 16 && rev.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
        None => false,
    }
}

/// No send request carries a formatting mode, and every one goes to the paired chat.
fn assert_plain_and_paired(tg: &FakeTelegram) {
    for r in tg.calls("sendMessage") {
        let obj = r["json"].as_object().expect("sendMessage JSON body");
        assert!(
            !obj.keys().any(|k| k == "parse_mode" || k == "entities"),
            "formatting mode on a message: {r}"
        );
        assert_eq!(r["json"]["chat_id"], json!(PAIRED_CHAT), "{r}");
    }
    for r in tg.calls("sendDocument") {
        assert_eq!(r["form"]["chat_id"], json!(PAIRED_CHAT.to_string()), "{r}");
        assert!(r["form"].get("parse_mode").is_none(), "{r}");
    }
}

// ---------------------------------------------------------------------------
// Task 1 (tracer)
// ---------------------------------------------------------------------------

#[test]
fn gate_from_terminal_autopilot_notifies_once() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let repo = setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    auto_start_to_gate(&sb, gate_scenario());

    let msg = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    assert_eq!(msg["json"]["chat_id"], json!(PAIRED_CHAT));
    let text = text_of(&msg).to_string();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(first_line(&msg), "Chờ duyệt plan · task 1", "{text}");
    assert!(text.contains("sửa app"), "{text}");
    assert!(
        text.contains("Dự án: web-ban-hang · profile work · nhánh task/sua-app"),
        "{text}"
    );
    assert!(lines.iter().any(|l| l.starts_with("Plan: ")), "{text}");
    assert!(lines.contains(&"Thay đổi (1 file):"), "{text}");
    assert!(lines.contains(&"- src/app.py"), "{text}");
    assert!(lines.contains(&"Review plan: duyệt · vòng 1 / 3"), "{text}");

    // Buttons: exactly Duyệt plan / Trả về Lead, carrying the task's current rev.
    let btns = buttons(&msg);
    assert_eq!(
        labels(&msg),
        vec!["Duyệt plan".to_string(), "Trả về Lead".to_string()],
        "{msg}"
    );
    assert!(is_rev_data(&btns[0].1, "a:1:"), "{btns:?}");
    assert!(is_rev_data(&btns[1].1, "r:1:"), "{btns:?}");
    let rev = srv.get("/api/tasks/1").json()["rev"]
        .as_str()
        .expect("rev")
        .to_string();
    assert_eq!(btns[0].1, format!("a:1:{rev}"));
    assert_eq!(btns[1].1, format!("r:1:{rev}"));

    // The plan document comes first and the text replies to it.
    let docs = tg.calls("sendDocument");
    assert_eq!(docs.len(), 1, "{docs:#?}");
    let doc = &docs[0];
    assert_eq!(doc["form"]["chat_id"], json!("5555"));
    assert_eq!(
        doc["form"]["document"]["filename"],
        json!("sua-app.plan.md")
    );
    let plan_on_disk = std::fs::read_to_string(repo.join(".plan.md")).expect("read .plan.md");
    assert_eq!(
        doc["form"]["document"]["content"].as_str(),
        Some(plan_on_disk.as_str())
    );
    let doc_mid = doc["response"]["result"]["message_id"]
        .as_i64()
        .expect("document message_id");
    assert_eq!(
        msg["json"]["reply_parameters"]["message_id"].as_i64(),
        Some(doc_mid),
        "{msg}"
    );
    assert!(
        doc["t"].as_f64().unwrap_or(0.0) <= msg["t"].as_f64().unwrap_or(0.0),
        "document must be sent before the text"
    );
    assert_plain_and_paired(&tg);
    assert!(
        wait_until(Duration::from_secs(5), || stdout_of(&srv)
            .contains("Telegram: đã gửi tin chờ duyệt plan cho task 1")),
        "no sent line; stdout:\n{}",
        stdout_of(&srv)
    );

    // Exactly once.
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(messages_starting(&tg, "Chờ duyệt plan").len(), 1);

    // A restart does not resend the gate that already existed.
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
    drop(srv);
    let mut srv2 = tg.serve(&sb, &[]);
    wait_connected(&srv2);
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(messages_starting(&tg, "Chờ duyệt plan").len(), 1);
    assert_eq!(tg.calls("sendDocument").len(), 1);
    assert_eq!(srv2.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}
