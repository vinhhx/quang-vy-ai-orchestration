//! Phase 7 plan 02: tracer of `agent-brain journal export` — brain.db → day file in the demo
//! format (D-17) → commit with the fixed author → push to a local bare repository (D-09, D-11).
//! Texts and projects are invented (`du-an-a`).

mod common;
#[allow(dead_code)]
#[path = "support/journal.rs"]
mod jsupport;

use common::Sandbox;
use jsupport::*;
use serde_json::json;
use std::path::PathBuf;

const TITLE: &str = "Dựng video giới thiệu sản phẩm mẫu";
const REQUEST: &str = "Dựng video giới thiệu sản phẩm mẫu\nthêm phụ đề tiếng Việt cho từng cảnh";

/// Project `du-an-a` in group `video`, one task with a proposal and three finished runs on
/// 2026-01-15 (local): lead 09:00-09:05, dev 09:10-09:30, qa 09:31-09:40 (`qa:pass`), round 1.
fn tracer_fixture(sb: &Sandbox) -> PathBuf {
    let remote = bare_remote(sb, "vault");
    sb.write_config(json!({"journal": journal_section(&remote)}));
    let conn = setup_projects(sb, &[("du-an-a", Some("video"))]);
    let task = seed_task(&conn, "du-an-a", "dung-video", REQUEST);
    seed_proposal(&conn, task, TITLE);
    seed_run(
        &conn,
        task,
        "lead",
        "succeeded",
        None,
        local_ms(2026, 1, 15, 9, 0, 0),
        Some(local_ms(2026, 1, 15, 9, 5, 0)),
        Some(1),
    );
    seed_run(
        &conn,
        task,
        "dev",
        "succeeded",
        None,
        local_ms(2026, 1, 15, 9, 10, 0),
        Some(local_ms(2026, 1, 15, 9, 30, 0)),
        Some(1),
    );
    seed_run(
        &conn,
        task,
        "qa",
        "succeeded",
        Some("qa:pass"),
        local_ms(2026, 1, 15, 9, 31, 0),
        Some(local_ms(2026, 1, 15, 9, 40, 0)),
        Some(1),
    );
    remote
}

fn now() -> i64 {
    local_ms(2026, 1, 15, 20, 0, 0)
}

const DAY_FILE: &str = "Nhật ký/2026/01/2026-01-15.md";

#[test]
fn tracer_export_pushes_one_day_file() {
    let sb = Sandbox::new();
    let remote = tracer_fixture(&sb);

    let out = export(&sb, &[], now());
    assert_eq!(
        out.code, 0,
        "stdout: {}\nstderr: {}",
        out.stdout, out.stderr
    );
    assert!(clone_dir(&sb).join(".git").exists());

    let commits = remote_commits(&sb, &remote);
    assert_eq!(commits.len(), 1, "{commits:?}");
    assert!(
        commits[0].starts_with("agent-brain journal|"),
        "{commits:?}"
    );

    let co = checkout(&sb, &remote);
    let text = std::fs::read_to_string(co.join(DAY_FILE)).expect("day file in the remote");
    assert!(!text.contains('\r'));
    let lines: Vec<&str> = text.lines().collect();
    for want in [
        "date: 2026-01-15",
        "tasks: 1",
        "tags: [nhat-ky]",
        agent_brain::journal::model::MARKER,
        "# Thứ Năm 15/01/2026 — 1 việc (1 xong)",
        "## Video (1)",
        "> [!success]- ✓ du-an-a — Dựng video giới thiệu sản phẩm mẫu",
        "> **Trạng thái:** xong · 34 phút",
        "> **Dự án:** [[Dự án/du-an-a|du-an-a]] · **Nhóm:** Video · **Số bước chạy:** 3 · **Vòng:** 1",
        "> **Vai đã chạy:** Lead, Dev, QA",
        "> Dựng video giới thiệu sản phẩm mẫu thêm phụ đề tiếng Việt cho từng cảnh",
    ] {
        assert!(lines.contains(&want), "missing line {want:?} in:\n{text}");
    }
}

#[test]
fn second_export_makes_no_commit() {
    let sb = Sandbox::new();
    let remote = tracer_fixture(&sb);

    let first = export(&sb, &[], now());
    assert_eq!(first.code, 0, "{}", first.stderr);
    let second = export(&sb, &[], now());
    assert_eq!(second.code, 0, "{}", second.stderr);

    assert_eq!(remote_commits(&sb, &remote).len(), 1);
    let status = sb.git(&clone_dir(&sb), &["status", "--porcelain"]);
    assert!(status.trim().is_empty(), "{status}");
}

#[test]
fn title_without_proposal_uses_cut_request() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    sb.write_config(json!({"journal": journal_section(&remote)}));
    let conn = setup_projects(&sb, &[("du-an-b", None)]);

    let line1 = "Sửa trang danh mục [[x]] theo mẫu mới #tag cho phần đầu trang";
    let line2 = "chạy lệnh `kiểm tra` rồi xem lại bố cục trên điện thoại";
    let mut request = format!("{line1}\n{line2}\n");
    while request.chars().count() < 300 {
        request.push_str("thêm nội dung mô tả ");
    }
    let request: String = request.chars().take(300).collect();
    assert_eq!(request.chars().count(), 300);
    assert_eq!(request.lines().count(), 3);

    let task = seed_task(&conn, "du-an-b", "sua-danh-muc", &request);
    seed_run(
        &conn,
        task,
        "dev",
        "succeeded",
        None,
        local_ms(2026, 1, 15, 10, 0, 0),
        Some(local_ms(2026, 1, 15, 10, 20, 0)),
        Some(1),
    );

    let out = export(&sb, &[], now());
    assert_eq!(out.code, 0, "{}", out.stderr);
    let co = checkout(&sb, &remote);
    let text = std::fs::read_to_string(co.join(DAY_FILE)).unwrap();
    let lines: Vec<&str> = text.lines().collect();

    let title_line = lines
        .iter()
        .find(|l| l.starts_with("> [!"))
        .expect("callout line");
    let title = title_line
        .split_once(" — ")
        .map(|(_, t)| t)
        .expect("title after the dash");
    let body_at = lines
        .iter()
        .position(|l| *l == ">")
        .expect("blank callout line")
        + 1;
    let body = lines[body_at].strip_prefix("> ").expect("body line");

    assert!(
        body.chars().count() <= 140,
        "{} chars: {body}",
        body.chars().count()
    );
    assert!(
        body.contains("Sửa trang danh mục") && body.contains("chạy lệnh"),
        "{body}"
    );
    assert!(title.starts_with("Sửa trang danh mục"), "{title}");
    for part in [title, body] {
        for bad in ["[[x]]", "#tag", "`"] {
            assert!(!part.contains(bad), "{bad} in {part}");
        }
    }
    // The request's second line is not a line of its own.
    assert!(!lines.iter().any(|l| l.starts_with("chạy lệnh")));
}

#[test]
fn export_without_journal_config_refuses() {
    let sb = Sandbox::new();
    let out = export(&sb, &[], now());
    assert_eq!(
        out.code, 1,
        "stdout: {}\nstderr: {}",
        out.stdout, out.stderr
    );
    assert!(out.stderr.contains("journal.repo_url"), "{}", out.stderr);
    assert!(!clone_dir(&sb).exists());
}
