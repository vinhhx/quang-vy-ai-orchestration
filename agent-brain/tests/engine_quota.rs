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

// ---------------------------------------------------------------------------
// Task 2: autopilot het_quota, rounds
// ---------------------------------------------------------------------------

fn by_role(roles: Value) -> Value {
    json!({ "by_role": roles })
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

fn review_entry(file: &str, verdict: &str, blocking: i64, advisory: i64) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": file,
        "content": review_report(verdict, blocking, advisory),
    }]})
}

fn lead_plan_entry() -> Value {
    protocol::lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])
}

fn dev_impl_entry() -> Value {
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
        {"op": "git", "args": ["add", "src/app.py"]},
        {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
    ]})
}

fn qa_ok_entry() -> Value {
    let report = "# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- src/app.py: đã xem.\n## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông.\n## Kết luận\nĐạt\n";
    json!({"ops": [{"op": "write", "path": ".qa-report.md", "content": report}]})
}

/// Repo `web-ban-hang` with an entry without `workflow` (server default: mac-dinh).
fn setup_auto_project(sb: &Sandbox) -> PathBuf {
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    entry.as_object_mut().unwrap().remove("workflow");
    sb.write_map(json!([entry]), &[]);
    repo
}

/// `auto --start` on a `mac-dinh` task: Lead plans, plan-review approves, the driver waits at the gate.
fn flow_to_gate(sb: &Sandbox) {
    setup_auto_project(sb);
    let sc = by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [review_entry(".plan-review.md", "approve", 0, 1)],
    }));
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
    assert_ok(&res, "auto --start");
}

/// `(role, stage, round, outcome)` of every run, oldest first.
type RunTuple = (String, Option<String>, Option<i64>, Option<String>);

fn runs(sb: &Sandbox) -> Vec<RunTuple> {
    let conn = sb.db();
    let mut stmt = conn
        .prepare("SELECT role, stage, round, outcome FROM runs ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn auto_state(sb: &Sandbox, task_id: i64) -> (Option<String>, Value) {
    let (state, aj): (Option<String>, Option<String>) = sb
        .db()
        .query_row(
            "SELECT auto_state, auto_json FROM tasks WHERE id = ?1",
            [task_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    (
        state,
        serde_json::from_str(&aj.unwrap_or_else(|| "{}".to_string())).unwrap(),
    )
}

/// Gate reached, then `approve 1` lets the driver run Dev, whose fake stream hits quota.
fn drive_into_quota(sb: &Sandbox) -> RunOut {
    flow_to_gate(sb);
    sb.run(
        &["approve", "1"],
        Some(by_role(json!({"dev": [quota_entry(vec![])]}))),
    )
}

#[test]
fn eng02_autopilot_stops_het_quota() {
    let sb = Sandbox::new();
    let res = drive_into_quota(&sb);
    assert_eq!(
        res.code, 4,
        "driver stop exits 4; stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(res.stderr.contains("hết quota"), "stderr: {}", res.stderr);

    let (state, aj) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("stopped"), "aj: {aj}");
    assert_eq!(aj["stop"]["reason"], "het_quota", "aj: {aj}");
    assert_eq!(aj["stop"]["role"], "dev");
    let dev_id = last_run_id(&sb, "dev");
    assert_eq!(aj["stop"]["run_id"], json!(dev_id));
    assert_eq!(
        aj["stop"]["detail_line"],
        json!(format!(
            "Run {dev_id} (dev) trên engine claude dừng vì hết quota hoặc bị giới hạn tốc độ. Agent-brain không tự đổi engine."
        ))
    );

    let all = runs(&sb);
    assert_eq!(all.iter().filter(|r| r.0 == "dev").count(), 1, "{all:?}");
    assert!(
        all.iter().all(|r| r.0 != "qa" && r.0 != "code-review"),
        "nothing may start after the quota run: {all:?}"
    );
    assert_eq!(all.last().unwrap().3.as_deref(), Some("quota"));
}

#[test]
fn quota_run_does_not_consume_round() {
    let sb = Sandbox::new();
    let res = drive_into_quota(&sb);
    assert_eq!(res.code, 4, "stderr: {}", res.stderr);
    let quota_round = runs(&sb).iter().find(|r| r.0 == "dev").unwrap().2;
    assert_eq!(quota_round, Some(1));

    let res = sb.run(
        &["auto", "1", "--more-round"],
        Some(by_role(json!({
            "dev": [dev_impl_entry()],
            "code-review": [review_entry(".code-review.md", "pass", 0, 1)],
            "qa": [qa_ok_entry()],
        }))),
    );
    assert_ok(&res, "auto 1 --more-round after the quota stop");

    let all = runs(&sb);
    let devs: Vec<&RunTuple> = all.iter().filter(|r| r.0 == "dev").collect();
    assert_eq!(devs.len(), 2, "{all:?}");
    assert_eq!(devs[0].3.as_deref(), Some("quota"));
    assert_eq!(devs[1].2, Some(1), "the re-run keeps the round: {all:?}");
    assert!(all.iter().any(|r| r.0 == "qa"), "flow continues: {all:?}");
    let (state, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("done"));
}

#[test]
fn manual_lead_round_skips_quota() {
    let sb = Sandbox::new();
    setup_auto_project(&sb);

    let res = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(quota_entry(vec![])),
    );
    assert_eq!(res.code, 5, "stderr: {}", res.stderr);

    let res = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "second run lead");

    let all = runs(&sb);
    assert_eq!(all.len(), 2, "{all:?}");
    assert_eq!(all[0].3.as_deref(), Some("quota"));
    assert_eq!(all[0].2, Some(1));
    assert_eq!(
        all[1].2,
        Some(1),
        "the re-run of the stopped step keeps its round: {all:?}"
    );
}

// ---------------------------------------------------------------------------
// Task 3: Dev continuation after a quota stop (ENG-03)
// ---------------------------------------------------------------------------

/// `run dev` against the scenario already on disk, so the by_role queue keeps its position.
fn run_dev_queued(sb: &Sandbox) -> RunOut {
    let out = sb
        .command(&["run", "dev", "web-ban-hang"], &[])
        .output()
        .expect("execute agent-brain");
    RunOut {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

/// Dev queue: a first run that writes `first_ops` and hits quota, then one that completes.
fn queue_quota_then_ok(sb: &Sandbox, first_ops: Vec<Value>) {
    sb.write_scenario(by_role(json!({
        "dev": [
            quota_entry(first_ops),
            ok_entry(vec![write_op("src/app.py", "def app():\n    return 3\n")]),
        ]
    })));
}

fn argv_of(record: &Value) -> Vec<String> {
    record["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

/// The text after `-p` of a recorded fake claude call.
fn input_of(record: &Value) -> String {
    let argv = argv_of(record);
    let i = argv.iter().position(|a| a == "-p").expect("-p in argv");
    argv[i + 1].clone()
}

const CONTINUE_TEXT: &str = "Tiếp tục việc dở của lượt trước (dừng vì hết quota)";

#[test]
fn eng03_rerun_continues_partial_diff() {
    let sb = Sandbox::new();
    let repo = setup_lead_task(&sb);
    queue_quota_then_ok(&sb, vec![write_op("src/app.py", "phần 1")]);

    let first = run_dev_queued(&sb);
    assert_eq!(first.code, 5, "stderr: {}", first.stderr);
    let first_id = last_run_id(&sb, "dev");
    let dirty = sb.git(&repo, &["status", "--porcelain"]);
    assert!(dirty.contains("src/app.py"), "status: {dirty}");
    assert_eq!(
        std::fs::read_to_string(repo.join("src/app.py")).unwrap(),
        "phần 1"
    );

    let second = run_dev_queued(&sb);
    assert_eq!(
        second.code, 0,
        "the re-run is not refused; stderr: {}\nstdout: {}",
        second.stderr, second.stdout
    );

    let recs = sb.records();
    let rec = recs.last().unwrap();
    let argv = argv_of(rec);
    assert!(
        input_of(rec).contains(CONTINUE_TEXT),
        "input: {}",
        input_of(rec)
    );
    assert!(
        argv.contains(&"Bash(git diff *)".to_string()),
        "argv: {argv:?}"
    );
    assert!(argv.contains(&"Bash(git status *)".to_string()));
    assert!(
        !argv.iter().any(|a| a == "--resume" || a == "--continue"),
        "the old session is never resumed: {argv:?}"
    );
    assert!(
        !argv.iter().any(|a| a.contains(".handover.md")),
        "no handover file exists: {argv:?}"
    );

    // The first run's own input carries no continuation text.
    assert!(!input_of(&recs[recs.len() - 2]).contains(CONTINUE_TEXT));

    // The partial diff was kept and finished: one commit `sua-app: …` with src/app.py.
    assert_eq!(
        std::fs::read_to_string(repo.join("src/app.py")).unwrap(),
        "def app():\n    return 3\n"
    );
    let subject = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert!(subject.starts_with("sua-app: "), "subject: {subject}");
    let files = sb.git(&repo, &["show", "--name-only", "--format=", "HEAD"]);
    assert_eq!(files.trim(), "src/app.py");
    assert!(sb.git(&repo, &["status", "--porcelain"]).trim().is_empty());

    let second_id = last_run_id(&sb, "dev");
    assert_ne!(second_id, first_id);
    let (_, outcome, _, metrics) = run_row(&sb, second_id);
    assert_ne!(outcome.as_deref(), Some("quota"));
    assert_eq!(metrics["continuation"], json!(true), "metrics: {metrics}");
    let (_, _, _, first_metrics) = run_row(&sb, first_id);
    assert!(first_metrics.get("continuation").is_none());
}

#[test]
fn eng03_dirty_outside_plan_refused() {
    let sb = Sandbox::new();
    let repo = setup_lead_task(&sb);
    queue_quota_then_ok(
        &sb,
        vec![
            write_op("src/app.py", "phần 1"),
            write_op("ngoai-plan.txt", "ngoài plan"),
        ],
    );

    let first = run_dev_queued(&sb);
    assert_eq!(first.code, 5, "stderr: {}", first.stderr);
    let calls = sb.records().len();

    let second = run_dev_queued(&sb);
    assert_eq!(second.code, 1, "stderr: {}", second.stderr);
    assert!(
        second
            .stderr
            .contains("Working tree có thay đổi chưa commit"),
        "stderr: {}",
        second.stderr
    );
    assert!(
        second.stderr.contains("  ngoai-plan.txt"),
        "stderr: {}",
        second.stderr
    );
    assert!(
        !second.stderr.contains("  src/app.py"),
        "only the disallowed paths are listed: {}",
        second.stderr
    );
    assert_eq!(sb.records().len(), calls, "no process was spawned");
    assert!(sb
        .git(&repo, &["status", "--porcelain"])
        .contains("ngoai-plan.txt"));
}

#[test]
fn eng03_handover_included() {
    let sb = Sandbox::new();
    let repo = setup_lead_task(&sb);
    queue_quota_then_ok(&sb, vec![write_op("src/app.py", "phần 1")]);

    let first = run_dev_queued(&sb);
    assert_eq!(first.code, 5, "stderr: {}", first.stderr);
    std::fs::write(repo.join(".handover.md"), "# Bàn giao\nCòn lại: phần 2\n").unwrap();

    let second = run_dev_queued(&sb);
    assert_eq!(
        second.code, 0,
        "stderr: {}\nstdout: {}",
        second.stderr, second.stdout
    );

    let recs = sb.records();
    let rec = recs.last().unwrap();
    let argv = argv_of(rec);
    assert!(input_of(rec).contains(CONTINUE_TEXT));
    assert!(
        input_of(rec).contains(".handover.md"),
        "input: {}",
        input_of(rec)
    );
    assert!(
        argv.contains(&"Read(./.handover.md)".to_string()),
        "argv: {argv:?}"
    );
    let repo_str = repo.to_string_lossy();
    assert!(argv.contains(&format!("Read(/{repo_str}/.handover.md)")));

    // The handover file is context, not a deliverable: never committed, never a violation.
    assert!(
        !second.stdout.contains("ROLE-06"),
        "stdout: {}",
        second.stdout
    );
    let tracked = sb.git(&repo, &["ls-files"]);
    assert!(!tracked.contains(".handover.md"), "tracked: {tracked}");
}

#[test]
fn no_quota_dirty_still_refused() {
    // No Dev run at all: a dirty plan file is refused as before.
    let sb = Sandbox::new();
    let repo = setup_lead_task(&sb);
    std::fs::write(repo.join("src/app.py"), "sửa tay").unwrap();
    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(ok_entry(vec![])));
    assert_eq!(out.code, 1, "stderr: {}", out.stderr);
    assert!(
        out.stderr.contains("Working tree có thay đổi chưa commit"),
        "stderr: {}",
        out.stderr
    );
    assert!(out.stderr.contains("  src/app.py"));

    // A previous Dev run that failed for another reason is not a quota stop either.
    let sb = Sandbox::new();
    let repo = setup_lead_task(&sb);
    let failing = json!({
        "ops": [write_op("src/app.py", "dở dang")],
        "events": [init_event(), json!({
            "type": "result",
            "subtype": "error_during_execution",
            "is_error": true,
            "result": "boom",
            "num_turns": 1,
            "session_id": "fake-quota",
            "usage": {"input_tokens": 0, "output_tokens": 0},
            "permission_denials": []
        })],
        "exit_code": 1,
    });
    let first = sb.run(&["run", "dev", "web-ban-hang"], Some(failing));
    assert_eq!(first.code, 1, "stderr: {}", first.stderr);
    let id = last_run_id(&sb, "dev");
    assert_ne!(run_row(&sb, id).1.as_deref(), Some("quota"));
    let second = sb.run(&["run", "dev", "web-ban-hang"], Some(ok_entry(vec![])));
    assert_eq!(second.code, 1, "stderr: {}", second.stderr);
    assert!(
        second
            .stderr
            .contains("Working tree có thay đổi chưa commit"),
        "stderr: {}",
        second.stderr
    );
    assert!(sb
        .git(&repo, &["status", "--porcelain"])
        .contains("src/app.py"));
}
