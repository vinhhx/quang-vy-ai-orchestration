mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{tree_procs, wait_until, ProcGuard, Sandbox};
use serde_json::json;
use std::time::Duration;

#[test]
fn d08_ctrl_c_on_serve_stops_its_runs() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# test\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(json!([entry]), &[]);
    sb.write_config(json!({
        "claude_bin": Sandbox::fake_claude_tree().to_str().unwrap(),
        "kill_grace_secs": 1,
    }));

    let pidfile = sb.root.join("tree.pid");
    let mut srv = sb.serve(&[
        ("FAKE_TREE_PIDFILE", pidfile.to_str().unwrap()),
        ("FAKE_TREE_MODE", "coop"),
        ("FAKE_TREE_SLEEP", "317"),
    ]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "chạy sleep",
            "name": "sua-app"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();

    let job_info = srv.get(&format!("/api/jobs/{job_id}")).json();
    let job_pid = job_info["pid"].as_i64().unwrap() as i32;
    let job_proc_id = agent_brain::procs::proc_id(job_pid).expect("job proc_id");

    let procs = tree_procs(&pidfile, Duration::from_secs(10)).expect("tree_procs");
    let [fake_id, bash_id, sleep_id] = procs;

    let _guard = ProcGuard(vec![job_proc_id, fake_id, bash_id, sleep_id]);

    let exit_code = srv.stop(libc::SIGINT, Duration::from_secs(12));
    assert_eq!(exit_code, Some(0));

    let ok = wait_until(Duration::from_secs(5), || {
        let conn = sb.db();
        let st: Result<String, _> =
            conn.query_row("SELECT status FROM runs WHERE id = 1", [], |r| r.get(0));
        st.as_deref() == Ok("killed")
    });
    assert!(ok, "run was not marked killed");

    std::thread::sleep(Duration::from_secs(1));
    assert!(!agent_brain::procs::alive(job_proc_id));
    assert!(!agent_brain::procs::alive(fake_id));
    assert!(!agent_brain::procs::alive(bash_id));
    assert!(!agent_brain::procs::alive(sleep_id));
}

#[test]
fn d08_sighup_on_serve_stops_its_runs() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# test\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(json!([entry]), &[]);
    sb.write_config(json!({
        "claude_bin": Sandbox::fake_claude_tree().to_str().unwrap(),
        "kill_grace_secs": 1,
    }));

    let pidfile = sb.root.join("tree.pid");
    let mut srv = sb.serve(&[
        ("FAKE_TREE_PIDFILE", pidfile.to_str().unwrap()),
        ("FAKE_TREE_MODE", "coop"),
        ("FAKE_TREE_SLEEP", "317"),
    ]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "chạy sleep sighup",
            "name": "sua-app-hup"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();

    let job_info = srv.get(&format!("/api/jobs/{job_id}")).json();
    let job_pid = job_info["pid"].as_i64().unwrap() as i32;
    let job_proc_id = agent_brain::procs::proc_id(job_pid).expect("job proc_id");

    let procs = tree_procs(&pidfile, Duration::from_secs(10)).expect("tree_procs");
    let [fake_id, bash_id, sleep_id] = procs;

    let _guard = ProcGuard(vec![job_proc_id, fake_id, bash_id, sleep_id]);

    let exit_code = srv.stop(libc::SIGHUP, Duration::from_secs(12));
    assert_eq!(exit_code, Some(0));

    let ok = wait_until(Duration::from_secs(5), || {
        let conn = sb.db();
        let st: Result<String, _> =
            conn.query_row("SELECT status FROM runs WHERE id = 1", [], |r| r.get(0));
        st.as_deref() == Ok("killed")
    });
    assert!(ok, "run was not marked killed");

    std::thread::sleep(Duration::from_secs(1));
    assert!(!agent_brain::procs::alive(job_proc_id));
    assert!(!agent_brain::procs::alive(fake_id));
    assert!(!agent_brain::procs::alive(bash_id));
    assert!(!agent_brain::procs::alive(sleep_id));
}

#[test]
fn d08_server_crash_stops_its_runs() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# test\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(json!([entry]), &[]);
    sb.write_config(json!({
        "claude_bin": Sandbox::fake_claude_tree().to_str().unwrap(),
        "kill_grace_secs": 1,
    }));

    let pidfile = sb.root.join("tree.pid");
    let mut srv = sb.serve(&[
        ("FAKE_TREE_PIDFILE", pidfile.to_str().unwrap()),
        ("FAKE_TREE_MODE", "coop"),
        ("FAKE_TREE_SLEEP", "317"),
    ]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "chạy sleep crash",
            "name": "sua-app-crash"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();

    let job_info = srv.get(&format!("/api/jobs/{job_id}")).json();
    let job_pid = job_info["pid"].as_i64().unwrap() as i32;
    let job_proc_id = agent_brain::procs::proc_id(job_pid).expect("job proc_id");

    let procs = tree_procs(&pidfile, Duration::from_secs(10)).expect("tree_procs");
    let [fake_id, bash_id, sleep_id] = procs;

    let _guard = ProcGuard(vec![job_proc_id, fake_id, bash_id, sleep_id]);

    let _ = srv.stop(libc::SIGKILL, Duration::from_secs(5));

    let ok = wait_until(Duration::from_secs(10), || {
        let conn = sb.db();
        let st: Result<String, _> =
            conn.query_row("SELECT status FROM runs WHERE id = 1", [], |r| r.get(0));
        st.as_deref() == Ok("killed")
    });
    assert!(ok, "run row was not marked killed within 10s");

    assert!(!agent_brain::procs::alive(job_proc_id));
    assert!(!agent_brain::procs::alive(fake_id));
    assert!(!agent_brain::procs::alive(bash_id));
    assert!(!agent_brain::procs::alive(sleep_id));
}

#[test]
fn dash06_kill_dashboard_started_run() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# test\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(json!([entry]), &[]);
    sb.write_config(json!({
        "claude_bin": Sandbox::fake_claude_tree().to_str().unwrap(),
        "kill_grace_secs": 1,
    }));

    let pidfile = sb.root.join("dash_tree.pid");
    let srv = sb.serve(&[
        ("FAKE_TREE_PIDFILE", pidfile.to_str().unwrap()),
        ("FAKE_TREE_MODE", "coop"),
        ("FAKE_TREE_SLEEP", "317"),
    ]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "chạy sleep kill dash",
            "name": "sua-app-kill-dash"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();

    let job_info = srv.get(&format!("/api/jobs/{job_id}")).json();
    let job_pid = job_info["pid"].as_i64().unwrap() as i32;
    let job_proc_id = agent_brain::procs::proc_id(job_pid).expect("job proc_id");

    let procs = tree_procs(&pidfile, Duration::from_secs(10)).expect("tree_procs");
    let [fake_id, bash_id, sleep_id] = procs;

    let _guard = ProcGuard(vec![job_proc_id, fake_id, bash_id, sleep_id]);

    let kill_resp = srv.post_json("/api/runs/1/kill", &json!({}));
    assert_eq!(kill_resp.status, 200);
    assert_eq!(kill_resp.json()["ok"], true);

    std::thread::sleep(Duration::from_secs(5));
    assert!(!agent_brain::procs::alive(fake_id));
    assert!(!agent_brain::procs::alive(bash_id));
    assert!(!agent_brain::procs::alive(sleep_id));

    let st: String = sb
        .db()
        .query_row("SELECT status FROM runs WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(st, "killed");

    let ok = wait_until(Duration::from_secs(5), || {
        let j = srv.get(&format!("/api/jobs/{job_id}")).json();
        j["state"] == "exited" && j["exit_code"] == 130
    });
    assert!(ok, "job did not exit with 130");

    let root_resp = srv.get("/");
    assert_eq!(root_resp.status, 200);
}

#[test]
fn dash06_kill_terminal_started_run() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# test\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(json!([entry]), &[]);
    sb.write_config(json!({
        "claude_bin": Sandbox::fake_claude_tree().to_str().unwrap(),
        "kill_grace_secs": 1,
    }));

    let srv = sb.serve(&[]);

    let pidfile = sb.root.join("term_tree.pid");

    let mut child = sb
        .command(
            &[
                "run",
                "lead",
                "web-ban-hang",
                "test terminal run",
                "--name",
                "sua-term",
            ],
            &[
                ("FAKE_TREE_PIDFILE", pidfile.to_str().unwrap()),
                ("FAKE_TREE_MODE", "coop"),
                ("FAKE_TREE_SLEEP", "317"),
            ],
        )
        .spawn()
        .expect("spawn terminal run");

    let term_pid = child.id() as i32;
    let term_proc_id = agent_brain::procs::proc_id(term_pid).expect("term proc_id");

    let procs = tree_procs(&pidfile, Duration::from_secs(10)).expect("tree_procs");
    let [fake_id, bash_id, sleep_id] = procs;
    let _guard = ProcGuard(vec![term_proc_id, fake_id, bash_id, sleep_id]);

    let ok = wait_until(Duration::from_secs(5), || {
        let conn = sb.db();
        let st: Result<String, _> =
            conn.query_row("SELECT status FROM runs WHERE id = 1", [], |r| r.get(0));
        st.as_deref() == Ok("running")
    });
    assert!(ok, "terminal run was not found running in DB");

    let kill_resp = srv.post_json("/api/runs/1/kill", &json!({}));
    assert_eq!(kill_resp.status, 200);
    assert_eq!(kill_resp.json()["ok"], true);

    let status = child.wait().expect("wait on child");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            status.code().or_else(|| status.signal().map(|s| 128 + s)),
            Some(130)
        );
    }

    std::thread::sleep(Duration::from_secs(1));
    assert!(!agent_brain::procs::alive(fake_id));
    assert!(!agent_brain::procs::alive(bash_id));
    assert!(!agent_brain::procs::alive(sleep_id));

    let st: String = sb
        .db()
        .query_row("SELECT status FROM runs WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(st, "killed");
}

#[test]
fn dash06_kill_escalates_when_sigterm_ignored() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# test\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(json!([entry]), &[]);
    sb.write_config(json!({
        "claude_bin": Sandbox::fake_claude_tree().to_str().unwrap(),
        "kill_grace_secs": 1,
    }));

    let pidfile = sb.root.join("ignore_tree.pid");
    let srv = sb.serve(&[
        ("FAKE_TREE_PIDFILE", pidfile.to_str().unwrap()),
        ("FAKE_TREE_MODE", "ignore"),
        ("FAKE_TREE_SLEEP", "317"),
    ]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "chạy sleep ignore",
            "name": "sua-app-ignore"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();

    let job_info = srv.get(&format!("/api/jobs/{job_id}")).json();
    let job_pid = job_info["pid"].as_i64().unwrap() as i32;
    let job_proc_id = agent_brain::procs::proc_id(job_pid).expect("job proc_id");

    let procs = tree_procs(&pidfile, Duration::from_secs(10)).expect("tree_procs");
    let [fake_id, bash_id, sleep_id] = procs;
    let _guard = ProcGuard(vec![job_proc_id, fake_id, bash_id, sleep_id]);

    let kill_resp = srv.post_json("/api/runs/1/kill", &json!({}));
    assert_eq!(kill_resp.status, 200);

    std::thread::sleep(Duration::from_secs(5));
    assert!(!agent_brain::procs::alive(fake_id));
    assert!(!agent_brain::procs::alive(bash_id));
    assert!(!agent_brain::procs::alive(sleep_id));
}

#[test]
fn dash06_kill_refusals() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[("README.md", "# test\n")], None, None);
    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(json!([entry]), &[]);
    let srv = sb.serve(&[]);

    let r_unknown = srv.post_json("/api/runs/999/kill", &json!({}));
    assert_eq!(r_unknown.status, 404);

    let conn = sb.db();
    let p_id = agent_brain::db::project_row_id(&conn, "web-ban-hang").unwrap();
    let t_id =
        agent_brain::db::upsert_task(&conn, p_id, "t1", "T1", "task/t1", None, None).unwrap();
    let r_id = agent_brain::db::insert_run(
        &conn,
        &agent_brain::db::NewRun {
            task_id: Some(t_id),
            project_id: p_id,
            role: "lead".to_string(),
            profile: "work".to_string(),
            model: None,
            pid: 1234,
            pgid: 1234,
            pid_start: 1234,
            base_commit: None,
            run_dir: None,
        },
    )
    .unwrap();
    agent_brain::db::mark_status(&conn, r_id, "succeeded").unwrap();

    let r_finished = srv.post_json(&format!("/api/runs/{r_id}/kill"), &json!({}));
    assert_eq!(r_finished.status, 409);
    assert!(r_finished.json()["error"]
        .as_str()
        .unwrap()
        .contains("không còn chạy"));

    let r_notask = agent_brain::db::insert_run(
        &conn,
        &agent_brain::db::NewRun {
            task_id: None,
            project_id: p_id,
            role: "inspect".to_string(),
            profile: "work".to_string(),
            model: None,
            pid: 1234,
            pgid: 1234,
            pid_start: 1234,
            base_commit: None,
            run_dir: None,
        },
    )
    .unwrap();

    let r_synth = srv.post_json(&format!("/api/runs/{r_notask}/kill"), &json!({}));
    assert_eq!(r_synth.status, 409);
    assert!(r_synth.json()["error"]
        .as_str()
        .unwrap()
        .contains("không thuộc task nào"));
}

#[test]
fn raw_event_and_run_summary() {
    let sb = Sandbox::new();
    let _repo = protocol::setup_project(&sb, "work");

    let sc_lead = protocol::lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_lead).unwrap()).unwrap();

    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();

    let ok = wait_until(Duration::from_secs(30), || {
        let r = srv.get(&format!("/api/jobs/{job_id}"));
        r.status == 200 && r.json()["state"] == "exited"
    });
    assert!(ok);

    let r_run = srv.get("/api/runs/1");
    assert_eq!(r_run.status, 200);
    let j_run = r_run.json();
    assert_eq!(j_run["role"], "lead");
    assert_eq!(j_run["status"], "succeeded");

    let r_raw = srv.get("/api/runs/1/events/1/raw");
    assert_eq!(r_raw.status, 200);
    let j_raw = r_raw.json();
    assert_eq!(j_raw["run_id"], 1);
    assert_eq!(j_raw["seq"], 1);
    assert_eq!(j_raw["ev"], "system/init");
    let raw_val: serde_json::Value =
        serde_json::from_str(j_raw["raw_json"].as_str().unwrap()).unwrap();
    assert_eq!(raw_val["type"], "system");

    let r_404 = srv.get("/api/runs/1/events/999/raw");
    assert_eq!(r_404.status, 404);
}
