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

// ---------------------------------------------------------------------------
// Plan 06-11: the engine list (D-30) — `anthropic` entries, ids in teams and in `runs.engine`
// ---------------------------------------------------------------------------

const PLANTED_GW_KEY: &str = "test-planted-gw-key-0123456789";

fn fake_cmd() -> String {
    Sandbox::fake_claude().to_str().unwrap().to_string()
}

/// A gateway entry: `anthropic` kind with `base_url`, run by the fake claude.
fn gateway_entry(id: &str, base_url: &str, key_env: &str) -> Value {
    json!({
        "id": id,
        "kind": "anthropic",
        "command": fake_cmd(),
        "base_url": base_url,
        "key_env": key_env,
        "models": {
            "lead": "deepseek/deepseek-reasoner",
            "dev": "deepseek/deepseek-chat",
            "qa": "deepseek/deepseek-chat"
        },
        "verified": "test"
    })
}

fn write_map_of(sb: &Sandbox, entries: Vec<Value>) {
    sb.write_map(Value::Array(entries), &[]);
}

/// Map entry with the given default team and no workflow (the manual flow).
fn team_project_entry(
    sb: &Sandbox,
    repo: &std::path::Path,
    id: &str,
    profile: &str,
    team: &str,
) -> Value {
    let mut entry = sb.project_entry(repo, id, profile);
    let obj = entry.as_object_mut().unwrap();
    obj.remove("workflow");
    obj.insert("team".to_string(), json!(team));
    entry
}

fn app_repo(sb: &Sandbox, name: &str) -> PathBuf {
    sb.make_repo(
        name,
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    )
}

fn dev_entry_for(name: &str) -> Value {
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
        {"op": "git", "args": ["add", "src/app.py"]},
        {"op": "git", "args": ["commit", "-m", format!("{name}: sửa app")]}
    ]})
}

fn qa_entry() -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".qa-report.md",
        "content": "# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- src/app.py: khớp.\n## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông phát hiện rủi ro.\n## Kết luận\nĐạt\n",
        "keep_meta": true
    }]})
}

fn rec_argv(rec: &Value) -> Vec<String> {
    rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

fn rec_env_names(rec: &Value) -> Vec<String> {
    rec["env_names"]
        .as_array()
        .unwrap_or_else(|| panic!("record has no env_names: {rec}"))
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

fn engines_of_runs(sb: &Sandbox) -> Vec<String> {
    let db = sb.db();
    let mut stmt = db.prepare("SELECT engine FROM runs ORDER BY id").unwrap();
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
    rows.map(|r| r.unwrap()).collect()
}

#[test]
fn tiet_kiem_routes_lead_dev_qa() {
    let sb = Sandbox::new();
    sb.write_config(json!({"engines": [
        gateway_entry("vcadm-deepseek", "https://gateway.invalid/proxyllm", "VCADM_GATEWAY_KEY"),
        {
            "id": "deepseek", "kind": "anthropic", "command": fake_cmd(),
            "base_url": "https://api.deepseek.com/anthropic", "key_env": "DEEPSEEK_API_KEY",
            "models": {"dev": "sonnet"}
        }
    ]}));
    // The D-43 override path: the built-in `tiet-kiem` with Dev on the configured gateway id.
    sb.write_teams(json!({"tiet-kiem": {
        "description": "Dev qua gateway công ty.",
        "roles": {"dev": "vcadm-deepseek"}
    }}));
    let repo = app_repo(&sb, "web-ban-hang");
    write_map_of(
        &sb,
        vec![team_project_entry(
            &sb,
            &repo,
            "web-ban-hang",
            "work",
            "tiet-kiem",
        )],
    );

    let env = [
        ("VCADM_GATEWAY_KEY", PLANTED_GW_KEY),
        ("FAKE_CLAUDE_EXPECT_TOKEN", PLANTED_GW_KEY),
        ("ANTHROPIC_MODEL", "x"),
        ("DEEPSEEK_API_KEY", "y"),
    ];

    let res = sb.run_env(
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
        &env,
    );
    assert_ok(&res, "run lead");
    let res = sb.run_env(
        &["run", "dev", "--", "web-ban-hang"],
        Some(dev_entry_for("sua-app")),
        &env,
    );
    assert_ok(&res, "run dev");
    assert!(
        res.stderr
            .contains("· engine vcadm-deepseek · model deepseek/deepseek-chat"),
        "dev header: {}",
        res.stderr
    );
    let res = sb.run_env(&["run", "qa", "--", "web-ban-hang"], Some(qa_entry()), &env);
    assert_ok(&res, "run qa");

    assert_eq!(
        engines_of_runs(&sb),
        ["claude", "vcadm-deepseek", "claude"],
        "one run per role, each on its own engine"
    );

    let recs = sb.records();
    assert_eq!(recs.len(), 3, "records: {recs:?}");

    let dev = &recs[1];
    assert_eq!(
        dev["anthropic_base_url"],
        json!("https://gateway.invalid/proxyllm")
    );
    assert_eq!(dev["auth_token_matches_expected"], json!(true));
    let argv = rec_argv(dev);
    let m = argv.iter().position(|a| a == "--model").unwrap();
    assert_eq!(argv[m + 1], "deepseek/deepseek-chat");
    for flag in [
        "-p",
        "--output-format",
        "--verbose",
        "--settings",
        "--append-system-prompt",
        "--allowedTools",
        "--disallowedTools",
    ] {
        assert!(
            argv.iter().any(|a| a == flag),
            "dev argv lacks {flag}: {argv:?}"
        );
    }
    let names = rec_env_names(dev);
    for gone in [
        "VCADM_GATEWAY_KEY",
        "DEEPSEEK_API_KEY",
        "ANTHROPIC_MODEL",
        "ANTHROPIC_API_KEY",
    ] {
        assert!(!names.iter().any(|n| n == gone), "dev env still has {gone}");
    }
    assert!(names.iter().any(|n| n == "ANTHROPIC_AUTH_TOKEN"));
    assert!(names
        .iter()
        .any(|n| n == "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"));

    for rec in [&recs[0], &recs[2]] {
        assert_eq!(rec["anthropic_base_url"], Value::Null);
        assert_eq!(rec["auth_token_matches_expected"], Value::Null);
        let names = rec_env_names(rec);
        for gone in ["VCADM_GATEWAY_KEY", "DEEPSEEK_API_KEY"] {
            assert!(
                !names.iter().any(|n| n == gone),
                "claude run still has {gone}: {names:?}"
            );
        }
    }
    // The planted key never reaches what is stored for the run.
    let (_, metrics) = run_of(&sb, "dev");
    assert!(!metrics.to_string().contains(PLANTED_GW_KEY));
}

#[test]
fn two_anthropic_endpoints() {
    let sb = Sandbox::new();
    sb.write_config(json!({"engines": [
        gateway_entry("gw-a", "https://a.invalid/x", "GW_A_KEY"),
        gateway_entry("gw-b", "https://b.invalid/x", "GW_B_KEY"),
    ]}));
    sb.write_teams(json!({
        "doi-a": {"description": "Dev trên gw-a", "roles": {"dev": "gw-a"}},
        "doi-b": {"description": "Dev trên gw-b", "roles": {"dev": "gw-b"}},
    }));
    let repo_a = app_repo(&sb, "proj-a");
    let repo_b = app_repo(&sb, "proj-b");
    write_map_of(
        &sb,
        vec![
            team_project_entry(&sb, &repo_a, "proj-a", "work", "doi-a"),
            team_project_entry(&sb, &repo_b, "proj-b", "work", "doi-b"),
        ],
    );
    let key_a = "test-planted-gw-key-aaaaaaaaaa";
    let key_b = "test-planted-gw-key-bbbbbbbbbb";
    // Both keys are in the serve/CLI environment on every call.
    for (proj, name, expect) in [("proj-a", "sua-a", key_a), ("proj-b", "sua-b", key_b)] {
        let env = [
            ("GW_A_KEY", key_a),
            ("GW_B_KEY", key_b),
            ("FAKE_CLAUDE_EXPECT_TOKEN", expect),
        ];
        let res = sb.run_env(
            &["run", "lead", "--name", name, "--", proj, "sửa app"],
            Some(lead_plan_entry()),
            &env,
        );
        assert_ok(&res, "run lead");
        let res = sb.run_env(&["run", "dev", "--", proj], Some(dev_entry_for(name)), &env);
        assert_ok(&res, "run dev");
    }

    assert_eq!(engines_of_runs(&sb), ["claude", "gw-a", "claude", "gw-b"]);
    let recs = sb.records();
    assert_eq!(recs.len(), 4);
    for (rec, base) in [
        (&recs[1], "https://a.invalid/x"),
        (&recs[3], "https://b.invalid/x"),
    ] {
        assert_eq!(rec["anthropic_base_url"], json!(base));
        assert_eq!(rec["auth_token_matches_expected"], json!(true));
        let names = rec_env_names(rec);
        for gone in ["GW_A_KEY", "GW_B_KEY"] {
            assert!(
                !names.iter().any(|n| n == gone),
                "{base}: env still has {gone}"
            );
        }
    }
}

#[test]
fn anthropic_missing_key_refused() {
    let sb = Sandbox::new();
    sb.write_config(json!({"engines": [
        gateway_entry("vcadm-deepseek", "https://gateway.invalid/proxyllm", "VCADM_GATEWAY_KEY"),
    ]}));
    sb.write_teams(
        json!({"doi-gw": {"description": "Dev qua gateway", "roles": {"dev": "vcadm-deepseek"}}}),
    );
    let repo = app_repo(&sb, "web-ban-hang");
    write_map_of(
        &sb,
        vec![team_project_entry(
            &sb,
            &repo,
            "web-ban-hang",
            "work",
            "doi-gw",
        )],
    );
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
    let runs_before = run_count(&sb);
    let records_before = sb.records().len();

    // An empty value counts as missing.
    for env in [vec![], vec![("VCADM_GATEWAY_KEY", "  ")]] {
        let res = sb.run_env(
            &["run", "dev", "--", "web-ban-hang"],
            Some(dev_entry_for("sua-app")),
            &env,
        );
        assert_eq!(res.code, 1, "stderr: {}", res.stderr);
        assert!(
            res.stderr.contains(
                "Chưa đặt biến môi trường VCADM_GATEWAY_KEY cho engine vcadm-deepseek — đặt biến này trong shell chạy agent-brain (serve hoặc CLI) rồi chạy lại."
            ),
            "stderr: {}",
            res.stderr
        );
        assert_eq!(run_count(&sb), runs_before, "no run row for a refused step");
        assert_eq!(sb.records().len(), records_before, "nothing was spawned");
    }
}

#[test]
fn legacy_object_shape_still_routes() {
    let sb = Sandbox::new();
    sb.write_config(json!({"engines": {"deepseek": {"command": fake_cmd()}}}));
    let repo = app_repo(&sb, "web-ban-hang");
    write_map_of(
        &sb,
        vec![team_project_entry(
            &sb,
            &repo,
            "web-ban-hang",
            "work",
            "tiet-kiem",
        )],
    );
    let env = [
        ("DEEPSEEK_API_KEY", PLANTED_GW_KEY),
        ("FAKE_CLAUDE_EXPECT_TOKEN", PLANTED_GW_KEY),
    ];

    let res = sb.run_env(
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
        &env,
    );
    assert_ok(&res, "run lead");
    let res = sb.run_env(
        &["run", "dev", "--", "web-ban-hang"],
        Some(dev_entry_for("sua-app")),
        &env,
    );
    assert_ok(&res, "run dev");

    assert_eq!(engines_of_runs(&sb), ["claude", "deepseek"]);
    let dev = &sb.records()[1];
    assert_eq!(
        dev["anthropic_base_url"],
        json!("https://api.deepseek.com/anthropic")
    );
    assert_eq!(dev["auth_token_matches_expected"], json!(true));
    assert!(!rec_env_names(dev).iter().any(|n| n == "DEEPSEEK_API_KEY"));
}

#[test]
fn unknown_engine_refused_before_run() {
    let sb = Sandbox::new();
    sb.write_teams(
        json!({"doi-x": {"description": "Dev trên engine không có", "roles": {"dev": "khong-co"}}}),
    );
    let repo = app_repo(&sb, "web-ban-hang");
    write_map_of(
        &sb,
        vec![team_project_entry(
            &sb,
            &repo,
            "web-ban-hang",
            "work",
            "doi-x",
        )],
    );
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
    let runs_before = run_count(&sb);
    let records_before = sb.records().len();

    let res = sb.run(
        &["run", "dev", "--", "web-ban-hang"],
        Some(dev_entry_for("sua-app")),
    );
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(
        res.stderr.contains(
            "Engine 'khong-co' của đội hình 'doi-x' không có trong config.json (engines) — thêm engine hoặc đổi đội hình rồi chạy lại."
        ),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(run_count(&sb), runs_before);
    assert_eq!(sb.records().len(), records_before);
}

// ---------------------------------------------------------------------------
// Plan 06-11 Task 2: the profile guard (D-36 / D-38 / D-42), the unverified mark (D-40), QA (D-39)
// ---------------------------------------------------------------------------

/// A native entry pinned to the personal profile, run by the fake claude.
fn claude_max_entry() -> Value {
    json!({
        "id": "claude-max",
        "kind": "anthropic",
        "command": fake_cmd(),
        "pin_profile": "personal",
        "models": {"lead": "opus", "dev": "opus"}
    })
}

#[test]
fn eng10_seam_backstop() {
    let sb = Sandbox::new();
    sb.write_config(json!({"engines": [claude_max_entry()]}));
    sb.write_teams(
        json!({"doi-max": {"description": "Dev trên Claude Max", "roles": {"dev": "claude-max"}}}),
    );
    let repo = app_repo(&sb, "web-ban-hang");
    write_map_of(
        &sb,
        vec![team_project_entry(
            &sb,
            &repo,
            "web-ban-hang",
            "work",
            "mac-dinh",
        )],
    );
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
    // The hand-edited case: the task row names a team whose Dev is the pinned engine.
    set_team_json(&sb, Some(r#"{"name":"doi-max","history":[]}"#));
    let runs_before = run_count(&sb);
    let records_before = sb.records().len();

    let res = sb.run(
        &["run", "dev", "--", "web-ban-hang"],
        Some(dev_entry_for("sua-app")),
    );
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(
        res.stderr.contains(
            "Engine 'claude-max' chỉ dùng cho dự án profile personal — dự án 'web-ban-hang' dùng profile work (ranh giới work/personal, SPK-10). Đổi engine của vai trò dev hoặc chọn đội hình khác."
        ),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(run_count(&sb), runs_before, "no run row for a refused step");
    assert_eq!(sb.records().len(), records_before, "nothing was spawned");
}

#[test]
fn pinned_engine_runs_on_personal_project() {
    let sb = Sandbox::new();
    sb.write_config(json!({"engines": [claude_max_entry()]}));
    sb.write_teams(
        json!({"doi-max": {"description": "Dev trên Claude Max", "roles": {"dev": "claude-max"}}}),
    );
    let repo = app_repo(&sb, "web-ban-hang");
    write_map_of(
        &sb,
        vec![team_project_entry(
            &sb,
            &repo,
            "web-ban-hang",
            "personal",
            "doi-max",
        )],
    );
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
    let res = sb.run(
        &["run", "dev", "--", "web-ban-hang"],
        Some(dev_entry_for("sua-app")),
    );
    assert_ok(&res, "run dev on the pinned engine");

    assert_eq!(engines_of_runs(&sb), ["claude", "claude-max"]);
    let dev = &sb.records()[1];
    // The project's own profile env: no account is borrowed, nothing is injected.
    assert_eq!(
        dev["claude_config_dir"],
        json!(sb.personal_dir.to_str().unwrap())
    );
    assert_eq!(dev["anthropic_base_url"], Value::Null);
    let argv = rec_argv(dev);
    let m = argv.iter().position(|a| a == "--model").unwrap();
    assert_eq!(argv[m + 1], "opus");
    assert!(res.stderr.contains("· engine claude-max · model opus"));
    // A native run is not an unverified endpoint.
    let (_, metrics) = run_of(&sb, "dev");
    assert!(metrics.get("unverified").is_none(), "metrics: {metrics}");
}

/// One Dev run on a gateway entry; returns the CLI result of `run dev`.
fn run_gateway_dev(sb: &Sandbox, entry: Value) -> RunOut {
    sb.write_config(json!({ "engines": [entry] }));
    sb.write_teams(
        json!({"doi-gw": {"description": "Dev qua gateway", "roles": {"dev": "vcadm-deepseek"}}}),
    );
    let repo = app_repo(sb, "web-ban-hang");
    write_map_of(
        sb,
        vec![team_project_entry(
            sb,
            &repo,
            "web-ban-hang",
            "work",
            "doi-gw",
        )],
    );
    let env = [("VCADM_GATEWAY_KEY", PLANTED_GW_KEY)];
    let res = sb.run_env(
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
        &env,
    );
    assert_ok(&res, "run lead");
    let res = sb.run_env(
        &["run", "dev", "--", "web-ban-hang"],
        Some(dev_entry_for("sua-app")),
        &env,
    );
    assert_ok(&res, "run dev");
    res
}

#[test]
fn unverified_endpoint_marked() {
    // No `verified` text: the run happens, and is marked.
    let sb = Sandbox::new();
    let mut entry = gateway_entry(
        "vcadm-deepseek",
        "https://gateway.invalid/proxyllm",
        "VCADM_GATEWAY_KEY",
    );
    entry.as_object_mut().unwrap().remove("verified");
    let res = run_gateway_dev(&sb, entry);
    let header = res
        .stderr
        .lines()
        .find(|l| l.starts_with("[agent-brain] dev "))
        .unwrap_or_else(|| panic!("no dev header: {}", res.stderr));
    assert!(
        header.ends_with("· engine vcadm-deepseek · model deepseek/deepseek-chat · chưa kiểm E-09"),
        "header: {header}"
    );
    let (engine, metrics) = run_of(&sb, "dev");
    assert_eq!(engine, "vcadm-deepseek");
    assert_eq!(metrics["unverified"], json!(true), "metrics: {metrics}");

    // With E-09 evidence: no mark.
    let sb = Sandbox::new();
    let mut entry = gateway_entry(
        "vcadm-deepseek",
        "https://gateway.invalid/proxyllm",
        "VCADM_GATEWAY_KEY",
    );
    entry["verified"] = json!("2026-10-28 E-09");
    let res = run_gateway_dev(&sb, entry);
    let header = res
        .stderr
        .lines()
        .find(|l| l.starts_with("[agent-brain] dev "))
        .unwrap_or_else(|| panic!("no dev header: {}", res.stderr));
    assert!(!header.contains("chưa kiểm"), "header: {header}");
    assert!(
        header.ends_with("· model deepseek/deepseek-chat"),
        "header: {header}"
    );
    let (_, metrics) = run_of(&sb, "dev");
    assert!(
        metrics
            .get("unverified")
            .map_or(true, |v| v == &json!(false)),
        "metrics: {metrics}"
    );
}

#[test]
fn qa_on_list_id_refused() {
    // D-39: QA stays the built-in claude, a pinned `claude-max` included.
    let sb = Sandbox::new();
    sb.write_config(json!({"engines": [claude_max_entry()]}));
    sb.write_teams(
        json!({"doi-qa": {"description": "QA trên Claude Max", "roles": {"qa": "claude-max"}}}),
    );
    setup_auto_project(&sb, "personal");
    let res = sb.run(
        &[
            "run",
            "lead",
            "--team",
            "doi-qa",
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
        res.stderr.contains("Không có đội hình 'doi-qa'"),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(task_count(&sb), 0);
    assert!(sb.records().is_empty(), "no process may be spawned");
}
