//! Phase 6 plan 10: task-level team operations on the dashboard. A team is chosen when a task is
//! created (D-12), shown with its history in the task detail (D-13), changed through one shared
//! function, and a quota-stopped step is re-run with another team (D-16, D-18). All of it runs
//! against the fake claude with Rust-side stream data; nothing here talks to a real engine.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::dashboard::api_tasks::{new_task_args, new_task_args_team, Origin};
use common::{wait_until, Sandbox, Serve};
use protocol::{lead_scenario, setup_project};
use serde_json::{json, Value};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
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

fn write_lead_scenario(sb: &Sandbox) {
    sb.write_scenario(lead_scenario(&[(
        "src/app.py",
        "app()",
        "trả về 2",
        "yêu cầu",
    )]));
}

/// teams.json with `doi-a` (all claude, no stage override).
fn write_doi_a(sb: &Sandbox) {
    sb.write_teams(json!({
        "doi-a": {"description": "Toàn claude, đội A", "roles": {}}
    }));
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

fn set_team_json(sb: &Sandbox, task_id: i64, json: Option<&str>) {
    sb.db()
        .execute(
            "UPDATE tasks SET team_json = ?1 WHERE id = ?2",
            rusqlite::params![json, task_id],
        )
        .unwrap();
}

/// Task 1 `sua-app` through the CLI (manual flow), so tests can start from a stored team.
fn cli_lead_task(sb: &Sandbox) -> std::path::PathBuf {
    let repo = setup_project(sb, "work");
    let out = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_scenario(&[(
            "src/app.py",
            "app()",
            "trả về 2",
            "yêu cầu",
        )])),
    );
    assert_eq!(
        out.code, 0,
        "run lead failed: stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    repo
}

// ---------------------------------------------------------------------------
// Task 1: team on creation and in the task detail
// ---------------------------------------------------------------------------

#[test]
fn new_task_args_team_vector() {
    let (role, args) = new_task_args_team(
        false,
        "s",
        "nhanh",
        Some("doi-a"),
        "web",
        "r",
        Origin::Dashboard,
    );
    assert_eq!(role, "lead");
    assert_eq!(
        args,
        strings(&[
            "run",
            "lead",
            "--name",
            "s",
            "--workflow",
            "nhanh",
            "--team",
            "doi-a",
            "--",
            "web",
            "r"
        ])
    );

    let (role, args) = new_task_args_team(
        true,
        "s",
        "mac-dinh",
        Some("doi-a"),
        "web",
        "r",
        Origin::Telegram,
    );
    assert_eq!(role, "auto");
    assert_eq!(
        args,
        strings(&[
            "auto",
            "--start",
            "--name",
            "s",
            "--workflow",
            "mac-dinh",
            "--team",
            "doi-a",
            "--origin",
            "telegram",
            "--",
            "web",
            "r"
        ])
    );

    // Without a team the vector is the one `new_task_args` always produced.
    let plain = new_task_args(false, "s", "nhanh", "web", "r", Origin::Dashboard);
    assert_eq!(
        new_task_args_team(false, "s", "nhanh", None, "web", "r", Origin::Dashboard),
        plain
    );
    assert!(!plain.1.iter().any(|a| a == "--team"));
}

#[test]
fn create_task_with_team_records() {
    let sb = Sandbox::new();
    write_doi_a(&sb);
    setup_project(&sb, "work");
    write_lead_scenario(&sb);
    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app",
            "workflow": "nhanh",
            "team": "doi-a"
        }),
    );
    assert_eq!(resp.status, 202, "body: {}", resp.text());
    let body = resp.json();
    assert_eq!(body["team"], "doi-a");
    let job_id = body["job_id"].as_u64().unwrap();
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 0, "job: {job}");

    // The CLI got `--team doi-a` and recorded it before the Lead started.
    let stored: Value = serde_json::from_str(&team_json_of(&sb, 1).expect("team_json")).unwrap();
    assert_eq!(stored["name"], "doi-a");

    let task = srv.get("/api/tasks/1").json();
    assert_eq!(
        task["team"],
        json!({
            "name": "doi-a",
            "summary": "tất cả: claude",
            "description": "Toàn claude, đội A",
            "missing": false,
            "legacy": false
        })
    );
    assert_eq!(task["team_history"], json!([]));
}

#[test]
fn create_task_unknown_team_422() {
    let sb = Sandbox::new();
    write_doi_a(&sb);
    setup_project(&sb, "work");
    write_lead_scenario(&sb);
    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app",
            "team": "khong-co"
        }),
    );
    assert_eq!(resp.status, 422, "body: {}", resp.text());
    let error = resp.json()["error"].as_str().unwrap_or("").to_string();
    assert!(
        error.contains("Không có đội hình 'khong-co'"),
        "error: {error}"
    );
    assert!(
        error.contains("doi-a"),
        "error lists the known teams: {error}"
    );

    // No job started and no task row written.
    assert_eq!(srv.get("/api/jobs/1").status, 404);
    let rows: i64 = sb
        .db()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap_or(0);
    assert_eq!(rows, 0);
}

#[test]
fn create_task_project_default_team() {
    let sb = Sandbox::new();
    write_doi_a(&sb);
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    entry["name_vi"] = json!("Web bán hàng");
    entry["team"] = json!("doi-a");
    sb.write_map(json!([entry]), &[]);
    write_lead_scenario(&sb);
    let srv = sb.serve(&[]);

    // No `team` in the body: the project's `team` key decides.
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "sửa app", "name": "sua-app"}),
    );
    assert_eq!(resp.status, 202, "body: {}", resp.text());
    assert_eq!(resp.json()["team"], "doi-a");
    let job = wait_for_job(&srv, resp.json()["job_id"].as_u64().unwrap());
    assert_eq!(job["exit_code"], 0, "job: {job}");
    let task = srv.get("/api/tasks/1").json();
    assert_eq!(task["team"]["name"], "doi-a");
    assert_eq!(task["team_history"], json!([]));
}

#[test]
fn project_without_team_key_uses_mac_dinh() {
    let sb = Sandbox::new();
    setup_project(&sb, "work");
    write_lead_scenario(&sb);
    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "sửa app", "name": "sua-app"}),
    );
    assert_eq!(resp.status, 202, "body: {}", resp.text());
    assert_eq!(resp.json()["team"], "mac-dinh");
    let job = wait_for_job(&srv, resp.json()["job_id"].as_u64().unwrap());
    assert_eq!(job["exit_code"], 0, "job: {job}");
    assert_eq!(srv.get("/api/tasks/1").json()["team"]["name"], "mac-dinh");
}

#[test]
fn legacy_and_missing_team_payload() {
    let sb = Sandbox::new();
    cli_lead_task(&sb);
    let srv = sb.serve(&[]);

    // A task created before Phase 6 has NULL team_json: mac-dinh, every role on claude.
    set_team_json(&sb, 1, None);
    let task = srv.get("/api/tasks/1").json();
    assert_eq!(
        task["team"],
        json!({
            "name": "mac-dinh",
            "summary": "tất cả: claude",
            "description": Value::Null,
            "missing": false,
            "legacy": true
        })
    );
    assert_eq!(task["team_history"], json!([]));

    // A stored team that teams.json no longer has is `missing`.
    set_team_json(&sb, 1, Some(r#"{"name":"da-xoa"}"#));
    let task = srv.get("/api/tasks/1").json();
    assert_eq!(
        task["team"],
        json!({
            "name": "da-xoa",
            "summary": Value::Null,
            "description": Value::Null,
            "missing": true,
            "legacy": false
        })
    );
}

#[test]
fn rev_changes_with_team_json() {
    let sb = Sandbox::new();
    cli_lead_task(&sb);
    let srv = sb.serve(&[]);

    let before = srv.get("/api/tasks/1").json()["rev"].clone();
    set_team_json(
        &sb,
        1,
        Some(
            r#"{"name":"tiet-kiem","history":[{"at":1,"by":"cli","from":"mac-dinh","to":"tiet-kiem"}]}"#,
        ),
    );
    let after = srv.get("/api/tasks/1").json();
    assert_ne!(before, after["rev"]);
    assert_eq!(after["team"]["name"], "tiet-kiem");
    assert_eq!(
        after["team_history"],
        json!([{"at": 1, "by": "cli", "from": "mac-dinh", "to": "tiet-kiem"}])
    );
}
