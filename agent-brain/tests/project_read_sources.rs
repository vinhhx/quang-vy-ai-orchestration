#![allow(clippy::while_let_loop)]

mod common;

use agent_brain::guard::{agy_tool_decision, decide, load_policy, Policy};
use agent_brain::projects::{read_source_paths, read_sources_prompt_block, Project};
use agent_brain::roles::{build_command_with_prompt, check_forbidden_args, Role, RoleSpec};
use common::Sandbox;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn agy_params(pairs: &[(&str, &str)]) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    for (k, v) in pairs {
        map.insert(k.to_string(), serde_json::Value::String(v.to_string()));
    }
    map
}

#[test]
fn test_projects_read_sources_parsing_and_filtering() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();

    let cur_dir = root.join("app-main");
    let valid_src = root.join("lib-source");
    let missing_dir = root.join("lib-missing-dir");
    let nested_dir = cur_dir.join("nested");

    fs::create_dir_all(&cur_dir).unwrap();
    fs::create_dir_all(&valid_src).unwrap();
    fs::create_dir_all(&nested_dir).unwrap();

    let cur_proj = Project {
        id: "app-main".to_string(),
        path: cur_dir.to_str().unwrap().to_string(),
        name_vi: "App Main".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
        stack: vec![],
        read_sources: vec![
            "lib-source".to_string(),
            "app-main".to_string(), // self
            "ghost".to_string(),    // unknown
            "proj-missing".to_string(), // missing = true
            "lib-missing-dir".to_string(), // dir not exist
            "lib-nested".to_string(),      // overlapping
        ],
        protocol_dir: String::new(),
        workspace: Default::default(),
    };

    let map = json!({
        "projects": [
            {
                "id": "app-main",
                "path": cur_dir.to_str().unwrap(),
                "name_vi": "App Main",
                "read_sources": cur_proj.read_sources
            },
            {
                "id": "lib-source",
                "path": valid_src.to_str().unwrap(),
                "name_vi": "Lib Source",
            },
            {
                "id": "proj-missing",
                "path": root.join("missing").to_str().unwrap(),
                "name_vi": "Missing Proj",
                "missing": true
            },
            {
                "id": "lib-missing-dir",
                "path": missing_dir.to_str().unwrap(),
                "name_vi": "Missing Dir",
            },
            {
                "id": "lib-nested",
                "path": nested_dir.to_str().unwrap(),
                "name_vi": "Nested Proj",
            }
        ]
    });

    let (sources, warnings) = read_source_paths(&map, &cur_proj);

    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].0, "lib-source");
    assert_eq!(sources[0].1, valid_src);

    assert!(warnings.iter().any(|w| w.contains("trùng chính nó") && w.contains("app-main")));
    assert!(warnings.iter().any(|w| w.contains("không tồn tại") && w.contains("ghost")));
    assert!(warnings.iter().any(|w| w.contains("missing") && w.contains("proj-missing")));
    assert!(warnings.iter().any(|w| w.contains("thư mục không tồn tại") && w.contains("lib-missing-dir")));
    assert!(warnings.iter().any(|w| w.contains("chồng lấn") && w.contains("lib-nested")));
}

#[test]
fn test_api_projects_read_sources() {
    let sb = Sandbox::new();
    let r_app = sb.repos.join("e2e-app");
    let r_src1 = sb.repos.join("backend-api");
    let r_src2 = sb.repos.join("auth-svc");
    let r_src3 = sb.repos.join("billing-svc");
    let r_src4 = sb.repos.join("mail-svc");
    let r_src5 = sb.repos.join("report-svc");
    let r_src6 = sb.repos.join("extra-svc");

    for p in [&r_app, &r_src1, &r_src2, &r_src3, &r_src4, &r_src5, &r_src6] {
        fs::create_dir_all(p).unwrap();
    }

    sb.write_map(
        json!([
            {
                "id": "e2e-app",
                "path": r_app.to_str().unwrap(),
                "name_vi": "E2E App",
                "read_sources": []
            },
            {
                "id": "backend-api",
                "path": r_src1.to_str().unwrap(),
                "name_vi": "Backend API",
                "read_sources": []
            },
            {
                "id": "auth-svc",
                "path": r_src2.to_str().unwrap(),
                "name_vi": "Auth Service",
                "read_sources": []
            },
            {
                "id": "billing-svc",
                "path": r_src3.to_str().unwrap(),
                "name_vi": "Billing",
                "read_sources": []
            },
            {
                "id": "mail-svc",
                "path": r_src4.to_str().unwrap(),
                "name_vi": "Mail",
                "read_sources": []
            },
            {
                "id": "report-svc",
                "path": r_src5.to_str().unwrap(),
                "name_vi": "Report",
                "read_sources": []
            },
            {
                "id": "extra-svc",
                "path": r_src6.to_str().unwrap(),
                "name_vi": "Extra",
                "read_sources": []
            }
        ]),
        &[],
    );

    let srv = sb.serve(&[]);

    // 1. GET /api/projects includes read_sources
    let resp = srv.get("/api/projects");
    assert_eq!(resp.status, 200);
    let list = resp.json();
    let app_obj = list["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "e2e-app")
        .expect("find e2e-app");
    assert_eq!(app_obj["read_sources"], json!([]));

    // 2. Valid save
    let resp = srv.post_json(
        "/api/projects/e2e-app/read-sources",
        &json!({ "read_sources": ["backend-api", "auth-svc"] }),
    );
    assert_eq!(resp.status, 200, "resp: {}", resp.text());
    let body = resp.json();
    let updated_app = body["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "e2e-app")
        .expect("find e2e-app in resp");
    assert_eq!(updated_app["read_sources"], json!(["backend-api", "auth-svc"]));

    // Verify persisted via GET
    let resp = srv.get("/api/projects");
    let list = resp.json();
    let app_obj = list["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "e2e-app")
        .unwrap();
    assert_eq!(app_obj["read_sources"], json!(["backend-api", "auth-svc"]));

    // 3. Reject self id
    let resp = srv.post_json(
        "/api/projects/e2e-app/read-sources",
        &json!({ "read_sources": ["e2e-app"] }),
    );
    assert_eq!(resp.status, 422);
    let body = resp.json();
    assert_eq!(body["errors"][0]["field"], "read_sources");
    assert!(body["errors"][0]["message"].as_str().unwrap().contains("chính dự án"));

    // 4. Reject unknown id
    let resp = srv.post_json(
        "/api/projects/e2e-app/read-sources",
        &json!({ "read_sources": ["unknown-id"] }),
    );
    assert_eq!(resp.status, 422);
    let body = resp.json();
    assert_eq!(body["errors"][0]["field"], "read_sources");
    assert!(body["errors"][0]["message"].as_str().unwrap().contains("không tồn tại"));

    // 5. Reject duplicate id
    let resp = srv.post_json(
        "/api/projects/e2e-app/read-sources",
        &json!({ "read_sources": ["backend-api", "backend-api"] }),
    );
    assert_eq!(resp.status, 422);
    let body = resp.json();
    assert_eq!(body["errors"][0]["field"], "read_sources");
    assert!(body["errors"][0]["message"].as_str().unwrap().contains("trùng lặp"));

    // 6. Reject > 5 sources
    let resp = srv.post_json(
        "/api/projects/e2e-app/read-sources",
        &json!({
            "read_sources": [
                "backend-api",
                "auth-svc",
                "billing-svc",
                "mail-svc",
                "report-svc",
                "extra-svc"
            ]
        }),
    );
    assert_eq!(resp.status, 422);
    let body = resp.json();
    assert_eq!(body["errors"][0]["field"], "read_sources");
    assert!(body["errors"][0]["message"].as_str().unwrap().contains("Tối đa 5 nguồn"));

    // 7. Version conflict check
    let payload = serde_json::to_vec(&json!({ "read_sources": ["backend-api"] })).unwrap();
    let origin = format!("http://127.0.0.1:{}", srv.port);
    let resp = srv.request(
        "POST",
        "/api/projects/e2e-app/read-sources",
        &[
            ("Cookie", &srv.cookie),
            ("Origin", &origin),
            ("If-Match", "\"invalid-etag\""),
            ("Content-Type", "application/json"),
        ],
        &payload,
    );
    assert_eq!(resp.status, 409);
}

#[test]
fn test_claude_argv_and_tools_for_task_with_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let src_dir = tmp.path().join("src-proj").canonicalize().unwrap_or_else(|_| tmp.path().join("src-proj"));
    fs::create_dir_all(&src_dir).unwrap();

    let cfg = agent_brain::config::config_from_value(
        &serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap(),
    )
    .unwrap();

    let spec = RoleSpec {
        role: Role::Dev,
        user_input: "Dev task prompt".to_string(),
        allowed_tools: vec![],
        disallowed_tools: vec![],
        extra_args: vec![],
        source_paths: vec![src_dir.clone()],
    };

    let cmd = build_command_with_prompt(&spec, &cfg, "Dev prompt").unwrap();

    // Verify forbidden args pass
    check_forbidden_args(&cmd).expect("check_forbidden_args must pass");

    // Must have --add-dir <src>
    let add_dir_idx = cmd.iter().position(|arg| arg == "--add-dir").expect("has --add-dir");
    assert_eq!(cmd[add_dir_idx + 1], src_dir.to_string_lossy().to_string());

    // Allowed tools must have Read(//<P>/**)
    let allowed_idx = cmd.iter().position(|arg| arg == "--allowedTools").expect("has --allowedTools");
    let allowed_tools: Vec<_> = cmd[allowed_idx + 1..]
        .iter()
        .take_while(|a| !a.starts_with("--"))
        .collect();
    let expected_read = format!("Read(/{})", src_dir.join("**").display());
    assert!(
        allowed_tools.contains(&&expected_read),
        "missing {expected_read} in {:?}",
        allowed_tools
    );

    // Disallowed tools must have .env*, .git, Edit, Write
    let disallowed_idx = cmd.iter().position(|arg| arg == "--disallowedTools").expect("has --disallowedTools");
    let disallowed_tools: Vec<_> = cmd[disallowed_idx + 1..]
        .iter()
        .take_while(|a| !a.starts_with("--"))
        .collect();

    let exp_env_glob = format!("Read(/{})", src_dir.join("**/.env*").display());
    let exp_env_root = format!("Read(/{})", src_dir.join(".env*").display());
    let exp_git = format!("Read(/{})", src_dir.join(".git/**").display());
    let exp_edit = format!("Edit(/{})", src_dir.join("**").display());
    let exp_write = format!("Write(/{})", src_dir.join("**").display());

    assert!(disallowed_tools.contains(&&exp_env_glob));
    assert!(disallowed_tools.contains(&&exp_env_root));
    assert!(disallowed_tools.contains(&&exp_git));
    assert!(disallowed_tools.contains(&&exp_edit));
    assert!(disallowed_tools.contains(&&exp_write));
}

#[test]
fn test_guard_decisions_for_source_project() {
    let td = tempfile::tempdir().unwrap();
    let root = td.path().join("main-proj").canonicalize().unwrap_or_else(|_| td.path().join("main-proj"));
    let src_p = td.path().join("source-proj").canonicalize().unwrap_or_else(|_| td.path().join("source-proj"));
    let outside = td.path().join("outside-proj").canonicalize().unwrap_or_else(|_| td.path().join("outside-proj"));

    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(src_p.join("src")).unwrap();
    fs::create_dir_all(src_p.join("sub")).unwrap();
    fs::create_dir_all(&outside).unwrap();

    let safe_php = src_p.join("src/a.php");
    fs::write(&safe_php, "<?php echo 'ok';").unwrap();

    let env_file = src_p.join(".env");
    fs::write(&env_file, "KEY=dummy_val").unwrap();

    let env_local = src_p.join("sub/.env.local");
    fs::write(&env_local, "KEY=dummy_local").unwrap();

    let outside_file = outside.join("secret.txt");
    fs::write(&outside_file, "secret text").unwrap();

    // Symlink in source_dir pointing outside
    let symlink_path = src_p.join("symlink_out.txt");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_file, &symlink_path).unwrap();

    let pol = Policy {
        root: root.clone(),
        read_allow: Some(vec![root.join(".plan.md")]),
        write_allow: vec![root.join(".plan.md"), safe_php.clone()],
        log: None,
        read_dirs: vec![],
        write_dirs: vec![],
        read_files: vec![],
        write_files: vec![],
        bash_allow: None,
        source_roots: vec![src_p.clone()],
    };

    // 1. decide: read P/src/a.php -> allow: true, reason: "in read-only source project"
    let dec = decide(
        &pol,
        &json!({
            "tool_name": "Read",
            "tool_input": { "file_path": safe_php.to_str().unwrap() },
            "cwd": root.to_str().unwrap()
        }),
    );
    assert!(dec.allow, "decide must allow reading source php: {:?}", dec);
    assert_eq!(dec.reason, "in read-only source project");

    // 2. decide: read P/.env and P/sub/.env.local -> deny
    let dec_env = decide(
        &pol,
        &json!({
            "tool_name": "Read",
            "tool_input": { "file_path": env_file.to_str().unwrap() },
            "cwd": root.to_str().unwrap()
        }),
    );
    assert!(!dec_env.allow, "decide must deny reading .env: {:?}", dec_env);

    let dec_env_local = decide(
        &pol,
        &json!({
            "tool_name": "Read",
            "tool_input": { "file_path": env_local.to_str().unwrap() },
            "cwd": root.to_str().unwrap()
        }),
    );
    assert!(!dec_env_local.allow, "decide must deny reading sub/.env.local: {:?}", dec_env_local);

    // 3. decide: write P/src/a.php -> deny, reason: "thuộc dự án nguồn chỉ đọc"
    let dec_write = decide(
        &pol,
        &json!({
            "tool_name": "Write",
            "tool_input": { "file_path": safe_php.to_str().unwrap() },
            "cwd": root.to_str().unwrap()
        }),
    );
    assert!(!dec_write.allow, "decide must deny writing source file even if in write_allow");
    assert_eq!(dec_write.reason, "thuộc dự án nguồn chỉ đọc");

    // 4. decide: symlink in P pointing outside -> deny
    #[cfg(unix)]
    {
        let dec_symlink = decide(
            &pol,
            &json!({
                "tool_name": "Read",
                "tool_input": { "file_path": symlink_path.to_str().unwrap() },
                "cwd": root.to_str().unwrap()
            }),
        );
        assert!(!dec_symlink.allow, "decide must deny symlink pointing outside source: {:?}", dec_symlink);
    }

    // 5. Grep under P -> allow; Grep under root when read_allow is set -> deny
    let dec_grep_src = decide(
        &pol,
        &json!({
            "tool_name": "Grep",
            "tool_input": { "path": src_p.join("src").to_str().unwrap() },
            "cwd": root.to_str().unwrap()
        }),
    );
    assert!(dec_grep_src.allow, "decide must allow grep under source: {:?}", dec_grep_src);

    let dec_grep_root = decide(
        &pol,
        &json!({
            "tool_name": "Grep",
            "tool_input": { "path": root.to_str().unwrap() },
            "cwd": root.to_str().unwrap()
        }),
    );
    assert!(!dec_grep_root.allow, "decide must deny grep under root when read_allow is set");

    // 6. agy tools: read_file, list_dir, write_file same decisions
    let agy_read = agy_tool_decision(
        &pol,
        "read_file",
        &agy_params(&[("Path", safe_php.to_str().unwrap())]),
    ).unwrap();
    assert!(agy_read.allow, "agy read_file on safe_php: {:?}", agy_read);

    let agy_read_env = agy_tool_decision(
        &pol,
        "read_file",
        &agy_params(&[("Path", env_file.to_str().unwrap())]),
    ).unwrap();
    assert!(!agy_read_env.allow, "agy read_file on .env: {:?}", agy_read_env);

    let agy_write = agy_tool_decision(
        &pol,
        "write_file",
        &agy_params(&[("Path", safe_php.to_str().unwrap())]),
    ).unwrap();
    assert!(!agy_write.allow, "agy write_file on source: {:?}", agy_write);
    assert_eq!(agy_write.reason, "thuộc dự án nguồn chỉ đọc");

    let agy_list_src = agy_tool_decision(
        &pol,
        "list_dir",
        &agy_params(&[("DirectoryPath", src_p.join("src").to_str().unwrap())]),
    ).unwrap();
    assert!(agy_list_src.allow, "agy list_dir on source src: {:?}", agy_list_src);

    #[cfg(unix)]
    {
        let agy_list_symlink = agy_tool_decision(
            &pol,
            "list_dir",
            &agy_params(&[("DirectoryPath", symlink_path.to_str().unwrap())]),
        ).unwrap();
        assert!(!agy_list_symlink.allow, "agy list_dir on symlink pointing outside: {:?}", agy_list_symlink);
    }
}

#[test]
fn test_policy_load_backward_compatibility() {
    let td = tempfile::tempdir().unwrap();
    let root = td.path().join("proj");
    fs::create_dir_all(&root).unwrap();

    let policy_path = td.path().join("policy.json");
    let legacy_policy = json!({
        "root": root.to_str().unwrap(),
        "read_allow": null,
        "write_allow": [],
        "log": null
    });
    fs::write(&policy_path, legacy_policy.to_string()).unwrap();

    let pol = load_policy(&policy_path).expect("legacy policy must load");
    assert_eq!(pol.source_roots, Vec::<PathBuf>::new());
}

#[test]
fn test_prompt_block_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();

    let app_dir = root.join("app");
    let src_dir = root.join("src-lib");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(src_dir.join(".gsd")).unwrap();

    let p_with_src = Project {
        id: "app".to_string(),
        path: app_dir.to_str().unwrap().to_string(),
        name_vi: "App".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
        stack: vec![],
        read_sources: vec!["src-lib".to_string()],
        protocol_dir: String::new(),
        workspace: Default::default(),
    };

    let p_without_src = Project {
        id: "app-standalone".to_string(),
        path: app_dir.to_str().unwrap().to_string(),
        name_vi: "App Standalone".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
        stack: vec![],
        read_sources: vec![],
        protocol_dir: String::new(),
        workspace: Default::default(),
    };

    let map = json!({
        "projects": [
            {
                "id": "app",
                "path": app_dir.to_str().unwrap(),
                "name_vi": "App",
                "read_sources": ["src-lib"]
            },
            {
                "id": "src-lib",
                "path": src_dir.to_str().unwrap(),
                "name_vi": "Src Lib",
            }
        ]
    });

    let (block_with, _) = read_sources_prompt_block(&map, &p_with_src);
    assert!(block_with.contains("## Dự án nguồn (chỉ đọc)"));
    assert!(block_with.contains("- src-lib:"));
    assert!(block_with.contains("— thư mục tri thức: .gsd"));
    assert!(block_with.contains(
        "Mọi vai của task (Lead, Dev, QA, Review) đều đọc được các thư mục này bằng đường dẫn tuyệt đối."
    ));
    assert!(block_with.contains("Đọc bằng đường dẫn tuyệt đối. Không ghi, không chạy lệnh trong các thư mục này, không đọc file .env*."));

    let (block_without, _) = read_sources_prompt_block(&map, &p_without_src);
    assert_eq!(block_without, "");
}

#[test]
fn test_preview_api_and_cli_shows_read_sources() {
    let sb = Sandbox::new();
    let r_app = sb.repos.join("preview-app");
    let r_src = sb.repos.join("preview-src");
    fs::create_dir_all(&r_app).unwrap();
    fs::create_dir_all(&r_src).unwrap();

    sb.write_map(
        json!([
            {
                "id": "preview-app",
                "path": r_app.to_str().unwrap(),
                "name_vi": "Preview App",
                "read_sources": ["preview-src"]
            },
            {
                "id": "preview-src",
                "path": r_src.to_str().unwrap(),
                "name_vi": "Preview Source",
                "read_sources": []
            }
        ]),
        &[],
    );

    // 1. Test CLI 'agent-brain projects'
    let out = sb.run(&["projects"], None);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert!(out.stdout.contains("[nguồn: preview-src]"), "stdout: {}", out.stdout);

    // 2. Test GET /api/prompts/preview
    let srv = sb.serve(&[]);
    let resp = srv.get("/api/prompts/preview?project=preview-app&role=lead");
    assert_eq!(resp.status, 200, "resp: {}", resp.text());
    let body = resp.json();
    let preview_text = body["prompt"].as_str().unwrap();
    assert!(preview_text.contains("## Dự án nguồn (chỉ đọc)"));
    assert!(preview_text.contains("preview-src:"));
}

