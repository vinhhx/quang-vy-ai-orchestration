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

// ---------------------------------------------------------------------------
// Plan 05.3-06: Trả về Lead with a note, Kill with confirmation, messages that
// follow the task
// ---------------------------------------------------------------------------

const NOTE_ASK: &str = "Gửi ghi chú cho Lead hoặc bấm Không ghi chú";

fn run_metrics(sb: &Sandbox, run_id: i64) -> Value {
    let m: Option<String> = sb
        .db()
        .query_row(
            "SELECT metrics_json FROM runs WHERE id = ?1",
            [run_id],
            |r| r.get(0),
        )
        .unwrap();
    serde_json::from_str(&m.unwrap_or_else(|| "{}".to_string())).unwrap()
}

fn last_run_id(sb: &Sandbox, role: &str) -> i64 {
    sb.db()
        .query_row("SELECT MAX(id) FROM runs WHERE role = ?1", [role], |r| {
            r.get(0)
        })
        .unwrap()
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

/// mac-dinh gate, then one more Lead round (append) that plan review approves.
fn return_scenario() -> Value {
    by_role(json!({
        "lead": [lead_plan_entry(), lead_append_entry()],
        "plan-review": [plan_review_entry("approve", 0, 1), plan_review_entry("approve", 0, 2)],
    }))
}

fn wait_job_role(srv: &Serve, job_id: u64, timeout: Duration) -> String {
    let mut role = String::new();
    let ok = wait_until(timeout, || {
        let r = srv.get(&format!("/api/jobs/{job_id}"));
        if r.status == 200 {
            role = r.json()["role"].as_str().unwrap_or("").to_string();
            return true;
        }
        false
    });
    assert!(
        ok,
        "job {job_id} never started; stderr:\n{}",
        stderr_of(srv)
    );
    role
}

fn send_text(tg: &FakeTelegram, text: &str) -> i64 {
    tg.push_update(tg.message_update(PAIRED_CHAT, PAIRED_CHAT, "private", text))
}

/// `Trả về Lead` on `rec`: answered, and a note prompt replying to it.
fn open_note_prompt(tg: &FakeTelegram, srv: &Serve, rec: &Value, task_id: i64) -> Value {
    let cb_id = press(tg, PAIRED_CHAT, PAIRED_CHAT, rec, "Trả về Lead");
    let ans = wait_answer(tg, srv, &cb_id);
    assert_eq!(ans["json"]["text"], json!(NOTE_ASK), "{ans}");
    let prompt = wait_message(tg, srv, &format!("Ghi chú cho Lead (task {task_id})"));
    assert_eq!(
        text_of(&prompt),
        format!("Ghi chú cho Lead (task {task_id}): gửi tin nhắn kế tiếp làm ghi chú (tối đa 500 ký tự), hoặc bấm Không ghi chú. Hết hạn sau 10 phút.")
    );
    assert_eq!(
        labels(&prompt),
        vec!["Không ghi chú".to_string(), "Huỷ".to_string()],
        "{prompt}"
    );
    assert_eq!(prompt["json"]["chat_id"], json!(PAIRED_CHAT), "{prompt}");
    assert_eq!(
        prompt["json"]["reply_parameters"]["message_id"],
        json!(message_id_of(rec)),
        "{prompt}"
    );
    // Same task and rev as the pressed button.
    let rev = button_data(rec, "Trả về Lead")
        .rsplit(':')
        .next()
        .unwrap()
        .to_string();
    assert_eq!(
        button_data(&prompt, "Không ghi chú"),
        format!("n:{task_id}:{rev}")
    );
    assert_eq!(button_data(&prompt, "Huỷ"), format!("c:{task_id}:{rev}"));
    prompt
}

#[test]
fn return_lead_with_note_from_telegram() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    auto_start_to_gate(&sb, return_scenario());
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let m = message_id_of(&gate);
    let t = text_of(&gate).to_string();

    let prompt = open_note_prompt(&tg, &srv, &gate, 1);
    let p = message_id_of(&prompt);
    assert_eq!(srv.get("/api/jobs/1").status, 404, "no job before the note");

    send_text(&tg, "  -tách bước 2  ");
    assert_eq!(
        wait_job_role(&srv, 1, Duration::from_secs(15)),
        "auto",
        "the dashboard's return_lead job"
    );
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], 0, "{job}");

    let lead2 = last_run_id(&sb, "lead");
    assert_eq!(run_metrics(&sb, lead2)["note"], "-tách bước 2");
    let lead_prompt = last_lead_prompt(&sb);
    assert!(
        lead_prompt.contains("Ghi chú của người dùng: -tách bước 2"),
        "prompt: {lead_prompt}"
    );

    let edit = wait_edit(&tg, &srv, m, |r| {
        text_of(r).contains("✓ Đã trả về Lead (ghi chú: «-tách bước 2») — từ Telegram lúc ")
    });
    assert!(text_of(&edit).starts_with(&t), "{edit}");
    assert!(empty_keyboard(&edit), "{edit}");
    let pe = wait_edit(&tg, &srv, p, |r| text_of(r) == "Đã gửi cho Lead.");
    assert!(empty_keyboard(&pe), "{pe}");
    assert_eq!(approved_commit(&sb, 1), None);

    let out = stdout_of(&srv);
    assert!(
        out.contains("Telegram: Trả về Lead task 1 — đã giao (ghi chú 12 ký tự)"),
        "{out}"
    );
    assert!(!out.contains("tách bước"), "note text on stdout: {out}");
    assert!(!stderr_of(&srv).contains("tách bước"));
    // The note was not answered with the help line.
    assert!(
        !tg.calls("sendMessage").iter().any(|r| text_of(r) == HELP),
        "{:#?}",
        tg.records()
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// Plan 05.3-06 Task 2: the other prompt paths and Kill with confirmation
// ---------------------------------------------------------------------------

const NOTE_TOO_LONG: &str =
    "Ghi chú quá dài (tối đa 500 ký tự) — gửi lại ngắn hơn hoặc bấm Không ghi chú.";
const NOTE_LINE: &str = "Ghi chú của người dùng";

fn run_status(sb: &Sandbox, run_id: i64) -> String {
    sb.db()
        .query_row("SELECT status FROM runs WHERE id = ?1", [run_id], |r| {
            r.get(0)
        })
        .unwrap()
}

/// Presses `label` on the prompt message `prompt`; returns the callback id.
fn press_prompt(tg: &FakeTelegram, prompt: &Value, label: &str) -> String {
    press(tg, PAIRED_CHAT, PAIRED_CHAT, prompt, label)
}

#[test]
fn return_lead_without_note() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    // Without a note the Lead re-plans from a fresh skeleton (05.2 D-17), so
    // its second entry writes a whole plan.
    auto_start_to_gate(
        &sb,
        by_role(json!({
            "lead": [lead_plan_entry(), lead_plan_entry()],
            "plan-review": [plan_review_entry("approve", 0, 1), plan_review_entry("approve", 0, 2)],
        })),
    );
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let m = message_id_of(&gate);
    let prompt = open_note_prompt(&tg, &srv, &gate, 1);
    let p = message_id_of(&prompt);

    let cb_id = press_prompt(&tg, &prompt, "Không ghi chú");
    let ans = wait_answer(&tg, &srv, &cb_id);
    assert_eq!(ans["json"]["text"], json!("Đã trả về Lead"), "{ans}");
    assert_eq!(wait_job_role(&srv, 1, Duration::from_secs(15)), "auto");
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], 0, "{job}");

    let lead_prompt = last_lead_prompt(&sb);
    assert!(!lead_prompt.contains(NOTE_LINE), "prompt: {lead_prompt}");
    assert!(
        lead_prompt.contains("đã có sẵn khung (skeleton)"),
        "prompt: {lead_prompt}"
    );
    assert!(run_metrics(&sb, last_run_id(&sb, "lead"))
        .get("note")
        .map_or(true, Value::is_null));
    let edit = wait_edit(&tg, &srv, m, |r| {
        text_of(r).contains("✓ Đã trả về Lead (không ghi chú) — từ Telegram lúc ")
    });
    assert!(empty_keyboard(&edit), "{edit}");
    wait_edit(&tg, &srv, p, |r| text_of(r) == "Đã gửi cho Lead.");
    assert!(
        stdout_of(&srv).contains("Telegram: Trả về Lead task 1 — đã giao (không ghi chú)"),
        "{}",
        stdout_of(&srv)
    );
    // Pressing Không ghi chú again: the prompt is closed.
    let again = press_prompt(&tg, &prompt, "Không ghi chú");
    let ans = wait_answer(&tg, &srv, &again);
    assert_eq!(ans["json"]["text"], json!(INVALID), "{ans}");
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(srv.get("/api/jobs/2").status, 404, "a second job started");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn cancel_and_too_long_note() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    auto_start_to_gate(&sb, return_scenario());
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let m = message_id_of(&gate);

    // A second press replaces the open prompt.
    let first = open_note_prompt(&tg, &srv, &gate, 1);
    let p1 = message_id_of(&first);
    let cb_id = press(&tg, PAIRED_CHAT, PAIRED_CHAT, &gate, "Trả về Lead");
    wait_answer(&tg, &srv, &cb_id);
    let replaced = wait_edit(&tg, &srv, p1, |r| {
        text_of(r) == "Đã huỷ — có yêu cầu ghi chú mới."
    });
    assert!(empty_keyboard(&replaced), "{replaced}");
    let prompt = match tg.wait_call("sendMessage", Duration::from_secs(10), |r| {
        text_of(r).starts_with("Ghi chú cho Lead (task 1)") && message_id_of(r) != p1
    }) {
        Some(r) => r,
        None => panic!("no second prompt; records:\n{:#?}", tg.records()),
    };
    let p = message_id_of(&prompt);
    // The replaced prompt's buttons do nothing.
    let old = press_prompt(&tg, &first, "Không ghi chú");
    let ans = wait_answer(&tg, &srv, &old);
    assert_eq!(ans["json"]["text"], json!(INVALID), "{ans}");

    // 501 characters: refused, the prompt stays open.
    send_text(&tg, &"ă".repeat(501));
    let too_long = wait_message(&tg, &srv, NOTE_TOO_LONG);
    assert_eq!(text_of(&too_long), NOTE_TOO_LONG);
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(srv.get("/api/jobs/1").status, 404);
    assert!(edits_of(&tg, p).is_empty(), "{:#?}", tg.records());

    let cb_id = press_prompt(&tg, &prompt, "Huỷ");
    let ans = wait_answer(&tg, &srv, &cb_id);
    assert_eq!(ans["json"]["text"], json!("Đã huỷ"), "{ans}");
    let pe = wait_edit(&tg, &srv, p, |r| text_of(r) == "Đã huỷ — không làm gì.");
    assert!(empty_keyboard(&pe), "{pe}");

    // After Huỷ a text is not a note.
    send_text(&tg, "abc");
    assert!(
        tg.wait_call("sendMessage", Duration::from_secs(10), |r| text_of(r)
            == HELP)
            .is_some(),
        "no help reply; records:\n{:#?}",
        tg.records()
    );
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(srv.get("/api/jobs/1").status, 404, "a job started");
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("gate"));
    // The gate message keeps its buttons: nothing was done.
    assert!(edits_of(&tg, m).is_empty(), "{:#?}", tg.records());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn relead_with_note_from_failed_run() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    protocol::setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

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
        Some(json!({"exit_code": 1, "ops": []})),
    );
    assert_ne!(res.code, 0, "lead should fail: {}", res.stderr);
    let failed = wait_message(&tg, &srv, "Run lỗi · task 1");
    assert_eq!(labels(&failed), vec!["Trả về Lead".to_string()], "{failed}");
    let m = message_id_of(&failed);

    sb.write_scenario(lead_plan_entry());
    let prompt = open_note_prompt(&tg, &srv, &failed, 1);
    let p = message_id_of(&prompt);
    send_text(&tg, "thêm bước kiểm tra");

    assert_eq!(
        wait_job_role(&srv, 1, Duration::from_secs(15)),
        "lead",
        "the dashboard's Chạy lại Lead job"
    );
    let job = wait_for_job(&srv, 1);
    assert_eq!(job["exit_code"], 0, "{job}");
    let lead_prompt = last_lead_prompt(&sb);
    assert!(
        lead_prompt.contains("Ghi chú của người dùng: thêm bước kiểm tra"),
        "prompt: {lead_prompt}"
    );
    let edit = wait_edit(&tg, &srv, m, |r| {
        text_of(r).contains("✓ Đã trả về Lead (ghi chú: «thêm bước kiểm tra») — từ Telegram")
    });
    assert!(empty_keyboard(&edit), "{edit}");
    wait_edit(&tg, &srv, p, |r| text_of(r) == "Đã gửi cho Lead.");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

/// A press of `data` on the synthetic message `message_id` showing `text`.
fn push_press(tg: &FakeTelegram, message_id: i64, text: &str, data: &str) -> String {
    let up = tg.callback_update(PAIRED_CHAT, PAIRED_CHAT, message_id, text, data);
    let id = up["callback_query"]["id"].as_str().unwrap().to_string();
    tg.push_update(up);
    id
}

fn confirm_edits(tg: &FakeTelegram, message_id: i64) -> Vec<Value> {
    edits_of(tg, message_id)
        .into_iter()
        .filter(|r| text_of(r).ends_with("Xác nhận Kill run 1?"))
        .collect()
}

#[test]
fn kill_with_confirmation_and_cancel() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# test\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(json!([entry]), &[]);
    sb.write_config(json!({
        "claude_bin": Sandbox::fake_claude_tree().to_str().unwrap(),
        "kill_grace_secs": 1,
    }));

    let pidfile = sb.root.join("tg_tree.pid");
    let mut srv = tg.serve(
        &sb,
        &[
            ("FAKE_TREE_PIDFILE", pidfile.to_str().unwrap()),
            ("FAKE_TREE_MODE", "coop"),
            ("FAKE_TREE_SLEEP", "317"),
        ],
    );
    wait_polling(&tg, &srv);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "chạy sleep kill tg", "name": "kill-tg"}),
    );
    assert_eq!(resp.status, 202, "{}", resp.text());
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    let job_pid = srv.get(&format!("/api/jobs/{job_id}")).json()["pid"]
        .as_i64()
        .unwrap() as i32;
    let job_proc = agent_brain::procs::proc_id(job_pid).expect("job proc_id");
    let [fake_id, bash_id, sleep_id] =
        common::tree_procs(&pidfile, Duration::from_secs(10)).expect("tree_procs");
    let _guard = common::ProcGuard(vec![job_proc, fake_id, bash_id, sleep_id]);
    assert_eq!(run_status(&sb, 1), "running");

    let rev = srv.get("/api/tasks/1").json()["rev"]
        .as_str()
        .unwrap()
        .to_string();
    let shown = "Run đang chạy";

    // First press: only the confirmation is shown.
    let ask = push_press(&tg, 9001, shown, &format!("k:1:1:{rev}"));
    let ans = wait_answer(&tg, &srv, &ask);
    assert_eq!(ans["json"]["text"], json!("Xác nhận Kill run 1?"), "{ans}");
    let confirm = wait_edit(&tg, &srv, 9001, |r| {
        text_of(r).ends_with("Xác nhận Kill run 1?")
    });
    assert_eq!(text_of(&confirm), "Run đang chạy\n\nXác nhận Kill run 1?");
    assert_eq!(
        buttons(&confirm),
        vec![
            ("Kill".to_string(), format!("K:1:1:{rev}")),
            ("Huỷ".to_string(), format!("x:1:1:{rev}")),
        ],
        "{confirm}"
    );
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(run_status(&sb, 1), "running", "a single press killed");
    assert!(agent_brain::procs::alive(sleep_id));

    // Huỷ: the message loses the question, the run keeps running.
    let cancel = push_press(&tg, 9001, text_of(&confirm), &format!("x:1:1:{rev}"));
    let ans = wait_answer(&tg, &srv, &cancel);
    assert_eq!(ans["json"]["text"], json!("Đã huỷ Kill"), "{ans}");
    assert!(
        wait_until(Duration::from_secs(10), || edits_of(&tg, 9001)
            .last()
            .is_some_and(|r| !text_of(r).contains("Xác nhận Kill"))),
        "{:#?}",
        tg.records()
    );
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(run_status(&sb, 1), "running", "Huỷ killed");
    assert!(agent_brain::procs::alive(fake_id));
    assert!(agent_brain::procs::alive(sleep_id));

    // Kill, then the confirming Kill.
    push_press(&tg, 9001, shown, &format!("k:1:1:{rev}"));
    assert!(
        wait_until(Duration::from_secs(10), || confirm_edits(&tg, 9001).len()
            == 2),
        "{:#?}",
        tg.records()
    );
    let confirm = confirm_edits(&tg, 9001).pop().unwrap();
    let ok = push_press(&tg, 9001, text_of(&confirm), &format!("K:1:1:{rev}"));
    wait_answer(&tg, &srv, &ok);
    assert!(
        wait_until(Duration::from_secs(10), || run_status(&sb, 1) == "killed"),
        "run 1 status {}",
        run_status(&sb, 1)
    );
    assert!(
        wait_until(Duration::from_secs(5), || {
            !agent_brain::procs::alive(fake_id)
                && !agent_brain::procs::alive(bash_id)
                && !agent_brain::procs::alive(sleep_id)
        }),
        "process tree survived the Kill"
    );
    let done = wait_edit(&tg, &srv, 9001, |r| text_of(r).contains("✓ Đã dừng run 1"));
    assert!(text_of(&done).contains("từ Telegram"), "{done}");
    assert!(!text_of(&done).contains("Xác nhận Kill"), "{done}");
    assert!(text_of(&done).starts_with(shown), "{done}");
    assert!(
        wait_until(Duration::from_secs(5), || stdout_of(&srv)
            .contains("Telegram: Kill task 1 — Đã dừng run 1")),
        "{}",
        stdout_of(&srv)
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// Plan 05.3-06 Task 3: messages follow the task
// ---------------------------------------------------------------------------

const OUTSIDE: &str = "xử lý ngoài Telegram (dashboard hoặc terminal)";

/// Waits until the sender tracks the message it sent: the registry entry is
/// written right after Telegram answered, before this stdout line.
fn wait_sent_line(srv: &Serve, line: &str) {
    assert!(
        wait_until(Duration::from_secs(10), || stdout_of(srv).contains(line)),
        "no {line:?} on stdout:\n{}\nstderr:\n{}",
        stdout_of(srv),
        stderr_of(srv)
    );
}

/// nhanh gate message, tracked by the bot.
fn nhanh_gate_message(tg: &FakeTelegram, srv: &Serve, sb: &Sandbox) -> Value {
    nhanh_gate(sb);
    let gate = wait_message(tg, srv, "Chờ duyệt plan · task 1");
    wait_sent_line(srv, "Telegram: đã gửi tin chờ duyệt plan cho task 1");
    gate
}

fn assert_outside_edit(tg: &FakeTelegram, srv: &Serve, gate: &Value) {
    let m = message_id_of(gate);
    let edit = match tg.wait_call("editMessageText", Duration::from_secs(5), |r| {
        r["json"]["message_id"].as_i64() == Some(m) && text_of(r).contains(OUTSIDE)
    }) {
        Some(r) => r,
        None => panic!(
            "no moved-on edit of message {m}; records:\n{:#?}\nstderr:\n{}",
            tg.records(),
            stderr_of(srv)
        ),
    };
    assert!(empty_keyboard(&edit), "{edit}");
    let text = text_of(&edit);
    assert!(text.starts_with(text_of(gate)), "{edit}");
    assert!(!text.contains("từ Telegram"), "{edit}");
    assert_eq!(edit["json"]["chat_id"], json!(PAIRED_CHAT), "{edit}");
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(edits_of(tg, m).len(), 1, "{:#?}", tg.records());
}

#[test]
fn dashboard_action_edits_message() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    protocol::setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    let gate = nhanh_gate_message(&tg, &srv, &sb);

    let resp = srv.post_json("/api/tasks/1/approve", &json!({}));
    assert_eq!(resp.status, 200, "{}", resp.text());
    assert_outside_edit(&tg, &srv, &gate);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn terminal_action_edits_message() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    protocol::setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    let gate = nhanh_gate_message(&tg, &srv, &sb);

    let res = sb.run(&["approve", "1"], None);
    assert_ok(&res, "approve 1");
    assert_outside_edit(&tg, &srv, &gate);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn telegram_action_edited_once() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    auto_start_to_gate(&sb, full_flow_scenario());
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    wait_sent_line(&srv, "Telegram: đã gửi tin chờ duyệt plan cho task 1");
    let m = message_id_of(&gate);

    let cb_id = press(&tg, PAIRED_CHAT, PAIRED_CHAT, &gate, "Duyệt plan");
    wait_answer(&tg, &srv, &cb_id);
    wait_edit(&tg, &srv, m, |r| text_of(r).contains("từ Telegram"));
    std::thread::sleep(Duration::from_secs(5));

    let edits = edits_of(&tg, m);
    assert_eq!(edits.len(), 1, "{edits:#?}");
    assert!(text_of(&edits[0]).contains("✓ Đã duyệt plan"), "{edits:#?}");
    assert!(!text_of(&edits[0]).contains(OUTSIDE), "{edits:#?}");
    assert!(
        !tg.calls("editMessageReplyMarkup")
            .iter()
            .any(|r| r["json"]["message_id"].as_i64() == Some(m)),
        "{:#?}",
        tg.records()
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

fn markups_of(tg: &FakeTelegram, message_id: i64) -> Vec<Value> {
    tg.calls("editMessageReplyMarkup")
        .into_iter()
        .filter(|r| r["json"]["message_id"].as_i64() == Some(message_id))
        .collect()
}

#[test]
fn running_run_adds_kill_button() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    setup_auto_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    plan_cap_stopped(&sb);
    let stop = wait_message(&tg, &srv, "Autopilot dừng · hết vòng · task 1");
    wait_sent_line(&srv, "Telegram: đã gửi tin autopilot dừng cho task 1");
    let m = message_id_of(&stop);
    let stop_labels = vec![
        "Chạy thêm 1 vòng".to_string(),
        "Trả về Lead".to_string(),
        "Dừng hẳn".to_string(),
    ];
    assert_eq!(labels(&stop), stop_labels, "{stop}");

    // Chạy Dev from the dashboard at the stop: a run that keeps running.
    sb.write_scenario(json!({"by_role": {"dev": [{"sleep": 60, "ops": []}]}}));
    let resp = srv.post_json("/api/tasks/1/dev", &json!({}));
    assert_eq!(resp.status, 202, "{}", resp.text());
    let mut dev_run = 0;
    assert!(
        wait_until(Duration::from_secs(10), || {
            let id: Option<i64> = sb
                .db()
                .query_row(
                    "SELECT MAX(id) FROM runs WHERE role = 'dev' AND status = 'running'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            dev_run = id.unwrap_or(0);
            dev_run > 0
        }),
        "no running dev run; stderr:\n{}",
        stderr_of(&srv)
    );
    // The fake claude of the dev run, stopped by its recorded PID if the test fails.
    let claude_pid: i64 = sb
        .db()
        .query_row("SELECT pid FROM runs WHERE id = ?1", [dev_run], |r| {
            r.get(0)
        })
        .unwrap();
    let claude = agent_brain::procs::proc_id(claude_pid as i32).expect("dev run proc_id");
    let _guard = common::ProcGuard(vec![claude]);

    let kill_data = |rec: &Value| -> Option<String> {
        buttons(rec)
            .into_iter()
            .find(|(l, _)| l == "Kill")
            .map(|(_, d)| d)
    };
    let refresh = match tg.wait_call("editMessageReplyMarkup", Duration::from_secs(10), |r| {
        r["json"]["message_id"].as_i64() == Some(m) && kill_data(r).is_some()
    }) {
        Some(r) => r,
        None => panic!(
            "no button refresh with Kill; records:\n{:#?}\nstderr:\n{}",
            tg.records(),
            stderr_of(&srv)
        ),
    };
    let rev = srv.get("/api/tasks/1").json()["rev"]
        .as_str()
        .unwrap()
        .to_string();
    let data = kill_data(&refresh).unwrap();
    assert_eq!(data, format!("k:1:{dev_run}:{rev}"), "{refresh}");
    // Kill is last; the other buttons are what the stop allows while the run
    // runs (no Trả về Lead while a run is busy).
    let refreshed = labels(&refresh);
    assert_eq!(
        refreshed.last().map(String::as_str),
        Some("Kill"),
        "{refresh}"
    );
    assert!(
        refreshed.contains(&"Chạy thêm 1 vòng".to_string()),
        "{refresh}"
    );
    assert!(refreshed.contains(&"Dừng hẳn".to_string()), "{refresh}");
    // Every button carries the task's current rev.
    for (_, d) in buttons(&refresh) {
        assert!(d.ends_with(&format!(":{rev}")), "{refresh}");
    }
    assert!(
        edits_of(&tg, m).is_empty(),
        "text untouched: {:#?}",
        tg.records()
    );

    // Kill from the refreshed message, then confirm.
    let shown = text_of(&stop).to_string();
    let ask = push_press(&tg, m, &shown, &data);
    wait_answer(&tg, &srv, &ask);
    let confirm = wait_edit(&tg, &srv, m, |r| {
        text_of(r).ends_with(&format!("Xác nhận Kill run {dev_run}?"))
    });
    let confirm_data = button_data(&confirm, "Kill");
    assert_eq!(confirm_data, format!("K:1:{dev_run}:{rev}"));
    assert_eq!(run_status(&sb, dev_run), "running");
    let ok = push_press(&tg, m, text_of(&confirm), &confirm_data);
    wait_answer(&tg, &srv, &ok);
    assert!(
        wait_until(Duration::from_secs(10), || run_status(&sb, dev_run)
            == "killed"),
        "dev run status {}",
        run_status(&sb, dev_run)
    );
    let done = wait_edit(&tg, &srv, m, |r| {
        text_of(r).contains(&format!("✓ Đã dừng run {dev_run}"))
    });
    assert!(text_of(&done).starts_with(&shown), "{done}");
    assert!(!text_of(&done).contains("Xác nhận Kill"), "{done}");
    // The stop is still current: its buttons stay (without Kill).
    assert_eq!(labels(&done), stop_labels, "{done}");
    let new_rev = srv.get("/api/tasks/1").json()["rev"]
        .as_str()
        .unwrap()
        .to_string();
    for (_, d) in buttons(&done) {
        assert!(d.ends_with(&format!(":{new_rev}")), "{done}");
    }
    // Nothing else edits the message afterwards (no moved-on edit, no refresh back).
    std::thread::sleep(Duration::from_secs(2));
    let newest = edits_of(&tg, m).pop().unwrap();
    assert_eq!(newest, done, "{:#?}", tg.records());
    assert!(
        markups_of(&tg, m).iter().all(|r| r == &refresh),
        "{:#?}",
        tg.records()
    );
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// WR-03: a press on a message whose text Telegram no longer gives the bot
// ---------------------------------------------------------------------------

/// A press as Telegram sends it for a message the bot can no longer read:
/// only `chat`, `message_id` and `date = 0` — no `text`.
fn press_unreadable(tg: &FakeTelegram, rec: &Value, message_id: i64, label: &str) -> String {
    let data = button_data(rec, label);
    let mut up = tg.callback_update(PAIRED_CHAT, PAIRED_CHAT, message_id, "", &data);
    up["callback_query"]["message"] = json!({
        "message_id": message_id,
        "date": 0,
        "chat": {"id": PAIRED_CHAT, "type": "private"},
    });
    let id = up["callback_query"]["id"].as_str().unwrap().to_string();
    tg.push_update(up);
    id
}

#[test]
fn press_without_text_keeps_tracked_text() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    protocol::setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    nhanh_gate(&sb);
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    let m = message_id_of(&gate);
    let shown = text_of(&gate).to_string();

    let cb_id = press_unreadable(&tg, &gate, m, "Duyệt plan");
    wait_answer(&tg, &srv, &cb_id);
    // The edit keeps the original text (from the tracked entry), not only the result line.
    let edit = wait_edit(&tg, &srv, m, |r| text_of(r).contains("✓ Đã duyệt plan"));
    assert!(text_of(&edit).starts_with(&shown), "{edit}");
    assert!(empty_keyboard(&edit), "{edit}");
    assert!(approved_commit(&sb, 1).is_some());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

#[test]
fn press_without_text_untracked_sends_new_message() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    protocol::setup_project(&sb, "work");

    let mut srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    nhanh_gate(&sb);
    let gate = wait_message(&tg, &srv, "Chờ duyệt plan · task 1");
    // Not the gate's message id: the bot tracks nothing for it and gets no text.
    let other: i64 = 9999;

    let cb_id = press_unreadable(&tg, &gate, other, "Duyệt plan");
    wait_answer(&tg, &srv, &cb_id);
    let result = match tg.wait_call("sendMessage", Duration::from_secs(10), |r| {
        text_of(r).contains("✓ Đã duyệt plan")
    }) {
        Some(r) => r,
        None => panic!(
            "no result message; records:\n{:#?}\nstderr:\n{}",
            tg.records(),
            stderr_of(&srv)
        ),
    };
    assert_eq!(
        result["json"]["reply_parameters"]["message_id"],
        json!(other),
        "{result}"
    );
    // The unreadable message is never overwritten with the result line alone.
    std::thread::sleep(Duration::from_secs(1));
    assert!(edits_of(&tg, other).is_empty(), "{:#?}", tg.records());
    let stripped = tg
        .calls("editMessageReplyMarkup")
        .into_iter()
        .any(|r| r["json"]["message_id"].as_i64() == Some(other) && empty_keyboard(&r));
    assert!(stripped, "{:#?}", tg.records());
    assert!(approved_commit(&sb, 1).is_some());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}
