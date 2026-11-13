mod common;
#[allow(unused)]
#[path = "support/protocol.rs"]
mod protocol;
#[allow(unused)]
#[path = "support/telegram.rs"]
mod tg;

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
    app_state_with_exe(sb, common::bin())
}

fn app_state_with_exe(sb: &Sandbox, exe: std::path::PathBuf) -> std::sync::Arc<agent_brain::dashboard::AppState> {
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
        exe: exe.clone(),
        hints: tokio::sync::broadcast::channel(16).0,
        shutdown: tokio::sync::watch::channel(false).1,
        jobs: agent_brain::dashboard::jobs::JobManager::new(exe),
    })
}

fn setup_fake_job(sb: &Sandbox) -> std::path::PathBuf {
    let script = sb.root.join("fake_job.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").expect("write fake_job.sh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod");
    }
    script
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

#[test]
fn scheduler_handles_split_queue_respecting_parallel_and_dependencies() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    // Enable worktree mode on project so child tasks can run in parallel
    let mut map = sb.read_map();
    map["projects"][0]["workspace"] = serde_json::json!("worktree");
    map["projects"][0]["max_parallel_tasks"] = serde_json::json!(3);
    sb.write_map(map["projects"].clone(), &[]);

    let fake_job = setup_fake_job(&sb);

    let (conn, pid) = init_test_db(&sb);
    let parent_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "parent-sch",
        "Parent Scheduler Task",
        "task/parent-sch",
        Some("main"),
        None,
    )
    .unwrap();

    let cfg_default = agent_brain::config::config_from_value(
        &serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap(),
    )
    .unwrap();
    let wfs = agent_brain::workflows::load_from(None, &cfg_default);
    let nhanh = wfs.get("nhanh").unwrap();
    agent_brain::db::set_task_workflow_if_null(&conn, parent_id, &nhanh.to_snapshot()).unwrap();
    agent_brain::db::set_auto_state(&conn, parent_id, Some("waiting_children")).unwrap();

    let st = app_state_with_exe(&sb, fake_job);
    st.orch.write().unwrap().max_parallel = 2;

    // 4 items:
    // Item 1: independent
    // Item 2: independent
    // Item 3: after 1
    // Item 4: after 2
    let items = vec![
        agent_brain::db::NewSplitItem {
            idx: 1,
            title: "Item 1".to_string(),
            request: "Req 1".to_string(),
            after_idx: None,
        },
        agent_brain::db::NewSplitItem {
            idx: 2,
            title: "Item 2".to_string(),
            request: "Req 2".to_string(),
            after_idx: None,
        },
        agent_brain::db::NewSplitItem {
            idx: 3,
            title: "Item 3".to_string(),
            request: "Req 3".to_string(),
            after_idx: Some(1),
        },
        agent_brain::db::NewSplitItem {
            idx: 4,
            title: "Item 4".to_string(),
            request: "Req 4".to_string(),
            after_idx: Some(2),
        },
    ];
    agent_brain::db::insert_split_items(&conn, parent_id, "commit_plan", &items).unwrap();

    // Tick 1: limit is 2 -> starts items 1 and 2
    agent_brain::orchestrator::scheduler::schedule_tick(&st).unwrap();

    let items_after_t1 = agent_brain::db::split_items_of(&conn, parent_id).unwrap();
    assert_eq!(items_after_t1[0].state, "started");
    assert_eq!(items_after_t1[1].state, "started");
    assert_eq!(items_after_t1[2].state, "waiting");
    assert_eq!(items_after_t1[3].state, "waiting");

    let c1_id = items_after_t1[0].child_task_id.unwrap();
    let c2_id = items_after_t1[1].child_task_id.unwrap();
    assert_eq!(agent_brain::db::task_parent(&conn, c1_id).unwrap(), Some(parent_id));
    assert_eq!(agent_brain::db::task_parent(&conn, c2_id).unwrap(), Some(parent_id));

    // Let fake jobs exit
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Commit a file on child 1's branch so it is not yet merged into main
    let _ = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "checkout", "-b", "task/parent-sch-c1"])
        .output()
        .unwrap();
    std::fs::write(repo.join("c1.txt"), "c1 content").unwrap();
    let _ = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "add", "c1.txt"])
        .output()
        .unwrap();
    let _ = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "commit", "-m", "c1 work"])
        .output()
        .unwrap();
    let _ = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "checkout", "main"])
        .output()
        .unwrap();

    // Now child 1 finishes: Passed
    conn.execute("UPDATE runs SET status = 'succeeded' WHERE task_id = ?1", rusqlite::params![c1_id]).unwrap();
    agent_brain::db::set_task_status(&conn, c1_id, "qa_pass").unwrap();
    agent_brain::db::set_auto_state(&conn, c1_id, Some("done")).unwrap();

    // And child 2 finishes: Failed
    conn.execute("UPDATE runs SET status = 'failed' WHERE task_id = ?1", rusqlite::params![c2_id]).unwrap();
    agent_brain::db::set_task_status(&conn, c2_id, "failed").unwrap();
    agent_brain::db::set_auto_state(&conn, c2_id, Some("stopped")).unwrap();

    // For child 1: its branch task/parent-sch-c1 needs to be merged into main for item 3 to proceed
    // Before merge: tick should skip item 3 because branch is not merged
    agent_brain::orchestrator::scheduler::schedule_tick(&st).unwrap();
    let items_after_t2 = agent_brain::db::split_items_of(&conn, parent_id).unwrap();
    assert_eq!(items_after_t2[2].state, "waiting", "item 3 waits for branch merge");
    assert_eq!(items_after_t2[3].state, "skipped", "item 4 skipped because item 2 failed");
    assert_eq!(items_after_t2[3].reason.as_deref(), Some("Việc #2 không đạt"));

    // Now merge child 1 branch into main
    let merge_out = std::process::Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "merge", "--no-ff", "task/parent-sch-c1", "-m", "merge c1"])
        .output()
        .unwrap();
    assert!(merge_out.status.success(), "merge c1 branch succeeds: {}", String::from_utf8_lossy(&merge_out.stderr));

    // Tick 3: now item 3 can start!
    agent_brain::orchestrator::scheduler::schedule_tick(&st).unwrap();
    let items_after_t3 = agent_brain::db::split_items_of(&conn, parent_id).unwrap();
    assert_eq!(items_after_t3[2].state, "started", "item 3 started after merge");
    let c3_id = items_after_t3[2].child_task_id.unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));
    conn.execute("UPDATE runs SET status = 'succeeded' WHERE task_id = ?1", rusqlite::params![c3_id]).unwrap();

    // Child 3 passes
    agent_brain::db::set_task_status(&conn, c3_id, "qa_pass").unwrap();
    agent_brain::db::set_auto_state(&conn, c3_id, Some("done")).unwrap();

    // Tick 4: all settled -> aggregate report!
    agent_brain::orchestrator::scheduler::schedule_tick(&st).unwrap();
    let parent_after = agent_brain::db::get_task(&conn, parent_id).unwrap().unwrap();
    assert_eq!(parent_after.status, "split_done");
    let parent_auto = agent_brain::db::task_auto(&conn, parent_id).unwrap().unwrap();
    assert_eq!(parent_auto.auto_state.as_deref(), Some("done"));

    // Check report file exists and has 4 rows
    let report_content = agent_brain::answers::read("web-ban-hang", parent_id, &parent_after.slug).unwrap().unwrap();
    assert!(report_content.contains("# Báo cáo gộp: Parent Scheduler Task"));
    assert!(report_content.contains("| 1 | Item 1 |"));
    assert!(report_content.contains("| 2 | Item 2 |"));
    assert!(report_content.contains("| 3 | Item 3 |"));
    assert!(report_content.contains("| 4 | Item 4 |"));

    // Tick 5: tick again doesn't start anything new or overwrite status
    agent_brain::orchestrator::scheduler::schedule_tick(&st).unwrap();
    let parent_after_t5 = agent_brain::db::get_task(&conn, parent_id).unwrap().unwrap();
    assert_eq!(parent_after_t5.status, "split_done");

    st.jobs.kill_all_now();
}

#[test]
fn test_telegram_render_split_plan_and_detect_split_done() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let (conn, pid) = init_test_db(&sb);
    let st = app_state(&sb);

    let parent_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "parent-tg",
        "Parent Telegram Task",
        "task/parent-tg",
        Some("main"),
        None,
    )
    .unwrap();

    let plan_text = r#"# Plan: Demo task tách việc

## Tách việc
| # | Việc con | Yêu cầu | Phụ thuộc |
|---|---|---|---|
| 1 | Đăng nhập CMS | Làm form login | — |
| 2 | Quản lý sản phẩm | CRUD sản phẩm | Sau #1 |
| 3 | Xuất báo cáo | Xuất Excel | Sau #2 |
"#;

    // Test Gate message rendering with split plan
    let mut state = agent_brain::dashboard::api_tasks::load_task_state(&st, &conn, parent_id).unwrap().unwrap();
    state.plan_text = Some(plan_text.to_string());
    state.view.controls.push(agent_brain::dashboard::taskstate::ActionView {
        id: "approve".to_string(),
        label: "Duyệt".to_string(),
        enabled: true,
        reason: None,
        run_id: None,
    });

    let ev_gate = agent_brain::telegram::detect::Event {
        task_id: parent_id,
        kind: agent_brain::telegram::detect::EventKind::Gate,
        key: "gate:commit123".to_string(),
        is_update: false,
    };
    let rendered_gate = agent_brain::telegram::render::message_for(&ev_gate, &state);
    assert!(rendered_gate.text.contains("Tách thành 3 việc con:"), "text: {}", rendered_gate.text);
    assert!(rendered_gate.text.contains("1. Đăng nhập CMS (Phụ thuộc: —)"), "text: {}", rendered_gate.text);
    assert!(rendered_gate.text.contains("2. Quản lý sản phẩm (Phụ thuộc: #1)"), "text: {}", rendered_gate.text);
    assert!(rendered_gate.text.contains("3. Xuất báo cáo (Phụ thuộc: #2)"), "text: {}", rendered_gate.text);
    let gate_btns = agent_brain::telegram::render::buttons_for(&ev_gate, &state);
    assert!(!gate_btns.is_empty(), "Gate buttons must be present");

    // Test child task gate skip in scan
    let child_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "child-tg",
        "Child Telegram Task",
        "task/child-tg",
        Some("main"),
        None,
    )
    .unwrap();
    agent_brain::db::set_task_parent(&conn, child_id, parent_id).unwrap();

    // Detector scan skips Gate when task has parent
    let mut detector = agent_brain::telegram::detect::Detector::default();
    let reg = std::sync::Mutex::new(agent_brain::telegram::outbox::Registry::default());
    // Simulate detector having seen or not seen
    let scan_out = detector.scan(&st, &conn, true, &reg).unwrap();
    assert!(
        scan_out.iter().all(|item| match item {
            agent_brain::telegram::outbox::Outgoing::Notify(ev) => ev.task_id != child_id,
            _ => true,
        }),
        "Child task gate notification must be skipped"
    );

    // Test SplitDone detection and rendering
    std::env::set_var("XDG_DATA_HOME", &sb.xdg_data);
    let report_content = r#"# Báo cáo gộp: Parent Telegram Task
- **Tổng kết**: Đạt 2 / Không đạt 1 / Hủy 0 / Tổng 3 · Chi phí: $0.1500

| # | Việc con | Task | Kết quả | Ghi chú |
|---|---|---|---|---|
| 1 | Đăng nhập CMS | #2 dang-nhap-cms | Đạt | Xong login |
| 2 | Quản lý sản phẩm | #3 quan-ly-san-pham | Đạt | Xong CRUD |
| 3 | Xuất báo cáo | #4 xuat-bao-cao | Không đạt | Lỗi export |

## Lỗi nổi bật
- #3: Không tìm thấy thư viện excel
"#;
    agent_brain::answers::save("web-ban-hang", parent_id, "parent-tg", report_content).unwrap();
    agent_brain::db::set_task_status(&conn, parent_id, "split_done").unwrap();

    let parent_split_state = agent_brain::dashboard::api_tasks::load_task_state(&st, &conn, parent_id).unwrap().unwrap();
    let ev_split = agent_brain::telegram::detect::current_event(&parent_split_state).expect("current_event for split_done");
    assert_eq!(ev_split.kind, agent_brain::telegram::detect::EventKind::SplitDone);
    assert_eq!(ev_split.key, format!("split:{}:{}", parent_id, parent_split_state.task.updated_at));

    let rendered_split = agent_brain::telegram::render::message_for(&ev_split, &parent_split_state);
    assert!(rendered_split.text.contains("Tách việc xong: Parent Telegram Task — Đạt 2 / Không đạt 1 / Hủy 0 / Tổng 3"), "text: {}", rendered_split.text);
    assert!(rendered_split.text.contains("Lỗi nổi bật:"), "text: {}", rendered_split.text);
    assert!(rendered_split.text.contains("- #3: Không tìm thấy thư viện excel"), "text: {}", rendered_split.text);
    assert!(rendered_split.text.contains("Báo cáo gộp chi tiết ở file đính kèm ngay trên."), "text: {}", rendered_split.text);

    let split_btns = agent_brain::telegram::render::buttons_for(&ev_split, &parent_split_state);
    assert!(split_btns.is_empty(), "SplitDone has no buttons");
}

#[test]
fn telegram_split_done_notifies_with_document_and_no_dup() {
    let sb = Sandbox::new();
    let tg = tg::FakeTelegram::start(&sb);
    tg::write_config(&sb, tg::PAIRED_CHAT, 0o600);
    let _repo = setup_project(&sb, "work");

    let (conn, pid) = init_test_db(&sb);
    let parent_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "parent-e2e",
        "Parent E2E Task",
        "task/parent-e2e",
        Some("main"),
        None,
    )
    .unwrap();

    // Write report in answers dir before status split_done
    let report_path = sb.xdg_data.join("agent-brain/answers/web-ban-hang").join(format!("{parent_id}-parent-e2e.md"));
    std::fs::create_dir_all(report_path.parent().unwrap()).unwrap();
    let report_content = r#"# Báo cáo gộp: Parent E2E Task
- **Tổng kết**: Đạt 1 / Không đạt 0 / Hủy 0 / Tổng 1 · Chi phí: $0.0500

| # | Việc con | Task | Kết quả | Ghi chú |
|---|---|---|---|---|
| 1 | Child 1 | #2 child-1 | Đạt | OK |
"#;
    std::fs::write(&report_path, report_content).unwrap();

    // Also insert child task with gate to verify it is NOT notified
    let child_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "child-e2e",
        "Child E2E Task",
        "task/child-e2e",
        Some("main"),
        None,
    )
    .unwrap();
    agent_brain::db::set_task_parent(&conn, child_id, parent_id).unwrap();

    let mut srv = tg.serve(&sb, &[]);
    assert!(
        common::wait_until(std::time::Duration::from_secs(10), || {
            srv.stdout.lock().unwrap().contains("Telegram: đã kết nối (@ab_fake_bot)")
        }),
        "Telegram failed to connect"
    );

    // After serve is connected, update task status to split_done
    agent_brain::db::set_task_status(&conn, parent_id, "split_done").unwrap();

    // Wait for SplitDone message
    let msg = tg
        .wait_call("sendMessage", std::time::Duration::from_secs(15), |r| {
            r["json"]["text"].as_str().unwrap_or("").starts_with("Tách việc xong")
        })
        .expect("expected sendMessage for SplitDone");

    let text = msg["json"]["text"].as_str().unwrap();
    assert!(text.contains("Parent E2E Task — Đạt 1 / Không đạt 0 / Hủy 0 / Tổng 1"), "unexpected text: {text}");
    assert!(text.contains("Báo cáo gộp chi tiết ở file đính kèm ngay trên."), "missing file attachment notice in text: {text}");

    // Verify sendDocument was called
    let docs = tg.calls("sendDocument");
    assert_eq!(docs.len(), 1, "sendDocument: {docs:#?}");
    assert_eq!(docs[0]["form"]["document"]["filename"], serde_json::json!(format!("{parent_id}-parent-e2e.md")));
    assert_eq!(docs[0]["form"]["caption"], serde_json::json!(format!("Báo cáo gộp task {parent_id} — web-ban-hang")));

    // Child task with gate must not have sent any message
    let gate_msgs: Vec<_> = tg
        .calls("sendMessage")
        .into_iter()
        .filter(|r| r["json"]["text"].as_str().unwrap_or("").starts_with("Chờ duyệt plan"))
        .collect();
    assert!(gate_msgs.is_empty(), "Child task gate must not be notified: {gate_msgs:#?}");

    // Rescan does not send duplicate
    std::thread::sleep(std::time::Duration::from_secs(2));
    let split_msgs: Vec<_> = tg
        .calls("sendMessage")
        .into_iter()
        .filter(|r| r["json"]["text"].as_str().unwrap_or("").starts_with("Tách việc xong"))
        .collect();
    assert_eq!(split_msgs.len(), 1, "SplitDone must be sent exactly once: {split_msgs:#?}");

    assert_eq!(srv.stop(libc::SIGINT, std::time::Duration::from_secs(10)), Some(0));
}

#[test]
fn test_cli_split_status_and_aggregate() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let (conn, pid) = init_test_db(&sb);
    let parent_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "parent-cli",
        "Parent CLI Task",
        "task/parent-cli",
        Some("main"),
        None,
    )
    .unwrap();

    // Split status when no items
    let out0 = sb.run(&["split", "status", &parent_id.to_string()], None);
    assert_eq!(out0.code, 0);
    assert!(out0.stdout.contains("không có danh sách việc con"), "stdout: {}", out0.stdout);

    // Insert split items
    let items = vec![
        agent_brain::db::NewSplitItem {
            idx: 1,
            title: "CLI Item 1".to_string(),
            request: "Do 1".to_string(),
            after_idx: None,
        },
        agent_brain::db::NewSplitItem {
            idx: 2,
            title: "CLI Item 2".to_string(),
            request: "Do 2".to_string(),
            after_idx: Some(1),
        },
    ];
    agent_brain::db::insert_split_items(&conn, parent_id, "commit_cli", &items).unwrap();

    // Split status with items
    let out1 = sb.run(&["split", "status", &parent_id.to_string()], None);
    assert_eq!(out1.code, 0);
    assert!(out1.stdout.contains("CLI Item 1"), "stdout: {}", out1.stdout);
    assert!(out1.stdout.contains("CLI Item 2"), "stdout: {}", out1.stdout);
    assert!(out1.stdout.contains("chờ"), "stdout: {}", out1.stdout);

    // Split aggregate
    let out2 = sb.run(&["split", "aggregate", &parent_id.to_string()], None);
    assert_eq!(out2.code, 0, "stderr: {}", out2.stderr);
    assert!(out2.stdout.contains("Đã tổng hợp báo cáo gộp cho task #"), "stdout: {}", out2.stdout);

    let parent_row = agent_brain::db::get_task(&conn, parent_id).unwrap().unwrap();
    assert_eq!(parent_row.status, "split_done");
}
