mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{wait_until, RunOut, Sandbox};
use protocol::{lead_scenario, setup_project};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn wait_for_job(srv: &common::Serve, job_id: u64) -> serde_json::Value {
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

fn lead_plan_entry() -> Value {
    lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])
}

fn by_role(roles: Value) -> Value {
    json!({ "by_role": roles })
}

/// Repo `web-ban-hang` với entry không có khóa `workflow` (mặc định server: mac-dinh).
fn setup_auto_project(sb: &Sandbox, profile: &str) -> PathBuf {
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", profile);
    entry.as_object_mut().unwrap().remove("workflow");
    sb.write_map(json!([entry]), &[]);
    repo
}

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

/// `auto --start` trên quy trình mặc định (mac-dinh): Lead, review plan duyệt, dừng ở cổng.
fn auto_to_gate(sb: &Sandbox) -> PathBuf {
    let repo = setup_auto_project(sb, "work");
    let sc = by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry("approve", 0, 2)],
    }));
    let res = sb.run(
        &[
            "auto",
            "--start",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(sc),
    );
    assert_ok(&res, "auto --start");
    repo
}

struct SeedRun {
    role: &'static str,
    status: &'static str,
    outcome: Option<&'static str>,
    started_at: i64,
    ended_at: Option<i64>,
}

fn seed_task(
    sb: &Sandbox,
    repo: &Path,
    branch: &str,
    base_commit: Option<&str>,
    runs: &[SeedRun],
) -> i64 {
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let project = agent_brain::projects::Project {
        id: "web-ban-hang".to_string(),
        path: repo.to_string_lossy().to_string(),
        name_vi: "Web bán hàng".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    agent_brain::db::sync_projects(&conn, &[project]).unwrap();
    let pid = agent_brain::db::project_row_id(&conn, "web-ban-hang").unwrap();
    let task_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "seed-task",
        "Seed task",
        branch,
        Some("main"),
        base_commit,
    )
    .unwrap();

    for sr in runs {
        let new_run = agent_brain::db::NewRun {
            task_id: Some(task_id),
            project_id: pid,
            role: sr.role.to_string(),
            profile: "work".to_string(),
            model: Some("sonnet".to_string()),
            pid: 1000,
            pgid: 1000,
            pid_start: 1000,
            base_commit: None,
            run_dir: None,
        };
        let r_id = agent_brain::db::insert_run(&conn, &new_run).unwrap();
        let finish = agent_brain::db::RunFinish {
            status: sr.status.to_string(),
            exit_code: Some(0),
            session_id: None,
            usage: agent_brain::stream::Usage::default(),
            usage_source: "result".to_string(),
            cost_usd: 1.0,
            num_turns: Some(5),
            assistant_turns: Some(5),
            turns_before_first_edit: None,
            reads_outside_plan: None,
            permission_denials: 0,
            error: None,
        };
        agent_brain::db::finish_run(&conn, r_id, &finish).unwrap();
        conn.execute(
            "UPDATE runs SET started_at = ?1, ended_at = ?2, outcome = ?3 WHERE id = ?4",
            rusqlite::params![sr.started_at, sr.ended_at, sr.outcome, r_id],
        )
        .unwrap();
    }
    task_id
}

fn lead_seed_run() -> SeedRun {
    let t0 = 1_700_000_000_000i64;
    SeedRun {
        role: "lead",
        status: "succeeded",
        outcome: None,
        started_at: t0,
        ended_at: Some(t0 + 60_000),
    }
}

fn action<'a>(task: &'a Value, id: &str) -> &'a Value {
    task["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
        .unwrap_or_else(|| panic!("action {id} missing in {}", task["actions"]))
}

// ---------------------------------------------------------------------------
// Task 1: pipeline flow over HTTP
// ---------------------------------------------------------------------------

#[test]
fn pipeline_flow_payload() {
    let sb = Sandbox::new();
    let _repo = auto_to_gate(&sb);

    let srv = sb.serve(&[]);
    let resp = srv.get("/api/tasks/1/pipeline");
    assert_eq!(resp.status, 200, "{}", resp.text());
    let p = resp.json();

    assert_eq!(p["workflow"]["name"], "mac-dinh");

    let g0 = &p["groups"][0];
    assert_eq!(g0["key"], "plan");
    let cards0 = g0["cards"].as_array().unwrap();
    assert_eq!(cards0.len(), 2);
    assert_eq!(cards0[0]["key"], "plan");
    assert_eq!(cards0[0]["state"], "xong");
    assert_eq!(cards0[1]["key"], "plan_review");
    assert_eq!(cards0[1]["state"], "duyet");
    assert_eq!(cards0[1]["result"], "duyệt");
    assert_eq!(cards0[1]["counts"]["blocking"], 0);
    assert_eq!(cards0[1]["counts"]["advisory"], 2);
    assert_eq!(g0["round"], 1);
    assert_eq!(g0["cap"], 3);

    let g1 = &p["groups"][1];
    assert_eq!(g1["cards"][1]["key"], "code_review");
    assert_eq!(g1["cards"][1]["state"], "cho");

    assert_eq!(p["gate"]["state"], "cho_ban");
    assert_eq!(p["gate"]["note_kind"], "auto_waiting");
    assert_eq!(p["current_card"], "ban");

    assert_eq!(p["budget"]["limit_minutes"], 60);
    assert_eq!(p["budget"]["limit_usd"].as_f64(), Some(20.0));
    assert_eq!(p["budget"]["level"], "ok");

    // 05.1 keys stay
    assert!(p["cards"].is_object());
    assert!(p["cards"]["ban"].is_object());
    assert!(p["ledger"].is_object());
}

#[test]
fn pipeline_legacy_task() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    let task_id = seed_task(&sb, &repo, "main", None, &[lead_seed_run()]);

    let srv = sb.serve(&[]);
    let resp = srv.get(&format!("/api/tasks/{task_id}/pipeline"));
    assert_eq!(resp.status, 200, "{}", resp.text());
    let p = resp.json();

    assert!(p["workflow"].is_null());
    assert_eq!(p["gate"]["derived"], true);
    let groups = p["groups"].as_array().unwrap();
    assert!(!groups.is_empty());
    for g in groups {
        assert_eq!(g["cards"].as_array().unwrap().len(), 1, "group {g}");
    }
}

#[test]
fn wr01_old_plan_commit_ignored() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    // main already carries an old "seed-task: plan" commit
    std::fs::write(repo.join("notes.md"), "cũ\n").unwrap();
    sb.git(&repo, &["add", "notes.md"]);
    sb.git(&repo, &["commit", "-m", "seed-task: plan"]);
    let head = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    sb.git(&repo, &["branch", "task/seed-task"]);

    let task_id = seed_task(
        &sb,
        &repo,
        "task/seed-task",
        Some(&head),
        &[lead_seed_run()],
    );

    let srv = sb.serve(&[]);
    let resp = srv.get(&format!("/api/tasks/{task_id}/pipeline"));
    assert_eq!(resp.status, 200, "{}", resp.text());
    let p = resp.json();
    assert_eq!(p["cards"]["ban"]["state"], "cho");
    assert_eq!(p["gate"]["state"], "cho");
}

#[test]
fn review_commit_not_code() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let res = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead");
    let res = sb.run(
        &["run", "plan-review", "--", "web-ban-hang"],
        Some(plan_review_entry("approve", 0, 1)),
    );
    assert_ok(&res, "run plan-review");

    sb.db()
        .execute("UPDATE tasks SET status = 'dev_done' WHERE id = 1", [])
        .unwrap();

    let srv = sb.serve(&[]);
    let resp = srv.get("/api/tasks/1");
    assert_eq!(resp.status, 200, "{}", resp.text());
    let task = resp.json();
    let qa = action(&task, "run_qa");
    assert_eq!(qa["enabled"], false);
    assert_eq!(qa["reason"], "chưa có thay đổi code sau plan");
}

// ---------------------------------------------------------------------------
// Task 2: busy rules with live drivers, task detail controls / auto / rev
// ---------------------------------------------------------------------------

/// A `sleep 60` the test spawned; killed (by its own handle) when dropped.
struct Helper {
    child: std::process::Child,
    id: agent_brain::procs::ProcId,
}

impl Helper {
    fn spawn() -> Helper {
        let child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("spawn sleep helper");
        let id = agent_brain::procs::proc_id(child.id() as i32).expect("helper proc id");
        Helper { child, id }
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.stop();
    }
}

fn set_driver(sb: &Sandbox, task_id: i64, h: &Helper) {
    sb.db()
        .execute(
            "UPDATE tasks SET auto_state = 'running', auto_pid = ?1, auto_pid_start = ?2 WHERE id = ?3",
            rusqlite::params![h.id.pid as i64, h.id.start as i64, task_id],
        )
        .unwrap();
}

fn run_lead(sb: &Sandbox, project: &str, name: &str, request: &str) {
    let res = sb.run(
        &["run", "lead", "--name", name, "--", project, request],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead");
}

fn control<'a>(task: &'a Value, id: &str) -> &'a Value {
    task["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
        .unwrap_or_else(|| panic!("control {id} missing in {}", task["controls"]))
}

#[test]
fn busy_live_driver_blocks_project() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    run_lead(&sb, "web-ban-hang", "sua-app", "sửa app");
    run_lead(&sb, "web-ban-hang", "viec-hai", "việc hai");

    let mut helper = Helper::spawn();
    set_driver(&sb, 1, &helper);

    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "việc ba", "name": "viec-ba"}),
    );
    assert_eq!(resp.status, 409, "{}", resp.text());
    let body = resp.json();
    assert_eq!(body["kind"], "busy");
    let err = body["error"].as_str().unwrap().to_string();
    assert!(err.contains("đang chạy autopilot cho task 1"), "{err}");

    let t2 = srv.get("/api/tasks/2").json();
    for id in ["run_dev", "relead"] {
        let a = action(&t2, id);
        assert_eq!(a["enabled"], false, "{id}: {a}");
        assert_eq!(a["reason"], err.as_str(), "{id}");
    }

    let pf = srv
        .get("/api/preflight?project=web-ban-hang&slug=viec-ba")
        .json();
    assert!(pf["busy"]
        .as_str()
        .unwrap()
        .contains("đang chạy autopilot cho task 1"));

    helper.stop();

    let t2 = srv.get("/api/tasks/2").json();
    let a = action(&t2, "run_dev");
    assert_eq!(a["enabled"], true, "{a}");
    let pf = srv
        .get("/api/preflight?project=web-ban-hang&slug=viec-ba")
        .json();
    assert!(pf["busy"].is_null(), "{pf}");
}

#[test]
fn busy_personal_driver() {
    let sb = Sandbox::new();
    let repo1 = sb.make_repo("p1", &[("src/main.py", "")], None, None);
    let repo2 = sb.make_repo("p2", &[("src/main.py", "")], None, None);
    let e1 = sb.project_entry(&repo1, "p1", "personal");
    let e2 = sb.project_entry(&repo2, "p2", "personal");
    sb.write_map(json!([e1, e2]), &[]);

    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let projects = agent_brain::projects::projects_of(&sb.read_map());
    agent_brain::db::sync_projects(&conn, &projects).unwrap();
    let p1 = agent_brain::db::project_row_id(&conn, "p1").unwrap();
    let task_id =
        agent_brain::db::upsert_task(&conn, p1, "viec-p1", "Việc p1", "task/viec-p1", None, None)
            .unwrap();
    drop(conn);

    let helper = Helper::spawn();
    set_driver(&sb, task_id, &helper);

    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "p2", "request": "việc p2", "name": "viec-p2"}),
    );
    assert_eq!(resp.status, 409, "{}", resp.text());
    let body = resp.json();
    assert_eq!(body["kind"], "busy");
    let err = body["error"].as_str().unwrap();
    assert!(err.contains("(E-06)"), "{err}");
    assert!(
        err.contains(&format!("autopilot cho task {task_id}")),
        "{err}"
    );
    drop(helper);
}

#[test]
fn running_review_counts_as_run() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    run_lead(&sb, "web-ban-hang", "sua-app", "sửa app");

    let helper = Helper::spawn();
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let pid = agent_brain::db::project_row_id(&conn, "web-ban-hang").unwrap();
    let run_id = agent_brain::db::insert_run(
        &conn,
        &agent_brain::db::NewRun {
            task_id: Some(1),
            project_id: pid,
            role: "plan-review".to_string(),
            profile: "work".to_string(),
            model: Some("opus".to_string()),
            pid: helper.id.pid as i64,
            pgid: helper.id.pid as i64,
            pid_start: helper.id.start as i64,
            base_commit: None,
            run_dir: None,
        },
    )
    .unwrap();
    drop(conn);

    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "việc hai", "name": "viec-hai"}),
    );
    assert_eq!(resp.status, 409, "{}", resp.text());
    let body = resp.json();
    assert_eq!(body["kind"], "busy");
    assert!(
        body["error"].as_str().unwrap().contains("đang có run"),
        "{body}"
    );

    let t1 = srv.get("/api/tasks/1").json();
    let actions = t1["actions"].as_array().unwrap();
    assert_eq!(actions.len(), 1, "{}", t1["actions"]);
    assert_eq!(actions[0]["id"], "kill");
    assert_eq!(actions[0]["run_id"], run_id);
    drop(helper);
}

#[test]
fn task_detail_controls_and_auto() {
    let sb = Sandbox::new();
    let _repo = auto_to_gate(&sb);

    let srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();

    let ids: Vec<&str> = t["controls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["approve", "return_lead", "auto_start"]);
    assert_eq!(control(&t, "approve")["enabled"], true);
    assert_eq!(control(&t, "return_lead")["enabled"], true);
    let start = control(&t, "auto_start");
    assert_eq!(start["enabled"], false);
    assert_eq!(
        start["reason"],
        "Autopilot dừng ở bước duyệt plan — bấm Duyệt plan để chạy tiếp."
    );

    assert_eq!(t["auto"]["state"], "gate");
    assert_eq!(t["workflow"]["name"], "mac-dinh");
    assert_eq!(t["workflow"]["auto"], true);
    assert!(t["workflow"]["summary"].is_string());

    let rev = t["rev"].as_str().unwrap().to_string();
    assert_eq!(rev.len(), 16, "{rev}");
    assert!(rev.chars().all(|c| c.is_ascii_hexdigit()), "{rev}");

    let runs = t["runs"].as_array().unwrap();
    assert!(!runs.is_empty());
    for r in runs {
        let obj = r.as_object().unwrap();
        assert!(obj.contains_key("stage"), "{r}");
        assert!(obj.contains_key("round"), "{r}");
    }
    assert_eq!(runs[0]["stage"], "plan");
    assert_eq!(runs[0]["round"], 1);

    let t_again = srv.get("/api/tasks/1").json();
    assert_eq!(t_again["rev"], rev.as_str());

    let res = sb.run(&["approve", "1", "--no-continue"], None);
    assert_ok(&res, "approve --no-continue");
    let t_after = srv.get("/api/tasks/1").json();
    assert_ne!(t_after["rev"], rev.as_str());
}

#[test]
fn manual_actions_disabled_while_driver() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    run_lead(&sb, "web-ban-hang", "sua-app", "sửa app");

    let helper = Helper::spawn();
    set_driver(&sb, 1, &helper);

    let srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();
    let dev = action(&t, "run_dev");
    assert_eq!(dev["enabled"], false);
    assert_eq!(
        dev["reason"],
        "Autopilot đang chạy task này — bấm Dừng autopilot để làm tay"
    );
    let stop = control(&t, "auto_stop");
    assert_eq!(stop["enabled"], true, "{stop}");
    drop(helper);
}

#[test]
fn list_tasks_auto_fields() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    run_lead(&sb, "web-ban-hang", "sua-app", "sửa app");

    let aj = json!({"stop": {"reason": "het_vong", "loop_name": "build", "round": 3, "cap": 3, "at_ms": 1}});
    sb.db()
        .execute(
            "UPDATE tasks SET auto_state = 'stopped', auto_json = ?1 WHERE id = 1",
            [aj.to_string()],
        )
        .unwrap();

    let srv = sb.serve(&[]);
    let list = srv.get("/api/tasks").json();
    let item = list["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == 1)
        .unwrap()
        .clone();
    assert_eq!(item["auto_state"], "stopped");
    assert_eq!(item["stop_reason"], "het_vong");
    assert_eq!(
        item["stop_detail"],
        "Review code vẫn yêu cầu sửa lại sau 3 / 3 vòng."
    );
    assert_eq!(item["banner"], "Chờ bạn · hết vòng");
    assert_eq!(item["group"], "waiting");
}
