//! Phase 05.4 plan 02: the hidden `--origin telegram` flag. `run lead` and
//! `auto --start` record `tasks.origin` before the Lead agent runs, later
//! rounds keep it, and the flag is refused where it makes no sense. Every test
//! uses its own Sandbox (temp brain.db, fake claude); nothing contacts Telegram.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{RunOut, Sandbox};
use serde_json::{json, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Scenario helpers (copied from tests/telegram_actions.rs)
// ---------------------------------------------------------------------------

fn review_report(verdict: &str, blocking: i64, advisory: i64) -> String {
    let mut body = format!(
        "---\nverdict: {verdict}\nblocking: {blocking}\nadvisory: {advisory}\nfiles: []\n---\n# Review: sửa app\n## Mục chặn (blocking)\n"
    );
    for i in 0..blocking {
        body.push_str(&format!("- mục chặn {}\n", i + 1));
    }
    body.push_str("## Góp ý (advisory)\n");
    for i in 0..advisory {
        body.push_str(&format!("- góp ý {}\n", i + 1));
    }
    body
}

fn plan_review_entry(verdict: &str, blocking: i64, advisory: i64) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan-review.md",
        "content": review_report(verdict, blocking, advisory),
    }]})
}

fn lead_plan_entry() -> Value {
    protocol::lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])
}

fn by_role(roles: Value) -> Value {
    json!({ "by_role": roles })
}

/// Repo `web-ban-hang` with an entry without `workflow` (server default: mac-dinh).
fn setup_auto_project(sb: &Sandbox, profile: &str) -> PathBuf {
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", profile);
    entry.as_object_mut().unwrap().remove("workflow");
    sb.write_map(json!([entry]), &[]);
    repo
}

fn gate_scenario() -> Value {
    by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry("approve", 0, 2)],
    }))
}

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

// ---------------------------------------------------------------------------
// DB probes
// ---------------------------------------------------------------------------

fn task_origin_of(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row("SELECT origin FROM tasks WHERE id = ?1", [task_id], |r| {
            r.get::<_, Option<String>>(0)
        })
        .unwrap()
}

/// 0 also while the database or its tasks table does not exist yet.
fn task_rows(sb: &Sandbox) -> i64 {
    if !sb.db_path.is_file() {
        return 0;
    }
    sb.db()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap_or(0)
}

fn auto_state_of(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT auto_state FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
}

const LEAD_ARGS: [&str; 3] = ["--name", "sua-app", "--"];

fn run_lead(sb: &Sandbox, origin: Option<&str>, sc: Value) -> RunOut {
    let mut args: Vec<&str> = vec!["run", "lead"];
    if let Some(o) = origin {
        args.push("--origin");
        args.push(o);
    }
    args.extend_from_slice(&LEAD_ARGS);
    args.extend_from_slice(&["web-ban-hang", "sửa app"]);
    sb.run(&args, Some(sc))
}

// ---------------------------------------------------------------------------
// run lead
// ---------------------------------------------------------------------------

#[test]
fn run_lead_origin_telegram_recorded() {
    let sb = Sandbox::new();
    protocol::setup_project(&sb, "work");

    let res = run_lead(&sb, Some("telegram"), lead_plan_entry());
    assert_ok(&res, "run lead --origin telegram");

    assert_eq!(task_origin_of(&sb, 1).as_deref(), Some("telegram"));
}

#[test]
fn run_lead_without_origin_is_null() {
    let sb = Sandbox::new();
    protocol::setup_project(&sb, "work");

    let res = run_lead(&sb, None, lead_plan_entry());
    assert_ok(&res, "run lead");

    assert_eq!(task_rows(&sb), 1);
    assert_eq!(task_origin_of(&sb, 1), None);
}

#[test]
fn replan_keeps_origin() {
    let sb = Sandbox::new();
    protocol::setup_project(&sb, "work");

    let first = run_lead(&sb, Some("telegram"), lead_plan_entry());
    assert_ok(&first, "first run lead");
    assert_eq!(task_origin_of(&sb, 1).as_deref(), Some("telegram"));

    // Re-plan on the checked-out task branch, no flag: the mark must stay.
    let second = run_lead(&sb, None, lead_plan_entry());
    assert_ok(&second, "second run lead");

    assert_eq!(task_rows(&sb), 1);
    assert_eq!(task_origin_of(&sb, 1).as_deref(), Some("telegram"));
}

#[test]
fn origin_value_restricted() {
    let sb = Sandbox::new();
    protocol::setup_project(&sb, "work");

    let res = run_lead(&sb, Some("dashboard"), lead_plan_entry());
    assert_ne!(res.code, 0, "stdout: {}", res.stdout);
    assert!(
        res.stderr.contains("telegram"),
        "clap should list the possible value: {}",
        res.stderr
    );
    assert_eq!(task_rows(&sb), 0);
}

#[test]
fn origin_only_for_lead() {
    let sb = Sandbox::new();
    protocol::setup_project(&sb, "work");

    let res = sb.run(
        &["run", "dev", "--origin", "telegram", "--", "web-ban-hang"],
        None,
    );
    assert_ne!(res.code, 0, "stdout: {}", res.stdout);
    assert!(
        res.stderr.contains("--origin chỉ dùng với vai trò lead"),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(task_rows(&sb), 0);
}

// ---------------------------------------------------------------------------
// auto
// ---------------------------------------------------------------------------

#[test]
fn auto_start_forwards_origin() {
    let sb = Sandbox::new();
    setup_auto_project(&sb, "work");

    let res = sb.run(
        &[
            "auto",
            "--start",
            "--origin",
            "telegram",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(gate_scenario()),
    );
    assert_ok(&res, "auto --start --origin telegram");

    assert_eq!(auto_state_of(&sb, 1).as_deref(), Some("gate"));
    assert_eq!(task_origin_of(&sb, 1).as_deref(), Some("telegram"));
    assert!(
        !sb.records().is_empty(),
        "the fake claude should have recorded the Lead call"
    );
}

#[test]
fn auto_origin_only_with_start() {
    let sb = Sandbox::new();
    setup_auto_project(&sb, "work");

    let res = sb.run(&["auto", "1", "--origin", "telegram"], None);
    assert_ne!(res.code, 0, "stdout: {}", res.stdout);
    assert!(
        res.stderr.contains("--origin chỉ dùng với --start"),
        "stderr: {}",
        res.stderr
    );
}

// ---------------------------------------------------------------------------
// v2 -> v3 through the binary
// ---------------------------------------------------------------------------

#[test]
fn cli_upgrades_v2_db() {
    let sb = Sandbox::new();
    protocol::setup_project(&sb, "work");

    // A v2 database exactly where the binary will look for it.
    std::fs::create_dir_all(sb.db_path.parent().unwrap()).unwrap();
    {
        let conn = rusqlite::Connection::open(&sb.db_path).unwrap();
        conn.execute_batch(agent_brain::db::MIGRATIONS[0]).unwrap();
        conn.execute_batch(agent_brain::db::MIGRATIONS[1]).unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        conn.execute(
            "INSERT INTO projects (id, slug, path, name_vi, aliases_json, type, profile, repo, missing)
             VALUES (1, 'cu', '/path/cu', 'Dự án cũ', '[]', 'code', 'work', NULL, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at)
             VALUES (1, 1, 'task-cu', 'Task cũ', 'task/cu', 'main', 'base1', 'open', 1000, 1000)",
            [],
        )
        .unwrap();
    }

    let res = sb.run(&["status", "web-ban-hang"], None);
    assert_ok(&res, "status on a v2 database");

    let conn = sb.db();
    let ver: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(ver, 3);
    let (title, origin): (String, Option<String>) = conn
        .query_row("SELECT title, origin FROM tasks WHERE id = 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(title, "Task cũ");
    assert_eq!(origin, None);
}
