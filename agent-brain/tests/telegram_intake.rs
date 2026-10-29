//! Telegram task intake (Phase 05.4): approval separation (plan 05.4-03) and, in
//! later plans, task creation from Telegram. Nothing here contacts
//! api.telegram.org; every test talks to the local fake Telegram only.
//!
//! The harness helpers below are copied from tests/telegram_actions.rs (that file
//! stays untouched); some are only used by tests added by later 05.4 plans.

#![allow(dead_code, unused_imports)]

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use agent_brain::procs::{alive, descendants, ProcId};
use common::{wait_until, RunOut, Sandbox, Serve};
use protocol::setup_project;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tg::{write_config, FakeTelegram, FAKE_TOKEN, PAIRED_CHAT, STRANGER_CHAT};

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
    protocol::lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])
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

fn approved_commit(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT approved_commit FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
}

fn task_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap()
}

fn run_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
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

/// mac-dinh through `auto --start` (CLI): plan review always revises, so the
/// autopilot stops with `hết vòng` at round 3 / 3 (copied from dashboard_autopilot.rs).
fn plan_cap_stopped(sb: &Sandbox) {
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
    assert_eq!(task_auto_state(sb, 1).as_deref(), Some("stopped"));
    let stop = task_auto_json(sb, 1)["stop"].clone();
    assert_eq!(stop["reason"], "het_vong", "{stop}");
    assert_eq!(stop["loop_name"], "plan", "{stop}");
}

fn job_role(srv: &Serve, job_id: u64) -> String {
    let j = srv.get(&format!("/api/jobs/{job_id}")).json();
    j["role"].as_str().unwrap_or("").to_string()
}

fn wait_for_job(srv: &Serve, job_id: u64) -> Value {
    let mut job_view = None;
    let ok = wait_until(Duration::from_secs(60), || {
        let r = srv.get(&format!("/api/jobs/{job_id}"));
        if r.status == 200 {
            let j = r.json();
            if j["state"] == "exited" {
                job_view = Some(j);
                return true;
            }
        }
        false
    });
    assert!(ok, "job {job_id} did not exit within timeout");
    job_view.unwrap()
}

/// Sha of the newest commit with `subject` on the task branch.
fn commit_with_subject(sb: &Sandbox, repo: &Path, subject: &str) -> String {
    let log = sb.git(repo, &["log", "--format=%H %s", "task/sua-app"]);
    log.lines()
        .find_map(|l| {
            let (sha, subj) = l.split_once(' ')?;
            (subj == subject).then(|| sha.to_string())
        })
        .unwrap_or_else(|| panic!("no commit {subject:?} on task/sua-app:\n{log}"))
}

// ---------------------------------------------------------------------------
// Telegram helpers
// ---------------------------------------------------------------------------

const CONNECTED: &str = "Telegram: đã kết nối (@ab_fake_bot)";
const HELP: &str = agent_brain::telegram::bot::HELP_TEXT;
const INVALID: &str = "Nút không hợp lệ.";

fn stdout_of(srv: &Serve) -> String {
    srv.stdout.lock().unwrap().clone()
}

fn stderr_of(srv: &Serve) -> String {
    std::fs::read_to_string(&srv.stderr_path).unwrap_or_default()
}

/// Waits for the connected line and for the first long poll (after the
/// start-up drain), so updates pushed from now on are delivered, not dropped.
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

fn first_line(rec: &Value) -> &str {
    text_of(rec).lines().next().unwrap_or("")
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

fn button_data(rec: &Value, label: &str) -> String {
    buttons(rec)
        .into_iter()
        .find(|(l, _)| l == label)
        .map(|(_, d)| d)
        .unwrap_or_else(|| panic!("no {label:?} button on {rec}"))
}

fn is_rev_data(data: &str, prefix: &str) -> bool {
    match data.strip_prefix(prefix) {
        Some(rev) => rev.len() == 16 && rev.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
        None => false,
    }
}

fn message_id_of(rec: &Value) -> i64 {
    rec["response"]["result"]["message_id"]
        .as_i64()
        .unwrap_or_else(|| panic!("no message_id in {rec}"))
}

/// Pushes a callback press on message `rec` (button `label`); returns the callback id.
fn press(tg: &FakeTelegram, chat: i64, from: i64, rec: &Value, label: &str) -> String {
    let data = button_data(rec, label);
    let up = tg.callback_update(chat, from, message_id_of(rec), text_of(rec), &data);
    let id = up["callback_query"]["id"].as_str().unwrap().to_string();
    tg.push_update(up);
    id
}

fn answers_for(tg: &FakeTelegram, callback_id: &str) -> Vec<Value> {
    tg.calls("answerCallbackQuery")
        .into_iter()
        .filter(|r| r["json"]["callback_query_id"].as_str() == Some(callback_id))
        .collect()
}

fn wait_answer(tg: &FakeTelegram, srv: &Serve, callback_id: &str) -> Value {
    match tg.wait_call("answerCallbackQuery", Duration::from_secs(10), |r| {
        r["json"]["callback_query_id"].as_str() == Some(callback_id)
    }) {
        Some(r) => r,
        None => panic!(
            "no answerCallbackQuery for {callback_id}; records:\n{:#?}\nstdout:\n{}\nstderr:\n{}",
            tg.records(),
            stdout_of(srv),
            stderr_of(srv)
        ),
    }
}

fn edits_of(tg: &FakeTelegram, message_id: i64) -> Vec<Value> {
    tg.calls("editMessageText")
        .into_iter()
        .filter(|r| r["json"]["message_id"].as_i64() == Some(message_id))
        .collect()
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

fn empty_keyboard(rec: &Value) -> bool {
    rec["json"]["reply_markup"]["inline_keyboard"] == json!([])
}

fn count_lines(text: &str, line: &str) -> usize {
    text.lines().filter(|l| l.trim_end() == line).count()
}

/// Prompt (`-p`) of the fake claude's last Lead call (copied from dashboard_autopilot.rs).
fn last_lead_prompt(sb: &Sandbox) -> String {
    let rec = sb
        .records()
        .into_iter()
        .filter(|r| {
            let argv = r["argv"].as_array().unwrap();
            argv.iter()
                .position(|a| a.as_str() == Some("--append-system-prompt"))
                .map(|i| {
                    argv[i + 1]
                        .as_str()
                        .unwrap_or("")
                        .starts_with("Bạn là Lead")
                })
                .unwrap_or(false)
        })
        .last()
        .expect("a Lead record");
    let argv: Vec<String> = rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();
    let i = argv.iter().position(|a| a == "-p").unwrap();
    argv[i + 1].clone()
}

// ---------------------------------------------------------------------------
// Plan 05.4-03 revised by D-48: plans of Telegram-created tasks are approved on Telegram
// (two-step confirm) while the project is in `telegram_projects`; the dashboard still works
// ---------------------------------------------------------------------------

const ORIGIN_LINE: &str = "Task giao từ Telegram — xem file plan trước khi duyệt; plan có thể chứa nội dung từ tin nhắn bạn gửi hoặc chuyển tiếp.";
const REFUSED: &str = "Dự án không còn trong danh sách Telegram — duyệt plan trên dashboard.";
const PROJECT_LINE: &str = "Dự án: web-ban-hang · profile work · nhánh task/sua-app";
const CONFIRM_PREFIX: &str = "Duyệt plan task 1?";

/// `auto --start --origin telegram` (what the bot does): Lead, plan review, gate.
fn auto_start_telegram_to_gate(sb: &Sandbox, sc: Value) {
    let res = sb.run(
        &[
            "auto",
            "--start",
            "--origin",
            "telegram",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(sc),
    );
    assert_ok(&res, "auto --start --origin telegram");
    assert_eq!(task_auto_state(sb, 1).as_deref(), Some("gate"));
}

fn approved_at(sb: &Sandbox, task_id: i64) -> Option<i64> {
    sb.db()
        .query_row(
            "SELECT approved_at FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
}

/// A Telegram-origin task at the gate, its project listed (or not) in `telegram_projects`.
fn telegram_gate(allowed: bool, sc: Value) -> (Sandbox, FakeTelegram, Serve, Value) {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");
    if allowed {
        allow_projects(&sb, &["web-ban-hang"]);
    }
    let srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    auto_start_telegram_to_gate(&sb, sc);
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    (sb, tg, srv, gate)
}

#[test]
fn gate_approve_button_follows_the_allowlist() {
    // Listed project: the button is there, with the new origin line.
    let (sb, _tg, mut srv, gate) = telegram_gate(true, gate_scenario());
    assert_eq!(
        labels(&gate),
        vec!["Duyệt plan".to_string(), "Trả về Lead".to_string()],
        "{gate}"
    );
    assert!(
        button_data(&gate, "Duyệt plan").starts_with("p:1:"),
        "{gate}"
    );
    let lines: Vec<&str> = text_of(&gate).lines().collect();
    let project = lines
        .iter()
        .position(|l| *l == PROJECT_LINE)
        .unwrap_or_else(|| panic!("no project line in {gate}"));
    assert_eq!(lines.get(project + 1).copied(), Some(ORIGIN_LINE), "{gate}");
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));

    // Project not listed (fail closed): no button.
    let (sb, _tg, mut srv, gate) = telegram_gate(false, gate_scenario());
    assert_eq!(labels(&gate), vec!["Trả về Lead".to_string()], "{gate}");
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn approve_asks_then_confirm_approves() {
    let (sb, tg, mut srv, gate) = telegram_gate(true, full_flow_scenario());
    let m = message_id_of(&gate);

    // First press: confirmation screen, nothing approved.
    press(&tg, PAIRED_CHAT, PAIRED_CHAT, &gate, "Duyệt plan");
    let ask = wait_edit(&tg, &srv, m, |r| text_of(r).starts_with(CONFIRM_PREFIX));
    assert!(text_of(&ask).contains("Bấm Xác nhận"), "{ask}");
    assert_eq!(
        labels(&ask),
        vec!["Xác nhận duyệt".to_string(), "Huỷ".to_string()]
    );
    assert_eq!(approved_at(&sb, 1), None);
    assert_eq!(approved_commit(&sb, 1), None);

    // Second press approves; autopilot continues.
    let cb = press_on(&tg, m, &ask, "Xác nhận duyệt");
    wait_answer(&tg, &srv, &cb);
    let done = wait_edit(&tg, &srv, m, |r| {
        text_of(r).contains("✓ Đã duyệt plan") && text_of(r).contains("— từ Telegram lúc")
    });
    assert!(empty_keyboard(&done), "{done}");
    assert!(
        text_of(&done).starts_with("Chờ duyệt plan · task 1"),
        "{done}"
    );
    assert!(approved_at(&sb, 1).is_some());
    assert!(approved_commit(&sb, 1).is_some());
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    assert!(
        stdout_of(&srv).contains("Telegram: Duyệt plan task 1 — Yêu cầu xác nhận duyệt"),
        "{}",
        stdout_of(&srv)
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn approve_cancel_restores_the_gate() {
    let (sb, tg, mut srv, gate) = telegram_gate(true, gate_scenario());
    let m = message_id_of(&gate);
    press(&tg, PAIRED_CHAT, PAIRED_CHAT, &gate, "Duyệt plan");
    let ask = wait_edit(&tg, &srv, m, |r| text_of(r).starts_with(CONFIRM_PREFIX));
    press_on(&tg, m, &ask, "Huỷ");
    let back = wait_edit(&tg, &srv, m, |r| {
        text_of(r).starts_with("Chờ duyệt plan · task 1")
    });
    assert_eq!(
        labels(&back),
        vec!["Duyệt plan".to_string(), "Trả về Lead".to_string()],
        "{back}"
    );
    assert!(button_data(&back, "Duyệt plan").starts_with("p:1:"));
    assert_eq!(approved_at(&sb, 1), None);
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("gate"));
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn approve_confirm_refused_when_project_leaves_the_allowlist() {
    let (sb, tg, mut srv, gate) = telegram_gate(true, full_flow_scenario());
    let m = message_id_of(&gate);
    press(&tg, PAIRED_CHAT, PAIRED_CHAT, &gate, "Duyệt plan");
    let ask = wait_edit(&tg, &srv, m, |r| text_of(r).starts_with(CONFIRM_PREFIX));

    // The project leaves the list between the two presses.
    allow_projects(&sb, &[]);
    let cb = press_on(&tg, m, &ask, "Xác nhận duyệt");
    let ans = wait_answer(&tg, &srv, &cb);
    assert_eq!(ans["json"]["text"], json!(REFUSED), "{ans}");
    assert_eq!(ans["json"]["show_alert"], json!(true), "{ans}");
    assert_eq!(approved_at(&sb, 1), None);
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("gate"));

    // A forged one-press `a:` is refused for the same reason (fail closed at press time).
    let rev = button_data(&gate, "Trả về Lead")
        .strip_prefix("r:1:")
        .expect("r:1:{rev} data")
        .to_string();
    let cb = press_data(&tg, m, text_of(&gate), &format!("a:1:{rev}"));
    let ans = wait_answer(&tg, &srv, &cb);
    assert_eq!(ans["json"]["text"], json!(REFUSED), "{ans}");
    assert_eq!(approved_commit(&sb, 1), None);

    // The dashboard still approves the same task and autopilot continues.
    let resp = srv.post_json("/api/tasks/1/approve", &json!({}));
    assert_eq!(resp.status, 200, "{}", resp.text());
    let job_id = resp.json()["job_id"]
        .as_u64()
        .unwrap_or_else(|| panic!("no job_id in {}", resp.text()));
    assert!(approved_commit(&sb, 1).is_some());
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn approve_old_button_is_stale_and_strangers_are_ignored() {
    let (sb, tg, mut srv, gate) = telegram_gate(true, full_flow_scenario());
    let m = message_id_of(&gate);

    // A press from another chat is ignored: no answer, nothing asked.
    let data = button_data(&gate, "Duyệt plan");
    let stranger = tg.callback_update(STRANGER_CHAT, STRANGER_CHAT, m, text_of(&gate), &data);
    let stranger_id = stranger["callback_query"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    tg.push_update(stranger);

    // The dashboard approves: the gate message's rev is now stale.
    let resp = srv.post_json("/api/tasks/1/approve", &json!({}));
    assert_eq!(resp.status, 200, "{}", resp.text());
    let job_id = resp.json()["job_id"].as_u64().expect("job_id");
    let dash_commit = approved_commit(&sb, 1).expect("dashboard approval");
    let cb = press(&tg, PAIRED_CHAT, PAIRED_CHAT, &gate, "Duyệt plan");
    let ans = wait_answer(&tg, &srv, &cb);
    assert!(
        ans["json"]["text"]
            .as_str()
            .unwrap_or("")
            .starts_with("Task đã đổi trạng thái"),
        "{ans}"
    );
    assert!(answers_for(&tg, &stranger_id).is_empty());
    assert!(edits_of(&tg, m)
        .iter()
        .all(|r| !text_of(r).starts_with(CONFIRM_PREFIX)));
    assert_eq!(approved_commit(&sb, 1), Some(dash_commit));
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn non_telegram_task_still_approves_in_one_press() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");
    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    auto_start_to_gate(&sb, full_flow_scenario());
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    assert!(
        button_data(&gate, "Duyệt plan").starts_with("a:1:"),
        "{gate}"
    );
    let m = message_id_of(&gate);
    press(&tg, PAIRED_CHAT, PAIRED_CHAT, &gate, "Duyệt plan");
    let done = wait_edit(&tg, &srv, m, |r| text_of(r).contains("✓ Đã duyệt plan"));
    assert!(empty_keyboard(&done), "{done}");
    assert!(approved_commit(&sb, 1).is_some());
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn gate_title_is_first_line() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let res = sb.run(
        &[
            "run",
            "lead",
            "--origin",
            "telegram",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "dòng một\ndòng hai",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead --origin telegram");

    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let lines: Vec<&str> = text_of(&gate).lines().collect();
    assert_eq!(lines.get(1).copied(), Some("dòng một"), "{gate}");
    assert!(!lines.iter().any(|l| *l == "dòng hai"), "{gate}");
    assert_eq!(labels(&gate), vec!["Trả về Lead".to_string()], "{gate}");
    assert!(text_of(&gate).contains(ORIGIN_LINE), "{gate}");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// Plan 05.4-05: `/task` intake (typed request -> project -> workflow -> confirm)
// ---------------------------------------------------------------------------

/// Sets the top-level `telegram_projects` allowlist of projects_map.json (OD-1).
fn allow_projects(sb: &Sandbox, ids: &[&str]) {
    let mut map = sb.read_map();
    map["telegram_projects"] = json!(ids);
    std::fs::write(sb.map_path(), serde_json::to_string_pretty(&map).unwrap()).expect("write map");
}

/// A plain text message from the paired user in the paired chat.
fn send_text(tg: &FakeTelegram, text: &str) {
    tg.push_update(tg.message_update(PAIRED_CHAT, PAIRED_CHAT, "private", text));
}

/// Pushes a press of `data` on message `message_id`; returns the callback id.
fn press_data(tg: &FakeTelegram, message_id: i64, text: &str, data: &str) -> String {
    let up = tg.callback_update(PAIRED_CHAT, PAIRED_CHAT, message_id, text, data);
    let id = up["callback_query"]["id"].as_str().unwrap().to_string();
    tg.push_update(up);
    id
}

/// Presses button `label` of `rec` (a sent message or an edit); returns the callback id.
fn press_on(tg: &FakeTelegram, message_id: i64, rec: &Value, label: &str) -> String {
    press_data(tg, message_id, text_of(rec), &button_data(rec, label))
}

fn is_flow_id(s: &str) -> bool {
    s.len() == 8 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The 8-hex flow id inside `iP:{flow}:0` style data.
fn flow_of(data: &str) -> String {
    data.split(':').nth(1).unwrap_or("").to_string()
}

/// The team step (TEAM-03) after the workflow step: taps the project default (the first button).
fn tap_default_team(tg: &FakeTelegram, srv: &Serve, mid: i64) {
    let team = wait_edit(tg, srv, mid, |r| {
        text_of(r).starts_with("Chọn đội hình cho task:")
    });
    let default = labels(&team)[0].clone();
    assert!(default.starts_with("Mặc định ("), "{team}");
    press_on(tg, mid, &team, &default);
}

fn task_row(sb: &Sandbox, task_id: i64) -> (Option<String>, String) {
    sb.db()
        .query_row(
            "SELECT origin, title FROM tasks WHERE id = ?1",
            [task_id],
            |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, String>(1)?)),
        )
        .unwrap()
}

/// Serve output lines (stdout and stderr) that start with `Telegram:`.
fn telegram_lines(srv: &Serve) -> Vec<String> {
    format!("{}\n{}", stdout_of(srv), stderr_of(srv))
        .lines()
        .filter(|l| l.starts_with("Telegram:"))
        .map(|l| l.to_string())
        .collect()
}

#[test]
fn task_command_full_flow() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);
    sb.write_scenario(lead_plan_entry());

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    send_text(&tg, "/task sửa app");
    let screen = wait_message(&tg, &srv, "Chọn dự án cho task:");
    assert!(text_of(&screen).contains("«sửa app»"), "{screen}");
    let b = buttons(&screen);
    assert_eq!(b.len(), 2, "{screen}");
    let flow = flow_of(&b[0].1);
    assert!(is_flow_id(&flow), "{screen}");
    assert_eq!(
        b,
        vec![
            ("Web bán hàng · work".to_string(), format!("iP:{flow}:0")),
            ("Huỷ".to_string(), format!("iX:{flow}")),
        ]
    );
    let mid = message_id_of(&screen);

    // Project -> workflow screen with the project's default first.
    press_on(&tg, mid, &screen, "Web bán hàng · work");
    let wf_screen = wait_edit(&tg, &srv, mid, |r| {
        text_of(r).starts_with("Chọn quy trình · dự án web-ban-hang")
    });
    assert_eq!(
        labels(&wf_screen).first().map(String::as_str),
        Some("Mặc định (nhanh)"),
        "{wf_screen}"
    );

    // Workflow -> team screen (TEAM-03) -> confirm screen.
    press_on(&tg, mid, &wf_screen, "Mặc định (nhanh)");
    tap_default_team(&tg, &srv, mid);
    let confirm = wait_edit(&tg, &srv, mid, |r| {
        text_of(r).starts_with("Xác nhận giao việc")
    });
    assert!(
        text_of(&confirm).contains("Nhánh: task/sua-app"),
        "{confirm}"
    );
    assert!(
        text_of(&confirm).contains(
            "Plan sẽ gửi lại để bạn duyệt: trên Telegram (bấm hai lần) hoặc trên dashboard."
        ),
        "{confirm}"
    );
    assert_eq!(
        labels(&confirm),
        vec!["Tạo task".to_string(), "Huỷ".to_string()]
    );
    assert_eq!(task_count(&sb), 0, "nothing is created before Tạo task");

    // Tạo task -> exactly one Telegram-origin job.
    press_on(&tg, mid, &confirm, "Tạo task");
    let done = wait_edit(&tg, &srv, mid, |r| {
        text_of(r).starts_with("Đã nhận — đang lập plan · dự án web-ban-hang · nhánh task/sua-app")
    });
    assert!(empty_keyboard(&done), "{done}");

    assert_eq!(job_role(&srv, 1), "lead");
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    assert_eq!(srv.get("/api/jobs/2").status, 404);
    assert_eq!(task_count(&sb), 1);
    let (origin, title) = task_row(&sb, 1);
    assert_eq!(origin.as_deref(), Some("telegram"));
    assert_eq!(title, "sửa app");
    assert!(
        last_lead_prompt(&sb).contains("Yêu cầu: sửa app"),
        "{}",
        last_lead_prompt(&sb)
    );

    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    assert_eq!(
        labels(&gate),
        vec!["Duyệt plan".to_string(), "Trả về Lead".to_string()],
        "{gate}"
    );

    // The request text is never printed on the terminal.
    for line in telegram_lines(&srv) {
        assert!(!line.contains("sửa app"), "request leaked: {line}");
    }
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// Phase 6 plan 14 Task 3: the team step of `/task` (TEAM-03)
// ---------------------------------------------------------------------------

/// teams.json with `doi-b` (Dev on agy) and, in the map, the project's default team.
fn setup_team_flow(sb: &Sandbox, project_team: &str) {
    sb.write_teams(json!({
        "doi-b": {"description": "Dev trên agy", "roles": {"dev": "agy"}}
    }));
    setup_project(sb, "work");
    allow_projects(sb, &["web-ban-hang"]);
    let mut map = sb.read_map();
    map["projects"][0]["team"] = json!(project_team);
    std::fs::write(sb.map_path(), serde_json::to_string_pretty(&map).unwrap()).expect("write map");
    sb.write_scenario(lead_plan_entry());
}

fn team_name_of(sb: &Sandbox, task_id: i64) -> String {
    let json: Option<String> = sb
        .db()
        .query_row(
            "SELECT team_json FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap();
    let v: Value = serde_json::from_str(&json.expect("team_json")).unwrap();
    v["name"].as_str().unwrap_or("").to_string()
}

/// `/task sửa app` -> project -> default workflow; returns the flow message id and the team screen.
fn reach_team_screen(tg: &FakeTelegram, srv: &Serve) -> (i64, Value) {
    send_text(tg, "/task sửa app");
    let screen = wait_message(tg, srv, CHOOSE_PROJECT);
    let mid = message_id_of(&screen);
    press_on(tg, mid, &screen, "Web bán hàng · work");
    let wf = wait_edit(tg, srv, mid, |r| text_of(r).starts_with("Chọn quy trình"));
    press_on(tg, mid, &wf, "Mặc định (nhanh)");
    let team = wait_edit(tg, srv, mid, |r| {
        text_of(r).starts_with("Chọn đội hình cho task:")
    });
    (mid, team)
}

#[test]
fn intake_team_step_choose_team() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_team_flow(&sb, "tiet-kiem");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let (mid, team) = reach_team_screen(&tg, &srv);
    assert_eq!(
        labels(&team),
        vec![
            "Mặc định (tiet-kiem)".to_string(),
            "mac-dinh".to_string(),
            "tiet-kiem".to_string(),
            "doi-b".to_string(),
            "Huỷ".to_string(),
        ],
        "{team}"
    );
    let b = buttons(&team);
    let flow = flow_of(&b[0].1);
    assert!(is_flow_id(&flow), "{team}");
    for (i, (_, data)) in b[..4].iter().enumerate() {
        assert_eq!(data, &format!("iD:{flow}:{i}"));
    }
    assert_eq!(b[4].1, format!("iX:{flow}"));

    press_on(&tg, mid, &team, "doi-b");
    let confirm = wait_edit(&tg, &srv, mid, |r| {
        text_of(r).starts_with("Xác nhận giao việc")
    });
    assert!(
        text_of(&confirm)
            .contains("Đội hình: doi-b (claude: lead, qa, review plan, review code · agy: dev)"),
        "{confirm}"
    );
    assert_eq!(task_count(&sb), 0, "nothing is created before Tạo task");

    press_on(&tg, mid, &confirm, "Tạo task");
    wait_edit(&tg, &srv, mid, |r| {
        text_of(r).starts_with("Đã nhận — đang lập plan")
    });
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    assert_eq!(team_name_of(&sb, 1), "doi-b");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn intake_team_default() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_team_flow(&sb, "doi-b");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let (mid, team) = reach_team_screen(&tg, &srv);
    assert_eq!(
        labels(&team).first().map(String::as_str),
        Some("Mặc định (doi-b)"),
        "{team}"
    );
    press_on(&tg, mid, &team, "Mặc định (doi-b)");
    let confirm = wait_edit(&tg, &srv, mid, |r| {
        text_of(r).starts_with("Xác nhận giao việc")
    });
    assert!(
        text_of(&confirm).contains("Đội hình: doi-b ("),
        "the project default is shown: {confirm}"
    );

    press_on(&tg, mid, &confirm, "Tạo task");
    wait_edit(&tg, &srv, mid, |r| {
        text_of(r).starts_with("Đã nhận — đang lập plan")
    });
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    // The request went without a team: the server took the project's own.
    assert_eq!(team_name_of(&sb, 1), "doi-b");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn intake_team_bad_index() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_team_flow(&sb, "tiet-kiem");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let (mid, team) = reach_team_screen(&tg, &srv);
    let flow = flow_of(&buttons(&team)[0].1);
    // Four teams (indexes 1..=3 after the default 0): 4 and 99 are past the list.
    for bad in [format!("iD:{flow}:4"), format!("iD:{flow}:99")] {
        let cb = press_data(&tg, mid, text_of(&team), &bad);
        let ans = wait_answer(&tg, &srv, &cb);
        assert_eq!(ans["json"]["text"], json!(INVALID), "{bad}: {ans}");
    }
    // The flow stays on the team step: a valid press still works.
    assert_eq!(task_count(&sb), 0);
    press_on(&tg, mid, &team, "tiet-kiem");
    let confirm = wait_edit(&tg, &srv, mid, |r| {
        text_of(r).starts_with("Xác nhận giao việc")
    });
    assert!(
        text_of(&confirm).contains("Đội hình: tiet-kiem ("),
        "{confirm}"
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// Plan 05.4-05 Task 2: other ways to start, abandon or revisit a flow
// ---------------------------------------------------------------------------

const ASK_REQUEST: &str =
    "Gửi yêu cầu cho task mới (một tin nhắn), hoặc bấm Huỷ. Hết hạn sau 10 phút.";
const CHOOSE_PROJECT: &str = "Chọn dự án cho task:";
const CANCELLED: &str = "Đã huỷ — không tạo task.";
const REPLACED: &str = "Đã huỷ — có yêu cầu mới.";
const STALE_FLOW: &str = "Yêu cầu đã hết hạn — gõ /task để bắt đầu lại";
const NO_FLOW: &str = "Không có yêu cầu nào đang mở.";
const NO_ALLOWLIST: &str = "Chưa có telegram_projects trong projects_map.json — thêm id dự án được phép để giao việc từ Telegram.";
const UNKNOWN_IDS: &str =
    "Telegram: telegram_projects có id không có trong projects_map.json: khong-co";
const NOTE_ASK: &str = "Gửi ghi chú cho Lead hoặc bấm Không ghi chú";

/// The first sendMessage starting with `prefix` whose message id is not in `seen`.
fn wait_new_message(tg: &FakeTelegram, srv: &Serve, prefix: &str, seen: &[i64]) -> Value {
    match tg.wait_call("sendMessage", Duration::from_secs(15), |r| {
        text_of(r).starts_with(prefix)
            && r["response"]["result"]["message_id"]
                .as_i64()
                .is_some_and(|m| !seen.contains(&m))
    }) {
        Some(r) => r,
        None => panic!(
            "no new sendMessage starting with {prefix:?}; records:\n{:#?}\nstdout:\n{}\nstderr:\n{}",
            tg.records(),
            stdout_of(srv),
            stderr_of(srv)
        ),
    }
}

fn sent_texts(tg: &FakeTelegram, text: &str) -> usize {
    tg.calls("sendMessage")
        .iter()
        .filter(|r| text_of(r) == text)
        .count()
}

fn flow_messages(tg: &FakeTelegram) -> usize {
    tg.calls("sendMessage")
        .iter()
        .filter(|r| text_of(r).starts_with(CHOOSE_PROJECT) || text_of(r) == ASK_REQUEST)
        .count()
}

/// `/task sửa app` up to the confirm screen; returns the flow message id and the screen.
fn reach_confirm(tg: &FakeTelegram, srv: &Serve) -> (i64, Value) {
    reach_confirm_for(tg, srv, "sửa app")
}

/// Ids of every message the bot has sent so far.
fn sent_ids(tg: &FakeTelegram) -> Vec<i64> {
    tg.calls("sendMessage")
        .iter()
        .filter_map(|r| r["response"]["result"]["message_id"].as_i64())
        .collect()
}

/// `/task {request}` -> first project -> default workflow; returns the flow
/// message id and the confirm screen. Works for a second flow in the same test.
fn reach_confirm_for(tg: &FakeTelegram, srv: &Serve, request: &str) -> (i64, Value) {
    let seen = sent_ids(tg);
    send_text(tg, &format!("/task {request}"));
    let screen = wait_new_message(tg, srv, CHOOSE_PROJECT, &seen);
    let mid = message_id_of(&screen);
    press_on(tg, mid, &screen, "Web bán hàng · work");
    let wf = wait_edit(tg, srv, mid, |r| text_of(r).starts_with("Chọn quy trình"));
    press_on(tg, mid, &wf, "Mặc định (nhanh)");
    tap_default_team(tg, srv, mid);
    let confirm = wait_edit(tg, srv, mid, |r| {
        text_of(r).starts_with("Xác nhận giao việc")
    });
    (mid, confirm)
}

#[test]
fn task_command_asks_then_takes_text() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    // Bare `/task` asks for the request.
    send_text(&tg, "/task");
    let ask = wait_message(&tg, &srv, ASK_REQUEST);
    assert_eq!(text_of(&ask), ASK_REQUEST);
    let b = buttons(&ask);
    assert_eq!(b.len(), 1, "{ask}");
    assert_eq!(b[0].0, "Huỷ");
    let flow = b[0].1.strip_prefix("iX:").expect("iX:{flow}").to_string();
    assert!(is_flow_id(&flow), "{ask}");
    let mid = message_id_of(&ask);

    // The next text is the request: the same message becomes the project screen.
    send_text(&tg, "sửa app");
    let screen = wait_edit(&tg, &srv, mid, |r| text_of(r).starts_with(CHOOSE_PROJECT));
    assert!(text_of(&screen).contains("«sửa app»"), "{screen}");
    assert_eq!(
        buttons(&screen),
        vec![
            ("Web bán hàng · work".to_string(), format!("iP:{flow}:0")),
            ("Huỷ".to_string(), format!("iX:{flow}")),
        ]
    );
    // The request was not answered with the help line.
    assert_eq!(sent_texts(&tg, HELP), 0, "{:#?}", tg.records());

    // `/task@bot <request>` works like `/task <request>` and replaces the open flow.
    send_text(&tg, "/task@ab_fake_bot sửa app");
    let second = wait_new_message(&tg, &srv, CHOOSE_PROJECT, &[mid]);
    assert!(text_of(&second).contains("«sửa app»"), "{second}");
    wait_edit(&tg, &srv, mid, |r| text_of(r) == REPLACED);

    for line in telegram_lines(&srv) {
        assert!(!line.contains("sửa app"), "request leaked: {line}");
    }
    assert_eq!(task_count(&sb), 0);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn allowlist_missing_key_refuses() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    send_text(&tg, "/task sửa app");
    let refusal = wait_message(&tg, &srv, "Chưa có telegram_projects");
    assert_eq!(text_of(&refusal), NO_ALLOWLIST);
    assert!(buttons(&refusal).is_empty(), "{refusal}");

    // An empty list refuses the same way.
    allow_projects(&sb, &[]);
    send_text(&tg, "/task sửa app");
    assert!(
        wait_until(Duration::from_secs(10), || sent_texts(&tg, NO_ALLOWLIST)
            == 2),
        "{:#?}",
        tg.records()
    );
    assert_eq!(flow_messages(&tg), 0, "{:#?}", tg.records());
    assert_eq!(task_count(&sb), 0);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn allowlist_offers_only_listed_runnable() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let web = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let cu = sb.make_repo("cu-hong", &[], None, None);
    let khac = sb.make_repo("khac", &[], None, None);
    let mut web_e = sb.project_entry(&web, "web-ban-hang", "work");
    web_e["name_vi"] = json!("Web bán hàng");
    let mut cu_e = sb.project_entry(&cu, "cu-hong", "work");
    cu_e["missing"] = json!(true);
    let khac_e = sb.project_entry(&khac, "khac", "work");
    sb.write_map(json!([web_e, cu_e, khac_e]), &[]);
    allow_projects(&sb, &["web-ban-hang", "cu-hong", "khong-co"]);

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    send_text(&tg, "/task sửa app");
    let first = wait_message(&tg, &srv, CHOOSE_PROJECT);
    assert_eq!(
        labels(&first),
        vec!["Web bán hàng · work".to_string(), "Huỷ".to_string()],
        "{first}"
    );
    send_text(&tg, "/task sửa app");
    let second = wait_new_message(&tg, &srv, CHOOSE_PROJECT, &[message_id_of(&first)]);
    assert_eq!(
        labels(&second),
        vec!["Web bán hàng · work".to_string(), "Huỷ".to_string()],
        "{second}"
    );

    // The unknown id is warned once per serve.
    let out = format!("{}\n{}", stdout_of(&srv), stderr_of(&srv));
    assert_eq!(count_lines(&out, UNKNOWN_IDS), 1, "{out}");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn allowlist_rechecked_at_confirm() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);
    sb.write_scenario(lead_plan_entry());

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let (mid, confirm) = reach_confirm(&tg, &srv);
    // The project leaves the allowlist while the confirm screen is open.
    let khac = sb.make_repo("khac", &[], None, None);
    let mut map = sb.read_map();
    let khac_e = sb.project_entry(&khac, "khac", "work");
    map["projects"].as_array_mut().unwrap().push(khac_e);
    std::fs::write(sb.map_path(), serde_json::to_string_pretty(&map).unwrap()).unwrap();
    allow_projects(&sb, &["khac"]);

    press_on(&tg, mid, &confirm, "Tạo task");
    let refused = wait_edit(&tg, &srv, mid, |r| {
        text_of(r).starts_with("Không giao được: web-ban-hang không còn trong telegram_projects")
    });
    assert!(empty_keyboard(&refused), "{refused}");
    assert_eq!(srv.get("/api/jobs/1").status, 404);
    assert_eq!(task_count(&sb), 0);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn allowlist_survives_projects_scan() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);
    let root = repo.parent().unwrap().to_str().unwrap().to_string();

    let res = sb.run(&["projects", "scan", "--root", &root], None);
    assert_ok(&res, "projects scan");
    let map = sb.read_map();
    assert_eq!(map["telegram_projects"], json!(["web-ban-hang"]), "{map}");
    assert!(
        map["scan_roots"].as_array().is_some_and(|r| !r.is_empty()),
        "scan did not write the map: {map}"
    );
}

#[test]
fn flow_state_cancel() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    // `/huy` cancels the open flow.
    send_text(&tg, "/task sửa app");
    let first = wait_message(&tg, &srv, CHOOSE_PROJECT);
    let m1 = message_id_of(&first);
    send_text(&tg, "/huy");
    let cancelled = wait_edit(&tg, &srv, m1, |r| text_of(r) == CANCELLED);
    assert!(empty_keyboard(&cancelled), "{cancelled}");

    // Nothing open: `/huy` says so.
    send_text(&tg, "/huy");
    let none = wait_message(&tg, &srv, NO_FLOW);
    assert_eq!(text_of(&none), NO_FLOW);

    // The `Huỷ` button cancels too.
    send_text(&tg, "/task sửa app");
    let second = wait_new_message(&tg, &srv, CHOOSE_PROJECT, &[m1]);
    let m2 = message_id_of(&second);
    let cb = press_on(&tg, m2, &second, "Huỷ");
    let ans = wait_answer(&tg, &srv, &cb);
    assert_eq!(ans["json"]["text"], json!("Đã huỷ"), "{ans}");
    let edit = wait_edit(&tg, &srv, m2, |r| text_of(r) == CANCELLED);
    assert!(empty_keyboard(&edit), "{edit}");
    assert_eq!(task_count(&sb), 0);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn flow_state_replaced_and_stale() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    send_text(&tg, "/task một");
    let first = wait_message(&tg, &srv, CHOOSE_PROJECT);
    let m1 = message_id_of(&first);
    send_text(&tg, "/task hai");
    let second = wait_new_message(&tg, &srv, CHOOSE_PROJECT, &[m1]);
    let m2 = message_id_of(&second);
    assert!(text_of(&second).contains("«hai»"), "{second}");
    let replaced = wait_edit(&tg, &srv, m1, |r| text_of(r) == REPLACED);
    assert!(empty_keyboard(&replaced), "{replaced}");

    // A button of the replaced flow is stale and its message loses its buttons.
    let stale_data = button_data(&first, "Web bán hàng · work");
    let before = tg.calls("editMessageReplyMarkup").len();
    let cb = press_data(&tg, m1, text_of(&first), &stale_data);
    let ans = wait_answer(&tg, &srv, &cb);
    assert_eq!(ans["json"]["text"], json!(STALE_FLOW), "{ans}");
    let strip = tg
        .wait_call("editMessageReplyMarkup", Duration::from_secs(10), |r| {
            r["json"]["message_id"].as_i64() == Some(m1)
        })
        .unwrap_or_else(|| panic!("no keyboard-only edit; records:\n{:#?}", tg.records()));
    assert!(empty_keyboard(&strip), "{strip}");
    assert!(tg.calls("editMessageReplyMarkup").len() > before);

    // A crafted press with an unknown flow id on the current message is stale too.
    let cb = press_data(&tg, m2, text_of(&second), "iP:00000000:0");
    let ans = wait_answer(&tg, &srv, &cb);
    assert_eq!(ans["json"]["text"], json!(STALE_FLOW), "{ans}");
    assert_eq!(task_count(&sb), 0);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn flow_state_note_prompt_exclusive() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    auto_start_to_gate(&sb, gate_scenario());
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let cb = press(&tg, PAIRED_CHAT, PAIRED_CHAT, &gate, "Trả về Lead");
    let ans = wait_answer(&tg, &srv, &cb);
    assert_eq!(ans["json"]["text"], json!(NOTE_ASK), "{ans}");
    let prompt = wait_message(&tg, &srv, "Ghi chú cho Lead (task 1)");
    let p = message_id_of(&prompt);

    // Opening a flow closes the note prompt.
    send_text(&tg, "/task sửa x");
    let closed = wait_edit(&tg, &srv, p, |r| text_of(r) == REPLACED);
    assert!(empty_keyboard(&closed), "{closed}");
    wait_message(&tg, &srv, CHOOSE_PROJECT);

    // The next text is not a note any more.
    send_text(&tg, "abc");
    assert!(
        wait_until(Duration::from_secs(10), || sent_texts(&tg, HELP) == 1),
        "{:#?}",
        tg.records()
    );
    assert_eq!(srv.get("/api/jobs/1").status, 404);
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("gate"));
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn unknown_command_gets_help() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    for text in ["/Task x", "/create-task x", "/start"] {
        send_text(&tg, text);
    }
    assert!(
        wait_until(Duration::from_secs(10), || sent_texts(&tg, HELP) == 3),
        "{:#?}",
        tg.records()
    );
    assert_eq!(flow_messages(&tg), 0, "{:#?}", tg.records());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn forwarded_task_is_not_a_command() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);

    let mut srv = tg.serve(&sb, &[QUIET]);
    wait_polling(&tg, &srv);

    let mut up = tg.message_update(PAIRED_CHAT, PAIRED_CHAT, "private", "/task sửa app");
    up["message"]["forward_origin"] =
        json!({"type": "hidden_user", "sender_user_name": "Khách", "date": 1});
    tg.push_update(up);
    // Plan 05.4-07: the forward is collected as data, not run as `/task`.
    let selection = wait_message(&tg, &srv, "Đã nhận 1 tin chuyển tiếp");
    assert!(text_of(&selection).contains("/task sửa app"), "{selection}");
    assert_eq!(sent_texts(&tg, HELP), 0, "{:#?}", tg.records());
    assert_eq!(flow_messages(&tg), 0, "{:#?}", tg.records());
    for line in telegram_lines(&srv) {
        assert!(!line.contains("sửa app"), "forwarded text leaked: {line}");
    }
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// Plan 05.4-06: outcome report, daily cap, command menu
// ---------------------------------------------------------------------------

const ACCEPTED_EDIT: &str = "Đã nhận — đang lập plan · dự án web-ban-hang · nhánh task/sua-app";
const CREATED_TEXT: &str = "Đã tạo task 1 · task/sua-app. Plan đang được lập; khi xong sẽ có tin chờ duyệt — duyệt plan trên Telegram hoặc dashboard.";
const FAILED_START: &str = "Không tạo được task (task/sua-app):";

/// `/task {request}` -> project 0 -> default workflow -> `Tạo task`; waits for the
/// `Đã nhận …` edit and returns the flow message id.
fn confirm_task(tg: &FakeTelegram, srv: &Serve, request: &str) -> i64 {
    let (mid, confirm) = reach_confirm_for(tg, srv, request);
    press_on(tg, mid, &confirm, "Tạo task");
    let done = wait_edit(tg, srv, mid, |r| {
        text_of(r).starts_with("Đã nhận — đang lập plan")
    });
    assert!(empty_keyboard(&done), "{done}");
    mid
}

fn sent_starting(tg: &FakeTelegram, prefix: &str) -> usize {
    tg.calls("sendMessage")
        .iter()
        .filter(|r| text_of(r).starts_with(prefix))
        .count()
}

/// Index in the record log of the first record matching `pred`.
fn record_index(tg: &FakeTelegram, pred: impl Fn(&Value) -> bool) -> Option<usize> {
    tg.records().iter().position(pred)
}

#[test]
fn outcome_reported_created() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);
    sb.write_scenario(lead_plan_entry());

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let mid = confirm_task(&tg, &srv, "sửa app");
    let created = wait_message(&tg, &srv, "Đã tạo task 1 · task/sua-app.");
    assert_eq!(text_of(&created), CREATED_TEXT);
    assert!(buttons(&created).is_empty(), "{created}");

    // The `Đã nhận …` edit came first, then the report.
    let accepted_at = record_index(&tg, |r| {
        r["method"] == "editMessageText"
            && r["json"]["message_id"].as_i64() == Some(mid)
            && text_of(r) == ACCEPTED_EDIT
    })
    .expect("accepted edit");
    let created_at = record_index(&tg, |r| {
        r["method"] == "sendMessage" && text_of(r) == CREATED_TEXT
    })
    .expect("created message");
    assert!(accepted_at < created_at, "{:#?}", tg.records());

    // Short polls while watching, the long poll again after the report.
    let records = tg.records();
    assert!(
        records[..created_at]
            .iter()
            .any(|r| r["method"] == "getUpdates" && r["json"]["timeout"] == json!(1)),
        "no 1-second poll while watching: {records:#?}"
    );
    assert!(
        wait_until(Duration::from_secs(10), || tg.records()[created_at..]
            .iter()
            .any(|r| r["method"] == "getUpdates")),
        "no poll after the report"
    );

    // Exactly one report, even a few seconds later.
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(sent_starting(&tg, "Đã tạo task"), 1, "{:#?}", tg.records());
    assert_eq!(sent_starting(&tg, "Không tạo được task"), 0);
    let after: Vec<Value> = tg.records()[created_at..]
        .iter()
        .filter(|r| r["method"] == "getUpdates")
        .cloned()
        .collect();
    assert!(!after.is_empty());
    for r in &after {
        assert_eq!(r["json"]["timeout"], json!(25), "{r}");
    }
    assert_eq!(task_count(&sb), 1);
    for line in telegram_lines(&srv) {
        assert!(!line.contains("sửa app"), "request leaked: {line}");
    }
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn outcome_reported_failed() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let repo = setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);
    sb.write_scenario(lead_plan_entry());

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let (mid, confirm) = reach_confirm(&tg, &srv);
    // A ref lock makes `git checkout -b task/sua-app` fail before any task row,
    // while the branch still does not exist for the last dry run.
    let refs = repo.join(".git/refs/heads/task");
    std::fs::create_dir_all(&refs).expect("refs dir");
    std::fs::write(refs.join("sua-app.lock"), "").expect("ref lock");

    press_on(&tg, mid, &confirm, "Tạo task");
    wait_edit(&tg, &srv, mid, |r| text_of(r) == ACCEPTED_EDIT);
    let failed = wait_message(&tg, &srv, FAILED_START);
    let lines: Vec<&str> = text_of(&failed).lines().collect();
    assert_eq!(lines[0], FAILED_START, "{failed}");
    assert!(
        (2..=4).contains(&lines.len()),
        "prefix plus 1..3 lines: {failed}"
    );
    for l in &lines[1..] {
        assert!(!l.trim().is_empty() && l.chars().count() <= 200, "{failed}");
    }
    assert!(buttons(&failed).is_empty(), "{failed}");

    let job = wait_for_job(&srv, 1);
    assert_ne!(job["exit_code"], json!(0), "{job}");
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(
        sent_starting(&tg, "Không tạo được task"),
        1,
        "{:#?}",
        tg.records()
    );
    assert_eq!(sent_starting(&tg, "Đã tạo task"), 0, "{:#?}", tg.records());
    assert_eq!(task_count(&sb), 0);
    let out = stdout_of(&srv);
    assert_eq!(
        count_lines(&out, "Telegram: giao việc — job 1 dừng trước khi có task"),
        1,
        "{out}"
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

fn cap_text(n: u32) -> String {
    format!("Đã đủ {n} task giao từ Telegram hôm nay — giao tiếp trên dashboard hoặc chờ sang ngày mai.")
}
const CAP_OFF: &str =
    "Giao việc từ Telegram đang tắt (telegram_max_tasks_per_day = 0 trong config.json).";

/// Allowlisted nhanh project, Lead scenario, `telegram_max_tasks_per_day = cap`.
fn cap_sandbox(cap: u32) -> (Sandbox, FakeTelegram, PathBuf) {
    let sb = Sandbox::new();
    sb.write_config(json!({"telegram_max_tasks_per_day": cap}));
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let repo = setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);
    sb.write_scenario(lead_plan_entry());
    (sb, tg, repo)
}

/// Like `wait_polling`, for a serve started after `polls_before` getUpdates calls
/// (a restart): waits for a long poll of the new serve.
fn wait_polling_again(tg: &FakeTelegram, srv: &Serve, polls_before: usize) {
    assert!(
        wait_until(Duration::from_secs(10), || stdout_of(srv)
            .contains(CONNECTED)),
        "no connected line; stderr:\n{}",
        stderr_of(srv)
    );
    assert!(
        wait_until(Duration::from_secs(10), || tg.calls("getUpdates")
            [polls_before..]
            .iter()
            .any(|r| r["json"]["timeout"].as_i64().unwrap_or(0) > 0)),
        "no long poll after the restart; stderr:\n{}",
        stderr_of(srv)
    );
}

#[test]
fn daily_cap_refuses_at_start() {
    let (sb, tg, _repo) = cap_sandbox(1);
    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    confirm_task(&tg, &srv, "sửa app");
    wait_message(&tg, &srv, "Đã tạo task 1 · task/sua-app.");
    assert_eq!(flow_messages(&tg), 1);

    send_text(&tg, "/task việc khác");
    let refusal = wait_message(&tg, &srv, "Đã đủ");
    assert_eq!(text_of(&refusal), cap_text(1));
    assert!(buttons(&refusal).is_empty(), "{refusal}");
    assert_eq!(flow_messages(&tg), 1, "{:#?}", tg.records());
    assert_eq!(task_count(&sb), 1);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn daily_cap_refuses_at_confirm() {
    let (sb, tg, _repo) = cap_sandbox(1);
    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let (mid, confirm) = reach_confirm(&tg, &srv);
    // A Telegram-origin task appears while the confirm screen is open.
    let res = sb.run(
        &[
            "run",
            "lead",
            "--origin",
            "telegram",
            "--name",
            "khac",
            "--",
            "web-ban-hang",
            "việc khác",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead --origin telegram");
    assert_eq!(task_count(&sb), 1);

    let cb = press_on(&tg, mid, &confirm, "Tạo task");
    let refused = wait_edit(&tg, &srv, mid, |r| text_of(r) == cap_text(1));
    assert!(empty_keyboard(&refused), "{refused}");
    wait_answer(&tg, &srv, &cb);
    assert_eq!(srv.get("/api/jobs/1").status, 404);
    assert_eq!(task_count(&sb), 1);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn daily_cap_counts_only_telegram() {
    let (sb, tg, _repo) = cap_sandbox(1);
    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "việc dashboard", "name": "viec-dashboard"}),
    );
    assert_eq!(resp.status, 202, "{}", resp.text());
    let job_id = resp.json()["job_id"].as_u64().expect("job_id");
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    assert_eq!(task_count(&sb), 1);

    send_text(&tg, "/task sửa app");
    let screen = wait_message(&tg, &srv, CHOOSE_PROJECT);
    assert!(text_of(&screen).contains("«sửa app»"), "{screen}");
    assert_eq!(sent_starting(&tg, "Đã đủ"), 0, "{:#?}", tg.records());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn daily_cap_survives_restart() {
    let (sb, tg, _repo) = cap_sandbox(1);
    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    confirm_task(&tg, &srv, "sửa app");
    wait_message(&tg, &srv, "Đã tạo task 1 · task/sua-app.");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));

    let polls = tg.calls("getUpdates").len();
    let mut srv = tg.serve(&sb, &[]);
    wait_polling_again(&tg, &srv, polls);
    send_text(&tg, "/task x");
    let refusal = wait_message(&tg, &srv, "Đã đủ");
    assert_eq!(text_of(&refusal), cap_text(1));
    assert_eq!(flow_messages(&tg), 1, "{:#?}", tg.records());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn daily_cap_zero_disables() {
    let (sb, tg, _repo) = cap_sandbox(0);
    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    send_text(&tg, "/task x");
    let refusal = wait_message(&tg, &srv, "Giao việc từ Telegram đang tắt");
    assert_eq!(text_of(&refusal), CAP_OFF);
    assert!(buttons(&refusal).is_empty(), "{refusal}");
    assert_eq!(flow_messages(&tg), 0, "{:#?}", tg.records());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn commands_registered() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let calls = tg.calls("setMyCommands");
    assert_eq!(calls.len(), 1, "{:#?}", tg.records());
    let commands = &calls[0]["json"]["commands"];
    assert_eq!(
        commands,
        &json!([
            {"command": "task", "description": "Giao việc mới cho agent-brain"},
            {"command": "huy", "description": "Huỷ yêu cầu giao việc đang mở"},
        ]),
        "{:#?}",
        calls[0]
    );
    for c in commands.as_array().unwrap() {
        let name = c["command"].as_str().unwrap();
        assert!(
            (1..=32).contains(&name.len())
                && name
                    .bytes()
                    .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_')),
            "{name}"
        );
    }
    // Registered after getMe, before the first long poll.
    let records = tg.records();
    let me = records.iter().position(|r| r["method"] == "getMe").unwrap();
    let set = records
        .iter()
        .position(|r| r["method"] == "setMyCommands")
        .unwrap();
    assert!(me < set, "{records:#?}");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// Plan 05.4-07: forwarded messages become one task
// ---------------------------------------------------------------------------

/// Serve env: the forward collection window closes after 300 ms without a new forward.
const QUIET: (&str, &str) = ("AGENT_BRAIN_TELEGRAM_INTAKE_QUIET_MS", "300");
const OVER_CAP: &str = "Chỉ nhận tối đa 20 tin một lần — các tin sau bị bỏ qua.";
const D1: i64 = 1_792_600_000;
const D2: i64 = D1 + 60;
const D3: i64 = D1 + 120;

/// A message forwarded by the paired user into chat `chat`. Without `text` it is
/// a photo (no file is ever fetched) with the optional `caption`.
fn forward_update(
    tg: &FakeTelegram,
    chat: i64,
    text: Option<&str>,
    caption: Option<&str>,
    origin: Value,
) -> Value {
    let mut up = tg.message_update(chat, chat, "private", text.unwrap_or(""));
    let m = up["message"].as_object_mut().unwrap();
    if text.is_none() {
        m.remove("text");
        m.insert(
            "photo".to_string(),
            json!([{"file_id": "f1", "file_unique_id": "u1", "width": 1, "height": 1}]),
        );
        if let Some(c) = caption {
            m.insert("caption".to_string(), json!(c));
        }
    }
    m.insert("forward_origin".to_string(), origin);
    up
}

fn user_origin(first: &str, last: &str, username: &str, date: i64) -> Value {
    json!({"type": "user", "date": date, "sender_user": {
        "id": 4242, "is_bot": false, "first_name": first, "last_name": last, "username": username}})
}

fn hidden_origin(name: &str, date: i64) -> Value {
    json!({"type": "hidden_user", "date": date, "sender_user_name": name})
}

fn channel_origin(title: &str, signature: &str, date: i64) -> Value {
    json!({"type": "channel", "date": date, "message_id": 77,
        "chat": {"id": -1001234, "type": "channel", "title": title},
        "author_signature": signature})
}

/// A text forward from a hidden user (the shortest helper for bursts).
fn fwd(tg: &FakeTelegram, text: &str) -> Value {
    forward_update(tg, PAIRED_CHAT, Some(text), None, hidden_origin("Bình", D2))
}

/// Appends `updates` to the fake's update log in one step (a new file renamed
/// over the old one), so a burst reaches serve in a single getUpdates answer and
/// the quiet window cannot close in the middle of it, however loaded the machine.
fn push_burst(tg: &FakeTelegram, updates: Vec<Value>) -> Vec<i64> {
    use std::io::Write;
    let path = tg.dir.join("updates.jsonl");
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut next = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| v["update_id"].as_i64())
        .max()
        .unwrap_or(0);
    let mut ids = Vec::new();
    for mut up in updates {
        next += 1;
        up["update_id"] = json!(next);
        ids.push(next);
        text.push_str(&serde_json::to_string(&up).unwrap());
        text.push('\n');
    }
    let tmp = tg.dir.join("updates.jsonl.burst");
    let mut f = std::fs::File::create(&tmp).expect("create burst file");
    f.write_all(text.as_bytes()).expect("write burst");
    f.sync_all().expect("sync burst");
    std::fs::rename(&tmp, &path).expect("replace updates.jsonl");
    ids
}

/// `--append-system-prompt` of the fake claude's last Lead call.
fn last_lead_system_prompt(sb: &Sandbox) -> String {
    let rec = sb
        .records()
        .into_iter()
        .filter(|r| {
            let argv = r["argv"].as_array().unwrap();
            argv.iter()
                .position(|a| a.as_str() == Some("--append-system-prompt"))
                .map(|i| {
                    argv[i + 1]
                        .as_str()
                        .unwrap_or("")
                        .starts_with("Bạn là Lead")
                })
                .unwrap_or(false)
        })
        .last()
        .expect("a Lead record");
    let argv = rec["argv"].as_array().unwrap();
    let i = argv
        .iter()
        .position(|a| a.as_str() == Some("--append-system-prompt"))
        .unwrap();
    argv[i + 1].as_str().unwrap_or("").to_string()
}

fn when(secs: i64) -> String {
    agent_brain::telegram::forward::when_label(secs)
}

fn task_branch(sb: &Sandbox, task_id: i64) -> String {
    sb.db()
        .query_row("SELECT branch FROM tasks WHERE id = ?1", [task_id], |r| {
            r.get(0)
        })
        .unwrap()
}

/// Presses `Tiếp tục ({k} tin)` on the selection, then the first project and
/// the default workflow (`default_label`); returns the confirm screen.
fn forward_to_confirm(
    tg: &FakeTelegram,
    srv: &Serve,
    mid: i64,
    selection: &Value,
    k: usize,
    default_label: &str,
) -> Value {
    press_on(tg, mid, selection, &format!("Tiếp tục ({k} tin)"));
    let projects = wait_edit(tg, srv, mid, |r| text_of(r).starts_with(CHOOSE_PROJECT));
    assert!(
        text_of(&projects).contains(&format!("«[Telegram · {k} tin chuyển tiếp]»")),
        "{projects}"
    );
    let project = labels(&projects)[0].clone();
    press_on(tg, mid, &projects, &project);
    let wf = wait_edit(tg, srv, mid, |r| text_of(r).starts_with("Chọn quy trình"));
    press_on(tg, mid, &wf, default_label);
    tap_default_team(tg, srv, mid);
    let confirm = wait_edit(tg, srv, mid, |r| {
        text_of(r).starts_with("Xác nhận giao việc")
    });
    assert!(
        text_of(&confirm).contains(&format!("Tin chuyển tiếp: {k} tin đã chọn")),
        "{confirm}"
    );
    confirm
}

/// `Tạo task` on the confirm screen; waits for the accepted edit and the job.
fn create_from_confirm(tg: &FakeTelegram, srv: &Serve, mid: i64, confirm: &Value) {
    press_on(tg, mid, confirm, "Tạo task");
    let done = wait_edit(tg, srv, mid, |r| {
        text_of(r).starts_with("Đã nhận — đang lập plan")
    });
    assert!(empty_keyboard(&done), "{done}");
}

/// Lines of `prompt` from the opening delimiter to the closing one.
fn block_lines(prompt: &str) -> Vec<&str> {
    let lines: Vec<&str> = prompt.lines().collect();
    let open = lines
        .iter()
        .position(|l| l.starts_with("<<<TIN_CHUYEN_TIEP "))
        .unwrap_or_else(|| panic!("no opening delimiter in {prompt}"));
    let close = lines
        .iter()
        .position(|l| l.starts_with("<<<HET_TIN_CHUYEN_TIEP "))
        .unwrap_or_else(|| panic!("no closing delimiter in {prompt}"));
    assert!(open < close, "{prompt}");
    lines[open..=close].to_vec()
}

#[test]
fn forward_burst_one_task() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);
    sb.write_scenario(lead_plan_entry());

    let mut srv = tg.serve(&sb, &[QUIET]);
    wait_polling(&tg, &srv);

    push_burst(
        &tg,
        vec![
            forward_update(
                &tg,
                PAIRED_CHAT,
                Some("Sửa nút đăng nhập trên trang chủ"),
                None,
                user_origin("Nguyễn", "An", "an_ng", D1),
            ),
            forward_update(
                &tg,
                PAIRED_CHAT,
                Some("Thêm kiểm tra email"),
                None,
                hidden_origin("Bình", D2),
            ),
            forward_update(
                &tg,
                PAIRED_CHAT,
                Some("Hạn chót thứ sáu"),
                None,
                channel_origin("Kênh Dự án", "Chi", D3),
            ),
        ],
    );
    let selection = wait_message(&tg, &srv, "Đã nhận 3 tin chuyển tiếp");
    let lines: Vec<&str> = text_of(&selection).lines().collect();
    assert_eq!(
        lines,
        vec![
            "Đã nhận 3 tin chuyển tiếp — chọn tin cần gộp thành một task:".to_string(),
            format!(
                "[1] Nguyễn An (@an_ng) · {} · Sửa nút đăng nhập trên trang chủ",
                when(D1)
            ),
            format!("[2] Bình · {} · Thêm kiểm tra email", when(D2)),
            format!("[3] Kênh Dự án (Chi) · {} · Hạn chót thứ sáu", when(D3)),
        ],
        "{selection}"
    );
    assert_eq!(
        labels(&selection),
        vec![
            "☑ 1 · Nguyễn An (@an_ng)",
            "☑ 2 · Bình",
            "☑ 3 · Kênh Dự án (Chi)",
            "Chọn hết",
            "Bỏ hết",
            "Tiếp tục (3 tin)",
            "Huỷ",
        ],
        "{selection}"
    );
    // Callback data: the flow id and an index only.
    let flow = flow_of(&button_data(&selection, "Huỷ"));
    assert!(is_flow_id(&flow), "{selection}");
    assert_eq!(
        button_data(&selection, "☑ 2 · Bình"),
        format!("iT:{flow}:1")
    );
    assert_eq!(
        button_data(&selection, "Tiếp tục (3 tin)"),
        format!("iG:{flow}")
    );
    let mid = message_id_of(&selection);

    let confirm = forward_to_confirm(&tg, &srv, mid, &selection, 3, "Mặc định (nhanh)");
    assert!(
        text_of(&confirm).contains(&format!(
            "Việc cần làm: {}",
            agent_brain::telegram::compose::DEFAULT_INSTRUCTION
        )),
        "{confirm}"
    );
    assert!(
        text_of(&confirm).contains("Nhánh: task/tin-chuyen-tiep-"),
        "{confirm}"
    );
    assert!(
        text_of(&confirm)
            .contains("Gửi một tin chữ để thêm việc cần làm (không bắt buộc), hoặc bấm Tạo task."),
        "{confirm}"
    );
    assert_eq!(task_count(&sb), 0, "nothing is created before Tạo task");
    create_from_confirm(&tg, &srv, mid, &confirm);

    assert_eq!(job_role(&srv, 1), "lead");
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    assert_eq!(srv.get("/api/jobs/2").status, 404);
    assert_eq!(task_count(&sb), 1);
    let (origin, title) = task_row(&sb, 1);
    assert_eq!(origin.as_deref(), Some("telegram"));
    assert!(
        title
            .lines()
            .next()
            .unwrap_or("")
            .starts_with("[Telegram · 3 tin chuyển tiếp]"),
        "{title}"
    );
    assert!(
        task_branch(&sb, 1).starts_with("task/tin-chuyen-tiep-"),
        "{}",
        task_branch(&sb, 1)
    );

    let prompt = last_lead_prompt(&sb);
    let opens = prompt
        .lines()
        .filter(|l| l.starts_with("<<<TIN_CHUYEN_TIEP "))
        .count();
    assert_eq!(opens, 1, "{prompt}");
    let block = block_lines(&prompt);
    for want in [
        format!("[1] Nguyễn An (@an_ng) · {}", when(D1)),
        format!("[2] Bình · {}", when(D2)),
        format!("[3] Kênh Dự án (Chi) · {}", when(D3)),
        "Sửa nút đăng nhập trên trang chủ".to_string(),
        "Thêm kiểm tra email".to_string(),
        "Hạn chót thứ sáu".to_string(),
    ] {
        assert!(block.contains(&want.as_str()), "{want:?} not in {block:#?}");
    }
    assert_eq!(
        last_lead_system_prompt(&sb),
        agent_brain::roles::lead_untrusted_prompt()
    );

    // Still exactly one selection message, seconds after the window closed.
    assert_eq!(sent_starting(&tg, "Đã nhận"), 1, "{:#?}", tg.records());
    assert!(tg.calls("getFile").is_empty());
    for line in telegram_lines(&srv) {
        for text in ["Sửa nút", "kiểm tra email", "Hạn chót", "Nguyễn"] {
            assert!(!line.contains(text), "forwarded text leaked: {line}");
        }
    }
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// Plan 05.4-07 Task 2: selection behaviours
// ---------------------------------------------------------------------------

const NONE_SELECTED: &str = "Chọn ít nhất một tin";
const TOO_LONG: &str = "Quá dài — bỏ bớt tin (tối đa 20000 ký tự)";

/// The three forwards of the tracer: user, hidden user, channel.
fn three_forwards(tg: &FakeTelegram) -> Vec<Value> {
    vec![
        forward_update(
            tg,
            PAIRED_CHAT,
            Some("Sửa nút đăng nhập trên trang chủ"),
            None,
            user_origin("Nguyễn", "An", "an_ng", D1),
        ),
        forward_update(
            tg,
            PAIRED_CHAT,
            Some("Thêm kiểm tra email"),
            None,
            hidden_origin("Bình", D2),
        ),
        forward_update(
            tg,
            PAIRED_CHAT,
            Some("Hạn chót thứ sáu"),
            None,
            channel_origin("Kênh Dự án", "Chi", D3),
        ),
    ]
}

/// Allowlisted nhanh project, Lead scenario, serve with the short quiet window.
fn forward_sandbox() -> (Sandbox, FakeTelegram, Serve) {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);
    sb.write_scenario(lead_plan_entry());
    let srv = tg.serve(&sb, &[QUIET]);
    wait_polling(&tg, &srv);
    (sb, tg, srv)
}

fn has_label(rec: &Value, label: &str) -> bool {
    labels(rec).iter().any(|l| l == label)
}

fn toggles(rec: &Value) -> Vec<String> {
    labels(rec)
        .into_iter()
        .filter(|l| l.starts_with('☑') || l.starts_with('☐'))
        .collect()
}

#[test]
fn forward_toggle_and_select_all() {
    let (sb, tg, mut srv) = forward_sandbox();
    push_burst(&tg, three_forwards(&tg));
    let selection = wait_message(&tg, &srv, "Đã nhận 3 tin chuyển tiếp");
    let mid = message_id_of(&selection);

    // One toggle flips one message.
    let cb = press_on(&tg, mid, &selection, "☑ 2 · Bình");
    let toggled = wait_edit(&tg, &srv, mid, |r| has_label(r, "Tiếp tục (2 tin)"));
    assert_eq!(
        toggles(&toggled),
        vec![
            "☑ 1 · Nguyễn An (@an_ng)",
            "☐ 2 · Bình",
            "☑ 3 · Kênh Dự án (Chi)"
        ],
        "{toggled}"
    );
    assert!(text_of(&toggled).starts_with("Đã nhận 3 tin chuyển tiếp"));
    assert_eq!(
        wait_answer(&tg, &srv, &cb)["json"]["show_alert"],
        json!(false)
    );

    // `Bỏ hết` clears every message; `Tiếp tục (0 tin)` is refused with an alert.
    press_on(&tg, mid, &toggled, "Bỏ hết");
    let none = wait_edit(&tg, &srv, mid, |r| has_label(r, "Tiếp tục (0 tin)"));
    assert!(toggles(&none).iter().all(|l| l.starts_with('☐')), "{none}");
    assert_eq!(toggles(&none).len(), 3, "{none}");
    let cb = press_on(&tg, mid, &none, "Tiếp tục (0 tin)");
    let ans = wait_answer(&tg, &srv, &cb);
    assert_eq!(ans["json"]["text"], json!(NONE_SELECTED), "{ans}");
    assert_eq!(ans["json"]["show_alert"], json!(true), "{ans}");

    // `Chọn hết` selects every message again (its edit follows any earlier one).
    press_on(&tg, mid, &none, "Chọn hết");
    let all = wait_edit(&tg, &srv, mid, |r| has_label(r, "Tiếp tục (3 tin)"));
    assert!(toggles(&all).iter().all(|l| l.starts_with('☑')), "{all}");
    assert!(
        !edits_of(&tg, mid)
            .iter()
            .any(|r| text_of(r).starts_with(CHOOSE_PROJECT)),
        "{:#?}",
        tg.records()
    );
    assert_eq!(task_count(&sb), 0);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn forward_append_while_selecting() {
    let (_sb, tg, mut srv) = forward_sandbox();
    let mut burst = three_forwards(&tg);
    let late = burst.pop().unwrap();
    push_burst(&tg, burst);
    let selection = wait_message(&tg, &srv, "Đã nhận 2 tin chuyển tiếp");
    let mid = message_id_of(&selection);

    // A forward while the list is open is appended (☑) to the same message.
    tg.push_update(late);
    let edited = wait_edit(&tg, &srv, mid, |r| {
        text_of(r).starts_with("Đã nhận 3 tin chuyển tiếp")
    });
    assert!(
        text_of(&edited).contains(&format!("[3] Kênh Dự án (Chi) · {}", when(D3))),
        "{edited}"
    );
    assert!(has_label(&edited, "☑ 3 · Kênh Dự án (Chi)"), "{edited}");
    assert!(has_label(&edited, "Tiếp tục (3 tin)"), "{edited}");
    assert_eq!(sent_starting(&tg, "Đã nhận"), 1, "{:#?}", tg.records());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn forward_cap_20() {
    let (_sb, tg, mut srv) = forward_sandbox();
    let burst: Vec<Value> = (1..=22).map(|i| fwd(&tg, &format!("tin số {i}"))).collect();
    push_burst(&tg, burst);
    let selection = wait_message(&tg, &srv, "Đã nhận 20 tin chuyển tiếp");
    let text = text_of(&selection);
    assert!(text.contains("\n[20] Bình · "), "{text}");
    assert!(!text.contains("[21]"), "{text}");
    assert!(!text.contains("tin số 21"), "{text}");
    assert_eq!(toggles(&selection).len(), 20, "{selection}");
    assert!(has_label(&selection, "Tiếp tục (20 tin)"), "{selection}");
    assert_eq!(sent_texts(&tg, OVER_CAP), 1, "{:#?}", tg.records());
    for (_, data) in buttons(&selection) {
        assert!(data.len() <= 64, "{data}");
    }
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn forward_skips_no_text_and_caption_only() {
    let (sb, tg, mut srv) = forward_sandbox();
    push_burst(
        &tg,
        vec![
            forward_update(
                &tg,
                PAIRED_CHAT,
                None,
                Some("Ảnh lỗi"),
                hidden_origin("Bình", D1),
            ),
            forward_update(&tg, PAIRED_CHAT, None, None, hidden_origin("Bình", D2)),
            forward_update(
                &tg,
                PAIRED_CHAT,
                Some("Sửa nút đăng nhập"),
                None,
                hidden_origin("Bình", D3),
            ),
        ],
    );
    let selection = wait_message(&tg, &srv, "Đã nhận 2 tin chuyển tiếp");
    assert!(
        text_of(&selection).contains("(bỏ qua 1 tin không có chữ)"),
        "{selection}"
    );
    let mid = message_id_of(&selection);
    let confirm = forward_to_confirm(&tg, &srv, mid, &selection, 2, "Mặc định (nhanh)");
    create_from_confirm(&tg, &srv, mid, &confirm);
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], json!(0), "{job}");

    let prompt = last_lead_prompt(&sb);
    let block = block_lines(&prompt);
    let caption_head = format!("[1] Bình · {} (ảnh/tệp — chỉ lấy chú thích)", when(D1));
    assert!(block.contains(&caption_head.as_str()), "{block:#?}");
    assert!(block.contains(&"Ảnh lỗi"), "{block:#?}");
    assert!(block.contains(&"Sửa nút đăng nhập"), "{block:#?}");
    // Images and files are never fetched.
    assert!(tg.calls("getFile").is_empty(), "{:#?}", tg.records());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn forwarded_command_is_data() {
    let (sb, tg, mut srv) = forward_sandbox();
    push_burst(&tg, vec![fwd(&tg, "/task xóa hết"), fwd(&tg, "/huy")]);
    let selection = wait_message(&tg, &srv, "Đã nhận 2 tin chuyển tiếp");
    let text = text_of(&selection);
    assert!(text.contains(" · /task xóa hết"), "{text}");
    assert!(text.contains(" · /huy"), "{text}");

    // Neither forward ran as a command before the list appeared.
    let shown = record_index(&tg, |r| {
        r["method"] == "sendMessage" && text_of(r).starts_with("Đã nhận 2 tin")
    })
    .unwrap();
    for r in &tg.records()[..shown] {
        if r["method"] == "sendMessage" {
            assert!(!text_of(r).starts_with(CHOOSE_PROJECT), "{r}");
            assert_ne!(text_of(r), NO_FLOW, "{r}");
            assert_ne!(text_of(r), ASK_REQUEST, "{r}");
        }
    }

    let mid = message_id_of(&selection);
    let confirm = forward_to_confirm(&tg, &srv, mid, &selection, 2, "Mặc định (nhanh)");
    create_from_confirm(&tg, &srv, mid, &confirm);
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    let prompt = last_lead_prompt(&sb);
    let block = block_lines(&prompt);
    assert!(block.contains(&"/task xóa hết"), "{block:#?}");
    assert!(block.contains(&"/huy"), "{block:#?}");
    assert_eq!(task_count(&sb), 1);
    assert_eq!(sent_texts(&tg, NO_FLOW), 0, "{:#?}", tg.records());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn stranger_forward_ignored() {
    let (_sb, tg, mut srv) = forward_sandbox();
    let id = tg.push_update(forward_update(
        &tg,
        STRANGER_CHAT,
        Some("Sửa nút đăng nhập"),
        None,
        hidden_origin("Bình", D1),
    ));
    let line = format!("Telegram: bỏ qua tin từ chat lạ {STRANGER_CHAT}");
    assert!(
        wait_until(Duration::from_secs(10), || stderr_of(&srv).contains(&line)),
        "no stranger line; stderr:\n{}",
        stderr_of(&srv)
    );
    // Handled (offset past it), then two more polls: any collection window
    // (300 ms) would have closed and sent its list by now.
    assert!(
        wait_until(Duration::from_secs(10), || tg
            .calls("getUpdates")
            .iter()
            .any(|r| r["json"]["offset"].as_i64() == Some(id + 1))),
        "update {id} never acknowledged"
    );
    let polls = tg.calls("getUpdates").len();
    assert!(
        wait_until(Duration::from_secs(10), || tg.calls("getUpdates").len()
            >= polls + 2),
        "no further polls"
    );
    assert_eq!(sent_starting(&tg, "Đã nhận"), 0, "{:#?}", tg.records());
    assert!(tg.calls("sendMessage").is_empty(), "{:#?}", tg.records());
    assert_eq!(count_lines(&stderr_of(&srv), &line), 1);

    // The paired user's own forward still works, and counts only itself.
    tg.push_update(fwd(&tg, "Thêm kiểm tra email"));
    let selection = wait_message(&tg, &srv, "Đã nhận ");
    assert!(
        text_of(&selection).starts_with("Đã nhận 1 tin chuyển tiếp"),
        "{selection}"
    );
    assert!(!text_of(&selection).contains("Sửa nút"), "{selection}");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn forward_instruction_outside_block() {
    let (sb, tg, mut srv) = forward_sandbox();
    let mut burst = three_forwards(&tg);
    burst.pop();
    push_burst(&tg, burst);
    let selection = wait_message(&tg, &srv, "Đã nhận 2 tin chuyển tiếp");
    let mid = message_id_of(&selection);
    forward_to_confirm(&tg, &srv, mid, &selection, 2, "Mặc định (nhanh)");

    // A text on the confirm screen is the instruction and names the branch.
    send_text(&tg, "Làm theo tin 1");
    let confirm = wait_edit(&tg, &srv, mid, |r| {
        text_of(r).contains("Việc cần làm: Làm theo tin 1")
    });
    assert!(
        text_of(&confirm).contains("Nhánh: task/lam-theo-tin-1"),
        "{confirm}"
    );
    assert_eq!(
        labels(&confirm),
        vec!["Tạo task".to_string(), "Huỷ".to_string()]
    );
    assert_eq!(sent_texts(&tg, HELP), 0, "{:#?}", tg.records());

    create_from_confirm(&tg, &srv, mid, &confirm);
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    assert_eq!(task_branch(&sb, 1), "task/lam-theo-tin-1");
    let prompt = last_lead_prompt(&sb);
    assert!(
        prompt.contains("Yêu cầu: [Telegram · 2 tin chuyển tiếp] Làm theo tin 1\n"),
        "{prompt}"
    );
    let open = prompt
        .find("<<<TIN_CHUYEN_TIEP ")
        .unwrap_or_else(|| panic!("no block in {prompt}"));
    assert!(prompt[..open].contains("Làm theo tin 1"), "{prompt}");
    assert!(!prompt[open..].contains("Làm theo tin 1"), "{prompt}");
    for line in telegram_lines(&srv) {
        assert!(!line.contains("Làm theo"), "instruction leaked: {line}");
    }
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn forward_too_long_refused() {
    let (_sb, tg, mut srv) = forward_sandbox();
    let burst: Vec<Value> = (1..=6)
        .map(|i| {
            let head = format!("tin dài {i} ");
            let text = format!("{head}{}", "x".repeat(3900 - head.chars().count()));
            assert_eq!(text.chars().count(), 3900);
            fwd(&tg, &text)
        })
        .collect();
    push_burst(&tg, burst);
    let selection = wait_message(&tg, &srv, "Đã nhận 6 tin chuyển tiếp");
    let mid = message_id_of(&selection);

    // Over 20,000 characters once composed: refused with an alert, list kept.
    let cb = press_on(&tg, mid, &selection, "Tiếp tục (6 tin)");
    let ans = wait_answer(&tg, &srv, &cb);
    assert_eq!(ans["json"]["text"], json!(TOO_LONG), "{ans}");
    assert_eq!(ans["json"]["show_alert"], json!(true), "{ans}");

    // Untick two: the same list still works and four messages fit.
    press_on(&tg, mid, &selection, "☑ 5 · Bình");
    let five = wait_edit(&tg, &srv, mid, |r| has_label(r, "Tiếp tục (5 tin)"));
    press_on(&tg, mid, &five, "☑ 6 · Bình");
    let four = wait_edit(&tg, &srv, mid, |r| has_label(r, "Tiếp tục (4 tin)"));
    assert!(
        !edits_of(&tg, mid)
            .iter()
            .any(|r| text_of(r).starts_with(CHOOSE_PROJECT)),
        "the refused press moved on: {:#?}",
        tg.records()
    );
    press_on(&tg, mid, &four, "Tiếp tục (4 tin)");
    let projects = wait_edit(&tg, &srv, mid, |r| text_of(r).starts_with(CHOOSE_PROJECT));
    assert!(
        text_of(&projects).contains("«[Telegram · 4 tin chuyển tiếp]»"),
        "{projects}"
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn forward_after_continue_starts_new_flow() {
    let (_sb, tg, mut srv) = forward_sandbox();
    let mut burst = three_forwards(&tg);
    let late = burst.pop().unwrap();
    push_burst(&tg, burst);
    let selection = wait_message(&tg, &srv, "Đã nhận 2 tin chuyển tiếp");
    let mid = message_id_of(&selection);
    press_on(&tg, mid, &selection, "Tiếp tục (2 tin)");
    wait_edit(&tg, &srv, mid, |r| text_of(r).starts_with(CHOOSE_PROJECT));

    // A forward after `Tiếp tục` replaces the flow and starts a new list.
    tg.push_update(late);
    let replaced = wait_edit(&tg, &srv, mid, |r| text_of(r) == REPLACED);
    assert!(empty_keyboard(&replaced), "{replaced}");
    let second = wait_new_message(&tg, &srv, "Đã nhận 1 tin chuyển tiếp", &[mid]);
    assert!(
        text_of(&second).contains(&format!("[1] Kênh Dự án (Chi) · {}", when(D3))),
        "{second}"
    );
    assert_ne!(
        flow_of(&button_data(&second, "Huỷ")),
        flow_of(&button_data(&selection, "Huỷ"))
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// Plan 05.4-07 Task 3: the framing survives re-plans; help text
// ---------------------------------------------------------------------------

/// mac-dinh gate, then one more Lead round (append) that plan review approves
/// (the shape of telegram_actions.rs `return_scenario`).
fn return_scenario() -> Value {
    by_role(json!({
        "lead": [lead_plan_entry(), lead_append_entry()],
        "plan-review": [plan_review_entry("approve", 0, 1), plan_review_entry("approve", 0, 2)],
    }))
}

/// The nonce of the opening delimiter line in a Lead prompt.
fn nonce_of(prompt: &str) -> String {
    prompt
        .lines()
        .find_map(|l| l.strip_prefix("<<<TIN_CHUYEN_TIEP "))
        .and_then(|rest| rest.strip_suffix(">>>"))
        .unwrap_or_else(|| panic!("no opening delimiter in {prompt}"))
        .to_string()
}

fn lead_records(sb: &Sandbox) -> usize {
    sb.records()
        .iter()
        .filter(|r| {
            let argv = r["argv"].as_array().unwrap();
            argv.iter()
                .position(|a| a.as_str() == Some("--append-system-prompt"))
                .is_some_and(|i| {
                    argv[i + 1]
                        .as_str()
                        .unwrap_or("")
                        .starts_with("Bạn là Lead")
                })
        })
        .count()
}

#[test]
fn untrusted_block_survives_return_lead() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");
    allow_projects(&sb, &["web-ban-hang"]);
    sb.write_scenario(return_scenario());

    let mut srv = tg.serve(&sb, &[QUIET]);
    wait_polling(&tg, &srv);

    let mut burst = three_forwards(&tg);
    burst.pop();
    push_burst(&tg, burst);
    let selection = wait_message(&tg, &srv, "Đã nhận 2 tin chuyển tiếp");
    let mid = message_id_of(&selection);
    let confirm = forward_to_confirm(&tg, &srv, mid, &selection, 2, "Mặc định (mac-dinh)");
    create_from_confirm(&tg, &srv, mid, &confirm);
    assert_eq!(job_role(&srv, 1), "auto");

    // The gate message: a two-step approve button (D-48), the one-line title.
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    assert_eq!(
        labels(&gate),
        vec!["Duyệt plan".to_string(), "Trả về Lead".to_string()],
        "{gate}"
    );
    let lines: Vec<&str> = text_of(&gate).lines().collect();
    assert!(
        lines
            .get(1)
            .is_some_and(|l| l.starts_with("[Telegram · 2 tin chuyển tiếp]")),
        "{gate}"
    );
    assert!(text_of(&gate).contains(ORIGIN_LINE), "{gate}");
    assert!(text_of(&gate).chars().count() <= 3900, "{gate}");
    assert!(!text_of(&gate).contains("<<<"), "{gate}");
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("gate"));

    let first = last_lead_prompt(&sb);
    let nonce = nonce_of(&first);
    assert_eq!(
        last_lead_system_prompt(&sb),
        agent_brain::roles::lead_untrusted_prompt()
    );
    assert_eq!(lead_records(&sb), 1);

    // `Trả về Lead` with a note: the next Lead run keeps the same framing.
    let cb = press(&tg, PAIRED_CHAT, PAIRED_CHAT, &gate, "Trả về Lead");
    let ans = wait_answer(&tg, &srv, &cb);
    assert_eq!(ans["json"]["text"], json!(NOTE_ASK), "{ans}");
    wait_message(&tg, &srv, "Ghi chú cho Lead (task 1)");
    send_text(&tg, "tách bước 2");
    let job = wait_for_job(&srv, 2);
    assert_eq!(job["exit_code"], json!(0), "{job}");
    assert_eq!(job_role(&srv, 2), "auto");
    assert_eq!(lead_records(&sb), 2);

    let second = last_lead_prompt(&sb);
    assert!(
        second.contains(&format!("<<<TIN_CHUYEN_TIEP {nonce}>>>")),
        "{second}"
    );
    assert!(
        second.contains(&format!("<<<HET_TIN_CHUYEN_TIEP {nonce}>>>")),
        "{second}"
    );
    assert!(
        second.contains("Ghi chú của người dùng: tách bước 2"),
        "{second}"
    );
    assert_eq!(
        last_lead_system_prompt(&sb),
        agent_brain::roles::lead_untrusted_prompt()
    );
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn help_text_mentions_forwarding() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    send_text(&tg, "xin chào");
    let reply = wait_message(&tg, &srv, "Gõ /task");
    assert_eq!(text_of(&reply), agent_brain::telegram::bot::HELP_TEXT);
    assert!(text_of(&reply).contains("/task"), "{reply}");
    assert!(text_of(&reply).contains("Chuyển tiếp tin nhắn"), "{reply}");
    assert!(buttons(&reply).is_empty(), "{reply}");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

/// Adds repo-backed projects to the map; each is `(id, profile)`. Returns nothing, writes the map.
fn write_projects(sb: &Sandbox, specs: &[(&str, &str)]) {
    let entries: Vec<Value> = specs
        .iter()
        .map(|(id, profile)| {
            let repo = sb.make_repo(id, &[], None, None);
            sb.project_entry(&repo, id, profile)
        })
        .collect();
    sb.write_map(json!(entries), &[]);
}

#[test]
fn task_refusal_lists_reasons_per_project() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    write_projects(&sb, &[("srt", ""), ("gone", "work"), ("media-x", "work")]);
    let mut map = sb.read_map();
    // `gone`: directory does not exist; `media-x`: media type.
    map["projects"][1]["path"] = json!(sb.root.join("khong-ton-tai").to_str().unwrap());
    map["projects"][2]["type"] = json!("media");
    map["telegram_projects"] = json!(["srt", "gone", "media-x"]);
    std::fs::write(sb.map_path(), serde_json::to_string_pretty(&map).unwrap()).unwrap();

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    send_text(&tg, "/task làm gì đó");
    let reply = wait_message(
        &tg,
        &srv,
        "Không có dự án nào trong telegram_projects chạy được",
    );
    let text = text_of(&reply);
    let line = |id: &str| -> String {
        text.lines()
            .find(|l| l.starts_with(&format!("• {id}: ")))
            .unwrap_or_else(|| panic!("no line for {id}: {text}"))
            .to_string()
    };
    assert!(line("srt").contains("chưa đặt profile"), "{text}");
    assert!(line("gone").contains("không tồn tại"), "{text}");
    assert!(line("media-x").contains("media"), "{text}");
    assert!(
        !text.contains(sb.root.to_str().unwrap()),
        "absolute path leaked: {text}"
    );
    assert!(text.contains("khong-ton-tai"), "{text}");
    assert!(text.ends_with("projects_map.json."), "{text}");
    assert!(buttons(&reply).is_empty(), "{reply}");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn task_with_one_runnable_project_still_offers_it() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    write_projects(&sb, &[("srt", ""), ("tot", "work")]);
    allow_projects(&sb, &["srt", "tot"]);

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    send_text(&tg, "/task việc");
    let screen = wait_message(&tg, &srv, "Chọn dự án cho task:");
    let b = labels(&screen);
    assert_eq!(b.len(), 2, "{screen}");
    assert!(b[0].starts_with("tot") || b[0].contains("tot"), "{screen}");
    assert!(!text_of(&screen).contains("srt"), "{screen}");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}
