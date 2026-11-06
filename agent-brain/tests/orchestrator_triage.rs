mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::setup_project;
use serde_json::json;
use std::sync::Arc;
use agent_brain::dashboard::jobs::JobManager;
use agent_brain::dashboard::AppState;
use agent_brain::orchestrator::triage::{validate, ProposalDraft};

static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct EnvVarGuard(&'static str);
impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        std::env::remove_var(self.0);
    }
}

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

/// Acceptance 1: Intake queued → (khi bật enabled) thành proposed với các proposal pending đã validate.
#[test]
fn test_triage_success_turns_intake_to_proposed() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "orchestrator": {
            "enabled": true
        }
    }));
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let intake_id = agent_brain::db::insert_intake(
        &conn,
        "ui",
        None,
        "Thêm trang giới thiệu cho web",
        "normal",
    )
    .expect("insert intake");

    // Stub engine trả về đề xuất hợp lệ
    let fake_result = json!({
        "tasks": [
            {
                "project": "web-ban-hang",
                "title": "Thêm trang giới thiệu",
                "request": "Tạo trang about.html và link trên header",
                "priority": "normal",
                "team": "mac-dinh",
                "workflow": "mac-dinh",
                "reason": "Dự án web phù hợp đội hình mặc định"
            }
        ]
    })
    .to_string();

    sb.write_scenario(json!({
        "events": [
            {
                "type": "result",
                "subtype": "success",
                "result": fake_result
            }
        ]
    }));

    let st = app_state(&sb);
    agent_brain::orchestrator::tick(&st).expect("tick");

    // Kiểm tra trạng thái intake chuyển sang proposed
    let intake = agent_brain::db::get_intake(&conn, intake_id)
        .expect("get intake")
        .expect("some intake");
    assert_eq!(intake.status, "proposed");
    assert!(intake.error.is_none());

    // Kiểm tra proposal được tạo với status pending và dữ liệu chính xác
    let proposals = agent_brain::db::list_proposals(&conn, None).expect("list proposals");
    assert_eq!(proposals.len(), 1);
    let p = &proposals[0];
    assert_eq!(p.intake_id, intake_id);
    assert_eq!(p.title, "Thêm trang giới thiệu");
    assert_eq!(p.request, "Tạo trang about.html và link trên header");
    assert_eq!(p.priority, "normal");
    assert_eq!(p.team, "mac-dinh");
    assert_eq!(p.workflow, "mac-dinh");
    assert_eq!(p.status, "pending");
    assert_eq!(
        p.reason.as_deref(),
        Some("Dự án web phù hợp đội hình mặc định")
    );
    assert!(p.task_id.is_none());
}

/// Acceptance 2a: Output sai định dạng → intake failed, không có proposal.
#[test]
fn test_triage_malformed_json_fails_intake() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "orchestrator": {
            "enabled": true
        }
    }));
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let intake_id = agent_brain::db::insert_intake(
        &conn,
        "ui",
        None,
        "Yêu cầu bất kỳ",
        "normal",
    )
    .expect("insert intake");

    // Engine trả văn bản không phải JSON hợp lệ
    sb.write_scenario(json!({
        "raw_lines": [
            "Xin lỗi, tôi không thể phân tích yêu cầu này."
        ]
    }));

    let st = app_state(&sb);
    agent_brain::orchestrator::tick(&st).expect("tick");

    let intake = agent_brain::db::get_intake(&conn, intake_id)
        .expect("get intake")
        .expect("intake");
    assert_eq!(intake.status, "failed");
    assert!(intake.error.is_some());

    let proposals = agent_brain::db::list_proposals(&conn, None).expect("list proposals");
    assert_eq!(proposals.len(), 0);
}

/// Acceptance 2b: Đội hình không tồn tại → intake failed, không có proposal.
#[test]
fn test_triage_nonexistent_team_fails_intake() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "orchestrator": {
            "enabled": true
        }
    }));
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let intake_id = agent_brain::db::insert_intake(
        &conn,
        "ui",
        None,
        "Làm việc A",
        "normal",
    )
    .expect("insert intake");

    // Engine trả đội hình 'khong-ton-tai'
    let fake_result = json!({
        "tasks": [
            {
                "project": "web-ban-hang",
                "title": "Task A",
                "request": "Yêu cầu A",
                "priority": "normal",
                "team": "khong-ton-tai",
                "workflow": "mac-dinh",
                "reason": "Test"
            }
        ]
    })
    .to_string();

    sb.write_scenario(json!({
        "events": [
            {
                "type": "result",
                "subtype": "success",
                "result": fake_result
            }
        ]
    }));

    let st = app_state(&sb);
    agent_brain::orchestrator::tick(&st).expect("tick");

    let intake = agent_brain::db::get_intake(&conn, intake_id)
        .expect("get intake")
        .expect("intake");
    assert_eq!(intake.status, "failed");
    assert!(intake
        .error
        .as_deref()
        .unwrap_or("")
        .contains("khong-ton-tai"));

    let proposals = agent_brain::db::list_proposals(&conn, None).expect("list proposals");
    assert_eq!(proposals.len(), 0);
}

/// Acceptance 2c: Nhiều mục, mục sai bị bỏ ghi reason vào intake error; mục đúng được tạo.
#[test]
fn test_triage_partial_valid_keeps_valid_and_records_warning() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "orchestrator": {
            "enabled": true
        }
    }));
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let intake_id = agent_brain::db::insert_intake(
        &conn,
        "ui",
        None,
        "Làm 2 việc",
        "normal",
    )
    .expect("insert intake");

    // 2 tasks: task 1 sai đội hình, task 2 hợp lệ
    let fake_result = json!({
        "tasks": [
            {
                "project": "web-ban-hang",
                "title": "Task Sai Team",
                "request": "Yêu cầu 1",
                "priority": "normal",
                "team": "doi-hinh-ao",
                "workflow": "mac-dinh",
                "reason": "Sai"
            },
            {
                "project": "web-ban-hang",
                "title": "Task Dung",
                "request": "Yêu cầu 2",
                "priority": "urgent",
                "team": "mac-dinh",
                "workflow": "mac-dinh",
                "reason": "Dung"
            }
        ]
    })
    .to_string();

    sb.write_scenario(json!({
        "events": [
            {
                "type": "result",
                "subtype": "success",
                "result": fake_result
            }
        ]
    }));

    let st = app_state(&sb);
    agent_brain::orchestrator::tick(&st).expect("tick");

    let intake = agent_brain::db::get_intake(&conn, intake_id)
        .expect("get intake")
        .expect("intake");
    assert_eq!(intake.status, "proposed");
    assert!(intake
        .error
        .as_deref()
        .unwrap_or("")
        .contains("Task Sai Team"));

    let proposals = agent_brain::db::list_proposals(&conn, None).expect("list proposals");
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].title, "Task Dung");
    assert_eq!(proposals[0].priority, "urgent");
}

/// Acceptance 3: Không có dòng nào mới trong tasks hay runs sau khi triage.
#[test]
fn test_triage_creates_no_rows_in_tasks_or_runs() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "orchestrator": {
            "enabled": true
        }
    }));
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let _intake_id = agent_brain::db::insert_intake(
        &conn,
        "ui",
        None,
        "Yêu cầu kiểm tra tasks và runs",
        "normal",
    )
    .expect("insert intake");

    let fake_result = json!({
        "tasks": [
            {
                "project": "web-ban-hang",
                "title": "Kiểm tra tasks và runs",
                "request": "Mô tả",
                "priority": "normal",
                "team": "mac-dinh",
                "workflow": "mac-dinh",
                "reason": "Test"
            }
        ]
    })
    .to_string();

    sb.write_scenario(json!({
        "events": [
            {
                "type": "result",
                "subtype": "success",
                "result": fake_result
            }
        ]
    }));

    let tasks_before: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .expect("count tasks");
    let runs_before: i64 = conn
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .expect("count runs");

    let st = app_state(&sb);
    agent_brain::orchestrator::tick(&st).expect("tick");

    let tasks_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .expect("count tasks");
    let runs_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .expect("count runs");

    assert_eq!(tasks_before, 0);
    assert_eq!(tasks_after, 0);
    assert_eq!(runs_before, 0);
    assert_eq!(runs_after, 0);
}

/// Acceptance 4: enabled = false → vòng lặp không làm gì.
#[test]
fn test_triage_disabled_does_nothing() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "orchestrator": {
            "enabled": false
        }
    }));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let intake_id = agent_brain::db::insert_intake(
        &conn,
        "ui",
        None,
        "Yêu cầu khi tắt orchestrator",
        "normal",
    )
    .expect("insert intake");

    let st = app_state(&sb);
    agent_brain::orchestrator::tick(&st).expect("tick");

    let intake = agent_brain::db::get_intake(&conn, intake_id)
        .expect("get intake")
        .expect("intake");
    assert_eq!(intake.status, "queued");

    let proposals = agent_brain::db::list_proposals(&conn, None).expect("list proposals");
    assert_eq!(proposals.len(), 0);
}

/// Live settings: `tick` reads `AppState::orch()`, so turning the queue off and on at
/// runtime (the Cài đặt switch) takes effect without rebuilding the state.
#[test]
fn test_triage_follows_live_toggle() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "orchestrator": {
            "enabled": true
        }
    }));
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let intake_id = agent_brain::db::insert_intake(
        &conn,
        "ui",
        None,
        "Thêm trang giới thiệu cho web",
        "normal",
    )
    .expect("insert intake");
    let fake_result = json!({
        "tasks": [
            {
                "project": "web-ban-hang",
                "title": "Thêm trang giới thiệu",
                "request": "Tạo trang about.html",
                "priority": "normal",
                "team": "mac-dinh",
                "workflow": "mac-dinh",
                "reason": "Phù hợp"
            }
        ]
    })
    .to_string();
    sb.write_scenario(json!({
        "events": [
            { "type": "result", "subtype": "success", "result": fake_result }
        ]
    }));

    let st = app_state(&sb);
    let mut off = st.orch();
    off.enabled = false;
    st.set_orch(off);
    assert!(!st.orch().enabled);

    agent_brain::orchestrator::tick(&st).expect("tick while off");
    let intake = agent_brain::db::get_intake(&conn, intake_id)
        .expect("get intake")
        .expect("intake");
    assert_eq!(intake.status, "queued", "off: queued intake stays queued");
    assert!(agent_brain::db::list_proposals(&conn, None)
        .expect("list proposals")
        .is_empty());

    let mut on = st.orch();
    on.enabled = true;
    st.set_orch(on);
    agent_brain::orchestrator::tick(&st).expect("tick while on");
    let intake = agent_brain::db::get_intake(&conn, intake_id)
        .expect("get intake")
        .expect("intake");
    assert_eq!(intake.status, "proposed", "on: the queued intake is triaged");
    assert_eq!(
        agent_brain::db::list_proposals(&conn, None)
            .expect("list proposals")
            .len(),
        1
    );
}

/// Test thứ tự ưu tiên: intake queued urgent được xử lý trước normal.
#[test]
fn test_triage_priority_ordering_urgent_first() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "orchestrator": {
            "enabled": true
        }
    }));
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let intake_normal = agent_brain::db::insert_intake(
        &conn,
        "ui",
        None,
        "Việc thường tạo trước",
        "normal",
    )
    .expect("insert intake normal");

    let intake_urgent = agent_brain::db::insert_intake(
        &conn,
        "ui",
        None,
        "Việc gấp tạo sau",
        "urgent",
    )
    .expect("insert intake urgent");

    let fake_result_urgent = json!({
        "tasks": [
            {
                "project": "web-ban-hang",
                "title": "Việc gấp",
                "request": "Xử lý gấp",
                "priority": "urgent",
                "team": "mac-dinh",
                "workflow": "mac-dinh",
                "reason": "Gấp"
            }
        ]
    })
    .to_string();

    sb.write_scenario(json!({
        "events": [
            {
                "type": "result",
                "subtype": "success",
                "result": fake_result_urgent
            }
        ]
    }));

    let st = app_state(&sb);
    // Nhịp 1: xử lý việc urgent trước
    agent_brain::orchestrator::tick(&st).expect("tick 1");

    let urgent_row = agent_brain::db::get_intake(&conn, intake_urgent)
        .expect("get intake")
        .expect("row");
    let normal_row = agent_brain::db::get_intake(&conn, intake_normal)
        .expect("get intake")
        .expect("row");

    assert_eq!(urgent_row.status, "proposed");
    assert_eq!(normal_row.status, "queued");

    // Nhịp 2: xử lý việc normal
    let fake_result_normal = json!({
        "tasks": [
            {
                "project": "web-ban-hang",
                "title": "Việc thường",
                "request": "Xử lý thường",
                "priority": "normal",
                "team": "mac-dinh",
                "workflow": "mac-dinh",
                "reason": "Thường"
            }
        ]
    })
    .to_string();

    sb.write_scenario(json!({
        "events": [
            {
                "type": "result",
                "subtype": "success",
                "result": fake_result_normal
            }
        ]
    }));

    agent_brain::orchestrator::tick(&st).expect("tick 2");

    let normal_row = agent_brain::db::get_intake(&conn, intake_normal)
        .expect("get intake")
        .expect("row");
    assert_eq!(normal_row.status, "proposed");
}

/// Unit test các quy tắc trong validate(draft)
#[test]
fn test_validate_rules() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let st = app_state(&sb);

    let map = agent_brain::projects::load_map(&st.map_path).expect("load map");
    let projects = agent_brain::projects::projects_of(&map);
    let (teams, _) = agent_brain::teams::load();
    let workflows = agent_brain::workflows::load(&st.cfg);

    let valid_draft = ProposalDraft {
        project: "web-ban-hang".to_string(),
        title: "Tieu de hop le".to_string(),
        request: "Yeu cau hop le".to_string(),
        priority: "normal".to_string(),
        team: "mac-dinh".to_string(),
        workflow: "mac-dinh".to_string(),
        reason: Some("Ly do".to_string()),
    };
    assert!(validate(&valid_draft, &projects, &teams, &workflows, &st.cfg.engines).is_ok());

    // 1. Project không tồn tại
    let mut d = valid_draft.clone();
    d.project = "du-an-la".to_string();
    assert!(validate(&d, &projects, &teams, &workflows, &st.cfg.engines).is_err());

    // 2. Team không tồn tại
    let mut d = valid_draft.clone();
    d.team = "doi-la".to_string();
    assert!(validate(&d, &projects, &teams, &workflows, &st.cfg.engines).is_err());

    // 3. Workflow không tồn tại
    let mut d = valid_draft.clone();
    d.workflow = "quy-trinh-la".to_string();
    assert!(validate(&d, &projects, &teams, &workflows, &st.cfg.engines).is_err());

    // 4. Priority sai
    let mut d = valid_draft.clone();
    d.priority = "cao".to_string();
    assert!(validate(&d, &projects, &teams, &workflows, &st.cfg.engines).is_err());

    // 5. Title rỗng hoặc quá 200 ký tự
    let mut d = valid_draft.clone();
    d.title = "   ".to_string();
    assert!(validate(&d, &projects, &teams, &workflows, &st.cfg.engines).is_err());

    let mut d = valid_draft.clone();
    d.title = "a".repeat(201);
    assert!(validate(&d, &projects, &teams, &workflows, &st.cfg.engines).is_err());

    // 6. Request rỗng hoặc quá 20_000 ký tự
    let mut d = valid_draft.clone();
    d.request = "   ".to_string();
    assert!(validate(&d, &projects, &teams, &workflows, &st.cfg.engines).is_err());

    let mut d = valid_draft.clone();
    d.request = "a".repeat(20_001);
    assert!(validate(&d, &projects, &teams, &workflows, &st.cfg.engines).is_err());
}

#[test]
fn test_triage_engine_runs_with_disallowed_tools() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "orchestrator": {
            "enabled": true
        }
    }));
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);
    std::env::set_var("FAKE_CLAUDE_RECORD", &sb.record_file);
    let _rec_guard = EnvVarGuard("FAKE_CLAUDE_RECORD");

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let _intake_id = agent_brain::db::insert_intake(
        &conn,
        "ui",
        None,
        "Thêm trang liên hệ",
        "normal",
    )
    .expect("insert intake");

    sb.write_scenario(json!({
        "events": [
            {
                "type": "result",
                "subtype": "success",
                "result": json!({
                    "tasks": [
                        {
                            "project": "web-ban-hang",
                            "title": "Trang lien he",
                            "request": "Tao trang lien he",
                            "priority": "normal",
                            "team": "mac-dinh",
                            "workflow": "mac-dinh",
                            "reason": "hop le"
                        }
                    ]
                }).to_string()
            }
        ]
    }));

    let st = app_state(&sb);
    agent_brain::orchestrator::tick(&st).expect("tick");

    let records = sb.records();
    assert_eq!(records.len(), 1, "expected 1 engine invocation");
    let argv = records[0]["argv"].as_array().expect("argv array");
    let argv_strs: Vec<&str> = argv.iter().filter_map(|v| v.as_str()).collect();

    let disallowed_idx = argv_strs
        .iter()
        .position(|&arg| arg == "--disallowedTools")
        .expect("argv must contain --disallowedTools");

    assert!(argv_strs.contains(&"Read"), "disallowed tools must contain Read");
    assert!(argv_strs.contains(&"Bash"), "disallowed tools must contain Bash");
    assert!(argv_strs.contains(&"WebFetch"), "disallowed tools must contain WebFetch");

    let read_idx = argv_strs.iter().position(|&arg| arg == "Read").unwrap();
    let bash_idx = argv_strs.iter().position(|&arg| arg == "Bash").unwrap();
    let webfetch_idx = argv_strs.iter().position(|&arg| arg == "WebFetch").unwrap();
    assert!(read_idx > disallowed_idx);
    assert!(bash_idx > disallowed_idx);
    assert!(webfetch_idx > disallowed_idx);
}

