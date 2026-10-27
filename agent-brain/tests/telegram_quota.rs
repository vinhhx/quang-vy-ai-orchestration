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
    let _ = STRANGER_CHAT;
}
