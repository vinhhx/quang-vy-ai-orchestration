//! Phase 6 plan 16: the dashboard side of the human engine (D-19, D-20, D-21). A waiting or
//! rejected person step shows up in the task payload, `Xong` and `Trả lại` are task actions that
//! run the 06-12 CLI as jobs, nothing else starts while a person works, and the pipeline and the
//! overview name who is working. The remote is a local bare repository and the "staff" is a second
//! clone; Claude is the fake one and is never called.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::teams::TaskTeam;
use common::{wait_until, Sandbox, Serve};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers (the bare remote / staff clone ones are copies of tests/human_engine.rs)
// ---------------------------------------------------------------------------

fn lead_plan_entry() -> Value {
    protocol::lead_scenario(&[("src/a.txt", "a()", "đổi nội dung", "demo")])
}

fn bare_remote(sb: &Sandbox) -> PathBuf {
    let dir = sb.root.join("remote.git");
    std::fs::create_dir_all(&dir).unwrap();
    sb.git(&dir, &["init", "--bare", "-b", "main"]);
    dir
}

/// The staff's machine: a clone of `remote` that checks out `branch`, applies `edits`, commits
/// with `message` and pushes. Returns the clone path.
fn staff_clone_push(
    sb: &Sandbox,
    remote: &std::path::Path,
    branch: &str,
    edits: &[(&str, &str)],
    message: &str,
) -> PathBuf {
    let clone = sb.root.join("staff");
    if !clone.join(".git").exists() {
        sb.git(
            &sb.root,
            &["clone", remote.to_str().unwrap(), clone.to_str().unwrap()],
        );
    }
    sb.git(&clone, &["fetch", "origin"]);
    sb.git(
        &clone,
        &["checkout", "-B", branch, &format!("origin/{branch}")],
    );
    for (rel, content) in edits {
        let p = clone.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, content).unwrap();
    }
    sb.git(&clone, &["add", "-A"]);
    sb.git(&clone, &["commit", "-m", message]);
    sb.git(&clone, &["push", "origin", branch]);
    clone
}

#[allow(dead_code)]
struct World {
    repo: PathBuf,
    remote: Option<PathBuf>,
}

/// Repo `web` (optionally with a bare `origin`), staff `ha` (`Hà`, notes `Zalo 09xx`) and team
/// `doi-nguoi` (Dev = `người:ha`). The map entry keeps the manual `nhanh` workflow. No task yet.
fn setup_base(sb: &Sandbox, with_origin: bool) -> World {
    let remote = with_origin.then(|| bare_remote(sb));
    let repo = sb.make_repo(
        "web",
        &[("src/a.txt", "a\n")],
        None,
        remote.as_ref().map(|r| r.to_str().unwrap()),
    );
    if remote.is_some() {
        sb.git(&repo, &["push", "-u", "origin", "main"]);
    }
    let entry = sb.project_entry(&repo, "web", "work");
    sb.write_map(json!([entry]), &[]);
    sb.write_staff(json!({"ha": {"display_name": "Hà", "notes": "Zalo 09xx"}}));
    sb.write_teams(json!({
        "doi-nguoi": {"description": "Dev do Hà làm", "roles": {"dev": "người:ha"}}
    }));
    World { repo, remote }
}

/// `setup_base` plus task 1 (`task/sua-app`) created by `run lead --team doi-nguoi`.
fn setup(sb: &Sandbox, with_origin: bool) -> World {
    let world = setup_base(sb, with_origin);
    let res = sb.run(
        &[
            "run",
            "lead",
            "--team",
            "doi-nguoi",
            "--name",
            "sua-app",
            "--",
            "web",
            "sửa app",
        ],
        Some(json!({ "by_role": { "lead": [lead_plan_entry()] } })),
    );
    assert_eq!(
        res.code, 0,
        "run lead failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    world
}

fn run_dev_waits(sb: &Sandbox) {
    let res = sb.run(&["run", "dev", "--", "web"], None);
    assert_eq!(
        res.code, 6,
        "run dev should wait: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

fn team_json(sb: &Sandbox) -> Option<String> {
    sb.db()
        .query_row("SELECT team_json FROM tasks WHERE id = 1", [], |r| {
            r.get::<_, Option<String>>(0)
        })
        .unwrap()
}

fn task_team(sb: &Sandbox) -> TaskTeam {
    TaskTeam::parse(team_json(sb).as_deref()).expect("task 1 has a parsable team_json")
}

/// Rewrites the wait marker of task 1 (the marker is the source of truth the dashboard reads).
fn edit_wait(sb: &Sandbox, edit: impl FnOnce(&mut agent_brain::teams::HumanWait)) {
    let mut team = task_team(sb);
    edit(team.wait.as_mut().expect("wait marker"));
    sb.db()
        .execute(
            "UPDATE tasks SET team_json = ?1 WHERE id = 1",
            rusqlite::params![team.to_json()],
        )
        .unwrap();
}

fn human_run_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE engine = 'người'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

fn run_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap()
}

fn task_detail(srv: &Serve) -> Value {
    let r = srv.get("/api/tasks/1");
    assert_eq!(r.status, 200, "GET /api/tasks/1: {}", r.text());
    r.json()
}

fn action<'a>(detail: &'a Value, id: &str) -> Option<&'a Value> {
    detail["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
}

fn wait_for_job(srv: &Serve, job_id: u64) -> Value {
    let mut job_view = None;
    let ok = wait_until(Duration::from_secs(60), || {
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

// ---------------------------------------------------------------------------
// Task 1 (tracer): marker -> payload -> Xong action -> job -> human run -> payload cleared
// ---------------------------------------------------------------------------

#[test]
fn human_payload_and_done_job() {
    let sb = Sandbox::new();
    let w = setup(&sb, true);
    let remote = w.remote.clone().unwrap();
    run_dev_waits(&sb);
    let wait = task_team(&sb).wait.expect("marker");
    let srv = sb.serve(&[]);

    let d = task_detail(&srv);
    let h = &d["human"];
    assert_eq!(h["mode"], "waiting");
    assert_eq!(h["staff_id"], "ha");
    assert_eq!(h["staff_name"], "Hà");
    assert_eq!(h["notes"], "Zalo 09xx");
    assert_eq!(h["step"], "implement");
    assert_eq!(h["step_name"], "Code");
    assert!(h["round"].is_null() || h["round"].is_i64(), "{h}");
    assert_eq!(h["branch"], "task/sua-app");
    assert_eq!(h["since_ms"], wait.since_ms);
    assert!(h["waiting_ms"].as_i64().unwrap() >= 0);
    assert_eq!(h["report_paths"], json!([]));
    assert_eq!(h["push"]["ok"], true);
    assert_eq!(h["push"]["command"], "git push -u origin task/sua-app");
    assert!(h["push"]["error"].is_null());

    let done = action(&d, "human_done").expect("human_done action");
    assert_eq!(done["label"], "Xong");
    assert_eq!(done["enabled"], true, "{done}");
    let ret = action(&d, "human_return").expect("human_return action");
    assert_eq!(ret["label"], "Trả lại");
    assert_eq!(ret["enabled"], false);
    assert_eq!(ret["reason"], "Chỉ trả lại khi kết quả của Hà chưa đạt.");
    assert_eq!(d["banner"], "Chờ Hà làm");
    assert_eq!(d["group"], "running");

    // The person pushes, the user presses Xong.
    staff_clone_push(
        &sb,
        &remote,
        "task/sua-app",
        &[("src/a.txt", "a do Hà sửa\n")],
        "sua-app: Hà làm",
    );
    let runs_before = run_count(&sb);
    let resp = srv.post_json(
        "/api/tasks/1/human/done",
        &json!({ "rev": d["rev"].as_str().unwrap() }),
    );
    assert_eq!(resp.status, 202, "body: {}", resp.text());
    let job_id = resp.json()["job_id"].as_u64().expect("job_id");
    let job = wait_for_job(&srv, job_id);
    assert_eq!(
        job["exit_code"],
        0,
        "job output: {}",
        job["output"].to_string()
    );

    assert_eq!(human_run_count(&sb), 1, "a finished người run is recorded");
    assert_eq!(run_count(&sb), runs_before + 1);
    let d = task_detail(&srv);
    assert!(d["human"].is_null(), "payload cleared: {}", d["human"]);
    assert!(action(&d, "human_done").is_none());
    assert!(action(&d, "human_return").is_none());
}

#[test]
fn human_action_refusals() {
    let sb = Sandbox::new();
    let _w = setup(&sb, true);
    let srv = sb.serve(&[]);

    // No marker: neither route applies.
    let d = task_detail(&srv);
    assert!(d["human"].is_null());
    assert!(action(&d, "human_done").is_none());
    for route in ["done", "return"] {
        let r = srv.post_json(&format!("/api/tasks/1/human/{route}"), &json!({}));
        assert_eq!(r.status, 409, "{route}: {}", r.text());
        assert_eq!(
            r.json()["error"],
            "Hành động không áp dụng cho trạng thái hiện tại"
        );
    }
    assert_eq!(human_run_count(&sb), 0);

    // With a marker a stale rev is refused and nothing starts.
    run_dev_waits(&sb);
    let marker = team_json(&sb);
    let r = srv.post_json(
        "/api/tasks/1/human/done",
        &json!({ "rev": "0000000000000000" }),
    );
    assert_eq!(r.status, 409, "{}", r.text());
    assert_eq!(r.json()["kind"], "stale");

    // `return` is refused in waiting mode with the server text.
    let r = srv.post_json("/api/tasks/1/human/return", &json!({}));
    assert_eq!(r.status, 409, "{}", r.text());
    assert_eq!(
        r.json()["error"],
        "Chỉ trả lại khi kết quả của Hà chưa đạt."
    );

    // Unknown task.
    let r = srv.post_json("/api/tasks/99/human/done", &json!({}));
    assert_eq!(r.status, 404, "{}", r.text());

    assert_eq!(team_json(&sb), marker, "refusals leave the marker alone");
    assert_eq!(human_run_count(&sb), 0);
}
