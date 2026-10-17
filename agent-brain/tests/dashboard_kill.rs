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
