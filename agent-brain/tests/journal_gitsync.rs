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
