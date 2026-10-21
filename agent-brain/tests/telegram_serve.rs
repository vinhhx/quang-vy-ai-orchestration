mod common;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use agent_brain::procs::{alive, descendants, ProcId};
use common::{wait_until, Sandbox, Serve};
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tg::{
    telegram_json_path, write_config, write_raw_config, FakeTelegram, FAKE_TOKEN, PAIRED_CHAT,
};

fn stdout_of(srv: &Serve) -> String {
    srv.stdout.lock().unwrap().clone()
}

fn stderr_of(srv: &Serve) -> String {
    std::fs::read_to_string(&srv.stderr_path).unwrap_or_default()
}

#[test]
fn serve_connects_to_fake_telegram() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let mut srv = tg.serve(&sb, &[]);

    let connected = "Telegram: đã kết nối (@ab_fake_bot)";
    assert!(
        wait_until(Duration::from_secs(10), || stdout_of(&srv)
            .contains(connected)),
        "no connected line; stdout:\n{}\nstderr:\n{}",
        stdout_of(&srv),
        stderr_of(&srv)
    );

    let out = stdout_of(&srv);
    let url_pos = out
        .find("http://127.0.0.1:")
        .expect("URL line in serve stdout");
    let tg_pos = out.find(connected).expect("connected line");
    assert!(url_pos < tg_pos, "URL line must come first:\n{out}");
    assert!(
        out.lines().next().unwrap_or("").contains("/?token="),
        "first stdout line must stay the dashboard URL:\n{out}"
    );

    let calls = tg.calls("getMe");
    assert!(!calls.is_empty(), "fake Telegram recorded no getMe");
    for c in &calls {
        assert_eq!(c["token"].as_str(), Some(FAKE_TOKEN));
    }

    assert!(!out.contains(FAKE_TOKEN), "token leaked to stdout");
    assert!(
        !stderr_of(&srv).contains(FAKE_TOKEN),
        "token leaked to stderr"
    );

    assert_eq!(srv.get("/").status, 200);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

fn telegram_lines(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.starts_with("Telegram"))
        .map(|l| l.to_string())
        .collect()
}

fn cmdline(pid: i32) -> String {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    String::from_utf8_lossy(&raw).replace('\0', " ")
}

/// Waits until the fake consumed the single delayed getMe rule (the request is
/// pending inside the fake), then returns the curl descendant of serve.
fn pending_curl(tg: &FakeTelegram, srv: &Serve) -> (ProcId, String) {
    let behavior = tg.dir.join("behavior.json");
    assert!(
        wait_until(Duration::from_secs(10), || {
            std::fs::read_to_string(&behavior)
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|v| v[0]["times"].as_i64())
                == Some(0)
        }),
        "getMe never reached the fake Telegram"
    );
    let serve_pid = srv.child.id() as i32;
    let mut found = None;
    wait_until(Duration::from_secs(5), || {
        found = descendants(serve_pid).into_iter().find_map(|p| {
            let c = cmdline(p.pid);
            if c.contains("curl") {
                Some((p, c))
            } else {
                None
            }
        });
        found.is_some()
    });
    found.expect("no curl descendant of serve while getMe is pending")
}

#[test]
fn no_config_serve_unchanged() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    let mut srv = tg.serve(&sb, &[]);
    std::thread::sleep(Duration::from_secs(2));
    assert!(
        telegram_lines(&stdout_of(&srv)).is_empty(),
        "{}",
        stdout_of(&srv)
    );
    assert!(
        telegram_lines(&stderr_of(&srv)).is_empty(),
        "{}",
        stderr_of(&srv)
    );
    assert!(tg.records().is_empty(), "{:?}", tg.records());
    assert_eq!(srv.get("/").status, 200);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn insecure_file_refused() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o644);
    let path = telegram_json_path(&sb);
    let mut srv = tg.serve(&sb, &[]);
    assert!(
        wait_until(Duration::from_secs(5), || stderr_of(&srv)
            .contains("Telegram: lỗi —")),
        "{}",
        stderr_of(&srv)
    );
    let err = stderr_of(&srv);
    assert!(err.contains("chmod 600"), "{err}");
    assert!(err.contains(&path.display().to_string()), "{err}");
    assert!(!err.contains(FAKE_TOKEN));
    std::thread::sleep(Duration::from_millis(500));
    assert!(tg.records().is_empty());
    assert_eq!(srv.get("/").status, 200);
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o644, "mode must not be fixed silently");
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn invalid_file_refused() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    let raw = r#"{"token": "", "chat_id": 0}"#;
    write_raw_config(&sb, raw, 0o600);
    let mut srv = tg.serve(&sb, &[]);
    assert!(
        wait_until(Duration::from_secs(5), || stderr_of(&srv)
            .contains("không hợp lệ")),
        "{}",
        stderr_of(&srv)
    );
    let err = stderr_of(&srv);
    assert!(err.contains("Telegram: lỗi —"), "{err}");
    assert!(!err.contains(raw), "{err}");
    std::thread::sleep(Duration::from_millis(500));
    assert!(tg.records().is_empty());
    assert_eq!(srv.get("/").status, 200);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn missing_curl_refused() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let mut srv = tg.serve(&sb, &[("AGENT_BRAIN_CURL", "/nonexistent/curl")]);
    assert!(
        wait_until(Duration::from_secs(5), || stderr_of(&srv)
            .contains("Không tìm thấy lệnh curl")),
        "{}",
        stderr_of(&srv)
    );
    assert!(stderr_of(&srv).contains("Telegram: lỗi —"));
    assert!(tg.records().is_empty());
    assert_eq!(srv.get("/").status, 200);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn unauthorized_token_reported() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    tg.set_behavior(json!([
        {"method": "getMe", "times": 1, "status": 401, "description": "Unauthorized"}
    ]));
    write_config(&sb, PAIRED_CHAT, 0o600);
    let mut srv = tg.serve(&sb, &[]);
    assert!(
        wait_until(Duration::from_secs(10), || stderr_of(&srv)
            .contains("Telegram từ chối token (401)")),
        "{}",
        stderr_of(&srv)
    );
    std::thread::sleep(Duration::from_millis(800));
    assert!(!stdout_of(&srv).contains("Telegram: đã kết nối"));
    assert_eq!(tg.calls("getMe").len(), 1, "no retry after 401");
    assert!(!stderr_of(&srv).contains(FAKE_TOKEN));
    assert_eq!(srv.get("/").status, 200);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn unreachable_one_warning() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let closed_port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        l.local_addr().unwrap().port()
    };
    let api = format!("http://127.0.0.1:{closed_port}");
    let mut srv = tg.serve(
        &sb,
        &[
            ("AGENT_BRAIN_TELEGRAM_API", api.as_str()),
            ("AGENT_BRAIN_TELEGRAM_BACKOFF_MS", "200"),
        ],
    );
    std::thread::sleep(Duration::from_secs(3));
    let err = stderr_of(&srv);
    let warnings = err
        .lines()
        .filter(|l| l.contains("Telegram: không tới được Telegram"))
        .count();
    assert_eq!(warnings, 1, "{err}");
    assert!(!err.contains(FAKE_TOKEN));
    assert!(!stdout_of(&srv).contains("Telegram: đã kết nối"));
    assert_eq!(srv.get("/").status, 200);
    assert!(tg.records().is_empty());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn token_not_on_curl_argv() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    tg.set_behavior(json!([{"method": "getMe", "times": 1, "delay_ms": 3000}]));
    write_config(&sb, PAIRED_CHAT, 0o600);
    let mut srv = tg.serve(&sb, &[]);
    let (_curl, cmd) = pending_curl(&tg, &srv);
    assert!(cmd.contains("-K"), "{cmd}");
    assert!(!cmd.contains(FAKE_TOKEN), "token on curl argv: {cmd}");
    assert!(
        wait_until(Duration::from_secs(10), || stdout_of(&srv)
            .contains("Telegram: đã kết nối (@ab_fake_bot)")),
        "{}",
        stdout_of(&srv)
    );
    assert!(!stdout_of(&srv).contains(FAKE_TOKEN));
    assert!(!stderr_of(&srv).contains(FAKE_TOKEN));
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn sigint_during_pending_call_is_prompt() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    tg.set_behavior(json!([{"method": "getMe", "times": 1, "delay_ms": 20000}]));
    write_config(&sb, PAIRED_CHAT, 0o600);
    let mut srv = tg.serve(&sb, &[]);
    let (curl, _) = pending_curl(&tg, &srv);
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

#[test]
fn transport_upload_reaches_stub() {
    use agent_brain::telegram::transport::{Body, Transport};
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    let up = sb.root.join("up");
    let stop = Arc::new(AtomicBool::new(false));
    let t = Transport::new(
        FAKE_TOKEN.to_string(),
        tg.url(),
        "curl".to_string(),
        up.clone(),
        stop,
    );
    let caption = "Plan task 1: \"x\"; type=evil";
    let content = "# Plan\nnội dung\n";
    let res = t
        .call(
            "sendDocument",
            &Body::Upload {
                fields: vec![
                    ("chat_id".to_string(), "5555".to_string()),
                    ("caption".to_string(), caption.to_string()),
                ],
                file_field: "document".to_string(),
                filename: "sua-app.plan.md".to_string(),
                content: content.to_string(),
            },
            20,
        )
        .expect("sendDocument through curl");
    assert!(
        res.get("message_id").and_then(|m| m.as_i64()).is_some(),
        "{res}"
    );

    let rec = tg.calls("sendDocument").pop().expect("sendDocument record");
    assert_eq!(rec["token"].as_str(), Some(FAKE_TOKEN));
    let form = &rec["form"];
    assert_eq!(form["caption"].as_str(), Some(caption), "{rec}");
    assert_eq!(form["chat_id"].as_str(), Some("5555"), "{rec}");
    assert_eq!(
        form["document"]["filename"].as_str(),
        Some("sua-app.plan.md"),
        "{rec}"
    );
    assert_eq!(form["document"]["content"].as_str(), Some(content), "{rec}");

    let leftovers: Vec<_> = std::fs::read_dir(&up)
        .map(|it| it.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    assert!(
        leftovers.is_empty(),
        "upload dir not cleaned: {leftovers:?}"
    );
}
