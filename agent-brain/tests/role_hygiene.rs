//! Role hygiene (05.2-27):
//! - WR-05: a failed or killed review / QA run leaves no report behind.

mod common;
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::{lead_scenario, setup_project};
use std::path::PathBuf;

/// Stops the decoy child through its own handle (never by name).
struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn setup_lead(sb: &Sandbox) -> (PathBuf, String, String) {
    let repo = setup_project(sb, "work");
    let sc_lead = lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")]);
    let res_lead = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
        ],
        Some(sc_lead),
    );
    assert_eq!(
        res_lead.code, 0,
        "Lead failed: stderr: {}\nstdout: {}",
        res_lead.stderr, res_lead.stdout
    );
    let task = "sua-app".to_string();
    let plan_sha = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    (repo, task, plan_sha)
}

fn setup_lead_and_dev(sb: &Sandbox) -> (PathBuf, String, String, String) {
    let (repo, task, plan_sha) = setup_lead(sb);
    let sc_dev = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", format!("{task}: sửa app")]}
        ]
    });
    let res_dev = sb.run(&["run", "dev", "web-ban-hang"], Some(sc_dev));
    assert_eq!(
        res_dev.code, 0,
        "Dev failed: stderr: {}\nstdout: {}",
        res_dev.stderr, res_dev.stdout
    );
    let dev_sha = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    (repo, task, plan_sha, dev_sha)
}

const APPROVE_REPORT: &str = "\
---
verdict: approve
blocking: 0
advisory: 0
files: []
---
# Review plan: x
## Mục chặn (blocking)
## Góp ý (advisory)
";

fn lead_append_entry() -> serde_json::Value {
    serde_json::json!({"ops": [{
        "op": "write",
        "path": ".plan.md",
        "content": "\n<!-- v2 -->\n",
        "append": true,
    }]})
}

fn porcelain(sb: &Sandbox, repo: &std::path::Path) -> String {
    sb.git(repo, &["status", "--porcelain"])
}

#[test]
fn failed_plan_review_leaves_clean_tree() {
    let sb = Sandbox::new();
    let (repo, _task, _plan_sha) = setup_lead(&sb);

    let sc = serde_json::json!({"exit_code": 1, "ops": []});
    let res = sb.run(&["run", "plan-review", "--", "web-ban-hang"], Some(sc));
    assert_ne!(
        res.code, 0,
        "plan-review should fail: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        !repo.join(".plan-review.md").exists(),
        "the failed review left .plan-review.md behind"
    );
    let status = porcelain(&sb, &repo);
    assert!(status.trim().is_empty(), "tree not clean: {status}");
    assert!(
        res.stdout.contains("Đã dọn .plan-review.md")
            || res.stderr.contains("Đã dọn .plan-review.md"),
        "no cleanup line: stderr: {}\nstdout: {}",
        res.stderr,
        res.stdout
    );

    // Lead can re-plan right after (not blocked by a dirty tree).
    let res_lead = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--note=thêm",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_append_entry()),
    );
    assert_eq!(
        res_lead.code, 0,
        "relead failed: stderr: {}\nstdout: {}",
        res_lead.stderr, res_lead.stdout
    );
}

#[test]
fn failed_review_restores_tracked_report() {
    let sb = Sandbox::new();
    let (repo, task, _plan_sha) = setup_lead(&sb);

    let sc_ok = serde_json::json!({"ops": [
        {"op": "write", "path": ".plan-review.md", "content": APPROVE_REPORT}
    ]});
    let res = sb.run(&["run", "plan-review", "--", "web-ban-hang"], Some(sc_ok));
    assert_eq!(
        res.code, 0,
        "approve review failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: plan-review"));

    // The failing run writes a partial report, then exits 1.
    let sc_fail = serde_json::json!({"exit_code": 1, "ops": [
        {"op": "write", "path": ".plan-review.md", "content": "# nửa chừng\n"}
    ]});
    let res = sb.run(&["run", "plan-review", "--", "web-ban-hang"], Some(sc_fail));
    assert_ne!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let on_disk = std::fs::read_to_string(repo.join(".plan-review.md")).expect("report");
    let at_head = sb.git(&repo, &["show", "HEAD:.plan-review.md"]);
    assert_eq!(on_disk, at_head, "the tracked report was not restored");
    let status = porcelain(&sb, &repo);
    assert!(status.trim().is_empty(), "tree not clean: {status}");
}

#[test]
fn e06_refusal_leaves_no_skeleton() {
    let sb = Sandbox::new();
    let repo1 = sb.make_repo(
        "proj-1",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let repo2 = sb.make_repo(
        "proj-2",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry1 = sb.project_entry(&repo1, "proj-1", "personal");
    entry1["name_vi"] = serde_json::json!("Dự án 1");
    let mut entry2 = sb.project_entry(&repo2, "proj-2", "personal");
    entry2["name_vi"] = serde_json::json!("Dự án 2");
    sb.write_map(serde_json::json!([entry1, entry2]), &[]);

    // Project 2 gets a committed plan first.
    let sc_lead = lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")]);
    let res_lead = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "proj-2",
            "sửa app",
        ],
        Some(sc_lead),
    );
    assert_eq!(
        res_lead.code, 0,
        "Lead failed: stderr: {}\nstdout: {}",
        res_lead.stderr, res_lead.stdout
    );

    // A live personal run on project 1 (a recorded decoy child).
    let decoy_child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn decoy sleep");
    let decoy = ChildGuard(decoy_child);
    let decoy_pid = decoy.0.id() as i32;
    let decoy_start = agent_brain::procs::read_stat(decoy_pid)
        .expect("read decoy stat")
        .start;

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = sb.read_map();
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");
    let proj_row_id = agent_brain::db::project_row_id(&conn, "proj-1").expect("proj1 id");
    agent_brain::db::insert_run(
        &conn,
        &agent_brain::db::NewRun {
            task_id: None,
            project_id: proj_row_id,
            role: "plan-review".to_string(),
            profile: "personal".to_string(),
            model: Some("opus".to_string()),
            pid: decoy_pid as i64,
            pgid: decoy_pid as i64,
            pid_start: decoy_start as i64,
            base_commit: None,
            run_dir: None,
        },
    )
    .expect("insert run");

    let out = sb.run(&["run", "plan-review", "--", "proj-2"], None);
    assert_eq!(
        out.code, 1,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stderr.contains("(E-06)"),
        "stderr missing E-06: {}",
        out.stderr
    );
    assert!(
        !repo2.join(".plan-review.md").exists(),
        "the E-06 refusal left .plan-review.md behind"
    );
    let status = porcelain(&sb, &repo2);
    assert!(status.trim().is_empty(), "tree not clean: {status}");

    drop(decoy);
}

#[test]
fn failed_qa_leaves_clean_tree() {
    let sb = Sandbox::new();
    let (repo, _task, _plan_sha, dev_sha) = setup_lead_and_dev(&sb);

    let sc = serde_json::json!({"exit_code": 1, "ops": [
        {"op": "write", "path": ".qa-report.md", "content": "# QA nửa chừng\n"}
    ]});
    let res = sb.run(&["run", "qa", "--", "web-ban-hang"], Some(sc));
    assert_ne!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        !repo.join(".qa-report.md").exists(),
        "the failed QA run left .qa-report.md behind"
    );
    let status = porcelain(&sb, &repo);
    assert!(status.trim().is_empty(), "tree not clean: {status}");
    assert!(
        res.stderr.contains("Đã dọn .qa-report.md"),
        "no cleanup line: {}",
        res.stderr
    );
    let head = sb.git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(head.trim(), dev_sha);
}

#[test]
fn successful_and_violation_reviews_keep_report() {
    let sb = Sandbox::new();
    let (repo, task, _plan_sha) = setup_lead(&sb);

    // A malformed report ends protocol_violation and stays in the tree.
    let sc_bad = serde_json::json!({"ops": [
        {"op": "write", "path": ".plan-review.md", "content": "# không có front matter\n"}
    ]});
    let res = sb.run(&["run", "plan-review", "--", "web-ban-hang"], Some(sc_bad));
    assert_eq!(
        res.code,
        agent_brain::error::EXIT_PROTOCOL,
        "stderr: {}\nstdout: {}",
        res.stderr,
        res.stdout
    );
    assert_eq!(
        std::fs::read_to_string(repo.join(".plan-review.md")).unwrap(),
        "# không có front matter\n"
    );
    assert!(!res.stderr.contains("Đã dọn"), "stderr: {}", res.stderr);
    let outcome: Option<String> = sb
        .db()
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'plan-review' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome.as_deref(), Some("protocol_violation"));

    // An approve review still commits its report.
    let sc_ok = serde_json::json!({"ops": [
        {"op": "write", "path": ".plan-review.md", "content": APPROVE_REPORT}
    ]});
    let res = sb.run(&["run", "plan-review", "--", "web-ban-hang"], Some(sc_ok));
    assert_eq!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: plan-review"));
    assert_eq!(
        std::fs::read_to_string(repo.join(".plan-review.md")).unwrap(),
        APPROVE_REPORT
    );
    let status = porcelain(&sb, &repo);
    assert!(status.trim().is_empty(), "tree not clean: {status}");
}
