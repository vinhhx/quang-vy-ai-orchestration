use std::path::Path;

fn init_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let nohooks = dir.join("nohooks");
    std::fs::create_dir_all(&nohooks).unwrap();
    let global_cfg = dir.join("gitconfig");
    std::fs::write(&global_cfg, "").unwrap();

    let run_git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &global_cfg)
            .status()
            .expect("git command failed to run");
        assert!(status.success(), "git {:?} failed", args);
    };

    run_git(&["init", "-b", "main"]);
    run_git(&["config", "user.name", "Sandbox User"]);
    run_git(&["config", "user.email", "sandbox@example.com"]);
    run_git(&["config", "commit.gpgsign", "false"]);
    run_git(&["config", "core.hooksPath", nohooks.to_str().unwrap()]);
    std::fs::write(dir.join("init.txt"), "hello").unwrap();
    run_git(&["add", "init.txt"]);
    run_git(&["commit", "-m", "init"]);
}

#[test]
fn config_default_and_validation() {
    let default_val: serde_json::Value =
        serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap();
    let cfg = agent_brain::config::config_from_value(&default_val).expect("default config valid");
    assert_eq!(cfg.claude_bin, "claude");
    assert_eq!(cfg.model_for("lead").unwrap(), "opus");
    assert_eq!(cfg.model_for("dev").unwrap(), "sonnet");
    assert_eq!(cfg.inspect_model, "sonnet");
    assert_eq!(cfg.kill_grace_secs, 3);
    assert_eq!(cfg.inspect_max_diff_bytes, 100000);
    assert_eq!(cfg.common_args, vec!["--strict-mcp-config"]);
    assert_eq!(
        cfg.timeout_for("lead"),
        std::time::Duration::from_secs(1800)
    );
    assert_eq!(cfg.timeout_for("qa"), std::time::Duration::from_secs(1200));
    assert_eq!(
        cfg.timeout_for("inspect"),
        std::time::Duration::from_secs(300)
    );

    // missing models
    let mut no_models = default_val.clone();
    no_models.as_object_mut().unwrap().remove("models");
    let err = agent_brain::config::config_from_value(&no_models).unwrap_err();
    assert_eq!(err.code, 1);
    assert_eq!(err.message, "Cấu hình thiếu trường 'models'");

    // models without dev
    let mut no_dev = default_val.clone();
    no_dev
        .as_object_mut()
        .unwrap()
        .get_mut("models")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("dev");
    let err = agent_brain::config::config_from_value(&no_dev).unwrap_err();
    assert_eq!(err.code, 1);
    assert_eq!(err.message, "Cấu hình thiếu model cho vai trò 'dev'");

    // profiles without personal
    let mut no_personal = default_val.clone();
    no_personal
        .as_object_mut()
        .unwrap()
        .get_mut("profiles")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("personal");
    let err = agent_brain::config::config_from_value(&no_personal).unwrap_err();
    assert_eq!(err.code, 1);
    assert_eq!(err.message, "Cấu hình thiếu profile 'personal'");

    // JSON array
    let arr_val = serde_json::json!([1, 2, 3]);
    let err = agent_brain::config::config_from_value(&arr_val).unwrap_err();
    assert_eq!(err.code, 1);
    assert_eq!(
        err.message,
        "Cấu hình không hợp lệ (phải là JSON object)"
    );

    // expand_tilde
    let h = Path::new("/h");
    assert_eq!(
        agent_brain::config::expand_tilde("~/x", h),
        std::path::PathBuf::from("/h/x")
    );
    assert_eq!(
        agent_brain::config::expand_tilde("/abs", h),
        std::path::PathBuf::from("/abs")
    );
}

#[test]
fn routing_normalize_slugify_resolve() {
    assert_eq!(
        agent_brain::routing::normalize("Web Bán_Hàng-Đẹp"),
        "web ban hang dep"
    );
    assert_eq!(
        agent_brain::routing::slugify("thêm nút đăng xuất"),
        "them-nut-dang-xuat"
    );
    assert_eq!(
        agent_brain::routing::slugify("aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk"),
        "aaaa-bbbb-cccc-dddd-eeee-ffff-gggg-hhhh-iiii"
    );
    assert_eq!(agent_brain::routing::slugify("!!!"), "");

    let p1 = agent_brain::projects::Project {
        id: "web-ban-hang".to_string(),
        path: "/path/1".to_string(),
        name_vi: "Web bán hàng".to_string(),
        aliases: vec!["shop".to_string()],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    let p2 = agent_brain::projects::Project {
        id: "kho-anh".to_string(),
        path: "/path/2".to_string(),
        name_vi: "Kho ảnh".to_string(),
        aliases: vec!["shop".to_string()],
        kind: "code".to_string(),
        profile: "personal".to_string(),
        repo: None,
        missing: false,
    };
    let projects = vec![p1, p2];

    let r1 =
        agent_brain::routing::resolve("web bán hàng", &projects).expect("resolve web-ban-hang");
    assert_eq!(r1.id, "web-ban-hang");

    let r2 = agent_brain::routing::resolve(" kho-anh ", &projects).expect("resolve kho-anh");
    assert_eq!(r2.id, "kho-anh");

    let err_amb = agent_brain::routing::resolve("shop", &projects).unwrap_err();
    assert_eq!(err_amb.kind, agent_brain::routing::RouteKind::Ambiguous);
    let ids: Vec<&str> = err_amb.candidates.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec!["web-ban-hang", "kho-anh"]);

    let err_none = agent_brain::routing::resolve("không có", &projects).unwrap_err();
    assert_eq!(err_none.kind, agent_brain::routing::RouteKind::None);
    assert_eq!(err_none.candidates.len(), 2);

    let err_blank = agent_brain::routing::resolve("   ", &projects).unwrap_err();
    assert_eq!(err_blank.kind, agent_brain::routing::RouteKind::None);
    assert_eq!(err_blank.candidates.len(), 2);
}

#[test]
fn select_project_fallback_ask_texts() {
    let tmp = tempfile::tempdir().unwrap();

    let map_data = serde_json::json!({
        "scan_roots": [],
        "projects": [
            {
                "id": "web-ban-hang",
                "path": "/path/1",
                "name_vi": "Web bán hàng",
                "aliases": ["shop"],
                "type": "code",
                "profile": "work"
            },
            {
                "id": "kho-anh",
                "path": "/path/2",
                "name_vi": "Kho ảnh",
                "aliases": ["shop"],
                "type": "code",
                "profile": "personal"
            }
        ]
    });

    let err_amb = agent_brain::projects::select_project("shop", &map_data).unwrap_err();
    assert_eq!(err_amb.code, 2);
    assert!(err_amb
        .message
        .starts_with("Tên \"shop\" khớp 2 dự án — dừng, không chạy agent."));
    assert!(err_amb.message.contains("web-ban-hang"));
    assert!(err_amb.message.contains("kho-anh"));

    let err_none = agent_brain::projects::select_project("khong-co", &map_data).unwrap_err();
    assert_eq!(err_none.code, 2);
    assert!(err_none
        .message
        .starts_with("Không tìm thấy dự án \"khong-co\" — dừng, không chạy agent."));

    let empty_map = serde_json::json!({"scan_roots": [], "projects": []});
    let err_empty = agent_brain::projects::select_project("anything", &empty_map).unwrap_err();
    assert_eq!(err_empty.code, 2);
    assert!(err_empty
        .message
        .contains("  (trống) — chạy: agent-brain projects scan --root <thư mục>"));

    // load_map missing file
    let missing_path = tmp.path().join("missing.json");
    let loaded_missing = agent_brain::projects::load_map(&missing_path).unwrap();
    assert_eq!(
        loaded_missing,
        serde_json::json!({"scan_roots": [], "projects": []})
    );

    // load_map bad file
    let bad_file = tmp.path().join("bad.json");
    std::fs::write(&bad_file, "{bad").unwrap();
    let err_bad = agent_brain::projects::load_map(&bad_file).unwrap_err();
    assert_eq!(err_bad.code, 1);
    assert!(err_bad
        .message
        .contains("(agent-brain không ghi đè file này)"));
    assert_eq!(std::fs::read_to_string(&bad_file).unwrap(), "{bad");
}

#[test]
fn require_runnable_checks() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_dir = tmp.path().join("repo");
    init_repo(&repo_dir);

    let default_val: serde_json::Value =
        serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap();
    let cfg = agent_brain::config::config_from_value(&default_val).unwrap();

    // missing true
    let mut p = agent_brain::projects::Project {
        id: "p1".to_string(),
        path: repo_dir.to_str().unwrap().to_string(),
        name_vi: "P1".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: true,
    };
    let err = agent_brain::projects::require_runnable(&p, &cfg).unwrap_err();
    assert_eq!(err.code, 1);
    assert!(err.message.contains("bị đánh dấu missing"));

    // non-existent path
    p.missing = false;
    p.path = tmp.path().join("nope").to_str().unwrap().to_string();
    let err = agent_brain::projects::require_runnable(&p, &cfg).unwrap_err();
    assert_eq!(err.code, 1);
    assert!(err
        .message
        .contains("Đường dẫn dự án 'p1' không tồn tại: "));

    // plain directory
    let plain_dir = tmp.path().join("plain");
    std::fs::create_dir(&plain_dir).unwrap();
    p.path = plain_dir.to_str().unwrap().to_string();
    let err = agent_brain::projects::require_runnable(&p, &cfg).unwrap_err();
    assert_eq!(err.code, 1);
    assert!(err.message.contains("không phải repo git"));

    // empty profile
    p.path = repo_dir.to_str().unwrap().to_string();
    p.profile = "".to_string();
    let err = agent_brain::projects::require_runnable(&p, &cfg).unwrap_err();
    assert_eq!(err.code, 1);
    assert_eq!(
        err.message,
        "Dự án chưa đặt profile (work|personal) — sửa projects_map.json rồi chạy lại"
    );

    // unknown profile
    p.profile = "other".to_string();
    let err = agent_brain::projects::require_runnable(&p, &cfg).unwrap_err();
    assert_eq!(err.code, 1);
    assert_eq!(
        err.message,
        "Profile 'other' không hợp lệ. Các profile đã cấu hình: personal, work"
    );

    // valid profile work
    p.profile = "work".to_string();
    let resolved = agent_brain::projects::require_runnable(&p, &cfg).expect("require_runnable ok");
    assert_eq!(resolved, std::fs::canonicalize(&repo_dir).unwrap());
}

#[test]
fn profile_env_work_and_personal() {
    let default_val: serde_json::Value =
        serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap();
    let cfg = agent_brain::config::config_from_value(&default_val).unwrap();

    let mut base_env = std::collections::BTreeMap::new();
    base_env.insert("HOME".to_string(), "/h".to_string());
    base_env.insert("PATH".to_string(), "/usr/bin".to_string());
    base_env.insert("CLAUDECODE".to_string(), "1".to_string());
    base_env.insert("CLAUDE_CODE_ENTRYPOINT".to_string(), "cli".to_string());
    base_env.insert("ANTHROPIC_API_KEY".to_string(), "k".to_string());
    base_env.insert("CLAUDE_CONFIG_DIR".to_string(), "/old".to_string());

    let work_env = agent_brain::profile::build_env("work", &cfg.profiles, &base_env).unwrap();
    let mut expected_work_keys: Vec<String> = vec![
        "CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR".to_string(),
        "HOME".to_string(),
        "PATH".to_string(),
    ];
    let mut work_keys: Vec<String> = work_env.keys().cloned().collect();
    work_keys.sort();
    expected_work_keys.sort();
    assert_eq!(work_keys, expected_work_keys);
    assert_eq!(
        work_env
            .get("CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR")
            .unwrap(),
        "1"
    );

    let personal_env =
        agent_brain::profile::build_env("personal", &cfg.profiles, &base_env).unwrap();
    assert_eq!(
        personal_env.get("CLAUDE_CONFIG_DIR").unwrap(),
        "/h/.claude-orca"
    );
    assert!(!personal_env.contains_key("ANTHROPIC_API_KEY"));

    assert_eq!(agent_brain::profile::NESTED_SESSION_VARS.len(), 11);

    assert_eq!(
        agent_brain::profile::describe_profile("work", &cfg.profiles),
        "CLAUDE_CONFIG_DIR không đặt → mặc định ~/.claude"
    );

    let err = agent_brain::profile::check_profile("", &cfg.profiles).unwrap_err();
    assert_eq!(err.code, 1);
}

#[test]
fn gitops_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_dir = tmp.path().join("repo");
    init_repo(&repo_dir);

    assert!(agent_brain::gitops::is_repo_root(&repo_dir));
    let sub = repo_dir.join("sub");
    std::fs::create_dir(&sub).unwrap();
    assert!(!agent_brain::gitops::is_repo_root(&sub));

    assert_eq!(
        agent_brain::gitops::current_branch(&repo_dir),
        Some("main".to_string())
    );
    let sha = agent_brain::gitops::head_sha(&repo_dir).unwrap();
    assert_eq!(sha.len(), 40);

    let changed = agent_brain::gitops::changed_paths(&repo_dir).unwrap();
    assert!(changed.is_empty());

    let new_txt = repo_dir.join("new.txt");
    std::fs::write(&new_txt, "content").unwrap();
    let changed_after = agent_brain::gitops::changed_paths(&repo_dir).unwrap();
    assert_eq!(
        changed_after,
        ["new.txt".to_string()].into_iter().collect()
    );

    agent_brain::gitops::create_branch(&repo_dir, "task/x").unwrap();
    assert_eq!(
        agent_brain::gitops::current_branch(&repo_dir),
        Some("task/x".to_string())
    );
    assert!(agent_brain::gitops::branch_exists(&repo_dir, "task/x"));
    assert!(!agent_brain::gitops::branch_exists(&repo_dir, "task/nope"));

    let msg = "x: it's \"quoted\" $(echo no)";
    let new_sha = agent_brain::gitops::commit_paths(&repo_dir, &["new.txt"], msg).unwrap();
    assert_eq!(new_sha, agent_brain::gitops::head_sha(&repo_dir).unwrap());

    // verify subject exactly
    let subjects = agent_brain::gitops::commit_subjects(&repo_dir, &sha, "HEAD").unwrap();
    assert_eq!(subjects.len(), 1);
    assert_eq!(subjects[0].0, new_sha);
    assert_eq!(subjects[0].1, msg);

    let found_sha = agent_brain::gitops::find_commit_by_subject(&repo_dir, msg).unwrap();
    assert_eq!(found_sha, Some(new_sha));

    let diff = agent_brain::gitops::diff_names(&repo_dir, &sha, "HEAD").unwrap();
    assert_eq!(diff, ["new.txt".to_string()].into_iter().collect());

    let err = agent_brain::gitops::git(
        &repo_dir,
        &["rev-parse", "--verify", "refs/heads/nope"],
    )
    .unwrap_err();
    assert_eq!(err.code, 1);
    assert!(!err.message.is_empty());
}

#[test]
fn planfile_meta_line_and_skeleton() {
    let meta = serde_json::json!({
        "task": "t",
        "request": "a > b -->",
        "branch": "task/t"
    });
    let ml = agent_brain::planfile::meta_line(&meta);
    assert!(ml.starts_with("<!-- brain-meta: {\"branch\""));
    assert!(ml.ends_with(" -->"));
    let inner_json = &ml["<!-- brain-meta: ".len()..ml.len() - " -->".len()];
    assert!(!inner_json.contains('>'));
    assert!(inner_json.contains(r"\u003e"));

    let read_back = agent_brain::planfile::read_meta(&ml).expect("read_meta ok");
    assert_eq!(read_back, meta);

    let skel = agent_brain::planfile::render_plan_skeleton("Thêm nút\nthứ hai", &meta);
    let lines: Vec<&str> = skel.lines().collect();
    assert_eq!(lines[0], "# Plan: Thêm nút");
    assert_eq!(lines[1], ml);
    assert!(skel.ends_with('\n'));
    assert!(!skel.contains("thứ hai"));

    let long_title = "a".repeat(150);
    let skel_long = agent_brain::planfile::render_plan_skeleton(&long_title, &meta);
    let expected_title = format!("# Plan: {}", "a".repeat(100));
    assert_eq!(skel_long.lines().next().unwrap(), expected_title);

    let other_meta = serde_json::json!({"task": "other", "branch": "task/other"});
    let replaced = agent_brain::planfile::set_meta(&skel, &other_meta);
    assert_eq!(replaced.matches("<!-- brain-meta:").count(), 1);
    assert_eq!(
        agent_brain::planfile::read_meta(&replaced).unwrap(),
        other_meta
    );

    let from_short = agent_brain::planfile::set_meta("# T\nbody\n", &meta);
    assert_eq!(from_short, format!("# T\n{}\nbody\n", ml));

    assert_eq!(
        agent_brain::planfile::read_meta("<!-- brain-meta: not json -->"),
        None
    );
    assert!(agent_brain::planfile::PLAN_TEMPLATE.starts_with("# Plan: <tên task>"));
}
