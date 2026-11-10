//! Phase 7 plan 10: the export guards and config checks from the code review (07-REVIEW.md
//! WR-01, WR-02, WR-03, WR-07, WR-11, WR-12, IN-01, IN-02, IN-03, IN-09). Every remote is a
//! local bare repository in the sandbox, every clone a temp directory; no network. Texts,
//! projects and secrets are invented.

mod common;
#[allow(dead_code)]
#[path = "support/journal.rs"]
mod jsupport;

use agent_brain::journal::config::{parse, JournalConfig};
use agent_brain::journal::gitsync::GitStop;
use agent_brain::journal::model::MARKER;
use agent_brain::journal::{
    self, collect, render, state, writer, ExportOutcome, ExportParams, ExportReport,
};
use chrono::{FixedOffset, NaiveDate};
use common::Sandbox;
use jsupport::*;
use rusqlite::{params, Connection};
use serde_json::json;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

const DAY_FILE: &str = "Nhật ký/2026/01/2026-01-15.md";

fn tz() -> FixedOffset {
    FixedOffset::east_opt(7 * 3600).unwrap()
}

fn jan(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 1, d).unwrap()
}

fn now() -> i64 {
    local_ms(2026, 1, 15, 20, 0, 0)
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

fn marked(title: &str) -> String {
    format!("---\ndate: x\n---\n{MARKER}\n# {title}\n")
}

/// One task of `project` with one run on `at` (+07:00), ending 20 minutes later.
fn task_with_run(conn: &Connection, project: &str, slug: &str, status: &str, at: i64) -> i64 {
    let task = seed_task(conn, project, slug, &format!("Việc mẫu {slug}"));
    seed_run(
        conn,
        task,
        "dev",
        status,
        None,
        at,
        Some(at + 20 * 60_000),
        Some(1),
    );
    task
}

/// A finished task on 2026-01-`d` at 10:00 (+07:00).
fn done_on(conn: &Connection, project: &str, d: u32) -> i64 {
    task_with_run(
        conn,
        project,
        &format!("viec-{d:02}"),
        "succeeded",
        local_ms(2026, 1, d, 10, 0, 0),
    )
}

fn mail_count(sb: &Sandbox, kind: &str) -> i64 {
    sb.db()
        .query_row(
            "SELECT count(*) FROM journal_mail WHERE kind = ?1",
            params![kind],
            |r| r.get(0),
        )
        .unwrap()
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if entry.file_type().unwrap().is_dir() {
            out.extend(files_under(&p));
        } else {
            out.push(p);
        }
    }
    out
}

fn assert_clean(sb: &Sandbox, repo: &Path) {
    let status = sb.git(repo, &["status", "--porcelain"]);
    assert!(status.trim().is_empty(), "{status}");
}

// ------------------------------------------------------------------ WR-01: deletion and clock

/// (a) A wiped brain.db against a vault of 12 days: refused before anything is written,
/// committed or pushed, one mail for the episode; `--allow-removals` lets it through once.
#[test]
fn wiped_db_is_refused_until_allow_removals() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    sb.write_config(json!({"journal": journal_section(&remote)}));
    let conn = setup_projects(&sb, &[("du-an-a", None)]);
    for d in 1..=12 {
        done_on(&conn, "du-an-a", d);
    }
    drop(conn);
    let at = local_ms(2026, 1, 12, 20, 0, 0);
    ok(&export(&sb, &[], at));
    let clone = clone_dir(&sb);
    let first_day = clone.join("Nhật ký/2026/01/2026-01-01.md");
    assert!(first_day.is_file());
    let pushed = remote_commits(&sb, &remote).len();
    let head = sb.git(&remote, &["rev-parse", "main"]);

    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", sb.db_path.display()));
    }
    for _ in 0..2 {
        let out = export(&sb, &[], at);
        failed(&out);
        assert!(out.stderr.contains("từ chối xuất"), "{}", out.stderr);
        assert!(out.stderr.contains("--allow-removals"), "{}", out.stderr);
        assert!(!out.stderr.contains(&clone.display().to_string()));
        assert!(first_day.is_file());
        assert_clean(&sb, &clone);
        assert_eq!(sb.git(&remote, &["rev-parse", "main"]), head);
    }
    assert_eq!(mail_count(&sb, "refused"), 1);
    let last: Option<String> = state::get(&sb.db(), state::KEY_LAST_EXPORT_MS).unwrap();
    assert_eq!(last, Some(at.to_string()));

    ok(&export(&sb, &["--allow-removals"], at));
    assert!(!first_day.exists());
    assert_eq!(remote_commits(&sb, &remote).len(), pushed + 1);
}

/// (b) The clock is days behind the data: refused, nothing cloned or pushed, and
/// `--allow-removals` does not bypass it.
#[test]
fn clock_behind_data_is_refused() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    sb.write_config(json!({"journal": journal_section(&remote)}));
    let conn = setup_projects(&sb, &[("du-an-a", None)]);
    done_on(&conn, "du-an-a", 15);
    drop(conn);

    let early = local_ms(2026, 1, 10, 9, 0, 0);
    for args in [&[][..], &["--allow-removals"][..]] {
        let out = export(&sb, args, early);
        failed(&out);
        assert!(
            out.stderr.contains("đồng hồ máy đang chậm hơn dữ liệu"),
            "{}",
            out.stderr
        );
    }
    assert!(!clone_dir(&sb).exists());
    assert!(remote_commits(&sb, &remote).is_empty());
    assert_eq!(mail_count(&sb, "refused"), 1);

    ok(&export(&sb, &[], now()));
    assert_eq!(remote_commits(&sb, &remote).len(), 1);
}

// ------------------------------------------------------------------ WR-02: slugs and errors

/// (c) Slugs `a..b` and `foo.` are not used as file names (they fall back along the usual
/// chain: slugified name, then `du-an-{id}`), and the export succeeds.
#[test]
fn dotted_slugs_fall_back_and_export_succeeds() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    sb.write_config(json!({"journal": journal_section(&remote)}));
    let conn = setup_projects(&sb, &[("a..b", None), ("foo.", None)]);
    done_on(&conn, "a..b", 14);
    done_on(&conn, "foo.", 15);
    drop(conn);

    ok(&export(&sb, &[], now()));
    let co = checkout(&sb, &remote);
    let mut names: Vec<String> = std::fs::read_dir(co.join("Dự án"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["ab.md".to_string(), "foo.md".to_string()]);
    assert_eq!(state_blocked(&sb), "");

    // With no usable name either, the key is `du-an-{id}`.
    let mut taken = HashSet::new();
    assert_eq!(
        collect::project_file_key("a..b", "..", 7, &mut taken),
        "du-an-7"
    );
    assert_eq!(
        collect::project_file_key("foo.", "", 8, &mut taken),
        "du-an-8"
    );
}

fn state_blocked(sb: &Sandbox) -> String {
    state::get(&sb.db(), state::KEY_BLOCKED)
        .unwrap()
        .unwrap_or_default()
}

/// (d) An export `Err` is recorded: `last_export_ms` advances (the loop backs off), the
/// vault is untouched, and exactly one mail exists across two failing runs.
#[test]
fn export_error_is_recorded_once_and_backs_off() {
    let dir = tempfile::tempdir().unwrap();
    let conn = agent_brain::db::open(&dir.path().join("brain.db")).unwrap();
    // A broken brain.db: the read transaction of the exporter fails.
    conn.execute_batch("DROP TABLE plan_approvals").unwrap();
    let cfg = JournalConfig {
        repo_url: Some(dir.path().join("vault.git").display().to_string()),
        clone_dir: dir.path().join("clone"),
        utc_offset_minutes: Some(420),
        ..JournalConfig::default()
    };
    let map = json!({"projects": []});
    let lock_dir = dir.path().join("data");
    let params_at = |now_ms: i64| ExportParams {
        conn: &conn,
        map: &map,
        cfg: &cfg,
        secrets: &[],
        now_ms,
        push: true,
        lock_dir: &lock_dir,
        kill_grace: Duration::from_secs(1),
    };
    let t0 = local_ms(2026, 1, 15, 10, 0, 0);
    assert!(journal::run_export_once(&params_at(t0)).is_err());
    let last = state::get(&conn, state::KEY_LAST_EXPORT_MS).unwrap();
    assert_eq!(last, Some(t0.to_string()));
    assert!(!journal::loop_::export_due(
        t0 + 60_000,
        &tz(),
        &cfg,
        Some(t0),
        Some(jan(15)),
        false
    ));
    let t1 = t0 + 31 * 60_000;
    assert!(journal::run_export_once(&params_at(t1)).is_err());
    let rows: Vec<String> = conn
        .prepare("SELECT text FROM journal_mail WHERE kind = 'failed'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(rows, vec![state::FAILED_MAIL_TEXT.to_string()]);
    assert_eq!(
        state::get(&conn, state::KEY_BLOCKED).unwrap().as_deref(),
        Some("failed")
    );
    assert!(!cfg.clone_dir.exists());
}

// ------------------------------------------------------------------ WR-03: episodes

/// (e) A dirty clone is mailed once while it stays dirty, and again after it was fixed (a
/// successful export) and came back.
#[test]
fn dirty_clone_alert_recurs_in_new_episode() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    sb.write_config(json!({"journal": journal_section(&remote)}));
    let conn = setup_projects(&sb, &[("du-an-a", None)]);
    done_on(&conn, "du-an-a", 15);
    drop(conn);
    ok(&export(&sb, &[], now()));
    let gi = clone_dir(&sb).join(".gitignore");
    let original = std::fs::read_to_string(&gi).unwrap();
    let dirty = format!("{original}ghi-chu-rieng/\n");

    std::fs::write(&gi, &dirty).unwrap();
    failed(&export(&sb, &[], now()));
    failed(&export(&sb, &[], now() + 60_000));
    assert_eq!(mail_count(&sb, "dirty"), 1);

    std::fs::write(&gi, &original).unwrap();
    ok(&export(&sb, &[], now() + 120_000));
    std::fs::write(&gi, &dirty).unwrap();
    failed(&export(&sb, &[], now() + 180_000));
    failed(&export(&sb, &[], now() + 240_000));
    assert_eq!(mail_count(&sb, "dirty"), 2);
}

/// (e) The same conflict after a successful export is a new episode and a new mail.
#[test]
fn conflict_alert_recurs_in_new_episode() {
    let dir = tempfile::tempdir().unwrap();
    let conn = agent_brain::db::open(&dir.path().join("brain.db")).unwrap();
    let cfg = JournalConfig::default();
    let report = |outcome: ExportOutcome| ExportReport {
        outcome,
        today: jan(15),
        today_summary: None,
        changed: Vec::new(),
        removed: Vec::new(),
        foreign: Vec::new(),
    };
    let conflict = report(ExportOutcome::Stopped(GitStop::Conflict {
        detail: DAY_FILE.to_string(),
    }));
    let count = || -> i64 {
        conn.query_row(
            "SELECT count(*) FROM journal_mail WHERE kind = 'conflict'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    state::record_outcome(&conn, &cfg, &conflict, 100).unwrap();
    state::record_outcome(&conn, &cfg, &conflict, 200).unwrap();
    assert_eq!(count(), 1);
    state::record_outcome(&conn, &cfg, &report(ExportOutcome::UpToDate), 300).unwrap();
    state::record_outcome(&conn, &cfg, &conflict, 400).unwrap();
    state::record_outcome(&conn, &cfg, &conflict, 500).unwrap();
    assert_eq!(count(), 2);
}

// ------------------------------------------------------------------ WR-07: carry-over

/// (f) Today's "Cần anh xử lý" carries a recent failure, but not one from 60 days ago nor a
/// task whose own status is a success; the list of the day it happened is unchanged.
#[test]
fn stale_or_closed_attention_is_not_carried_over() {
    let sb = Sandbox::new();
    let conn = setup_projects(&sb, &[("du-an-a", None)]);
    let old_at = local_ms(2025, 11, 16, 10, 0, 0);
    let recent_at = local_ms(2026, 1, 12, 10, 0, 0);
    let old = task_with_run(&conn, "du-an-a", "viec-cu", "failed", old_at);
    let recent = task_with_run(&conn, "du-an-a", "viec-moi", "failed", recent_at);
    let closed = task_with_run(&conn, "du-an-a", "viec-dong", "failed", recent_at);
    conn.execute(
        "UPDATE tasks SET status = ?1 WHERE id = ?2",
        params![agent_brain::tasks::SUCCESS_STATUSES[0], closed],
    )
    .unwrap();
    let cfg = JournalConfig {
        utc_offset_minutes: Some(420),
        ..JournalConfig::default()
    };
    let model = collect::collect(&conn, &sb.read_map(), &cfg, &[], jan(15), &tz()).unwrap();
    let today: Vec<i64> = model.attention[&jan(15)]
        .iter()
        .map(|i| i.task_id)
        .collect();
    assert_eq!(today, vec![recent]);
    let old_day = NaiveDate::from_ymd_opt(2025, 11, 16).unwrap();
    assert!(model.attention[&old_day].iter().any(|i| i.task_id == old));

    // A larger cap carries the old one again.
    let wide = JournalConfig {
        attention_max_days: 90,
        ..cfg
    };
    let model = collect::collect(&conn, &sb.read_map(), &wide, &[], jan(15), &tz()).unwrap();
    let mut today: Vec<i64> = model.attention[&jan(15)]
        .iter()
        .map(|i| i.task_id)
        .collect();
    today.sort_unstable();
    assert_eq!(today, vec![old, recent]);
}

// ------------------------------------------------------------------ WR-11: symlinks

/// (g) A symlinked `Nhật ký` pointing outside the clone: nothing is written outside, the
/// file is reported foreign, the rest of the export is written.
#[test]
fn symlinked_journal_dir_is_not_written_through() {
    let clone = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), clone.path().join("Nhật ký")).unwrap();
    let mut files = BTreeMap::new();
    files.insert(DAY_FILE.to_string(), marked("ngày"));
    files.insert("Today.md".to_string(), marked("hôm nay"));

    let r = writer::apply(clone.path(), &files).unwrap();
    assert_eq!(r.foreign, vec![DAY_FILE.to_string()]);
    assert!(clone.path().join("Today.md").is_file());
    assert!(files_under(outside.path()).is_empty());
}

// ------------------------------------------------------------------ WR-12, IN-09: config

/// (h) A relative `clone_dir` and a `repo_url` with a password are rejected; the message
/// names the key and never echoes the password.
#[test]
fn relative_clone_dir_and_credential_url_rejected() {
    let e = parse(Some(&json!({"clone_dir": "journal"}))).unwrap_err();
    assert!(e.message.contains("journal.clone_dir"), "{}", e.message);
    assert!(parse(Some(&json!({"clone_dir": "/srv/vault"}))).is_ok());

    for url in [
        "https://user:pw-zq7planted@git.example.test/o/vault.git",
        "ssh://git:pw-zq7planted@git.example.test/o/vault.git",
    ] {
        let e = parse(Some(&json!({ "repo_url": url }))).unwrap_err();
        assert!(e.message.contains("journal.repo_url"), "{}", e.message);
        assert!(e.message.contains("SSH"), "{}", e.message);
        assert!(!e.message.contains("zq7planted"), "{}", e.message);
    }
    let good = json!({"repo_url": "ssh://git@git.example.test:22/o/v.git"});
    assert!(parse(Some(&good)).is_ok());
}

// ------------------------------------------------------------------ IN-01, IN-02

/// (i) The fallback `du-an-{id}` never takes the key of another project.
#[test]
fn fallback_key_does_not_collide() {
    let mut taken = HashSet::new();
    assert_eq!(
        collect::project_file_key("du-an-9", "", 3, &mut taken),
        "du-an-9"
    );
    assert_eq!(
        collect::project_file_key("..lạ", "", 9, &mut taken),
        "du-an-9-2"
    );
    assert_eq!(
        collect::project_file_key("du-an-9-2", "", 4, &mut taken),
        "du-an-4"
    );
    assert_eq!(taken.len(), 3);
}

fn today_page(days: &[u32]) -> String {
    let sb = Sandbox::new();
    let conn = setup_projects(&sb, &[("du-an-a", None)]);
    for d in days {
        done_on(&conn, "du-an-a", *d);
    }
    let cfg = JournalConfig {
        utc_offset_minutes: Some(420),
        ..JournalConfig::default()
    };
    let model = collect::collect(&conn, &sb.read_map(), &cfg, &[], jan(15), &tz()).unwrap();
    render::render_all(&model, &cfg)["Today.md"].clone()
}

/// (j) `Hôm qua` only for yesterday, otherwise `Trước đó`.
#[test]
fn previous_day_label() {
    let page = today_page(&[14, 15]);
    assert!(page.contains("Hôm qua: [[2026-01-14]]"), "{page}");
    let page = today_page(&[10, 15]);
    assert!(page.contains("Trước đó: [[2026-01-10]]"), "{page}");
    assert!(!page.contains("Hôm qua"), "{page}");
}

// ------------------------------------------------------------------ IN-03: secret set

/// (k) The CLI's known-secret set is the serve loop's set: it includes the dashboard token
/// when one is given and the Telegram token next to the map file.
#[test]
fn cli_and_loop_scan_the_same_secrets() {
    let sb = Sandbox::new();
    let cfg_dir = sb.root.join("cau-hinh");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    let tg_token = "123456789:zq7telegramtokenplanted0000000000";
    let tg_file = cfg_dir.join(agent_brain::telegram::config::FILE_NAME);
    std::fs::write(
        &tg_file,
        json!({"token": tg_token, "chat_id": 1, "paired_at": 1}).to_string(),
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tg_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let dash = "zq7dashboardtokenplanted01234567";
    sb.write_config(json!({}));
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&sb.config_path).unwrap()).unwrap();
    let cfg = agent_brain::config::config_from_value(&raw).unwrap();

    let set = journal::known_secrets(&cfg, &cfg_dir, Some(dash));
    assert!(set.contains(&dash.to_string()), "dashboard token missing");
    assert!(set.contains(&tg_token.to_string()), "telegram token");

    let st = agent_brain::dashboard::AppState {
        orch: std::sync::RwLock::new(cfg.orchestrator.clone()),
        cfg: cfg.clone(),
        port: 0,
        token: dash.to_string(),
        map_path: cfg_dir.join("projects_map.json"),
        db_path: sb.db_path.clone(),
        exe: PathBuf::from("/bin/true"),
        hints: tokio::sync::broadcast::channel(1).0,
        shutdown: tokio::sync::watch::channel(false).1,
        jobs: agent_brain::dashboard::jobs::JobManager::new(PathBuf::from("/bin/true")),
    };
    assert_eq!(set, agent_brain::inputs::collect_secrets(&st));
}
