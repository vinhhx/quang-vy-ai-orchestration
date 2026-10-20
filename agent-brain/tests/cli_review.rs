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

#[test]
fn plan_review_revise_recorded() {
    let sb = Sandbox::new();
    let (repo, task, _plan_sha) = setup_lead(&sb);

    let review_report = "\
---
verdict: revise
blocking: 1
advisory: 4
files: []
---
# Review plan: x
## Mục chặn (blocking)
- thiếu test
## Góp ý (advisory)
";
    let sc_review = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": review_report,
            }
        ]
    });

    let res = sb.run(
        &["run", "plan-review", "web-ban-hang", "--round", "1"],
        Some(sc_review),
    );
    assert_eq!(
        res.code, 0,
        "plan-review failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: plan-review"));

    let conn = sb.db();
    let (role, stage, round, status, outcome, metrics_json): (
        String,
        Option<String>,
        Option<i64>,
        String,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT role, stage, round, status, outcome, metrics_json FROM runs ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .unwrap();

    assert_eq!(role, "plan-review");
    assert_eq!(stage.as_deref(), Some("plan-review"));
    assert_eq!(round, Some(1));
    assert_eq!(status, "succeeded");
    assert_eq!(outcome.as_deref(), Some("review:revise"));

    let metrics: serde_json::Value =
        serde_json::from_str(&metrics_json.expect("metrics_json should not be None")).unwrap();
    assert_eq!(metrics["verdict"], "revise");
    assert_eq!(metrics["blocking"], 1);
    assert_eq!(metrics["advisory"], 4);

    let task_status: String = conn
        .query_row(
            "SELECT status FROM tasks WHERE slug = ?1",
            rusqlite::params![task],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(task_status, "planned");

    let git_status = sb.git(&repo, &["status", "--porcelain"]);
    assert!(
        git_status.trim().is_empty(),
        "git status --porcelain was not empty: {git_status}"
    );
}

#[test]
fn plan_review_approve() {
    let sb = Sandbox::new();
    let (repo, task, _plan_sha) = setup_lead(&sb);

    let review_report = "\
---
verdict: approve
blocking: 0
advisory: 2
files: []
---
# Review plan: x
## Mục chặn (blocking)
## Góp ý (advisory)
- Nên thêm docstring
";
    let sc_review = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": review_report,
            }
        ]
    });

    let res = sb.run(&["run", "plan-review", "web-ban-hang"], Some(sc_review));
    assert_eq!(
        res.code, 0,
        "plan-review failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: plan-review"));

    let conn = sb.db();
    let (role, stage, round, status, outcome, metrics_json): (
        String,
        Option<String>,
        Option<i64>,
        String,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT role, stage, round, status, outcome, metrics_json FROM runs ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .unwrap();

    assert_eq!(role, "plan-review");
    assert_eq!(stage.as_deref(), Some("plan-review"));
    assert_eq!(round, None);
    assert_eq!(status, "succeeded");
    assert_eq!(outcome.as_deref(), Some("review:approve"));

    let metrics: serde_json::Value =
        serde_json::from_str(&metrics_json.expect("metrics_json should not be None")).unwrap();
    assert_eq!(metrics["verdict"], "approve");
    assert_eq!(metrics["blocking"], 0);
    assert_eq!(metrics["advisory"], 2);

    let task_status: String = conn
        .query_row(
            "SELECT status FROM tasks WHERE slug = ?1",
            rusqlite::params![task],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(task_status, "planned");
}

#[test]
fn plan_commit_lookup_unaffected() {
    let sb = Sandbox::new();
    let (repo, task, plan_sha) = setup_lead(&sb);

    let review_report = "\
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
    let sc_review = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": review_report,
            }
        ]
    });

    let res = sb.run(&["run", "plan-review", "web-ban-hang"], Some(sc_review));
    assert_eq!(res.code, 0);

    let found = agent_brain::gitops::find_commit_by_subject(&repo, &format!("{task}: plan"))
        .unwrap()
        .expect("plan commit should be found");
    assert_eq!(found, plan_sha);
}
