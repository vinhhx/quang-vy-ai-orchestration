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

    // Workflow -> confirm screen.
    press_on(&tg, mid, &wf_screen, "Mặc định (nhanh)");
    let confirm = wait_edit(&tg, &srv, mid, |r| {
        text_of(r).starts_with("Xác nhận giao việc")
    });
    assert!(
        text_of(&confirm).contains("Nhánh: task/sua-app"),
        "{confirm}"
    );
    assert!(
        text_of(&confirm).contains("Plan của task giao từ Telegram chỉ duyệt trên dashboard."),
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
    assert_eq!(labels(&gate), vec!["Trả về Lead".to_string()], "{gate}");

    // The request text is never printed on the terminal.
    for line in telegram_lines(&srv) {
        assert!(!line.contains("sửa app"), "request leaked: {line}");
    }
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

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let mut up = tg.message_update(PAIRED_CHAT, PAIRED_CHAT, "private", "/task sửa app");
    up["message"]["forward_origin"] =
        json!({"type": "hidden_user", "sender_user_name": "Khách", "date": 1});
    tg.push_update(up);
    assert!(
        wait_until(Duration::from_secs(10), || sent_texts(&tg, HELP) == 1),
        "{:#?}",
        tg.records()
    );
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
const CREATED_TEXT: &str = "Đã tạo task 1 · task/sua-app. Plan đang được lập; khi xong sẽ có tin chờ duyệt — duyệt plan trên dashboard.";
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
