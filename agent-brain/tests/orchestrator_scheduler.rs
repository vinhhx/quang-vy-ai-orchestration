mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::dashboard::jobs::JobManager;
use agent_brain::dashboard::AppState;
use agent_brain::db::{
    count_active_tasks, get_proposal, insert_proposal,
    list_quota_stops, set_proposal_status, NewProposal,
};
use agent_brain::orchestrator::scheduler::{code_wait_reason, schedule_tick};
use agent_brain::telegram::detect::ChainWaits;
use agent_brain::telegram::outbox::Outgoing;
use common::Sandbox;
use serde_json::json;
use std::path::PathBuf;

fn insert_intake(
    conn: &rusqlite::Connection,
    source: &str,
    project_id: Option<i64>,
    request: &str,
    priority: &str,
) -> agent_brain::error::BrainResult<i64> {
    agent_brain::db::insert_intake(conn, source, project_id, request, priority, true)
}
use std::sync::Arc;

static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn setup_fake_job(sb: &Sandbox) -> PathBuf {
    let script = sb.root.join("fake_job.sh");
    std::fs::write(&script, "#!/bin/sh\nsleep 30\nexit 0\n").expect("write fake_job.sh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod");
    }
    script
}

fn app_state(sb: &Sandbox, exe: PathBuf) -> Arc<AppState> {
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
        exe: exe.clone(),
        hints: tokio::sync::broadcast::channel(16).0,
        shutdown: tokio::sync::watch::channel(false).1,
        jobs: JobManager::new(exe),
    })
}

fn setup_projects(sb: &Sandbox, names: &[&str]) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    let mut repos = Vec::new();
    for name in names {
        let repo = sb.make_repo(
            name,
            &[("README.md", &format!("# Project {name}\n"))],
            None,
            None,
        );
        let mut entry = sb.project_entry(&repo, name, "work");
        entry["name_vi"] = serde_json::json!(format!("Dự án {name}"));
        entries.push(entry);
        repos.push(repo);
    }
    sb.write_map(serde_json::Value::Array(entries), &[]);
    repos
}

/// Acceptance 1: Với 5 proposal approved (2 urgent, 3 normal): tối đa 3 được bắt đầu cùng lúc; việc urgent được bắt đầu trước.
#[test]
fn test_scheduler_runs_proposals_priority_order_and_max_parallel() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["p1", "p2", "p3", "p4", "p5"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 3
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let intake_id = insert_intake(&conn, "ui", None, "Intake batch", "normal").expect("insert intake");

    // Tạo 5 proposals theo thứ tự:
    // Prop 1: p1, normal, title "Task 1"
    // Prop 2: p2, urgent, title "Task 2"
    // Prop 3: p3, normal, title "Task 3"
    // Prop 4: p4, urgent, title "Task 4"
    // Prop 5: p5, normal, title "Task 5"
    let p1 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: agent_brain::db::project_row_id(&conn, "p1").expect("pid"),
            title: "Task 1",
            request: "Do task 1",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p1");
    set_proposal_status(&conn, p1, "approved").expect("approve p1");

    let p2 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: agent_brain::db::project_row_id(&conn, "p2").expect("pid"),
            title: "Task 2",
            request: "Do task 2",
            priority: "urgent",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p2");
    set_proposal_status(&conn, p2, "approved").expect("approve p2");

    let p3 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: agent_brain::db::project_row_id(&conn, "p3").expect("pid"),
            title: "Task 3",
            request: "Do task 3",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p3");
    set_proposal_status(&conn, p3, "approved").expect("approve p3");

    let p4 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: agent_brain::db::project_row_id(&conn, "p4").expect("pid"),
            title: "Task 4",
            request: "Do task 4",
            priority: "urgent",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p4");
    set_proposal_status(&conn, p4, "approved").expect("approve p4");

    let p5 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: agent_brain::db::project_row_id(&conn, "p5").expect("pid"),
            title: "Task 5",
            request: "Do task 5",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p5");
    set_proposal_status(&conn, p5, "approved").expect("approve p5");

    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("schedule_tick");

    // Kiểm tra: 2 urgent (p2, p4) và 1 normal cũ nhất (p1) được bắt đầu (tổng 3 slots)
    let prop1 = get_proposal(&conn, p1).unwrap().unwrap();
    let prop2 = get_proposal(&conn, p2).unwrap().unwrap();
    let prop3 = get_proposal(&conn, p3).unwrap().unwrap();
    let prop4 = get_proposal(&conn, p4).unwrap().unwrap();
    let prop5 = get_proposal(&conn, p5).unwrap().unwrap();

    assert_eq!(prop2.status, "started", "p2 (urgent) must be started");
    assert_eq!(prop4.status, "started", "p4 (urgent) must be started");
    assert_eq!(prop1.status, "started", "p1 (normal, first) must be started");

    assert_eq!(prop3.status, "approved", "p3 must remain approved (max 3 slots)");
    assert_eq!(prop5.status, "approved", "p5 must remain approved (max 3 slots)");

    st.jobs.kill_all_now();
}

/// Acceptance 2: Khi một task xong, việc tiếp theo trong hàng được bắt đầu.
#[test]
fn test_scheduler_starts_next_when_task_completes() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["p1", "p2", "p3", "p4"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 2
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let intake_id = insert_intake(&conn, "ui", None, "Intake batch", "normal").expect("insert intake");

    let p1 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: agent_brain::db::project_row_id(&conn, "p1").expect("pid"),
            title: "Task 1",
            request: "Do task 1",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p1");
    set_proposal_status(&conn, p1, "approved").expect("approve p1");

    let p2 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: agent_brain::db::project_row_id(&conn, "p2").expect("pid"),
            title: "Task 2",
            request: "Do task 2",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p2");
    set_proposal_status(&conn, p2, "approved").expect("approve p2");

    let p3 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: agent_brain::db::project_row_id(&conn, "p3").expect("pid"),
            title: "Task 3",
            request: "Do task 3",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p3");
    set_proposal_status(&conn, p3, "approved").expect("approve p3");

    let st = app_state(&sb, fake_job);

    // Giả lập 1 task đang chạy trong hệ thống (auto_state = 'running')
    // Slot max = 2, nên chỉ còn 1 slot trống cho proposal
    let pid1 = agent_brain::db::project_row_id(&conn, "p4").expect("pid");
    let t_existing = agent_brain::db::upsert_task(
        &conn,
        pid1,
        "task-running",
        "Running Task",
        "task-running",
        Some("task/task-running"),
        None,
    )
    .expect("upsert task");
    agent_brain::db::set_auto_state(&conn, t_existing, Some("running")).expect("set auto running");

    assert_eq!(count_active_tasks(&conn).unwrap(), 1);

    // Tick 1: Bắt đầu 1 proposal (p1) vì còn 1 slot (1 active + 1 new = 2 max)
    schedule_tick(&st).expect("tick 1");

    let prop1 = get_proposal(&conn, p1).unwrap().unwrap();
    let prop2 = get_proposal(&conn, p2).unwrap().unwrap();
    assert_eq!(prop1.status, "started");
    assert_eq!(prop2.status, "approved");

    // Task t_existing hoàn thành: chuyển auto_state sang 'done'
    agent_brain::db::set_auto_state(&conn, t_existing, Some("done")).expect("set auto done");

    // Tick 2: Slot được giải phóng, proposal tiếp theo (p2) được bắt đầu
    schedule_tick(&st).expect("tick 2");

    let prop2_after = get_proposal(&conn, p2).unwrap().unwrap();
    assert_eq!(prop2_after.status, "started", "p2 must start when slot frees");

    let prop3 = get_proposal(&conn, p3).unwrap().unwrap();
    assert_eq!(prop3.status, "approved", "p3 remains approved until another slot frees");

    st.jobs.kill_all_now();
}

/// Acceptance 3: Task hết quota dừng, không chặn task khác, scheduler không gửi tin Telegram nào (thông báo đến từ detect.rs có sẵn).
#[test]
fn test_scheduler_quota_stop_does_not_block_slot_and_records_quota_stops() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["p1", "p2"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 1
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    // Tạo task 1 ở dự án p1 dừng vì hết quota (auto_stop_kind = 'het_quota')
    let pid1 = agent_brain::db::project_row_id(&conn, "p1").expect("pid");
    let t1 = agent_brain::db::upsert_task(
        &conn,
        pid1,
        "task-1",
        "Task 1",
        "task-1",
        Some("task/task-1"),
        None,
    )
    .expect("upsert task");

    let now = agent_brain::db::now_ms();
    conn.execute(
        "UPDATE tasks SET auto_state = 'stopped', auto_stop_at = ?1, auto_stop_kind = 'het_quota' WHERE id = ?2",
        rusqlite::params![now - 1000, t1],
    )
    .expect("update task quota stop");

    // Tạo run kết thúc quota cho task 1 trên engine deepseek
    conn.execute(
        "INSERT INTO runs (task_id, project_id, role, engine, profile, status, outcome, started_at, ended_at)
         VALUES (?1, ?2, 'dev', 'deepseek', 'default', 'succeeded', 'quota', ?3, ?4)",
        rusqlite::params![t1, pid1, now - 2000, now - 1000],
    )
    .expect("insert quota run");

    // Task hết quota KHÔNG được tính vào count_active_tasks
    assert_eq!(count_active_tasks(&conn).unwrap(), 0, "quota stopped task must not count as active");

    // Tạo proposal đã started gắn với task 1
    let intake_1 = insert_intake(&conn, "ui", None, "Intake 1", "normal").expect("insert intake 1");
    let prop_1 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id: intake_1,
            project_id: pid1,
            title: "Task 1",
            request: "Do task 1",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("prop 1");
    agent_brain::db::set_proposal_started(&conn, prop_1, Some(t1)).expect("start prop 1");

    // Tạo proposal approved cho dự án p2
    let intake_id = insert_intake(&conn, "ui", None, "Intake 2", "normal").expect("insert intake");
    let p2 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: agent_brain::db::project_row_id(&conn, "p2").expect("pid"),
            title: "Task 2",
            request: "Do task 2",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p2");
    set_proposal_status(&conn, p2, "approved").expect("approve p2");

    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("schedule_tick");

    // 1. Quota stop được ghi nhận vào bảng quota_stops với notified = 0
    let q_stops = list_quota_stops(&conn).expect("list quota stops");
    assert_eq!(q_stops.len(), 1);
    assert_eq!(q_stops[0].task_id, t1);
    assert_eq!(q_stops[0].engine, "deepseek");
    assert_eq!(q_stops[0].notified, 0, "notified must remain 0 per spec");

    // 2. Proposal cho dự án p2 được bắt đầu thành công (không bị task hết quota chặn slot)
    let prop2 = get_proposal(&conn, p2).unwrap().unwrap();
    assert_eq!(prop2.status, "started", "proposal p2 must start in the free slot");

    st.jobs.kill_all_now();
}

/// Acceptance 4: auto_resume_quota = true và đã qua thời gian: task được chạy tiếp đúng một lần; không lặp.
#[test]
fn test_scheduler_auto_resume_quota_once() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["p1"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 3,
            "auto_resume_quota": true,
            "resume_after_minutes": 5
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let pid1 = agent_brain::db::project_row_id(&conn, "p1").expect("pid");
    let t1 = agent_brain::db::upsert_task(
        &conn,
        pid1,
        "task-resume",
        "Task Resume",
        "task-resume",
        Some("task/task-resume"),
        None,
    )
    .expect("upsert task");

    let now = agent_brain::db::now_ms();
    // Đã dừng vì quota cách đây 6 phút (> resume_after_minutes = 5)
    let stopped_at = now - (6 * 60 * 1000);

    conn.execute(
        "UPDATE tasks SET auto_state = 'stopped', auto_stop_at = ?1, auto_stop_kind = 'het_quota' WHERE id = ?2",
        rusqlite::params![stopped_at, t1],
    )
    .expect("update task quota stop");

    conn.execute(
        "INSERT INTO runs (task_id, project_id, role, engine, profile, status, outcome, started_at, ended_at)
         VALUES (?1, ?2, 'dev', 'claude', 'default', 'succeeded', 'quota', ?3, ?4)",
        rusqlite::params![t1, pid1, stopped_at - 1000, stopped_at],
    )
    .expect("insert quota run");

    // Tạo proposal đã started cho task t1
    let intake_1 = insert_intake(&conn, "ui", None, "Intake resume", "normal").expect("insert intake");
    let prop_resume = insert_proposal(
        &conn,
        &NewProposal {
            intake_id: intake_1,
            project_id: pid1,
            title: "Task Resume",
            request: "Do task resume",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("prop resume");
    agent_brain::db::set_proposal_started(&conn, prop_resume, Some(t1)).expect("start prop resume");

    let st = app_state(&sb, fake_job);

    // Tick 1: Phát hiện dừng và vì đã qua thời gian nên tự động resume
    schedule_tick(&st).expect("tick 1");

    // Kiểm tra: 1 job được start qua JobManager với role 'auto'
    let jobs = st.jobs.list();
    assert_eq!(jobs.len(), 1, "exactly 1 job started for auto resume");
    assert_eq!(jobs[0].role, "auto");
    assert_eq!(jobs[0].task_id, Some(t1));

    // Bảng quota_stops đã được xóa dòng của task t1
    let q_stops = list_quota_stops(&conn).expect("list quota stops");
    assert_eq!(q_stops.len(), 0, "quota_stops entry must be removed on resume");

    // Tick 2: Gọi lại không bắt đầu thêm job nào (không lặp)
    schedule_tick(&st).expect("tick 2");
    let jobs_after = st.jobs.list();
    assert_eq!(jobs_after.len(), 1, "no second job must be started (no loop)");

    st.jobs.kill_all_now();
}

/// C3: a quota run that reported `metrics.quota.reset_at` is resumed at
/// `max(reset_at, stop + resume_after_minutes)`: not before the reset time, then once.
#[test]
fn test_scheduler_auto_resume_waits_for_reset_at() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["p1"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 3,
            "auto_resume_quota": true,
            "resume_after_minutes": 5
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let pid1 = agent_brain::db::project_row_id(&conn, "p1").expect("pid");
    let t1 = agent_brain::db::upsert_task(
        &conn,
        pid1,
        "task-reset",
        "Task Reset",
        "task-reset",
        Some("task/task-reset"),
        None,
    )
    .expect("upsert task");

    let now = agent_brain::db::now_ms();
    // Stopped 6 minutes ago (> resume_after_minutes = 5), but the quota resets in an hour.
    let stopped_at = now - (6 * 60 * 1000);
    conn.execute(
        "UPDATE tasks SET auto_state = 'stopped', auto_stop_at = ?1, auto_stop_kind = 'het_quota' WHERE id = ?2",
        rusqlite::params![stopped_at, t1],
    )
    .expect("update task quota stop");
    let later = json!({"quota": {"engine": "claude", "signal": "x", "reset_at": now + 3_600_000}});
    conn.execute(
        "INSERT INTO runs (task_id, project_id, role, engine, profile, status, outcome, started_at, ended_at, metrics_json)
         VALUES (?1, ?2, 'dev', 'claude', 'default', 'succeeded', 'quota', ?3, ?4, ?5)",
        rusqlite::params![t1, pid1, stopped_at - 1000, stopped_at, later.to_string()],
    )
    .expect("insert quota run");
    assert_eq!(
        agent_brain::db::latest_quota_reset_at(&conn, t1),
        Some(now + 3_600_000)
    );

    let intake_1 = insert_intake(&conn, "ui", None, "Intake reset", "normal").expect("insert intake");
    let prop = insert_proposal(
        &conn,
        &NewProposal {
            intake_id: intake_1,
            project_id: pid1,
            title: "Task Reset",
            request: "Do task reset",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("prop");
    agent_brain::db::set_proposal_started(&conn, prop, Some(t1)).expect("start prop");

    let st = app_state(&sb, fake_job);

    schedule_tick(&st).expect("tick 1");
    assert_eq!(st.jobs.list().len(), 0, "not resumed before reset_at");
    assert_eq!(list_quota_stops(&conn).expect("list").len(), 1);

    // The reset time passes (written in seconds, as some streams report it): resumed once.
    let passed = json!({"quota": {"engine": "claude", "signal": "x", "reset_at": (now - 60_000) / 1000}});
    conn.execute(
        "UPDATE runs SET metrics_json = ?1 WHERE task_id = ?2",
        rusqlite::params![passed.to_string(), t1],
    )
    .expect("update metrics");
    schedule_tick(&st).expect("tick 2");
    let jobs = st.jobs.list();
    assert_eq!(jobs.len(), 1, "resumed after reset_at");
    assert_eq!(jobs[0].task_id, Some(t1));

    st.jobs.kill_all_now();
}

/// Acceptance 5: enabled = false: không có task nào được bắt đầu bởi bộ lập lịch.
#[test]
fn test_scheduler_disabled_starts_nothing() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["p1"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": false,
            "max_parallel": 3
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let intake_id = insert_intake(&conn, "ui", None, "Intake disabled", "normal").expect("insert intake");
    let p1 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: agent_brain::db::project_row_id(&conn, "p1").expect("pid"),
            title: "Task Disabled",
            request: "Do task",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p1");
    set_proposal_status(&conn, p1, "approved").expect("approve p1");

    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("schedule_tick");

    // Proposal vẫn giữ nguyên approved, 0 job nào được tạo
    let prop1 = get_proposal(&conn, p1).unwrap().unwrap();
    assert_eq!(prop1.status, "approved");
    assert!(st.jobs.list().is_empty());
}

/// Preflight refused: proposal bị từ chối chuyển sang status 'failed' và lưu lỗi, không thử lại vô hạn.
#[test]
fn test_scheduler_preflight_refusal_marks_failed() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["p1"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 3
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let intake_id = insert_intake(&conn, "ui", None, "Intake invalid", "normal").expect("insert intake");

    // Tạo proposal với team không hợp lệ
    let p1 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: agent_brain::db::project_row_id(&conn, "p1").expect("pid"),
            title: "Task Invalid",
            request: "Do task",
            priority: "normal",
            level: "vua",
            team: "team-khong-ton-tai",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p1");
    set_proposal_status(&conn, p1, "approved").expect("approve p1");

    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("schedule_tick");

    let prop1 = get_proposal(&conn, p1).unwrap().unwrap();
    assert_eq!(prop1.status, "failed");
    assert!(prop1.error.is_some());
    assert!(st.jobs.list().is_empty());
}

/// Test bổ sung (a): Task quota không có proposal không bị ghi quota_stops và không auto_resume.
#[test]
fn test_scheduler_quota_stop_without_proposal_ignored() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["p1"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 2,
            "auto_resume_quota": true,
            "resume_after_minutes": 1
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let pid1 = agent_brain::db::project_row_id(&conn, "p1").expect("pid");
    let t_legacy = agent_brain::db::upsert_task(
        &conn,
        pid1,
        "legacy-quota-task",
        "Legacy Quota Task",
        "legacy-quota-task",
        Some("task/legacy-quota-task"),
        None,
    )
    .expect("upsert task");

    let now = agent_brain::db::now_ms();
    let stopped_at = now - (5 * 60 * 1000); // 5 phút trước (> resume_after_minutes = 1)

    conn.execute(
        "UPDATE tasks SET auto_state = 'stopped', auto_stop_at = ?1, auto_stop_kind = 'het_quota' WHERE id = ?2",
        rusqlite::params![stopped_at, t_legacy],
    )
    .expect("update task quota stop");

    conn.execute(
        "INSERT INTO runs (task_id, project_id, role, engine, profile, status, outcome, started_at, ended_at)
         VALUES (?1, ?2, 'dev', 'claude', 'default', 'succeeded', 'quota', ?3, ?4)",
        rusqlite::params![t_legacy, pid1, stopped_at - 1000, stopped_at],
    )
    .expect("insert quota run");

    // Task này KHÔNG có dòng nào trong proposals
    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("tick");

    // Bảng quota_stops phải rỗng vì task không thuộc proposal nào
    let q_stops = list_quota_stops(&conn).expect("list quota stops");
    assert_eq!(q_stops.len(), 0, "task without started proposal must not be recorded in quota_stops");

    // Không có job nào được start để auto_resume
    let jobs = st.jobs.list();
    assert_eq!(jobs.len(), 0, "task without started proposal must not be auto-resumed");
}

/// Test bổ sung (b): Hai proposal approved cùng dự án, cái thứ hai gặp busy vẫn approved,
/// và một proposal dự án khác phía sau vẫn được bắt đầu.
#[test]
fn test_scheduler_busy_project_skips_and_starts_next_project_proposal() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["p1", "p2"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 2
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let pid1 = agent_brain::db::project_row_id(&conn, "p1").expect("pid p1");
    let pid2 = agent_brain::db::project_row_id(&conn, "p2").expect("pid p2");

    let intake_id = insert_intake(&conn, "ui", None, "Intake batch", "normal").expect("insert intake");

    // Tạo 3 proposals:
    // p1_prop1: project p1, urgent
    // p1_prop2: project p1, normal (cùng dự án p1)
    // p2_prop: project p2, low (dự án p2 khác)
    let p1_prop1 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid1,
            title: "Task P1 A",
            request: "Do task P1 A",
            priority: "urgent",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p1_prop1");
    set_proposal_status(&conn, p1_prop1, "approved").expect("approve p1_prop1");

    let p1_prop2 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid1,
            title: "Task P1 B",
            request: "Do task P1 B",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p1_prop2");
    set_proposal_status(&conn, p1_prop2, "approved").expect("approve p1_prop2");

    let p2_prop = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid2,
            title: "Task P2 C",
            request: "Do task P2 C",
            priority: "low",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p2_prop");
    set_proposal_status(&conn, p2_prop, "approved").expect("approve p2_prop");

    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("schedule_tick");

    // Kết quả:
    // 1. p1_prop1 được bắt đầu -> status = "started"
    let prop1 = get_proposal(&conn, p1_prop1).unwrap().unwrap();
    assert_eq!(prop1.status, "started", "p1_prop1 should start");

    // 2. p1_prop2 cùng dự án p1, gặp preflight busy -> VẪN GIỮ status = "approved" (không phải "failed")
    let prop2 = get_proposal(&conn, p1_prop2).unwrap().unwrap();
    assert_eq!(prop2.status, "approved", "p1_prop2 on busy project must stay approved");

    // 3. p2_prop (dự án p2) phía sau vẫn được bắt đầu vì còn slot -> status = "started"
    let prop3 = get_proposal(&conn, p2_prop).unwrap().unwrap();
    assert_eq!(prop3.status, "started", "p2_prop on free project should start");

    // Dọn dẹp jobs
    st.jobs.kill_all_now();
}

#[test]
fn test_scheduler_starts_proposal_links_task_id_and_level() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["proj1"]);
    let args_log = sb.root.join("fake_job_args.txt");
    let script = sb.root.join("fake_job.sh");
    std::fs::write(
        &script,
        format!("#!/bin/sh\necho \"$@\" > {}\nsleep 30\nexit 0\n", args_log.display()),
    )
    .expect("write fake_job.sh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod");
    }

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 2
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let intake_id = insert_intake(&conn, "ui", None, "viết test", "normal").expect("intake");
    let pid = agent_brain::db::project_row_id(&conn, "proj1").unwrap();
    let prop_id = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid,
            title: "Task Cần Làm",
            request: "Làm việc này thật tốt",
            priority: "urgent",
            level: "kho",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("prop");
    set_proposal_status(&conn, prop_id, "approved").expect("approve prop");

    let st = app_state(&sb, script);
    schedule_tick(&st).expect("schedule_tick");

    let prop = get_proposal(&conn, prop_id).unwrap().unwrap();
    assert_eq!(prop.status, "started");
    let tid = prop.task_id.expect("proposals.task_id must be populated with new task id");
    let task = agent_brain::db::get_task(&conn, tid).unwrap().expect("task must exist");
    assert_eq!(task.level, "kho", "tasks.level must match proposal level");

    std::thread::sleep(std::time::Duration::from_millis(100));
    let logged = std::fs::read_to_string(&args_log).expect("read args_log");
    assert!(
        logged.contains("--level kho"),
        "job args must contain '--level kho', got: {logged}"
    );

    st.jobs.kill_all_now();
}

#[test]
fn test_chained_proposal_fails_when_prior_timed_out() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["proj1"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 2
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let intake_id = insert_intake(&conn, "ui", None, "chuỗi việc", "normal").expect("intake");
    let pid = agent_brain::db::project_row_id(&conn, "proj1").unwrap();

    let p0 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid,
            title: "Khảo sát",
            request: "Khảo sát hệ thống",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "phan-tich",
            reason: None,
        },
    )
    .expect("p0");

    let p1 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid,
            title: "Thực thi",
            request: "Thực thi theo khảo sát",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p1");
    agent_brain::db::set_proposal_after_id(&conn, p1, Some(p0)).unwrap();

    let task_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "khao-sat",
        "Khảo sát",
        "analysis/khao-sat",
        None,
        None,
    )
    .unwrap();
    agent_brain::db::set_proposal_status(&conn, p0, "started").unwrap();
    conn.execute("UPDATE proposals SET task_id = ?1 WHERE id = ?2", rusqlite::params![task_id, p0]).unwrap();
    conn.execute("UPDATE tasks SET status = 'timed_out' WHERE id = ?1", [task_id]).unwrap();

    set_proposal_status(&conn, p1, "approved").expect("approve p1");

    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("schedule_tick");

    let p1_after = get_proposal(&conn, p1).unwrap().unwrap();
    assert_eq!(p1_after.status, "failed");
    assert!(p1_after.error.unwrap().contains(&format!("Việc trước (#{p0}) không có kết quả")));

    st.jobs.kill_all_now();
}

#[test]
fn test_chained_proposal_waits_when_prior_stopped() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["proj1"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 2
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let intake_id = insert_intake(&conn, "ui", None, "chuỗi việc", "normal").expect("intake");
    let pid = agent_brain::db::project_row_id(&conn, "proj1").unwrap();

    let p0 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid,
            title: "Khảo sát",
            request: "Khảo sát hệ thống",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "phan-tich",
            reason: None,
        },
    )
    .expect("p0");

    let p1 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid,
            title: "Thực thi",
            request: "Thực thi theo khảo sát",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p1");
    agent_brain::db::set_proposal_after_id(&conn, p1, Some(p0)).unwrap();

    let task_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "khao-sat",
        "Khảo sát",
        "analysis/khao-sat",
        None,
        None,
    )
    .unwrap();
    agent_brain::db::set_proposal_status(&conn, p0, "started").unwrap();
    conn.execute("UPDATE proposals SET task_id = ?1 WHERE id = ?2", rusqlite::params![task_id, p0]).unwrap();
    
    // Set task to stopped with vuong_mac
    let stop = agent_brain::autopilot::decide::StopInfo {
        reason: agent_brain::autopilot::decide::StopReason::VuongMac,
        loop_name: None,
        stage: None,
        round: None,
        cap: None,
        run_id: None,
        role: None,
        at_ms: 1000,
        detail_line: None,
        used_ms: None,
        used_usd: None,
        estimated: None,
        limit_minutes: None,
        limit_usd: None,
    };
    let aj = agent_brain::autopilot::decide::AutoJson {
        stop: Some(stop),
        ..Default::default()
    };
    conn.execute(
        "UPDATE tasks SET status = 'open', auto_state = 'stopped', auto_json = ?1 WHERE id = ?2",
        rusqlite::params![aj.to_json(), task_id],
    )
    .unwrap();

    set_proposal_status(&conn, p1, "approved").expect("approve p1");

    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("schedule_tick");

    let p1_after = get_proposal(&conn, p1).unwrap().unwrap();
    // p1 should still be approved, not failed, and waiting on task
    assert_eq!(p1_after.status, "approved");
    assert!(p1_after.error.unwrap().contains(&format!("Chờ task #{task_id} (đang dừng: vướng mắc)")));

    st.jobs.kill_all_now();
}

#[test]
fn test_chained_proposal_fails_when_prior_ended_qa_fix() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["proj1"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 2
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let intake_id = insert_intake(&conn, "ui", None, "chuỗi việc", "normal").expect("intake");
    let pid = agent_brain::db::project_row_id(&conn, "proj1").unwrap();

    let p0 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid,
            title: "Build",
            request: "Build app",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p0");

    let p1 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid,
            title: "Tiếp theo",
            request: "Làm tiếp",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p1");
    agent_brain::db::set_proposal_after_id(&conn, p1, Some(p0)).unwrap();

    let task_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "build-app",
        "Build",
        "task/build-app",
        None,
        None,
    )
    .unwrap();
    agent_brain::db::set_proposal_status(&conn, p0, "started").unwrap();
    conn.execute("UPDATE proposals SET task_id = ?1 WHERE id = ?2", rusqlite::params![task_id, p0]).unwrap();
    conn.execute("UPDATE tasks SET status = 'qa_fix', auto_state = 'ended' WHERE id = ?1", [task_id]).unwrap();

    set_proposal_status(&conn, p1, "approved").expect("approve p1");

    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("schedule_tick");

    let p1_after = get_proposal(&conn, p1).unwrap().unwrap();
    assert_eq!(p1_after.status, "failed");
    assert!(p1_after.error.unwrap().contains(&format!("Việc trước (#{p0}) không có kết quả")));

    st.jobs.kill_all_now();
}

#[test]
fn test_chained_proposal_proceeds_on_answered_with_document() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["proj1"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 2
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let intake_id = insert_intake(&conn, "ui", None, "chuỗi việc", "normal").expect("intake");
    let pid = agent_brain::db::project_row_id(&conn, "proj1").unwrap();

    let p0 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid,
            title: "Khảo sát",
            request: "Khảo sát hệ thống",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "phan-tich",
            reason: None,
        },
    )
    .expect("p0");

    let p1 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid,
            title: "Thực thi",
            request: "Thực thi theo khảo sát",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p1");
    agent_brain::db::set_proposal_after_id(&conn, p1, Some(p0)).unwrap();

    let task_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "khao-sat",
        "Khảo sát",
        "analysis/khao-sat",
        None,
        None,
    )
    .unwrap();
    agent_brain::db::set_proposal_status(&conn, p0, "started").unwrap();
    conn.execute("UPDATE proposals SET task_id = ?1 WHERE id = ?2", rusqlite::params![task_id, p0]).unwrap();
    conn.execute("UPDATE tasks SET status = 'answered' WHERE id = ?1", [task_id]).unwrap();

    agent_brain::answers::save("proj1", task_id, "khao-sat", "# Kết quả phân tích\n\nNội dung khảo sát.").unwrap();

    set_proposal_status(&conn, p1, "approved").expect("approve p1");

    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("schedule_tick");

    let p1_after = get_proposal(&conn, p1).unwrap().unwrap();
    assert_eq!(p1_after.status, "started");

    let task1_id = p1_after.task_id.expect("task_id");
    let inputs = agent_brain::inputs::list_for_task(&conn, task1_id).unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].name, format!("phan-tich-{task_id}.md"));
    assert_eq!(inputs[0].source, format!("answer:{task_id}"));
    assert_eq!(inputs[0].content, "# Kết quả phân tích\n\nNội dung khảo sát.");

    st.jobs.kill_all_now();
}

/// B2: an approved proposal chained after a code proposal whose task finished (`qa_pass`).
/// Returns (repo, earlier task id, later proposal id, state).
fn code_chain(
    sb: &Sandbox,
    base: Option<&str>,
    make_branch: bool,
) -> (PathBuf, i64, i64, Arc<AppState>, rusqlite::Connection) {
    let repos = setup_projects(sb, &["proj1"]);
    let repo = repos[0].clone();
    let fake_job = setup_fake_job(sb);
    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 2
        }
    }));
    if make_branch {
        sb.git(&repo, &["checkout", "-q", "-b", "task/build-1"]);
        std::fs::write(repo.join("model.txt"), "cột mới\n").unwrap();
        sb.git(&repo, &["add", "model.txt"]);
        sb.git(&repo, &["commit", "-q", "-m", "build 1"]);
        sb.git(&repo, &["checkout", "-q", "main"]);
    }

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let intake_id = insert_intake(&conn, "ui", None, "chuỗi việc code", "normal").expect("intake");
    let pid = agent_brain::db::project_row_id(&conn, "proj1").unwrap();
    let new = |title: &'static str, request: &'static str| NewProposal {
        intake_id,
        project_id: pid,
        title,
        request,
        priority: "normal",
        level: "vua",
        team: "mac-dinh",
        workflow: "mac-dinh",
        reason: None,
    };
    let p0 = insert_proposal(&conn, &new("Build 1", "Sửa model")).expect("p0");
    let p1 = insert_proposal(&conn, &new("Build 2", "Sửa màn hình dùng model")).expect("p1");
    agent_brain::db::set_proposal_after_id(&conn, p1, Some(p0)).unwrap();

    let task_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "build-1",
        "Build 1",
        "task/build-1",
        base,
        None,
    )
    .unwrap();
    set_proposal_status(&conn, p0, "started").unwrap();
    conn.execute("UPDATE proposals SET task_id = ?1 WHERE id = ?2", rusqlite::params![task_id, p0]).unwrap();
    conn.execute("UPDATE tasks SET status = 'qa_pass' WHERE id = ?1", [task_id]).unwrap();
    set_proposal_status(&conn, p1, "approved").expect("approve p1");

    let st = app_state(sb, fake_job);
    (repo, task_id, p1, st, conn)
}

#[test]
fn test_chained_code_waits_until_prior_branch_merged() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let (repo, task_id, p1, st, conn) = code_chain(&sb, Some("main"), true);
    let mut waits = ChainWaits::seed(&conn).unwrap();

    // Not merged yet: the later proposal stays approved and says why.
    schedule_tick(&st).expect("tick 1");
    let after1 = get_proposal(&conn, p1).unwrap().unwrap();
    assert_eq!(after1.status, "approved");
    assert_eq!(after1.task_id, None);
    let wait = format!("Chờ task #{task_id} được merge vào main");
    assert_eq!(after1.error.as_deref(), Some(wait.as_str()));
    let sent = waits.scan(&conn).unwrap();
    assert_eq!(
        sent,
        vec![Outgoing::Message {
            text: format!("Đề xuất #{p1} «Build 2» đang chờ task #{task_id} được merge vào main.")
        }]
    );

    // B3: a second tick neither rewrites the error nor sends again.
    std::thread::sleep(std::time::Duration::from_millis(5));
    schedule_tick(&st).expect("tick 2");
    let after2 = get_proposal(&conn, p1).unwrap().unwrap();
    assert_eq!(after2.status, "approved");
    assert_eq!(after2.updated_at, after1.updated_at, "same wait is not written again");
    assert!(waits.scan(&conn).unwrap().is_empty(), "the wait is reported once");

    // Merged into main: the next tick starts it and clears the wait.
    sb.git(&repo, &["merge", "-q", "--ff-only", "task/build-1"]);
    schedule_tick(&st).expect("tick 3");
    let after3 = get_proposal(&conn, p1).unwrap().unwrap();
    assert_eq!(after3.status, "started");
    assert!(after3.task_id.is_some());
    assert_eq!(after3.error, None);
    let inputs = agent_brain::inputs::list_for_task(&conn, after3.task_id.unwrap()).unwrap();
    assert!(inputs.is_empty(), "a code task hands on no document: {inputs:?}");
    assert!(waits.scan(&conn).unwrap().is_empty());

    st.jobs.kill_all_now();
}

#[test]
fn test_chained_code_prior_without_base_does_not_run() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let (_repo, task_id, p1, st, conn) = code_chain(&sb, None, true);

    schedule_tick(&st).expect("tick");
    let after = get_proposal(&conn, p1).unwrap().unwrap();
    assert_eq!(after.status, "approved");
    assert_eq!(after.task_id, None);
    assert_eq!(
        after.error.as_deref(),
        Some(format!("Task #{task_id} không có nhánh gốc — merge tay rồi duyệt lại").as_str())
    );
    // Not a merge wait: nothing for Telegram.
    let mut waits = ChainWaits::default();
    assert!(waits.scan(&conn).unwrap().is_empty());

    st.jobs.kill_all_now();
}

#[test]
fn test_chained_code_prior_branch_deleted_counts_as_not_merged() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let (_repo, task_id, p1, st, conn) = code_chain(&sb, Some("main"), false);

    schedule_tick(&st).expect("tick");
    let after = get_proposal(&conn, p1).unwrap().unwrap();
    assert_eq!(after.status, "approved");
    assert_eq!(
        after.error.as_deref(),
        Some(
            format!(
                "Chờ task #{task_id} được merge vào main (nhánh task/build-1 không còn — coi như chưa merge)"
            )
            .as_str()
        )
    );

    st.jobs.kill_all_now();
}

#[test]
fn test_code_wait_reason_cases() {
    assert_eq!(code_wait_reason(3, "task/a", Some("main"), true, true), None);
    assert_eq!(
        code_wait_reason(3, "task/a", Some("dev"), true, false).as_deref(),
        Some("Chờ task #3 được merge vào dev")
    );
    assert_eq!(
        code_wait_reason(3, "task/a", None, true, true).as_deref(),
        Some("Task #3 không có nhánh gốc — merge tay rồi duyệt lại")
    );
    assert!(code_wait_reason(3, "task/a", Some("main"), false, false)
        .unwrap()
        .contains("không còn"));
}

#[test]
fn test_chained_proposal_waits_when_last_run_timed_out_but_task_stopped() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["proj1"]);
    let fake_job = setup_fake_job(&sb);

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true,
            "max_parallel": 2
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");

    let intake_id = insert_intake(&conn, "ui", None, "chuỗi việc", "normal").expect("intake");
    let pid = agent_brain::db::project_row_id(&conn, "proj1").unwrap();

    let p0 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid,
            title: "Khảo sát",
            request: "Khảo sát hệ thống",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "phan-tich",
            reason: None,
        },
    )
    .expect("p0");

    let p1 = insert_proposal(
        &conn,
        &NewProposal {
            intake_id,
            project_id: pid,
            title: "Thực thi",
            request: "Thực thi theo khảo sát",
            priority: "normal",
            level: "vua",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .expect("p1");
    agent_brain::db::set_proposal_after_id(&conn, p1, Some(p0)).unwrap();

    let task_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "khao-sat",
        "Khảo sát",
        "analysis/khao-sat",
        None,
        None,
    )
    .unwrap();
    agent_brain::db::set_proposal_status(&conn, p0, "started").unwrap();
    conn.execute("UPDATE proposals SET task_id = ?1 WHERE id = ?2", rusqlite::params![task_id, p0]).unwrap();
    
    // Seed a timed_out run
    let new_run = agent_brain::db::NewRun {
        task_id: Some(task_id),
        project_id: pid,
        role: "dev".to_string(),
        profile: "default".to_string(),
        model: None,
        pid: 1,
        pgid: 1,
        pid_start: 1,
        base_commit: None,
        run_dir: None,
    };
    let run_id = agent_brain::db::insert_run(&conn, &new_run).unwrap();
    conn.execute("UPDATE runs SET status = 'timed_out' WHERE id = ?1", [run_id]).unwrap();

    // But task is stopped, NOT ended
    let stop = agent_brain::autopilot::decide::StopInfo {
        reason: agent_brain::autopilot::decide::StopReason::RunLoi,
        loop_name: None,
        stage: None,
        round: None,
        cap: None,
        run_id: Some(run_id),
        role: Some("dev".to_string()),
        at_ms: 1000,
        detail_line: None,
        used_ms: None,
        used_usd: None,
        estimated: None,
        limit_minutes: None,
        limit_usd: None,
    };
    let aj = agent_brain::autopilot::decide::AutoJson {
        stop: Some(stop),
        ..Default::default()
    };
    conn.execute(
        "UPDATE tasks SET status = 'open', auto_state = 'stopped', auto_json = ?1 WHERE id = ?2",
        rusqlite::params![aj.to_json(), task_id],
    )
    .unwrap();

    set_proposal_status(&conn, p1, "approved").expect("approve p1");

    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("schedule_tick");

    let p1_after = get_proposal(&conn, p1).unwrap().unwrap();
    // p1 should still be approved, waiting on task, NOT failed
    assert_eq!(p1_after.status, "approved");
    assert!(p1_after.error.unwrap().contains(&format!("Chờ task #{task_id} (đang dừng: run lỗi)")));

    st.jobs.kill_all_now();
}

/// C2: a task stopped with `moi_truong_chua_san` on a project with `preflight_retry_minutes` is
/// re-checked when due and restarted with `auto <task>` once the environment answers; the
/// orchestrator does not need to be on. While the target is down nothing starts.
#[test]
fn test_env_stop_rerun_when_environment_is_back() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let repos = setup_projects(&sb, &["p1"]);
    let fake_job = setup_fake_job(&sb);
    sb.write_config(json!({"orchestrator": {"enabled": false}}));

    let dead = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let write_map = |port: u16| {
        let mut entry = sb.project_entry(&repos[0], "p1", "work");
        entry["preflight"] = json!([{"kind": "tcp", "host": "127.0.0.1", "port": port, "timeout_seconds": 1}]);
        entry["preflight_retry_minutes"] = json!(5);
        sb.write_map(json!([entry]), &[]);
    };
    write_map(dead);

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).expect("load map");
    agent_brain::db::sync_projects(&conn, &agent_brain::projects::projects_of(&map)).unwrap();
    let pid1 = agent_brain::db::project_row_id(&conn, "p1").unwrap();
    let t1 = agent_brain::db::upsert_task(&conn, pid1, "env-task", "Env", "env-task", Some("task/env-task"), None)
        .unwrap();
    let now = agent_brain::db::now_ms();
    let stopped_at = now - 6 * 60 * 1000;
    let aj = json!({"stop": {"reason": "moi_truong_chua_san", "at_ms": stopped_at, "detail_line": "x"}});
    conn.execute(
        "UPDATE tasks SET auto_state = 'stopped', auto_json = ?1 WHERE id = ?2",
        rusqlite::params![aj.to_string(), t1],
    )
    .unwrap();

    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("tick down");
    assert!(st.jobs.list().is_empty(), "target down: nothing starts");

    // Back up: the next check is due 5 minutes after the last one.
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    write_map(l.local_addr().unwrap().port());
    let map = agent_brain::projects::load_map(&sb.map_path()).unwrap();
    let started =
        agent_brain::orchestrator::scheduler::handle_env_stops(&st, &conn, &map, now + 60_000).unwrap();
    assert!(started.is_empty(), "not due yet");
    let started =
        agent_brain::orchestrator::scheduler::handle_env_stops(&st, &conn, &map, now + 6 * 60_000).unwrap();
    assert_eq!(started, vec![t1]);
    let jobs = st.jobs.list();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].role, "auto");
    assert_eq!(jobs[0].task_id, Some(t1));
    let aj_after: String = conn
        .query_row("SELECT auto_json FROM tasks WHERE id = ?1", [t1], |r| r.get(0))
        .unwrap();
    let aj_after: serde_json::Value = serde_json::from_str(&aj_after).unwrap();
    assert_eq!(aj_after["env_rerun_at"], json!(now + 6 * 60_000));
    drop(l);
    st.jobs.kill_all_now();
}

/// C2: without `preflight_retry_minutes` an env stop stays stopped (no job, nothing opened).
#[test]
fn test_env_stop_without_retry_stays_stopped() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repos = setup_projects(&sb, &["p1"]);
    let fake_job = setup_fake_job(&sb);
    sb.write_config(json!({"orchestrator": {"enabled": false}}));
    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).unwrap();
    agent_brain::db::sync_projects(&conn, &agent_brain::projects::projects_of(&map)).unwrap();
    let pid1 = agent_brain::db::project_row_id(&conn, "p1").unwrap();
    let t1 = agent_brain::db::upsert_task(&conn, pid1, "env-task", "Env", "env-task", Some("task/env-task"), None)
        .unwrap();
    let aj = json!({"stop": {"reason": "moi_truong_chua_san", "at_ms": 1, "detail_line": "x"}});
    conn.execute(
        "UPDATE tasks SET auto_state = 'stopped', auto_json = ?1 WHERE id = ?2",
        rusqlite::params![aj.to_string(), t1],
    )
    .unwrap();
    let st = app_state(&sb, fake_job);
    schedule_tick(&st).expect("tick");
    assert!(st.jobs.list().is_empty());
}
