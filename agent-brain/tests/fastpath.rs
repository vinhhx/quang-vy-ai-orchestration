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
