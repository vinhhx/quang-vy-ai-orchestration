mod common;

use common::Sandbox;

#[test]
fn p2_06_status_zero_token() {
    let sb = Sandbox::new();
    sb.write_config(serde_json::json!({
        "claude_bin": "/nonexistent/claude"
    }));
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );

    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    entry["name_vi"] = serde_json::json!("Web bán hàng");
    entry["aliases"] = serde_json::json!(["shop"]);
    sb.write_map(serde_json::json!([entry]), &[]);

    // Modify a tracked file without committing
    std::fs::write(repo.join("src/app.py"), "def app():\n    return 2\n").expect("modify file");

    let runs_before = if sb.db_path.exists() {
        agent_brain::db::count_runs(&sb.db()).expect("count runs before")
    } else {
        0
    };
    assert_eq!(runs_before, 0);

    let out = sb.run(&["status", "web bán hàng"], None);

    assert_eq!(
        out.code, 0,
        "status failed with code {}: {}\nstdout: {}",
        out.code, out.stderr, out.stdout
    );
    assert!(
        out.stdout.contains("Nhánh: main"),
        "stdout missing branch: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("== git status --short =="),
        "stdout missing git status: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("src/app.py"),
        "stdout missing modified file: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("== git log --oneline -5 =="),
        "stdout missing git log: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("init"),
        "stdout missing init commit: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("Không ở nhánh task"),
        "stdout missing not on task branch notice: {}",
        out.stdout
    );

    let runs_after = agent_brain::db::count_runs(&sb.db()).expect("count runs after");
    assert_eq!(runs_after, 0);
    assert!(sb.records().is_empty(), "fake record file should be empty");
}

#[test]
fn status_on_task_branch_shows_diff_and_latest_run() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );

    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    entry["name_vi"] = serde_json::json!("Web bán hàng");
    sb.write_map(serde_json::json!([entry]), &[]);

    let scenario = serde_json::json!({
        "ops": [
            {
                "op": "read",
                "path": "src/app.py"
            },
            {
                "op": "write",
                "path": ".plan.md",
                "keep_meta": true,
                "content": "# Plan: thêm nút đăng xuất\n\n## Mục tiêu\nThêm nút đăng xuất\n\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | toàn bộ | thêm nút | yêu cầu |\n\n## Không được đụng tới\n\n## Cách kiểm tra\npython3 -m unittest\n\n## Vướng mắc\n\n"
            }
        ]
    });

    let run_out = sb.run(
        &["run", "lead", "web-ban-hang", "thêm nút đăng xuất"],
        Some(scenario),
    );
    assert_eq!(run_out.code, 0, "run lead failed: {}", run_out.stderr);

    // Edit and commit a file on the task branch
    std::fs::write(repo.join("src/app.py"), "def app():\n    return 99\n").expect("write file");
    sb.git(&repo, &["add", "src/app.py"]);
    sb.git(&repo, &["commit", "-m", "fix app"]);

    let runs_before = agent_brain::db::count_runs(&sb.db()).expect("count runs before");
    assert_eq!(runs_before, 1);

    let out = sb.run(&["status", "web-ban-hang"], None);
    assert_eq!(
        out.code, 0,
        "status failed with code {}: {}\nstdout: {}",
        out.code, out.stderr, out.stdout
    );

    assert!(
        out.stdout.contains("== git diff --stat "),
        "stdout missing diff --stat: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("src/app.py"),
        "stdout diff missing file: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("Task 1 · task/"),
        "stdout missing Task line: {}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("Run gần nhất: run 1 · lead · succeeded"),
        "stdout missing latest run: {}",
        out.stdout
    );

    let runs_after = agent_brain::db::count_runs(&sb.db()).expect("count runs after");
    assert_eq!(runs_after, 1);
}

#[test]
fn status_adopts_python_task_branch() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );

    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    let head_sha = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();

    sb.git(&repo, &["checkout", "-b", "task/py-task"]);
    let plan_content = format!(
        "# Plan\n<!-- brain-meta: {{\"base_branch\": \"main\", \"base_commit\": \"{head_sha}\", \"branch\": \"task/py-task\", \"request\": \"việc cũ\", \"task\": \"py-task\"}} -->\n\n## Mục tiêu\nviệc cũ\n"
    );
    std::fs::write(repo.join(".plan.md"), plan_content).expect("write .plan.md");
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", "add plan"]);

    let runs_before = if sb.db_path.exists() {
        agent_brain::db::count_runs(&sb.db()).expect("count runs before")
    } else {
        0
    };
    assert_eq!(runs_before, 0);

    let out = sb.run(&["status", "web-ban-hang"], None);
    assert_eq!(
        out.code, 0,
        "status failed: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    assert!(
        out.stdout.contains("Task "),
        "stdout missing Task line: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("task/py-task"),
        "stdout missing task branch in Task line: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("Run gần nhất: chưa có"),
        "stdout missing 'Run gần nhất: chưa có': {}",
        out.stdout
    );

    let runs_after = agent_brain::db::count_runs(&sb.db()).expect("count runs after");
    assert_eq!(runs_after, 0);
}

#[test]
fn status_works_without_profile() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );

    let entry = sb.project_entry(&repo, "web-ban-hang", "");
    sb.write_map(serde_json::json!([entry]), &[]);

    let out = sb.run(&["status", "web-ban-hang"], None);
    assert_eq!(
        out.code, 0,
        "status with empty profile failed: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stdout.contains("Nhánh: main"),
        "stdout missing branch: {}",
        out.stdout
    );
}

#[test]
fn status_unknown_project_exit_2() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );

    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    let out = sb.run(&["status", "nonexistent-project"], None);
    assert_eq!(out.code, 2, "expected exit 2, got: {}", out.code);
    assert!(
        out.stdout
            .contains("Không tìm thấy dự án \"nonexistent-project\""),
        "stdout missing Fallback Ask: {}",
        out.stdout
    );
}

#[test]
fn p2_07_inspect_one_turn_no_session() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );

    let entry = sb.project_entry(&repo, "web-ban-hang", "personal");
    sb.write_map(serde_json::json!([entry]), &[]);

    // Tracked modified file
    std::fs::write(repo.join("src/app.py"), "def app():\n    return 2\n").expect("write file");

    let u = "aaaaaaaa-1111-2222-3333-444444444444";
    let v = "bbbbbbbb-1111-2222-3333-444444444444";
    let proj_dir = sb.personal_dir.join("projects").join("-x");
    std::fs::create_dir_all(&proj_dir).expect("create proj dir");
    std::fs::write(proj_dir.join(format!("{u}.jsonl")), "u jsonl content").expect("write u.jsonl");
    let u_dir = proj_dir.join(u);
    std::fs::create_dir_all(&u_dir).expect("create u dir");
    std::fs::write(u_dir.join("sub.txt"), "sub content").expect("write sub.txt");
    std::fs::write(proj_dir.join(format!("{v}.jsonl")), "v jsonl content").expect("write v.jsonl");

    let scenario = serde_json::json!({
        "events": [
            {
                "type": "system",
                "subtype": "init",
                "session_id": u,
                "model": "sonnet"
            },
            {
                "type": "assistant",
                "message": {
                    "role": "assistant",
                    "content": [{"type": "text", "text": "reviewing"}]
                }
            },
            {
                "type": "result",
                "subtype": "success",
                "num_turns": 1,
                "result": "Tóm tắt: sửa app",
                "total_cost_usd": 0.01,
                "usage": {
                    "input_tokens": 5,
                    "output_tokens": 6,
                    "cache_read_input_tokens": 0,
                    "cache_creation_input_tokens": 0
                }
            }
        ]
    });

    let out = sb.run(&["inspect", "web-ban-hang"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "inspect failed with code {}: {}\nstdout: {}",
        out.code, out.stderr, out.stdout
    );
    assert!(
        out.stdout.contains("Tóm tắt: sửa app"),
        "stdout missing review text: {}",
        out.stdout
    );

    let records = sb.records();
    assert_eq!(records.len(), 1, "expected 1 record");
    let rec = &records[0];
    let argv = rec
        .get("argv")
        .and_then(|a| a.as_array())
        .expect("recorded argv");
    let argv_strs: Vec<&str> = argv.iter().filter_map(|s| s.as_str()).collect();

    assert!(argv_strs.contains(&"--output-format"));
    assert!(argv_strs.contains(&"stream-json"));
    assert!(argv_strs.contains(&"--verbose"));
    assert!(argv_strs.contains(&"--model"));
    assert!(argv_strs.contains(&"sonnet"));
    assert!(argv_strs.contains(&"--max-turns"));
    assert!(argv_strs.contains(&"1"));
    let tools_idx = argv_strs
        .iter()
        .position(|&x| x == "--tools")
        .expect("--tools present");
    assert_eq!(
        argv_strs.get(tools_idx + 1),
        Some(&""),
        "--tools followed by empty string"
    );
    assert!(argv_strs.contains(&"--no-session-persistence"));
    assert!(argv_strs.contains(&"--strict-mcp-config"));

    let p_idx = argv_strs
        .iter()
        .position(|&x| x == "-p")
        .expect("-p present");
    let p_arg = argv_strs[p_idx + 1];
    assert!(p_arg.starts_with(agent_brain::fastpath::INSPECT_PROMPT_HEADER));
    assert!(p_arg.contains("src/app.py"));

    let head_sha = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    let conn = sb.db();
    let run = agent_brain::db::get_run(&conn, 1)
        .expect("get run")
        .expect("run 1 exists");
    assert_eq!(run.role, "inspect");
    assert_eq!(run.status, "succeeded");
    assert_eq!(run.num_turns, Some(1));
    assert_eq!(run.task_id, None);
    assert_eq!(run.base_commit, Some(head_sha));
    assert_eq!(run.profile, "personal");

    assert!(
        !proj_dir.join(format!("{u}.jsonl")).exists(),
        "u.jsonl must be deleted"
    );
    assert!(!proj_dir.join(u).exists(), "u dir must be deleted");
    assert!(
        proj_dir.join(format!("{v}.jsonl")).exists(),
        "v.jsonl must still exist"
    );
}

#[test]
fn inspect_invalid_session_id_touches_nothing() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );

    let entry = sb.project_entry(&repo, "web-ban-hang", "personal");
    sb.write_map(serde_json::json!([entry]), &[]);

    std::fs::write(repo.join("src/app.py"), "def app():\n    return 2\n").expect("write file");

    let proj_dir = sb.personal_dir.join("projects").join("-x");
    std::fs::create_dir_all(&proj_dir).expect("create proj dir");
    let keep_file = proj_dir.join("keep.jsonl");
    std::fs::write(&keep_file, "keep content").expect("write keep.jsonl");

    let scenario = serde_json::json!({
        "events": [
            {
                "type": "system",
                "subtype": "init",
                "session_id": "../../etc",
                "model": "sonnet"
            },
            {
                "type": "result",
                "subtype": "success",
                "num_turns": 1,
                "result": "Tóm tắt: sửa app",
                "total_cost_usd": 0.01,
                "usage": {
                    "input_tokens": 5,
                    "output_tokens": 6,
                    "cache_read_input_tokens": 0,
                    "cache_creation_input_tokens": 0
                }
            }
        ]
    });

    let out = sb.run(&["inspect", "web-ban-hang"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "inspect failed: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(keep_file.exists(), "keep.jsonl must still exist");
}

#[test]
fn inspect_two_turns_is_rejected() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );

    let entry = sb.project_entry(&repo, "web-ban-hang", "personal");
    sb.write_map(serde_json::json!([entry]), &[]);

    std::fs::write(repo.join("src/app.py"), "def app():\n    return 2\n").expect("write file");

    let scenario = serde_json::json!({
        "events": [
            {
                "type": "system",
                "subtype": "init",
                "session_id": "aaaaaaaa-1111-2222-3333-444444444444",
                "model": "sonnet"
            },
            {
                "type": "result",
                "subtype": "success",
                "num_turns": 2,
                "result": "Tóm tắt: 2 turns",
                "total_cost_usd": 0.02,
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 12,
                    "cache_read_input_tokens": 0,
                    "cache_creation_input_tokens": 0
                }
            }
        ]
    });

    let out = sb.run(&["inspect", "web-ban-hang"], Some(scenario));
    assert_eq!(out.code, 1, "expected exit code 1, got: {}", out.code);
    assert!(
        out.stderr.contains("inspect không dùng được"),
        "stderr missing rejection message: {}",
        out.stderr
    );

    let conn = sb.db();
    let run = agent_brain::db::get_run(&conn, 1)
        .expect("get run")
        .expect("run exists");
    assert_eq!(run.role, "inspect");
}

#[test]
fn inspect_empty_diff_does_not_call_ai() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );

    let entry = sb.project_entry(&repo, "web-ban-hang", "personal");
    sb.write_map(serde_json::json!([entry]), &[]);

    let out = sb.run(&["inspect", "web-ban-hang"], None);
    assert_eq!(
        out.code, 0,
        "expected exit 0, got {}: {}",
        out.code, out.stderr
    );
    assert!(
        out.stdout
            .contains("Không có thay đổi so với HEAD — không gọi AI"),
        "stdout missing message: {}",
        out.stdout
    );
    assert!(sb.records().is_empty(), "must not start claude");
    let runs_count = if sb.db_path.exists() {
        agent_brain::db::count_runs(&sb.db()).unwrap_or(0)
    } else {
        0
    };
    assert_eq!(runs_count, 0);
}

#[test]
fn inspect_large_diff_is_cut() {
    let sb = Sandbox::new();
    sb.write_config(serde_json::json!({
        "inspect_max_diff_bytes": 2000
    }));

    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );

    let entry = sb.project_entry(&repo, "web-ban-hang", "personal");
    sb.write_map(serde_json::json!([entry]), &[]);

    // ~10 KB of Vietnamese lines
    let mut large_text = String::new();
    for i in 0..300 {
        large_text.push_str(&format!(
            "Dòng mã tiếng Việt thứ {i} cần kiểm tra kỹ lưỡng.\n"
        ));
    }
    std::fs::write(repo.join("src/app.py"), large_text).expect("write large file");

    let scenario = serde_json::json!({
        "events": [
            {
                "type": "system",
                "subtype": "init",
                "session_id": "aaaaaaaa-1111-2222-3333-444444444444",
                "model": "sonnet"
            },
            {
                "type": "result",
                "subtype": "success",
                "num_turns": 1,
                "result": "Tóm tắt: file lớn",
                "total_cost_usd": 0.01,
                "usage": {
                    "input_tokens": 5,
                    "output_tokens": 6,
                    "cache_read_input_tokens": 0,
                    "cache_creation_input_tokens": 0
                }
            }
        ]
    });

    let out = sb.run(&["inspect", "web-ban-hang"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "inspect failed: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stderr.contains("đã cắt"),
        "stderr missing 'đã cắt': {}",
        out.stderr
    );

    let records = sb.records();
    assert_eq!(records.len(), 1);
    let rec = &records[0];
    let argv = rec.get("argv").and_then(|a| a.as_array()).expect("argv");
    let p_idx = argv
        .iter()
        .position(|x| x.as_str() == Some("-p"))
        .expect("-p");
    let p_arg = argv[p_idx + 1].as_str().expect("p_arg string");

    assert!(
        p_arg.len() <= 2000,
        "expected prompt len <= 2000, got: {}",
        p_arg.len()
    );
    assert!(
        p_arg.contains("[diff cắt: đã gửi "),
        "prompt missing cut notice: {}",
        p_arg
    );
    let notice_idx = p_arg.find("[diff cắt: đã gửi ").expect("notice position");
    assert!(
        p_arg[..notice_idx].ends_with('\n'),
        "text before notice must end with newline"
    );
}

#[test]
fn inspect_uses_task_base_commit() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );

    let entry = sb.project_entry(&repo, "web-ban-hang", "personal");
    sb.write_map(serde_json::json!([entry]), &[]);

    let scenario_lead = serde_json::json!({
        "ops": [
            {
                "op": "read",
                "path": "src/app.py"
            },
            {
                "op": "write",
                "path": ".plan.md",
                "keep_meta": true,
                "content": "# Plan: thêm tính năng\n\n## Mục tiêu\nthêm tính năng\n\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | toàn bộ | sửa | yêu cầu |\n\n## Không được đụng tới\n\n## Cách kiểm tra\npython3 -m unittest\n\n## Vướng mắc\n\n"
            }
        ]
    });

    let lead_out = sb.run(
        &["run", "lead", "web-ban-hang", "thêm tính năng"],
        Some(scenario_lead),
    );
    assert_eq!(lead_out.code, 0, "lead run failed: {}", lead_out.stderr);

    // Edit without committing
    std::fs::write(repo.join("src/app.py"), "def app():\n    return 42\n").expect("write file");

    let scenario_inspect = serde_json::json!({
        "events": [
            {
                "type": "system",
                "subtype": "init",
                "session_id": "aaaaaaaa-1111-2222-3333-444444444444",
                "model": "sonnet"
            },
            {
                "type": "result",
                "subtype": "success",
                "num_turns": 1,
                "result": "Tóm tắt: khớp plan",
                "total_cost_usd": 0.01,
                "usage": {
                    "input_tokens": 5,
                    "output_tokens": 6,
                    "cache_read_input_tokens": 0,
                    "cache_creation_input_tokens": 0
                }
            }
        ]
    });

    let out = sb.run(&["inspect", "web-ban-hang"], Some(scenario_inspect));
    assert_eq!(
        out.code, 0,
        "inspect failed: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let conn = sb.db();
    let task = agent_brain::db::get_task(&conn, 1)
        .expect("get task")
        .expect("task 1 exists");
    // inspect run is run 2 (run 1 was lead)
    let run = agent_brain::db::get_run(&conn, 2)
        .expect("get run")
        .expect("run 2 exists");
    assert_eq!(run.role, "inspect");
    assert_eq!(run.task_id, Some(1));
    assert_eq!(run.base_commit, task.base_commit);

    let records = sb.records();
    let last_rec = &records[records.len() - 1];
    let argv = last_rec
        .get("argv")
        .and_then(|a| a.as_array())
        .expect("argv");
    let p_idx = argv
        .iter()
        .position(|x| x.as_str() == Some("-p"))
        .expect("-p");
    let p_arg = argv[p_idx + 1].as_str().expect("p_arg");
    assert!(
        p_arg.contains("--- .plan.md ---"),
        "prompt missing .plan.md section"
    );
    let diff_part = p_arg.split("--- git diff ").nth(1).unwrap_or("");
    assert!(
        !diff_part.contains(".plan.md"),
        "git diff must exclude .plan.md"
    );
}
