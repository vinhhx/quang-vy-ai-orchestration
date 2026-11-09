//! D2.2 / D2.4 / D2.5 (`workspace: "worktree"`): every task in its own git worktree under the
//! sandbox data dir.
//! - Lead, plan review and Dev run inside the task's worktree; the main repo keeps its branch;
//! - with `max_parallel_tasks = 2` two tasks run side by side and a third one is refused;
//! - `task cleanup` refuses a task that is not done, a dirty worktree and an unmerged branch
//!   without confirmation, and removes a clean merged worktree without deleting the branch;
//! - Dọn worktree on the dashboard asks for confirmation when the branch is not merged;
//! - `task merge` merges a QA-passed task into its base, drops a conflicting protocol file and
//!   aborts on any other conflict;
//! - `PUT /api/projects/{id}/workspace` validates and stores `workspace` / `max_parallel_tasks`.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{RunOut, Sandbox};
use protocol::lead_scenario;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

const PROJECT: &str = "web-ban-hang";

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

fn by_role(roles: Value) -> Value {
    json!({ "by_role": roles })
}

fn lead_plan_entry() -> Value {
    lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])
}

fn plan_review_entry() -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan-review.md",
        "content": "---\nverdict: approve\nblocking: 0\nadvisory: 0\nfiles: []\n---\n# Review: sửa app\n## Mục chặn (blocking)\n## Góp ý (advisory)\n",
    }]})
}

/// Repo `web-ban-hang` in worktree mode on the server default workflow (auto).
fn setup(sb: &Sandbox, extra: Value) -> PathBuf {
    let repo = sb.make_repo(
        PROJECT,
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, PROJECT, "work");
    let obj = entry.as_object_mut().unwrap();
    obj.remove("workflow");
    obj.insert("workspace".to_string(), json!("worktree"));
    if let Some(e) = extra.as_object() {
        for (k, v) in e {
            obj.insert(k.clone(), v.clone());
        }
    }
    sb.write_map(json!([entry]), &[]);
    repo
}

/// `auto --start` of task `name`: Lead, plan review approves, stops at the plan gate.
fn auto_to_gate(sb: &Sandbox, name: &str) {
    let sc = by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry()],
    }));
    let res = sb.run(&["auto", "--start", "--name", name, "--", PROJECT, name], Some(sc));
    assert_ok(&res, "auto --start");
}

fn expected_worktree(sb: &Sandbox, slug: &str) -> PathBuf {
    sb.xdg_data
        .join("agent-brain")
        .join("worktrees")
        .join(PROJECT)
        .join(slug)
}

fn task_field(sb: &Sandbox, task_id: i64, col: &str) -> Option<String> {
    sb.db()
        .query_row(&format!("SELECT {col} FROM tasks WHERE id = ?1"), [task_id], |r| r.get(0))
        .unwrap()
}

fn task_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap()
}

fn current_branch(sb: &Sandbox, dir: &Path) -> String {
    sb.git(dir, &["branch", "--show-current"]).trim().to_string()
}

fn branch_exists(repo: &Path, branch: &str) -> bool {
    std::process::Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")])
        .current_dir(repo)
        .output()
        .unwrap()
        .status
        .success()
}

fn roles_of_runs(sb: &Sandbox, task_id: i64) -> Vec<String> {
    let conn = sb.db();
    let mut stmt = conn
        .prepare("SELECT role FROM runs WHERE task_id = ?1 ORDER BY id")
        .unwrap();
    let rows = stmt
        .query_map([task_id], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<String>, _>>()
        .unwrap();
    rows
}

/// A live process standing in for an autopilot driver of `task_id`.
struct FakeDriver(std::process::Child);

impl FakeDriver {
    fn hold(sb: &Sandbox, task_id: i64) -> FakeDriver {
        let child = std::process::Command::new("sleep").arg("60").spawn().unwrap();
        let pid = child.id() as i32;
        let mut start = None;
        common::wait_until(Duration::from_secs(5), || {
            start = agent_brain::procs::proc_id(pid).map(|p| p.start as i64);
            start.is_some()
        });
        sb.db()
            .execute(
                "UPDATE tasks SET auto_state = 'running', auto_pid = ?1, auto_pid_start = ?2 WHERE id = ?3",
                rusqlite::params![pid as i64, start.expect("proc start"), task_id],
            )
            .unwrap();
        FakeDriver(child)
    }
}

impl Drop for FakeDriver {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn dev_commit_entry() -> Value {
    by_role(json!({
        "dev": [{"ops": [
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
        ]}],
    }))
}

// ---------------------------------------------------------------------------
// D2.2: the task works in its worktree
// ---------------------------------------------------------------------------

#[test]
fn lead_and_dev_run_in_task_worktree() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({}));
    auto_to_gate(&sb, "sua-app");

    let wt = expected_worktree(&sb, "sua-app");
    assert_eq!(task_field(&sb, 1, "workdir").as_deref(), Some(wt.to_str().unwrap()));
    assert!(wt.join(".plan.md").is_file(), "plan written in the worktree");
    assert_eq!(current_branch(&sb, &wt), "task/sua-app");
    assert_eq!(current_branch(&sb, &repo), "main", "the main repo keeps its branch");
    assert!(!repo.join(".plan.md").exists(), "nothing written in the main repo");
    assert_eq!(task_field(&sb, 1, "auto_state").as_deref(), Some("gate"));
    let records = sb.records();
    assert_eq!(records.len(), 2, "lead + plan review");
    for r in &records {
        assert_eq!(r["cwd"], wt.to_str().unwrap(), "{r}");
    }

    // Approve: never a checkout in the main repo; Dev runs in the worktree.
    let res = sb.run(&["approve", "1"], Some(dev_commit_entry()));
    let roles = roles_of_runs(&sb, 1);
    assert!(roles.iter().any(|r| r == "dev"), "{roles:?} {}", res.stderr);
    let records = sb.records();
    assert!(records.len() >= 3, "{records:?}");
    for r in &records {
        assert_eq!(r["cwd"], wt.to_str().unwrap(), "{r}");
    }
    assert_eq!(current_branch(&sb, &repo), "main");
    assert_eq!(
        std::fs::read_to_string(repo.join("src/app.py")).unwrap(),
        "def app():\n    return 1\n",
        "the main repo's tree is untouched"
    );
    assert_eq!(
        std::fs::read_to_string(wt.join("src/app.py")).unwrap(),
        "def app():\n    return 2\n"
    );
    let log = sb.git(&repo, &["log", "--format=%s", "task/sua-app"]);
    assert!(log.contains("sua-app: sửa app"), "{log}");

    // The dashboard reads the task's facts from its worktree.
    let srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();
    assert_eq!(t["workdir"], wt.to_str().unwrap(), "{t}");
}

#[test]
fn main_repo_changes_do_not_block_a_new_worktree_task() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({}));
    std::fs::write(repo.join("nhap.txt"), "nháp\n").unwrap();
    auto_to_gate(&sb, "sua-app");
    assert!(expected_worktree(&sb, "sua-app").is_dir());
    assert!(!expected_worktree(&sb, "sua-app").join("nhap.txt").exists());
    assert!(repo.join("nhap.txt").is_file());
}

// ---------------------------------------------------------------------------
// D2.2: max_parallel_tasks
// ---------------------------------------------------------------------------

#[test]
fn two_tasks_in_parallel_third_refused() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({ "max_parallel_tasks": 2 }));
    auto_to_gate(&sb, "viec-a");
    // No gate hold in worktree mode: task b starts while a waits at the gate.
    auto_to_gate(&sb, "viec-b");
    assert_eq!(task_count(&sb), 2);
    assert_eq!(current_branch(&sb, &repo), "main");
    assert!(expected_worktree(&sb, "viec-a").is_dir());
    assert!(expected_worktree(&sb, "viec-b").is_dir());

    // Task a's driver is alive; task b's approval still runs on, in its own worktree.
    let driver_a = FakeDriver::hold(&sb, 1);
    let res = sb.run(&["approve", "2"], Some(dev_commit_entry()));
    let roles = roles_of_runs(&sb, 2);
    assert!(roles.iter().any(|r| r == "dev"), "{roles:?} {}", res.stderr);
    let last = sb.records().last().cloned().unwrap();
    assert_eq!(last["cwd"], expected_worktree(&sb, "viec-b").to_str().unwrap());

    // Two live tasks: a third one waits.
    let driver_b = FakeDriver::hold(&sb, 2);
    let res = sb.run(
        &["auto", "--start", "--name", "viec-c", "--", PROJECT, "việc c"],
        Some(by_role(json!({ "lead": [lead_plan_entry()] }))),
    );
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(res.stderr.contains("max_parallel_tasks"), "stderr: {}", res.stderr);
    assert_eq!(task_count(&sb), 2);

    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({"project": PROJECT, "request": "việc c", "name": "viec-c", "workflow": "nhanh"}),
    );
    assert_eq!(resp.status, 409, "{}", resp.text());
    assert_eq!(resp.json()["kind"], "busy");
    drop(srv);

    // One task done: the third one starts.
    drop(driver_b);
    sb.db()
        .execute("UPDATE tasks SET auto_state = 'stopped' WHERE id = 2", [])
        .unwrap();
    let res = sb.run(
        &["run", "lead", "--name", "viec-c", "--", PROJECT, "việc c"],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "lead of task c");
    assert_eq!(task_count(&sb), 3);
    assert!(expected_worktree(&sb, "viec-c").is_dir());
    drop(driver_a);
}

// ---------------------------------------------------------------------------
// D2.4: cleanup
// ---------------------------------------------------------------------------

#[test]
fn cleanup_refuses_then_removes_merged_worktree_keeping_branch() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({}));
    auto_to_gate(&sb, "sua-app");
    let wt = expected_worktree(&sb, "sua-app");

    // Not done yet.
    let res = sb.run(&["task", "cleanup", "1", "--yes"], None);
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(res.stderr.contains("Chỉ dọn"), "stderr: {}", res.stderr);

    sb.db()
        .execute("UPDATE tasks SET status = 'qa_pass' WHERE id = 1", [])
        .unwrap();

    // A dirty worktree is refused, even confirmed.
    std::fs::write(wt.join("nhap.txt"), "nháp\n").unwrap();
    let res = sb.run(&["task", "cleanup", "1", "--yes"], None);
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(res.stderr.contains("nhap.txt"), "stderr: {}", res.stderr);
    assert!(wt.is_dir());
    std::fs::remove_file(wt.join("nhap.txt")).unwrap();

    // Not merged and not confirmed: refused; `--merged` skips it.
    let res = sb.run(&["task", "cleanup", "1"], None);
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(res.stderr.contains("chưa merge"), "stderr: {}", res.stderr);
    let res = sb.run(&["task", "cleanup", "--merged"], None);
    assert_ok(&res, "cleanup --merged (nothing merged)");
    assert!(wt.is_dir());

    // Merged into main: removed, the branch stays.
    sb.git(&repo, &["merge", "--no-ff", "--no-edit", "task/sua-app"]);
    let res = sb.run(&["task", "cleanup", "--merged"], None);
    assert_ok(&res, "cleanup --merged");
    assert!(!wt.exists(), "worktree removed");
    assert_eq!(task_field(&sb, 1, "workdir"), None);
    assert!(branch_exists(&repo, "task/sua-app"), "the branch is never deleted");
    assert_eq!(current_branch(&sb, &repo), "main");
}

#[test]
fn dashboard_cleanup_asks_confirmation_for_unmerged_branch() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({}));
    auto_to_gate(&sb, "sua-app");
    let wt = expected_worktree(&sb, "sua-app");
    sb.db()
        .execute("UPDATE tasks SET status = 'qa_pass' WHERE id = 1", [])
        .unwrap();

    let srv = sb.serve(&[]);
    let t = srv.get("/api/tasks/1").json();
    assert_eq!(t["workdir_merged"], false, "{t}");
    let ctl = t["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "cleanup_worktree")
        .cloned()
        .unwrap_or_else(|| panic!("no cleanup control: {}", t["controls"]));
    assert_eq!(ctl["enabled"], true, "{ctl}");

    let rev = t["rev"].as_str().unwrap().to_string();
    let resp = srv.post_json("/api/tasks/1/cleanup-worktree", &json!({ "rev": rev }));
    assert_eq!(resp.status, 409, "{}", resp.text());
    assert!(resp.json()["error"].as_str().unwrap().contains("chưa merge"));
    assert!(wt.is_dir());

    let resp = srv.post_json(
        "/api/tasks/1/cleanup-worktree",
        &json!({ "rev": rev, "confirm": true }),
    );
    assert_eq!(resp.status, 200, "{}", resp.text());
    assert_eq!(resp.json()["cleaned_up"], true);
    assert!(!wt.exists());
    assert!(branch_exists(&repo, "task/sua-app"));
    let t = srv.get("/api/tasks/1").json();
    assert!(t["workdir"].is_null(), "{t}");
    assert!(
        !t["controls"].as_array().unwrap().iter().any(|c| c["id"] == "cleanup_worktree"),
        "{}",
        t["controls"]
    );
}

// ---------------------------------------------------------------------------
// D2.5: task merge
// ---------------------------------------------------------------------------

fn commit_in(sb: &Sandbox, dir: &Path, file: &str, content: &str, msg: &str) {
    std::fs::write(dir.join(file), content).unwrap();
    sb.git(dir, &["add", file]);
    sb.git(dir, &["commit", "-m", msg]);
}

#[test]
fn task_merge_rules_and_conflicts() {
    let sb = Sandbox::new();
    let repo = setup(&sb, json!({}));
    auto_to_gate(&sb, "viec-a");
    auto_to_gate(&sb, "viec-b");
    let wt_a = expected_worktree(&sb, "viec-a");
    let wt_b = expected_worktree(&sb, "viec-b");
    commit_in(&sb, &wt_a, "a.txt", "a\n", "viec-a: a");

    // Only a QA-passed task.
    let res = sb.run(&["task", "merge", "1"], None);
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(res.stderr.contains("qua QA"), "stderr: {}", res.stderr);
    sb.db()
        .execute("UPDATE tasks SET status = 'qa_pass' WHERE id IN (1, 2)", [])
        .unwrap();

    // The main repo must be clean.
    std::fs::write(repo.join("nhap.txt"), "nháp\n").unwrap();
    let res = sb.run(&["task", "merge", "1"], None);
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(res.stderr.contains("nhap.txt"), "stderr: {}", res.stderr);
    std::fs::remove_file(repo.join("nhap.txt")).unwrap();

    let res = sb.run(&["task", "merge", "1"], None);
    assert_ok(&res, "merge task a");
    assert!(repo.join("a.txt").is_file());
    let merged = std::process::Command::new("git")
        .args(["merge-base", "--is-ancestor", "task/viec-a", "main"])
        .current_dir(&repo)
        .status()
        .unwrap();
    assert!(merged.success());

    // Task b: its plan file conflicts with a's (no protocol_dir: dropped), no other conflict.
    let res = sb.run(&["task", "merge", "2"], None);
    assert_ok(&res, "merge task b");
    assert!(res.stdout.contains(".plan.md"), "stdout: {}", res.stdout);
    assert_eq!(sb.git(&repo, &["status", "--porcelain"]).trim(), "");

    // A code conflict aborts the merge and leaves main as it was.
    commit_in(&sb, &repo, "src/app.py", "def app():\n    return 3\n", "main: 3");
    let res = sb.run(
        &["run", "lead", "--name", "viec-c", "--", PROJECT, "việc c"],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "lead of task c");
    let wt_c = expected_worktree(&sb, "viec-c");
    sb.git(&wt_c, &["reset", "--hard", "HEAD~2"]);
    commit_in(&sb, &wt_c, "src/app.py", "def app():\n    return 4\n", "viec-c: 4");
    sb.db()
        .execute("UPDATE tasks SET status = 'qa_pass' WHERE id = 3", [])
        .unwrap();
    let head_before = sb.git(&repo, &["rev-parse", "HEAD"]);
    let res = sb.run(&["task", "merge", "3"], None);
    assert_eq!(res.code, 1, "stderr: {}", res.stderr);
    assert!(res.stderr.contains("src/app.py"), "stderr: {}", res.stderr);
    assert_eq!(sb.git(&repo, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(sb.git(&repo, &["status", "--porcelain"]).trim(), "");
    let _ = wt_b;
}

// ---------------------------------------------------------------------------
// Dashboard endpoint and the default
// ---------------------------------------------------------------------------

#[test]
fn workspace_mode_endpoint_validates_and_stores() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(PROJECT, &[("a.txt", "a\n")], None, None);
    let entry = sb.project_entry(&repo, PROJECT, "work");
    sb.write_map(json!([entry]), &[]);
    let mut srv = sb.serve(&[]);

    let p = srv.get("/api/projects").json()["projects"][0].clone();
    assert_eq!(p["workspace"], "shared", "{p}");
    assert_eq!(p["max_parallel_tasks"], 1);

    for bad in [json!({"workspace": "pool"}), json!({"max_parallel_tasks": 0}), json!({"max_parallel_tasks": 11})] {
        let r = srv.put_json(&format!("/api/projects/{PROJECT}/workspace"), &bad);
        assert_eq!(r.status, 400, "{bad}");
    }
    let r = srv.put_json(
        &format!("/api/projects/{PROJECT}/workspace"),
        &json!({"workspace": "worktree", "max_parallel_tasks": 3}),
    );
    assert_eq!(r.status, 200, "{}", r.text());
    let entry = sb.read_map()["projects"][0].clone();
    assert_eq!(entry["workspace"], "worktree");
    assert_eq!(entry["max_parallel_tasks"], 3);
    let p = srv.get("/api/projects").json()["projects"][0].clone();
    assert_eq!(p["workspace"], "worktree");
    assert_eq!(p["max_parallel_tasks"], 3);

    let r = srv.put_json(
        &format!("/api/projects/{PROJECT}/workspace"),
        &json!({"workspace": "shared", "max_parallel_tasks": 1}),
    );
    assert_eq!(r.status, 200);
    let entry = sb.read_map()["projects"][0].clone();
    assert!(entry.get("workspace").is_none(), "{entry}");
    assert!(entry.get("max_parallel_tasks").is_none(), "{entry}");

    let exit = srv.stop(libc::SIGINT, Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn shared_default_has_no_worktree() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(PROJECT, &[("src/app.py", "def app():\n    return 1\n")], None, None);
    let entry = sb.project_entry(&repo, PROJECT, "work");
    sb.write_map(json!([entry]), &[]);
    let res = sb.run(
        &["run", "lead", "--name", "sua-app", "--", PROJECT, "sửa app"],
        Some(lead_plan_entry()),
    );
    assert_ok(&res, "run lead");
    assert_eq!(current_branch(&sb, &repo), "task/sua-app");
    assert_eq!(task_field(&sb, 1, "workdir"), None);
    assert!(!sb.xdg_data.join("agent-brain").join("worktrees").exists());
}
