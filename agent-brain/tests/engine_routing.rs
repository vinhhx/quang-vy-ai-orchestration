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

// ---------------------------------------------------------------------------
// Task 2: `auto --start --team`, refusals, legacy / missing / waiting teams
// ---------------------------------------------------------------------------

fn dev_impl_entry() -> Value {
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
        {"op": "git", "args": ["add", "src/app.py"]},
        {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
    ]})
}

fn run_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap()
}

fn task_auto_state(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT auto_state FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
}

fn set_team_json(sb: &Sandbox, json_text: Option<&str>) {
    sb.db()
        .execute("UPDATE tasks SET team_json = ?1 WHERE id = 1", [json_text])
        .unwrap();
}

/// `run lead` creates task 1 (`sua-app`) on `task/sua-app`, with the plan committed.
fn create_task(sb: &Sandbox) {
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
    assert_ok(&res, "run lead");
}

#[test]
fn auto_team_only_with_start() {
    let sb = Sandbox::new();
    setup_auto_project(&sb, "work");

    let res = sb.run(&["auto", "1", "--team", "x"], None);
    assert_ne!(res.code, 0);
    assert!(
        res.stderr.contains("--team chỉ dùng với --start"),
        "stderr: {}",
        res.stderr
    );
}

#[test]
fn auto_start_records_team() {
    let sb = Sandbox::new();
    write_doi_claude(&sb);
    setup_auto_project(&sb, "work");

    let res = sb.run(
        &[
            "auto",
            "--start",
            "--team",
            "doi-claude",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(gate_scenario()),
    );
    assert_ok(&res, "auto --start --team");
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("gate"));
    assert_eq!(task_team(&sb, 1).name, "doi-claude");

    // The plan-review child took the team from the task row, not from argv.
    let (_, metrics) = run_of(&sb, "plan-review");
    assert_eq!(metrics["team"], json!("doi-claude"), "metrics: {metrics}");
    for rec in sb.records() {
        let argv = rec["argv"].to_string();
        assert!(
            !argv.contains("--team"),
            "claude argv carries the team: {argv}"
        );
    }
}

#[test]
fn auto_start_unknown_team_refused() {
    let sb = Sandbox::new();
    setup_auto_project(&sb, "work");

    let res = sb.run(
        &[
            "auto",
            "--start",
            "--team",
            "khong-co",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(gate_scenario()),
    );
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(
        res.stderr.contains("Không có đội hình 'khong-co'"),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(task_count(&sb), 0);
    assert!(sb.records().is_empty());
}

#[test]
fn legacy_task_runs_claude() {
    let sb = Sandbox::new();
    setup_auto_project(&sb, "work");
    create_task(&sb);
    set_team_json(&sb, None);

    let res = sb.run(
        &["run", "dev", "--", "web-ban-hang"],
        Some(dev_impl_entry()),
    );
    assert_ok(&res, "run dev (legacy task)");

    let (engine, metrics) = run_of(&sb, "dev");
    assert_eq!(engine, "claude");
    assert_eq!(
        metrics.get("team"),
        Some(&Value::Null),
        "metrics: {metrics}"
    );
    assert!(res.stderr.contains("· engine claude · model sonnet"));
}

#[test]
fn missing_team_refused_before_run() {
    let sb = Sandbox::new();
    setup_auto_project(&sb, "work");
    create_task(&sb);
    set_team_json(&sb, Some(r#"{"name":"da-xoa","history":[]}"#));
    let runs_before = run_count(&sb);
    let records_before = sb.records().len();

    let res = sb.run(
        &["run", "dev", "--", "web-ban-hang"],
        Some(dev_impl_entry()),
    );
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(
        res.stderr.contains(
            "Đội hình 'da-xoa' của task không còn trong teams.json — đổi đội hình rồi chạy lại."
        ),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(run_count(&sb), runs_before, "no run row for a refused step");
    assert_eq!(sb.records().len(), records_before, "nothing was spawned");
}

#[test]
fn waiting_marker_blocks_other_roles() {
    let sb = Sandbox::new();
    sb.write_staff(json!({"ha": {"display_name": "Hà"}}));
    setup_auto_project(&sb, "work");
    create_task(&sb);
    let dev = sb.run(
        &["run", "dev", "--", "web-ban-hang"],
        Some(dev_impl_entry()),
    );
    assert_ok(&dev, "run dev");

    let waiting = json!({
        "name": "mac-dinh",
        "history": [],
        "wait": {
            "mode": "waiting",
            "role": "dev",
            "stage": "implement",
            "staff": "ha",
            "since_ms": 1,
            "branch": "task/sua-app",
            "base": "0000000000000000000000000000000000000000"
        }
    });
    set_team_json(&sb, Some(&waiting.to_string()));
    let runs_before = run_count(&sb);

    let res = sb.run(&["run", "qa", "--", "web-ban-hang"], None);
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(
        res.stderr.contains("Task đang chờ Hà làm bước Code"),
        "stderr: {}",
        res.stderr
    );
    assert!(res.stderr.contains("agent-brain human done 1"));
    assert_eq!(run_count(&sb), runs_before);
}

/// The record of the last fake-claude call with the sandbox root and the per-run settings
/// path removed, so two sandboxes compare equal.
fn normalized_last_record(sb: &Sandbox) -> Value {
    let root = sb.root.to_string_lossy().to_string();
    let rec = sb.records().pop().expect("a fake-claude record");
    let mut argv: Vec<String> = rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().replace(&root, "<ROOT>"))
        .collect();
    if let Some(i) = argv.iter().position(|a| a == "--settings") {
        argv.remove(i + 1);
        argv.remove(i);
    }
    let norm = |v: &Value| v.as_str().map(|s| s.replace(&root, "<ROOT>"));
    json!({
        "argv": argv,
        "cwd": norm(&rec["cwd"]),
        "claude_config_dir": norm(&rec["claude_config_dir"]),
        "present_vars": rec["present_vars"],
    })
}

#[test]
fn all_claude_team_matches_legacy_record() {
    // Team mac-dinh (every role claude).
    let with_team = Sandbox::new();
    setup_auto_project(&with_team, "personal");
    create_task(&with_team);
    assert_eq!(task_team(&with_team, 1).name, "mac-dinh");
    let res = with_team.run(
        &["run", "dev", "--", "web-ban-hang"],
        Some(dev_impl_entry()),
    );
    assert_ok(&res, "run dev (team)");
    assert!(res.stderr.contains("· engine claude"));
    let team_record = normalized_last_record(&with_team);

    // Legacy task: no team at all.
    let legacy = Sandbox::new();
    setup_auto_project(&legacy, "personal");
    create_task(&legacy);
    set_team_json(&legacy, None);
    let res = legacy.run(
        &["run", "dev", "--", "web-ban-hang"],
        Some(dev_impl_entry()),
    );
    assert_ok(&res, "run dev (legacy)");
    let legacy_record = normalized_last_record(&legacy);

    assert_eq!(team_record, legacy_record);
    assert!(
        team_record["claude_config_dir"].is_string(),
        "the personal profile env must be applied: {team_record}"
    );
}

// ---------------------------------------------------------------------------
// Task 3: engine-first pricing
// ---------------------------------------------------------------------------

#[test]
fn claude_cost_unchanged() {
    // A `pricing.deepseek` entry must not change what a claude run stores.
    let sb = Sandbox::new();
    sb.write_config(json!({
        "pricing": {
            "opus": {"input": 4.0, "output": 20.0, "cache_read": 0.2, "cache_write": 8.0},
            "sonnet": {"input": 2.0, "output": 10.0, "cache_read": 0.1, "cache_write": 4.0},
            "deepseek": {"input": 0.27, "output": 1.1, "cache_read": 0.07, "cache_write": 0.27}
        }
    }));
    setup_auto_project(&sb, "work");

    let mut entry = lead_plan_entry();
    entry["result_cost"] = json!(1.25);
    entry["result_tokens"] = json!({"input": 1000, "output": 500});
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
        Some(entry),
    );
    assert_ok(&res, "run lead");

    let (engine, cost): (String, f64) = sb
        .db()
        .query_row(
            "SELECT engine, cost_usd FROM runs WHERE role = 'lead'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(engine, "claude");
    assert!((cost - 1.25).abs() < 1e-9, "stored cost {cost}");
}
