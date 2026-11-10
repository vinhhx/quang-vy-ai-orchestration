//! Merge & push after QA (`agent-brain task merge <id> --push`). Real git: a local bare
//! repository is `origin`, the project repo and a second clone live in the Sandbox. Every case
//! runs the binary, so git sees only the Sandbox's HOME and git config.

mod common;

use common::{RunOut, Sandbox};
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const BRANCH: &str = "task/sua-app";

struct World {
    repo: PathBuf,
    remote: Option<PathBuf>,
}

/// `git` in `dir` with the Sandbox's config; `None` when it fails.
fn try_git(sb: &Sandbox, dir: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", &sb.home)
        .env("GIT_CONFIG_GLOBAL", &sb.git_config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("run git");
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn sha(sb: &Sandbox, dir: &Path, rev: &str) -> String {
    sb.git(dir, &["rev-parse", rev]).trim().to_string()
}

fn has_branch(sb: &Sandbox, dir: &Path, b: &str) -> bool {
    try_git(sb, dir, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{b}")]).is_some()
}

fn current(sb: &Sandbox, dir: &Path) -> String {
    sb.git(dir, &["branch", "--show-current"]).trim().to_string()
}

fn commit_file(sb: &Sandbox, dir: &Path, file: &str, content: &str, msg: &str) {
    let p = dir.join(file);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(p, content).unwrap();
    sb.git(dir, &["add", "--", file]);
    sb.git(dir, &["commit", "-q", "-m", msg]);
}

/// First-parent subjects of `rev`, newest first.
fn first_parent_subjects(sb: &Sandbox, dir: &Path, rev: &str) -> Vec<String> {
    sb.git(dir, &["log", "--first-parent", "--format=%s", rev])
        .lines()
        .map(str::to_string)
        .collect()
}

fn marked(subjects: &[String], task_id: i64) -> usize {
    let mark = format!("[task #{task_id}]");
    subjects.iter().filter(|s| s.contains(&mark)).count()
}

/// Project `web`: `main` and `dev` (pushed to `origin` when `with_origin`), the task branch
/// `task/sua-app` from `dev` with two commits, checked out and clean. Task 1 `qa_pass` with
/// `task_base` as its base branch. `entry_extra` is merged into the map entry.
fn setup(sb: &Sandbox, with_origin: bool, task_base: Option<&str>, entry_extra: serde_json::Value) -> World {
    let remote = with_origin.then(|| {
        let dir = sb.root.join("remote.git");
        std::fs::create_dir_all(&dir).unwrap();
        sb.git(&dir, &["init", "-q", "--bare", "-b", "main"]);
        dir
    });
    let repo = sb.make_repo(
        "web",
        &[("src/a.txt", "a\n")],
        None,
        remote.as_ref().map(|r| r.to_str().unwrap()),
    );
    sb.git(&repo, &["checkout", "-q", "-b", "dev"]);
    commit_file(sb, &repo, "src/base.txt", "base\n", "dev: khởi tạo");
    if remote.is_some() {
        sb.git(&repo, &["push", "-q", "origin", "main", "dev"]);
        sb.git(&repo, &["fetch", "-q", "origin"]);
    }
    sb.git(&repo, &["checkout", "-q", "-b", BRANCH]);
    commit_file(sb, &repo, "src/a.txt", "a2\n", "sua-app: sửa a");
    commit_file(sb, &repo, "src/b.txt", "b\n", "sua-app: thêm b");

    let mut entry = sb.project_entry(&repo, "web", "work");
    if let Some(obj) = entry_extra.as_object() {
        for (k, v) in obj {
            entry[k] = v.clone();
        }
    }
    sb.write_map(json!([entry]), &[]);

    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let now = agent_brain::db::now_ms();
    conn.execute(
        "INSERT INTO projects (id, slug, path) VALUES (1, 'web', ?1)",
        rusqlite::params![repo.to_str().unwrap()],
    )
    .unwrap();
    insert_task(sb, 1, "sua-app", BRANCH, task_base, "qa_pass", now);
    World { repo, remote }
}

fn insert_task(sb: &Sandbox, id: i64, slug: &str, branch: &str, base: Option<&str>, status: &str, now: i64) {
    sb.db()
        .execute(
            "INSERT INTO tasks (id, project_id, slug, title, branch, base_branch, status, created_at, updated_at)
             VALUES (?1, 1, ?2, 'Sửa app thanh toán', ?3, ?4, ?5, ?6, ?6)",
            rusqlite::params![id, slug, branch, base, status, now],
        )
        .unwrap();
}

fn set_status(sb: &Sandbox, id: i64, status: &str) {
    sb.db()
        .execute("UPDATE tasks SET status = ?1 WHERE id = ?2", rusqlite::params![status, id])
        .unwrap();
}

fn merge_push(sb: &Sandbox, id: i64) -> RunOut {
    sb.run(&["task", "merge", &id.to_string(), "--push"], None)
}

fn assert_code(res: &RunOut, code: i32, what: &str) {
    assert_eq!(
        res.code, code,
        "{what}: stdout:\n{}\nstderr:\n{}",
        res.stdout, res.stderr
    );
}

/// A second clone of origin that pushes one commit to `dev` (someone else's work).
fn push_from_other_clone(sb: &Sandbox, remote: &Path, file: &str, content: &str, msg: &str) {
    let other = sb.root.join("other");
    if !other.join(".git").exists() {
        sb.git(&sb.root, &["clone", "-q", remote.to_str().unwrap(), other.to_str().unwrap()]);
    }
    sb.git(&other, &["fetch", "-q", "origin"]);
    sb.git(&other, &["checkout", "-q", "-B", "dev", "origin/dev"]);
    commit_file(sb, &other, file, content, msg);
    sb.git(&other, &["push", "-q", "origin", "dev"]);
}

// 1. Happy path.
#[test]
fn merges_pushes_and_deletes_the_task_branch() {
    let sb = Sandbox::new();
    let w = setup(&sb, true, Some("dev"), json!({}));
    let remote = w.remote.as_ref().unwrap();
    let tip = sha(&sb, &w.repo, BRANCH);
    let dev_before = sha(&sb, &w.repo, "dev");

    let res = merge_push(&sb, 1);
    assert_code(&res, 0, "merge --push");
    assert!(
        res.stdout.contains("Đã merge 'task/sua-app' vào 'dev' (") && res.stdout.contains("và push lên origin."),
        "{}",
        res.stdout
    );
    assert!(res.stdout.contains("Đã xoá nhánh cục bộ 'task/sua-app'."), "{}", res.stdout);

    // origin/dev is the local dev, which is one merge on top of the old dev.
    assert_eq!(sha(&sb, &w.repo, "dev"), sha(&sb, remote, "dev"));
    assert_eq!(sha(&sb, &w.repo, "dev^1"), dev_before);
    assert_eq!(sha(&sb, &w.repo, "dev^2"), tip);
    let subject = sb.git(&w.repo, &["log", "-1", "--format=%s", "dev"]);
    assert_eq!(
        subject.trim(),
        "Merge task/sua-app vào dev [task #1]: Sửa app thanh toán"
    );
    let body = sb.git(&w.repo, &["log", "-1", "--format=%b", "dev"]);
    assert!(body.contains("Dự án: web") && body.contains("Task: #1 · QA: đạt"), "{body}");
    let subjects = first_parent_subjects(&sb, &w.repo, "dev");
    assert_eq!(marked(&subjects, 1), 1, "{subjects:?}");
    assert_eq!(subjects.len(), 3, "one line for the task on top of dev: {subjects:?}");

    assert!(!has_branch(&sb, &w.repo, BRANCH), "the task branch is deleted");
    assert!(
        try_git(&sb, remote, &["rev-parse", "--verify", "--quiet", "refs/heads/task/sua-app"]).is_none(),
        "the task branch is never pushed"
    );
    assert_eq!(current(&sb, &w.repo), "dev");
    assert_eq!(sb.git(&w.repo, &["status", "--porcelain"]).trim(), "");
    // The log lines of every step, without the remote's path.
    for step in ["task 1: fetch origin dev", "task 1: checkout dev", "task 1: merge task/sua-app vào dev", "task 1: push origin dev", "task 1: đã push dev (", "task 1: đã xoá nhánh task/sua-app"] {
        assert!(res.stderr.contains(step), "missing {step:?} in:\n{}", res.stderr);
    }
    assert!(!res.stderr.contains(remote.to_str().unwrap()), "{}", res.stderr);
}

// 2. The local base is behind origin: fast-forward, then merge and push.
#[test]
fn behind_origin_fast_forwards_then_pushes() {
    let sb = Sandbox::new();
    let w = setup(&sb, true, Some("dev"), json!({}));
    let remote = w.remote.as_ref().unwrap();
    push_from_other_clone(&sb, remote, "src/other.txt", "o\n", "người khác: thêm other");
    let theirs = sha(&sb, remote, "dev");

    let res = merge_push(&sb, 1);
    assert_code(&res, 0, "merge --push behind");
    assert_eq!(sha(&sb, &w.repo, "dev"), sha(&sb, remote, "dev"));
    assert_eq!(sha(&sb, &w.repo, "dev^1"), theirs, "merged on top of the fast-forwarded dev");
    assert!(res.stderr.contains("fast-forward dev theo origin/dev"), "{}", res.stderr);
    assert!(w.repo.join("src/other.txt").is_file());
}

// 3. A conflict on a code file: refused, the merge aborted, nothing changed.
#[test]
fn code_conflict_aborts_and_changes_nothing() {
    let sb = Sandbox::new();
    let w = setup(&sb, true, Some("dev"), json!({}));
    let remote = w.remote.as_ref().unwrap();
    // dev (local and origin) changes src/a.txt differently from the task.
    sb.git(&w.repo, &["checkout", "-q", "dev"]);
    commit_file(&sb, &w.repo, "src/a.txt", "khác\n", "dev: sửa a");
    sb.git(&w.repo, &["push", "-q", "origin", "dev"]);
    sb.git(&w.repo, &["checkout", "-q", BRANCH]);
    let dev_before = sha(&sb, &w.repo, "dev");
    let origin_before = sha(&sb, remote, "dev");
    let task_before = sha(&sb, &w.repo, BRANCH);

    let res = merge_push(&sb, 1);
    assert_code(&res, 1, "merge --push conflict");
    assert!(res.stderr.contains("có xung đột — đã huỷ merge (git merge --abort)"), "{}", res.stderr);
    assert!(res.stderr.contains("src/a.txt"), "{}", res.stderr);
    assert_eq!(current(&sb, &w.repo), BRANCH, "back on the task branch");
    assert!(!w.repo.join(".git/MERGE_HEAD").exists());
    assert_eq!(sb.git(&w.repo, &["status", "--porcelain"]).trim(), "");
    assert_eq!(sha(&sb, remote, "dev"), origin_before);
    assert_eq!(sha(&sb, &w.repo, BRANCH), task_before);
    assert_eq!(sha(&sb, &w.repo, "dev"), dev_before);
    assert_eq!(dev_before, origin_before);
    assert_eq!(marked(&first_parent_subjects(&sb, &w.repo, "dev"), 1), 0);
}

// 4. The local base has a commit that is not the task's: refused, nothing pushed.
#[test]
fn local_base_ahead_with_foreign_commit_is_refused() {
    let sb = Sandbox::new();
    let w = setup(&sb, true, Some("dev"), json!({}));
    let remote = w.remote.as_ref().unwrap();
    sb.git(&w.repo, &["checkout", "-q", "dev"]);
    commit_file(&sb, &w.repo, "src/local.txt", "l\n", "việc riêng chưa push");
    sb.git(&w.repo, &["checkout", "-q", BRANCH]);
    let origin_before = sha(&sb, remote, "dev");
    let dev_before = sha(&sb, &w.repo, "dev");

    let res = merge_push(&sb, 1);
    assert_code(&res, 1, "merge --push ahead");
    assert!(res.stderr.contains("có 1 commit chưa push"), "{}", res.stderr);
    assert!(res.stderr.contains("- việc riêng chưa push"), "{}", res.stderr);
    assert_eq!(sha(&sb, remote, "dev"), origin_before);
    assert_eq!(sha(&sb, &w.repo, "dev"), dev_before);
    assert_eq!(current(&sb, &w.repo), BRANCH);
    assert!(has_branch(&sb, &w.repo, BRANCH));
}

// 5. Push refused by origin: the merge stays; once origin accepts, the next run only pushes.
#[test]
fn rejected_push_keeps_merge_and_retry_only_pushes() {
    let sb = Sandbox::new();
    let w = setup(&sb, true, Some("dev"), json!({}));
    let remote = w.remote.as_ref().unwrap();
    let hook = remote.join("hooks").join("pre-receive");
    std::fs::write(&hook, "#!/bin/sh\necho 'từ chối push để thử' >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let origin_before = sha(&sb, remote, "dev");

    let res = merge_push(&sb, 1);
    assert_code(&res, 1, "merge --push rejected");
    assert!(res.stdout.contains("), chưa push."), "{}", res.stdout);
    assert!(res.stdout.contains("Push lỗi:"), "{}", res.stdout);
    assert!(res.stdout.contains("Bấm Merge & push lần nữa để thử push lại."), "{}", res.stdout);
    assert!(!res.stdout.contains(remote.to_str().unwrap()), "remote path leaked: {}", res.stdout);
    assert_eq!(sha(&sb, remote, "dev"), origin_before);
    let merge_sha = sha(&sb, &w.repo, "dev");
    assert_eq!(marked(&first_parent_subjects(&sb, &w.repo, "dev"), 1), 1);
    assert!(has_branch(&sb, &w.repo, BRANCH), "branch kept until pushed");
    assert_eq!(current(&sb, &w.repo), "dev");

    std::fs::remove_file(&hook).unwrap();
    let res = merge_push(&sb, 1);
    assert_code(&res, 0, "merge --push retry");
    assert!(
        res.stdout.contains("Đã push 'dev' lên origin (merge 'task/sua-app' đã có từ lần trước)."),
        "{}",
        res.stdout
    );
    assert_eq!(sha(&sb, &w.repo, "dev"), merge_sha, "no second merge commit");
    assert_eq!(sha(&sb, remote, "dev"), merge_sha);
    assert_eq!(marked(&first_parent_subjects(&sb, &w.repo, "dev"), 1), 1);
    assert!(!has_branch(&sb, &w.repo, BRANCH));
}

// 6. Dirty tree, wrong status, busy.
#[test]
fn refusals_dirty_status_and_busy() {
    let sb = Sandbox::new();
    let w = setup(&sb, true, Some("dev"), json!({}));
    let remote = w.remote.as_ref().unwrap();
    let origin_before = sha(&sb, remote, "dev");

    std::fs::write(w.repo.join("src/a.txt"), "sửa dở\n").unwrap();
    std::fs::write(w.repo.join("ghichu.txt"), "x\n").unwrap();
    let res = merge_push(&sb, 1);
    assert_code(&res, 1, "dirty");
    assert!(res.stderr.contains("Repo chính có thay đổi chưa commit — không merge:"), "{}", res.stderr);
    assert!(res.stderr.contains("src/a.txt") && res.stderr.contains("ghichu.txt"), "{}", res.stderr);
    sb.git(&w.repo, &["checkout", "-q", "--", "src/a.txt"]);
    std::fs::remove_file(w.repo.join("ghichu.txt")).unwrap();

    // Only protocol files left: still refused, and said so.
    std::fs::write(w.repo.join(".plan.md"), "# Plan\n").unwrap();
    let res = merge_push(&sb, 1);
    assert_code(&res, 1, "dirty protocol only");
    assert!(res.stderr.contains("chỉ còn file giao thức chưa commit"), "{}", res.stderr);
    std::fs::remove_file(w.repo.join(".plan.md")).unwrap();

    set_status(&sb, 1, "dev_done");
    let res = merge_push(&sb, 1);
    assert_code(&res, 1, "status");
    assert!(
        res.stderr.contains("Chỉ merge task đã qua QA (trạng thái hiện tại: dev_done)."),
        "{}",
        res.stderr
    );
    set_status(&sb, 1, "qa_pass");

    // A live run of the task itself.
    let mut decoy = std::process::Command::new("sleep").arg("60").spawn().unwrap();
    let pid = decoy.id() as i64;
    let start = agent_brain::procs::read_stat(pid as i32).expect("stat").start as i64;
    let now = agent_brain::db::now_ms();
    sb.db()
        .execute(
            "INSERT INTO runs (id, task_id, project_id, role, engine, profile, pid, pgid, pid_start, status, started_at)
             VALUES (1, 1, 1, 'qa', 'claude', 'work', ?1, ?1, ?2, 'running', ?3)",
            rusqlite::params![pid, start, now],
        )
        .unwrap();
    let res = merge_push(&sb, 1);
    assert_code(&res, 1, "busy task");
    assert!(res.stderr.contains(agent_brain::workspace::MERGE_TASK_BUSY), "{}", res.stderr);

    // A live run of another task of the project.
    insert_task(&sb, 2, "viec-khac", "task/viec-khac", Some("dev"), "planned", now);
    sb.db()
        .execute("UPDATE runs SET task_id = 2 WHERE id = 1", [])
        .unwrap();
    let res = merge_push(&sb, 1);
    assert_code(&res, 1, "busy project");
    assert!(res.stderr.contains("Dự án web đang có run 1 (qa, task 2) chạy"), "{}", res.stderr);
    let _ = decoy.kill();
    let _ = decoy.wait();

    assert_eq!(sha(&sb, remote, "dev"), origin_before);
    assert_eq!(current(&sb, &w.repo), BRANCH);
    assert!(has_branch(&sb, &w.repo, BRANCH));
}

// 7. No origin: a local merge, nothing pushed, exit 0.
#[test]
fn without_origin_merges_locally() {
    let sb = Sandbox::new();
    let w = setup(&sb, false, Some("dev"), json!({}));
    let res = merge_push(&sb, 1);
    assert_code(&res, 0, "no origin");
    assert!(res.stdout.contains("), chưa push."), "{}", res.stdout);
    assert!(res.stdout.contains("Repo không có origin, chưa push."), "{}", res.stdout);
    assert_eq!(current(&sb, &w.repo), "dev");
    assert_eq!(marked(&first_parent_subjects(&sb, &w.repo, "dev"), 1), 1);
    assert!(has_branch(&sb, &w.repo, BRANCH), "not pushed: the branch stays");
    // Again: nothing left to do.
    let res = merge_push(&sb, 1);
    assert_code(&res, 0, "no origin again");
    assert!(res.stdout.contains("Nhánh 'task/sua-app' đã merge vào 'dev' từ trước."), "{}", res.stdout);
    assert_eq!(marked(&first_parent_subjects(&sb, &w.repo, "dev"), 1), 1);
}

// 8. No base on the task or the project: merge.default_base_branch (dev, then main).
#[test]
fn default_base_branch_from_config() {
    let sb = Sandbox::new();
    let w = setup(&sb, true, None, json!({}));
    let res = merge_push(&sb, 1);
    assert_code(&res, 0, "default dev");
    assert!(res.stdout.contains("vào 'dev'"), "{}", res.stdout);
    assert_eq!(current(&sb, &w.repo), "dev");

    let sb = Sandbox::new();
    sb.write_config(json!({"merge": {"default_base_branch": "main"}}));
    let w = setup(&sb, true, None, json!({}));
    let remote = w.remote.as_ref().unwrap();
    let res = merge_push(&sb, 1);
    assert_code(&res, 0, "default main");
    assert!(res.stdout.contains("vào 'main'"), "{}", res.stdout);
    assert_eq!(current(&sb, &w.repo), "main");
    assert_eq!(sha(&sb, &w.repo, "main"), sha(&sb, remote, "main"));
    assert_eq!(marked(&first_parent_subjects(&sb, &w.repo, "main"), 1), 1);
}

// 9. Merged and pushed before: nothing new, the task branch is still cleaned up.
#[test]
fn already_merged_and_pushed_is_done() {
    let sb = Sandbox::new();
    let w = setup(&sb, true, Some("dev"), json!({}));
    let remote = w.remote.as_ref().unwrap();
    sb.git(&w.repo, &["checkout", "-q", "dev"]);
    sb.git(&w.repo, &["merge", "-q", "--no-ff", "-m", "Merge task/sua-app vào dev [task #1]: tay", BRANCH]);
    sb.git(&w.repo, &["push", "-q", "origin", "dev"]);
    sb.git(&w.repo, &["checkout", "-q", BRANCH]);
    let dev_before = sha(&sb, &w.repo, "dev");

    let res = merge_push(&sb, 1);
    assert_code(&res, 0, "already done");
    assert!(
        res.stdout.contains("Nhánh 'task/sua-app' đã merge vào 'dev' và push từ trước."),
        "{}",
        res.stdout
    );
    assert_eq!(sha(&sb, &w.repo, "dev"), dev_before, "no new commit");
    assert_eq!(sha(&sb, remote, "dev"), dev_before);
    assert!(!has_branch(&sb, &w.repo, BRANCH), "branch cleaned up");
    assert_eq!(current(&sb, &w.repo), "dev");

    // Branch gone and the task no longer qa_pass: still found as done through the merge subject.
    set_status(&sb, 1, "dev_done");
    let res = merge_push(&sb, 1);
    assert_code(&res, 0, "already done, branch gone");
    assert!(res.stdout.contains("và push từ trước."), "{}", res.stdout);
    assert_eq!(sha(&sb, &w.repo, "dev"), dev_before);
}

fn protocol_case(protocol_dir: &str, plan: &str, qa: &str) {
    let sb = Sandbox::new();
    let extra = if protocol_dir.is_empty() {
        json!({})
    } else {
        json!({"protocol_dir": protocol_dir})
    };
    let w = setup(&sb, true, Some("dev"), extra);
    commit_file(&sb, &w.repo, plan, "# Plan\n", "sua-app: plan");
    commit_file(&sb, &w.repo, qa, "# QA\nĐạt\n", "sua-app: qa");
    let tip = sha(&sb, &w.repo, BRANCH);

    let res = merge_push(&sb, 1);
    assert_code(&res, 0, "protocol files");
    assert!(res.stderr.contains("loại file giao thức khỏi merge"), "{}", res.stderr);
    let tree = sb.git(&w.repo, &["ls-tree", "-r", "--name-only", "dev"]);
    assert!(!tree.lines().any(|l| l == plan || l == qa), "{tree}");
    assert!(tree.lines().any(|l| l == "src/b.txt"), "{tree}");
    assert!(!w.repo.join(plan).exists() && !w.repo.join(qa).exists());
    // The task's own history (second parent) still has them.
    assert_eq!(sha(&sb, &w.repo, "dev^2"), tip);
    let task_tree = sb.git(&w.repo, &["ls-tree", "-r", "--name-only", "dev^2"]);
    assert!(task_tree.lines().any(|l| l == plan) && task_tree.lines().any(|l| l == qa), "{task_tree}");
    let task_log = sb.git(&w.repo, &["log", "--format=%s", "dev^2"]);
    assert!(task_log.contains("sua-app: plan"), "{task_log}");
    assert_eq!(sb.git(&w.repo, &["status", "--porcelain"]).trim(), "");
}

// 10. Protocol files at the root, and under protocol_dir.
#[test]
fn protocol_files_left_out_of_the_merge() {
    protocol_case("", ".plan.md", ".qa-report.md");
    protocol_case(".brain/{slug}", ".brain/sua-app/.plan.md", ".brain/sua-app/.qa-report.md");
}

/// A `git` in front of the real one that writes every command line to `log`.
fn git_recorder(sb: &Sandbox) -> (PathBuf, PathBuf) {
    let real = std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .map(|d| Path::new(d).join("git"))
        .find(|p| p.is_file())
        .expect("git on PATH");
    let bin = sb.root.join("gitbin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = sb.root.join("git-commands.log");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
        log.display(),
        real.display()
    );
    let wrapper = bin.join("git");
    std::fs::write(&wrapper, script).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    (bin, log)
}

fn path_with(bin: &Path) -> String {
    format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default())
}

fn recorded(log: &Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

// 11. Never forced: every git command line of a full run (and of a rejected push and its retry).
#[test]
fn never_forces_anything() {
    let sb = Sandbox::new();
    let w = setup(&sb, true, Some("dev"), json!({}));
    let remote = w.remote.as_ref().unwrap();
    let hook = remote.join("hooks").join("pre-receive");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (bin, log) = git_recorder(&sb);
    let path = path_with(&bin);
    let env = [("PATH", path.as_str())];

    let res = sb.run_env(&["task", "merge", "1", "--push"], None, &env);
    assert_code(&res, 1, "rejected push");
    std::fs::remove_file(&hook).unwrap();
    let res = sb.run_env(&["task", "merge", "1", "--push"], None, &env);
    assert_code(&res, 0, "retry");
    assert!(!has_branch(&sb, &w.repo, BRANCH));

    let lines = recorded(&log);
    assert!(
        lines.iter().any(|l| l == "push origin refs/heads/dev:refs/heads/dev"),
        "{lines:#?}"
    );
    assert!(lines.iter().any(|l| l.starts_with("merge --no-ff --no-commit refs/heads/task/sua-app")), "{lines:#?}");
    assert!(lines.iter().any(|l| l == "branch -d -- task/sua-app"), "{lines:#?}");
    for l in &lines {
        let words: Vec<&str> = l.split_whitespace().collect();
        assert!(
            !words.iter().any(|w| *w == "-f" || w.starts_with("--force") || w.starts_with('+') || *w == "-D" || *w == "--hard" || *w == "--delete"),
            "forbidden git word in {l:?}"
        );
        assert!(
            !matches!(words.first(), Some(&"reset") | Some(&"clean")),
            "forbidden git command {l:?}"
        );
    }
}

// 12. A bad branch name is refused before any git command touches the repo.
#[test]
fn bad_branch_names_refused_before_git() {
    let sb = Sandbox::new();
    let w = setup(&sb, true, Some("dev"), json!({}));
    let now = agent_brain::db::now_ms();
    insert_task(&sb, 2, "xau", "-x", Some("dev"), "qa_pass", now);
    insert_task(&sb, 3, "xau-2", "task/xau", Some("a b"), "qa_pass", now);
    let (bin, log) = git_recorder(&sb);
    let path = path_with(&bin);
    let env = [("PATH", path.as_str())];

    let res = sb.run_env(&["task", "merge", "2", "--push"], None, &env);
    assert_code(&res, 1, "branch -x");
    assert!(res.stderr.contains("Tên nhánh task không hợp lệ: '-x'"), "{}", res.stderr);
    let res = sb.run_env(&["task", "merge", "3", "--push"], None, &env);
    assert_code(&res, 1, "base 'a b'");
    assert!(res.stderr.contains("Tên nhánh gốc không hợp lệ: 'a b'"), "{}", res.stderr);

    for l in recorded(&log) {
        let first = l.split_whitespace().next().unwrap_or("");
        assert!(
            !matches!(first, "fetch" | "push" | "merge" | "checkout" | "branch" | "commit" | "rm" | "status" | "log"),
            "git ran before the name check: {l:?}"
        );
        assert!(!l.contains("-x") && !l.contains("a b"), "a bad name reached git: {l:?}");
    }
    assert_eq!(current(&sb, &w.repo), BRANCH);
}
