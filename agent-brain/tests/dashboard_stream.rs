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
