mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::procs::ProcId;
use common::Sandbox;
use protocol::lead_scenario;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

const KILL_SENTENCE: &str = "Autopilot của task 1 cũng dừng với lý do \"dừng bởi bạn\".";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .canonicalize()
        .expect("canonicalize repo root")
}

fn fake_tree_bin() -> PathBuf {
    repo_root()
        .join("tests")
        .join("fakes")
        .join("fake_claude_tree.py")
        .canonicalize()
        .expect("canonicalize fake_claude_tree.py")
}

/// Tiến trình do test sinh ra: Drop dừng đúng Child đã ghi và các tiến trình đã ghi
/// (pid + start time), không bao giờ tìm theo tên.
struct KillGuard {
    child: Option<std::process::Child>,
    procs: Vec<ProcId>,
}

impl KillGuard {
    fn new(child: std::process::Child) -> KillGuard {
        KillGuard {
            child: Some(child),
            procs: Vec::new(),
        }
    }

    fn pid(&self) -> i32 {
        self.child.as_ref().map(|c| c.id() as i32).unwrap_or(0)
    }

    fn record(&mut self, p: ProcId) {
        if !self.procs.contains(&p) {
            self.procs.push(p);
        }
    }

    /// Ghi lại các tiến trình con hiện có của driver (agent-brain run, claude giả, ...).
    fn record_descendants(&mut self) {
        let pid = self.pid();
        if pid > 0 {
            for p in agent_brain::procs::descendants(pid) {
                self.record(p);
            }
        }
    }

    fn wait_exit(&mut self, timeout: Duration) -> Option<i32> {
        use std::os::unix::process::ExitStatusExt;
        let child = self.child.as_mut()?;
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if let Ok(Some(st)) = child.try_wait() {
                return Some(st.code().unwrap_or_else(|| 128 + st.signal().unwrap_or(0)));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    fn alive_recorded(&self) -> Vec<ProcId> {
        self.procs
            .iter()
            .copied()
            .filter(|p| agent_brain::procs::alive(*p))
            .collect()
    }
}

impl Drop for KillGuard {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            if let Ok(None) = child.try_wait() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        for p in &self.procs {
            if agent_brain::procs::alive(*p) {
                agent_brain::procs::signal_proc(*p, libc::SIGKILL);
            }
        }
    }
}

fn assert_ok(res: &common::RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

/// Task 1 theo quy trình `nhanh` (entry mặc định của Sandbox), plan đã duyệt, chưa có run Dev.
fn approved_nhanh_task(sb: &Sandbox) {
    protocol::setup_project(sb, "work");
    let res = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
        ],
        Some(lead_scenario(&[(
            "src/app.py",
            "app()",
            "trả về 2",
            "demo",
        )])),
    );
    assert_ok(&res, "run lead");
    let res = sb.run(&["approve", "1", "--no-continue"], None);
    assert_ok(&res, "approve 1 --no-continue");
}

fn driver_stderr(sb: &Sandbox) -> String {
    std::fs::read_to_string(sb.root.join("driver.stderr.txt")).unwrap_or_default()
}

/// `agent-brain auto 1` với claude giả dạng cây (bash → sleep 317), stdio ra file.
fn spawn_tree_driver(sb: &Sandbox) -> (KillGuard, [ProcId; 3]) {
    sb.write_config(json!({
        "claude_bin": fake_tree_bin().to_str().unwrap(),
        "kill_grace_secs": 1,
    }));
    let pidfile = sb.root.join("tree.pid");
    let out = std::fs::File::create(sb.root.join("driver.stdout.txt")).unwrap();
    let err = std::fs::File::create(sb.root.join("driver.stderr.txt")).unwrap();
    let mut cmd = sb.command(
        &["auto", "1"],
        &[
            ("FAKE_TREE_PIDFILE", pidfile.to_str().unwrap()),
            ("FAKE_TREE_MODE", "coop"),
            ("FAKE_TREE_SLEEP", "317"),
        ],
    );
    cmd.stdout(std::process::Stdio::from(out));
    cmd.stderr(std::process::Stdio::from(err));
    let mut guard = KillGuard::new(cmd.spawn().expect("spawn auto 1"));
    if let Some(p) = agent_brain::procs::proc_id(guard.pid()) {
        guard.record(p);
    }

    let tree = common::tree_procs(&pidfile, Duration::from_secs(15));
    let tree = match tree {
        Some(t) => t,
        None => panic!("fake claude tree did not start: {}", driver_stderr(sb)),
    };
    for p in tree {
        guard.record(p);
    }
    guard.record_descendants();
    (guard, tree)
}

/// Chờ run `dev` đang chạy; trả id của run.
fn wait_running_dev(sb: &Sandbox) -> i64 {
    let mut found = None;
    let ok = common::wait_until(Duration::from_secs(15), || {
        let row: rusqlite::Result<i64> = sb.db().query_row(
            "SELECT id FROM runs WHERE status = 'running' AND role = 'dev'",
            [],
            |r| r.get(0),
        );
        match row {
            Ok(id) => {
                found = Some(id);
                true
            }
            Err(_) => false,
        }
    });
    assert!(ok, "no running dev run: {}", driver_stderr(sb));
    found.unwrap()
}

fn run_status(sb: &Sandbox, run_id: i64) -> String {
    sb.db()
        .query_row("SELECT status FROM runs WHERE id = ?1", [run_id], |r| {
            r.get(0)
        })
        .unwrap()
}

fn running_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE status = 'running'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

fn auto_row(sb: &Sandbox) -> agent_brain::db::TaskAuto {
    agent_brain::db::task_auto(&sb.db(), 1)
        .unwrap()
        .expect("task 1")
}

fn auto_json(sb: &Sandbox) -> Value {
    let aj = auto_row(sb).auto_json.unwrap_or_else(|| "{}".to_string());
    serde_json::from_str(&aj).unwrap()
}

#[test]
fn kill_mid_run_stops_driver_and_tree() {
    let sb = Sandbox::new();
    approved_nhanh_task(&sb);

    let (mut driver, tree) = spawn_tree_driver(&sb);
    let dev_run = wait_running_dev(&sb);
    driver.record_descendants();

    let kill = sb.run(&["kill", "1"], None);
    assert_eq!(kill.code, 0, "kill failed: {}", kill.stderr);
    assert!(
        kill.stderr.contains(KILL_SENTENCE),
        "missing autopilot sentence: {}",
        kill.stderr
    );

    let code = driver.wait_exit(Duration::from_secs(15));
    assert!(
        code.is_some(),
        "driver still running: {}",
        driver_stderr(&sb)
    );
    assert_eq!(code, Some(4), "stderr: {}", driver_stderr(&sb));

    let gone = common::wait_until(Duration::from_secs(15), || {
        tree.iter().all(|p| !agent_brain::procs::alive(*p))
    });
    assert!(gone, "tree survived: {:?}", driver.alive_recorded());
    let all_gone = common::wait_until(Duration::from_secs(5), || {
        driver.alive_recorded().is_empty()
    });
    assert!(all_gone, "alive: {:?}", driver.alive_recorded());

    assert_eq!(run_status(&sb, dev_run), "killed");
    let auto = auto_row(&sb);
    assert_eq!(auto.auto_state.as_deref(), Some("stopped"));
    let stop = auto_json(&sb)["stop"].clone();
    assert_eq!(stop["reason"], "dung_boi_ban", "auto_json: {stop}");
    assert_eq!(stop["run_id"], dev_run, "auto_json: {stop}");
    assert_eq!(running_count(&sb), 0);
}

/// Driver giả còn sống: một tiến trình python3 ngủ (không dùng `sleep N`), ghi
/// (pid, start) của nó vào auto_pid / auto_pid_start với auto_state `running`.
fn fake_live_driver(sb: &Sandbox) -> (KillGuard, ProcId) {
    let child = std::process::Command::new("python3")
        .args(["-c", "import time; time.sleep(60)"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn python3 sleeper");
    let mut guard = KillGuard::new(child);
    let p = agent_brain::procs::proc_id(guard.pid()).expect("sleeper proc_id");
    guard.record(p);
    sb.db()
        .execute(
            "UPDATE tasks SET auto_state = 'running', auto_pid = ?1, auto_pid_start = ?2 WHERE id = 1",
            rusqlite::params![p.pid as i64, p.start as i64],
        )
        .unwrap();
    (guard, p)
}

#[test]
fn kill_in_gap_sets_flag() {
    let sb = Sandbox::new();
    approved_nhanh_task(&sb);
    let (_guard, sleeper) = fake_live_driver(&sb);
    assert_eq!(running_count(&sb), 0);

    let kill = sb.run(&["kill", "1"], None);
    assert_eq!(kill.code, 0, "kill failed: {}", kill.stderr);
    assert!(
        kill.stderr.contains("Đã yêu cầu autopilot dừng (task 1)"),
        "stderr: {}",
        kill.stderr
    );
    assert!(
        kill.stderr
            .contains("không có run đang chạy; autopilot dừng trước bước tiếp theo."),
        "stderr: {}",
        kill.stderr
    );

    let auto = auto_row(&sb);
    assert_eq!(auto.auto_stop_kind.as_deref(), Some("kill"));
    assert!(auto.auto_stop_at.is_some(), "auto_stop_at not set");
    assert_eq!(auto.auto_state.as_deref(), Some("running"));
    // Kill không bao giờ gửi tín hiệu cho chính driver.
    assert!(
        agent_brain::procs::alive(sleeper),
        "kill signalled the driver process"
    );
}

#[test]
fn kill_without_driver_unchanged() {
    let sb = Sandbox::new();
    approved_nhanh_task(&sb);
    sb.db()
        .execute(
            "UPDATE tasks SET auto_state = 'running', auto_pid = 999999, auto_pid_start = 1 WHERE id = 1",
            [],
        )
        .unwrap();

    let kill = sb.run(&["kill", "1"], None);
    assert_eq!(kill.code, 1, "stderr: {}", kill.stderr);
    assert!(
        kill.stderr.contains("không có run đang chạy"),
        "stderr: {}",
        kill.stderr
    );
    assert!(
        !kill.stderr.contains("Đã yêu cầu autopilot dừng"),
        "stderr: {}",
        kill.stderr
    );
    let auto = auto_row(&sb);
    assert_eq!(auto.auto_stop_kind, None);
    assert_eq!(auto.auto_stop_at, None);
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

fn qa_entry(verdict: &str) -> Value {
    let report = format!(
        "# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- src/app.py: đã xem.\n## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông.\n## Kết luận\n{verdict}\n"
    );
    json!({"ops": [{"op": "write", "path": ".qa-report.md", "content": report}]})
}

#[test]
fn sigkilled_driver_leaves_nothing_running() {
    let sb = Sandbox::new();
    approved_nhanh_task(&sb);

    let (mut driver, tree) = spawn_tree_driver(&sb);
    let dev_run = wait_running_dev(&sb);
    driver.record_descendants();

    // SIGKILL chỉ cho pid driver đã ghi (Child do test sinh ra).
    let driver_id = agent_brain::procs::proc_id(driver.pid()).expect("driver proc_id");
    assert!(agent_brain::procs::signal_proc(driver_id, libc::SIGKILL));
    let code = driver.wait_exit(Duration::from_secs(15));
    assert_eq!(code, Some(128 + libc::SIGKILL), "driver did not die");

    let gone = common::wait_until(Duration::from_secs(15), || {
        tree.iter().all(|p| !agent_brain::procs::alive(*p))
    });
    assert!(gone, "tree survived: {:?}", driver.alive_recorded());
    let none_running = common::wait_until(Duration::from_secs(15), || running_count(&sb) == 0);
    assert!(
        none_running,
        "run {dev_run} still running: {}",
        run_status(&sb, dev_run)
    );
    let all_gone = common::wait_until(Duration::from_secs(5), || {
        driver.alive_recorded().is_empty()
    });
    assert!(all_gone, "alive: {:?}", driver.alive_recorded());

    // Hàng `running` cũ của driver đã chết được coi như không có driver.
    let auto = auto_row(&sb);
    assert_eq!(auto.auto_state.as_deref(), Some("running"));
    let kill = sb.run(&["kill", "1"], None);
    assert_eq!(kill.code, 1, "stderr: {}", kill.stderr);
    assert!(
        kill.stderr.contains("không có run đang chạy"),
        "stderr: {}",
        kill.stderr
    );
    assert!(
        !kill.stderr.contains("Đã yêu cầu autopilot dừng"),
        "stderr: {}",
        kill.stderr
    );
    assert_eq!(auto_row(&sb).auto_stop_kind, None);

    // `agent-brain auto 1` nhận lại task (Pitfall 11) và chạy tiếp tới xong.
    sb.write_config(json!({}));
    let sc = json!({"by_role": {"dev": [dev_impl_entry()], "qa": [qa_entry("Đạt")]}});
    let res = sb.run(&["auto", "1"], Some(sc));
    assert_ok(&res, "auto 1 after a SIGKILLed driver");
    assert_eq!(auto_row(&sb).auto_state.as_deref(), Some("done"));
    assert_eq!(running_count(&sb), 0);
}
