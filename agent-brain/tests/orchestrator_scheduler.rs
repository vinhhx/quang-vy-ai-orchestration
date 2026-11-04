mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::dashboard::jobs::JobManager;
use agent_brain::dashboard::AppState;
use agent_brain::db::{
    count_active_tasks, get_proposal, insert_intake, insert_proposal,
    list_quota_stops, set_proposal_status, NewProposal,
};
use agent_brain::orchestrator::scheduler::schedule_tick;
use common::Sandbox;
use serde_json::json;
use std::path::PathBuf;
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
