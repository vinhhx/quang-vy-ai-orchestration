//! `agent-brain telegram setup` / `telegram test` against the fake Telegram
//! (Phase 05.3 plan 07, D-08/D-09). Every run uses its own sandbox (XDG dirs
//! inside a tempdir) and its own fake; no test contacts api.telegram.org.

mod common;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use common::{wait_until, Sandbox};
use std::io::{BufRead, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tg::{telegram_json_path, FakeTelegram, BOT_USERNAME, FAKE_TOKEN, PAIRED_CHAT};

/// A running `agent-brain telegram …` child with its output collected on threads.
struct CliRun {
    child: Child,
    out: Arc<Mutex<String>>,
    err: Arc<Mutex<String>>,
}

impl CliRun {
    fn spawn(sb: &Sandbox, args: &[&str], env: &[(String, String)], stdin_text: &str) -> CliRun {
        let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let mut cmd = sb.command(args, &env);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        let mut child = cmd.spawn().expect("spawn agent-brain telegram");
        let mut stdin = child.stdin.take().expect("child stdin");
        stdin
            .write_all(stdin_text.as_bytes())
            .expect("write child stdin");
        drop(stdin);

        let out = Arc::new(Mutex::new(String::new()));
        let err = Arc::new(Mutex::new(String::new()));
        let stdout = child.stdout.take().expect("child stdout");
        let out2 = out.clone();
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stdout);
            let mut line = String::new();
            while let Ok(n) = reader.read_line(&mut line) {
                if n == 0 {
                    break;
                }
                out2.lock().unwrap().push_str(&line);
                line.clear();
            }
        });
        let mut stderr = child.stderr.take().expect("child stderr");
        let err2 = err.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = stderr.read(&mut buf) {
                if n == 0 {
                    break;
                }
                err2.lock()
                    .unwrap()
                    .push_str(&String::from_utf8_lossy(&buf[..n]));
            }
        });
        CliRun { child, out, err }
    }

    fn stdout(&self) -> String {
        self.out.lock().unwrap().clone()
    }

    fn stderr(&self) -> String {
        self.err.lock().unwrap().clone()
    }

    /// Waits for the code line and returns its 6 digits.
    fn wait_code(&self) -> String {
        let mut code = None;
        wait_until(Duration::from_secs(10), || {
            code = self
                .stdout()
                .lines()
                .find(|l| l.contains("Mở Telegram"))
                .map(|l| {
                    let chars: Vec<char> = l.trim_end().chars().collect();
                    chars[chars.len().saturating_sub(6)..].iter().collect()
                });
            code.is_some()
        });
        let code: String = code.unwrap_or_else(|| {
            panic!(
                "no code line; stdout:\n{}\nstderr:\n{}",
                self.stdout(),
                self.stderr()
            )
        });
        assert!(
            code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()),
            "code {code:?}"
        );
        code
    }

    /// Exit code; kills the child (by its own PID) and panics on timeout.
    fn wait_exit(&mut self, timeout: Duration) -> i32 {
        let start = Instant::now();
        loop {
            if let Some(st) = self.child.try_wait().expect("try_wait") {
                // Let the reader threads drain the pipes.
                std::thread::sleep(Duration::from_millis(100));
                use std::os::unix::process::ExitStatusExt;
                return st.code().unwrap_or_else(|| 128 + st.signal().unwrap_or(0));
            }
            if start.elapsed() > timeout {
                let _ = self.child.kill();
                let _ = self.child.wait();
                panic!(
                    "agent-brain telegram did not exit; stdout:\n{}\nstderr:\n{}",
                    self.stdout(),
                    self.stderr()
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for CliRun {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn read_json(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read telegram.json"))
        .expect("parse telegram.json")
}

fn mode_of(path: &std::path::Path) -> u32 {
    std::fs::metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777
}

fn token_line() -> String {
    format!("{FAKE_TOKEN}\n")
}

#[test]
fn setup_pairs_private_chat() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    let mut run = CliRun::spawn(&sb, &["telegram", "setup"], &tg.env(), &token_line());
    let code = run.wait_code();
    tg.push_update(tg.message_update(PAIRED_CHAT, PAIRED_CHAT, "private", &code));
    let exit = run.wait_exit(Duration::from_secs(20));
    let (out, err) = (run.stdout(), run.stderr());
    assert_eq!(exit, 0, "stdout:\n{out}\nstderr:\n{err}");

    let path = telegram_json_path(&sb);
    assert!(path.is_file(), "telegram.json not written");
    assert_eq!(mode_of(&path), 0o600);
    let v = read_json(&path);
    assert_eq!(v["chat_id"].as_i64(), Some(PAIRED_CHAT));
    assert_eq!(v["token"].as_str(), Some(FAKE_TOKEN));

    assert!(
        out.contains(&format!(
            "Đã ghép nối với chat {PAIRED_CHAT} (@{BOT_USERNAME})"
        )),
        "{out}"
    );
    assert!(
        tg.calls("sendMessage").iter().any(|r| {
            r["json"]["chat_id"].as_i64() == Some(PAIRED_CHAT)
                && r["json"]["text"]
                    .as_str()
                    .unwrap_or("")
                    .contains("Đã ghép nối agent-brain với chat này")
        }),
        "no confirmation sendMessage: {:?}",
        tg.calls("sendMessage")
    );
    let first = tg
        .calls("getUpdates")
        .into_iter()
        .next()
        .expect("a getUpdates call");
    assert_eq!(first["json"]["offset"].as_i64(), Some(-1), "{first}");
    assert!(!out.contains(FAKE_TOKEN), "token on stdout");
    assert!(!err.contains(FAKE_TOKEN), "token on stderr");
}

/// Runs `pty_secret.py` with the sandbox environment plus `env`, returning its JSON line.
fn run_pty(sb: &Sandbox, env: &[(String, String)], at_prompt: bool) -> serde_json::Value {
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let template = sb.command(&[], &env);
    let mut cmd = std::process::Command::new("python3");
    cmd.env_clear();
    for (k, v) in template.get_envs() {
        if let Some(v) = v {
            cmd.env(k, v);
        }
    }
    cmd.arg("-I")
        .arg(common::repo_root().join("tests/fakes/pty_secret.py"));
    if at_prompt {
        cmd.arg("--sigint-at-prompt");
    }
    cmd.arg(FAKE_TOKEN)
        .arg("--")
        .arg(common::bin())
        .arg("telegram")
        .arg("setup");
    cmd.stdin(Stdio::null());
    let out = cmd.output().expect("run pty_secret.py");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "pty_secret.py failed: {text}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_str(text.trim()).unwrap_or_else(|e| panic!("bad JSON {e}: {text}"))
}

#[test]
fn setup_prompt_hides_token_on_tty() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    let res = run_pty(&sb, &tg.env(), false);
    let output = res["output"].as_str().unwrap_or("");
    assert!(output.contains("Token bot"), "{res}");
    assert!(output.contains("Mở Telegram"), "{res}");
    assert!(!output.contains(FAKE_TOKEN), "token echoed: {res}");
    assert_eq!(res["exit"].as_i64(), Some(130), "{res}");
    assert_eq!(
        res["echo"].as_bool(),
        Some(true),
        "echo not restored: {res}"
    );
    assert!(!telegram_json_path(&sb).exists());
}

#[test]
fn setup_prompt_ctrl_c_restores_terminal() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    let res = run_pty(&sb, &tg.env(), true);
    let output = res["output"].as_str().unwrap_or("");
    assert!(output.contains("Token bot"), "{res}");
    assert_eq!(res["exit"].as_i64(), Some(130), "{res}");
    assert_eq!(res["echo"].as_bool(), Some(true), "echo left off: {res}");
    assert!(!telegram_json_path(&sb).exists());
    assert!(tg.calls("getMe").is_empty());
}

fn wrong_code_for(code: &str) -> &'static str {
    if code == "000000" {
        "111111"
    } else {
        "000000"
    }
}

#[test]
fn setup_ignores_group_and_counts_wrong_codes() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    let mut run = CliRun::spawn(&sb, &["telegram", "setup"], &tg.env(), &token_line());
    let code = run.wait_code();
    tg.push_update(tg.message_update(-100123, PAIRED_CHAT, "group", &code));
    tg.push_update(tg.message_update(PAIRED_CHAT, PAIRED_CHAT, "private", "/start"));
    tg.push_update(tg.message_update(6666, 6666, "private", wrong_code_for(&code)));
    tg.push_update(tg.message_update(PAIRED_CHAT, PAIRED_CHAT, "private", &code));
    let exit = run.wait_exit(Duration::from_secs(20));
    let (out, err) = (run.stdout(), run.stderr());
    assert_eq!(exit, 0, "stdout:\n{out}\nstderr:\n{err}");
    assert!(err.contains("Mã sai (1/5)"), "{err}");
    let v = read_json(&telegram_json_path(&sb));
    assert_eq!(v["chat_id"].as_i64(), Some(PAIRED_CHAT));
    // Nothing was sent to the group or to the stranger.
    for r in tg.calls("sendMessage") {
        assert_eq!(r["json"]["chat_id"].as_i64(), Some(PAIRED_CHAT), "{r}");
    }
}

#[test]
fn setup_too_many_wrong_codes() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    let mut run = CliRun::spawn(&sb, &["telegram", "setup"], &tg.env(), &token_line());
    let code = run.wait_code();
    for _ in 0..5 {
        tg.push_update(tg.message_update(
            PAIRED_CHAT,
            PAIRED_CHAT,
            "private",
            wrong_code_for(&code),
        ));
    }
    let exit = run.wait_exit(Duration::from_secs(20));
    let err = run.stderr();
    assert_eq!(exit, 1, "stdout:\n{}\nstderr:\n{err}", run.stdout());
    assert!(err.contains("Quá nhiều mã sai"), "{err}");
    assert!(!telegram_json_path(&sb).exists());
    assert!(tg.calls("sendMessage").is_empty());
}

#[test]
fn setup_rerun_replaces_pairing() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    tg::write_config(&sb, 1111, 0o600);
    let mut run = CliRun::spawn(&sb, &["telegram", "setup"], &tg.env(), &token_line());
    let code = run.wait_code();
    tg.push_update(tg.message_update(PAIRED_CHAT, PAIRED_CHAT, "private", &code));
    let exit = run.wait_exit(Duration::from_secs(20));
    assert_eq!(
        exit,
        0,
        "stdout:\n{}\nstderr:\n{}",
        run.stdout(),
        run.stderr()
    );
    let path = telegram_json_path(&sb);
    assert_eq!(read_json(&path)["chat_id"].as_i64(), Some(PAIRED_CHAT));
    assert_eq!(mode_of(&path), 0o600);
}

#[test]
fn setup_refused_while_serve_runs() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    let mut srv = tg.serve(&sb, &[]);
    let mut run = CliRun::spawn(&sb, &["telegram", "setup"], &tg.env(), &token_line());
    let exit = run.wait_exit(Duration::from_secs(20));
    let err = run.stderr();
    assert_eq!(exit, 1, "stdout:\n{}\nstderr:\n{err}", run.stdout());
    assert!(err.contains("agent-brain serve đang chạy"), "{err}");
    assert!(err.contains(&format!("(pid {})", srv.child.id())), "{err}");
    assert!(
        !err.contains("Token bot"),
        "prompted while serve runs: {err}"
    );
    assert!(!telegram_json_path(&sb).exists());
    assert!(tg.records().is_empty(), "{:?}", tg.records());
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn setup_bad_token() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    // An existing pairing survives every failed setup untouched.
    tg::write_config(&sb, 1111, 0o600);
    let path = telegram_json_path(&sb);
    let before = std::fs::read(&path).unwrap();

    let mut run = CliRun::spawn(&sb, &["telegram", "setup"], &tg.env(), "\n");
    assert_eq!(run.wait_exit(Duration::from_secs(20)), 1);
    assert!(run.stderr().contains("Chưa nhập token"), "{}", run.stderr());

    let mut run = CliRun::spawn(&sb, &["telegram", "setup"], &tg.env(), "abc\n");
    assert_eq!(run.wait_exit(Duration::from_secs(20)), 1);
    let err = run.stderr();
    assert!(err.contains("Token không đúng dạng"), "{err}");
    assert!(!err.contains("abc"), "{err}");
    assert!(tg.records().is_empty(), "bad shape reached Telegram");

    tg.set_behavior(serde_json::json!([{"method": "getMe", "times": 1, "status": 401}]));
    let mut run = CliRun::spawn(&sb, &["telegram", "setup"], &tg.env(), &token_line());
    assert_eq!(run.wait_exit(Duration::from_secs(20)), 1);
    let (out, err) = (run.stdout(), run.stderr());
    assert!(err.contains("Telegram từ chối token (401)"), "{err}");
    assert!(!err.contains(FAKE_TOKEN) && !out.contains(FAKE_TOKEN));
    assert!(!out.contains("Mở Telegram"), "{out}");

    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(mode_of(&path), 0o600);
}

#[test]
fn setup_missing_curl() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    let mut env = tg.env();
    env.push((
        "AGENT_BRAIN_CURL".to_string(),
        "/nonexistent/curl".to_string(),
    ));
    let mut run = CliRun::spawn(&sb, &["telegram", "setup"], &env, &token_line());
    assert_eq!(run.wait_exit(Duration::from_secs(20)), 1);
    let err = run.stderr();
    assert!(err.contains("Không tìm thấy lệnh curl"), "{err}");
    assert!(err.contains("agent-brain telegram setup"), "{err}");
    assert!(!telegram_json_path(&sb).exists());
}

#[test]
fn telegram_test_sends_message() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);

    let mut run = CliRun::spawn(&sb, &["telegram", "test"], &tg.env(), "");
    assert_eq!(run.wait_exit(Duration::from_secs(20)), 1);
    assert!(
        run.stderr().contains("Chưa ghép nối Telegram"),
        "{}",
        run.stderr()
    );

    tg::write_config(&sb, PAIRED_CHAT, 0o600);
    let mut run = CliRun::spawn(&sb, &["telegram", "test"], &tg.env(), "");
    let exit = run.wait_exit(Duration::from_secs(20));
    let (out, err) = (run.stdout(), run.stderr());
    assert_eq!(exit, 0, "stdout:\n{out}\nstderr:\n{err}");
    assert!(out.contains("Đã gửi tin thử tới chat 5555."), "{out}");
    let sent = tg.calls("sendMessage");
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0]["json"]["chat_id"].as_i64(), Some(PAIRED_CHAT));
    assert!(sent[0]["json"]["text"]
        .as_str()
        .unwrap_or("")
        .starts_with("Tin thử từ agent-brain trên máy"));
    assert!(!out.contains(FAKE_TOKEN) && !err.contains(FAKE_TOKEN));

    tg::write_config(&sb, PAIRED_CHAT, 0o644);
    let mut run = CliRun::spawn(&sb, &["telegram", "test"], &tg.env(), "");
    assert_eq!(run.wait_exit(Duration::from_secs(20)), 1);
    assert!(run.stderr().contains("chmod 600"), "{}", run.stderr());
    assert_eq!(tg.calls("sendMessage").len(), 1);
}

#[test]
fn setup_conflict_409() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    tg.set_behavior(serde_json::json!([{
        "method": "getUpdates", "times": 100, "status": 409,
        "description": "Conflict: can't use getUpdates method while webhook is active"
    }]));
    let mut run = CliRun::spawn(&sb, &["telegram", "setup"], &tg.env(), &token_line());
    let exit = run.wait_exit(Duration::from_secs(20));
    let (out, err) = (run.stdout(), run.stderr());
    assert_eq!(exit, 1, "stdout:\n{out}\nstderr:\n{err}");
    assert!(
        err.contains("Bot đang được đọc ở nơi khác (409) — tắt webhook hoặc tiến trình đang dùng bot rồi chạy lại"),
        "{err}"
    );
    assert!(!telegram_json_path(&sb).exists());
    assert!(!out.contains(FAKE_TOKEN) && !err.contains(FAKE_TOKEN));
}

/// TELE-02: the token only ever lands in telegram.json, never in config.json
/// or any other file of the sandbox config/data dirs.
#[test]
fn setup_never_writes_secret_to_config_json() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    let before = std::fs::read(&sb.config_path).expect("sandbox config.json");

    let mut run = CliRun::spawn(&sb, &["telegram", "setup"], &tg.env(), &token_line());
    let code = run.wait_code();
    tg.push_update(tg.message_update(PAIRED_CHAT, PAIRED_CHAT, "private", &code));
    assert_eq!(
        run.wait_exit(Duration::from_secs(20)),
        0,
        "{}",
        run.stderr()
    );

    // A serve start/stop with the paired config must not leak it either.
    let mut srv = tg.serve(&sb, &[]);
    assert!(wait_until(Duration::from_secs(10), || srv
        .stdout
        .lock()
        .unwrap()
        .contains("Telegram: đã kết nối")));
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));

    assert_eq!(
        std::fs::read(&sb.config_path).unwrap(),
        before,
        "config.json was modified"
    );
    let tg_json = telegram_json_path(&sb);
    assert!(tg_json.is_file());

    fn walk(dir: &std::path::Path, hits: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, hits);
            } else if std::fs::read(&p)
                .map(|b| String::from_utf8_lossy(&b).contains(FAKE_TOKEN))
                .unwrap_or(false)
            {
                hits.push(p);
            }
        }
    }
    let mut hits = Vec::new();
    walk(&sb.xdg_config, &mut hits);
    walk(&sb.xdg_data, &mut hits);
    walk(&sb.personal_dir, &mut hits);
    assert_eq!(hits, vec![tg_json], "token found outside telegram.json");
}
