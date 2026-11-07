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
use protocol::{lead_scenario, setup_project};
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

fn lead_append_entry() -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan.md",
        "content": "\n<!-- v2 -->\n",
        "append": true,
    }]})
}

fn task_auto_json(sb: &Sandbox, task_id: i64) -> Value {
    let aj: Option<String> = sb
        .db()
        .query_row(
            "SELECT auto_json FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap();
    serde_json::from_str(&aj.unwrap_or_else(|| "{}".to_string())).unwrap()
}

fn dev_impl_entry() -> Value {
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
        {"op": "git", "args": ["add", "src/app.py"]},
        {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
    ]})
}

fn code_review_entry(verdict: &str, blocking: i64) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".code-review.md",
        "content": review_report(verdict, blocking, 1),
    }]})
}

fn qa_entry(verdict: &str) -> Value {
    let report = format!(
        "# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- src/app.py: đã xem.\n## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông.\n## Kết luận\n{verdict}\n"
    );
    json!({"ops": [{"op": "write", "path": ".qa-report.md", "content": report}]})
}

/// Every mac-dinh step: plan, plan review approves, dev, code review passes, QA passes.
fn full_flow_scenario() -> Value {
    by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry("approve", 0, 1)],
        "dev": [dev_impl_entry()],
        "code-review": [code_review_entry("pass", 0)],
        "qa": [qa_entry("Đạt")],
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

// ---------------------------------------------------------------------------
// Task 2: stop, QA passed, run failed; profile shown; buttons by state
// ---------------------------------------------------------------------------

#[test]
fn stop_notifies_reason_and_buttons() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    // mac-dinh, plan review always revises: `hết vòng` at round 3 / 3.
    let sc = by_role(json!({
        "lead": [lead_plan_entry(), lead_append_entry()],
        "plan-review": [
            plan_review_entry("revise", 1, 0),
            plan_review_entry("revise", 1, 1),
            plan_review_entry("revise", 1, 2)
        ],
    }));
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
    assert_eq!(res.code, 4, "stderr: {}", res.stderr);
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("stopped"));
    assert_eq!(task_auto_json(&sb, 1)["stop"]["reason"], "het_vong");

    let msg = wait_message(&tg, &srv, "Autopilot dừng");
    assert_eq!(first_line(&msg), "Autopilot dừng · hết vòng · task 1");
    let text = text_of(&msg).to_string();
    assert!(text.contains("sửa app"), "{text}");
    assert!(
        text.contains("Dự án: web-ban-hang · profile work · nhánh task/sua-app"),
        "{text}"
    );
    assert!(
        text.contains("Review plan vẫn yêu cầu sửa lại sau 3 / 3 vòng."),
        "{text}"
    );
    assert_eq!(
        labels(&msg),
        vec![
            "Chạy thêm 1 vòng".to_string(),
            "Trả về Lead".to_string(),
            "Dừng hẳn".to_string()
        ],
        "{msg}"
    );
    let btns = buttons(&msg);
    assert!(is_rev_data(&btns[0].1, "m:1:"), "{btns:?}");
    assert!(is_rev_data(&btns[1].1, "r:1:"), "{btns:?}");
    assert!(is_rev_data(&btns[2].1, "e:1:"), "{btns:?}");

    // One message for the stop, no separate gate message (stop takes precedence).
    std::thread::sleep(Duration::from_secs(3));
    let all = tg.calls("sendMessage");
    assert_eq!(all.len(), 1, "{all:#?}");
    assert!(tg.calls("sendDocument").is_empty());
    assert_plain_and_paired(&tg);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn qa_pass_notifies_summary() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    auto_start_to_gate(&sb, full_flow_scenario());
    wait_message(&tg, &srv, "Chờ duyệt plan · task 1");

    let res = sb.run(&["approve", "1"], Some(full_flow_scenario()));
    assert_ok(&res, "approve 1");

    let msg = wait_message(&tg, &srv, "QA đạt");
    assert_eq!(first_line(&msg), "QA đạt · task 1");
    let text = text_of(&msg).to_string();
    assert!(text.contains("sửa app"), "{text}");
    assert!(
        text.contains("Dự án: web-ban-hang · profile work · nhánh task/sua-app"),
        "{text}"
    );
    assert!(text.contains("Tổng: 5 run ·"), "{text}");
    assert_eq!(
        msg["json"]["reply_markup"]["inline_keyboard"],
        json!([]),
        "{msg}"
    );

    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(messages_starting(&tg, "Chờ duyệt plan").len(), 1);
    assert_eq!(messages_starting(&tg, "QA đạt").len(), 1);
    assert!(messages_starting(&tg, "Run lỗi").is_empty());
    assert!(messages_starting(&tg, "Autopilot dừng").is_empty());
    assert_plain_and_paired(&tg);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn qa_pass_sends_summary_file_before_text() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    auto_start_to_gate(&sb, full_flow_scenario());
    wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let res = sb.run(&["approve", "1"], Some(full_flow_scenario()));
    assert_ok(&res, "approve 1");
    let msg = wait_message(&tg, &srv, "QA đạt");

    let docs = tg.calls("sendDocument");
    let summary: Vec<&Value> = docs
        .iter()
        .filter(|d| d["form"]["document"]["filename"] == json!("sua-app.summary.md"))
        .collect();
    assert_eq!(summary.len(), 1, "{docs:#?}");
    let content = summary[0]["form"]["document"]["content"]
        .as_str()
        .expect("summary content");
    let p1 = content.find("## Yêu cầu").expect("Yêu cầu");
    let p2 = content.find("## Cách thực hiện").expect("Cách thực hiện");
    let p3 = content.find("## Kết quả").expect("Kết quả");
    let p4 = content.find("## Tổng").expect("Tổng");
    assert!(p1 < p2 && p2 < p3 && p3 < p4, "{content}");
    assert!(!content.contains("\n+++ ") && !content.contains("\n@@ "), "{content}");
    let caption = summary[0]["form"]["caption"].as_str().expect("caption");
    assert!(caption.starts_with("Tóm tắt task 1:"), "{caption}");

    // The text follows the file and points at it.
    let doc_mid = summary[0]["response"]["result"]["message_id"]
        .as_i64()
        .expect("document message_id");
    assert_eq!(
        msg["json"]["reply_parameters"]["message_id"].as_i64(),
        Some(doc_mid),
        "{msg}"
    );
    assert!(text_of(&msg).contains("file đính kèm"), "{msg}");

    std::thread::sleep(Duration::from_secs(2));
    let again = tg
        .calls("sendDocument")
        .iter()
        .filter(|d| d["form"]["document"]["filename"] == json!("sua-app.summary.md"))
        .count();
    assert_eq!(again, 1);
    assert_plain_and_paired(&tg);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

fn lead_plan_with_report_entry() -> Value {
    let rows = &[
        ("src/app.py", "app()", "trả về 2", "demo"),
        ("reports/ket-qua.md", "-", "báo cáo", "kết quả"),
    ];
    let mut p_text = protocol::plan_text("Kế hoạch", rows, "python3 -m unittest", "");
    p_text.push_str("\n## Gửi báo cáo\n- `reports/ket-qua.md`\n");
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": ".plan.md", "content": p_text, "keep_meta": true}
    ]})
}

fn dev_impl_with_report_entry() -> Value {
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
        {"op": "write", "path": "reports/ket-qua.md", "content": "# Báo cáo\nĐã sửa app thành công.\n"},
        {"op": "git", "args": ["add", "src/app.py", "reports/ket-qua.md"]},
        {"op": "git", "args": ["commit", "-m", "sua-app: sửa app và thêm báo cáo"]}
    ]})
}

fn full_flow_with_report_scenario() -> Value {
    by_role(json!({
        "lead": [lead_plan_with_report_entry()],
        "plan-review": [plan_review_entry("approve", 0, 1)],
        "dev": [dev_impl_with_report_entry()],
        "code-review": [code_review_entry("pass", 0)],
        "qa": [qa_entry("Đạt")],
    }))
}

#[test]
fn qa_pass_sends_summary_and_report_file_before_text() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    auto_start_to_gate(&sb, full_flow_with_report_scenario());
    wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let res = sb.run(&["approve", "1"], Some(full_flow_with_report_scenario()));
    assert_ok(&res, "approve 1");
    let msg = wait_message(&tg, &srv, "QA đạt");

    let docs = tg.calls("sendDocument");
    // Gate sends 1 plan document, QA pass sends 2 documents (summary + report)
    let qa_docs: Vec<&Value> = docs
        .iter()
        .filter(|d| d["form"]["document"]["filename"] != json!("sua-app.plan.md"))
        .collect();
    assert_eq!(qa_docs.len(), 2, "must have 2 QA sendDocument calls: {docs:#?}");

    // 1. Tóm tắt trước
    assert_eq!(qa_docs[0]["form"]["document"]["filename"], json!("sua-app.summary.md"));
    let caption0 = qa_docs[0]["form"]["caption"].as_str().expect("caption summary");
    assert!(caption0.starts_with("Tóm tắt task 1:"), "{caption0}");

    // 2. Báo cáo sau
    assert_eq!(qa_docs[1]["form"]["document"]["filename"], json!("ket-qua.md"));
    let caption1 = qa_docs[1]["form"]["caption"].as_str().expect("caption report");
    assert!(caption1.starts_with("Báo cáo task 1:"), "{caption1}");
    let report_content = qa_docs[1]["form"]["document"]["content"].as_str().expect("report content");
    assert!(report_content.contains("Đã sửa app thành công."));

    // 3. Sau đó có 1 sendMessage (msg đã nhận là QA đạt)
    let send_messages = tg.calls("sendMessage");
    assert!(send_messages.len() >= 2, "{send_messages:#?}");
    let doc_mid = qa_docs[0]["response"]["result"]["message_id"]
        .as_i64()
        .expect("summary document message_id");
    assert_eq!(
        msg["json"]["reply_parameters"]["message_id"].as_i64(),
        Some(doc_mid),
        "{msg}"
    );

    assert_plain_and_paired(&tg);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn run_failed_notifies() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    let res = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead");
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    assert!(
        text_of(&gate).contains("Review plan: không có trong quy trình nhanh"),
        "{gate}"
    );

    let res = sb.run(&["approve", "1"], None);
    assert_ok(&res, "approve 1");

    let res = sb.run(
        &["run", "dev", "--", "web-ban-hang"],
        Some(json!({"exit_code": 1, "ops": []})),
    );
    assert_ne!(res.code, 0, "dev should fail: {}", res.stderr);
    let (dev_id, dev_status): (i64, String) = sb
        .db()
        .query_row(
            "SELECT id, status FROM runs WHERE role = 'dev' ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(
        dev_status == "failed" || dev_status == "timed_out",
        "dev run status {dev_status}"
    );

    let msg = wait_message(&tg, &srv, "Run lỗi");
    assert_eq!(first_line(&msg), "Run lỗi · task 1");
    let text = text_of(&msg).to_string();
    assert!(
        text.contains(&format!(
            "Run {dev_id} (dev) lỗi hoặc quá thời gian — xem log của run này."
        )),
        "{text}"
    );
    assert!(text.contains("Tổng: 2 run ·"), "{text}");
    assert_eq!(labels(&msg), vec!["Trả về Lead".to_string()], "{msg}");
    assert!(is_rev_data(&buttons(&msg)[0].1, "r:1:"), "{msg}");

    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(messages_starting(&tg, "Run lỗi").len(), 1);
    assert_eq!(tg.calls("sendMessage").len(), 2);
    assert_plain_and_paired(&tg);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn gate_message_shows_personal_profile() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "personal");

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    auto_start_to_gate(&sb, gate_scenario());

    let msg = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let text = text_of(&msg).to_string();
    assert!(
        text.contains("Dự án: web-ban-hang · profile personal · nhánh task/sua-app"),
        "{text}"
    );
    assert_eq!(
        labels(&msg),
        vec!["Duyệt plan".to_string(), "Trả về Lead".to_string()],
        "{msg}"
    );
    assert_eq!(tg.calls("sendDocument").len(), 1);
    assert_plain_and_paired(&tg);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// Plan 04 Task 2: two tasks reaching events at the same time (TELE-05)
// ---------------------------------------------------------------------------

#[test]
fn two_tasks_each_notified_once() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let mut entries = Vec::new();
    for id in ["web-ban-hang", "kho-hang"] {
        let repo = sb.make_repo(
            id,
            &[("src/app.py", "def app():\n    return 1\n")],
            None,
            None,
        );
        entries.push(sb.project_entry(&repo, id, "work"));
    }
    sb.write_map(Value::Array(entries), &[]);

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    // Both Lead runs at once, so both gates appear together.
    sb.write_scenario(lead_plan_entry());
    let leads: Vec<_> = [
        ("web-ban-hang", "sua-app", "sửa app"),
        ("kho-hang", "kiem-kho", "kiểm kho"),
    ]
    .into_iter()
    .map(|(project, name, request)| {
        let mut cmd = sb.command(
            &["run", "lead", "--name", name, "--", project, request],
            &[],
        );
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        (project, cmd)
    })
    .collect();
    let children: Vec<_> = leads
        .into_iter()
        .map(|(project, mut cmd)| (project, cmd.spawn().expect("spawn run lead")))
        .collect();
    for (project, child) in children {
        let out = child.wait_with_output().expect("wait run lead");
        assert!(
            out.status.success(),
            "run lead {project} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    wait_message(&tg, &srv, "Chờ duyệt plan · task 2");
    std::thread::sleep(Duration::from_secs(3));
    for task in [1, 2] {
        let sent = messages_starting(&tg, &format!("Chờ duyệt plan · task {task}"));
        assert_eq!(sent.len(), 1, "task {task}: {sent:#?}");
        assert_eq!(sent[0]["json"]["chat_id"], json!(5555), "{}", sent[0]);
    }
    assert_eq!(tg.calls("sendMessage").len(), 2, "{:#?}", tg.records());
    assert_eq!(tg.calls("sendDocument").len(), 2, "{:#?}", tg.records());
    assert_plain_and_paired(&tg);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

/// WR-06: a task whose state cannot be read while the detector seeds (here a corrupt
/// run row) must not have its pre-restart event notified once it can be read again.
#[test]
fn unreadable_task_at_start_does_not_resend_old_event() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    let res = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead");

    // The gate exists before serve starts. Break the task's run row so that reading
    // the task fails while the detector seeds.
    let started_at: i64 = sb
        .db()
        .query_row(
            "SELECT started_at FROM runs WHERE task_id = 1 ORDER BY id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .expect("read started_at");
    sb.db()
        .execute("UPDATE runs SET started_at = 'x' WHERE task_id = 1", [])
        .expect("corrupt run row");

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);
    assert!(
        wait_until(Duration::from_secs(10), || stderr_of(&srv)
            .contains("Telegram: lỗi đọc task 1")),
        "the task was readable at start-up; stderr:\n{}",
        stderr_of(&srv)
    );

    // The row is readable again: the next scan reads the task.
    sb.db()
        .execute(
            "UPDATE runs SET started_at = ?1 WHERE task_id = 1",
            [started_at],
        )
        .expect("repair run row");
    std::thread::sleep(Duration::from_secs(5));
    assert!(
        messages_starting(&tg, "Chờ duyệt plan").is_empty(),
        "the old gate was sent again: {:#?}",
        tg.records()
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// `## Sản phẩm`: product files sent after the summary and the reports
// ---------------------------------------------------------------------------

fn lead_plan_with_products_entry() -> Value {
    let rows = &[
        ("src/app.py", "app()", "trả về 2", "demo"),
        ("reports/ket-qua.md", "-", "báo cáo", "kết quả"),
    ];
    let mut p_text = protocol::plan_text("Kế hoạch", rows, "python3 -m unittest", "");
    p_text.push_str(
        "\n## Gửi báo cáo\n- `reports/ket-qua.md`\n## Thư mục được ghi\n- `out/**`\n## Sản phẩm\n- `out/v.mp4`\n- `out/r.pdf`\n- `out/big.mp4`\n",
    );
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": ".plan.md", "content": p_text, "keep_meta": true}
    ]})
}

fn full_flow_with_products_scenario() -> Value {
    by_role(json!({
        "lead": [lead_plan_with_products_entry()],
        "plan-review": [plan_review_entry("approve", 0, 1)],
        "dev": [dev_impl_with_report_entry()],
        "code-review": [code_review_entry("pass", 0)],
        "qa": [qa_entry("Đạt")],
    }))
}

#[test]
fn qa_pass_sends_products_after_summary_and_report() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let repo = sb.make_repo(
        "web-ban-hang",
        &[
            ("src/app.py", "def app():\n    return 1\n"),
            (".gitignore", "out/\n"),
        ],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    entry.as_object_mut().unwrap().remove("workflow");
    sb.write_map(json!([entry]), &[]);

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    auto_start_to_gate(&sb, full_flow_with_products_scenario());
    wait_message(&tg, &srv, "Chờ duyệt plan · task 1");

    // the products, made outside git (gitignored) before the run goes on
    std::fs::create_dir_all(repo.join("out")).unwrap();
    std::fs::write(repo.join("out/v.mp4"), "fake video bytes").unwrap();
    std::fs::write(repo.join("out/r.pdf"), "%PDF fake").unwrap();
    let big = std::fs::File::create(repo.join("out/big.mp4")).unwrap();
    big.set_len(60 * 1024 * 1024).unwrap();
    drop(big);

    let res = sb.run(&["approve", "1"], Some(full_flow_with_products_scenario()));
    assert_ok(&res, "approve 1");
    let msg = wait_message(&tg, &srv, "QA đạt");

    // every request from the summary file on, in order
    let records = tg.records();
    let start = records
        .iter()
        .position(|r| r["form"]["document"]["filename"] == json!("sua-app.summary.md"))
        .unwrap_or_else(|| panic!("no summary file: {records:#?}"));
    let order: Vec<(String, String)> = records[start..]
        .iter()
        .map(|r| {
            let method = r["method"].as_str().unwrap_or("").to_string();
            let what = match method.as_str() {
                "sendDocument" => r["form"]["document"]["filename"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                "sendVideo" => r["form"]["video"]["filename"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                _ => text_of(r).lines().next().unwrap_or("").to_string(),
            };
            (method, what)
        })
        .filter(|(m, _)| m.starts_with("send"))
        .collect();
    let methods: Vec<&str> = order.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(
        methods,
        vec![
            "sendDocument",
            "sendDocument",
            "sendVideo",
            "sendDocument",
            "sendMessage",
            "sendMessage"
        ],
        "{order:#?}"
    );
    assert_eq!(order[1].1, "ket-qua.md", "{order:#?}");
    assert_eq!(order[2].1, "v.mp4", "{order:#?}");
    assert_eq!(order[3].1, "r.pdf", "{order:#?}");
    assert!(
        order[4].1.starts_with("Sản phẩm task 1: big.mp4"),
        "{order:#?}"
    );
    assert!(order[5].1.starts_with("QA đạt"), "{order:#?}");

    // the video: streaming, paired chat, caption, real bytes
    let videos = tg.calls("sendVideo");
    assert_eq!(videos.len(), 1, "{videos:#?}");
    let v = &videos[0];
    assert_eq!(v["form"]["supports_streaming"], json!("true"), "{v}");
    assert_eq!(v["form"]["chat_id"], json!(PAIRED_CHAT.to_string()), "{v}");
    assert_eq!(v["form"]["caption"], json!("Sản phẩm task 1: v.mp4"), "{v}");
    assert_eq!(
        v["form"]["video"]["content"],
        json!("fake video bytes"),
        "{v}"
    );
    assert_eq!(v["status"], json!(200), "{v}");

    // the pdf goes as a document with its own bytes
    let pdf = tg
        .calls("sendDocument")
        .into_iter()
        .find(|d| d["form"]["document"]["filename"] == json!("r.pdf"))
        .expect("r.pdf sent");
    assert_eq!(pdf["form"]["document"]["content"], json!("%PDF fake"));
    assert_eq!(pdf["form"]["caption"], json!("Sản phẩm task 1: r.pdf"));

    // the big file is never uploaded: a text with its size and absolute path
    let big_msg = messages_starting(&tg, "Sản phẩm task 1: big.mp4");
    assert_eq!(big_msg.len(), 1);
    let big_text = text_of(&big_msg[0]);
    let big_path = repo.join("out/big.mp4").canonicalize().unwrap();
    assert!(big_text.contains("(60.0 MB)"), "{big_text}");
    assert!(big_text.contains("lớn hơn 50 MB"), "{big_text}");
    assert!(
        big_text.contains(&format!("File ở: {}", big_path.display())),
        "{big_text}"
    );
    assert!(tg
        .records()
        .iter()
        .all(|r| r["form"]["video"]["filename"] != json!("big.mp4")
            && r["form"]["document"]["filename"] != json!("big.mp4")));

    // the text still replies to the summary file
    let doc_mid = records[start]["response"]["result"]["message_id"]
        .as_i64()
        .expect("summary message_id");
    assert_eq!(
        msg["json"]["reply_parameters"]["message_id"].as_i64(),
        Some(doc_mid),
        "{msg}"
    );
    assert_plain_and_paired(&tg);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn gate_plan_commit_change_sends_plan_updated_message() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let repo = setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    auto_start_to_gate(&sb, gate_scenario());

    let msg1 = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let m1 = msg1["response"]["result"]["message_id"].as_i64().unwrap();

    // Update plan on the branch: commit with subject "sua-app: plan"
    sb.git(&repo, &["checkout", "task/sua-app"]);
    let plan2 = "<!-- id: sua-app -->\n# Kế hoạch sửa app (v2)\n\n## Mục tiêu\nSửa app tốt hơn\n\n## Các bước\n1. Sửa app.py\n\n## Thay đổi\n| File | Thao tác | Lý do |\n| --- | --- | --- |\n| src/app.py | sửa | cải tiến |\n\n## Không được đụng tới\nKhông có\n\n## Lệnh được chạy\nKhông có\n\n## Thư mục được ghi\nKhông có\n\n## Gửi báo cáo\nKhông có\n\n## Vướng mắc\nKhông có\n";
    std::fs::write(repo.join(".plan.md"), plan2).unwrap();
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", "sua-app: plan"]);
    sb.db()
        .execute(
            "UPDATE tasks SET updated_at = updated_at + 1 WHERE id = 1",
            [],
        )
        .unwrap();

    // The detector detects the new plan commit and sends "Plan đã cập nhật · task 1"
    let msg2 = wait_message(&tg, &srv, "Plan đã cập nhật · task 1");
    let m2 = msg2["response"]["result"]["message_id"].as_i64().unwrap();
    assert_ne!(m2, m1);
    assert_eq!(first_line(&msg2), "Plan đã cập nhật · task 1");
    assert_eq!(labels(&msg2), vec!["Duyệt plan".to_string(), "Trả về Lead".to_string()]);

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}
