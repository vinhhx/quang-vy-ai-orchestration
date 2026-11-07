#![allow(clippy::while_let_loop)]

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use common::{Sandbox, Serve};
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;

fn fake_agy() -> PathBuf {
    common::repo_root()
        .join("tests")
        .join("fakes")
        .join("fake_agy.py")
        .canonicalize()
        .expect("canonicalize fake_agy.py")
}

fn valid_analyst_text() -> String {
    "# Phân tích chức năng hệ thống\n\n## Tóm tắt\n- Đã kiểm tra mã nguồn hệ thống trong dự án.\n- Chức năng được yêu cầu đã có một phần ở src/main.rs:1.\n- Không cần bổ sung cơ sở dữ liệu mới.\n- Rủi ro kỹ thuật thấp, có thể tiến hành ngay.\n\n## Đã có chưa\nCó một phần trong module chính.\n\n## Bằng chứng\nsrc/main.rs:1\n\n## Giả định & cần làm rõ\nGiả định rằng phiên bản PHP/Rust tương thích với môi trường hiện tại.".to_string()
}

fn valid_reviewer_text() -> String {
    "# Phân tích chức năng hệ thống (Đã review)\n\n## Tóm tắt\n- Báo cáo đã được Analysis Reviewer thẩm định và hiệu đính chi tiết.\n- Xác nhận chức năng đã có ở src/main.rs:1.\n- Đã bổ sung các trường hợp ngoại lệ cần chú ý.\n- Rủi ro kỹ thuật ở mức tối thiểu.\n\n## Bằng chứng\nsrc/main.rs:1\n\n## Ghi chú review\nĐã đối chiếu với code thực tế và chuẩn hóa lại các khuyến nghị kỹ thuật.".to_string()
}

fn init_project(sb: &Sandbox, name: &str) -> PathBuf {
    let repo = sb.make_repo(name, &[("src/main.rs", "fn main() {}\n")], None, None);
    let entry = sb.project_entry(&repo, name, "work");
    sb.write_map(serde_json::json!([entry]), &[]);
    repo
}

#[test]
fn test_analyst_run_claude_success_b() {
    let sb = Sandbox::new();
    let repo = init_project(&sb, "my-proj");

    let initial_status = sb.git(&repo, &["status", "--porcelain"]);
    assert!(initial_status.trim().is_empty());

    let scenario = json!({
        "result": valid_analyst_text(),
    });

    let res = sb.run(
        &[
            "run",
            "analyst",
            "my-proj",
            "Hệ thống đã có chức năng X chưa?",
            "--name",
            "task-x",
        ],
        Some(scenario),
    );

    assert_eq!(
        res.code, 0,
        "Analyst run failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let answer_file = sb
        .xdg_data
        .join("agent-brain")
        .join("answers")
        .join("my-proj")
        .join("1-task-x.md");
    assert!(answer_file.is_file(), "File answer phải tồn tại");

    let meta = fs::metadata(&answer_file).expect("metadata answer file");
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "Quyền file phải là 0600");

    let dir_meta = fs::metadata(answer_file.parent().unwrap()).expect("metadata answer dir");
    let dir_mode = dir_meta.permissions().mode() & 0o777;
    assert_eq!(dir_mode, 0o700, "Quyền thư mục answers phải là 0700");

    let content = fs::read_to_string(&answer_file).expect("read answer");
    assert!(content.contains("## Tóm tắt"));
    assert!(content.contains("src/main.rs:1"));

    // Check git: branch analysis/task-x MUST NOT exist in git
    let git_branches = sb.git(&repo, &["branch", "--list", "analysis/task-x"]);
    assert!(
        git_branches.trim().is_empty(),
        "Nhánh analysis/task-x không được tồn tại trong git"
    );

    // Git status must be unchanged
    let post_status = sb.git(&repo, &["status", "--porcelain"]);
    assert_eq!(
        post_status, initial_status,
        "Git status không được thay đổi sau run analyst"
    );

    // Check task status in db: answered
    let conn = rusqlite::Connection::open(&sb.db_path).unwrap();
    let status: String = conn
        .query_row("SELECT status FROM tasks WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "answered", "Task status phải là answered");
}

#[test]
fn test_analyst_run_claude_modifies_repo_violates_protocol_c() {
    let sb = Sandbox::new();
    let _repo = init_project(&sb, "my-proj");

    let scenario = json!({
        "ops": [
            {"op": "write", "path": "pwned.txt", "content": "violation"}
        ],
        "result": valid_analyst_text(),
    });

    let res = sb.run(
        &[
            "run",
            "analyst",
            "my-proj",
            "Test protocol violation",
            "--name",
            "test-viol",
        ],
        Some(scenario),
    );

    assert_ne!(res.code, 0, "Chạy phải thất bại khi vi phạm protocol");
    assert!(
        res.stderr.contains("vi phạm quy tắc chỉ đọc")
            || res.stdout.contains("vi phạm quy tắc chỉ đọc"),
        "Phải thông báo vi phạm protocol"
    );

    let answer_file = sb
        .xdg_data
        .join("agent-brain")
        .join("answers")
        .join("my-proj")
        .join("1-test-viol.md");
    assert!(
        !answer_file.exists(),
        "Không được lưu file answer khi vi phạm protocol"
    );

    let conn = rusqlite::Connection::open(&sb.db_path).unwrap();
    let outcome: String = conn
        .query_row("SELECT outcome FROM runs WHERE task_id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        outcome, "protocol_violation",
        "Outcome phải là protocol_violation"
    );

    let task_status: String = conn
        .query_row("SELECT status FROM tasks WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(task_status, "open", "Task phải giữ nguyên trạng thái open");
}

#[test]
fn test_analyst_run_missing_summary_fails_d() {
    let sb = Sandbox::new();
    let _repo = init_project(&sb, "my-proj");

    let scenario = json!({
        "result": "Đoạn văn này không có mục tóm tắt theo quy định cấu trúc báo cáo.",
    });

    let res = sb.run(
        &[
            "run",
            "analyst",
            "my-proj",
            "Test invalid answer",
            "--name",
            "test-invalid",
        ],
        Some(scenario),
    );

    assert_ne!(
        res.code, 0,
        "Chạy phải thất bại khi thiếu mục tóm tắt hoặc dưới 200 ký tự"
    );

    let answer_file = sb
        .xdg_data
        .join("agent-brain")
        .join("answers")
        .join("my-proj")
        .join("1-test-invalid.md");
    assert!(!answer_file.exists(), "Không được lưu file answer");

    let conn = rusqlite::Connection::open(&sb.db_path).unwrap();
    let outcome: String = conn
        .query_row("SELECT outcome FROM runs WHERE task_id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(outcome, "analyst:invalid");

    let task_status: String = conn
        .query_row("SELECT status FROM tasks WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(task_status, "open");
}

#[test]
fn test_analyst_run_agy_read_only_e() {
    let sb = Sandbox::new();
    let repo = init_project(&sb, "my-proj");

    let fake_agy_bin = fake_agy();
    sb.write_config(json!({
        "engines": {
            "agy": {
                "command": fake_agy_bin.to_str().unwrap()
            }
        }
    }));

    sb.write_teams(json!({
        "team-agy": {
            "description": "Đội Agy",
            "roles": {
                "lead": "agy"
            }
        }
    }));

    let agy_scenario = json!({
        "status": "SUCCESS",
        "response": valid_analyst_text(),
    });
    let scenario_path = sb.root.join("fake_agy_scenario.json");
    let record_path = sb.root.join("fake_agy_record.jsonl");
    fs::write(&scenario_path, serde_json::to_string(&agy_scenario).unwrap()).unwrap();

    let res = sb.run_env(
        &[
            "run",
            "analyst",
            "my-proj",
            "Câu hỏi cho agy",
            "--name",
            "agy-task",
            "--team",
            "team-agy",
        ],
        None,
        &[
            ("FAKE_AGY_SCENARIO", scenario_path.to_str().unwrap()),
            ("FAKE_AGY_RECORD", record_path.to_str().unwrap()),
        ],
    );

    assert_eq!(
        res.code, 0,
        "Analyst qua agy thất bại: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let answer_file = sb
        .xdg_data
        .join("agent-brain")
        .join("answers")
        .join("my-proj")
        .join("1-agy-task.md");
    assert!(answer_file.is_file(), "File answer của agy phải được lưu");

    let content = fs::read_to_string(&answer_file).unwrap();
    assert!(content.contains("## Tóm tắt"));

    // Ensure NO files were written into repo
    assert!(!repo.join(".plan.md").exists());
    assert!(!repo.join(".analyst.md").exists());
    let st = sb.git(&repo, &["status", "--porcelain"]);
    assert!(st.trim().is_empty(), "Repo không được bị ghi file nào");
}

#[test]
fn test_analyst_review_workflow_f() {
    let sb = Sandbox::new();
    let _repo = init_project(&sb, "my-proj");

    // Case 1: reviewer succeeds -> reviewer answer replaces analyst answer
    let scenario_success = json!({
        "by_role": {
            "analyst": [{
                "result": valid_analyst_text()
            }],
            "analysis-review": [{
                "result": valid_reviewer_text()
            }]
        }
    });

    let res = sb.run(
        &[
            "run",
            "analyst",
            "my-proj",
            "Câu hỏi cần review",
            "--name",
            "review-ok",
            "--workflow",
            "phan-tich-review",
        ],
        Some(scenario_success),
    );

    assert_eq!(
        res.code, 0,
        "Phân tích review thành công: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let answer_file = sb
        .xdg_data
        .join("agent-brain")
        .join("answers")
        .join("my-proj")
        .join("1-review-ok.md");
    assert!(answer_file.is_file());
    let content = fs::read_to_string(&answer_file).unwrap();
    assert!(
        content.contains("Đã review"),
        "Nội dung phải là bản của reviewer"
    );
    assert!(content.contains("## Ghi chú review"));

    let conn = rusqlite::Connection::open(&sb.db_path).unwrap();
    let run_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM runs WHERE task_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(run_count, 2, "Phải có 2 run (analyst và analysis-review)");

    // Case 2: reviewer fails -> keeps analyst answer with `> Bước review lỗi` prefix
    let scenario_fail = json!({
        "by_role": {
            "analyst": [{
                "result": valid_analyst_text()
            }],
            "analysis-review": [{
                "exit_code": 1,
                "result": ""
            }]
        }
    });

    let res2 = sb.run(
        &[
            "run",
            "analyst",
            "my-proj",
            "Câu hỏi reviewer lỗi",
            "--name",
            "review-fail",
            "--workflow",
            "phan-tich-review",
        ],
        Some(scenario_fail),
    );

    assert_eq!(
        res2.code, 0,
        "Vẫn lưu được câu trả lời analyst khi reviewer lỗi"
    );

    let answer_file2 = sb
        .xdg_data
        .join("agent-brain")
        .join("answers")
        .join("my-proj")
        .join("2-review-fail.md");
    assert!(answer_file2.is_file());
    let content2 = fs::read_to_string(&answer_file2).unwrap();
    assert!(
        content2.starts_with("> Bước review lỗi:"),
        "Phải có dòng thông báo review lỗi ở đầu file"
    );
    assert!(
        content2.contains("## Tóm tắt"),
        "Vẫn giữ nguyên bản phân tích của analyst"
    );
}

#[test]
fn test_analyst_engine_and_model_from_lead_g() {
    let sb = Sandbox::new();
    let _repo = init_project(&sb, "my-proj");

    let fake_agy_bin = fake_agy();
    sb.write_config(json!({
        "engines": {
            "agy": {
                "command": fake_agy_bin.to_str().unwrap()
            }
        },
        "models": {
            "lead": "model-lead-custom",
            "dev": "model-dev-custom",
            "qa": "model-qa",
            "plan-review": "model-pr",
            "code-review": "model-cr"
        }
    }));

    sb.write_teams(json!({
        "team-lead-agy": {
            "description": "Đội lead agy",
            "roles": {
                "lead": "agy"
            }
        }
    }));

    let res = sb.run(
        &[
            "run",
            "analyst",
            "my-proj",
            "Dry run test",
            "--team",
            "team-lead-agy",
            "--dry-run",
        ],
        None,
    );

    assert_eq!(
        res.code, 0,
        "Dry run failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        res.stdout.contains("Model: model-lead-custom"),
        "Model của analyst phải kế thừa từ vai lead: {}",
        res.stdout
    );
}

fn app_state(sb: &Sandbox) -> Arc<agent_brain::dashboard::AppState> {
    let text = std::fs::read_to_string(&sb.config_path).expect("read config");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parse config");
    let cfg = agent_brain::config::config_from_value(&value).expect("config_from_value");
    Arc::new(agent_brain::dashboard::AppState {
        orch: std::sync::RwLock::new(cfg.orchestrator.clone()),
        cfg,
        port: 0,
        token: "0".repeat(64),
        map_path: sb.map_path(),
        db_path: sb.db_path.clone(),
        exe: common::bin(),
        hints: tokio::sync::broadcast::channel(16).0,
        shutdown: tokio::sync::watch::channel(false).1,
        jobs: agent_brain::dashboard::jobs::JobManager::new(common::bin()),
    })
}

fn wait_connected(srv: &Serve) {
    assert!(
        common::wait_until(std::time::Duration::from_secs(10), || srv
            .stdout
            .lock()
            .unwrap()
            .contains("Telegram: đã kết nối")),
        "no connected line"
    );
}

#[test]
fn test_start_new_task_analysis_dirty_repo_h() {
    let sb = Sandbox::new();
    let repo = init_project(&sb, "dirty-proj");

    // Dirty the repo
    std::fs::write(repo.join("dirty_file.txt"), "unstaged dirty content").unwrap();
    let status = sb.git(&repo, &["status", "--porcelain"]);
    assert!(!status.trim().is_empty(), "Repo phải bẩn");

    let st = app_state(&sb);

    // If we tried starting a code workflow on dirty repo, it would be refused by preflight:
    let code_req = agent_brain::dashboard::api_tasks::StartRequest {
        project: "dirty-proj",
        request: "Sửa code",
        name: Some("test-code"),
        workflow: Some("mac-dinh"),
        team: None,
        origin: agent_brain::dashboard::api_tasks::Origin::Dashboard,
        dry_run: false,
    };
    let code_res = agent_brain::dashboard::api_tasks::start_new_task(&st, &code_req).unwrap();
    assert!(
        matches!(
            code_res,
            agent_brain::dashboard::api_tasks::StartOutcome::PreflightRefused { .. }
        ),
        "Workflow code phải bị preflight từ chối khi repo bẩn"
    );

    // But with an analysis workflow (phan-tich), preflight is skipped and task starts!
    let analysis_req = agent_brain::dashboard::api_tasks::StartRequest {
        project: "dirty-proj",
        request: "Phân tích hệ thống",
        name: Some("test-analysis"),
        workflow: Some("phan-tich"),
        team: None,
        origin: agent_brain::dashboard::api_tasks::Origin::Dashboard,
        dry_run: false,
    };
    let res = agent_brain::dashboard::api_tasks::start_new_task(&st, &analysis_req).unwrap();
    match res {
        agent_brain::dashboard::api_tasks::StartOutcome::Started { job_id, info, .. } => {
            assert!(job_id > 0);
            assert_eq!(info.slug, "test-analysis");
            assert_eq!(info.branch, "analysis/test-analysis");
            assert_eq!(info.workflow, "phan-tich");
        }
        other => panic!("Mong muốn StartOutcome::Started nhưng nhận được {:?}", other),
    }
}

#[test]
fn test_triage_analysis_workflow_validation_i() {
    let sb = Sandbox::new();
    let repo = init_project(&sb, "my-proj");
    let proj = sb.project_entry(&repo, "my-proj", "work");
    let projects = vec![agent_brain::projects::Project::from_value(&proj).unwrap()];

    let (teams_loaded, _) = agent_brain::teams::load();
    let text = std::fs::read_to_string(&sb.config_path).expect("read config");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parse config");
    let cfg = agent_brain::config::config_from_value(&value).expect("config_from_value");
    let workflows_loaded = agent_brain::workflows::load(&cfg);
    let engines_cfg = agent_brain::config::EnginesCfg::defaults("claude");

    let draft = agent_brain::orchestrator::triage::ProposalDraft {
        project: "my-proj".to_string(),
        title: "Khảo sát tính năng".to_string(),
        request: "Hệ thống đã có chức năng X chưa?".to_string(),
        priority: "normal".to_string(),
        level: "vua".to_string(),
        team: "mac-dinh".to_string(),
        workflow: "phan-tich".to_string(),
        reason: Some("Yêu cầu khảo sát".to_string()),
    };

    let res = agent_brain::orchestrator::triage::validate(
        &draft,
        &projects,
        &teams_loaded,
        &workflows_loaded,
        &engines_cfg,
    );
    assert!(res.is_ok(), "Workflow phan-tich phải validate thành công: {:?}", res);

    let draft_review = agent_brain::orchestrator::triage::ProposalDraft {
        project: "my-proj".to_string(),
        title: "Khảo sát có review".to_string(),
        request: "Hệ thống đã có chức năng Y chưa?".to_string(),
        priority: "normal".to_string(),
        level: "vua".to_string(),
        team: "mac-dinh".to_string(),
        workflow: "phan-tich-review".to_string(),
        reason: Some("Cần review kỹ".to_string()),
    };

    let res_review = agent_brain::orchestrator::triage::validate(
        &draft_review,
        &projects,
        &teams_loaded,
        &workflows_loaded,
        &engines_cfg,
    );
    assert!(
        res_review.is_ok(),
        "Workflow phan-tich-review phải validate thành công: {:?}",
        res_review
    );
}

#[test]
fn test_telegram_answered_notification_j() {
    let sb = Sandbox::new();
    let tg = tg::FakeTelegram::start(&sb);
    tg::write_config(&sb, tg::PAIRED_CHAT, 0o600);
    let _repo = init_project(&sb, "my-proj");

    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let map = agent_brain::projects::load_map(&sb.map_path()).unwrap();
    agent_brain::db::sync_projects(&conn, &agent_brain::projects::projects_of(&map)).unwrap();
    let pid = agent_brain::db::project_row_id(&conn, "my-proj").unwrap();
    let task_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "task-ans",
        "Câu hỏi phân tích",
        "analysis/task-ans",
        Some("main"),
        None,
    )
    .unwrap();

    let run_id = agent_brain::db::insert_run(
        &conn,
        &agent_brain::db::NewRun {
            task_id: Some(task_id),
            project_id: pid,
            role: "analyst".to_string(),
            profile: "work".to_string(),
            model: Some("opus".to_string()),
            pid: 1000,
            pgid: 1000,
            pid_start: 1,
            base_commit: None,
            run_dir: None,
        },
    )
    .unwrap();
    conn.execute(
        "UPDATE runs SET status = 'succeeded', outcome = 'ok' WHERE id = ?1",
        [run_id],
    )
    .unwrap();

    let mut srv = tg.serve(&sb, &[]);
    wait_connected(&srv);

    // Save answer file directly in the sandbox's xdg_data
    let ans_text = valid_analyst_text();
    let ans_dir = sb.xdg_data.join("agent-brain").join("answers").join("my-proj");
    std::fs::create_dir_all(&ans_dir).unwrap();
    std::fs::write(ans_dir.join(format!("{task_id}-task-ans.md")), &ans_text).unwrap();

    // Set task status to answered while serve is running
    conn.execute(
        "UPDATE tasks SET status = 'answered', updated_at = ?1 WHERE id = ?2",
        rusqlite::params![agent_brain::db::now_ms(), task_id],
    )
    .unwrap();

    // Wait until sendMessage is delivered
    let ok = common::wait_until(std::time::Duration::from_secs(15), || {
        !tg.calls("sendMessage").is_empty()
    });
    assert!(ok, "Telegram phải nhận được sendMessage cho task answered");

    // 1. sendDocument must be sent with .md file
    let docs = tg.calls("sendDocument");
    assert_eq!(docs.len(), 1, "Phải gửi đúng 1 document: {docs:#?}");
    let doc = &docs[0];
    assert_eq!(
        doc["form"]["document"]["filename"],
        serde_json::json!("1-task-ans.md")
    );
    let caption = doc["form"]["caption"].as_str().unwrap();
    assert!(caption.contains("Phân tích task 1 — my-proj"));

    // 2. sendMessage must reply to the document and contain ## Tóm tắt content
    let msgs = tg.calls("sendMessage");
    assert_eq!(msgs.len(), 1, "Phải gửi đúng 1 sendMessage: {msgs:#?}");
    let msg = &msgs[0];
    let text = msg["json"]["text"].as_str().unwrap();
    assert!(text.contains("Đã kiểm tra mã nguồn hệ thống trong dự án"));

    let doc_mid = doc["response"]["result"]["message_id"]
        .as_i64()
        .expect("doc message id");
    assert_eq!(
        msg["json"]["reply_parameters"]["message_id"].as_i64(),
        Some(doc_mid),
        "Tin nhắn phải reply vào document"
    );

    // 3. Subsequent tick must not resend
    std::thread::sleep(std::time::Duration::from_secs(3));
    assert_eq!(
        tg.calls("sendDocument").len(),
        1,
        "Không được gửi lại document lần hai"
    );
    assert_eq!(
        tg.calls("sendMessage").len(),
        1,
        "Không được gửi lại tin nhắn lần hai"
    );

    assert_eq!(
        srv.stop(libc::SIGINT, std::time::Duration::from_secs(10)),
        Some(0)
    );
}

#[test]
fn test_api_task_answer_and_taskstate_k() {
    let sb = Sandbox::new();
    let _repo = init_project(&sb, "my-proj");

    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let map = agent_brain::projects::load_map(&sb.map_path()).unwrap();
    agent_brain::db::sync_projects(&conn, &agent_brain::projects::projects_of(&map)).unwrap();
    let pid = agent_brain::db::project_row_id(&conn, "my-proj").unwrap();
    let task_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "task-answer-api",
        "Câu hỏi API",
        "analysis/task-answer-api",
        Some("main"),
        None,
    )
    .unwrap();
    let text = std::fs::read_to_string(&sb.config_path).expect("read config");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parse config");
    let cfg = agent_brain::config::config_from_value(&value).expect("config_from_value");
    let workflows_loaded = agent_brain::workflows::load(&cfg);
    let wf = workflows_loaded.get("phan-tich").unwrap();
    agent_brain::db::set_task_workflow_if_null(&conn, task_id, &wf.to_snapshot()).unwrap();

    let mut srv = sb.serve(&[]);

    // 1. When answer file doesn't exist -> 404
    let r404 = srv.get(&format!("/api/tasks/{task_id}/answer"));
    assert_eq!(r404.status, 404);
    assert_eq!(
        r404.json()["error"],
        "Chưa có câu trả lời cho task này"
    );

    // 2. Save answer file
    let ans_text = valid_analyst_text();
    let ans_dir = sb.xdg_data.join("agent-brain").join("answers").join("my-proj");
    std::fs::create_dir_all(&ans_dir).unwrap();
    let ans_file = ans_dir.join(format!("{task_id}-task-answer-api.md"));
    std::fs::write(&ans_file, &ans_text).unwrap();

    // Now GET /api/tasks/{id}/answer -> 200 with path and markdown
    let r200 = srv.get(&format!("/api/tasks/{task_id}/answer"));
    assert_eq!(r200.status, 200);
    let j200 = r200.json();
    assert_eq!(j200["markdown"], ans_text);
    assert!(j200["path"]
        .as_str()
        .unwrap()
        .ends_with(&format!("{task_id}-task-answer-api.md")));

    // 3. Check taskstate when answered
    conn.execute(
        "UPDATE tasks SET status = 'answered', updated_at = ?1 WHERE id = ?2",
        rusqlite::params![agent_brain::db::now_ms(), task_id],
    )
    .unwrap();

    let r_task = srv.get(&format!("/api/tasks/{task_id}"));
    assert_eq!(r_task.status, 200);
    let task_json = r_task.json();
    assert_eq!(task_json["group"], "done");
    assert_eq!(task_json["banner"], "Đã trả lời");
    assert!(!task_json["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["id"] == "run_dev" || a["id"] == "relead" || a["id"] == "run_qa"));

    // Check taskstate::derive directly has no actions
    let task_row = agent_brain::db::get_task(&conn, task_id).unwrap().unwrap();
    let facts = agent_brain::dashboard::taskstate::TaskFacts {
        task: &task_row,
        lead: None,
        dev: None,
        qa: None,
        git: &agent_brain::dashboard::taskstate::GitFacts::default(),
        busy: None,
        now_ms: 2000,
    };
    let derived = agent_brain::dashboard::taskstate::derive(&facts);
    assert_eq!(derived.group, "done");
    assert_eq!(derived.banner.as_deref(), Some("Đã trả lời"));
    assert!(derived.actions.is_empty());

    let auto_ctl = task_json["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "auto_start")
        .unwrap();
    assert_eq!(auto_ctl["enabled"], false);
    assert_eq!(
        auto_ctl["reason"],
        "Task đã xong (Đã trả lời)."
    );

    assert_eq!(
        srv.stop(libc::SIGINT, std::time::Duration::from_secs(10)),
        Some(0)
    );
}
