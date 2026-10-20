#![allow(dead_code)]

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("canonicalize repo root")
}

pub fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_agent-brain"))
}

#[derive(Debug, Clone)]
pub struct RunOut {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub struct Sandbox {
    pub dir: tempfile::TempDir,
    pub root: PathBuf,
    pub home: PathBuf,
    pub xdg_config: PathBuf,
    pub xdg_data: PathBuf,
    pub db_path: PathBuf,
    pub config_path: PathBuf,
    pub runs: PathBuf,
    pub personal_dir: PathBuf,
    pub repos: PathBuf,
    pub record_file: PathBuf,
    pub scenario_file: PathBuf,
    pub git_config: PathBuf,
}

impl Sandbox {
    pub fn new() -> Sandbox {
        let dir = tempfile::tempdir().expect("create tempdir");
        let root = dir.path().canonicalize().expect("canonicalize root");
        let home = root.join("home");
        let xdg_config = root.join("xdg_config");
        let xdg_data = root.join("xdg_data");
        let runs = root.join("runs");
        let personal_dir = root.join("personal_cfg");
        let repos = root.join("repos");

        for d in [&home, &xdg_config, &xdg_data, &runs, &personal_dir, &repos] {
            fs::create_dir_all(d).expect("create dir");
        }

        let db_path = xdg_data.join("agent-brain").join("brain.db");
        let config_path = root.join("brain_config.json");
        let record_file = root.join("fake_claude_record.jsonl");
        let scenario_file = root.join("fake_claude_scenario.json");
        let git_config = root.join("temp_gitconfig");

        fs::write(
            &git_config,
            "[user]\n  name = Sandbox User\n  email = sandbox@example.com\n[commit]\n  gpgsign = false\n[init]\n  defaultBranch = main\n",
        )
        .expect("write gitconfig");

        let fake = Self::fake_claude();
        if let Ok(metadata) = fs::metadata(&fake) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&fake, perms);
        }

        let sb = Sandbox {
            dir,
            root,
            home,
            xdg_config,
            xdg_data,
            db_path,
            config_path,
            runs,
            personal_dir,
            repos,
            record_file,
            scenario_file,
            git_config,
        };

        sb.write_config(serde_json::json!({}));
        sb
    }

    pub fn write_config(&self, overrides: serde_json::Value) {
        let mut cfg = serde_json::json!({
            "claude_bin": Self::fake_claude().to_str().unwrap(),
            "models": {
                "lead": "opus",
                "dev": "sonnet",
                "qa": "sonnet",
                "plan-review": "opus",
                "code-review": "opus"
            },
            "inspect_model": "sonnet",
            "profiles": {
                "work": {
                    "claude_config_dir": null,
                    "unset_env": [
                        "ANTHROPIC_API_KEY",
                        "ANTHROPIC_AUTH_TOKEN",
                        "ANTHROPIC_BASE_URL"
                    ]
                },
                "personal": {
                    "claude_config_dir": self.personal_dir.to_str().unwrap(),
                    "unset_env": [
                        "ANTHROPIC_API_KEY",
                        "ANTHROPIC_AUTH_TOKEN",
                        "ANTHROPIC_BASE_URL"
                    ]
                }
            },
            "common_args": ["--strict-mcp-config"],
            "max_turns": {
                "lead": null,
                "dev": null,
                "qa": null,
                "plan-review": null,
                "code-review": null
            },
            "timeout_seconds": {
                "lead": 30,
                "dev": 30,
                "qa": 30,
                "inspect": 30,
                "plan-review": 30,
                "code-review": 30
            },
            "runs_dir": self.runs.to_str().unwrap(),
            "kill_grace_secs": 1,
            "inspect_max_diff_bytes": 100000
        });

        if let Some(obj) = overrides.as_object() {
            let cfg_obj = cfg.as_object_mut().unwrap();
            for (k, v) in obj {
                cfg_obj.insert(k.clone(), v.clone());
            }
        }

        fs::write(
            &self.config_path,
            serde_json::to_string_pretty(&cfg).unwrap(),
        )
        .expect("write config");
    }

    pub fn fake_claude() -> PathBuf {
        repo_root()
            .join("tests")
            .join("fakes")
            .join("fake_claude.py")
            .canonicalize()
            .expect("canonicalize fake_claude.py")
    }

    pub fn make_repo(
        &self,
        name: &str,
        files: &[(&str, &str)],
        root: Option<&Path>,
        remote: Option<&str>,
    ) -> PathBuf {
        let target = root.unwrap_or(&self.repos).join(name);
        fs::create_dir_all(&target).expect("create repo dir");

        let mut init_cmd = std::process::Command::new("git");
        init_cmd.args(["init", "-b", "main"]);
        init_cmd.current_dir(&target);
        init_cmd.env_clear();
        if let Ok(p) = std::env::var("PATH") {
            init_cmd.env("PATH", p);
        }
        init_cmd.env("HOME", &self.home);
        init_cmd.env("GIT_CONFIG_GLOBAL", &self.git_config);
        init_cmd.env("GIT_CONFIG_NOSYSTEM", "1");
        let init_res = init_cmd.output().expect("git init");
        assert!(init_res.status.success(), "git init failed");

        if files.is_empty() {
            fs::write(target.join("README.md"), "# demo\n").expect("write README.md");
        } else {
            for (rel, content) in files {
                let fpath = target.join(rel);
                if let Some(p) = fpath.parent() {
                    fs::create_dir_all(p).expect("create parent dir");
                }
                fs::write(fpath, content).expect("write file");
            }
        }

        self.git(&target, &["add", "-A"]);
        self.git(&target, &["commit", "-m", "init"]);

        if let Some(r) = remote {
            self.git(&target, &["remote", "add", "origin", r]);
        }

        target
    }

    pub fn project_entry(&self, repo: &Path, id: &str, profile: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "path": repo.to_str().unwrap(),
            "name_vi": id,
            "aliases": [],
            "type": "code",
            "repo": serde_json::Value::Null,
            "profile": profile,
            "missing": false,
            // existing dashboard tests post /api/tasks without a workflow and must keep the manual flow; a test that needs the server default removes the key
            "workflow": "nhanh"
        })
    }

    pub fn workflows_path(&self) -> PathBuf {
        self.xdg_config.join("agent-brain").join("workflows.json")
    }

    pub fn write_workflows(&self, data: serde_json::Value) {
        let wf_path = self.workflows_path();
        if let Some(p) = wf_path.parent() {
            fs::create_dir_all(p).expect("create workflows dir");
        }
        fs::write(wf_path, serde_json::to_string_pretty(&data).unwrap()).expect("write workflows");
    }

    pub fn queue_state_path(&self) -> PathBuf {
        PathBuf::from(format!("{}.state.json", self.scenario_file.display()))
    }

    pub fn write_scenario(&self, sc: serde_json::Value) {
        fs::write(&self.scenario_file, serde_json::to_string(&sc).unwrap())
            .expect("write scenario");
        let _ = fs::remove_file(self.queue_state_path());
    }

    pub fn write_map(&self, projects: serde_json::Value, scan_roots: &[&str]) {
        let map_path = self.map_path();
        if let Some(p) = map_path.parent() {
            fs::create_dir_all(p).expect("create map dir");
        }
        let data = serde_json::json!({
            "scan_roots": scan_roots,
            "projects": projects
        });
        fs::write(map_path, serde_json::to_string_pretty(&data).unwrap()).expect("write map");
    }

    pub fn map_path(&self) -> PathBuf {
        self.xdg_config
            .join("agent-brain")
            .join("projects_map.json")
    }

    pub fn read_map(&self) -> serde_json::Value {
        let content = fs::read_to_string(self.map_path()).expect("read map");
        serde_json::from_str(&content).expect("parse map")
    }

    pub fn command(&self, args: &[&str], extra_env: &[(&str, &str)]) -> std::process::Command {
        let mut cmd = std::process::Command::new(bin());
        cmd.args(args);
        cmd.env_clear();
        if let Ok(p) = std::env::var("PATH") {
            cmd.env("PATH", p);
        }
        cmd.env("HOME", &self.home);
        cmd.env("XDG_CONFIG_HOME", &self.xdg_config);
        cmd.env("XDG_DATA_HOME", &self.xdg_data);
        cmd.env("AGENT_BRAIN_DB", &self.db_path);
        cmd.env("AGENT_BRAIN_CONFIG", &self.config_path);
        cmd.env("GIT_CONFIG_GLOBAL", &self.git_config);
        cmd.env("GIT_CONFIG_NOSYSTEM", "1");
        cmd.env("FAKE_CLAUDE_RECORD", &self.record_file);
        if self.scenario_file.is_file() {
            cmd.env("FAKE_CLAUDE_SCENARIO", &self.scenario_file);
        }
        cmd.env("CLAUDECODE", "1");
        cmd.env("CLAUDE_CODE_ENTRYPOINT", "cli");
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        cmd
    }

    pub fn run(&self, args: &[&str], scenario: Option<serde_json::Value>) -> RunOut {
        self.run_env(args, scenario, &[])
    }

    pub fn run_env(
        &self,
        args: &[&str],
        scenario: Option<serde_json::Value>,
        extra_env: &[(&str, &str)],
    ) -> RunOut {
        if let Some(sc) = scenario {
            fs::write(&self.scenario_file, serde_json::to_string(&sc).unwrap())
                .expect("write scenario");
            let _ = fs::remove_file(self.queue_state_path());
        } else {
            let _ = fs::remove_file(&self.scenario_file);
            let _ = fs::remove_file(self.queue_state_path());
        }

        let mut cmd = self.command(args, extra_env);
        let output = cmd.output().expect("execute agent-brain");
        RunOut {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
    }

    pub fn spawn_env(
        &self,
        args: &[&str],
        scenario: Option<serde_json::Value>,
        extra_env: &[(&str, &str)],
    ) -> std::process::Child {
        if let Some(sc) = scenario {
            fs::write(&self.scenario_file, serde_json::to_string(&sc).unwrap())
                .expect("write scenario");
            let _ = fs::remove_file(self.queue_state_path());
        } else {
            let _ = fs::remove_file(&self.scenario_file);
            let _ = fs::remove_file(self.queue_state_path());
        }

        self.command(args, extra_env)
            .spawn()
            .expect("spawn agent-brain")
    }

    pub fn records(&self) -> Vec<serde_json::Value> {
        if !self.record_file.is_file() {
            return Vec::new();
        }
        let content = fs::read_to_string(&self.record_file).expect("read record file");
        content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).expect("parse record json"))
            .collect()
    }

    pub fn git(&self, repo: &Path, args: &[&str]) -> String {
        let mut cmd = std::process::Command::new("git");
        cmd.args(args);
        cmd.current_dir(repo);
        cmd.env_clear();
        if let Ok(p) = std::env::var("PATH") {
            cmd.env("PATH", p);
        }
        cmd.env("HOME", &self.home);
        cmd.env("GIT_CONFIG_GLOBAL", &self.git_config);
        cmd.env("GIT_CONFIG_NOSYSTEM", "1");
        let out = cmd.output().expect("git output");
        if !out.status.success() {
            panic!(
                "git {:?} in {} failed: {}",
                args,
                repo.display(),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    pub fn db(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(&self.db_path).expect("open sqlite connection")
    }

    pub fn fake_claude_tree() -> PathBuf {
        repo_root()
            .join("tests")
            .join("fakes")
            .join("fake_claude_tree.py")
            .canonicalize()
            .expect("canonicalize fake_claude_tree.py")
    }

    pub fn serve(&self, extra_env: &[(&str, &str)]) -> Serve {
        let n = SERVE_ID.fetch_add(1, Ordering::SeqCst);
        let stderr_path = self.root.join(format!("serve-{n}.stderr.txt"));
        let stderr_file = fs::File::create(&stderr_path).expect("create serve stderr file");

        let mut all_env = extra_env.to_vec();
        let sc_str = self.scenario_file.to_str().unwrap();
        all_env.push(("FAKE_CLAUDE_SCENARIO", sc_str));

        let mut cmd = self.command(&["serve", "--port", "0"], &all_env);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::from(stderr_file));

        let mut child = cmd.spawn().expect("spawn agent-brain serve");
        let child_stdout = child.stdout.take().expect("take serve stdout");

        let stdout_buf = Arc::new(Mutex::new(String::new()));
        let stdout_buf_clone = stdout_buf.clone();
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(child_stdout);
            let mut line = String::new();
            use std::io::BufRead;
            while let Ok(n) = reader.read_line(&mut line) {
                if n == 0 {
                    break;
                }
                let mut guard = stdout_buf_clone.lock().unwrap();
                guard.push_str(&line);
                line.clear();
            }
        });

        let mut matched = None;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(15) {
            {
                let guard = stdout_buf.lock().unwrap();
                if let Some(pos) = guard.find("http://127.0.0.1:") {
                    let after = &guard[pos + "http://127.0.0.1:".len()..];
                    if let Some(slash_pos) = after.find("/?token=") {
                        let port_str = &after[..slash_pos];
                        let token_after = &after[slash_pos + "/?token=".len()..];
                        if token_after.len() >= 64 {
                            let token_candidate = &token_after[..64];
                            if let Ok(port) = port_str.parse::<u16>() {
                                if token_candidate.chars().all(|c| c.is_ascii_hexdigit()) {
                                    matched = Some((port, token_candidate.to_string()));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let (port, token) = match matched {
            Some(p) => p,
            None => {
                let err_content = fs::read_to_string(&stderr_path).unwrap_or_default();
                panic!("Timeout waiting for serve to print URL. Stderr:\n{err_content}");
            }
        };

        let handshake = http_raw(port, "GET", &format!("/?token={token}"), &[], b"");
        assert_eq!(
            handshake.status,
            303,
            "Handshake failed with status {}: {}",
            handshake.status,
            handshake.text()
        );

        let cookie = format!("agent_brain_token={token}");

        Serve {
            child,
            port,
            token,
            cookie,
            stdout: stdout_buf,
            stderr_path,
        }
    }
}

static SERVE_ID: AtomicUsize = AtomicUsize::new(0);

pub struct HttpResp {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResp {
    pub fn header(&self, name: &str) -> Option<&str> {
        let lower = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k == &lower)
            .map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("parse response body as JSON")
    }
}

pub fn http_raw(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> HttpResp {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .unwrap_or_else(|e| panic!("Failed to connect to 127.0.0.1:{port}: {e}"));
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("set read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .expect("set write timeout");

    let has_host = headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host"));
    let has_connection = headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("connection"));
    let has_content_len = headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-length"));

    let mut req_bytes = Vec::new();
    req_bytes.extend_from_slice(format!("{method} {path} HTTP/1.1\r\n").as_bytes());

    if !has_host {
        req_bytes.extend_from_slice(format!("host: 127.0.0.1:{port}\r\n").as_bytes());
    }
    if !has_connection {
        req_bytes.extend_from_slice(b"connection: close\r\n");
    }
    if !has_content_len && !body.is_empty() {
        req_bytes.extend_from_slice(format!("content-length: {}\r\n", body.len()).as_bytes());
    }
    for (k, v) in headers {
        req_bytes.extend_from_slice(format!("{k}: {v}\r\n").as_bytes());
    }
    req_bytes.extend_from_slice(b"\r\n");
    req_bytes.extend_from_slice(body);

    stream.write_all(&req_bytes).expect("write request");
    stream.flush().expect("flush request");

    let mut resp_bytes = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => resp_bytes.extend_from_slice(&buf[..n]),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                break
            }
            Err(e) => panic!("read response error: {e}"),
        }
    }

    parse_http_response(&resp_bytes)
}

fn parse_http_response(raw: &[u8]) -> HttpResp {
    let header_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or(raw.len());
    let header_bytes = &raw[..header_end];
    let body_bytes = if header_end + 4 <= raw.len() {
        &raw[header_end + 4..]
    } else {
        b""
    };

    let header_str = String::from_utf8_lossy(header_bytes);
    let mut lines = header_str.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    let mut headers = Vec::new();
    let mut is_chunked = false;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim().to_string();
            if key == "transfer-encoding" && val.to_ascii_lowercase().contains("chunked") {
                is_chunked = true;
            }
            headers.push((key, val));
        }
    }

    let final_body = if is_chunked {
        dechunk(body_bytes)
    } else {
        body_bytes.to_vec()
    };

    HttpResp {
        status,
        headers,
        body: final_body,
    }
}

fn dechunk(mut input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let pos = match input.windows(2).position(|w| w == b"\r\n") {
            Some(p) => p,
            None => break,
        };
        let len_str = String::from_utf8_lossy(&input[..pos]);
        let hex_part = len_str.split(';').next().unwrap_or("").trim();
        let chunk_len = match usize::from_str_radix(hex_part, 16) {
            Ok(n) => n,
            Err(_) => break,
        };
        if chunk_len == 0 {
            break;
        }
        let data_start = pos + 2;
        let data_end = data_start + chunk_len;
        if data_end > input.len() {
            out.extend_from_slice(&input[data_start..]);
            break;
        }
        out.extend_from_slice(&input[data_start..data_end]);
        input = if data_end + 2 <= input.len() {
            &input[data_end + 2..]
        } else {
            &input[data_end..]
        };
    }
    out
}

pub fn wait_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

pub fn tree_procs(pidfile: &Path, timeout: Duration) -> Option<[agent_brain::procs::ProcId; 3]> {
    let start_wait = Instant::now();
    let mut fake_pid = None;
    let mut bash_pid = None;
    while start_wait.elapsed() < timeout {
        if pidfile.is_file() {
            if let Ok(c) = std::fs::read_to_string(pidfile) {
                let parts: Vec<&str> = c.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let (Ok(f), Ok(b)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                        fake_pid = Some(f);
                        bash_pid = Some(b);
                        break;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let fake_pid = fake_pid?;
    let bash_pid = bash_pid?;

    let start_sleep = Instant::now();
    let mut sleep_pid = None;
    while start_sleep.elapsed() < Duration::from_secs(3) {
        let descs = agent_brain::procs::descendants(bash_pid);
        if let Some(d) = descs.first() {
            sleep_pid = Some(d.pid);
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let sleep_pid = sleep_pid?;

    let fake_id = agent_brain::procs::proc_id(fake_pid)?;
    let bash_id = agent_brain::procs::proc_id(bash_pid)?;
    let sleep_id = agent_brain::procs::proc_id(sleep_pid)?;

    Some([fake_id, bash_id, sleep_id])
}

pub struct ProcGuard(pub Vec<agent_brain::procs::ProcId>);

impl Drop for ProcGuard {
    fn drop(&mut self) {
        for p in &self.0 {
            if agent_brain::procs::alive(*p) {
                agent_brain::procs::signal_proc(*p, libc::SIGKILL);
            }
        }
    }
}

pub struct Serve {
    pub child: std::process::Child,
    pub port: u16,
    pub token: String,
    pub cookie: String,
    pub stdout: Arc<Mutex<String>>,
    pub stderr_path: PathBuf,
}

impl Serve {
    pub fn get(&self, path: &str) -> HttpResp {
        self.request("GET", path, &[("Cookie", &self.cookie)], b"")
    }

    pub fn post_json(&self, path: &str, body: &serde_json::Value) -> HttpResp {
        let bytes = serde_json::to_vec(body).expect("serialize post_json");
        let origin = format!("http://127.0.0.1:{}", self.port);
        self.request(
            "POST",
            path,
            &[
                ("Cookie", &self.cookie),
                ("Origin", &origin),
                ("Content-Type", "application/json"),
            ],
            &bytes,
        )
    }

    pub fn put_json(&self, path: &str, body: &serde_json::Value) -> HttpResp {
        let bytes = serde_json::to_vec(body).expect("serialize put_json");
        let origin = format!("http://127.0.0.1:{}", self.port);
        self.request(
            "PUT",
            path,
            &[
                ("Cookie", &self.cookie),
                ("Origin", &origin),
                ("Content-Type", "application/json"),
            ],
            &bytes,
        )
    }

    pub fn request(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> HttpResp {
        http_raw(self.port, method, path, headers, body)
    }

    pub fn sse(
        &self,
        path: &str,
        last_event_id: Option<&str>,
        until: &str,
        timeout: Duration,
    ) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port))
            .unwrap_or_else(|e| panic!("Failed to connect to 127.0.0.1:{}: {e}", self.port));
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .expect("set read timeout");

        let mut req_str = format!(
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAccept: text/event-stream\r\nCookie: {}\r\n",
            self.port, self.cookie
        );
        if let Some(lid) = last_event_id {
            req_str.push_str(&format!("Last-Event-ID: {lid}\r\n"));
        }
        req_str.push_str("Connection: close\r\n\r\n");

        stream
            .write_all(req_str.as_bytes())
            .expect("send sse request");
        stream.flush().expect("flush sse request");

        let deadline = Instant::now() + timeout;
        let mut raw_buf = Vec::new();
        let mut buf = [0u8; 1024];

        while Instant::now() < deadline {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    raw_buf.extend_from_slice(&buf[..n]);
                    if let Some(pos) = raw_buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header_part = String::from_utf8_lossy(&raw_buf[..pos]);
                        let status = header_part
                            .lines()
                            .next()
                            .and_then(|l| l.split_whitespace().nth(1))
                            .and_then(|s| s.parse::<u16>().ok())
                            .unwrap_or(0);
                        if status != 200 {
                            continue;
                        }
                        let body_part = &raw_buf[pos + 4..];
                        let decoded = String::from_utf8_lossy(&dechunk(body_part)).to_string();
                        if decoded.contains(until) {
                            return decoded;
                        }
                    }
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    if let Some(pos) = raw_buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let body_part = &raw_buf[pos + 4..];
                        let decoded = String::from_utf8_lossy(&dechunk(body_part)).to_string();
                        if decoded.contains(until) {
                            return decoded;
                        }
                    }
                }
                Err(e) => panic!("read sse error: {e}"),
            }
        }

        if let Some(pos) = raw_buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let header_part = String::from_utf8_lossy(&raw_buf[..pos]);
            let status = header_part
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(0);
            if status != 200 {
                return String::from_utf8_lossy(&raw_buf).to_string();
            }
            let body_part = &raw_buf[pos + 4..];
            String::from_utf8_lossy(&dechunk(body_part)).to_string()
        } else {
            String::from_utf8_lossy(&raw_buf).to_string()
        }
    }

    pub fn stop(&mut self, sig: i32, timeout: Duration) -> Option<i32> {
        unsafe {
            libc::kill(self.child.id() as i32, sig);
        }
        let start = Instant::now();
        while start.elapsed() < timeout {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        if let Some(code) = status.code() {
                            return Some(code);
                        } else if let Some(s) = status.signal() {
                            return Some(128 + s);
                        }
                    }
                    return status.code();
                }
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => return None,
            }
        }
        None
    }
}

impl Drop for Serve {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            unsafe {
                libc::kill(self.child.id() as i32, libc::SIGKILL);
            }
            let _ = self.child.wait();
        }
    }
}
