//! Plan 05.2-26: mọi ghi trạng thái autopilot gắn với trạng thái đã kiểm tra
//! (WR-04) và thân 500 cố định trên các route của task (W-3).

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{wait_until, RunOut, Sandbox};
use protocol::lead_scenario;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers (copied from tests/dashboard_autopilot.rs)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn wait_for_job(srv: &common::Serve, job_id: u64) -> Value {
    let mut job_view = None;
    let ok = wait_until(Duration::from_secs(30), || {
        let r = srv.get(&format!("/api/jobs/{job_id}"));
        if r.status == 200 {
            let j = r.json();
            if j["state"] == "exited" {
                job_view = Some(j);
                return true;
            }
        }
        false
    });
    assert!(ok, "job {job_id} did not exit within timeout");
    job_view.unwrap()
}

fn review_report(verdict: &str, blocking: i64, advisory: i64) -> String {
    let mut body = format!(
        "---\nverdict: {verdict}\nblocking: {blocking}\nadvisory: {advisory}\nfiles: []\n---\n# Review: sửa app\n## Mục chặn (blocking)\n"
    );
    for i in 0..blocking {
        body.push_str(&format!("- mục chặn {}\n", i + 1));
    }
    body.push_str("## Góp ý (advisory)\n");
    for i in 0..advisory {
        body.push_str(&format!("- góp ý {}\n", i + 1));
    }
    body
}

fn plan_review_entry(verdict: &str, blocking: i64, advisory: i64) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan-review.md",
        "content": review_report(verdict, blocking, advisory),
    }]})
}

fn lead_plan_entry() -> Value {
    lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])
}

fn lead_append_entry() -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan.md",
        "content": "\n<!-- v2 -->\n",
        "append": true,
    }]})
}

fn by_role(roles: Value) -> Value {
    json!({ "by_role": roles })
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

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

/// `auto --start` trên quy trình mặc định (mac-dinh): Lead, review plan duyệt, dừng ở cổng.
#[allow(dead_code)]
fn auto_to_gate(sb: &Sandbox) -> PathBuf {
    let repo = setup_auto_project(sb, "work");
    let sc = by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry("approve", 0, 2)],
    }));
    let res = sb.run(
        &[
            "auto",
            "--start",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(sc),
    );
    assert_ok(&res, "auto --start");
    repo
}

fn task_auto_state(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT auto_state FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
}

fn task_auto_pid(sb: &Sandbox, task_id: i64) -> Option<i64> {
    sb.db()
        .query_row("SELECT auto_pid FROM tasks WHERE id = ?1", [task_id], |r| {
            r.get(0)
        })
        .unwrap()
}

fn task_auto_json(sb: &Sandbox, task_id: i64) -> Value {
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

/// mac-dinh qua `auto --start` (CLI): review plan luôn revise → dừng `hết vòng` ở vòng 3 / 3.
fn plan_cap_stopped(sb: &Sandbox) {
    setup_auto_project(sb, "work");
    let sc = by_role(json!({
        "lead": [lead_plan_entry(), lead_append_entry()],
        "plan-review": [
            plan_review_entry("revise", 1, 0),
            plan_review_entry("revise", 1, 1),
            plan_review_entry("revise", 1, 2)
        ],
    }));
    let res = sb.run(
        &[
            "auto",
            "--start",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(sc),
    );
    assert_eq!(res.code, 4, "stderr: {}", res.stderr);
    assert_eq!(task_auto_state(sb, 1).as_deref(), Some("stopped"));
    let stop = task_auto_json(sb, 1)["stop"].clone();
    assert_eq!(stop["reason"], "het_vong", "{stop}");
    assert_eq!(stop["loop_name"], "plan", "{stop}");
}

// ---------------------------------------------------------------------------
// Task 1: Dừng hẳn chỉ ghi trạng thái đã kiểm tra
// ---------------------------------------------------------------------------

#[test]
fn auto_end_ok_then_duplicate() {
    let sb = Sandbox::new();
    plan_cap_stopped(&sb);
    let mut srv = sb.serve(&[]);

    let resp = srv.post_json("/api/tasks/1/auto/end", &json!({}));
    assert_eq!(resp.status, 200, "{}", resp.text());
    assert_eq!(resp.json()["ok"], true);
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("ended"));
    assert_eq!(task_auto_pid(&sb, 1), None);
    // auto_json.stop giữ lại làm lịch sử.
    assert_eq!(task_auto_json(&sb, 1)["stop"]["reason"], "het_vong");

    let again = srv.post_json("/api/tasks/1/auto/end", &json!({}));
    assert_eq!(again.status, 409, "{}", again.text());
    assert_eq!(again.json()["kind"], "duplicate", "{}", again.text());
    assert_eq!(task_auto_state(&sb, 1).as_deref(), Some("ended"));

    srv.stop(libc::SIGTERM, Duration::from_secs(10));
}

// ---------------------------------------------------------------------------
// Task 2: Trả về Lead bỏ duyệt sau khi job đã bắt đầu
// ---------------------------------------------------------------------------

fn approved_commit(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT approved_commit FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
}

fn run_metrics(sb: &Sandbox, run_id: i64) -> Value {
    let m: Option<String> = sb
        .db()
        .query_row(
            "SELECT metrics_json FROM runs WHERE id = ?1",
            [run_id],
            |r| r.get(0),
        )
        .unwrap();
    serde_json::from_str(&m.unwrap_or_else(|| "{}".to_string())).unwrap()
}

fn lead_run_ids(sb: &Sandbox) -> Vec<i64> {
    let conn = sb.db();
    let mut stmt = conn
        .prepare("SELECT id FROM runs WHERE role = 'lead' ORDER BY id")
        .unwrap();
    let ids = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<i64>, _>>()
        .unwrap();
    ids
}

#[test]
fn return_clears_approval_after_start() {
    let sb = Sandbox::new();
    let repo = auto_to_gate(&sb);
    let res = sb.run(&["approve", "1", "--no-continue"], None);
    assert_ok(&res, "approve --no-continue");
    let approved = approved_commit(&sb, 1).expect("approval recorded");

    // Sửa tay plan sau khi duyệt: duyệt cũ, cổng chờ duyệt lại.
    let plan_path = repo.join(".plan.md");
    let mut text = std::fs::read_to_string(&plan_path).unwrap();
    text.push_str("\n<!-- sửa tay sau khi duyệt -->\n");
    std::fs::write(&plan_path, text).unwrap();
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", "sua-app: plan"]);
    let leads_before = lead_run_ids(&sb);

    sb.write_scenario(by_role(json!({
        "lead": [lead_append_entry()],
        "plan-review": [plan_review_entry("approve", 0, 1)],
    })));
    let mut srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();
    let rl = t["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "return_lead")
        .unwrap_or_else(|| panic!("return_lead missing: {}", t["controls"]))
        .clone();
    assert_eq!(rl["enabled"], true, "{rl}");
    // Trước khi bấm: duyệt cũ vẫn còn.
    assert_eq!(approved_commit(&sb, 1), Some(approved));

    let resp = srv.post_json("/api/tasks/1/return", &json!({"note": "tách bước"}));
    assert_eq!(resp.status, 202, "{}", resp.text());
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 0, "{job}");

    assert_eq!(approved_commit(&sb, 1), None);
    let leads = lead_run_ids(&sb);
    assert_eq!(leads.len(), leads_before.len() + 1, "one new lead run");
    let new_lead = *leads.last().unwrap();
    assert_eq!(run_metrics(&sb, new_lead)["note"], "tách bước");

    srv.stop(libc::SIGTERM, Duration::from_secs(10));
}
