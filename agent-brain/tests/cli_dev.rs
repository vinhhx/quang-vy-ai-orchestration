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

// ---- skill mode (D-49): the Dev gets what the approved plan declares ----

const GRANTS_SECTIONS: &str = "## Skill sử dụng\n- `fake-idea` (.agents/skills/fake-idea/SKILL.md)\n## Lệnh được chạy\n- `python scripts/create.py *`\n## Thư mục được đọc\n- `data/**`\n## Thư mục được ghi\n- `out/**`\n";

fn setup_grants_task(sb: &Sandbox, extra_sections: Option<&str>) -> (PathBuf, String, String) {
    let repo = sb.make_repo(
        "web-ban-hang",
        &[
            ("src/app.py", "def app():\n    return 1\n"),
            ("scripts/create.py", "print('x')\n"),
            (
                ".agents/skills/fake-idea/SKILL.md",
                "---\nname: fake-idea\ndescription: giả\n---\n",
            ),
        ],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    entry["name_vi"] = serde_json::json!("Web bán hàng");
    entry["aliases"] = serde_json::json!(["shop"]);
    sb.write_map(serde_json::json!([entry]), &[]);
    let mut p_text = plan_text(
        "Kế hoạch",
        &[("src/app.py", "app()", "trả về 2", "yêu cầu")],
        "python3 -m unittest",
        "",
    );
    p_text.push_str(extra_sections.unwrap_or(GRANTS_SECTIONS));
    let scenario = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "write", "path": ".plan.md", "content": p_text, "keep_meta": true}
        ]
    });
    let out = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(scenario));
    assert_eq!(out.code, 0, "lead failed: {}\n{}", out.stderr, out.stdout);
    let plan_commit = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    (repo, "sua-app".to_string(), plan_commit)
}

fn last_argv(sb: &Sandbox) -> Vec<String> {
    sb.records().last().unwrap()["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect()
}

#[test]
fn dev_with_grants_gets_rules_and_may_write_under_approved_dir() {
    let sb = Sandbox::new();
    let (repo, _task, plan_commit) = setup_grants_task(&sb, None);
    let scenario = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": ".agents/skills/fake-idea/SKILL.md"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "write", "path": "out/video/a.txt", "content": "ok\n"},
            {"op": "write", "path": "out/.env", "content": "SECRET=1\n"}
        ]
    });
    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(scenario));
    // the guard blocked the `.env` write; the other files are not violations
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    let argv = last_argv(&sb);
    let repo_s = repo.to_string_lossy();
    for rule in [
        "Bash(python scripts/create.py *)".to_string(),
        "Edit(./out/**)".to_string(),
        "Write(./out/**)".to_string(),
        "Read(./out/**)".to_string(),
        "Read(./data/**)".to_string(),
        "Read(./.agents/skills/fake-idea/SKILL.md)".to_string(),
        format!("Edit(/{repo_s}/out/**)"),
        format!("Read(/{repo_s}/data/**)"),
        "Bash(git add *)".to_string(),
    ] {
        assert!(argv.contains(&rule), "missing {rule}");
    }
    assert!(!argv.contains(&"Edit(./data/**)".to_string()));
    let diff = sb.git(
        &repo,
        &["diff", "--name-only", &format!("{plan_commit}..HEAD")],
    );
    let mut names: Vec<&str> = diff.lines().collect();
    names.sort();
    assert_eq!(names, vec!["out/video/a.txt", "src/app.py"]);
    assert!(!repo.join("out/.env").exists());
    let rec = sb.records().pop().unwrap();
    assert!(rec["blocked"]
        .as_array()
        .unwrap()
        .iter()
        .any(|b| b["path"].as_str().unwrap_or("").ends_with("out/.env")));
    assert!(argv
        .iter()
        .any(|a| a.contains("Plan có khai báo Lệnh được chạy")));
}

#[test]
fn dev_with_grants_guard_blocks_outside_dirs() {
    let sb = Sandbox::new();
    let (repo, _task, _pc) = setup_grants_task(&sb, None);
    let scenario = serde_json::json!({
        "ops": [
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "write", "path": "other/x.txt", "content": "no\n"},
            {"op": "write", "path": "scripts/create.py", "content": "evil\n"},
            {"op": "read", "path": "src/other.py"}
        ]
    });
    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    let rec = sb.records().pop().unwrap();
    let blocked: Vec<String> = rec["blocked"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["path"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        blocked.iter().any(|p| p.ends_with("other/x.txt")),
        "{blocked:?}"
    );
    assert!(
        blocked.iter().any(|p| p.ends_with("scripts/create.py")),
        "{blocked:?}"
    );
    assert!(!repo.join("other/x.txt").exists());
}

#[test]
fn dev_with_grants_file_outside_dirs_and_plan_is_protocol_violation() {
    let sb = Sandbox::new();
    let (repo, _task, plan_commit) = setup_grants_task(&sb, None);
    let scenario = serde_json::json!({
        "ignore_hooks": true,
        "ops": [
            {"op": "write", "path": "out/ok.txt", "content": "ok\n"},
            {"op": "write", "path": "other/x.txt", "content": "no\n"}
        ]
    });
    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(scenario));
    assert_eq!(out.code, 3, "stdout: {}", out.stdout);
    assert!(out.stdout.contains("other/x.txt"));
    assert!(!out.stdout.contains("  out/ok.txt"));
    assert_eq!(sb.git(&repo, &["rev-parse", "HEAD"]).trim(), plan_commit);
}

#[test]
fn dev_with_grants_sensitive_and_planning_files_are_violations_even_in_approved_dir() {
    let sb = Sandbox::new();
    let (_repo, _task, _pc) = setup_grants_task(&sb, None);
    let scenario = serde_json::json!({
        "ignore_hooks": true,
        "ops": [
            {"op": "write", "path": "out/.env", "content": "S=1\n"},
            {"op": "write", "path": "out/.planning/x.md", "content": "x\n"},
            {"op": "write", "path": "out/fine.txt", "content": "ok\n"}
        ]
    });
    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(scenario));
    assert_eq!(out.code, 3, "stdout: {}", out.stdout);
    assert!(out.stdout.contains("out/.env"));
    assert!(out.stdout.contains("out/.planning/x.md"));
    assert!(!out.stdout.contains("  out/fine.txt"));
}

#[test]
fn plan_widened_after_approval_does_not_widen_the_grants() {
    let sb = Sandbox::new();
    let (repo, task, plan_commit) = setup_grants_task(&sb, None);
    // the user approved plan_commit
    let conn = sb.db();
    conn.execute(
        "UPDATE tasks SET approved_at = 1, approved_commit = ?1",
        rusqlite::params![plan_commit],
    )
    .unwrap();
    drop(conn);
    // a later commit declares more
    let text = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    let widened = text
        .replace("- `out/**`", "- `out/**`\n- `extra/**`")
        .replace(
            "- `python scripts/create.py *`",
            "- `python scripts/create.py *`\n- `python scripts/more.py *`",
        );
    assert_ne!(text, widened);
    std::fs::write(repo.join(".plan.md"), widened).unwrap();
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", &format!("{task}: plan")]);

    let scenario = serde_json::json!({ "ops": [
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"}
    ]});
    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    let argv = last_argv(&sb);
    assert!(argv.contains(&"Bash(python scripts/create.py *)".to_string()));
    assert!(!argv.contains(&"Bash(python scripts/more.py *)".to_string()));
    assert!(!argv.contains(&"Edit(./extra/**)".to_string()));
    assert!(argv.contains(&"Edit(./out/**)".to_string()));
}

#[test]
fn dev_without_grant_sections_is_unchanged() {
    let sb = Sandbox::new();
    // a project that has skills but a plan that declares nothing
    let (_repo, _task, _pc) = setup_grants_task(&sb, Some(""));
    let scenario = serde_json::json!({ "ops": [
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"}
    ]});
    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    let argv = last_argv(&sb);
    assert!(!argv.iter().any(|a| a.starts_with("Bash(python")));
    assert!(!argv.iter().any(|a| a.contains("out/**")));
    assert!(!argv.iter().any(|a| a.contains("SKILL.md")));
    assert!(!argv
        .iter()
        .any(|a| a.contains("Plan có khai báo Lệnh được chạy")));
    // the policy file keeps the old shape
    let settings_idx = argv.iter().position(|x| x == "--settings").unwrap();
    let settings = PathBuf::from(&argv[settings_idx + 1]);
    let policy: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(settings.parent().unwrap().join("policy.json")).unwrap(),
    )
    .unwrap();
    assert!(policy.get("bash_allow").is_none());
    assert!(policy.get("write_dirs").is_none());
}

#[test]
fn dev_with_grants_policy_file_carries_dirs_and_commands() {
    let sb = Sandbox::new();
    let (_repo, _task, _pc) = setup_grants_task(&sb, None);
    let scenario = serde_json::json!({ "ops": [
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"}
    ]});
    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    let argv = last_argv(&sb);
    let settings_idx = argv.iter().position(|x| x == "--settings").unwrap();
    let settings = PathBuf::from(&argv[settings_idx + 1]);
    let policy: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(settings.parent().unwrap().join("policy.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(policy["bash_allow"][0], "python scripts/create.py *");
    assert!(policy["write_dirs"][0].as_str().unwrap().ends_with("/out"));
    assert!(policy["read_dirs"][0].as_str().unwrap().ends_with("/data"));
    let read_allow = policy["read_allow"].as_array().unwrap();
    assert!(read_allow.iter().any(|p| p
        .as_str()
        .unwrap()
        .ends_with(".agents/skills/fake-idea/SKILL.md")));
}

#[test]
fn dev_dry_run_with_grants_shows_rules() {
    let sb = Sandbox::new();
    let (_repo, _task, _pc) = setup_grants_task(&sb, None);
    let out = sb.run(&["run", "dev", "web-ban-hang", "--dry-run"], None);
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stdout.contains("Bash(python scripts/create.py *)"),
        "{}",
        out.stdout
    );
}

#[test]
fn dev_with_invalid_grants_in_approved_plan_is_refused() {
    let sb = Sandbox::new();
    let (repo, task, plan_commit) = setup_grants_task(&sb, None);
    let conn = sb.db();
    conn.execute(
        "UPDATE tasks SET approved_at = 1, approved_commit = ?1",
        rusqlite::params![plan_commit],
    )
    .unwrap();
    drop(conn);
    // a bad grant committed afterwards: the working plan is refused by validation too
    let text = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    std::fs::write(
        repo.join(".plan.md"),
        text.replace("python scripts/create.py *", "sudo ls"),
    )
    .unwrap();
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", &format!("{task}: plan")]);
    let out = sb.run(&["run", "dev", "web-ban-hang"], None);
    assert_ne!(out.code, 0);
    assert!(format!("{}{}", out.stdout, out.stderr).contains("bị từ chối"));
}

#[test]
fn agy_dev_ignores_grants_and_says_so() {
    let sb = Sandbox::new();
    let fake_agy = common::repo_root()
        .join("tests")
        .join("fakes")
        .join("fake_agy.py")
        .canonicalize()
        .unwrap();
    sb.write_config(
        serde_json::json!({"engines": {"agy": {"command": fake_agy.to_str().unwrap()}}}),
    );
    sb.write_teams(
        serde_json::json!({"doi-agy": {"description": "Dev bằng agy", "roles": {"dev": "agy"}}}),
    );
    let repo = sb.make_repo(
        "web",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web", "work");
    entry.as_object_mut().unwrap().remove("workflow");
    sb.write_map(serde_json::json!([entry]), &[]);
    let mut p_text = plan_text(
        "Kế hoạch",
        &[("src/app.py", "app()", "trả về 2", "yêu cầu")],
        "python3 -m unittest",
        "",
    );
    p_text.push_str(GRANTS_SECTIONS);
    let lead = serde_json::json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "write", "path": ".plan.md", "content": p_text, "keep_meta": true}
    ]});
    let out = sb.run(
        &[
            "run",
            "lead",
            "--team",
            "doi-agy",
            "--name",
            "sua-app",
            "--",
            "web",
            "sửa app",
        ],
        Some(lead),
    );
    assert_eq!(out.code, 0, "lead: {}\n{}", out.stderr, out.stdout);

    let scenario_path = sb.root.join("fake_agy_scenario.json");
    let record_path = sb.root.join("fake_agy_record.jsonl");
    let usage = serde_json::json!({"input_tokens": 1, "output_tokens": 1, "total_tokens": 2});
    std::fs::write(
        &scenario_path,
        serde_json::to_string(&serde_json::json!({
            "ops": [
                {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
                {"op": "write", "path": "out/x.txt", "content": "no\n"}
            ],
            "lines": [
                {"event": "init", "conversation_id": "c1", "init": {"cwd": "/x", "tools": [], "permission_mode": "accept-edits"}},
                {"event": "result", "result": {"conversation_id": "c1", "status": "SUCCESS", "response": "ok", "num_turns": 1, "usage": usage}}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let out = sb.run_env(
        &["run", "dev", "web"],
        None,
        &[
            ("FAKE_AGY_SCENARIO", scenario_path.to_str().unwrap()),
            ("FAKE_AGY_RECORD", record_path.to_str().unwrap()),
        ],
    );
    assert!(
        out.stdout
            .contains("Quyền chạy lệnh của plan chỉ áp dụng cho engine claude."),
        "{}",
        out.stdout
    );
    // run as before: a file under the declared dir is outside the plan for agy
    assert_eq!(
        out.code, 3,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(out.stdout.contains("out/x.txt"));
    let rec = std::fs::read_to_string(&record_path).unwrap();
    assert!(!rec.contains("Bash("), "{rec}");
    assert!(!rec.contains("create.py"), "{rec}");
}
