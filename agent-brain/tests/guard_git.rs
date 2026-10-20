//! W-1 (05.2-SECURITY): the guard hook also checks Bash git calls.

mod common;
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::{lead_scenario, setup_project};
use std::path::PathBuf;

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

fn run_plan_review(sb: &Sandbox, git_args: &[&str]) -> common::RunOut {
    let sc_review = serde_json::json!({
        "ops": [
            {"op": "git", "args": git_args},
            {"op": "write", "path": ".plan-review.md", "content": APPROVE_REPORT}
        ]
    });
    sb.run(
        &["run", "plan-review", "--", "web-ban-hang"],
        Some(sc_review),
    )
}

fn latest_outcome(sb: &Sandbox) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT outcome FROM runs WHERE role = 'plan-review' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn reviewer_git_output_blocked() {
    let sb = Sandbox::new();
    let (_repo, _task, _plan_sha) = setup_lead(&sb);

    let res = run_plan_review(&sb, &["diff", "--output=../leak-review.txt"]);
    assert_eq!(
        res.code, 0,
        "plan-review failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert_eq!(latest_outcome(&sb).as_deref(), Some("review:approve"));

    let rec = sb.records().pop().expect("fake record");
    let blocked = rec["blocked"].as_array().expect("blocked array");
    assert!(
        blocked.iter().any(|b| b["tool"] == "Bash"),
        "no Bash entry in blocked: {blocked:?}"
    );
    assert!(
        !sb.repos.join("leak-review.txt").exists(),
        "git diff --output wrote outside the repo"
    );

    let run_dir = latest_run_dir(&sb, "plan-review");
    let log_path = run_dir.join("guard.jsonl");
    let policy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("policy.json")).unwrap())
            .unwrap();
    assert_eq!(
        policy["log"].as_str(),
        Some(log_path.to_string_lossy().as_ref()),
        "guard log path differs from policy.json"
    );
    let lines = guard_log_lines(&run_dir);
    assert!(
        lines.iter().any(|l| l["decision"] == "block"
            && l["tool"] == "Bash"
            && l["path"]
                .as_str()
                .is_some_and(|p| p.contains("--output=../leak-review.txt"))),
        "no Bash block line in guard log: {lines:?}"
    );
}

#[test]
fn reviewer_git_status_allowed() {
    let sb = Sandbox::new();
    let (_repo, _task, _plan_sha) = setup_lead(&sb);

    let res = run_plan_review(&sb, &["status", "--porcelain"]);
    assert_eq!(
        res.code, 0,
        "plan-review failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert_eq!(latest_outcome(&sb).as_deref(), Some("review:approve"));

    let rec = sb.records().pop().expect("fake record");
    let blocked = rec["blocked"].as_array().expect("blocked array");
    assert!(
        !blocked.iter().any(|b| b["tool"] == "Bash"),
        "git status was blocked: {blocked:?}"
    );
}

/// policy.json for the guard binary, written the way contract_guard.rs does.
fn write_policy(dir: &std::path::Path) -> PathBuf {
    let policy_file = dir.join("policy.json");
    let policy_data = serde_json::json!({
        "root": dir.to_string_lossy(),
        "read_allow": null,
        "write_allow": [],
        "log": dir.join("guard.jsonl").to_string_lossy(),
    });
    std::fs::write(&policy_file, serde_json::to_string(&policy_data).unwrap())
        .expect("write policy.json");
    policy_file
}

/// Runs `agent-brain guard --policy <policy>` with one hook payload on stdin.
fn run_guard(policy: &std::path::Path, payload: &serde_json::Value) -> (Option<i32>, String) {
    let dir = policy.parent().unwrap();
    let mut child = std::process::Command::new(common::bin())
        .arg("guard")
        .arg("--policy")
        .arg(policy)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", dir)
        .env("AGENT_BRAIN_DB", dir.join("never.db"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn guard");
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("stdin");
        stdin
            .write_all(serde_json::to_string(payload).unwrap().as_bytes())
            .expect("write stdin");
    }
    let out = child.wait_with_output().expect("wait guard");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn bash_payload(dir: &std::path::Path, command: &str) -> serde_json::Value {
    serde_json::json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command, "description": "x"},
        "cwd": dir.to_string_lossy(),
    })
}

#[test]
fn guard_binary_bash_exit_codes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().canonicalize().expect("canonicalize");
    let policy = write_policy(&dir);

    let (code, stderr) = run_guard(&policy, &bash_payload(&dir, "git log $(id)"));
    assert_eq!(code, Some(2), "stderr: {stderr}");
    assert!(
        stderr.starts_with("agent-brain guard: chặn lệnh Bash"),
        "stderr: {stderr}"
    );

    let (code, stderr) = run_guard(&policy, &bash_payload(&dir, "git diff --output=x"));
    assert_eq!(code, Some(2), "stderr: {stderr}");
    assert!(
        stderr.starts_with("agent-brain guard: chặn lệnh Bash 'git diff --output=x'"),
        "stderr: {stderr}"
    );
    assert!(!dir.join("x").exists());

    let (code, stderr) = run_guard(&policy, &bash_payload(&dir, "git status"));
    assert_eq!(code, Some(0), "stderr: {stderr}");

    // a Bash payload without a command is refused
    let no_cmd = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {},
        "cwd": dir.to_string_lossy(),
    });
    let (code, _) = run_guard(&policy, &no_cmd);
    assert_eq!(code, Some(2));

    let lines = guard_log_lines(&dir);
    let decisions: Vec<&str> = lines
        .iter()
        .map(|l| l["decision"].as_str().unwrap())
        .collect();
    assert_eq!(decisions, vec!["block", "block", "allow", "block"]);
    assert!(lines.iter().all(|l| l["tool"] == "Bash"));
}

fn setup_lead_task(sb: &Sandbox) -> (PathBuf, String, String) {
    let repo = setup_project(sb, "work");
    let sc_lead = lead_scenario(&[("src/app.py", "app()", "trả về 2", "yêu cầu")]);
    let out_lead = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(sc_lead));
    assert_eq!(
        out_lead.code, 0,
        "lead failed: stderr: {}\nstdout: {}",
        out_lead.stderr, out_lead.stdout
    );
    let task = "sua-app".to_string();
    let plan_commit = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    (repo, task, plan_commit)
}

#[test]
fn dev_heredoc_commit_falls_back_to_brain_commit() {
    let sb = Sandbox::new();
    let (repo, _task, plan_commit) = setup_lead_task(&sb);

    let new_app = "def app():\n    return 2\n";
    let dev_scenario = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": new_app},
            {"op": "git", "args": ["add", "src/app.py", "&&", "git", "commit", "-m", "sua-app: sửa app"]},
            {"op": "git", "args": ["commit", "-m", "\"$(cat <<'EOF'\nsua-app: sửa app\nEOF\n)\""]},
            {"op": "git", "args": ["commit", "-m", "\"$(echo sua-app: sửa app)\""]}
        ]
    });
    let out = sb.run(&["run", "dev", "web-ban-hang"], Some(dev_scenario));
    assert_eq!(
        out.code, 0,
        "dev failed: stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    let rec = sb.records().pop().expect("fake record");
    let blocked = rec["blocked"].as_array().expect("blocked array");
    assert_eq!(blocked.len(), 3, "blocked: {blocked:?}");
    assert!(blocked.iter().all(|b| b["tool"] == "Bash"), "{blocked:?}");

    let subjects = sb.git(
        &repo,
        &["log", "--format=%s", &format!("{plan_commit}..HEAD")],
    );
    let subjects: Vec<&str> = subjects.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(subjects, vec!["sua-app: Thực hiện Kế hoạch"]);

    let diff_tree = sb.git(
        &repo,
        &["diff", "--name-only", &format!("{plan_commit}..HEAD")],
    );
    assert_eq!(diff_tree.trim(), "src/app.py");
    assert_eq!(sb.git(&repo, &["show", "HEAD:src/app.py"]), new_app);
    let status = sb.git(&repo, &["status", "--porcelain"]);
    assert!(status.trim().is_empty(), "dirty tree: {status}");
    let all_out = format!("{}{}", out.stdout, out.stderr);
    assert!(!all_out.contains("CẢNH BÁO: Commit"), "{all_out}");

    let run_dir = latest_run_dir(&sb, "dev");
    let lines = guard_log_lines(&run_dir);
    let bash_blocks = lines
        .iter()
        .filter(|l| l["decision"] == "block" && l["tool"] == "Bash")
        .count();
    assert_eq!(bash_blocks, 3, "guard log: {lines:?}");
}
