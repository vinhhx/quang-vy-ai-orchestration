#![allow(clippy::while_let_loop)]

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn fake_agy() -> PathBuf {
    common::repo_root()
        .join("tests")
        .join("fakes")
        .join("fake_agy.py")
        .canonicalize()
        .expect("canonicalize fake_agy.py")
}

fn valid_analyst_text() -> String {
    "# Phân tích chức năng hệ thống\n\n## Tóm tắt\n- Đã kiểm tra mã nguồn hệ thống trong dự án.\n- Chức năng được yêu cầu đã có một phần ở src/main.rs:1.\n- Không cần bổ sung cơ sở dữ liệu mới.\n- Rủi ro kỹ thuật thấp, có thể tiến hành ngay.\n\n## Đã có chưa\nCó một phần trong module chính.\n\n## Bằng chứng\nsrc/main.rs:1\n\n## Giả định & cần làm rõ\nGiả định rằng phiên bản PHP/Rust tương thích với môi trường hiện tại.".to_string()
}

fn valid_reviewer_text() -> String {
    "# Phân tích chức năng hệ thống (Đã review)\n\n## Tóm tắt\n- Báo cáo đã được Analysis Reviewer thẩm định và hiệu đính chi tiết.\n- Xác nhận chức năng đã có ở src/main.rs:1.\n- Đã bổ sung các trường hợp ngoại lệ cần chú ý.\n- Rủi ro kỹ thuật ở mức tối thiểu.\n\n## Bằng chứng\nsrc/main.rs:1\n\n## Ghi chú review\nĐã đối chiếu với code thực tế và chuẩn hóa lại các khuyến nghị kỹ thuật.".to_string()
}

fn init_project(sb: &Sandbox, name: &str) -> PathBuf {
    let repo = sb.make_repo(name, &[("src/main.rs", "fn main() {}\n")], None, None);
    let entry = sb.project_entry(&repo, name, "work");
    sb.write_map(serde_json::json!([entry]), &[]);
    repo
}

#[test]
fn test_analyst_run_claude_success_b() {
    let sb = Sandbox::new();
    let repo = init_project(&sb, "my-proj");

    let initial_status = sb.git(&repo, &["status", "--porcelain"]);
    assert!(initial_status.trim().is_empty());

    let scenario = json!({
        "result": valid_analyst_text(),
    });

    let res = sb.run(
        &[
            "run",
            "analyst",
            "my-proj",
            "Hệ thống đã có chức năng X chưa?",
            "--name",
            "task-x",
        ],
        Some(scenario),
    );

    assert_eq!(
        res.code, 0,
        "Analyst run failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let answer_file = sb
        .xdg_data
        .join("agent-brain")
        .join("answers")
        .join("my-proj")
        .join("1-task-x.md");
    assert!(answer_file.is_file(), "File answer phải tồn tại");

    let meta = fs::metadata(&answer_file).expect("metadata answer file");
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "Quyền file phải là 0600");

    let dir_meta = fs::metadata(answer_file.parent().unwrap()).expect("metadata answer dir");
    let dir_mode = dir_meta.permissions().mode() & 0o777;
    assert_eq!(dir_mode, 0o700, "Quyền thư mục answers phải là 0700");

    let content = fs::read_to_string(&answer_file).expect("read answer");
    assert!(content.contains("## Tóm tắt"));
    assert!(content.contains("src/main.rs:1"));

    // Check git: branch analysis/task-x MUST NOT exist in git
    let git_branches = sb.git(&repo, &["branch", "--list", "analysis/task-x"]);
    assert!(
        git_branches.trim().is_empty(),
        "Nhánh analysis/task-x không được tồn tại trong git"
    );

    // Git status must be unchanged
    let post_status = sb.git(&repo, &["status", "--porcelain"]);
    assert_eq!(
        post_status, initial_status,
        "Git status không được thay đổi sau run analyst"
    );

    // Check task status in db: answered
    let conn = rusqlite::Connection::open(&sb.db_path).unwrap();
    let status: String = conn
        .query_row("SELECT status FROM tasks WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "answered", "Task status phải là answered");
}

#[test]
fn test_analyst_run_claude_modifies_repo_violates_protocol_c() {
    let sb = Sandbox::new();
    let _repo = init_project(&sb, "my-proj");

    let scenario = json!({
        "ops": [
            {"op": "write", "path": "pwned.txt", "content": "violation"}
        ],
        "result": valid_analyst_text(),
    });

    let res = sb.run(
        &[
            "run",
            "analyst",
            "my-proj",
            "Test protocol violation",
            "--name",
            "test-viol",
        ],
        Some(scenario),
    );

    assert_ne!(res.code, 0, "Chạy phải thất bại khi vi phạm protocol");
    assert!(
        res.stderr.contains("vi phạm quy tắc chỉ đọc")
            || res.stdout.contains("vi phạm quy tắc chỉ đọc"),
        "Phải thông báo vi phạm protocol"
    );

    let answer_file = sb
        .xdg_data
        .join("agent-brain")
        .join("answers")
        .join("my-proj")
        .join("1-test-viol.md");
    assert!(
        !answer_file.exists(),
        "Không được lưu file answer khi vi phạm protocol"
    );

    let conn = rusqlite::Connection::open(&sb.db_path).unwrap();
    let outcome: String = conn
        .query_row("SELECT outcome FROM runs WHERE task_id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        outcome, "protocol_violation",
        "Outcome phải là protocol_violation"
    );

    let task_status: String = conn
        .query_row("SELECT status FROM tasks WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(task_status, "open", "Task phải giữ nguyên trạng thái open");
}

#[test]
fn test_analyst_run_missing_summary_fails_d() {
    let sb = Sandbox::new();
    let _repo = init_project(&sb, "my-proj");

    let scenario = json!({
        "result": "Đoạn văn này không có mục tóm tắt theo quy định cấu trúc báo cáo.",
    });

    let res = sb.run(
        &[
            "run",
            "analyst",
            "my-proj",
            "Test invalid answer",
            "--name",
            "test-invalid",
        ],
        Some(scenario),
    );

    assert_ne!(
        res.code, 0,
        "Chạy phải thất bại khi thiếu mục tóm tắt hoặc dưới 200 ký tự"
    );

    let answer_file = sb
        .xdg_data
        .join("agent-brain")
        .join("answers")
        .join("my-proj")
        .join("1-test-invalid.md");
    assert!(!answer_file.exists(), "Không được lưu file answer");

    let conn = rusqlite::Connection::open(&sb.db_path).unwrap();
    let outcome: String = conn
        .query_row("SELECT outcome FROM runs WHERE task_id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(outcome, "analyst:invalid");

    let task_status: String = conn
        .query_row("SELECT status FROM tasks WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(task_status, "open");
}

#[test]
fn test_analyst_run_agy_read_only_e() {
    let sb = Sandbox::new();
    let repo = init_project(&sb, "my-proj");

    let fake_agy_bin = fake_agy();
    sb.write_config(json!({
        "engines": {
            "agy": {
                "command": fake_agy_bin.to_str().unwrap()
            }
        }
    }));

    sb.write_teams(json!({
        "team-agy": {
            "description": "Đội Agy",
            "roles": {
                "lead": "agy"
            }
        }
    }));

    let agy_scenario = json!({
        "status": "SUCCESS",
        "response": valid_analyst_text(),
    });
    let scenario_path = sb.root.join("fake_agy_scenario.json");
    let record_path = sb.root.join("fake_agy_record.jsonl");
    fs::write(&scenario_path, serde_json::to_string(&agy_scenario).unwrap()).unwrap();

    let res = sb.run_env(
        &[
            "run",
            "analyst",
            "my-proj",
            "Câu hỏi cho agy",
            "--name",
            "agy-task",
            "--team",
            "team-agy",
        ],
        None,
        &[
            ("FAKE_AGY_SCENARIO", scenario_path.to_str().unwrap()),
            ("FAKE_AGY_RECORD", record_path.to_str().unwrap()),
        ],
    );

    assert_eq!(
        res.code, 0,
        "Analyst qua agy thất bại: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let answer_file = sb
        .xdg_data
        .join("agent-brain")
        .join("answers")
        .join("my-proj")
        .join("1-agy-task.md");
    assert!(answer_file.is_file(), "File answer của agy phải được lưu");

    let content = fs::read_to_string(&answer_file).unwrap();
    assert!(content.contains("## Tóm tắt"));

    // Ensure NO files were written into repo
    assert!(!repo.join(".plan.md").exists());
    assert!(!repo.join(".analyst.md").exists());
    let st = sb.git(&repo, &["status", "--porcelain"]);
    assert!(st.trim().is_empty(), "Repo không được bị ghi file nào");
}

#[test]
fn test_analyst_review_workflow_f() {
    let sb = Sandbox::new();
    let _repo = init_project(&sb, "my-proj");

    // Case 1: reviewer succeeds -> reviewer answer replaces analyst answer
    let scenario_success = json!({
        "by_role": {
            "analyst": [{
                "result": valid_analyst_text()
            }],
            "analysis-review": [{
                "result": valid_reviewer_text()
            }]
        }
    });

    let res = sb.run(
        &[
            "run",
            "analyst",
            "my-proj",
            "Câu hỏi cần review",
            "--name",
            "review-ok",
            "--workflow",
            "phan-tich-review",
        ],
        Some(scenario_success),
    );

    assert_eq!(
        res.code, 0,
        "Phân tích review thành công: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let answer_file = sb
        .xdg_data
        .join("agent-brain")
        .join("answers")
        .join("my-proj")
        .join("1-review-ok.md");
    assert!(answer_file.is_file());
    let content = fs::read_to_string(&answer_file).unwrap();
    assert!(
        content.contains("Đã review"),
        "Nội dung phải là bản của reviewer"
    );
    assert!(content.contains("## Ghi chú review"));

    let conn = rusqlite::Connection::open(&sb.db_path).unwrap();
    let run_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM runs WHERE task_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(run_count, 2, "Phải có 2 run (analyst và analysis-review)");

    // Case 2: reviewer fails -> keeps analyst answer with `> Bước review lỗi` prefix
    let scenario_fail = json!({
        "by_role": {
            "analyst": [{
                "result": valid_analyst_text()
            }],
            "analysis-review": [{
                "exit_code": 1,
                "result": ""
            }]
        }
    });

    let res2 = sb.run(
        &[
            "run",
            "analyst",
            "my-proj",
            "Câu hỏi reviewer lỗi",
            "--name",
            "review-fail",
            "--workflow",
            "phan-tich-review",
        ],
        Some(scenario_fail),
    );

    assert_eq!(
        res2.code, 0,
        "Vẫn lưu được câu trả lời analyst khi reviewer lỗi"
    );

    let answer_file2 = sb
        .xdg_data
        .join("agent-brain")
        .join("answers")
        .join("my-proj")
        .join("2-review-fail.md");
    assert!(answer_file2.is_file());
    let content2 = fs::read_to_string(&answer_file2).unwrap();
    assert!(
        content2.starts_with("> Bước review lỗi:"),
        "Phải có dòng thông báo review lỗi ở đầu file"
    );
    assert!(
        content2.contains("## Tóm tắt"),
        "Vẫn giữ nguyên bản phân tích của analyst"
    );
}

#[test]
fn test_analyst_engine_and_model_from_lead_g() {
    let sb = Sandbox::new();
    let _repo = init_project(&sb, "my-proj");

    let fake_agy_bin = fake_agy();
    sb.write_config(json!({
        "engines": {
            "agy": {
                "command": fake_agy_bin.to_str().unwrap()
            }
        },
        "models": {
            "lead": "model-lead-custom",
            "dev": "model-dev-custom",
            "qa": "model-qa",
            "plan-review": "model-pr",
            "code-review": "model-cr"
        }
    }));

    sb.write_teams(json!({
        "team-lead-agy": {
            "description": "Đội lead agy",
            "roles": {
                "lead": "agy"
            }
        }
    }));

    let res = sb.run(
        &[
            "run",
            "analyst",
            "my-proj",
            "Dry run test",
            "--team",
            "team-lead-agy",
            "--dry-run",
        ],
        None,
    );

    assert_eq!(
        res.code, 0,
        "Dry run failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        res.stdout.contains("Model: model-lead-custom"),
        "Model của analyst phải kế thừa từ vai lead: {}",
        res.stdout
    );
}
