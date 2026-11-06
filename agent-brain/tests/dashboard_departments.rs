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
