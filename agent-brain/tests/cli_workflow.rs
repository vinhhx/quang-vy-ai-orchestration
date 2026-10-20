mod common;
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::workflows::Workflow;
use common::Sandbox;
use protocol::{lead_scenario, setup_project};

#[test]
fn snapshot_default_mac_dinh() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    entry.as_object_mut().unwrap().remove("workflow");
    entry["name_vi"] = serde_json::json!("Web bán hàng");
    sb.write_map(serde_json::json!([entry]), &[]);

    let scenario = lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let out = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let conn = sb.db();
    let wf_json: String = conn
        .query_row(
            "SELECT workflow_json FROM tasks WHERE slug = 'sua-app'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let wf = Workflow::from_snapshot(&wf_json).unwrap();
    assert_eq!(wf.name, "mac-dinh");
    assert_eq!(wf.source, "builtin");
    assert!(wf.auto);
    assert_eq!(wf.max_rounds, 3);
}

#[test]
fn snapshot_project_default() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work"); // project default is "nhanh"

    let scenario = lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let out = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let conn = sb.db();
    let wf_json: String = conn
        .query_row(
            "SELECT workflow_json FROM tasks WHERE slug = 'sua-app'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let wf = Workflow::from_snapshot(&wf_json).unwrap();
    assert_eq!(wf.name, "nhanh");
    assert!(!wf.auto);
}

#[test]
fn cli_flag_wins() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work"); // project default is "nhanh"

    let scenario = lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let out = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--workflow",
            "mac-dinh",
        ],
        Some(scenario),
    );
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let conn = sb.db();
    let wf_json: String = conn
        .query_row(
            "SELECT workflow_json FROM tasks WHERE slug = 'sua-app'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let wf = Workflow::from_snapshot(&wf_json).unwrap();
    assert_eq!(wf.name, "mac-dinh");
}

#[test]
fn unknown_flag_fails_before_git() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let scenario = lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let out = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--workflow",
            "khong-co",
        ],
        Some(scenario),
    );
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Không có quy trình 'khong-co'"));

    let branches = sb.git(&repo, &["branch", "--list", "task/sua-app"]);
    assert_eq!(branches.trim(), "");

    assert!(!repo.join(".plan.md").exists());

    let conn = sb.db();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn unknown_project_default_falls_back() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    entry["workflow"] = serde_json::json!("x");
    entry["name_vi"] = serde_json::json!("Web bán hàng");
    sb.write_map(serde_json::json!([entry]), &[]);

    let scenario = lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let out = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(out
        .stderr
        .contains("Quy trình 'x' của dự án không có trong workflows.json — dùng mac-dinh."));

    let conn = sb.db();
    let wf_json: String = conn
        .query_row(
            "SELECT workflow_json FROM tasks WHERE slug = 'sua-app'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let wf = Workflow::from_snapshot(&wf_json).unwrap();
    assert_eq!(wf.name, "mac-dinh");
}

#[test]
fn file_workflow_snapshot() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_workflows(serde_json::json!({
        "chi-phi-thap": {
            "stages": ["plan", "plan-review", "gate:you", "implement", "qa"],
            "budget_usd": 0.05,
            "auto": true
        }
    }));

    let scenario = lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let out = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--workflow",
            "chi-phi-thap",
        ],
        Some(scenario),
    );
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let conn = sb.db();
    let wf_json: String = conn
        .query_row(
            "SELECT workflow_json FROM tasks WHERE slug = 'sua-app'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let wf = Workflow::from_snapshot(&wf_json).unwrap();
    assert!((wf.budget_usd - 0.05).abs() < 1e-6);
    assert_eq!(wf.source, "file");
    assert_eq!(wf.loops, vec!["plan"]);
}

#[test]
fn invalid_file_keeps_builtins() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work"); // project default is "nhanh"
    let wf_path = sb.workflows_path();
    if let Some(p) = wf_path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(&wf_path, "{ broken").unwrap();

    let scenario = lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let out = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(out.stderr.contains("không phải JSON hợp lệ"));

    let conn = sb.db();
    let wf_json: String = conn
        .query_row(
            "SELECT workflow_json FROM tasks WHERE slug = 'sua-app'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let wf = Workflow::from_snapshot(&wf_json).unwrap();
    assert_eq!(wf.name, "nhanh");
}

#[test]
fn snapshot_survives_file_edit() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_workflows(serde_json::json!({
        "chi-phi-thap": {
            "stages": ["plan", "plan-review", "gate:you", "implement", "qa"],
            "budget_usd": 0.05,
            "auto": true
        }
    }));

    let scenario = lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let out = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--workflow",
            "chi-phi-thap",
        ],
        Some(scenario.clone()),
    );
    assert_eq!(out.code, 0);

    // Rewrite workflows.json with budget_usd 9
    sb.write_workflows(serde_json::json!({
        "chi-phi-thap": {
            "stages": ["plan", "plan-review", "gate:you", "implement", "qa"],
            "budget_usd": 9.0,
            "auto": true
        }
    }));

    // Re-lead the same task with --name sua-app --workflow nhanh
    let scenario2 = lead_scenario(&[("src/app.py", "1-5", "sửa app lần 2", "yêu cầu")]);
    let out2 = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
            "--workflow",
            "nhanh",
        ],
        Some(scenario2),
    );
    assert_eq!(
        out2.code, 0,
        "stderr: {}\nstdout: {}",
        out2.stderr, out2.stdout
    );
    assert!(out2
        .stderr
        .contains("Task đã có quy trình 'chi-phi-thap' — bỏ qua --workflow."));

    let conn = sb.db();
    let wf_json: String = conn
        .query_row(
            "SELECT workflow_json FROM tasks WHERE slug = 'sua-app'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let wf = Workflow::from_snapshot(&wf_json).unwrap();
    assert!((wf.budget_usd - 0.05).abs() < 1e-6);
    assert_eq!(wf.name, "chi-phi-thap");
}
