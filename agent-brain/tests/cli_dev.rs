mod common;
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::{lead_scenario, plan_text, setup_project};
use std::path::PathBuf;

fn setup_lead_task(sb: &Sandbox) -> (PathBuf, String, String) {
    let repo = setup_project(sb, "work");
    let sc_lead = lead_scenario(&[("src/app.py", "app()", "trả về 2", "yêu cầu")]);
    let out_lead = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(sc_lead));
    assert_eq!(
        out_lead.code, 0,
        "lead failed: stderr: {}\nstdout: {}",
        out_lead.stderr, out_lead.stdout
    );
    let task = "sua-app".to_string();
    let plan_commit = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    (repo, task, plan_commit)
}

#[test]
fn p1_07_dev_edits_only_plan_files() {
    let sb = Sandbox::new();
    let (repo, task, plan_commit) = setup_lead_task(&sb);

    let dev_scenario = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", format!("{task}: sửa app")]}
        ]
    });

    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(dev_scenario));
    assert_eq!(
        out.code, 0,
        "dev failed: stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let diff_tree = sb.git(
        &repo,
        &["diff", "--name-only", &format!("{plan_commit}..HEAD")],
    );
    assert_eq!(diff_tree.trim(), "src/app.py");

    let recs = sb.records();
    assert!(recs.len() >= 2);
    let dev_rec = &recs[recs.len() - 1];
    let argv: Vec<String> = dev_rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    assert!(argv.contains(&"Read(./src/app.py)".to_string()));
    assert!(argv.contains(&"Edit(./src/app.py)".to_string()));
    assert!(argv.contains(&"Write(./src/app.py)".to_string()));
    assert!(argv.contains(&"Read(./.plan.md)".to_string()));
    assert!(argv.contains(&"Edit(./.plan.md)".to_string()));
    assert!(argv.contains(&"Write(./.plan.md)".to_string()));

    let repo_str = repo.to_string_lossy();
    assert!(argv.contains(&format!("Read(/{repo_str}/src/app.py)")));
    assert!(argv.contains(&format!("Edit(/{repo_str}/src/app.py)")));
    assert!(argv.contains(&format!("Write(/{repo_str}/src/app.py)")));
    assert!(argv.contains(&format!("Read(/{repo_str}/.plan.md)")));
    assert!(argv.contains(&format!("Edit(/{repo_str}/.plan.md)")));
    assert!(argv.contains(&format!("Write(/{repo_str}/.plan.md)")));

    assert!(argv.contains(&"Bash(git add *)".to_string()));
    assert!(argv.contains(&"Bash(git commit *)".to_string()));

    let disallowed_idx = argv.iter().position(|x| x == "--disallowedTools").unwrap();
    let disallowed_slice = &argv[disallowed_idx + 1..];
    assert!(disallowed_slice.iter().any(|x| x == "Grep"));
    assert!(disallowed_slice.iter().any(|x| x == "Glob"));

    let settings_idx = argv.iter().position(|x| x == "--settings").unwrap();
    let settings_path = PathBuf::from(&argv[settings_idx + 1]);
    assert!(settings_path.is_file());
    assert_eq!(
        settings_path.file_name().unwrap().to_string_lossy(),
        "settings.json"
    );

    let settings_content = std::fs::read_to_string(&settings_path).unwrap();
    let settings_json: serde_json::Value = serde_json::from_str(&settings_content).unwrap();
    let cmd = settings_json["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    let exe = std::env::current_exe().unwrap();
    assert!(cmd.contains("guard --policy"));
    assert!(
        cmd.starts_with('\'')
            || cmd.contains(&exe.file_name().unwrap().to_string_lossy().to_string())
    );

    let model_idx = argv.iter().position(|x| x == "--model").unwrap();
    assert_eq!(argv[model_idx + 1], "sonnet");
}

#[test]
fn dev_commits_pending_change() {
    let sb = Sandbox::new();
    let (repo, task, plan_commit) = setup_lead_task(&sb);

    let dev_scenario = serde_json::json!({
        "ops": [
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"}
        ]
    });

    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(dev_scenario));
    assert_eq!(
        out.code, 0,
        "dev failed: stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert!(
        head_subj.trim().starts_with(&format!("{task}: ")),
        "unexpected head subject: {head_subj}"
    );

    let diff_tree = sb.git(
        &repo,
        &["diff", "--name-only", &format!("{plan_commit}..HEAD")],
    );
    assert_eq!(diff_tree.trim(), "src/app.py");
}

#[test]
fn dev_guard_blocks_read_outside_plan() {
    let sb = Sandbox::new();
    let (repo, task, _plan_commit) = setup_lead_task(&sb);

    std::fs::write(repo.join("other.py"), "# other\n").unwrap();
    sb.git(&repo, &["add", "other.py"]);
    sb.git(&repo, &["commit", "-m", &format!("{task}: add other")]);

    let dev_scenario = serde_json::json!({
        "ops": [
            {"op": "read", "path": "src/app.py"},
            {"op": "read", "path": "other.py"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 3\n"}
        ]
    });

    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(dev_scenario));
    assert_eq!(
        out.code, 0,
        "dev failed: stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let dev_rec = sb.records().pop().unwrap();
    let blocked = dev_rec["blocked"].as_array().expect("blocked array");
    assert!(
        blocked.iter().any(|b| b["path"]
            .as_str()
            .map(|p| p.contains("other.py"))
            .unwrap_or(false)),
        "blocked missing other.py: {:?}",
        blocked
    );

    let mut found_guard_log = false;
    for entry in walkdir(&sb.runs) {
        if entry.file_name().and_then(|n| n.to_str()) == Some("guard.jsonl") {
            let content = std::fs::read_to_string(&entry).unwrap();
            for line in content.lines() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                    if v["decision"] == "block"
                        && v["path"]
                            .as_str()
                            .map(|p| p.contains("other.py"))
                            .unwrap_or(false)
                    {
                        found_guard_log = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        found_guard_log,
        "did not find guard.jsonl with block decision for other.py"
    );
}

fn walkdir(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walkdir(&path));
            } else {
                files.push(path);
            }
        }
    }
    files
}

#[test]
fn dev_backstop_exit_3_when_hooks_ignored() {
    let sb = Sandbox::new();
    let (repo, _task, plan_commit) = setup_lead_task(&sb);

    let dev_scenario = serde_json::json!({
        "ignore_hooks": true,
        "ops": [
            {"op": "write", "path": "outside.py", "content": "# forbidden\n"}
        ]
    });

    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(dev_scenario));
    assert_eq!(out.code, 3);
    assert!(out
        .stdout
        .contains("Dev đã sửa file ngoài plan — vi phạm giao thức (ROLE-06):"));
    assert!(out.stdout.contains("outside.py"));
    assert!(out.stdout.contains("agent-brain không tự hoàn tác"));

    let curr_head = sb.git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(curr_head.trim(), plan_commit);
    assert!(repo.join("outside.py").is_file());

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
fn p1_08_dev_blocked_writes_vuong_mac() {
    let sb = Sandbox::new();
    let (repo, task, plan_commit) = setup_lead_task(&sb);

    let dev_scenario = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan.md",
                "content": "\nThiếu tài liệu API\n",
                "append": true
            }
        ]
    });

    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(dev_scenario));
    assert_eq!(
        out.code, 4,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: vướng mắc"));

    let diff_tree = sb.git(
        &repo,
        &["diff", "--name-only", &format!("{plan_commit}..HEAD")],
    );
    assert_eq!(diff_tree.trim(), ".plan.md");

    assert!(out.stdout.contains("Thiếu tài liệu API"));
    assert!(out.stdout.contains("agent-brain run lead"));

    let conn = sb.db();
    let outcome: String = conn
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'dev' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome, "dev:blocked");

    let status: String = conn
        .query_row(
            "SELECT status FROM tasks WHERE slug = ?1",
            rusqlite::params![task],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "blocked");
}

#[test]
fn dev_refusals_exit_1_without_claude() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    // 1. Not on task branch (on main)
    let out1 = sb.run(&["run", "dev", "web-ban-hang"], None);
    assert_eq!(out1.code, 1);
    assert!(sb.records().is_empty());

    // Setup lead
    let sc_lead = lead_scenario(&[("src/app.py", "app()", "trả về 2", "yêu cầu")]);
    let out_lead = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(sc_lead));
    assert_eq!(out_lead.code, 0);
    let task = "sua-app";
    let records_after_lead = sb.records().len();

    // 2. Dirty tree
    std::fs::write(repo.join("dirty.txt"), "dirty\n").unwrap();
    let out_dirty = sb.run(&["run", "dev", "web-ban-hang"], None);
    assert_eq!(out_dirty.code, 1);
    assert!(out_dirty.stderr.contains("chưa commit"));
    assert_eq!(sb.records().len(), records_after_lead);
    std::fs::remove_file(repo.join("dirty.txt")).unwrap();

    // 3. Plan with zero rows
    let empty_plan = plan_text("Trống", &[], "python3 -m unittest", "");
    let old_text = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    let meta = agent_brain::planfile::read_meta(&old_text).unwrap();
    let set_text_empty = agent_brain::planfile::set_meta(&empty_plan, &meta);
    std::fs::write(repo.join(".plan.md"), set_text_empty).unwrap();
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", &format!("{task}: plan")]);

    let out_empty = sb.run(&["run", "dev", "web-ban-hang"], None);
    assert_eq!(out_empty.code, 1);
    assert_eq!(sb.records().len(), records_after_lead);

    // 4. Plan row with parent traversal
    let traversal_plan = plan_text(
        "Sửa traversal",
        &[("../outside.py", "1", "sửa", "hack")],
        "python3 -m unittest",
        "",
    );
    let old_text = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    let meta = agent_brain::planfile::read_meta(&old_text).unwrap();
    let set_text = agent_brain::planfile::set_meta(&traversal_plan, &meta);
    std::fs::write(repo.join(".plan.md"), set_text).unwrap();
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", &format!("{task}: plan")]);

    let out_trav = sb.run(&["run", "dev", "web-ban-hang"], None);
    assert_eq!(out_trav.code, 1);
    assert_eq!(sb.records().len(), records_after_lead);

    // 5. Unresolved vướng mắc before the run
    let vm_plan = plan_text(
        "Sửa vướng mắc",
        &[("src/app.py", "1", "sửa", "yêu cầu")],
        "python3 -m unittest",
        "Chưa có hướng dẫn API",
    );
    let set_text_vm = agent_brain::planfile::set_meta(&vm_plan, &meta);
    std::fs::write(repo.join(".plan.md"), set_text_vm).unwrap();
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", &format!("{task}: plan")]);

    let out_vm = sb.run(&["run", "dev", "web-ban-hang"], None);
    assert_eq!(out_vm.code, 1);
    assert!(out_vm.stderr.contains("Vướng mắc"));
    assert_eq!(sb.records().len(), records_after_lead);
}

#[test]
fn dev_timeout_makes_no_commit() {
    let sb = Sandbox::new();
    let (repo, _task, plan_commit) = setup_lead_task(&sb);

    sb.write_config(serde_json::json!({
        "timeout_seconds": { "lead": 30, "dev": 1, "qa": 30 }
    }));

    let dev_scenario = serde_json::json!({
        "sleep": 5,
        "ops": [
            {"op": "write", "path": "src/app.py", "content": "def app(): return 9\n"}
        ]
    });

    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(dev_scenario));
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Dev chạy quá thời gian (timeout)"));

    let curr_head = sb.git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(curr_head.trim(), plan_commit);
}

#[test]
fn dev_never_runs_plan_commands() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let check_cmd = "touch KHONG_DUOC_TAO";
    let p_text = plan_text(
        "Kế hoạch",
        &[("src/app.py", "1", "sửa", "yêu cầu")],
        check_cmd,
        "",
    );
    let sc_lead = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "write", "path": ".plan.md", "content": p_text, "keep_meta": true}
        ]
    });
    let out_lead = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(sc_lead));
    assert_eq!(out_lead.code, 0);

    let dev_scenario = serde_json::json!({
        "ops": [
            {"op": "write", "path": "src/app.py", "content": "def app(): return 2\n"}
        ]
    });

    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(dev_scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(!repo.join("KHONG_DUOC_TAO").exists());
    assert!(out.stdout.contains(check_cmd));
    assert!(out.stdout.contains("agent-brain không tự chạy"));
}

#[test]
fn dev_warns_bad_commit_subject() {
    let sb = Sandbox::new();
    let (_repo, _task, _plan_commit) = setup_lead_task(&sb);

    let dev_scenario = serde_json::json!({
        "ops": [
            {"op": "write", "path": "src/app.py", "content": "def app(): return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "random"]}
        ]
    });

    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(dev_scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(out.stderr.contains("CẢNH BÁO: Commit"));
    assert!(out.stderr.contains("random"));
}

#[test]
fn dev_input_never_mentions_qa_report() {
    let sb = Sandbox::new();
    let (_repo, _task, _plan_commit) = setup_lead_task(&sb);

    let dev_scenario = serde_json::json!({
        "ops": [
            {"op": "write", "path": "src/app.py", "content": "def app(): return 2\n"}
        ]
    });

    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(dev_scenario));
    assert_eq!(out.code, 0);

    let dev_rec = sb.records().pop().unwrap();
    let argv_str = dev_rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(!argv_str.contains("qa-report"));

    let dev_prompt = agent_brain::roles::DEV_PROMPT;
    assert!(!dev_prompt.contains("qa-report"));
}

#[test]
fn dev_records_kpi() {
    let sb = Sandbox::new();
    let (_repo, task, _plan_commit) = setup_lead_task(&sb);

    let dev_scenario = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", format!("{task}: sửa app")]}
        ]
    });

    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(dev_scenario));
    assert_eq!(out.code, 0);

    let conn = sb.db();
    let (turns_before_first_edit, reads_outside_plan): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT turns_before_first_edit, reads_outside_plan FROM runs WHERE role = 'dev' ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    assert!(turns_before_first_edit.is_some());
    assert_eq!(reads_outside_plan, Some(0));
}

#[test]
fn dev_adopts_python_task_branch() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    // Create branch task/python-task manually
    sb.git(&repo, &["checkout", "-b", "task/python-task"]);

    let meta = serde_json::json!({
        "task": "python-task",
        "branch": "task/python-task",
        "request": "thực hiện python task",
        "base_branch": "main",
        "base_commit": sb.git(&repo, &["rev-parse", "HEAD"]).trim()
    });
    let p_text = plan_text(
        "Kế hoạch python",
        &[("src/app.py", "1", "sửa", "yêu cầu")],
        "python3 -m unittest",
        "",
    );
    let full_plan = agent_brain::planfile::set_meta(&p_text, &meta);
    std::fs::write(repo.join(".plan.md"), full_plan).unwrap();
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", "python-task: plan"]);

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let count_before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE slug = 'python-task'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count_before, 0);

    let dev_scenario = serde_json::json!({
        "ops": [
            {"op": "write", "path": "src/app.py", "content": "def app(): return 2\n"}
        ]
    });

    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(dev_scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let count_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE slug = 'python-task'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count_after, 1);
}

#[test]
fn run_dev_ignores_request_with_note() {
    let sb = Sandbox::new();
    let (_repo, _task, _plan_commit) = setup_lead_task(&sb);

    let dev_scenario = serde_json::json!({
        "ops": [
            {"op": "write", "path": "src/app.py", "content": "def app(): return 2\n"}
        ]
    });

    let out = sb.run(
        &["run", "dev", "web-ban-hang", "làm thêm việc khác"],
        Some(dev_scenario),
    );
    assert_eq!(out.code, 0);
    assert!(out.stderr.contains("Bỏ qua <yêu cầu> cho vai trò dev"));

    let dev_rec = sb.records().pop().unwrap();
    let argv = dev_rec["argv"].as_array().unwrap();
    let p_idx = argv.iter().position(|x| x == "-p").unwrap();
    let user_input = argv[p_idx + 1].as_str().unwrap();
    assert!(!user_input.contains("làm thêm việc khác"));
}
