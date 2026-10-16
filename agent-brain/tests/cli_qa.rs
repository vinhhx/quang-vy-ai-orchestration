mod common;
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::{lead_scenario, setup_project};
use std::path::PathBuf;

fn setup_lead_and_dev(sb: &Sandbox) -> (PathBuf, String, String, String) {
    let repo = setup_project(sb, "work");

    // 1. Lead
    let sc_lead = lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")]);
    let res_lead = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(sc_lead));
    assert_eq!(
        res_lead.code, 0,
        "Lead failed: stderr: {}\nstdout: {}",
        res_lead.stderr, res_lead.stdout
    );
    let task = "sua-app".to_string();
    let plan_sha = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();

    // 2. Dev
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
fn p1_09_qa_writes_report_without_code_change() {
    let sb = Sandbox::new();
    let (repo, task, plan_sha, _dev_sha) = setup_lead_and_dev(&sb);

    let qa_report_content = "\
# QA: sửa app
## Thay đổi có khớp plan không (từng mục)
- src/app.py: khớp.
## Sửa ngoài plan (nếu có)
Không.
## Rủi ro
Không phát hiện rủi ro.
## Kết luận
Đạt
";
    let qa_scenario = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".qa-report.md",
                "content": qa_report_content,
                "keep_meta": true
            }
        ]
    });

    let res = sb.run(&["run", "qa", "web-ban-hang"], Some(qa_scenario));
    assert_eq!(
        res.code, 0,
        "QA failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: qa"));

    let diff_tree = sb.git(
        &repo,
        &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
    );
    assert_eq!(diff_tree.trim(), ".qa-report.md");

    let recs = sb.records();
    assert!(recs.len() >= 3);
    let qa_rec = &recs[recs.len() - 1];
    let argv: Vec<String> = qa_rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    assert!(argv.contains(&"Write(./.qa-report.md)".to_string()));
    assert!(argv.contains(&"Edit(./.qa-report.md)".to_string()));

    let repo_str = repo.to_string_lossy();
    assert!(argv.contains(&format!("Write(/{repo_str}/.qa-report.md)")));
    assert!(argv.contains(&format!("Edit(/{repo_str}/.qa-report.md)")));

    let model_idx = argv.iter().position(|x| x == "--model").unwrap();
    assert_eq!(argv[model_idx + 1], "sonnet");

    let p_idx = argv.iter().position(|x| x == "-p").unwrap();
    let user_input = &argv[p_idx + 1];
    assert!(user_input.contains(&format!("git diff {plan_sha}..HEAD")));
    assert!(user_input.contains("':(exclude).plan.md'"));
    assert!(user_input.contains("':(exclude).qa-report.md'"));

    let prompt_idx = argv
        .iter()
        .position(|x| x == "--append-system-prompt")
        .unwrap();
    assert!(argv[prompt_idx + 1].starts_with("Bạn là QA."));

    assert!(res.stdout.contains("Kết luận QA: Đạt"));

    let conn = sb.db();
    let outcome: String = conn
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'qa' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome, "qa:pass");

    let status: String = conn
        .query_row(
            "SELECT status FROM tasks WHERE slug = ?1",
            rusqlite::params![task],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "qa_pass");
}

#[test]
fn qa_code_edit_is_violation() {
    let sb = Sandbox::new();
    let (repo, _task, _plan_sha, dev_sha) = setup_lead_and_dev(&sb);

    let qa_scenario = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".qa-report.md",
                "content": "# QA\n## Kết luận\nĐạt\n",
                "keep_meta": true
            },
            {
                "op": "write",
                "path": "src/app.py",
                "content": "def app(): return 999\n"
            }
        ]
    });

    let res = sb.run(&["run", "qa", "web-ban-hang"], Some(qa_scenario));
    assert_eq!(res.code, 3);
    assert!(res.stdout.contains("src/app.py"));
    assert!(res
        .stdout
        .contains("QA đã sửa file ngoài .qa-report.md — vi phạm giao thức (ROLE-04):"));

    let curr_head = sb.git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(curr_head.trim(), dev_sha);

    let conn = sb.db();
    let outcome: String = conn
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'qa' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome, "protocol_violation");
}

#[test]
fn qa_without_verdict_exit_1() {
    let sb = Sandbox::new();
    let (repo, _task, _plan_sha, dev_sha) = setup_lead_and_dev(&sb);

    let qa_scenario = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".qa-report.md",
                "content": "# QA\n## Kết luận\nChưa biết\n",
                "keep_meta": true
            }
        ]
    });

    let res = sb.run(&["run", "qa", "web-ban-hang"], Some(qa_scenario));
    assert_eq!(res.code, 1);
    assert!(res.stderr.contains("QA chưa ghi Kết luận (Đạt / Cần sửa)"));

    let curr_head = sb.git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(curr_head.trim(), dev_sha);
    assert!(repo.join(".qa-report.md").is_file());
}

#[test]
fn qa_can_sua_reports_next_steps() {
    let sb = Sandbox::new();
    let (repo, task, _plan_sha, _dev_sha) = setup_lead_and_dev(&sb);

    let qa_report_content = "\
# QA: sửa app
## Thay đổi có khớp plan không (từng mục)
Chưa khớp.
## Sửa ngoài plan (nếu có)
Không.
## Rủi ro
Có lỗi logic.
## Kết luận
Cần sửa
";
    let qa_scenario = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".qa-report.md",
                "content": qa_report_content,
                "keep_meta": true
            }
        ]
    });

    let res = sb.run(&["run", "qa", "web-ban-hang"], Some(qa_scenario));
    assert_eq!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(res.stdout.contains("Cần sửa"));
    assert!(res.stdout.contains("agent-brain run lead"));
    assert!(res.stdout.contains("agent-brain run dev"));

    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: qa"));

    let conn = sb.db();
    let outcome: String = conn
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'qa' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome, "qa:fix");

    let status: String = conn
        .query_row(
            "SELECT status FROM tasks WHERE slug = ?1",
            rusqlite::params![task],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "qa_fix");
}

#[test]
fn qa_no_code_change_exit_1() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let sc_lead = lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")]);
    let res_lead = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(sc_lead));
    assert_eq!(res_lead.code, 0);

    let records_before = sb.records().len();
    let res_qa = sb.run(&["run", "qa", "web-ban-hang"], None);
    assert_eq!(res_qa.code, 1);
    assert!(res_qa.stderr.contains("chưa có thay đổi code sau plan"));
    assert_eq!(sb.records().len(), records_before);
}

#[test]
fn qa_refusals() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    // 1. Not on task branch (on main)
    let res1 = sb.run(&["run", "qa", "web-ban-hang"], None);
    assert_eq!(res1.code, 1);

    // 2. On task branch without {task}: plan commit
    sb.git(&repo, &["checkout", "-b", "task/fake-task"]);
    let meta_json = serde_json::json!({
        "task": "fake-task",
        "branch": "task/fake-task"
    });
    let ml = agent_brain::planfile::meta_line(&meta_json);
    std::fs::write(
        repo.join(".plan.md"),
        format!("# Plan: Fake\n{ml}\n## Mục tiêu\nx\n## Thay đổi\n| # | File | Dòng | Sửa | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | 1 | x | y |\n## Không được đụng tới\n\n## Cách kiểm tra\n\n## Vướng mắc\n\n"),
    )
    .unwrap();
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", "init fake task"]);

    let res2 = sb.run(&["run", "qa", "web-ban-hang"], None);
    assert_eq!(res2.code, 1);
    assert!(res2
        .stderr
        .contains("Không tìm thấy commit 'fake-task: plan' trên nhánh hiện tại"));
}

#[test]
fn role_05_pipeline_lead_dev_qa() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    let task = "sua-app";

    // 1. Lead
    let sc_lead = lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")]);
    let res_lead = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(sc_lead));
    assert_eq!(
        res_lead.code, 0,
        "Lead failed: stderr: {}\nstdout: {}",
        res_lead.stderr, res_lead.stdout
    );

    // 2. Dev
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

    // 3. QA
    let qa_report_content = "\
# QA: sửa app
## Thay đổi có khớp plan không (từng mục)
- src/app.py: khớp.
## Sửa ngoài plan (nếu có)
Không.
## Rủi ro
Không có.
## Kết luận
Đạt
";
    let sc_qa = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".qa-report.md",
                "content": qa_report_content,
                "keep_meta": true
            }
        ]
    });
    let res_qa = sb.run(&["run", "qa", "web-ban-hang"], Some(sc_qa));
    assert_eq!(
        res_qa.code, 0,
        "QA failed: stderr: {}\nstdout: {}",
        res_qa.stderr, res_qa.stdout
    );

    // Verify commits in order on task branch
    let subjects: Vec<String> = sb
        .git(&repo, &["log", "--reverse", "--format=%s", "main..HEAD"])
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(
        subjects,
        vec![
            format!("{task}: plan"),
            format!("{task}: sửa app"),
            format!("{task}: qa")
        ]
    );

    // Verify 3 records
    let recs = sb.records();
    assert_eq!(recs.len(), 3);

    let lead_argv = recs[0]["argv"].as_array().unwrap();
    let dev_argv = recs[1]["argv"].as_array().unwrap();
    let qa_argv = recs[2]["argv"].as_array().unwrap();

    let get_model = |argv: &[serde_json::Value]| -> String {
        let idx = argv.iter().position(|x| x == "--model").unwrap();
        argv[idx + 1].as_str().unwrap().to_string()
    };
    assert_eq!(get_model(lead_argv), "opus");
    assert_eq!(get_model(dev_argv), "sonnet");
    assert_eq!(get_model(qa_argv), "sonnet");

    let get_prompt = |argv: &[serde_json::Value]| -> String {
        let idx = argv
            .iter()
            .position(|x| x == "--append-system-prompt")
            .unwrap();
        argv[idx + 1].as_str().unwrap().to_string()
    };
    assert!(get_prompt(lead_argv).starts_with("Bạn là Lead."));
    assert!(get_prompt(dev_argv).starts_with("Bạn là Dev."));
    assert!(get_prompt(qa_argv).starts_with("Bạn là QA."));

    // Verify 3 runs rows in SQLite with roles lead, dev, qa, same task_id, status succeeded
    let conn = sb.db();
    let rows: Vec<(String, Option<i64>, String)> = conn
        .prepare("SELECT role, task_id, status FROM runs ORDER BY id ASC")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].0, "lead");
    assert_eq!(rows[1].0, "dev");
    assert_eq!(rows[2].0, "qa");

    let task_id = rows[0].1;
    assert!(task_id.is_some());
    assert_eq!(rows[1].1, task_id);
    assert_eq!(rows[2].1, task_id);

    assert_eq!(rows[0].2, "succeeded");
    assert_eq!(rows[1].2, "succeeded");
    assert_eq!(rows[2].2, "succeeded");
}
