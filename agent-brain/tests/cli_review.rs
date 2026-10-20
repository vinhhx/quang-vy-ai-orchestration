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
fn identical_report_rerun_ok() {
    let sb = Sandbox::new();
    let (repo, _task, _plan_sha) = setup_lead(&sb);

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
        Some(sc_review.clone()),
    );
    assert_eq!(
        res.code, 0,
        "plan-review round 1 failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let head_before = sb.git(&repo, &["rev-parse", "HEAD"]);

    let res = sb.run(
        &["run", "plan-review", "web-ban-hang", "--round", "2"],
        Some(sc_review),
    );
    assert_eq!(
        res.code, 0,
        "plan-review round 2 failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let head_after = sb.git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(head_before.trim(), head_after.trim());
    assert!(
        res.stdout.contains("giống bản đã commit"),
        "stdout missing unchanged-report line: {}",
        res.stdout
    );

    let conn = sb.db();
    let (role, round, status, outcome): (String, Option<i64>, String, Option<String>) = conn
        .query_row(
            "SELECT role, round, status, outcome FROM runs ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(role, "plan-review");
    assert_eq!(round, Some(2));
    assert_eq!(status, "succeeded");
    assert_eq!(outcome.as_deref(), Some("review:revise"));

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

struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn review_violation_blocked_by_guard() {
    let sb = Sandbox::new();
    let (repo, task, _plan_sha) = setup_lead(&sb);

    let original_app = std::fs::read_to_string(repo.join("src/app.py")).unwrap();

    let approve_report = "\
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
    let sc = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": "src/app.py",
                "content": "def app(): return 999\n",
            },
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": approve_report,
            }
        ]
    });

    let res = sb.run(&["run", "plan-review", "web-ban-hang"], Some(sc));
    assert_eq!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let current_app = std::fs::read_to_string(repo.join("src/app.py")).unwrap();
    assert_eq!(current_app, original_app);

    let recs = sb.records();
    let last_rec = recs.last().unwrap();
    let blocked = last_rec["blocked"].as_array().expect("blocked array");
    assert!(!blocked.is_empty(), "expected blocked entries");
    let has_app_blocked = blocked.iter().any(|b| {
        let tool = b["tool"].as_str().unwrap_or("");
        let path = b["path"].as_str().unwrap_or("");
        (tool == "Write" || tool == "Edit") && path.ends_with("src/app.py")
    });
    assert!(has_app_blocked, "blocked was: {:?}", blocked);

    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: plan-review"));
}

#[test]
fn review_violation() {
    let sb = Sandbox::new();
    let (repo, _task, plan_sha) = setup_lead(&sb);

    let approve_report = "\
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
    let sc = serde_json::json!({
        "ignore_hooks": true,
        "ops": [
            {
                "op": "write",
                "path": "src/app.py",
                "content": "def app(): return 999\n",
            },
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": approve_report,
            }
        ]
    });

    let res = sb.run(&["run", "plan-review", "web-ban-hang"], Some(sc));
    assert_eq!(res.code, 3);
    assert!(
        res.stdout.contains("vi phạm giao thức"),
        "stdout was: {}",
        res.stdout
    );

    let conn = sb.db();
    let outcome: Option<String> = conn
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'plan-review' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome.as_deref(), Some("protocol_violation"));

    let curr_head = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    assert_eq!(curr_head, plan_sha);
}

#[test]
fn malformed_reports_are_violations() {
    let sb = Sandbox::new();
    let (repo, _task, plan_sha) = setup_lead(&sb);

    // (a) fake writes nothing -> skeleton stays
    let sc_a = serde_json::json!({
        "ops": []
    });
    let res_a = sb.run(&["run", "plan-review", "web-ban-hang"], Some(sc_a));
    assert_eq!(res_a.code, 3);
    assert!(
        res_a.stdout.contains("thiếu verdict"),
        "stdout was: {}",
        res_a.stdout
    );
    let conn = sb.db();
    let outcome: Option<String> = conn
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'plan-review' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome.as_deref(), Some("protocol_violation"));
    assert_eq!(sb.git(&repo, &["rev-parse", "HEAD"]).trim(), plan_sha);

    // (b) report without front matter
    let sc_b = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": "# Review plan without front matter\n"
            }
        ]
    });
    let res_b = sb.run(&["run", "plan-review", "web-ban-hang"], Some(sc_b));
    assert_eq!(res_b.code, 3);
    assert!(
        res_b.stdout.contains("thiếu khối front matter"),
        "stdout was: {}",
        res_b.stdout
    );
    let outcome: Option<String> = conn
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'plan-review' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome.as_deref(), Some("protocol_violation"));
    assert_eq!(sb.git(&repo, &["rev-parse", "HEAD"]).trim(), plan_sha);

    // (c) verdict approve with blocking 1
    let sc_c = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": "---\nverdict: approve\nblocking: 1\nadvisory: 0\nfiles: []\n---\n# Review plan\n"
            }
        ]
    });
    let res_c = sb.run(&["run", "plan-review", "web-ban-hang"], Some(sc_c));
    assert_eq!(res_c.code, 3);
    assert!(
        res_c
            .stdout
            .contains("có 1 mục chặn nên verdict phải là revise"),
        "stdout was: {}",
        res_c.stdout
    );
    let outcome: Option<String> = conn
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'plan-review' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome.as_deref(), Some("protocol_violation"));
    assert_eq!(sb.git(&repo, &["rev-parse", "HEAD"]).trim(), plan_sha);
}

#[test]
fn forbidden_args_refused() {
    let sb = Sandbox::new();
    let (_repo, _task, _plan_sha) = setup_lead(&sb);

    sb.write_config(serde_json::json!({
        "common_args": ["--strict-mcp-config", "--dangerously-skip-permissions"]
    }));

    let out = sb.run(&["run", "plan-review", "web-ban-hang"], None);
    assert_eq!(out.code, 1);
    assert!(
        out.stderr
            .contains("Cờ quyền không được phép: --dangerously-skip-permissions"),
        "stderr was: {}",
        out.stderr
    );
}

#[test]
fn review_run_counts_for_personal_busy_rule() {
    let sb = Sandbox::new();
    let repo1 = sb.make_repo("proj-1", &[("src/main.py", "")], None, None);
    let repo2 = sb.make_repo("proj-2", &[("src/main.py", "")], None, None);
    let mut entry1 = sb.project_entry(&repo1, "proj-1", "personal");
    entry1["name_vi"] = serde_json::json!("Dự án 1");
    let mut entry2 = sb.project_entry(&repo2, "proj-2", "personal");
    entry2["name_vi"] = serde_json::json!("Dự án 2");
    sb.write_map(serde_json::json!([entry1, entry2]), &[]);

    let decoy_child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn decoy sleep");
    let decoy = ChildGuard(decoy_child);
    let decoy_pid = decoy.0.id() as i32;
    let proc_stat = agent_brain::procs::read_stat(decoy_pid).expect("read decoy stat");
    let decoy_start = proc_stat.start;

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

    let out = sb.run(&["run", "lead", "proj-2", "thêm việc"], None);
    assert_eq!(
        out.code, 1,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stderr
            .contains("không chạy song song trên cùng thư mục cấu hình (E-06)"),
        "stderr missing E-06 message: {}",
        out.stderr
    );
}

#[test]
fn dirty_tree_refused_report_excepted() {
    let sb = Sandbox::new();
    let (repo, _task, _plan_sha) = setup_lead(&sb);

    // An untracked file in the repo
    std::fs::write(repo.join("untracked.txt"), "hello").unwrap();
    let out = sb.run(&["run", "plan-review", "web-ban-hang"], None);
    assert_eq!(out.code, 1);
    assert!(
        out.stderr
            .contains("Working tree có thay đổi chưa commit:\n  untracked.txt"),
        "stderr was: {}",
        out.stderr
    );

    // Clean up untracked file
    std::fs::remove_file(repo.join("untracked.txt")).unwrap();

    // A dirty .plan-review.md alone does not block a new review
    std::fs::write(repo.join(".plan-review.md"), "dirty content").unwrap();

    let approve_report = "\
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
    let sc = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": approve_report,
            }
        ]
    });

    let res = sb.run(&["run", "plan-review", "web-ban-hang"], Some(sc));
    assert_eq!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}
