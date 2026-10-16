mod common;

use common::Sandbox;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

struct TestTreeGuard {
    procs: Vec<agent_brain::procs::ProcId>,
}

impl Drop for TestTreeGuard {
    fn drop(&mut self) {
        for p in &self.procs {
            if agent_brain::procs::alive(*p) {
                agent_brain::procs::signal_proc(*p, libc::SIGKILL);
            }
        }
    }
}

fn start_tree(
    mode: &str,
    grace_secs: u64,
    timeout_lead: Option<u64>,
) -> (
    Sandbox,
    std::process::Child,
    i64,
    [agent_brain::procs::ProcId; 3],
    TestTreeGuard,
) {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("README.md", "# web-ban-hang\n")],
        None,
        None,
    );
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    let mut cfg_overrides = serde_json::json!({
        "claude_bin": fake_tree_bin().to_str().unwrap(),
        "kill_grace_secs": grace_secs,
    });
    if let Some(t) = timeout_lead {
        cfg_overrides["timeout_seconds"] = serde_json::json!({
            "lead": t,
            "dev": 30,
            "qa": 30,
            "inspect": 30
        });
    }
    sb.write_config(cfg_overrides);

    let pidfile = sb.root.join("tree.pid");
    let mut cmd = sb.command(
        &["run", "lead", "web-ban-hang", "chạy sleep"],
        &[
            ("FAKE_TREE_PIDFILE", pidfile.to_str().unwrap()),
            ("FAKE_TREE_MODE", mode),
            ("FAKE_TREE_SLEEP", "317"),
        ],
    );
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let child = cmd.spawn().expect("spawn run lead");

    // Wait <= 10 s for pidfile
    let start_wait = Instant::now();
    let mut fake_pid = None;
    let mut bash_pid = None;
    while start_wait.elapsed() < Duration::from_secs(10) {
        if pidfile.is_file() {
            if let Ok(c) = std::fs::read_to_string(&pidfile) {
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
    let fake_pid = fake_pid.expect("timeout waiting for fake tree pidfile");
    let bash_pid = bash_pid.expect("timeout waiting for fake tree bash pid");

    // Find sleep child (ppid == bash_pid) <= 3 s
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
    let sleep_pid = sleep_pid.expect("timeout finding sleep child");

    let fake_id = agent_brain::procs::proc_id(fake_pid).expect("fake proc_id");
    let bash_id = agent_brain::procs::proc_id(bash_pid).expect("bash proc_id");
    let sleep_id = agent_brain::procs::proc_id(sleep_pid).expect("sleep proc_id");

    // Read task id from SQLite
    let mut task_id = None;
    let start_db = Instant::now();
    while start_db.elapsed() < Duration::from_secs(3) {
        if let Ok(conn) = rusqlite::Connection::open(&sb.db_path) {
            let res: rusqlite::Result<i64> =
                conn.query_row("SELECT id FROM tasks ORDER BY id DESC LIMIT 1", [], |r| {
                    r.get(0)
                });
            if let Ok(id) = res {
                task_id = Some(id);
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let task_id = task_id.expect("timeout reading task_id from db");

    let guard = TestTreeGuard {
        procs: vec![fake_id, bash_id, sleep_id],
    };

    (sb, child, task_id, [fake_id, bash_id, sleep_id], guard)
}

#[test]
fn p2_02_kill_command_coop_no_survivors() {
    let (sb, mut run_child, task_id, procs, _guard) = start_tree("coop", 1, None);

    let kill_start = Instant::now();
    let kill_out = sb.run(&["kill", &task_id.to_string()], None);
    assert_eq!(kill_out.code, 0, "kill command failed: {}", kill_out.stderr);

    // All recorded processes dead within 5 s
    while kill_start.elapsed() < Duration::from_secs(5) {
        if !procs.iter().any(|p| agent_brain::procs::alive(*p)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    for p in &procs {
        assert!(
            !agent_brain::procs::alive(*p),
            "process {:?} survived kill",
            p
        );
    }

    // Run process exits 130 within 5 s
    let child_exit_start = Instant::now();
    let mut exit_code = None;
    while child_exit_start.elapsed() < Duration::from_secs(5) {
        if let Ok(Some(st)) = run_child.try_wait() {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                exit_code = st.code().or_else(|| st.signal().map(|s| 128 + s));
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(exit_code, Some(130), "run process did not exit with 130");

    // Check DB row
    let conn = sb.db();
    let row: (String, String, i64, Option<i64>) = conn
        .query_row(
            "SELECT status, usage_source, input_tokens, kill_requested_at FROM runs WHERE task_id = ?1",
            rusqlite::params![task_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("query run row");

    assert_eq!(row.0, "killed");
    assert_eq!(row.1, "result");
    assert_eq!(row.2, 3);
    assert!(row.3.is_some(), "kill_requested_at should be set");
}

#[test]
fn p2_02_kill_command_ignore_sigterm_default_grace() {
    let (sb, mut run_child, task_id, procs, _guard) = start_tree("ignore", 3, None);

    let kill_start = Instant::now();
    let kill_out = sb.run(&["kill", &task_id.to_string()], None);
    assert_eq!(kill_out.code, 0, "kill command failed: {}", kill_out.stderr);
    assert!(
        kill_start.elapsed() < Duration::from_secs(5),
        "kill command exceeded 5s"
    );

    // All recorded processes dead
    while kill_start.elapsed() < Duration::from_secs(5) {
        if !procs.iter().any(|p| agent_brain::procs::alive(*p)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    for p in &procs {
        assert!(
            !agent_brain::procs::alive(*p),
            "process {:?} survived kill",
            p
        );
    }

    let _ = run_child.wait();

    let conn = sb.db();
    let row: (String, String, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT status, usage_source, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens FROM runs WHERE task_id = ?1",
            rusqlite::params![task_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .expect("query run row");

    assert_eq!(row.0, "killed");
    assert_eq!(row.1, "partial");
    assert_eq!(row.2, 2);
    assert_eq!(row.3, 8);
    assert_eq!(row.4, 100);
    assert_eq!(row.5, 50);
}

#[test]
fn p2_03_sigint_once_graceful() {
    let (sb, mut run_child, task_id, procs, _guard) = start_tree("coop", 1, None);

    let supervisor_pid = run_child.id() as i32;
    unsafe {
        libc::kill(supervisor_pid, libc::SIGINT);
    }

    let wait_start = Instant::now();
    let mut exit_code = None;
    while wait_start.elapsed() < Duration::from_secs(3) {
        if let Ok(Some(st)) = run_child.try_wait() {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                exit_code = st.code().or_else(|| st.signal().map(|s| 128 + s));
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        exit_code,
        Some(130),
        "supervisor did not exit 130 on SIGINT"
    );

    for p in &procs {
        assert!(!agent_brain::procs::alive(*p), "process {:?} survived", p);
    }

    let conn = sb.db();
    let status: String = conn
        .query_row(
            "SELECT status FROM runs WHERE task_id = ?1",
            rusqlite::params![task_id],
            |r| r.get(0),
        )
        .expect("query status");
    assert_eq!(status, "killed");
}

#[test]
fn p2_03_sigint_twice_forces_sigkill() {
    let (sb, mut run_child, task_id, procs, _guard) = start_tree("ignore", 10, None);

    let supervisor_pid = run_child.id() as i32;
    let t0 = Instant::now();
    unsafe {
        libc::kill(supervisor_pid, libc::SIGINT);
    }
    std::thread::sleep(Duration::from_millis(300));
    unsafe {
        libc::kill(supervisor_pid, libc::SIGINT);
    }

    while t0.elapsed() < Duration::from_secs(2) {
        if !procs.iter().any(|p| agent_brain::procs::alive(*p)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        t0.elapsed() < Duration::from_secs(2),
        "processes not dead within 2s of first SIGINT: elapsed {:?}",
        t0.elapsed()
    );
    for p in &procs {
        assert!(
            !agent_brain::procs::alive(*p),
            "process {:?} survived double SIGINT",
            p
        );
    }

    let exit_status = run_child.wait().expect("wait run_child");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        let code = exit_status
            .code()
            .or_else(|| exit_status.signal().map(|s| 128 + s));
        assert_eq!(code, Some(130));
    }

    let conn = sb.db();
    let status: String = conn
        .query_row(
            "SELECT status FROM runs WHERE task_id = ?1",
            rusqlite::params![task_id],
            |r| r.get(0),
        )
        .expect("query status");
    assert_eq!(status, "killed");
}

#[test]
fn p2_03_sighup_kills_tree() {
    let (sb, mut run_child, task_id, procs, _guard) = start_tree("ignore", 1, None);

    let supervisor_pid = run_child.id() as i32;
    unsafe {
        libc::kill(supervisor_pid, libc::SIGHUP);
    }

    let wait_start = Instant::now();
    while wait_start.elapsed() < Duration::from_secs(4) {
        if !procs.iter().any(|p| agent_brain::procs::alive(*p)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    for p in &procs {
        assert!(
            !agent_brain::procs::alive(*p),
            "process {:?} survived SIGHUP",
            p
        );
    }

    let exit_status = run_child.wait().expect("wait run_child");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        let code = exit_status
            .code()
            .or_else(|| exit_status.signal().map(|s| 128 + s));
        assert_eq!(code, Some(130));
    }

    let conn = sb.db();
    let status: String = conn
        .query_row(
            "SELECT status FROM runs WHERE task_id = ?1",
            rusqlite::params![task_id],
            |r| r.get(0),
        )
        .expect("query status");
    assert_eq!(status, "killed");
}

#[test]
fn p2_03_sigterm_to_supervisor_kills_tree() {
    let (sb, mut run_child, task_id, procs, _guard) = start_tree("ignore", 1, None);

    let supervisor_pid = run_child.id() as i32;
    unsafe {
        libc::kill(supervisor_pid, libc::SIGTERM);
    }

    let wait_start = Instant::now();
    while wait_start.elapsed() < Duration::from_secs(4) {
        if !procs.iter().any(|p| agent_brain::procs::alive(*p)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    for p in &procs {
        assert!(
            !agent_brain::procs::alive(*p),
            "process {:?} survived SIGTERM",
            p
        );
    }

    let exit_status = run_child.wait().expect("wait run_child");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        let code = exit_status
            .code()
            .or_else(|| exit_status.signal().map(|s| 128 + s));
        assert_eq!(code, Some(130));
    }

    let conn = sb.db();
    let status: String = conn
        .query_row(
            "SELECT status FROM runs WHERE task_id = ?1",
            rusqlite::params![task_id],
            |r| r.get(0),
        )
        .expect("query status");
    assert_eq!(status, "killed");
}

#[test]
fn timeout_kills_tree() {
    let (sb, mut run_child, task_id, procs, _guard) = start_tree("ignore", 1, Some(1));

    let wait_start = Instant::now();
    let mut exit_code = None;
    while wait_start.elapsed() < Duration::from_secs(6) {
        if let Ok(Some(st)) = run_child.try_wait() {
            exit_code = st.code();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(exit_code, Some(1), "timeout run should exit 1");

    for p in &procs {
        assert!(
            !agent_brain::procs::alive(*p),
            "process {:?} survived timeout",
            p
        );
    }

    let conn = sb.db();
    let status: String = conn
        .query_row(
            "SELECT status FROM runs WHERE task_id = ?1",
            rusqlite::params![task_id],
            |r| r.get(0),
        )
        .expect("query status");
    assert_eq!(status, "timed_out");
}

#[test]
fn kill_unknown_or_idle_task() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# demo\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    // 1. kill unknown task
    let out = sb.run(&["kill", "999"], None);
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("Không tìm thấy task 999"),
        "stderr should say 'Không tìm thấy task 999', got: {}",
        out.stderr
    );

    // 2. finished run with fake_claude
    let run_out = sb.run(&["run", "lead", "web-ban-hang", "tạo task"], None);
    assert_eq!(run_out.code, 0, "run lead failed: {}", run_out.stderr);

    let kill_idle = sb.run(&["kill", "1"], None);
    assert_eq!(kill_idle.code, 1);
    assert!(
        kill_idle.stderr.contains("không có run đang chạy"),
        "stderr should contain 'không có run đang chạy', got: {}",
        kill_idle.stderr
    );
}

#[test]
fn reconcile_marks_dead_run_lost() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# demo\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    // Spawn and reap a short-lived process to obtain a dead pid
    let mut dead_proc = std::process::Command::new("true").spawn().unwrap();
    let dead_pid = dead_proc.id() as i32;
    let _ = dead_proc.wait();

    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let now = agent_brain::db::now_ms();
    conn.execute(
        "INSERT INTO projects (id, slug, path) VALUES (1, 'web-ban-hang', ?1)",
        rusqlite::params![repo.to_str().unwrap()],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, status, created_at, updated_at)
         VALUES (1, 1, 'task-1', 'task 1', 'task/task-1', 'open', ?1, ?1)",
        rusqlite::params![now],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, pid, pgid, pid_start, status, started_at)
         VALUES (1, 1, 1, 'lead', 'claude', 'work', ?1, ?1, 100, 'running', ?2)",
        rusqlite::params![dead_pid as i64, now],
    )
    .unwrap();

    let out = sb.run(&["kill", "1"], None);
    assert_eq!(out.code, 1);

    let status: String = conn
        .query_row("SELECT status FROM runs WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "lost");
}

#[test]
fn kill_never_signals_reused_pid() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# demo\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    // Spawn decoy
    let mut decoy = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let decoy_pid = decoy.id() as i32;
    let decoy_stat = agent_brain::procs::read_stat(decoy_pid).expect("read decoy stat");

    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let now = agent_brain::db::now_ms();
    conn.execute(
        "INSERT INTO projects (id, slug, path) VALUES (1, 'web-ban-hang', ?1)",
        rusqlite::params![repo.to_str().unwrap()],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, status, created_at, updated_at)
         VALUES (1, 1, 'task-1', 'task 1', 'task/task-1', 'open', ?1, ?1)",
        rusqlite::params![now],
    )
    .unwrap();
    // Insert with wrong start time (pid_start + 1)
    conn.execute(
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, pid, pgid, pid_start, status, started_at)
         VALUES (1, 1, 1, 'lead', 'claude', 'work', ?1, ?1, ?2, 'running', ?3)",
        rusqlite::params![decoy_pid as i64, (decoy_stat.start + 1) as i64, now],
    )
    .unwrap();

    let out = sb.run(&["kill", "1"], None);
    assert_eq!(out.code, 1);

    // Decoy is still running!
    assert!(
        decoy.try_wait().unwrap().is_none(),
        "decoy process was terminated!"
    );

    let status: String = conn
        .query_row("SELECT status FROM runs WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "lost");

    // Clean up decoy safely
    let _ = decoy.kill();
    let _ = decoy.wait();
}

#[test]
fn fast_run_drain_breaks_immediately() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# demo\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    let start = Instant::now();
    let out = sb.run(&["run", "lead", "web-ban-hang", "test nhanh"], None);
    let elapsed = start.elapsed();

    assert_eq!(out.code, 0, "run lead failed: {}", out.stderr);
    assert!(
        elapsed < Duration::from_millis(1500),
        "fast run took {:?}, expected well under 2s (drain loop did not break immediately)",
        elapsed
    );
}

#[test]
fn grandchild_holding_stdout_drain_deadline() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# demo\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    // Create a fake claude that exits quickly but leaves a grandchild holding stdout open
    let fake_holder = sb.root.join("fake_holder.sh");
    std::fs::write(
        &fake_holder,
        r#"#!/bin/bash
echo '{"type":"system","subtype":"init","session_id":"holder-session","model":"fake"}'
(sleep 5) &
echo '{"type":"result","subtype":"success","is_error":false,"num_turns":1,"session_id":"holder-session","total_cost_usd":0,"usage":{"input_tokens":1,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0},"permission_denials":[]}'
exit 0
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake_holder, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    sb.write_config(serde_json::json!({
        "claude_bin": fake_holder.to_str().unwrap()
    }));

    let start = Instant::now();
    let out = sb.run(&["run", "lead", "web-ban-hang", "test holder"], None);
    let elapsed = start.elapsed();

    assert_eq!(out.code, 0, "run lead failed: {}", out.stderr);
    // Drain deadline is 2s, so total time should be between 2.0s and 3.5s (not 5s)
    assert!(
        elapsed >= Duration::from_millis(1900),
        "expected drain deadline to wait around 2s, took {:?}",
        elapsed
    );
    assert!(
        elapsed < Duration::from_millis(3500),
        "drain loop took {:?}, expected to end within 2s deadline",
        elapsed
    );
}
