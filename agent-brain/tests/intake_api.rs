mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::setup_project;
use serde_json::json;

#[test]
fn create_intake_ui_and_list() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let srv = sb.serve(&[]);

    // 1. POST /api/intake without project
    let resp = srv.post_json(
        "/api/intake",
        &json!({
            "request": "Thêm tính năng thanh toán",
            "priority": "urgent"
        }),
    );
    assert_eq!(resp.status, 201, "body: {}", resp.text());
    let body = resp.json();
    assert_eq!(body["id"], 1);

    // 2. GET /api/intake
    let resp = srv.get("/api/intake");
    assert_eq!(resp.status, 200);
    let list = resp.json();
    let arr = list.as_array().expect("array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], 1);
    assert_eq!(arr[0]["source"], "ui");
    assert_eq!(arr[0]["request"], "Thêm tính năng thanh toán");
    assert_eq!(arr[0]["priority"], "urgent");
    assert_eq!(arr[0]["status"], "queued");
    assert!(arr[0]["project_id"].is_null());

    // Filter status=queued
    let resp = srv.get("/api/intake?status=queued");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.json().as_array().unwrap().len(), 1);

    // Filter status=triaging (empty)
    let resp = srv.get("/api/intake?status=triaging");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.json().as_array().unwrap().len(), 0);
}

#[test]
fn create_intake_with_project_and_cancel() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let srv = sb.serve(&[]);

    // POST with project slug
    let resp = srv.post_json(
        "/api/intake",
        &json!({
            "request": "Sửa lỗi giỏ hàng",
            "project": "web-ban-hang"
        }),
    );
    assert_eq!(resp.status, 201, "body: {}", resp.text());
    let id = resp.json()["id"].as_i64().unwrap();
    assert_eq!(id, 1);

    // Check project_id was resolved
    let resp = srv.get("/api/intake");
    let arr = resp.json();
    assert_eq!(arr[0]["priority"], "normal"); // default priority
    assert!(arr[0]["project_id"].is_number());

    // Cancel intake
    let resp = srv.post_json("/api/intake/1/cancel", &json!({}));
    assert_eq!(resp.status, 200);
    assert_eq!(resp.json()["status"], "cancelled");

    // Verify list reflects cancelled
    let resp = srv.get("/api/intake?status=cancelled");
    assert_eq!(resp.json().as_array().unwrap().len(), 1);

    // Cancel 404
    let resp = srv.post_json("/api/intake/999/cancel", &json!({}));
    assert_eq!(resp.status, 404);
}

#[test]
fn intake_input_validations() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let srv = sb.serve(&[]);

    // Empty request
    let resp = srv.post_json("/api/intake", &json!({ "request": "   " }));
    assert_eq!(resp.status, 422);

    // Invalid priority
    let resp = srv.post_json(
        "/api/intake",
        &json!({
            "request": "test",
            "priority": "invalid_priority"
        }),
    );
    assert_eq!(resp.status, 422);

    // Non-existent project
    let resp = srv.post_json(
        "/api/intake",
        &json!({
            "request": "test",
            "project": "khong-ton-tai"
        }),
    );
    assert_eq!(resp.status, 422);

    // Request too long
    let long_req = "a".repeat(20_001);
    let resp = srv.post_json("/api/intake", &json!({ "request": long_req }));
    assert_eq!(resp.status, 422);
}

#[test]
fn proposals_flow_priority_approve_reject() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    let srv = sb.serve(&[]);

    // Insert an intake and proposal directly into db
    let conn = sb.db();
    let intake_id =
        agent_brain::db::insert_intake(&conn, "ui", None, "yêu cầu tạo giao diện", "normal")
            .unwrap();
    let proj_id = agent_brain::db::project_row_id(&conn, "web-ban-hang").unwrap();

    let prop_id = agent_brain::db::insert_proposal(
        &conn,
        &agent_brain::db::NewProposal {
            intake_id,
            project_id: proj_id,
            title: "Tạo trang chủ",
            request: "Dựng trang chủ mới",
            priority: "normal",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: Some("Được agent điều phối đề xuất"),
        },
    )
    .unwrap();
    drop(conn);

    // GET /api/proposals
    let resp = srv.get("/api/proposals");
    assert_eq!(resp.status, 200);
    let props = resp.json();
    let arr = props.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], prop_id);
    assert_eq!(arr[0]["status"], "pending");
    assert_eq!(arr[0]["priority"], "normal");

    // Change priority to urgent
    let resp = srv.post_json(
        &format!("/api/proposals/{prop_id}/priority"),
        &json!({ "priority": "urgent" }),
    );
    assert_eq!(resp.status, 200);
    assert_eq!(resp.json()["priority"], "urgent");

    // Invalid priority
    let resp = srv.post_json(
        &format!("/api/proposals/{prop_id}/priority"),
        &json!({ "priority": "super" }),
    );
    assert_eq!(resp.status, 422);

    // Branch count before approve
    let branches_before = sb.git(&repo, &["branch"]);

    // Approve proposal
    let resp = srv.post_json(&format!("/api/proposals/{prop_id}/approve"), &json!({}));
    assert_eq!(resp.status, 200);
    assert_eq!(resp.json()["status"], "approved");

    // Acceptance 3: approve chỉ đổi trạng thái sang approved, không tạo task, không tạo nhánh git
    let conn = sb.db();
    let prop = agent_brain::db::get_proposal(&conn, prop_id)
        .unwrap()
        .unwrap();
    assert_eq!(prop.status, "approved");

    let task_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        task_count, 0,
        "approve must NOT create any task row directly"
    );
    drop(conn);

    let branches_after = sb.git(&repo, &["branch"]);
    assert_eq!(
        branches_before, branches_after,
        "approve must NOT create any git branch"
    );

    // Cannot approve again (only pending allowed)
    let resp = srv.post_json(&format!("/api/proposals/{prop_id}/approve"), &json!({}));
    assert_eq!(resp.status, 409);

    // Priority can still be changed when approved
    let resp = srv.post_json(
        &format!("/api/proposals/{prop_id}/priority"),
        &json!({ "priority": "low" }),
    );
    assert_eq!(resp.status, 200);
    assert_eq!(resp.json()["priority"], "low");

    // Cannot reject when approved
    let resp = srv.post_json(
        &format!("/api/proposals/{prop_id}/reject"),
        &json!({ "reason": "hủy" }),
    );
    assert_eq!(resp.status, 409);
}

#[test]
fn reject_proposal_flow() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let srv = sb.serve(&[]);

    let conn = sb.db();
    let intake_id =
        agent_brain::db::insert_intake(&conn, "ui", None, "yêu cầu tính năng", "normal").unwrap();
    let proj_id = agent_brain::db::project_row_id(&conn, "web-ban-hang").unwrap();

    let prop_id = agent_brain::db::insert_proposal(
        &conn,
        &agent_brain::db::NewProposal {
            intake_id,
            project_id: proj_id,
            title: "Tính năng thừa",
            request: "Xóa toàn bộ database",
            priority: "low",
            team: "mac-dinh",
            workflow: "mac-dinh",
            reason: None,
        },
    )
    .unwrap();
    drop(conn);

    // Reject with reason
    let resp = srv.post_json(
        &format!("/api/proposals/{prop_id}/reject"),
        &json!({ "reason": "Yêu cầu nguy hiểm" }),
    );
    assert_eq!(resp.status, 200);
    assert_eq!(resp.json()["status"], "rejected");

    let conn = sb.db();
    let prop = agent_brain::db::get_proposal(&conn, prop_id)
        .unwrap()
        .unwrap();
    assert_eq!(prop.status, "rejected");
    assert_eq!(prop.reason.as_deref(), Some("Yêu cầu nguy hiểm"));
    drop(conn);

    // Cannot change priority when rejected
    let resp = srv.post_json(
        &format!("/api/proposals/{prop_id}/priority"),
        &json!({ "priority": "urgent" }),
    );
    assert_eq!(resp.status, 409);

    // Cannot approve when rejected
    let resp = srv.post_json(&format!("/api/proposals/{prop_id}/approve"), &json!({}));
    assert_eq!(resp.status, 409);
}

#[test]
fn orchestrator_status_and_defaults() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let srv = sb.serve(&[]);

    let resp = srv.get("/api/orchestrator/status");
    assert_eq!(resp.status, 200);
    let body = resp.json();
    assert_eq!(body["enabled"], false);
    assert_eq!(body["max_parallel"], 3);
    assert_eq!(body["auto_resume_quota"], false);
    assert_eq!(body["resume_after_minutes"], 60);
}

#[test]
fn next_approved_proposal_ordering_and_count_active_tasks() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    let conn = agent_brain::db::open(&db_path).unwrap();

    let p = agent_brain::projects::Project {
        id: "proj".to_string(),
        path: "/path".to_string(),
        name_vi: "Proj".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    agent_brain::db::sync_projects(&conn, &[p]).unwrap();
    let proj_id = agent_brain::db::project_row_id(&conn, "proj").unwrap();

    let intake_id =
        agent_brain::db::insert_intake(&conn, "ui", Some(proj_id), "req", "normal").unwrap();

    // 1. Insert 3 proposals: low, normal, urgent
    let p_low = agent_brain::db::insert_proposal(
        &conn,
        &agent_brain::db::NewProposal {
            intake_id,
            project_id: proj_id,
            title: "Low task",
            request: "low req",
            priority: "low",
            team: "team",
            workflow: "wf",
            reason: None,
        },
    )
    .unwrap();
    let p_normal = agent_brain::db::insert_proposal(
        &conn,
        &agent_brain::db::NewProposal {
            intake_id,
            project_id: proj_id,
            title: "Normal task",
            request: "normal req",
            priority: "normal",
            team: "team",
            workflow: "wf",
            reason: None,
        },
    )
    .unwrap();
    let p_urgent = agent_brain::db::insert_proposal(
        &conn,
        &agent_brain::db::NewProposal {
            intake_id,
            project_id: proj_id,
            title: "Urgent task",
            request: "urgent req",
            priority: "urgent",
            team: "team",
            workflow: "wf",
            reason: None,
        },
    )
    .unwrap();

    // None approved yet
    assert!(agent_brain::db::next_approved_proposal(&conn)
        .unwrap()
        .is_none());

    // Approve all three
    agent_brain::db::set_proposal_status(&conn, p_low, "approved").unwrap();
    agent_brain::db::set_proposal_status(&conn, p_normal, "approved").unwrap();
    agent_brain::db::set_proposal_status(&conn, p_urgent, "approved").unwrap();

    // next_approved_proposal must pick urgent first!
    let next = agent_brain::db::next_approved_proposal(&conn)
        .unwrap()
        .expect("next proposal");
    assert_eq!(next.id, p_urgent);
    assert_eq!(next.priority, "urgent");

    // Now set urgent to started
    agent_brain::db::set_proposal_status(&conn, p_urgent, "started").unwrap();

    // Next must be normal
    let next = agent_brain::db::next_approved_proposal(&conn)
        .unwrap()
        .expect("next proposal");
    assert_eq!(next.id, p_normal);
    assert_eq!(next.priority, "normal");

    // 2. Test count_active_tasks
    assert_eq!(agent_brain::db::count_active_tasks(&conn).unwrap(), 0);

    let task_id = agent_brain::db::upsert_task(
        &conn,
        proj_id,
        "task-1",
        "Task 1",
        "task/1",
        Some("main"),
        None,
    )
    .unwrap();

    // Running run for task_id
    let run = agent_brain::db::NewRun {
        task_id: Some(task_id),
        project_id: proj_id,
        role: "dev".to_string(),
        profile: "work".to_string(),
        model: None,
        pid: 100,
        pgid: 100,
        pid_start: 1,
        base_commit: None,
        run_dir: None,
    };
    let _run_id = agent_brain::db::insert_run(&conn, &run).unwrap();

    // 1 active task from running run
    assert_eq!(agent_brain::db::count_active_tasks(&conn).unwrap(), 1);

    // Set auto_state to 'running' on the SAME task -> count must still be 1 (deduplicated)
    agent_brain::db::set_auto_state(&conn, task_id, Some("running")).unwrap();
    assert_eq!(agent_brain::db::count_active_tasks(&conn).unwrap(), 1);

    // Create a second task with auto_state='running' but no run -> count becomes 2
    let task_id_2 = agent_brain::db::upsert_task(
        &conn,
        proj_id,
        "task-2",
        "Task 2",
        "task/2",
        Some("main"),
        None,
    )
    .unwrap();
    agent_brain::db::set_auto_state(&conn, task_id_2, Some("running")).unwrap();
    assert_eq!(agent_brain::db::count_active_tasks(&conn).unwrap(), 2);
}

#[test]
fn create_task_endpoint_routes_to_intake_when_orchestrator_enabled() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    sb.write_config(json!({
        "orchestrator": {
            "enabled": true
        }
    }));

    let srv = sb.serve(&[]);

    // POST /api/tasks khi orchestrator.enabled = true
    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "Yêu cầu qua giao diện"
        }),
    );
    assert_eq!(resp.status, 201, "body: {}", resp.text());
    let body = resp.json();
    assert_eq!(body["status"], "queued");
    let intake_id = body["intake_id"].as_i64().expect("intake_id");
    assert_eq!(intake_id, 1);

    // Không có task nào được tạo trong bảng tasks
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let task_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(task_count, 0, "No task should be created directly");

    // Dòng intake được tạo trong bảng intake
    let intake = agent_brain::db::get_intake(&conn, intake_id)
        .unwrap()
        .expect("intake exists");
    assert_eq!(intake.source, "ui");
    assert_eq!(intake.status, "queued");
    assert_eq!(intake.request, "Yêu cầu qua giao diện");
    assert!(intake.project_id.is_some());
}
