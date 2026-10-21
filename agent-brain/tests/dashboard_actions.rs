//! Lớp hành động dùng chung của dashboard (05.3 D-02): gọi trực tiếp trong tiến
//! trình test, không qua HTTP và không bắt đầu job nào (job con sẽ thừa hưởng
//! môi trường thật của tiến trình test).

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::dashboard::api_runs::{kill_run, KillOutcome};
use agent_brain::dashboard::api_tasks::{
    lead_return_args, load_task_state, normalize_note, run_control, run_task_action, ControlOutcome,
};
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

fn task_row() -> agent_brain::db::TaskRow {
    agent_brain::db::TaskRow {
        id: 7,
        project_id: 1,
        slug: "sua-app".to_string(),
        title: "sửa app".to_string(),
        branch: "ab/sua-app".to_string(),
        base_branch: Some("main".to_string()),
        base_commit: None,
        status: "planned".to_string(),
        created_at: 0,
        updated_at: 0,
    }
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn lead_return_args_autopilot_replans() {
    let task = task_row();
    assert_eq!(
        lead_return_args(&task, "web-ban-hang", true, None),
        ("auto", strings(&["auto", "7", "--replan"]))
    );
    assert_eq!(
        lead_return_args(&task, "web-ban-hang", true, Some("tách bước 2")),
        (
            "auto",
            strings(&["auto", "7", "--replan", "--note=tách bước 2"])
        )
    );
}

#[test]
fn lead_return_args_manual_lead() {
    let task = task_row();
    assert_eq!(
        lead_return_args(&task, "web-ban-hang", false, None),
        (
            "lead",
            strings(&[
                "run",
                "lead",
                "--name",
                "sua-app",
                "--",
                "web-ban-hang",
                "sửa app"
            ])
        )
    );
    // Ghi chú bắt đầu bằng `-` vẫn là một phần tử argv `--note=…`, không thành cờ.
    assert_eq!(
        lead_return_args(&task, "web-ban-hang", false, Some("-x")),
        (
            "lead",
            strings(&[
                "run",
                "lead",
                "--name",
                "sua-app",
                "--note=-x",
                "--",
                "web-ban-hang",
                "sửa app"
            ])
        )
    );
}

#[test]
fn normalize_note_trims_and_caps() {
    assert_eq!(normalize_note(Some("  a  ")), Ok(Some("a".to_string())));
    assert_eq!(normalize_note(Some("   ")), Ok(None));
    assert_eq!(normalize_note(None), Ok(None));
    assert_eq!(
        normalize_note(Some(&"ă".repeat(500))),
        Ok(Some("ă".repeat(500)))
    );
    assert_eq!(
        normalize_note(Some(&"ă".repeat(501))),
        Err("Ghi chú quá dài (tối đa 500 ký tự)".to_string())
    );
}

#[test]
fn return_lead_long_note_is_invalid() {
    let sb = Sandbox::new();
    let _repo = lead_task(&sb);
    let st = app_state(&sb);
    let long = "ă".repeat(501);
    assert_eq!(
        run_control(&st, 1, "return_lead", None, Some(&long)).unwrap(),
        ControlOutcome::Invalid("Ghi chú quá dài (tối đa 500 ký tự)".to_string())
    );
    assert!(st.jobs.list().is_empty(), "no job started");
}

#[test]
fn run_task_action_not_found_and_stale() {
    let sb = Sandbox::new();
    let _repo = lead_task(&sb);
    let st = app_state(&sb);
    assert_eq!(
        run_task_action(&st, 999, "relead", "lead", None, None).unwrap(),
        ControlOutcome::NotFound("Không tìm thấy task 999".to_string())
    );
    assert_eq!(
        run_task_action(&st, 1, "relead", "lead", Some("0000000000000000"), None).unwrap(),
        ControlOutcome::Stale
    );
    assert_eq!(
        run_task_action(&st, 1, "relead", "lead", None, Some(&"ă".repeat(501))).unwrap(),
        ControlOutcome::Invalid("Ghi chú quá dài (tối đa 500 ký tự)".to_string())
    );
    assert!(st.jobs.list().is_empty(), "no job started");
}

#[test]
fn kill_run_refuses_finished_and_missing() {
    let sb = Sandbox::new();
    let _repo = lead_task(&sb);
    let st = app_state(&sb);
    assert_eq!(
        kill_run(&st, 1).unwrap(),
        KillOutcome::Refused("Run 1 không còn chạy (trạng thái succeeded)".to_string())
    );
    assert_eq!(
        kill_run(&st, 999).unwrap(),
        KillOutcome::NotFound("Không tìm thấy run 999".to_string())
    );
}

/// Thêm một dòng run `running` không có tiến trình thật (pid 0) cho task — `cmd_kill`
/// chỉ đánh dấu `lost`, không gửi tín hiệu nào.
fn add_stale_running_run(sb: &Sandbox, task_id: i64) -> i64 {
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

fn status_of(sb: &Sandbox, run_id: i64) -> String {
    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    agent_brain::db::get_run(&conn, run_id)
        .expect("get_run")
        .expect("run")
        .status
}

#[test]
fn kill_run_refuses_when_task_has_two_running_runs() {
    let sb = Sandbox::new();
    let _repo = lead_task(&sb);
    let a = add_stale_running_run(&sb, 1);
    let b = add_stale_running_run(&sb, 1);
    let st = app_state(&sb);
    for confirmed in [a, b] {
        match kill_run(&st, confirmed).unwrap() {
            KillOutcome::Refused(msg) => {
                assert!(msg.contains("nhiều run đang chạy"), "{msg}");
            }
            other => panic!("phải từ chối, nhận {other:?}"),
        }
    }
    // Không run nào bị đụng tới.
    assert_eq!(status_of(&sb, a), "running");
    assert_eq!(status_of(&sb, b), "running");
}

#[test]
fn kill_run_single_running_run_reaches_cmd_kill() {
    let sb = Sandbox::new();
    let _repo = lead_task(&sb);
    let a = add_stale_running_run(&sb, 1);
    let st = app_state(&sb);
    // Qua được kiểm tra "run duy nhất đang chạy"; run không có tiến trình thật nên bị
    // đánh dấu lost và cmd_kill từ chối (không gửi tín hiệu nào).
    match kill_run(&st, a).unwrap() {
        KillOutcome::Refused(msg) => {
            assert!(!msg.contains("nhiều run đang chạy"), "{msg}");
        }
        other => panic!("nhận {other:?}"),
    }
    assert_eq!(status_of(&sb, a), "lost");
}
