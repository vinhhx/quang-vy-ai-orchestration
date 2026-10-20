mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{RunOut, Sandbox};
use protocol::{lead_scenario, setup_project};
use serde_json::{json, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers (copied from cli_autopilot.rs)
// ---------------------------------------------------------------------------

const AUTO_ENV: &str = "AGENT_BRAIN_AUTOPILOT_TASK";

fn review_report(verdict: &str, blocking: i64, advisory: i64) -> String {
    let mut body = format!(
        "---\nverdict: {verdict}\nblocking: {blocking}\nadvisory: {advisory}\nfiles: []\n---\n# Review: việc b\n## Mục chặn (blocking)\n"
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

fn code_review_entry() -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".code-review.md",
        "content": review_report("pass", 0, 1),
    }]})
}

fn qa_entry() -> Value {
    let report = "# QA: việc b\n## Thay đổi có khớp plan không (từng mục)\n- src/app.py: đã xem.\n## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông.\n## Kết luận\nĐạt\n";
    json!({"ops": [{"op": "write", "path": ".qa-report.md", "content": report}]})
}

fn dev_impl_entry() -> Value {
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
        {"op": "git", "args": ["add", "src/app.py"]},
        {"op": "git", "args": ["commit", "-m", "viec-b: sửa app"]}
    ]})
}

fn lead_plan_entry() -> Value {
    lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])
}

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

fn run_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap()
}

fn approved_commit(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT approved_commit FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
}

/// Hai task: sua-app (task 1) rồi viec-b (task 2, tạo từ task/sua-app, đang checkout).
fn two_tasks(sb: &Sandbox) -> PathBuf {
    let repo = setup_project(sb, "work");
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
    assert_ok(&res, "run lead sua-app");
    let res = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "viec-b",
            "--",
            "web-ban-hang",
            "việc b",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead viec-b");
    let cur = sb.git(&repo, &["branch", "--show-current"]);
    assert_eq!(cur.trim(), "task/viec-b");
    let ids: Vec<(i64, String)> = {
        let conn = sb.db();
        let mut stmt = conn
            .prepare("SELECT id, branch FROM tasks ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    assert_eq!(
        ids,
        vec![
            (1, "task/sua-app".to_string()),
            (2, "task/viec-b".to_string())
        ]
    );
    repo
}

// ---------------------------------------------------------------------------
// Task 1: autopilot_task_guard (CR-01)
// ---------------------------------------------------------------------------

#[test]
fn role_refuses_foreign_branch_under_autopilot() {
    let sb = Sandbox::new();
    let repo = two_tasks(&sb);
    let before = run_count(&sb);
    assert_eq!(approved_commit(&sb, 2), None);

    let cases: Vec<(&str, Value)> = vec![
        ("dev", dev_impl_entry()),
        ("plan-review", plan_review_entry("approve", 0, 1)),
        ("code-review", code_review_entry()),
        ("qa", qa_entry()),
    ];
    for (role, scenario) in cases {
        let res = sb.run_env(
            &["run", role, "--", "web-ban-hang"],
            Some(scenario),
            &[(AUTO_ENV, "1")],
        );
        assert_eq!(res.code, 1, "{role}: stderr: {}", res.stderr);
        assert!(
            res.stderr
                .contains("Nhánh hiện tại 'task/viec-b' không phải nhánh của task autopilot 1"),
            "{role}: stderr: {}",
            res.stderr
        );
        assert_eq!(run_count(&sb), before, "{role} created a run row");
        assert_eq!(approved_commit(&sb, 2), None, "{role} approved task 2");
        let status = sb.git(&repo, &["status", "--porcelain"]);
        assert!(
            status.trim().is_empty(),
            "{role} left files behind: {status}"
        );
    }
    assert!(sb.records().len() == 2, "only the two leads ran claude");
}

#[test]
fn role_runs_own_branch_under_autopilot() {
    let sb = Sandbox::new();
    let _repo = two_tasks(&sb);
    let before = run_count(&sb);

    let res = sb.run_env(
        &["run", "plan-review", "--", "web-ban-hang"],
        Some(plan_review_entry("approve", 0, 1)),
        &[(AUTO_ENV, "2")],
    );
    assert_ok(&res, "plan-review as task 2's own child");
    assert_eq!(run_count(&sb), before + 1);
    let (role, task_id): (String, Option<i64>) = sb
        .db()
        .query_row(
            "SELECT role, task_id FROM runs ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(role, "plan-review");
    assert_eq!(task_id, Some(2));
}

#[test]
fn role_guard_ignores_unset_env() {
    let sb = Sandbox::new();
    let _repo = two_tasks(&sb);
    let before = run_count(&sb);

    let res = sb.run(
        &["run", "plan-review", "--", "web-ban-hang"],
        Some(plan_review_entry("approve", 0, 1)),
    );
    assert_ok(&res, "plan-review without the autopilot env");
    assert_eq!(run_count(&sb), before + 1);
}
