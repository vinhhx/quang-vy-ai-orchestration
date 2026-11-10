//! Fixture builder shared by the journal integration tests (Phase 7). Everything lives in the
//! test's Sandbox: a local bare repository is the vault remote, brain.db is the sandbox DB, and
//! the clock comes from `AGENT_BRAIN_JOURNAL_NOW_MS`. All texts are invented.

use crate::common::{RunOut, Sandbox};
use chrono::TimeZone;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// `git init --bare -b main` under the sandbox root.
pub fn bare_remote(sb: &Sandbox, name: &str) -> PathBuf {
    let dir = sb.root.join(format!("{name}.git"));
    std::fs::create_dir_all(&dir).unwrap();
    sb.git(&dir, &["init", "--bare", "-b", "main"]);
    dir
}

/// Epoch ms of a wall time at +07:00.
pub fn local_ms(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> i64 {
    chrono::FixedOffset::east_opt(7 * 3600)
        .unwrap()
        .with_ymd_and_hms(y, mo, d, h, mi, s)
        .single()
        .unwrap()
        .timestamp_millis()
}

/// The `journal` config section pointing at `remote`, days counted at +07:00.
pub fn journal_section(remote: &Path) -> Value {
    json!({"repo_url": remote.to_str().unwrap(), "utc_offset_minutes": 420})
}

/// One git repo per project, the projects map (with `journal_group` when given), and an opened
/// brain.db with the projects synced.
pub fn setup_projects(sb: &Sandbox, projects: &[(&str, Option<&str>)]) -> Connection {
    let mut entries = Vec::new();
    for (slug, group) in projects {
        let repo = sb.make_repo(slug, &[], None, None);
        let mut entry = sb.project_entry(&repo, slug, "work");
        if let Some(g) = group {
            entry["journal_group"] = json!(g);
        }
        entries.push(entry);
    }
    sb.write_map(Value::Array(entries), &[]);
    let conn = agent_brain::db::open(&sb.db_path).unwrap();
    let map = sb.read_map();
    agent_brain::db::sync_projects(&conn, &agent_brain::projects::projects_of(&map)).unwrap();
    conn
}

fn project_id(conn: &Connection, slug: &str) -> i64 {
    conn.query_row(
        "SELECT id FROM projects WHERE slug = ?1",
        params![slug],
        |r| r.get(0),
    )
    .unwrap()
}

fn project_of_task(conn: &Connection, task_id: i64) -> i64 {
    conn.query_row(
        "SELECT project_id FROM tasks WHERE id = ?1",
        params![task_id],
        |r| r.get(0),
    )
    .unwrap()
}

/// A task of project `project_slug`; `title` is stored as `tasks.title` (the request).
pub fn seed_task(conn: &Connection, project_slug: &str, task_slug: &str, title: &str) -> i64 {
    let pid = project_id(conn, project_slug);
    agent_brain::db::upsert_task(
        conn,
        pid,
        task_slug,
        title,
        &format!("task/{task_slug}"),
        None,
        None,
    )
    .unwrap()
}

/// An intake row and a started proposal with `title` linked to `task_id`.
pub fn seed_proposal(conn: &Connection, task_id: i64, title: &str) {
    let pid = project_of_task(conn, task_id);
    let intake =
        agent_brain::db::insert_intake(conn, "ui", Some(pid), title, "normal", false).unwrap();
    let prop = agent_brain::db::insert_proposal(
        conn,
        &agent_brain::db::NewProposal {
            intake_id: intake,
            project_id: pid,
            title,
            request: title,
            priority: "normal",
            team: "mac-dinh",
            workflow: "nhanh",
            reason: None,
            level: "",
        },
    )
    .unwrap();
    conn.execute(
        "UPDATE proposals SET task_id = ?1, status = 'started' WHERE id = ?2",
        params![task_id, prop],
    )
    .unwrap();
}

/// A finished (or failed …) run: `db::insert_run` then one UPDATE of the facts.
#[allow(clippy::too_many_arguments)]
pub fn seed_run(
    conn: &Connection,
    task_id: i64,
    role: &str,
    status: &str,
    outcome: Option<&str>,
    started_ms: i64,
    ended_ms: Option<i64>,
    round: Option<i64>,
) -> i64 {
    let pid = project_of_task(conn, task_id);
    let id = agent_brain::db::insert_run(
        conn,
        &agent_brain::db::NewRun {
            task_id: Some(task_id),
            project_id: pid,
            role: role.to_string(),
            profile: "work".to_string(),
            model: None,
            pid: 0,
            pgid: 0,
            pid_start: 0,
            base_commit: None,
            run_dir: None,
        },
    )
    .unwrap();
    conn.execute(
        "UPDATE runs SET status = ?1, outcome = ?2, started_at = ?3, ended_at = ?4, round = ?5 WHERE id = ?6",
        params![status, outcome, started_ms, ended_ms, round, id],
    )
    .unwrap();
    id
}

/// `agent-brain journal export <args>` with the clock fixed at `now_ms`.
pub fn export(sb: &Sandbox, args: &[&str], now_ms: i64) -> RunOut {
    let mut full = vec!["journal", "export"];
    full.extend_from_slice(args);
    let now = now_ms.to_string();
    sb.run_env(
        &full,
        None,
        &[(agent_brain::journal::NOW_ENV, now.as_str())],
    )
}

/// The default clone directory of the exporter inside the sandbox.
pub fn clone_dir(sb: &Sandbox) -> PathBuf {
    sb.xdg_data.join("agent-brain").join("journal")
}

static CHECKOUTS: AtomicUsize = AtomicUsize::new(0);

/// A fresh clone of `remote` for assertions.
pub fn checkout(sb: &Sandbox, remote: &Path) -> PathBuf {
    let n = CHECKOUTS.fetch_add(1, Ordering::SeqCst);
    let dir = sb.root.join(format!("checkout-{n}"));
    sb.git(
        &sb.root,
        &[
            "clone",
            "-q",
            "--branch",
            "main",
            remote.to_str().unwrap(),
            dir.to_str().unwrap(),
        ],
    );
    dir
}

/// `git log --format=%an|%s main` of the remote; empty when the branch does not exist.
pub fn remote_commits(sb: &Sandbox, remote: &Path) -> Vec<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["rev-parse", "--verify", "--quiet", "refs/heads/main"])
        .current_dir(remote)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", &sb.home)
        .env("GIT_CONFIG_GLOBAL", &sb.git_config)
        .env("GIT_CONFIG_NOSYSTEM", "1");
    let exists = cmd.output().map(|o| o.status.success()).unwrap_or(false);
    if !exists {
        return Vec::new();
    }
    sb.git(remote, &["log", "--format=%an|%s", "main"])
        .lines()
        .map(str::to_string)
        .collect()
}
