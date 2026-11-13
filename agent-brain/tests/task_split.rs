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
