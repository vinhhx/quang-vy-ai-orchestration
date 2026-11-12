//! `RunRequest::stall_after` (`run_stall_minutes`): a run whose engine writes no line for the
//! limit is stopped like a timeout. The runner is driven directly with a fake engine (a python3
//! script in a temp dir): no agy, no claude, no real brain.db or run directory.

use agent_brain::runner::{
    stall_message, supervise, RunObserver, RunRequest, RunStatus, RunStop,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The fake engine. argv: pidfile, mode. It starts one sleeping child, records "{own} {child}"
/// in the pidfile, then:
/// - `silent`: prints two lines and goes quiet for 600 s;
/// - `steady`: prints a line every 0.5 s for 4 s and exits 0;
/// - `quiet_exit`: prints a line, stays quiet for 3 s and exits 0;
/// - `ignore_term`: ignores SIGTERM, prints one line and goes quiet for 600 s.
const FAKE: &str = r#"
import os, signal, subprocess, sys, time
pidfile, mode = sys.argv[1], sys.argv[2]
child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(600)"])
with open(pidfile, "w") as f:
    f.write("%d %d" % (os.getpid(), child.pid))
def say(text):
    sys.stdout.write(text + "\n")
    sys.stdout.flush()
if mode == "silent":
    say('{"n":1}'); say('{"n":2}')
    time.sleep(600)
elif mode == "steady":
    for i in range(8):
        say('{"n":%d}' % i)
        time.sleep(0.5)
    child.kill()
elif mode == "quiet_exit":
    say('{"n":1}')
    time.sleep(3)
    child.kill()
elif mode == "ignore_term":
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    say('{"n":1}')
    time.sleep(600)
"#;

struct Obs {
    pidfile: PathBuf,
    procs: Vec<agent_brain::procs::ProcId>,
    lines: usize,
    notices: Vec<String>,
    stop_on_first_line: bool,
}

impl Obs {
    fn new(dir: &Path) -> Self {
        Obs {
            pidfile: dir.join("fake.pid"),
            procs: Vec::new(),
            lines: 0,
            notices: Vec::new(),
            stop_on_first_line: false,
        }
    }
}

impl RunObserver for Obs {
    fn on_spawn(&mut self, _: i32, _: i32, _: u64) -> agent_brain::error::BrainResult<()> {
        Ok(())
    }

    fn on_line(&mut self, _line: &[u8]) {
        self.lines += 1;
        if self.procs.is_empty() {
            if let Ok(text) = std::fs::read_to_string(&self.pidfile) {
                for pid in text.split_whitespace().filter_map(|p| p.parse::<i32>().ok()) {
                    if let Some(id) = agent_brain::procs::proc_id(pid) {
                        self.procs.push(id);
                    }
                }
            }
        }
    }

    fn on_notice(&mut self, message: &str) {
        self.notices.push(message.to_string());
    }

    fn kill_requested(&mut self) -> bool {
        false
    }

    fn stop_requested(&self) -> Option<String> {
        (self.stop_on_first_line && self.lines >= 1).then(|| "stop de thu".to_string())
    }
}

/// Kills what the test recorded even when an assert fails half way.
struct Reaper(Vec<agent_brain::procs::ProcId>);

impl Drop for Reaper {
    fn drop(&mut self) {
        for p in &self.0 {
            if agent_brain::procs::alive(*p) {
                agent_brain::procs::signal_proc(*p, libc::SIGKILL);
            }
        }
    }
}

fn request(
    dir: &Path,
    mode: &str,
    stall_after: Option<Duration>,
    kill_grace: Duration,
    stdout_file: bool,
) -> RunRequest {
    let script = dir.join("fake_engine.py");
    std::fs::write(&script, FAKE).unwrap();
    let mut env = std::collections::BTreeMap::new();
    env.insert(
        "PATH".to_string(),
        std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string()),
    );
    RunRequest {
        argv: vec![
            "python3".to_string(),
            script.display().to_string(),
            dir.join("fake.pid").display().to_string(),
            mode.to_string(),
        ],
        cwd: dir.to_path_buf(),
        env,
        stderr_path: dir.join("stderr.txt"),
        stream_path: dir.join("stream.jsonl"),
        timeout: Duration::from_secs(120),
        kill_grace,
        stdout_file,
        redact: Vec::new(),
        container_stop: None,
        finish_after_result: None,
        env_watch: None,
        stall_after,
    }
}

#[test]
fn silent_engine_is_stopped_as_timed_out_with_its_whole_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let after = Duration::from_secs(2);
    let req = request(tmp.path(), "silent", Some(after), Duration::from_secs(1), false);
    let mut obs = Obs::new(tmp.path());

    let t0 = Instant::now();
    let outcome = supervise(&req, &mut obs).expect("supervise");
    let _reaper = Reaper(obs.procs.clone());

    assert!(
        t0.elapsed() < Duration::from_secs(20),
        "stopped after {:?}, expected about the 2 s limit",
        t0.elapsed()
    );
    assert!(t0.elapsed() >= Duration::from_secs(2), "stopped too early: {:?}", t0.elapsed());
    assert_eq!(outcome.status, RunStatus::TimedOut);
    assert_eq!(outcome.stopped_by, Some(RunStop::Stalled));
    assert_eq!(outcome.stalled_after, Some(after));
    let text = stall_message(after);
    assert!(text.contains("không có hoạt động nào"), "{text}");
    assert!(
        obs.notices.iter().any(|n| n == &text),
        "notices: {:?}",
        obs.notices
    );
    assert_eq!(obs.lines, 2);
    assert_eq!(obs.procs.len(), 2, "fake engine and its child recorded");
    for p in &obs.procs {
        assert!(!agent_brain::procs::alive(*p), "process {p:?} survived");
    }
}

#[test]
fn silent_engine_in_stdout_file_mode_is_stopped_too() {
    let tmp = tempfile::tempdir().unwrap();
    let after = Duration::from_secs(2);
    let req = request(tmp.path(), "silent", Some(after), Duration::from_secs(1), true);
    let mut obs = Obs::new(tmp.path());

    let outcome = supervise(&req, &mut obs).expect("supervise");
    let _reaper = Reaper(obs.procs.clone());

    assert_eq!(outcome.status, RunStatus::TimedOut);
    assert_eq!(outcome.stopped_by, Some(RunStop::Stalled));
    for p in &obs.procs {
        assert!(!agent_brain::procs::alive(*p), "process {p:?} survived");
    }
}

#[test]
fn engine_that_keeps_writing_is_not_stopped() {
    let tmp = tempfile::tempdir().unwrap();
    // 4 s of output with a line every 0.5 s against a 2 s limit: the clock is reset by each line.
    let req = request(
        tmp.path(),
        "steady",
        Some(Duration::from_secs(2)),
        Duration::from_secs(1),
        false,
    );
    let mut obs = Obs::new(tmp.path());

    let outcome = supervise(&req, &mut obs).expect("supervise");
    let _reaper = Reaper(obs.procs.clone());

    assert_eq!(outcome.status, RunStatus::Succeeded);
    assert_eq!(outcome.stopped_by, None);
    assert_eq!(outcome.stalled_after, None);
    assert_eq!(obs.lines, 8);
    assert!(
        !obs.notices.iter().any(|n| n.contains("không có hoạt động")),
        "notices: {:?}",
        obs.notices
    );
}

#[test]
fn without_a_limit_a_quiet_engine_ends_by_itself() {
    let tmp = tempfile::tempdir().unwrap();
    let req = request(tmp.path(), "quiet_exit", None, Duration::from_secs(1), false);
    let mut obs = Obs::new(tmp.path());

    let outcome = supervise(&req, &mut obs).expect("supervise");
    let _reaper = Reaper(obs.procs.clone());

    assert_eq!(outcome.status, RunStatus::Succeeded);
    assert_eq!(outcome.stopped_by, None);
    assert_eq!(outcome.stalled_after, None);
    assert!(obs.notices.is_empty(), "notices: {:?}", obs.notices);
}

#[test]
fn a_run_already_being_stopped_keeps_its_stop_reason() {
    let tmp = tempfile::tempdir().unwrap();
    // The observer stops the run at the first line; the engine ignores SIGTERM and stays quiet,
    // so the 1 s silence limit passes while the 3 s grace is still running.
    let req = request(
        tmp.path(),
        "ignore_term",
        Some(Duration::from_secs(1)),
        Duration::from_secs(3),
        false,
    );
    let mut obs = Obs::new(tmp.path());
    obs.stop_on_first_line = true;

    let outcome = supervise(&req, &mut obs).expect("supervise");
    let _reaper = Reaper(obs.procs.clone());

    assert_eq!(outcome.stopped_by, Some(RunStop::Observer));
    assert_eq!(outcome.status, RunStatus::Killed);
    assert_eq!(outcome.stalled_after, None);
    assert!(
        !obs.notices.iter().any(|n| n.contains("không có hoạt động")),
        "notices: {:?}",
        obs.notices
    );
    for p in &obs.procs {
        assert!(!agent_brain::procs::alive(*p), "process {p:?} survived");
    }
}

#[test]
fn stall_message_rounds_up_to_whole_minutes() {
    assert!(stall_message(Duration::from_secs(20 * 60)).contains("trong 20 phút"));
    assert!(stall_message(Duration::from_secs(2)).contains("trong 1 phút"));
}
