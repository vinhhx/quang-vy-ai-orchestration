//! W-1 (05.2-SECURITY): the guard hook also checks Bash git calls.

mod common;
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::{lead_scenario, setup_project};
use std::path::PathBuf;

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

/// The `run_dir` of the latest run of `role`.
fn latest_run_dir(sb: &Sandbox, role: &str) -> PathBuf {
    let run_dir: String = sb
        .db()
        .query_row(
            "SELECT run_dir FROM runs WHERE role = ?1 ORDER BY id DESC LIMIT 1",
            rusqlite::params![role],
            |r| r.get(0),
        )
        .unwrap();
    PathBuf::from(run_dir)
}

fn guard_log_lines(run_dir: &std::path::Path) -> Vec<serde_json::Value> {
    let log_path = run_dir.join("guard.jsonl");
    let content = std::fs::read_to_string(&log_path)
        .unwrap_or_else(|e| panic!("read guard log {}: {e}", log_path.display()));
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("parse guard log line"))
        .collect()
}

fn run_plan_review(sb: &Sandbox, git_args: &[&str]) -> common::RunOut {
    let sc_review = serde_json::json!({
        "ops": [
            {"op": "git", "args": git_args},
            {"op": "write", "path": ".plan-review.md", "content": APPROVE_REPORT}
        ]
    });
    sb.run(
        &["run", "plan-review", "--", "web-ban-hang"],
        Some(sc_review),
    )
}

fn latest_outcome(sb: &Sandbox) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'plan-review' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn reviewer_git_output_blocked() {
    let sb = Sandbox::new();
    let (_repo, _task, _plan_sha) = setup_lead(&sb);

    let res = run_plan_review(&sb, &["diff", "--output=../leak-review.txt"]);
    assert_eq!(
        res.code, 0,
        "plan-review failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert_eq!(latest_outcome(&sb).as_deref(), Some("review:approve"));

    let rec = sb.records().pop().expect("fake record");
    let blocked = rec["blocked"].as_array().expect("blocked array");
    assert!(
        blocked.iter().any(|b| b["tool"] == "Bash"),
        "no Bash entry in blocked: {blocked:?}"
    );
    assert!(
        !sb.repos.join("leak-review.txt").exists(),
        "git diff --output wrote outside the repo"
    );

    let run_dir = latest_run_dir(&sb, "plan-review");
    let log_path = run_dir.join("guard.jsonl");
    let policy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("policy.json")).unwrap())
            .unwrap();
    assert_eq!(
        policy["log"].as_str(),
        Some(log_path.to_string_lossy().as_ref()),
        "guard log path differs from policy.json"
    );
    let lines = guard_log_lines(&run_dir);
    assert!(
        lines.iter().any(|l| l["decision"] == "block"
            && l["tool"] == "Bash"
            && l["path"]
                .as_str()
                .is_some_and(|p| p.contains("--output=../leak-review.txt"))),
        "no Bash block line in guard log: {lines:?}"
    );
}

#[test]
fn reviewer_git_status_allowed() {
    let sb = Sandbox::new();
    let (_repo, _task, _plan_sha) = setup_lead(&sb);

    let res = run_plan_review(&sb, &["status", "--porcelain"]);
    assert_eq!(
        res.code, 0,
        "plan-review failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert_eq!(latest_outcome(&sb).as_deref(), Some("review:approve"));

    let rec = sb.records().pop().expect("fake record");
    let blocked = rec["blocked"].as_array().expect("blocked array");
    assert!(
        !blocked.iter().any(|b| b["tool"] == "Bash"),
        "git status was blocked: {blocked:?}"
    );
}
