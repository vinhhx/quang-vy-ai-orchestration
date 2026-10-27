mod common;

use common::Sandbox;

fn seed_stats_data(sb: &Sandbox) -> rusqlite::Connection {
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
                      started_at: Option<i64>| {
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

        if let Some(ts) = started_at {
            conn.execute(
                "UPDATE runs SET started_at = ?1 WHERE id = ?2",
                rusqlite::params![ts, run_id],
            )
            .expect("update started_at");
        }
    };

    let now = agent_brain::db::now_ms();
    let three_days_ago = now - 3 * 86_400_000;

    // work / proj-a / lead
    insert_run(id_a, "lead", "work", 100, 10, 1000, 50, 0.1, Some(now));
    // work / proj-a / dev
    insert_run(id_a, "dev", "work", 200, 20, 2000, 60, 0.2, Some(now));
    // work / proj-a / inspect
    insert_run(id_a, "inspect", "work", 5, 5, 0, 0, 0.01, Some(now));
    // personal / proj-b / qa
    insert_run(id_b, "qa", "personal", 7, 7, 7, 7, 0.7, Some(now));
    // work / proj-a / lead (3 days ago)
    insert_run(
        id_a,
        "lead",
        "work",
        50,
        5,
        500,
        25,
        0.05,
        Some(three_days_ago),
    );

    conn
}

#[test]
fn day_key_midnight() {
    let tz = chrono::FixedOffset::east_opt(7 * 3600).expect("FixedOffset +07:00");
    let t1 = chrono::DateTime::parse_from_rfc3339("2026-10-16T16:59:00Z")
        .expect("t1 parse")
        .timestamp_millis();
    let t2 = chrono::DateTime::parse_from_rfc3339("2026-10-16T17:01:00Z")
        .expect("t2 parse")
        .timestamp_millis();

    assert_eq!(agent_brain::stats::day_key(t1, &tz), "2026-10-16");
    assert_eq!(agent_brain::stats::day_key(t2, &tz), "2026-10-17");
}

#[test]
fn query_by_day_in_midnight() {
    let sb = Sandbox::new();
    let repo_a = sb.make_repo("proj-a", &[], None, None);
    let entry_a = sb.project_entry(&repo_a, "proj-a", "work");
    sb.write_map(serde_json::json!([entry_a]), &[]);

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
    agent_brain::db::sync_projects(&conn, &[p_a]).expect("sync projects");
    let id_a = agent_brain::db::project_row_id(&conn, "proj-a").expect("id_a");

    let t1 = chrono::DateTime::parse_from_rfc3339("2026-10-16T16:59:00Z")
        .expect("t1 parse")
        .timestamp_millis();
    let t2 = chrono::DateTime::parse_from_rfc3339("2026-10-16T17:01:00Z")
        .expect("t2 parse")
        .timestamp_millis();

    let insert_run = |in_tok: i64, cost: f64, ts: i64| {
        let run_id = agent_brain::db::insert_run(
            &conn,
            &agent_brain::db::NewRun {
                task_id: None,
                project_id: id_a,
                role: "dev".to_string(),
                profile: "work".to_string(),
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
                    output_tokens: 10,
                    cache_read_input_tokens: 100,
                    cache_creation_input_tokens: 20,
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

        conn.execute(
            "UPDATE runs SET started_at = ?1 WHERE id = ?2",
            rusqlite::params![ts, run_id],
        )
        .expect("update started_at");
    };

    insert_run(111, 0.11, t1);
    insert_run(222, 0.22, t2);

    let tz = chrono::FixedOffset::east_opt(7 * 3600).expect("tz +07:00");
    let days =
        agent_brain::stats::query_by_day_in(&conn, None, None, &tz).expect("query_by_day_in");

    assert_eq!(days.len(), 2);
    assert_eq!(days[0].day, "2026-10-16");
    assert_eq!(days[0].totals.runs, 1);
    assert_eq!(days[0].totals.input_tokens, 111);
    assert!((days[0].totals.cost_usd - 0.11).abs() < 1e-6);

    assert_eq!(days[1].day, "2026-10-17");
    assert_eq!(days[1].totals.runs, 1);
    assert_eq!(days[1].totals.input_tokens, 222);
    assert!((days[1].totals.cost_usd - 0.22).abs() < 1e-6);
}

#[test]
fn query_by_day_and_profile_filters() {
    let sb = Sandbox::new();
    let conn = seed_stats_data(&sb);

    // query_by_day with profile Some("work") excludes personal runs
    let work_days = agent_brain::stats::query_by_day(&conn, Some("work"), None).expect("work days");
    let total_work_runs: i64 = work_days.iter().map(|d| d.totals.runs).sum();
    assert_eq!(total_work_runs, 4);

    let personal_days =
        agent_brain::stats::query_by_day(&conn, Some("personal"), None).expect("personal days");
    let total_personal_runs: i64 = personal_days.iter().map(|d| d.totals.runs).sum();
    assert_eq!(total_personal_runs, 1);

    // query_by_day with since_ms excludes older runs (run 3 days ago excluded)
    let since_ms = agent_brain::db::now_ms() - 86_400_000;
    let recent_days =
        agent_brain::stats::query_by_day(&conn, None, Some(since_ms)).expect("recent days");
    let total_recent_runs: i64 = recent_days.iter().map(|d| d.totals.runs).sum();
    assert_eq!(total_recent_runs, 4);

    // query_by_profile with no filter -> one row per profile sorted by name
    let profiles = agent_brain::stats::query_by_profile(&conn, None, None).expect("by profile");
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].profile, "personal");
    assert_eq!(profiles[0].totals.runs, 1);
    assert_eq!(profiles[0].totals.input_tokens, 7);
    assert_eq!(profiles[1].profile, "work");
    assert_eq!(profiles[1].totals.runs, 4);
    assert_eq!(profiles[1].totals.input_tokens, 355);

    // totals add up to query_report's total
    let report = agent_brain::stats::query_report(&conn, None, None, None).expect("report");
    let sum_prof_runs: i64 = profiles.iter().map(|p| p.totals.runs).sum();
    assert_eq!(sum_prof_runs, report.total.runs);
    let sum_prof_in: i64 = profiles.iter().map(|p| p.totals.input_tokens).sum();
    assert_eq!(sum_prof_in, report.total.input_tokens);

    // sum of by_day runs equals query_report total.runs for the same filters
    let all_days = agent_brain::stats::query_by_day(&conn, None, None).expect("all days");
    let sum_day_runs: i64 = all_days.iter().map(|d| d.totals.runs).sum();
    assert_eq!(sum_day_runs, report.total.runs);

    let report_work =
        agent_brain::stats::query_report(&conn, Some("work"), None, None).expect("report work");
    assert_eq!(total_work_runs, report_work.total.runs);

    let report_recent =
        agent_brain::stats::query_report(&conn, None, None, Some(since_ms)).expect("report recent");
    assert_eq!(total_recent_runs, report_recent.total.runs);
}

#[test]
fn dash11_stats_breakdowns() {
    let sb = Sandbox::new();
    let _conn = seed_stats_data(&sb);
    let mut srv = sb.serve(&[]);

    let r = srv.get("/api/stats");
    assert_eq!(r.status, 200);
    let j = r.json();

    assert_eq!(j["total"]["runs"], 5);
    assert_eq!(j["total"]["input_tokens"], 362);
    assert_eq!(j["total"]["output_tokens"], 47);
    assert_eq!(j["total"]["cache_read_tokens"], 3507);
    assert_eq!(j["total"]["cache_write_tokens"], 142);
    assert!((j["total"]["cost_usd"].as_f64().unwrap() - 1.06).abs() < 1e-4);

    let by_project = j["by_project"].as_array().expect("by_project array");
    assert_eq!(by_project.len(), 2);
    assert_eq!(by_project[0]["project"], "proj-a");
    assert_eq!(by_project[0]["runs"], 4);
    assert_eq!(by_project[1]["project"], "proj-b");
    assert_eq!(by_project[1]["runs"], 1);

    let by_role = j["by_role"].as_array().expect("by_role array");
    assert_eq!(by_role.len(), 4);

    let by_day = j["by_day"].as_array().expect("by_day array");
    assert_eq!(by_day.len(), 2);
    assert!(by_day[0]["runs"].as_i64().unwrap() >= 1);
    assert!(by_day[1]["runs"].as_i64().unwrap() >= 1);
    let sum_day_runs: i64 = by_day.iter().map(|d| d["runs"].as_i64().unwrap()).sum();
    assert_eq!(sum_day_runs, 5);

    let by_profile = j["by_profile"].as_array().expect("by_profile array");
    assert_eq!(by_profile.len(), 2);
    assert_eq!(by_profile[0]["profile"], "personal");
    assert_eq!(by_profile[0]["runs"], 1);
    assert_eq!(by_profile[1]["profile"], "work");
    assert_eq!(by_profile[1]["runs"], 4);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn dash08_stats_profile_filter() {
    let sb = Sandbox::new();
    let _conn = seed_stats_data(&sb);
    let mut srv = sb.serve(&[]);

    // ?profile=work
    let r_work = srv.get("/api/stats?profile=work");
    assert_eq!(r_work.status, 200);
    let j_work = r_work.json();
    assert_eq!(j_work["total"]["runs"], 4);
    let by_prof_work = j_work["by_profile"].as_array().unwrap();
    assert_eq!(by_prof_work.len(), 1);
    assert_eq!(by_prof_work[0]["profile"], "work");
    assert_eq!(by_prof_work[0]["runs"], 4);

    // ?profile=personal
    let r_pers = srv.get("/api/stats?profile=personal");
    assert_eq!(r_pers.status, 200);
    let j_pers = r_pers.json();
    assert_eq!(j_pers["total"]["runs"], 1);
    let by_prof_pers = j_pers["by_profile"].as_array().unwrap();
    assert_eq!(by_prof_pers.len(), 1);
    assert_eq!(by_prof_pers[0]["profile"], "personal");
    assert_eq!(by_prof_pers[0]["runs"], 1);

    // no parameter -> both
    let r_all = srv.get("/api/stats");
    assert_eq!(r_all.status, 200);
    let j_all = r_all.json();
    assert_eq!(j_all["total"]["runs"], 5);
    let by_prof_all = j_all["by_profile"].as_array().unwrap();
    assert_eq!(by_prof_all.len(), 2);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn stats_since_filter() {
    let sb = Sandbox::new();
    let _conn = seed_stats_data(&sb);
    let mut srv = sb.serve(&[]);

    // ?since=1d excludes the run seeded 3 days ago
    let r = srv.get("/api/stats?since=1d");
    assert_eq!(r.status, 200);
    let j = r.json();
    assert_eq!(j["total"]["runs"], 4);
    assert_eq!(j["total"]["input_tokens"], 312);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn stats_bad_params_are_400() {
    let sb = Sandbox::new();
    let _conn = seed_stats_data(&sb);
    let mut srv = sb.serve(&[]);

    // ?profile=bogus -> 400 with check_profile message
    let r_prof = srv.get("/api/stats?profile=bogus");
    assert_eq!(r_prof.status, 400);
    let err_prof = r_prof.json()["error"].as_str().unwrap().to_string();
    assert!(
        err_prof.contains("Profile 'bogus' không hợp lệ"),
        "unexpected error: {err_prof}"
    );

    // ?since=abc -> 400 containing Khoảng thời gian không hợp lệ
    let r_since = srv.get("/api/stats?since=abc");
    assert_eq!(r_since.status, 400);
    let err_since = r_since.json()["error"].as_str().unwrap().to_string();
    assert!(
        err_since.contains("Khoảng thời gian không hợp lệ"),
        "unexpected error: {err_since}"
    );

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn stats_requires_cookie() {
    let sb = Sandbox::new();
    let mut srv = sb.serve(&[]);

    let r = srv.request("GET", "/api/stats", &[], b"");
    assert_eq!(r.status, 401);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

// ---- 06-13: per-engine split (ENG-04 / D-23) ----

const HUMAN_ENGINE: &str = "ng\u{01b0}\u{1edd}i";

struct EngineSeed {
    id_a: i64,
    id_b: i64,
    conn: rusqlite::Connection,
}

impl EngineSeed {
    fn new(sb: &Sandbox) -> EngineSeed {
        let repo_a = sb.make_repo("proj-a", &[], None, None);
        let repo_b = sb.make_repo("proj-b", &[], None, None);
        let entry_a = sb.project_entry(&repo_a, "proj-a", "work");
        let entry_b = sb.project_entry(&repo_b, "proj-b", "personal");
        sb.write_map(serde_json::json!([entry_a, entry_b]), &[]);
        let conn = agent_brain::db::open(&sb.db_path).expect("open db");
        let projects: Vec<agent_brain::projects::Project> =
            [("proj-a", &repo_a, "work"), ("proj-b", &repo_b, "personal")]
                .iter()
                .map(|(id, repo, profile)| agent_brain::projects::Project {
                    id: id.to_string(),
                    path: repo.to_string_lossy().to_string(),
                    name_vi: id.to_string(),
                    aliases: vec![],
                    kind: "code".to_string(),
                    profile: profile.to_string(),
                    repo: None,
                    missing: false,
                })
                .collect();
        agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");
        let id_a = agent_brain::db::project_row_id(&conn, "proj-a").expect("id_a");
        let id_b = agent_brain::db::project_row_id(&conn, "proj-b").expect("id_b");
        EngineSeed { id_a, id_b, conn }
    }

    /// A finished run of `engine` with the given tokens and stored cost; `started_at` overrides
    /// the insert time when given.
    #[allow(clippy::too_many_arguments)]
    fn run(
        &self,
        proj_id: i64,
        profile: &str,
        role: &str,
        engine: &str,
        input: i64,
        output: i64,
        cost: f64,
        started_at: Option<i64>,
    ) -> i64 {
        let id = agent_brain::db::insert_run_with_engine(
            &self.conn,
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
            engine,
        )
        .expect("insert run");
        agent_brain::db::finish_run(
            &self.conn,
            id,
            &agent_brain::db::RunFinish {
                status: "succeeded".to_string(),
                exit_code: Some(0),
                session_id: None,
                usage: agent_brain::stream::Usage {
                    input_tokens: input,
                    output_tokens: output,
                    cache_read_input_tokens: 0,
                    cache_creation_input_tokens: 0,
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
        if let Some(ts) = started_at {
            self.conn
                .execute(
                    "UPDATE runs SET started_at = ?1 WHERE id = ?2",
                    rusqlite::params![ts, id],
                )
                .expect("update started_at");
        }
        id
    }

    fn human(&self, proj_id: i64, profile: &str) {
        let task_id = agent_brain::db::upsert_task(
            &self.conn,
            proj_id,
            "t-human",
            "Human task",
            "task/human",
            Some("main"),
            Some("base1"),
        )
        .expect("task");
        let now = agent_brain::db::now_ms();
        agent_brain::db::insert_human_run(
            &self.conn,
            &agent_brain::db::HumanRun {
                task_id,
                project_id: proj_id,
                role: "dev".to_string(),
                profile: profile.to_string(),
                stage: Some("implement".to_string()),
                round: Some(1),
                started_at: now,
                ended_at: now + 60_000,
                outcome: None,
                base_commit: None,
                metrics_json: "{\"staff\":\"ha\"}".to_string(),
            },
        )
        .expect("human run");
    }
}

fn engine_row<'a>(engines: &'a [serde_json::Value], name: &str) -> &'a serde_json::Value {
    engines
        .iter()
        .find(|e| e["engine"] == name)
        .unwrap_or_else(|| panic!("no engine row {name}: {engines:?}"))
}

#[test]
fn eng04_engines_split_and_priced() {
    let sb = Sandbox::new();
    let seed = EngineSeed::new(&sb);
    seed.run(seed.id_a, "work", "lead", "claude", 100, 10, 0.10, None);
    seed.run(seed.id_a, "work", "dev", "claude", 200, 20, 0.20, None);
    seed.run(seed.id_a, "work", "inspect", "claude", 5, 5, 0.01, None);
    seed.run(seed.id_a, "work", "dev", "deepseek", 1000, 100, 0.0, None);
    seed.run(seed.id_a, "work", "dev", "agy", 50, 5, 0.0, None);
    seed.human(seed.id_a, "work");

    let mut srv = sb.serve(&[]);
    let r = srv.get("/api/stats");
    assert_eq!(r.status, 200);
    let j = r.json();

    let engines = j["engines"].as_array().expect("engines array");
    let names: Vec<&str> = engines
        .iter()
        .map(|e| e["engine"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["deepseek", "claude", "agy", HUMAN_ENGINE]);

    let ds = &engines[0];
    assert_eq!(ds["runs"], 1);
    assert_eq!(ds["input"], 1000);
    assert_eq!(ds["output"], 100);
    assert_eq!(ds["cost"].as_f64().unwrap(), 0.0);
    assert_eq!(ds["priced"], false);

    let cl = &engines[1];
    assert_eq!(cl["runs"], 3);
    assert_eq!(cl["input"], 305);
    assert_eq!(cl["output"], 35);
    assert!((cl["cost"].as_f64().unwrap() - 0.31).abs() < 1e-9);
    assert_eq!(cl["priced"], true);

    assert_eq!(engines[2]["input"], 50);
    assert_eq!(engines[2]["output"], 5);
    assert_eq!(engines[2]["priced"], false);

    let human = &engines[3];
    assert_eq!(human["runs"], 1);
    assert_eq!(human["input"], 0);
    assert_eq!(human["output"], 0);
    assert_eq!(human["cost"].as_f64().unwrap(), 0.0);
    assert_eq!(human["priced"], false);

    // The existing fields keep their meaning and the tokens add up across engines.
    assert_eq!(j["total"]["runs"], 6);
    assert_eq!(j["total"]["input_tokens"], 1355);
    assert!(j["by_project"].is_array() && j["by_role"].is_array());
    assert!(j["by_day"].is_array() && j["by_profile"].is_array());
    let sum_in: i64 = engines.iter().map(|e| e["input"].as_i64().unwrap()).sum();
    assert_eq!(sum_in, j["total"]["input_tokens"].as_i64().unwrap());

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));

    // A `pricing.deepseek` entry in config.json makes the deepseek row priced.
    sb.write_config(serde_json::json!({
        "pricing": {
            "deepseek": {"input": 0.3, "output": 1.2, "cache_read": 0.03, "cache_write": 0.3}
        }
    }));
    let mut srv = sb.serve(&[]);
    let j = srv.get("/api/stats").json();
    let engines = j["engines"].as_array().expect("engines array");
    assert_eq!(engine_row(engines, "deepseek")["priced"], true);
    assert_eq!(engine_row(engines, "agy")["priced"], false);
    assert_eq!(engine_row(engines, HUMAN_ENGINE)["priced"], false);
    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn eng04_engines_follow_filters() {
    let sb = Sandbox::new();
    let seed = EngineSeed::new(&sb);
    let now = agent_brain::db::now_ms();
    let three_days_ago = now - 3 * 86_400_000;
    seed.run(seed.id_a, "work", "dev", "claude", 10, 1, 0.01, Some(now));
    seed.run(seed.id_a, "work", "dev", "deepseek", 20, 2, 0.0, Some(now));
    seed.run(
        seed.id_a,
        "work",
        "dev",
        "agy",
        30,
        3,
        0.0,
        Some(three_days_ago),
    );
    seed.run(
        seed.id_b,
        "personal",
        "dev",
        "claude",
        40,
        4,
        0.04,
        Some(three_days_ago),
    );

    let mut srv = sb.serve(&[]);

    let all = srv.get("/api/stats").json();
    let all_engines = all["engines"].as_array().unwrap();
    assert_eq!(all_engines.len(), 3);
    assert_eq!(engine_row(all_engines, "claude")["runs"], 2);

    let work = srv.get("/api/stats?profile=work").json();
    let work_engines = work["engines"].as_array().unwrap();
    assert_eq!(work_engines.len(), 3);
    assert_eq!(engine_row(work_engines, "claude")["runs"], 1);
    assert_eq!(engine_row(work_engines, "claude")["input"], 10);

    let personal = srv.get("/api/stats?profile=personal").json();
    let personal_engines = personal["engines"].as_array().unwrap();
    assert_eq!(personal_engines.len(), 1);
    assert_eq!(personal_engines[0]["engine"], "claude");
    assert_eq!(personal_engines[0]["input"], 40);

    let recent = srv.get("/api/stats?since=1d").json();
    let recent_engines = recent["engines"].as_array().unwrap();
    assert_eq!(recent_engines.len(), 2);
    assert!(recent_engines.iter().all(|e| e["engine"] != "agy"));
    let sum_runs: i64 = recent_engines
        .iter()
        .map(|e| e["runs"].as_i64().unwrap())
        .sum();
    assert_eq!(sum_runs, recent["total"]["runs"].as_i64().unwrap());

    // Empty window: the personal runs are all older than a day.
    let empty = srv.get("/api/stats?profile=personal&since=1d").json();
    assert_eq!(empty["engines"], serde_json::json!([]));
    assert_eq!(empty["total"]["runs"], 0);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}
