mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::autopilot::decide::{FixFor, Stage, StageRun};
use agent_brain::autopilot::stage_args;
use common::{RunOut, Sandbox};
use protocol::{lead_scenario, plan_text};
use serde_json::{json, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn review_report(verdict: &str, blocking: i64, advisory: i64, files: &[&str]) -> String {
    let files_json = serde_json::to_string(files).unwrap();
    let mut body = format!(
        "---\nverdict: {verdict}\nblocking: {blocking}\nadvisory: {advisory}\nfiles: {files_json}\n---\n# Review: sửa app\n## Mục chặn (blocking)\n"
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

fn by_role(roles: Value) -> Value {
    json!({ "by_role": roles })
}

fn plan_review_entry(verdict: &str, blocking: i64, advisory: i64) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan-review.md",
        "content": review_report(verdict, blocking, advisory, &[]),
    }]})
}

fn lead_append_entry() -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan.md",
        "content": "\n<!-- v2 -->\n",
        "append": true,
    }]})
}

fn lead_plan_entry() -> Value {
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

fn auto_start(sb: &Sandbox, workflow: &str, scenario: Value) -> RunOut {
    sb.run(
        &[
            "auto",
            "--start",
            "--name",
            "sua-app",
            "--workflow",
            workflow,
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(scenario),
    )
}

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

fn rt(role: &str, stage: &str, round: i64, outcome: Option<&str>) -> RunTuple {
    (
        role.to_string(),
        Some(stage.to_string()),
        Some(round),
        outcome.map(|s| s.to_string()),
    )
}

fn auto_state(sb: &Sandbox, task_id: i64) -> (Option<String>, Option<i64>, Option<String>) {
    sb.db()
        .query_row(
            "SELECT auto_state, auto_pid, auto_json FROM tasks WHERE id = ?1",
            [task_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
}

fn auto_json(sb: &Sandbox, task_id: i64) -> Value {
    let (_, _, aj) = auto_state(sb, task_id);
    serde_json::from_str(&aj.unwrap_or_else(|| "{}".to_string())).unwrap()
}

fn prompt_of(record: &Value) -> String {
    let argv: Vec<String> = record["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();
    let i = argv.iter().position(|a| a == "-p").unwrap();
    argv[i + 1].clone()
}

fn records_for_role(sb: &Sandbox, first_line_prefix: &str) -> Vec<Value> {
    sb.records()
        .into_iter()
        .filter(|r| {
            let argv = r["argv"].as_array().unwrap();
            let i = argv
                .iter()
                .position(|a| a.as_str() == Some("--append-system-prompt"));
            match i {
                Some(i) => argv[i + 1]
                    .as_str()
                    .unwrap_or("")
                    .starts_with(first_line_prefix),
                None => false,
            }
        })
        .collect()
}

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

/// Luồng Task 1: `auto --start` chạy plan ⇄ review (revise rồi approve) và dừng ở cổng.
fn flow_to_gate(sb: &Sandbox) -> PathBuf {
    let repo = setup_auto_project(sb, "work");
    let sc = by_role(json!({
        "lead": [lead_plan_entry(), lead_append_entry()],
        "plan-review": [plan_review_entry("revise", 1, 0), plan_review_entry("approve", 0, 1)],
    }));
    let res = auto_start(sb, "mac-dinh", sc);
    assert_ok(&res, "auto --start");
    assert!(
        res.stderr
            .contains("Autopilot dừng ở bước duyệt plan (task 1) — duyệt: agent-brain approve 1"),
        "missing gate line: {}",
        res.stderr
    );
    repo
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

fn dev_fix_review_entry() -> Value {
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": ".code-review.md"},
        {"op": "write", "path": "src/util.py", "content": "def util():\n    return 1\n"},
        {"op": "git", "args": ["add", "src/util.py"]},
        {"op": "git", "args": ["commit", "-m", "sua-app: sửa theo review"]}
    ]})
}

fn dev_fix_qa_entry() -> Value {
    json!({"ops": [
        {"op": "read", "path": ".plan.md"},
        {"op": "read", "path": ".qa-report.md"},
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 10\n"},
        {"op": "git", "args": ["add", "src/app.py"]},
        {"op": "git", "args": ["commit", "-m", "sua-app: sửa theo QA"]}
    ]})
}

fn code_review_entry(verdict: &str, blocking: i64, files: &[&str]) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".code-review.md",
        "content": review_report(verdict, blocking, 1, files),
    }]})
}

fn qa_entry(verdict: &str) -> Value {
    let report = format!(
        "# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- src/app.py: đã xem.\n## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông.\n## Kết luận\n{verdict}\n"
    );
    json!({"ops": [{"op": "write", "path": ".qa-report.md", "content": report}]})
}

/// Sha của commit `sua-app: plan` mới nhất trên nhánh hiện tại.
fn plan_sha(sb: &Sandbox, repo: &std::path::Path) -> String {
    let log = sb.git(repo, &["log", "--format=%H %s"]);
    log.lines()
        .find_map(|l| {
            let (sha, subj) = l.split_once(' ')?;
            (subj == "sua-app: plan").then(|| sha.to_string())
        })
        .expect("plan commit")
}

fn approved_commit(sb: &Sandbox, task_id: i64) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT approved_commit FROM tasks WHERE id = ?1",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
}

fn task_status(sb: &Sandbox, task_id: i64) -> String {
    sb.db()
        .query_row("SELECT status FROM tasks WHERE id = ?1", [task_id], |r| {
            r.get(0)
        })
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

/// Mọi bước của mac-dinh sau cổng: dev, review code đạt, QA đạt.
fn build_ok_roles() -> Value {
    json!({
        "dev": [dev_impl_entry()],
        "code-review": [code_review_entry("pass", 0, &[])],
        "qa": [qa_entry("Đạt")],
    })
}

// ---------------------------------------------------------------------------
// Task 1
// ---------------------------------------------------------------------------

#[test]
fn stage_args_shapes() {
    fn sr(stage: Stage, round: i64) -> StageRun {
        StageRun {
            stage,
            round,
            fix_for: None,
            review_input: false,
            note: None,
            fresh_plan: false,
        }
    }
    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }
    let p = "web-ban-hang";
    let t = "sửa app";
    assert_eq!(
        stage_args(&sr(Stage::Plan, 1), "sua-app", p, t),
        v(&["run", "lead", "--name", "sua-app", "--round", "1", "--", p, t])
    );
    let mut plan2 = sr(Stage::Plan, 2);
    plan2.review_input = true;
    plan2.note = Some("-x".to_string());
    let args = stage_args(&plan2, "sua-app", p, t);
    assert_eq!(
        args,
        v(&[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--round",
            "2",
            "--review",
            "--note=-x",
            "--",
            p,
            t
        ])
    );
    assert_eq!(args.iter().filter(|a| a.contains("-x")).count(), 1);
    assert_eq!(
        stage_args(&sr(Stage::PlanReview, 2), "sua-app", p, t),
        v(&["run", "plan-review", "--round", "2", "--", p])
    );
    assert_eq!(
        stage_args(&sr(Stage::Implement, 1), "sua-app", p, t),
        v(&["run", "dev", "--round", "1", "--", p])
    );
    assert_eq!(
        stage_args(&sr(Stage::CodeReview, 3), "sua-app", p, t),
        v(&["run", "code-review", "--round", "3", "--", p])
    );
    assert_eq!(
        stage_args(&sr(Stage::Qa, 2), "sua-app", p, t),
        v(&["run", "qa", "--round", "2", "--", p])
    );
    let mut fix_cr = sr(Stage::Fix, 2);
    fix_cr.fix_for = Some(FixFor::CodeReview);
    assert_eq!(
        stage_args(&fix_cr, "sua-app", p, t),
        v(&[
            "run",
            "dev",
            "--fix",
            "code-review",
            "--round",
            "2",
            "--",
            p
        ])
    );
    let mut fix_qa = sr(Stage::Fix, 1);
    fix_qa.fix_for = Some(FixFor::Qa);
    assert_eq!(
        stage_args(&fix_qa, "sua-app", p, t),
        v(&["run", "dev", "--fix", "qa", "--round", "1", "--", p])
    );
}

#[test]
fn auto_start_plan_loop_to_gate() {
    let sb = Sandbox::new();
    flow_to_gate(&sb);

    assert_eq!(
        runs(&sb),
        vec![
            rt("lead", "plan", 1, None),
            rt("plan-review", "plan-review", 1, Some("review:revise")),
            rt("lead", "plan", 2, None),
            rt("plan-review", "plan-review", 2, Some("review:approve")),
        ]
    );

    let leads = records_for_role(&sb, "Bạn là Lead");
    assert_eq!(leads.len(), 2);
    assert!(
        prompt_of(&leads[1]).contains(".plan-review.md"),
        "lead round 2 prompt: {}",
        prompt_of(&leads[1])
    );

    let (state, pid, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("gate"));
    assert_eq!(pid, None);
}

#[test]
fn auto_task_at_gate_is_idempotent() {
    let sb = Sandbox::new();
    flow_to_gate(&sb);
    let before = runs(&sb).len();

    let res = sb.run(&["auto", "1"], None);
    assert_ok(&res, "auto 1");
    assert_eq!(runs(&sb).len(), before);
    let (state, pid, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("gate"));
    assert_eq!(pid, None);
    assert!(res
        .stderr
        .contains("Autopilot dừng ở bước duyệt plan (task 1)"));
}

// ---------------------------------------------------------------------------
// Task 2
// ---------------------------------------------------------------------------

#[test]
fn approve_runs_build_and_qa_to_done() {
    let sb = Sandbox::new();
    let repo = flow_to_gate(&sb);

    let res = sb.run(&["approve", "1"], Some(by_role(build_ok_roles())));
    assert_ok(&res, "approve 1");
    assert_eq!(approved_commit(&sb, 1), Some(plan_sha(&sb, &repo)));

    let all = runs(&sb);
    assert_eq!(
        all[4..].to_vec(),
        vec![
            rt("dev", "implement", 1, None),
            rt("code-review", "code-review", 1, Some("review:pass")),
            rt("qa", "qa", 1, Some("qa:pass")),
        ]
    );
    let (state, pid, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("done"));
    assert_eq!(pid, None);
    assert_eq!(task_status(&sb, 1), "qa_pass");
    assert!(
        res.stderr.contains("Autopilot xong: QA đạt (task 1)"),
        "stderr: {}",
        res.stderr
    );
}

#[test]
fn approve_refusals() {
    let sb = Sandbox::new();
    let repo = flow_to_gate(&sb);

    let res = sb.run(&["approve", "1", "--no-continue"], None);
    assert_ok(&res, "approve --no-continue");
    assert_eq!(runs(&sb).len(), 4, "no run after --no-continue");
    assert_eq!(approved_commit(&sb, 1), Some(plan_sha(&sb, &repo)));

    let again = sb.run(&["approve", "1"], None);
    assert_eq!(again.code, 1, "stderr: {}", again.stderr);
    assert!(
        again.stderr.contains("Plan hiện tại đã được duyệt lúc"),
        "stderr: {}",
        again.stderr
    );

    let plan_path = repo.join(".plan.md");
    let mut text = std::fs::read_to_string(&plan_path).unwrap();
    text.push_str("\n<!-- sửa tay -->\n");
    std::fs::write(&plan_path, text).unwrap();
    let dirty = sb.run(&["approve", "1"], None);
    assert_eq!(dirty.code, 1, "stderr: {}", dirty.stderr);
    assert!(
        dirty.stderr.contains("Plan có thay đổi chưa commit"),
        "stderr: {}",
        dirty.stderr
    );
    assert_eq!(runs(&sb).len(), 4);
}

#[test]
fn build_loop_with_fix() {
    let sb = Sandbox::new();
    flow_to_gate(&sb);

    let sc = by_role(json!({
        "dev": [dev_impl_entry(), dev_fix_review_entry()],
        "code-review": [
            code_review_entry("revise", 1, &["src/util.py"]),
            code_review_entry("pass", 0, &[])
        ],
        "qa": [qa_entry("Đạt")],
    }));
    let res = sb.run(&["approve", "1"], Some(sc));
    assert_ok(&res, "approve 1");

    let all = runs(&sb);
    assert_eq!(
        all[4..].to_vec(),
        vec![
            rt("dev", "implement", 1, None),
            rt("code-review", "code-review", 1, Some("review:revise")),
            rt("dev", "fix", 2, None),
            rt("code-review", "code-review", 2, Some("review:pass")),
            rt("qa", "qa", 1, Some("qa:pass")),
        ]
    );
    assert_eq!(run_metrics(&sb, 7)["fix_for"], "code_review");
    let (state, _, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("done"));
}

#[test]
fn qa_loop_with_fix() {
    let sb = Sandbox::new();
    flow_to_gate(&sb);

    let sc = by_role(json!({
        "dev": [dev_impl_entry(), dev_fix_qa_entry()],
        "code-review": [code_review_entry("pass", 0, &[])],
        "qa": [qa_entry("Cần sửa"), qa_entry("Đạt")],
    }));
    let res = sb.run(&["approve", "1"], Some(sc));
    assert_ok(&res, "approve 1");

    let all = runs(&sb);
    assert_eq!(
        all[4..].to_vec(),
        vec![
            rt("dev", "implement", 1, None),
            rt("code-review", "code-review", 1, Some("review:pass")),
            rt("qa", "qa", 1, Some("qa:fix")),
            rt("dev", "fix", 1, None),
            rt("qa", "qa", 2, Some("qa:pass")),
        ]
    );
    assert_eq!(run_metrics(&sb, 8)["fix_for"], "qa");
    let (state, _, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("done"));
}

fn plan_cap_flow(sb: &Sandbox) -> RunOut {
    setup_auto_project(sb, "work");
    // Plan review luôn revise. Mỗi vòng một báo cáo khác nhau: review.rs commit
    // báo cáo sau mỗi run, và một báo cáo y hệt bản đã commit làm `git commit` lỗi.
    let sc = by_role(json!({
        "lead": [lead_plan_entry(), lead_append_entry()],
        "plan-review": [
            plan_review_entry("revise", 1, 0),
            plan_review_entry("revise", 1, 1),
            plan_review_entry("revise", 1, 2)
        ],
    }));
    auto_start(sb, "mac-dinh", sc)
}

#[test]
fn plan_cap_stops_het_vong() {
    let sb = Sandbox::new();
    let res = plan_cap_flow(&sb);
    assert_eq!(res.code, 4, "stderr: {}", res.stderr);

    let all = runs(&sb);
    assert_eq!(all.iter().filter(|r| r.0 == "lead").count(), 3);
    assert_eq!(all.iter().filter(|r| r.0 == "plan-review").count(), 3);

    let (state, pid, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("stopped"));
    assert_eq!(pid, None);
    let stop = &auto_json(&sb, 1)["stop"];
    assert_eq!(stop["reason"], "het_vong");
    assert_eq!(stop["loop_name"], "plan");
    assert_eq!(stop["round"], 3);
    assert_eq!(stop["cap"], 3);
    assert!(
        res.stderr.contains(
            "Autopilot dừng · hết vòng: Review plan vẫn yêu cầu sửa lại sau 3 / 3 vòng. (task 1)"
        ),
        "stderr: {}",
        res.stderr
    );
}

#[test]
fn more_round_after_het_vong() {
    let sb = Sandbox::new();
    let res = plan_cap_flow(&sb);
    assert_eq!(res.code, 4, "stderr: {}", res.stderr);

    let sc = by_role(json!({
        "lead": [lead_append_entry()],
        "plan-review": [plan_review_entry("approve", 0, 0)],
    }));
    let more = sb.run(&["auto", "1", "--more-round"], Some(sc));
    assert_ok(&more, "auto 1 --more-round");

    let all = runs(&sb);
    assert_eq!(all.len(), 8);
    assert_eq!(
        all[6..].to_vec(),
        vec![
            rt("lead", "plan", 4, None),
            rt("plan-review", "plan-review", 4, Some("review:approve")),
        ]
    );
    let leads = records_for_role(&sb, "Bạn là Lead");
    assert!(prompt_of(leads.last().unwrap()).contains(".plan-review.md"));
    let (state, _, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("gate"));

    let again = sb.run(&["auto", "1", "--more-round"], None);
    assert_eq!(again.code, 1, "stderr: {}", again.stderr);
    assert!(
        again
            .stderr
            .contains("Chỉ chạy thêm 1 vòng khi autopilot đã dừng"),
        "stderr: {}",
        again.stderr
    );
    assert_eq!(runs(&sb).len(), 8);
}

#[test]
fn budget_stop_and_bypass() {
    let sb = Sandbox::new();
    setup_auto_project(&sb, "work");
    sb.write_workflows(json!({
        "chi-phi-thap": {
            "stages": ["plan", "plan-review", "gate:you", "implement", "qa"],
            "budget_usd": 0.05,
            "auto": true
        }
    }));
    let mut lead = lead_plan_entry();
    lead["result_cost"] = json!(0.10);
    let res = auto_start(&sb, "chi-phi-thap", by_role(json!({"lead": [lead]})));
    assert_eq!(res.code, 4, "stderr: {}", res.stderr);
    assert_eq!(runs(&sb), vec![rt("lead", "plan", 1, None)]);
    let stop = auto_json(&sb, 1)["stop"].clone();
    assert_eq!(stop["reason"], "het_ngan_sach");
    assert!((stop["used_usd"].as_f64().unwrap() - 0.10).abs() < 1e-9);
    assert!((stop["limit_usd"].as_f64().unwrap() - 0.05).abs() < 1e-9);
    assert!(res.stderr.contains("Autopilot dừng · hết ngân sách"));

    let sc = by_role(json!({
        "plan-review": [plan_review_entry("revise", 1, 0)],
        "lead": [lead_append_entry()],
    }));
    let more = sb.run(&["auto", "1", "--more-round"], Some(sc));
    assert_eq!(more.code, 4, "stderr: {}", more.stderr);
    assert_eq!(
        runs(&sb),
        vec![
            rt("lead", "plan", 1, None),
            rt("plan-review", "plan-review", 1, Some("review:revise")),
            rt("lead", "plan", 2, None),
        ]
    );
    let aj = auto_json(&sb, 1);
    assert_eq!(aj["stop"]["reason"], "het_ngan_sach");
    assert_eq!(aj["bypass_stages"], 0);
}

#[test]
fn protocol_violation_stops() {
    let sb = Sandbox::new();
    setup_auto_project(&sb, "work");
    let sc = by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [{
            "ignore_hooks": true,
            "ops": [{"op": "write", "path": "src/app.py", "content": "def app():\n    return 99\n"}]
        }],
    }));
    let res = auto_start(&sb, "mac-dinh", sc);
    assert_eq!(res.code, 4, "stderr: {}", res.stderr);
    let stop = auto_json(&sb, 1)["stop"].clone();
    assert_eq!(stop["reason"], "vi_pham_giao_thuc");
    assert_eq!(stop["run_id"], 2);
    assert_eq!(runs(&sb)[1].3.as_deref(), Some("protocol_violation"));
}

#[test]
fn dev_blocked_stops() {
    let sb = Sandbox::new();
    flow_to_gate(&sb);
    let blocked_plan = plan_text(
        "Kế hoạch",
        &[("src/app.py", "app()", "trả về 2", "demo")],
        "python3 -m unittest",
        "Thiếu thông tin API",
    );
    let sc = by_role(json!({
        "dev": [{"ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "write", "path": ".plan.md", "content": blocked_plan, "keep_meta": true}
        ]}],
    }));
    let res = sb.run(&["approve", "1"], Some(sc));
    assert_eq!(res.code, 4, "stderr: {}", res.stderr);
    let stop = auto_json(&sb, 1)["stop"].clone();
    assert_eq!(stop["reason"], "vuong_mac");
    assert_eq!(runs(&sb)[4].3.as_deref(), Some("dev:blocked"));
}

#[test]
fn run_errors() {
    let sb = Sandbox::new();
    let repo = flow_to_gate(&sb);

    let res = sb.run(
        &["approve", "1"],
        Some(by_role(json!({"dev": [{"exit_code": 1}]}))),
    );
    assert_eq!(res.code, 4, "stderr: {}", res.stderr);
    let stop = auto_json(&sb, 1)["stop"].clone();
    assert_eq!(stop["reason"], "run_loi");
    assert_eq!(stop["run_id"], 5);
    assert!(res.stderr.contains("Autopilot dừng · run lỗi: Run 5 (dev)"));

    std::fs::write(repo.join("ghi-chu.txt"), "chưa commit\n").unwrap();
    let before = runs(&sb).len();
    let again = sb.run(&["auto", "1"], Some(by_role(build_ok_roles())));
    assert_eq!(again.code, 4, "stderr: {}", again.stderr);
    assert_eq!(runs(&sb).len(), before, "no new run row");
    let stop = auto_json(&sb, 1)["stop"].clone();
    assert_eq!(stop["reason"], "run_loi");
    assert!(stop["run_id"].is_null());
    assert!(
        stop["detail_line"]
            .as_str()
            .unwrap()
            .starts_with("Working tree có thay đổi chưa commit"),
        "stop: {stop}"
    );
    let (state, pid, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("stopped"));
    assert_eq!(pid, None);
}

#[test]
fn stale_approval_gates() {
    let sb = Sandbox::new();
    let repo = flow_to_gate(&sb);
    let res = sb.run(&["approve", "1", "--no-continue"], None);
    assert_ok(&res, "approve --no-continue");

    let plan_path = repo.join(".plan.md");
    let mut text = std::fs::read_to_string(&plan_path).unwrap();
    text.push_str("\n<!-- sửa tay sau khi duyệt -->\n");
    std::fs::write(&plan_path, text).unwrap();
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", "sua-app: plan"]);

    let res = sb.run(&["auto", "1"], Some(by_role(build_ok_roles())));
    assert_ok(&res, "auto 1");
    assert!(res
        .stderr
        .contains("Autopilot dừng ở bước duyệt plan (task 1)"));
    assert!(runs(&sb).iter().all(|r| r.0 != "dev"), "no dev run");
    let (state, _, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("gate"));
}

#[test]
fn one_driver_per_task() {
    let sb = Sandbox::new();
    flow_to_gate(&sb);

    let me = std::process::id() as i32;
    let start = agent_brain::procs::proc_id(me).unwrap().start as i64;
    sb.db()
        .execute(
            "UPDATE tasks SET auto_state = 'running', auto_pid = ?1, auto_pid_start = ?2 WHERE id = 1",
            rusqlite::params![me as i64, start],
        )
        .unwrap();
    let res = sb.run(&["auto", "1"], None);
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(
        res.stderr.contains("Autopilot đang chạy task 1"),
        "stderr: {}",
        res.stderr
    );

    sb.db()
        .execute(
            "UPDATE tasks SET auto_state = 'running', auto_pid = 999999, auto_pid_start = 1 WHERE id = 1",
            [],
        )
        .unwrap();
    let res = sb.run(&["auto", "1"], None);
    assert_ok(&res, "auto 1 (reclaim)");
    let (state, pid, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("gate"));
    assert_eq!(pid, None);
}

#[test]
fn nhanh_flow() {
    let sb = Sandbox::new();
    protocol::setup_project(&sb, "work"); // entry mặc định quy trình nhanh
    let res = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
        ],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead");

    let res = sb.run(&["approve", "1"], None);
    assert_ok(&res, "approve 1 (nhanh)");
    assert_eq!(runs(&sb).len(), 1, "nhanh is manual: approve only records");

    let sc = by_role(json!({"dev": [dev_impl_entry()], "qa": [qa_entry("Cần sửa")]}));
    let res = sb.run(&["auto", "1"], Some(sc));
    assert_eq!(res.code, 4, "stderr: {}", res.stderr);
    let all = runs(&sb);
    assert_eq!(
        all[1..].to_vec(),
        vec![
            rt("dev", "implement", 1, None),
            rt("qa", "qa", 1, Some("qa:fix")),
        ]
    );
    let stop = auto_json(&sb, 1)["stop"].clone();
    assert_eq!(stop["reason"], "het_vong");
    assert_eq!(stop["loop_name"], "qa");
    assert_eq!(stop["round"], 1);
    assert_eq!(stop["cap"], 1);
}

#[test]
fn replan_with_note() {
    let sb = Sandbox::new();
    flow_to_gate(&sb);
    let res = sb.run(&["approve", "1", "--no-continue"], None);
    assert_ok(&res, "approve --no-continue");
    assert!(approved_commit(&sb, 1).is_some());

    let sc = by_role(json!({
        "lead": [lead_append_entry()],
        "plan-review": [plan_review_entry("approve", 0, 0)],
    }));
    let res = sb.run(&["auto", "1", "--replan", "--note=-gộp bước"], Some(sc));
    assert_ok(&res, "auto 1 --replan");

    let all = runs(&sb);
    assert_eq!(
        all[4..].to_vec(),
        vec![
            rt("lead", "plan", 3, None),
            rt("plan-review", "plan-review", 3, Some("review:approve")),
        ]
    );
    assert_eq!(run_metrics(&sb, 5)["note"], "-gộp bước");
    let leads = records_for_role(&sb, "Bạn là Lead");
    let p = prompt_of(leads.last().unwrap());
    assert!(
        p.contains("Ghi chú của người dùng: -gộp bước"),
        "prompt: {p}"
    );
    let (state, _, _) = auto_state(&sb, 1);
    assert_eq!(state.as_deref(), Some("gate"));
    assert_eq!(approved_commit(&sb, 1), None, "approval cleared by re-plan");
    assert!(auto_json(&sb, 1)["replan"].is_null());
}

#[test]
fn legacy_task_refused() {
    let sb = Sandbox::new();
    flow_to_gate(&sb);
    sb.db()
        .execute("UPDATE tasks SET workflow_json = NULL WHERE id = 1", [])
        .unwrap();
    let res = sb.run(&["auto", "1"], None);
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(
        res.stderr.contains("tạo trước Phase 5.2"),
        "stderr: {}",
        res.stderr
    );
}
