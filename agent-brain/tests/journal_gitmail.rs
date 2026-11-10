//! Phase 7 plan 11 (gap closure from the code review): the journal git runner ignores inherited
//! repository variables and treats pathspecs literally (WR-04), keeps HEAD on `journal.branch`
//! (WR-10), keeps the user's ssh command (IN-07); the Telegram mail scan never loses a claimed
//! row (WR-09). Remotes are local bare repositories, the ssh "client" is a script that records its
//! arguments, and Telegram is not involved. Texts and projects are invented.

mod common;
#[allow(dead_code)]
#[path = "support/journal.rs"]
mod jsupport;

use agent_brain::journal::config::JournalConfig;
use agent_brain::journal::gitsync::{
    commit_and_push, prepare, GitCtx, GitStop, Prepared, Published,
};
use agent_brain::telegram::detect::JournalMail;
use agent_brain::telegram::outbox::Outgoing;
use common::Sandbox;
use jsupport::*;
use serde_json::json;
use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

const REQUEST: &str = "Dựng video giới thiệu sản phẩm mẫu";
const MARK: &str = agent_brain::journal::model::MARKER;

fn now() -> i64 {
    local_ms(2026, 1, 15, 20, 0, 0)
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn rev(sb: &Sandbox, repo: &Path, r: &str) -> String {
    sb.git(repo, &["rev-parse", r]).trim().to_string()
}

/// A working repo that pushes `files` to `branch` of `remote` (the human or another machine).
fn push_branch(sb: &Sandbox, remote: &Path, work: &str, branch: &str, files: &[(&str, &str)]) {
    let dir = sb.root.join(work);
    std::fs::create_dir_all(&dir).unwrap();
    sb.git(&dir, &["init", "-q", "-b", branch]);
    for (rel, text) in files {
        write(&dir, rel, text);
        sb.git(&dir, &["add", "--", rel]);
    }
    sb.git(&dir, &["commit", "-q", "-m", "seed"]);
    sb.git(
        &dir,
        &[
            "push",
            "-q",
            remote.to_str().unwrap(),
            &format!("HEAD:refs/heads/{branch}"),
        ],
    );
}

/// Library-level config: clone at `<sandbox>/libclone`, branch `main`.
fn lib_cfg(sb: &Sandbox, remote: &Path) -> (JournalConfig, GitCtx) {
    let mut cfg = JournalConfig::default();
    cfg.repo_url = Some(remote.to_str().unwrap().to_string());
    cfg.clone_dir = sb.root.join("libclone");
    cfg.git_timeout_seconds = 30;
    let ctx = GitCtx::from_cfg(&cfg, Duration::from_millis(200));
    (cfg, ctx)
}

fn no_generated() -> BTreeSet<String> {
    BTreeSet::new()
}

/// Remote with `main` (one human file) and `other` (one more commit on its own), and a bootstrapped
/// clone of it on `main`.
fn two_branch_vault(sb: &Sandbox) -> (PathBuf, JournalConfig, GitCtx) {
    let remote = bare_remote(sb, "vault");
    push_branch(
        sb,
        &remote,
        "seed-main",
        "main",
        &[("Dự án/ghi-chu.md", "Ghi chú viết tay.\n")],
    );
    let work = sb.root.join("seed-main");
    sb.git(&work, &["checkout", "-q", "-b", "other"]);
    write(&work, "khac.md", "Nhánh khác.\n");
    sb.git(&work, &["add", "--", "khac.md"]);
    sb.git(&work, &["commit", "-q", "-m", "chỉ ở nhánh khác"]);
    sb.git(
        &work,
        &[
            "push",
            "-q",
            remote.to_str().unwrap(),
            "HEAD:refs/heads/other",
        ],
    );
    let (cfg, ctx) = lib_cfg(sb, &remote);
    assert_eq!(prepare(&ctx, &cfg, &no_generated()), Prepared::Ready);
    (remote, cfg, ctx)
}

fn head_ref(sb: &Sandbox, clone: &Path) -> String {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["symbolic-ref", "-q", "HEAD"]).current_dir(clone);
    cmd.env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", &sb.home)
        .env("GIT_CONFIG_GLOBAL", &sb.git_config)
        .env("GIT_CONFIG_NOSYSTEM", "1");
    String::from_utf8_lossy(&cmd.output().unwrap().stdout)
        .trim()
        .to_string()
}

fn is_ancestor(sb: &Sandbox, repo: &Path, a: &str, b: &str) -> bool {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["merge-base", "--is-ancestor", a, b])
        .current_dir(repo);
    cmd.env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", &sb.home)
        .env("GIT_CONFIG_GLOBAL", &sb.git_config)
        .env("GIT_CONFIG_NOSYSTEM", "1");
    cmd.status().unwrap().success()
}

// ---------------------------------------------------------------------------------------------
// WR-04
// ---------------------------------------------------------------------------------------------

/// Project `du-an-a` with one finished task, journal pointing at `remote`.
fn fixture(sb: &Sandbox, remote_url: &str) {
    sb.write_config(json!({"journal": {"repo_url": remote_url, "utc_offset_minutes": 420}}));
    let conn = setup_projects(sb, &[("du-an-a", Some("video"))]);
    let task = seed_task(&conn, "du-an-a", "dung-video", REQUEST);
    seed_proposal(&conn, task, REQUEST);
    seed_run(
        &conn,
        task,
        "dev",
        "succeeded",
        None,
        local_ms(2026, 1, 15, 9, 10, 0),
        Some(local_ms(2026, 1, 15, 9, 30, 0)),
        Some(1),
    );
}

#[test]
fn export_ignores_inherited_git_dir() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    fixture(&sb, remote.to_str().unwrap());

    // Another repository, as an agent worktree or a hook would leave in the environment.
    let other = sb.root.join("other-repo");
    std::fs::create_dir_all(&other).unwrap();
    sb.git(&other, &["init", "-q", "-b", "trunk"]);
    write(&other, "mine.txt", "không liên quan\n");
    sb.git(&other, &["add", "--", "mine.txt"]);
    sb.git(&other, &["commit", "-q", "-m", "gốc"]);
    let other_head = rev(&sb, &other, "HEAD");
    let other_git = other.join(".git");

    let nows = now().to_string();
    let out = sb.run_env(
        &["journal", "export"],
        None,
        &[
            (agent_brain::journal::NOW_ENV, nows.as_str()),
            ("GIT_DIR", other_git.to_str().unwrap()),
            ("GIT_WORK_TREE", other.to_str().unwrap()),
            ("GIT_INDEX_FILE", other_git.join("index").to_str().unwrap()),
        ],
    );
    assert_eq!(
        out.code, 0,
        "stdout: {}\nstderr: {}",
        out.stdout, out.stderr
    );

    assert_eq!(remote_commits(&sb, &remote).len(), 1, "vault got no commit");
    assert!(clone_dir(&sb).join(".git").exists());
    assert_eq!(rev(&sb, &other, "HEAD"), other_head, "other repo moved");
    assert_eq!(sb.git(&other, &["status", "--porcelain"]).trim(), "");
    assert_eq!(
        sb.git(&other, &["ls-files"]).trim(),
        "mine.txt",
        "other repo's index changed"
    );
}

#[test]
fn marked_files_with_glob_names_are_staged_by_exact_name() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    // A tracked human file that the glob `c*.md` would also match when un-staging a deleted name.
    push_branch(
        &sb,
        &remote,
        "seed-main",
        "main",
        &[("Dự án/cc.md", "bản tay\n")],
    );
    let (cfg, ctx) = lib_cfg(&sb, &remote);
    assert_eq!(prepare(&ctx, &cfg, &no_generated()), Prepared::Ready);
    let clone = cfg.clone_dir.clone();

    // Present generated files whose names are globs, next to untracked files they would match.
    write(&clone, "Dự án/a*.md", &format!("{MARK}\nmột\n"));
    write(&clone, "Dự án/b[x].md", &format!("{MARK}\nhai\n"));
    write(&clone, "Dự án/aaa.md", "tệp khác, không được stage\n");
    write(&clone, "Dự án/bx.md", "tệp khác, không được stage\n");
    // And a generated file with a glob name that is gone from disk and not tracked (`rm --cached
    // --ignore-unmatch`): git matches an existing exact path first, so only this case tells a
    // literal pathspec from a glob; `cc.md` must survive.

    let paths = vec![
        "Dự án/a*.md".to_string(),
        "Dự án/b[x].md".to_string(),
        "Dự án/c*.md".to_string(),
    ];
    let published = commit_and_push(&ctx, &cfg, &paths, "nhật ký thử", true);
    assert!(
        matches!(published, Published::Pushed { .. }),
        "{published:?}"
    );

    let files = sb.git(&clone, &["ls-tree", "-r", "--name-only", "-z", "HEAD"]);
    let mut names: Vec<&str> = files.split('\0').filter(|n| !n.is_empty()).collect();
    names.sort();
    assert_eq!(
        names,
        vec!["Dự án/a*.md", "Dự án/b[x].md", "Dự án/cc.md"],
        "committed tree"
    );
    assert_eq!(
        sb.git(&clone, &["diff", "--name-only", "-z", "--cached"]),
        ""
    );
}

// ---------------------------------------------------------------------------------------------
// WR-10
// ---------------------------------------------------------------------------------------------

#[test]
fn clone_on_another_branch_is_repaired_and_only_the_journal_branch_is_pushed() {
    let sb = Sandbox::new();
    let (remote, cfg, ctx) = two_branch_vault(&sb);
    let clone = cfg.clone_dir.clone();
    sb.git(&clone, &["checkout", "-q", "other"]);
    assert_eq!(head_ref(&sb, &clone), "refs/heads/other");
    let other_sha = rev(&sb, &remote, "refs/heads/other");
    let main_before = rev(&sb, &remote, "refs/heads/main");

    let gen: BTreeSet<String> = ["Nhật ký/ngay.md".to_string()].into();
    assert_eq!(prepare(&ctx, &cfg, &gen), Prepared::Ready);
    assert_eq!(head_ref(&sb, &clone), "refs/heads/main");

    write(&clone, "Nhật ký/ngay.md", &format!("{MARK}\nmột ngày\n"));
    let published = commit_and_push(
        &ctx,
        &cfg,
        &["Nhật ký/ngay.md".to_string()],
        "nhật ký",
        true,
    );
    assert!(
        matches!(published, Published::Pushed { .. }),
        "{published:?}"
    );

    assert_eq!(rev(&sb, &remote, "refs/heads/other"), other_sha);
    let main_after = rev(&sb, &remote, "refs/heads/main");
    assert_ne!(main_after, main_before);
    assert!(is_ancestor(&sb, &remote, &main_before, &main_after));
    assert!(
        !is_ancestor(&sb, &remote, &other_sha, &main_after),
        "the other branch's commit reached main"
    );
}

#[test]
fn dirty_clone_on_another_branch_stops_and_pushes_nothing() {
    let sb = Sandbox::new();
    let (remote, cfg, ctx) = two_branch_vault(&sb);
    let clone = cfg.clone_dir.clone();
    sb.git(&clone, &["checkout", "-q", "other"]);
    // A hand edit of a tracked file.
    write(&clone, "khac.md", "tôi đang sửa dở\n");
    let main_before = rev(&sb, &remote, "refs/heads/main");
    let other_before = rev(&sb, &remote, "refs/heads/other");

    match prepare(&ctx, &cfg, &no_generated()) {
        Prepared::Stopped(GitStop::Conflict { detail }) => {
            assert!(
                !detail.contains("libclone") && !detail.contains("khac.md"),
                "no paths in the notice: {detail}"
            );
            assert!(!detail.is_empty());
        }
        other => panic!("expected Stopped(Conflict), got {other:?}"),
    }
    assert_eq!(head_ref(&sb, &clone), "refs/heads/other", "HEAD was moved");
    assert_eq!(
        std::fs::read_to_string(clone.join("khac.md")).unwrap(),
        "tôi đang sửa dở\n",
        "hand edit lost"
    );

    // Even if a caller went on to commit, nothing is staged or pushed from the wrong branch.
    write(&clone, "Nhật ký/ngay.md", &format!("{MARK}\nmột ngày\n"));
    let published = commit_and_push(
        &ctx,
        &cfg,
        &["Nhật ký/ngay.md".to_string()],
        "nhật ký",
        true,
    );
    assert!(
        matches!(published, Published::CommittedNotPushed(_)),
        "{published:?}"
    );
    assert_eq!(rev(&sb, &remote, "refs/heads/main"), main_before);
    assert_eq!(rev(&sb, &remote, "refs/heads/other"), other_before);
}

#[test]
fn detached_head_is_repaired() {
    let sb = Sandbox::new();
    let (remote, cfg, ctx) = two_branch_vault(&sb);
    let clone = cfg.clone_dir.clone();
    sb.git(&clone, &["checkout", "-q", "--detach", "origin/other"]);
    assert_eq!(head_ref(&sb, &clone), "");

    assert_eq!(prepare(&ctx, &cfg, &no_generated()), Prepared::Ready);
    assert_eq!(head_ref(&sb, &clone), "refs/heads/main");
    assert_eq!(
        rev(&sb, &clone, "HEAD"),
        rev(&sb, &remote, "refs/heads/main")
    );
}

#[test]
fn unborn_head_on_a_stray_branch_is_repaired() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    push_branch(
        &sb,
        &remote,
        "seed-main",
        "main",
        &[("Dự án/ghi-chu.md", "Ghi chú viết tay.\n")],
    );
    // What an interrupted first `prepare` leaves: cloned with --no-checkout, HEAD not on `main`.
    let (cfg, ctx) = lib_cfg(&sb, &remote);
    sb.git(
        &sb.root,
        &[
            "clone",
            "-q",
            "--no-checkout",
            remote.to_str().unwrap(),
            cfg.clone_dir.to_str().unwrap(),
        ],
    );
    sb.git(
        &cfg.clone_dir,
        &["symbolic-ref", "HEAD", "refs/heads/stray"],
    );
    assert_eq!(head_ref(&sb, &cfg.clone_dir), "refs/heads/stray");

    assert_eq!(prepare(&ctx, &cfg, &no_generated()), Prepared::Ready);
    assert_eq!(head_ref(&sb, &cfg.clone_dir), "refs/heads/main");
    assert_eq!(
        rev(&sb, &cfg.clone_dir, "HEAD"),
        rev(&sb, &remote, "refs/heads/main")
    );
}

// ---------------------------------------------------------------------------------------------
// IN-07
// ---------------------------------------------------------------------------------------------

/// A script that records its arguments, one per line, then fails like an unreachable host.
fn fake_ssh(sb: &Sandbox, name: &str) -> (PathBuf, PathBuf) {
    let script = sb.root.join(name);
    let log = sb.root.join(format!("{name}.args"));
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{}'\nexit 255\n",
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    (script, log)
}

/// Run an export against an ssh:// vault with the given extra environment; returns the recorded
/// ssh arguments (the export itself fails: nothing is reachable).
fn export_over_ssh(sb: &Sandbox, extra_env: &[(&str, &str)], log: &Path) -> Option<Vec<String>> {
    fixture(sb, "ssh://vault.invalid/vault.git");
    let nows = now().to_string();
    let mut env = vec![(agent_brain::journal::NOW_ENV, nows.as_str())];
    env.extend_from_slice(extra_env);
    let _ = sb.run_env(&["journal", "export"], None, &env);
    std::fs::read_to_string(log)
        .ok()
        .map(|s| s.lines().map(str::to_string).collect())
}

#[test]
fn users_ssh_command_is_kept_and_hardened() {
    let sb = Sandbox::new();
    let (script, log) = fake_ssh(&sb, "myssh");
    let user_cmd = format!("{} -i /keys/vault_key", script.display());
    let args = export_over_ssh(&sb, &[("GIT_SSH_COMMAND", user_cmd.as_str())], &log)
        .expect("the user's ssh command was not used");
    let joined = args.join(" ");
    assert!(joined.contains("-i /keys/vault_key"), "{joined}");
    assert!(joined.contains("BatchMode=yes"), "{joined}");
    assert!(joined.contains("StrictHostKeyChecking=yes"), "{joined}");
}

#[test]
fn core_ssh_command_is_kept_and_hardened() {
    let sb = Sandbox::new();
    let (script, log) = fake_ssh(&sb, "cfgssh");
    let mut gitconfig = std::fs::read_to_string(&sb.git_config).unwrap();
    gitconfig.push_str(&format!(
        "[core]\n  sshCommand = {} -i /keys/cfg_key\n",
        script.display()
    ));
    std::fs::write(&sb.git_config, gitconfig).unwrap();
    let args =
        export_over_ssh(&sb, &[], &log).expect("core.sshCommand was replaced by a fixed command");
    let joined = args.join(" ");
    assert!(joined.contains("-i /keys/cfg_key"), "{joined}");
    assert!(joined.contains("BatchMode=yes"), "{joined}");
    assert!(joined.contains("StrictHostKeyChecking=yes"), "{joined}");
}

#[test]
fn git_ssh_program_alone_is_left_untouched() {
    let sb = Sandbox::new();
    let (script, log) = fake_ssh(&sb, "progssh");
    let args = export_over_ssh(&sb, &[("GIT_SSH", script.to_str().unwrap())], &log)
        .expect("GIT_SSH program was not used");
    let joined = args.join(" ");
    assert!(
        !joined.contains("BatchMode") && !joined.contains("StrictHostKeyChecking"),
        "options forced on a GIT_SSH program: {joined}"
    );
    assert!(joined.contains("vault.invalid"), "{joined}");
}

// ---------------------------------------------------------------------------------------------
// WR-09
// ---------------------------------------------------------------------------------------------

fn mail(conn: &rusqlite::Connection, key: &str, text: &str) {
    conn.execute(
        "INSERT INTO journal_mail (kind, dedupe_key, text, created_at) VALUES ('alert', ?1, ?2, 1)",
        rusqlite::params![key, text],
    )
    .unwrap();
}

fn texts(items: &[Outgoing]) -> Vec<String> {
    items
        .iter()
        .map(|i| match i {
            Outgoing::Message { text } => text.clone(),
            other => panic!("unexpected item {other:?}"),
        })
        .collect()
}

fn unsent(conn: &rusqlite::Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT dedupe_key FROM journal_mail WHERE sent_at IS NULL ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

#[test]
fn mail_scan_returns_claimed_rows_when_a_later_row_fails_and_retries_it() {
    let sb = Sandbox::new();
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let cfg = JournalConfig::default();
    mail(&conn, "a:1", "thư một");
    mail(&conn, "a:2", "thư hai");
    mail(&conn, "a:3", "thư ba");
    // The claim of the second row fails (what SQLITE_BUSY does while the exporter writes).
    conn.execute_batch(
        "CREATE TRIGGER fail_second BEFORE UPDATE ON journal_mail WHEN OLD.dedupe_key = 'a:2'
         BEGIN SELECT RAISE(ABORT, 'database is locked'); END;",
    )
    .unwrap();

    let mut scan = JournalMail;
    let first = scan
        .scan(&conn, &cfg, now())
        .expect("the claimed first row must not be discarded");
    assert_eq!(texts(&first), vec!["thư một".to_string()]);
    assert_eq!(unsent(&conn), vec!["a:2".to_string(), "a:3".to_string()]);

    // Next scan, the failure gone: the rest goes out, each row exactly once.
    conn.execute_batch("DROP TRIGGER fail_second").unwrap();
    let second = scan.scan(&conn, &cfg, now()).unwrap();
    assert_eq!(
        texts(&second),
        vec!["thư hai".to_string(), "thư ba".to_string()]
    );
    assert!(unsent(&conn).is_empty());

    // A restart (a new scanner, a new connection) sends nothing again.
    let reopened = agent_brain::db::open(&sb.db_path).unwrap();
    assert!(JournalMail.scan(&reopened, &cfg, now()).unwrap().is_empty());
}

#[test]
fn mail_scan_with_the_very_first_row_failing_is_an_error_and_claims_nothing() {
    let sb = Sandbox::new();
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let cfg = JournalConfig::default();
    mail(&conn, "b:1", "thư một");
    conn.execute_batch(
        "CREATE TRIGGER fail_all BEFORE UPDATE ON journal_mail
         BEGIN SELECT RAISE(ABORT, 'database is locked'); END;",
    )
    .unwrap();
    assert!(JournalMail.scan(&conn, &cfg, now()).is_err());
    assert_eq!(unsent(&conn), vec!["b:1".to_string()]);
    conn.execute_batch("DROP TRIGGER fail_all").unwrap();
    assert_eq!(
        texts(&JournalMail.scan(&conn, &cfg, now()).unwrap()),
        vec!["thư một".to_string()]
    );
}
