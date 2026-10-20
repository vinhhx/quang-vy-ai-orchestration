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

// ---------------------------------------------------------------------------
// Task 3: Giao việc with a workflow
// ---------------------------------------------------------------------------

fn auto_scenario() -> Value {
    by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry("approve", 0, 1)],
    }))
}

fn task_snapshot_name(sb: &Sandbox, task_id: i64) -> Option<String> {
    let wf: Option<String> = sb
        .db()
        .query_row(
            "SELECT workflow_json FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap();
    wf.map(|j| {
        let v: Value = serde_json::from_str(&j).unwrap();
        v["name"].as_str().unwrap().to_string()
    })
}

fn task_auto_state(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT auto_state FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
}

fn job_role(srv: &common::Serve, job_id: u64) -> String {
    let j = srv.get(&format!("/api/jobs/{job_id}")).json();
    j["role"].as_str().unwrap().to_string()
}

#[test]
fn create_task_nhanh_runs_lead() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_scenario(lead_plan_entry());

    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "sửa app", "name": "sua-app"}),
    );
    assert_eq!(resp.status, 202, "{}", resp.text());
    let body = resp.json();
    assert_eq!(body["workflow"], "nhanh");
    assert_eq!(body["auto"], false);
    let job_id = body["job_id"].as_u64().unwrap();
    assert_eq!(job_role(&srv, job_id), "lead");
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 0, "{job}");
    assert_eq!(job["role"], "lead");

    assert_eq!(task_snapshot_name(&sb, 1).as_deref(), Some("nhanh"));
    assert_eq!(task_auto_state(&sb, 1), None);
}

#[test]
fn create_task_mac_dinh_starts_autopilot() {
    let sb = Sandbox::new();
    let _repo = setup_auto_project(&sb, "work");
    sb.write_scenario(auto_scenario());

    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "sửa app", "name": "sua-app"}),
    );
    assert_eq!(resp.status, 202, "{}", resp.text());
    let body = resp.json();
    assert_eq!(body["workflow"], "mac-dinh");
    assert_eq!(body["auto"], true);
    let job_id = body["job_id"].as_u64().unwrap();
    assert_eq!(job_role(&srv, job_id), "auto");
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 0, "{job}");

    assert_eq!(task_snapshot_name(&sb, 1).as_deref(), Some("mac-dinh"));
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("gate"));
    let conn = sb.db();
    let mut stmt = conn
        .prepare("SELECT role FROM runs WHERE task_id = 1 ORDER BY id")
        .unwrap();
    let roles: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(roles, vec!["lead", "plan-review"]);
}

#[test]
fn create_task_explicit_workflow_wins() {
    let sb = Sandbox::new();
    let repo_a = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let repo_b = sb.make_repo(
        "kho",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    // web-ban-hang defaults to nhanh; kho has no workflow key (server default mac-dinh)
    let entry_a = sb.project_entry(&repo_a, "web-ban-hang", "work");
    let mut entry_b = sb.project_entry(&repo_b, "kho", "work");
    entry_b.as_object_mut().unwrap().remove("workflow");
    sb.write_map(json!([entry_a, entry_b]), &[]);
    sb.write_scenario(auto_scenario());

    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "sửa app", "name": "sua-app", "workflow": "mac-dinh"}),
    );
    assert_eq!(resp.status, 202, "{}", resp.text());
    let body = resp.json();
    assert_eq!(body["workflow"], "mac-dinh");
    assert_eq!(body["auto"], true);
    let job_id = body["job_id"].as_u64().unwrap();
    assert_eq!(job_role(&srv, job_id), "auto");
    assert_eq!(wait_for_job(&srv, job_id)["exit_code"], 0);

    sb.write_scenario(lead_plan_entry());
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "kho", "request": "sửa kho", "name": "sua-kho", "workflow": "nhanh"}),
    );
    assert_eq!(resp.status, 202, "{}", resp.text());
    let body = resp.json();
    assert_eq!(body["workflow"], "nhanh");
    assert_eq!(body["auto"], false);
    let job_id = body["job_id"].as_u64().unwrap();
    assert_eq!(job_role(&srv, job_id), "lead");
    assert_eq!(wait_for_job(&srv, job_id)["exit_code"], 0);

    let kho_task: i64 = sb
        .db()
        .query_row("SELECT id FROM tasks WHERE slug = 'sua-kho'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(task_snapshot_name(&sb, kho_task).as_deref(), Some("nhanh"));
}

#[test]
fn create_task_unknown_workflow() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "sửa app", "name": "sua-app", "workflow": "khong-co"}),
    );
    assert_eq!(resp.status, 422, "{}", resp.text());
    let err = resp.json()["error"].as_str().unwrap().to_string();
    assert!(err.starts_with("Không có quy trình 'khong-co'"), "{err}");

    assert_eq!(srv.get("/api/jobs/1").status, 404);
    let tasks: i64 = sb
        .db()
        .query_row("SELECT count(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(tasks, 0);
}

#[test]
fn create_task_body_fields() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_scenario(lead_plan_entry());
    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "sửa app", "name": "sua-app", "extra": 1}),
    );
    assert_eq!(resp.status, 422, "{}", resp.text());
    assert_eq!(srv.get("/api/jobs/1").status, 404);

    let request = "-sửa app gấp";
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": request, "name": "sua-gap"}),
    );
    assert_eq!(resp.status, 202, "{}", resp.text());
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 0, "{job}");

    let title: String = sb
        .db()
        .query_row("SELECT title FROM tasks WHERE slug = 'sua-gap'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(title, request);
}

// ---------------------------------------------------------------------------
// Plan 05.2-15 Task 1: Duyệt plan from the dashboard
// ---------------------------------------------------------------------------

fn dev_impl_entry() -> Value {
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
        {"op": "git", "args": ["add", "src/app.py"]},
        {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
    ]})
}

fn code_review_entry(verdict: &str, blocking: i64) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".code-review.md",
        "content": review_report(verdict, blocking, 1),
    }]})
}

fn qa_entry(verdict: &str) -> Value {
    let report = format!(
        "# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- src/app.py: đã xem.\n## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông.\n## Kết luận\n{verdict}\n"
    );
    json!({"ops": [{"op": "write", "path": ".qa-report.md", "content": report}]})
}

/// Sha của commit `sua-app: plan` mới nhất trên nhánh hiện tại.
fn plan_sha(sb: &Sandbox, repo: &Path) -> String {
    let log = sb.git(repo, &["log", "--format=%H %s"]);
    log.lines()
        .find_map(|l| {
            let (sha, subj) = l.split_once(' ')?;
            (subj == "sua-app: plan").then(|| sha.to_string())
        })
        .expect("plan commit")
}

fn approved_commit(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT approved_commit FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
}

fn task_status(sb: &Sandbox, task_id: i64) -> String {
    sb.db()
        .query_row("SELECT status FROM tasks WHERE id = ?1", [task_id], |r| {
            r.get(0)
        })
        .unwrap()
}

fn run_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap()
}

fn run_roles(sb: &Sandbox) -> Vec<String> {
    let conn = sb.db();
    let mut stmt = conn.prepare("SELECT role FROM runs ORDER BY id").unwrap();
    let roles = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<String>, _>>()
        .unwrap();
    roles
}

/// Mọi bước của mac-dinh: plan, review plan duyệt, dev, review code đạt, QA đạt.
fn full_flow_scenario() -> Value {
    by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry("approve", 0, 1)],
        "dev": [dev_impl_entry()],
        "code-review": [code_review_entry("pass", 0)],
        "qa": [qa_entry("Đạt")],
    }))
}

/// Giao việc mac-dinh từ dashboard (`sua-app`) và chờ job dừng ở cổng duyệt plan.
fn giao_viec_to_gate(sb: &Sandbox, srv: &common::Serve) -> u64 {
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": "web-ban-hang", "request": "sửa app", "name": "sua-app"}),
    );
    assert_eq!(resp.status, 202, "{}", resp.text());
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    let job = wait_for_job(srv, job_id);
    assert_eq!(job["exit_code"], 0, "{job}");
    assert_eq!(task_auto_state(sb, 1).as_deref(), Some("gate"));
    job_id
}

#[test]
fn approve_endpoint_flow() {
    let sb = Sandbox::new();
    let repo = setup_auto_project(&sb, "work");
    sb.write_scenario(full_flow_scenario());

    let srv = sb.serve(&[]);
    giao_viec_to_gate(&sb, &srv);

    let t = srv.get("/api/tasks/1").json();
    let rev = t["rev"].as_str().unwrap().to_string();
    assert_eq!(control(&t, "approve")["enabled"], true);

    let resp = srv.post_json("/api/tasks/1/approve", &json!({ "rev": rev }));
    assert_eq!(resp.status, 200, "{}", resp.text());
    let body = resp.json();
    let sha = plan_sha(&sb, &repo);
    assert_eq!(body["plan_commit"], sha.as_str(), "{body}");
    assert!(body["approved_at"].as_i64().unwrap() > 0, "{body}");
    let job_id = body["job_id"]
        .as_u64()
        .expect("job_id for an auto workflow");
    assert_eq!(job_role(&srv, job_id), "auto");

    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 0, "{job}");
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("done"));
    assert_eq!(task_status(&sb, 1), "qa_pass");
    assert_eq!(approved_commit(&sb, 1), Some(sha));
    assert_eq!(
        run_roles(&sb),
        vec!["lead", "plan-review", "dev", "code-review", "qa"]
    );
}

#[test]
fn approve_manual_workflow_no_job() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    run_lead(&sb, "web-ban-hang", "sua-app", "sửa app");
    let runs_before = run_count(&sb);

    let srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();
    assert_eq!(t["workflow"]["name"], "nhanh");
    assert_eq!(control(&t, "approve")["enabled"], true);

    let resp = srv.post_json("/api/tasks/1/approve", &json!({}));
    assert_eq!(resp.status, 200, "{}", resp.text());
    let body = resp.json();
    assert!(body["job_id"].is_null(), "{body}");
    assert_eq!(body["plan_commit"], plan_sha(&sb, &repo).as_str());

    let p = srv.get("/api/tasks/1/pipeline").json();
    assert_eq!(p["gate"]["state"], "xong", "{}", p["gate"]);
    assert_eq!(p["gate"]["note_kind"], "approved_explicit");
    assert_eq!(run_count(&sb), runs_before, "no new run");
    assert_eq!(srv.get("/api/jobs/1").status, 404, "no job started");
}

#[test]
fn approve_duplicate() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    run_lead(&sb, "web-ban-hang", "sua-app", "sửa app");

    let srv = sb.serve(&[]);
    let first = srv.post_json("/api/tasks/1/approve", &json!({}));
    assert_eq!(first.status, 200, "{}", first.text());
    let commit = approved_commit(&sb, 1);
    assert!(commit.is_some());

    let again = srv.post_json("/api/tasks/1/approve", &json!({}));
    assert_eq!(again.status, 409, "{}", again.text());
    let body = again.json();
    assert_eq!(body["kind"], "duplicate", "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .starts_with("Plan hiện tại đã được duyệt lúc"),
        "{body}"
    );
    assert_eq!(approved_commit(&sb, 1), commit);
    assert_eq!(srv.get("/api/jobs/1").status, 404, "no job started");
}

#[test]
fn approve_refused_reasons() {
    // 1. .plan.md sửa tay chưa commit -> nút tắt, 409 not_allowed với lý do của server.
    let sb = Sandbox::new();
    let repo = auto_to_gate(&sb);
    let plan_path = repo.join(".plan.md");
    let mut text = std::fs::read_to_string(&plan_path).unwrap();
    text.push_str("\n<!-- sửa tay -->\n");
    std::fs::write(&plan_path, text).unwrap();

    let srv = sb.serve(&[]);
    let resp = srv.post_json("/api/tasks/1/approve", &json!({}));
    assert_eq!(resp.status, 409, "{}", resp.text());
    let body = resp.json();
    assert_eq!(body["kind"], "not_allowed", "{body}");
    assert_eq!(
        body["error"],
        "Plan có thay đổi chưa commit — commit với message 'sua-app: plan' rồi duyệt"
    );
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(srv.get("/api/jobs/1").status, 404);
    drop(srv);

    // 2. Cổng chưa mở (review plan yêu cầu sửa, Lead chưa chạy lại) -> 409 not_allowed.
    let sb = Sandbox::new();
    let _repo = setup_auto_project(&sb, "work");
    run_lead(&sb, "web-ban-hang", "sua-app", "sửa app");
    let res = sb.run(
        &["run", "plan-review", "--", "web-ban-hang"],
        Some(plan_review_entry("revise", 1, 0)),
    );
    assert_ok(&res, "run plan-review");

    let srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();
    assert!(
        !t["controls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == "approve"),
        "{}",
        t["controls"]
    );
    let resp = srv.post_json("/api/tasks/1/approve", &json!({}));
    assert_eq!(resp.status, 409, "{}", resp.text());
    let body = resp.json();
    assert_eq!(body["kind"], "not_allowed", "{body}");
    assert_eq!(
        body["error"],
        "Hành động không áp dụng cho trạng thái hiện tại"
    );
    assert_eq!(approved_commit(&sb, 1), None);
}

// ---------------------------------------------------------------------------
// Plan 05.2-15 Task 2: return, autopilot start / stop / more-round / end, relead
// ---------------------------------------------------------------------------

fn lead_append_entry() -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan.md",
        "content": "\n<!-- v2 -->\n",
        "append": true,
    }]})
}

fn task_auto_json(sb: &Sandbox, task_id: i64) -> Value {
    let aj: Option<String> = sb
        .db()
        .query_row(
            "SELECT auto_json FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap();
    serde_json::from_str(&aj.unwrap_or_else(|| "{}".to_string())).unwrap()
}

fn run_metrics(sb: &Sandbox, run_id: i64) -> Value {
    let m: Option<String> = sb
        .db()
        .query_row(
            "SELECT metrics_json FROM runs WHERE id = ?1",
            [run_id],
            |r| r.get(0),
        )
        .unwrap();
    serde_json::from_str(&m.unwrap_or_else(|| "{}".to_string())).unwrap()
}

fn last_run_id(sb: &Sandbox, role: &str) -> i64 {
    sb.db()
        .query_row("SELECT MAX(id) FROM runs WHERE role = ?1", [role], |r| {
            r.get(0)
        })
        .unwrap()
}

/// Prompt (`-p`) của lần gọi claude giả cuối cùng cho Lead.
fn last_lead_prompt(sb: &Sandbox) -> String {
    let rec = sb
        .records()
        .into_iter()
        .filter(|r| {
            let argv = r["argv"].as_array().unwrap();
            argv.iter()
                .position(|a| a.as_str() == Some("--append-system-prompt"))
                .map(|i| {
                    argv[i + 1]
                        .as_str()
                        .unwrap_or("")
                        .starts_with("Bạn là Lead")
                })
                .unwrap_or(false)
        })
        .last()
        .expect("a Lead record");
    let argv: Vec<String> = rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();
    let i = argv.iter().position(|a| a == "-p").unwrap();
    argv[i + 1].clone()
}

const FRESH_SKELETON: &str = "đã có sẵn khung (skeleton)";

/// mac-dinh qua `auto --start` (CLI): review plan luôn revise → dừng `hết vòng` ở vòng 3 / 3.
fn plan_cap_stopped(sb: &Sandbox) {
    setup_auto_project(sb, "work");
    let sc = by_role(json!({
        "lead": [lead_plan_entry(), lead_append_entry()],
        "plan-review": [
            plan_review_entry("revise", 1, 0),
            plan_review_entry("revise", 1, 1),
            plan_review_entry("revise", 1, 2)
        ],
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
    assert_eq!(res.code, 4, "stderr: {}", res.stderr);
    assert_eq!(task_auto_state(sb, 1).as_deref(), Some("stopped"));
    let stop = task_auto_json(sb, 1)["stop"].clone();
    assert_eq!(stop["reason"], "het_vong", "{stop}");
    assert_eq!(stop["loop_name"], "plan", "{stop}");
}

/// Chờ một run `dev` đang chạy; trả id của run.
fn wait_running_dev(sb: &Sandbox) -> i64 {
    let mut found = None;
    let ok = wait_until(Duration::from_secs(20), || {
        let row: rusqlite::Result<i64> = sb.db().query_row(
            "SELECT id FROM runs WHERE status = 'running' AND role = 'dev'",
            [],
            |r| r.get(0),
        );
        match row {
            Ok(id) => {
                found = Some(id);
                true
            }
            Err(_) => false,
        }
    });
    assert!(ok, "no running dev run");
    found.unwrap()
}

fn run_status(sb: &Sandbox, run_id: i64) -> String {
    sb.db()
        .query_row("SELECT status FROM runs WHERE id = ?1", [run_id], |r| {
            r.get(0)
        })
        .unwrap()
}

#[test]
fn return_with_note_auto() {
    let sb = Sandbox::new();
    let repo = setup_auto_project(&sb, "work");
    sb.write_scenario(by_role(json!({
        "lead": [lead_plan_entry(), lead_append_entry()],
        "plan-review": [plan_review_entry("approve", 0, 1), plan_review_entry("approve", 0, 2)],
    })));
    let srv = sb.serve(&[]);
    giao_viec_to_gate(&sb, &srv);

    let t = srv.get("/api/tasks/1").json();
    let rev = t["rev"].as_str().unwrap().to_string();
    assert_eq!(control(&t, "return_lead")["enabled"], true);

    let resp = srv.post_json(
        "/api/tasks/1/return",
        &json!({"note": "  -tách bước 2  ", "rev": rev}),
    );
    assert_eq!(resp.status, 202, "{}", resp.text());
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(job_role(&srv, job_id), "auto");
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 0, "{job}");

    assert_eq!(
        run_roles(&sb),
        vec!["lead", "plan-review", "lead", "plan-review"]
    );
    let lead2 = last_run_id(&sb, "lead");
    assert_eq!(run_metrics(&sb, lead2)["note"], "-tách bước 2");
    let prompt = last_lead_prompt(&sb);
    assert!(
        prompt.contains("Ghi chú của người dùng: -tách bước 2"),
        "prompt: {prompt}"
    );
    assert!(!prompt.contains(FRESH_SKELETON), "prompt: {prompt}");

    // Lead sửa trên plan hiện tại: các dòng đầu vẫn còn, phần thêm nằm sau.
    let plan = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    assert!(plan.contains("| 1 | src/app.py |"), "{plan}");
    assert!(plan.contains("<!-- v2 -->"), "{plan}");

    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("gate"));
    assert_eq!(approved_commit(&sb, 1), None);
}

#[test]
fn return_manual_and_limits() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    run_lead(&sb, "web-ban-hang", "sua-app", "sửa app");
    sb.write_scenario(lead_plan_entry());
    let srv = sb.serve(&[]);

    let long = "ă".repeat(501);
    let resp = srv.post_json("/api/tasks/1/return", &json!({ "note": long }));
    assert_eq!(resp.status, 422, "{}", resp.text());
    assert_eq!(resp.json()["error"], "Ghi chú quá dài (tối đa 500 ký tự)");
    assert_eq!(srv.get("/api/jobs/1").status, 404, "no job for a long note");

    let resp = srv.post_json("/api/tasks/1/return", &json!({}));
    assert_eq!(resp.status, 202, "{}", resp.text());
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(job_role(&srv, job_id), "lead");
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 0, "{job}");

    assert_eq!(run_roles(&sb), vec!["lead", "lead"]);
    let prompt = last_lead_prompt(&sb);
    assert!(prompt.contains(FRESH_SKELETON), "prompt: {prompt}");
    assert!(
        !prompt.contains("Ghi chú của người dùng"),
        "prompt: {prompt}"
    );
    assert_eq!(approved_commit(&sb, 1), None);
}

#[test]
fn auto_start_stop_end() {
    // Chạy thêm 1 vòng sau `hết vòng`: một job, một lượt cộng vòng.
    let sb = Sandbox::new();
    plan_cap_stopped(&sb);
    sb.write_scenario(by_role(json!({
        "lead": [lead_append_entry()],
        "plan-review": [plan_review_entry("approve", 0, 0)],
    })));
    let srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();
    assert_eq!(t["auto"]["state"], "stopped");
    assert_eq!(control(&t, "auto_more_round")["enabled"], true, "{t}");

    let resp = srv.post_json("/api/tasks/1/auto/more-round", &json!({}));
    assert_eq!(resp.status, 202, "{}", resp.text());
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(job_role(&srv, job_id), "auto");

    let again = srv.post_json("/api/tasks/1/auto/more-round", &json!({}));
    assert_eq!(again.status, 409, "{}", again.text());
    let kind = again.json()["kind"].as_str().unwrap().to_string();
    assert!(kind == "busy" || kind == "duplicate", "{}", again.text());

    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 0, "{job}");
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("gate"));
    assert_eq!(task_auto_json(&sb, 1)["grants"]["plan"], 1);
    assert_eq!(run_count(&sb), 8);

    let late = srv.post_json("/api/tasks/1/auto/more-round", &json!({}));
    assert_eq!(late.status, 409, "{}", late.text());
    let body = late.json();
    assert_eq!(body["kind"], "duplicate", "{body}");
    assert_eq!(body["error"], "Autopilot không ở trạng thái dừng");
    assert_eq!(task_auto_json(&sb, 1)["grants"]["plan"], 1);
    assert_eq!(run_count(&sb), 8);
    drop(srv);

    // Dừng hẳn trên task đang dừng.
    let sb = Sandbox::new();
    plan_cap_stopped(&sb);
    let srv = sb.serve(&[]);
    let resp = srv.post_json("/api/tasks/1/auto/end", &json!({}));
    assert_eq!(resp.status, 200, "{}", resp.text());
    assert_eq!(resp.json()["ok"], true);
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("ended"));
    // auto_json.stop giữ lại làm lịch sử.
    assert_eq!(task_auto_json(&sb, 1)["stop"]["reason"], "het_vong");

    let t = srv.get("/api/tasks/1").json();
    assert_eq!(t["auto"]["state"], "off", "{}", t["auto"]);
    assert!(t["auto"]["stop_reason"].is_null(), "{}", t["auto"]);
    let banner = t["banner"].as_str().unwrap_or("");
    assert!(
        !banner.starts_with("Chờ bạn"),
        "stop banner still shown: {banner}"
    );
    // Không còn nút của banner dừng; plan vẫn chờ duyệt ở cổng.
    let ids: Vec<&str> = t["controls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["approve", "return_lead", "auto_start"], "{t}");

    let again = srv.post_json("/api/tasks/1/auto/end", &json!({}));
    assert_eq!(again.status, 409, "{}", again.text());
    assert_eq!(again.json()["kind"], "duplicate");
    assert_eq!(srv.get("/api/jobs/1").status, 404, "end starts no job");
}

#[test]
fn auto_stop_soft() {
    let sb = Sandbox::new();
    let _repo = auto_to_gate(&sb);
    let res = sb.run(&["approve", "1", "--no-continue"], None);
    assert_ok(&res, "approve --no-continue");

    let mut dev = dev_impl_entry();
    dev["sleep"] = json!(3.0);
    sb.write_scenario(by_role(json!({
        "dev": [dev],
        "code-review": [code_review_entry("pass", 0)],
        "qa": [qa_entry("Đạt")],
    })));
    let srv = sb.serve(&[]);

    let stop_early = srv.post_json("/api/tasks/1/auto/stop", &json!({}));
    assert_eq!(stop_early.status, 409, "{}", stop_early.text());
    assert_eq!(stop_early.json()["error"], "Autopilot không chạy");

    let resp = srv.post_json("/api/tasks/1/auto", &json!({}));
    assert_eq!(resp.status, 202, "{}", resp.text());
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(job_role(&srv, job_id), "auto");
    let dev_run = wait_running_dev(&sb);

    let dup = srv.post_json("/api/tasks/1/auto", &json!({}));
    assert_eq!(dup.status, 409, "{}", dup.text());
    let body = dup.json();
    assert_eq!(body["kind"], "duplicate", "{body}");
    assert_eq!(body["error"], "Autopilot đang chạy task này");

    let stop = srv.post_json("/api/tasks/1/auto/stop", &json!({}));
    assert_eq!(stop.status, 200, "{}", stop.text());
    assert_eq!(stop.json()["ok"], true);

    let stop2 = srv.post_json("/api/tasks/1/auto/stop", &json!({}));
    assert_eq!(stop2.status, 409, "{}", stop2.text());
    assert_eq!(stop2.json()["error"], "Đang chờ run hiện tại chạy xong.");

    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 4, "{job}");
    assert_eq!(run_status(&sb, dev_run), "succeeded");
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("stopped"));
    assert_eq!(task_auto_json(&sb, 1)["stop"]["reason"], "dung_boi_ban");
    assert!(
        !run_roles(&sb).iter().any(|r| r == "code-review"),
        "no stage after a soft stop"
    );
}

#[test]
fn relead_auto_workflow_replans() {
    let sb = Sandbox::new();
    let _repo = auto_to_gate(&sb);
    let aj =
        json!({"stop": {"reason": "dung_boi_ban", "loop_name": "plan", "round": 1, "at_ms": 1}});
    sb.db()
        .execute(
            "UPDATE tasks SET auto_state = 'stopped', auto_json = ?1 WHERE id = 1",
            [aj.to_string()],
        )
        .unwrap();
    sb.write_scenario(by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry("approve", 0, 3)],
    })));

    let srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();
    assert_eq!(t["auto"]["state"], "stopped");
    assert_eq!(action(&t, "relead")["enabled"], true, "{}", t["actions"]);

    let resp = srv.post_json("/api/tasks/1/relead", &json!({}));
    assert_eq!(resp.status, 202, "{}", resp.text());
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(job_role(&srv, job_id), "auto");
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 0, "{job}");

    assert_eq!(
        run_roles(&sb),
        vec!["lead", "plan-review", "lead", "plan-review"]
    );
    let prompt = last_lead_prompt(&sb);
    assert!(prompt.contains(FRESH_SKELETON), "prompt: {prompt}");
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("gate"));
}

#[test]
fn stale_rev_refused() {
    let sb = Sandbox::new();
    let _repo = auto_to_gate(&sb);
    let srv = sb.serve(&[]);

    for path in ["/api/tasks/1/approve", "/api/tasks/1/return"] {
        let resp = srv.post_json(path, &json!({"rev": "0000000000000000"}));
        assert_eq!(resp.status, 409, "{path}: {}", resp.text());
        let body = resp.json();
        assert_eq!(body["kind"], "stale", "{path}: {body}");
        assert_eq!(
            body["error"],
            "Trạng thái task đã thay đổi — màn hình sẽ tải lại."
        );
    }
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(srv.get("/api/jobs/1").status, 404, "no job started");
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("gate"));
}

#[test]
fn legacy_task_refused() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    let task_id = seed_task(&sb, &repo, "main", None, &[lead_seed_run()]);

    let srv = sb.serve(&[]);
    let resp = srv.post_json(&format!("/api/tasks/{task_id}/auto"), &json!({}));
    assert_eq!(resp.status, 409, "{}", resp.text());
    let body = resp.json();
    assert_eq!(
        body["error"],
        "Task tạo trước Phase 5.2 — không có quy trình để chạy autopilot."
    );
    assert_eq!(body["kind"], "not_allowed");
    assert_eq!(srv.get("/api/jobs/1").status, 404);
}

#[test]
fn gate_checks_on_new_routes() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);
    let good_origin = format!("http://127.0.0.1:{}", srv.port);
    let cookie = srv.cookie.clone();

    for path in [
        "/api/tasks/1/approve",
        "/api/tasks/1/return",
        "/api/tasks/1/auto",
        "/api/tasks/1/auto/stop",
        "/api/tasks/1/auto/more-round",
        "/api/tasks/1/auto/end",
    ] {
        let no_cookie = srv.request(
            "POST",
            path,
            &[
                ("Origin", &good_origin),
                ("Content-Type", "application/json"),
            ],
            b"{}",
        );
        assert_eq!(no_cookie.status, 401, "{path}");

        let evil = srv.request(
            "POST",
            path,
            &[
                ("Cookie", &cookie),
                ("Origin", "http://evil.example"),
                ("Content-Type", "application/json"),
            ],
            b"{}",
        );
        assert_eq!(evil.status, 403, "{path}");

        let plain = srv.request(
            "POST",
            path,
            &[
                ("Cookie", &cookie),
                ("Origin", &good_origin),
                ("Content-Type", "text/plain"),
            ],
            b"{}",
        );
        assert_eq!(plain.status, 415, "{path}");

        let unknown = srv.post_json(path, &json!({"x": 1}));
        assert_eq!(unknown.status, 422, "{path}: {}", unknown.text());
    }
    assert_eq!(srv.get("/api/jobs/1").status, 404);
}
