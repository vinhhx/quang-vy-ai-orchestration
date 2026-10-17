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
