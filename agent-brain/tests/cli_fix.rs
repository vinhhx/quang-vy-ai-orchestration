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

#[test]
fn fix_write_outside_blocked() {
    let sb = Sandbox::new();
    let (repo, task, _plan_sha, _dev_sha) = setup_lead_and_dev(&sb);

    std::fs::write(repo.join("src/other.py"), "def other(): pass\n").unwrap();
    sb.git(&repo, &["add", "src/other.py"]);
    sb.git(&repo, &["commit", "-m", "add other.py"]);

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

    let sc_fix_blocked = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": ".code-review.md"},
            {"op": "write", "path": "src/other.py", "content": "def other(): return 99\n"},
            {"op": "write", "path": "src/app.py", "content": "def app(): return 4\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", format!("{task}: sửa theo review")]}
        ]
    });
    let res_fix_blocked = sb.run(
        &["run", "dev", "web-ban-hang", "--fix", "code-review"],
        Some(sc_fix_blocked),
    );
    assert_eq!(
        res_fix_blocked.code, 0,
        "stderr: {}",
        res_fix_blocked.stderr
    );
    let dev_rec = sb.records().pop().unwrap();
    let blocked = dev_rec["blocked"].as_array().expect("blocked array");
    assert!(
        blocked.iter().any(|b| b["path"]
            .as_str()
            .map(|p| p.contains("src/other.py") || p.contains("other.py"))
            .unwrap_or(false)),
        "blocked missing other.py: {:?}",
        blocked
    );

    let sc_fix_ignore = serde_json::json!({
        "ignore_hooks": true,
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": ".code-review.md"},
            {"op": "write", "path": "src/other.py", "content": "def other(): return 100\n"}
        ]
    });
    let res_fix_ignore = sb.run(
        &["run", "dev", "web-ban-hang", "--fix", "code-review"],
        Some(sc_fix_ignore),
    );
    assert_eq!(res_fix_ignore.code, 3);
    assert!(res_fix_ignore.stdout.contains("vi phạm giao thức"));

    let conn = sb.db();
    let outcome: String = conn
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'dev' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome, "protocol_violation");
}

#[test]
fn fix_cannot_edit_report() {
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

    let sc_edit_report = serde_json::json!({
        "ignore_hooks": true,
        "ops": [
            {"op": "read", "path": ".code-review.md"},
            {"op": "write", "path": ".code-review.md", "content": "edited report\n"}
        ]
    });
    let res = sb.run(
        &["run", "dev", "web-ban-hang", "--fix", "code-review"],
        Some(sc_edit_report),
    );
    assert_eq!(res.code, 3);
    assert!(res.stdout.contains("vi phạm giao thức"));
}

#[test]
fn fix_report_not_actionable() {
    let sb = Sandbox::new();
    let (repo, _task, _plan_sha, _dev_sha) = setup_lead_and_dev(&sb);

    let count_runs = || -> i64 {
        sb.db()
            .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
            .unwrap()
    };
    let initial_runs = count_runs();

    // 1. No .code-review.md -> exit 1 "Không tìm thấy .code-review.md"
    let res_missing = sb.run(
        &["run", "dev", "web-ban-hang", "--fix", "code-review"],
        None,
    );
    assert_eq!(res_missing.code, 1);
    assert!(res_missing
        .stderr
        .contains("Không tìm thấy .code-review.md"));
    assert_eq!(count_runs(), initial_runs);

    // 2. Code-review pass report -> exit 1 "không yêu cầu sửa (verdict pass)"
    let pass_report = "\
---
verdict: pass
blocking: 0
advisory: 1
---
# Review code: sửa app
## Mục chặn (blocking)
## Góp ý (advisory)
- tốt
";
    let sc_pass = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".code-review.md",
                "content": pass_report,
            }
        ]
    });
    let res_rev = sb.run(&["run", "code-review", "web-ban-hang"], Some(sc_pass));
    assert_eq!(res_rev.code, 0);
    let runs_after_review = count_runs();

    let res_not_req = sb.run(
        &["run", "dev", "web-ban-hang", "--fix", "code-review"],
        None,
    );
    assert_eq!(res_not_req.code, 1);
    assert!(res_not_req
        .stderr
        .contains("không yêu cầu sửa (verdict pass)"));
    assert_eq!(count_runs(), runs_after_review);

    // 3. Malformed review report -> exit 1 "không đúng định dạng"
    std::fs::write(
        repo.join(".code-review.md"),
        "malformed without frontmatter\n",
    )
    .unwrap();
    sb.git(&repo, &["add", ".code-review.md"]);
    sb.git(&repo, &["commit", "-m", "malformed review"]);

    let res_mal = sb.run(
        &["run", "dev", "web-ban-hang", "--fix", "code-review"],
        None,
    );
    assert_eq!(res_mal.code, 1);
    assert!(res_mal.stderr.contains("không đúng định dạng"));
    assert_eq!(count_runs(), runs_after_review);

    // 4. QA report Đạt with --fix qa -> exit 1 "không kết luận Cần sửa"
    let qa_pass = "\
# QA: sửa app
## Thay đổi có khớp plan không (từng mục)
- ok
## Sửa ngoài plan (nếu có)
## Rủi ro
## Kết luận
Đạt
";
    let sc_qa = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".qa-report.md",
                "content": qa_pass,
            }
        ]
    });
    let res_qa = sb.run(&["run", "qa", "web-ban-hang"], Some(sc_qa));
    assert_eq!(res_qa.code, 0);
    let runs_after_qa = count_runs();

    let res_qa_pass_fix = sb.run(&["run", "dev", "web-ban-hang", "--fix", "qa"], None);
    assert_eq!(res_qa_pass_fix.code, 1);
    assert!(res_qa_pass_fix.stderr.contains("không kết luận Cần sửa"));
    assert_eq!(count_runs(), runs_after_qa);
}

#[test]
fn fix_named_secret_rejected() {
    let sb = Sandbox::new();
    let (repo, _task, _plan_sha, _dev_sha) = setup_lead_and_dev(&sb);

    std::fs::write(repo.join(".gitignore"), ".env\n").unwrap();
    sb.git(&repo, &["add", ".gitignore"]);
    sb.git(&repo, &["commit", "-m", "add gitignore"]);

    std::fs::write(repo.join(".env"), "SECRET=123\n").unwrap();

    let review_report = "\
---
verdict: revise
blocking: 1
advisory: 0
files: [\".env\", \"src/util.py\"]
---
# Review code: sửa app
## Mục chặn (blocking)
- sửa util
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
    let res_rev = sb.run(&["run", "code-review", "web-ban-hang"], Some(sc_review));
    assert_eq!(res_rev.code, 0);

    let sc_fix = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": ".code-review.md"},
            {"op": "write", "path": ".env", "content": "SECRET=hacked\n"},
            {"op": "write", "path": "src/app.py", "content": "def app(): return 5\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa theo review"]}
        ]
    });
    let res_fix = sb.run(
        &["run", "dev", "web-ban-hang", "--fix", "code-review"],
        Some(sc_fix),
    );
    assert_eq!(
        res_fix.code, 0,
        "fix failed: stderr: {}\nstdout: {}",
        res_fix.stderr, res_fix.stdout
    );
    assert!(res_fix
        .stdout
        .contains("Bỏ qua (không được phép): .env (sensitive:.env)"));

    let dev_rec = sb.records().pop().unwrap();
    let blocked = dev_rec["blocked"].as_array().expect("blocked array");
    assert!(
        blocked.iter().any(|b| b["path"]
            .as_str()
            .map(|p| p.contains(".env"))
            .unwrap_or(false)),
        "blocked missing .env: {:?}",
        blocked
    );

    let conn = sb.db();
    let run_dir: String = conn
        .query_row(
            "SELECT run_dir FROM runs WHERE role = 'dev' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let policy_path = PathBuf::from(run_dir).join("policy.json");
    let policy_str = std::fs::read_to_string(&policy_path).unwrap();
    let policy: serde_json::Value = serde_json::from_str(&policy_str).unwrap();
    let write_allow = policy["write_allow"].as_array().unwrap();
    for p in write_allow {
        assert!(!p.as_str().unwrap().contains(".env"));
    }
}

#[test]
fn fix_after_qa() {
    let sb = Sandbox::new();
    let (_repo, task, _plan_sha, _dev_sha) = setup_lead_and_dev(&sb);

    let qa_report = "\
# QA: sửa app
## Thay đổi có khớp plan không (từng mục)
- lỗi logic
## Sửa ngoài plan (nếu có)
## Rủi ro
## Kết luận
Cần sửa
";
    let sc_qa = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".qa-report.md",
                "content": qa_report,
            }
        ]
    });
    let res_qa = sb.run(&["run", "qa", "web-ban-hang"], Some(sc_qa));
    assert_eq!(res_qa.code, 0);

    let sc_fix = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": ".qa-report.md"},
            {"op": "write", "path": "src/app.py", "content": "def app(): return 10\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa theo QA"]}
        ]
    });
    let res_fix = sb.run(
        &["run", "dev", "web-ban-hang", "--fix", "qa", "--round", "1"],
        Some(sc_fix),
    );
    assert_eq!(
        res_fix.code, 0,
        "fix failed: stderr: {}\nstdout: {}",
        res_fix.stderr, res_fix.stdout
    );

    let conn = sb.db();
    let (role, stage, round, metrics_json, status): (
        String,
        Option<String>,
        Option<i64>,
        Option<String>,
        String,
    ) = conn
        .query_row(
            "SELECT role, stage, round, metrics_json, status FROM runs ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();

    assert_eq!(role, "dev");
    assert_eq!(stage.as_deref(), Some("fix"));
    assert_eq!(round, Some(1));
    assert_eq!(status, "succeeded");
    let metrics: serde_json::Value =
        serde_json::from_str(&metrics_json.unwrap_or_default()).unwrap();
    assert_eq!(metrics["fix_for"], "qa");

    let task_status: String = conn
        .query_row("SELECT status FROM tasks WHERE slug = ?1", [&task], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(task_status, "dev_done");
}

#[test]
fn fix_policy_snapshot() {
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

    let sc_fix = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": ".code-review.md"},
            {"op": "write", "path": ".code-review.md", "content": "append `src/extra.py`\n"},
            {"op": "write", "path": "src/app.py", "content": "def app(): return 6\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa theo review"]}
        ]
    });
    let res_fix = sb.run(
        &["run", "dev", "web-ban-hang", "--fix", "code-review"],
        Some(sc_fix),
    );
    assert_eq!(res_fix.code, 0);

    let conn = sb.db();
    let run_dir: String = conn
        .query_row(
            "SELECT run_dir FROM runs WHERE role = 'dev' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let policy_path = PathBuf::from(run_dir).join("policy.json");
    let policy_str = std::fs::read_to_string(&policy_path).unwrap();
    let policy: serde_json::Value = serde_json::from_str(&policy_str).unwrap();
    let write_allow = policy["write_allow"].as_array().unwrap();
    for p in write_allow {
        assert!(!p.as_str().unwrap().contains("extra.py"));
    }
}

#[test]
fn implicit_approval() {
    let sb = Sandbox::new();
    let (_repo, task, plan_sha) = {
        let repo = setup_project(&sb, "work");
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
        assert_eq!(res_lead.code, 0);
        let task = "sua-app".to_string();
        let plan_sha = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
        (repo, task, plan_sha)
    };

    let conn = sb.db();
    let tid: i64 = conn
        .query_row("SELECT id FROM tasks WHERE slug = ?1", [&task], |r| {
            r.get(0)
        })
        .unwrap();

    // 1. Task with workflow_json NULL keeps both NULL
    let sc_dev1 = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app(): return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", format!("{task}: sửa app")]}
        ]
    });
    let res_dev1 = sb.run(&["run", "dev", "web-ban-hang"], Some(sc_dev1));
    assert_eq!(res_dev1.code, 0);

    let (app_at, app_commit): (Option<i64>, Option<String>) = conn
        .query_row(
            "SELECT approved_at, approved_commit FROM tasks WHERE id = ?1",
            [tid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(app_at.is_none());
    assert!(app_commit.is_none());

    // 2. Set workflow_json = '{"name":"nhanh"}'
    conn.execute(
        "UPDATE tasks SET workflow_json = '{\"name\":\"nhanh\"}' WHERE id = ?1",
        [tid],
    )
    .unwrap();

    let sc_dev2 = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app(): return 3\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", format!("{task}: sửa tiếp")]}
        ]
    });
    let res_dev2 = sb.run(&["run", "dev", "web-ban-hang"], Some(sc_dev2));
    assert_eq!(res_dev2.code, 0);

    let (app_at, app_commit): (Option<i64>, Option<String>) = conn
        .query_row(
            "SELECT approved_at, approved_commit FROM tasks WHERE id = ?1",
            [tid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(app_at.is_some());
    assert_eq!(app_commit.as_deref(), Some(plan_sha.as_str()));

    // 3. A second Dev run on the same plan leaves approved_at unchanged
    let saved_at = app_at.unwrap();
    let sc_dev3 = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app(): return 4\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", format!("{task}: sửa lại")]}
        ]
    });
    let res_dev3 = sb.run(&["run", "dev", "web-ban-hang"], Some(sc_dev3));
    assert_eq!(res_dev3.code, 0);

    let (app_at2, app_commit2): (Option<i64>, Option<String>) = conn
        .query_row(
            "SELECT approved_at, approved_commit FROM tasks WHERE id = ?1",
            [tid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(app_at2, Some(saved_at));
    assert_eq!(app_commit2.as_deref(), Some(plan_sha.as_str()));
}

#[test]
fn implement_stage_recorded() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let sc_lead = lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")]);
    sb.run(
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

    let sc_dev1 = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app(): return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
        ]
    });
    let res1 = sb.run(
        &["run", "dev", "web-ban-hang", "--round", "1"],
        Some(sc_dev1),
    );
    assert_eq!(res1.code, 0);

    let conn = sb.db();
    let (stage, round): (Option<String>, Option<i64>) = conn
        .query_row(
            "SELECT stage, round FROM runs WHERE role = 'dev' ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(stage.as_deref(), Some("implement"));
    assert_eq!(round, Some(1));

    let sc_dev2 = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app(): return 3\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa app tiếp"]}
        ]
    });
    let res2 = sb.run(&["run", "dev", "web-ban-hang"], Some(sc_dev2));
    assert_eq!(res2.code, 0);

    let (stage2, round2): (Option<String>, Option<i64>) = conn
        .query_row(
            "SELECT stage, round FROM runs WHERE role = 'dev' ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(stage2.as_deref(), Some("implement"));
    assert_eq!(round2, None);
}
