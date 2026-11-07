#[path = "support/fixbug.rs"]
mod support_fixbug;

use agent_brain::dashboard::jobs::JobManager;
use agent_brain::dashboard::AppState;
use agent_brain::db::{
    fixbug_ref_by_key, insert_fixbug_ref, insert_intake, open, sync_projects,
};
use agent_brain::fixbug::init::init;
use agent_brain::fixbug::writeback::{deployed, reject};
use agent_brain::fixbug::{cmd_check, cmd_deployed, cmd_init, cmd_list, cmd_pull, cmd_reject};
use agent_brain::projects::Project;
use serde_json::json;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use support_fixbug::FakeTrello;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn setup_sandbox(root: &Path) -> (rusqlite::Connection, Project) {
    let db_path = root.join("brain.db");
    let conn = open(&db_path).expect("open db");
    let project = Project {
        id: "demo".to_string(),
        path: root.to_string_lossy().to_string(),
        name_vi: "Dự án Demo".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "default".to_string(),
        repo: None,
        missing: false,
        stack: vec![],
        read_sources: vec![],
    };
    sync_projects(&conn, std::slice::from_ref(&project)).expect("sync projects");
    (conn, project)
}

fn write_mock_trello_config_and_env(
    root: &Path,
    fake_url: &str,
    board_id: &str,
    intake_list: &str,
    fixed_list: &str,
    rejected_list: &str,
) {
    let cfg_path = agent_brain::fixbug::config::config_path(root);
    std::fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
    let cfg = json!({
        "sources": [
            {
                "type": "trello",
                "board": board_id,
                "intake_lists": [intake_list],
                "fixed_list": fixed_list,
                "rejected_list": rejected_list,
                "labels": [],
                "priority_labels": { "urgent": ["high"], "low": [] },
                "comment_prefix": "[agent-brain]"
            }
        ]
    });
    std::fs::write(&cfg_path, cfg.to_string()).unwrap();

    let env_content = format!(
        "TRELLO_API_KEY=mock_key_123\nTRELLO_TOKEN=mock_token_456\nTRELLO_BOARD_ID={board_id}\n"
    );
    std::fs::write(root.join(".env.fixbugs"), env_content).unwrap();

    std::env::set_var("AGENT_BRAIN_TRELLO_API", fake_url);
}

#[test]
fn test_writeback_deployed_and_idempotency_trello() {
    let _env_lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let fake = FakeTrello::start(root);

    let board_id = "board_wb_1";
    fake.set_data(json!({
        "lists": [
            { "id": "list_bugs", "name": "Bugs", "closed": false },
            { "id": "list_testing", "name": "Testing", "closed": false },
            { "id": "list_reject", "name": "Reject", "closed": false }
        ],
        "cards": [
            {
                "id": "card_t3",
                "idShort": 3,
                "idList": "list_bugs",
                "name": "Lỗi crash đăng nhập",
                "desc": "Chi tiết crash",
                "labels": [],
                "closed": false
            },
            {
                "id": "card_t5",
                "idShort": 5,
                "idList": "list_testing",
                "name": "Thẻ đã ở Testing sẵn",
                "desc": "Đã di chuyển trước đó",
                "labels": [],
                "closed": false
            }
        ]
    }));

    write_mock_trello_config_and_env(root, &fake.url(), board_id, "Bugs", "Testing", "Reject");
    let (mut conn, _proj) = setup_sandbox(root);

    let intake_id = insert_intake(&conn, "trello", Some(1), "Sửa bug crash", "urgent").unwrap();
    insert_fixbug_ref(
        &conn,
        1,
        "trello",
        "card_t3",
        "T-3",
        "Lỗi crash đăng nhập",
        Some("https://trello.com/c/card_t3"),
        Some(intake_id),
    )
    .unwrap();

    // 1. Chạy deployed lần 1
    let items = deployed(
        &mut conn,
        root,
        1,
        &["T-3".to_string()],
        Some("đã fix trên staging"),
    )
    .expect("deployed ok");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "T-3");
    assert_eq!(items[0].state, "deployed");

    // Kiểm tra DB state
    let ref_row = fixbug_ref_by_key(&conn, 1, "T-3").unwrap().unwrap();
    assert_eq!(ref_row.state, "deployed");
    assert_eq!(ref_row.note.as_deref(), Some("đã fix trên staging"));

    // Kiểm tra fake trello nhận comment và move
    let records = fake.read_records();
    let comments: Vec<_> = records
        .iter()
        .filter(|r| r["path"] == "/1/cards/card_t3/actions/comments")
        .collect();
    assert_eq!(comments.len(), 1);
    assert!(comments[0]["body"].as_str().unwrap().contains("fix+tr"));

    let moves: Vec<_> = records
        .iter()
        .filter(|r| r["path"] == "/1/cards/card_t3" && r["method"] == "PUT")
        .collect();
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0]["query"]["idList"], "list_testing");

    // 2. Gọi lại khi ref đã deployed -> báo lỗi vì chỉ nhận state pulled
    let err = deployed(&mut conn, root, 1, &["T-3".to_string()], None).unwrap_err();
    assert!(err.to_string().contains("chỉ có thể deploy khi ở trạng thái 'pulled'"));

    // 3. Nếu ref là pulled nhưng card trên trello đã ở Testing -> chỉ đổi state, không thêm comment lặp
    let intake_id_2 = insert_intake(&conn, "trello", Some(1), "Bug 5", "normal").unwrap();
    insert_fixbug_ref(
        &conn,
        1,
        "trello",
        "card_t5", // card đã nằm ở Testing sẵn
        "T-5",
        "Thẻ đã ở Testing sẵn",
        None,
        Some(intake_id_2),
    )
    .unwrap();

    let records_count_before = fake.read_records().len();
    let items_5 = deployed(&mut conn, root, 1, &["T-5".to_string()], None).unwrap();
    assert_eq!(items_5.len(), 1);
    assert!(items_5[0].message.contains("Thẻ đã nằm trong list 'Testing'"));

    let records_after = fake.read_records();
    let new_comments: Vec<_> = records_after[records_count_before..]
        .iter()
        .filter(|r| r["path"] == "/1/cards/card_t5/actions/comments")
        .collect();
    assert_eq!(new_comments.len(), 0, "Không được comment lặp khi thẻ đã ở list đích");
}

#[test]
fn test_writeback_reject_trello() {
    let _env_lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let fake = FakeTrello::start(root);

    let board_id = "board_wb_reject";
    fake.set_data(json!({
        "lists": [
            { "id": "list_bugs", "name": "Bugs", "closed": false },
            { "id": "list_testing", "name": "Testing", "closed": false },
            { "id": "list_reject", "name": "Reject", "closed": false }
        ],
        "cards": [
            {
                "id": "card_t4",
                "idShort": 4,
                "idList": "list_bugs",
                "name": "Yêu cầu đổi màu nút",
                "desc": "Đổi sang màu hồng",
                "labels": [],
                "closed": false
            }
        ]
    }));

    write_mock_trello_config_and_env(root, &fake.url(), board_id, "Bugs", "Testing", "Reject");
    let (mut conn, _proj) = setup_sandbox(root);

    let intake_id = insert_intake(&conn, "trello", Some(1), "Đổi màu nút", "normal").unwrap();
    insert_fixbug_ref(
        &conn,
        1,
        "trello",
        "card_t4",
        "T-4",
        "Yêu cầu đổi màu nút",
        None,
        Some(intake_id),
    )
    .unwrap();

    // 1. Reject thiếu lý do -> lỗi
    let err = reject(&mut conn, root, 1, &["T-4".to_string()], "   ").unwrap_err();
    assert!(err.to_string().contains("Lý do từ chối sửa (--reason) là bắt buộc"));

    // 2. Reject có lý do -> chuyển cột Reject và thêm comment
    let items = reject(&mut conn, root, 1, &["T-4".to_string()], "Không thuộc phạm vi dự án").unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "T-4");
    assert_eq!(items[0].state, "rejected");

    let ref_row = fixbug_ref_by_key(&conn, 1, "T-4").unwrap().unwrap();
    assert_eq!(ref_row.state, "rejected");
    assert_eq!(ref_row.note.as_deref(), Some("Không thuộc phạm vi dự án"));

    let records = fake.read_records();
    let moves: Vec<_> = records
        .iter()
        .filter(|r| r["path"] == "/1/cards/card_t4" && r["method"] == "PUT")
        .collect();
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0]["query"]["idList"], "list_reject");
}

#[test]
fn test_writeback_gsheet_no_http() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    let cfg_path = agent_brain::fixbug::config::config_path(root);
    std::fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
    let cfg = json!({
        "sources": [
            {
                "type": "gsheet",
                "url": "https://docs.google.com/spreadsheets/d/abc/edit?gid=0#gid=0"
            }
        ]
    });
    std::fs::write(&cfg_path, cfg.to_string()).unwrap();

    let (mut conn, _proj) = setup_sandbox(root);

    let intake_1 = insert_intake(&conn, "sheet", Some(1), "Bug sheet 1", "normal").unwrap();
    insert_fixbug_ref(
        &conn,
        1,
        "sheet",
        "0:1",
        "S-1",
        "Bug sheet 1",
        None,
        Some(intake_1),
    )
    .unwrap();

    let intake_2 = insert_intake(&conn, "sheet", Some(1), "Bug sheet 2", "low").unwrap();
    insert_fixbug_ref(
        &conn,
        1,
        "sheet",
        "0:2",
        "S-2",
        "Bug sheet 2",
        None,
        Some(intake_2),
    )
    .unwrap();

    // Deployed S-1 -> state deployed, nhắc cập nhật thủ công
    let dep_items = deployed(&mut conn, root, 1, &["S-1".to_string()], Some("done")).unwrap();
    assert_eq!(dep_items[0].state, "deployed");
    assert!(dep_items[0].message.contains("thủ công"));
    let r1 = fixbug_ref_by_key(&conn, 1, "S-1").unwrap().unwrap();
    assert_eq!(r1.state, "deployed");

    // Reject S-2 -> state rejected, nhắc cập nhật thủ công
    let rej_items = reject(&mut conn, root, 1, &["S-2".to_string()], "Trùng bug").unwrap();
    assert_eq!(rej_items[0].state, "rejected");
    assert!(rej_items[0].message.contains("thủ công"));
    let r2 = fixbug_ref_by_key(&conn, 1, "S-2").unwrap().unwrap();
    assert_eq!(r2.state, "rejected");
}

#[test]
fn test_init_with_legacy_gsd_and_dryrun() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    // Tạo config cũ .gsd/fix-bugs/config.json mô phỏng bizsurvey
    let gsd_dir = root.join(".gsd/fix-bugs");
    std::fs::create_dir_all(&gsd_dir).unwrap();
    let legacy_json = json!({
        "trello": {
            "lists": {
                "intake": ["Bugs", "HTML", "Admin"],
                "fixed": "Testing",
                "rejected": "Reject"
            },
            "labels": ["v1", "v2"],
            "comment_prefix": "[fix-bugs]"
        }
    });
    std::fs::write(gsd_dir.join("config.json"), legacy_json.to_string()).unwrap();

    // 1. Dry run: không tạo file
    let proposed = init(root, true).expect("init dry-run");
    assert!(proposed.contains("\"intake_lists\""));
    assert!(proposed.contains("\"HTML\""));
    assert!(proposed.contains("\"[fix-bugs]\""));
    assert!(proposed.contains("\"v1\""));
    assert!(!root.join(".gsd/fix-bugs/fixbugs.json").exists());

    // 2. Chạy thật: tạo file .gsd/fix-bugs/fixbugs.json và không đụng config.json cũ
    let created = init(root, false).expect("init write");
    assert!(root.join(".gsd/fix-bugs/fixbugs.json").is_file());
    assert!(root.join(".gsd/fix-bugs/config.json").is_file());
    assert_eq!(created, proposed);

    // 3. Chạy lại khi đã có file -> báo lỗi không ghi đè
    let err = init(root, false).unwrap_err();
    assert!(err.to_string().contains("đã tồn tại, không ghi đè"));
}

#[test]
fn test_init_with_planning_only() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    let plan_dir = root.join(".planning");
    std::fs::create_dir_all(&plan_dir).unwrap();

    let _ = init(root, false).expect("init planning");
    assert!(root.join(".planning/fix-bugs/fixbugs.json").is_file());
}

#[test]
fn test_init_empty_project_defaults_to_agent_brain() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    let _ = init(root, false).expect("init empty");
    assert!(root.join(".agent-brain/fix-bugs/fixbugs.json").is_file());
}

fn http_request(
    port: u16,
    method: &str,
    path: &str,
    auth_token: Option<&str>,
    body: Option<&str>,
) -> (u16, serde_json::Value, String) {
    use std::io::{Read, Write};

    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to server");
    let body_str = body.unwrap_or("");
    let body_bytes = body_str.as_bytes();

    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n");
    if let Some(tok) = auth_token {
        req.push_str(&format!("Cookie: agent_brain_token={tok}\r\n"));
    }
    if method != "GET" && method != "HEAD" {
        req.push_str(&format!("Origin: http://127.0.0.1:{port}\r\n"));
    }
    if !body_str.is_empty() {
        req.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body_bytes.len()
        ));
    }
    req.push_str("\r\n");

    stream.write_all(req.as_bytes()).unwrap();
    if !body_bytes.is_empty() {
        stream.write_all(body_bytes).unwrap();
    }
    stream.flush().unwrap();

    let mut resp = Vec::new();
    stream.read_to_end(&mut resp).unwrap();
    let resp_str = String::from_utf8_lossy(&resp).to_string();

    let mut parts = resp_str.splitn(2, "\r\n\r\n");
    let header_part = parts.next().unwrap_or("");
    let body_part = parts.next().unwrap_or("");

    let status = header_part
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    let json_val = serde_json::from_str(body_part).unwrap_or(serde_json::Value::Null);
    (status, json_val, body_part.to_string())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fixbug_api_endpoints() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    let (_conn, _project) = setup_sandbox(root);

    // Cấu hình fixbugs.json và .env.fixbugs
    let cfg_path = agent_brain::fixbug::config::config_path(root);
    std::fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
    let cfg_json = json!({
        "sources": [
            {
                "type": "gsheet",
                "url": "https://docs.google.com/spreadsheets/d/mock_id/edit?gid=0#gid=0"
            }
        ]
    });
    std::fs::write(&cfg_path, cfg_json.to_string()).unwrap();

    // Ghi project map
    let map_path = root.join("projects_map.json");
    let map_json = json!({
        "projects": [{
            "id": "demo",
            "path": root.to_string_lossy(),
            "name_vi": "Dự án Demo"
        }]
    });
    std::fs::write(&map_path, map_json.to_string()).unwrap();

    let config_val: serde_json::Value =
        serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap();
    let config = agent_brain::config::config_from_value(&config_val).unwrap();

    let token = "f".repeat(64);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let state = Arc::new(AppState {
        orch: std::sync::RwLock::new(config.orchestrator.clone()),
        cfg: config,
        port,
        token: token.clone(),
        map_path,
        db_path: root.join("brain.db"),
        exe: PathBuf::from("agent-brain"),
        hints: tokio::sync::broadcast::channel(16).0,
        shutdown: tokio::sync::watch::channel(false).1,
        jobs: JobManager::new(PathBuf::from("agent-brain")),
    });

    let router = agent_brain::dashboard::router(state);
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // 1. GET /api/fixbug/demo không có token -> 401 Unauthorized
    let (code, _, _) = http_request(port, "GET", "/api/fixbug/demo", None, None);
    assert_eq!(code, 401);

    // 2. GET /api/fixbug/demo có token -> 200 OK, trả configured = true, không có token trong body
    let (code, body, raw_body) = http_request(port, "GET", "/api/fixbug/demo", Some(&token), None);
    assert_eq!(code, 200, "raw body: {raw_body}");
    assert_eq!(body["configured"], true);
    assert_eq!(body["config_path"], ".agent-brain/fix-bugs/fixbugs.json");
    assert_eq!(body["home"], "agent-brain");
    assert!(body["sources"].is_array());
    assert!(!raw_body.contains("TRELLO_TOKEN"));
    assert!(!raw_body.contains("TRELLO_API_KEY"));

    // Thêm bug ref vào DB để test deployed/reject qua API
    {
        let conn = open(&root.join("brain.db")).unwrap();
        let intake_id = insert_intake(&conn, "sheet", Some(1), "Bug sheet", "normal").unwrap();
        insert_fixbug_ref(
            &conn,
            1,
            "sheet",
            "0:1",
            "S-1",
            "Bug sheet",
            None,
            Some(intake_id),
        )
        .unwrap();

        let intake_id_2 = insert_intake(&conn, "sheet", Some(1), "Bug sheet 2", "normal").unwrap();
        insert_fixbug_ref(
            &conn,
            1,
            "sheet",
            "0:2",
            "S-2",
            "Bug sheet 2",
            None,
            Some(intake_id_2),
        )
        .unwrap();
    }

    // 3. POST /api/fixbug/demo/deployed -> 200 OK
    let (code, body, _) = http_request(
        port,
        "POST",
        "/api/fixbug/demo/deployed",
        Some(&token),
        Some(r#"{"keys": ["S-1"], "note": "fix ok"}"#),
    );
    assert_eq!(code, 200);
    assert_eq!(body[0]["key"], "S-1");
    assert_eq!(body[0]["state"], "deployed");

    // 4. POST /api/fixbug/demo/reject -> thiếu reason -> 400 Bad Request
    let (code, _, _) = http_request(
        port,
        "POST",
        "/api/fixbug/demo/reject",
        Some(&token),
        Some(r#"{"keys": ["S-2"], "reason": ""}"#),
    );
    assert_eq!(code, 400);

    // 5. POST /api/fixbug/demo/reject -> có reason -> 200 OK
    let (code, body, _) = http_request(
        port,
        "POST",
        "/api/fixbug/demo/reject",
        Some(&token),
        Some(r#"{"keys": ["S-2"], "reason": "not a bug"}"#),
    );
    assert_eq!(code, 200);
    assert_eq!(body[0]["key"], "S-2");
    assert_eq!(body[0]["state"], "rejected");
}

#[test]
fn test_cli_fixbug_commands() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    let (_conn, _project) = setup_sandbox(root);

    // Ghi project map
    let map_path = root.join("projects_map.json");
    let map_json = json!({
        "projects": [{
            "id": "demo",
            "path": root.to_string_lossy(),
            "name_vi": "Dự án Demo"
        }]
    });
    std::fs::write(&map_path, map_json.to_string()).unwrap();

    let config_val: serde_json::Value =
        serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap();
    let cfg = agent_brain::config::config_from_value(&config_val).unwrap();

    let mut ctx = agent_brain::app::open_ctx_with(cfg, &map_path, &root.join("brain.db")).unwrap();

    // 1. cmd_init dry-run -> 0
    let code = cmd_init(&ctx, "demo", true);
    assert_eq!(code, 0);

    // 2. cmd_init write -> 0
    let code = cmd_init(&ctx, "demo", false);
    assert_eq!(code, 0);
    assert!(agent_brain::fixbug::config::config_path(root).is_file());

    // 3. cmd_check -> 0
    let code = cmd_check(&ctx, "demo");
    assert_eq!(code, 0);

    // 4. cmd_list (chưa có bug) -> 0
    let code = cmd_list(&ctx, "demo");
    assert_eq!(code, 0);

    // 5. Thêm ref và test cmd_deployed / cmd_reject
    let intake_1 = insert_intake(&ctx.conn, "sheet", Some(1), "Bug S1", "normal").unwrap();
    insert_fixbug_ref(
        &ctx.conn,
        1,
        "sheet",
        "0:1",
        "S-1",
        "Bug S1",
        None,
        Some(intake_1),
    )
    .unwrap();

    let code = cmd_deployed(&mut ctx, "demo", &["S-1".to_string()], Some("done"));
    assert_eq!(code, 0);

    let intake_2 = insert_intake(&ctx.conn, "sheet", Some(1), "Bug S2", "normal").unwrap();
    insert_fixbug_ref(
        &ctx.conn,
        1,
        "sheet",
        "0:2",
        "S-2",
        "Bug S2",
        None,
        Some(intake_2),
    )
    .unwrap();

    let code = cmd_reject(&mut ctx, "demo", &["S-2".to_string()], "not bug");
    assert_eq!(code, 0);

    // cmd_list sau khi có bug -> 0
    let code = cmd_list(&ctx, "demo");
    assert_eq!(code, 0);

    // cmd_pull với config trello không có mạng thật -> trả lỗi thoát 1, không panic
    let code = cmd_pull(&mut ctx, "demo", Some(5));
    assert_eq!(code, 1);
}
