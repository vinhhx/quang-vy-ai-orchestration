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
    assert_eq!(user_ver, 2);

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
    assert_eq!(user_ver, 2);

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
    raw_conn.pragma_update(None, "user_version", 3).unwrap();
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
    assert!(
        err_msg.contains("có phiên bản 3"),
        "error message should contain 'có phiên bản 3': {err_msg}"
    );
    assert!(
        err_msg.contains("hỗ trợ tới 2"),
        "error message should contain 'hỗ trợ tới 2': {err_msg}"
    );

    let raw_conn = rusqlite::Connection::open(&db_path).unwrap();
    let ver_after: i64 = raw_conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(ver_after, 3);

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
    assert_eq!(final_ver, 2);

    let ev_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ev_count, 24);
}
