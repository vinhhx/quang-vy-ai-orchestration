mod common;
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::{lead_scenario, setup_project};
use std::path::PathBuf;

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
fn fix_after_code_review() {
    let sb = Sandbox::new();
    let (repo, task, _plan_sha, _dev_sha) = setup_lead_and_dev(&sb);

    let review_report = "\
---
verdict: revise
blocking: 1
advisory: 0
files: [\"src/util.py\"]
---
# Review code: sửa app
## Mục chặn (blocking)
- `src/app.py:2` trả về sai
## Góp ý (advisory)
";
    let sc_review = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".code-review.md",
                "content": review_report,
            }
        ]
    });
    let res_review = sb.run(&["run", "code-review", "web-ban-hang"], Some(sc_review));
    assert_eq!(
        res_review.code, 0,
        "code-review failed: stderr: {}\nstdout: {}",
        res_review.stderr, res_review.stdout
    );

    let sc_fix = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": ".code-review.md"},
            {"op": "write", "path": "src/util.py", "content": "def util(): return 1\n"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 3\n"},
            {"op": "git", "args": ["add", "src/app.py", "src/util.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa theo review"]}
        ]
    });
    let res_fix = sb.run(
        &[
            "run",
            "dev",
            "web-ban-hang",
            "--fix",
            "code-review",
            "--round",
            "2",
        ],
        Some(sc_fix),
    );
    assert_eq!(
        res_fix.code, 0,
        "fix failed: stderr: {}\nstdout: {}",
        res_fix.stderr, res_fix.stdout
    );

    let conn = sb.db();
    let (role, stage, round, metrics_json, status, run_dir): (
        String,
        Option<String>,
        Option<i64>,
        Option<String>,
        String,
        String,
    ) = conn
        .query_row(
            "SELECT role, stage, round, metrics_json, status, run_dir FROM runs ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .unwrap();

    assert_eq!(role, "dev");
    assert_eq!(stage.as_deref(), Some("fix"));
    assert_eq!(round, Some(2));
    assert_eq!(status, "succeeded");
    let metrics: serde_json::Value =
        serde_json::from_str(&metrics_json.unwrap_or_default()).unwrap();
    assert_eq!(metrics["fix_for"], "code_review");

    let task_status: String = conn
        .query_row("SELECT status FROM tasks WHERE slug = ?1", [&task], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(task_status, "dev_done");

    let head_subject = sb
        .git(&repo, &["log", "-1", "--format=%s"])
        .trim()
        .to_string();
    assert_eq!(head_subject, "sua-app: sửa theo review");

    let recs = sb.records();
    let last_rec = recs.last().unwrap();
    let argv = last_rec["argv"].as_array().unwrap();
    let argv_strs: Vec<&str> = argv.iter().filter_map(|v| v.as_str()).collect();
    assert!(argv_strs
        .iter()
        .any(|s| s.contains("Bạn là Dev (chế độ sửa).")));
    assert!(argv_strs.contains(&"--settings"));

    let policy_path = PathBuf::from(run_dir).join("policy.json");
    let policy_str = std::fs::read_to_string(&policy_path).unwrap();
    let policy: serde_json::Value = serde_json::from_str(&policy_str).unwrap();

    let write_allow: Vec<PathBuf> = policy["write_allow"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| std::fs::canonicalize(v.as_str().unwrap()).unwrap())
        .collect();
    let mut expected_write = vec![
        std::fs::canonicalize(repo.join("src/app.py")).unwrap(),
        std::fs::canonicalize(repo.join("src/util.py")).unwrap(),
        std::fs::canonicalize(repo.join(".plan.md")).unwrap(),
    ];
    let mut actual_write = write_allow;
    expected_write.sort();
    actual_write.sort();
    assert_eq!(actual_write, expected_write);

    let read_allow: Vec<PathBuf> = policy["read_allow"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| std::fs::canonicalize(v.as_str().unwrap()).unwrap())
        .collect();
    let mut expected_read = expected_write.clone();
    expected_read.push(std::fs::canonicalize(repo.join(".code-review.md")).unwrap());
    expected_read.sort();
    let mut actual_read = read_allow;
    actual_read.sort();
    assert_eq!(actual_read, expected_read);
}

#[test]
fn fix_dry_run() {
    let sb = Sandbox::new();
    let (_repo, _task, _plan_sha, _dev_sha) = setup_lead_and_dev(&sb);

    let review_report = "\
---
verdict: revise
blocking: 1
advisory: 0
files: [\"src/util.py\"]
---
# Review code: sửa app
## Mục chặn (blocking)
- `src/app.py:2` trả về sai
## Góp ý (advisory)
";
    let sc_review = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".code-review.md",
                "content": review_report,
            }
        ]
    });
    let res_review = sb.run(&["run", "code-review", "web-ban-hang"], Some(sc_review));
    assert_eq!(res_review.code, 0);

    let runs_before: i64 = sb
        .db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap();

    let res_dry = sb.run(
        &[
            "run",
            "dev",
            "web-ban-hang",
            "--fix",
            "code-review",
            "--dry-run",
        ],
        None,
    );
    assert_eq!(
        res_dry.code, 0,
        "dry-run failed: stderr: {}\nstdout: {}",
        res_dry.stderr, res_dry.stdout
    );
    assert!(res_dry
        .stdout
        .contains("Allowlist: src/app.py, src/util.py"));
    assert!(res_dry.stdout.contains("<assets/prompts/dev-fix.md>"));

    let runs_after: i64 = sb
        .db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(runs_before, runs_after);
}

#[test]
fn fix_flag_only_for_dev() {
    let sb = Sandbox::new();
    setup_project(&sb, "work");

    let res = sb.run(&["run", "qa", "web-ban-hang", "--fix", "qa"], None);
    assert_eq!(res.code, 1);
    assert!(res.stderr.contains("--fix chỉ dùng với vai trò dev"));
}
