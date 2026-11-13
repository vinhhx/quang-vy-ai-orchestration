mod common;
#[allow(unused)]
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::setup_project;

fn init_test_db(sb: &Sandbox) -> (rusqlite::Connection, i64) {
    let conn = agent_brain::db::open(&sb.db_path).expect("init db");
    let map = sb.read_map();
    let projs = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projs).expect("sync_projects");
    let pid = agent_brain::db::project_row_id(&conn, "web-ban-hang").expect("project_row_id");
    (conn, pid)
}

#[test]
fn lead_task_goc_receives_can_split_flag() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let out = sb.run(
        &[
            "run",
            "lead",
            "--workflow",
            "mac-dinh",
            "web bán hàng",
            "test e2e toàn bộ",
            "--dry-run",
        ],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}\nstdout: {}", out.stderr, out.stdout);
    assert!(
        out.stdout.contains("Được phép tách việc: có"),
        "stdout missing 'Được phép tách việc: có': {}",
        out.stdout
    );
}

#[test]
fn lead_task_con_receives_child_notice_and_no_can_split_flag() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let (conn, pid) = init_test_db(&sb);
    let parent_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "test-e2e-goc",
        "Test E2E toàn bộ hệ thống",
        "task/test-e2e-goc",
        Some("main"),
        None,
    )
    .expect("upsert parent");

    let child_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "test-e2e-goc-c1",
        "Test login",
        "task/test-e2e-goc-c1",
        Some("main"),
        None,
    )
    .expect("upsert child");

    agent_brain::db::set_task_parent(&conn, child_id, parent_id).expect("set parent");

    let out = sb.run(
        &[
            "run",
            "lead",
            "--workflow",
            "mac-dinh",
            "web bán hàng",
            "Test login",
            "--name",
            "test-e2e-goc-c1",
            "--dry-run",
        ],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}\nstdout: {}", out.stderr, out.stdout);
    assert!(
        out.stdout.contains(&format!(
            "Task này là việc con của task #{parent_id} \"Test E2E toàn bộ hệ thống\"; không được tách tiếp."
        )),
        "stdout missing child notice: {}",
        out.stdout
    );
    assert!(
        !out.stdout.contains("Được phép tách việc: có"),
        "stdout should NOT have 'Được phép tách việc: có': {}",
        out.stdout
    );
}

#[test]
fn lead_task_con_rejects_nested_split_plan() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let (conn, pid) = init_test_db(&sb);
    let parent_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "test-e2e-goc",
        "Test E2E toàn bộ hệ thống",
        "task/test-e2e-goc",
        Some("main"),
        None,
    )
    .expect("upsert parent");

    let child_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "test-e2e-goc-c2",
        "Test nested",
        "task/test-e2e-goc-c2",
        Some("main"),
        None,
    )
    .expect("upsert child");

    agent_brain::db::set_task_parent(&conn, child_id, parent_id).expect("set parent");

    // Fake claude writes a split plan for the child task
    let split_plan_text = "# Plan: Test nested\n## Mục tiêu\nx\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n## Không được đụng tới\n-\n## Cách kiểm tra\n-\n## Vướng mắc\n## Tách việc\n| # | Việc con | Yêu cầu | Sau |\n|---|---|---|---|\n| 1 | C1 | R1 | |\n| 2 | C2 | R2 | 1 |\n";
    let scenario = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "write", "path": ".plan.md", "content": split_plan_text, "keep_meta": true}
        ]
    });

    let out = sb.run(
        &[
            "run",
            "lead",
            "--workflow",
            "mac-dinh",
            "web bán hàng",
            "Test nested",
            "--name",
            "test-e2e-goc-c2",
        ],
        Some(scenario),
    );
    assert_ne!(out.code, 0);
    assert!(
        out.stderr.contains("split_nested") || out.stdout.contains("split_nested"),
        "output missing 'split_nested':\nstdout: {}\nstderr: {}",
        out.stdout,
        out.stderr
    );
}

fn app_state(sb: &Sandbox) -> std::sync::Arc<agent_brain::dashboard::AppState> {
    let text = std::fs::read_to_string(&sb.config_path).expect("read config");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parse config");
    let cfg = agent_brain::config::config_from_value(&value).expect("config_from_value");
    std::sync::Arc::new(agent_brain::dashboard::AppState {
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

#[test]
fn approve_split_plan_cli_and_dashboard_and_cancel() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let (conn, pid) = init_test_db(&sb);
    let parent_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "split-parent",
        "Việc cha chia việc",
        "task/split-parent",
        Some("main"),
        None,
    )
    .expect("upsert parent");

    let cfg_default = agent_brain::config::config_from_value(
        &serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap(),
    )
    .unwrap();
    let wfs = agent_brain::workflows::load_from(None, &cfg_default);
    let nhanh = wfs.get("nhanh").unwrap();
    agent_brain::db::set_task_workflow_if_null(&conn, parent_id, &nhanh.to_snapshot()).unwrap();

    // Checkout branch and commit split plan
    let _ = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "checkout", "-b", "task/split-parent"])
        .output()
        .unwrap();

    let split_plan_text = "# Plan: Việc cha chia việc\n## Mục tiêu\nMục tiêu lớn\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n## Không được đụng tới\n-\n## Cách kiểm tra\n-\n## Vướng mắc\n## Tách việc\n| # | Việc con | Yêu cầu | Sau |\n|---|---|---|---|\n| 1 | Việc 1 | Yêu cầu 1 | |\n| 2 | Việc 2 | Yêu cầu 2 | 1 |\n";
    std::fs::write(repo.join(".plan.md"), split_plan_text).unwrap();
    let _ = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "add", ".plan.md"])
        .output()
        .unwrap();
    let _ = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "commit", "-m", "split-parent: plan"])
        .output()
        .unwrap();

    let st = app_state(&sb);

    // 1. approve_dev & approve_qa must be rejected on split plan
    let res_dev = agent_brain::dashboard::api_tasks::run_control_as(
        &st,
        parent_id,
        "approve_dev",
        None,
        None,
        agent_brain::dashboard::api_tasks::ActionSource::Dashboard,
    )
    .unwrap();
    assert_eq!(
        res_dev,
        agent_brain::dashboard::api_tasks::ControlOutcome::Invalid(
            "Plan tách việc: chỉ dùng Duyệt".to_string()
        )
    );

    let res_qa = agent_brain::dashboard::api_tasks::run_control_as(
        &st,
        parent_id,
        "approve_qa",
        None,
        None,
        agent_brain::dashboard::api_tasks::ActionSource::Dashboard,
    )
    .unwrap();
    assert_eq!(
        res_qa,
        agent_brain::dashboard::api_tasks::ControlOutcome::Invalid(
            "Plan tách việc: chỉ dùng Duyệt".to_string()
        )
    );

    // 2. CLI approve parent split plan
    let out = sb.run(&["approve", &parent_id.to_string()], None);
    assert_eq!(out.code, 0, "stderr: {}\nstdout: {}", out.stderr, out.stdout);

    // Check database
    let items = agent_brain::db::split_items_of(&conn, parent_id).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "Việc 1");
    assert_eq!(items[0].state, "waiting");
    assert_eq!(items[1].title, "Việc 2");
    assert_eq!(items[1].after_idx, Some(1));
    assert_eq!(items[1].state, "waiting");

    let auto = agent_brain::db::task_auto(&conn, parent_id).unwrap().unwrap();
    assert_eq!(auto.auto_state.as_deref(), Some("waiting_children"));

    // 3. Idempotent approve / materialize: re-approving doesn't duplicate
    let plan_commit = agent_brain::gitops::plan_commit_on_branch(&repo, "task/split-parent", "split-parent", None).unwrap().unwrap();
    let split_items = agent_brain::split::plan_split_items(&repo, &agent_brain::protocol_paths::paths(&agent_brain::projects::projects_of(&sb.read_map())[0], "split-parent"), &plan_commit).unwrap().unwrap();
    let re_materialized = agent_brain::split::materialize(&conn, &agent_brain::db::get_task(&conn, parent_id).unwrap().unwrap(), &plan_commit, &split_items).unwrap();
    assert_eq!(re_materialized, 0, "second materialize inserts 0 rows");
    let items2 = agent_brain::db::split_items_of(&conn, parent_id).unwrap();
    assert_eq!(items2.len(), 2);

    // 4. Cancel parent task via auto_end (calls split::cancel_waiting)
    agent_brain::db::set_auto_state(&conn, parent_id, Some("stopped")).unwrap();
    let res_cancel = agent_brain::dashboard::api_tasks::run_control_as(
        &st,
        parent_id,
        "auto_end",
        None,
        None,
        agent_brain::dashboard::api_tasks::ActionSource::Dashboard,
    )
    .unwrap();
    assert_eq!(
        res_cancel,
        agent_brain::dashboard::api_tasks::ControlOutcome::Ended { at_gate: false }
    );

    let waiting = agent_brain::db::waiting_split_items(&conn).unwrap();
    assert_eq!(waiting.len(), 0);

    let items_after_cancel = agent_brain::db::split_items_of(&conn, parent_id).unwrap();
    assert_eq!(items_after_cancel[0].state, "cancelled");
    assert_eq!(items_after_cancel[1].state, "cancelled");
}

#[test]
fn child_task_auto_approves_plan_when_started_by_split() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let (conn, pid) = init_test_db(&sb);
    let parent_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "parent-p",
        "Parent",
        "task/parent-p",
        Some("main"),
        None,
    )
    .unwrap();

    let child_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "child-c1",
        "Child 1",
        "task/child-c1",
        Some("main"),
        None,
    )
    .unwrap();

    agent_brain::db::set_task_parent(&conn, child_id, parent_id).unwrap();

    // Insert split items for parent, item 1 started with child_id
    let new_items = vec![agent_brain::db::NewSplitItem {
        idx: 1,
        title: "Child 1".to_string(),
        request: "Req 1".to_string(),
        after_idx: None,
    }];
    agent_brain::db::insert_split_items(&conn, parent_id, "commit_p", &new_items).unwrap();
    let parent_items = agent_brain::db::split_items_of(&conn, parent_id).unwrap();
    agent_brain::db::set_split_item_started(&conn, parent_items[0].id, child_id).unwrap();

    // Setup child branch and plan
    let _ = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "checkout", "-b", "task/child-c1"])
        .output()
        .unwrap();

    let plan_text = "# Plan: Child 1\n## Mục tiêu\nChild goal\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | 1 | edit | test |\n## Không được đụng tới\n-\n## Cách kiểm tra\n-\n## Vướng mắc\n";
    std::fs::write(repo.join(".plan.md"), plan_text).unwrap();
    let _ = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "add", ".plan.md"])
        .output()
        .unwrap();
    let _ = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "commit", "-m", "child-c1: plan"])
        .output()
        .unwrap();

    let cfg_default = agent_brain::config::config_from_value(
        &serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap(),
    )
    .unwrap();
    let wfs = agent_brain::workflows::load_from(None, &cfg_default);
    let nhanh = wfs.get("nhanh").unwrap();
    agent_brain::db::set_task_workflow_if_null(&conn, child_id, &nhanh.to_snapshot()).unwrap();

    // Check before: not approved
    let auto_before = agent_brain::db::task_auto(&conn, child_id).unwrap().unwrap();
    assert_eq!(auto_before.approved_at, None);

    // Run auto on child task. When entering Gate, it auto-approves!
    let out = sb.run(&["auto", &child_id.to_string()], None);
    assert!(
        out.stdout.contains("tự duyệt plan việc con") || out.stderr.contains("tự duyệt plan việc con"),
        "missing auto-approve notice:\nstdout: {}\nstderr: {}",
        out.stdout,
        out.stderr
    );

    let auto_after = agent_brain::db::task_auto(&conn, child_id).unwrap().unwrap();
    assert!(auto_after.approved_at.is_some(), "child plan must be approved");
    assert_eq!(auto_after.approved_commit.as_deref().map(|c| c.len()), Some(40));
}
