mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{RunOut, Sandbox};
use protocol::{lead_scenario, plan_text, setup_project};
use serde_json::{json, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Lead ghi một plan có đường dẫn `../ngoai.py` trong bảng Thay đổi (path_error).
fn invalid_lead_entry() -> Value {
    let p_text = plan_text(
        "Kế hoạch",
        &[("../ngoai.py", "1", "sửa", "lý do")],
        "python3 -m unittest",
        "",
    );
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "write", "path": ".plan.md", "content": p_text, "keep_meta": true}
    ]})
}

fn valid_lead_entry() -> Value {
    lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])
}

/// Repo `web-ban-hang` với entry không có khóa `workflow` (mặc định server: mac-dinh).
fn setup_auto_project(sb: &Sandbox, profile: &str) -> PathBuf {
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", profile);
    entry.as_object_mut().unwrap().remove("workflow");
    sb.write_map(json!([entry]), &[]);
    repo
}

fn run_lead(sb: &Sandbox, extra: &[&str], scenario: Value) -> RunOut {
    let mut args = vec!["run", "lead", "--name", "sua-app"];
    args.extend_from_slice(extra);
    args.extend_from_slice(&["--", "web-ban-hang", "sửa app"]);
    sb.run(&args, Some(scenario))
}

type LeadRow = (i64, String, Option<String>, Option<i64>, Option<String>);

/// (id, status, stage, round, outcome) của các run lead, theo id.
fn lead_runs(sb: &Sandbox) -> Vec<LeadRow> {
    let conn = sb.db();
    let mut stmt = conn
        .prepare(
            "SELECT id, status, stage, round, outcome FROM runs WHERE role = 'lead' ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

fn auto_json(sb: &Sandbox, task_id: i64) -> Value {
    let aj: Option<String> = sb
        .db()
        .query_row(
            "SELECT auto_json FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap();
    serde_json::from_str(&aj.unwrap_or_else(|| "{}".to_string())).unwrap()
}

// ---------------------------------------------------------------------------
// WR-02: Lead plan failures are recorded as protocol_violation
// ---------------------------------------------------------------------------

#[test]
fn invalid_plan_records_violation() {
    let sb = Sandbox::new();
    setup_project(&sb, "work");

    let out = run_lead(&sb, &[], invalid_lead_entry());
    assert_eq!(
        out.code, 1,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stderr.contains("Kế hoạch trong .plan.md không hợp lệ"),
        "stderr: {}",
        out.stderr
    );

    let leads = lead_runs(&sb);
    assert_eq!(leads.len(), 1, "lead runs: {leads:?}");
    assert_eq!(leads[0].1, "succeeded");
    assert_eq!(leads[0].4.as_deref(), Some("protocol_violation"));
}

#[test]
fn autopilot_invalid_plan_stops_once() {
    let sb = Sandbox::new();
    setup_auto_project(&sb, "work");

    let sc = json!({ "by_role": { "lead": [invalid_lead_entry()] } });
    let res = sb.run(
        &[
            "auto",
            "--start",
            "--name",
            "sua-app",
            "--workflow",
            "mac-dinh",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(sc),
    );
    assert_eq!(
        res.code, 4,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let leads = lead_runs(&sb);
    assert_eq!(leads.len(), 1, "lead runs: {leads:?}");
    assert_eq!(leads[0].4.as_deref(), Some("protocol_violation"));

    let stop = auto_json(&sb, 1)["stop"].clone();
    assert_eq!(stop["reason"], "vi_pham_giao_thuc", "stop: {stop}");
    assert_eq!(stop["run_id"], leads[0].0, "stop: {stop}");
}

#[test]
fn valid_plan_unchanged() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let out = run_lead(&sb, &[], valid_lead_entry());
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let leads = lead_runs(&sb);
    assert_eq!(leads.len(), 1, "lead runs: {leads:?}");
    assert_eq!(leads[0].1, "succeeded");
    assert_eq!(leads[0].4, None);

    let head_subject = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subject.trim(), "sua-app: plan");
    let status = sb.git(&repo, &["status", "--porcelain"]);
    assert_eq!(status.trim(), "");
}

// ---------------------------------------------------------------------------
// IN-02: manual Lead runs store their round
// ---------------------------------------------------------------------------

#[test]
fn manual_first_lead_stores_round_1() {
    let sb = Sandbox::new();
    setup_project(&sb, "work");

    let out = run_lead(&sb, &[], valid_lead_entry());
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let leads = lead_runs(&sb);
    assert_eq!(leads.len(), 1, "lead runs: {leads:?}");
    assert_eq!(leads[0].2.as_deref(), Some("plan"));
    assert_eq!(leads[0].3, Some(1));
}

#[test]
fn manual_replan_stores_round_2() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let first = run_lead(&sb, &[], valid_lead_entry());
    assert_eq!(first.code, 0, "stderr: {}", first.stderr);
    let branch = sb.git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(branch.trim(), "task/sua-app");

    let second = run_lead(&sb, &["--note", "thêm"], valid_lead_entry());
    assert_eq!(
        second.code, 0,
        "stderr: {}\nstdout: {}",
        second.stderr, second.stdout
    );

    let leads = lead_runs(&sb);
    assert_eq!(leads.len(), 2, "lead runs: {leads:?}");
    assert_eq!(leads[0].3, Some(1));
    assert_eq!(leads[1].2.as_deref(), Some("plan"));
    assert_eq!(leads[1].3, Some(2));
}

#[test]
fn manual_lead_after_qa_new_cycle() {
    let sb = Sandbox::new();
    setup_project(&sb, "work");

    let first = run_lead(&sb, &[], valid_lead_entry());
    assert_eq!(first.code, 0, "stderr: {}", first.stderr);

    {
        let conn = sb.db();
        let (task_id, project_id, profile, started_at): (i64, i64, String, i64) = conn
            .query_row(
                "SELECT task_id, project_id, profile, started_at FROM runs WHERE role = 'lead'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO runs (task_id, project_id, role, profile, status, outcome, started_at, ended_at, stage, round)
             VALUES (?1, ?2, 'dev', ?3, 'succeeded', NULL, ?4, ?4, 'implement', 1)",
            rusqlite::params![task_id, project_id, profile, started_at + 1],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs (task_id, project_id, role, profile, status, outcome, started_at, ended_at, stage, round)
             VALUES (?1, ?2, 'qa', ?3, 'succeeded', 'qa:fix', ?4, ?4, 'qa', 1)",
            rusqlite::params![task_id, project_id, profile, started_at + 2],
        )
        .unwrap();
    }

    let again = run_lead(&sb, &["--note", "lại"], valid_lead_entry());
    assert_eq!(
        again.code, 0,
        "stderr: {}\nstdout: {}",
        again.stderr, again.stdout
    );

    let leads = lead_runs(&sb);
    assert_eq!(leads.len(), 2, "lead runs: {leads:?}");
    assert_eq!(leads[1].2.as_deref(), Some("plan"));
    assert_eq!(leads[1].3, Some(1), "new cycle restarts at round 1");
}

#[test]
fn explicit_round_wins() {
    let sb = Sandbox::new();
    setup_project(&sb, "work");

    let out = run_lead(&sb, &["--round", "5"], valid_lead_entry());
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let leads = lead_runs(&sb);
    assert_eq!(leads.len(), 1, "lead runs: {leads:?}");
    assert_eq!(leads[0].3, Some(5));
}
