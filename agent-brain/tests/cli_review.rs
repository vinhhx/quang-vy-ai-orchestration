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

fn setup_lead_and_dev(sb: &Sandbox) -> (PathBuf, String, String, String) {
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

#[test]
fn code_review_pass_after_dev() {
    let sb = Sandbox::new();
    let (repo, task, plan_sha, _dev_sha) = setup_lead_and_dev(&sb);

    let pass_report = "\
---
verdict: pass
blocking: 0
advisory: 1
files: []
---
# Review code: sửa app
## Mục chặn (blocking)
## Góp ý (advisory)
- Cân nhắc đặt tên biến rõ hơn
";
    let sc_review = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".code-review.md",
                "content": pass_report,
            }
        ]
    });

    let res = sb.run(&["run", "code-review", "web-ban-hang"], Some(sc_review));
    assert_eq!(
        res.code, 0,
        "code-review failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: code-review"));

    let conn = sb.db();
    let (role, stage, status, outcome): (String, Option<String>, String, Option<String>) = conn
        .query_row(
            "SELECT role, stage, status, outcome FROM runs ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();

    assert_eq!(role, "code-review");
    assert_eq!(stage.as_deref(), Some("code-review"));
    assert_eq!(status, "succeeded");
    assert_eq!(outcome.as_deref(), Some("review:pass"));

    let task_status: String = conn
        .query_row(
            "SELECT status FROM tasks WHERE slug = ?1",
            rusqlite::params![task],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(task_status, "dev_done");

    let recs = sb.records();
    let last_rec = recs.last().unwrap();
    let argv: Vec<String> = last_rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let p_idx = argv.iter().position(|x| x == "-p").unwrap();
    let expected_diff_cmd = format!(
        "git diff {plan_sha}..HEAD -- . ':(exclude).plan.md' ':(exclude).qa-report.md' ':(exclude).plan-review.md' ':(exclude).code-review.md'"
    );
    assert!(argv[p_idx + 1].contains(&expected_diff_cmd));
}

#[test]
fn code_review_needs_code() {
    let sb = Sandbox::new();
    let (_repo, _task, _plan_sha) = setup_lead(&sb);

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
    let res_plan_rev = sb.run(&["run", "plan-review", "web-ban-hang"], Some(sc_review));
    assert_eq!(res_plan_rev.code, 0);

    let res = sb.run(&["run", "code-review", "web-ban-hang"], None);
    assert_eq!(res.code, 1);
    assert!(
        res.stderr.contains("chưa có thay đổi code sau plan"),
        "stderr was: {}",
        res.stderr
    );
}

#[test]
fn code_review_revise_with_files() {
    let sb = Sandbox::new();
    let (repo, task, _plan_sha, _dev_sha) = setup_lead_and_dev(&sb);

    let revise_report = "\
---
verdict: revise
blocking: 2
advisory: 1
files: [\"src/app.py\"]
---
# Review code: sửa app
## Mục chặn (blocking)
- `src/app.py`: thiếu xử lý lỗi
- `src/app.py`: sai kiểu trả về
## Góp ý (advisory)
";
    let sc_review = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".code-review.md",
                "content": revise_report,
            }
        ]
    });

    let res = sb.run(&["run", "code-review", "web-ban-hang"], Some(sc_review));
    assert_eq!(res.code, 0);

    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: code-review"));

    let conn = sb.db();
    let (outcome, metrics_json): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT outcome, metrics_json FROM runs WHERE role = 'code-review' ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    assert_eq!(outcome.as_deref(), Some("review:revise"));
    let metrics: serde_json::Value = serde_json::from_str(&metrics_json.unwrap()).unwrap();
    assert_eq!(metrics["blocking"], 2);
}

#[test]
fn round_two_hint() {
    let sb = Sandbox::new();
    let (_repo, _task, _plan_sha) = setup_lead(&sb);

    let report_1 = "\
---
verdict: revise
blocking: 1
advisory: 0
files: []
---
# Review plan: x
## Mục chặn (blocking)
- lỗi 1
## Góp ý (advisory)
";
    let sc_1 = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": report_1,
            }
        ]
    });
    let res_1 = sb.run(
        &["run", "plan-review", "web-ban-hang", "--round", "1"],
        Some(sc_1),
    );
    assert_eq!(res_1.code, 0);

    let report_2 = "\
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
    let sc_2 = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": report_2,
            }
        ]
    });
    let res_2 = sb.run(
        &["run", "plan-review", "web-ban-hang", "--round", "2"],
        Some(sc_2),
    );
    assert_eq!(res_2.code, 0);

    let recs = sb.records();
    assert!(recs.len() >= 3);
    let rev1_argv: Vec<String> = recs[1]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let rev2_argv: Vec<String> = recs[2]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    let p1 = rev1_argv.iter().position(|x| x == "-p").unwrap();
    let p2 = rev2_argv.iter().position(|x| x == "-p").unwrap();

    assert!(!rev1_argv[p1 + 1].contains("git show HEAD:.plan-review.md"));
    assert!(rev2_argv[p2 + 1].contains("git show HEAD:.plan-review.md"));
}

#[test]
fn dry_run_shows_command() {
    let sb = Sandbox::new();
    let (repo, _task, plan_sha) = setup_lead(&sb);

    let res = sb.run(&["run", "plan-review", "web-ban-hang", "--dry-run"], None);
    assert_eq!(res.code, 0);
    assert!(res.stdout.contains("<assets/prompts/plan-review.md>"));
    assert!(res.stdout.contains("--settings"));

    let conn = sb.db();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE role = 'plan-review'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);

    let head = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    assert_eq!(head, plan_sha);
}

#[test]
fn review_model_default_and_override() {
    let sb = Sandbox::new();
    let (_repo, _task, _plan_sha) = setup_lead(&sb);

    let report_1 = "\
---
verdict: revise
blocking: 1
advisory: 0
files: []
---
# Review plan: x
## Mục chặn (blocking)
- lỗi 1
## Góp ý (advisory)
";
    let sc_1 = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": report_1,
            }
        ]
    });

    let res = sb.run(&["run", "plan-review", "web-ban-hang"], Some(sc_1));
    assert_eq!(res.code, 0);

    let recs = sb.records();
    let last_rec = recs.last().unwrap();
    let argv: Vec<String> = last_rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let m_idx = argv.iter().position(|x| x == "--model").unwrap();
    assert_eq!(argv[m_idx + 1], "opus");

    // Override config
    sb.write_config(serde_json::json!({
        "models": {
            "lead": "opus",
            "dev": "sonnet",
            "qa": "sonnet",
            "plan-review": "sonnet"
        }
    }));

    let report_2 = "\
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
    let sc_2 = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": report_2,
            }
        ]
    });

    let res2 = sb.run(&["run", "plan-review", "web-ban-hang"], Some(sc_2));
    assert_eq!(
        res2.code, 0,
        "res2 failed: stderr: {}\nstdout: {}",
        res2.stderr, res2.stdout
    );

    let recs2 = sb.records();
    let last_rec2 = recs2.last().unwrap();
    let argv2: Vec<String> = last_rec2["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let m2_idx = argv2.iter().position(|x| x == "--model").unwrap();
    assert_eq!(argv2[m2_idx + 1], "sonnet");
}
