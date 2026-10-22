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
const HELP: &str = "Bot chỉ nhận nút bấm trên tin thông báo — không giao việc mới qua Telegram.";
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
// Plan 05.4-03: plans of Telegram-created tasks are approved on the dashboard only
// ---------------------------------------------------------------------------

const ORIGIN_LINE: &str = "Task giao từ Telegram — duyệt plan trên dashboard.";
const REFUSED: &str = "Task giao từ Telegram — chỉ duyệt plan trên dashboard";
const PROJECT_LINE: &str = "Dự án: web-ban-hang · profile work · nhánh task/sua-app";

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

#[test]
fn gate_without_approve_button() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    auto_start_telegram_to_gate(&sb, gate_scenario());
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");

    assert_eq!(labels(&gate), vec!["Trả về Lead".to_string()], "{gate}");
    let lines: Vec<&str> = text_of(&gate).lines().collect();
    let project = lines
        .iter()
        .position(|l| *l == PROJECT_LINE)
        .unwrap_or_else(|| panic!("no project line in {gate}"));
    assert_eq!(lines.get(project + 1).copied(), Some(ORIGIN_LINE), "{gate}");
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn forged_approve_refused() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    auto_start_telegram_to_gate(&sb, full_flow_scenario());
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    assert!(
        !labels(&gate).iter().any(|l| l == "Duyệt plan"),
        "Telegram-origin gate must not offer Duyệt plan: {gate}"
    );

    // A crafted `a:{task}:{rev}` with the CORRECT rev, taken from the Trả về Lead button.
    let ret = button_data(&gate, "Trả về Lead");
    let rev = ret
        .strip_prefix("r:1:")
        .expect("r:1:{rev} data")
        .to_string();
    let forged = tg.callback_update(
        PAIRED_CHAT,
        PAIRED_CHAT,
        message_id_of(&gate),
        text_of(&gate),
        &format!("a:1:{rev}"),
    );
    let cb_id = forged["callback_query"]["id"].as_str().unwrap().to_string();
    tg.push_update(forged);

    let ans = wait_answer(&tg, &srv, &cb_id);
    assert_eq!(ans["json"]["text"], json!(REFUSED), "{ans}");
    assert_eq!(ans["json"]["show_alert"], json!(true), "{ans}");
    assert_eq!(
        approved_commit(&sb, 1),
        None,
        "forged approve must not approve"
    );
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("gate"));

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
