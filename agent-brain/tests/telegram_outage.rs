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
use protocol::{lead_scenario, setup_project};
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

/// `None` also while the task row does not exist yet.
fn task_auto_state(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT auto_state FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
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

// ---------------------------------------------------------------------------
// Task 3: 429, non-retryable 4xx, stale drop, never block
// ---------------------------------------------------------------------------

#[test]
fn rate_limit_retry_after_honored() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    tg.set_behavior(json!([
        {"method": "sendMessage", "times": 1, "status": 429,
         "description": "Too Many Requests: retry after 2", "retry_after": 2}
    ]));

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    run_lead(&sb, "web-ban-hang", "sua-app", "sửa app");

    let ok = wait_delivered(
        &tg,
        &srv,
        "Chờ duyệt plan · task 1",
        Duration::from_secs(20),
    );
    let limited = messages(&tg, "Chờ duyệt plan · task 1", 429);
    assert_eq!(limited.len(), 1, "{:#?}", tg.records());
    let waited = t_of(&ok) - t_of(&limited[0]);
    assert!(
        waited >= 2.0,
        "retry after {waited:.3} s, expected at least 2 s; records:\n{:#?}",
        tg.records()
    );
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(messages(&tg, "Chờ duyệt plan · task 1", 200).len(), 1);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn non_retryable_drops_one_item() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_two_projects(&sb);
    tg.set_behavior(json!([
        {"method": "sendMessage", "times": 1, "status": 400,
         "description": "Bad Request: chat not found"}
    ]));

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    run_lead(&sb, "web-ban-hang", "sua-app", "sửa app");
    assert!(
        wait_until(Duration::from_secs(15), || stderr_of(&srv)
            .contains("Telegram: bỏ tin cho task 1 —")),
        "no drop line; stderr:\n{}\nrecords:\n{:#?}",
        stderr_of(&srv),
        tg.records()
    );
    std::thread::sleep(Duration::from_secs(3));
    let task1: Vec<Value> = tg
        .calls("sendMessage")
        .into_iter()
        .filter(|r| text_of(r).starts_with("Chờ duyệt plan · task 1"))
        .collect();
    assert_eq!(task1.len(), 1, "{task1:#?}");
    assert_eq!(status_of(&task1[0]), 400, "{task1:#?}");
    assert_eq!(
        count_lines(&stderr_of(&srv), "Telegram: bỏ tin cho task 1 —"),
        1
    );

    run_lead(&sb, "kho-hang", "kiem-kho", "kiểm kho");
    let msg = wait_delivered(
        &tg,
        &srv,
        "Chờ duyệt plan · task 2",
        Duration::from_secs(15),
    );
    assert_eq!(msg["json"]["chat_id"], json!(PAIRED_CHAT));
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn stale_after_outage_dropped() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    tg.set_behavior(json!([
        {"method": "sendDocument", "times": 100000, "status": 502, "description": "Bad Gateway"},
        {"method": "sendMessage", "times": 100000, "status": 502, "description": "Bad Gateway"}
    ]));

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    run_lead(&sb, "web-ban-hang", "sua-app", "sửa app");
    let failing = |r: &Value| status_of(r) == 502;
    assert!(
        wait_until(Duration::from_secs(15), || {
            tg.calls("sendDocument").iter().any(failing)
                || tg.calls("sendMessage").iter().any(failing)
        }),
        "no failed attempt; records:\n{:#?}\nstderr:\n{}",
        tg.records(),
        stderr_of(&srv)
    );

    // Approved from the dashboard while Telegram is down: the gate is stale.
    let resp = srv.post_json("/api/tasks/1/approve", &json!({}));
    assert_eq!(resp.status, 200, "{}", resp.text());
    tg.set_behavior(json!([]));

    std::thread::sleep(Duration::from_secs(5));
    let delivered: Vec<Value> = tg
        .records()
        .into_iter()
        .filter(|r| {
            matches!(r["method"].as_str(), Some("sendMessage" | "sendDocument"))
                && status_of(r) == 200
        })
        .collect();
    assert!(delivered.is_empty(), "stale gate delivered: {delivered:#?}");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn autopilot_not_blocked_by_outage() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");
    sb.write_scenario(full_flow_scenario());
    // getMe passes (a delay-only rule matches first); everything else fails.
    tg.set_behavior(json!([
        {"method": "getMe", "times": 100000, "delay_ms": 0},
        {"method": "*", "times": 100000, "status": 502, "description": "Bad Gateway"}
    ]));

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "sửa app", "name": "sua-app"}),
    );
    assert_eq!(resp.status, 202, "{}", resp.text());
    assert!(
        wait_until(Duration::from_secs(30), || task_auto_state(&sb, 1)
            .as_deref()
            == Some("gate")),
        "no gate; auto_state {:?}; stderr:\n{}",
        task_auto_state(&sb, 1),
        stderr_of(&srv)
    );

    let resp = srv.post_json("/api/tasks/1/approve", &json!({}));
    assert_eq!(resp.status, 200, "{}", resp.text());

    let mut api_failures = Vec::new();
    let done = wait_until(Duration::from_secs(60), || {
        let r = srv.get("/api/tasks");
        if r.status != 200 {
            api_failures.push(r.status);
        }
        task_auto_state(&sb, 1).as_deref() == Some("done")
    });
    assert!(
        done,
        "autopilot did not finish; auto_state {:?}; stderr:\n{}",
        task_auto_state(&sb, 1),
        stderr_of(&srv)
    );
    assert!(api_failures.is_empty(), "GET /api/tasks: {api_failures:?}");
    assert_eq!(srv.get("/api/tasks").status, 200);

    // Telegram really was down the whole time, and the outage was reported once.
    let attempts: Vec<Value> = tg
        .records()
        .into_iter()
        .filter(|r| r["method"] != json!("getMe"))
        .collect();
    assert!(!attempts.is_empty(), "the sender never tried");
    assert!(
        attempts.iter().all(|r| status_of(r) == 502),
        "{attempts:#?}"
    );
    let err = stderr_of(&srv);
    assert_eq!(count_lines(&err, WARNING), 1, "stderr:\n{err}");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}
