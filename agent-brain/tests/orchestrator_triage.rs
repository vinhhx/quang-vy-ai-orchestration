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
        level: "vua".to_string(),
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

    // 7. Level không hợp lệ
    let mut d = valid_draft.clone();
    d.level = "x".to_string();
    assert!(validate(&d, &projects, &teams, &workflows, &st.cfg.engines).is_err());

    let mut d = valid_draft.clone();
    d.level = "de".to_string();
    assert!(validate(&d, &projects, &teams, &workflows, &st.cfg.engines).is_ok());

    let mut d = valid_draft.clone();
    d.level = "kho".to_string();
    assert!(validate(&d, &projects, &teams, &workflows, &st.cfg.engines).is_ok());

    // JSON thiếu level tự mặc định "vua"
    let json_no_level = serde_json::json!({
        "project": "web-ban-hang",
        "title": "Tieu de",
        "request": "Yeu cau",
        "team": "mac-dinh",
        "workflow": "mac-dinh"
    });
    let parsed: ProposalDraft = serde_json::from_value(json_no_level).unwrap();
    assert_eq!(parsed.level, "vua");
    assert_eq!(parsed.priority, "normal");
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

// ---------------------------------------------------------------------------
// Plan triage-engine: a selectable triage engine and a clear failure reason
// ---------------------------------------------------------------------------

/// The stream claude wrote for an expired login (2026-11-06, ids scrubbed).
fn auth_failed_lines() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/triage/claude_auth_failed.jsonl");
    std::fs::read_to_string(path)
        .expect("read auth fixture")
        .lines()
        .map(str::to_string)
        .collect()
}

fn fake_agy() -> std::path::PathBuf {
    common::repo_root()
        .join("tests/fakes/fake_agy.py")
        .canonicalize()
        .expect("canonicalize fake_agy.py")
}

fn valid_tasks_json() -> String {
    json!({
        "tasks": [{
            "project": "web-ban-hang",
            "title": "Trang gioi thieu",
            "request": "Tao trang gioi thieu",
            "priority": "normal",
            "team": "mac-dinh",
            "workflow": "mac-dinh",
            "reason": "hop le"
        }]
    })
    .to_string()
}

fn queued(sb: &Sandbox, request: &str) -> i64 {
    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    agent_brain::db::insert_intake(&conn, "ui", None, request, "normal").expect("insert intake")
}

fn intake_of(sb: &Sandbox, id: i64) -> agent_brain::db::IntakeRow {
    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    agent_brain::db::get_intake(&conn, id)
        .expect("get intake")
        .expect("intake")
}

/// (b) An `anthropic` engine (fake claude) whose login expired: the intake fails with claude's
/// own text and the re-login hint naming the engine's account directory.
#[test]
fn test_triage_auth_failed_engine_reports_reason_and_login_hint() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let account = sb.root.join("claude-max-account");
    std::fs::create_dir_all(&account).unwrap();
    sb.write_config(json!({
        "engines": [{
            "id": "claude-max",
            "kind": "anthropic",
            "command": common::Sandbox::fake_claude().to_str().unwrap(),
            "claude_config_dir": account.to_str().unwrap()
        }],
        "orchestrator": {"enabled": true, "triage_engine": "claude-max"}
    }));
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);
    std::env::set_var("FAKE_CLAUDE_RECORD", &sb.record_file);
    let _rec_guard = EnvVarGuard("FAKE_CLAUDE_RECORD");
    sb.write_scenario(json!({"raw_lines": auth_failed_lines(), "exit_code": 1}));

    let id = queued(&sb, "Thêm trang giới thiệu");
    let st = app_state(&sb);
    assert_eq!(st.orch().triage_engine, "claude-max");
    agent_brain::orchestrator::tick(&st).expect("tick");

    let intake = intake_of(&sb, id);
    assert_eq!(intake.status, "failed");
    let err = intake.error.expect("error");
    assert!(
        err.starts_with("Triage (claude-max) lỗi: Failed to authenticate: this login reached its time limit"),
        "{err}"
    );
    assert!(
        err.contains(&format!(
            "— đăng nhập lại: CLAUDE_CONFIG_DIR={} claude auth login, hoặc đổi engine triage trong Cài đặt → Hàng đợi",
            account.display()
        )),
        "{err}"
    );
    // The engine's own account was used, not the project profile's.
    let records = sb.records();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0]["claude_config_dir"].as_str(),
        Some(account.to_str().unwrap())
    );
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    assert!(agent_brain::db::list_proposals(&conn, None).unwrap().is_empty());
}

/// (b) The default `claude` engine fails the same way on the project's profile account.
#[test]
fn test_triage_default_claude_auth_failed_names_profile_account() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "personal");
    sb.write_config(json!({"orchestrator": {"enabled": true}}));
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);
    sb.write_scenario(json!({"raw_lines": auth_failed_lines(), "exit_code": 1}));

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = agent_brain::projects::load_map(&sb.map_path()).unwrap();
    agent_brain::db::sync_projects(&conn, &agent_brain::projects::projects_of(&map)).unwrap();
    let pid = agent_brain::db::project_row_id(&conn, "web-ban-hang").unwrap();
    let id = agent_brain::db::insert_intake(&conn, "telegram", Some(pid), "sửa app", "normal")
        .unwrap();

    let st = app_state(&sb);
    agent_brain::orchestrator::tick(&st).expect("tick");

    let err = intake_of(&sb, id).error.expect("error");
    assert!(err.starts_with("Triage (claude) lỗi: Failed to authenticate"), "{err}");
    assert!(
        err.contains(&format!(
            "CLAUDE_CONFIG_DIR={} claude auth login",
            sb.personal_dir.display()
        )),
        "{err}"
    );
}

/// (c) agy as the triage engine: read-only (no permission flag), the JSON comes from
/// `result.response`, and an agy that hangs after its result still finishes the triage.
#[test]
fn test_triage_agy_engine_proposes_and_survives_hang_after_result() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "engines": [{"id": "agy", "kind": "agy", "command": fake_agy().to_str().unwrap()}],
        "orchestrator": {"enabled": true, "triage_engine": "agy"}
    }));
    let scenario = sb.root.join("fake_agy_scenario.json");
    let record = sb.root.join("fake_agy_record.jsonl");
    std::fs::write(
        &scenario,
        json!({"response": valid_tasks_json(), "hang_s": 120}).to_string(),
    )
    .unwrap();
    std::env::set_var("FAKE_AGY_SCENARIO", &scenario);
    let _sc_guard = EnvVarGuard("FAKE_AGY_SCENARIO");
    std::env::set_var("FAKE_AGY_RECORD", &record);
    let _rec_guard = EnvVarGuard("FAKE_AGY_RECORD");

    let id = queued(&sb, "Thêm trang giới thiệu");
    let st = app_state(&sb);
    let started = std::time::Instant::now();
    agent_brain::orchestrator::tick(&st).expect("tick");
    let took = started.elapsed();

    let intake = intake_of(&sb, id);
    assert_eq!(intake.status, "proposed", "{:?}", intake.error);
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let proposals = agent_brain::db::list_proposals(&conn, None).unwrap();
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].title, "Trang gioi thieu");
    // Ended by the after-result grace (15 s), well before the 30 s timeout.
    assert!(took < std::time::Duration::from_secs(28), "took {took:?}");

    let text = std::fs::read_to_string(&record).unwrap();
    let rec: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(rec["permission"], "request-review");
    let argv: Vec<String> = rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(!argv.iter().any(|a| a == "--mode" || a == "--dangerously-skip-permissions"));
    let prompt = &argv[argv.iter().position(|a| a == "-p").unwrap() + 1];
    assert!(prompt.contains("Bạn là Agent điều phối"), "{prompt}");
    assert!(prompt.contains("DUY NHẤT một JSON"), "{prompt}");
    assert!(!prompt.contains(".plan.md"), "{prompt}");
    assert!(prompt.contains("Thêm trang giới thiệu"));
}

/// (b/agy) agy answering with a non-SUCCESS result: the intake carries status and error.
#[test]
fn test_triage_agy_error_result_reports_status_and_error() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "engines": [{"id": "agy", "kind": "agy", "command": fake_agy().to_str().unwrap()}],
        "orchestrator": {"enabled": true, "triage_engine": "agy"}
    }));
    let scenario = sb.root.join("fake_agy_scenario.json");
    std::fs::write(
        &scenario,
        json!({"status": "ERROR", "error": "invalid model selection (--model \"x\")", "response": ""})
            .to_string(),
    )
    .unwrap();
    std::env::set_var("FAKE_AGY_SCENARIO", &scenario);
    let _sc_guard = EnvVarGuard("FAKE_AGY_SCENARIO");

    let id = queued(&sb, "Việc bất kỳ");
    let st = app_state(&sb);
    agent_brain::orchestrator::tick(&st).expect("tick");

    let intake = intake_of(&sb, id);
    assert_eq!(intake.status, "failed");
    assert_eq!(
        intake.error.as_deref(),
        Some("Triage (agy) lỗi: ERROR: invalid model selection (--model \"x\")")
    );
}

/// The triage engine follows the live setting (Cài đặt saves without a restart).
#[test]
fn test_triage_engine_follows_live_setting() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({
        "engines": [{"id": "agy", "kind": "agy", "command": fake_agy().to_str().unwrap()}],
        "orchestrator": {"enabled": true}
    }));
    let scenario = sb.root.join("fake_agy_scenario.json");
    std::fs::write(&scenario, json!({"response": valid_tasks_json()}).to_string()).unwrap();
    std::env::set_var("FAKE_AGY_SCENARIO", &scenario);
    let _sc_guard = EnvVarGuard("FAKE_AGY_SCENARIO");
    // claude would fail: only agy can produce the proposal.
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);
    sb.write_scenario(json!({"raw_lines": auth_failed_lines(), "exit_code": 1}));

    let id = queued(&sb, "Thêm trang giới thiệu");
    let st = app_state(&sb);
    let mut live = st.orch();
    live.triage_engine = "agy".to_string();
    st.set_orch(live);
    agent_brain::orchestrator::tick(&st).expect("tick");
    assert_eq!(intake_of(&sb, id).status, "proposed");
}

/// Malformed output keeps a clear, prefixed reason.
#[test]
fn test_triage_malformed_output_reason_is_prefixed() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({"orchestrator": {"enabled": true}}));
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);
    sb.write_scenario(json!({"raw_lines": ["Xin lỗi, không phân tích được."]}));

    let id = queued(&sb, "Yêu cầu bất kỳ");
    let st = app_state(&sb);
    agent_brain::orchestrator::tick(&st).expect("tick");
    assert_eq!(
        intake_of(&sb, id).error.as_deref(),
        Some("Triage (claude) lỗi: Output của engine không đúng định dạng JSON đề xuất (yêu cầu {\"tasks\":[...]})")
    );
}

/// Acceptance Mục 0: có departments.json có thành viên → prompt chứa phong-ban (mặc định); không có → mac-dinh là mặc định.
#[test]
fn test_triage_prompt_marks_default_team() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({"orchestrator": {"enabled": true}}));
    std::env::set_var("FAKE_CLAUDE_SCENARIO", &sb.scenario_file);
    std::env::set_var("FAKE_CLAUDE_RECORD", &sb.record_file);
    let _sc_guard = EnvVarGuard("FAKE_CLAUDE_SCENARIO");
    let _rec_guard = EnvVarGuard("FAKE_CLAUDE_RECORD");

    // 1. Without departments.json: mac-dinh is default
    let tmp_cfg1 = tempfile::tempdir().unwrap();
    std::env::set_var("AGENT_BRAIN_CONFIG_DIR", tmp_cfg1.path());
    let _cfg_guard = EnvVarGuard("AGENT_BRAIN_CONFIG_DIR");

    let fake_result = valid_tasks_json();
    sb.write_scenario(json!({
        "events": [
            {
                "type": "result",
                "subtype": "success",
                "result": fake_result
            }
        ]
    }));
    let id1 = queued(&sb, "Yêu cầu 1");
    let st = app_state(&sb);
    agent_brain::orchestrator::tick(&st).expect("tick 1");
    let in1 = intake_of(&sb, id1);
    assert_eq!(in1.status, "proposed");

    let records1 = std::fs::read_to_string(&sb.record_file).expect("read record 1");
    let last1: serde_json::Value = serde_json::from_str(records1.lines().last().unwrap()).unwrap();
    let argv1 = last1["argv"].as_array().unwrap();
    let prompt_idx1 = argv1
        .iter()
        .position(|x| x == "--append-system-prompt" || x == "--system-prompt")
        .unwrap();
    let system_prompt1 = argv1[prompt_idx1 + 1].as_str().unwrap();

    assert!(
        system_prompt1.contains("- tên: \"mac-dinh\"") && system_prompt1.contains("\"mac-dinh\", mô tả: \"Mọi vai trò chạy bằng claude theo profile của dự án.\" (mặc định)"),
        "mac-dinh should be marked default without departments. Prompt:\n{system_prompt1}"
    );
    assert!(
        !system_prompt1.contains("- tên: \"phong-ban\"") || !system_prompt1.contains("\"phong-ban\", mô tả: \"Chọn thành viên theo phòng ban và độ khó (departments.json); vai chưa có thành viên chạy claude.\" (mặc định)"),
        "phong-ban should NOT be marked default without departments"
    );

    // 2. With departments.json having members: phong-ban is default
    let dept_json = json!({
        "thi-cong": {
            "members": {
                "dev-1": {
                    "display_name": "Dev 1",
                    "engine": "claude",
                    "model": "claude-sonnet-5-5",
                    "levels": ["de", "vua", "kho"]
                }
            }
        }
    });
    let tmp_cfg2 = tempfile::tempdir().unwrap();
    std::fs::write(tmp_cfg2.path().join("departments.json"), dept_json.to_string()).unwrap();
    std::env::set_var("AGENT_BRAIN_CONFIG_DIR", tmp_cfg2.path());

    let _ = std::fs::remove_file(&sb.record_file);
    let id2 = queued(&sb, "Yêu cầu 2");
    agent_brain::orchestrator::tick(&st).expect("tick 2");
    assert_eq!(intake_of(&sb, id2).status, "proposed");

    let records2 = std::fs::read_to_string(&sb.record_file).expect("read record 2");
    let last2: serde_json::Value = serde_json::from_str(records2.lines().last().unwrap()).unwrap();
    let argv2 = last2["argv"].as_array().unwrap();
    let prompt_idx2 = argv2
        .iter()
        .position(|x| x == "--append-system-prompt" || x == "--system-prompt")
        .unwrap();
    let system_prompt2 = argv2[prompt_idx2 + 1].as_str().unwrap();

    assert!(
        system_prompt2.contains("\"phong-ban\", mô tả: \"Chọn thành viên theo phòng ban và độ khó (departments.json); vai chưa có thành viên chạy claude.\" (mặc định)"),
        "phong-ban should be marked default when departments have members. Prompt:\n{system_prompt2}"
    );
    assert!(
        !system_prompt2.contains("\"mac-dinh\", mô tả: \"Mọi vai trò chạy bằng claude theo profile của dự án.\" (mặc định)"),
        "mac-dinh should NOT be marked default when departments have members"
    );
}


