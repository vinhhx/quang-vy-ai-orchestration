//! Telegram outages (Phase 05.3 plan 04, D-05): notifications wait and retry
//! quietly while Telegram fails, stale ones are dropped, one warning line per
//! outage, and agent work never waits. Failures are injected only through the
//! local fake Telegram's `behavior.json` (tests/support/telegram.rs); nothing
//! contacts api.telegram.org.

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
// Scenario helpers (copied from tests/telegram_notify.rs)
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

/// Repos `web-ban-hang` and `kho-hang`, both on workflow `nhanh`.
fn setup_two_projects(sb: &Sandbox) {
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

fn gate_scenario() -> Value {
    by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry("approve", 0, 2)],
    }))
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

/// `run lead` on a nhanh project: the task stops at the plan gate.
fn run_lead(sb: &Sandbox, project: &str, name: &str, request: &str) {
    let res = sb.run(
        &["run", "lead", "--name", name, "--", project, request],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead");
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
const WARNING: &str = "Telegram: không tới được Telegram";
const RECOVERED: &str = "Telegram: đã kết nối lại";

fn stdout_of(srv: &Serve) -> String {
    srv.stdout.lock().unwrap().clone()
}

fn stderr_of(srv: &Serve) -> String {
    std::fs::read_to_string(&srv.stderr_path).unwrap_or_default()
}

fn count_lines(text: &str, needle: &str) -> usize {
    text.lines().filter(|l| l.contains(needle)).count()
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

fn status_of(rec: &Value) -> i64 {
    rec["status"].as_i64().unwrap_or(0)
}

fn t_of(rec: &Value) -> f64 {
    rec["t"].as_f64().unwrap_or(0.0)
}

/// sendMessage records whose text starts with `prefix` and whose answer was `status`.
fn messages(tg: &FakeTelegram, prefix: &str, status: i64) -> Vec<Value> {
    tg.calls("sendMessage")
        .into_iter()
        .filter(|r| text_of(r).starts_with(prefix) && status_of(r) == status)
        .collect()
}

fn wait_delivered(tg: &FakeTelegram, srv: &Serve, prefix: &str, timeout: Duration) -> Value {
    match tg.wait_call("sendMessage", timeout, |r| {
        text_of(r).starts_with(prefix) && status_of(r) == 200
    }) {
        Some(r) => r,
        None => panic!(
            "no delivered sendMessage starting with {prefix:?}; records:\n{:#?}\nstdout:\n{}\nstderr:\n{}",
            tg.records(),
            stdout_of(srv),
            stderr_of(srv)
        ),
    }
}

// ---------------------------------------------------------------------------
// Task 1 (tracer)
// ---------------------------------------------------------------------------

#[test]
fn outage_then_recovery_delivers_once() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");
    // The document fails three times, is delivered, then the text fails three
    // times. getUpdates (plan 05.3-05) keeps succeeding throughout.
    tg.set_behavior(json!([
        {"method": "sendDocument", "times": 3, "status": 502, "description": "Bad Gateway"},
        {"method": "sendMessage", "times": 3, "status": 502, "description": "Bad Gateway"}
    ]));

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    auto_start_to_gate(&sb, gate_scenario());

    wait_delivered(
        &tg,
        &srv,
        "Chờ duyệt plan · task 1",
        Duration::from_secs(20),
    );
    std::thread::sleep(Duration::from_secs(3));

    let failed_docs = tg
        .calls("sendDocument")
        .into_iter()
        .filter(|r| status_of(r) == 502)
        .count();
    let failed_msgs = messages(&tg, "Chờ duyệt plan · task 1", 502).len();
    assert_eq!(failed_docs, 3, "{:#?}", tg.records());
    assert_eq!(failed_msgs, 3, "{:#?}", tg.records());
    assert_eq!(
        messages(&tg, "Chờ duyệt plan · task 1", 200).len(),
        1,
        "{:#?}",
        tg.records()
    );
    let delivered_docs: Vec<Value> = tg
        .calls("sendDocument")
        .into_iter()
        .filter(|r| status_of(r) == 200)
        .collect();
    assert_eq!(delivered_docs.len(), 1, "{delivered_docs:#?}");

    let err = stderr_of(&srv);
    let out = stdout_of(&srv);
    assert_eq!(
        count_lines(&err, WARNING),
        1,
        "stderr:\n{err}\nstdout:\n{out}"
    );
    assert_eq!(
        count_lines(&out, RECOVERED),
        1,
        "stderr:\n{err}\nstdout:\n{out}"
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}
