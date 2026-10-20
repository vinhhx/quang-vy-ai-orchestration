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

// ---------------------------------------------------------------------------
// Task 2: plan_commit_for (IN-09)
// ---------------------------------------------------------------------------

/// Nhánh task/sua-app làm tay: `.plan.md` (meta task sua-app, base_commit = HEAD lúc tạo nhánh)
/// và một thay đổi code. `plan_subject` là subject của commit chứa cả hai.
fn manual_branch(sb: &Sandbox, repo: &std::path::Path, plan_subject: &str) -> String {
    sb.git(repo, &["checkout", "-b", "task/sua-app"]);
    let base = sb.git(repo, &["rev-parse", "HEAD"]).trim().to_string();
    let meta = json!({
        "task": "sua-app",
        "branch": "task/sua-app",
        "base_branch": "main",
        "base_commit": base,
        "request": "sửa app",
        "created_at": "2026-10-21T00:00:00+00:00",
    });
    let text = protocol::plan_text(
        "sửa app",
        &[("src/app.py", "app()", "trả về 2", "demo")],
        "python3 -m unittest",
        "",
    );
    let text = agent_brain::planfile::set_meta(&text, &meta);
    std::fs::write(repo.join(".plan.md"), text).unwrap();
    std::fs::write(repo.join("src/app.py"), "def app():\n    return 2\n").unwrap();
    sb.git(repo, &["add", ".plan.md", "src/app.py"]);
    sb.git(repo, &["commit", "-m", plan_subject]);
    base
}

fn insert_task_row(sb: &Sandbox, base: &str) {
    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = sb.read_map();
    let projs = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projs).expect("sync_projects");
    let p = agent_brain::db::project_row_id(&conn, "web-ban-hang").expect("project_row_id");
    agent_brain::db::upsert_task(
        &conn,
        p,
        "sua-app",
        "sửa app",
        "task/sua-app",
        Some("main"),
        Some(base),
    )
    .expect("upsert_task");
}

#[test]
fn reused_slug_old_plan_commit_ignored() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    // Task cũ cùng slug đã merge vào main.
    sb.git(&repo, &["commit", "--allow-empty", "-m", "sua-app: plan"]);
    let base = manual_branch(&sb, &repo, "sua-app: nháp");
    insert_task_row(&sb, &base);

    for (role, scenario) in [("code-review", code_review_entry()), ("qa", qa_entry())] {
        let res = sb.run(&["run", role, "--", "web-ban-hang"], Some(scenario));
        assert_eq!(res.code, 1, "{role}: stderr: {}", res.stderr);
        assert!(
            res.stderr
                .contains("Không tìm thấy commit 'sua-app: plan' trên nhánh hiện tại"),
            "{role}: stderr: {}",
            res.stderr
        );
        assert_eq!(run_count(&sb), 0, "{role}: no run row");
    }
}

#[test]
fn plan_commit_for_legacy_fallback() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    // Nhánh cũ chưa có hàng task trong DB: tìm commit plan theo subject như trước.
    manual_branch(&sb, &repo, "sua-app: plan");
    let plan = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    std::fs::write(repo.join("src/app.py"), "def app():\n    return 3\n").unwrap();
    sb.git(&repo, &["add", "src/app.py"]);
    sb.git(&repo, &["commit", "-m", "sua-app: sửa app"]);

    let res = sb.run(&["run", "qa", "--", "web-ban-hang"], Some(qa_entry()));
    assert_ok(&res, "qa on a legacy branch");
    let base: Option<String> = sb
        .db()
        .query_row(
            "SELECT base_commit FROM runs WHERE role = 'qa' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(base.as_deref(), Some(plan.as_str()));
}

// ---------------------------------------------------------------------------
// Task 3: live-driver check on every profile (IN-10)
// ---------------------------------------------------------------------------

#[test]
fn work_project_driver_blocks_cli_run() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
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

    // Task 1 giả làm driver còn sống: pid của chính tiến trình test.
    let me = std::process::id() as i32;
    let start = agent_brain::procs::proc_id(me).unwrap().start as i64;
    sb.db()
        .execute(
            "UPDATE tasks SET auto_state = 'running', auto_pid = ?1, auto_pid_start = ?2 WHERE id = 1",
            rusqlite::params![me as i64, start],
        )
        .unwrap();
    let before = run_count(&sb);

    let args = [
        "run",
        "lead",
        "--name",
        "viec-khac",
        "--",
        "web-ban-hang",
        "việc khác",
    ];
    let res = sb.run(&args, Some(lead_plan_entry()));
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(
        res.stderr
            .contains("Dự án web-ban-hang đang chạy autopilot cho task 1"),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(run_count(&sb), before, "blocked run starts no claude");

    let res = sb.run_env(&args, Some(lead_plan_entry()), &[(AUTO_ENV, "1")]);
    assert_ok(&res, "run lead as the driver's own child");
    assert_eq!(run_count(&sb), before + 1);

    sb.db()
        .execute(
            "UPDATE tasks SET auto_state = NULL, auto_pid = NULL, auto_pid_start = NULL WHERE id = 1",
            [],
        )
        .unwrap();
}
