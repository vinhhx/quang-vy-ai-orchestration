use std::path::Path;

fn init_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let temp_root = dir.parent().unwrap_or(dir);
    let nohooks = temp_root.join("nohooks");
    std::fs::create_dir_all(&nohooks).unwrap();
    let global_cfg = temp_root.join("gitconfig");
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
    assert_eq!(err.message, "Cấu hình không hợp lệ (phải là JSON object)");

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
    assert!(err.message.contains("Đường dẫn dự án 'p1' không tồn tại: "));

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
    assert_eq!(changed_after, ["new.txt".to_string()].into_iter().collect());

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

    let err = agent_brain::gitops::git(&repo_dir, &["rev-parse", "--verify", "refs/heads/nope"])
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

#[test]
fn db_open_creates_schema_with_private_modes() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let db_dir = tmp.path().join("data").join("agent-brain");
    let db_file = db_dir.join("brain.db");

    let conn = agent_brain::db::open(&db_file).expect("db::open ok");
    let dir_meta = std::fs::metadata(&db_dir).unwrap();
    assert_eq!(dir_meta.permissions().mode() & 0o777, 0o700);

    let file_meta = std::fs::metadata(&db_file).unwrap();
    assert_eq!(file_meta.permissions().mode() & 0o777, 0o600);

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);

    let journal_mode: String = conn
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(journal_mode.to_lowercase(), "wal");

    let fk: i64 = conn
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1);

    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type IN ('table', 'index')")
        .unwrap();
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    for expected in [
        "projects",
        "tasks",
        "runs",
        "events",
        "runs_started",
        "runs_task",
        "runs_running",
    ] {
        assert!(names.contains(&expected.to_string()), "missing: {expected}");
    }

    drop(stmt);
    drop(conn);
    let conn2 = agent_brain::db::open(&db_file).expect("db::open reopen ok");
    let user_ver2: i64 = conn2
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver2, agent_brain::db::SCHEMA_VERSION);
}

#[test]
fn db_task_run_event_api() {
    let tmp = tempfile::tempdir().unwrap();
    let db_file = tmp.path().join("brain.db");
    let conn = agent_brain::db::open(&db_file).unwrap();

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
        aliases: vec![],
        kind: "code".to_string(),
        profile: "personal".to_string(),
        repo: None,
        missing: false,
    };

    agent_brain::db::sync_projects(&conn, &[p1.clone(), p2.clone()]).unwrap();

    let mut p1_mod = p1.clone();
    p1_mod.name_vi = "Web bán hàng mới".to_string();
    let mut p2_mod = p2.clone();
    p2_mod.missing = true;
    agent_brain::db::sync_projects(&conn, &[p1_mod, p2_mod]).unwrap();

    let p1_row_id = agent_brain::db::project_row_id(&conn, "web-ban-hang").unwrap();
    let _p2_row_id = agent_brain::db::project_row_id(&conn, "kho-anh").unwrap();

    // sync with only web-ban-hang still leaves 2 rows
    agent_brain::db::sync_projects(&conn, &[p1.clone()]).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 2);

    assert!(agent_brain::db::project_row_id(&conn, "nope").is_err());

    let task_id1 = agent_brain::db::upsert_task(
        &conn,
        p1_row_id,
        "t",
        "Title 1",
        "task/t",
        Some("main"),
        Some("sha1"),
    )
    .unwrap();
    let task_id2 = agent_brain::db::upsert_task(
        &conn,
        p1_row_id,
        "t",
        "Title 2",
        "task/t",
        Some("main"),
        Some("sha2"),
    )
    .unwrap();
    assert_eq!(task_id1, task_id2);

    let t = agent_brain::db::find_task_by_branch(&conn, p1_row_id, "task/t")
        .unwrap()
        .expect("find_task_by_branch ok");
    assert_eq!(t.status, "open");
    assert_eq!(t.title, "Title 2");

    agent_brain::db::set_task_status(&conn, task_id1, "planned").unwrap();
    let t_planned = agent_brain::db::get_task(&conn, task_id1).unwrap().unwrap();
    assert_eq!(t_planned.status, "planned");

    let new_run = agent_brain::db::NewRun {
        task_id: Some(task_id1),
        project_id: p1_row_id,
        role: "lead".to_string(),
        profile: "work".to_string(),
        model: Some("opus".to_string()),
        pid: 123,
        pgid: 123,
        pid_start: 456,
        base_commit: Some("sha1".to_string()),
        run_dir: Some("/run/dir".to_string()),
    };
    let run_id = agent_brain::db::insert_run(&conn, &new_run).unwrap();

    let r_init = agent_brain::db::get_run(&conn, run_id).unwrap().unwrap();
    assert_eq!(r_init.status, "running");
    assert!(r_init.started_at > 0);
    assert_eq!(r_init.ended_at, None);

    let running = agent_brain::db::running_runs(&conn).unwrap();
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].id, run_id);

    let running_for_task = agent_brain::db::running_run_for_task(&conn, task_id1)
        .unwrap()
        .expect("running_run_for_task ok");
    assert_eq!(running_for_task.id, run_id);

    agent_brain::db::insert_event(&conn, run_id, 1, 1000, "user", r#"{"type":"user"}"#).unwrap();
    agent_brain::db::insert_event(
        &conn,
        run_id,
        2,
        2000,
        "assistant",
        r#"{"type":"assistant"}"#,
    )
    .unwrap();
    assert!(agent_brain::db::insert_event(&conn, run_id, 1, 3000, "user", "{}").is_err());

    let finish = agent_brain::db::RunFinish {
        status: "succeeded".to_string(),
        exit_code: Some(0),
        session_id: Some("sess-1".to_string()),
        usage: agent_brain::stream::Usage {
            input_tokens: 11,
            output_tokens: 22,
            cache_read_input_tokens: 33,
            cache_creation_input_tokens: 44,
        },
        usage_source: agent_brain::stream::UsageSource::Result
            .as_str()
            .to_string(),
        cost_usd: 0.5,
        num_turns: Some(2),
        assistant_turns: Some(1),
        turns_before_first_edit: Some(1),
        reads_outside_plan: Some(0),
        permission_denials: 0,
        error: None,
    };
    agent_brain::db::finish_run(&conn, run_id, &finish).unwrap();

    let r_fin = agent_brain::db::get_run(&conn, run_id).unwrap().unwrap();
    assert_eq!(r_fin.input_tokens, 11);
    assert_eq!(r_fin.output_tokens, 22);
    assert_eq!(r_fin.cache_read_tokens, 33);
    assert_eq!(r_fin.cache_write_tokens, 44);
    assert_eq!(r_fin.cost_usd, 0.5);
    assert_eq!(r_fin.usage_source, "result");
    assert!(r_fin.ended_at.unwrap() >= r_fin.started_at);

    assert!(agent_brain::db::running_runs(&conn).unwrap().is_empty());

    let latest = agent_brain::db::latest_run_for_task(&conn, task_id1)
        .unwrap()
        .expect("latest ok");
    assert_eq!(latest.id, run_id);

    agent_brain::db::set_run_outcome(&conn, run_id, "qa:pass").unwrap();
    agent_brain::db::mark_kill_requested(&conn, run_id).unwrap();

    let r_updated = agent_brain::db::get_run(&conn, run_id).unwrap().unwrap();
    assert_eq!(r_updated.outcome.as_deref(), Some("qa:pass"));
    assert!(r_updated.kill_requested_at.is_some());

    let new_run2 = agent_brain::db::NewRun {
        task_id: Some(task_id1),
        project_id: p1_row_id,
        role: "dev".to_string(),
        profile: "work".to_string(),
        model: Some("sonnet".to_string()),
        pid: 124,
        pgid: 124,
        pid_start: 457,
        base_commit: None,
        run_dir: None,
    };
    let run_id2 = agent_brain::db::insert_run(&conn, &new_run2).unwrap();
    agent_brain::db::mark_status(&conn, run_id2, "lost").unwrap();
    let r2 = agent_brain::db::get_run(&conn, run_id2).unwrap().unwrap();
    assert_eq!(r2.status, "lost");
    assert!(r2.ended_at.is_some());

    assert_eq!(agent_brain::db::count_runs(&conn).unwrap(), 2);

    let bad_project_run = agent_brain::db::NewRun {
        task_id: None,
        project_id: 9999,
        role: "lead".to_string(),
        profile: "work".to_string(),
        model: None,
        pid: 10,
        pgid: 10,
        pid_start: 10,
        base_commit: None,
        run_dir: None,
    };
    assert!(agent_brain::db::insert_run(&conn, &bad_project_run).is_err());

    let bad_role_run = agent_brain::db::NewRun {
        task_id: None,
        project_id: p1_row_id,
        role: "bogus".to_string(),
        profile: "work".to_string(),
        model: None,
        pid: 10,
        pgid: 10,
        pid_start: 10,
        base_commit: None,
        run_dir: None,
    };
    assert!(agent_brain::db::insert_run(&conn, &bad_role_run).is_err());
}

#[test]
fn tasks_current_task_and_adoption() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_dir = tmp.path().join("repo");
    init_repo(&repo_dir);

    let db_file = tmp.path().join("brain.db");
    let conn = agent_brain::db::open(&db_file).unwrap();

    let p = agent_brain::projects::Project {
        id: "proj".to_string(),
        path: repo_dir.to_str().unwrap().to_string(),
        name_vi: "Proj".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    agent_brain::db::sync_projects(&conn, &[p]).unwrap();
    let proj_row_id = agent_brain::db::project_row_id(&conn, "proj").unwrap();

    // on main -> None
    let cur_main = agent_brain::tasks::current_task(&conn, proj_row_id, &repo_dir).unwrap();
    assert!(cur_main.is_none());

    // new branch task/py-task
    agent_brain::gitops::create_branch(&repo_dir, "task/py-task").unwrap();
    let sha = agent_brain::gitops::head_sha(&repo_dir).unwrap();

    let meta = serde_json::json!({
        "base_branch": "main",
        "base_commit": sha,
        "branch": "task/py-task",
        "request": "việc cũ",
        "task": "py-task"
    });
    let ml = agent_brain::planfile::meta_line(&meta);
    let plan_text = format!("# Plan: việc cũ\n{}\n## Mục tiêu\n", ml);
    std::fs::write(repo_dir.join(".plan.md"), plan_text).unwrap();
    agent_brain::gitops::commit_paths(&repo_dir, &[".plan.md"], "add plan").unwrap();

    let adopted = agent_brain::tasks::current_task(&conn, proj_row_id, &repo_dir)
        .unwrap()
        .expect("adopted ok");
    assert_eq!(adopted.slug, "py-task");
    assert_eq!(adopted.title, "việc cũ");
    assert_eq!(adopted.base_branch.as_deref(), Some("main"));
    assert_eq!(adopted.base_commit.as_deref(), Some(sha.as_str()));

    // second call returns the same id
    let adopted2 = agent_brain::tasks::current_task(&conn, proj_row_id, &repo_dir)
        .unwrap()
        .expect("second call ok");
    assert_eq!(adopted2.id, adopted.id);
    let task_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(task_count, 1);

    // branch task/other whose .plan.md names task/py-task -> None
    agent_brain::gitops::create_branch(&repo_dir, "task/other").unwrap();
    let other = agent_brain::tasks::current_task(&conn, proj_row_id, &repo_dir).unwrap();
    assert!(other.is_none());

    // branch task/z with upsert_task works even without .plan.md
    agent_brain::gitops::create_branch(&repo_dir, "task/z").unwrap();
    std::fs::remove_file(repo_dir.join(".plan.md")).unwrap();
    let z_id =
        agent_brain::db::upsert_task(&conn, proj_row_id, "z", "Title Z", "task/z", None, None)
            .unwrap();
    let cur_z = agent_brain::tasks::current_task(&conn, proj_row_id, &repo_dir)
        .unwrap()
        .expect("task z found");
    assert_eq!(cur_z.id, z_id);
}

#[test]
fn procs_identity() {
    let own_pid = std::process::id() as i32;
    let proc_id = agent_brain::procs::proc_id(own_pid).expect("own proc_id found");
    assert!(proc_id.start > 0);
    assert!(agent_brain::procs::alive(proc_id));

    let bogus_proc = agent_brain::procs::ProcId {
        pid: own_pid,
        start: proc_id.start + 1,
    };
    assert!(!agent_brain::procs::alive(bogus_proc));

    let stat = agent_brain::procs::read_stat(own_pid).expect("read_stat ok");
    assert!(stat.ppid > 0);
    assert!(stat.state == 'R' || stat.state == 'S');

    // child sleep 30
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn sleep ok");
    let child_pid = child.id() as i32;
    let child_proc_id = agent_brain::procs::proc_id(child_pid).expect("child proc_id found");
    assert!(agent_brain::procs::alive(child_proc_id));

    child.kill().unwrap();
    child.wait().unwrap();
    assert!(!agent_brain::procs::alive(child_proc_id));
}

#[test]
fn reconcile_marks_only_dead_runs_lost() {
    let tmp = tempfile::tempdir().unwrap();
    let db_file = tmp.path().join("brain.db");
    let conn = agent_brain::db::open(&db_file).unwrap();

    let p = agent_brain::projects::Project {
        id: "proj".to_string(),
        path: "/tmp/p".to_string(),
        name_vi: "P".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    agent_brain::db::sync_projects(&conn, &[p]).unwrap();
    let proj_row_id = agent_brain::db::project_row_id(&conn, "proj").unwrap();

    // A: child spawned and reaped
    let mut child_a = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let pid_a = child_a.id() as i32;
    let proc_a = agent_brain::procs::proc_id(pid_a).unwrap();
    child_a.kill().unwrap();
    child_a.wait().unwrap();

    let run_a = agent_brain::db::NewRun {
        task_id: None,
        project_id: proj_row_id,
        role: "lead".to_string(),
        profile: "work".to_string(),
        model: None,
        pid: pid_a as i64,
        pgid: pid_a as i64,
        pid_start: proc_a.start as i64,
        base_commit: None,
        run_dir: None,
    };
    let run_id_a = agent_brain::db::insert_run(&conn, &run_a).unwrap();

    // B: own test pid
    let own_pid = std::process::id() as i32;
    let own_proc = agent_brain::procs::proc_id(own_pid).unwrap();
    let run_b = agent_brain::db::NewRun {
        task_id: None,
        project_id: proj_row_id,
        role: "dev".to_string(),
        profile: "work".to_string(),
        model: None,
        pid: own_pid as i64,
        pgid: own_pid as i64,
        pid_start: own_proc.start as i64,
        base_commit: None,
        run_dir: None,
    };
    let run_id_b = agent_brain::db::insert_run(&conn, &run_b).unwrap();

    // C: pid null
    let run_c = agent_brain::db::NewRun {
        task_id: None,
        project_id: proj_row_id,
        role: "qa".to_string(),
        profile: "work".to_string(),
        model: None,
        pid: 0,
        pgid: 0,
        pid_start: 0,
        base_commit: None,
        run_dir: None,
    };
    let run_id_c = agent_brain::db::insert_run(&conn, &run_c).unwrap();
    conn.execute(
        "UPDATE runs SET pid = NULL, pid_start = NULL WHERE id = ?1",
        rusqlite::params![run_id_c],
    )
    .unwrap();

    // D: live decoy sleep 30, recorded with pid_start + 1
    let mut decoy = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let decoy_pid = decoy.id() as i32;
    let decoy_proc = agent_brain::procs::proc_id(decoy_pid).unwrap();
    let run_d = agent_brain::db::NewRun {
        task_id: None,
        project_id: proj_row_id,
        role: "inspect".to_string(),
        profile: "work".to_string(),
        model: None,
        pid: decoy_pid as i64,
        pgid: decoy_pid as i64,
        pid_start: (decoy_proc.start + 1) as i64,
        base_commit: None,
        run_dir: None,
    };
    let run_id_d = agent_brain::db::insert_run(&conn, &run_d).unwrap();

    let reconciled = agent_brain::runner::reconcile_stale_runs(&conn).unwrap();
    assert_eq!(reconciled, 3);

    let row_a = agent_brain::db::get_run(&conn, run_id_a).unwrap().unwrap();
    assert_eq!(row_a.status, "lost");
    assert!(row_a.ended_at.is_some());

    let row_b = agent_brain::db::get_run(&conn, run_id_b).unwrap().unwrap();
    assert_eq!(row_b.status, "running");
    assert_eq!(row_b.ended_at, None);

    let row_c = agent_brain::db::get_run(&conn, run_id_c).unwrap().unwrap();
    assert_eq!(row_c.status, "lost");
    assert!(row_c.ended_at.is_some());

    let row_d = agent_brain::db::get_run(&conn, run_id_d).unwrap().unwrap();
    assert_eq!(row_d.status, "lost");
    assert!(row_d.ended_at.is_some());

    // decoy is still running
    assert!(decoy.try_wait().unwrap().is_none());
    decoy.kill().unwrap();
    decoy.wait().unwrap();
}

#[test]
fn open_ctx_with_reconciles_and_mirrors() {
    let tmp = tempfile::tempdir().unwrap();
    let map_file = tmp.path().join("projects_map.json");
    let map_json = serde_json::json!({
        "scan_roots": [],
        "projects": [
            {
                "id": "p1",
                "path": "/path/p1",
                "name_vi": "Dự án 1",
                "aliases": [],
                "type": "code",
                "profile": "work"
            },
            {
                "id": "p2",
                "path": "/path/p2",
                "name_vi": "Dự án 2",
                "aliases": [],
                "type": "code",
                "profile": "personal"
            }
        ]
    });
    std::fs::write(&map_file, serde_json::to_string(&map_json).unwrap()).unwrap();

    let db_file = tmp.path().join("brain.db");
    {
        let conn = agent_brain::db::open(&db_file).unwrap();
        // insert a project to satisfy foreign key for run
        conn.execute(
            "INSERT INTO projects (slug, path, name_vi, type, profile) VALUES ('p1', '/path/p1', 'Dự án 1', 'code', 'work')",
            [],
        ).unwrap();
        let proj_id = conn.last_insert_rowid();

        // dead child run
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let pid = child.id() as i32;
        let proc = agent_brain::procs::proc_id(pid).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();

        let run = agent_brain::db::NewRun {
            task_id: None,
            project_id: proj_id,
            role: "lead".to_string(),
            profile: "work".to_string(),
            model: None,
            pid: pid as i64,
            pgid: pid as i64,
            pid_start: proc.start as i64,
            base_commit: None,
            run_dir: None,
        };
        agent_brain::db::insert_run(&conn, &run).unwrap();
    }

    let default_val: serde_json::Value =
        serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap();
    let cfg = agent_brain::config::config_from_value(&default_val).unwrap();

    let ctx = agent_brain::app::open_ctx_with(cfg, &map_file, &db_file).unwrap();
    assert_eq!(ctx.projects.len(), 2);

    let count: i64 = ctx
        .conn
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 2);

    let stale_run = agent_brain::db::get_run(&ctx.conn, 1).unwrap().unwrap();
    assert_eq!(stale_run.status, "lost");
    assert!(stale_run.ended_at.is_some());
}
