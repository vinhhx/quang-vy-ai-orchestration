mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::autopilot::decide::{FixFor, Stage, StageRun};
use agent_brain::autopilot::stage_args;
use common::{RunOut, Sandbox};
use protocol::lead_scenario;
use serde_json::{json, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn review_report(verdict: &str, blocking: i64, advisory: i64, files: &[&str]) -> String {
    let files_json = serde_json::to_string(files).unwrap();
    let mut body = format!(
        "---\nverdict: {verdict}\nblocking: {blocking}\nadvisory: {advisory}\nfiles: {files_json}\n---\n# Review: sửa app\n## Mục chặn (blocking)\n"
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

fn by_role(roles: Value) -> Value {
    json!({ "by_role": roles })
}

fn plan_review_entry(verdict: &str, blocking: i64, advisory: i64) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan-review.md",
        "content": review_report(verdict, blocking, advisory, &[]),
    }]})
}

fn lead_append_entry() -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan.md",
        "content": "\n<!-- v2 -->\n",
        "append": true,
    }]})
}

fn lead_plan_entry() -> Value {
    lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])
}

/// Repo `web-ban-hang` với entry không có khóa `workflow` (mặc định server: mac-dinh).
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

fn auto_start(sb: &Sandbox, workflow: &str, scenario: Value) -> RunOut {
    sb.run(
        &[
            "auto",
            "--start",
            "--name",
            "sua-app",
            "--workflow",
            workflow,
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(scenario),
    )
}

type RunTuple = (String, Option<String>, Option<i64>, Option<String>);

fn runs(sb: &Sandbox) -> Vec<RunTuple> {
    let conn = sb.db();
    let mut stmt = conn
        .prepare("SELECT role, stage, round, outcome FROM runs ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn rt(role: &str, stage: &str, round: i64, outcome: Option<&str>) -> RunTuple {
    (
        role.to_string(),
        Some(stage.to_string()),
        Some(round),
        outcome.map(|s| s.to_string()),
    )
}

fn auto_state(sb: &Sandbox, task_id: i64) -> (Option<String>, Option<i64>, Option<String>) {
    sb.db()
        .query_row(
            "SELECT auto_state, auto_pid, auto_json FROM tasks WHERE id = ?1",
            [task_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
}

#[allow(dead_code)]
fn auto_json(sb: &Sandbox, task_id: i64) -> Value {
    let (_, _, aj) = auto_state(sb, task_id);
    serde_json::from_str(&aj.unwrap_or_else(|| "{}".to_string())).unwrap()
}

fn prompt_of(record: &Value) -> String {
    let argv: Vec<String> = record["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();
    let i = argv.iter().position(|a| a == "-p").unwrap();
    argv[i + 1].clone()
}

fn records_for_role(sb: &Sandbox, first_line_prefix: &str) -> Vec<Value> {
    sb.records()
        .into_iter()
        .filter(|r| {
            let argv = r["argv"].as_array().unwrap();
            let i = argv
                .iter()
                .position(|a| a.as_str() == Some("--append-system-prompt"));
            match i {
                Some(i) => argv[i + 1]
                    .as_str()
                    .unwrap_or("")
                    .starts_with(first_line_prefix),
                None => false,
            }
        })
        .collect()
}

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

/// Luồng Task 1: `auto --start` chạy plan ⇄ review (revise rồi approve) và dừng ở cổng.
fn flow_to_gate(sb: &Sandbox) -> PathBuf {
    let repo = setup_auto_project(sb, "work");
    let sc = by_role(json!({
        "lead": [lead_plan_entry(), lead_append_entry()],
        "plan-review": [plan_review_entry("revise", 1, 0), plan_review_entry("approve", 0, 1)],
    }));
    let res = auto_start(sb, "mac-dinh", sc);
    assert_ok(&res, "auto --start");
    assert!(
        res.stderr
            .contains("Autopilot dừng ở bước duyệt plan (task 1) — duyệt: agent-brain approve 1"),
        "missing gate line: {}",
        res.stderr
    );
    repo
}

// ---------------------------------------------------------------------------
// Task 1
// ---------------------------------------------------------------------------

#[test]
fn stage_args_shapes() {
    fn sr(stage: Stage, round: i64) -> StageRun {
        StageRun {
            stage,
            round,
            fix_for: None,
            review_input: false,
            note: None,
            fresh_plan: false,
        }
    }
    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }
    let p = "web-ban-hang";
    let t = "sửa app";
    assert_eq!(
        stage_args(&sr(Stage::Plan, 1), "sua-app", p, t),
        v(&["run", "lead", "--name", "sua-app", "--round", "1", "--", p, t])
    );
    let mut plan2 = sr(Stage::Plan, 2);
    plan2.review_input = true;
    plan2.note = Some("-x".to_string());
    let args = stage_args(&plan2, "sua-app", p, t);
    assert_eq!(
        args,
        v(&[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--round",
            "2",
            "--review",
            "--note=-x",
            "--",
            p,
            t
        ])
    );
    assert_eq!(args.iter().filter(|a| a.contains("-x")).count(), 1);
    assert_eq!(
        stage_args(&sr(Stage::PlanReview, 2), "sua-app", p, t),
        v(&["run", "plan-review", "--round", "2", "--", p])
    );
    assert_eq!(
        stage_args(&sr(Stage::Implement, 1), "sua-app", p, t),
        v(&["run", "dev", "--round", "1", "--", p])
    );
    assert_eq!(
        stage_args(&sr(Stage::CodeReview, 3), "sua-app", p, t),
        v(&["run", "code-review", "--round", "3", "--", p])
    );
    assert_eq!(
        stage_args(&sr(Stage::Qa, 2), "sua-app", p, t),
        v(&["run", "qa", "--round", "2", "--", p])
    );
    let mut fix_cr = sr(Stage::Fix, 2);
    fix_cr.fix_for = Some(FixFor::CodeReview);
    assert_eq!(
        stage_args(&fix_cr, "sua-app", p, t),
        v(&[
            "run",
            "dev",
            "--fix",
            "code-review",
            "--round",
            "2",
            "--",
            p
        ])
    );
    let mut fix_qa = sr(Stage::Fix, 1);
    fix_qa.fix_for = Some(FixFor::Qa);
    assert_eq!(
        stage_args(&fix_qa, "sua-app", p, t),
        v(&["run", "dev", "--fix", "qa", "--round", "1", "--", p])
    );
}

#[test]
fn auto_start_plan_loop_to_gate() {
    let sb = Sandbox::new();
    flow_to_gate(&sb);

    assert_eq!(
        runs(&sb),
        vec![
            rt("lead", "plan", 1, None),
            rt("plan-review", "plan-review", 1, Some("review:revise")),
            rt("lead", "plan", 2, None),
            rt("plan-review", "plan-review", 2, Some("review:approve")),
        ]
    );

    let leads = records_for_role(&sb, "Bạn là Lead");
    assert_eq!(leads.len(), 2);
    assert!(
        prompt_of(&leads[1]).contains(".plan-review.md"),
        "lead round 2 prompt: {}",
        prompt_of(&leads[1])
    );

    let (state, pid, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("gate"));
    assert_eq!(pid, None);
}

#[test]
fn auto_task_at_gate_is_idempotent() {
    let sb = Sandbox::new();
    flow_to_gate(&sb);
    let before = runs(&sb).len();

    let res = sb.run(&["auto", "1"], None);
    assert_ok(&res, "auto 1");
    assert_eq!(runs(&sb).len(), before);
    let (state, pid, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("gate"));
    assert_eq!(pid, None);
    assert!(res
        .stderr
        .contains("Autopilot dừng ở bước duyệt plan (task 1)"));
}
