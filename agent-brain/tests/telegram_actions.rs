//! Telegram button actions (Phase 05.3 plan 05): the bot long-polls the local
//! fake Telegram, accepts updates only from the paired chat and runs
//! `Duyệt plan`, `Chạy thêm 1 vòng` and `Dừng hẳn` through the dashboard's own
//! `run_control`. Nothing here contacts api.telegram.org.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use agent_brain::procs::{alive, descendants, ProcId};
use common::{wait_until, RunOut, Sandbox, Serve};
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

// ---------------------------------------------------------------------------
// Task 1 (tracer)
// ---------------------------------------------------------------------------

#[test]
fn approve_from_telegram_continues_autopilot() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let repo = setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    auto_start_to_gate(&sb, full_flow_scenario());
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let m = message_id_of(&gate);
    let t = text_of(&gate).to_string();
    assert!(
        is_rev_data(&button_data(&gate, "Duyệt plan"), "a:1:"),
        "{gate}"
    );

    let cb_id = press(&tg, PAIRED_CHAT, PAIRED_CHAT, &gate, "Duyệt plan");

    let ans = wait_answer(&tg, &srv, &cb_id);
    assert_eq!(ans["json"]["text"], json!("Đã duyệt plan"), "{ans}");
    let edit = wait_edit(&tg, &srv, m, |r| {
        let text = text_of(r);
        text.starts_with(&t) && text.contains("✓ Đã duyệt plan") && text.contains("từ Telegram")
    });
    assert!(empty_keyboard(&edit), "{edit}");
    assert_eq!(edit["json"]["chat_id"], json!(PAIRED_CHAT), "{edit}");

    // Approved exactly the plan commit the message was rendered for.
    let plan_sha = commit_with_subject(&sb, &repo, "sua-app: plan");
    assert_eq!(approved_commit(&sb, 1).as_deref(), Some(plan_sha.as_str()));
    assert!(
        text_of(&edit).contains(&format!(
            "✓ Đã duyệt plan {} · autopilot chạy tiếp — từ Telegram lúc ",
            &plan_sha[..7]
        )),
        "{edit}"
    );
    assert_eq!(job_role(&srv, 1), "auto");

    // Autopilot continues to QA passed.
    assert!(
        wait_until(Duration::from_secs(60), || task_auto_state(&sb, 1)
            .as_deref()
            == Some("done")),
        "autopilot did not finish; auto_state {:?}; stderr:\n{}",
        task_auto_state(&sb, 1),
        stderr_of(&srv)
    );
    wait_message(&tg, &srv, "QA đạt · task 1");
    assert!(
        stdout_of(&srv).contains("Telegram: Duyệt plan task 1 —"),
        "{}",
        stdout_of(&srv)
    );
    assert_eq!(answers_for(&tg, &cb_id).len(), 1);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

/// Appends `lines` (updates carrying their own update_id) to the fake's queue
/// in ONE write and in the given file order.
fn push_batch(tg: &FakeTelegram, lines: &[Value]) {
    use std::io::Write;
    let mut text = String::new();
    for u in lines {
        text.push_str(&serde_json::to_string(u).unwrap());
        text.push('\n');
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(tg.dir.join("updates.jsonl"))
        .expect("open updates.jsonl");
    f.write_all(text.as_bytes()).expect("append updates");
    f.flush().expect("flush updates.jsonl");
}

/// Waits until the bot asked for updates from `offset` on (every update below
/// `offset` was handled or ignored).
fn wait_offset(tg: &FakeTelegram, srv: &Serve, offset: i64, timeout: Duration) {
    assert!(
        wait_until(timeout, || tg
            .calls("getUpdates")
            .iter()
            .any(|r| r["json"]["offset"].as_i64() == Some(offset))),
        "no getUpdates with offset {offset}; records:\n{:#?}\nstderr:\n{}",
        tg.records(),
        stderr_of(srv)
    );
}

fn stranger_line(id: i64) -> String {
    format!("Telegram: bỏ qua tin từ chat lạ {id}")
}

/// `run lead` (nhanh workflow): the task waits at the gate.
fn nhanh_gate(sb: &Sandbox) {
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
}

// ---------------------------------------------------------------------------
// Task 2: allowlist, ordering, invalid data, stale / refused presses, more
// round, end, text messages, shutdown with an open long poll
// ---------------------------------------------------------------------------

#[test]
fn stranger_chat_ignored() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    auto_start_to_gate(&sb, gate_scenario());
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let m = message_id_of(&gate);
    let t = text_of(&gate).to_string();
    let d = button_data(&gate, "Duyệt plan");

    // (a) a stranger's press with valid data, (b) a stranger's text, (c) the
    // paired chat's message pressed by another sender.
    let a = tg.callback_update(STRANGER_CHAT, STRANGER_CHAT, m, &t, &d);
    let b = tg.message_update(STRANGER_CHAT, STRANGER_CHAT, "private", "xin chào");
    let c = tg.callback_update(PAIRED_CHAT, 8888, m, &t, &d);
    let a_id = a["callback_query"]["id"].as_str().unwrap().to_string();
    let c_id = c["callback_query"]["id"].as_str().unwrap().to_string();
    tg.push_update(a);
    tg.push_update(b);
    let last = tg.push_update(c);
    wait_offset(&tg, &srv, last + 1, Duration::from_secs(10));
    std::thread::sleep(Duration::from_secs(3));

    assert!(answers_for(&tg, &a_id).is_empty(), "{:#?}", tg.records());
    assert!(answers_for(&tg, &c_id).is_empty(), "{:#?}", tg.records());
    assert!(tg.calls("answerCallbackQuery").is_empty());
    assert!(tg.calls("editMessageText").is_empty());
    assert!(tg.calls("editMessageReplyMarkup").is_empty());
    let sent = tg.calls("sendMessage");
    assert_eq!(sent.len(), 1, "{sent:#?}");
    assert_eq!(message_id_of(&sent[0]), m);
    // No request at all was made for the strangers.
    for r in tg.records() {
        let method = r["method"].as_str().unwrap_or("");
        assert!(
            matches!(
                method,
                "getMe" | "getUpdates" | "sendDocument" | "sendMessage"
            ),
            "unexpected request: {r}"
        );
    }
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(srv.get("/api/jobs/1").status, 404);

    let err = stderr_of(&srv);
    assert_eq!(count_lines(&err, &stranger_line(7777)), 1, "stderr:\n{err}");
    assert_eq!(count_lines(&err, &stranger_line(8888)), 1, "stderr:\n{err}");
    assert!(!stdout_of(&srv).contains("Telegram: Duyệt plan task"));
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn updates_in_order_offset_advances() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    protocol::setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    let tasks_before = task_count(&sb);

    // Three consecutive updates written at once, out of order in the file.
    let n = 41;
    let mut stranger = tg.message_update(STRANGER_CHAT, STRANGER_CHAT, "private", "hi");
    stranger["update_id"] = json!(n);
    let mut start = tg.message_update(PAIRED_CHAT, PAIRED_CHAT, "private", "/start");
    start["update_id"] = json!(n + 1);
    let mut cb = tg.callback_update(PAIRED_CHAT, PAIRED_CHAT, 999, "x", "a:1:XYZ");
    cb["update_id"] = json!(n + 2);
    let cb_id = cb["callback_query"]["id"].as_str().unwrap().to_string();
    push_batch(&tg, &[cb, stranger, start]);

    wait_offset(&tg, &srv, n + 3, Duration::from_secs(5));
    let ans = wait_answer(&tg, &srv, &cb_id);
    std::thread::sleep(Duration::from_secs(2));

    let helps: Vec<Value> = tg
        .calls("sendMessage")
        .into_iter()
        .filter(|r| text_of(r) == HELP)
        .collect();
    assert_eq!(helps.len(), 1, "{:#?}", tg.records());
    assert_eq!(helps[0]["json"]["chat_id"], json!(PAIRED_CHAT));
    assert_eq!(tg.calls("sendMessage").len(), 1, "{:#?}", tg.records());
    let invalid: Vec<Value> = tg
        .calls("answerCallbackQuery")
        .into_iter()
        .filter(|r| r["json"]["text"] == json!(INVALID))
        .collect();
    assert_eq!(invalid.len(), 1, "{:#?}", tg.records());
    assert_eq!(tg.calls("answerCallbackQuery").len(), 1);
    // Handled in update_id order: the help reply (n + 1) before the answer (n + 2).
    assert!(
        helps[0]["t"].as_f64().unwrap() <= ans["t"].as_f64().unwrap(),
        "{:#?}",
        tg.records()
    );
    // Every later poll asks from n + 3 on; nothing was handled twice.
    let polls = tg.calls("getUpdates");
    let pos = polls
        .iter()
        .position(|r| r["json"]["offset"].as_i64() == Some(n + 3))
        .unwrap();
    for r in &polls[pos..] {
        assert!(r["json"]["offset"].as_i64().unwrap_or(0) >= n + 3, "{r}");
    }
    let err = stderr_of(&srv);
    assert_eq!(count_lines(&err, &stranger_line(7777)), 1, "stderr:\n{err}");
    assert_eq!(task_count(&sb), tasks_before);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn invalid_callback_answered() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    protocol::setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    nhanh_gate(&sb);
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let m = message_id_of(&gate);
    let t = text_of(&gate).to_string();
    let runs_before = run_count(&sb);
    let tasks_before = task_count(&sb);

    let long = format!("a:1:{}", "0".repeat(66));
    assert_eq!(long.len(), 70);
    let mut ids = Vec::new();
    for data in ["a:1:XYZ", long.as_str()] {
        let up = tg.callback_update(PAIRED_CHAT, PAIRED_CHAT, m, &t, data);
        ids.push(up["callback_query"]["id"].as_str().unwrap().to_string());
        tg.push_update(up);
    }
    for id in &ids {
        let ans = wait_answer(&tg, &srv, id);
        assert_eq!(ans["json"]["text"], json!(INVALID), "{ans}");
    }
    std::thread::sleep(Duration::from_secs(2));
    assert!(edits_of(&tg, m).is_empty(), "{:#?}", tg.records());
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(run_count(&sb), runs_before);
    assert_eq!(task_count(&sb), tasks_before);
    assert_eq!(srv.get("/api/jobs/1").status, 404);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn stale_button_answers_and_removes() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    protocol::setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    nhanh_gate(&sb);
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let m = message_id_of(&gate);

    // Approved on the dashboard: the message's rev is now stale.
    let resp = srv.post_json("/api/tasks/1/approve", &json!({}));
    assert_eq!(resp.status, 200, "{}", resp.text());
    let dash_commit = approved_commit(&sb, 1).expect("dashboard approval");

    let cb_id = press(&tg, PAIRED_CHAT, PAIRED_CHAT, &gate, "Duyệt plan");
    let ans = wait_answer(&tg, &srv, &cb_id);
    let ans_text = ans["json"]["text"].as_str().unwrap_or("");
    assert!(ans_text.starts_with("Task đã đổi trạng thái:"), "{ans}");
    let edit = wait_edit(&tg, &srv, m, |r| {
        text_of(r).contains("Task đã đổi trạng thái:")
    });
    assert!(text_of(&edit).starts_with(text_of(&gate)), "{edit}");
    std::thread::sleep(Duration::from_secs(1));
    let newest = edits_of(&tg, m).pop().expect("an edit");
    assert!(empty_keyboard(&newest), "{newest}");

    assert_eq!(approved_commit(&sb, 1), Some(dash_commit));
    assert_eq!(srv.get("/api/jobs/1").status, 404, "no job started");
    assert!(
        stdout_of(&srv).contains("Telegram: Duyệt plan task 1 — Task đã đổi trạng thái:"),
        "{}",
        stdout_of(&srv)
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn refused_press_keeps_buttons() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let repo = setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    auto_start_to_gate(&sb, gate_scenario());
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let m = message_id_of(&gate);

    // .plan.md edited by hand and not committed.
    let plan_path = repo.join(".plan.md");
    let mut text = std::fs::read_to_string(&plan_path).unwrap();
    text.push_str("\n<!-- sửa tay -->\n");
    std::fs::write(&plan_path, text).unwrap();

    let cb_id = press(&tg, PAIRED_CHAT, PAIRED_CHAT, &gate, "Duyệt plan");
    let ans = wait_answer(&tg, &srv, &cb_id);
    assert_eq!(ans["json"]["show_alert"], json!(true), "{ans}");
    assert_eq!(
        ans["json"]["text"],
        json!("Plan có thay đổi chưa commit — commit với message 'sua-app: plan' rồi duyệt"),
        "{ans}"
    );
    std::thread::sleep(Duration::from_secs(2));
    assert!(edits_of(&tg, m).is_empty(), "{:#?}", tg.records());
    assert!(tg.calls("editMessageReplyMarkup").is_empty());
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(srv.get("/api/jobs/1").status, 404);
    assert!(
        stdout_of(&srv).contains("Telegram: Duyệt plan task 1 — Plan có thay đổi chưa commit"),
        "{}",
        stdout_of(&srv)
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn more_round_from_telegram() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    plan_cap_stopped(&sb);
    let stop = wait_message(&tg, &srv, "Autopilot dừng · hết vòng · task 1");
    let m = message_id_of(&stop);

    // One more round: Lead appends, plan review approves.
    sb.write_scenario(by_role(json!({
        "lead": [lead_append_entry()],
        "plan-review": [plan_review_entry("approve", 0, 0)],
    })));
    let cb_id = press(&tg, PAIRED_CHAT, PAIRED_CHAT, &stop, "Chạy thêm 1 vòng");
    let ans = wait_answer(&tg, &srv, &cb_id);
    assert_eq!(ans["json"]["text"], json!("Đã giao thêm 1 vòng"), "{ans}");
    let edit = wait_edit(&tg, &srv, m, |r| {
        let text = text_of(r);
        text.contains("✓ Đã giao thêm 1 vòng") && text.contains("từ Telegram")
    });
    assert!(empty_keyboard(&edit), "{edit}");
    assert!(text_of(&edit).starts_with(text_of(&stop)), "{edit}");
    assert_eq!(job_role(&srv, 1), "auto");
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], 0, "{job}");
    assert_eq!(task_auto_json(&sb, 1)["grants"]["plan"], 1);
    assert!(
        stdout_of(&srv).contains("Telegram: Chạy thêm 1 vòng task 1 — Đã giao thêm 1 vòng"),
        "{}",
        stdout_of(&srv)
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn end_from_telegram() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    plan_cap_stopped(&sb);
    let stop = wait_message(&tg, &srv, "Autopilot dừng · hết vòng · task 1");
    let m = message_id_of(&stop);
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(tg.calls("sendMessage").len(), 1, "{:#?}", tg.records());

    let cb_id = press(&tg, PAIRED_CHAT, PAIRED_CHAT, &stop, "Dừng hẳn");
    wait_answer(&tg, &srv, &cb_id);
    assert!(
        wait_until(Duration::from_secs(10), || task_auto_state(&sb, 1)
            .as_deref()
            == Some("ended")),
        "auto_state {:?}",
        task_auto_state(&sb, 1)
    );
    let edit = wait_edit(&tg, &srv, m, |r| {
        text_of(r).contains(
            "✓ Đã dừng hẳn autopilot. Bật autopilot trên dashboard để chạy tiếp. — từ Telegram",
        )
    });
    assert!(empty_keyboard(&edit), "{edit}");
    assert!(
        stdout_of(&srv).contains("Telegram: Dừng hẳn task 1 — Đã dừng hẳn autopilot."),
        "{}",
        stdout_of(&srv)
    );

    // The task now waits at the Bạn gate with autopilot off: one gate message.
    let gate = match tg.wait_call("sendMessage", Duration::from_secs(10), |r| {
        first_line(r) == "Chờ duyệt plan · task 1"
    }) {
        Some(r) => r,
        None => panic!(
            "no gate message after Dừng hẳn; records:\n{:#?}\nstderr:\n{}",
            tg.records(),
            stderr_of(&srv)
        ),
    };
    let gate_text = text_of(&gate).to_string();
    assert!(gate_text.contains("Review plan: sửa lại"), "{gate_text}");
    assert_eq!(
        labels(&gate),
        vec!["Duyệt plan".to_string(), "Trả về Lead".to_string()],
        "{gate}"
    );
    let btns = buttons(&gate);
    assert!(is_rev_data(&btns[0].1, "a:1:"), "{btns:?}");
    assert!(is_rev_data(&btns[1].1, "r:1:"), "{btns:?}");

    std::thread::sleep(Duration::from_secs(3));
    let gates: Vec<Value> = tg
        .calls("sendMessage")
        .into_iter()
        .filter(|r| first_line(r) == "Chờ duyệt plan · task 1")
        .collect();
    assert_eq!(gates.len(), 1, "{gates:#?}");
    assert_eq!(tg.calls("sendMessage").len(), 2, "{:#?}", tg.records());
    assert_eq!(edits_of(&tg, m).len(), 1, "{:#?}", tg.records());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

fn cmdline(pid: i32) -> String {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    String::from_utf8_lossy(&raw).replace('\0', " ")
}

#[test]
fn sigint_with_open_long_poll_is_prompt() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    tg.set_behavior(json!([{"method": "getUpdates", "times": 2, "delay_ms": 20000}]));
    write_config(&sb, PAIRED_CHAT, 0o600);
    let mut srv = tg.serve(&sb, &[]);
    assert!(
        wait_until(Duration::from_secs(10), || stdout_of(&srv)
            .contains(CONNECTED)),
        "no connected line; stdout:\n{}\nstderr:\n{}",
        stdout_of(&srv),
        stderr_of(&srv)
    );

    // The first getUpdates is pending inside the fake.
    let behavior = tg.dir.join("behavior.json");
    assert!(
        wait_until(Duration::from_secs(10), || {
            std::fs::read_to_string(&behavior)
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|v| v[0]["times"].as_i64())
                .is_some_and(|n| n <= 1)
        }),
        "getUpdates never reached the fake Telegram"
    );
    let serve_pid = srv.child.id() as i32;
    let mut found: Option<(ProcId, String)> = None;
    wait_until(Duration::from_secs(5), || {
        found = descendants(serve_pid).into_iter().find_map(|p| {
            let c = cmdline(p.pid);
            c.contains("curl").then_some((p, c))
        });
        found.is_some()
    });
    let (curl, cmd) = found.expect("no curl descendant of serve during the long poll");
    assert!(!cmd.contains(FAKE_TOKEN), "token on curl argv: {cmd}");
    assert!(alive(curl));

    let started = std::time::Instant::now();
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        wait_until(Duration::from_secs(2), || !alive(curl)),
        "curl child {} survived serve",
        curl.pid
    );
}
