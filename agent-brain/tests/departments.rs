use agent_brain::config::Config;
use agent_brain::departments::{
    dept_name_of, dept_of_role, load_from, model_family, pick_from, Level,
};
use rusqlite::Connection;
use serde_json::json;

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
