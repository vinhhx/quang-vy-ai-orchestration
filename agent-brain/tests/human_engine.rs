//! Phase 6 plan 12: a person does the Dev step (D-19..D-25). agent-brain pushes the task branch,
//! waits without any process or `running` row, fetches the pushed branch fast-forward-only on
//! `human done`, checks the diff and records the work as a finished `người` run. The remote is a
//! local bare repository and the "staff" is a second clone; Claude is the fake one.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::teams::TaskTeam;
use common::{RunOut, Sandbox};
use serde_json::{json, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn lead_plan_entry() -> Value {
    protocol::lead_scenario(&[("src/a.txt", "a()", "đổi nội dung", "demo")])
}

/// A bare repository under the Sandbox root, used as `origin`.
fn bare_remote(sb: &Sandbox) -> PathBuf {
    let dir = sb.root.join("remote.git");
    std::fs::create_dir_all(&dir).unwrap();
    sb.git(&dir, &["init", "--bare", "-b", "main"]);
    dir
}

/// The staff's machine: a clone of `remote` that checks out `branch`, applies `edits`, commits
/// with `message` and pushes (no force unless `force`). Returns the clone path.
fn staff_clone_push(
    sb: &Sandbox,
    remote: &std::path::Path,
    branch: &str,
    edits: &[(&str, &str)],
    message: &str,
) -> PathBuf {
    let clone = sb.root.join("staff");
    if !clone.join(".git").exists() {
        sb.git(
            &sb.root,
            &["clone", remote.to_str().unwrap(), clone.to_str().unwrap()],
        );
    }
    sb.git(&clone, &["fetch", "origin"]);
    sb.git(
        &clone,
        &["checkout", "-B", branch, &format!("origin/{branch}")],
    );
    for (rel, content) in edits {
        let p = clone.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, content).unwrap();
    }
    sb.git(&clone, &["add", "-A"]);
    sb.git(&clone, &["commit", "-m", message]);
    sb.git(&clone, &["push", "origin", branch]);
    clone
}

struct World {
    repo: PathBuf,
    remote: Option<PathBuf>,
}

/// Repo `web` (optionally with a bare `origin`), staff `ha`, team `doi-nguoi` (Dev = `người:ha`)
/// and task 1 (`task/sua-app`) created by `run lead --team doi-nguoi`, plan committed.
fn setup(sb: &Sandbox, with_origin: bool) -> World {
    let remote = with_origin.then(|| bare_remote(sb));
    let repo = sb.make_repo(
        "web",
        &[("src/a.txt", "a\n")],
        None,
        remote.as_ref().map(|r| r.to_str().unwrap()),
    );
    if remote.is_some() {
        sb.git(&repo, &["push", "-u", "origin", "main"]);
    }
    let entry = sb.project_entry(&repo, "web", "work");
    sb.write_map(json!([entry]), &[]);
    sb.write_staff(json!({"ha": {"display_name": "Hà"}}));
    sb.write_teams(json!({
        "doi-nguoi": {"description": "Dev do Hà làm", "roles": {"dev": "người:ha"}}
    }));
    let res = sb.run(
        &[
            "run",
            "lead",
            "--team",
            "doi-nguoi",
            "--name",
            "sua-app",
            "--",
            "web",
            "sửa app",
        ],
        Some(json!({ "by_role": { "lead": [lead_plan_entry()] } })),
    );
    assert_ok(&res, "run lead --team doi-nguoi");
    World { repo, remote }
}

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

fn both(res: &RunOut) -> String {
    format!("{}\n{}", res.stdout, res.stderr)
}

fn run_dev(sb: &Sandbox) -> RunOut {
    sb.run(&["run", "dev", "--", "web"], None)
}

fn human(sb: &Sandbox, action: &str) -> RunOut {
    sb.run(&["human", action, "1"], None)
}

fn team_json(sb: &Sandbox) -> Option<String> {
    sb.db()
        .query_row("SELECT team_json FROM tasks WHERE id = 1", [], |r| {
            r.get::<_, Option<String>>(0)
        })
        .unwrap()
}

fn task_team(sb: &Sandbox) -> TaskTeam {
    TaskTeam::parse(team_json(sb).as_deref()).expect("task 1 has a parsable team_json")
}

fn run_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap()
}

fn running_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE status = 'running'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

fn task_status(sb: &Sandbox) -> String {
    sb.db()
        .query_row("SELECT status FROM tasks WHERE id = 1", [], |r| r.get(0))
        .unwrap()
}

fn head(sb: &Sandbox, repo: &std::path::Path) -> String {
    sb.git(repo, &["rev-parse", "HEAD"]).trim().to_string()
}

struct HumanRunRow {
    status: String,
    outcome: Option<String>,
    usage_source: String,
    tokens: i64,
    stage: Option<String>,
    round: Option<i64>,
    started_at: i64,
    base_commit: Option<String>,
    metrics: Value,
}

/// The `người` runs of task 1, oldest first.
fn human_runs(sb: &Sandbox) -> Vec<HumanRunRow> {
    let conn = sb.db();
    let mut stmt = conn
        .prepare(
            "SELECT status, outcome, usage_source, input_tokens + output_tokens, stage, round,
                    started_at, base_commit, metrics_json
             FROM runs WHERE engine = 'người' ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |r| {
        let metrics: Option<String> = r.get(8)?;
        Ok(HumanRunRow {
            status: r.get(0)?,
            outcome: r.get(1)?,
            usage_source: r.get(2)?,
            tokens: r.get(3)?,
            stage: r.get(4)?,
            round: r.get(5)?,
            started_at: r.get(6)?,
            base_commit: r.get(7)?,
            metrics: serde_json::from_str(&metrics.unwrap_or_else(|| "{}".to_string())).unwrap(),
        })
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

// ---------------------------------------------------------------------------
// Task 1 (tracer): wait, push, Xong
// ---------------------------------------------------------------------------

#[test]
fn human_dev_waits_and_done_fetches() {
    let sb = Sandbox::new();
    let w = setup(&sb, true);
    let remote = w.remote.clone().unwrap();
    let base = head(&sb, &w.repo);
    let runs_before = run_count(&sb);
    let records_before = sb.records().len();

    let res = run_dev(&sb);
    assert_eq!(
        res.code, 6,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    let out = both(&res);
    assert!(
        out.contains("Chờ Hà làm bước Code trên nhánh task/sua-app."),
        "{out}"
    );
    assert!(
        out.contains("Đã đẩy nhánh task/sua-app lên remote origin."),
        "{out}"
    );
    assert_eq!(run_count(&sb), runs_before, "a human wait inserts no run");
    assert_eq!(running_count(&sb), 0);
    assert_eq!(
        sb.records().len(),
        records_before,
        "claude must not be called"
    );

    let wait = task_team(&sb).wait.expect("wait marker");
    assert_eq!(wait.mode, "waiting");
    assert_eq!(wait.role, "dev");
    assert_eq!(wait.stage, "implement");
    assert_eq!(wait.fix_for, None);
    assert_eq!(wait.staff, "ha");
    assert_eq!(wait.branch, "task/sua-app");
    assert_eq!(wait.base, base);
    assert!(!wait.autopilot);
    let push = wait.push.expect("push result");
    assert!(push.ok, "{push:?}");
    assert_eq!(push.command, "git push -u origin task/sua-app");
    sb.git(
        &remote,
        &["rev-parse", "--verify", "refs/heads/task/sua-app"],
    );
    let since_ms = wait.since_ms;

    staff_clone_push(
        &sb,
        &remote,
        "task/sua-app",
        &[("src/a.txt", "a của Hà\n")],
        "sua-app: xong phần của Hà",
    );
    let staff_head = sb
        .git(&remote, &["rev-parse", "refs/heads/task/sua-app"])
        .trim()
        .to_string();

    let done = human(&sb, "done");
    assert_ok(&done, "human done");
    assert!(
        both(&done).contains("Đã lấy nhánh task/sua-app của Hà — run "),
        "{}",
        both(&done)
    );
    assert_eq!(head(&sb, &w.repo), staff_head);
    assert_eq!(
        std::fs::read_to_string(w.repo.join("src/a.txt")).unwrap(),
        "a của Hà\n"
    );

    let rows = human_runs(&sb);
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.status, "succeeded");
    assert_eq!(r.outcome, None);
    assert_eq!(r.usage_source, "none");
    assert_eq!(r.tokens, 0);
    assert_eq!(r.stage.as_deref(), Some("implement"));
    assert_eq!(r.round, None);
    assert_eq!(r.started_at, since_ms);
    assert_eq!(r.base_commit.as_deref(), Some(base.as_str()));
    assert_eq!(r.metrics["staff"], json!("ha"));
    assert_eq!(r.metrics["team"], json!("doi-nguoi"));
    assert_eq!(r.metrics["human"]["branch"], json!("task/sua-app"));
    assert_eq!(r.metrics["human"]["base"], json!(base));
    assert_eq!(r.metrics["human"]["head"], json!(staff_head));
    assert_eq!(running_count(&sb), 0);
    assert!(task_team(&sb).wait.is_none());
    assert!(team_json(&sb).unwrap().contains("\"wait\":null"));
    assert_eq!(task_status(&sb), "dev_done");
}

#[test]
fn human_wait_idempotent() {
    let sb = Sandbox::new();
    let w = setup(&sb, true);
    let _ = w;

    let first = run_dev(&sb);
    assert_eq!(first.code, 6, "{}", both(&first));
    let marker = team_json(&sb);
    let since = task_team(&sb).wait.unwrap().since_ms;
    std::thread::sleep(std::time::Duration::from_millis(20));

    let second = run_dev(&sb);
    assert_eq!(second.code, 6, "{}", both(&second));
    assert!(
        both(&second).contains("Chờ Hà làm bước Code trên nhánh task/sua-app."),
        "{}",
        both(&second)
    );
    assert_eq!(team_json(&sb), marker, "the marker must stay unchanged");
    assert_eq!(task_team(&sb).wait.unwrap().since_ms, since);
    assert_eq!(run_count(&sb), 1, "only the lead run exists");
}

#[test]
fn human_wait_without_origin() {
    let sb = Sandbox::new();
    let _w = setup(&sb, false);

    let res = run_dev(&sb);
    assert_eq!(res.code, 6, "{}", both(&res));
    assert!(
        res.stdout.contains("git push -u origin task/sua-app"),
        "stdout: {}",
        res.stdout
    );
    let wait = task_team(&sb).wait.expect("wait marker");
    let push = wait.push.expect("push result");
    assert!(!push.ok);
    assert_eq!(push.command, "git push -u origin task/sua-app");
    assert!(push.error.is_some());
    assert_eq!(wait.mode, "waiting");
}
