mod common;

use common::Sandbox;
use std::os::unix::fs::PermissionsExt;

#[test]
fn p2_01_run_lead_prints_steps_and_records_run() {
    let sb = Sandbox::new();
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

    let out = sb.run(
        &["run", "lead", "web bán hàng", "thêm nút đăng xuất"],
        Some(scenario),
    );

    assert_eq!(
        out.code, 0,
        "Expected exit code 0, stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    // Stderr checks
    assert!(
        out.stderr.contains("task 1, run 1"),
        "stderr missing 'task 1, run 1': {}",
        out.stderr
    );

    let re_time = regex::Regex::new(r"^\d\d:\d\d:\d\d ").unwrap();
    let time_lines: Vec<&str> = out.stderr.lines().filter(|l| re_time.is_match(l)).collect();
    assert!(!time_lines.is_empty(), "No time lines matching HH:MM:SS in stderr");

    assert!(
        time_lines.iter().any(|l| l.contains("init")),
        "No time line with 'init' found in stderr: {}",
        out.stderr
    );
    assert!(
        time_lines.iter().any(|l| l.contains("tool Read")),
        "No time line with 'tool Read' found in stderr: {}",
        out.stderr
    );
    assert!(
        time_lines.iter().any(|l| l.contains("tool Edit")),
        "No time line with 'tool Edit' found in stderr: {}",
        out.stderr
    );
    assert!(
        time_lines.iter().any(|l| l.contains("kết quả tool: ok")),
        "No time line with 'kết quả tool: ok' found in stderr: {}",
        out.stderr
    );

    assert!(
        out.stderr
            .lines()
            .any(|l| l.starts_with("Kết thúc run 1 (task 1): succeeded")),
        "stderr missing 'Kết thúc run 1 (task 1): succeeded': {}",
        out.stderr
    );

    // Stdout checks
    let plan_path = repo.join(".plan.md");
    assert!(
        out.stdout.trim().ends_with(plan_path.to_str().unwrap()),
        "stdout expected to end with .plan.md path ({}): {}",
        plan_path.display(),
        out.stdout
    );

    // Git checks
    let current_branch = sb.git(&repo, &["branch", "--show-current"]);
    assert_eq!(current_branch.trim(), "task/them-nut-dang-xuat");

    let head_subject = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subject.trim(), "them-nut-dang-xuat: plan");

    let diff_files = sb.git(&repo, &["diff", "--name-only", "HEAD~1", "HEAD"]);
    assert_eq!(diff_files.trim(), ".plan.md");

    // Fake claude record checks
    let records = sb.records();
    assert_eq!(records.len(), 1, "Expected 1 record");
    let rec = &records[0];

    let prompt_bytes = include_str!("../assets/prompts/lead.md");
    let argv = rec["argv"].as_array().expect("argv array");
    let argv_strs: Vec<&str> = argv.iter().map(|v| v.as_str().unwrap()).collect();

    assert_eq!(argv_strs[0], "-p");
    assert_eq!(argv_strs[2], "--output-format");
    assert_eq!(argv_strs[3], "stream-json");
    assert_eq!(argv_strs[4], "--verbose");
    assert_eq!(argv_strs[5], "--model");
    assert_eq!(argv_strs[6], "opus");
    assert_eq!(argv_strs[7], "--append-system-prompt");
    assert_eq!(argv_strs[8], prompt_bytes);
    assert!(argv_strs.contains(&"--strict-mcp-config"));

    let abs_plan_str = repo.join(".plan.md").to_str().unwrap().to_string();
    let expected_allowed = [
        "Read",
        "Grep",
        "Glob",
        "Write(./.plan.md)",
        "Edit(./.plan.md)",
        "Bash(git log *)",
        "Bash(git show *)",
        "Bash(git diff *)",
        "Bash(git status *)",
        &format!("Write(/{})", abs_plan_str),
        &format!("Edit(/{})", abs_plan_str),
    ];
    let allowed_idx = argv_strs
        .iter()
        .position(|&x| x == "--allowedTools")
        .expect("--allowedTools");
    let disallowed_idx = argv_strs
        .iter()
        .position(|&x| x == "--disallowedTools")
        .expect("--disallowedTools");

    let actual_allowed = &argv_strs[allowed_idx + 1..disallowed_idx];
    assert_eq!(actual_allowed, &expected_allowed);

    let actual_disallowed = &argv_strs[disallowed_idx + 1..];
    assert_eq!(
        actual_disallowed,
        &["WebFetch", "WebSearch", "NotebookEdit", "Task"]
    );

    assert!(
        rec["claude_config_dir"].is_null(),
        "claude_config_dir should be null"
    );
    let present_vars = rec["present_vars"].as_array().unwrap();
    assert!(
        present_vars.is_empty(),
        "present_vars should be empty but was {:?}",
        present_vars
    );

    // Database checks
    let conn = sb.db();
    let proj_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM projects WHERE slug = 'web-ban-hang'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(proj_count, 1);

    let (task_id, task_branch, task_status): (i64, String, String) = conn
        .query_row(
            "SELECT id, branch, status FROM tasks WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(task_id, 1);
    assert_eq!(task_branch, "task/them-nut-dang-xuat");
    assert_eq!(task_status, "planned");

    let (
        role,
        profile,
        model,
        status,
        exit_code,
        session_id,
        in_tok,
        out_tok,
        cr_tok,
        cw_tok,
        usage_source,
        pid,
        pgid,
        pid_start,
        started_at,
        ended_at,
        db_run_dir,
    ): (
        String,
        String,
        String,
        String,
        Option<i32>,
        Option<String>,
        i64,
        i64,
        i64,
        i64,
        String,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        i64,
        Option<i64>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT role, profile, model, status, exit_code, session_id,
                    input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
                    usage_source, pid, pgid, pid_start, started_at, ended_at, run_dir
             FROM runs WHERE id = 1",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                    r.get(13)?,
                    r.get(14)?,
                    r.get(15)?,
                    r.get(16)?,
                ))
            },
        )
        .unwrap();

    assert_eq!(role, "lead");
    assert_eq!(profile, "work");
    assert_eq!(model, "opus");
    assert_eq!(status, "succeeded");
    assert_eq!(exit_code, Some(0));
    assert_eq!(session_id.as_deref(), Some("fake-session"));
    assert_eq!(in_tok, 10);
    assert_eq!(out_tok, 5);
    assert_eq!(cr_tok, 0);
    assert_eq!(cw_tok, 0);
    assert_eq!(usage_source, "result");
    let pid = pid.expect("pid");
    let pgid = pgid.expect("pgid");
    let pid_start = pid_start.expect("pid_start");
    assert!(pid > 0);
    assert_eq!(pgid, pid);
    assert!(pid_start > 0);
    assert!(started_at <= ended_at.expect("ended_at"));

    let event_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM events WHERE run_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(event_count, 6);

    let (first_kind, first_raw): (String, String) = conn
        .query_row(
            "SELECT kind, raw_json FROM events WHERE run_id = 1 AND seq = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(first_kind, "system/init");
    let first_json: serde_json::Value = serde_json::from_str(&first_raw).unwrap();
    assert_eq!(first_json["type"], "system");
    assert_eq!(first_json["subtype"], "init");

    let last_kind: String = conn
        .query_row(
            "SELECT kind FROM events WHERE run_id = 1 AND seq = 6",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(last_kind, "result");

    // Run directory checks
    let run_dir_path = std::path::PathBuf::from(db_run_dir.expect("run_dir in DB"));
    assert!(run_dir_path.exists());
    assert!(run_dir_path.is_dir());
    assert!(run_dir_path.to_str().unwrap().ends_with("-lead"));

    let meta = std::fs::metadata(&run_dir_path).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o700);

    assert!(run_dir_path.join("stream.jsonl").is_file());
    assert!(run_dir_path.join("stderr.txt").is_file());

    let task_runs_dir = sb.runs.join("web-ban-hang").join("them-nut-dang-xuat");
    let entries: Vec<_> = std::fs::read_dir(task_runs_dir).unwrap().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].as_ref().unwrap().path(), run_dir_path);
}

#[test]
fn run_lead_personal_profile_sets_config_dir() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );

    let mut entry = sb.project_entry(&repo, "web-ban-hang", "personal");
    entry["name_vi"] = serde_json::json!("Web bán hàng");
    entry["aliases"] = serde_json::json!(["shop"]);
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

    let out = sb.run(
        &["run", "lead", "web bán hàng", "thêm nút đăng xuất"],
        Some(scenario),
    );
    assert_eq!(
        out.code, 0,
        "Failed exit code: {}\nstderr: {}",
        out.code, out.stderr
    );

    let records = sb.records();
    assert_eq!(records.len(), 1);
    let rec = &records[0];
    assert_eq!(
        rec["claude_config_dir"].as_str().unwrap(),
        sb.personal_dir.to_str().unwrap()
    );

    let conn = sb.db();
    let prof: String = conn
        .query_row("SELECT profile FROM runs WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(prof, "personal");
}
