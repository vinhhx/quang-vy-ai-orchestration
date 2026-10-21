//! Role hygiene (05.2-27):
//! - WR-05: a failed or killed review / QA run leaves no report behind.
//! - W-1: Lead and QA run under the Bash-only git guard, and their prompts
//!   carry the one-line git rule.

mod common;
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::{lead_scenario, setup_project};
use std::path::PathBuf;

/// Stops the decoy child through its own handle (never by name).
struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn setup_lead(sb: &Sandbox) -> (PathBuf, String, String) {
    let repo = setup_project(sb, "work");
    let sc_lead = lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")]);
    let res_lead = sb.run(
        &[
            "run",
            "lead",
            "web-ban-hang",
            "sửa app",
            "--name",
            "sua-app",
        ],
        Some(sc_lead),
    );
    assert_eq!(
        res_lead.code, 0,
        "Lead failed: stderr: {}\nstdout: {}",
        res_lead.stderr, res_lead.stdout
    );
    let task = "sua-app".to_string();
    let plan_sha = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    (repo, task, plan_sha)
}

fn setup_lead_and_dev(sb: &Sandbox) -> (PathBuf, String, String, String) {
    let (repo, task, plan_sha) = setup_lead(sb);
    let sc_dev = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", format!("{task}: sửa app")]}
        ]
    });
    let res_dev = sb.run(&["run", "dev", "web-ban-hang"], Some(sc_dev));
    assert_eq!(
        res_dev.code, 0,
        "Dev failed: stderr: {}\nstdout: {}",
        res_dev.stderr, res_dev.stdout
    );
    let dev_sha = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    (repo, task, plan_sha, dev_sha)
}

const APPROVE_REPORT: &str = "\
---
verdict: approve
blocking: 0
advisory: 0
files: []
---
# Review plan: x
## Mục chặn (blocking)
## Góp ý (advisory)
";

fn lead_append_entry() -> serde_json::Value {
    serde_json::json!({"ops": [{
        "op": "write",
        "path": ".plan.md",
        "content": "\n<!-- v2 -->\n",
        "append": true,
    }]})
}

fn porcelain(sb: &Sandbox, repo: &std::path::Path) -> String {
    sb.git(repo, &["status", "--porcelain"])
}

#[test]
fn failed_plan_review_leaves_clean_tree() {
    let sb = Sandbox::new();
    let (repo, _task, _plan_sha) = setup_lead(&sb);

    let sc = serde_json::json!({"exit_code": 1, "ops": []});
    let res = sb.run(&["run", "plan-review", "--", "web-ban-hang"], Some(sc));
    assert_ne!(
        res.code, 0,
        "plan-review should fail: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        !repo.join(".plan-review.md").exists(),
        "the failed review left .plan-review.md behind"
    );
    let status = porcelain(&sb, &repo);
    assert!(status.trim().is_empty(), "tree not clean: {status}");
    assert!(
        res.stdout.contains("Đã dọn .plan-review.md")
            || res.stderr.contains("Đã dọn .plan-review.md"),
        "no cleanup line: stderr: {}\nstdout: {}",
        res.stderr,
        res.stdout
    );

    // Lead can re-plan right after (not blocked by a dirty tree).
    let res_lead = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--note=thêm",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(lead_append_entry()),
    );
    assert_eq!(
        res_lead.code, 0,
        "relead failed: stderr: {}\nstdout: {}",
        res_lead.stderr, res_lead.stdout
    );
}

#[test]
fn failed_review_restores_tracked_report() {
    let sb = Sandbox::new();
    let (repo, task, _plan_sha) = setup_lead(&sb);

    let sc_ok = serde_json::json!({"ops": [
        {"op": "write", "path": ".plan-review.md", "content": APPROVE_REPORT}
    ]});
    let res = sb.run(&["run", "plan-review", "--", "web-ban-hang"], Some(sc_ok));
    assert_eq!(
        res.code, 0,
        "approve review failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: plan-review"));

    // The failing run writes a partial report, then exits 1.
    let sc_fail = serde_json::json!({"exit_code": 1, "ops": [
        {"op": "write", "path": ".plan-review.md", "content": "# nửa chừng\n"}
    ]});
    let res = sb.run(&["run", "plan-review", "--", "web-ban-hang"], Some(sc_fail));
    assert_ne!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let on_disk = std::fs::read_to_string(repo.join(".plan-review.md")).expect("report");
    let at_head = sb.git(&repo, &["show", "HEAD:.plan-review.md"]);
    assert_eq!(on_disk, at_head, "the tracked report was not restored");
    let status = porcelain(&sb, &repo);
    assert!(status.trim().is_empty(), "tree not clean: {status}");
}

#[test]
fn e06_refusal_leaves_no_skeleton() {
    let sb = Sandbox::new();
    let repo1 = sb.make_repo(
        "proj-1",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let repo2 = sb.make_repo(
        "proj-2",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry1 = sb.project_entry(&repo1, "proj-1", "personal");
    entry1["name_vi"] = serde_json::json!("Dự án 1");
    let mut entry2 = sb.project_entry(&repo2, "proj-2", "personal");
    entry2["name_vi"] = serde_json::json!("Dự án 2");
    sb.write_map(serde_json::json!([entry1, entry2]), &[]);

    // Project 2 gets a committed plan first.
    let sc_lead = lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")]);
    let res_lead = sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "proj-2",
            "sửa app",
        ],
        Some(sc_lead),
    );
    assert_eq!(
        res_lead.code, 0,
        "Lead failed: stderr: {}\nstdout: {}",
        res_lead.stderr, res_lead.stdout
    );

    // A live personal run on project 1 (a recorded decoy child).
    let decoy_child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn decoy sleep");
    let decoy = ChildGuard(decoy_child);
    let decoy_pid = decoy.0.id() as i32;
    let decoy_start = agent_brain::procs::read_stat(decoy_pid)
        .expect("read decoy stat")
        .start;

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = sb.read_map();
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");
    let proj_row_id = agent_brain::db::project_row_id(&conn, "proj-1").expect("proj1 id");
    agent_brain::db::insert_run(
        &conn,
        &agent_brain::db::NewRun {
            task_id: None,
            project_id: proj_row_id,
            role: "plan-review".to_string(),
            profile: "personal".to_string(),
            model: Some("opus".to_string()),
            pid: decoy_pid as i64,
            pgid: decoy_pid as i64,
            pid_start: decoy_start as i64,
            base_commit: None,
            run_dir: None,
        },
    )
    .expect("insert run");

    let out = sb.run(&["run", "plan-review", "--", "proj-2"], None);
    assert_eq!(
        out.code, 1,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stderr.contains("(E-06)"),
        "stderr missing E-06: {}",
        out.stderr
    );
    assert!(
        !repo2.join(".plan-review.md").exists(),
        "the E-06 refusal left .plan-review.md behind"
    );
    let status = porcelain(&sb, &repo2);
    assert!(status.trim().is_empty(), "tree not clean: {status}");

    drop(decoy);
}

#[test]
fn failed_qa_leaves_clean_tree() {
    let sb = Sandbox::new();
    let (repo, _task, _plan_sha, dev_sha) = setup_lead_and_dev(&sb);

    let sc = serde_json::json!({"exit_code": 1, "ops": [
        {"op": "write", "path": ".qa-report.md", "content": "# QA nửa chừng\n"}
    ]});
    let res = sb.run(&["run", "qa", "--", "web-ban-hang"], Some(sc));
    assert_ne!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        !repo.join(".qa-report.md").exists(),
        "the failed QA run left .qa-report.md behind"
    );
    let status = porcelain(&sb, &repo);
    assert!(status.trim().is_empty(), "tree not clean: {status}");
    assert!(
        res.stderr.contains("Đã dọn .qa-report.md"),
        "no cleanup line: {}",
        res.stderr
    );
    let head = sb.git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(head.trim(), dev_sha);
}

#[test]
fn successful_and_violation_reviews_keep_report() {
    let sb = Sandbox::new();
    let (repo, task, _plan_sha) = setup_lead(&sb);

    // A malformed report ends protocol_violation and stays in the tree.
    let sc_bad = serde_json::json!({"ops": [
        {"op": "write", "path": ".plan-review.md", "content": "# không có front matter\n"}
    ]});
    let res = sb.run(&["run", "plan-review", "--", "web-ban-hang"], Some(sc_bad));
    assert_eq!(
        res.code,
        agent_brain::error::EXIT_PROTOCOL,
        "stderr: {}\nstdout: {}",
        res.stderr,
        res.stdout
    );
    assert_eq!(
        std::fs::read_to_string(repo.join(".plan-review.md")).unwrap(),
        "# không có front matter\n"
    );
    assert!(!res.stderr.contains("Đã dọn"), "stderr: {}", res.stderr);
    let outcome: Option<String> = sb
        .db()
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'plan-review' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome.as_deref(), Some("protocol_violation"));

    // An approve review still commits its report.
    let sc_ok = serde_json::json!({"ops": [
        {"op": "write", "path": ".plan-review.md", "content": APPROVE_REPORT}
    ]});
    let res = sb.run(&["run", "plan-review", "--", "web-ban-hang"], Some(sc_ok));
    assert_eq!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: plan-review"));
    assert_eq!(
        std::fs::read_to_string(repo.join(".plan-review.md")).unwrap(),
        APPROVE_REPORT
    );
    let status = porcelain(&sb, &repo);
    assert!(status.trim().is_empty(), "tree not clean: {status}");
}

// ---------------------------------------------------------------------------
// W-1: Lead and QA under the Bash git guard.
// ---------------------------------------------------------------------------

/// Same literal as tests/assets_parity.rs.
const GIT_RULE_READ: &str =
    "- Chạy mỗi lệnh git riêng, trên một dòng, không dùng `&&`, `|`, `;` hay `$(…)`.";

/// The `run_dir` of the latest run of `role`.
fn latest_run_dir(sb: &Sandbox, role: &str) -> PathBuf {
    let run_dir: String = sb
        .db()
        .query_row(
            "SELECT run_dir FROM runs WHERE role = ?1 ORDER BY id DESC LIMIT 1",
            rusqlite::params![role],
            |r| r.get(0),
        )
        .unwrap();
    PathBuf::from(run_dir)
}

fn guard_log_lines(run_dir: &std::path::Path) -> Vec<serde_json::Value> {
    let log_path = run_dir.join("guard.jsonl");
    let content = std::fs::read_to_string(&log_path)
        .unwrap_or_else(|e| panic!("read guard log {}: {e}", log_path.display()));
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("parse guard log line"))
        .collect()
}

/// The normal Lead plan scenario with one git call in front.
fn lead_scenario_with_git(git_args: &[&str]) -> serde_json::Value {
    let mut sc = lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")]);
    let ops = sc["ops"].as_array_mut().expect("ops");
    ops.insert(0, serde_json::json!({"op": "git", "args": git_args}));
    sc
}

fn run_lead(sb: &Sandbox, sc: serde_json::Value) -> common::RunOut {
    sb.run(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(sc),
    )
}

#[test]
fn lead_git_output_blocked() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let res = run_lead(
        &sb,
        lead_scenario_with_git(&["log", "--output=../leak-lead.txt"]),
    );
    assert_eq!(
        res.code, 0,
        "Lead failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), "sua-app: plan");

    let rec = sb.records().pop().expect("fake record");
    assert!(
        rec["settings"].as_str().is_some(),
        "Lead ran without --settings: {rec}"
    );
    let blocked = rec["blocked"].as_array().expect("blocked array");
    assert!(
        blocked.iter().any(|b| b["tool"] == "Bash"),
        "no Bash entry in blocked: {blocked:?}"
    );
    assert!(
        !sb.repos.join("leak-lead.txt").exists(),
        "git log --output wrote outside the repo"
    );

    let run_dir = latest_run_dir(&sb, "lead");
    assert!(run_dir.join("settings.json").is_file());
    assert!(run_dir.join("policy.json").is_file());
    assert_eq!(
        rec["settings"].as_str(),
        Some(run_dir.join("settings.json").to_string_lossy().as_ref())
    );
    let lines = guard_log_lines(&run_dir);
    assert!(
        lines
            .iter()
            .any(|l| l["decision"] == "block" && l["tool"] == "Bash"),
        "no Bash block line in guard log: {lines:?}"
    );
}

#[test]
fn lead_git_status_allowed() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let res = run_lead(&sb, lead_scenario_with_git(&["status", "--porcelain"]));
    assert_eq!(
        res.code, 0,
        "Lead failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    let rec = sb.records().pop().expect("fake record");
    assert!(rec["settings"].as_str().is_some());
    let blocked = rec["blocked"].as_array().expect("blocked array");
    assert!(
        !blocked.iter().any(|b| b["tool"] == "Bash"),
        "git status was blocked: {blocked:?}"
    );
}

#[test]
fn qa_git_ext_diff_blocked() {
    let sb = Sandbox::new();
    let (repo, task, _plan_sha, _dev_sha) = setup_lead_and_dev(&sb);

    let qa_report = "\
# QA: sửa app
## Thay đổi có khớp plan không (từng mục)
- src/app.py: khớp.
## Sửa ngoài plan (nếu có)
Không.
## Rủi ro
Không phát hiện rủi ro.
## Kết luận
Đạt
";
    let sc = serde_json::json!({"ops": [
        {"op": "git", "args": ["show", "--ext-diff", "HEAD"]},
        {"op": "write", "path": ".qa-report.md", "content": qa_report, "keep_meta": true}
    ]});
    let res = sb.run(&["run", "qa", "--", "web-ban-hang"], Some(sc));
    assert_eq!(
        res.code, 0,
        "QA failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    let head_subj = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subj.trim(), format!("{task}: qa"));
    let outcome: Option<String> = sb
        .db()
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'qa' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome.as_deref(), Some("qa:pass"));

    let rec = sb.records().pop().expect("fake record");
    assert!(
        rec["settings"].as_str().is_some(),
        "QA ran without --settings"
    );
    let blocked = rec["blocked"].as_array().expect("blocked array");
    assert!(
        blocked.iter().any(|b| b["tool"] == "Bash"),
        "no Bash entry in blocked: {blocked:?}"
    );

    let run_dir = latest_run_dir(&sb, "qa");
    let lines = guard_log_lines(&run_dir);
    assert!(
        lines
            .iter()
            .any(|l| l["decision"] == "block" && l["tool"] == "Bash"),
        "no Bash block line in guard log: {lines:?}"
    );
}

#[test]
fn lead_dry_run_shows_settings() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let res = sb.run(
        &[
            "run",
            "lead",
            "--dry-run",
            "--name",
            "x",
            "--",
            "web-ban-hang",
            "x",
        ],
        None,
    );
    assert_eq!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    // The Command line runs to the end of stdout (the quoted -p input has
    // newlines).
    let cmd_start = res
        .stdout
        .find("Command:")
        .unwrap_or_else(|| panic!("no Command line: {}", res.stdout));
    let cmd_line = &res.stdout[cmd_start..];
    assert!(
        cmd_line.contains("--settings"),
        "dry-run Command has no --settings: {cmd_line}"
    );
    // --dry-run creates no run directory.
    assert!(
        !sb.runs.join("web-ban-hang").exists(),
        "dry-run created a run directory"
    );
    assert!(sb.records().is_empty(), "dry-run started claude");
}

#[test]
fn lead_qa_prompts_carry_git_rule() {
    for (name, prompt) in [
        ("lead", agent_brain::roles::LEAD_PROMPT),
        ("qa", agent_brain::roles::QA_PROMPT),
    ] {
        assert_eq!(
            prompt.lines().filter(|l| *l == GIT_RULE_READ).count(),
            1,
            "prompt '{name}' must carry the git rule exactly once"
        );
    }
}
