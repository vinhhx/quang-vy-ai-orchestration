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
