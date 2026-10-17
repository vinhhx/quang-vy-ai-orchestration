mod common;

use common::Sandbox;
use std::time::Duration;

#[test]
fn d10_port_in_use_exits_1() {
    let sb = Sandbox::new();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let port = listener.local_addr().expect("local addr").port();

    let mut cmd = sb.command(&["serve", "--port", &port.to_string()], &[]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("spawn serve");

    let start = std::time::Instant::now();
    let mut exit_code = None;
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(Some(status)) = child.try_wait() {
            exit_code = status.code();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if exit_code.is_none() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("serve did not exit within 5s when port was in use");
    }
    assert_eq!(exit_code, Some(1));

    let mut stderr = String::new();
    if let Some(mut r) = child.stderr.take() {
        use std::io::Read;
        let _ = r.read_to_string(&mut stderr);
    }
    assert!(
        stderr.contains(&format!("Cổng {port} đang được dùng")),
        "stderr missing 'Cổng {port} đang được dùng', got:\n{stderr}"
    );
}

#[test]
fn d10_config_port_used_without_flag() {
    let default_val: serde_json::Value =
        serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap();
    let cfg = agent_brain::config::config_from_value(&default_val).unwrap();
    assert_eq!(cfg.dashboard_port, 7437);

    let mut bad_val = default_val.clone();
    bad_val["dashboard_port"] = serde_json::json!(70000);
    assert!(agent_brain::config::config_from_value(&bad_val).is_err());

    let free_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let q = free_listener.local_addr().unwrap().port();
    drop(free_listener);

    let sb = Sandbox::new();
    sb.write_config(serde_json::json!({ "dashboard_port": q }));

    let mut cmd = sb.command(&["serve"], &[]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("spawn serve with config port");
    let stdout = child.stdout.take().expect("take stdout");

    let stdout_buf = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let stdout_buf_clone = stdout_buf.clone();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        while let Ok(n) = reader.read_line(&mut line) {
            if n == 0 {
                break;
            }
            stdout_buf_clone.lock().unwrap().push_str(&line);
            line.clear();
        }
    });

    let expected_prefix = format!("http://127.0.0.1:{q}/");
    let found = common::wait_until(Duration::from_secs(10), || {
        stdout_buf.lock().unwrap().contains(&expected_prefix)
    });
    assert!(
        found,
        "serve did not print http://127.0.0.1:{q}/ within 10s"
    );

    unsafe {
        libc::kill(child.id() as i32, libc::SIGINT);
    }
    let _ = child.wait();
}

#[test]
fn d14_second_serve_refused() {
    let sb = Sandbox::new();
    let mut srv = sb.serve(&[]);
    let first_pid = srv.child.id();
    let first_port = srv.port;

    let mut cmd2 = sb.command(&["serve", "--port", "0"], &[]);
    cmd2.stdout(std::process::Stdio::piped());
    cmd2.stderr(std::process::Stdio::piped());
    let mut child2 = cmd2.spawn().expect("spawn second serve");

    let start = std::time::Instant::now();
    let mut exit_code2 = None;
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(Some(st)) = child2.try_wait() {
            exit_code2 = st.code();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if exit_code2.is_none() {
        let _ = child2.kill();
        let _ = child2.wait();
        panic!("second serve did not exit within 5s");
    }
    assert_eq!(exit_code2, Some(1));

    let mut out2 = String::new();
    let mut err2 = String::new();
    use std::io::Read;
    if let Some(mut r) = child2.stdout.take() {
        let _ = r.read_to_string(&mut out2);
    }
    if let Some(mut r) = child2.stderr.take() {
        let _ = r.read_to_string(&mut err2);
    }

    assert!(
        err2.contains(&format!("agent-brain serve đã chạy (pid {first_pid})")),
        "stderr should mention pid {first_pid}, got:\n{err2}"
    );
    assert!(
        err2.contains(&format!("http://127.0.0.1:{first_port}/")),
        "stderr should mention http://127.0.0.1:{first_port}/, got:\n{err2}"
    );

    assert!(
        !out2.contains(&srv.token),
        "second serve stdout should not contain token"
    );
    assert!(
        !err2.contains(&srv.token),
        "second serve stderr should not contain token"
    );

    let lock_path = sb.xdg_data.join("agent-brain").join("serve.lock");
    assert!(lock_path.is_file(), "serve.lock should exist");
    let lock_content = std::fs::read_to_string(&lock_path).expect("read serve.lock");
    assert!(
        !lock_content.contains(&srv.token),
        "serve.lock should not contain token"
    );
    let lock_val: serde_json::Value =
        serde_json::from_str(&lock_content).expect("parse serve.lock JSON");
    let lock_obj = lock_val
        .as_object()
        .expect("serve.lock must be JSON object");
    assert_eq!(lock_obj.len(), 3);
    assert_eq!(
        lock_obj.get("pid").and_then(|v| v.as_u64()),
        Some(first_pid as u64)
    );
    assert_eq!(
        lock_obj.get("port").and_then(|v| v.as_u64()),
        Some(first_port as u64)
    );
    assert!(lock_obj.contains_key("started_at"));

    let exit_code = srv.stop(libc::SIGINT, Duration::from_secs(6));
    assert_eq!(exit_code, Some(0));
}

fn test_shutdown_with_signal(sig: i32) {
    let sb = Sandbox::new();
    let repo = sb.make_repo("app", &[("src/main.rs", "fn main() {}")], None, None);
    let p_entry = sb.project_entry(&repo, "app", "work");
    sb.write_map(serde_json::json!([p_entry]), &[]);

    let mut conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = sb.read_map();
    let projs = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&mut conn, &projs).expect("sync_projects");
    let proj_id = agent_brain::db::project_row_id(&conn, "app").expect("project_row_id");
    let task_id = agent_brain::db::upsert_task(
        &conn,
        proj_id,
        "sua-app",
        "sửa app",
        "task/sua-app",
        Some("main"),
        None,
    )
    .expect("upsert_task");

    let pid = std::process::id();
    let pid_start = agent_brain::procs::proc_id(pid as i32)
        .map(|p| p.start)
        .unwrap_or(0);
    let new_run = agent_brain::db::NewRun {
        task_id: Some(task_id),
        project_id: proj_id,
        role: "lead".to_string(),
        profile: "work".to_string(),
        model: Some("opus".to_string()),
        pid: pid as i64,
        pgid: pid as i64,
        pid_start: pid_start as i64,
        base_commit: None,
        run_dir: None,
    };
    let run_id = agent_brain::db::insert_run(&conn, &new_run).expect("insert_run");
    let raw_event = serde_json::json!({
        "type": "system",
        "subtype": "init",
        "session_id": "session-1"
    })
    .to_string();
    agent_brain::db::insert_event(
        &conn,
        run_id,
        1,
        agent_brain::db::now_ms(),
        "system",
        &raw_event,
    )
    .expect("insert_event");

    let mut srv = sb.serve(&[]);
    let port = srv.port;
    let cookie = srv.cookie.clone();

    let stream_thread = std::thread::spawn(move || {
        use std::io::{Read, Write};
        let mut stream = match std::net::TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => s,
            Err(_) => return,
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let req = format!(
            "GET /api/runs/{run_id}/stream HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/event-stream\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n"
        );
        let _ = stream.write_all(req.as_bytes());
        let _ = stream.flush();
        let mut buf = [0u8; 1024];
        let mut total = 0;
        while total < 50 {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => total += n,
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(200));

    let exit_code = srv.stop(sig, Duration::from_secs(6));
    assert_eq!(exit_code, Some(0), "serve did not exit 0 on signal {sig}");

    let srv_stderr = std::fs::read_to_string(&srv.stderr_path).expect("read serve stderr");
    assert!(
        srv_stderr.contains("Đã dừng dashboard."),
        "serve stderr should contain 'Đã dừng dashboard.', got:\n{srv_stderr}"
    );

    let _ = stream_thread.join();
}

#[test]
fn shutdown_with_open_sse_is_prompt() {
    test_shutdown_with_signal(libc::SIGINT);
    test_shutdown_with_signal(libc::SIGTERM);
    test_shutdown_with_signal(libc::SIGHUP);
}
