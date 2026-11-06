use agent_brain::config::Config;
use agent_brain::departments::{
    dept_name_of, dept_of_role, load_from, model_family, pick_from, Level,
};
use agent_brain::engine::{plan_for, resolve_for_run_with, Engine};
use rusqlite::Connection;
use serde_json::json;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn test_cfg_with_agy() -> Config {
    let val = json!({
        "models": { "lead": "opus", "dev": "sonnet", "qa": "sonnet" },
        "profiles": { "work": {}, "personal": {} },
        "engines": [
            { "id": "agy", "kind": "agy" }
        ]
    });
    agent_brain::config::config_from_value(&val).unwrap()
}

fn test_db() -> (tempfile::TempDir, Connection) {
    let tmp = tempfile::tempdir().unwrap();
    let conn = agent_brain::db::open(&tmp.path().join("brain.db")).unwrap();
    conn.execute(
        "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
         VALUES (1, 'p1', '/path/1', 'Dự án 1', '[]', 'code', 'work', NULL, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level)
         VALUES (1, 1, 'task-1', 'Task 1', 'task/1', 'main', 'base1', 'open', 1000, 1000, 'vua')",
        [],
    )
    .unwrap();
    (tmp, conn)
}

#[test]
fn example_departments_json_loads_cleanly() {
    let example_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/departments.example.json");
    let text = std::fs::read_to_string(&example_path).expect("read departments.example.json");
    let cfg = test_cfg_with_agy();
    let loaded = load_from(&text, &cfg);

    assert!(
        loaded.errors.is_empty(),
        "departments.example.json phải tải không lỗi, nhưng có: {:?}",
        loaded.errors
    );
    assert_eq!(loaded.members.len(), 17);
    assert!(loaded.has_members());

    // Kiểm tra đủ 5 phòng
    assert_eq!(loaded.members_of("phan-tich").len(), 6);
    assert_eq!(loaded.members_of("ke-hoach").len(), 3);
    assert_eq!(loaded.members_of("thi-cong").len(), 4);
    assert_eq!(loaded.members_of("qa").len(), 3);
    assert_eq!(loaded.members_of("bao-mat").len(), 1);
}

#[test]
fn departments_and_roles_mapping() {
    assert_eq!(dept_name_of("phan-tich"), Some("Phòng phân tích"));
    assert_eq!(dept_name_of("ke-hoach"), Some("Phòng kế hoạch"));
    assert_eq!(dept_name_of("thi-cong"), Some("Phòng thi công"));
    assert_eq!(dept_name_of("qa"), Some("Phòng QA"));
    assert_eq!(dept_name_of("bao-mat"), Some("Phòng security"));
    assert_eq!(dept_name_of("la"), None);

    assert_eq!(dept_of_role("analyst"), Some("phan-tich"));
    assert_eq!(dept_of_role("analysis-review"), Some("phan-tich"));
    assert_eq!(dept_of_role("lead"), Some("ke-hoach"));
    assert_eq!(dept_of_role("plan-review"), Some("ke-hoach"));
    assert_eq!(dept_of_role("dev"), Some("thi-cong"));
    assert_eq!(dept_of_role("qa"), Some("qa"));
    assert_eq!(dept_of_role("code-review"), Some("qa"));
    assert_eq!(dept_of_role("unknown"), None);
}

#[test]
fn level_parse_and_display() {
    assert_eq!(Level::parse("de"), Some(Level::De));
    assert_eq!(Level::parse("vua"), Some(Level::Vua));
    assert_eq!(Level::parse("kho"), Some(Level::Kho));
    assert_eq!(Level::parse("DE"), Some(Level::De));
    assert_eq!(Level::parse("khac"), None);

    assert_eq!(Level::De.as_str(), "de");
    assert_eq!(Level::Vua.as_str(), "vua");
    assert_eq!(Level::Kho.as_str(), "kho");

    assert_eq!(Level::De.display_vi(), "Dễ");
    assert_eq!(Level::Vua.display_vi(), "Vừa");
    assert_eq!(Level::Kho.display_vi(), "Khó");
}

#[test]
fn validation_rejects_unknown_dept_key() {
    let cfg = test_cfg_with_agy();
    let json_text = json!({
        "phong-la": {
            "members": {}
        }
    })
    .to_string();
    let loaded = load_from(&json_text, &cfg);
    assert!(!loaded.errors.is_empty());
    assert!(loaded.errors[0].contains("Khóa phòng ban không hợp lệ: phong-la"));
}

#[test]
fn validation_rejects_duplicate_member_id_across_departments() {
    let cfg = test_cfg_with_agy();
    let json_text = json!({
        "phan-tich": {
            "members": {
                "m-1": {
                    "display_name": "Member 1",
                    "engine": "claude",
                    "levels": ["vua"]
                }
            }
        },
        "thi-cong": {
            "members": {
                "m-1": {
                    "display_name": "Member 1 trùng",
                    "engine": "claude",
                    "levels": ["vua"]
                }
            }
        }
    })
    .to_string();
    let loaded = load_from(&json_text, &cfg);
    assert!(!loaded.errors.is_empty());
    assert!(loaded.errors.iter().any(|e| e.contains("Mã thành viên bị trùng: 'm-1'")));
    assert_eq!(loaded.members.len(), 1);
}

#[test]
fn validation_rejects_human_and_dsh_engines() {
    let val = json!({
        "models": { "lead": "opus", "dev": "sonnet", "qa": "sonnet" },
        "profiles": { "work": {}, "personal": {} },
        "engines": [
            { "id": "agy", "kind": "agy" },
            { "id": "my-dsh", "kind": "dsh", "container": "c1" }
        ]
    });
    let cfg = agent_brain::config::config_from_value(&val).unwrap();

    let json_text = json!({
        "thi-cong": {
            "members": {
                "m-human": {
                    "display_name": "Người",
                    "engine": "người:ha",
                    "levels": ["vua"]
                },
                "m-dsh": {
                    "display_name": "Dsh",
                    "engine": "my-dsh",
                    "levels": ["vua"]
                }
            }
        }
    })
    .to_string();
    let loaded = load_from(&json_text, &cfg);
    assert_eq!(loaded.members.len(), 0);
    assert!(loaded.errors.iter().any(|e| e.contains("không thể là người thật")));
    assert!(loaded.errors.iter().any(|e| e.contains("Engine dsh không được hỗ trợ")));
}

#[test]
fn validation_rejects_empty_levels_and_invalid_model() {
    let cfg = test_cfg_with_agy();
    let json_text = json!({
        "thi-cong": {
            "members": {
                "m-bad-level": {
                    "display_name": "Bad Level",
                    "engine": "claude",
                    "levels": []
                },
                "m-bad-model": {
                    "display_name": "Bad Model",
                    "engine": "claude",
                    "model": "model with spaces!@#",
                    "levels": ["vua"]
                }
            }
        }
    })
    .to_string();
    let loaded = load_from(&json_text, &cfg);
    assert_eq!(loaded.members.len(), 0);
    assert!(loaded.errors.iter().any(|e| e.contains("levels không được để rỗng")));
    assert!(loaded.errors.iter().any(|e| e.contains("không hợp lệ")));
}

#[test]
fn test_model_family() {
    assert_eq!(model_family("claude-opus-5-5-high"), "claude-opus-5-5");
    assert_eq!(model_family("claude-opus-5-5-medium"), "claude-opus-5-5");
    assert_eq!(model_family("claude-opus-5-5-low"), "claude-opus-5-5");
    assert_eq!(model_family("claude-opus-5-5"), "claude-opus-5-5");
    assert_eq!(model_family("gemini-4-argon-high"), "gemini-4-argon");
    assert_eq!(model_family("gemini-3.8-flash-medium"), "gemini-3.8-flash");
}

#[test]
fn pick_rules_level_match_and_fallback() {
    let (_tmp, conn) = test_db();
    let cfg = test_cfg_with_agy();

    let dept_json = json!({
        "thi-cong": {
            "members": {
                "dev-de": {
                    "display_name": "Dev Dễ",
                    "engine": "claude",
                    "levels": ["de"]
                },
                "dev-kho": {
                    "display_name": "Dev Khó",
                    "engine": "claude",
                    "levels": ["kho"]
                }
            }
        }
    })
    .to_string();

    let loaded = load_from(&dept_json, &cfg);

    // Task mức de -> chọn dev-de
    let picked = pick_from(&conn, &cfg, &loaded, 1, "dev", Level::De, "work").unwrap();
    assert!(picked.is_some());
    assert_eq!(picked.unwrap().member.id, "dev-de");

    // Task mức vua (không ai có mức vua) -> fallback lấy cả phòng, dev-de đứng trước -> chọn dev-de
    let picked_vua = pick_from(&conn, &cfg, &loaded, 1, "dev", Level::Vua, "work").unwrap();
    assert!(picked_vua.is_some());
    assert_eq!(picked_vua.unwrap().member.id, "dev-de");

    // Vai trò không thuộc phòng nào -> None
    let picked_none = pick_from(&conn, &cfg, &loaded, 1, "inspect", Level::Vua, "work").unwrap();
    assert!(picked_none.is_none());
}

#[test]
fn pick_rules_keeps_previous_member_same_role() {
    let (_tmp, conn) = test_db();
    let cfg = test_cfg_with_agy();

    let dept_json = json!({
        "thi-cong": {
            "members": {
                "dev-1": {
                    "display_name": "Dev 1",
                    "engine": "claude",
                    "levels": ["vua", "kho"]
                },
                "dev-2": {
                    "display_name": "Dev 2",
                    "engine": "claude",
                    "levels": ["vua", "kho"]
                }
            }
        }
    })
    .to_string();

    let loaded = load_from(&dept_json, &cfg);

    // Chèn run trước đó của task 1 với vai dev và member_id = dev-2
    conn.execute(
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, status, started_at, member_id)
         VALUES (10, 1, 1, 'dev', 'claude', 'work', 'sonnet', 'succeeded', 1000, 'dev-2')",
        [],
    )
    .unwrap();

    let picked = pick_from(&conn, &cfg, &loaded, 1, "dev", Level::Kho, "work").unwrap().unwrap();
    assert_eq!(picked.member.id, "dev-2");
    assert!(picked.reason.contains("giữ người cũ"));
}

#[test]
fn pick_rules_review_independence_prefers_different_model_family() {
    let (_tmp, conn) = test_db();
    let cfg = test_cfg_with_agy();

    let dept_json = json!({
        "qa": {
            "members": {
                "qa-sonnet": {
                    "display_name": "QA Sonnet",
                    "engine": "claude",
                    "model": "claude-sonnet-5-5-high",
                    "levels": ["vua"]
                },
                "qa-opus": {
                    "display_name": "QA Opus",
                    "engine": "claude",
                    "model": "claude-opus-5-5-high",
                    "levels": ["vua"]
                }
            }
        }
    })
    .to_string();

    let loaded = load_from(&dept_json, &cfg);

    // Task 1 có run dev chạy model claude-sonnet-5-5
    conn.execute(
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, status, started_at)
         VALUES (20, 1, 1, 'dev', 'claude', 'work', 'claude-sonnet-5-5', 'succeeded', 1000)",
        [],
    )
    .unwrap();

    // Khi chọn QA, qa-sonnet cùng họ model claude-sonnet-5-5 bị loại, chọn qa-opus (họ claude-opus-5-5)
    let picked = pick_from(&conn, &cfg, &loaded, 1, "qa", Level::Vua, "work").unwrap().unwrap();
    assert_eq!(picked.member.id, "qa-opus");
}

#[test]
fn pick_rules_fewest_running_and_score_tie_break() {
    let (_tmp, conn) = test_db();
    let cfg = test_cfg_with_agy();

    let dept_json = json!({
        "thi-cong": {
            "members": {
                "dev-busy": {
                    "display_name": "Dev Busy",
                    "engine": "claude",
                    "levels": ["vua"]
                },
                "dev-free": {
                    "display_name": "Dev Free",
                    "engine": "claude",
                    "levels": ["vua"]
                }
            }
        }
    })
    .to_string();

    let loaded = load_from(&dept_json, &cfg);

    // Thêm task 2 để dev-busy có run đang chạy ở task khác (tránh kích hoạt luật giữ người cũ cho task 1)
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level)
         VALUES (2, 1, 'task-2', 'Task 2', 'task/2', 'main', 'base1', 'open', 1000, 1000, 'vua')",
        [],
    )
    .unwrap();

    // dev-busy có 1 run đang running trên task 2
    conn.execute(
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, status, started_at, member_id)
         VALUES (30, 2, 1, 'dev', 'claude', 'work', 'sonnet', 'running', 1000, 'dev-busy')",
        [],
    )
    .unwrap();

    let picked = pick_from(&conn, &cfg, &loaded, 1, "dev", Level::Vua, "work").unwrap().unwrap();
    assert_eq!(picked.member.id, "dev-free");

    // Nếu cả 2 đều rảnh (0 running), người có điểm cao hơn (>= 5 lượt) thắng
    conn.execute("UPDATE runs SET status = 'succeeded' WHERE id = 30", []).unwrap();

    // dev-busy có 5 điểm 5.0 (cần tạo run trước vì có khoá ngoại)
    for i in 1..=5 {
        let run_id = 100 + i;
        conn.execute(
            "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, status, started_at, member_id)
             VALUES (?1, 1, 1, 'dev', 'claude', 'work', 'sonnet', 'succeeded', 1000, 'dev-busy')",
            rusqlite::params![run_id],
        )
        .unwrap();
        agent_brain::db::record_member_score(&conn, run_id, "dev-busy", Some(1), 5, None).unwrap();
    }
    // dev-free chỉ có 2 điểm 5.0 -> tính là 3.0 (< 5 lượt)
    for run_id in [201, 202] {
        conn.execute(
            "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, status, started_at, member_id)
             VALUES (?1, 1, 1, 'dev', 'claude', 'work', 'sonnet', 'succeeded', 1000, 'dev-free')",
            rusqlite::params![run_id],
        )
        .unwrap();
        agent_brain::db::record_member_score(&conn, run_id, "dev-free", Some(1), 5, None).unwrap();
    }

    // Chọn cho task 3 (chưa có người cũ) -> dev-busy có điểm 5.0 thắng dev-free (điểm mặc định 3.0)
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level)
         VALUES (3, 1, 'task-3', 'Task 3', 'task/3', 'main', 'base1', 'open', 1000, 1000, 'vua')",
        [],
    )
    .unwrap();

    let picked_scored = pick_from(&conn, &cfg, &loaded, 3, "dev", Level::Vua, "work").unwrap().unwrap();
    assert_eq!(picked_scored.member.id, "dev-busy");
}

#[test]
fn resolve_for_run_and_plan_for_phong_ban_and_mac_dinh() {
    let _lock = ENV_LOCK.lock().unwrap();
    let (_tmp, conn) = test_db();
    let cfg = test_cfg_with_agy();

    // 1. Task với đội hình mac-dinh -> giữ nguyên hành vi, member = None
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level, team_json)
         VALUES (10, 1, 'task-10', 'Task 10', 'task/10', 'main', 'base1', 'open', 1000, 1000, 'kho', '{\"name\":\"mac-dinh\"}')",
        [],
    )
    .unwrap();

    let resolved_mac_dinh = resolve_for_run_with(&conn, Some(10), "dev", Some(&cfg)).unwrap();
    assert_eq!(resolved_mac_dinh.engine, Engine::Claude);
    assert!(resolved_mac_dinh.member.is_none());

    let plan_mac_dinh = plan_for(
        &cfg,
        &resolved_mac_dinh,
        "dev",
        &[
            "claude".to_string(),
            "-p".to_string(),
            "hello".to_string(),
            "--append-system-prompt".to_string(),
            "sys".to_string(),
        ],
        "claude-sonnet-5-5",
        std::collections::BTreeMap::new(),
        std::time::Duration::from_secs(60),
        std::path::Path::new("/tmp"),
    )
    .unwrap();
    assert_eq!(plan_mac_dinh.model_label, "claude-sonnet-5-5");

    // 2. Task với đội hình phong-ban
    let tmp_cfg = tempfile::tempdir().unwrap();
    let dept_file = tmp_cfg.path().join("departments.json");
    let dept_json = json!({
        "thi-cong": {
            "members": {
                "tc-opus": {
                    "display_name": "Thi công Opus",
                    "engine": "claude",
                    "model": "claude-opus-5-5",
                    "levels": ["kho"]
                },
                "tc-flash": {
                    "display_name": "Thi công Flash",
                    "engine": "agy",
                    "model": "gemini-3.8-flash-high",
                    "levels": ["de"]
                }
            }
        }
    })
    .to_string();
    std::fs::write(&dept_file, dept_json).unwrap();
    std::env::set_var("AGENT_BRAIN_CONFIG_DIR", tmp_cfg.path());

    // Task 11 mức kho -> chọn tc-opus (engine claude, model claude-opus-5-5)
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level, team_json)
         VALUES (11, 1, 'task-11', 'Task 11', 'task/11', 'main', 'base1', 'open', 1000, 1000, 'kho', '{\"name\":\"phong-ban\"}')",
        [],
    )
    .unwrap();

    let resolved_claude = resolve_for_run_with(&conn, Some(11), "dev", Some(&cfg)).unwrap();
    assert_eq!(resolved_claude.engine, Engine::Claude);
    assert_eq!(resolved_claude.member.as_ref().unwrap().member.id, "tc-opus");

    let plan_claude = plan_for(
        &cfg,
        &resolved_claude,
        "dev",
        &[
            "claude".to_string(),
            "-p".to_string(),
            "hello".to_string(),
            "--append-system-prompt".to_string(),
            "sys".to_string(),
        ],
        "claude-sonnet-5-5",
        std::collections::BTreeMap::new(),
        std::time::Duration::from_secs(60),
        std::path::Path::new("/tmp"),
    )
    .unwrap();
    assert_eq!(plan_claude.model_label, "claude-opus-5-5");
    assert!(plan_claude.argv.contains(&"--model".to_string()));
    let model_idx = plan_claude.argv.iter().position(|x| x == "--model").unwrap();
    assert_eq!(plan_claude.argv[model_idx + 1], "claude-opus-5-5");

    // Task 12 mức de -> chọn tc-flash (engine agy, model gemini-3.8-flash-high)
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level, team_json)
         VALUES (12, 1, 'task-12', 'Task 12', 'task/12', 'main', 'base1', 'open', 1000, 1000, 'de', '{\"name\":\"phong-ban\"}')",
        [],
    )
    .unwrap();

    let resolved_agy = resolve_for_run_with(&conn, Some(12), "dev", Some(&cfg)).unwrap();
    assert_eq!(resolved_agy.engine, Engine::Named("agy".to_string()));
    assert_eq!(resolved_agy.member.as_ref().unwrap().member.id, "tc-flash");

    let plan_agy = plan_for(
        &cfg,
        &resolved_agy,
        "dev",
        &[
            "claude".to_string(),
            "-p".to_string(),
            "hello".to_string(),
            "--append-system-prompt".to_string(),
            "sys".to_string(),
        ],
        "claude-sonnet-5-5",
        std::collections::BTreeMap::new(),
        std::time::Duration::from_secs(60),
        std::path::Path::new("/tmp"),
    )
    .unwrap();
    assert_eq!(plan_agy.model_label, "gemini-3.8-flash-high");
    assert!(plan_agy.argv.contains(&"--model".to_string()));
    let agy_model_idx = plan_agy.argv.iter().position(|x| x == "--model").unwrap();
    assert_eq!(plan_agy.argv[agy_model_idx + 1], "gemini-3.8-flash-high");
}

#[test]
fn cli_dept_check_and_list_and_pick() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let example_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/departments.example.json");
    let dept_file = tmp.path().join("departments.json");
    std::fs::copy(&example_path, &dept_file).unwrap();
    std::env::set_var("AGENT_BRAIN_CONFIG_DIR", tmp.path());

    let val = json!({
        "models": { "lead": "opus", "dev": "sonnet", "qa": "sonnet" },
        "profiles": { "work": {}, "personal": {} },
        "engines": [
            { "id": "agy", "kind": "agy" }
        ]
    });
    let cfg = agent_brain::config::config_from_value(&val).unwrap();
    let conn = agent_brain::db::open(&tmp.path().join("brain.db")).unwrap();
    conn.execute(
        "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
         VALUES (1, 'p1', '/path/1', 'Dự án 1', '[]', 'code', 'work', NULL, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level)
         VALUES (1, 1, 'task-1', 'Task 1', 'task/1', 'main', 'base1', 'open', 1000, 1000, 'kho')",
        [],
    )
    .unwrap();

    let ctx = agent_brain::app::Ctx {
        cfg,
        conn,
        map: json!({}),
        projects: vec![],
    };

    // cmd_check trên file mẫu phải thoát 0
    let check_res = agent_brain::departments::cmd_check(&ctx).unwrap();
    assert_eq!(check_res, 0);

    // cmd_list phải thoát 0
    let list_res = agent_brain::departments::cmd_list(&ctx).unwrap();
    assert_eq!(list_res, 0);

    // cmd_pick phải thoát 0 và chọn được người
    let pick_res = agent_brain::departments::cmd_pick(&ctx, 1, "dev").unwrap();
    assert_eq!(pick_res, 0);
}

#[test]
fn resolve_for_run_with_broken_config_has_no_member() {
    let _lock = ENV_LOCK.lock().unwrap();
    let (_tmp, conn) = test_db();
    let tmp_cfg = tempfile::tempdir().unwrap();
    let bad_cfg = tmp_cfg.path().join("config.json");
    std::fs::write(&bad_cfg, "{invalid json}").unwrap();
    std::env::set_var("AGENT_BRAIN_CONFIG", &bad_cfg);

    let dept_file = tmp_cfg.path().join("departments.json");
    let dept_json = json!({
        "thi-cong": {
            "members": {
                "tc-opus": {
                    "display_name": "Thi công Opus",
                    "engine": "claude",
                    "model": "claude-opus-5-5",
                    "levels": ["kho"]
                }
            }
        }
    })
    .to_string();
    std::fs::write(&dept_file, dept_json).unwrap();
    std::env::set_var("AGENT_BRAIN_CONFIG_DIR", tmp_cfg.path());

    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level, team_json)
         VALUES (99, 1, 'task-99', 'Task 99', 'task/99', 'main', 'base1', 'open', 1000, 1000, 'kho', '{\"name\":\"phong-ban\"}')",
        [],
    )
    .unwrap();

    let res = agent_brain::engine::resolve_for_run(&conn, Some(99), "dev").unwrap();
    assert_eq!(res.engine, Engine::Claude);
    assert!(res.member.is_none());

    std::env::remove_var("AGENT_BRAIN_CONFIG");
}

#[test]
fn prompts_departments_layer_and_builtins() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let cfg_file = tmp.path().join("config.json");
    std::fs::write(&cfg_file, agent_brain::config::DEFAULT_CONFIG_JSON).unwrap();
    std::env::set_var("AGENT_BRAIN_CONFIG", &cfg_file);

    // 1. Vai -> phòng tương ứng
    assert_eq!(dept_of_role("dev"), Some("thi-cong"));
    assert_eq!(dept_of_role("qa"), Some("qa"));

    // 2. Builtin file được đọc khi chưa có file người dùng
    let note_dev = agent_brain::prompts::read_note("departments", "thi-cong", "dev").unwrap();
    assert!(note_dev.is_some());
    assert!(note_dev.unwrap().contains("Chỉ sửa đổi các file đã được liệt kê trong plan"));

    // 3. File người dùng thắng file có sẵn
    let user_note_dir = tmp.path().join("prompts").join("departments").join("thi-cong");
    std::fs::create_dir_all(&user_note_dir).unwrap();
    std::fs::write(user_note_dir.join("dev.md"), "- Luật riêng từ người dùng cho dev\n").unwrap();

    let note_user = agent_brain::prompts::read_note("departments", "thi-cong", "dev").unwrap();
    assert_eq!(note_user.as_deref(), Some("- Luật riêng từ người dùng cho dev\n"));

    // 4. Tên phòng lạ -> lỗi API
    let err_dept = agent_brain::prompts::read_note("departments", "phong-la", "dev").unwrap_err();
    assert!(err_dept.contains("Phòng ban 'phong-la' không tồn tại"));

    // 5. Vai sai phòng -> lỗi API
    let err_role = agent_brain::prompts::read_note("departments", "thi-cong", "qa").unwrap_err();
    assert!(err_role.contains("Vai 'qa' không thuộc phòng 'thi-cong'"));

    // 6. Vai lead của dự án không có ghi chú vẫn có block "Phòng: Phòng kế hoạch" từ file có sẵn
    let base_lead = agent_brain::prompts::base_prompt_for("lead").unwrap();
    let layers = agent_brain::prompts::PromptLayers::default();
    let (composed, _, count) = agent_brain::prompts::compose_detailed(base_lead, "lead", &layers);
    assert_eq!(count, 1);
    assert!(composed.contains("### Phòng: Phòng kế hoạch"));
    assert!(composed.contains("Bản kế hoạch phải liệt kê chính xác các file cần sửa"));

    std::env::remove_var("AGENT_BRAIN_CONFIG");
}

#[test]
fn dept_skills_discovery_and_budget() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("AGENT_BRAIN_CONFIG_DIR", tmp.path());

    let skills_dir = tmp.path().join("departments").join("thi-cong").join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    // 1. Skill hợp lệ với frontmatter
    let s1 = skills_dir.join("clean-code");
    std::fs::create_dir_all(&s1).unwrap();
    std::fs::write(
        s1.join("SKILL.md"),
        "---\nname: Clean Code\ndescription: Playbook clean code\n---\nNội dung chi tiết clean code",
    )
    .unwrap();

    // 2. Skill > 64 KiB -> bị bỏ qua
    let s_big = skills_dir.join("too-big");
    std::fs::create_dir_all(&s_big).unwrap();
    std::fs::write(s_big.join("SKILL.md"), vec![b'x'; 65 * 1024]).unwrap();

    // 3. Symlink -> bị bỏ qua
    let outside_file = tmp.path().join("outside.md");
    std::fs::write(&outside_file, "outside").unwrap();
    let s_sym = skills_dir.join("sym-skill");
    std::fs::create_dir_all(&s_sym).unwrap();
    std::os::unix::fs::symlink(&outside_file, s_sym.join("SKILL.md")).unwrap();

    let skills = agent_brain::departments::dept_skills("thi-cong");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "Clean Code");
    assert_eq!(skills[0].description, "Playbook clean code");

    // 4. Giới hạn 24 KiB: skill còn lại chỉ đưa tên và mô tả, kèm cảnh báo
    let kh_skills_dir = tmp.path().join("departments").join("ke-hoach").join("skills");
    std::fs::create_dir_all(&kh_skills_dir).unwrap();

    let kh_s1 = kh_skills_dir.join("skill-1");
    std::fs::create_dir_all(&kh_s1).unwrap();
    let body_20k = "A".repeat(20 * 1024);
    std::fs::write(
        kh_s1.join("SKILL.md"),
        format!("---\nname: Skill 1\ndescription: Mô tả 1\n---\n{body_20k}"),
    )
    .unwrap();

    let kh_s2 = kh_skills_dir.join("skill-2");
    std::fs::create_dir_all(&kh_s2).unwrap();
    let body_10k = "B".repeat(10 * 1024);
    std::fs::write(
        kh_s2.join("SKILL.md"),
        format!("---\nname: Skill 2\ndescription: Mô tả 2\n---\n{body_10k}"),
    )
    .unwrap();

    let (formatted, warnings) = agent_brain::departments::format_dept_skills("ke-hoach");
    assert!(formatted.contains("## Skill của phòng Phòng kế hoạch (playbook dùng chung — bổ sung, không mở rộng quyền)"));
    assert!(formatted.contains("### Skill: Skill 1"));
    assert!(formatted.contains("- Skill 2: Mô tả 2"));
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("vượt quá giới hạn 24 KiB"));

    std::env::remove_var("AGENT_BRAIN_CONFIG_DIR");
}

#[test]
fn member_rules_validation() {
    let cfg = test_cfg_with_agy();

    // 1. Rules > 1000 ký tự -> lỗi
    let long_rules = "a".repeat(1001);
    let dept_json_bad = json!({
        "thi-cong": {
            "members": {
                "tc-long": {
                    "display_name": "Thi công Long",
                    "engine": "claude",
                    "model": "claude-sonnet-5-5",
                    "levels": ["vua"],
                    "rules": long_rules
                }
            }
        }
    })
    .to_string();

    let loaded_bad = load_from(&dept_json_bad, &cfg);
    assert!(!loaded_bad.errors.is_empty());
    assert!(loaded_bad.errors[0].contains("Rules riêng quá dài (tối đa 1000 ký tự)"));

    // 2. Rules hợp lệ <= 1000 ký tự -> nạp thành công
    let dept_json_good = json!({
        "thi-cong": {
            "members": {
                "tc-good": {
                    "display_name": "Thi công Good",
                    "engine": "claude",
                    "model": "claude-sonnet-5-5",
                    "levels": ["vua"],
                    "rules": "Không refactor ngoài phạm vi"
                }
            }
        }
    })
    .to_string();

    let loaded_good = load_from(&dept_json_good, &cfg);
    assert!(loaded_good.errors.is_empty());
    let m = loaded_good.get("tc-good").unwrap();
    assert_eq!(m.rules.as_deref(), Some("Không refactor ngoài phạm vi"));
}

#[test]
fn execute_seam_phong_ban_and_mac_dinh() {
    let _lock = ENV_LOCK.lock().unwrap();
    let (_tmp, conn) = test_db();
    let cfg = test_cfg_with_agy();

    let tmp_cfg = tempfile::tempdir().unwrap();
    let dept_file = tmp_cfg.path().join("departments.json");
    let dept_json = json!({
        "thi-cong": {
            "members": {
                "tc-claude": {
                    "display_name": "Thi công Claude",
                    "engine": "claude",
                    "model": "claude-opus-5-5",
                    "levels": ["kho"],
                    "rules": "Claude: tuân thủ nghiêm ngặt plan"
                },
                "tc-agy": {
                    "display_name": "Thi công Agy",
                    "engine": "agy",
                    "model": "gemini-3.8-flash-high",
                    "levels": ["de"],
                    "rules": "Agy: không sửa dependency"
                }
            }
        }
    })
    .to_string();
    std::fs::write(&dept_file, dept_json).unwrap();

    let skills_dir = tmp_cfg.path().join("departments").join("thi-cong").join("skills").join("checklist");
    std::fs::create_dir_all(&skills_dir).unwrap();
    std::fs::write(
        skills_dir.join("SKILL.md"),
        "---\nname: Checklist\ndescription: Kiểm tra kỹ\n---\nNội dung playbook thi công",
    )
    .unwrap();

    std::env::set_var("AGENT_BRAIN_CONFIG_DIR", tmp_cfg.path());

    // 1. Task phong-ban mức kho -> tc-claude (có rules và skill)
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level, team_json)
         VALUES (21, 1, 'task-21', 'Task 21', 'task/21', 'main', 'base1', 'open', 1000, 1000, 'kho', '{\"name\":\"phong-ban\"}')",
        [],
    )
    .unwrap();

    let resolved_claude = resolve_for_run_with(&conn, Some(21), "dev", Some(&cfg)).unwrap();
    assert_eq!(resolved_claude.engine, Engine::Claude);

    let (extra_claude, _) = agent_brain::departments::prompt_append_for_member(
        "dev",
        resolved_claude.member.as_ref().map(|p| &p.member),
    );
    let mut argv_claude = vec![
        "claude".to_string(),
        "-p".to_string(),
        "hello".to_string(),
        "--append-system-prompt".to_string(),
        "sys base".to_string(),
    ];
    agent_brain::engine::append_to_system_prompt(&mut argv_claude, &extra_claude);

    let plan_claude = plan_for(
        &cfg,
        &resolved_claude,
        "dev",
        &argv_claude,
        "claude-sonnet-5-5",
        std::collections::BTreeMap::new(),
        std::time::Duration::from_secs(60),
        std::path::Path::new("/tmp"),
    )
    .unwrap();

    assert!(plan_claude.argv.iter().any(|arg| arg.contains("## Skill của phòng Phòng thi công")));
    assert!(plan_claude.argv.iter().any(|arg| arg.contains("Nội dung playbook thi công")));
    assert!(plan_claude.argv.iter().any(|arg| arg.contains("## Rules riêng của thành viên Thi công Claude")));
    assert!(plan_claude.argv.iter().any(|arg| arg.contains("Claude: tuân thủ nghiêm ngặt plan")));

    // 2. Task phong-ban mức de -> tc-agy (có rules và skill)
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level, team_json)
         VALUES (22, 1, 'task-22', 'Task 22', 'task/22', 'main', 'base1', 'open', 1000, 1000, 'de', '{\"name\":\"phong-ban\"}')",
        [],
    )
    .unwrap();

    let resolved_agy = resolve_for_run_with(&conn, Some(22), "dev", Some(&cfg)).unwrap();
    assert_eq!(resolved_agy.engine, Engine::Named("agy".to_string()));

    let (extra_agy, _) = agent_brain::departments::prompt_append_for_member(
        "dev",
        resolved_agy.member.as_ref().map(|p| &p.member),
    );
    let mut argv_agy = vec![
        "claude".to_string(),
        "-p".to_string(),
        "hello".to_string(),
        "--append-system-prompt".to_string(),
        "sys base".to_string(),
    ];
    agent_brain::engine::append_to_system_prompt(&mut argv_agy, &extra_agy);

    let plan_agy = plan_for(
        &cfg,
        &resolved_agy,
        "dev",
        &argv_agy,
        "claude-sonnet-5-5",
        std::collections::BTreeMap::new(),
        std::time::Duration::from_secs(60),
        std::path::Path::new("/tmp"),
    )
    .unwrap();

    let agy_prompt_pos = plan_agy.argv.iter().position(|a| a == "-p").unwrap();
    let agy_prompt = &plan_agy.argv[agy_prompt_pos + 1];
    assert!(agy_prompt.contains("## Skill của phòng Phòng thi công"));
    assert!(agy_prompt.contains("Nội dung playbook thi công"));
    assert!(agy_prompt.contains("## Rules riêng của thành viên Thi công Agy"));
    assert!(agy_prompt.contains("Agy: không sửa dependency"));

    // 3. Task mac-dinh: có skill của phòng nhưng KHÔNG có rules thành viên
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level, team_json)
         VALUES (23, 1, 'task-23', 'Task 23', 'task/23', 'main', 'base1', 'open', 1000, 1000, 'kho', '{\"name\":\"mac-dinh\"}')",
        [],
    )
    .unwrap();

    let resolved_mac_dinh = resolve_for_run_with(&conn, Some(23), "dev", Some(&cfg)).unwrap();
    assert!(resolved_mac_dinh.member.is_none());

    let (extra_mac_dinh, _) = agent_brain::departments::prompt_append_for_member(
        "dev",
        resolved_mac_dinh.member.as_ref().map(|p| &p.member),
    );
    let mut argv_mac_dinh = vec![
        "claude".to_string(),
        "-p".to_string(),
        "hello".to_string(),
        "--append-system-prompt".to_string(),
        "sys base".to_string(),
    ];
    agent_brain::engine::append_to_system_prompt(&mut argv_mac_dinh, &extra_mac_dinh);

    let plan_mac_dinh = plan_for(
        &cfg,
        &resolved_mac_dinh,
        "dev",
        &argv_mac_dinh,
        "claude-sonnet-5-5",
        std::collections::BTreeMap::new(),
        std::time::Duration::from_secs(60),
        std::path::Path::new("/tmp"),
    )
    .unwrap();

    assert!(plan_mac_dinh.argv.iter().any(|arg| arg.contains("## Skill của phòng Phòng thi công")));
    assert!(!plan_mac_dinh.argv.iter().any(|arg| arg.contains("## Rules riêng của thành viên")));

    std::env::remove_var("AGENT_BRAIN_CONFIG_DIR");
}

#[test]
fn project_skills_in_qa_and_analyst() {
    let tmp = tempfile::tempdir().unwrap();
    let skill_dir = tmp.path().join(".claude").join("skills").join("my-tool");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: My Tool\ndescription: Công cụ kiểm tra tự động\n---\nScript hướng dẫn",
    )
    .unwrap();

    // 1. Analyst prompt
    let analyst_input = agent_brain::roles::analyst::build_analyst_user_input(tmp.path(), "cần phân tích gì", None);
    assert!(analyst_input.contains("## Skill có sẵn trong dự án"));
    assert!(analyst_input.contains("My Tool"));
    assert!(analyst_input.contains("Công cụ kiểm tra tự động"));

    // 2. QA prompt
    let skills = agent_brain::skills::discover(tmp.path());
    let qa_skill_block = agent_brain::skills::prompt_block(&skills);
    assert!(qa_skill_block.contains("## Skill có sẵn trong dự án"));
    assert!(qa_skill_block.contains("My Tool"));
}
