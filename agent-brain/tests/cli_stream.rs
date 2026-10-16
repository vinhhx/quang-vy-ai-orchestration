mod common;

use common::Sandbox;
use std::path::PathBuf;

#[test]
fn p2_04_runs_row_complete() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-p204",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-p204", "work");
    entry["name_vi"] = serde_json::json!("Web P204");
    sb.write_map(serde_json::json!([entry]), &[]);

    let scenario = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan.md",
                "keep_meta": true,
                "content": "# Plan: p204\n\n## Mục tiêu\ntest\n\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | toàn bộ | sửa | test |\n\n## Không được đụng tới\n\n## Cách kiểm tra\ntrue\n\n## Vướng mắc\n\n"
            }
        ],
        "events": [
            {
                "type": "system",
                "subtype": "init",
                "session_id": "sess-p204",
                "model": "opus"
            },
            {
                "type": "assistant",
                "message": {
                    "id": "m1",
                    "model": "opus",
                    "content": [
                        {
                            "type": "text",
                            "text": "working on plan"
                        }
                    ]
                }
            },
            {
                "type": "result",
                "subtype": "success",
                "is_error": false,
                "session_id": "sess-p204",
                "num_turns": 1,
                "total_cost_usd": 0.5,
                "usage": {
                    "input_tokens": 11,
                    "output_tokens": 22,
                    "cache_read_input_tokens": 33,
                    "cache_creation_input_tokens": 44
                },
                "permission_denials": []
            }
        ]
    });

    let out = sb.run(&["run", "lead", "web-p204", "p204 plan"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let conn = sb.db();
    let row = conn
        .query_row(
            "SELECT input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, session_id, pid, pgid, pid_start, status, exit_code, started_at, ended_at, num_turns, assistant_turns, usage_source, model, profile, run_dir FROM runs WHERE id = 1",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, f64>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, Option<i32>>(10)?,
                    r.get::<_, i64>(11)?,
                    r.get::<_, Option<i64>>(12)?,
                    r.get::<_, Option<i64>>(13)?,
                    r.get::<_, Option<i64>>(14)?,
                    r.get::<_, String>(15)?,
                    r.get::<_, Option<String>>(16)?,
                    r.get::<_, String>(17)?,
                    r.get::<_, Option<String>>(18)?,
                ))
            },
        )
        .expect("query runs row");

    assert_eq!(row.0, 11, "input_tokens");
    assert_eq!(row.1, 22, "output_tokens");
    assert_eq!(row.2, 33, "cache_read_tokens");
    assert_eq!(row.3, 44, "cache_write_tokens");
    assert!((row.4 - 0.5).abs() < 1e-6, "cost_usd: {}", row.4);
    assert_eq!(row.5.as_deref(), Some("sess-p204"), "session_id");
    assert!(row.6 > 0, "pid > 0: {}", row.6);
    assert_eq!(row.6, row.7, "pgid == pid: {} vs {}", row.7, row.6);
    assert!(row.8 > 0, "pid_start > 0: {}", row.8);
    assert_eq!(row.9, "succeeded", "status");
    assert_eq!(row.10, Some(0), "exit_code");
    let started_at = row.11;
    let ended_at = row.12.expect("ended_at");
    assert!(
        started_at <= ended_at,
        "started_at <= ended_at: {started_at} <= {ended_at}"
    );
    assert_eq!(row.13, Some(1), "num_turns");
    assert_eq!(row.14, Some(1), "assistant_turns");
    assert_eq!(row.15, "result", "usage_source");
    assert_eq!(row.16.as_deref(), Some("opus"), "model");
    assert_eq!(row.17, "work", "profile");

    // Events rows
    let run_dir = PathBuf::from(row.18.expect("run_dir"));
    let stream_file = run_dir.join("stream.jsonl");
    let stream_content = std::fs::read_to_string(&stream_file).expect("read stream.jsonl");
    let non_empty_lines: Vec<&str> = stream_content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    let mut stmt = conn
        .prepare("SELECT raw_json FROM events WHERE run_id = 1 ORDER BY seq ASC")
        .expect("prepare events query");
    let event_raws: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .expect("query events")
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(event_raws.len(), non_empty_lines.len());
    for (i, (evt, line)) in event_raws.iter().zip(non_empty_lines.iter()).enumerate() {
        assert_eq!(
            evt,
            line,
            "event raw at seq {} does not match stream.jsonl",
            i + 1
        );
    }
}

#[test]
fn p2_05_bad_json_line_stored_raw() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-p205",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-p205", "work");
    entry["name_vi"] = serde_json::json!("Web P205");
    sb.write_map(serde_json::json!([entry]), &[]);

    let scenario = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan.md",
                "keep_meta": true,
                "content": "# Plan: p205\n\n## Mục tiêu\ntest\n\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | toàn bộ | sửa | test |\n\n## Không được đụng tới\n\n## Cách kiểm tra\ntrue\n\n## Vướng mắc\n\n"
            }
        ],
        "raw_lines": [
            "{\"type\": \"system\", \"subtype\": \"init\", \"session_id\": \"s5\", \"model\": \"opus\"}",
            "not json",
            "[1, 2]",
            "{\"type\": \"system\", \"sub",
            "{\"type\": \"brand_new_event\", \"foo\": 1}",
            "{\"type\": \"system\", \"subtype\": \"init\", \"x_unknown\": {\"a\": 1}, \"session_id\": \"s5\"}",
            "{\"type\": \"result\", \"subtype\": \"success\", \"session_id\": \"s5\", \"is_error\": false, \"num_turns\": 1, \"usage\": {\"input_tokens\": 1, \"output_tokens\": 2, \"cache_read_input_tokens\": 0, \"cache_creation_input_tokens\": 0}}"
        ]
    });

    let out = sb.run(&["run", "lead", "web-p205", "p205 plan"], Some(scenario));
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let conn = sb.db();
    let (status, in_tok, out_tok): (String, i64, i64) = conn
        .query_row(
            "SELECT status, input_tokens, output_tokens FROM runs WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("query run row");
    assert_eq!(status, "succeeded");
    assert_eq!(in_tok, 1);
    assert_eq!(out_tok, 2);

    let mut stmt = conn
        .prepare("SELECT kind, raw_json FROM events WHERE run_id = 1 ORDER BY seq ASC")
        .expect("prepare events query");
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .expect("query events")
        .map(|r| r.unwrap())
        .collect();

    let expected_kinds = vec![
        "system/init",
        "bad_json",
        "bad_json",
        "bad_json",
        "unknown:brand_new_event",
        "system/init",
        "result",
    ];
    let actual_kinds: Vec<&str> = rows.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(actual_kinds, expected_kinds);

    assert_eq!(rows[1].1, "not json");
    assert_eq!(rows[2].1, "[1, 2]");
    assert_eq!(rows[3].1, "{\"type\": \"system\", \"sub");
}

#[test]
fn closed_stderr_does_not_abort_run() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        "web-pipe",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-pipe", "work");
    entry["name_vi"] = serde_json::json!("Web Pipe");
    sb.write_map(serde_json::json!([entry]), &[]);

    let scenario = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan.md",
                "keep_meta": true,
                "content": "# Plan: pipe\n\n## Mục tiêu\ntest\n\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | toàn bộ | sửa | test |\n\n## Không được đụng tới\n\n## Cách kiểm tra\ntrue\n\n## Vướng mắc\n\n"
            }
        ]
    });
    std::fs::write(&sb.scenario_file, serde_json::to_string(&scenario).unwrap())
        .expect("write scenario");

    let mut child = sb
        .command(&["run", "lead", "web-pipe", "test closed stderr"], &[])
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn agent-brain");

    // Close the read end of stderr immediately
    drop(child.stderr.take());

    let status = child.wait().expect("wait child");
    assert_eq!(status.code(), Some(0));

    let conn = sb.db();
    let run_status: String = conn
        .query_row("SELECT status FROM runs WHERE id = 1", [], |r| r.get(0))
        .expect("query run status");
    assert_eq!(run_status, "succeeded");

    let head_subject = sb.git(&repo, &["log", "-1", "--pretty=%s"]);
    assert!(
        head_subject.trim().ends_with(": plan"),
        "HEAD commit subject should end with ': plan', got: {head_subject}"
    );
}

#[test]
fn default_mode_truncates_verbose_shows_all() {
    let mut text_300 = String::new();
    for _ in 0..249 {
        text_300.push('A');
    }
    text_300.push('Z');
    for _ in 0..50 {
        text_300.push('B');
    }
    assert_eq!(text_300.len(), 300);

    let scenario = serde_json::json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan.md",
                "keep_meta": true,
                "content": "# Plan: trunc\n\n## Mục tiêu\ntest\n\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | src/app.py | toàn bộ | sửa | test |\n\n## Không được đụng tới\n\n## Cách kiểm tra\ntrue\n\n## Vướng mắc\n\n"
            }
        ],
        "events": [
            {
                "type": "system",
                "subtype": "init",
                "session_id": "sess-trunc",
                "model": "opus"
            },
            {
                "type": "assistant",
                "message": {
                    "id": "m1",
                    "content": [
                        {
                            "type": "text",
                            "text": text_300
                        }
                    ]
                }
            },
            {
                "type": "user",
                "message": {
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_1",
                            "content": "SECRET-CONTENT",
                            "is_error": false
                        }
                    ]
                }
            },
            {
                "type": "result",
                "subtype": "success",
                "is_error": false,
                "session_id": "sess-trunc",
                "num_turns": 1,
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 10,
                    "cache_read_input_tokens": 0,
                    "cache_creation_input_tokens": 0
                }
            }
        ]
    });

    // 1. Default mode: truncates assistant text and omits tool_result content
    let sb1 = Sandbox::new();
    let repo1 = sb1.make_repo("proj-trunc1", &[("src/app.py", "x")], None, None);
    let entry1 = sb1.project_entry(&repo1, "proj-trunc1", "work");
    sb1.write_map(serde_json::json!([entry1]), &[]);

    let out1 = sb1.run(
        &["run", "lead", "proj-trunc1", "test default"],
        Some(scenario.clone()),
    );
    assert_eq!(out1.code, 0, "stderr: {}", out1.stderr);
    let first_100 = "A".repeat(100);
    assert!(
        out1.stderr.contains(&first_100),
        "Default stderr should contain first 100 characters of assistant text: {}",
        out1.stderr
    );
    assert!(
        !out1.stderr.contains('Z'),
        "Default stderr should NOT contain character 'Z': {}",
        out1.stderr
    );
    assert!(
        !out1.stderr.contains("SECRET-CONTENT"),
        "Default stderr should NOT contain SECRET-CONTENT: {}",
        out1.stderr
    );

    // 2. Verbose mode: includes 'Z' and SECRET-CONTENT
    let sb2 = Sandbox::new();
    let repo2 = sb2.make_repo("proj-trunc2", &[("src/app.py", "x")], None, None);
    let entry2 = sb2.project_entry(&repo2, "proj-trunc2", "work");
    sb2.write_map(serde_json::json!([entry2]), &[]);

    let out2 = sb2.run(
        &["run", "lead", "proj-trunc2", "test verbose", "--verbose"],
        Some(scenario),
    );
    assert_eq!(out2.code, 0, "stderr: {}", out2.stderr);
    assert!(
        out2.stderr.contains('Z'),
        "Verbose stderr should contain 'Z': {}",
        out2.stderr
    );
    assert!(
        out2.stderr.contains("SECRET-CONTENT"),
        "Verbose stderr should contain SECRET-CONTENT: {}",
        out2.stderr
    );
}
