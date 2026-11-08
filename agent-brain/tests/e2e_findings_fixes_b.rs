//! Test suite cho Phase B (B1–B5):
//! - B1: Reopen task đã xong (qa_pass / answered), mở khóa auto_start & run_dev; từ chối khi chưa xong hoặc nhánh đã xóa.
//! - B2: Duyệt plan với bước tiếp theo (none / qa / dev / auto); Rule D6 bỏ qua implement; từ chối qa khi chưa có code.
//! - B3: Plan diff so với lần duyệt trước qua GET /api/tasks/{id}/plan-diff và plan_diff_summary.
//! - B4: Giao thẳng task direct=true khi orchestrator bật; từ chối workflow khi direct=false; truyền level khi orchestrator tắt.
//! - B5: Không tách yêu cầu split=false; đưa dự án được chọn vào triage prompt.

#![allow(clippy::while_let_loop)]

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::autopilot::decide::{rule_d6_build_phase, AutoJson, Facts};
use agent_brain::dashboard::api_tasks::{
    load_task_state, run_control, ControlOutcome,
};
use agent_brain::dashboard::jobs::JobManager;
use agent_brain::dashboard::AppState;
use common::Sandbox;
use protocol::{lead_scenario, setup_project};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn app_state(sb: &Sandbox) -> Arc<AppState> {
    let text = std::fs::read_to_string(&sb.config_path).expect("read config");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parse config");
    let cfg = agent_brain::config::config_from_value(&value).expect("config_from_value");
    Arc::new(AppState {
        orch: std::sync::RwLock::new(cfg.orchestrator.clone()),
        cfg,
        port: 0,
        token: "0".repeat(64),
        map_path: sb.map_path(),
        db_path: sb.db_path.clone(),
        exe: common::bin(),
        hints: tokio::sync::broadcast::channel(16).0,
        shutdown: tokio::sync::watch::channel(false).1,
        jobs: JobManager::new(common::bin()),
    })
}

fn lead_task(sb: &Sandbox) -> PathBuf {
    let repo = setup_project(sb, "work");
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
        Some(lead_scenario(&[(
            "src/app.py",
            "app()",
            "trả về 2",
            "demo",
        )])),
    );
    assert_eq!(
        res.code, 0,
        "run lead failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    repo
}

fn current_rev(st: &AppState, task_id: i64) -> String {
    let conn = agent_brain::db::open(&st.db_path).expect("open db");
    match load_task_state(st, &conn, task_id).expect("load_task_state") {
        Ok(state) => state.rev,
        Err(msg) => panic!("task {task_id} missing: {msg}"),
    }
}

// -----------------------------------------------------------------------------
// B1: Mở lại task đã xong (Reopen)
// -----------------------------------------------------------------------------

#[test]
fn test_b1_reopen_qa_pass_task_success() {
    let sb = Sandbox::new();
    let repo = lead_task(&sb);
    let st = app_state(&sb);

    let plan_commit = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();

    // Giả lập task hoàn thành qa_pass với plan đã duyệt và workflow hợp lệ
    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let workflows = agent_brain::workflows::load(&st.cfg);
    let wf = workflows.get("nhanh").expect("workflow nhanh exists");
    conn.execute(
        "UPDATE tasks SET status = 'qa_pass', auto_state = 'stopped', approved_commit = ?1, workflow_json = ?2 WHERE id = 1",
        rusqlite::params![plan_commit, wf.to_snapshot()],
    )
    .unwrap();
    let initial_json = "{\"stop\":{\"kind\":\"qa_dat\",\"word\":\"qa đạt\",\"at_ms\":1000}}";
    agent_brain::db::set_auto_json(&conn, 1, Some(initial_json)).unwrap();

    let rev = current_rev(&st, 1);

    // Reopen task
    let outcome = run_control(&st, 1, "reopen", Some(&rev), Some("cần sửa thêm")).unwrap();
    match outcome {
        ControlOutcome::Reopened { new_status } => {
            assert_eq!(new_status, "qa_fix");
        }
        other => panic!("expected Reopened, got {:?}", other),
    }

    // Kiểm tra DB: status thành qa_fix, auto_state thành stopped, aj.stop bị xóa
    let (status, auto_state): (String, Option<String>) = conn
        .query_row(
            "SELECT status, auto_state FROM tasks WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, "qa_fix");
    assert_eq!(auto_state.as_deref(), Some("stopped"));

    let auto = agent_brain::db::task_auto(&conn, 1).unwrap().unwrap();
    let loaded_aj: AutoJson = auto
        .auto_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    assert!(loaded_aj.stop.is_none(), "auto_json.stop phải bị xóa sau reopen");

    // Kiểm tra TaskState: auto_start và run_dev đều có sẵn
    let state = load_task_state(&st, &conn, 1).unwrap().unwrap();
    assert!(
        state.view.controls.iter().any(|c| c.id == "auto_start" && c.enabled),
        "auto_start phải được mở khóa sau reopen: {:?}",
        state.view.controls
    );
    assert!(
        state.view.actions.iter().any(|a| a.id == "run_dev" && a.enabled),
        "run_dev phải có sẵn khi status là qa_fix: {:?}",
        state.view.actions
    );
}

#[test]
fn test_b1_reopen_non_success_status_refused() {
    let sb = Sandbox::new();
    let _repo = lead_task(&sb);
    let st = app_state(&sb);
    let rev = current_rev(&st, 1);

    // Task đang ở planned (chưa hoàn thành)
    let outcome = run_control(&st, 1, "reopen", Some(&rev), None).unwrap();
    match outcome {
        ControlOutcome::Refused { reason, .. } => {
            assert!(
                reason.contains("chưa hoàn thành"),
                "expected refusal reason, got {reason}"
            );
        }
        other => panic!("expected Refused, got {:?}", other),
    }
}

#[test]
fn test_b1_reopen_deleted_branch_refused() {
    let sb = Sandbox::new();
    let repo = lead_task(&sb);
    let st = app_state(&sb);

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    conn.execute("UPDATE tasks SET status = 'qa_pass' WHERE id = 1", [])
        .unwrap();

    // Xóa nhánh task trên git
    sb.git(&repo, &["checkout", "main"]);
    sb.git(&repo, &["branch", "-D", "task/sua-app"]);

    let rev = current_rev(&st, 1);
    let outcome = run_control(&st, 1, "reopen", Some(&rev), None).unwrap();
    match outcome {
        ControlOutcome::Refused { reason, .. } => {
            assert!(
                reason.contains("không còn tồn tại") || reason.contains("bị xóa"),
                "expected git branch deleted reason, got {reason}"
            );
        }
        other => panic!("expected Refused, got {:?}", other),
    }
}

// -----------------------------------------------------------------------------
// B2: Duyệt plan và chọn bước tiếp theo
// -----------------------------------------------------------------------------

#[test]
fn test_b2_approve_with_next_none() {
    let sb = Sandbox::new();
    let _repo = lead_task(&sb);
    let st = app_state(&sb);
    let rev = current_rev(&st, 1);

    let outcome = run_control(&st, 1, "approve_none", Some(&rev), None).unwrap();
    match outcome {
        ControlOutcome::Approved { job_id, .. } => {
            assert!(job_id.is_none(), "approve_none không được khởi động job");
        }
        other => panic!("expected Approved, got {:?}", other),
    }

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let auto_state: Option<String> = conn
        .query_row("SELECT auto_state FROM tasks WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(auto_state.as_deref(), Some("stopped"));
}

#[test]
fn test_b2_approve_with_next_qa_without_code_refused() {
    let sb = Sandbox::new();
    let _repo = lead_task(&sb);
    let st = app_state(&sb);
    let rev = current_rev(&st, 1);

    // Chưa có code thay đổi sau plan commit
    let outcome = run_control(&st, 1, "approve_qa", Some(&rev), None).unwrap();
    match outcome {
        ControlOutcome::Refused { reason, .. } => {
            assert!(
                reason.contains("Chưa có thay đổi code sau plan để chạy QA"),
                "expected no code after plan refusal, got {reason}"
            );
        }
        other => panic!("expected Refused, got {:?}", other),
    }
}

#[test]
fn test_b2_rule_d6_skips_implement_when_skip_build_once() {
    let sb = Sandbox::new();
    let st = app_state(&sb);
    let workflows = agent_brain::workflows::load(&st.cfg);
    let wf = workflows.get("nhanh").expect("workflow nhanh exists");

    // 1. Khi skip_build_once = false: Rule D6 trả về Start(Dev)
    let facts_normal = Facts::default();
    let res_normal = rule_d6_build_phase(wf, &facts_normal, 1, &[], &[]).unwrap();
    assert!(res_normal.is_some(), "Bình thường phải trả về quyết định chạy Dev");

    // 2. Khi skip_build_once = true VÀ code_after_plan = true: Rule D6 trả về Ok(None) (bỏ qua build)
    let mut facts_skip = Facts::default().with_code_after_plan(true);
    facts_skip.skip_build_once = true;
    let res_skip = rule_d6_build_phase(wf, &facts_skip, 1, &[], &[]).unwrap();
    assert!(res_skip.is_none(), "Rule D6 phải bỏ qua implement khi skip_build_once && code_after_plan");
}

// -----------------------------------------------------------------------------
// B3: Diff plan so với lần duyệt trước
// -----------------------------------------------------------------------------

#[test]
fn test_b3_plan_diff_first_and_second_approval() {
    let sb = Sandbox::new();
    let repo = lead_task(&sb);
    let srv = sb.serve(&[]);

    // Lần 1: Chưa duyệt lần nào -> previous_commit là null
    let resp = srv.get("/api/tasks/1/plan-diff");
    assert_eq!(resp.status, 200);
    let body = resp.json();
    assert!(body["previous_commit"].is_null(), "Lần đầu previous_commit phải là null");
    assert!(body["current_commit"].is_string());

    // Duyệt plan lần 1 qua API
    let resp = srv.post_json("/api/tasks/1/approve", &json!({ "next": "none" }));
    assert!(resp.status == 200 || resp.status == 202);

    // Sửa plan trên git và commit với subject "{slug}: plan"
    let plan_path = repo.join(".plan.md");
    let old_plan = std::fs::read_to_string(&plan_path).unwrap();
    let new_plan = format!("{old_plan}\n\nThêm ghi chú chỉnh sửa sau lần duyệt 1.\n");
    std::fs::write(&plan_path, new_plan).unwrap();
    sb.git(&repo, &["commit", "-am", "sua-app: plan"]);

    // Lần 2: GET /api/tasks/1/plan-diff phải trả về previous_commit và diff có dòng mới
    let resp = srv.get("/api/tasks/1/plan-diff");
    assert_eq!(resp.status, 200);
    let body = resp.json();
    assert!(body["previous_commit"].is_string(), "previous_commit phải có sha lần duyệt 1: {}", body);
    let diff = body["diff"].as_str().unwrap();
    assert!(diff.contains("+Thêm ghi chú chỉnh sửa sau lần duyệt 1."), "diff phải chứa dòng mới: {}", diff);

    // Kiểm tra TaskState có plan_diff_summary
    let st = app_state(&sb);
    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let state = load_task_state(&st, &conn, 1).unwrap().unwrap();
    let summary = state.plan_diff_summary.expect("plan_diff_summary phải có giá trị");
    assert!(summary.contains("Plan đổi +"), "summary phải ghi số dòng đổi: {summary}");
}

// -----------------------------------------------------------------------------
// B4: Giao thẳng task direct=true khi orchestrator bật
// -----------------------------------------------------------------------------

#[test]
fn test_b4_direct_task_creation_with_orchestrator() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "orchestrator": {
            "enabled": true
        }
    }));
    let srv = sb.serve(&[]);

    // 1. direct = true -> Tạo ngay đề xuất approved mà không gọi triage
    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "Cần thêm trang liên hệ",
            "name": "trang-lien-he",
            "workflow": "nhanh",
            "team": "mac-dinh",
            "level": "de",
            "direct": true
        }),
    );
    assert_eq!(resp.status, 201, "body: {}", resp.text());
    let body = resp.json();
    assert_eq!(body["status"], "approved");
    let proposal_id = body["proposal_id"].as_i64().expect("proposal_id");
    let intake_id = body["intake_id"].as_i64().expect("intake_id");

    // Kiểm tra DB
    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let (prop_wf, prop_team, prop_level, prop_status): (String, String, String, String) = conn
        .query_row(
            "SELECT workflow, team, level, status FROM proposals WHERE id = ?1",
            [proposal_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(prop_wf, "nhanh");
    assert_eq!(prop_team, "mac-dinh");
    assert_eq!(prop_level, "de");
    assert_eq!(prop_status, "approved");

    let intake_status: String = conn
        .query_row("SELECT status FROM intake WHERE id = ?1", [intake_id], |r| r.get(0))
        .unwrap();
    assert_eq!(intake_status, "proposed");

    // 2. direct = false nhưng có workflow -> Phải trả về 400
    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "Test không direct",
            "name": "ko-direct",
            "workflow": "nhanh",
            "direct": false
        }),
    );
    assert_eq!(resp.status, 400);
    assert!(resp.text().contains("workflow/team/level chỉ dùng với direct=true"));

    // 3. direct = true với dự án không tồn tại -> 400
    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "du-an-ma",
            "request": "Yêu cầu",
            "name": "task-ma",
            "direct": true
        }),
    );
    assert_eq!(resp.status, 400);
    assert!(resp.text().contains("Không tìm thấy dự án 'du-an-ma'"));
}

// -----------------------------------------------------------------------------
// B5: Không tách yêu cầu split=false & dự án trong prompt
// -----------------------------------------------------------------------------

#[test]
fn test_b5_no_split_intake_and_project_in_prompt() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let repo_str = repo.to_string_lossy();
    conn.execute(
        "INSERT OR IGNORE INTO projects (id, slug, path, name_vi, profile)
         VALUES (1, 'web-ban-hang', ?1, 'Web bán hàng', 'work')",
        rusqlite::params![repo_str],
    )
    .unwrap();

    // insert_intake với split: false và project_id: Some(1)
    let intake_id = agent_brain::db::insert_intake(
        &conn,
        "ui",
        Some(1),
        "Thêm thanh tìm kiếm trên trang chủ",
        "normal",
        false,
    )
    .unwrap();

    let intake = agent_brain::db::get_intake(&conn, intake_id).unwrap().unwrap();
    assert!(!intake.split, "intake.split phải là false");

    let st = app_state(&sb);
    let (system_prompt, _) =
        agent_brain::orchestrator::triage::build_triage_prompt(&st, &intake).unwrap();

    // Kiểm tra luật không tách: "Trả đúng 1 mục trong 'tasks', không dùng 'after'"
    assert!(
        system_prompt.contains("Trả đúng 1 mục trong 'tasks', không dùng 'after'"),
        "Prompt phải chứa quy tắc không tách khi split=false: {system_prompt}"
    );

    // Kiểm tra dự án được chỉ định có trong prompt
    assert!(
        system_prompt.contains("Người gửi đã chọn dự án 'web-ban-hang': giữ nguyên project là 'web-ban-hang'"),
        "Prompt phải giữ nguyên dự án khi người gửi đã chọn: {system_prompt}"
    );
}
