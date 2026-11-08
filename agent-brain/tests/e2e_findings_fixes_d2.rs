//! D2 (shared workspace): `base_branch`, `hold_project_at_gate`, `auto_checkout_on_approve`.
//! - a task waiting at the plan gate holds the project for other tasks (option on, the default);
//!   off = the old behaviour;
//! - approving off the task branch checks it out when the tree is clean; a dirty tree keeps the
//!   409 with the dirty files; option off = the old refusal;
//! - new tasks start from `base_branch`, else the repo's current branch;
//! - `PUT /api/projects/{id}/workspace` validates and stores the three options.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{wait_until, RunOut, Sandbox};
use protocol::lead_scenario;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

const PROJECT: &str = "web-ban-hang";
const MISMATCH: &str = "Không ở nhánh task 'task/sua-app' (nhánh hiện tại: 'main') — không đoán";

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

fn by_role(roles: Value) -> Value {
    json!({ "by_role": roles })
}

fn lead_plan_entry() -> Value {
    lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])
}

fn plan_review_entry() -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan-review.md",
        "content": "---\nverdict: approve\nblocking: 0\nadvisory: 0\nfiles: []\n---\n# Review: sửa app\n## Mục chặn (blocking)\n## Góp ý (advisory)\n",
    }]})
}

/// Repo `web-ban-hang` on the server default workflow (auto), with extra project keys.
fn setup(sb: &Sandbox, extra: Value) -> PathBuf {
    let repo = sb.make_repo(
        PROJECT,
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    write_entry(sb, &repo, extra);
    repo
}

fn write_entry(sb: &Sandbox, repo: &Path, extra: Value) {
    let mut entry = sb.project_entry(repo, PROJECT, "work");
    let obj = entry.as_object_mut().unwrap();
    obj.remove("workflow");
    if let Some(e) = extra.as_object() {
        for (k, v) in e {
            obj.insert(k.clone(), v.clone());
        }
    }
    sb.write_map(json!([entry]), &[]);
}

/// `auto --start` task `sua-app`: Lead, plan review approves, stops at the plan gate.
fn auto_to_gate(sb: &Sandbox) {
    let sc = by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry()],
    }));
    let res = sb.run(
        &["auto", "--start", "--name", "sua-app", "--", PROJECT, "sửa app"],
        Some(sc),
    );
    assert_ok(&res, "auto --start");
    assert_eq!(auto_state(sb, 1).as_deref(), Some("gate"));
}

fn auto_state(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row("SELECT auto_state FROM tasks WHERE id = ?1", [task_id], |r| r.get(0))
        .unwrap()
}

fn approved_commit(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row("SELECT approved_commit FROM tasks WHERE id = ?1", [task_id], |r| r.get(0))
        .unwrap()
}

fn base_branch_of(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row("SELECT base_branch FROM tasks WHERE id = ?1", [task_id], |r| r.get(0))
        .unwrap()
}

fn task_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap()
}

fn current_branch(sb: &Sandbox, repo: &Path) -> String {
    sb.git(repo, &["branch", "--show-current"]).trim().to_string()
}

fn control<'a>(task: &'a Value, id: &str) -> &'a Value {
    task["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("control {id} missing: {}", task["controls"]))
}

fn wait_for_job(srv: &common::Serve, job_id: u64) -> Value {
    let mut view = None;
    let ok = wait_until(Duration::from_secs(30), || {
        let r = srv.get(&format!("/api/jobs/{job_id}"));
        if r.status == 200 {
            let j = r.json();
            if j["state"] == "exited" {
                view = Some(j);
                return true;
            }
        }
        false
    });
    assert!(ok, "job {job_id} did not exit");
    view.unwrap()
}

// ---------------------------------------------------------------------------
// D2.3a: hold_project_at_gate
// ---------------------------------------------------------------------------

#[test]
fn gate_holds_project_for_another_task() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({}));
    auto_to_gate(&sb);

    // CLI: another task's autopilot waits with a clear reason (claim::project_busy_reason).
    let res = sb.run(
        &["auto", "--start", "--name", "viec-b", "--", PROJECT, "việc b"],
        Some(by_role(json!({ "lead": [lead_plan_entry()] }))),
    );
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(
        res.stderr.contains("chờ duyệt plan") && res.stderr.contains("hold_project_at_gate"),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(task_count(&sb), 1, "no second task");
    assert_eq!(current_branch(&sb, &repo), "task/sua-app");

    // Dashboard: a new task is refused as busy, the gated task stays approvable.
    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": PROJECT, "request": "việc b", "name": "viec-b", "workflow": "nhanh"}),
    );
    assert_eq!(resp.status, 409, "{}", resp.text());
    let body = resp.json();
    assert_eq!(body["kind"], "busy", "{body}");
    assert!(body["error"].as_str().unwrap().contains("task 1 chờ duyệt plan"), "{body}");
    assert_eq!(task_count(&sb), 1);

    let t = srv.get("/api/tasks/1").json();
    assert_eq!(control(&t, "approve")["enabled"], true, "{}", t["controls"]);
}

#[test]
fn gate_hold_off_keeps_old_behaviour() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({ "hold_project_at_gate": false }));
    auto_to_gate(&sb);

    sb.write_scenario(by_role(json!({ "lead": [lead_plan_entry()] })));
    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": PROJECT, "request": "việc b", "name": "viec-b", "workflow": "nhanh"}),
    );
    assert_eq!(resp.status, 202, "{}", resp.text());
    let job = wait_for_job(&srv, resp.json()["job_id"].as_u64().unwrap());
    assert_eq!(job["exit_code"], 0, "{job}");
    assert_eq!(task_count(&sb), 2);
    assert_eq!(current_branch(&sb, &repo), "task/viec-b");
}

// ---------------------------------------------------------------------------
// D2.3b: auto_checkout_on_approve
// ---------------------------------------------------------------------------

#[test]
fn approve_checks_out_task_branch_when_clean() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({}));
    auto_to_gate(&sb);
    sb.git(&repo, &["checkout", "main"]);

    let srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();
    let ap = control(&t, "approve");
    assert_eq!(ap["enabled"], true, "{ap}");

    // approve_none: record only, but the clean tree is still put on the task branch.
    let rev = t["rev"].as_str().unwrap().to_string();
    let resp = srv.post_json("/api/tasks/1/approve", &json!({ "rev": rev, "next": "none" }));
    assert_eq!(resp.status, 200, "{}", resp.text());
    assert_eq!(current_branch(&sb, &repo), "task/sua-app");
    assert!(approved_commit(&sb, 1).is_some());
}

#[test]
fn approve_auto_continue_checks_out_then_runs() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({}));
    auto_to_gate(&sb);
    sb.git(&repo, &["checkout", "main"]);

    // The driver that starts after the approval must find the task branch.
    sb.write_scenario(by_role(json!({
        "dev": [{"ops": [
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
        ]}],
    })));
    let srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();
    let rev = t["rev"].as_str().unwrap().to_string();
    let resp = srv.post_json("/api/tasks/1/approve", &json!({ "rev": rev }));
    assert_eq!(resp.status, 200, "{}", resp.text());
    let body = resp.json();
    assert!(body["job_id"].as_u64().is_some(), "{body}");
    assert_eq!(current_branch(&sb, &repo), "task/sua-app");
    let job = wait_for_job(&srv, body["job_id"].as_u64().unwrap());
    let roles: Vec<String> = {
        let conn = sb.db();
        let mut stmt = conn.prepare("SELECT role FROM runs ORDER BY id").unwrap();
        let rows = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<String>, _>>()
            .unwrap();
        rows
    };
    assert!(roles.iter().any(|r| r == "dev"), "dev ran on the task branch: {roles:?} {job}");
}

#[test]
fn approve_dirty_tree_refused_with_file_list() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({}));
    auto_to_gate(&sb);
    sb.git(&repo, &["checkout", "main"]);
    std::fs::write(repo.join("ghi-chu.txt"), "nháp\n").unwrap();

    let srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();
    let ap = control(&t, "approve");
    assert_eq!(ap["enabled"], false, "{ap}");
    let reason = ap["reason"].as_str().unwrap();
    assert!(reason.starts_with(MISMATCH), "{reason}");
    assert!(reason.contains("ghi-chu.txt"), "{reason}");

    let rev = t["rev"].as_str().unwrap().to_string();
    let resp = srv.post_json("/api/tasks/1/approve", &json!({ "rev": rev }));
    assert_eq!(resp.status, 409, "{}", resp.text());
    let body = resp.json();
    assert!(body["error"].as_str().unwrap().contains("ghi-chu.txt"), "{body}");
    assert_eq!(current_branch(&sb, &repo), "main");
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(auto_state(&sb, 1).as_deref(), Some("gate"));

    // CLI: the same refusal, nothing checked out or approved.
    let res = sb.run(&["approve", "1"], None);
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(res.stderr.contains("ghi-chu.txt"), "stderr: {}", res.stderr);
    assert_eq!(current_branch(&sb, &repo), "main");
    assert_eq!(approved_commit(&sb, 1), None);
}

#[test]
fn approve_auto_checkout_off_keeps_old_refusal() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({ "auto_checkout_on_approve": false }));
    auto_to_gate(&sb);
    sb.git(&repo, &["checkout", "main"]);

    let srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();
    let ap = control(&t, "approve");
    assert_eq!(ap["enabled"], false, "{ap}");
    assert_eq!(ap["reason"], MISMATCH);
    let rev = t["rev"].as_str().unwrap().to_string();
    let resp = srv.post_json("/api/tasks/1/approve", &json!({ "rev": rev }));
    assert_eq!(resp.status, 409, "{}", resp.text());
    assert_eq!(resp.json()["error"], MISMATCH);
    assert_eq!(current_branch(&sb, &repo), "main");
    assert_eq!(approved_commit(&sb, 1), None);
}

#[test]
fn cli_approve_checks_out_when_clean() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({}));
    auto_to_gate(&sb);
    sb.git(&repo, &["checkout", "main"]);

    let res = sb.run(&["approve", "1", "--no-continue"], None);
    assert_ok(&res, "approve --no-continue");
    assert_eq!(current_branch(&sb, &repo), "task/sua-app");
    assert!(approved_commit(&sb, 1).is_some());
}

// ---------------------------------------------------------------------------
// D2.1: base_branch
// ---------------------------------------------------------------------------

#[test]
fn base_branch_default_follows_current_branch() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({ "workflow": "nhanh" }));
    sb.git(&repo, &["checkout", "-b", "trunk"]);
    sb.write_scenario(by_role(json!({ "lead": [lead_plan_entry()] })));

    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": PROJECT, "request": "sửa app", "name": "sua-app"}),
    );
    assert_eq!(resp.status, 202, "{}", resp.text());
    let job = wait_for_job(&srv, resp.json()["job_id"].as_u64().unwrap());
    assert_eq!(job["exit_code"], 0, "{job}");
    assert_eq!(base_branch_of(&sb, 1).as_deref(), Some("trunk"));
}

#[test]
fn base_branch_config_is_the_start_point() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({ "workflow": "nhanh", "base_branch": "develop" }));
    sb.git(&repo, &["checkout", "-b", "develop"]);
    std::fs::write(repo.join("dev.txt"), "d\n").unwrap();
    sb.git(&repo, &["add", "dev.txt"]);
    sb.git(&repo, &["commit", "-m", "develop only"]);
    let develop_tip = sb.git(&repo, &["rev-parse", "develop"]).trim().to_string();
    sb.git(&repo, &["checkout", "main"]);

    let res = sb.run(
        &["run", "lead", "--name", "sua-app", "--", PROJECT, "sửa app"],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead");
    assert_eq!(current_branch(&sb, &repo), "task/sua-app");
    let parent = sb.git(&repo, &["rev-parse", "HEAD~1"]).trim().to_string();
    assert_eq!(parent, develop_tip, "the task branch starts from develop");
    assert_eq!(base_branch_of(&sb, 1).as_deref(), Some("develop"));
}

#[test]
fn base_branch_missing_is_refused() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({ "workflow": "nhanh", "base_branch": "khong-co" }));

    let res = sb.run(
        &["run", "lead", "--name", "sua-app", "--", PROJECT, "sửa app"],
        Some(lead_plan_entry()),
    );
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(res.stderr.contains("khong-co"), "stderr: {}", res.stderr);
    assert_eq!(current_branch(&sb, &repo), "main");

    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": PROJECT, "request": "sửa app", "name": "sua-app"}),
    );
    assert_eq!(resp.status, 409, "{}", resp.text());
    assert_eq!(resp.json()["kind"], "base_missing");
}

// ---------------------------------------------------------------------------
// Dashboard endpoint
// ---------------------------------------------------------------------------

#[test]
fn workspace_endpoint_validates_and_stores() {
    let sb = Sandbox::new();
    let _repo = setup(&sb, json!({}));
    let mut srv = sb.serve(&[]);

    let j = srv.get("/api/projects").json();
    let p = &j["projects"][0];
    assert!(p["base_branch"].is_null(), "{p}");
    assert_eq!(p["hold_project_at_gate"], true);
    assert_eq!(p["auto_checkout_on_approve"], true);

    for bad in ["a b", "a..b", "-x", "x.lock"] {
        let r = srv.put_json(
            &format!("/api/projects/{PROJECT}/workspace"),
            &json!({ "base_branch": bad }),
        );
        assert_eq!(r.status, 400, "{bad}");
    }
    let r = srv.put_json("/api/projects/nope/workspace", &json!({ "base_branch": "develop" }));
    assert_eq!(r.status, 404);
    let r = srv.put_json(
        &format!("/api/projects/{PROJECT}/workspace"),
        &json!({ "base_branch": "develop", "etag": "wrong" }),
    );
    assert_eq!(r.status, 409);

    let r = srv.put_json(
        &format!("/api/projects/{PROJECT}/workspace"),
        &json!({
            "base_branch": " develop ",
            "hold_project_at_gate": false,
            "auto_checkout_on_approve": false,
            "etag": j["etag"],
        }),
    );
    assert_eq!(r.status, 200, "{}", r.text());
    let entry = sb.read_map()["projects"][0].clone();
    assert_eq!(entry["base_branch"], "develop");
    assert_eq!(entry["hold_project_at_gate"], false);
    assert_eq!(entry["auto_checkout_on_approve"], false);
    let p = srv.get("/api/projects").json()["projects"][0].clone();
    assert_eq!(p["base_branch"], "develop");
    assert_eq!(p["hold_project_at_gate"], false);

    // Back to the defaults: the keys go away.
    let r = srv.put_json(
        &format!("/api/projects/{PROJECT}/workspace"),
        &json!({ "base_branch": "", "hold_project_at_gate": true, "auto_checkout_on_approve": true }),
    );
    assert_eq!(r.status, 200);
    let entry = sb.read_map()["projects"][0].clone();
    for key in ["base_branch", "hold_project_at_gate", "auto_checkout_on_approve"] {
        assert!(entry.get(key).is_none(), "{key}: {entry}");
    }

    let exit = srv.stop(libc::SIGINT, Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}
