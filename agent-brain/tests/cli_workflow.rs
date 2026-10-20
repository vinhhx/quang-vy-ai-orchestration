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

#[test]
fn review_replan_edits_in_place() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    let sc_lead = lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let out_lead = sb.run(
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
    assert_eq!(out_lead.code, 0);
    let lead_sha = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();

    let review_report = "\
---
verdict: revise
blocking: 1
advisory: 1
files: []
---
# Review plan
## Mục chặn (blocking)
- thiếu kiểm thử
## Góp ý (advisory)
";
    let sc_rev = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan-review.md",
                "content": review_report,
            }
        ]
    });
    let out_rev = sb.run(
        &["run", "plan-review", "web-ban-hang", "--round", "1"],
        Some(sc_rev),
    );
    assert_eq!(out_rev.code, 0);
    let rev_sha = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();

    let sc_relead = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan.md",
                "content": "\n<!-- sửa theo review -->\n",
                "append": true
            }
        ]
    });
    let out_relead = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
            "--review",
            "--round",
            "2",
        ],
        Some(sc_relead),
    );
    assert_eq!(
        out_relead.code, 0,
        "stderr: {}\nstdout: {}",
        out_relead.stderr, out_relead.stdout
    );

    let new_head = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    assert_ne!(new_head, rev_sha);
    assert_ne!(new_head, lead_sha);

    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), "sua-app: plan");

    let plan_content = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    assert!(plan_content.contains("src/app.py"));
    assert!(
        plan_content.ends_with("<!-- sửa theo review -->\n")
            || plan_content
                .trim_end()
                .ends_with("<!-- sửa theo review -->")
    );

    let recs = sb.records();
    let last_rec = recs.last().unwrap();
    let argv: Vec<String> = last_rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let p_idx = argv.iter().position(|x| x == "-p").unwrap();
    let p_msg = &argv[p_idx + 1];
    assert!(p_msg.contains(".plan-review.md"));
    assert!(p_msg.contains("mục chặn"));

    let conn = sb.db();
    let (stage, round): (Option<String>, Option<i64>) = conn
        .query_row(
            "SELECT stage, round FROM runs WHERE task_id = 1 ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(stage.as_deref(), Some("plan"));
    assert_eq!(round, Some(2));
}

#[test]
fn review_requires_revise_report() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    // On main branch (not task branch)
    let out = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
            "--review",
        ],
        None,
    );
    assert_eq!(out.code, 1);
    assert!(out
        .stderr
        .contains("--review chỉ dùng khi đang ở nhánh task/sua-app"));

    // Now lead on task branch
    let sc_lead = lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let out_lead = sb.run(
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
    assert_eq!(out_lead.code, 0);

    // On task branch, but no .plan-review.md
    let out2 = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
            "--review",
        ],
        None,
    );
    assert_eq!(out2.code, 1);
    assert!(out2
        .stderr
        .contains("--review cần .plan-review.md có verdict revise"));

    // Write an approve report
    let approve_report =
        "---\nverdict: approve\nblocking: 0\nadvisory: 0\nfiles: []\n---\n# Review plan\n";
    std::fs::write(repo.join(".plan-review.md"), approve_report).unwrap();
    let out3 = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
            "--review",
        ],
        None,
    );
    assert_eq!(out3.code, 1);
    assert!(out3
        .stderr
        .contains("--review cần .plan-review.md có verdict revise"));
}

#[test]
fn note_stored_and_passed() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    let sc_lead = lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let out_lead = sb.run(
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
    assert_eq!(out_lead.code, 0);

    let sc_relead = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan.md",
                "content": "\n<!-- note update -->\n",
                "append": true
            }
        ]
    });
    let out = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
            "--note=-tách bước 2",
        ],
        Some(sc_relead),
    );
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let conn = sb.db();
    let metrics_json: Option<String> = conn
        .query_row(
            "SELECT metrics_json FROM runs WHERE task_id = 1 ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let metrics: serde_json::Value =
        serde_json::from_str(&metrics_json.expect("metrics_json present")).unwrap();
    assert_eq!(metrics["note"], "-tách bước 2");

    let recs = sb.records();
    let last_rec = recs.last().unwrap();
    let argv: Vec<String> = last_rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let p_idx = argv.iter().position(|x| x == "-p").unwrap();
    let p_msg = &argv[p_idx + 1];
    assert!(p_msg.contains("Ghi chú của người dùng: -tách bước 2"));

    let plan_content = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    assert!(plan_content.contains("src/app.py"));
}

#[test]
fn note_limits() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    // 501 characters note
    let long_note = "a".repeat(501);
    let out = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            &format!("--note={long_note}"),
        ],
        None,
    );
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Ghi chú quá dài (tối đa 500 ký tự)"));

    let conn = sb.db();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);

    // Note of only spaces
    let scenario = lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let out2 = sb.run(
        &["run", "lead", "web-ban-hang", "sửa app", "--note=   "],
        Some(scenario),
    );
    assert_eq!(
        out2.code, 0,
        "stderr: {}\nstdout: {}",
        out2.stderr, out2.stdout
    );
    let metrics_json: Option<String> = conn
        .query_row("SELECT metrics_json FROM runs WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(metrics_json.is_none());
}

#[test]
fn plain_relead_uses_skeleton() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    let sc1 = lead_scenario(&[("src/app.py", "1-5", "sửa app", "bước 1")]);
    let out1 = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
        ],
        Some(sc1),
    );
    assert_eq!(out1.code, 0);

    let sc2 = lead_scenario(&[("src/app.py", "1-10", "sửa app lần 2", "bước 2")]);
    let out2 = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
        ],
        Some(sc2),
    );
    assert_eq!(out2.code, 0);

    let plan_content = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    assert!(plan_content.contains("bước 2"));
    assert!(!plan_content.contains("bước 1"));
}

#[test]
fn approval_cleared_after_lead() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let sc1 = lead_scenario(&[("src/app.py", "1-5", "sửa app", "bước 1")]);
    let out1 = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
        ],
        Some(sc1),
    );
    assert_eq!(out1.code, 0);

    let conn = sb.db();
    agent_brain::db::set_task_approval(&conn, 1, 1234567890, "commit-sha").unwrap();
    let auto_before = agent_brain::db::task_auto(&conn, 1).unwrap().unwrap();
    assert!(auto_before.approved_at.is_some());
    assert!(auto_before.approved_commit.is_some());

    let sc2 = lead_scenario(&[("src/app.py", "1-5", "sửa app", "bước 2")]);
    let out2 = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
        ],
        Some(sc2),
    );
    assert_eq!(out2.code, 0);

    let auto_after = agent_brain::db::task_auto(&conn, 1).unwrap().unwrap();
    assert!(auto_after.approved_at.is_none());
    assert!(auto_after.approved_commit.is_none());
}

#[test]
fn lead_unchanged_plan_no_commit() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    let sc1 = lead_scenario(&[("src/app.py", "1-5", "sửa app", "bước 1")]);
    let out1 = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
        ],
        Some(sc1),
    );
    assert_eq!(out1.code, 0);
    let head1 = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();

    let conn = sb.db();
    agent_brain::db::set_task_approval(&conn, 1, 9999999, &head1).unwrap();

    // Scenario that does NOT write .plan.md
    let sc_noop = serde_json::json!({
        "ops": [
            { "op": "read", "path": ".plan.md" }
        ]
    });
    let out2 = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
            "--note=xem lại thôi",
        ],
        Some(sc_noop),
    );
    assert_eq!(out2.code, 0);
    assert!(out2
        .stdout
        .contains("Lead không sửa .plan.md — giữ nguyên commit plan hiện tại."));

    let head2 = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    assert_eq!(head1, head2);

    let auto_after = agent_brain::db::task_auto(&conn, 1).unwrap().unwrap();
    assert_eq!(auto_after.approved_at, Some(9999999));
    assert_eq!(auto_after.approved_commit.as_deref(), Some(head1.as_str()));
}

#[test]
fn flags_only_for_lead() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let out_dev = sb.run(&["run", "dev", "web-ban-hang", "--review"], None);
    assert_eq!(out_dev.code, 1);
    assert!(out_dev
        .stderr
        .contains("--review, --note và --workflow chỉ dùng với vai trò lead"));

    let out_qa = sb.run(&["run", "qa", "web-ban-hang", "--workflow", "nhanh"], None);
    assert_eq!(out_qa.code, 1);
    assert!(out_qa
        .stderr
        .contains("--review, --note và --workflow chỉ dùng với vai trò lead"));
}
