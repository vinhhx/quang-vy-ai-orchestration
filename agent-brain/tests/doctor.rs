//! `agent-brain doctor`: detection of dead, silent and long runs, stale drivers, stuck queue,
//! low disk and a dead serve; the only repair (`run-dead` -> `lost`); alert dedup; the CLI exit
//! codes. Everything runs in a temp dir with a temp brain.db; the "processes" are python3
//! sleepers the test starts itself and stops by the pid it recorded. The real data, config and
//! Telegram are never touched (the one Telegram test talks to the local fake).

mod common;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use agent_brain::db;
use agent_brain::doctor::{
    alert_text, heal_dead_runs, scan, Dedup, DoctorOpts, Finding, MAX_PER_MESSAGE,
};
use agent_brain::procs;
use common::{wait_until, Sandbox};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tg::{write_config, FakeTelegram, PAIRED_CHAT};

const MIN: i64 = 60_000;

/// A python3 process this test started. Stopped on drop, by the pid recorded here.
struct Fake {
    child: std::process::Child,
    pid: i64,
    start: i64,
}

impl Fake {
    fn start() -> Fake {
        let child = std::process::Command::new("python3")
            .args(["-I", "-c", "import time\nwhile True:\n    time.sleep(1)\n"])
            .stdin(std::process::Stdio::null())
            .spawn()
            .expect("spawn python3");
        let pid = child.id() as i32;
        let mut id = None;
        assert!(wait_until(Duration::from_secs(5), || {
            id = procs::proc_id(pid);
            id.is_some()
        }));
        Fake {
            child,
            pid: pid as i64,
            start: id.unwrap().start as i64,
        }
    }

    /// The pid and start of a process that has already exited.
    fn dead() -> (i64, i64) {
        let mut f = Fake::start();
        let ids = (f.pid, f.start);
        f.child.kill().expect("kill the fake");
        f.child.wait().expect("reap the fake");
        assert!(!procs::alive(procs::ProcId {
            pid: ids.0 as i32,
            start: ids.1 as u64
        }));
        ids
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn open_db(sb: &Sandbox) -> Connection {
    let conn = db::open(&sb.db_path).expect("open brain.db");
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .unwrap();
    if n == 0 {
        conn.execute(
            "INSERT INTO projects (slug, path, profile) VALUES ('web-ban-hang', '/tmp/p1', 'work')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO projects (slug, path, profile) VALUES ('khac', '/tmp/p2', 'work')",
            [],
        )
        .unwrap();
    }
    conn
}

fn add_task(conn: &Connection, project_id: i64, slug: &str) -> i64 {
    conn.execute(
        "INSERT INTO tasks (project_id, slug, title, branch, created_at, updated_at)
         VALUES (?1, ?2, 'việc bí mật của người dùng', ?3, 0, 0)",
        params![project_id, slug, format!("task/{slug}")],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn add_run(
    conn: &Connection,
    task_id: Option<i64>,
    pid: Option<(i64, i64)>,
    age_min: i64,
    run_dir: Option<&Path>,
) -> i64 {
    let (p, s) = pid.unwrap_or((0, 0));
    let id = db::insert_run(
        conn,
        &db::NewRun {
            task_id,
            project_id: 1,
            role: "dev".to_string(),
            profile: "work".to_string(),
            model: None,
            pid: p,
            pgid: p,
            pid_start: s,
            base_commit: None,
            run_dir: run_dir.map(|d| d.to_string_lossy().to_string()),
        },
    )
    .unwrap();
    if pid.is_none() {
        conn.execute("UPDATE runs SET pid = NULL, pgid = NULL, pid_start = NULL WHERE id = ?1", [id])
            .unwrap();
    }
    conn.execute(
        "UPDATE runs SET started_at = ?1 WHERE id = ?2",
        params![db::now_ms() - age_min * MIN, id],
    )
    .unwrap();
    id
}

fn run_status(conn: &Connection, id: i64) -> String {
    conn.query_row("SELECT status FROM runs WHERE id = ?1", [id], |r| r.get(0))
        .unwrap()
}

/// `<root>/<name>/stream.jsonl` with `lines`, modified `age_min` minutes ago.
fn stream(root: &Path, name: &str, lines: &[&str], age_min: u64) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("stream.jsonl");
    std::fs::write(&file, lines.join("\n") + "\n").unwrap();
    set_age(&file, age_min);
    dir
}

fn set_age(file: &Path, age_min: u64) {
    let f = std::fs::OpenOptions::new().write(true).open(file).unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(age_min * 60))
        .unwrap();
}

fn opts(sb: &Sandbox) -> DoctorOpts {
    DoctorOpts {
        result_no_exit_minutes: 3,
        silent_minutes: 30,
        run_stall_minutes: 20,
        run_max_minutes: 150,
        driver_stale_minutes: 10,
        queue_stuck_minutes: 60,
        disk_min_gb: 20,
        realert_minutes: 60,
        free_gb: None,
        check_serve: false,
        data_dir: sb.xdg_data.join("agent-brain"),
    }
}

fn kinds(findings: &[Finding]) -> Vec<String> {
    findings.iter().map(|f| f.key.clone()).collect()
}

fn scan_now(conn: &Connection, o: &DoctorOpts) -> Vec<Finding> {
    scan(conn, db::now_ms(), o).expect("scan")
}

const RESULT_LINE: &str = r#"{"type":"result","subtype":"success","result":"xong"}"#;
const ASSISTANT_LINE: &str = r#"{"type":"assistant","message":{"content":[]}}"#;

// (a) run-dead and the one repair

#[test]
fn a_dead_process_is_run_dead_and_heal_marks_it_lost() {
    let sb = Sandbox::new();
    let conn = open_db(&sb);
    let task = add_task(&conn, 1, "t1");
    let o = opts(&sb);
    let dead = add_run(&conn, Some(task), Some(Fake::dead()), 5, None);
    let live = Fake::start();
    let alive = add_run(&conn, Some(task), Some((live.pid, live.start)), 5, None);
    let no_pid_young = add_run(&conn, None, None, 1, None);
    let no_pid_old = add_run(&conn, None, None, 3, None);

    let mut got = scan_now(&conn, &o);
    let dead_keys: Vec<String> = got
        .iter()
        .filter(|f| f.kind == "run-dead")
        .map(|f| f.key.clone())
        .collect();
    assert_eq!(
        dead_keys,
        vec![format!("run-dead:{dead}"), format!("run-dead:{no_pid_old}")],
        "dead pid and a missing pid older than 2 minutes; not the live one, not the one just spawned"
    );
    let f = got.iter().find(|f| f.run_id == Some(dead)).unwrap();
    assert_eq!(f.task_id, Some(task));
    assert!(f.text.contains("agent-brain doctor --heal"), "{}", f.text);
    // scan reports only: nothing changed.
    for id in [dead, alive, no_pid_young, no_pid_old] {
        assert_eq!(run_status(&conn, id), "running");
    }

    let healed = heal_dead_runs(&conn, &mut got).expect("heal");
    assert_eq!(healed, 2);
    assert_eq!(run_status(&conn, dead), "lost");
    assert_eq!(run_status(&conn, no_pid_old), "lost");
    assert_eq!(run_status(&conn, alive), "running", "a live run is never touched");
    assert_eq!(run_status(&conn, no_pid_young), "running");
    let f = got.iter().find(|f| f.run_id == Some(dead)).unwrap();
    assert!(f.text.contains("Đã đánh dấu lost"), "{}", f.text);
    assert!(scan_now(&conn, &o).iter().all(|f| f.kind != "run-dead"));
}

#[test]
fn heal_drops_a_finding_whose_run_changed_meanwhile() {
    let sb = Sandbox::new();
    let conn = open_db(&sb);
    let dead = add_run(&conn, None, Some(Fake::dead()), 5, None);
    let mut got = scan_now(&conn, &opts(&sb));
    assert_eq!(got.len(), 1);
    db::mark_status(&conn, dead, "killed").unwrap();
    assert_eq!(heal_dead_runs(&conn, &mut got).unwrap(), 0);
    assert!(got.is_empty());
    assert_eq!(run_status(&conn, dead), "killed", "a killed run is not turned into lost");
}

// (b) result-no-exit

#[test]
fn result_event_with_a_live_process_and_an_old_file_is_result_no_exit() {
    let sb = Sandbox::new();
    let conn = open_db(&sb);
    let fake = Fake::start();
    let pid = Some((fake.pid, fake.start));
    let root = sb.root.join("rd");
    let task = add_task(&conn, 1, "t1");

    let stale = add_run(&conn, Some(task), pid, 10, Some(&stream(&root, "stale", &[ASSISTANT_LINE, RESULT_LINE], 5)));
    let fresh = add_run(&conn, Some(task), pid, 10, Some(&stream(&root, "fresh", &[ASSISTANT_LINE, RESULT_LINE], 1)));
    let agy = add_run(
        &conn,
        None,
        pid,
        10,
        Some(&stream(&root, "agy", &[r#"{"event":"init"}"#, r#"{"event":"result","status":"SUCCESS"}"#], 5)),
    );
    let talking = add_run(&conn, None, pid, 10, Some(&stream(&root, "talking", &[RESULT_LINE, ASSISTANT_LINE], 5)));

    let got = scan_now(&conn, &opts(&sb));
    let keys = kinds(&got);
    assert!(keys.contains(&format!("result-no-exit:{stale}")), "{keys:?}");
    assert!(keys.contains(&format!("result-no-exit:{agy}")), "{keys:?}");
    assert!(!keys.iter().any(|k| k.ends_with(&format!(":{fresh}"))), "{keys:?}");
    assert!(!keys.iter().any(|k| k.ends_with(&format!(":{talking}"))), "{keys:?}");
    let f = got.iter().find(|f| f.run_id == Some(stale)).unwrap();
    assert_eq!(f.kind, "result-no-exit");
    assert!(f.text.contains(&format!("agent-brain kill {task}")), "{}", f.text);
    assert!(!keys.iter().any(|k| k.starts_with("run-silent")), "a result is not silence: {keys:?}");
    assert_eq!(run_status(&conn, stale), "running", "only a warning, never a kill");
}

// (c) run-silent

#[test]
fn silence_beyond_the_stall_limit_is_run_silent() {
    let sb = Sandbox::new();
    let conn = open_db(&sb);
    let fake = Fake::start();
    let pid = Some((fake.pid, fake.start));
    let root = sb.root.join("sl");
    let task = add_task(&conn, 1, "t1");

    // run_stall_minutes = 20: the limit is 25 minutes.
    let quiet = add_run(&conn, Some(task), pid, 40, Some(&stream(&root, "quiet", &[ASSISTANT_LINE], 26)));
    let busy = add_run(&conn, None, pid, 40, Some(&stream(&root, "busy", &[ASSISTANT_LINE], 24)));
    let o = opts(&sb);
    let got = scan_now(&conn, &o);
    assert_eq!(kinds(&got), vec![format!("run-silent:{quiet}")], "{got:?}");
    assert!(got[0].text.contains(&format!("agent-brain kill {task}")), "{}", got[0].text);
    assert_eq!(run_status(&conn, busy), "running");

    // run_stall_minutes = 0: silent_minutes (30) is the limit.
    let mut off = opts(&sb);
    off.run_stall_minutes = 0;
    assert!(kinds(&scan_now(&conn, &off)).iter().all(|k| !k.starts_with("run-silent")));
    set_age(&root.join("busy/stream.jsonl"), 31);
    let keys = kinds(&scan_now(&conn, &off));
    assert_eq!(keys, vec![format!("run-silent:{busy}")]);
}

// (d) run-long

#[test]
fn a_run_over_run_max_minutes_is_run_long() {
    let sb = Sandbox::new();
    let conn = open_db(&sb);
    let fake = Fake::start();
    let pid = Some((fake.pid, fake.start));
    let root = sb.root.join("lg");
    let long = add_run(&conn, None, pid, 151, Some(&stream(&root, "long", &[ASSISTANT_LINE], 0)));
    let ok = add_run(&conn, None, pid, 149, Some(&stream(&root, "ok", &[ASSISTANT_LINE], 0)));
    let got = scan_now(&conn, &opts(&sb));
    assert_eq!(kinds(&got), vec![format!("run-long:{long}")], "{got:?}");
    assert_eq!(run_status(&conn, ok), "running");
}

// (e) driver-stale

#[test]
fn an_autopilot_task_with_no_driver_and_no_run_is_driver_stale() {
    let sb = Sandbox::new();
    let conn = open_db(&sb);
    let o = opts(&sb);
    let stale = add_task(&conn, 1, "stale");
    let recent = add_task(&conn, 1, "recent");
    let driven = add_task(&conn, 1, "driven");
    let busy = add_task(&conn, 1, "busy");
    let stopping = add_task(&conn, 1, "stopping");
    let ago = db::now_ms() - 11 * MIN;
    let live = Fake::start();
    for (t, state, updated) in [
        (stale, "running", ago),
        (recent, "running", db::now_ms() - 5 * MIN),
        (driven, "running", ago),
        (busy, "running", ago),
        (stopping, "stopping", ago),
    ] {
        conn.execute(
            "UPDATE tasks SET auto_state = ?1, updated_at = ?2 WHERE id = ?3",
            params![state, updated, t],
        )
        .unwrap();
    }
    conn.execute(
        "UPDATE tasks SET auto_pid = ?1, auto_pid_start = ?2 WHERE id = ?3",
        params![live.pid, live.start, driven],
    )
    .unwrap();
    let (dp, ds) = Fake::dead();
    conn.execute(
        "UPDATE tasks SET auto_pid = ?1, auto_pid_start = ?2 WHERE id = ?3",
        params![dp, ds, stale],
    )
    .unwrap();
    // A live run of the task: not stale.
    add_run(&conn, Some(busy), Some((live.pid, live.start)), 1, None);

    let got = scan_now(&conn, &o);
    let keys: Vec<String> = got.iter().filter(|f| f.kind == "driver-stale").map(|f| f.key.clone()).collect();
    assert_eq!(
        keys,
        vec![format!("driver-stale:{stale}"), format!("driver-stale:{stopping}")],
        "{got:?}"
    );
    let f = got.iter().find(|f| f.task_id == Some(stale)).unwrap();
    assert!(f.text.contains(&format!("agent-brain auto {stale}")), "{}", f.text);
    assert!(f.text.contains(&format!("agent-brain kill {stale}")), "{}", f.text);
    // Reports only: auto_state is not changed.
    let state: String = conn
        .query_row("SELECT auto_state FROM tasks WHERE id = ?1", [stale], |r| r.get(0))
        .unwrap();
    assert_eq!(state, "running");
}

// (f) queue-stuck

#[test]
fn a_waiting_action_older_than_the_limit_in_an_idle_project_is_queue_stuck() {
    let sb = Sandbox::new();
    let conn = open_db(&sb);
    let o = opts(&sb);
    let idle = add_task(&conn, 1, "idle");
    let fresh = add_task(&conn, 1, "fresh");
    let held = add_task(&conn, 2, "held");
    let holder = add_task(&conn, 2, "holder");
    let old = db::now_ms() - 61 * MIN;
    let q_idle = db::enqueue_action(&conn, idle, "run_dev", None, "test").unwrap();
    let q_fresh = db::enqueue_action(&conn, fresh, "run_qa", None, "test").unwrap();
    let q_held = db::enqueue_action(&conn, held, "run_dev", None, "test").unwrap();
    for q in [q_idle, q_held] {
        conn.execute("UPDATE queued_actions SET created_at = ?1 WHERE id = ?2", params![old, q])
            .unwrap();
    }
    conn.execute(
        "UPDATE queued_actions SET created_at = ?1 WHERE id = ?2",
        params![db::now_ms() - 30 * MIN, q_fresh],
    )
    .unwrap();
    // Project 2 is busy with a live run of another task: its queue is waiting for good reason.
    let live = Fake::start();
    conn.execute(
        "INSERT INTO runs (task_id, project_id, role, engine, profile, pid, pgid, pid_start, status, started_at)
         VALUES (?1, 2, 'dev', 'claude', 'work', ?2, ?2, ?3, 'running', ?4)",
        params![holder, live.pid, live.start, db::now_ms() - MIN],
    )
    .unwrap();

    let got = scan_now(&conn, &o);
    assert_eq!(kinds(&got), vec![format!("queue-stuck:{q_idle}")], "{got:?}");
    assert_eq!(got[0].task_id, Some(idle));
    assert!(got[0].text.contains("run_dev"), "{}", got[0].text);
}

// (g) disk-low

#[test]
fn free_space_below_the_limit_is_disk_low_and_zero_turns_it_off() {
    let sb = Sandbox::new();
    let conn = open_db(&sb);
    let mut o = opts(&sb);
    o.free_gb = Some(19);
    assert_eq!(kinds(&scan_now(&conn, &o)), vec!["disk-low".to_string()]);
    o.free_gb = Some(20);
    assert!(scan_now(&conn, &o).is_empty());
    o.free_gb = None;
    assert!(scan_now(&conn, &o).is_empty(), "unknown free space is not a finding");
    o.free_gb = Some(1);
    o.disk_min_gb = 0;
    assert!(scan_now(&conn, &o).is_empty(), "disk_min_gb = 0 is off");
    let real = agent_brain::doctor::free_gb_of(&sb.root);
    assert!(real.is_some(), "df works on the temp dir");
}

// (h) serve-down

#[test]
fn a_dead_serve_with_work_running_is_serve_down_only_with_check_serve() {
    let sb = Sandbox::new();
    let conn = open_db(&sb);
    let mut o = opts(&sb);
    let dir = o.data_dir.clone();
    std::fs::create_dir_all(&dir).unwrap();
    let live = Fake::start();
    let (dead_pid, _) = Fake::dead();
    std::fs::write(dir.join("serve.pid"), format!("{dead_pid}\n")).unwrap();
    add_run(&conn, None, Some((live.pid, live.start)), 1, None);

    assert!(scan_now(&conn, &o).is_empty(), "without --check-serve serve is not looked at");
    o.check_serve = true;
    let got = scan_now(&conn, &o);
    assert_eq!(kinds(&got), vec!["serve-down".to_string()], "{got:?}");

    // The JSON lock that `agent-brain serve` writes counts the same way.
    std::fs::remove_file(dir.join("serve.pid")).unwrap();
    std::fs::write(
        dir.join("serve.lock"),
        json!({"pid": dead_pid, "port": 7437, "started_at": 1}).to_string(),
    )
    .unwrap();
    assert_eq!(kinds(&scan_now(&conn, &o)), vec!["serve-down".to_string()]);
    // A live serve: fine.
    std::fs::write(dir.join("serve.lock"), json!({"pid": live.pid}).to_string()).unwrap();
    assert!(scan_now(&conn, &o).is_empty());
    // A dead serve with nothing running: fine too.
    std::fs::write(dir.join("serve.lock"), json!({"pid": dead_pid}).to_string()).unwrap();
    conn.execute("UPDATE runs SET status = 'succeeded'", []).unwrap();
    assert!(scan_now(&conn, &o).is_empty());
    // No file at all: nothing to compare with.
    std::fs::remove_file(dir.join("serve.lock")).unwrap();
    add_run(&conn, None, Some((live.pid, live.start)), 1, None);
    assert!(scan_now(&conn, &o).is_empty());
}

// (i) dedup

fn finding(key: &str) -> Finding {
    Finding {
        kind: "run-long",
        key: key.to_string(),
        task_id: None,
        run_id: None,
        text: format!("cảnh báo {key}"),
    }
}

#[test]
fn dedup_sends_first_not_again_inside_realert_again_after_and_forgets() {
    let hour = 60 * MIN;
    let mut d = Dedup::new();
    let a = [finding("run-long:1")];
    assert_eq!(d.select(&a, 0, hour).len(), 1, "first time");
    assert!(d.select(&a, 30 * MIN, hour).is_empty());
    assert!(d.select(&a, hour - 1, hour).is_empty());
    assert_eq!(d.select(&a, hour, hour).len(), 1, "again at realert");
    assert!(d.select(&a, hour + MIN, hour).is_empty(), "the clock restarts at each send");
    // Gone: forgotten; back later it is new at once.
    assert!(d.select(&[], hour + 2 * MIN, hour).is_empty());
    assert!(d.is_empty());
    assert_eq!(d.select(&a, hour + 3 * MIN, hour).len(), 1);
    // Only the new key of a mixed scan.
    let ab = [finding("run-long:1"), finding("run-long:2")];
    let due = d.select(&ab, hour + 4 * MIN, hour);
    assert_eq!(kinds(&due), vec!["run-long:2".to_string()]);
}

#[test]
fn dedup_state_is_read_and_written_through_a_json_file_with_mode_600() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub").join("doctor-state.json");
    assert!(Dedup::load(&path).is_empty(), "missing file = empty state");
    let mut d = Dedup::new();
    assert_eq!(d.select(&[finding("disk-low")], 1000, 60 * MIN).len(), 1);
    d.save(&path).expect("save");
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(v.is_object());
    let mut back = Dedup::load(&path);
    assert_eq!(back, d);
    assert!(back.select(&[finding("disk-low")], 2000, 60 * MIN).is_empty());
    assert_eq!(back.select(&[finding("disk-low")], 1000 + 60 * MIN, 60 * MIN).len(), 1);
    std::fs::write(&path, "not json").unwrap();
    assert!(Dedup::load(&path).is_empty(), "a damaged file is an empty state");
    assert!(std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .all(|e| !e.unwrap().file_name().to_string_lossy().contains("tmp")));
}

// (j) CLI

fn cli_sandbox() -> Sandbox {
    let sb = Sandbox::new();
    // A fixed free-space limit of 0 keeps the CLI tests independent of the disk they run on.
    sb.write_config(json!({"doctor": {"disk_min_gb": 0}}));
    sb
}

#[test]
fn cli_exit_codes_json_and_heal() {
    let sb = cli_sandbox();
    // No brain.db yet: the command cannot read it.
    let none = sb.run(&["doctor"], None);
    assert_eq!(none.code, 2, "{none:?}");
    assert!(!sb.db_path.exists(), "doctor does not create the database");

    let conn = open_db(&sb);
    let clean = sb.run(&["doctor"], None);
    assert_eq!(clean.code, 0, "{clean:?}");
    assert_eq!(clean.stdout.trim(), "");
    let clean_json = sb.run(&["doctor", "--json"], None);
    assert_eq!(clean_json.code, 0);
    assert_eq!(serde_json::from_str::<Value>(&clean_json.stdout).unwrap(), json!([]));

    let task = add_task(&conn, 1, "t1");
    let dead = add_run(&conn, Some(task), Some(Fake::dead()), 5, None);
    let live = Fake::start();
    let alive = add_run(&conn, None, Some((live.pid, live.start)), 5, None);

    let found = sb.run(&["doctor"], None);
    assert_eq!(found.code, 1, "{found:?}");
    assert_eq!(found.stdout.lines().count(), 1, "one line per finding: {}", found.stdout);
    assert!(found.stdout.starts_with("[run-dead] "), "{}", found.stdout);
    assert_eq!(run_status(&conn, dead), "running", "no repair without --heal (and no reconcile on start)");

    let js = sb.run(&["doctor", "--json"], None);
    assert_eq!(js.code, 1);
    let arr: Value = serde_json::from_str(&js.stdout).expect("valid JSON");
    let first = &arr.as_array().unwrap()[0];
    assert_eq!(first["kind"], "run-dead");
    assert_eq!(first["key"], format!("run-dead:{dead}"));
    assert_eq!(first["task_id"], task);
    assert_eq!(first["run_id"], dead);
    assert!(first["text"].as_str().unwrap().contains(&format!("Run {dead}")));
    let mut names: Vec<&str> = first.as_object().unwrap().keys().map(|k| k.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["key", "kind", "run_id", "task_id", "text"]);

    let healed = sb.run(&["doctor", "--heal"], None);
    assert_eq!(healed.code, 1, "{healed:?}");
    assert!(healed.stdout.contains("Đã đánh dấu lost"), "{}", healed.stdout);
    assert_eq!(run_status(&conn, dead), "lost");
    assert_eq!(run_status(&conn, alive), "running");
    assert_eq!(sb.run(&["doctor"], None).code, 0);

    let serve = sb.run(&["doctor", "--check-serve"], None);
    assert_eq!(serve.code, 0, "no serve file, nothing to compare: {serve:?}");
    let dir = sb.xdg_data.join("agent-brain");
    std::fs::write(dir.join("serve.pid"), format!("{}\n", Fake::dead().0)).unwrap();
    let down = sb.run(&["doctor", "--check-serve"], None);
    assert_eq!(down.code, 1, "{down:?}");
    assert!(down.stdout.starts_with("[serve-down] "), "{}", down.stdout);
    assert_eq!(sb.run(&["doctor"], None).code, 0, "serve is only looked at with --check-serve");
}

#[test]
fn cli_notify_without_telegram_setup_exits_2() {
    let sb = cli_sandbox();
    let conn = open_db(&sb);
    add_run(&conn, None, Some(Fake::dead()), 5, None);
    let res = sb.run(&["doctor", "--notify"], None);
    assert_eq!(res.code, 2, "{res:?}");
    assert!(res.stderr.contains("Telegram"), "{}", res.stderr);
    assert!(!sb.xdg_data.join("agent-brain").join("doctor-state.json").exists());
    // Also when there is nothing to report: the setup is missing either way.
    conn.execute("UPDATE runs SET status = 'lost'", []).unwrap();
    assert_eq!(sb.run(&["doctor", "--notify"], None).code, 2);
}

#[test]
fn cli_notify_sends_one_message_and_not_again_until_realert() {
    let sb = cli_sandbox();
    let conn = open_db(&sb);
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let task = add_task(&conn, 1, "t1");
    let dead = add_run(&conn, Some(task), Some(Fake::dead()), 5, None);
    let env = tg.env();
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    let first = sb.run_env(&["doctor", "--notify"], None, &env);
    assert_eq!(first.code, 1, "{first:?}");
    let sent = tg.calls("sendMessage");
    assert_eq!(sent.len(), 1, "{sent:?}");
    let text = sent[0]["json"]["text"].as_str().unwrap().to_string();
    assert_eq!(sent[0]["json"]["chat_id"], json!(PAIRED_CHAT));
    assert!(text.starts_with("agent-brain doctor: 1 cảnh báo mới"), "{text}");
    assert!(text.contains(&format!("Run {dead} (dev, task {task}, dự án web-ban-hang)")), "{text}");
    assert_no_leak(&sb, &text);

    let state = sb.xdg_data.join("agent-brain").join("doctor-state.json");
    assert!(state.is_file());
    let second = sb.run_env(&["doctor", "--notify"], None, &env);
    assert_eq!(second.code, 1, "findings still exit 1: {second:?}");
    assert_eq!(tg.calls("sendMessage").len(), 1, "not sent again inside realert_minutes");

    // After realert_minutes it is sent again: age the stored send time.
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
    for (_, at) in v["sent"].as_object_mut().unwrap().iter_mut() {
        *at = json!(at.as_i64().unwrap() - 61 * MIN);
    }
    std::fs::write(&state, v.to_string()).unwrap();
    let third = sb.run_env(&["doctor", "--notify"], None, &env);
    assert_eq!(third.code, 1);
    assert_eq!(tg.calls("sendMessage").len(), 2);

    // Healed and gone: nothing to send, and the key is forgotten.
    let healed = sb.run_env(&["doctor", "--notify", "--heal"], None, &env);
    assert_eq!(healed.code, 1);
    assert_eq!(run_status(&conn, dead), "lost");
    assert_eq!(sb.run_env(&["doctor", "--notify"], None, &env).code, 0);
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
    assert!(v["sent"].as_object().unwrap().is_empty(), "{v}");
}

#[test]
fn cli_notify_failure_keeps_the_finding_new() {
    let sb = cli_sandbox();
    let conn = open_db(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    add_run(&conn, None, Some(Fake::dead()), 5, None);
    // Nothing listens on this port: the send fails.
    let res = sb.run_env(
        &["doctor", "--notify"],
        None,
        &[("AGENT_BRAIN_TELEGRAM_API", "http://127.0.0.1:9"), ("AGENT_BRAIN_TELEGRAM_BACKOFF_MS", "50")],
    );
    assert_eq!(res.code, 2, "{res:?}");
    assert!(!res.stderr.contains(tg::FAKE_TOKEN), "{}", res.stderr);
    assert!(!sb.xdg_data.join("agent-brain").join("doctor-state.json").exists());
}

// (k) the message carries no path, token or request text

fn assert_no_leak(sb: &Sandbox, text: &str) {
    assert!(!text.contains(tg::FAKE_TOKEN), "{text}");
    assert!(!text.contains("bí mật"), "the request/title must not be sent: {text}");
    assert!(!text.contains(sb.root.to_string_lossy().as_ref()), "{text}");
    for word in text.split_whitespace() {
        let w = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '-' && c != '.');
        assert!(!w.starts_with('/') && !w.contains("/home") && !w.contains("/tmp"), "path-like word {word:?} in {text}");
    }
}

#[test]
fn messages_carry_no_absolute_path_token_or_request_text() {
    let sb = Sandbox::new();
    let conn = open_db(&sb);
    let fake = Fake::start();
    let pid = Some((fake.pid, fake.start));
    let root = sb.root.join("leak");
    let task = add_task(&conn, 1, "t1");
    add_run(&conn, Some(task), Some(Fake::dead()), 5, None);
    add_run(&conn, Some(task), None, 5, None);
    add_run(&conn, Some(task), pid, 10, Some(&stream(&root, "r1", &[RESULT_LINE], 5)));
    add_run(&conn, Some(task), pid, 200, Some(&stream(&root, "r2", &[ASSISTANT_LINE], 100)));
    let t2 = add_task(&conn, 2, "t2");
    conn.execute(
        "UPDATE tasks SET auto_state = 'running', updated_at = ?1 WHERE id = ?2",
        params![db::now_ms() - 30 * MIN, t2],
    )
    .unwrap();
    let q = add_task(&conn, 2, "t3");
    let qid = db::enqueue_action(&conn, q, "run_dev", Some("ghi chú bí mật"), "test").unwrap();
    conn.execute(
        "UPDATE queued_actions SET created_at = ?1 WHERE id = ?2",
        params![db::now_ms() - 90 * MIN, qid],
    )
    .unwrap();
    let mut o = opts(&sb);
    o.free_gb = Some(1);
    o.check_serve = true;
    std::fs::create_dir_all(&o.data_dir).unwrap();
    std::fs::write(o.data_dir.join("serve.pid"), format!("{}\n", Fake::dead().0)).unwrap();

    let got = scan_now(&conn, &o);
    let all: std::collections::BTreeSet<&str> = got.iter().map(|f| f.kind).collect();
    for k in [
        "run-dead", "result-no-exit", "run-silent", "run-long", "driver-stale", "queue-stuck",
        "disk-low", "serve-down",
    ] {
        assert!(all.contains(k), "{k} missing in {all:?}");
    }
    for f in &got {
        assert_no_leak(&sb, &f.text);
    }
    assert_no_leak(&sb, &alert_text(&got));
}

#[test]
fn a_message_lists_at_most_five_findings_and_counts_the_rest() {
    let many: Vec<Finding> = (0..8).map(|i| finding(&format!("run-long:{i}"))).collect();
    let text = alert_text(&many);
    assert!(text.starts_with("agent-brain doctor: 8 cảnh báo mới"), "{text}");
    assert_eq!(text.lines().filter(|l| l.starts_with("- ")).count(), MAX_PER_MESSAGE);
    assert!(text.contains("… và 3 cảnh báo nữa"), "{text}");
    let few = alert_text(&many[..2]);
    assert!(!few.contains("…"), "{few}");
}
