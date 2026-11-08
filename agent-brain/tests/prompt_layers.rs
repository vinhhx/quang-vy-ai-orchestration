use agent_brain::projects::Project;
use agent_brain::prompts::{
    compose, compose_detailed, layers_for, read_note, with_test_builtin_notes, PromptLayers,
    VALID_ROLES,
};
use std::fs;
use std::os::unix::fs::symlink;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn setup_temp_config_env() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let cfg_path = tmp.path().join("config.json");
    fs::write(&cfg_path, agent_brain::config::DEFAULT_CONFIG_JSON).unwrap();
    std::env::set_var("AGENT_BRAIN_CONFIG", cfg_path.to_str().unwrap());
    tmp
}

#[test]
fn test_a_no_notes_byte_for_byte_all_roles() {
    let _lock = ENV_LOCK.lock().unwrap();
    let _tmp = setup_temp_config_env();
    let layers = PromptLayers::default();

    for role in VALID_ROLES {
        let base = agent_brain::prompts::base_prompt_for(role).unwrap();
        let (composed, warnings) = compose(base, role, &layers);
        assert_eq!(warnings.len(), 0);
        assert!(composed.starts_with(base), "Failed for role {role}");
        assert!(composed.contains("## Ghi chú nghiệp vụ"), "Failed for role {role}");
        assert!(composed.contains("### Phòng: "), "Failed for role {role}");
    }
}

#[test]
fn test_b_four_layers_two_stacks_ordering_and_empty_dropped() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = setup_temp_config_env();
    let prompts_dir = tmp.path().join("prompts");

    // stack-a has content
    let s_a = prompts_dir.join("stacks").join("stack-a");
    fs::create_dir_all(&s_a).unwrap();
    fs::write(s_a.join("lead.md"), "Stack A notes\n").unwrap();

    // stack-b is empty after trim -> should be dropped
    let s_b = prompts_dir.join("stacks").join("stack-b");
    fs::create_dir_all(&s_b).unwrap();
    fs::write(s_b.join("lead.md"), "   \n\t  ").unwrap();

    // project
    let p = prompts_dir.join("projects").join("demo-proj");
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("lead.md"), "Project demo notes\n").unwrap();

    // workflow
    let w = prompts_dir.join("workflows").join("fast-track");
    fs::create_dir_all(&w).unwrap();
    fs::write(w.join("lead.md"), "Workflow fast-track notes\n").unwrap();

    // team
    let t = prompts_dir.join("teams").join("core-team");
    fs::create_dir_all(&t).unwrap();
    fs::write(t.join("lead.md"), "Team core notes\n").unwrap();

    let layers = PromptLayers {
        stacks: vec!["stack-a".to_string(), "stack-b".to_string()],
        project: Some("demo-proj".to_string()),
        workflow: Some("fast-track".to_string()),
        team: Some("core-team".to_string()),
    };

    let base = agent_brain::prompts::base_prompt_for("lead").unwrap();
    let (composed, warnings, count) = compose_detailed(base, "lead", &layers);

    assert_eq!(warnings.len(), 0);
    assert_eq!(count, 5); // stack-a, project, workflow, team, and department; stack-b dropped
    assert!(composed.contains("### Stack: stack-a\nStack A notes"));
    assert!(!composed.contains("stack-b"));
    assert!(composed.contains("### Dự án: demo-proj\nProject demo notes"));
    assert!(composed.contains("### Workflow: fast-track\nWorkflow fast-track notes"));
    assert!(composed.contains("### Đội hình: core-team\nTeam core notes"));
    assert!(composed.contains("### Phòng: Phòng kế hoạch"));

    // Check order
    let pos_stack = composed.find("### Stack: stack-a").unwrap();
    let pos_proj = composed.find("### Dự án: demo-proj").unwrap();
    let pos_wf = composed.find("### Workflow: fast-track").unwrap();
    let pos_team = composed.find("### Đội hình: core-team").unwrap();
    let pos_dept = composed.find("### Phòng: Phòng kế hoạch").unwrap();
    assert!(pos_stack < pos_proj);
    assert!(pos_proj < pos_wf);
    assert!(pos_wf < pos_team);
    assert!(pos_team < pos_dept);
}

#[test]
fn test_c_builtin_stack_template_and_empty_user_file_override() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = setup_temp_config_env();

    with_test_builtin_notes(
        vec![("python", "dev", "Builtin python note for dev")],
        || {
            // When no user file exists:
            let note = read_note("stacks", "python", "dev").unwrap();
            assert_eq!(note.as_deref(), Some("Builtin python note for dev"));

            // When empty user file exists: overrides / turns off builtin template
            let p = tmp.path().join("prompts").join("stacks").join("python");
            fs::create_dir_all(&p).unwrap();
            fs::write(p.join("dev.md"), "").unwrap();

            let note_override = read_note("stacks", "python", "dev").unwrap();
            assert_eq!(note_override.as_deref(), Some(""));

            // In compose, empty after trim is dropped
            let layers = PromptLayers {
                stacks: vec!["python".to_string()],
                ..Default::default()
            };
            let (comp, _, count) = compose_detailed("base", "dev", &layers);
            assert_eq!(count, 1);
            assert!(comp.contains("### Phòng: Phòng thi công"));
            assert!(!comp.contains("python"));
        },
    );
}

#[test]
fn test_d_file_size_invalid_name_symlink_outside() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = setup_temp_config_env();
    let prompts_dir = tmp.path().join("prompts");

    // 1. File > 8192 bytes
    let big_dir = prompts_dir.join("stacks").join("big");
    fs::create_dir_all(&big_dir).unwrap();
    fs::write(big_dir.join("dev.md"), vec![b'a'; 8193]).unwrap();

    let err_big = read_note("stacks", "big", "dev").unwrap_err();
    assert!(err_big.contains("vượt quá giới hạn 8192 byte"));

    // 2. Invalid name
    assert!(read_note("stacks", "../escape", "dev").is_err());
    assert!(read_note("stacks", "bad/slash", "dev").is_err());

    // 3. Symlink pointing outside
    let outside_tmp = tempfile::tempdir().unwrap();
    let outside_file = outside_tmp.path().join("secret.md");
    fs::write(&outside_file, "secret outside prompts").unwrap();

    let sym_dir = prompts_dir.join("projects").join("symproj");
    fs::create_dir_all(&sym_dir).unwrap();
    symlink(&outside_file, sym_dir.join("dev.md")).unwrap();

    let err_sym = read_note("projects", "symproj", "dev").unwrap_err();
    assert!(err_sym.contains("trỏ ra ngoài thư mục prompts"));

    // Compose should collect warnings and skip these
    let layers = PromptLayers {
        stacks: vec!["big".to_string()],
        project: Some("symproj".to_string()),
        ..Default::default()
    };
    let (comp, warnings, count) = compose_detailed("base", "dev", &layers);
    assert_eq!(count, 1);
    assert!(comp.contains("### Phòng: Phòng thi công"));
    assert_eq!(warnings.len(), 2);
}

#[test]
fn test_e_dev_fix_uses_dev_notes() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = setup_temp_config_env();
    let prompts_dir = tmp.path().join("prompts");
    let s_react = prompts_dir.join("stacks").join("react");
    fs::create_dir_all(&s_react).unwrap();
    fs::write(s_react.join("dev.md"), "React fix rules\n").unwrap();

    let layers = PromptLayers {
        stacks: vec!["react".to_string()],
        ..Default::default()
    };

    let fix_prompt = agent_brain::roles::DEV_FIX_PROMPT;
    let (comp, warnings) = compose(fix_prompt, "dev", &layers);
    assert_eq!(warnings.len(), 0);
    assert!(comp.contains("React fix rules"));
    assert!(comp.starts_with(fix_prompt));
}

#[test]
fn test_f_run_dir_prompt_md_created_and_matches() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = setup_temp_config_env();
    let run_dir = tmp.path().join("runs").join("test_run");
    fs::create_dir_all(&run_dir).unwrap();

    let layers = PromptLayers {
        stacks: vec!["myserver".to_string()],
        ..Default::default()
    };
    let s_dir = tmp.path().join("prompts").join("stacks").join("myserver");
    fs::create_dir_all(&s_dir).unwrap();
    fs::write(s_dir.join("qa.md"), "QA custom check\n").unwrap();

    let base = agent_brain::prompts::base_prompt_for("qa").unwrap();
    let (composed, _) = compose(base, "qa", &layers);

    let spec = agent_brain::roles::RoleSpec {
        role: agent_brain::roles::Role::Qa,
        user_input: "test user input".to_string(),
        allowed_tools: vec![],
        disallowed_tools: vec![],
        extra_args: vec![],
        source_paths: vec![],
    };
    let cfg = agent_brain::config::config_from_value(&serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap()).unwrap();
    let (cmd, _) = agent_brain::roles::build_command_layered(&spec, &cfg, base, &layers).unwrap();

    // Verify claude_argv_parts extracts the exact composed prompt
    let (_, extracted_prompt) = agent_brain::engine::claude_argv_parts(&cmd).unwrap();
    assert_eq!(extracted_prompt, composed);

    // Write to run_dir/prompt.md as execute_role_staged does
    let prompt_file = run_dir.join("prompt.md");
    fs::write(&prompt_file, &extracted_prompt).unwrap();

    let saved = fs::read_to_string(&prompt_file).unwrap();
    assert_eq!(saved, composed);
}

#[test]
fn test_g_prompt_show_cli() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = setup_temp_config_env();
    let db_path = tmp.path().join("brain.db");
    let conn = agent_brain::db::open(&db_path).unwrap();

    let p = Project {
        id: "my-app".to_string(),
        path: "/path/app".to_string(),
        name_vi: "Ứng dụng".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "default".to_string(),
        repo: None,
        missing: false,
        stack: vec!["rust".to_string()],
        read_sources: vec![],
        protocol_dir: String::new(),
    };
    agent_brain::db::sync_projects(&conn, std::slice::from_ref(&p)).unwrap();

    let map = serde_json::json!({
        "projects": [
            {
                "id": "my-app",
                "name_vi": "Ứng dụng",
                "path": "/path/app",
                "type": "code",
                "profile": "default",
                "stack": ["rust"]
            }
        ]
    });
    let cfg = agent_brain::config::config_from_value(&serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap()).unwrap();
    let ctx = agent_brain::app::Ctx {
        conn,
        cfg,
        map,
        projects: vec![p],
    };

    // Valid role and project
    let res = agent_brain::prompts::cmd_show(&ctx, "my-app", "lead", None, None, None);
    assert_eq!(res.unwrap(), 0);

    // Invalid role -> error
    let res_err_role = agent_brain::prompts::cmd_show(&ctx, "my-app", "invalid-role", None, None, None);
    assert!(res_err_role.is_err());

    // Invalid project -> routing error
    let res_err_proj = agent_brain::prompts::cmd_show(&ctx, "unknown-proj", "lead", None, None, None);
    assert!(res_err_proj.is_err());
    assert_eq!(res_err_proj.unwrap_err().code, agent_brain::error::EXIT_ROUTING);
}

#[test]
fn test_h_projects_map_stack_parsing_and_invalid_filtering() {
    // 1. With valid stack
    let val_valid = serde_json::json!({
        "id": "p1",
        "name_vi": "P1",
        "path": "/p1",
        "type": "code",
        "profile": "default",
        "stack": ["react", "node-js"]
    });
    let p1 = Project::from_value(&val_valid).unwrap();
    assert_eq!(p1.stack, vec!["react".to_string(), "node-js".to_string()]);

    // 2. Without stack field
    let val_none = serde_json::json!({
        "id": "p2",
        "name_vi": "P2",
        "path": "/p2",
        "type": "code",
        "profile": "default"
    });
    let p2 = Project::from_value(&val_none).unwrap();
    assert!(p2.stack.is_empty());

    // 3. With invalid stack name (contains uppercase, slash, or invalid chars)
    let val_invalid = serde_json::json!({
        "id": "p3",
        "name_vi": "P3",
        "path": "/p3",
        "type": "code",
        "profile": "default",
        "stack": ["valid-1", "BAD/NAME", "../hack", "valid-2"]
    });
    let p3 = Project::from_value(&val_invalid).unwrap();
    assert_eq!(p3.stack, vec!["valid-1".to_string(), "valid-2".to_string()]);

    // 4. Test layers_for
    let layers = layers_for(&p3, Some("wf-a"), Some("team-b"));
    assert_eq!(layers.stacks, vec!["valid-1".to_string(), "valid-2".to_string()]);
    assert_eq!(layers.project.as_deref(), Some("p3"));
    assert_eq!(layers.workflow.as_deref(), Some("wf-a"));
    assert_eq!(layers.team.as_deref(), Some("team-b"));
}
