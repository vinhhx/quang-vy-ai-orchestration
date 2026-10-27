//! Phase 6 plan 07: a quota or rate-limit failure stops the step cleanly (D-15, D-16). One
//! detector at the run seam turns it into outcome `quota` and exit code 5; nothing switches
//! engine or retries. All of it runs against the fake claude with Rust-side stream data.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{RunOut, Sandbox};
use serde_json::{json, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Scenario helpers
// ---------------------------------------------------------------------------

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

fn init_event() -> Value {
    json!({"type": "system", "subtype": "init", "session_id": "fake-quota", "model": "fake"})
}

fn rejected_rate_limit_event() -> Value {
    json!({
        "type": "rate_limit_event",
        "rate_limit_info": {
            "status": "rejected",
            "resetsAt": 1792310400,
            "rateLimitType": "five_hour",
            "isUsingOverage": false,
            "overageStatus": "rejected"
        },
        "uuid": "u1",
        "session_id": "fake-quota"
    })
}

fn quota_result_event() -> Value {
    json!({
        "type": "result",
        "subtype": "success",
        "is_error": true,
        "api_error_status": 429,
        "terminal_reason": "blocking_limit",
        "result": "You've hit your session limit",
        "num_turns": 1,
        "session_id": "fake-quota",
        "total_cost_usd": 0,
        "usage": {
            "input_tokens": 0,
            "output_tokens": 0,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0
        },
        "permission_denials": []
    })
}

fn success_result_event() -> Value {
    json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "num_turns": 1,
        "session_id": "fake-quota",
        "total_cost_usd": 0,
        "usage": {
            "input_tokens": 10,
            "output_tokens": 5,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0
        },
        "permission_denials": []
    })
}

fn write_op(path: &str, content: &str) -> Value {
    json!({"op": "write", "path": path, "content": content})
}

/// A claude run that writes `ops` and then hits the quota wall: rejected rate-limit event,
/// an error result with HTTP 429 and `blocking_limit`, exit 1.
fn quota_entry(ops: Vec<Value>) -> Value {
    json!({
        "ops": ops,
        "events": [init_event(), rejected_rate_limit_event(), quota_result_event()],
        "exit_code": 1,
    })
}

fn ok_entry(ops: Vec<Value>) -> Value {
    json!({
        "ops": ops,
        "events": [init_event(), success_result_event()],
        "exit_code": 0,
    })
}

/// Repo `web-ban-hang` after `run lead`: branch `task/sua-app`, plan committed, returns the repo.
fn setup_lead_task(sb: &Sandbox) -> PathBuf {
    let repo = protocol::setup_project(sb, "work");
    let sc = protocol::lead_scenario(&[("src/app.py", "app()", "trả về 2", "yêu cầu")]);
    let out = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(sc));
    assert_ok(&out, "run lead");
    repo
}

/// `(status, outcome, error, metrics)` of the run with `id`.
fn run_row(sb: &Sandbox, id: i64) -> (String, Option<String>, Option<String>, Value) {
    let (status, outcome, error, metrics): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sb
        .db()
        .query_row(
            "SELECT status, outcome, error, metrics_json FROM runs WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap_or_else(|e| panic!("no run row {id}: {e}"));
    let metrics = serde_json::from_str(&metrics.unwrap_or_else(|| "{}".to_string())).unwrap();
    (status, outcome, error, metrics)
}

fn last_run_id(sb: &Sandbox, role: &str) -> i64 {
    sb.db()
        .query_row(
            "SELECT id FROM runs WHERE role = ?1 ORDER BY id DESC LIMIT 1",
            [role],
            |r| r.get(0),
        )
        .unwrap_or_else(|e| panic!("no {role} run: {e}"))
}

fn run_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Task 1: quota detection at the run seam
// ---------------------------------------------------------------------------

#[test]
fn eng02_dev_quota_stops() {
    let sb = Sandbox::new();
    let repo = setup_lead_task(&sb);
    let calls_before = sb.records().len();
    let runs_before = run_count(&sb);

    let out = sb.run(
        &["run", "dev", "web-ban-hang"],
        Some(quota_entry(vec![write_op(
            "src/app.py",
            "def app():\n    return 2\n",
        )])),
    );
    assert_eq!(
        out.code, 5,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stderr.contains("Hết quota engine claude"),
        "stderr: {}",
        out.stderr
    );
    assert!(
        out.stderr.contains("agent-brain không tự đổi engine"),
        "stderr: {}",
        out.stderr
    );

    let id = last_run_id(&sb, "dev");
    let (status, outcome, error, metrics) = run_row(&sb, id);
    assert_eq!(status, "failed");
    assert_eq!(outcome.as_deref(), Some("quota"));
    assert!(
        error.as_deref().unwrap_or("").starts_with("quota:"),
        "error: {error:?}"
    );
    assert_eq!(
        metrics["quota"],
        json!({"engine": "claude", "signal": "rate_limit_event.rejected", "reset_at": 1792310400000i64}),
        "metrics: {metrics}"
    );

    // No retry and no other engine: exactly one more process, exactly one more run row.
    assert_eq!(sb.records().len(), calls_before + 1);
    assert_eq!(run_count(&sb), runs_before + 1);

    // The role code after the run never ran: the partial edit stays uncommitted.
    let status_out = sb.git(&repo, &["status", "--porcelain"]);
    assert!(status_out.contains("src/app.py"), "status: {status_out}");
}

#[test]
fn eng02_retry_then_success_not_quota() {
    let sb = Sandbox::new();
    setup_lead_task(&sb);

    let retry = |attempt: i64| {
        json!({
            "type": "system",
            "subtype": "api_retry",
            "attempt": attempt,
            "max_retries": 5,
            "retry_delay_ms": 10,
            "error_status": 429,
            "error": "rate_limit",
            "session_id": "fake-quota"
        })
    };
    let scenario = json!({
        "ops": [write_op("src/app.py", "def app():\n    return 2\n")],
        "events": [init_event(), retry(1), retry(2), success_result_event()],
        "exit_code": 0,
    });
    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(scenario));
    assert_ok(&out, "run dev after a recovered 429");
    assert!(!out.stderr.contains("Hết quota"), "stderr: {}", out.stderr);

    let id = last_run_id(&sb, "dev");
    let (status, outcome, error, metrics) = run_row(&sb, id);
    assert_eq!(status, "succeeded");
    assert_ne!(outcome.as_deref(), Some("quota"));
    assert!(error.is_none(), "error: {error:?}");
    assert!(metrics.get("quota").is_none(), "metrics: {metrics}");
}
