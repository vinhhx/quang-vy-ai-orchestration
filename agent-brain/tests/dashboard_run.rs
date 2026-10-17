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
    assert_eq!(resp.status, 202, "POST /api/tasks response: {}", resp.text());
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
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE role != 'lead'",
            [],
            |r| r.get(0),
        )
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
    assert!(sse_out.contains("event: end"), "SSE did not contain event: end: {sse_out}");
    assert!(sse_out.contains("succeeded"), "SSE did not contain succeeded: {sse_out}");

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
