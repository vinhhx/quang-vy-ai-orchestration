use std::path::Path;

type DbSnapshot = (
    Vec<Vec<String>>,
    Vec<Vec<String>>,
    Vec<Vec<String>>,
    Vec<Vec<String>>,
);

fn dump_table(conn: &rusqlite::Connection, sql: &str, col_count: usize) -> Vec<Vec<String>> {
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt
        .query_map([], |row| {
            let mut v = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let val: rusqlite::types::Value = row.get(i)?;
                v.push(format!("{val:?}"));
            }
            Ok(v)
        })
        .unwrap();
    rows.map(|r| r.unwrap()).collect()
}

fn snapshot(conn: &rusqlite::Connection) -> DbSnapshot {
    let projects = dump_table(
        conn,
        "SELECT id, slug, path, name_vi, aliases_json, type, profile, repo, missing FROM projects ORDER BY id",
        9,
    );
    let tasks = dump_table(
        conn,
        "SELECT id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at FROM tasks ORDER BY id",
        10,
    );
    let runs = dump_table(
        conn,
        "SELECT id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error FROM runs ORDER BY id",
        31,
    );
    let events = dump_table(
        conn,
        "SELECT id, run_id, seq, ts, kind, raw_json FROM events ORDER BY id",
        6,
    );
    (projects, tasks, runs, events)
}

fn build_v1(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch(agent_brain::db::MIGRATIONS[0]).unwrap();
    conn.pragma_update(None, "user_version", 1).unwrap();

    // 2 projects
    conn.execute(
        "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
         VALUES (1, 'proj-1', '/path/1', 'Dự án 1', '[]', 'code', 'work', NULL, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
         VALUES (2, 'proj-2', '/path/2', 'Dự án 2', '[]', 'code', 'work', NULL, 0)",
        [],
    )
    .unwrap();

    // 2 tasks
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at)
         VALUES (1, 1, 'task-1', 'Task 1', 'task/1', 'main', 'base1', 'open', 1000, 1000)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at)
         VALUES (2, 2, 'task-2', 'Task 2', 'task/2', 'main', 'base2', 'dev_done', 2000, 2000)",
        [],
    )
    .unwrap();

    // 8 runs covering roles lead, dev, qa, inspect and statuses running, succeeded, failed, timed_out, killed, lost
    // (non-zero tokens, cost and outcome on some)
    let run_sqls = [
        // 1: lead, succeeded
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error)
         VALUES (1, 1, 1, 'lead', 'claude', 'work', 'sonnet', 's1', 101, 101, 1, 'succeeded', 'lead:done', 0, NULL, 1000, 1050, 100, 50, 10, 5, 0.05, 2, 2, 1, 0, 0, 'result', 'c1', '/tmp/r1', NULL)",
        // 2: dev, running
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error)
         VALUES (2, 1, 1, 'dev', 'claude', 'work', 'sonnet', 's2', 102, 102, 2, 'running', NULL, NULL, NULL, 1060, NULL, 0, 0, 0, 0, 0.0, 0, 0, 0, 0, 0, 'none', 'c1', '/tmp/r2', NULL)",
        // 3: qa, failed
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error)
         VALUES (3, 1, 1, 'qa', 'claude', 'work', 'sonnet', 's3', 103, 103, 3, 'failed', 'error', 1, NULL, 1070, 1080, 50, 20, 0, 0, 0.01, 1, 1, 0, 0, 0, 'partial', 'c1', '/tmp/r3', 'compile error')",
        // 4: inspect, timed_out
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error)
         VALUES (4, 2, 2, 'inspect', 'claude', 'work', 'sonnet', 's4', 104, 104, 4, 'timed_out', 'timed_out', 124, NULL, 2000, 2050, 80, 40, 0, 0, 0.02, 1, 1, 0, 0, 0, 'partial', 'c2', '/tmp/r4', 'timeout')",
        // 5: lead, killed
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error)
         VALUES (5, 2, 2, 'lead', 'claude', 'work', 'sonnet', 's5', 105, 105, 5, 'killed', 'killed', 137, 2090, 2060, 2095, 60, 30, 0, 0, 0.015, 1, 1, 0, 0, 0, 'partial', 'c2', '/tmp/r5', 'killed by user')",
        // 6: dev, lost
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error)
         VALUES (6, 2, 2, 'dev', 'claude', 'work', 'sonnet', 's6', 106, 106, 6, 'lost', 'lost', NULL, NULL, 2100, 2120, 0, 0, 0, 0, 0.0, 0, 0, 0, 0, 0, 'none', 'c2', '/tmp/r6', 'proc lost')",
        // 7: qa, succeeded
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error)
         VALUES (7, 1, 1, 'qa', 'claude', 'work', 'sonnet', 's7', 107, 107, 7, 'succeeded', 'qa:pass', 0, NULL, 2130, 2160, 120, 60, 30, 15, 0.04, 2, 2, 0, 0, 0, 'result', 'c1', '/tmp/r7', NULL)",
        // 8: inspect, succeeded
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error)
         VALUES (8, 2, 2, 'inspect', 'claude', 'work', 'sonnet', 's8', 108, 108, 8, 'succeeded', 'inspect:done', 0, NULL, 2170, 2200, 90, 45, 10, 5, 0.03, 1, 1, 0, 0, 0, 'result', 'c2', '/tmp/r8', NULL)",
    ];
    for sql in run_sqls {
        conn.execute(sql, []).unwrap();
    }

    // 3 events per run = 24 rows
    for run_id in 1..=8 {
        for seq in 1..=3 {
            conn.execute(
                "INSERT INTO events (run_id, seq, ts, kind, raw_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![run_id, seq, 1000 + run_id * 10 + seq, "msg", "{\"data\":1}"],
            )
            .unwrap();
        }
    }
}

#[test]
fn v1_db_upgrades_in_place_keeping_events() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v1(&db_path);

    let raw_conn = rusqlite::Connection::open(&db_path).unwrap();
    let pre_snap = snapshot(&raw_conn);
    drop(raw_conn);

    let conn = agent_brain::db::open(&db_path).expect("open v1 DB upgrades to v2");

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);

    let fk: i64 = conn
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1);

    let bad_fk: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(bad_fk, 0);

    let ev_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ev_count, 24);

    let post_snap = snapshot(&conn);
    assert_eq!(pre_snap, post_snap);

    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'index'")
        .unwrap();
    let idx_names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(idx_names.contains(&"runs_started".to_string()));
    assert!(idx_names.contains(&"runs_task".to_string()));
    assert!(idx_names.contains(&"runs_running".to_string()));

    let mut stmt = conn.prepare("PRAGMA table_info(runs)").unwrap();
    let run_cols: Vec<String> = stmt
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(run_cols.contains(&"stage".to_string()));
    assert!(run_cols.contains(&"round".to_string()));
    assert!(run_cols.contains(&"metrics_json".to_string()));

    let mut stmt = conn.prepare("PRAGMA table_info(tasks)").unwrap();
    let task_cols: Vec<String> = stmt
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for col in [
        "workflow_json",
        "approved_at",
        "approved_commit",
        "auto_state",
        "auto_pid",
        "auto_pid_start",
        "auto_stop_at",
        "auto_stop_kind",
        "auto_json",
        "origin",
    ] {
        assert!(
            task_cols.contains(&col.to_string()),
            "missing task column: {col}"
        );
    }
}

#[test]
fn v2_role_check_extended() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v1(&db_path);

    let conn = agent_brain::db::open(&db_path).unwrap();

    let res_pr = conn.execute(
        "INSERT INTO runs (task_id, project_id, role, profile, status, started_at) VALUES (1, 1, 'plan-review', 'work', 'succeeded', 3000)",
        [],
    );
    assert!(
        res_pr.is_ok(),
        "plan-review should be accepted by role CHECK"
    );

    let res_cr = conn.execute(
        "INSERT INTO runs (task_id, project_id, role, profile, status, started_at) VALUES (1, 1, 'code-review', 'work', 'succeeded', 3010)",
        [],
    );
    assert!(
        res_cr.is_ok(),
        "code-review should be accepted by role CHECK"
    );

    let res_bogus = conn.execute(
        "INSERT INTO runs (task_id, project_id, role, profile, status, started_at) VALUES (1, 1, 'bogus', 'work', 'succeeded', 3020)",
        [],
    );
    assert!(res_bogus.is_err(), "bogus should be rejected by role CHECK");
}

#[test]
fn v2_reopen_is_noop() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v1(&db_path);

    let conn1 = agent_brain::db::open(&db_path).unwrap();
    let ver1: i64 = conn1
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    let schema1: Vec<String> = {
        let mut stmt = conn1
            .prepare("SELECT sql FROM sqlite_master ORDER BY name")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap_or_default())
            .collect()
    };
    let count_p1: i64 = conn1
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .unwrap();
    let count_t1: i64 = conn1
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    let count_r1: i64 = conn1
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap();
    let count_e1: i64 = conn1
        .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
        .unwrap();
    drop(conn1);

    let conn2 = agent_brain::db::open(&db_path).unwrap();
    let ver2: i64 = conn2
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    let schema2: Vec<String> = {
        let mut stmt = conn2
            .prepare("SELECT sql FROM sqlite_master ORDER BY name")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap_or_default())
            .collect()
    };
    let count_p2: i64 = conn2
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .unwrap();
    let count_t2: i64 = conn2
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    let count_r2: i64 = conn2
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap();
    let count_e2: i64 = conn2
        .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
        .unwrap();

    assert_eq!(ver1, ver2);
    assert_eq!(schema1, schema2);
    assert_eq!(count_p1, count_p2);
    assert_eq!(count_t1, count_t2);
    assert_eq!(count_r1, count_r2);
    assert_eq!(count_e1, count_e2);
}

#[test]
fn fresh_db_is_v2() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("fresh.db");
    let conn = agent_brain::db::open(&db_path).unwrap();

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);

    let mut stmt = conn.prepare("PRAGMA table_info(runs)").unwrap();
    let run_cols: Vec<String> = stmt
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(run_cols.contains(&"stage".to_string()));
    assert!(run_cols.contains(&"round".to_string()));
    assert!(run_cols.contains(&"metrics_json".to_string()));

    let mut stmt = conn.prepare("PRAGMA table_info(tasks)").unwrap();
    let task_cols: Vec<String> = stmt
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for col in [
        "workflow_json",
        "approved_at",
        "approved_commit",
        "auto_state",
        "auto_pid",
        "auto_pid_start",
        "auto_stop_at",
        "auto_stop_kind",
        "auto_json",
        "origin",
    ] {
        assert!(task_cols.contains(&col.to_string()));
    }
}

#[test]
fn newer_version_is_refused_and_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v1(&db_path);

    let raw_conn = rusqlite::Connection::open(&db_path).unwrap();
    let newer = agent_brain::db::SCHEMA_VERSION + 1;
    raw_conn.pragma_update(None, "user_version", newer).unwrap();
    let mut stmt = raw_conn
        .prepare("SELECT name FROM sqlite_master ORDER BY name")
        .unwrap();
    let recorded_names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    drop(stmt);
    drop(raw_conn);

    let res = agent_brain::db::open(&db_path);
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    let want_newer = format!("có phiên bản {newer}");
    let want_supported = format!("hỗ trợ tới {}", agent_brain::db::SCHEMA_VERSION);
    assert!(
        err_msg.contains(&want_newer),
        "error message should contain '{want_newer}': {err_msg}"
    );
    assert!(
        err_msg.contains(&want_supported),
        "error message should contain '{want_supported}': {err_msg}"
    );

    let raw_conn = rusqlite::Connection::open(&db_path).unwrap();
    let ver_after: i64 = raw_conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(ver_after, newer);

    let mut stmt = raw_conn
        .prepare("SELECT name FROM sqlite_master ORDER BY name")
        .unwrap();
    let names_after: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(recorded_names, names_after);

    let mut stmt = raw_conn.prepare("PRAGMA table_info(runs)").unwrap();
    let run_cols: Vec<String> = stmt
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(!run_cols.contains(&"stage".to_string()));
}

#[test]
fn concurrent_open_of_v1_both_succeed() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v1(&db_path);

    let p1 = db_path.clone();
    let t1 = std::thread::spawn(move || {
        agent_brain::db::open(&p1)
            .map(|_| ())
            .map_err(|e| e.to_string())
    });

    let p2 = db_path.clone();
    let t2 = std::thread::spawn(move || {
        agent_brain::db::open(&p2)
            .map(|_| ())
            .map_err(|e| e.to_string())
    });

    let res1 = t1.join().unwrap();
    let res2 = t2.join().unwrap();
    assert!(res1.is_ok(), "thread 1 failed: {res1:?}");
    assert!(res2.is_ok(), "thread 2 failed: {res2:?}");

    let conn = agent_brain::db::open(&db_path).unwrap();
    let final_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(final_ver, agent_brain::db::SCHEMA_VERSION);

    let ev_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ev_count, 24);
}

#[test]
fn helpers_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    let conn = agent_brain::db::open(&db_path).unwrap();

    let p = agent_brain::projects::Project {
        id: "test-proj".to_string(),
        path: "/path/test".to_string(),
        name_vi: "Test".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    agent_brain::db::sync_projects(&conn, &[p]).unwrap();
    let proj_id = agent_brain::db::project_row_id(&conn, "test-proj").unwrap();
    let task_id = agent_brain::db::upsert_task(
        &conn,
        proj_id,
        "task-1",
        "Task 1",
        "task/1",
        Some("main"),
        Some("base1"),
    )
    .unwrap();
    let new_run = agent_brain::db::NewRun {
        task_id: Some(task_id),
        project_id: proj_id,
        role: "lead".to_string(),
        profile: "work".to_string(),
        model: None,
        pid: 0,
        pgid: 0,
        pid_start: 0,
        base_commit: None,
        run_dir: None,
    };
    let run_id = agent_brain::db::insert_run(&conn, &new_run).unwrap();

    agent_brain::db::set_run_stage_round(&conn, run_id, Some("plan-review"), Some(2)).unwrap();
    let run = agent_brain::db::get_run(&conn, run_id).unwrap().unwrap();
    assert_eq!(run.stage.as_deref(), Some("plan-review"));
    assert_eq!(run.round, Some(2));

    agent_brain::db::set_run_stage_round(&conn, run_id, None, None).unwrap();
    let run = agent_brain::db::get_run(&conn, run_id).unwrap().unwrap();
    assert_eq!(run.stage, None);
    assert_eq!(run.round, None);
}

#[test]
fn merge_metrics_integers_and_nulls() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    let conn = agent_brain::db::open(&db_path).unwrap();

    let p = agent_brain::projects::Project {
        id: "test-proj".to_string(),
        path: "/path/test".to_string(),
        name_vi: "Test".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    agent_brain::db::sync_projects(&conn, &[p]).unwrap();
    let proj_id = agent_brain::db::project_row_id(&conn, "test-proj").unwrap();
    let task_id = agent_brain::db::upsert_task(
        &conn,
        proj_id,
        "task-1",
        "Task 1",
        "task/1",
        Some("main"),
        Some("base1"),
    )
    .unwrap();
    let new_run = agent_brain::db::NewRun {
        task_id: Some(task_id),
        project_id: proj_id,
        role: "lead".to_string(),
        profile: "work".to_string(),
        model: None,
        pid: 0,
        pgid: 0,
        pid_start: 0,
        base_commit: None,
        run_dir: None,
    };
    let run_id = agent_brain::db::insert_run(&conn, &new_run).unwrap();

    // non-object patch returns Err
    let non_obj = serde_json::json!("not-an-object");
    assert!(agent_brain::db::merge_run_metrics(&conn, run_id, &non_obj).is_err());

    let patch1 = serde_json::json!({
        "verdict": "revise",
        "blocking": 1,
        "advisory": 4
    });
    agent_brain::db::merge_run_metrics(&conn, run_id, &patch1).unwrap();

    let patch2 = serde_json::json!({
        "advisory": null,
        "note": "x"
    });
    agent_brain::db::merge_run_metrics(&conn, run_id, &patch2).unwrap();

    let run = agent_brain::db::get_run(&conn, run_id).unwrap().unwrap();
    let metrics_raw = run.metrics_json.expect("metrics_json present");
    let parsed: serde_json::Value = serde_json::from_str(&metrics_raw).unwrap();

    assert_eq!(
        parsed,
        serde_json::json!({
            "verdict": "revise",
            "blocking": 1,
            "advisory": null,
            "note": "x"
        })
    );

    let blocking_val = parsed.get("blocking").unwrap();
    assert_eq!(blocking_val.as_i64(), Some(1));
    assert!(!blocking_val.is_f64());
    assert!(parsed.get("advisory").unwrap().is_null());
}

#[test]
fn task_auto_defaults_and_setters() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    let conn = agent_brain::db::open(&db_path).unwrap();

    let p = agent_brain::projects::Project {
        id: "test-proj".to_string(),
        path: "/path/test".to_string(),
        name_vi: "Test".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    agent_brain::db::sync_projects(&conn, &[p]).unwrap();
    let proj_id = agent_brain::db::project_row_id(&conn, "test-proj").unwrap();
    let task_id = agent_brain::db::upsert_task(
        &conn,
        proj_id,
        "task-1",
        "Task 1",
        "task/1",
        Some("main"),
        Some("base1"),
    )
    .unwrap();

    // task_auto on new task is Some(TaskAuto::default())
    let auto = agent_brain::db::task_auto(&conn, task_id).unwrap();
    assert_eq!(auto, Some(agent_brain::db::TaskAuto::default()));

    // set_task_workflow_if_null returns true first, false second, value unchanged
    let first =
        agent_brain::db::set_task_workflow_if_null(&conn, task_id, "{\"name\":\"a\"}").unwrap();
    assert!(first);
    let second =
        agent_brain::db::set_task_workflow_if_null(&conn, task_id, "{\"name\":\"b\"}").unwrap();
    assert!(!second);
    let auto = agent_brain::db::task_auto(&conn, task_id).unwrap().unwrap();
    assert_eq!(auto.workflow_json.as_deref(), Some("{\"name\":\"a\"}"));

    // set_task_approval and clear_task_approval
    agent_brain::db::set_task_approval(&conn, task_id, 123, "abc").unwrap();
    let auto = agent_brain::db::task_auto(&conn, task_id).unwrap().unwrap();
    assert_eq!(auto.approved_at, Some(123));
    assert_eq!(auto.approved_commit.as_deref(), Some("abc"));
    agent_brain::db::clear_task_approval(&conn, task_id).unwrap();
    let auto = agent_brain::db::task_auto(&conn, task_id).unwrap().unwrap();
    assert_eq!(auto.approved_at, None);
    assert_eq!(auto.approved_commit, None);

    // set_auto_state sets auto_state and updates tasks.updated_at
    let old_task = agent_brain::db::get_task(&conn, task_id).unwrap().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    agent_brain::db::set_auto_state(&conn, task_id, Some("running")).unwrap();
    let auto = agent_brain::db::task_auto(&conn, task_id).unwrap().unwrap();
    assert_eq!(auto.auto_state.as_deref(), Some("running"));
    let new_task = agent_brain::db::get_task(&conn, task_id).unwrap().unwrap();
    assert!(new_task.updated_at >= old_task.updated_at);

    // set_auto_driver
    agent_brain::db::set_auto_driver(&conn, task_id, Some(42), Some(7)).unwrap();
    let auto = agent_brain::db::task_auto(&conn, task_id).unwrap().unwrap();
    assert_eq!(auto.auto_pid, Some(42));
    assert_eq!(auto.auto_pid_start, Some(7));

    // request_auto_stop and clear_auto_stop
    agent_brain::db::request_auto_stop(&conn, task_id, "soft").unwrap();
    let auto = agent_brain::db::task_auto(&conn, task_id).unwrap().unwrap();
    assert_eq!(auto.auto_stop_kind.as_deref(), Some("soft"));
    assert!(auto.auto_stop_at.is_some());
    agent_brain::db::clear_auto_stop(&conn, task_id).unwrap();
    let auto = agent_brain::db::task_auto(&conn, task_id).unwrap().unwrap();
    assert_eq!(auto.auto_stop_kind, None);
    assert_eq!(auto.auto_stop_at, None);

    // set_auto_json round-trips
    agent_brain::db::set_auto_json(&conn, task_id, Some("{}")).unwrap();
    let auto = agent_brain::db::task_auto(&conn, task_id).unwrap().unwrap();
    assert_eq!(auto.auto_json.as_deref(), Some("{}"));

    // task_auto(999) -> None
    assert_eq!(agent_brain::db::task_auto(&conn, 999).unwrap(), None);
}

#[test]
fn latest_run_for_task_role_order() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    let conn = agent_brain::db::open(&db_path).unwrap();

    let p = agent_brain::projects::Project {
        id: "test-proj".to_string(),
        path: "/path/test".to_string(),
        name_vi: "Test".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    agent_brain::db::sync_projects(&conn, &[p]).unwrap();
    let proj_id = agent_brain::db::project_row_id(&conn, "test-proj").unwrap();
    let task_id = agent_brain::db::upsert_task(
        &conn,
        proj_id,
        "task-1",
        "Task 1",
        "task/1",
        Some("main"),
        Some("base1"),
    )
    .unwrap();

    // Insert runs with plain SQL to specify IDs 3 and 5, with equal started_at
    conn.execute(
        "INSERT INTO runs (id, task_id, project_id, role, profile, status, started_at)
         VALUES (3, ?1, ?2, 'lead', 'work', 'succeeded', 5000)",
        rusqlite::params![task_id, proj_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO runs (id, task_id, project_id, role, profile, status, started_at)
         VALUES (5, ?1, ?2, 'lead', 'work', 'succeeded', 5000)",
        rusqlite::params![task_id, proj_id],
    )
    .unwrap();

    let lead_run = agent_brain::db::latest_run_for_task_role(&conn, task_id, "lead")
        .unwrap()
        .expect("lead run found");
    assert_eq!(lead_run.id, 5);

    let qa_run = agent_brain::db::latest_run_for_task_role(&conn, task_id, "qa").unwrap();
    assert!(qa_run.is_none());
}

#[test]
fn auto_driver_rows_lists_running_only() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    let conn = agent_brain::db::open(&db_path).unwrap();

    let p = agent_brain::projects::Project {
        id: "proj-alpha".to_string(),
        path: "/path/alpha".to_string(),
        name_vi: "Alpha".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    agent_brain::db::sync_projects(&conn, &[p]).unwrap();
    let proj_id = agent_brain::db::project_row_id(&conn, "proj-alpha").unwrap();

    let t1 =
        agent_brain::db::upsert_task(&conn, proj_id, "t1", "T1", "task/t1", None, None).unwrap();
    let t2 =
        agent_brain::db::upsert_task(&conn, proj_id, "t2", "T2", "task/t2", None, None).unwrap();
    let _t3 =
        agent_brain::db::upsert_task(&conn, proj_id, "t3", "T3", "task/t3", None, None).unwrap();

    agent_brain::db::set_auto_state(&conn, t1, Some("running")).unwrap();
    agent_brain::db::set_auto_driver(&conn, t1, Some(100), Some(200)).unwrap();

    agent_brain::db::set_auto_state(&conn, t2, Some("stopped")).unwrap();
    // t3 has auto_state NULL

    let drivers = agent_brain::db::auto_driver_rows(&conn).unwrap();
    assert_eq!(drivers.len(), 1);
    assert_eq!(
        drivers[0],
        agent_brain::db::DriverRow {
            task_id: t1,
            project_slug: "proj-alpha".to_string(),
            profile: "work".to_string(),
            auto_pid: Some(100),
            auto_pid_start: Some(200),
        }
    );
}

fn build_v2(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch(agent_brain::db::MIGRATIONS[0]).unwrap();
    conn.execute_batch(agent_brain::db::MIGRATIONS[1]).unwrap();
    conn.pragma_update(None, "user_version", 2).unwrap();
    conn.execute(
        "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
         VALUES (1, 'proj-1', '/path/1', 'Dự án 1', '[]', 'code', 'work', NULL, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at)
         VALUES (1, 1, 'task-1', 'Task 1', 'task/1', 'main', 'base1', 'open', 1000, 1000)",
        [],
    )
    .unwrap();
}

fn insert_task_row(conn: &rusqlite::Connection, n: i64, origin: Option<&str>, created_at: i64) {
    conn.execute(
        "INSERT INTO tasks (project_id, slug, title, branch, status, created_at, updated_at, origin)
         VALUES (1, ?1, 'T', ?2, 'open', ?3, ?3, ?4)",
        rusqlite::params![format!("t-{n}"), format!("task/t-{n}"), created_at, origin],
    )
    .unwrap();
}

#[test]
fn v2_db_gains_origin_column() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v2(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("open v2 DB upgrades in place");

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);

    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    let origin: Option<String> = conn
        .query_row("SELECT origin FROM tasks WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(origin, None);
    assert_eq!(agent_brain::db::task_origin(&conn, 1).unwrap(), None);
}

#[test]
fn origin_helpers_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v2(&db_path);
    let conn = agent_brain::db::open(&db_path).unwrap();

    assert!(agent_brain::db::mark_task_origin(&conn, 1, agent_brain::db::ORIGIN_TELEGRAM).unwrap());
    assert_eq!(
        agent_brain::db::task_origin(&conn, 1).unwrap().as_deref(),
        Some("telegram")
    );
    // Monotonic: a second mark neither changes the row nor the value.
    assert!(!agent_brain::db::mark_task_origin(&conn, 1, "x").unwrap());
    assert!(
        !agent_brain::db::mark_task_origin(&conn, 1, agent_brain::db::ORIGIN_TELEGRAM).unwrap()
    );
    assert_eq!(
        agent_brain::db::task_origin(&conn, 1).unwrap().as_deref(),
        Some("telegram")
    );
    // Missing task.
    assert_eq!(agent_brain::db::task_origin(&conn, 999).unwrap(), None);
    assert!(!agent_brain::db::mark_task_origin(&conn, 999, "telegram").unwrap());
}

#[test]
fn count_tasks_since_counts_origin_only() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v2(&db_path);
    let conn = agent_brain::db::open(&db_path).unwrap();

    let now = agent_brain::db::now_ms();
    insert_task_row(&conn, 10, Some("telegram"), now);
    insert_task_row(&conn, 11, Some("telegram"), 1000);
    insert_task_row(&conn, 12, None, now);

    assert_eq!(
        agent_brain::db::count_tasks_since(&conn, "telegram", now - 60_000).unwrap(),
        1
    );
    assert_eq!(
        agent_brain::db::count_tasks_since(&conn, "telegram", 0).unwrap(),
        2
    );
}

// ---------------------------------------------------------------------------
// Plan 06-04: schema v4 (tasks.team_json), team helpers, run engine, human runs
// ---------------------------------------------------------------------------

fn build_v3(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for sql in &agent_brain::db::MIGRATIONS[..3] {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", 3).unwrap();
    conn.execute(
        "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
         VALUES (1, 'proj-1', '/path/1', 'Dự án 1', '[]', 'code', 'work', NULL, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, origin)
         VALUES (1, 1, 'task-1', 'Task 1', 'task/1', 'main', 'base1', 'open', 1000, 1000, 'telegram')",
        [],
    )
    .unwrap();
}

fn task_columns(conn: &rusqlite::Connection) -> Vec<String> {
    let mut stmt = conn.prepare("PRAGMA table_info(tasks)").unwrap();
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    cols
}

/// Fresh temp database with one project and one task. Returns (tempdir, conn, project id, task id).
fn fresh_task_db() -> (tempfile::TempDir, rusqlite::Connection, i64, i64) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    let conn = agent_brain::db::open(&db_path).unwrap();
    let p = agent_brain::projects::Project {
        id: "test-proj".to_string(),
        path: "/path/test".to_string(),
        name_vi: "Test".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    agent_brain::db::sync_projects(&conn, &[p]).unwrap();
    let proj_id = agent_brain::db::project_row_id(&conn, "test-proj").unwrap();
    let task_id = agent_brain::db::upsert_task(
        &conn,
        proj_id,
        "task-1",
        "Task 1",
        "task/1",
        Some("main"),
        Some("base1"),
    )
    .unwrap();
    (tmp, conn, proj_id, task_id)
}

fn task_updated_at(conn: &rusqlite::Connection, task_id: i64) -> i64 {
    agent_brain::db::get_task(conn, task_id)
        .unwrap()
        .unwrap()
        .updated_at
}

#[test]
fn v3_db_gains_team_json() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v3(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("open v3 DB upgrades to v4");

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, 4);
    assert_eq!(agent_brain::db::SCHEMA_VERSION, 4);
    assert!(task_columns(&conn).contains(&"team_json".to_string()));

    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    assert_eq!(
        agent_brain::db::task_origin(&conn, 1).unwrap().as_deref(),
        Some("telegram")
    );
    assert_eq!(agent_brain::db::task_team_json(&conn, 1).unwrap(), None);
}

#[test]
fn v2_db_jumps_to_v4() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v2(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("open v2 DB upgrades to v4");

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, 4);
    let cols = task_columns(&conn);
    assert!(cols.contains(&"origin".to_string()));
    assert!(cols.contains(&"team_json".to_string()));

    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    let (origin, team): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT origin, team_json FROM tasks WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(origin, None);
    assert_eq!(team, None);
}

#[test]
fn fresh_db_has_team_json_at_current_version() {
    let tmp = tempfile::tempdir().unwrap();
    let conn = agent_brain::db::open(&tmp.path().join("fresh.db")).unwrap();
    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, 4);
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);
    assert!(task_columns(&conn).contains(&"team_json".to_string()));
}

#[test]
fn team_if_null_writes_once() {
    let (_tmp, conn, _proj, task_id) = fresh_task_db();

    let before = task_updated_at(&conn, task_id);
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(agent_brain::db::set_task_team_if_null(&conn, task_id, "{\"name\":\"a\"}").unwrap());
    assert!(task_updated_at(&conn, task_id) > before);

    let after_first = task_updated_at(&conn, task_id);
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(!agent_brain::db::set_task_team_if_null(&conn, task_id, "{\"name\":\"b\"}").unwrap());
    assert_eq!(
        agent_brain::db::task_team_json(&conn, task_id)
            .unwrap()
            .as_deref(),
        Some("{\"name\":\"a\"}")
    );
    // A refused write does not touch updated_at.
    assert_eq!(task_updated_at(&conn, task_id), after_first);
}

#[test]
fn team_cas_rules() {
    let (_tmp, conn, _proj, task_id) = fresh_task_db();
    let a = "{\"name\":\"a\"}";
    let b = "{\"name\":\"b\"}";
    let c = "{\"name\":\"c\"}";
    let x = "{\"name\":\"x\"}";

    // NULL row: only `old = None` matches.
    assert!(!agent_brain::db::set_task_team_cas(&conn, task_id, Some(a), x).unwrap());
    assert_eq!(
        agent_brain::db::task_team_json(&conn, task_id).unwrap(),
        None
    );
    assert!(agent_brain::db::set_task_team_cas(&conn, task_id, None, a).unwrap());
    assert_eq!(
        agent_brain::db::task_team_json(&conn, task_id)
            .unwrap()
            .as_deref(),
        Some(a)
    );

    // Matching old value: swaps and bumps updated_at.
    let t0 = task_updated_at(&conn, task_id);
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(agent_brain::db::set_task_team_cas(&conn, task_id, Some(a), b).unwrap());
    let t1 = task_updated_at(&conn, task_id);
    assert!(t1 > t0);
    assert_eq!(
        agent_brain::db::task_team_json(&conn, task_id)
            .unwrap()
            .as_deref(),
        Some(b)
    );

    // Stale old value: refused, value and updated_at unchanged.
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(!agent_brain::db::set_task_team_cas(&conn, task_id, Some(a), c).unwrap());
    assert_eq!(
        agent_brain::db::task_team_json(&conn, task_id)
            .unwrap()
            .as_deref(),
        Some(b)
    );
    assert_eq!(task_updated_at(&conn, task_id), t1);

    // `old = None` on a non-NULL row is refused too.
    assert!(!agent_brain::db::set_task_team_cas(&conn, task_id, None, c).unwrap());
    assert_eq!(
        agent_brain::db::task_team_json(&conn, task_id)
            .unwrap()
            .as_deref(),
        Some(b)
    );
}

#[test]
fn team_json_missing_task() {
    let (_tmp, conn, _proj, _task_id) = fresh_task_db();
    assert_eq!(agent_brain::db::task_team_json(&conn, 999).unwrap(), None);
    assert!(!agent_brain::db::set_task_team_if_null(&conn, 999, "{}").unwrap());
    assert!(!agent_brain::db::set_task_team_cas(&conn, 999, None, "{}").unwrap());
}
