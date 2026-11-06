#![allow(clippy::while_let_loop)]

mod common;

use common::Sandbox;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

fn stop(mut srv: common::Serve) {
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

fn dept_ids(j: &Value) -> Vec<String> {
    j["departments"]
        .as_array()
        .expect("departments array")
        .iter()
        .map(|d| d["id"].as_str().unwrap().to_string())
        .collect()
}

fn find_dept<'a>(j: &'a Value, id: &str) -> &'a Value {
    j["departments"]
        .as_array()
        .expect("departments array")
        .iter()
        .find(|d| d["id"] == id)
        .unwrap_or_else(|| panic!("department {id} not found in {j}"))
}

fn find_member<'a>(dept: &'a Value, member_id: &str) -> Option<&'a Value> {
    dept["members"]
        .as_array()
        .expect("members array")
        .iter()
        .find(|m| m["id"] == member_id)
}

#[test]
fn departments_get_empty_or_absent() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);

    let r = srv.get("/api/departments");
    assert_eq!(r.status, 200);
    let j = r.json();
    assert_eq!(j["version"], "absent");
    assert_eq!(j["file_exists"], false);
    assert_eq!(j["errors"], json!([]));
    assert_eq!(
        dept_ids(&j),
        ["phan-tich", "ke-hoach", "thi-cong", "qa", "bao-mat"]
    );

    for dept in j["departments"].as_array().unwrap() {
        assert_eq!(dept["members"].as_array().unwrap().len(), 0);
    }

    stop(srv);
}

#[test]
fn departments_init_example() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);

    // 1. Initial get: absent
    let r1 = srv.get("/api/departments");
    assert_eq!(r1.json()["file_exists"], false);

    // 2. Init from example
    let r2 = srv.post_json("/api/departments/init-example", &json!({}));
    assert_eq!(r2.status, 200);
    let j2 = r2.json();
    assert_eq!(j2["ok"], true);
    assert_eq!(j2["file_exists"], true);
    assert_ne!(j2["version"], "absent");
    assert_eq!(j2["errors"], json!([]));

    let pt = find_dept(&j2, "phan-tich");
    assert_eq!(pt["members"].as_array().unwrap().len(), 6);
    let opus1 = find_member(pt, "opus-1").expect("opus-1 in phan-tich");
    assert_eq!(opus1["display_name"], "Senior Opus 1");
    assert_eq!(opus1["engine"], "claude");
    assert_eq!(opus1["model"], "claude-opus-5-5");
    assert_eq!(opus1["levels"], json!(["vua", "kho"]));

    // 3. Init again -> 409 Conflict
    let r3 = srv.post_json("/api/departments/init-example", &json!({}));
    assert_eq!(r3.status, 409);
    assert_eq!(r3.json()["code"], "conflict");

    stop(srv);
}

#[test]
fn departments_save_valid_and_edit_and_move() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);

    // 1. Init example first
    let r_init = srv.post_json("/api/departments/init-example", &json!({}));
    assert_eq!(r_init.status, 200);
    let version = r_init.json()["version"].as_str().unwrap().to_string();

    // 2. Add a new member into phan-tich
    let new_member_body = json!({
        "version": version,
        "department": "phan-tich",
        "id": "new-analyst",
        "member": {
            "display_name": "Analyst Mới",
            "engine": "claude",
            "model": "claude-sonnet-5-5",
            "levels": ["de", "vua"],
            "notes": "Chuyên gia phân tích sơ bộ"
        }
    });
    let r_save = srv.post_json("/api/departments/member/save", &new_member_body);
    assert_eq!(r_save.status, 200, "{}", r_save.text());
    let j_save = r_save.json();
    assert_eq!(j_save["ok"], true);
    let v2 = j_save["version"].as_str().unwrap().to_string();

    let pt = find_dept(&j_save, "phan-tich");
    let m = find_member(pt, "new-analyst").expect("new-analyst in phan-tich");
    assert_eq!(m["display_name"], "Analyst Mới");
    assert_eq!(m["model"], "claude-sonnet-5-5");
    assert_eq!(m["levels"], json!(["de", "vua"]));

    // 3. Move member to thi-cong
    let move_body = json!({
        "version": v2,
        "department": "thi-cong",
        "id": "new-analyst",
        "old_department": "phan-tich",
        "member": {
            "display_name": "Dev Chuyển Phòng",
            "engine": "claude",
            "model": "claude-opus-5-5",
            "levels": ["kho"],
            "notes": "Đã chuyển sang thi công"
        }
    });
    let r_move = srv.post_json("/api/departments/member/save", &move_body);
    assert_eq!(r_move.status, 200, "{}", r_move.text());
    let j_move = r_move.json();
    assert_eq!(j_move["ok"], true);

    // Verify it is in thi-cong and NOT in phan-tich
    let pt2 = find_dept(&j_move, "phan-tich");
    assert!(find_member(pt2, "new-analyst").is_none());

    let tc = find_dept(&j_move, "thi-cong");
    let moved_m = find_member(tc, "new-analyst").expect("new-analyst in thi-cong");
    assert_eq!(moved_m["display_name"], "Dev Chuyển Phòng");
    assert_eq!(moved_m["levels"], json!(["kho"]));

    stop(srv);
}

#[test]
fn departments_save_validation_errors() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);

    let r_init = srv.post_json("/api/departments/init-example", &json!({}));
    assert_eq!(r_init.status, 200);
    let version = r_init.json()["version"].as_str().unwrap().to_string();

    // 1. Human engine rejected
    let body_human = json!({
        "version": version,
        "department": "thi-cong",
        "id": "human-dev",
        "member": {
            "display_name": "Dev Người Thật",
            "engine": "người:ha",
            "levels": ["de"]
        }
    });
    let r1 = srv.post_json("/api/departments/member/save", &body_human);
    assert_eq!(r1.status, 422);
    let j1 = r1.json();
    assert!(j1["errors"].as_array().unwrap().iter().any(|e| e["field"] == "engine"));

    // 2. Empty levels rejected
    let body_empty_levels = json!({
        "version": version,
        "department": "qa",
        "id": "qa-tester",
        "member": {
            "display_name": "QA Tester",
            "engine": "claude",
            "levels": []
        }
    });
    let r2 = srv.post_json("/api/departments/member/save", &body_empty_levels);
    assert_eq!(r2.status, 422);
    let j2 = r2.json();
    assert!(j2["errors"].as_array().unwrap().iter().any(|e| e["field"] == "levels"));

    // 3. Duplicate ID across departments rejected
    let body_dup = json!({
        "version": version,
        "department": "qa",
        "id": "opus-1", // already in phan-tich
        "member": {
            "display_name": "Opus 1 Duplicate",
            "engine": "claude",
            "levels": ["de"]
        }
    });
    let r3 = srv.post_json("/api/departments/member/save", &body_dup);
    assert_eq!(r3.status, 422);
    let j3 = r3.json();
    assert!(j3["errors"].as_array().unwrap().iter().any(|e| e["field"] == "id"));

    stop(srv);
}

#[test]
fn departments_version_conflict() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);

    let r_init = srv.post_json("/api/departments/init-example", &json!({}));
    assert_eq!(r_init.status, 200);

    let body_conflict = json!({
        "version": "bad-version",
        "department": "ke-hoach",
        "id": "lead-test",
        "member": {
            "display_name": "Lead Test",
            "engine": "claude",
            "levels": ["vua"]
        }
    });
    let r = srv.post_json("/api/departments/member/save", &body_conflict);
    assert_eq!(r.status, 409);
    assert_eq!(r.json()["code"], "conflict");

    stop(srv);
}

#[test]
fn departments_delete() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);

    let r_init = srv.post_json("/api/departments/init-example", &json!({}));
    assert_eq!(r_init.status, 200);
    let version = r_init.json()["version"].as_str().unwrap().to_string();

    // 1. Delete existing member (opus-1 in phan-tich)
    let del_body = json!({
        "version": version,
        "id": "opus-1"
    });
    let r_del = srv.post_json("/api/departments/member/delete", &del_body);
    assert_eq!(r_del.status, 200);
    let j_del = r_del.json();
    assert_eq!(j_del["ok"], true);

    let pt = find_dept(&j_del, "phan-tich");
    assert!(find_member(pt, "opus-1").is_none());

    // 2. Delete non-existent member
    let v2 = j_del["version"].as_str().unwrap().to_string();
    let r_del_non = srv.post_json(
        "/api/departments/member/delete",
        &json!({ "version": v2, "id": "nonexistent-id" }),
    );
    assert_eq!(r_del_non.status, 422);

    // 3. Delete with wrong version -> 409
    let r_del_bad_ver = srv.post_json(
        "/api/departments/member/delete",
        &json!({ "version": "wrong-version", "id": "opus-2" }),
    );
    assert_eq!(r_del_bad_ver.status, 409);

    stop(srv);
}

#[test]
fn departments_models_native_and_agy() {
    let sb = Sandbox::new();

    // Create fake agy executable
    let fake_agy = sb.root.join("fake_agy.sh");
    std::fs::write(
        &fake_agy,
        "#!/bin/sh\nprintf 'gemini-2.5-pro\tGemini 2.5 Pro\tGoogle\ngemini-2.5-flash\tGemini 2.5 Flash\tGoogle\n'\n",
    )
    .unwrap();
    let mut perms = std::fs::metadata(&fake_agy).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&fake_agy, perms).unwrap();

    sb.write_config(json!({
        "engines": [
            {
                "id": "agy",
                "kind": "agy",
                "command": fake_agy.to_string_lossy()
            }
        ]
    }));

    let srv = sb.serve(&[]);

    // 1. Claude native models
    let r_claude = srv.get("/api/departments/models?engine=claude");
    assert_eq!(r_claude.status, 200);
    let j_claude = r_claude.json();
    let models_claude: Vec<&str> = j_claude["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(models_claude.contains(&"claude-opus-5-5"));
    assert!(models_claude.contains(&"claude-sonnet-5-5"));
    assert!(models_claude.contains(&"claude-haiku-5-5"));

    // 2. Agy models via fake executable
    let r_agy = srv.get("/api/departments/models?engine=agy");
    assert_eq!(r_agy.status, 200);
    let j_agy = r_agy.json();
    let models_agy: Vec<&str> = j_agy["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(models_agy, vec!["gemini-2.5-pro", "gemini-2.5-flash"]);

    stop(srv);
}

fn setup_project_and_task(sb: &Sandbox, task_slug: &str, level: &str) -> (i64, i64) {
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    entry["name_vi"] = serde_json::json!("Web bán hàng");
    entry["aliases"] = serde_json::json!(["shop"]);
    sb.write_map(serde_json::json!([entry]), &[]);

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = sb.read_map();
    let projs = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projs).expect("sync_projects");
    let p_id = agent_brain::db::project_row_id(&conn, "web-ban-hang").expect("project_row_id");
    let t_id = agent_brain::db::upsert_task(
        &conn,
        p_id,
        task_slug,
        "Test Task",
        &format!("task/{task_slug}"),
        Some("main"),
        Some("1234567"),
    )
    .expect("upsert_task");
    agent_brain::db::set_task_level(&conn, t_id, level).expect("set_task_level");
    (p_id, t_id)
}

fn setup_proposal(sb: &Sandbox, p_id: i64, level: &str) -> i64 {
    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let intake_id = agent_brain::db::insert_intake(&conn, "ui", Some(p_id), "Yêu cầu", "normal").expect("insert_intake");
    let new_prop = agent_brain::db::NewProposal {
        intake_id,
        project_id: p_id,
        title: "Đề xuất test",
        request: "Yêu cầu test",
        priority: "normal",
        team: "phong-ban",
        workflow: "mac-dinh",
        reason: Some("Lý do test"),
        level,
    };
    agent_brain::db::insert_proposal(&conn, &new_prop).expect("insert_proposal")
}

fn setup_run(sb: &Sandbox, p_id: i64, t_id: i64, role: &str, member_id: Option<&str>, status: &str) -> i64 {
    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let nr = agent_brain::db::NewRun {
        task_id: Some(t_id),
        project_id: p_id,
        role: role.to_string(),
        profile: "work".to_string(),
        model: None,
        pid: 1234,
        pgid: 1234,
        pid_start: 1000,
        base_commit: None,
        run_dir: None,
    };
    let run_id = agent_brain::db::insert_run(&conn, &nr).expect("insert_run");
    conn.execute(
        "UPDATE runs SET status = ?1, member_id = ?2 WHERE id = ?3",
        rusqlite::params![status, member_id, run_id],
    )
    .expect("update run status and member_id");
    run_id
}

#[test]
fn departments_level_task_and_proposal() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);

    let (p_id, t_id) = setup_project_and_task(&sb, "task-level-test", "de");
    let prop_id = setup_proposal(&sb, p_id, "de");

    // 1. Proposal level: valid
    let r_p1 = srv.post_json(
        &format!("/api/proposals/{prop_id}/level"),
        &json!({ "level": "kho" }),
    );
    assert_eq!(r_p1.status, 200);
    assert_eq!(r_p1.json()["level"], "kho");

    // 2. Proposal level: invalid -> 400
    let r_p_bad = srv.post_json(
        &format!("/api/proposals/{prop_id}/level"),
        &json!({ "level": "sieu-kho" }),
    );
    assert_eq!(r_p_bad.status, 400);

    // 3. Task level: valid
    let r_t1 = srv.post_json(
        &format!("/api/tasks/{t_id}/level"),
        &json!({ "level": "vua" }),
    );
    assert_eq!(r_t1.status, 200);
    assert_eq!(r_t1.json()["level"], "vua");

    // 4. Task level: invalid -> 400
    let r_t_bad = srv.post_json(
        &format!("/api/tasks/{t_id}/level"),
        &json!({ "level": "bad-level" }),
    );
    assert_eq!(r_t_bad.status, 400);

    // 5. Task with running run -> refused (409)
    let _running_run = setup_run(&sb, p_id, t_id, "dev", Some("opus-1"), "running");
    let r_t_running = srv.post_json(
        &format!("/api/tasks/{t_id}/level"),
        &json!({ "level": "kho" }),
    );
    assert_eq!(r_t_running.status, 409);
    assert_eq!(
        r_t_running.json()["error"],
        "Không thể đổi độ khó khi task đang có tiến trình chạy"
    );

    stop(srv);
}

#[test]
fn departments_run_score() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);

    let (p_id, t_id) = setup_project_and_task(&sb, "score-test", "vua");

    // 1. Run without member_id -> 400
    let run_no_member = setup_run(&sb, p_id, t_id, "dev", None, "succeeded");
    let r_no_m = srv.post_json(
        &format!("/api/runs/{run_no_member}/score"),
        &json!({ "score": 5 }),
    );
    assert_eq!(r_no_m.status, 400);

    // 2. Run currently running -> 400
    let run_running = setup_run(&sb, p_id, t_id, "dev", Some("opus-1"), "running");
    let r_running = srv.post_json(
        &format!("/api/runs/{run_running}/score"),
        &json!({ "score": 5 }),
    );
    assert_eq!(r_running.status, 400);

    // 3. Finished run with member_id -> 200
    let run_ok = setup_run(&sb, p_id, t_id, "dev", Some("opus-1"), "succeeded");
    let r_score1 = srv.post_json(
        &format!("/api/runs/{run_ok}/score"),
        &json!({ "score": 5, "note": "Rất tốt" }),
    );
    assert_eq!(r_score1.status, 200);
    let j1 = r_score1.json();
    assert_eq!(j1["ok"], true);
    assert_eq!(j1["avg_score"], 5.0);
    assert_eq!(j1["score_count"], 1);

    // 4. Upsert same run -> updates score & avg
    let r_score2 = srv.post_json(
        &format!("/api/runs/{run_ok}/score"),
        &json!({ "score": 3, "note": "Sửa lại điểm" }),
    );
    assert_eq!(r_score2.status, 200);
    let j2 = r_score2.json();
    assert_eq!(j2["avg_score"], 3.0);
    assert_eq!(j2["score_count"], 1);

    // 5. Score out of range (0 or 6) -> 400
    let r_bad_score = srv.post_json(
        &format!("/api/runs/{run_ok}/score"),
        &json!({ "score": 6 }),
    );
    assert_eq!(r_bad_score.status, 400);

    stop(srv);
}

#[test]
fn departments_get_task_details() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);

    // Init example so opus-1 exists in departments.json
    let r_init = srv.post_json("/api/departments/init-example", &json!({}));
    assert_eq!(r_init.status, 200);

    let (p_id, t_id) = setup_project_and_task(&sb, "task-details-test", "kho");
    let run_id = setup_run(&sb, p_id, t_id, "dev", Some("opus-1"), "succeeded");

    // Score the run
    let r_score = srv.post_json(
        &format!("/api/runs/{run_id}/score"),
        &json!({ "score": 5, "note": "Làm xuất sắc" }),
    );
    assert_eq!(r_score.status, 200);

    // GET /api/tasks/{id}
    let r_task = srv.get(&format!("/api/tasks/{t_id}"));
    assert_eq!(r_task.status, 200);
    let j = r_task.json();
    assert_eq!(j["level"], "kho");

    let runs = j["runs"].as_array().expect("runs array");
    assert_eq!(runs.len(), 1);
    let r0 = &runs[0];
    assert_eq!(r0["id"], run_id);
    assert_eq!(r0["member_id"], "opus-1");
    assert_eq!(r0["member_name"], "Senior Opus 1");
    assert_eq!(r0["score"], 5);
    assert_eq!(r0["score_note"], "Làm xuất sắc");

    stop(srv);
}

#[test]
fn departments_payload_includes_skills_rules_and_preview() {
    let sb = Sandbox::new();

    // 1. Set up a department skill on disk
    let skill_dir = sb
        .xdg_config
        .join("agent-brain")
        .join("departments")
        .join("thi-cong")
        .join("skills")
        .join("dev-playbook");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: Dev Playbook\ndescription: Quy trình thi công chuẩn\n---\nNội dung playbook thi công.",
    )
    .unwrap();

    let srv = sb.serve(&[]);

    // 2. Initial init example
    let r_init = srv.post_json("/api/departments/init-example", &json!({}));
    assert_eq!(r_init.status, 200);
    let v1 = r_init.json()["version"].as_str().unwrap().to_string();

    // 3. GET /api/departments returns skills and roles_meta
    let r_depts = srv.get("/api/departments");
    assert_eq!(r_depts.status, 200);
    let j_depts = r_depts.json();
    let tc = find_dept(&j_depts, "thi-cong");

    let skills = tc["skills"].as_array().expect("skills array");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0]["name"], "Dev Playbook");
    assert_eq!(skills[0]["description"], "Quy trình thi công chuẩn");

    let roles_meta = tc["roles_meta"].as_array().expect("roles_meta array");
    let dev_meta = roles_meta.iter().find(|rm| rm["role"] == "dev").unwrap();
    assert_eq!(dev_meta["source"], "builtin");

    // 4. Save member with rules
    let body_save = json!({
        "version": v1,
        "department": "thi-cong",
        "id": "senior-dev-1",
        "old_department": "thi-cong",
        "member": {
            "display_name": "Senior Dev One",
            "engine": "claude",
            "model": "claude-sonnet-5-5",
            "levels": ["vua", "kho"],
            "notes": "Dev cứng",
            "rules": "Flash: không refactor ngoài phạm vi, chỉ sửa đúng file trong plan"
        }
    });
    let r_save = srv.post_json("/api/departments/member/save", &body_save);
    assert_eq!(r_save.status, 200, "{}", r_save.text());
    let j_save = r_save.json();
    let m = find_member(find_dept(&j_save, "thi-cong"), "senior-dev-1").unwrap();
    assert_eq!(m["rules"], "Flash: không refactor ngoài phạm vi, chỉ sửa đúng file trong plan");
    let v2 = j_save["version"].as_str().unwrap().to_string();

    // 5. Save with rules > 1000 chars -> 422
    let long_rules = "a".repeat(1001);
    let body_bad_rules = json!({
        "version": v2,
        "department": "thi-cong",
        "id": "senior-dev-1",
        "old_department": "thi-cong",
        "member": {
            "display_name": "Senior Dev One",
            "engine": "claude",
            "levels": ["vua"],
            "rules": long_rules
        }
    });
    let r_bad = srv.post_json("/api/departments/member/save", &body_bad_rules);
    assert_eq!(r_bad.status, 422);
    let j_bad = r_bad.json();
    assert!(j_bad["errors"].as_array().unwrap().iter().any(|e| e["field"] == "rules"));

    // 6. GET /api/prompts/preview?role=dev&member=senior-dev-1
    let r_prev = srv.get("/api/prompts/preview?role=dev&member=senior-dev-1");
    assert_eq!(r_prev.status, 200);
    let j_prev = r_prev.json();
    let prompt_text = j_prev["prompt"].as_str().expect("prompt text");
    assert!(prompt_text.contains("### Phòng: Phòng thi công"), "must have department layer");
    assert!(prompt_text.contains("## Skill của phòng Phòng thi công"), "must have dept skills");
    assert!(prompt_text.contains("Nội dung playbook thi công"), "must have skill content");
    assert!(prompt_text.contains("## Rules riêng của thành viên Senior Dev One"), "must have member rules header");
    assert!(prompt_text.contains("Flash: không refactor ngoài phạm vi"), "must have member rules body");

    // 7. GET /api/prompts/departments/thi-cong/dev (builtin note)
    let r_note_builtin = srv.get("/api/prompts/departments/thi-cong/dev");
    assert_eq!(r_note_builtin.status, 200);
    let j_note1 = r_note_builtin.json();
    assert_eq!(j_note1["source"], "builtin");
    assert!(j_note1["text"].as_str().unwrap().contains("liệt kê trong plan"));

    // 8. PUT /api/prompts/departments/thi-cong/dev
    let r_put = srv.put_json(
        "/api/prompts/departments/thi-cong/dev",
        &json!({
            "text": "Quy tắc thi công tùy chỉnh riêng của team.",
            "version": "absent"
        }),
    );
    assert_eq!(r_put.status, 200);
    let j_put = r_put.json();
    assert_eq!(j_put["ok"], true);
    assert_eq!(j_put["source"], "user");

    // Check /api/departments now shows source: "user"
    let r_depts2 = srv.get("/api/departments");
    let j_depts2 = r_depts2.json();
    let tc2 = find_dept(&j_depts2, "thi-cong");
    let rm_dev2 = tc2["roles_meta"].as_array().unwrap().iter().find(|rm| rm["role"] == "dev").unwrap();
    assert_eq!(rm_dev2["source"], "user");

    // 9. DELETE /api/prompts/departments/thi-cong/dev (restores builtin)
    let origin = format!("http://127.0.0.1:{}", srv.port);
    let r_del = srv.request(
        "DELETE",
        "/api/prompts/departments/thi-cong/dev",
        &[("Cookie", &srv.cookie), ("Origin", &origin)],
        b"",
    );
    assert_eq!(r_del.status, 200);
    let j_del = r_del.json();
    assert_eq!(j_del["source"], "builtin");
    assert!(j_del["text"].as_str().unwrap().contains("liệt kê trong plan"));

    // Check /api/departments now shows source: "builtin"
    let r_depts3 = srv.get("/api/departments");
    let j_depts3 = r_depts3.json();
    let tc3 = find_dept(&j_depts3, "thi-cong");
    let rm_dev3 = tc3["roles_meta"].as_array().unwrap().iter().find(|rm| rm["role"] == "dev").unwrap();
    assert_eq!(rm_dev3["source"], "builtin");

    // 10. Check GET /api/prompts has departments layer
    let r_prompts = srv.get("/api/prompts");
    assert_eq!(r_prompts.status, 200);
    let j_prompts = r_prompts.json();
    assert!(j_prompts["layers"]["departments"].is_array());

    stop(srv);
}
