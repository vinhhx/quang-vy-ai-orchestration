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

// ---- 06-13: QA "Cần sửa" rate by the Dev engine under test (ENG-07 / D-23) ----

const HUMAN_ENGINE: &str = "ng\u{01b0}\u{1edd}i";

struct QaSeed {
    conn: rusqlite::Connection,
    proj_id: i64,
}

impl QaSeed {
    fn new(sb: &Sandbox) -> QaSeed {
        let repo = sb.make_repo("proj-a", &[], None, None);
        let entry = sb.project_entry(&repo, "proj-a", "work");
        sb.write_map(serde_json::json!([entry]), &[]);
        let conn = agent_brain::db::open(&sb.db_path).expect("open db");
        let p = agent_brain::projects::Project {
            id: "proj-a".to_string(),
            path: repo.to_string_lossy().to_string(),
            name_vi: "proj-a".to_string(),
            aliases: vec![],
            kind: "code".to_string(),
            profile: "work".to_string(),
            repo: None,
            missing: false,
        };
        agent_brain::db::sync_projects(&conn, &[p]).expect("sync projects");
        let proj_id = agent_brain::db::project_row_id(&conn, "proj-a").expect("proj id");
        QaSeed { conn, proj_id }
    }

    fn task(&self, n: i64) -> i64 {
        agent_brain::db::upsert_task(
            &self.conn,
            self.proj_id,
            &format!("t{n}"),
            &format!("Task {n}"),
            &format!("task/{n}"),
            Some("main"),
            Some("base1"),
        )
        .expect("task")
    }

    /// A finished run of `task_id`; `started_ms_ago` moves its start into the past.
    fn run(
        &self,
        task_id: i64,
        role: &str,
        engine: &str,
        outcome: Option<&str>,
        status: &str,
        started_ms_ago: i64,
    ) -> i64 {
        let id = agent_brain::db::insert_run_with_engine(
            &self.conn,
            &agent_brain::db::NewRun {
                task_id: Some(task_id),
                project_id: self.proj_id,
                role: role.to_string(),
                profile: "work".to_string(),
                model: Some("sonnet".to_string()),
                pid: 1000,
                pgid: 1000,
                pid_start: 1000,
                base_commit: None,
                run_dir: None,
            },
            engine,
        )
        .expect("insert run");
        agent_brain::db::finish_run(
            &self.conn,
            id,
            &agent_brain::db::RunFinish {
                status: status.to_string(),
                exit_code: Some(0),
                session_id: None,
                usage: agent_brain::stream::Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                    cache_read_input_tokens: 0,
                    cache_creation_input_tokens: 0,
                },
                usage_source: "result".to_string(),
                cost_usd: 0.0,
                num_turns: Some(1),
                assistant_turns: Some(1),
                turns_before_first_edit: None,
                reads_outside_plan: None,
                permission_denials: 0,
                error: None,
            },
        )
        .expect("finish run");
        if let Some(o) = outcome {
            agent_brain::db::set_run_outcome(&self.conn, id, o).expect("outcome");
        }
        self.conn
            .execute(
                "UPDATE runs SET started_at = ?1 WHERE id = ?2",
                rusqlite::params![agent_brain::db::now_ms() - started_ms_ago, id],
            )
            .expect("started_at");
        id
    }

    fn human_dev(&self, task_id: i64) {
        let now = agent_brain::db::now_ms();
        agent_brain::db::insert_human_run(
            &self.conn,
            &agent_brain::db::HumanRun {
                task_id,
                project_id: self.proj_id,
                role: "dev".to_string(),
                profile: "work".to_string(),
                stage: Some("implement".to_string()),
                round: Some(1),
                started_at: now - 5_000,
                ended_at: now - 4_000,
                outcome: None,
                base_commit: None,
                metrics_json: "{\"staff\":\"ha\"}".to_string(),
            },
        )
        .expect("human run");
    }
}

fn rate_row(engine: &str, qa_fix: i64, qa_total: i64) -> agent_brain::stats::QaRate {
    agent_brain::stats::QaRate {
        engine: engine.to_string(),
        qa_fix,
        qa_total,
    }
}

const DAY_MS: i64 = 86_400_000;

#[test]
fn eng07_qa_rate_by_dev_engine() {
    let sb = Sandbox::new();
    let seed = QaSeed::new(&sb);

    // A: dev(deepseek) -> qa:fix -> dev(deepseek, fix) -> qa:pass
    let a = seed.task(1);
    seed.run(a, "dev", "deepseek", None, "succeeded", 9_000);
    seed.run(a, "qa", "claude", Some("qa:fix"), "succeeded", 3 * DAY_MS);
    seed.run(a, "dev", "deepseek", None, "succeeded", 7_000);
    seed.run(a, "qa", "claude", Some("qa:pass"), "succeeded", 6_000);
    // B: dev(claude) -> qa:pass
    let b = seed.task(2);
    seed.run(b, "dev", "claude", None, "succeeded", 9_000);
    seed.run(b, "qa", "claude", Some("qa:pass"), "succeeded", 6_000);
    // C: a person's dev run -> qa:fix
    let c = seed.task(3);
    seed.human_dev(c);
    seed.run(c, "qa", "claude", Some("qa:fix"), "succeeded", 3_000);
    // D: a QA verdict with no earlier Dev run is ignored
    let d = seed.task(4);
    seed.run(d, "qa", "claude", Some("qa:pass"), "succeeded", 3_000);
    // E: dev(agy) -> qa killed (no outcome) -> no row for agy
    let e = seed.task(5);
    seed.run(e, "dev", "agy", None, "succeeded", 9_000);
    seed.run(e, "qa", "claude", None, "killed", 6_000);

    let rows = agent_brain::stats::query_qa_by_dev_engine(&seed.conn, None, None).unwrap();
    assert_eq!(
        rows,
        vec![
            rate_row("claude", 0, 1),
            rate_row("deepseek", 1, 2),
            rate_row(HUMAN_ENGINE, 1, 1),
        ]
    );

    let mut srv = sb.serve(&[]);
    let j = srv.get("/api/stats").json();
    assert_eq!(
        j["qa_by_dev_engine"],
        serde_json::json!([
            {"engine": "claude", "qa_fix": 0, "qa_total": 1},
            {"engine": "deepseek", "qa_fix": 1, "qa_total": 2},
            {"engine": HUMAN_ENGINE, "qa_fix": 1, "qa_total": 1},
        ])
    );
    // Integer counts only: no float rate, every value a JSON integer.
    for row in j["qa_by_dev_engine"].as_array().unwrap() {
        assert_eq!(row.as_object().unwrap().len(), 3);
        assert!(row["qa_fix"].is_i64() && row["qa_total"].is_i64());
    }
    // The existing stats fields are still there.
    assert!(j["total"]["runs"].as_i64().unwrap() > 0);
    assert!(j["engines"].is_array());

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn eng07_rate_boundaries_and_filters() {
    let sb = Sandbox::new();
    let seed = QaSeed::new(&sb);

    // only qa:pass -> 0 % ; only qa:fix -> 100 % ; other outcomes are not counted
    let p = seed.task(1);
    seed.run(p, "dev", "claude", None, "succeeded", 9_000);
    seed.run(p, "qa", "claude", Some("qa:pass"), "succeeded", 6_000);
    let f = seed.task(2);
    seed.run(f, "dev", "deepseek", None, "succeeded", 9_000);
    seed.run(f, "qa", "claude", Some("qa:fix"), "succeeded", 6_000);
    let o = seed.task(3);
    seed.run(o, "dev", "agy", None, "succeeded", 9_000);
    seed.run(o, "qa", "claude", Some("killed"), "killed", 6_000);
    seed.run(o, "qa", "claude", Some("failed"), "failed", 5_000);
    seed.run(
        o,
        "qa",
        "claude",
        Some("protocol_violation"),
        "failed",
        4_000,
    );

    let rows = agent_brain::stats::query_qa_by_dev_engine(&seed.conn, None, None).unwrap();
    assert_eq!(
        rows,
        vec![rate_row("claude", 0, 1), rate_row("deepseek", 1, 1)]
    );
    // agy has no verdict, so no row (never qa_total: 0)
    assert!(rows.iter().all(|r| r.engine != "agy" && r.qa_total > 0));

    // since: only verdicts started at or after the cutoff count
    let cutoff = agent_brain::db::now_ms() - 3_000;
    let recent =
        agent_brain::stats::query_qa_by_dev_engine(&seed.conn, None, Some(cutoff)).unwrap();
    assert!(
        recent.is_empty(),
        "the verdicts started 6 s ago: {recent:?}"
    );

    // a window that covers the verdicts keeps both rows
    let wide = agent_brain::stats::query_qa_by_dev_engine(
        &seed.conn,
        None,
        Some(agent_brain::db::now_ms() - 30_000),
    )
    .unwrap();
    assert_eq!(wide.len(), 2);

    // profile: nothing for another profile
    assert!(
        agent_brain::stats::query_qa_by_dev_engine(&seed.conn, Some("personal"), None)
            .unwrap()
            .is_empty()
    );

    // over HTTP: ?since after the QA runs and ?profile=personal give empty lists
    let mut srv = sb.serve(&[]);
    let all = srv.get("/api/stats").json();
    assert_eq!(all["qa_by_dev_engine"].as_array().unwrap().len(), 2);
    let personal = srv.get("/api/stats?profile=personal").json();
    assert_eq!(personal["qa_by_dev_engine"], serde_json::json!([]));
    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));

    // ?since over HTTP: an old QA verdict falls out of the window
    let sb2 = Sandbox::new();
    let seed2 = QaSeed::new(&sb2);
    let a = seed2.task(1);
    seed2.run(a, "dev", "deepseek", None, "succeeded", 4 * DAY_MS);
    seed2.run(a, "qa", "claude", Some("qa:fix"), "succeeded", 3 * DAY_MS);
    seed2.run(a, "dev", "deepseek", None, "succeeded", 7_000);
    seed2.run(a, "qa", "claude", Some("qa:pass"), "succeeded", 6_000);
    let mut srv2 = sb2.serve(&[]);
    let all = srv2.get("/api/stats").json();
    assert_eq!(
        all["qa_by_dev_engine"],
        serde_json::json!([{"engine": "deepseek", "qa_fix": 1, "qa_total": 2}])
    );
    let recent = srv2.get("/api/stats?since=1d").json();
    assert_eq!(
        recent["qa_by_dev_engine"],
        serde_json::json!([{"engine": "deepseek", "qa_fix": 0, "qa_total": 1}])
    );
    let exit = srv2.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}
