mod common;

use common::Sandbox;

fn seed_db(sb: &Sandbox) {
    let repo_a = sb.make_repo("proj-a", &[], None, None);
    let repo_b = sb.make_repo("proj-b", &[], None, None);
    let entry_a = sb.project_entry(&repo_a, "proj-a", "work");
    let entry_b = sb.project_entry(&repo_b, "proj-b", "personal");
    sb.write_map(serde_json::json!([entry_a, entry_b]), &[]);

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let p_a = agent_brain::projects::Project {
        id: "proj-a".to_string(),
        path: repo_a.to_string_lossy().to_string(),
        name_vi: "proj-a".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    let p_b = agent_brain::projects::Project {
        id: "proj-b".to_string(),
        path: repo_b.to_string_lossy().to_string(),
        name_vi: "proj-b".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "personal".to_string(),
        repo: None,
        missing: false,
    };
    agent_brain::db::sync_projects(&conn, &[p_a, p_b]).expect("sync projects");

    let id_a = agent_brain::db::project_row_id(&conn, "proj-a").expect("id_a");
    let id_b = agent_brain::db::project_row_id(&conn, "proj-b").expect("id_b");

    let insert_run = |proj_id: i64,
                      role: &str,
                      profile: &str,
                      in_tok: i64,
                      out_tok: i64,
                      cache_read: i64,
                      cache_write: i64,
                      cost: f64,
                      days_ago: Option<i64>| {
        let run_id = agent_brain::db::insert_run(
            &conn,
            &agent_brain::db::NewRun {
                task_id: None,
                project_id: proj_id,
                role: role.to_string(),
                profile: profile.to_string(),
                model: Some("sonnet".to_string()),
                pid: 1000,
                pgid: 1000,
                pid_start: 1000,
                base_commit: None,
                run_dir: None,
            },
        )
        .expect("insert run");

        agent_brain::db::finish_run(
            &conn,
            run_id,
            &agent_brain::db::RunFinish {
                status: "succeeded".to_string(),
                exit_code: Some(0),
                session_id: Some("00000000-0000-0000-0000-000000000001".to_string()),
                usage: agent_brain::stream::Usage {
                    input_tokens: in_tok,
                    output_tokens: out_tok,
                    cache_read_input_tokens: cache_read,
                    cache_creation_input_tokens: cache_write,
                },
                usage_source: "result".to_string(),
                cost_usd: cost,
                num_turns: Some(1),
                assistant_turns: Some(1),
                turns_before_first_edit: None,
                reads_outside_plan: None,
                permission_denials: 0,
                error: None,
            },
        )
        .expect("finish run");

        if let Some(days) = days_ago {
            let past_ms = agent_brain::db::now_ms() - (days * 86_400_000);
            conn.execute(
                "UPDATE runs SET started_at = ?1 WHERE id = ?2",
                rusqlite::params![past_ms, run_id],
            )
            .expect("update started_at");
        }
    };

    // work/proj-a/lead 100/10/1000/50 cost 0.1
    insert_run(id_a, "lead", "work", 100, 10, 1000, 50, 0.1, None);
    // work/proj-a/dev 200/20/2000/60 cost 0.2
    insert_run(id_a, "dev", "work", 200, 20, 2000, 60, 0.2, None);
    // work/proj-a/inspect 5/5/0/0 cost 0.01
    insert_run(id_a, "inspect", "work", 5, 5, 0, 0, 0.01, None);
    // personal/proj-b/qa 7/7/7/7 cost 0.7
    insert_run(id_b, "qa", "personal", 7, 7, 7, 7, 0.7, None);
    // work/proj-a/lead 1/1/1/1 cost 0.001 whose started_at is then set to 10 days ago
    insert_run(id_a, "lead", "work", 1, 1, 1, 1, 0.001, Some(10));
}

#[test]
fn p2_08_stats_profile_filter() {
    let sb = Sandbox::new();
    seed_db(&sb);

    let out = sb.run(&["stats", "--profile", "work", "--json"], None);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let val: serde_json::Value = serde_json::from_str(&out.stdout).expect("parse json");
    assert_eq!(val["total"]["runs"], 4);
    assert_eq!(val["total"]["input_tokens"], 306);
    assert_eq!(val["total"]["output_tokens"], 36);
    assert_eq!(val["total"]["cache_read_tokens"], 3001);
    assert_eq!(val["total"]["cache_write_tokens"], 111);
    let cost = val["total"]["cost_usd"].as_f64().expect("cost_usd");
    assert!((cost - 0.311).abs() < 1e-9, "cost was {cost}");

    let by_proj = val["by_project"].as_array().expect("by_project array");
    assert_eq!(by_proj.len(), 1);
    assert_eq!(by_proj[0]["project"], "proj-a");
    assert_eq!(by_proj[0]["runs"], 4);

    let by_role = val["by_role"].as_array().expect("by_role array");
    let roles: Vec<&str> = by_role
        .iter()
        .map(|r| r["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, vec!["dev", "inspect", "lead"]);

    let str_val = out.stdout;
    assert!(!str_val.contains("personal"));
    assert!(!str_val.contains("proj-b"));
}

#[test]
fn stats_since_window() {
    let sb = Sandbox::new();
    seed_db(&sb);

    let out = sb.run(
        &["stats", "--profile", "work", "--since", "7d", "--json"],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let val: serde_json::Value = serde_json::from_str(&out.stdout).expect("parse json");
    assert_eq!(val["total"]["runs"], 3);
    assert_eq!(val["total"]["input_tokens"], 305);
}

#[test]
fn stats_all_profiles() {
    let sb = Sandbox::new();
    seed_db(&sb);

    let out = sb.run(&["stats", "--json"], None);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let val: serde_json::Value = serde_json::from_str(&out.stdout).expect("parse json");
    assert_eq!(val["total"]["runs"], 5);
    let by_proj = val["by_project"].as_array().expect("by_project array");
    let projs: Vec<&str> = by_proj
        .iter()
        .map(|p| p["project"].as_str().unwrap())
        .collect();
    assert_eq!(projs, vec!["proj-a", "proj-b"]);
}

#[test]
fn stats_table_labels_cost_equivalent() {
    let sb = Sandbox::new();
    seed_db(&sb);

    let out = sb.run(&["stats", "--profile", "work"], None);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert!(
        out.stdout.contains("tương đương"),
        "missing tương đương in stdout: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("proj-a"),
        "missing proj-a in stdout: {}",
        out.stdout
    );
}

#[test]
fn stats_rejects_bad_since_and_profile() {
    let sb = Sandbox::new();
    seed_db(&sb);

    let out_since = sb.run(&["stats", "--since", "7x"], None);
    assert_eq!(out_since.code, 1);
    assert!(
        out_since.stderr.contains("Khoảng thời gian không hợp lệ"),
        "stderr was: {}",
        out_since.stderr
    );

    let out_prof = sb.run(&["stats", "--profile", "foo"], None);
    assert_eq!(out_prof.code, 1);
    assert!(
        out_prof.stderr.contains("không hợp lệ"),
        "stderr was: {}",
        out_prof.stderr
    );
}

#[test]
fn stats_empty_db_zeroes() {
    let sb = Sandbox::new();
    let out = sb.run(&["stats", "--json"], None);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let val: serde_json::Value = serde_json::from_str(&out.stdout).expect("parse json");
    assert_eq!(val["total"]["runs"], 0);
    assert_eq!(val["total"]["cost_usd"], 0.0);
}
