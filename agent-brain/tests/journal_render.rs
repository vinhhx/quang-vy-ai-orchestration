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
