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
    cli_lead_task_with(sb, None)
}

/// As `cli_lead_task`, optionally with `--team <name>` so the stored team is known.
fn cli_lead_task_with(sb: &Sandbox, team: Option<&str>) -> std::path::PathBuf {
    let repo = setup_project(sb, "work");
    let mut args = vec!["run", "lead", "--name", "sua-app"];
    if let Some(t) = team {
        args.extend(["--team", t]);
    }
    args.extend(["--", "web-ban-hang", "sửa app"]);
    let out = sb.run(
        &args,
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

// ---------------------------------------------------------------------------
// Task 2: change the task's team
// ---------------------------------------------------------------------------

fn rev_of(srv: &Serve, id: i64) -> String {
    srv.get(&format!("/api/tasks/{id}")).json()["rev"]
        .as_str()
        .unwrap()
        .to_string()
}

fn action_of(task: &Value, id: &str) -> Value {
    task["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("action {id} missing in {}", task["actions"]))
}

/// A `running` run row without a real process (pid 0) for the task.
fn add_running_run(sb: &Sandbox, task_id: i64) -> i64 {
    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let base = agent_brain::db::get_run(&conn, 1)
        .expect("get_run")
        .expect("run 1");
    agent_brain::db::insert_run(
        &conn,
        &agent_brain::db::NewRun {
            task_id: Some(task_id),
            project_id: base.project_id,
            role: "dev".to_string(),
            profile: base.profile.clone(),
            model: None,
            pid: 0,
            pgid: 0,
            pid_start: 0,
            base_commit: None,
            run_dir: None,
        },
    )
    .expect("insert_run")
}

#[test]
fn change_team_records_history() {
    let sb = Sandbox::new();
    write_doi_a(&sb);
    cli_lead_task_with(&sb, Some("doi-a"));
    let srv = sb.serve(&[]);

    let before = srv.get("/api/tasks/1").json();
    let action = action_of(&before, "change_team");
    assert_eq!(action["label"], "Đổi đội hình");
    assert_eq!(action["enabled"], true, "action: {action}");
    let engines_before: Vec<String> = sb
        .db()
        .prepare("SELECT engine FROM runs ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    let resp = srv.post_json(
        "/api/tasks/1/team",
        &json!({"team": "tiet-kiem", "rev": before["rev"]}),
    );
    assert_eq!(resp.status, 200, "body: {}", resp.text());
    assert_eq!(
        resp.json(),
        json!({"ok": true, "from": "doi-a", "to": "tiet-kiem"})
    );

    let after = srv.get("/api/tasks/1").json();
    assert_eq!(after["team"]["name"], "tiet-kiem");
    let history = after["team_history"].as_array().unwrap();
    assert_eq!(history.len(), 1, "history: {history:?}");
    assert_eq!(history[0]["by"], "dashboard");
    assert_eq!(history[0]["from"], "doi-a");
    assert_eq!(history[0]["to"], "tiet-kiem");
    assert!(history[0]["at"].as_i64().unwrap_or(0) > 0);
    assert_ne!(before["rev"], after["rev"], "rev changes with team_json");

    // The runs that already ran keep their engine.
    let engines_after: Vec<String> = sb
        .db()
        .prepare("SELECT engine FROM runs ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(engines_before, engines_after);
}

#[test]
fn change_team_refusals() {
    let sb = Sandbox::new();
    write_doi_a(&sb);
    cli_lead_task_with(&sb, Some("doi-a"));
    let srv = sb.serve(&[]);
    let stored_before = team_json_of(&sb, 1);

    // The current team.
    let resp = srv.post_json("/api/tasks/1/team", &json!({"team": "doi-a"}));
    assert_eq!(resp.status, 409, "body: {}", resp.text());
    assert_eq!(
        resp.json(),
        json!({"error": "Chọn đội hình khác đội hình hiện tại.", "kind": "not_allowed"})
    );

    // An unknown team.
    let resp = srv.post_json("/api/tasks/1/team", &json!({"team": "khong-co"}));
    assert_eq!(resp.status, 422, "body: {}", resp.text());
    assert!(
        resp.json()["error"]
            .as_str()
            .unwrap_or("")
            .contains("Không có đội hình 'khong-co'"),
        "body: {}",
        resp.text()
    );

    // A stale rev.
    let resp = srv.post_json(
        "/api/tasks/1/team",
        &json!({"team": "tiet-kiem", "rev": "0000000000000000"}),
    );
    assert_eq!(resp.status, 409, "body: {}", resp.text());
    assert_eq!(resp.json()["kind"], "stale");

    // A missing task.
    let resp = srv.post_json("/api/tasks/999/team", &json!({"team": "tiet-kiem"}));
    assert_eq!(resp.status, 404, "body: {}", resp.text());

    // Unknown body fields are rejected.
    let resp = srv.post_json(
        "/api/tasks/1/team",
        &json!({"team": "tiet-kiem", "by": "telegram"}),
    );
    assert!(
        resp.status == 400 || resp.status == 422,
        "status {}",
        resp.status
    );

    // A live run: refused with the locked reason, nothing written.
    add_running_run(&sb, 1);
    let task = srv.get("/api/tasks/1").json();
    let action = action_of(&task, "change_team");
    assert_eq!(action["enabled"], false);
    let resp = srv.post_json("/api/tasks/1/team", &json!({"team": "tiet-kiem"}));
    assert_eq!(resp.status, 409, "body: {}", resp.text());
    assert_eq!(
        resp.json()["error"],
        "Đang có run chạy — chờ run xong hoặc Kill rồi mới đổi đội hình."
    );
    assert_eq!(team_json_of(&sb, 1), stored_before, "no write on a refusal");
}

#[test]
fn change_team_waiting_human_refused() {
    let sb = Sandbox::new();
    sb.write_staff(json!({"ha": {"display_name": "Hà"}}));
    cli_lead_task_with(&sb, None);
    let waiting = r#"{"name":"mac-dinh","history":[],"wait":{"mode":"waiting","role":"dev","stage":"implement","staff":"ha","since_ms":1000,"branch":"task/sua-app","base":"abc"}}"#;
    set_team_json(&sb, 1, Some(waiting));
    let srv = sb.serve(&[]);

    let task = srv.get("/api/tasks/1").json();
    let action = action_of(&task, "change_team");
    assert_eq!(action["enabled"], false);
    assert_eq!(
        action["reason"],
        "Đang chờ Hà làm — chưa đổi được đội hình."
    );

    let resp = srv.post_json(
        "/api/tasks/1/team",
        &json!({"team": "tiet-kiem", "rev": task["rev"]}),
    );
    assert_eq!(resp.status, 409, "body: {}", resp.text());
    assert_eq!(
        resp.json()["error"],
        "Đang chờ Hà làm — chưa đổi được đội hình."
    );
    assert_eq!(team_json_of(&sb, 1).as_deref(), Some(waiting));
}

#[test]
fn change_team_legacy_from_mac_dinh() {
    let sb = Sandbox::new();
    write_doi_a(&sb);
    cli_lead_task(&sb);
    set_team_json(&sb, 1, None);
    let srv = sb.serve(&[]);

    let resp = srv.post_json("/api/tasks/1/team", &json!({"team": "doi-a"}));
    assert_eq!(resp.status, 200, "body: {}", resp.text());
    assert_eq!(
        resp.json(),
        json!({"ok": true, "from": "mac-dinh", "to": "doi-a"})
    );
    let task = srv.get("/api/tasks/1").json();
    assert_eq!(task["team"]["name"], "doi-a");
    assert_eq!(task["team"]["legacy"], false);
    assert_eq!(task["team_history"][0]["from"], "mac-dinh");
    assert_eq!(task["team_history"][0]["to"], "doi-a");
    let stored: Value = serde_json::from_str(&team_json_of(&sb, 1).unwrap()).unwrap();
    assert_eq!(stored["name"], "doi-a");
}
