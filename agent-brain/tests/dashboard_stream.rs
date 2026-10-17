mod common;
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use std::time::Duration;

#[test]
fn tracer_terminal_run_streams_live_through_gate() {
    let sb = Sandbox::new();
    protocol::setup_project(&sb, "work");

    let mut srv = sb.serve(&[]);

    // 1. Root asset assertions
    let r_root = srv.get("/");
    assert_eq!(r_root.status, 200);
    let root_ct = r_root.header("content-type").expect("content-type header");
    assert!(
        root_ct.starts_with("text/html"),
        "root content-type should be text/html, got {root_ct}"
    );
    let root_text = r_root.text();
    assert!(
        root_text.contains("/vendor/alpine.csp.min.js"),
        "root html missing alpine script"
    );
    assert!(root_text.contains("/app.css"), "root html missing app.css");

    // 2. CSS asset assertions
    let r_css = srv.get("/app.css");
    assert_eq!(r_css.status, 200);
    let css_ct = r_css.header("content-type").expect("css content-type");
    assert!(
        css_ct.starts_with("text/css"),
        "css content-type should be text/css, got {css_ct}"
    );
    assert!(
        r_css.text().starts_with("/*! tailwindcss v4.3.3"),
        "app.css does not start with Tailwind v4.3.3 banner"
    );

    // 3. Vendor JS asset assertions
    let r_js = srv.get("/vendor/alpine.csp.min.js");
    assert_eq!(r_js.status, 200);

    // 4. Prepare scenario with "sleep": 2
    let mut scenario = protocol::lead_scenario(&[("src/app.py", "app()", "trả về 2", "yêu cầu")]);
    scenario["sleep"] = serde_json::json!(2);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&scenario).unwrap())
        .expect("write scenario");

    // 5. Spawn terminal run: agent-brain run lead -- "web bán hàng" "sửa app"
    let run_stderr_path = sb.root.join("terminal_run.stderr.txt");
    let run_stderr_file = std::fs::File::create(&run_stderr_path).expect("create run stderr file");
    let mut cmd = sb.command(&["run", "lead", "--", "web bán hàng", "sửa app"], &[]);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::from(run_stderr_file));
    let mut child = cmd.spawn().expect("spawn terminal run lead");

    // 6. Wait <= 10 s until runs row 1 exists with status running
    let start_wait = std::time::Instant::now();
    let mut run_ready = false;
    while start_wait.elapsed() < Duration::from_secs(10) {
        if let Ok(conn) = rusqlite::Connection::open(&sb.db_path) {
            let res: rusqlite::Result<String> =
                conn.query_row("SELECT status FROM runs WHERE id = 1", [], |r| r.get(0));
            if let Ok(st) = res {
                if st == "running" {
                    run_ready = true;
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(run_ready, "Timeout waiting for run 1 to be 'running'");

    // 7. Assert events count for run 1 is 0
    let conn = rusqlite::Connection::open(&sb.db_path).expect("open db");
    let ev_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM events WHERE run_id = 1", [], |r| {
            r.get(0)
        })
        .expect("count events");
    assert_eq!(
        ev_count, 0,
        "events count should be 0 while fake claude is sleeping"
    );

    // 8. Stream SSE until "event: end" (up to 30 s)
    let sse_text = srv.sse(
        "/api/runs/1/stream",
        None,
        "event: end",
        Duration::from_secs(30),
    );

    // 9. Assert SSE content
    assert!(
        sse_text.contains("event: status"),
        "SSE stream missing event: status"
    );
    assert!(sse_text.contains("id: 1"), "SSE stream missing id: 1");
    assert!(
        sse_text.contains("init · session fake-session · model fake"),
        "SSE stream missing init line text"
    );
    assert!(
        sse_text.contains("\"ev\":\"result\""),
        "SSE stream missing ev:result"
    );
    assert!(
        sse_text.contains("event: end"),
        "SSE stream missing event: end"
    );
    assert!(
        sse_text.contains("\"status\":\"succeeded\""),
        "SSE stream missing status:succeeded"
    );

    // Number of `id: ` lines equals the events count of run 1 in the DB
    let id_lines = sse_text.lines().filter(|l| l.starts_with("id: ")).count();
    let db_events_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM events WHERE run_id = 1", [], |r| {
            r.get(0)
        })
        .expect("count db events");
    assert_eq!(
        id_lines as i64, db_events_count,
        "SSE id: lines ({id_lines}) must match DB events count ({db_events_count})"
    );

    // 10. The terminal run exits 0
    let child_status = child.wait().expect("wait terminal run child");
    assert_eq!(
        child_status.code(),
        Some(0),
        "terminal run exited with {:?}",
        child_status.code()
    );

    // 11. No cookie request to SSE endpoint is 401
    let r_no_auth = srv.request("GET", "/api/runs/1/stream", &[], b"");
    assert_eq!(
        r_no_auth.status, 401,
        "unauthenticated SSE request should be 401"
    );

    // 12. Stop server with SIGINT within 10 s
    let exit_code = srv.stop(libc::SIGINT, Duration::from_secs(10));
    assert_eq!(exit_code, Some(0), "serve stop exit code should be 0");

    let srv_stderr = std::fs::read_to_string(&srv.stderr_path).expect("read serve stderr");
    assert!(
        srv_stderr.contains("Đã dừng dashboard."),
        "serve stderr should contain 'Đã dừng dashboard.', got:\n{srv_stderr}"
    );
}

#[test]
fn dash05_resume_with_last_event_id() {
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

    for seq in 1..=6 {
        let raw = serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [{ "type": "text", "text": format!("step {seq}") }]
            }
        })
        .to_string();
        agent_brain::db::insert_event(
            &conn,
            run_id,
            seq,
            agent_brain::db::now_ms(),
            "assistant",
            &raw,
        )
        .expect("insert_event");
    }
    agent_brain::db::mark_status(&conn, run_id, "succeeded").expect("mark_status");

    let mut srv = sb.serve(&[]);
    let sse_text = srv.sse(
        &format!("/api/runs/{run_id}/stream"),
        Some("3"),
        "event: end",
        Duration::from_secs(10),
    );

    assert!(sse_text.contains("id: 4"), "missing id: 4");
    assert!(sse_text.contains("id: 5"), "missing id: 5");
    assert!(sse_text.contains("id: 6"), "missing id: 6");
    assert!(sse_text.contains("event: end"), "missing event: end");

    let id_lines: Vec<i64> = sse_text
        .lines()
        .filter_map(|l| {
            if l.starts_with("id: ") {
                l["id: ".len()..].parse::<i64>().ok()
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        id_lines,
        vec![4, 5, 6],
        "only ids > 3 should be received when resuming with Last-Event-ID: 3"
    );

    let exit_code = srv.stop(libc::SIGINT, Duration::from_secs(5));
    assert_eq!(exit_code, Some(0));
}

#[test]
fn d12_external_writer_rows_arrive_live() {
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
    drop(conn);

    let mut srv = sb.serve(&[]);
    let db_path = sb.db_path.clone();

    let writer_thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        let conn = agent_brain::db::open(&db_path).expect("open db in writer");
        for seq in 1..=3 {
            let raw = serde_json::json!({
                "type": "assistant",
                "message": {
                    "content": [{ "type": "text", "text": format!("external step {seq}") }]
                }
            })
            .to_string();
            agent_brain::db::insert_event(
                &conn,
                run_id,
                seq,
                agent_brain::db::now_ms(),
                "assistant",
                &raw,
            )
            .expect("insert event");
            std::thread::sleep(Duration::from_millis(400));
        }
        agent_brain::db::mark_status(&conn, run_id, "succeeded").expect("mark succeeded");
    });

    let sse_text = srv.sse(
        &format!("/api/runs/{run_id}/stream"),
        None,
        "event: end",
        Duration::from_secs(10),
    );

    writer_thread.join().expect("join writer thread");

    let pos1 = sse_text.find("id: 1").expect("id: 1 present");
    let pos2 = sse_text.find("id: 2").expect("id: 2 present");
    let pos3 = sse_text.find("id: 3").expect("id: 3 present");
    let pos_end = sse_text.find("event: end").expect("event: end present");

    assert!(pos1 < pos2, "id: 1 must precede id: 2");
    assert!(pos2 < pos3, "id: 2 must precede id: 3");
    assert!(pos3 < pos_end, "id: 3 must precede event: end");

    let exit_code = srv.stop(libc::SIGINT, Duration::from_secs(5));
    assert_eq!(exit_code, Some(0));
}

#[test]
fn sse_unknown_run_is_404() {
    let sb = Sandbox::new();
    let mut srv = sb.serve(&[]);

    let resp = srv.get("/api/runs/999/stream");
    assert_eq!(resp.status, 404);
    let json = resp.json();
    assert!(
        json.get("error").is_some(),
        "missing error key in 404 response: {:?}",
        json
    );

    let exit_code = srv.stop(libc::SIGINT, Duration::from_secs(5));
    assert_eq!(exit_code, Some(0));
}
