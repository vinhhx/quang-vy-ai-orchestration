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
        hidden: false,
        stack: vec![],
        read_sources: vec![],
        protocol_dir: String::new(),
        workspace: Default::default(),
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
        hidden: false,
        stack: vec![],
        read_sources: vec![],
        protocol_dir: String::new(),
        workspace: Default::default(),
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
        hidden: false,
        stack: vec![],
        read_sources: vec![],
        protocol_dir: String::new(),
        workspace: Default::default(),
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
        hidden: false,
        stack: vec![],
        read_sources: vec![],
        protocol_dir: String::new(),
        workspace: Default::default(),
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
        hidden: false,
        stack: vec![],
        read_sources: vec![],
        protocol_dir: String::new(),
        workspace: Default::default(),
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
        hidden: false,
        stack: vec![],
        read_sources: vec![],
        protocol_dir: String::new(),
        workspace: Default::default(),
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
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);
    assert_eq!(agent_brain::db::SCHEMA_VERSION, 15);
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
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);
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
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);
    assert_eq!(user_ver, 15);
    assert!(task_columns(&conn).contains(&"team_json".to_string()));
}

fn build_v4(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for sql in &agent_brain::db::MIGRATIONS[..4] {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", 4).unwrap();
    conn.execute(
        "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
         VALUES (1, 'proj-1', '/path/1', 'Dự án 1', '[]', 'code', 'work', NULL, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, origin, team_json)
         VALUES (1, 1, 'task-1', 'Task 1', 'task/1', 'main', 'base1', 'open', 1000, 1000, 'telegram', '{\"name\":\"default\"}')",
        [],
    )
    .unwrap();
}

#[test]
fn v4_db_upgrades_to_v5_keeping_data() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v4(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("open v4 DB upgrades to v5");

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, 15);
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);

    // Verify existing data preserved
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    assert_eq!(
        agent_brain::db::task_origin(&conn, 1).unwrap().as_deref(),
        Some("telegram")
    );

    // Verify new tables exist
    let intake_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='intake'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap();
    assert!(intake_exists);

    let proposals_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='proposals'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap();
    assert!(proposals_exists);

    let quota_stops_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='quota_stops'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap();
    assert!(quota_stops_exists);
}

fn build_v5(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for sql in &agent_brain::db::MIGRATIONS[..5] {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", 5).unwrap();
    conn.execute(
        "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
         VALUES (1, 'proj-1', '/path/1', 'Dự án 1', '[]', 'code', 'work', NULL, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO intake (id, source, project_id, request, priority, status, created_at, updated_at)
         VALUES (1, 'ui', 1, 'yêu cầu từ UI', 'normal', 'queued', 1000, 1000),
                (2, 'telegram', 1, 'yêu cầu từ Telegram', 'urgent', 'queued', 1001, 1001)",
        [],
    )
    .unwrap();
}

#[test]
fn v5_db_upgrades_to_v6_keeping_data() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v5(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("open v5 DB upgrades to v6");

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, 15);
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);

    // Dữ liệu cũ trong intake phải được giữ nguyên
    let intake_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM intake", [], |r| r.get(0))
        .unwrap();
    assert_eq!(intake_count, 2);

    let item1 = agent_brain::db::get_intake(&conn, 1).unwrap().unwrap();
    assert_eq!(item1.source, "ui");
    assert_eq!(item1.request, "yêu cầu từ UI");

    let item2 = agent_brain::db::get_intake(&conn, 2).unwrap().unwrap();
    assert_eq!(item2.source, "telegram");
    assert_eq!(item2.priority, "urgent");

    // Thêm nguồn mới: trello và sheet thành công
    let id_trello = agent_brain::db::insert_intake(&conn, "trello", Some(1), "trello bug", "normal", true).unwrap();
    assert!(id_trello > 0);
    let id_sheet = agent_brain::db::insert_intake(&conn, "sheet", Some(1), "sheet bug", "low", true).unwrap();
    assert!(id_sheet > 0);

    // CHECK constraint chặn nguồn không hợp lệ
    let bad_source = agent_brain::db::insert_intake(&conn, "invalid_source", Some(1), "bad bug", "normal", true);
    assert!(bad_source.is_err(), "Kỳ vọng CHECK constraint chặn nguồn không hợp lệ");

    // Bảng fixbug_refs hoạt động bình thường
    let ref_id = agent_brain::db::insert_fixbug_ref(
        &conn,
        1,
        "trello",
        "card_ext_1",
        "T-100",
        "Tiêu đề bug",
        Some("https://trello.com/c/1"),
        Some(id_trello),
    )
    .unwrap();
    assert!(ref_id > 0);

    let ref_row = agent_brain::db::fixbug_ref_by_key(&conn, 1, "T-100").unwrap().unwrap();
    assert_eq!(ref_row.ext_id, "card_ext_1");
    assert_eq!(ref_row.state, "pulled");

    assert!(agent_brain::db::fixbug_ref_exists(&conn, 1, "trello", "card_ext_1").unwrap());
    assert!(!agent_brain::db::fixbug_ref_exists(&conn, 1, "trello", "card_nonexistent").unwrap());

    agent_brain::db::set_fixbug_ref_state(&conn, ref_id, "deployed", Some("Đã deploy lên staging")).unwrap();
    let updated_ref = agent_brain::db::fixbug_ref_by_key(&conn, 1, "T-100").unwrap().unwrap();
    assert_eq!(updated_ref.state, "deployed");
    assert_eq!(updated_ref.note.as_deref(), Some("Đã deploy lên staging"));

    let list = agent_brain::db::list_fixbug_refs(&conn, 1).unwrap();
    assert_eq!(list.len(), 1);

    // Kiểm tra khóa ngoại
    let bad_fk: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(bad_fk, 0);
}

fn build_v6(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for sql in &agent_brain::db::MIGRATIONS[..6] {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", 6).unwrap();
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
    conn.execute(
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error)
         VALUES (1, 1, 1, 'lead', 'claude', 'work', 'sonnet', 's1', 101, 101, 1, 'succeeded', 'lead:done', 0, NULL, 1000, 1050, 100, 50, 10, 5, 0.05, 2, 2, 1, 0, 0, 'result', 'c1', '/tmp/r1', NULL)",
        [],
    )
    .unwrap();
}

#[test]
fn v6_db_upgrades_to_v7_keeping_data() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v6(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("open v6 DB upgrades to v7");

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, 15);
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);

    // Dữ liệu cũ trong runs phải được giữ nguyên
    let run = agent_brain::db::get_run(&conn, 1).unwrap().expect("run 1 exists");
    assert_eq!(run.role, "lead");
    assert_eq!(run.status, "succeeded");
    assert_eq!(run.cost_usd, 0.05);

    // Chèn run mới với role analyst thành công
    let mut new_analyst = sample_new_run(1, 1);
    new_analyst.role = "analyst".to_string();
    let analyst_id = agent_brain::db::insert_run(&conn, &new_analyst).expect("insert analyst run");
    let analyst_run = agent_brain::db::get_run(&conn, analyst_id).unwrap().unwrap();
    assert_eq!(analyst_run.role, "analyst");

    // Chèn run mới với role analysis-review thành công
    let mut new_review = sample_new_run(1, 1);
    new_review.role = "analysis-review".to_string();
    let review_id = agent_brain::db::insert_run(&conn, &new_review).expect("insert analysis-review run");
    let review_run = agent_brain::db::get_run(&conn, review_id).unwrap().unwrap();
    assert_eq!(review_run.role, "analysis-review");

    // CHECK constraint chặn role không hợp lệ
    let mut bad_run = sample_new_run(1, 1);
    bad_run.role = "invalid_role".to_string();
    let bad = agent_brain::db::insert_run(&conn, &bad_run);
    assert!(bad.is_err(), "Kỳ vọng CHECK constraint chặn role không hợp lệ");

    // Kiểm tra khóa ngoại
    let bad_fk: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(bad_fk, 0);
}

fn build_v7(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for sql in &agent_brain::db::MIGRATIONS[..7] {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", 7).unwrap();
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
    conn.execute(
        "INSERT INTO intake (id, source, project_id, request, priority, status, error, created_at, updated_at)
         VALUES (1, 'ui', 1, 'Yêu cầu 1', 'normal', 'proposed', NULL, 1000, 1000)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO proposals (id, intake_id, project_id, title, request, priority, team, workflow, reason, status, task_id, error, created_at, updated_at)
         VALUES (1, 1, 1, 'Đề xuất 1', 'Yêu cầu 1', 'normal', 'mac-dinh', 'mac-dinh', NULL, 'pending', NULL, NULL, 1000, 1000)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error)
         VALUES (1, 1, 1, 'lead', 'claude', 'work', 'sonnet', 's1', 101, 101, 1, 'succeeded', 'lead:done', 0, NULL, 1000, 1050, 100, 50, 10, 5, 0.05, 2, 2, 1, 0, 0, 'result', 'c1', '/tmp/r1', NULL)",
        [],
    )
    .unwrap();
}

#[test]
fn v7_db_upgrades_to_v8_keeping_data() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v7(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("open v7 DB upgrades to v8");

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, 15);
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);

    // Dữ liệu cũ trong tasks phải được giữ nguyên và cột level mặc định là 'vua'
    let task = agent_brain::db::get_task(&conn, 1).unwrap().expect("task 1 exists");
    assert_eq!(task.level, "vua");
    assert_eq!(task.title, "Task 1");

    // Dữ liệu cũ trong proposals phải có level mặc định là 'vua'
    let prop = agent_brain::db::get_proposal(&conn, 1).unwrap().expect("proposal 1 exists");
    assert_eq!(prop.level, "vua");
    assert_eq!(prop.title, "Đề xuất 1");

    // Dữ liệu cũ trong runs có member_id = None
    let run = agent_brain::db::get_run(&conn, 1).unwrap().expect("run 1 exists");
    assert_eq!(run.member_id, None);

    // Cập nhật level cho task
    agent_brain::db::set_task_level(&conn, 1, "kho").unwrap();
    assert_eq!(agent_brain::db::task_level(&conn, 1).unwrap(), "kho");

    // CHECK constraint chặn level không hợp lệ trên tasks
    let bad_level = conn.execute("UPDATE tasks SET level = 'invalid' WHERE id = 1", []);
    assert!(bad_level.is_err(), "CHECK constraint phải chặn level không hợp lệ trên tasks");

    // Gán member_id cho run và ghi điểm member_scores
    agent_brain::db::set_run_member_id(&conn, 1, "opus-1").unwrap();
    let run_after = agent_brain::db::get_run(&conn, 1).unwrap().unwrap();
    assert_eq!(run_after.member_id.as_deref(), Some("opus-1"));

    agent_brain::db::record_member_score(&conn, 1, "opus-1", Some(1), 5, Some("Làm rất tốt")).unwrap();
    let (avg, count) = agent_brain::db::member_score_avg(&conn, "opus-1").unwrap();
    assert_eq!(count, 1);
    assert!((avg - 5.0).abs() < 1e-6);

    // CHECK constraint chặn score ngoài 1..5
    let bad_score = conn.execute(
        "INSERT INTO member_scores (run_id, member_id, task_id, score, created_at, updated_at) VALUES (99, 'opus-1', 1, 6, 1000, 1000)",
        [],
    );
    assert!(bad_score.is_err(), "CHECK constraint phải chặn score ngoài 1..5");

    // Kiểm tra khóa ngoại
    let bad_fk: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(bad_fk, 0);
}

fn build_v8(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for sql in &agent_brain::db::MIGRATIONS[..8] {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", 8).unwrap();
    conn.execute(
        "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
         VALUES (1, 'proj-1', '/path/1', 'Dự án 1', '[]', 'code', 'work', NULL, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level)
         VALUES (1, 1, 'task-1', 'Task 1', 'task/1', 'main', 'base1', 'open', 1000, 1000, 'vua')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO intake (id, source, project_id, request, priority, status, error, created_at, updated_at)
         VALUES (1, 'ui', 1, 'Yêu cầu 1', 'normal', 'proposed', NULL, 1000, 1000)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO proposals (id, intake_id, project_id, title, request, priority, team, workflow, reason, status, task_id, error, created_at, updated_at, level)
         VALUES (1, 1, 1, 'Đề xuất 1', 'Yêu cầu 1', 'normal', 'mac-dinh', 'mac-dinh', NULL, 'pending', NULL, NULL, 1000, 1000, 'vua')",
        [],
    )
    .unwrap();
}

#[test]
fn v8_db_upgrades_to_v9_keeping_data() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v8(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("open v8 DB upgrades to v9");

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, 15);
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);

    // Dữ liệu cũ trong proposals phải có after_proposal_id = None
    let prop = agent_brain::db::get_proposal(&conn, 1).unwrap().expect("proposal 1 exists");
    assert_eq!(prop.after_proposal_id, None);
    assert_eq!(prop.title, "Đề xuất 1");

    // Các bảng mới task_inputs, intake_inputs tồn tại và hoạt động
    let inp_id = agent_brain::inputs::add_to_proposal(
        &conn,
        1,
        "input-1.md",
        "paste",
        "# Nội dung tài liệu",
        &[],
    )
    .unwrap();
    assert!(inp_id > 0);

    let inputs = agent_brain::inputs::list_for_proposal(&conn, 1).unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].name, "input-1.md");

    // Foreign key check
    let bad_fk: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(bad_fk, 0);
}

fn build_v9(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for sql in &agent_brain::db::MIGRATIONS[..9] {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", 9).unwrap();
    conn.execute(
        "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
         VALUES (1, 'proj-1', '/path/1', 'Dự án 1', '[]', 'code', 'work', NULL, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level)
         VALUES (1, 1, 'task-1', 'Task 1', 'task/1', 'main', 'base1', 'open', 1000, 1000, 'vua')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO intake (id, source, project_id, request, priority, status, error, created_at, updated_at)
         VALUES (1, 'ui', 1, 'Yêu cầu 1', 'normal', 'proposed', NULL, 1000, 1000)",
        [],
    )
    .unwrap();
}

#[test]
fn v9_db_upgrades_to_v10_keeping_data() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v9(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("open v9 DB upgrades to v10");

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, 15);
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);

    // Existing intake row has default split = true (1)
    let intake = agent_brain::db::get_intake(&conn, 1).unwrap().expect("intake 1 exists");
    assert!(intake.split);

    // Test plan_approvals table
    agent_brain::db::insert_plan_approval(&conn, 1, "c111111", 1000, "ui").unwrap();
    agent_brain::db::insert_plan_approval(&conn, 1, "c222222", 2000, "telegram").unwrap();

    let diff_prev = agent_brain::db::latest_different_plan_approval(&conn, 1, "c222222").unwrap();
    assert_eq!(diff_prev.as_deref(), Some("c111111"));

    let diff_prev2 = agent_brain::db::latest_different_plan_approval(&conn, 1, "c111111").unwrap();
    assert_eq!(diff_prev2.as_deref(), Some("c222222"));

    // Foreign key check
    let bad_fk: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(bad_fk, 0);
}

/// Seed rows shared by `build_v10` / `build_v11`: a project, a task, two runs, three events,
/// then (foreign keys OFF) one run deleted so that two events become orphans.
fn seed_with_orphan_events(conn: &rusqlite::Connection) {
    conn.execute(
        "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
         VALUES (1, 'proj-1', '/path/1', 'Dự án 1', '[]', 'code', 'work', NULL, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level)
         VALUES (1, 1, 'task-1', 'Task 1', 'task/1', 'main', 'base1', 'open', 1000, 1000, 'vua')",
        [],
    )
    .unwrap();
    for run_id in 1..=2 {
        conn.execute(
            "INSERT INTO runs (id, task_id, project_id, role, engine, profile, status, started_at)
             VALUES (?1, 1, 1, 'dev', 'claude', 'work', 'succeeded', 1000)",
            rusqlite::params![run_id],
        )
        .unwrap();
    }
    // run 1 gets one event, run 2 gets two events
    for (run_id, seq) in [(1, 1), (2, 1), (2, 2)] {
        conn.execute(
            "INSERT INTO events (run_id, seq, ts, kind, raw_json) VALUES (?1, ?2, 1000, 'msg', '{}')",
            rusqlite::params![run_id, seq],
        )
        .unwrap();
    }
    // delete run 2 with foreign keys OFF: its two events become orphans
    conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
    conn.execute("DELETE FROM runs WHERE id = 2", []).unwrap();
}

fn build_v10(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for sql in &agent_brain::db::MIGRATIONS[..10] {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", 10).unwrap();
    seed_with_orphan_events(&conn);
}

fn fk_violation_count(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
        r.get(0)
    })
    .unwrap()
}

#[test]
fn v10_db_with_orphan_events_upgrades() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v10(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("v10 DB with orphan events must open");

    let user_ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(user_ver, agent_brain::db::SCHEMA_VERSION);
    let events: i64 = conn
        .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 3, "no event row is lost");
    assert_eq!(fk_violation_count(&conn), 2, "orphan count is unchanged");
}

fn build_v11(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for sql in &agent_brain::db::MIGRATIONS[..11] {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", 11).unwrap();
    conn.execute(
        "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
         VALUES (1, 'proj-1', '/path/1', 'Dự án 1', '[]', 'code', 'work', NULL, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level)
         VALUES (1, 1, 'task-1', 'Task 1', 'task/1', 'main', 'base1', 'open', 1000, 1000, 'vua')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO runs (id, task_id, project_id, role, engine, profile, status, started_at)
         VALUES (1, 1, 1, 'dev', 'claude', 'work', 'succeeded', 1000)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO events (run_id, seq, ts, kind, raw_json) VALUES (1, 1, 1000, 'msg', '{}')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO intake (id, source, project_id, request, priority, status, error, created_at, updated_at)
         VALUES (1, 'ui', 1, 'Yêu cầu 1', 'normal', 'proposed', NULL, 1000, 1000)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO proposals (id, intake_id, project_id, title, request, priority, team, workflow, reason, status, task_id, error, created_at, updated_at, level)
         VALUES (1, 1, 1, 'Đề xuất 1', 'Yêu cầu 1', 'normal', 'mac-dinh', 'mac-dinh', NULL, 'pending', NULL, NULL, 1000, 1000, 'vua')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO plan_approvals (task_id, \"commit\", approved_at, source) VALUES (1, 'c111111', 1000, 'ui')",
        [],
    )
    .unwrap();
}

fn build_v11_with_orphans(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for sql in &agent_brain::db::MIGRATIONS[..11] {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", 11).unwrap();
    seed_with_orphan_events(&conn);
}

fn count_rows(conn: &rusqlite::Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

fn user_version(conn: &rusqlite::Connection) -> i64 {
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

#[test]
fn v11_db_upgrades_to_v12_keeping_data() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v11(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("open v11 DB upgrades to v12");

    assert_eq!(user_version(&conn), 15);
    assert_eq!(user_version(&conn), agent_brain::db::SCHEMA_VERSION);
    for table in [
        "projects",
        "tasks",
        "runs",
        "events",
        "intake",
        "proposals",
        "plan_approvals",
    ] {
        assert_eq!(count_rows(&conn, table), 1, "{table} keeps its row");
    }

    for name in ["journal_state", "journal_mail", "journal_fts"] {
        let found: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = ?1",
                [name],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 1, "{name} exists");
    }

    // journal_mail.dedupe_key is UNIQUE and sent_at is nullable
    let insert_mail = "INSERT INTO journal_mail(kind, dedupe_key, text, created_at) VALUES ('digest', 'k1', 'xin chào', 1)";
    conn.execute(insert_mail, []).unwrap();
    assert!(conn.execute(insert_mail, []).is_err());
    let sent_at: Option<i64> = conn
        .query_row("SELECT sent_at FROM journal_mail", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sent_at, None);

    // journal_state.key is the primary key
    conn.execute(
        "INSERT INTO journal_state(key, value, updated_at) VALUES ('last_export', '1', 1)",
        [],
    )
    .unwrap();
    assert!(conn
        .execute(
            "INSERT INTO journal_state(key, value, updated_at) VALUES ('last_export', '2', 2)",
            [],
        )
        .is_err());

    // FTS5: a row is found by a quoted-token MATCH
    conn.execute(
        "INSERT INTO journal_fts(task_ref, day, display, title, project, grp, day_text, status)
         VALUES ('1', '2026-01-15', 'du-an-a — Sua loi', 'sua loi dang nhap', 'du-an-a', 'cong viec', '2026 01 15', 'xong')",
        [],
    )
    .unwrap();
    let found: String = conn
        .query_row(
            "SELECT task_ref FROM journal_fts WHERE journal_fts MATCH '\"dang\" \"nhap\"'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(found, "1");

    assert_eq!(fk_violation_count(&conn), 0);
}

#[test]
fn v11_db_with_orphan_events_upgrades_to_v12() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v11_with_orphans(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("v11 DB with orphan events must open");

    assert_eq!(user_version(&conn), agent_brain::db::SCHEMA_VERSION);
    assert_eq!(count_rows(&conn, "events"), 3, "no event row is lost");
    assert_eq!(fk_violation_count(&conn), 2, "orphan count is unchanged");
}

#[test]
fn v12_reopen_runs_no_migration() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v11(&db_path);

    let first = agent_brain::db::open(&db_path).expect("first open upgrades");
    assert_eq!(user_version(&first), 15);
    first
        .execute(
            "INSERT INTO journal_state(key, value, updated_at) VALUES ('k', 'v', 1)",
            [],
        )
        .unwrap();
    drop(first);

    let second = agent_brain::db::open(&db_path).expect("reopen of a v12 DB");
    assert_eq!(user_version(&second), 15);
    assert_eq!(count_rows(&second, "journal_state"), 1);
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

/// "người" in NFC, written with escapes so the test cannot silently use a decomposed form.
const HUMAN_ENGINE_NFC: &str = "ng\u{01b0}\u{1edd}i";

fn sample_new_run(task_id: i64, proj_id: i64) -> agent_brain::db::NewRun {
    agent_brain::db::NewRun {
        task_id: Some(task_id),
        project_id: proj_id,
        role: "dev".to_string(),
        profile: "work".to_string(),
        model: Some("sonnet".to_string()),
        pid: 4242,
        pgid: 4242,
        pid_start: 7,
        base_commit: Some("abc123".to_string()),
        run_dir: Some("/tmp/run-dir".to_string()),
    }
}

fn sample_human_run(task_id: i64, proj_id: i64) -> agent_brain::db::HumanRun {
    agent_brain::db::HumanRun {
        task_id,
        project_id: proj_id,
        role: "dev".to_string(),
        profile: "work".to_string(),
        stage: Some("implement".to_string()),
        round: Some(2),
        started_at: 1000,
        ended_at: 61000,
        outcome: None,
        base_commit: Some("base1".to_string()),
        metrics_json: "{\"team\":\"tieu-doi\",\"staff\":\"ha\"}".to_string(),
    }
}

#[test]
fn insert_run_with_engine_stores_engine() {
    let (_tmp, conn, proj_id, task_id) = fresh_task_db();
    let new_run = sample_new_run(task_id, proj_id);

    let ds_id = agent_brain::db::insert_run_with_engine(&conn, &new_run, "deepseek").unwrap();
    let ds = agent_brain::db::get_run(&conn, ds_id).unwrap().unwrap();
    assert_eq!(ds.engine, "deepseek");
    assert_eq!(ds.status, "running");
    assert_eq!(ds.pid, Some(4242));
    assert_eq!(ds.pgid, Some(4242));
    assert_eq!(ds.pid_start, Some(7));
    assert_eq!(ds.model.as_deref(), Some("sonnet"));
    assert_eq!(ds.base_commit.as_deref(), Some("abc123"));
    assert_eq!(ds.run_dir.as_deref(), Some("/tmp/run-dir"));

    // insert_run keeps its signature and stores `claude`.
    let cl_id = agent_brain::db::insert_run(&conn, &new_run).unwrap();
    let cl = agent_brain::db::get_run(&conn, cl_id).unwrap().unwrap();
    assert_eq!(cl.engine, "claude");
    assert_eq!(cl.status, "running");
    assert_eq!(cl.pid, ds.pid);
}

#[test]
fn insert_human_run_shape() {
    let (_tmp, conn, proj_id, task_id) = fresh_task_db();
    let h = sample_human_run(task_id, proj_id);

    let id = agent_brain::db::insert_human_run(&conn, &h).unwrap();
    let run = agent_brain::db::get_run(&conn, id).unwrap().unwrap();

    assert_eq!(run.engine, HUMAN_ENGINE_NFC);
    assert_eq!(run.status, "succeeded");
    assert_eq!(run.usage_source, "none");
    assert_eq!(run.task_id, Some(task_id));
    assert_eq!(run.project_id, proj_id);
    assert_eq!(run.role, "dev");
    assert_eq!(run.profile, "work");
    assert_eq!(run.stage.as_deref(), Some("implement"));
    assert_eq!(run.round, Some(2));
    assert_eq!(run.started_at, 1000);
    assert_eq!(run.ended_at, Some(61000));
    assert_eq!(run.outcome, None);
    assert_eq!(run.base_commit.as_deref(), Some("base1"));
    assert_eq!(
        run.metrics_json.as_deref(),
        Some("{\"team\":\"tieu-doi\",\"staff\":\"ha\"}")
    );

    assert_eq!(run.input_tokens, 0);
    assert_eq!(run.output_tokens, 0);
    assert_eq!(run.cache_read_tokens, 0);
    assert_eq!(run.cache_write_tokens, 0);
    assert_eq!(run.cost_usd, 0.0);
    assert_eq!(run.model, None);
    assert_eq!(run.pid, None);
    assert_eq!(run.pgid, None);
    assert_eq!(run.pid_start, None);
    assert_eq!(run.run_dir, None);
    assert_eq!(run.session_id, None);

    // Never a running row.
    assert!(agent_brain::db::running_runs(&conn).unwrap().is_empty());
}

#[test]
fn insert_human_run_stores_outcome_as_given() {
    let (_tmp, conn, proj_id, task_id) = fresh_task_db();
    let mut h = sample_human_run(task_id, proj_id);
    h.outcome = Some("protocol_violation".to_string());

    let id = agent_brain::db::insert_human_run(&conn, &h).unwrap();
    let run = agent_brain::db::get_run(&conn, id).unwrap().unwrap();
    assert_eq!(run.outcome.as_deref(), Some("protocol_violation"));
    assert_eq!(run.status, "succeeded");
}

#[test]
fn human_run_not_reconciled() {
    let (_tmp, conn, proj_id, task_id) = fresh_task_db();
    let id = agent_brain::db::insert_human_run(&conn, &sample_human_run(task_id, proj_id)).unwrap();

    let lost = agent_brain::runner::reconcile_stale_runs(&conn).unwrap();
    assert_eq!(lost, 0);
    let run = agent_brain::db::get_run(&conn, id).unwrap().unwrap();
    assert_eq!(run.status, "succeeded");
    assert_eq!(run.engine, HUMAN_ENGINE_NFC);
}

/// A v12 DB: the v11 fixture plus migration 12 applied by hand (journal tables).
fn build_v12(path: &Path) {
    build_v11(path);
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(agent_brain::db::MIGRATIONS[11]).unwrap();
    conn.pragma_update(None, "user_version", 12).unwrap();
}

fn object_exists(conn: &rusqlite::Connection, kind: &str, name: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = ?1 AND name = ?2",
        [kind, name],
        |r| r.get::<_, i64>(0),
    )
    .unwrap()
        == 1
}

#[test]
fn v12_db_upgrades_to_v13_keeping_data() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v12(&db_path);

    let conn = agent_brain::db::open(&db_path).expect("open v12 DB upgrades to v13");

    assert_eq!(user_version(&conn), 15);
    assert_eq!(user_version(&conn), agent_brain::db::SCHEMA_VERSION);
    for table in [
        "projects",
        "tasks",
        "runs",
        "events",
        "intake",
        "proposals",
        "plan_approvals",
    ] {
        assert_eq!(count_rows(&conn, table), 1, "{table} keeps its row");
    }
    assert!(object_exists(&conn, "table", "queued_actions"));
    assert!(object_exists(&conn, "index", "queued_actions_one_waiting"));
    assert_eq!(count_rows(&conn, "queued_actions"), 0);
    assert_eq!(fk_violation_count(&conn), 0);
}

/// A v13 DB: the v12 fixture plus migration 13 applied by hand (queued_actions).
fn build_v13(path: &Path) {
    build_v12(path);
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(agent_brain::db::MIGRATIONS[12]).unwrap();
    conn.pragma_update(None, "user_version", 13).unwrap();
}

/// A v14 DB: the v13 fixture plus migration 14 applied by hand (cloned_from, request).
fn build_v14(path: &Path) {
    build_v13(path);
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(agent_brain::db::MIGRATIONS[13]).unwrap();
    conn.pragma_update(None, "user_version", 14).unwrap();
}

fn column_exists(conn: &rusqlite::Connection, table: &str, column: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
        [table, column],
        |r| r.get::<_, i64>(0),
    )
    .unwrap()
        == 1
}

#[test]
fn v13_db_upgrades_to_v14_keeping_data() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v13(&db_path);
    {
        let old = rusqlite::Connection::open(&db_path).unwrap();
        assert!(!column_exists(&old, "tasks", "cloned_from"));
        assert!(!column_exists(&old, "tasks", "request"));
    }
    let before = snapshot(&rusqlite::Connection::open(&db_path).unwrap());

    let conn = agent_brain::db::open(&db_path).expect("open v13 DB upgrades to v14");

    assert_eq!(user_version(&conn), 15);
    assert_eq!(user_version(&conn), agent_brain::db::SCHEMA_VERSION);
    assert!(column_exists(&conn, "tasks", "cloned_from"));
    assert!(column_exists(&conn, "tasks", "request"));
    // Existing rows keep their data; the new columns read NULL (no backfill).
    assert_eq!(snapshot(&conn), before);
    for table in [
        "projects",
        "tasks",
        "runs",
        "events",
        "intake",
        "proposals",
        "plan_approvals",
    ] {
        assert_eq!(count_rows(&conn, table), 1, "{table} keeps its row");
    }
    assert_eq!(agent_brain::db::task_request(&conn, 1).unwrap(), None);
    assert_eq!(agent_brain::db::task_cloned_from(&conn, 1).unwrap(), None);
    assert_eq!(fk_violation_count(&conn), 0);

    // The new columns round-trip, and cloned_from is a real foreign key to tasks(id).
    agent_brain::db::set_task_request(&conn, 1, "sửa app").unwrap();
    assert_eq!(
        agent_brain::db::task_request(&conn, 1).unwrap().as_deref(),
        Some("sửa app")
    );
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, status, created_at, updated_at, level)
         VALUES (2, 1, 'task-1-r2', 'Task 1', 'task/1-r2', 'open', 3000, 3000, 'vua')",
        [],
    )
    .unwrap();
    agent_brain::db::set_task_cloned_from(&conn, 2, 1).unwrap();
    assert_eq!(agent_brain::db::task_cloned_from(&conn, 2).unwrap(), Some(1));
    assert_eq!(agent_brain::db::task_cloned_from(&conn, 1).unwrap(), None);
    assert!(
        conn.execute("UPDATE tasks SET cloned_from = 999 WHERE id = 2", [])
            .is_err(),
        "cloned_from must reference an existing task"
    );
}

#[test]
fn v14_db_upgrades_to_v15_keeping_data() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v14(&db_path);
    {
        let old = rusqlite::Connection::open(&db_path).unwrap();
        assert!(!column_exists(&old, "tasks", "parent_id"));
        assert!(!object_exists(&old, "table", "split_items"));
        assert!(!object_exists(&old, "index", "tasks_parent"));
        assert!(!object_exists(&old, "index", "split_items_waiting"));
    }
    let before = snapshot(&rusqlite::Connection::open(&db_path).unwrap());

    let conn = agent_brain::db::open(&db_path).expect("open v14 DB upgrades to v15");

    assert_eq!(user_version(&conn), 15);
    assert_eq!(user_version(&conn), agent_brain::db::SCHEMA_VERSION);
    assert!(column_exists(&conn, "tasks", "parent_id"));
    assert!(object_exists(&conn, "table", "split_items"));
    assert!(object_exists(&conn, "index", "tasks_parent"));
    assert!(object_exists(&conn, "index", "split_items_waiting"));

    // Existing rows keep their data; the new column reads NULL.
    assert_eq!(snapshot(&conn), before);
    for table in [
        "projects",
        "tasks",
        "runs",
        "events",
        "intake",
        "proposals",
        "plan_approvals",
    ] {
        assert_eq!(count_rows(&conn, table), 1, "{table} keeps its row");
    }
    assert_eq!(agent_brain::db::task_parent(&conn, 1).unwrap(), None);
    assert_eq!(fk_violation_count(&conn), 0);

    // The new column round-trips, and parent_id is a real foreign key to tasks(id).
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, status, created_at, updated_at, level)
         VALUES (2, 1, 'task-child', 'Child Task', 'task/child', 'open', 3000, 3000, 'vua')",
        [],
    )
    .unwrap();
    agent_brain::db::set_task_parent(&conn, 2, 1).unwrap();
    assert_eq!(agent_brain::db::task_parent(&conn, 2).unwrap(), Some(1));
    assert_eq!(agent_brain::db::task_parent(&conn, 1).unwrap(), None);
    assert!(
        conn.execute("UPDATE tasks SET parent_id = 999 WHERE id = 2", [])
            .is_err(),
        "parent_id must reference an existing task"
    );
}

#[test]
fn queued_actions_unique_index_rejects_two_waiting_rows_for_one_task() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v12(&db_path);
    let conn = agent_brain::db::open(&db_path).unwrap();

    let insert = |action: &str, status: &str| {
        conn.execute(
            "INSERT INTO queued_actions (task_id, action_id, source, status, created_at, updated_at)
             VALUES (1, ?1, 'ui', ?2, 1, 1)",
            [action, status],
        )
    };
    insert("run_dev", "waiting").unwrap();
    assert!(insert("relead", "waiting").is_err(), "second waiting row");
    // Rows in other states do not count against the unique index.
    insert("relead", "cancelled").unwrap();
    insert("relead", "failed").unwrap();
    insert("run_qa", "started").unwrap();
    assert_eq!(count_rows(&conn, "queued_actions"), 4);
}

#[test]
fn queued_actions_check_rejects_unknown_action_and_status() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v12(&db_path);
    let conn = agent_brain::db::open(&db_path).unwrap();

    let bad_action = conn.execute(
        "INSERT INTO queued_actions (task_id, action_id, source, created_at, updated_at)
         VALUES (1, 'approve', 'ui', 1, 1)",
        [],
    );
    assert!(bad_action.is_err(), "unknown action_id");
    let bad_status = conn.execute(
        "INSERT INTO queued_actions (task_id, action_id, source, status, created_at, updated_at)
         VALUES (1, 'run_dev', 'ui', 'done', 1, 1)",
        [],
    );
    assert!(bad_status.is_err(), "unknown status");
    assert_eq!(count_rows(&conn, "queued_actions"), 0);
}

#[test]
fn queued_actions_helpers_replace_waiting_row_and_order_fifo() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("brain.db");
    build_v12(&db_path);
    let conn = agent_brain::db::open(&db_path).unwrap();
    // A second task so FIFO has two entries.
    conn.execute(
        "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level)
         VALUES (2, 1, 'task-2', 'Task 2', 'task/2', 'main', 'base2', 'open', 1000, 1000, 'vua')",
        [],
    )
    .unwrap();

    let first = agent_brain::db::enqueue_action(&conn, 1, "run_dev", None, "ui").unwrap();
    let second =
        agent_brain::db::enqueue_action(&conn, 2, "run_qa", Some("ghi chú"), "ui").unwrap();
    let replaced = agent_brain::db::enqueue_action(&conn, 1, "relead", Some("x"), "ui").unwrap();
    assert_ne!(first, replaced);

    assert_eq!(count_rows(&conn, "queued_actions"), 3, "replaced row is kept");
    let old_status: String = conn
        .query_row(
            "SELECT status FROM queued_actions WHERE id = ?1",
            [first],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(old_status, "cancelled");

    let waiting = agent_brain::db::waiting_actions(&conn).unwrap();
    assert_eq!(waiting.len(), 2, "one waiting row per task");
    assert_eq!(waiting[0].id, second, "oldest first");
    assert_eq!(waiting[1].id, replaced);
    assert_eq!(waiting[1].action_id, "relead");

    let for_task = agent_brain::db::waiting_action_for_task(&conn, 1)
        .unwrap()
        .unwrap();
    assert_eq!(for_task.id, replaced);

    assert!(agent_brain::db::retarget_queued(
        &conn,
        replaced,
        "run_qa",
        Some("n"),
        Some("lý do")
    )
    .unwrap());
    let r = agent_brain::db::waiting_action_for_task(&conn, 1)
        .unwrap()
        .unwrap();
    assert_eq!(
        (r.action_id.as_str(), r.note.as_deref(), r.reason.as_deref()),
        ("run_qa", Some("n"), Some("lý do"))
    );

    assert!(agent_brain::db::set_queued_status(&conn, replaced, "cancelled", None).unwrap());
    assert!(agent_brain::db::waiting_action_for_task(&conn, 1)
        .unwrap()
        .is_none());
    // A non-waiting row cannot be retargeted.
    assert!(!agent_brain::db::retarget_queued(&conn, replaced, "run_dev", None, None).unwrap());
}
