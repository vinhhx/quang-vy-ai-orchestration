//! Housekeeping (`orchestrator::housekeeping`): tự dọn worktree của task đã xong và nhánh đã merge.
//! Mọi thứ chạy trong thư mục tạm: repo `git init`, DB tạm, `XDG_DATA_HOME` trỏ vào thư mục tạm nên
//! worktree nằm trong thư mục tạm; không bao giờ đụng dữ liệu hay dự án thật.

use agent_brain::app::Ctx;
use agent_brain::config::{config_from_value, Config, DEFAULT_CONFIG_JSON};
use agent_brain::db::{self, TaskRow};
use agent_brain::orchestrator::housekeeping::{delete_merged_branches, tick_with};
use agent_brain::projects::Project;
use agent_brain::workspace::{cleanup_task, create_task_worktree};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const DAY_MS: i64 = 86_400_000;

/// The data dir comes from the environment: one test at a time sets it.
static ENV: Mutex<()> = Mutex::new(());

fn git(root: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn branch_exists(repo: &Path, branch: &str) -> bool {
    std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")])
        .output()
        .unwrap()
        .status
        .success()
}

struct Env {
    _lock: std::sync::MutexGuard<'static, ()>,
    _data: tempfile::TempDir,
    tmp: tempfile::TempDir,
    old: Option<std::ffi::OsString>,
    repo: PathBuf,
    conn: Connection,
    pid: i64,
    project: Project,
    cfg: Config,
}

impl Drop for Env {
    fn drop(&mut self) {
        match &self.old {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
    }
}

impl Env {
    fn new() -> Env {
        Env::with(json!({}))
    }

    /// `hk`: the `housekeeping` block of config.json.
    fn with(hk: Value) -> Env {
        let lock = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let data = tempfile::tempdir().unwrap();
        let old = std::env::var_os("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", data.path());
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        std::fs::write(repo.join("README.md"), "# demo\n").unwrap();
        git(&repo, &["add", "--", "README.md"]);
        git(&repo, &["commit", "-q", "-m", "init"]);
        let repo = repo.canonicalize().unwrap();

        let project = Project::from_value(&json!({
            "id": "web", "path": repo.to_string_lossy(), "name_vi": "web", "aliases": [],
            "type": "code", "repo": null, "profile": "work", "missing": false,
            "workspace": "worktree", "protocol_dir": ""
        }))
        .unwrap();
        let conn = db::open(&tmp.path().join("brain.db")).unwrap();
        db::sync_projects(&conn, std::slice::from_ref(&project)).unwrap();
        let pid = db::project_row_id(&conn, "web").unwrap();

        let mut v: Value = serde_json::from_str(DEFAULT_CONFIG_JSON).unwrap();
        v["housekeeping"] = hk;
        let cfg = config_from_value(&v).unwrap();
        Env {
            _lock: lock,
            _data: data,
            tmp,
            old,
            repo,
            conn,
            pid,
            project,
            cfg,
        }
    }

    /// A task with its own worktree on `task/<slug>` (one commit of its own), created from `from`.
    /// `status`: the task status; `merged`: merge the branch into `main` (the repo stays on main).
    fn task_from(&self, slug: &str, from: &str, status: &str, merged: bool) -> i64 {
        let branch = format!("task/{slug}");
        let wt = create_task_worktree(&self.repo, "web", slug, &branch, from, false).unwrap();
        std::fs::write(wt.join(format!("{slug}.txt")), "x\n").unwrap();
        git(&wt, &["add", "--", &format!("{slug}.txt")]);
        git(&wt, &["commit", "-q", "-m", &format!("work {slug}")]);
        if merged {
            git(&self.repo, &["merge", "--no-ff", "--no-edit", &branch]);
        }
        let id = db::upsert_task(&self.conn, self.pid, slug, slug, &branch, Some(from), None).unwrap();
        db::set_task_status(&self.conn, id, status).unwrap();
        db::set_task_workdir(&self.conn, id, Some(&wt.to_string_lossy())).unwrap();
        self.age(id, 0);
        id
    }

    fn task(&self, slug: &str, status: &str, merged: bool) -> i64 {
        self.task_from(slug, "main", status, merged)
    }

    /// Makes the task last updated `days` days ago.
    fn age(&self, id: i64, days: i64) {
        self.conn
            .execute(
                "UPDATE tasks SET updated_at = ?1 WHERE id = ?2",
                rusqlite::params![db::now_ms() - days * DAY_MS, id],
            )
            .unwrap();
    }

    fn row(&self, id: i64) -> TaskRow {
        db::get_task(&self.conn, id).unwrap().unwrap()
    }

    fn wt(&self, id: i64) -> Option<PathBuf> {
        self.row(id).workdir.map(PathBuf::from)
    }

    fn run(&self, free_gb: Option<u64>) -> Vec<String> {
        self.run_busy(free_gb, &HashSet::new())
    }

    fn run_busy(&self, free_gb: Option<u64>, busy: &HashSet<i64>) -> Vec<String> {
        tick_with(
            &self.cfg,
            std::slice::from_ref(&self.project),
            &self.conn,
            db::now_ms(),
            free_gb,
            busy,
        )
        .unwrap()
    }

    /// Removes the task's worktree the normal way (merged: no confirmation needed).
    fn remove_worktree(&self, id: i64) {
        let t = self.row(id);
        cleanup_task(&self.conn, &self.project, &self.repo, &t, true)
            .unwrap()
            .expect("cleanup");
    }
}

fn branch_of(id: i64, env: &Env) -> String {
    env.row(id).branch
}

// (a) done + merged + clean: worktree and branch are removed.
#[test]
fn merged_clean_task_loses_worktree_and_branch() {
    let env = Env::new();
    let id = env.task("a", "qa_pass", true);
    let wt = env.wt(id).unwrap();
    assert!(wt.is_dir());
    let lines = env.run(None);
    assert!(!wt.exists(), "worktree removed");
    assert_eq!(env.row(id).workdir, None);
    assert!(!branch_exists(&env.repo, "task/a"), "merged branch deleted");
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("task/a")), "{lines:?}");
    // the main repo is untouched
    assert_eq!(git(&env.repo, &["branch", "--show-current"]).trim(), "main");
    assert_eq!(git(&env.repo, &["status", "--porcelain"]), "");
    // a second pass has nothing to do
    assert!(env.run(None).is_empty());
}

// An analysis task (answered, branch analysis/*) is swept like a task branch.
#[test]
fn answered_analysis_branch_is_swept() {
    let env = Env::new();
    let wt_branch = "analysis/q";
    let wt = create_task_worktree(&env.repo, "web", "q", wt_branch, "main", false).unwrap();
    let id = db::upsert_task(&env.conn, env.pid, "q", "q", wt_branch, Some("main"), None).unwrap();
    db::set_task_status(&env.conn, id, "answered").unwrap();
    db::set_task_workdir(&env.conn, id, Some(&wt.to_string_lossy())).unwrap();
    env.run(None);
    assert!(!wt.exists());
    assert!(!branch_exists(&env.repo, wt_branch), "same sha as main counts as merged");
}

// (b) unmerged and recent: worktree and branch stay.
#[test]
fn unmerged_recent_task_is_kept() {
    let env = Env::new();
    let id = env.task("b", "qa_pass", false);
    let wt = env.wt(id).unwrap();
    assert!(env.run(Some(500)).is_empty());
    assert!(wt.is_dir());
    assert_eq!(env.row(id).workdir.as_deref(), wt.to_str());
    assert!(branch_exists(&env.repo, "task/b"));
}

// (c) unmerged and older than keep_unmerged_days: worktree goes, branch stays.
#[test]
fn unmerged_old_task_loses_only_the_worktree() {
    let env = Env::new();
    let id = env.task("c", "qa_pass", false);
    let wt = env.wt(id).unwrap();
    env.age(id, 6);
    assert!(env.run(None).is_empty(), "6 days is within the 7 days kept");
    assert!(wt.is_dir());
    env.age(id, 8);
    let lines = env.run(None);
    assert!(!wt.exists());
    assert_eq!(env.row(id).workdir, None);
    assert!(branch_exists(&env.repo, "task/c"), "an unmerged branch is never deleted");
    assert_eq!(lines.len(), 1, "{lines:?}");
    // later passes never delete it either, however old
    env.age(id, 300);
    env.run(Some(0));
    assert!(branch_exists(&env.repo, "task/c"));
}

#[test]
fn keep_unmerged_days_comes_from_the_config() {
    let env = Env::with(json!({"keep_unmerged_days": 30}));
    let id = env.task("c2", "qa_pass", false);
    env.age(id, 20);
    assert!(env.run(None).is_empty());
    assert!(env.wt(id).is_some());
    env.age(id, 31);
    env.run(None);
    assert!(env.wt(id).is_none());
}

// (d) unmerged and recent, but the disk is nearly full: worktree goes, branch stays.
#[test]
fn low_disk_cleans_unmerged_worktrees_but_keeps_branches() {
    let env = Env::new();
    let id = env.task("d", "qa_pass", false);
    let wt = env.wt(id).unwrap();
    // unknown, enough, exactly at the limit: nothing
    for free in [None, Some(25), Some(20)] {
        assert!(env.run(free).is_empty(), "free={free:?}");
        assert!(wt.is_dir());
    }
    let lines = env.run(Some(5));
    assert!(!wt.exists());
    assert_eq!(env.row(id).workdir, None);
    assert!(branch_exists(&env.repo, "task/d"));
    assert_eq!(lines.len(), 1, "{lines:?}");
}

#[test]
fn min_free_gb_zero_turns_the_disk_rule_off() {
    let env = Env::with(json!({"min_free_gb": 0}));
    let id = env.task("d0", "qa_pass", false);
    assert!(env.run(Some(0)).is_empty());
    assert!(env.wt(id).is_some());
}

// (e) a dirty worktree is kept, merged or not, old or not.
#[test]
fn dirty_worktree_is_kept() {
    let env = Env::new();
    let merged = env.task("e1", "qa_pass", true);
    let old = env.task("e2", "qa_pass", false);
    env.age(old, 30);
    let w1 = env.wt(merged).unwrap();
    let w2 = env.wt(old).unwrap();
    std::fs::write(w1.join("nhap.txt"), "nháp\n").unwrap();
    std::fs::write(w2.join("nhap.txt"), "nháp\n").unwrap();
    let lines = env.run(Some(1));
    assert!(lines.is_empty(), "{lines:?}");
    assert!(w1.join("nhap.txt").is_file() && w2.join("nhap.txt").is_file());
    assert_eq!(env.row(merged).workdir.as_deref(), w1.to_str());
    assert_eq!(env.row(old).workdir.as_deref(), w2.to_str());
    // the merged branch has a worktree: its branch stays too
    assert!(branch_exists(&env.repo, "task/e1"));
    assert!(branch_exists(&env.repo, "task/e2"));
}

// (f) a task with a live run, a live autopilot driver, or a live job is kept.
#[test]
fn busy_task_is_kept() {
    let env = Env::new();
    let with_run = env.task("f1", "qa_pass", true);
    let with_auto = env.task("f2", "qa_pass", true);
    let with_job = env.task("f3", "qa_pass", true);
    let me = std::process::id() as i32;
    let start = agent_brain::procs::proc_id(me).expect("own proc id").start as i64;
    db::insert_run(
        &env.conn,
        &db::NewRun {
            task_id: Some(with_run),
            project_id: env.pid,
            role: "dev".into(),
            profile: "work".into(),
            model: None,
            pid: me as i64,
            pgid: me as i64,
            pid_start: start,
            base_commit: None,
            run_dir: None,
        },
    )
    .unwrap();
    env.conn
        .execute(
            "UPDATE tasks SET auto_state = 'running', auto_pid = ?1, auto_pid_start = ?2 WHERE id = ?3",
            rusqlite::params![me as i64, start, with_auto],
        )
        .unwrap();
    let busy: HashSet<i64> = [with_job].into_iter().collect();
    let lines = env.run_busy(None, &busy);
    assert!(lines.is_empty(), "{lines:?}");
    for id in [with_run, with_auto, with_job] {
        assert!(env.wt(id).is_some_and(|w| w.is_dir()), "task {id}");
        assert!(branch_exists(&env.repo, &branch_of(id, &env)));
    }
    // once the job is gone the task is cleaned (the run and driver rows are still live)
    env.run(None);
    assert!(env.wt(with_job).is_none());
    assert!(env.wt(with_run).is_some() && env.wt(with_auto).is_some());
}

// (g) the branch is the base of an unfinished task: worktree goes, branch stays until that task is done.
#[test]
fn base_of_an_unfinished_task_keeps_its_branch() {
    let env = Env::new();
    let a = env.task("g1", "qa_pass", true);
    let b = env.task_from("g2", "task/g1", "planned", false);
    let wb = env.wt(b).unwrap();
    let lines = env.run(None);
    assert!(env.wt(a).is_none(), "the finished task's worktree is cleaned");
    assert!(branch_exists(&env.repo, "task/g1"), "stacked base branch kept");
    assert_eq!(lines.len(), 1, "{lines:?}");
    // the unfinished task is not touched
    assert!(wb.is_dir());
    assert_eq!(env.row(b).workdir.as_deref(), wb.to_str());
    assert!(branch_exists(&env.repo, "task/g2"));
    // the stacked task finishes but is not merged yet (and has its worktree): it still needs its base
    db::set_task_status(&env.conn, b, "qa_pass").unwrap();
    env.run(None);
    assert!(branch_exists(&env.repo, "task/g1"), "a finished, unmerged child keeps its base");
    assert!(branch_exists(&env.repo, "task/g2"), "g2 is not merged");
    assert!(wb.is_dir(), "g2 is unmerged and recent");
}

// A finished child whose own branch is not merged into its base still needs that base branch.
// `parent` (finished, merged into main, no worktree) is the base of `child` (qa_pass).
fn parent_and_finished_child(env: &Env) -> (i64, i64) {
    let parent = env.task("p", "qa_pass", true);
    let child = env.task_from("c", "task/p", "qa_pass", false);
    env.remove_worktree(parent);
    env.remove_worktree(child);
    (parent, child)
}

#[test]
fn base_of_a_finished_but_unmerged_child_is_kept() {
    let env = Env::new();
    let (parent, _child) = parent_and_finished_child(&env);
    let lines = env.run(None);
    assert!(lines.is_empty(), "{lines:?}");
    assert!(branch_exists(&env.repo, &branch_of(parent, &env)), "parent branch kept");
    assert!(branch_exists(&env.repo, "task/c"), "unmerged child kept");
    // the helper path used by `task cleanup --merged --branches` agrees
    assert!(delete_merged_branches(&env.conn, &env.project, &env.repo).is_empty());
    assert!(branch_exists(&env.repo, "task/p"));
}

#[test]
fn base_of_a_child_merged_into_it_is_deleted() {
    let env = Env::new();
    let (_parent, _child) = parent_and_finished_child(&env);
    // the parent branch moves onto the child's commit: the child is now merged into it; then the
    // parent (with the child's work) is merged into main
    git(&env.repo, &["branch", "-f", "task/p", "task/c"]);
    git(&env.repo, &["merge", "--no-ff", "--no-edit", "task/p"]);
    env.run(None);
    assert!(!branch_exists(&env.repo, "task/p"), "child merged into the parent: parent is free");
}

#[test]
fn base_of_a_child_with_a_missing_branch_is_deleted() {
    let env = Env::new();
    let (_parent, _child) = parent_and_finished_child(&env);
    // the child's branch is gone (plain ref deletion in the temp repo)
    git(&env.repo, &["update-ref", "-d", "refs/heads/task/c"]);
    env.run(None);
    assert!(!branch_exists(&env.repo, "task/p"), "the child's branch is gone: parent is free");
}

#[test]
fn base_of_a_child_with_a_worktree_is_kept() {
    let env = Env::new();
    let parent = env.task("p", "qa_pass", true);
    let child = env.task_from("c", "task/p", "qa_pass", false);
    env.remove_worktree(parent);
    // the child's branch is merged into the parent, but the child still has its worktree
    git(&env.repo, &["branch", "-f", "task/p", "task/c"]);
    git(&env.repo, &["merge", "--no-ff", "--no-edit", "task/p"]);
    let lines = delete_merged_branches(&env.conn, &env.project, &env.repo);
    assert!(lines.is_empty(), "{lines:?}");
    assert!(branch_exists(&env.repo, "task/p"));
    assert!(env.wt(child).is_some_and(|w| w.is_dir()));
}

// (h) the branch checked out in the main repo is never deleted.
#[test]
fn checked_out_branch_is_kept() {
    let env = Env::new();
    let id = env.task("h", "qa_pass", true);
    env.remove_worktree(id);
    git(&env.repo, &["checkout", "-q", "task/h"]);
    let lines = env.run(None);
    assert!(lines.is_empty(), "{lines:?}");
    assert!(branch_exists(&env.repo, "task/h"));
    assert_eq!(git(&env.repo, &["branch", "--show-current"]).trim(), "task/h");
}

// (i) delete_merged_branches = false: the worktree goes, the branch stays.
#[test]
fn branches_are_kept_when_deleting_is_off() {
    let env = Env::with(json!({"delete_merged_branches": false}));
    let id = env.task("i", "qa_pass", true);
    let wt = env.wt(id).unwrap();
    let lines = env.run(None);
    assert!(!wt.exists());
    assert!(branch_exists(&env.repo, "task/i"));
    assert_eq!(lines.len(), 1, "{lines:?}");
}

// (j) enabled = false: nothing happens.
#[test]
fn disabled_does_nothing() {
    let env = Env::with(json!({"enabled": false}));
    let merged = env.task("j1", "qa_pass", true);
    let old = env.task("j2", "qa_pass", false);
    env.age(old, 100);
    let w = env.wt(merged).unwrap();
    assert!(env.run(Some(0)).is_empty());
    assert!(w.is_dir());
    assert!(env.wt(old).is_some());
    assert!(branch_exists(&env.repo, "task/j1") && branch_exists(&env.repo, "task/j2"));
}

// (k) an unfinished task is never touched, whatever its branch and age.
#[test]
fn unfinished_task_is_untouched() {
    let env = Env::new();
    let planned = env.task("k1", "planned", true);
    let failed = env.task("k2", "qa_fail", true);
    env.age(planned, 100);
    let ended = env.task("k3", "planned", true);
    env.conn
        .execute("UPDATE tasks SET auto_state = 'ended' WHERE id = ?1", [ended])
        .unwrap();
    let lines = env.run(Some(0));
    assert!(lines.is_empty(), "{lines:?}");
    for id in [planned, failed, ended] {
        assert!(env.wt(id).is_some_and(|w| w.is_dir()), "task {id}");
        assert!(branch_exists(&env.repo, &branch_of(id, &env)));
    }
}

// (l) a branch that is not in the tasks table is never touched.
#[test]
fn branches_outside_the_tasks_table_are_untouched() {
    let env = Env::new();
    for b in ["task/stray", "analysis/stray", "feature/mine"] {
        git(&env.repo, &["branch", b, "main"]);
    }
    // a done task whose branch does not use a task prefix is left alone as well
    git(&env.repo, &["branch", "feat/custom", "main"]);
    let id = db::upsert_task(&env.conn, env.pid, "custom", "c", "feat/custom", Some("main"), None).unwrap();
    db::set_task_status(&env.conn, id, "qa_pass").unwrap();
    // and a real merged task next to them is swept
    let a = env.task("l", "qa_pass", true);
    let lines = env.run(None);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(env.wt(a).is_none());
    assert!(!branch_exists(&env.repo, "task/l"));
    for b in ["task/stray", "analysis/stray", "feature/mine", "feat/custom", "main"] {
        assert!(branch_exists(&env.repo, b), "{b}");
    }
}

// A project in shared mode is never swept.
#[test]
fn shared_mode_project_is_skipped() {
    let mut env = Env::new();
    let id = env.task("s", "qa_pass", true);
    env.project = Project::from_value(&json!({
        "id": "web", "path": env.repo.to_string_lossy(), "name_vi": "web", "aliases": [],
        "type": "code", "repo": null, "profile": "work", "missing": false,
        "workspace": "shared", "protocol_dir": ""
    }))
    .unwrap();
    assert!(env.run(Some(0)).is_empty());
    assert!(env.wt(id).is_some());
    assert!(branch_exists(&env.repo, "task/s"));
}

fn ctx_of(env: &Env) -> Ctx {
    let map = json!({"scan_roots": [], "projects": [env.project_entry()]});
    Ctx {
        cfg: env.cfg.clone(),
        conn: db::open(&env.tmp.path().join("brain.db")).unwrap(),
        map,
        projects: vec![env.project.clone()],
    }
}

impl Env {
    fn project_entry(&self) -> Value {
        json!({
            "id": "web", "path": self.repo.to_string_lossy(), "name_vi": "web", "aliases": [],
            "type": "code", "repo": null, "profile": "work", "missing": false,
            "workspace": "worktree", "protocol_dir": ""
        })
    }
}

// (m) `task cleanup --merged [--branches]`.
#[test]
fn cleanup_merged_without_the_flag_keeps_branches() {
    let env = Env::new();
    let merged = env.task("m1", "qa_pass", true);
    let unmerged = env.task("m2", "qa_pass", false);
    let mut ctx = ctx_of(&env);
    let code = agent_brain::workspace::cmd_cleanup(&mut ctx, None, true, false, false).unwrap();
    assert_eq!(code, 0);
    assert!(env.wt(merged).is_none(), "the merged worktree is cleaned as before");
    assert!(branch_exists(&env.repo, "task/m1"), "no flag, no branch deletion");
    assert!(env.wt(unmerged).is_some());
    assert!(branch_exists(&env.repo, "task/m2"));
}

#[test]
fn cleanup_merged_with_branches_deletes_only_merged_branches() {
    let env = Env::new();
    let merged = env.task("m3", "qa_pass", true);
    let unmerged = env.task("m4", "qa_pass", false);
    // a merged branch whose worktree was cleaned earlier is swept too
    let earlier = env.task("m5", "qa_pass", true);
    env.remove_worktree(earlier);
    let mut ctx = ctx_of(&env);
    let code = agent_brain::workspace::cmd_cleanup(&mut ctx, None, true, false, true).unwrap();
    assert_eq!(code, 0);
    assert!(env.wt(merged).is_none());
    assert!(!branch_exists(&env.repo, "task/m3"));
    assert!(!branch_exists(&env.repo, "task/m5"));
    assert!(env.wt(unmerged).is_some_and(|w| w.is_dir()), "unmerged worktree untouched");
    assert!(branch_exists(&env.repo, "task/m4"), "unmerged branch kept");
    assert_eq!(git(&env.repo, &["branch", "--show-current"]).trim(), "main");
    // --branches alone, with a task id, is refused
    let err = agent_brain::workspace::cmd_cleanup(&mut ctx, Some(unmerged), false, false, true)
        .expect_err("--branches needs --merged");
    assert!(err.message.contains("--branches"), "{}", err.message);
}

#[test]
fn delete_merged_branches_reports_what_it_did() {
    let env = Env::new();
    let id = env.task("r", "qa_pass", true);
    env.remove_worktree(id);
    let lines = delete_merged_branches(&env.conn, &env.project, &env.repo);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("task/r"), "{lines:?}");
    assert!(!branch_exists(&env.repo, "task/r"));
    assert!(delete_merged_branches(&env.conn, &env.project, &env.repo).is_empty());
}

// (n) configuration: out-of-range or wrong-type values are refused, with the key in the message.
#[test]
fn bad_housekeeping_config_is_refused() {
    let base: Value = serde_json::from_str(DEFAULT_CONFIG_JSON).unwrap();
    let default = config_from_value(&base).unwrap().housekeeping;
    assert!(default.enabled && default.delete_merged_branches);
    assert_eq!(
        (default.interval_minutes, default.keep_unmerged_days, default.min_free_gb),
        (10, 7, 20)
    );
    for (key, val) in [
        ("interval_minutes", json!(0)),
        ("interval_minutes", json!(1441)),
        ("keep_unmerged_days", json!(0)),
        ("keep_unmerged_days", json!(366)),
        ("min_free_gb", json!(2001)),
        ("min_free_gb", json!("20")),
        ("enabled", json!("true")),
        ("delete_merged_branches", json!(null)),
    ] {
        let mut v = base.clone();
        v["housekeeping"] = json!({ key: val.clone() });
        let e = config_from_value(&v).expect_err(&format!("{key}={val}"));
        assert!(e.message.contains(&format!("housekeeping.{key}")), "{key}={val}: {}", e.message);
    }
}

// The scheduler tick runs housekeeping on its own (also with the orchestrator off), and only once per interval.
#[test]
fn scheduler_tick_runs_housekeeping_once_per_interval() {
    use agent_brain::dashboard::jobs::JobManager;
    use agent_brain::dashboard::AppState;
    let env = Env::new();
    let first = env.task("t1", "qa_pass", true);
    let first_wt = env.wt(first).unwrap();
    let map_path = env.tmp.path().join("projects_map.json");
    std::fs::write(
        &map_path,
        serde_json::to_string(&json!({"scan_roots": [], "projects": [env.project_entry()]})).unwrap(),
    )
    .unwrap();
    let exe = PathBuf::from("/bin/true");
    let mut orch = env.cfg.orchestrator.clone();
    orch.enabled = false;
    let st = AppState {
        orch: std::sync::RwLock::new(orch),
        cfg: env.cfg.clone(),
        port: 0,
        token: "0".repeat(64),
        map_path,
        db_path: env.tmp.path().join("brain.db"),
        exe: exe.clone(),
        hints: tokio::sync::broadcast::channel(16).0,
        shutdown: tokio::sync::watch::channel(false).1,
        jobs: JobManager::new(exe),
    };
    assert!(!st.orch().enabled, "the orchestrator is off: housekeeping runs anyway");
    agent_brain::orchestrator::scheduler::schedule_tick(&st).unwrap();
    assert!(!first_wt.exists(), "cleaned by the tick, no command needed");
    assert!(!branch_exists(&env.repo, "task/t1"));
    // a second finished task right after: the interval (10 minutes) has not passed
    let second = env.task("t2", "qa_pass", true);
    let second_wt = env.wt(second).unwrap();
    agent_brain::orchestrator::scheduler::schedule_tick(&st).unwrap();
    assert!(second_wt.is_dir(), "not due yet");
    assert!(branch_exists(&env.repo, "task/t2"));
    // an explicit pass cleans it
    env.run(None);
    assert!(!second_wt.exists());
}
