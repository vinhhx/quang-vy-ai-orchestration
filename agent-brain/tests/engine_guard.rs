//! Phase 6 plan 22: a profile-restricted engine (`pin_profile` / `allowed_profiles`) can be bound
//! to no task of a forbidden project through any path (ENG-10, D-36, D-38, D-42), a `dsh` engine
//! is only allowed for Dev (D-37), and an id missing from config.json is visible (D-43). All of it
//! runs against the fake claude; the `dsh` entry is only parsed, never run.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{wait_until, Sandbox, Serve};
use protocol::lead_scenario;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const TEAM_ID: &str = "doi-max";
const PROJECT: &str = "web-ban-hang";

fn fake_cmd() -> String {
    Sandbox::fake_claude().to_str().unwrap().to_string()
}

/// A native entry pinned to the personal profile, run by the fake claude.
fn claude_max_entry() -> Value {
    json!({
        "id": "claude-max",
        "kind": "anthropic",
        "command": fake_cmd(),
        "pin_profile": "personal",
        "models": {"lead": "opus", "dev": "opus"}
    })
}

/// An entry of kind dsh: only parsed, never run in this plan.
fn dsh_entry() -> Value {
    json!({
        "id": "dsh",
        "kind": "dsh",
        "docker": "/nonexistent/docker-test",
        "container": "fake-dsh"
    })
}

/// The exact refusal text of `engine::profile_refusal` for `claude-max` on `project_profile`.
fn refusal_text(project_profile: &str, role: &str) -> String {
    format!(
        "Engine 'claude-max' chỉ dùng cho dự án profile personal — dự án '{PROJECT}' dùng profile {project_profile} (ranh giới work/personal, SPK-10). Đổi engine của vai trò {role} hoặc chọn đội hình khác."
    )
}

fn write_doi_max(sb: &Sandbox) {
    sb.write_config(json!({"engines": [claude_max_entry(), dsh_entry()]}));
    sb.write_teams(json!({
        TEAM_ID: {
            "description": "Dev trên Claude Max",
            "roles": {"lead": "claude", "dev": "claude-max"}
        }
    }));
}

/// Repo `web-ban-hang`, map entry of `profile` and an optional project default `team`.
fn setup_project_with(sb: &Sandbox, profile: &str, team: Option<&str>) -> PathBuf {
    let repo = sb.make_repo(
        PROJECT,
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, PROJECT, profile);
    if let Some(t) = team {
        entry["team"] = json!(t);
    }
    sb.write_map(json!([entry]), &[]);
    repo
}

fn write_lead_scenario(sb: &Sandbox) {
    sb.write_scenario(lead_scenario(&[(
        "src/app.py",
        "app()",
        "trả về 2",
        "yêu cầu",
    )]));
}

fn wait_for_job(srv: &Serve, job_id: u64) -> Value {
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
    assert!(ok, "job {job_id} did not exit within timeout");
    job_view.unwrap()
}

fn task_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap_or(0)
}

fn team_json_of(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT team_json FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
}

fn task_branches(sb: &Sandbox, repo: &std::path::Path) -> String {
    sb.git(repo, &["branch", "--list", "task/*"])
}

// ---------------------------------------------------------------------------
// Task 1 (tracer): Giao việc
// ---------------------------------------------------------------------------

#[test]
fn eng10_giao_viec_refused_on_work() {
    let sb = Sandbox::new();
    write_doi_max(&sb);
    let repo = setup_project_with(&sb, "work", None);
    write_lead_scenario(&sb);
    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": PROJECT,
            "request": "sửa app",
            "name": "sua-app",
            "team": TEAM_ID
        }),
    );
    assert_eq!(resp.status, 422, "body: {}", resp.text());
    let error = resp.json()["error"].as_str().unwrap_or("").to_string();
    assert!(
        error.contains("Engine 'claude-max' chỉ dùng cho dự án profile personal"),
        "error: {error}"
    );
    assert_eq!(error, refusal_text("work", "dev"));

    // No job, no task row, no branch.
    assert_eq!(srv.get("/api/jobs/1").status, 404);
    assert_eq!(task_count(&sb), 0);
    assert_eq!(task_branches(&sb, &repo).trim(), "");
    assert!(sb.records().is_empty(), "nothing was spawned");
}

#[test]
fn eng10_giao_viec_ok_on_personal() {
    let sb = Sandbox::new();
    write_doi_max(&sb);
    setup_project_with(&sb, "personal", None);
    write_lead_scenario(&sb);
    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": PROJECT,
            "request": "sửa app",
            "name": "sua-app",
            "team": TEAM_ID
        }),
    );
    assert_eq!(resp.status, 202, "body: {}", resp.text());
    let body = resp.json();
    assert_eq!(body["team"], TEAM_ID);
    let job = wait_for_job(&srv, body["job_id"].as_u64().unwrap());
    assert_eq!(job["exit_code"], 0, "job: {job}");

    let stored: Value = serde_json::from_str(&team_json_of(&sb, 1).expect("team_json")).unwrap();
    assert_eq!(stored["name"], TEAM_ID);
}
