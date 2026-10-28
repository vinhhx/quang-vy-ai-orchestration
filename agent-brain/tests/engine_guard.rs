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

// ---------------------------------------------------------------------------
// Task 2: the other bind points
// ---------------------------------------------------------------------------

fn assert_refused_cli(res: &common::RunOut, what: &str) {
    assert_eq!(res.code, 1, "{what}: stderr: {}", res.stderr);
    assert!(
        res.stderr.contains(&refusal_text("work", "dev")),
        "{what}: stderr: {}",
        res.stderr
    );
}

fn lead_plan() -> Value {
    lead_scenario(&[("src/app.py", "app()", "trả về 2", "yêu cầu")])
}

#[test]
fn eng10_run_lead_team_refused() {
    let sb = Sandbox::new();
    write_doi_max(&sb);
    let repo = setup_project_with(&sb, "work", None);

    let res = sb.run(
        &[
            "run",
            "lead",
            "--team",
            TEAM_ID,
            "--name",
            "sua-app",
            "--",
            PROJECT,
            "sửa app",
        ],
        Some(lead_plan()),
    );
    assert_refused_cli(&res, "run lead --team");
    assert_eq!(task_branches(&sb, &repo).trim(), "");
    assert_eq!(task_count(&sb), 0);
    assert!(sb.records().is_empty(), "nothing was spawned");
}

#[test]
fn eng10_auto_start_team_refused() {
    let sb = Sandbox::new();
    write_doi_max(&sb);
    let repo = setup_project_with(&sb, "work", None);

    let res = sb.run(
        &[
            "auto",
            "--start",
            "--team",
            TEAM_ID,
            "--name",
            "sua-app",
            "--",
            PROJECT,
            "sửa app",
        ],
        Some(lead_plan()),
    );
    assert_refused_cli(&res, "auto --start --team");
    assert_eq!(task_branches(&sb, &repo).trim(), "");
    assert_eq!(task_count(&sb), 0);
    assert!(sb.records().is_empty(), "nothing was spawned");
}

#[test]
fn eng10_project_default_team_refused() {
    let sb = Sandbox::new();
    write_doi_max(&sb);
    let repo = setup_project_with(&sb, "work", Some(TEAM_ID));
    write_lead_scenario(&sb);
    let srv = sb.serve(&[]);

    // Giao việc without a team: the project default applies and is refused.
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": PROJECT, "request": "sửa app", "name": "sua-app"}),
    );
    assert_eq!(resp.status, 422, "body: {}", resp.text());
    assert_eq!(resp.json()["error"], refusal_text("work", "dev"));
    assert_eq!(srv.get("/api/jobs/1").status, 404);

    // Settings shows why the default does not work.
    let teams = srv.get("/api/teams").json();
    let note = teams["project_defaults"][PROJECT]["note"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert_eq!(note, refusal_text("work", "dev"), "teams: {teams}");

    // The CLI without --team applies the same default and refuses it.
    let res = sb.run(
        &["run", "lead", "--name", "sua-app", "--", PROJECT, "sửa app"],
        Some(lead_plan()),
    );
    assert_refused_cli(&res, "run lead (project default)");
    assert_eq!(task_branches(&sb, &repo).trim(), "");
    assert_eq!(task_count(&sb), 0);
}

/// A work-project task on the built-in `mac-dinh`, planned by the CLI Lead.
fn lead_task_on_work(sb: &Sandbox) {
    write_doi_max(sb);
    setup_project_with(sb, "work", None);
    let res = sb.run(
        &["run", "lead", "--name", "sua-app", "--", PROJECT, "sửa app"],
        Some(lead_plan()),
    );
    assert_eq!(
        res.code, 0,
        "run lead failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

#[test]
fn eng10_change_team_refused() {
    let sb = Sandbox::new();
    lead_task_on_work(&sb);
    let srv = sb.serve(&[]);
    let before = team_json_of(&sb, 1);

    let rev = srv.get("/api/tasks/1").json()["rev"].clone();
    let resp = srv.post_json("/api/tasks/1/team", &json!({"team": TEAM_ID, "rev": rev}));
    assert_eq!(resp.status, 422, "body: {}", resp.text());
    assert_eq!(resp.json()["error"], refusal_text("work", "dev"));
    assert_eq!(team_json_of(&sb, 1), before, "team_json is untouched");
}

/// A claude run that hits the quota wall (exit 1, outcome `quota`).
fn quota_entry() -> Value {
    json!({
        "ops": [{"op": "write", "path": "src/app.py", "content": "phần 1"}],
        "events": [
            {"type": "system", "subtype": "init", "session_id": "fake-quota", "model": "fake"},
            {
                "type": "rate_limit_event",
                "rate_limit_info": {
                    "status": "rejected",
                    "resetsAt": 1792310400,
                    "rateLimitType": "five_hour",
                    "isUsingOverage": false,
                    "overageStatus": "rejected"
                },
                "uuid": "u1",
                "session_id": "fake-quota"
            },
            {
                "type": "result",
                "subtype": "success",
                "is_error": true,
                "api_error_status": 429,
                "terminal_reason": "blocking_limit",
                "result": "You've hit your session limit",
                "num_turns": 1,
                "session_id": "fake-quota",
                "total_cost_usd": 0,
                "usage": {
                    "input_tokens": 0,
                    "output_tokens": 0,
                    "cache_read_input_tokens": 0,
                    "cache_creation_input_tokens": 0
                },
                "permission_denials": []
            }
        ],
        "exit_code": 1,
    })
}

fn run_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap_or(0)
}

#[test]
fn eng10_quota_rerun_refused() {
    let sb = Sandbox::new();
    lead_task_on_work(&sb);
    sb.write_scenario(json!({"by_role": {"dev": [quota_entry()]}}));
    let srv = sb.serve(&[]);
    let resp = srv.post_json("/api/tasks/1/dev", &json!({}));
    assert_eq!(resp.status, 202, "body: {}", resp.text());
    let job = wait_for_job(&srv, resp.json()["job_id"].as_u64().unwrap());
    assert_eq!(job["exit_code"], 5, "the quota stop exits 5: {job}");

    let before = team_json_of(&sb, 1);
    let runs_before = run_count(&sb);
    let task = srv.get("/api/tasks/1").json();
    let resp = srv.post_json(
        "/api/tasks/1/quota-rerun",
        &json!({"team": TEAM_ID, "rev": task["rev"]}),
    );
    assert_eq!(resp.status, 422, "body: {}", resp.text());
    assert_eq!(resp.json()["error"], refusal_text("work", "dev"));
    assert_eq!(team_json_of(&sb, 1), before, "team_json is untouched");
    assert_eq!(run_count(&sb), runs_before, "no new run row");
    assert_eq!(srv.get("/api/jobs/2").status, 404, "no new job");
}

// ---------------------------------------------------------------------------
// Task 3: loader and Settings save know the engine list
// ---------------------------------------------------------------------------

fn team_entry(name: &str, roles: Value) -> Value {
    let mut full = json!({
        "lead": "claude", "dev": "claude", "qa": "claude",
        "plan_review": "claude", "code_review": "claude"
    });
    for (k, v) in roles.as_object().unwrap() {
        full[k] = v.clone();
    }
    json!({"name": name, "description": "Thử", "roles": full})
}

fn teams_version(srv: &Serve) -> String {
    srv.get("/api/teams").json()["version"]
        .as_str()
        .unwrap()
        .to_string()
}

fn save_team(srv: &Serve, version: &str, entry: Value) -> common::HttpResp {
    srv.post_json(
        "/api/teams/save",
        &json!({"version": version, "create": true, "entry": entry}),
    )
}

/// The `(field, message)` pairs of a 422 save refusal.
fn refusals(resp: &common::HttpResp) -> Vec<(String, String)> {
    assert_eq!(resp.status, 422, "{}", resp.text());
    let j = resp.json();
    assert_eq!(j["error"], "Không lưu được đội hình");
    j["errors"]
        .as_array()
        .unwrap_or_else(|| panic!("errors array in {j}"))
        .iter()
        .map(|e| {
            (
                e["field"].as_str().unwrap_or("").to_string(),
                e["message"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn team_named<'a>(j: &'a Value, name: &str) -> Option<&'a Value> {
    j["teams"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == name)
}

fn teams_on_disk(sb: &Sandbox) -> Value {
    serde_json::from_str(&std::fs::read_to_string(sb.teams_path()).unwrap()).unwrap()
}

const DSH_ONLY_DEV: &str = "Engine 'dsh' là kiểu dsh, chỉ dùng cho vai trò dev.";

#[test]
fn d37_dsh_only_dev_loader_and_save() {
    let sb = Sandbox::new();
    write_doi_max(&sb);
    sb.write_teams(json!({}));
    let srv = sb.serve(&[]);
    let v = teams_version(&srv);

    // Settings save: dsh on a read-only role is refused with a field error on that role.
    let r = save_team(&srv, &v, team_entry("doi-x", json!({"plan_review": "dsh"})));
    assert_eq!(
        refusals(&r),
        [("plan_review".to_string(), DSH_ONLY_DEV.to_string())]
    );
    assert_eq!(teams_on_disk(&sb), json!({}), "nothing was written");

    // dsh on dev is accepted.
    let r = save_team(&srv, &v, team_entry("doi-dev", json!({"dev": "dsh"})));
    assert_eq!(r.status, 200, "{}", r.text());

    // A hand-edited file with dsh on Lead: the loader reports the line and drops the team.
    sb.write_teams(json!({
        "doi-dev": {"description": "Thử", "roles": {"dev": "dsh"}},
        "doi-dsh-sai": {"description": "Sai", "roles": {"lead": "dsh"}}
    }));
    let j = srv.get("/api/teams").json();
    let errors: Vec<String> = j["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_str().unwrap().to_string())
        .collect();
    assert_eq!(errors.len(), 1, "errors: {errors:?}");
    assert!(
        errors[0].starts_with("Dòng ")
            && errors[0].ends_with(&format!("đội hình 'doi-dsh-sai': {DSH_ONLY_DEV}")),
        "errors: {errors:?}"
    );
    assert!(team_named(&j, "doi-dsh-sai").is_none(), "teams: {j}");
    assert!(team_named(&j, "doi-dev").is_some(), "teams: {j}");
}

#[test]
fn team_unknown_engine_warns_and_save_refused() {
    let sb = Sandbox::new();
    // A list without `deepseek`: the built-in `tiet-kiem` keeps naming it.
    sb.write_config(json!({"engines": [claude_max_entry()]}));
    sb.write_teams(json!({"doi-cu": {"description": "Cũ", "roles": {"dev": "khong-co"}}}));
    let srv = sb.serve(&[]);

    let j = srv.get("/api/teams").json();
    assert_eq!(j["errors"], json!([]), "teams: {j}");
    let cu = team_named(&j, "doi-cu").expect("a hand-edited team with an unknown id still loads");
    assert_eq!(
        cu["warnings"],
        json!(["Engine 'khong-co' chưa khai báo trong config.json."])
    );
    let tk = team_named(&j, "tiet-kiem").expect("built-in team");
    assert_eq!(tk["roles"]["dev"], "deepseek");
    assert_eq!(
        tk["warnings"],
        json!(["Engine 'deepseek' chưa khai báo trong config.json."])
    );

    // Saving one is refused; nothing resolves it to another endpoint.
    let v = j["version"].as_str().unwrap().to_string();
    let r = save_team(&srv, &v, team_entry("doi-y", json!({"dev": "khong-co"})));
    assert_eq!(
        refusals(&r),
        [(
            "dev".to_string(),
            "Engine 'khong-co' không có trong config.json (engines).".to_string()
        )]
    );
}

fn d42_text(profile: &str, whereabouts: &str) -> String {
    format!(
        "Đội hình '{TEAM_ID}' đang được dùng trên dự án profile {profile} ({whereabouts}) — engine 'claude-max' chỉ dùng cho dự án profile personal. Đổi đội hình của dự án hoặc task đó trước rồi lưu lại."
    )
}

fn save_doi_max(srv: &Serve) -> common::HttpResp {
    let v = teams_version(srv);
    save_team(srv, &v, team_entry(TEAM_ID, json!({"dev": "claude-max"})))
}

/// Plans a task on the project, then points its team_json at `doi-max` directly in the DB.
fn bind_open_task(sb: &Sandbox) {
    let res = sb.run(
        &["run", "lead", "--name", "sua-app", "--", PROJECT, "sửa app"],
        Some(lead_plan()),
    );
    assert_eq!(
        res.code, 0,
        "run lead failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    sb.db()
        .execute(
            "UPDATE tasks SET team_json = ?1 WHERE id = 1",
            [format!(r#"{{"name":"{TEAM_ID}","history":[]}}"#)],
        )
        .unwrap();
}

#[test]
fn d42_save_refused_only_when_bound() {
    // No binding: the save succeeds.
    let sb = Sandbox::new();
    write_doi_max(&sb);
    sb.write_teams(json!({}));
    setup_project_with(&sb, "work", None);
    let srv = sb.serve(&[]);
    let r = save_doi_max(&srv);
    assert_eq!(r.status, 200, "{}", r.text());
    drop(srv);

    // A work project has it as its default team.
    let sb = Sandbox::new();
    write_doi_max(&sb);
    sb.write_teams(json!({}));
    setup_project_with(&sb, "work", Some(TEAM_ID));
    let srv = sb.serve(&[]);
    let r = save_doi_max(&srv);
    assert_eq!(
        refusals(&r),
        [(
            "name".to_string(),
            d42_text("work", &format!("dự án '{PROJECT}'"))
        )]
    );
    assert_eq!(teams_on_disk(&sb), json!({}), "nothing was written");
    drop(srv);

    // No default, but an open task of a work project uses it.
    let sb = Sandbox::new();
    write_doi_max(&sb);
    sb.write_teams(json!({}));
    setup_project_with(&sb, "work", None);
    bind_open_task(&sb);
    let srv = sb.serve(&[]);
    let r = save_doi_max(&srv);
    assert_eq!(
        refusals(&r),
        [("name".to_string(), d42_text("work", "1 task đang mở"))]
    );
    drop(srv);

    // The same bindings on a personal project are fine.
    let sb = Sandbox::new();
    write_doi_max(&sb);
    sb.write_teams(json!({}));
    setup_project_with(&sb, "personal", Some(TEAM_ID));
    bind_open_task(&sb);
    let srv = sb.serve(&[]);
    let r = save_doi_max(&srv);
    assert_eq!(r.status, 200, "{}", r.text());
}
