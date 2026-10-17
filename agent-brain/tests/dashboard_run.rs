mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{wait_until, Sandbox};
use protocol::{lead_scenario, setup_project};
use serde_json::json;
use std::time::Duration;

fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for b in input.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{:02X}", b));
        }
    }
    encoded
}

#[test]
fn dash04_assign_lead_from_api() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let sc = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc).unwrap()).unwrap();

    let mut srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    assert_eq!(
        resp.status,
        202,
        "POST /api/tasks response: {}",
        resp.text()
    );
    let res_json = resp.json();
    let job_id = res_json["job_id"].as_u64().expect("job_id in 202 response");
    assert_eq!(res_json["project"], "web-ban-hang");
    assert_eq!(res_json["slug"], "sua-app");
    assert_eq!(res_json["branch"], "task/sua-app");

    let mut job_view = None;
    let ok = wait_until(Duration::from_secs(30), || {
        let r = srv.get(&format!("/api/jobs/{job_id}"));
        if r.status == 200 {
            let j = r.json();
            if j["state"] == "exited" {
                job_view = Some(j);
                return true;
            }
        }
        false
    });
    assert!(ok, "job did not exit in 30s");
    let job = job_view.unwrap();
    assert_eq!(job["exit_code"], 0);
    assert_eq!(job["task_id"], 1);
    assert_eq!(job["run_id"], 1);

    let conn = sb.db();
    let task_status: String = conn
        .query_row("SELECT status FROM tasks WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(task_status, "planned");

    let (role, profile, status): (String, String, String) = conn
        .query_row(
            "SELECT role, profile, status FROM runs WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(role, "lead");
    assert_eq!(profile, "work");
    assert_eq!(status, "succeeded");

    let other_runs: i64 = conn
        .query_row("SELECT COUNT(*) FROM runs WHERE role != 'lead'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(other_runs, 0);

    let head_msg = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_msg.trim(), "sua-app: plan");

    let sse_out = srv.sse(
        "/api/runs/1/stream",
        None,
        "event: end",
        Duration::from_secs(10),
    );
    assert!(
        sse_out.contains("event: end"),
        "SSE did not contain event: end: {sse_out}"
    );
    assert!(
        sse_out.contains("succeeded"),
        "SSE did not contain succeeded: {sse_out}"
    );

    let record_content = std::fs::read_to_string(&sb.record_file).unwrap();
    let record_line = record_content.lines().next().unwrap();
    let record: serde_json::Value = serde_json::from_str(record_line).unwrap();
    let argv = record["argv"].as_array().unwrap();
    let p_idx = argv.iter().position(|x| x == "-p").expect("-p in argv");
    let prompt = argv[p_idx + 1].as_str().unwrap();
    assert!(
        prompt.contains("Yêu cầu: sửa app"),
        "Prompt did not contain 'Yêu cầu: sửa app': {prompt}"
    );

    srv.stop(libc::SIGINT, Duration::from_secs(5));
}

#[test]
fn dash04_request_starting_with_dash_is_passed_verbatim() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let sc = lead_scenario(&[("src/app.py", "1", "help", "help")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc).unwrap()).unwrap();

    let mut srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "--help me",
            "name": "help-me"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();

    let ok = wait_until(Duration::from_secs(30), || {
        let r = srv.get(&format!("/api/jobs/{job_id}"));
        r.status == 200 && r.json()["state"] == "exited"
    });
    assert!(ok);

    let r = srv.get(&format!("/api/jobs/{job_id}"));
    assert_eq!(r.json()["exit_code"], 0);

    let record_content = std::fs::read_to_string(&sb.record_file).unwrap();
    let record_line = record_content.lines().next().unwrap();
    let record: serde_json::Value = serde_json::from_str(record_line).unwrap();
    let argv = record["argv"].as_array().unwrap();
    let p_idx = argv.iter().position(|x| x == "-p").expect("-p in argv");
    let prompt = argv[p_idx + 1].as_str().unwrap();
    assert!(
        prompt.contains("Yêu cầu: --help me"),
        "Prompt did not contain 'Yêu cầu: --help me': {prompt}"
    );

    srv.stop(libc::SIGINT, Duration::from_secs(5));
}

#[test]
fn slug_endpoint_matches_cli() {
    let sb = Sandbox::new();
    let mut srv = sb.serve(&[]);

    let text = "Thêm nút Đăng xuất";
    let encoded = url_encode(text);
    let resp = srv.get(&format!("/api/slug?text={encoded}"));
    assert_eq!(resp.status, 200);
    let j = resp.json();
    assert_eq!(j["slug"], "them-nut-dang-xuat");
    assert_eq!(j["slug"], agent_brain::routing::slugify(text));

    srv.stop(libc::SIGINT, Duration::from_secs(5));
}

#[test]
fn jobs_unknown_id_is_404() {
    let sb = Sandbox::new();
    let mut srv = sb.serve(&[]);

    let resp = srv.get("/api/jobs/99999");
    assert_eq!(resp.status, 404);
    assert_eq!(resp.json()["error"], "Không tìm thấy job 99999");

    srv.stop(libc::SIGINT, Duration::from_secs(5));
}

#[test]
fn dash07_ambiguous_or_unknown_project_never_runs() {
    let sb = Sandbox::new();
    let repo1 = sb.make_repo("web-ban-hang", &[("README.md", "# 1\n")], None, None);
    let mut entry1 = sb.project_entry(&repo1, "web-ban-hang", "work");
    entry1["name_vi"] = json!("Web bán hàng");
    entry1["aliases"] = json!(["shop"]);

    let repo2 = sb.make_repo("kho-anh", &[("README.md", "# 2\n")], None, None);
    let mut entry2 = sb.project_entry(&repo2, "kho-anh", "personal");
    entry2["name_vi"] = json!("Kho ảnh");
    entry2["aliases"] = json!(["shop"]);

    sb.write_map(json!([entry1, entry2]), &[]);

    let mut srv = sb.serve(&[]);

    let resp_amb = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "shop",
            "request": "sửa shop"
        }),
    );
    assert_eq!(resp_amb.status, 409);
    let amb_json = resp_amb.json();
    assert_eq!(amb_json["kind"], "ambiguous");
    let candidates = amb_json["candidates"].as_array().expect("candidates");
    assert_eq!(candidates.len(), 2);
    let ids: Vec<&str> = candidates
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"web-ban-hang"));
    assert!(ids.contains(&"kho-anh"));

    let resp_none = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "khong-co",
            "request": "sửa gì đó"
        }),
    );
    assert_eq!(resp_none.status, 409);
    let none_json = resp_none.json();
    assert_eq!(none_json["kind"], "none");

    assert!(
        !sb.record_file.exists()
            || std::fs::read_to_string(&sb.record_file)
                .unwrap()
                .trim()
                .is_empty()
    );

    let conn = sb.db();
    let task_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap_or(0);
    assert_eq!(task_count, 0);

    let run_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap_or(0);
    assert_eq!(run_count, 0);

    assert!(!agent_brain::gitops::branch_exists(&repo1, "task/sua-shop"));
    assert!(!agent_brain::gitops::branch_exists(&repo2, "task/sua-shop"));

    srv.stop(libc::SIGINT, Duration::from_secs(5));
}

#[test]
fn d15_profile_field_rejected() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let sc = lead_scenario(&[("src/app.py", "1", "test", "test")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc).unwrap()).unwrap();

    let mut srv = sb.serve(&[]);

    let resp_rejected = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "test rejected",
            "profile": "personal"
        }),
    );
    assert_eq!(resp_rejected.status, 422);

    let conn = sb.db();
    let run_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap_or(0);
    assert_eq!(run_count, 0);

    let resp_ok = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "test normal",
            "name": "test-normal"
        }),
    );
    assert_eq!(resp_ok.status, 202);
    let job_id = resp_ok.json()["job_id"].as_u64().unwrap();

    let ok = wait_until(Duration::from_secs(30), || {
        let r = srv.get(&format!("/api/jobs/{job_id}"));
        r.status == 200 && r.json()["state"] == "exited"
    });
    assert!(ok);

    let record_content = std::fs::read_to_string(&sb.record_file).unwrap();
    let record_line = record_content.lines().next().unwrap();
    let record: serde_json::Value = serde_json::from_str(record_line).unwrap();
    assert!(
        record["claude_config_dir"].is_null(),
        "work project should have null claude_config_dir"
    );

    srv.stop(libc::SIGINT, Duration::from_secs(5));
}

#[test]
fn d06_dirty_tree_refused_with_reason() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    std::fs::write(repo.join("uncommitted.txt"), "hello").unwrap();

    let mut srv = sb.serve(&[]);

    let resp_pre = srv.get("/api/preflight?project=web-ban-hang&slug=sua-app");
    assert_eq!(resp_pre.status, 200);
    let pre_json = resp_pre.json();
    assert_eq!(pre_json["can_start"], false);
    assert_eq!(pre_json["kind"], "dirty");
    let reason = pre_json["reason"].as_str().unwrap();
    assert!(reason.contains("Working tree có thay đổi chưa commit"));
    assert!(reason.contains("uncommitted.txt"));

    let resp_post = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    assert_eq!(resp_post.status, 409);
    let post_json = resp_post.json();
    assert_eq!(post_json["kind"], "dirty");
    let err = post_json["error"].as_str().unwrap();
    assert!(err.contains("Working tree có thay đổi chưa commit"));
    assert!(err.contains("uncommitted.txt"));

    let conn = sb.db();
    let run_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap_or(0);
    assert_eq!(run_count, 0);

    srv.stop(libc::SIGINT, Duration::from_secs(5));
}

#[test]
fn d06_stacking_warning_allows_start() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    sb.git(&repo, &["checkout", "-b", "task/old"]);

    let sc = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc).unwrap()).unwrap();

    let mut srv = sb.serve(&[]);

    let resp_pre = srv.get("/api/preflight?project=web-ban-hang&slug=sua-app");
    assert_eq!(resp_pre.status, 200);
    let pre_json = resp_pre.json();
    assert_eq!(pre_json["can_start"], true);
    assert!(pre_json["warning"].as_str().unwrap().contains("(stacking)"));

    let resp_post = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    assert_eq!(resp_post.status, 202);
    let post_json = resp_post.json();
    assert!(post_json["warning"]
        .as_str()
        .unwrap()
        .contains("(stacking)"));
    let job_id = post_json["job_id"].as_u64().unwrap();

    let ok = wait_until(Duration::from_secs(30), || {
        let r = srv.get(&format!("/api/jobs/{job_id}"));
        r.status == 200 && r.json()["state"] == "exited"
    });
    assert!(ok);

    srv.stop(libc::SIGINT, Duration::from_secs(5));
}

#[test]
fn d06_branch_exists_refused() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    sb.git(&repo, &["branch", "task/sua-app"]);

    let mut srv = sb.serve(&[]);

    let resp_pre = srv.get("/api/preflight?project=web-ban-hang&slug=sua-app");
    assert_eq!(resp_pre.status, 200);
    let pre_json = resp_pre.json();
    assert_eq!(pre_json["can_start"], false);
    assert_eq!(pre_json["kind"], "branch_exists");
    assert!(pre_json["reason"]
        .as_str()
        .unwrap()
        .contains("đã tồn tại nhưng chưa được checkout"));

    let resp_post = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    assert_eq!(resp_post.status, 409);
    let post_json = resp_post.json();
    assert_eq!(post_json["kind"], "branch_exists");
    assert!(post_json["error"]
        .as_str()
        .unwrap()
        .contains("đã tồn tại nhưng chưa được checkout"));

    srv.stop(libc::SIGINT, Duration::from_secs(5));
}

#[test]
fn d04_project_busy_refused() {
    let sb = Sandbox::new();
    let repo_a = setup_project(&sb, "work");

    let repo_b = sb.make_repo("kho-hang", &[("README.md", "# kho\n")], None, None);
    let entry_b = sb.project_entry(&repo_b, "kho-hang", "work");
    let mut map = sb.read_map();
    map["projects"].as_array_mut().unwrap().push(entry_b);
    sb.write_map(map["projects"].clone(), &[]);

    sb.write_config(json!({
        "claude_bin": Sandbox::fake_claude_tree().to_str().unwrap(),
        "kill_grace_secs": 1,
    }));

    let pidfile_a = sb.root.join("tree_a.pid");
    let pidfile_b = sb.root.join("tree_b.pid");

    let mut srv = sb.serve(&[
        ("FAKE_TREE_PIDFILE", pidfile_a.to_str().unwrap()),
        ("FAKE_TREE_MODE", "coop"),
        ("FAKE_TREE_SLEEP", "317"),
    ]);

    let resp_a = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "chạy sleep 1",
            "name": "sleep-1"
        }),
    );
    assert_eq!(resp_a.status, 202);

    let procs_a = common::tree_procs(&pidfile_a, Duration::from_secs(10)).expect("tree_procs a");
    let _guard_a = common::ProcGuard(procs_a.to_vec());

    let resp_a2 = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "chạy sleep 2",
            "name": "sleep-2"
        }),
    );
    assert_eq!(resp_a2.status, 409);
    assert_eq!(resp_a2.json()["kind"], "busy");
    assert!(resp_a2.json()["error"]
        .as_str()
        .unwrap()
        .contains("đang có"));

    let mut cmd_b = sb.command(
        &[
            "run",
            "lead",
            "kho-hang",
            "chạy sleep B",
            "--name",
            "sleep-b",
        ],
        &[
            ("FAKE_TREE_PIDFILE", pidfile_b.to_str().unwrap()),
            ("FAKE_TREE_MODE", "coop"),
            ("FAKE_TREE_SLEEP", "317"),
        ],
    );
    let mut child_b = cmd_b.spawn().expect("spawn b");
    let procs_b = common::tree_procs(&pidfile_b, Duration::from_secs(10)).expect("tree_procs b");
    let _guard_b = common::ProcGuard(procs_b.to_vec());

    let resp_b = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "kho-hang",
            "request": "chạy sleep B2",
            "name": "sleep-b2"
        }),
    );
    assert_eq!(resp_b.status, 409);
    assert_eq!(resp_b.json()["kind"], "busy");
    assert!(resp_b.json()["error"].as_str().unwrap().contains("đang có"));

    let tid_b: i64 = sb
        .db()
        .query_row(
            "SELECT id FROM tasks WHERE branch = 'task/sleep-b'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    sb.run(&["kill", &tid_b.to_string()]);
    let _ = child_b.wait();

    let tid_a: i64 = sb
        .db()
        .query_row(
            "SELECT id FROM tasks WHERE branch = 'task/sleep-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    sb.run(&["kill", &tid_a.to_string()]);

    srv.stop(libc::SIGINT, Duration::from_secs(5));
}

#[test]
fn d17_second_personal_run_refused() {
    let sb = Sandbox::new();
    let repo1 = sb.make_repo("p1", &[("README.md", "# 1\n")], None, None);
    let entry1 = sb.project_entry(&repo1, "p1", "personal");
    let repo2 = sb.make_repo("p2", &[("README.md", "# 2\n")], None, None);
    let entry2 = sb.project_entry(&repo2, "p2", "personal");
    sb.write_map(json!([entry1, entry2]), &[]);

    sb.write_config(json!({
        "claude_bin": Sandbox::fake_claude_tree().to_str().unwrap(),
        "kill_grace_secs": 1,
    }));

    let pidfile_p1 = sb.root.join("tree_p1.pid");
    let mut srv = sb.serve(&[
        ("FAKE_TREE_PIDFILE", pidfile_p1.to_str().unwrap()),
        ("FAKE_TREE_MODE", "coop"),
        ("FAKE_TREE_SLEEP", "317"),
    ]);

    let resp1 = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "p1",
            "request": "task p1",
            "name": "task-p1"
        }),
    );
    assert_eq!(resp1.status, 202);

    let procs_p1 = common::tree_procs(&pidfile_p1, Duration::from_secs(10)).expect("tree_procs p1");
    let _guard_p1 = common::ProcGuard(procs_p1.to_vec());

    let resp2 = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "p2",
            "request": "task p2",
            "name": "task-p2"
        }),
    );
    assert_eq!(resp2.status, 409);
    let err_msg = resp2.json()["error"].as_str().unwrap().to_string();
    assert!(err_msg.contains("Profile personal đang có run"));
    assert!(err_msg.contains("(E-06)"));

    let tid_1: i64 = sb
        .db()
        .query_row(
            "SELECT id FROM tasks WHERE branch = 'task/task-p1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    sb.run(&["kill", &tid_1.to_string()]);

    srv.stop(libc::SIGINT, Duration::from_secs(5));
}

#[test]
fn request_limits() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let mut srv = sb.serve(&[]);

    let r1 = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "   "
        }),
    );
    assert_eq!(r1.status, 422);

    let big_req = "a".repeat(20001);
    let r2 = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": big_req
        }),
    );
    assert_eq!(r2.status, 422);
    assert!(r2.json()["error"]
        .as_str()
        .unwrap()
        .contains("Yêu cầu quá dài (tối đa 20000 ký tự)"));

    let r3 = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "!!!"
        }),
    );
    assert_eq!(r3.status, 422);
    assert!(r3.json()["error"]
        .as_str()
        .unwrap()
        .contains("Không thể tạo tên task từ yêu cầu — hãy truyền cờ --name <slug>"));

    srv.stop(libc::SIGINT, Duration::from_secs(5));
}
