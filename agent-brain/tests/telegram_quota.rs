//! Phase 6 plan 14: a quota stop reaches Telegram with one button per team, and the re-run on
//! another team needs a second confirming tap (D-17, D-28). Everything runs against the fake
//! claude and the local fake Telegram; nothing here talks to api.telegram.org.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use common::{wait_until, Sandbox, Serve};
use protocol::{lead_scenario, setup_project};
use regex::Regex;
use serde_json::{json, Value};
use std::time::Duration;
use tg::{write_config, FakeTelegram, PAIRED_CHAT, STRANGER_CHAT};

// ---------------------------------------------------------------------------
// Fake claude stream data
// ---------------------------------------------------------------------------

fn init_event() -> Value {
    json!({"type": "system", "subtype": "init", "session_id": "fake-quota", "model": "fake"})
}

fn rejected_rate_limit_event() -> Value {
    json!({
        "type": "rate_limit_event",
        "rate_limit_info": {
            "status": "rejected",
            "resetsAt": 1792310400,
            "rateLimitType": "five_hour",
            "isUsingOverage": false,
            "overageStatus": "rejected"
        },
        "uuid": "u1",
        "session_id": "fake-quota"
    })
}

fn quota_result_event() -> Value {
    json!({
        "type": "result",
        "subtype": "success",
        "is_error": true,
        "api_error_status": 429,
        "terminal_reason": "blocking_limit",
        "result": "You've hit your session limit",
        "num_turns": 1,
        "session_id": "fake-quota",
        "total_cost_usd": 0,
        "usage": {
            "input_tokens": 0,
            "output_tokens": 0,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0
        },
        "permission_denials": []
    })
}

fn success_result_event() -> Value {
    json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "num_turns": 1,
        "session_id": "fake-quota",
        "total_cost_usd": 0,
        "usage": {
            "input_tokens": 10,
            "output_tokens": 5,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0
        },
        "permission_denials": []
    })
}

fn write_op(path: &str, content: &str) -> Value {
    json!({"op": "write", "path": path, "content": content})
}

fn quota_entry(ops: Vec<Value>) -> Value {
    json!({
        "ops": ops,
        "events": [init_event(), rejected_rate_limit_event(), quota_result_event()],
        "exit_code": 1,
    })
}

fn ok_entry(ops: Vec<Value>) -> Value {
    json!({
        "ops": ops,
        "events": [init_event(), success_result_event()],
        "exit_code": 0,
    })
}

fn by_role(roles: Value) -> Value {
    json!({ "by_role": roles })
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

fn wait_message(tg: &FakeTelegram, srv: &Serve, prefix: &str) -> Value {
    match tg.wait_call("sendMessage", Duration::from_secs(20), |r| {
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

fn empty_keyboard(rec: &Value) -> bool {
    rec["json"]["reply_markup"]["inline_keyboard"] == json!([])
}

/// The message id a record (a `sendMessage` answer or an `editMessageText` request) is about.
fn message_id_of(rec: &Value) -> i64 {
    rec["response"]["result"]["message_id"]
        .as_i64()
        .or_else(|| rec["json"]["message_id"].as_i64())
        .unwrap_or_else(|| panic!("no message_id in {rec}"))
}

/// Presses `data` on message `message_id` showing `text`; returns the callback id.
fn press_data(tg: &FakeTelegram, from: i64, message_id: i64, text: &str, data: &str) -> String {
    let up = tg.callback_update(PAIRED_CHAT, from, message_id, text, data);
    let id = up["callback_query"]["id"].as_str().unwrap().to_string();
    tg.push_update(up);
    id
}

/// Presses the button `label` of the record `rec` (a message or an edit of it).
fn press(tg: &FakeTelegram, rec: &Value, label: &str) -> String {
    press_data(
        tg,
        PAIRED_CHAT,
        message_id_of(rec),
        text_of(rec),
        &button_data(rec, label),
    )
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

fn wait_edit(
    tg: &FakeTelegram,
    srv: &Serve,
    message_id: i64,
    pred: impl Fn(&Value) -> bool,
) -> Value {
    match tg.wait_call("editMessageText", Duration::from_secs(15), |r| {
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

// ---------------------------------------------------------------------------
// Scenario
// ---------------------------------------------------------------------------

struct Env {
    sb: Sandbox,
    tg: FakeTelegram,
    srv: Serve,
    /// The quota message the bot sent.
    quota: Value,
}

fn write_teams(sb: &Sandbox) {
    sb.write_teams(json!({
        "doi-b": {"description": "Toàn claude, đội B", "roles": {}}
    }));
}

/// Manual task `sua-app` (origin `origin` when given) whose Dev run stopped on quota while serve
/// was running; the second queued Dev entry completes the work. Teams: mac-dinh, tiet-kiem, doi-b.
fn setup_quota(origin: Option<&str>) -> Env {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    write_teams(&sb);
    setup_project(&sb, "work");

    let mut args = vec!["run", "lead"];
    if let Some(o) = origin {
        args.extend(["--origin", o]);
    }
    args.extend(["--name", "sua-app", "--", "web-ban-hang", "sửa app"]);
    let out = sb.run(
        &args,
        Some(lead_scenario(&[(
            "src/app.py",
            "app()",
            "trả về 2",
            "yêu cầu",
        )])),
    );
    assert_eq!(out.code, 0, "run lead: {}\n{}", out.stderr, out.stdout);

    let srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);

    let sc = by_role(json!({
        "dev": [
            quota_entry(vec![write_op("src/app.py", "phần 1")]),
            ok_entry(vec![write_op("src/app.py", "def app():\n    return 3\n")]),
        ]
    }));
    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(sc));
    assert_eq!(out.code, 5, "run dev: {}\n{}", out.stderr, out.stdout);

    let quota = wait_message(&tg, &srv, "Hết quota engine claude · task 1");
    Env { sb, tg, srv, quota }
}

fn team_json_of(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT team_json FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
}

fn run_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap()
}

fn latest_dev_run_id(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row(
            "SELECT id FROM runs WHERE role = 'dev' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

// ---------------------------------------------------------------------------
// Task 1 (tracer)
// ---------------------------------------------------------------------------

#[test]
fn quota_message_with_team_buttons() {
    let env = setup_quota(None);
    let dev_id = latest_dev_run_id(&env.sb);

    let text = text_of(&env.quota);
    assert!(
        text.starts_with("Hết quota engine claude · task 1 sua-app"),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "Run {dev_id} (dev) trên engine claude dừng vì hết quota"
        )),
        "{text}"
    );
    assert!(text.contains("bước Code"), "{text}");

    assert_eq!(
        labels(&env.quota),
        vec![
            "Đội hình mac-dinh",
            "Đội hình tiet-kiem",
            "Đội hình doi-b",
            "Thử lại sau"
        ]
    );
    let pick = Regex::new(r"^t:1:[0-2]:[0-9a-f]{4}:[0-9a-f]{16}$").unwrap();
    let later = Regex::new(r"^l:1:[0-9a-f]{16}$").unwrap();
    let data = buttons(&env.quota);
    for (label, d) in &data[..3] {
        assert!(pick.is_match(d), "{label}: {d}");
        assert!(d.len() <= 64, "{d}");
    }
    assert!(later.is_match(&data[3].1), "{data:?}");
    // No Xong or Trả lại button on Telegram (D-20).
    assert!(
        !labels(&env.quota)
            .iter()
            .any(|l| l == "Xong" || l == "Trả lại"),
        "{:?}",
        labels(&env.quota)
    );
    // One message, once.
    assert_eq!(env.tg.calls("sendMessage").len(), 1);
}

#[test]
fn quota_two_tap_rerun() {
    let mut env = setup_quota(None);
    let m = message_id_of(&env.quota);
    let team_before = team_json_of(&env.sb, 1);
    let runs_before = run_count(&env.sb);

    // First tap: only the confirmation, nothing runs and nothing changes (D-28).
    let cb = press(&env.tg, &env.quota, "Đội hình doi-b");
    let ans = wait_answer(&env.tg, &env.srv, &cb);
    assert_ne!(ans["json"]["show_alert"], json!(true), "{ans}");
    let confirm = wait_edit(&env.tg, &env.srv, m, |r| {
        text_of(r).starts_with("Chạy lại bước Code bằng đội hình doi-b?")
    });
    assert!(
        text_of(&confirm).contains("Bước Code sẽ chạy bằng: claude"),
        "{confirm}"
    );
    assert_eq!(
        labels(&confirm),
        vec!["Chạy lại bước này", "Không chạy lại"]
    );
    assert!(
        button_data(&confirm, "Chạy lại bước này").starts_with("u:1:"),
        "{confirm}"
    );
    assert!(
        button_data(&confirm, "Không chạy lại").starts_with("v:1:"),
        "{confirm}"
    );
    assert_eq!(run_count(&env.sb), runs_before, "no run on the first tap");
    assert_eq!(
        team_json_of(&env.sb, 1),
        team_before,
        "no write on the first tap"
    );

    // Second tap: the shared re-run starts a job.
    let cb = press(&env.tg, &confirm, "Chạy lại bước này");
    wait_answer(&env.tg, &env.srv, &cb);
    let done = wait_edit(&env.tg, &env.srv, m, |r| {
        text_of(r).starts_with("Đã đổi sang đội hình doi-b và giao lại bước Code (job ")
    });
    assert!(empty_keyboard(&done), "{done}");
    assert!(text_of(&done).contains("(job 1)."), "{done}");

    assert!(
        wait_until(Duration::from_secs(60), || {
            env.sb
                .db()
                .query_row(
                    "SELECT COUNT(*) FROM runs WHERE role = 'dev' AND status = 'succeeded'",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap_or(0)
                == 1
        }),
        "the re-run did not finish; stderr:\n{}",
        stderr_of(&env.srv)
    );
    let team: Value = serde_json::from_str(&team_json_of(&env.sb, 1).expect("team_json")).unwrap();
    assert_eq!(team["name"], "doi-b");
    let last = team["history"].as_array().unwrap().last().cloned().unwrap();
    assert_eq!(last["by"], "telegram", "{team}");
    assert_eq!(last["to"], "doi-b", "{team}");
    assert_eq!(env.srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

// ---------------------------------------------------------------------------
// Task 2: stale list or rev, cancel, later, Telegram-origin rule, strangers
// ---------------------------------------------------------------------------

const REFUSED: &str = "Dự án không còn trong danh sách Telegram — duyệt plan trên dashboard.";
const TEAMS_CHANGED: &str = "Danh sách đội hình đã đổi — chọn lại.";

fn approved_commit(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT approved_commit FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
}

fn wait_offset(tg: &FakeTelegram, srv: &Serve, offset: i64) {
    assert!(
        wait_until(Duration::from_secs(10), || tg
            .calls("getUpdates")
            .iter()
            .any(|r| r["json"]["offset"].as_i64() == Some(offset))),
        "no getUpdates with offset {offset}; stderr:\n{}",
        stderr_of(srv)
    );
}

fn stop(mut env: Env) {
    assert_eq!(env.srv.stop(libc::SIGINT, Duration::from_secs(10)), Some(0));
}

fn wait_dev_succeeded(env: &Env) {
    assert!(
        wait_until(Duration::from_secs(60), || {
            env.sb
                .db()
                .query_row(
                    "SELECT COUNT(*) FROM runs WHERE role = 'dev' AND status = 'succeeded'",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap_or(0)
                == 1
        }),
        "the re-run did not finish; stderr:\n{}",
        stderr_of(&env.srv)
    );
}

#[test]
fn quota_stale_team_list() {
    let env = setup_quota(None);
    let m = message_id_of(&env.quota);
    let team_before = team_json_of(&env.sb, 1);
    let runs_before = run_count(&env.sb);
    let old_pick = button_data(&env.quota, "Đội hình doi-b");

    // A team is added after the buttons were sent: the old list is no longer the server's.
    env.sb.write_teams(json!({
        "doi-b": {"description": "Toàn claude, đội B", "roles": {}},
        "doi-c": {"description": "Toàn claude, đội C", "roles": {}}
    }));
    let cb = press(&env.tg, &env.quota, "Đội hình doi-b");
    let ans = wait_answer(&env.tg, &env.srv, &cb);
    assert_eq!(ans["json"]["text"], json!(TEAMS_CHANGED), "{ans}");
    let fresh = wait_edit(&env.tg, &env.srv, m, |r| {
        labels(r).iter().any(|l| l == "Đội hình doi-c")
    });
    assert!(
        text_of(&fresh).starts_with("Hết quota engine claude · task 1"),
        "{fresh}"
    );
    assert_eq!(
        labels(&fresh),
        vec![
            "Đội hình mac-dinh",
            "Đội hình tiet-kiem",
            "Đội hình doi-b",
            "Đội hình doi-c",
            "Thử lại sau"
        ]
    );
    // The new buttons carry another list version; the rev is the same.
    let new_pick = button_data(&fresh, "Đội hình doi-b");
    let ver = |d: &str| d.split(':').nth(3).unwrap().to_string();
    assert_ne!(ver(&old_pick), ver(&new_pick));
    assert_eq!(run_count(&env.sb), runs_before);
    assert_eq!(team_json_of(&env.sb, 1), team_before);

    // The old confirm of a stale list is stale too: nothing runs.
    let forged = old_pick.replacen("t:", "u:", 1);
    let cb = press_data(&env.tg, PAIRED_CHAT, m, text_of(&fresh), &forged);
    let ans = wait_answer(&env.tg, &env.srv, &cb);
    assert_eq!(ans["json"]["text"], json!(TEAMS_CHANGED), "{ans}");
    assert_eq!(run_count(&env.sb), runs_before);
    assert_eq!(team_json_of(&env.sb, 1), team_before);
    stop(env);
}

#[test]
fn quota_stale_rev() {
    let env = setup_quota(None);
    let m = message_id_of(&env.quota);
    let runs_before = run_count(&env.sb);

    let cb = press(&env.tg, &env.quota, "Đội hình doi-b");
    wait_answer(&env.tg, &env.srv, &cb);
    let confirm = wait_edit(&env.tg, &env.srv, m, |r| {
        text_of(r).starts_with("Chạy lại bước Code bằng đội hình doi-b?")
    });

    // The team changes elsewhere (the dashboard): the task's rev moves on.
    let moved = r#"{"name":"tiet-kiem"}"#;
    env.sb
        .db()
        .execute("UPDATE tasks SET team_json = ?1 WHERE id = 1", [moved])
        .unwrap();

    let cb = press(&env.tg, &confirm, "Chạy lại bước này");
    let ans = wait_answer(&env.tg, &env.srv, &cb);
    assert!(
        ans["json"]["text"]
            .as_str()
            .unwrap_or("")
            .starts_with("Task đã đổi trạng thái"),
        "{ans}"
    );
    let gone = wait_edit(&env.tg, &env.srv, m, |r| {
        text_of(r).contains("Task đã đổi trạng thái")
    });
    assert!(empty_keyboard(&gone), "{gone}");
    assert_eq!(run_count(&env.sb), runs_before, "nothing ran");
    assert_eq!(team_json_of(&env.sb, 1).as_deref(), Some(moved));
    stop(env);
}

#[test]
fn quota_cancel_restores() {
    let env = setup_quota(None);
    let m = message_id_of(&env.quota);
    let team_before = team_json_of(&env.sb, 1);
    let runs_before = run_count(&env.sb);

    let cb = press(&env.tg, &env.quota, "Đội hình tiet-kiem");
    wait_answer(&env.tg, &env.srv, &cb);
    let confirm = wait_edit(&env.tg, &env.srv, m, |r| {
        text_of(r).starts_with("Chạy lại bước Code bằng đội hình tiet-kiem?")
    });
    assert!(
        text_of(&confirm).contains("Bước Code sẽ chạy bằng: deepseek"),
        "{confirm}"
    );

    let cb = press(&env.tg, &confirm, "Không chạy lại");
    wait_answer(&env.tg, &env.srv, &cb);
    let back = wait_edit(&env.tg, &env.srv, m, |r| {
        text_of(r).starts_with("Hết quota engine claude · task 1")
    });
    assert_eq!(
        labels(&back),
        vec![
            "Đội hình mac-dinh",
            "Đội hình tiet-kiem",
            "Đội hình doi-b",
            "Thử lại sau"
        ]
    );
    assert_eq!(run_count(&env.sb), runs_before);
    assert_eq!(team_json_of(&env.sb, 1), team_before);

    // The restored buttons work: the pick opens the confirmation again.
    let cb = press(&env.tg, &back, "Đội hình doi-b");
    wait_answer(&env.tg, &env.srv, &cb);
    wait_edit(&env.tg, &env.srv, m, |r| {
        text_of(r).starts_with("Chạy lại bước Code bằng đội hình doi-b?")
    });
    stop(env);
}

#[test]
fn quota_retry_later_writes_nothing() {
    let env = setup_quota(None);
    let m = message_id_of(&env.quota);
    let team_before = team_json_of(&env.sb, 1);
    let runs_before = run_count(&env.sb);

    let cb = press(&env.tg, &env.quota, "Thử lại sau");
    let ans = wait_answer(&env.tg, &env.srv, &cb);
    assert_eq!(ans["json"]["text"], json!("Đã hoãn."), "{ans}");
    let edit = wait_edit(&env.tg, &env.srv, m, |r| {
        text_of(r).contains("Đã hoãn — khi quota hồi phục, chạy lại bước này trên dashboard.")
    });
    assert!(
        text_of(&edit).starts_with(text_of(&env.quota)),
        "the message keeps its text: {edit}"
    );
    assert!(empty_keyboard(&edit), "{edit}");
    assert_eq!(run_count(&env.sb), runs_before);
    assert_eq!(team_json_of(&env.sb, 1), team_before);
    // Postponing is not a new event: nothing more is sent.
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(env.tg.calls("sendMessage").len(), 1);
    stop(env);
}

#[test]
fn quota_rerun_telegram_origin_not_approval() {
    let env = setup_quota(Some("telegram"));
    let m = message_id_of(&env.quota);
    let approved_before = approved_commit(&env.sb, 1);

    // A forged `Duyệt plan` with the right rev stays refused for a Telegram-origin task.
    let rev = button_data(&env.quota, "Thử lại sau")
        .strip_prefix("l:1:")
        .expect("l:1:{rev}")
        .to_string();
    let cb = press_data(
        &env.tg,
        PAIRED_CHAT,
        m,
        text_of(&env.quota),
        &format!("a:1:{rev}"),
    );
    let ans = wait_answer(&env.tg, &env.srv, &cb);
    // The terminal Dev run already recorded its own approval; Telegram refuses either way (the
    // gate-time `TELEGRAM_APPROVE_REFUSED` answer is asserted by telegram_intake).
    let text = ans["json"]["text"].as_str().unwrap_or("");
    assert!(
        text == REFUSED || text.starts_with("Plan hiện tại đã được duyệt"),
        "{ans}"
    );
    assert_eq!(ans["json"]["show_alert"], json!(true), "{ans}");
    assert_eq!(approved_commit(&env.sb, 1), approved_before);

    // The quota re-run itself works through the two taps.
    let cb = press(&env.tg, &env.quota, "Đội hình doi-b");
    wait_answer(&env.tg, &env.srv, &cb);
    let confirm = wait_edit(&env.tg, &env.srv, m, |r| {
        text_of(r).starts_with("Chạy lại bước Code bằng đội hình doi-b?")
    });
    let cb = press(&env.tg, &confirm, "Chạy lại bước này");
    wait_answer(&env.tg, &env.srv, &cb);
    wait_edit(&env.tg, &env.srv, m, |r| {
        text_of(r).starts_with("Đã đổi sang đội hình doi-b và giao lại bước Code (job ")
    });
    wait_dev_succeeded(&env);
    // The re-run approved nothing.
    assert_eq!(approved_commit(&env.sb, 1), approved_before);
    stop(env);
}

#[test]
fn quota_stranger_ignored() {
    let env = setup_quota(None);
    let m = message_id_of(&env.quota);
    let team_before = team_json_of(&env.sb, 1);
    let runs_before = run_count(&env.sb);
    let pick = button_data(&env.quota, "Đội hình doi-b");
    let text = text_of(&env.quota).to_string();

    // (a) another chat, (b) the paired chat pressed by another sender.
    let a = env
        .tg
        .callback_update(STRANGER_CHAT, STRANGER_CHAT, m, &text, &pick);
    let b = env.tg.callback_update(PAIRED_CHAT, 8888, m, &text, &pick);
    let a_id = a["callback_query"]["id"].as_str().unwrap().to_string();
    let b_id = b["callback_query"]["id"].as_str().unwrap().to_string();
    env.tg.push_update(a);
    let last = env.tg.push_update(b);
    wait_offset(&env.tg, &env.srv, last + 1);
    std::thread::sleep(Duration::from_secs(2));

    for id in [&a_id, &b_id] {
        assert!(
            env.tg
                .calls("answerCallbackQuery")
                .iter()
                .all(|r| r["json"]["callback_query_id"].as_str() != Some(id.as_str())),
            "{:#?}",
            env.tg.records()
        );
    }
    assert!(env.tg.calls("editMessageText").is_empty());
    assert_eq!(run_count(&env.sb), runs_before);
    assert_eq!(team_json_of(&env.sb, 1), team_before);
    stop(env);
}
