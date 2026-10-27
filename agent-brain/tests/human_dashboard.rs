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
    setup_workflow(sb, with_origin, None)
}

/// As `setup`, optionally creating the task with `--workflow <name>` (e.g. `mac-dinh`, whose
/// pipeline has the `fix` card).
fn setup_workflow(sb: &Sandbox, with_origin: bool, workflow: Option<&str>) -> World {
    let world = setup_base(sb, with_origin);
    let mut args = vec!["run", "lead", "--team", "doi-nguoi", "--name", "sua-app"];
    if let Some(w) = workflow {
        args.extend(["--workflow", w]);
    }
    args.extend(["--", "web", "sửa app"]);
    let res = sb.run(
        &args,
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

// ---------------------------------------------------------------------------
// Task 2: rejected mode, Trả lại, strip gating, autopilot waiting_human
// ---------------------------------------------------------------------------

/// Turns the waiting marker of task 1 into a rejected fix step, as `run dev --fix qa` leaves it.
fn make_rejected(sb: &Sandbox) {
    edit_wait(sb, |w| {
        w.mode = "rejected".to_string();
        w.stage = "fix".to_string();
        w.fix_for = Some("qa".to_string());
        w.round = Some(1);
        w.report_paths = vec![".qa-report.md".to_string()];
    });
}

#[test]
fn rejected_mode_and_return() {
    let sb = Sandbox::new();
    let _w = setup(&sb, true);
    run_dev_waits(&sb);
    make_rejected(&sb);
    let srv = sb.serve(&[]);

    let d = task_detail(&srv);
    assert_eq!(d["human"]["mode"], "rejected");
    assert_eq!(d["human"]["step"], "fix");
    assert_eq!(d["human"]["step_name"], "Sửa");
    assert_eq!(d["human"]["round"], 1);
    assert_eq!(d["human"]["report_paths"], json!([".qa-report.md"]));
    let done = action(&d, "human_done").expect("human_done");
    assert_eq!(done["enabled"], false);
    assert_eq!(
        done["reason"],
        "Bấm Trả lại trước: kết quả lần trước của Hà chưa đạt."
    );
    let back = action(&d, "human_return").expect("human_return");
    assert_eq!(back["enabled"], true, "{back}");
    assert_eq!(d["banner"], "Chờ bạn · trả lại cho Hà");
    assert_eq!(d["group"], "waiting");

    // Xong is refused with the server text and starts nothing.
    let r = srv.post_json(
        "/api/tasks/1/human/done",
        &json!({ "rev": d["rev"].as_str().unwrap() }),
    );
    assert_eq!(r.status, 409, "{}", r.text());
    assert_eq!(
        r.json()["error"],
        "Bấm Trả lại trước: kết quả lần trước của Hà chưa đạt."
    );
    assert_eq!(human_run_count(&sb), 0);

    // Trả lại runs the CLI as a job; the marker goes back to `waiting`.
    let since = task_team(&sb).wait.unwrap().since_ms;
    std::thread::sleep(Duration::from_millis(20));
    let r = srv.post_json(
        "/api/tasks/1/human/return",
        &json!({ "rev": d["rev"].as_str().unwrap() }),
    );
    assert_eq!(r.status, 202, "{}", r.text());
    let job = wait_for_job(&srv, r.json()["job_id"].as_u64().unwrap());
    assert_eq!(job["exit_code"], 0, "job output: {}", job["output"]);
    let wait = task_team(&sb).wait.expect("marker stays");
    assert_eq!(wait.mode, "waiting");
    assert!(wait.since_ms > since);

    let d = task_detail(&srv);
    assert_eq!(d["human"]["mode"], "waiting");
    assert_eq!(d["banner"], "Chờ Hà làm");
    assert_eq!(d["group"], "running");
    assert_eq!(action(&d, "human_done").unwrap()["enabled"], true);
    assert_eq!(action(&d, "human_return").unwrap()["enabled"], false);
}

#[test]
fn xong_refused_off_the_task_branch() {
    let sb = Sandbox::new();
    let w = setup(&sb, true);
    run_dev_waits(&sb);
    sb.git(&w.repo, &["checkout", "main"]);
    let srv = sb.serve(&[]);

    let d = task_detail(&srv);
    let done = action(&d, "human_done").expect("human_done");
    assert_eq!(done["enabled"], false);
    assert_eq!(
        done["reason"],
        "Không ở nhánh task 'task/sua-app' (nhánh hiện tại: 'main') — không đoán"
    );
    let r = srv.post_json("/api/tasks/1/human/done", &json!({}));
    assert_eq!(r.status, 409, "{}", r.text());
    assert_eq!(human_run_count(&sb), 0);
}

#[test]
fn strip_disabled_while_waiting() {
    let sb = Sandbox::new();
    let _w = setup(&sb, true);
    run_dev_waits(&sb);
    let srv = sb.serve(&[]);
    let waiting = "Đang chờ Hà làm — bấm Xong khi họ báo xong.";
    let rejected = "Kết quả của Hà chưa đạt — bấm Trả lại để Hà làm lại.";

    // planned: Chạy Dev / Chạy lại Lead are refused, in the payload and on the routes.
    let d = task_detail(&srv);
    for id in ["run_dev", "relead"] {
        let a = action(&d, id).unwrap_or_else(|| panic!("{id} present"));
        assert_eq!(a["enabled"], false, "{id}");
        assert_eq!(a["reason"], waiting, "{id}");
    }
    for route in ["dev", "relead"] {
        let r = srv.post_json(&format!("/api/tasks/1/{route}"), &json!({}));
        assert_eq!(r.status, 409, "{route}: {}", r.text());
        assert_eq!(r.json()["error"], waiting, "{route}");
    }
    // Đổi đội hình stays refused while the person works (06-10).
    let r = srv.post_json("/api/tasks/1/team", &json!({ "team": "mac-dinh" }));
    assert_eq!(r.status, 409, "{}", r.text());
    assert_eq!(
        r.json()["error"],
        "Đang chờ Hà làm — chưa đổi được đội hình."
    );

    // dev_done: Chạy QA is refused the same way.
    sb.db()
        .execute("UPDATE tasks SET status = 'dev_done' WHERE id = 1", [])
        .unwrap();
    let d = task_detail(&srv);
    let qa = action(&d, "run_qa").expect("run_qa present");
    assert_eq!(qa["enabled"], false);
    assert_eq!(qa["reason"], waiting);
    let r = srv.post_json("/api/tasks/1/qa", &json!({}));
    assert_eq!(r.status, 409, "{}", r.text());
    assert_eq!(r.json()["error"], waiting);

    // A rejected marker has its own text.
    make_rejected(&sb);
    sb.db()
        .execute("UPDATE tasks SET status = 'qa_fix' WHERE id = 1", [])
        .unwrap();
    let d = task_detail(&srv);
    for id in ["run_dev", "relead"] {
        let a = action(&d, id).unwrap_or_else(|| panic!("{id} present"));
        assert_eq!(a["enabled"], false, "{id}");
        assert_eq!(a["reason"], rejected, "{id}");
    }
    let r = srv.post_json("/api/tasks/1/dev", &json!({}));
    assert_eq!(r.status, 409, "{}", r.text());
    assert_eq!(r.json()["error"], rejected);
    assert_eq!(human_run_count(&sb), 0);
}

fn review_report(verdict: &str, blocking: i64, advisory: i64) -> String {
    let mut body = format!(
        "---\nverdict: {verdict}\nblocking: {blocking}\nadvisory: {advisory}\nfiles: []\n---\n# Review: sửa app\n## Mục chặn (blocking)\n"
    );
    for i in 0..blocking {
        body.push_str(&format!("- mục chặn {}\n", i + 1));
    }
    body.push_str("## Góp ý (advisory)\n");
    for i in 0..advisory {
        body.push_str(&format!("- góp ý {}\n", i + 1));
    }
    body
}

fn plan_review_entry(verdict: &str, blocking: i64, advisory: i64) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan-review.md",
        "content": review_report(verdict, blocking, advisory),
    }]})
}

#[test]
fn autopilot_waiting_human_state() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb);
    let repo = sb.make_repo(
        "web",
        &[("src/a.txt", "a\n")],
        None,
        Some(remote.to_str().unwrap()),
    );
    sb.git(&repo, &["push", "-u", "origin", "main"]);
    let mut entry = sb.project_entry(&repo, "web", "work");
    entry.as_object_mut().unwrap().remove("workflow");
    sb.write_map(json!([entry]), &[]);
    sb.write_staff(json!({"ha": {"display_name": "Hà"}}));
    sb.write_teams(json!({
        "doi-nguoi": {"description": "Dev do Hà làm", "roles": {"dev": "người:ha"}}
    }));

    // mac-dinh autopilot: Lead, plan review approves, stops at the gate; approve starts the
    // driver, which reaches the person's Dev step and releases the task as `waiting_human`.
    let sc = json!({ "by_role": {
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry("approve", 0, 2)],
    }});
    let res = sb.run(
        &[
            "auto",
            "--start",
            "--name",
            "sua-app",
            "--team",
            "doi-nguoi",
            "--",
            "web",
            "sửa app",
        ],
        Some(sc),
    );
    assert_eq!(res.code, 0, "auto --start: {}\n{}", res.stderr, res.stdout);
    let res = sb.run(&["approve", "1"], None);
    assert_eq!(res.code, 0, "approve 1: {}\n{}", res.stderr, res.stdout);
    let state: Option<String> = sb
        .db()
        .query_row("SELECT auto_state FROM tasks WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(state.as_deref(), Some("waiting_human"));

    let srv = sb.serve(&[]);
    let d = task_detail(&srv);
    assert_eq!(d["auto"]["state"], "waiting_human");
    assert_eq!(d["banner"], "Chờ Hà làm");
    assert_eq!(d["group"], "running");
    assert_eq!(d["human"]["mode"], "waiting");
    let start = d["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "auto_start")
        .expect("auto_start control");
    assert_eq!(start["enabled"], false);
    assert_eq!(
        start["reason"],
        "Đang chờ Hà làm — bấm Xong khi họ báo xong."
    );
    // Autopilot does not start from the dashboard while the person works.
    let r = srv.post_json("/api/tasks/1/auto", &json!({}));
    assert_eq!(r.status, 409, "{}", r.text());
    assert_eq!(human_run_count(&sb), 0);
}

// ---------------------------------------------------------------------------
// Task 3: pipeline synthesized row and card, overview banners
// ---------------------------------------------------------------------------

fn pipeline(srv: &Serve) -> Value {
    let r = srv.get("/api/tasks/1/pipeline");
    assert_eq!(r.status, 200, "GET pipeline: {}", r.text());
    r.json()
}

/// Ledger rows of the pipeline payload, oldest group first.
fn ledger_rows(p: &Value) -> Vec<Value> {
    p["ledger"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["rows"].as_array().unwrap().clone())
        .collect()
}

fn flow_card(p: &Value, key: &str) -> Value {
    p["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["cards"].as_array().unwrap().clone())
        .find(|c| c["key"] == key)
        .unwrap_or_else(|| panic!("flow card {key} in {}", p["groups"]))
}

#[test]
fn pipeline_synthesized_waiting_row() {
    let sb = Sandbox::new();
    let w = setup(&sb, true);
    let remote = w.remote.clone().unwrap();
    let srv = sb.serve(&[]);

    // Before the wait: only the Lead row; the code card has no run.
    let before = pipeline(&srv);
    assert_eq!(ledger_rows(&before).len(), 1);
    assert_eq!(before["ledger"]["total"]["runs"], 1);

    run_dev_waits(&sb);
    let wait = task_team(&sb).wait.expect("marker");
    let p = pipeline(&srv);
    let rows = ledger_rows(&p);
    assert_eq!(rows.len(), 2, "Lead row plus the synthesized one: {rows:?}");
    let row = rows
        .iter()
        .find(|r| r["run_id"] == 0)
        .expect("run_id 0 row");
    assert_eq!(row["human"], true);
    assert_eq!(row["engine"], "người:ha");
    assert_eq!(row["state"], "cho_nguoi");
    assert_eq!(row["verdict"], "chờ người");
    assert_eq!(row["started_at"], wait.since_ms);
    assert!(row["duration_ms"].as_i64().unwrap() >= 0);
    assert_eq!(row["running"], false);
    assert_eq!(row["label"], "Vòng 1 · code · người Hà");
    assert_eq!(row["role"], "dev");
    assert_eq!(row["stage"], "implement");
    // Not a run: it never counts in the totals, and no run row exists for it.
    assert_eq!(p["ledger"]["total"]["runs"], 1);
    assert_eq!(p["ledger"]["total"]["human_runs"], 0);
    assert_eq!(human_run_count(&sb), 0);

    let code = flow_card(&p, "code");
    assert_eq!(code["state"], "cho_nguoi");
    assert_eq!(code["state_word"], "chờ người");
    assert_eq!(code["result"], "Chờ Hà làm");
    assert_eq!(code["engine"], "người:ha");
    assert_eq!(code["engine_planned"], false);
    assert_eq!(p["current_card"], "code");

    // After Xong the person's work is a real run and the synthesized row is gone.
    staff_clone_push(
        &sb,
        &remote,
        "task/sua-app",
        &[("src/a.txt", "a do Hà sửa\n")],
        "sua-app: Hà làm",
    );
    let r = srv.post_json("/api/tasks/1/human/done", &json!({}));
    assert_eq!(r.status, 202, "{}", r.text());
    let job = wait_for_job(&srv, r.json()["job_id"].as_u64().unwrap());
    assert_eq!(job["exit_code"], 0, "job output: {}", job["output"]);
    let p = pipeline(&srv);
    let rows = ledger_rows(&p);
    assert!(rows.iter().all(|r| r["run_id"] != 0), "{rows:?}");
    let human_row = rows.iter().find(|r| r["human"] == true).expect("human run");
    assert_eq!(human_row["engine"], "người:ha");
    assert_eq!(p["ledger"]["total"]["human_runs"], 1);
    assert_ne!(flow_card(&p, "code")["state"], "cho_nguoi");
}

#[test]
fn pipeline_fix_wait_on_fix_card() {
    let sb = Sandbox::new();
    let _w = setup_workflow(&sb, true, Some("mac-dinh"));
    run_dev_waits(&sb);
    make_rejected(&sb);
    let srv = sb.serve(&[]);

    let p = pipeline(&srv);
    let row = ledger_rows(&p)
        .into_iter()
        .find(|r| r["run_id"] == 0)
        .expect("synthesized row");
    assert_eq!(row["kind"], "sửa theo QA");
    assert_eq!(row["stage"], "fix");
    assert_eq!(row["human"], true);
    assert_eq!(row["state"], "cho_nguoi");
    assert_eq!(row["label"], "Vòng 1 · sửa theo QA · người Hà");
    let fix = flow_card(&p, "fix");
    assert_eq!(fix["state"], "cho_nguoi");
    assert_eq!(fix["result"], "Chờ Hà làm");
    assert_eq!(fix["engine"], "người:ha");
    assert_ne!(flow_card(&p, "code")["state"], "cho_nguoi");
    assert_eq!(p["current_card"], "fix");
    assert_eq!(p["ledger"]["total"]["runs"], 1);
}

fn overview_task(srv: &Serve) -> Value {
    let r = srv.get("/api/tasks");
    assert_eq!(r.status, 200, "GET /api/tasks: {}", r.text());
    r.json()["tasks"][0].clone()
}

#[test]
fn overview_human_banners() {
    let sb = Sandbox::new();
    let _w = setup(&sb, true);
    let srv = sb.serve(&[]);

    // No marker: nothing about a person, no quota reason.
    let t = overview_task(&srv);
    assert!(t["human_mode"].is_null());
    assert!(t["human_name"].is_null());
    assert!(t["quota_detail"].is_null());

    run_dev_waits(&sb);
    let t = overview_task(&srv);
    assert_eq!(t["banner"], "Chờ Hà làm");
    assert_eq!(t["group"], "running");
    assert_eq!(t["human_mode"], "waiting");
    assert_eq!(t["human_name"], "Hà");
    assert!(t["quota_detail"].is_null());

    make_rejected(&sb);
    let t = overview_task(&srv);
    assert_eq!(t["banner"], "Chờ bạn · trả lại cho Hà");
    assert_eq!(t["group"], "waiting");
    assert_eq!(t["human_mode"], "rejected");
    assert_eq!(t["human_name"], "Hà");

    // The overview banner and group match the task detail.
    let d = task_detail(&srv);
    assert_eq!(d["banner"], t["banner"]);
    assert_eq!(d["group"], t["group"]);

    // A manual task whose latest run stopped on quota carries the reason line.
    let sb2 = Sandbox::new();
    let _w2 = setup(&sb2, true);
    sb2.db()
        .execute(
            "UPDATE runs SET status = 'failed', outcome = 'quota' WHERE id = 1",
            [],
        )
        .unwrap();
    let srv2 = sb2.serve(&[]);
    let t = overview_task(&srv2);
    assert_eq!(
        t["quota_detail"],
        "Run 1 (lead) trên engine claude dừng vì hết quota hoặc bị giới hạn tốc độ. Agent-brain không tự đổi engine."
    );
    assert!(t["human_mode"].is_null());
    assert_eq!(t["banner"], "Chờ bạn · hết quota");
}
