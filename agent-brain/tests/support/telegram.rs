//! FakeTelegram test driver (Phase 05.3). Every Telegram test starts its own
//! fake on an ephemeral 127.0.0.1 port inside its own sandbox; `env()` always
//! points serve at it, so no test ever contacts api.telegram.org (D-17).

use crate::common::{repo_root, wait_until, Sandbox, Serve};
use serde_json::{json, Value};
use std::io::BufRead;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

pub const FAKE_TOKEN: &str = "4242:fake-token-for-tests";
pub const PAIRED_CHAT: i64 = 5555;
pub const STRANGER_CHAT: i64 = 7777;
pub const BOT_USERNAME: &str = "ab_fake_bot";

pub struct FakeTelegram {
    pub child: std::process::Child,
    pub port: u16,
    pub dir: PathBuf,
    seq: AtomicI64,
}

impl FakeTelegram {
    pub fn start(sb: &Sandbox) -> FakeTelegram {
        let dir = sb.root.join("fake_telegram");
        std::fs::create_dir_all(&dir).expect("create fake telegram dir");
        let script = repo_root().join("tests/fakes/fake_telegram.py");
        let mut child = std::process::Command::new("python3")
            .arg("-I")
            .arg(&script)
            .arg(&dir)
            .env("FAKE_TELEGRAM_STDIN_WATCH", "1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("spawn fake_telegram.py");
        let stdout = child.stdout.take().expect("fake telegram stdout");
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .expect("read fake telegram port line");
        let port: u16 = match line.trim().parse() {
            Ok(p) => p,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("fake telegram printed no port: {line:?}");
            }
        };
        FakeTelegram {
            child,
            port,
            dir,
            seq: AtomicI64::new(1),
        }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn env(&self) -> Vec<(String, String)> {
        vec![
            ("AGENT_BRAIN_TELEGRAM_API".to_string(), self.url()),
            (
                "AGENT_BRAIN_TELEGRAM_BACKOFF_MS".to_string(),
                "200".to_string(),
            ),
        ]
    }

    /// Starts serve pointed at this fake; `extra` is applied after `env()` and wins.
    pub fn serve(&self, sb: &Sandbox, extra: &[(&str, &str)]) -> Serve {
        let base = self.env();
        let mut all: Vec<(&str, &str)> =
            base.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        all.extend_from_slice(extra);
        sb.serve(&all)
    }

    pub fn records(&self) -> Vec<Value> {
        let text = std::fs::read_to_string(self.dir.join("record.jsonl")).unwrap_or_default();
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    pub fn calls(&self, method: &str) -> Vec<Value> {
        self.records()
            .into_iter()
            .filter(|r| r.get("method").and_then(|m| m.as_str()) == Some(method))
            .collect()
    }

    pub fn wait_call(
        &self,
        method: &str,
        timeout: Duration,
        pred: impl Fn(&Value) -> bool,
    ) -> Option<Value> {
        let mut found = None;
        wait_until(timeout, || {
            found = self.calls(method).into_iter().find(|r| pred(r));
            found.is_some()
        });
        found
    }

    /// Appends one update; sets `update_id` (1 + the largest so far) when absent.
    pub fn push_update(&self, mut update: Value) -> i64 {
        let path = self.dir.join("updates.jsonl");
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let max_id = text
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter_map(|v| v.get("update_id").and_then(|u| u.as_i64()))
            .max()
            .unwrap_or(0);
        let id = match update.get("update_id").and_then(|u| u.as_i64()) {
            Some(id) => id,
            None => {
                let id = max_id + 1;
                update["update_id"] = json!(id);
                id
            }
        };
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .expect("open updates.jsonl");
        writeln!(f, "{}", serde_json::to_string(&update).unwrap()).expect("append update");
        f.flush().expect("flush updates.jsonl");
        id
    }

    fn next_seq(&self) -> i64 {
        self.seq.fetch_add(1, Ordering::SeqCst)
    }

    pub fn message_update(&self, chat_id: i64, from_id: i64, chat_type: &str, text: &str) -> Value {
        let n = self.next_seq();
        json!({
            "message": {
                "message_id": 1000 + n,
                "date": now_secs(),
                "chat": {"id": chat_id, "type": chat_type},
                "from": {"id": from_id, "is_bot": false, "first_name": "Tester"},
                "text": text
            }
        })
    }

    pub fn callback_update(
        &self,
        chat_id: i64,
        from_id: i64,
        message_id: i64,
        message_text: &str,
        data: &str,
    ) -> Value {
        let n = self.next_seq();
        json!({
            "callback_query": {
                "id": format!("cb-{n}"),
                "from": {"id": from_id, "is_bot": false, "first_name": "Tester"},
                "message": {
                    "message_id": message_id,
                    "date": now_secs(),
                    "chat": {"id": chat_id, "type": "private"},
                    "text": message_text
                },
                "chat_instance": "ci-test",
                "data": data
            }
        })
    }

    /// Replaces `behavior.json` atomically.
    pub fn set_behavior(&self, rules: Value) {
        let path = self.dir.join("behavior.json");
        let tmp = self.dir.join(format!("behavior.{}.tmp", self.next_seq()));
        std::fs::write(&tmp, serde_json::to_string(&rules).unwrap()).expect("write behavior tmp");
        std::fs::rename(&tmp, &path).expect("rename behavior.json");
    }
}

impl Drop for FakeTelegram {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn telegram_json_path(sb: &Sandbox) -> PathBuf {
    sb.xdg_config.join("agent-brain").join("telegram.json")
}

pub fn write_config(sb: &Sandbox, chat_id: i64, mode: u32) {
    let body = json!({"token": FAKE_TOKEN, "chat_id": chat_id, "paired_at": 1});
    write_raw_config(sb, &serde_json::to_string_pretty(&body).unwrap(), mode);
}

/// Writes arbitrary text as telegram.json with `mode`.
pub fn write_raw_config(sb: &Sandbox, text: &str, mode: u32) {
    let path = telegram_json_path(sb);
    let parent: &Path = path.parent().expect("telegram.json parent");
    std::fs::create_dir_all(parent).expect("create agent-brain config dir");
    std::fs::write(&path, text).expect("write telegram.json");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
        .expect("chmod telegram.json");
}
