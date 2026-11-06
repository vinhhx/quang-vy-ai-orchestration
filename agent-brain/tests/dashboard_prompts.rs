#[allow(clippy::while_let_loop)]
mod common;

use common::Sandbox;

#[test]
fn prompt_api_crud_and_preview() {
    let sb = Sandbox::new();
    let repo_a = sb.make_repo("proj-a", &[], None, None);
    let mut entry_a = sb.project_entry(&repo_a, "proj-a", "work");
    entry_a["name_vi"] = serde_json::json!("Dự án Alpha");
    entry_a["stack"] = serde_json::json!(["video-srt"]);
    sb.write_map(serde_json::json!([entry_a]), &[]);

    let srv = sb.serve(&[]);

    // 1. GET /api/prompts
    let r = srv.get("/api/prompts");
    assert_eq!(r.status, 200);
    let j = r.json();
    let stacks = j["layers"]["stacks"].as_array().expect("stacks array");
    assert!(stacks.iter().any(|v| v == "video-srt"));
    assert!(stacks.iter().any(|v| v == "laravel"));
    assert!(stacks.iter().any(|v| v == "yii2"));
    let builtin_stacks = j["builtin_stacks"].as_array().expect("builtin_stacks");
    assert!(builtin_stacks.iter().any(|v| v == "video-srt"));
    let roles = j["roles"].as_array().expect("roles");
    assert_eq!(roles.len(), 7);
    assert!(roles.iter().any(|v| v == "analyst"));
    assert!(roles.iter().any(|v| v == "analysis-review"));

    // 2. GET /api/prompts/stacks/video-srt/lead returns builtin
    let r = srv.get("/api/prompts/stacks/video-srt/lead");
    assert_eq!(r.status, 200);
    let j = r.json();
    assert_eq!(j["source"], "builtin");
    assert_eq!(j["version"], "absent");
    let text = j["text"].as_str().unwrap();
    assert!(text.contains(".venv/bin/python scripts/"));

    // 3. Validations: invalid layer, invalid name, invalid role -> 400
    let r = srv.get("/api/prompts/invalid_layer/video-srt/lead");
    assert_eq!(r.status, 400);

    let r = srv.get("/api/prompts/stacks/../lead");
    assert_eq!(r.status, 400);

    let r = srv.get("/api/prompts/stacks/video-srt/invalid_role");
    assert_eq!(r.status, 400);

    // 4. PUT > 8192 bytes -> 400
    let large_text = "x".repeat(8193);
    let r = srv.put_json(
        "/api/prompts/stacks/video-srt/lead",
        &serde_json::json!({
            "text": large_text,
            "version": "absent"
        }),
    );
    assert_eq!(r.status, 400);

    // 5. PUT with wrong version -> 409 conflict
    let r = srv.put_json(
        "/api/prompts/stacks/video-srt/lead",
        &serde_json::json!({
            "text": "User custom prompt note",
            "version": "wrong-version"
        }),
    );
    assert_eq!(r.status, 409);
    assert_eq!(r.json()["code"], "conflict");

    // 6. PUT with matching version ("absent") -> 200
    let r = srv.put_json(
        "/api/prompts/stacks/video-srt/lead",
        &serde_json::json!({
            "text": "Custom video-srt lead note",
            "version": "absent"
        }),
    );
    assert_eq!(r.status, 200);
    let put_res = r.json();
    assert_eq!(put_res["ok"], true);
    let new_ver = put_res["version"].as_str().unwrap().to_string();
    assert_ne!(new_ver, "absent");

    // 7. GET now returns user note and version
    let r = srv.get("/api/prompts/stacks/video-srt/lead");
    assert_eq!(r.status, 200);
    let j = r.json();
    assert_eq!(j["source"], "user");
    assert_eq!(j["version"], new_ver);
    assert_eq!(j["text"], "Custom video-srt lead note");

    // 8. PUT with updated text and matching version -> 200
    let r = srv.put_json(
        "/api/prompts/stacks/video-srt/lead",
        &serde_json::json!({
            "text": "Custom video-srt lead note v2",
            "version": new_ver
        }),
    );
    assert_eq!(r.status, 200);
    let v2 = r.json()["version"].as_str().unwrap().to_string();
    assert_ne!(v2, new_ver);

    // 9. DELETE returns fallback to builtin
    let origin = format!("http://127.0.0.1:{}", srv.port);
    let r = srv.request(
        "DELETE",
        "/api/prompts/stacks/video-srt/lead",
        &[
            ("Cookie", &srv.cookie),
            ("Origin", &origin),
        ],
        b"",
    );
    assert_eq!(r.status, 200);
    let del_j = r.json();
    assert_eq!(del_j["ok"], true);
    assert_eq!(del_j["source"], "builtin");
    assert_eq!(del_j["version"], "absent");

    // 10. GET after delete is back to builtin
    let r = srv.get("/api/prompts/stacks/video-srt/lead");
    assert_eq!(r.status, 200);
    let j = r.json();
    assert_eq!(j["source"], "builtin");

    // 11. Disable builtin note by putting empty text
    let r = srv.put_json(
        "/api/prompts/stacks/video-srt/lead",
        &serde_json::json!({
            "text": "",
            "version": "absent"
        }),
    );
    assert_eq!(r.status, 200);
    let empty_ver = r.json()["version"].as_str().unwrap().to_string();

    let r = srv.get("/api/prompts/stacks/video-srt/lead");
    assert_eq!(r.status, 200);
    let j = r.json();
    assert_eq!(j["source"], "user");
    assert_eq!(j["text"], "");
    assert_eq!(j["version"], empty_ver);

    // Clean up
    let _ = srv.request(
        "DELETE",
        "/api/prompts/stacks/video-srt/lead",
        &[
            ("Cookie", &srv.cookie),
            ("Origin", &origin),
        ],
        b"",
    );

    // 12. Preview prompt
    let r = srv.get("/api/prompts/preview?project=proj-a&role=lead&workflow=mac-dinh");
    assert_eq!(r.status, 200);
    let prev = r.json();
    let prompt = prev["prompt"].as_str().unwrap();
    // proj-a has stack video-srt -> should contain stack note in preview
    assert!(prompt.contains("### Stack: video-srt"));
    assert!(prompt.contains(".venv/bin/python"));
}

#[test]
fn builtin_stack_notes_sanity() {
    for (stack, role, content) in agent_brain::prompts::BUILTIN_STACK_NOTES {
        assert!(
            !content.trim().is_empty(),
            "Builtin note for {stack}/{role} must not be empty"
        );
        assert!(
            content.len() <= 1500,
            "Builtin note for {stack}/{role} must be <= 1500 bytes (got {})",
            content.len()
        );
    }

    let video_lead = agent_brain::prompts::builtin_stack_note("video-srt", "lead").unwrap();
    assert!(video_lead.contains(".venv/bin/python"));
}
