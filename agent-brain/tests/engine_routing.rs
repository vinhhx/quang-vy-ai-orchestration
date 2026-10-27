//! Phase 6 plan 05: every role run decides its engine at one seam. A task carries its team
//! from creation (`--team`, the project default, `mac-dinh`); the run row records the engine
//! and the team, and the log header shows the engine. All of it runs against the fake claude.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::teams::TaskTeam;
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

fn gate_scenario() -> Value {
    by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry("approve", 0, 2)],
    }))
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

// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

fn team_json(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT team_json FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .unwrap()
}

fn task_team(sb: &Sandbox, task_id: i64) -> TaskTeam {
    TaskTeam::parse(team_json(sb, task_id).as_deref())
        .unwrap_or_else(|| panic!("task {task_id} has no parsable team_json"))
}

fn task_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap()
}

/// `(engine, metrics_json)` of the first run with `role`.
fn run_of(sb: &Sandbox, role: &str) -> (String, Value) {
    let (engine, metrics): (String, Option<String>) = sb
        .db()
        .query_row(
            "SELECT engine, metrics_json FROM runs WHERE role = ?1 ORDER BY id LIMIT 1",
            [role],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or_else(|e| panic!("no {role} run row: {e}"));
    let metrics = serde_json::from_str(&metrics.unwrap_or_else(|| "{}".to_string())).unwrap();
    (engine, metrics)
}

fn write_doi_claude(sb: &Sandbox) {
    sb.write_teams(json!({"doi-claude": {"description": "Toàn claude", "roles": {}}}));
}

// ---------------------------------------------------------------------------
// Task 1: `run lead --team` and the engine seam
// ---------------------------------------------------------------------------

#[test]
fn team_flag_routes_and_records() {
    let sb = Sandbox::new();
    write_doi_claude(&sb);
    setup_auto_project(&sb, "work");

    let res = sb.run(
        &[
            "run",
            "lead",
            "--team",
            "doi-claude",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead --team");

    let team = task_team(&sb, 1);
    assert_eq!(team.name, "doi-claude");
    assert!(team.history.is_empty());
    assert!(team.wait.is_none());

    let (engine, metrics) = run_of(&sb, "lead");
    assert_eq!(engine, "claude");
    assert_eq!(metrics["team"], json!("doi-claude"), "metrics: {metrics}");
    assert!(
        res.stderr.contains("· engine claude · model opus"),
        "header lacks the engine: {}",
        res.stderr
    );
}

#[test]
fn project_default_team() {
    // Map entry with a team key and no flag: the project default applies.
    let sb = Sandbox::new();
    write_doi_claude(&sb);
    let repo = setup_auto_project(&sb, "work");
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    entry.as_object_mut().unwrap().remove("workflow");
    entry["team"] = json!("doi-claude");
    sb.write_map(json!([entry]), &[]);
    let res = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead (project team)");
    assert_eq!(task_team(&sb, 1).name, "doi-claude");

    // An unknown project team falls back to mac-dinh with a note.
    let sb = Sandbox::new();
    write_doi_claude(&sb);
    let repo = setup_auto_project(&sb, "work");
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    entry.as_object_mut().unwrap().remove("workflow");
    entry["team"] = json!("khong-co");
    sb.write_map(json!([entry]), &[]);
    let res = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead (unknown project team)");
    assert!(
        res.stderr
            .contains("Đội hình 'khong-co' của dự án không có trong teams.json — dùng mac-dinh."),
        "missing fallback note: {}",
        res.stderr
    );
    assert_eq!(task_team(&sb, 1).name, "mac-dinh");

    // No team key at all: mac-dinh.
    let sb = Sandbox::new();
    setup_auto_project(&sb, "work");
    let res = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead (no team key)");
    assert_eq!(task_team(&sb, 1).name, "mac-dinh");
}

#[test]
fn explicit_unknown_team_refused() {
    let sb = Sandbox::new();
    let repo = setup_auto_project(&sb, "work");

    let res = sb.run(
        &[
            "run",
            "lead",
            "--team",
            "khong-co",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_plan_entry()),
    );
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(
        res.stderr.contains("Không có đội hình 'khong-co'"),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(task_count(&sb), 0);
    assert_eq!(
        sb.git(&repo, &["branch", "--list", "task/sua-app"]).trim(),
        ""
    );
    assert!(sb.records().is_empty(), "no process may be spawned");
}

#[test]
fn replan_keeps_team() {
    let sb = Sandbox::new();
    write_doi_claude(&sb);
    setup_auto_project(&sb, "work");

    let first = sb.run(
        &[
            "run",
            "lead",
            "--team",
            "doi-claude",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&first, "first run lead");

    let second = sb.run(
        &[
            "run",
            "lead",
            "--team",
            "mac-dinh",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&second, "re-plan");
    assert!(
        second
            .stderr
            .contains("Task đã có đội hình 'doi-claude' — bỏ qua --team."),
        "stderr: {}",
        second.stderr
    );
    assert_eq!(task_team(&sb, 1).name, "doi-claude");
}

#[test]
fn team_flag_only_lead() {
    let sb = Sandbox::new();
    setup_auto_project(&sb, "work");

    let res = sb.run(&["run", "dev", "--team", "x", "--", "web-ban-hang"], None);
    assert_ne!(res.code, 0);
    assert!(
        res.stderr.contains("--team chỉ dùng với vai trò lead"),
        "stderr: {}",
        res.stderr
    );
}
