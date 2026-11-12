//! Tài liệu dán là đường dẫn tới file `.md` trong thư mục `answers` được thay bằng nội dung file;
//! mọi trường hợp ngoài điều kiện giữ nguyên chuỗi dán. Chỉ dùng thư mục và CSDL tạm.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::inputs::{resolve_answer_ref, validate_inputs, InputItem, MAX_INPUT_BYTES};
use common::Sandbox;
use protocol::setup_project;
use serde_json::json;
use std::path::{Path, PathBuf};

/// `answers/` tạm với một file trả lời hợp lệ.
fn answers_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("answers");
    let proj = root.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let file = proj.join("57-sitemap.md");
    std::fs::write(&file, "# Sitemap\n- https://example.test/a\n- https://example.test/b\n").unwrap();
    (tmp, root, file)
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[test]
fn a_valid_path_inside_answers_is_replaced_by_the_file_content() {
    let (_tmp, root, file) = answers_fixture();
    let want = std::fs::read_to_string(&file).unwrap();
    assert_eq!(resolve_answer_ref(&s(&file), &root), Some(want.clone()));
    // Khoảng trắng và xuống dòng ở hai đầu không ảnh hưởng.
    assert_eq!(resolve_answer_ref(&format!("  {}\n", s(&file)), &root), Some(want));
}

#[test]
fn paths_outside_answers_are_kept_as_typed() {
    let (tmp, root, file) = answers_fixture();
    assert_eq!(resolve_answer_ref("/etc/hostname", &root), None);

    // File .md hợp lệ nhưng nằm cạnh `answers`, không nằm trong đó.
    let outside = tmp.path().join("outside.md");
    std::fs::write(&outside, "ngoài").unwrap();
    assert_eq!(resolve_answer_ref(&s(&outside), &root), None);

    // `..` thoát ra khỏi answers.
    let escape = format!("{}/proj/../../outside.md", s(&root));
    assert_eq!(resolve_answer_ref(&escape, &root), None);

    // Thư mục anh em có tiền tố trùng tên (`answers-evil`) không được coi là nằm trong `answers`.
    let evil = tmp.path().join("answers-evil");
    std::fs::create_dir_all(&evil).unwrap();
    let evil_file = evil.join("x.md");
    std::fs::write(&evil_file, "x").unwrap();
    assert_eq!(resolve_answer_ref(&s(&evil_file), &root), None);

    // Đường dẫn tương đối không được nhận.
    assert_eq!(resolve_answer_ref("proj/57-sitemap.md", &root), None);
    assert_eq!(resolve_answer_ref("~/proj/57-sitemap.md", &root), None);
    // Sanity: file hợp lệ vẫn được nhận.
    assert!(resolve_answer_ref(&s(&file), &root).is_some());
}

#[cfg(unix)]
#[test]
fn a_symlink_escaping_answers_is_kept_as_typed() {
    let (tmp, root, _file) = answers_fixture();
    let outside = tmp.path().join("secret-notes.md");
    std::fs::write(&outside, "không được đọc").unwrap();
    let link = root.join("proj").join("link.md");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    assert_eq!(resolve_answer_ref(&s(&link), &root), None);

    // Thư mục con là symlink trỏ ra ngoài.
    let dir_link = root.join("dir-link");
    std::os::unix::fs::symlink(tmp.path(), &dir_link).unwrap();
    let via_dir = format!("{}/secret-notes.md", s(&dir_link));
    assert_eq!(resolve_answer_ref(&via_dir, &root), None);

    // Symlink trỏ vào bên trong answers thì vẫn nằm trong answers.
    let inner = root.join("proj").join("inner-link.md");
    std::os::unix::fs::symlink(root.join("proj").join("57-sitemap.md"), &inner).unwrap();
    assert!(resolve_answer_ref(&s(&inner), &root).is_some());
}

#[test]
fn a_non_md_extension_is_kept_as_typed() {
    let (_tmp, root, _file) = answers_fixture();
    let txt = root.join("proj").join("57-sitemap.txt");
    std::fs::write(&txt, "nội dung").unwrap();
    assert_eq!(resolve_answer_ref(&s(&txt), &root), None);
    let noext = root.join("proj").join("noext");
    std::fs::write(&noext, "nội dung").unwrap();
    assert_eq!(resolve_answer_ref(&s(&noext), &root), None);
}

#[test]
fn multiple_lines_or_extra_words_are_kept_as_typed() {
    let (_tmp, root, file) = answers_fixture();
    let p = s(&file);
    assert_eq!(resolve_answer_ref(&format!("{p}\n{p}"), &root), None);
    assert_eq!(resolve_answer_ref(&format!("{p}\nghi chú"), &root), None);
    assert_eq!(resolve_answer_ref(&format!("xem file {p}"), &root), None);
    assert_eq!(resolve_answer_ref(&format!("{p} và file kia"), &root), None);
    assert_eq!(resolve_answer_ref("", &root), None);
    assert_eq!(resolve_answer_ref("   \n ", &root), None);
}

#[test]
fn a_directory_a_missing_file_or_an_empty_file_is_kept_as_typed() {
    let (_tmp, root, _file) = answers_fixture();
    let dir = root.join("proj").join("dir.md");
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(resolve_answer_ref(&s(&dir), &root), None);
    assert_eq!(resolve_answer_ref(&s(&root.join("proj").join("khong-co.md")), &root), None);
    let empty = root.join("proj").join("empty.md");
    std::fs::write(&empty, "  \n").unwrap();
    assert_eq!(resolve_answer_ref(&s(&empty), &root), None);
    // Thư mục `answers` không tồn tại: không thay gì.
    assert_eq!(resolve_answer_ref("/nonexistent-answer-ref-test/answers/x.md", &root.join("nope")), None);
}

#[test]
fn a_file_over_the_size_limit_is_kept_as_typed() {
    let (_tmp, root, _file) = answers_fixture();
    let big = root.join("proj").join("big.md");
    std::fs::write(&big, "a".repeat(MAX_INPUT_BYTES + 1)).unwrap();
    assert_eq!(resolve_answer_ref(&s(&big), &root), None);
    // Đúng bằng giới hạn thì còn nhận.
    let edge = root.join("proj").join("edge.md");
    std::fs::write(&edge, "a".repeat(MAX_INPUT_BYTES)).unwrap();
    assert_eq!(resolve_answer_ref(&s(&edge), &root).map(|t| t.len()), Some(MAX_INPUT_BYTES));
    // Chuỗi người dùng dán nguyên văn vẫn là chuỗi (giữ nguyên) nên không bị lỗi kích thước nào thêm.
    let items = vec![InputItem { name: "big.md".into(), content: s(&big) }];
    assert!(validate_inputs(&items, &[]).is_ok());
}

#[test]
fn a_file_with_a_known_secret_is_rejected_like_any_document() {
    let (_tmp, root, _file) = answers_fixture();
    let leaky = root.join("proj").join("leak.md");
    std::fs::write(&leaky, "kết quả kèm khoá sk-test-abcdef123456 rò rỉ").unwrap();
    let secrets = vec!["sk-test-abcdef123456".to_string()];
    let content = resolve_answer_ref(&s(&leaky), &root).expect("path resolves to the content");
    let items = vec![InputItem { name: "leak.md".into(), content }];
    let err = validate_inputs(&items, &secrets).unwrap_err();
    assert!(err.contains("bí mật"), "{err}");
}

fn intake_docs(sb: &Sandbox, intake_id: i64) -> Vec<(String, String)> {
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let mut stmt = conn
        .prepare("SELECT name, content FROM intake_inputs WHERE intake_id = ?1 ORDER BY id")
        .unwrap();
    let rows = stmt
        .query_map([intake_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .unwrap();
    rows.map(|r| r.unwrap()).collect()
}

/// Ghi file trả lời vào `answers/` của sandbox (thư mục dữ liệu tạm của serve).
fn sandbox_answer(sb: &Sandbox, name: &str, content: &str) -> PathBuf {
    let dir = sb.xdg_data.join("agent-brain").join("answers").join("web-ban-hang");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join(name);
    std::fs::write(&f, content).unwrap();
    f
}

#[test]
fn create_intake_over_http_stores_the_file_content_for_an_answer_path() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let srv = sb.serve(&[]);

    let answer = sandbox_answer(&sb, "57-cnctech-sitemap.md", "# Sitemap\n- https://example.test/\n");
    let outside = sb.root.join("not-an-answer.md");
    std::fs::write(&outside, "ngoài answers").unwrap();

    let resp = srv.post_json(
        "/api/intake",
        &json!({
            "request": "Crawl website theo sitemap đính kèm",
            "inputs": [
                { "name": "cnctech-sitemap.md", "content": s(&answer) },
                { "name": "ngoai.md", "content": s(&outside) },
                { "name": "binh-thuong.md", "content": "# ghi chú thường" }
            ]
        }),
    );
    assert_eq!(resp.status, 201, "body: {}", resp.text());
    let id = resp.json()["id"].as_i64().unwrap();

    let docs = intake_docs(&sb, id);
    assert_eq!(docs.len(), 3);
    assert_eq!(docs[0].0, "cnctech-sitemap.md", "tên tài liệu không đổi");
    assert_eq!(docs[0].1, "# Sitemap\n- https://example.test/\n");
    assert_eq!(docs[1], ("ngoai.md".to_string(), s(&outside)), "ngoài answers: giữ nguyên chuỗi");
    assert_eq!(docs[2].1, "# ghi chú thường");
}

#[test]
fn create_task_queue_path_stores_the_file_content_for_an_answer_path() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    sb.write_config(json!({"orchestrator": {"enabled": true}}));
    let srv = sb.serve(&[]);

    let answer = sandbox_answer(&sb, "58-danh-sach.md", "# Danh sách URL\n- https://example.test/x\n");
    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "Kiểm tra các URL trong tài liệu",
            "inputs": [ { "name": "danh-sach.md", "content": s(&answer) } ]
        }),
    );
    assert_eq!(resp.status, 201, "body: {}", resp.text());
    let id = resp.json()["intake_id"].as_i64().unwrap();
    let docs = intake_docs(&sb, id);
    assert_eq!(docs, vec![("danh-sach.md".to_string(), "# Danh sách URL\n- https://example.test/x\n".to_string())]);
}

#[test]
fn create_intake_over_http_rejects_an_answer_file_that_holds_a_secret() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    // Biến môi trường chứa `token` của serve được coi là bí mật (inputs::env_secrets).
    let srv = sb.serve(&[("ANSWER_REF_TEST_TOKEN", "zz-secret-value-0987654321")]);

    let answer = sandbox_answer(&sb, "59-leak.md", "kết quả: zz-secret-value-0987654321");
    let resp = srv.post_json(
        "/api/intake",
        &json!({
            "request": "dùng tài liệu đính kèm",
            "inputs": [ { "name": "leak.md", "content": s(&answer) } ]
        }),
    );
    assert_eq!(resp.status, 422, "body: {}", resp.text());
    assert!(resp.text().contains("bí mật"), "{}", resp.text());
    // Không tạo intake nào.
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM intake", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
}
