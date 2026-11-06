#![allow(dead_code)]

//! Test drivers cho FakeTrello và FakeGsheet.
//! Khởi chạy fake server trên cổng loopback 127.0.0.1 ngẫu nhiên bên trong sandbox.
//! Không bao giờ gọi mạng thật ra ngoài.

use serde_json::Value;
use std::io::BufRead;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manifest parent")
        .to_path_buf()
}

pub struct FakeTrello {
    pub child: std::process::Child,
    pub port: u16,
    pub dir: PathBuf,
}

impl FakeTrello {
    pub fn start(work_dir: &Path) -> Self {
        let dir = work_dir.join("fake_trello");
        std::fs::create_dir_all(&dir).expect("create fake trello dir");
        let script = repo_root().join("tests/fakes/fake_trello.py");
        let mut child = std::process::Command::new("python3")
            .arg("-I")
            .arg(&script)
            .arg(&dir)
            .env("FAKE_TRELLO_STDIN_WATCH", "1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("spawn fake_trello.py");

        let stdout = child.stdout.take().expect("fake trello stdout");
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read fake trello port line");
        let port: u16 = match line.trim().parse() {
            Ok(p) => p,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("fake trello printed invalid port: {line:?}");
            }
        };

        Self { child, port, dir }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn set_data(&self, data: Value) {
        let path = self.dir.join("data.json");
        let text = serde_json::to_string_pretty(&data).unwrap();
        std::fs::write(path, text).unwrap();
    }

    pub fn set_behavior(&self, rules: Value) {
        let path = self.dir.join("behavior.json");
        let text = serde_json::to_string(&rules).unwrap();
        std::fs::write(path, text).unwrap();
    }

    pub fn read_records(&self) -> Vec<Value> {
        let path = self.dir.join("record.jsonl");
        if !path.is_file() {
            return Vec::new();
        }
        let content = std::fs::read_to_string(path).unwrap_or_default();
        content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }
}

impl Drop for FakeTrello {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct FakeGsheet {
    pub child: std::process::Child,
    pub port: u16,
    pub dir: PathBuf,
}

impl FakeGsheet {
    pub fn start(work_dir: &Path) -> Self {
        let dir = work_dir.join("fake_gsheet");
        std::fs::create_dir_all(&dir).expect("create fake gsheet dir");
        let script = repo_root().join("tests/fakes/fake_gsheet.py");
        let mut child = std::process::Command::new("python3")
            .arg("-I")
            .arg(&script)
            .arg(&dir)
            .env("FAKE_GSHEET_STDIN_WATCH", "1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("spawn fake_gsheet.py");

        let stdout = child.stdout.take().expect("fake gsheet stdout");
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read fake gsheet port line");
        let port: u16 = match line.trim().parse() {
            Ok(p) => p,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("fake gsheet printed invalid port: {line:?}");
            }
        };

        Self { child, port, dir }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn set_csv(&self, csv: &str) {
        let path = self.dir.join("sheet.csv");
        std::fs::write(path, csv).unwrap();
    }

    pub fn set_behavior(&self, rules: Value) {
        let path = self.dir.join("behavior.json");
        let text = serde_json::to_string(&rules).unwrap();
        std::fs::write(path, text).unwrap();
    }

    pub fn read_records(&self) -> Vec<Value> {
        let path = self.dir.join("record.jsonl");
        if !path.is_file() {
            return Vec::new();
        }
        let content = std::fs::read_to_string(path).unwrap_or_default();
        content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }
}

impl Drop for FakeGsheet {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
