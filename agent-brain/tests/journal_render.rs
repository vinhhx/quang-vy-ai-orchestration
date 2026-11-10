//! Phase 7 plan 03: the full journal content (D-02, D-16, D-17) for one synthetic fixture:
//! attention categories with precedence, approvals as activity, carry-over into today, groups
//! from `journal_group`, and the rendered file set. Every title and project is invented.

mod common;
#[allow(dead_code)]
#[path = "support/journal.rs"]
mod jsupport;

use agent_brain::journal::collect::collect;
use agent_brain::journal::config::JournalConfig;
use agent_brain::journal::model::{AttentionKind, Model, StatusKind};
use chrono::{FixedOffset, NaiveDate};
use common::Sandbox;
use jsupport::*;
use rusqlite::Connection;
use std::collections::BTreeMap;

fn tz() -> FixedOffset {
    FixedOffset::east_opt(7 * 3600).unwrap()
}

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 1, d).unwrap()
}

fn today() -> NaiveDate {
    day(15)
}

fn at(d: u32, h: u32, mi: u32, s: u32) -> i64 {
    local_ms(2026, 1, d, h, mi, s)
}

#[allow(clippy::too_many_arguments)]
fn run(
    conn: &Connection,
    task: i64,
    role: &str,
    status: &str,
    outcome: Option<&str>,
    d: u32,
    from: (u32, u32),
    to: (u32, u32),
) {
    seed_run(
        conn,
        task,
        role,
        status,
        outcome,
        at(d, from.0, from.1, 0),
        Some(at(d, to.0, to.1, 0)),
        Some(1),
    );
}

/// Eleven tasks over 13..15 January 2026 (+07:00); task ids equal the numbers of the plan.
fn fixture(sb: &Sandbox) -> Connection {
    let conn = setup_projects(
        sb,
        &[
            ("du-an-a", Some("video")),
            ("du-an-b", None),
            ("du-an-c", Some("khong-co")),
            ("du-an-d", Some("ho-tro")),
        ],
    );

    // 1: finished on 13/01.
    let t1 = seed_task(
        &conn,
        "du-an-a",
        "dung-video",
        "Dựng video giới thiệu sản phẩm mẫu cho khách hàng",
    );
    seed_proposal(&conn, t1, "Dựng video giới thiệu sản phẩm mẫu");
    run(&conn, t1, "lead", "succeeded", None, 13, (9, 0), (9, 5));
    run(&conn, t1, "dev", "succeeded", None, 13, (9, 10), (9, 30));
    run(
        &conn,
        t1,
        "qa",
        "succeeded",
        Some("qa:pass"),
        13,
        (9, 31),
        (9, 40),
    );

    // 2: plan waiting on 13/01, approved just after midnight, QA wants a fix on 14/01.
    let t2 = seed_task(
        &conn,
        "du-an-b",
        "sua-dang-nhap",
        "Sửa lỗi đăng nhập trang quản trị\nkhi mật khẩu có ký tự đặc biệt",
    );
    seed_run(
        &conn,
        t2,
        "lead",
        "succeeded",
        None,
        at(13, 23, 50, 0),
        Some(at(13, 23, 59, 59)),
        Some(1),
    );
    agent_brain::db::insert_plan_approval(&conn, t2, "abc1234", at(14, 0, 0, 1), "ui").unwrap();
    run(&conn, t2, "dev", "succeeded", None, 14, (8, 10), (8, 40));
    run(
        &conn,
        t2,
        "qa",
        "succeeded",
        Some("qa:fix"),
        14,
        (8, 45),
        (8, 50),
    );

    // 3: quota on 14/01 (status failed, outcome quota); unknown group id.
    let t3 = seed_task(&conn, "du-an-c", "so-lieu", "Chuẩn bị số liệu báo cáo tuần");
    run(
        &conn,
        t3,
        "dev",
        "failed",
        Some("quota"),
        14,
        (10, 0),
        (10, 5),
    );

    // 4: analyst answer on 15/01.
    let t4 = seed_task(
        &conn,
        "du-an-d",
        "tra-cuu",
        "Hỗ trợ phòng kinh doanh tra cứu đơn hàng",
    );
    run(
        &conn,
        t4,
        "analyst",
        "succeeded",
        None,
        15,
        (11, 0),
        (11, 20),
    );

    // 5: protocol violation on a succeeded run.
    let t5 = seed_task(
        &conn,
        "du-an-b",
        "trang-gioi-thieu",
        "Cập nhật trang giới thiệu công ty",
    );
    run(
        &conn,
        t5,
        "dev",
        "succeeded",
        Some("protocol_violation"),
        15,
        (12, 0),
        (12, 30),
    );

    // 6: timed out.
    let t6 = seed_task(
        &conn,
        "du-an-a",
        "nhac-nen",
        "Ghép nhạc nền cho video hướng dẫn",
    );
    run(&conn, t6, "dev", "timed_out", None, 15, (13, 0), (13, 40));

    // 7: environment not ready.
    let t7 = seed_task(
        &conn,
        "du-an-b",
        "moi-truong",
        "Kiểm tra môi trường thử nghiệm",
    );
    run(
        &conn,
        t7,
        "dev",
        "failed",
        Some("env:down"),
        15,
        (14, 0),
        (14, 1),
    );

    // 8: blocked.
    let t8 = seed_task(&conn, "du-an-c", "ra-soat", "Rà soát tài liệu hướng dẫn");
    run(
        &conn,
        t8,
        "dev",
        "succeeded",
        Some("dev:blocked"),
        15,
        (15, 0),
        (15, 20),
    );

    // 9: killed, not attention.
    let t9 = seed_task(&conn, "du-an-a", "xuat-ban", "Xuất bản video tháng trước");
    run(&conn, t9, "dev", "killed", None, 14, (16, 0), (16, 10));

    // 10: lead done on 15/01 and the plan approved the same morning.
    let t10 = seed_task(&conn, "du-an-d", "ke-hoach", "Lập kế hoạch nội dung tuần");
    run(&conn, t10, "lead", "succeeded", None, 15, (9, 0), (9, 10));
    agent_brain::db::set_task_approval(&conn, t10, at(15, 10, 0, 0), "def5678").unwrap();

    // 11: failed on 13/01, fixed and QA passed on 14/01.
    let t11 = seed_task(&conn, "du-an-b", "hoa-don", "Sửa lỗi xuất hóa đơn");
    run(&conn, t11, "dev", "failed", None, 13, (14, 0), (14, 30));
    run(&conn, t11, "dev", "succeeded", None, 14, (9, 0), (9, 30));
    run(
        &conn,
        t11,
        "qa",
        "succeeded",
        Some("qa:pass"),
        14,
        (9, 35),
        (9, 45),
    );

    assert_eq!((t1, t2, t3, t4, t5, t6), (1, 2, 3, 4, 5, 6));
    assert_eq!((t7, t8, t9, t10, t11), (7, 8, 9, 10, 11));
    conn
}

fn model(sb: &Sandbox, conn: &Connection) -> Model {
    collect(
        conn,
        &sb.read_map(),
        &JournalConfig::default(),
        &[],
        today(),
        &tz(),
    )
    .expect("collect")
}

fn attention_of(m: &Model, d: NaiveDate) -> Vec<(AttentionKind, i64, NaiveDate)> {
    m.attention
        .get(&d)
        .map(|items| items.iter().map(|i| (i.kind, i.task_id, i.day)).collect())
        .unwrap_or_default()
}

#[test]
fn attention_by_day() {
    let sb = Sandbox::new();
    let conn = fixture(&sb);
    let m = model(&sb, &conn);
    use AttentionKind::*;

    assert_eq!(
        attention_of(&m, day(13)),
        vec![(PlanWaiting, 2, day(13)), (Failed, 11, day(13))]
    );
    assert_eq!(
        attention_of(&m, day(14)),
        vec![(Quota, 3, day(14)), (QaFix, 2, day(14))]
    );
    assert_eq!(
        attention_of(&m, day(15)),
        vec![
            (Quota, 3, day(14)),
            (EnvDown, 7, day(15)),
            (QaFix, 2, day(14)),
            (Blocked, 8, day(15)),
            (ProtocolViolation, 5, day(15)),
            (Timeout, 6, day(15)),
        ]
    );
}

#[test]
fn killed_and_approved_are_not_attention() {
    let sb = Sandbox::new();
    let conn = fixture(&sb);
    let m = model(&sb, &conn);

    for items in m.attention.values() {
        for i in items {
            assert!(i.task_id != 9 && i.task_id != 10, "{i:?}");
        }
    }
    let t10 = m.days[&day(15)]
        .iter()
        .find(|t| t.task_id == 10)
        .expect("task 10 on 15/01");
    assert_eq!(t10.status.kind, StatusKind::Active);
    assert_eq!(t10.status.label, "đang làm");
    let t9 = m.days[&day(14)]
        .iter()
        .find(|t| t.task_id == 9)
        .expect("task 9 on 14/01");
    assert_eq!(t9.status.kind, StatusKind::Stopped);
    assert_eq!(t9.status.label, "đã dừng");
}

#[test]
fn groups_from_journal_group_only() {
    let sb = Sandbox::new();
    let conn = fixture(&sb);
    let m = model(&sb, &conn);

    let mut group_of: BTreeMap<i64, String> = BTreeMap::new();
    for tasks in m.days.values() {
        for t in tasks {
            group_of.insert(t.task_id, t.project.group_label.clone());
        }
    }
    assert_eq!(group_of[&3], "Công việc");
    assert_eq!(group_of[&8], "Công việc");
    assert_eq!(group_of[&2], "Công việc");
    assert_eq!(group_of[&4], "Hỗ trợ bộ phận khác");
    assert_eq!(group_of[&1], "Video");
}

#[test]
fn approval_days_are_listed() {
    let sb = Sandbox::new();
    let conn = fixture(&sb);
    let m = model(&sb, &conn);
    // Task 2's plan was approved on 14/01 (it also ran that day); task 10 on 15/01.
    assert!(m.days[&day(14)].iter().any(|t| t.task_id == 2));
    assert!(m.days[&day(15)].iter().any(|t| t.task_id == 10));
}

// ---- rendered file set -------------------------------------------------------------------

use agent_brain::journal::model::MARKER;
use agent_brain::journal::render::render_all;
use std::path::PathBuf;
use unicode_normalization::UnicodeNormalization;

fn rendered(sb: &Sandbox, conn: &Connection) -> BTreeMap<String, String> {
    render_all(&model(sb, conn), &JournalConfig::default())
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/journal/golden")
}

const EXPECTED_KEYS: &[&str] = &[
    "Dự án/du-an-a.md",
    "Dự án/du-an-b.md",
    "Dự án/du-an-c.md",
    "Dự án/du-an-d.md",
    "Nhật ký/2026/01/2026-01-13.md",
    "Nhật ký/2026/01/2026-01-14.md",
    "Nhật ký/2026/01/2026-01-15.md",
    "Nhật ký/2026/01/2026-01.md",
    "Nhật ký/2026/2026.md",
    "Today.md",
];

fn collect_golden(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries {
        let p = e.unwrap().path();
        if p.is_dir() {
            collect_golden(&p, base, out);
        } else {
            let rel = p.strip_prefix(base).unwrap().to_str().unwrap();
            out.push(rel.nfc().collect());
        }
    }
}

#[test]
fn golden_files_match() {
    let sb = Sandbox::new();
    let conn = fixture(&sb);
    let files = rendered(&sb, &conn);
    let keys: Vec<&str> = files.keys().map(String::as_str).collect();
    let mut want: Vec<String> = EXPECTED_KEYS.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(keys, want);

    let dir = golden_dir();
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        let mut stale = Vec::new();
        collect_golden(&dir, &dir, &mut stale);
        for rel in stale {
            if !files.contains_key(&rel) {
                std::fs::remove_file(dir.join(&rel)).unwrap();
            }
        }
        for (rel, body) in &files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, body).unwrap();
        }
        return;
    }
    for (rel, body) in &files {
        let golden = std::fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| {
            panic!("golden file {rel} missing ({e}); run with UPDATE_GOLDEN=1")
        });
        assert_eq!(&golden, body, "golden mismatch for {rel}");
    }
    let mut on_disk = Vec::new();
    collect_golden(&dir, &dir, &mut on_disk);
    on_disk.sort();
    assert_eq!(
        on_disk, want,
        "golden directory has files outside the rendered set"
    );
}

#[test]
fn day_file_has_attention_first() {
    let sb = Sandbox::new();
    let conn = fixture(&sb);
    let files = rendered(&sb, &conn);

    let d15: Vec<&str> = files["Nhật ký/2026/01/2026-01-15.md"].lines().collect();
    let h = d15
        .iter()
        .position(|l| l.starts_with("# Thứ Năm 15/01/2026"))
        .expect("day heading");
    assert_eq!(d15[h + 1], "");
    assert_eq!(d15[h + 2], "## Cần anh xử lý (6)");
    let items = &d15[h + 3..h + 9];
    let kinds: Vec<&str> = items
        .iter()
        .map(|l| l.split("**").nth(1).expect("label"))
        .collect();
    assert_eq!(
        kinds,
        [
            "hết quota",
            "môi trường chưa sẵn sàng",
            "QA cần sửa",
            "vướng mắc",
            "vi phạm giao thức",
            "quá thời gian"
        ]
    );
    for (i, l) in items.iter().enumerate() {
        assert!(l.starts_with("- **"), "{l}");
        assert!(l.contains(" — [[Dự án/du-an-"), "{l}");
        // Quota (task 3) and QA fix (task 2) are carried over from 14/01.
        assert_eq!(l.ends_with(" · từ 14/01"), i == 0 || i == 2, "{l}");
    }
    assert_eq!(d15[h + 9], "");

    let d13: Vec<&str> = files["Nhật ký/2026/01/2026-01-13.md"].lines().collect();
    let h = d13
        .iter()
        .position(|l| l.starts_with("# Thứ Ba 13/01/2026"))
        .expect("day heading");
    assert_eq!(d13[h + 2], "## Cần anh xử lý (2)");
    assert!(d13[h + 3].starts_with("- **chờ duyệt plan** — [[Dự án/du-an-b|du-an-b]] — "));
    assert!(d13[h + 4].starts_with("- **lỗi** — "));
}

#[test]
fn today_embeds_today() {
    let sb = Sandbox::new();
    let conn = fixture(&sb);
    let files = rendered(&sb, &conn);
    let lines: Vec<&str> = files["Today.md"].lines().collect();
    assert_eq!(lines[0], MARKER);
    assert_eq!(lines[1], "# Hôm nay — Thứ Năm 15/01/2026");
    assert!(lines.contains(&"![[2026-01-15]]"));
    assert!(lines.contains(&"Hôm qua: [[2026-01-14]] · Tháng: [[2026-01]] · Năm: [[2026]]"));
}

fn link_targets(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find("[[") {
        let after = &rest[at + 2..];
        let end = after.find("]]").expect("closed wikilink");
        let inner = &after[..end];
        out.push(inner.split('|').next().unwrap().to_string());
        rest = &after[end + 2..];
    }
    out
}

#[test]
fn all_links_resolve() {
    let sb = Sandbox::new();
    let conn = fixture(&sb);
    let files = rendered(&sb, &conn);
    let mut checked = 0;
    for (path, body) in &files {
        for target in link_targets(body) {
            let found = if target.contains('/') {
                files.contains_key(&format!("{target}.md"))
            } else {
                files.keys().any(|k| {
                    k.rsplit('/').next().and_then(|n| n.strip_suffix(".md")) == Some(&target)
                })
            };
            assert!(found, "dangling link [[{target}]] in {path}");
            checked += 1;
        }
    }
    assert!(checked > 30, "only {checked} links checked");
}

#[test]
fn no_weekly_file_and_nfc_paths() {
    let sb = Sandbox::new();
    let conn = fixture(&sb);
    let files = rendered(&sb, &conn);
    for key in files.keys() {
        assert!(
            !key.contains("Tuần") && !key.to_lowercase().contains("tuan"),
            "{key}"
        );
        assert!(!key.contains("-W") && !key.contains("/W"), "{key}");
        assert!(key.nfc().eq(key.chars()), "{key} is not NFC");
        let name = key.rsplit('/').next().unwrap();
        assert!(name.is_ascii(), "{key}: file name is not ASCII");
    }
}

#[test]
fn render_is_deterministic() {
    let sb = Sandbox::new();
    let conn = fixture(&sb);
    let a = rendered(&sb, &conn);
    let b = rendered(&sb, &conn);
    assert_eq!(a, b);
    for (path, body) in &a {
        assert!(!body.contains('\r'), "{path}");
        assert!(body.ends_with('\n') && !body.ends_with("\n\n"), "{path}");
        // Frontmatter carries only dates, counts and fixed tags.
        assert!(
            !body.contains("title:") && !body.contains("request:"),
            "{path}"
        );
    }
}
