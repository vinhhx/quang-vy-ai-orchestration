//! Driver autopilot: nhánh task (CR-01), Ctrl+C êm (WR-01), mã thoát của con
//! (WR-02) và dòng lỗi `run lỗi` (IN-06). Mọi test dùng claude giả qua `Sandbox`.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{RunOut, Sandbox};
use protocol::lead_scenario;
use serde_json::{json, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers (chép từ tests/cli_autopilot.rs)
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

fn auto_state(sb: &Sandbox, task_id: i64) -> (Option<String>, Option<i64>, Option<String>) {
    sb.db()
        .query_row(
            "SELECT auto_state, auto_pid, auto_json FROM tasks WHERE id = ?1",
            [task_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
}

fn auto_json(sb: &Sandbox, task_id: i64) -> Value {
    let (_, _, aj) = auto_state(sb, task_id);
    serde_json::from_str(&aj.unwrap_or_else(|| "{}".to_string())).unwrap()
}

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

/// `auto --start` chạy plan ⇄ review (revise rồi approve) và dừng ở cổng.
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

fn dev_impl_entry() -> Value {
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
        {"op": "git", "args": ["add", "src/app.py"]},
        {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
    ]})
}

fn code_review_entry(verdict: &str, blocking: i64, files: &[&str]) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".code-review.md",
        "content": review_report(verdict, blocking, 1, files),
    }]})
}

fn qa_entry(verdict: &str) -> Value {
    let report = format!(
        "# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- src/app.py: đã xem.\n## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông.\n## Kết luận\n{verdict}\n"
    );
    json!({"ops": [{"op": "write", "path": ".qa-report.md", "content": report}]})
}

/// Sha của commit `sua-app: plan` mới nhất trên nhánh hiện tại.
fn plan_sha(sb: &Sandbox, repo: &std::path::Path) -> String {
    let log = sb.git(repo, &["log", "--format=%H %s"]);
    log.lines()
        .find_map(|l| {
            let (sha, subj) = l.split_once(' ')?;
            (subj == "sua-app: plan").then(|| sha.to_string())
        })
        .expect("plan commit")
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

/// Mọi bước của mac-dinh sau cổng: dev, review code đạt, QA đạt.
fn build_ok_roles() -> Value {
    json!({
        "dev": [dev_impl_entry()],
        "code-review": [code_review_entry("pass", 0, &[])],
        "qa": [qa_entry("Đạt")],
    })
}

/// Tạo task 2 `viec-b` (xếp chồng trên task/sua-app) và để task/viec-b được checkout.
fn second_task_checked_out(sb: &Sandbox, repo: &std::path::Path) {
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
    assert_eq!(
        sb.git(repo, &["branch", "--show-current"]).trim(),
        "task/viec-b"
    );
}

// ---------------------------------------------------------------------------
// Task 1: nhánh task (CR-01), approve khi autopilot đã tắt, IN-03
// ---------------------------------------------------------------------------

const FOREIGN_BRANCH: &str =
    "Không ở nhánh task 'task/sua-app' (nhánh hiện tại: 'task/viec-b') — không đoán";

#[test]
fn driver_stops_on_foreign_branch() {
    let sb = Sandbox::new();
    let repo = flow_to_gate(&sb);
    let res = sb.run(&["approve", "1", "--no-continue"], None);
    assert_ok(&res, "approve --no-continue");
    second_task_checked_out(&sb, &repo);
    let bypass_before = auto_json(&sb, 1)["bypass_stages"].clone();

    let res = sb.run(&["auto", "1"], Some(by_role(build_ok_roles())));
    assert_eq!(res.code, 4, "stderr: {}", res.stderr);
    assert!(
        res.stderr.contains(FOREIGN_BRANCH),
        "stderr: {}",
        res.stderr
    );
    let aj = auto_json(&sb, 1);
    assert_eq!(aj["stop"]["reason"], "run_loi", "aj: {aj}");
    assert!(runs(&sb).iter().all(|r| r.0 != "dev"), "no dev run");
    assert_eq!(approved_commit(&sb, 2), None, "task 2 not approved");
    assert_eq!(aj["bypass_stages"], bypass_before);
}

#[test]
fn approve_continue_needs_branch() {
    let sb = Sandbox::new();
    let repo = flow_to_gate(&sb);
    second_task_checked_out(&sb, &repo);
    let before = runs(&sb).len();

    let res = sb.run(&["approve", "1"], Some(by_role(build_ok_roles())));
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(
        res.stderr.contains(FOREIGN_BRANCH),
        "stderr: {}",
        res.stderr
    );
    assert!(
        res.stderr.contains("agent-brain approve 1 --no-continue"),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(approved_commit(&sb, 1), None);
    assert_eq!(runs(&sb).len(), before);

    let res = sb.run(&["approve", "1", "--no-continue"], None);
    assert_ok(&res, "approve 1 --no-continue on another branch");
    assert_eq!(approved_commit(&sb, 1), Some(plan_sha(&sb, &repo)));
    assert_eq!(runs(&sb).len(), before);
}

#[test]
fn approve_after_autopilot_ended_records_only() {
    let sb = Sandbox::new();
    let repo = flow_to_gate(&sb);
    sb.db()
        .execute("UPDATE tasks SET auto_state = 'ended' WHERE id = 1", [])
        .unwrap();

    let res = sb.run(&["approve", "1"], Some(by_role(build_ok_roles())));
    assert_ok(&res, "approve 1 (ended)");
    assert_eq!(approved_commit(&sb, 1), Some(plan_sha(&sb, &repo)));
    assert!(runs(&sb).iter().all(|r| r.0 != "dev"), "no dev run");
    assert!(
        res.stderr
            .contains("Autopilot đã tắt cho task 1 — chỉ ghi nhận duyệt"),
        "stderr: {}",
        res.stderr
    );
    let (state, _, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("ended"));
}

#[test]
fn approve_prints_once() {
    let sb = Sandbox::new();
    flow_to_gate(&sb);
    let res = sb.run(&["approve", "1", "--no-continue"], None);
    assert_ok(&res, "approve --no-continue");
    assert_eq!(
        res.stderr.matches("Đã duyệt plan").count(),
        1,
        "stderr: {}",
        res.stderr
    );
}
