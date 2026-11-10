//! Phase 7 plan 05: the git side of `agent-brain journal export` on a real, non-empty vault
//! (D-22) and the D-10 failure policy. Every remote is a local bare repository in the sandbox;
//! "offline" is the bare repository renamed away. A second clone made with the sandbox git
//! config plays the human or another machine. Texts and projects are invented.

mod common;
#[allow(dead_code)]
#[path = "support/journal.rs"]
mod jsupport;

use common::Sandbox;
use jsupport::*;
use rusqlite::Connection;
use serde_json::json;
use std::path::{Path, PathBuf};

const REQUEST: &str = "Dựng video giới thiệu sản phẩm mẫu";
const DAY_FILE: &str = "Nhật ký/2026/01/2026-01-15.md";
const STALE_DAY: &str = "Nhật ký/2025/12/2025-12-01.md";
const HUMAN_NOTE: &str = "Dự án/ghi-chu-cua-toi.md";
const OTHER_NOTE: &str = "Ghi chú/khac.md";

fn marker() -> &'static str {
    agent_brain::journal::model::MARKER
}

fn now() -> i64 {
    local_ms(2026, 1, 15, 20, 0, 0)
}

fn later() -> i64 {
    local_ms(2026, 1, 16, 20, 0, 0)
}

/// Project `du-an-a`, one task with three finished runs on 2026-01-15 (local).
fn fixture(sb: &Sandbox, remote: &Path) -> Connection {
    sb.write_config(json!({"journal": journal_section(remote)}));
    let conn = setup_projects(sb, &[("du-an-a", Some("video"))]);
    let task = seed_task(&conn, "du-an-a", "dung-video", REQUEST);
    seed_proposal(&conn, task, REQUEST);
    for (role, h0, m0, h1, m1) in [
        ("lead", 9, 0, 9, 5),
        ("dev", 9, 10, 9, 30),
        ("qa", 9, 31, 9, 40),
    ] {
        seed_run(
            &conn,
            task,
            role,
            "succeeded",
            (role == "qa").then_some("qa:pass"),
            local_ms(2026, 1, 15, h0, m0, 0),
            Some(local_ms(2026, 1, 15, h1, m1, 0)),
            Some(1),
        );
    }
    conn
}

/// One more finished task on `day` of January 2026 (local).
fn add_task(conn: &Connection, slug: &str, day: u32) {
    let task = seed_task(conn, "du-an-a", slug, &format!("Việc thêm {slug}"));
    seed_run(
        conn,
        task,
        "dev",
        "succeeded",
        None,
        local_ms(2026, 1, day, 11, 0, 0),
        Some(local_ms(2026, 1, day, 11, 20, 0)),
        Some(1),
    );
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// Commit `files` from a working clone of `remote` (the human) and push to `main`.
fn human_push(sb: &Sandbox, remote: &Path, work: &Path, files: &[(&str, &str)], msg: &str) {
    for (rel, text) in files {
        write(work, rel, text);
        sb.git(work, &["add", "--", rel]);
    }
    sb.git(work, &["commit", "-q", "-m", msg]);
    sb.git(
        work,
        &[
            "push",
            "-q",
            remote.to_str().unwrap(),
            "HEAD:refs/heads/main",
        ],
    );
}

/// A fresh working directory that pushes the first commit of an empty remote.
fn seed_remote(sb: &Sandbox, remote: &Path, files: &[(&str, &str)]) {
    let work = sb.root.join("seed-work");
    std::fs::create_dir_all(&work).unwrap();
    sb.git(&work, &["init", "-q", "-b", "main"]);
    human_push(sb, remote, &work, files, "vault của tôi");
}

fn away(remote: &Path) -> PathBuf {
    PathBuf::from(format!("{}.away", remote.display()))
}

fn go_offline(remote: &Path) {
    std::fs::rename(remote, away(remote)).unwrap();
}

fn go_online(remote: &Path) {
    std::fs::rename(away(remote), remote).unwrap();
}

fn rev(sb: &Sandbox, repo: &Path, r: &str) -> String {
    sb.git(repo, &["rev-parse", r]).trim().to_string()
}

fn merges(sb: &Sandbox, repo: &Path, r: &str) -> String {
    sb.git(repo, &["rev-list", "--merges", "--count", r])
        .trim()
        .to_string()
}

fn ok(out: &common::RunOut) {
    assert_eq!(
        out.code, 0,
        "stdout: {}\nstderr: {}",
        out.stdout, out.stderr
    );
}

fn failed(out: &common::RunOut) {
    assert_eq!(
        out.code, 1,
        "stdout: {}\nstderr: {}",
        out.stdout, out.stderr
    );
}

// ---------------------------------------------------------------- Task 1: writer ownership

#[test]
fn nonempty_remote_keeps_human_files() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    let gitignore = ".obsidian/workspace*.json\n.obsidian/cache\n.trash/\n";
    let app = "{\"theme\": \"moonstone\"}\n";
    let note = "Ghi chú viết tay, không có dòng đánh dấu.\n";
    let stale = format!("---\ndate: 2025-12-01\n---\n{}\n# Ngày cũ\n", marker());
    seed_remote(
        &sb,
        &remote,
        &[
            (".gitignore", gitignore),
            (".obsidian/app.json", app),
            (HUMAN_NOTE, note),
            (STALE_DAY, &stale),
        ],
    );
    fixture(&sb, &remote);

    ok(&export(&sb, &[], now()));

    let co = checkout(&sb, &remote);
    assert!(!co.join(STALE_DAY).exists(), "stale generated day kept");
    assert_eq!(std::fs::read_to_string(co.join(HUMAN_NOTE)).unwrap(), note);
    assert_eq!(
        std::fs::read_to_string(co.join(".obsidian/app.json")).unwrap(),
        app
    );
    assert_eq!(
        std::fs::read(co.join(".gitignore")).unwrap(),
        gitignore.as_bytes()
    );
    assert!(co.join(DAY_FILE).is_file());
    assert_eq!(merges(&sb, &co, "main"), "0");
    assert_eq!(remote_commits(&sb, &remote).len(), 2);
}

#[test]
fn human_file_at_generated_path_is_not_overwritten() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    let mine = "# Trang hôm nay của tôi\nkhông có dòng đánh dấu\n";
    seed_remote(&sb, &remote, &[("Today.md", mine)]);
    fixture(&sb, &remote);

    let out = export(&sb, &[], now());
    ok(&out);

    let co = checkout(&sb, &remote);
    assert_eq!(std::fs::read_to_string(co.join("Today.md")).unwrap(), mine);

    // `Today.md` is rendered only once 07-03 is merged: probe the rendered map with the same
    // data in a second sandbox (empty remote, `--no-push`) and expect the warning only then.
    let probe = Sandbox::new();
    let probe_remote = bare_remote(&probe, "vault");
    fixture(&probe, &probe_remote);
    ok(&export(&probe, &["--no-push"], now()));
    if clone_dir(&probe).join("Today.md").exists() {
        let all = format!("{}{}", out.stdout, out.stderr);
        assert!(all.contains("Today.md"), "{all}");
    }
}

// ---------------------------------------------------------------- Task 2: sync of the clone

fn any_label(text: &str) -> bool {
    use agent_brain::journal::gitsync::FailCategory::*;
    [Network, SshAuth, Rejected, Timeout, Other]
        .iter()
        .any(|c| text.contains(c.label()))
}

fn commit_count(sb: &Sandbox, repo: &Path) -> usize {
    sb.git(repo, &["rev-list", "--count", "HEAD"])
        .trim()
        .parse()
        .unwrap()
}

/// The human edits the heading line of the day file in their own clone and pushes.
fn human_edits_heading(sb: &Sandbox, remote: &Path) -> String {
    let work = checkout(sb, remote);
    let text = std::fs::read_to_string(work.join(DAY_FILE)).unwrap();
    let edited: Vec<String> = text
        .lines()
        .map(|l| {
            if l.starts_with("# ") {
                "# Ngày này tôi đã sửa tay".to_string()
            } else {
                l.to_string()
            }
        })
        .collect();
    let edited = format!("{}\n", edited.join("\n"));
    human_push(sb, remote, &work, &[(DAY_FILE, &edited)], "sửa tay");
    edited
}

#[test]
fn offline_commit_then_catch_up() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    let conn = fixture(&sb, &remote);
    ok(&export(&sb, &[], now()));

    add_task(&conn, "viec-moi", 16);
    go_offline(&remote);
    let out = export(&sb, &[], later());
    failed(&out);
    assert!(any_label(&out.stderr), "{}", out.stderr);
    let clone = clone_dir(&sb);
    assert_eq!(commit_count(&sb, &clone), 2);
    assert!(sb.git(&clone, &["status", "--porcelain"]).trim().is_empty());

    go_online(&remote);
    ok(&export(&sb, &[], later()));
    assert_eq!(remote_commits(&sb, &remote).len(), 2);
    assert_eq!(merges(&sb, &remote, "main"), "0");
    assert_eq!(rev(&sb, &remote, "main"), rev(&sb, &clone, "HEAD"));
}

#[test]
fn remote_moved_fast_forwards() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    let conn = fixture(&sb, &remote);
    ok(&export(&sb, &[], now()));

    let work = checkout(&sb, &remote);
    human_push(
        &sb,
        &remote,
        &work,
        &[(OTHER_NOTE, "ghi chú khác\n")],
        "ghi chú",
    );
    add_task(&conn, "viec-moi", 16);
    ok(&export(&sb, &[], later()));

    assert_eq!(remote_commits(&sb, &remote).len(), 3);
    assert_eq!(merges(&sb, &remote, "main"), "0");
    let co = checkout(&sb, &remote);
    assert_eq!(
        std::fs::read_to_string(co.join(OTHER_NOTE)).unwrap(),
        "ghi chú khác\n"
    );
    assert!(co.join("Nhật ký/2026/01/2026-01-16.md").is_file());
}

#[test]
fn diverged_history_rebases_local_commits() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    let conn = fixture(&sb, &remote);
    ok(&export(&sb, &[], now()));

    add_task(&conn, "viec-moi", 16);
    go_offline(&remote);
    failed(&export(&sb, &[], later()));
    go_online(&remote);
    let work = checkout(&sb, &remote);
    human_push(
        &sb,
        &remote,
        &work,
        &[(OTHER_NOTE, "ghi chú khác\n")],
        "ghi chú",
    );

    ok(&export(&sb, &[], later()));
    assert_eq!(remote_commits(&sb, &remote).len(), 3);
    assert_eq!(merges(&sb, &remote, "main"), "0");
    let co = checkout(&sb, &remote);
    assert!(co.join(OTHER_NOTE).is_file());
    assert!(co.join("Nhật ký/2026/01/2026-01-16.md").is_file());
    assert_eq!(rev(&sb, &remote, "main"), rev(&sb, &clone_dir(&sb), "HEAD"));
}

#[test]
fn conflict_aborts_without_overwrite() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    let conn = fixture(&sb, &remote);
    ok(&export(&sb, &[], now()));

    add_task(&conn, "cung-ngay", 15);
    go_offline(&remote);
    failed(&export(&sb, &[], now()));
    let clone = clone_dir(&sb);
    let local_head = rev(&sb, &clone, "HEAD");
    go_online(&remote);
    let edited = human_edits_heading(&sb, &remote);
    let remote_head = rev(&sb, &remote, "main");

    for _ in 0..2 {
        let out = export(&sb, &[], now());
        failed(&out);
        assert!(out.stderr.contains("xung đột"), "{}", out.stderr);
        assert!(out.stderr.contains(DAY_FILE), "{}", out.stderr);
        assert_eq!(rev(&sb, &remote, "main"), remote_head);
        assert!(!clone.join(".git/rebase-merge").exists());
        assert!(!clone.join(".git/rebase-apply").exists());
        assert_eq!(rev(&sb, &clone, "HEAD"), local_head);
        assert!(sb.git(&clone, &["status", "--porcelain"]).trim().is_empty());
    }
    let co = checkout(&sb, &remote);
    assert_eq!(std::fs::read_to_string(co.join(DAY_FILE)).unwrap(), edited);
}

#[test]
fn dirty_clone_stops_without_overwrite() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    fixture(&sb, &remote);
    ok(&export(&sb, &[], now()));
    let clone = clone_dir(&sb);

    let gi = clone.join(".gitignore");
    let original = std::fs::read_to_string(&gi).unwrap();
    let mine = format!("{original}ghi-chu-rieng/\n");
    std::fs::write(&gi, &mine).unwrap();
    let out = export(&sb, &[], now());
    failed(&out);
    assert!(out.stderr.contains(".gitignore"), "{}", out.stderr);
    assert_eq!(std::fs::read_to_string(&gi).unwrap(), mine);
    std::fs::write(&gi, &original).unwrap();

    let day = clone.join(DAY_FILE);
    let generated = std::fs::read_to_string(&day).unwrap();
    std::fs::write(&day, format!("{generated}dòng thêm tay\n")).unwrap();
    ok(&export(&sb, &[], now()));
    assert_eq!(std::fs::read_to_string(&day).unwrap(), generated);
    assert!(sb.git(&clone, &["status", "--porcelain"]).trim().is_empty());
    assert_eq!(remote_commits(&sb, &remote).len(), 1);
}

#[test]
fn not_a_repo_dir_is_refused() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    fixture(&sb, &remote);
    let clone = clone_dir(&sb);
    write(&clone, "cua-toi.txt", "không xóa\n");

    failed(&export(&sb, &[], now()));
    assert_eq!(
        std::fs::read_to_string(clone.join("cua-toi.txt")).unwrap(),
        "không xóa\n"
    );
    assert!(!clone.join(".git").exists());
    assert!(remote_commits(&sb, &remote).is_empty());
}

/// Library level: the remote moves between `prepare` and the push, so the first push is
/// rejected; `commit_and_push` fetches, rebases its own commit and pushes once more.
#[test]
fn rejected_push_retries_once_after_rebase() {
    use agent_brain::journal::config::JournalConfig;
    use agent_brain::journal::gitsync::{self, GitCtx, Prepared, Published};
    use std::collections::BTreeSet;

    let sb = Sandbox::new();
    // Only this test runs git through the library in this process; keep it on the sandbox
    // config (every other git call here sets its own environment).
    std::env::set_var("GIT_CONFIG_GLOBAL", &sb.git_config);
    std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    let remote = bare_remote(&sb, "vault");
    let cfg = JournalConfig {
        repo_url: Some(remote.to_str().unwrap().to_string()),
        clone_dir: sb.root.join("lib-clone"),
        ..JournalConfig::default()
    };
    let ctx = GitCtx::from_cfg(&cfg, std::time::Duration::from_secs(1));
    let none = BTreeSet::new();
    let day = |title: &str| format!("---\ndate: x\n---\n{}\n# {title}\n", marker());

    assert_eq!(gitsync::prepare(&ctx, &cfg, &none), Prepared::Ready);
    write(&cfg.clone_dir, DAY_FILE, &day("một"));
    let first = gitsync::commit_and_push(&ctx, &cfg, &[DAY_FILE.to_string()], "j1", true);
    assert!(matches!(first, Published::Pushed { .. }), "{first:?}");

    let work = checkout(&sb, &remote);
    human_push(
        &sb,
        &remote,
        &work,
        &[(OTHER_NOTE, "ghi chú khác\n")],
        "ghi chú",
    );

    let second_day = "Nhật ký/2026/01/2026-01-16.md";
    write(&cfg.clone_dir, second_day, &day("hai"));
    let second = gitsync::commit_and_push(&ctx, &cfg, &[second_day.to_string()], "j2", true);
    let Published::Pushed { sha } = second else {
        panic!("{second:?}");
    };
    assert_eq!(rev(&sb, &remote, "main"), sha);
    assert_eq!(remote_commits(&sb, &remote).len(), 3);
    assert_eq!(merges(&sb, &remote, "main"), "0");
    assert!(cfg.clone_dir.join(OTHER_NOTE).is_file());
}

// ---------------------------------------------------------------- Task 3: failure policy

#[test]
fn secret_blocked_export_mails_user() {
    const PLANTED: &str = "zq7plantedsecret9x";
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    sb.write_config(json!({"journal": journal_section(&remote)}));
    let conn = setup_projects(&sb, &[(PLANTED, None)]);
    let task = seed_task(&conn, PLANTED, "viec-mot", "Việc có mã dự án lạ");
    seed_run(
        &conn,
        task,
        "dev",
        "succeeded",
        None,
        local_ms(2026, 1, 15, 10, 0, 0),
        Some(local_ms(2026, 1, 15, 10, 20, 0)),
        Some(1),
    );
    drop(conn);

    let now = now().to_string();
    let out = sb.run_env(
        &["journal", "export"],
        None,
        &[
            (agent_brain::journal::NOW_ENV, now.as_str()),
            ("AB_TEST_JOURNAL_TOKEN", PLANTED),
        ],
    );
    failed(&out);
    assert!(!out.stdout.contains(PLANTED) && !out.stderr.contains(PLANTED));

    let db = sb.db();
    let rows: Vec<(String, String)> = db
        .prepare("SELECT kind, text FROM journal_mail ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let secret: Vec<&(String, String)> = rows.iter().filter(|(k, _)| k == "secret").collect();
    assert_eq!(secret.len(), 1, "{rows:?}");
    assert!(!secret[0].1.contains(PLANTED), "{}", secret[0].1);
    assert!(secret[0].1.contains("known-value"), "{}", secret[0].1);
    assert!(remote_commits(&sb, &remote).is_empty());
}

// ------------------------------------------- Plan 07-09: --no-push then push on the same clone

/// Files of the commit at `r` (`git show --name-only`), paths as stored.
fn commit_files(sb: &Sandbox, repo: &Path, r: &str) -> Vec<String> {
    sb.git(
        repo,
        &[
            "-c",
            "core.quotepath=false",
            "show",
            "--name-only",
            "--format=",
            r,
        ],
    )
    .lines()
    .map(str::to_string)
    .collect()
}

/// The `<n>` of the commit subject `journal: <stamp> (<n> tệp)`.
fn subject_count(subject: &str) -> usize {
    let open = subject.rfind('(').expect("subject has a count");
    subject[open + 1..]
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn assert_clean(sb: &Sandbox, repo: &Path) {
    assert_eq!(
        sb.git(repo, &["status", "--porcelain", "--untracked-files=all"])
            .trim(),
        ""
    );
}

#[test]
fn no_push_then_push_commits_pending_files() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    fixture(&sb, &remote);
    let clone = clone_dir(&sb);

    ok(&export(&sb, &["--no-push"], now()));
    assert!(remote_commits(&sb, &remote).is_empty());
    assert!(clone.join(DAY_FILE).is_file());
    let status = sb.git(&clone, &["status", "--porcelain", "--untracked-files=all"]);
    assert!(status.contains("??"), "{status}");

    let out = export(&sb, &[], now());
    ok(&out);
    assert!(out.stdout.contains("đã đẩy"), "{}", out.stdout);

    let commits = remote_commits(&sb, &remote);
    assert_eq!(commits.len(), 1, "{commits:?}");
    assert!(commits[0].starts_with("agent-brain journal|"), "{commits:?}");
    let co = checkout(&sb, &remote);
    let files = commit_files(&sb, &co, "HEAD");
    assert!(files.iter().any(|f| f == DAY_FILE), "{files:?}");
    assert!(files.iter().any(|f| f == "Today.md"), "{files:?}");
    assert!(files.len() >= 4, "{files:?}");
    assert_eq!(subject_count(&commits[0]), files.len(), "{commits:?}");
    assert!(
        out.stdout.contains(&format!("đã đẩy {} tệp", files.len())),
        "{}",
        out.stdout
    );
    assert_clean(&sb, &clone);
}

#[test]
fn no_push_then_push_is_idempotent() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    fixture(&sb, &remote);

    ok(&export(&sb, &["--no-push"], now()));
    ok(&export(&sb, &[], now()));
    assert_eq!(remote_commits(&sb, &remote).len(), 1);

    let out = export(&sb, &[], now());
    ok(&out);
    assert!(out.stdout.contains("không có thay đổi"), "{}", out.stdout);
    assert_eq!(remote_commits(&sb, &remote).len(), 1);
}

#[test]
fn no_push_then_push_on_non_empty_vault() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    let older = format!("---\ndate: 2026-01-15\n---\n{}\n# Ngày cũ\n", marker());
    let human = "Ghi chú viết tay, không có dòng đánh dấu.\n";
    seed_remote(&sb, &remote, &[(DAY_FILE, &older), (HUMAN_NOTE, human)]);
    fixture(&sb, &remote);
    let clone = clone_dir(&sb);

    ok(&export(&sb, &["--no-push"], now()));
    assert_eq!(remote_commits(&sb, &remote).len(), 1);
    ok(&export(&sb, &[], now()));

    let commits = remote_commits(&sb, &remote);
    assert_eq!(commits.len(), 2, "{commits:?}");
    assert!(commits[0].starts_with("agent-brain journal|"), "{commits:?}");
    let co = checkout(&sb, &remote);
    let files = commit_files(&sb, &co, "HEAD");
    assert!(files.iter().any(|f| f == DAY_FILE), "{files:?}");
    assert!(files.iter().any(|f| f == "Today.md"), "{files:?}");
    assert!(!files.iter().any(|f| f == HUMAN_NOTE), "{files:?}");
    assert_eq!(subject_count(&commits[0]), files.len(), "{commits:?}");
    let day = std::fs::read_to_string(co.join(DAY_FILE)).unwrap();
    assert!(!day.contains("# Ngày cũ"), "{day}");
    assert_eq!(std::fs::read_to_string(co.join(HUMAN_NOTE)).unwrap(), human);
    assert_clean(&sb, &clone);
}

#[test]
fn no_push_removed_stale_then_push_commits_deletion() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    let stale = format!("---\ndate: 2025-12-01\n---\n{}\n# Ngày cũ\n", marker());
    seed_remote(&sb, &remote, &[(STALE_DAY, &stale)]);
    fixture(&sb, &remote);
    let clone = clone_dir(&sb);

    ok(&export(&sb, &["--no-push"], now()));
    assert!(!clone.join(STALE_DAY).exists(), "stale day kept on disk");
    assert_eq!(remote_commits(&sb, &remote).len(), 1);

    ok(&export(&sb, &[], now()));
    let commits = remote_commits(&sb, &remote);
    assert_eq!(commits.len(), 2, "{commits:?}");
    let co = checkout(&sb, &remote);
    assert!(!co.join(STALE_DAY).exists(), "deletion not pushed");
    assert!(co.join(DAY_FILE).is_file());
    let files = commit_files(&sb, &co, "HEAD");
    assert!(files.iter().any(|f| f == STALE_DAY), "{files:?}");
    assert_eq!(subject_count(&commits[0]), files.len(), "{commits:?}");
    assert_clean(&sb, &clone);
}

#[test]
fn pending_never_stages_foreign_or_obsidian() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    fixture(&sb, &remote);
    let clone = clone_dir(&sb);
    let foreign = "Nhật ký/2026/01/ghi-chu-tay.md";
    let obsidian = ".obsidian/app.json";

    ok(&export(&sb, &["--no-push"], now()));
    write(&clone, foreign, "ghi chú tay, không có dòng đánh dấu\n");
    write(&clone, obsidian, "{\"token\": \"khong-duoc-dua-len\"}\n");
    ok(&export(&sb, &[], now()));

    let commits = remote_commits(&sb, &remote);
    assert_eq!(commits.len(), 1, "{commits:?}");
    let co = checkout(&sb, &remote);
    let files = commit_files(&sb, &co, "HEAD");
    assert!(files.iter().any(|f| f == DAY_FILE), "{files:?}");
    assert!(!files.iter().any(|f| f == foreign), "{files:?}");
    assert!(
        !files.iter().any(|f| f.starts_with(".obsidian/")),
        "{files:?}"
    );
    assert_eq!(subject_count(&commits[0]), files.len(), "{commits:?}");
    assert!(clone.join(foreign).is_file());
    assert!(clone.join(obsidian).is_file());
}

#[test]
fn no_push_stages_nothing() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    fixture(&sb, &remote);
    let clone = clone_dir(&sb);

    ok(&export(&sb, &["--no-push"], now()));
    assert_eq!(
        sb.git(&clone, &["diff", "--cached", "--name-only"]).trim(),
        ""
    );
    assert!(remote_commits(&sb, &remote).is_empty());
    assert!(clone.join(DAY_FILE).is_file());
}
