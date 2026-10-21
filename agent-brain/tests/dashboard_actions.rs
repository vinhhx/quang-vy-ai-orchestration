//! Lớp hành động dùng chung của dashboard (05.3 D-02): gọi trực tiếp trong tiến
//! trình test, không qua HTTP và không bắt đầu job nào (job con sẽ thừa hưởng
//! môi trường thật của tiến trình test).

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::dashboard::api_tasks::{load_task_state, run_control, ControlOutcome};
use agent_brain::dashboard::jobs::JobManager;
use agent_brain::dashboard::AppState;
use common::Sandbox;
use protocol::{lead_scenario, setup_project};
use std::path::Path;
use std::sync::Arc;

/// AppState trỏ vào sandbox (cấu hình, bản đồ dự án, brain.db) như `serve` dựng.
fn app_state(sb: &Sandbox) -> Arc<AppState> {
    let text = std::fs::read_to_string(&sb.config_path).expect("read config");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parse config");
    let cfg = agent_brain::config::config_from_value(&value).expect("config_from_value");
    Arc::new(AppState {
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

/// Task 1 `sua-app` trên dự án `web-ban-hang` (quy trình `nhanh`), Lead đã viết plan.
fn lead_task(sb: &Sandbox) -> std::path::PathBuf {
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

fn current_rev(st: &AppState, task_id: i64) -> String {
    let conn = agent_brain::db::open(&st.db_path).expect("open db");
    match load_task_state(st, &conn, task_id).expect("load_task_state") {
        Ok(state) => state.rev,
        Err(msg) => panic!("task {task_id} missing: {msg}"),
    }
}

#[test]
fn run_control_approves_in_process() {
    let sb = Sandbox::new();
    let repo = lead_task(&sb);
    let st = app_state(&sb);
    let rev = current_rev(&st, 1);

    let stale = run_control(&st, 1, "approve", Some("0000000000000000"), None).unwrap();
    assert_eq!(stale, ControlOutcome::Stale);
    assert_eq!(
        approved_commit(&sb, 1),
        None,
        "a stale approve writes nothing"
    );

    let sha = plan_sha(&sb, &repo);
    match run_control(&st, 1, "approve", Some(&rev), None).unwrap() {
        ControlOutcome::Approved {
            approved_at,
            plan_commit,
            job_id,
        } => {
            assert!(approved_at > 0);
            assert_eq!(plan_commit, sha);
            assert_eq!(job_id, None, "manual workflow starts no job");
        }
        other => panic!("expected Approved, got {other:?}"),
    }
    assert_eq!(approved_commit(&sb, 1), Some(sha.clone()));

    match run_control(&st, 1, "approve", None, None).unwrap() {
        ControlOutcome::Refused { reason, kind } => {
            assert_eq!(kind, "duplicate");
            assert!(
                reason.starts_with("Plan hiện tại đã được duyệt lúc"),
                "{reason}"
            );
        }
        other => panic!("expected Refused duplicate, got {other:?}"),
    }
    assert_eq!(approved_commit(&sb, 1), Some(sha));

    assert_eq!(
        run_control(&st, 999, "approve", None, None).unwrap(),
        ControlOutcome::NotFound("Không tìm thấy task 999".to_string())
    );
    assert!(st.jobs.list().is_empty(), "no job started in process");
}
