//! `RunRequest::finish_after_result` for a claude-format engine: a run that already printed its
//! final `result` event but whose process does not exit is stopped after the grace. The runner is
//! driven directly with a fake engine (a python3 script in a temp dir) and the real
//! `stream::Summary` as the observer: no claude, no agy, no real brain.db or run directory.

use agent_brain::runner::{supervise, RunObserver, RunOutcome, RunRequest, RunStatus, RunStop};
use agent_brain::stream::Summary;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// argv: pidfile, mode. Starts one sleeping child, records "{own} {child}" in the pidfile, prints
/// two claude stream lines and then, by mode:
/// - `ok`: a successful `result`, then hangs for 600 s;
/// - `error`: a `result` with `is_error: true`, then hangs;
/// - `max_turns`: a `result` with subtype `error_max_turns` (is_error false), then hangs;
/// - `exit_early`: a successful `result`, then exits 0 after 0.5 s (the child is killed);
/// - `silent`: no `result` at all, hangs.
const FAKE: &str = r#"
import os, subprocess, sys, time
pidfile, mode = sys.argv[1], sys.argv[2]
child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(600)"])
with open(pidfile, "w") as f:
    f.write("%d %d" % (os.getpid(), child.pid))
def say(text):
    sys.stdout.write(text + "\n")
    sys.stdout.flush()
say('{"type":"system","subtype":"init","session_id":"s1","model":"fake"}')
say('{"type":"assistant","message":{"id":"m1","content":[{"type":"text","text":"xong"}],"usage":{"input_tokens":1,"output_tokens":1}}}')
def result(subtype, is_error):
    say('{"type":"result","subtype":"%s","is_error":%s,"result":"xong","num_turns":2,'
        '"total_cost_usd":1.08,"session_id":"s1","usage":{"input_tokens":1,"output_tokens":1}}'
        % (subtype, "true" if is_error else "false"))
if mode == "ok":
    result("success", False)
    time.sleep(600)
elif mode == "error":
    result("success", True)
    time.sleep(600)
elif mode == "max_turns":
    result("error_max_turns", False)
    time.sleep(600)
elif mode == "exit_early":
    result("success", False)
    time.sleep(0.5)
    child.kill()
elif mode == "silent":
    time.sleep(600)
"#;

struct Obs {
    pidfile: PathBuf,
    summary: Summary,
    procs: Vec<agent_brain::procs::ProcId>,
    notices: Vec<String>,
}

impl Obs {
    fn new(dir: &Path) -> Self {
        Obs {
            pidfile: dir.join("fake.pid"),
            summary: Summary::new(None, None),
            procs: Vec::new(),
            notices: Vec::new(),
        }
    }
}

impl RunObserver for Obs {
    fn on_spawn(&mut self, _: i32, _: i32, _: u64) -> agent_brain::error::BrainResult<()> {
        Ok(())
    }

    fn on_line(&mut self, line: &[u8]) {
        self.summary.feed(line);
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

    fn result_seen(&self) -> bool {
        self.summary.result_seen()
    }

    fn result_is_error(&self) -> bool {
        self.summary.result_is_error()
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
    timeout: Duration,
    finish_after_result: Option<Duration>,
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
        timeout,
        kill_grace: Duration::from_secs(1),
        stdout_file: false,
        redact: Vec::new(),
        container_stop: None,
        finish_after_result,
        env_watch: None,
        stall_after: None,
    }
}

fn run(
    mode: &str,
    timeout: Duration,
    grace: Option<Duration>,
) -> (RunOutcome, Obs, Reaper, Duration) {
    let tmp = tempfile::tempdir().unwrap();
    let req = request(tmp.path(), mode, timeout, grace);
    let mut obs = Obs::new(tmp.path());
    let t0 = Instant::now();
    let outcome = supervise(&req, &mut obs).expect("supervise");
    let reaper = Reaper(obs.procs.clone());
    (outcome, obs, reaper, t0.elapsed())
}

fn assert_tree_gone(obs: &Obs) {
    assert_eq!(obs.procs.len(), 2, "pidfile read at the first line");
    for p in &obs.procs {
        assert!(!agent_brain::procs::alive(*p), "process {p:?} survived");
    }
}

#[test]
fn claude_result_then_hang_is_stopped_after_grace_as_success() {
    let (outcome, obs, _reaper, elapsed) =
        run("ok", Duration::from_secs(60), Some(Duration::from_secs(2)));
    assert!(elapsed < Duration::from_secs(15), "run took {elapsed:?}");
    assert!(obs.summary.result_seen());
    assert!(!obs.summary.result_is_error());
    assert_eq!(outcome.status, RunStatus::Succeeded);
    assert_eq!(outcome.stopped_by, Some(RunStop::AfterResult));
    assert!(
        obs.notices
            .iter()
            .any(|n| n.contains("Run đã trả kết quả nhưng tiến trình chưa thoát sau 2s")),
        "notices: {:?}",
        obs.notices
    );
    assert_tree_gone(&obs);
}

#[test]
fn error_result_is_never_turned_into_success() {
    for mode in ["error", "max_turns"] {
        let (outcome, obs, _reaper, elapsed) =
            run(mode, Duration::from_secs(60), Some(Duration::from_secs(2)));
        assert!(elapsed < Duration::from_secs(15), "{mode}: run took {elapsed:?}");
        assert!(obs.summary.result_is_error(), "{mode}");
        assert_eq!(outcome.stopped_by, Some(RunStop::AfterResult), "{mode}");
        assert_eq!(outcome.status, RunStatus::Failed, "{mode}");
        assert_tree_gone(&obs);
    }
}

#[test]
fn result_then_clean_exit_before_grace_is_not_stopped() {
    let (outcome, obs, _reaper, elapsed) = run(
        "exit_early",
        Duration::from_secs(60),
        Some(Duration::from_secs(3)),
    );
    assert!(elapsed < Duration::from_secs(3), "run took {elapsed:?}");
    assert_eq!(outcome.status, RunStatus::Succeeded);
    assert_eq!(outcome.stopped_by, None);
    assert_eq!(outcome.exit_code, Some(0));
    assert!(
        !obs.notices.iter().any(|n| n.contains("chưa thoát")),
        "notices: {:?}",
        obs.notices
    );
}

#[test]
fn without_a_result_the_after_result_stop_never_fires() {
    // Silent engine, grace 1 s, overall timeout 3 s: only the timeout may stop it.
    let (outcome, obs, _reaper, _elapsed) =
        run("silent", Duration::from_secs(3), Some(Duration::from_secs(1)));
    assert!(!obs.summary.result_seen());
    assert_eq!(outcome.status, RunStatus::TimedOut);
    assert_eq!(outcome.stopped_by, Some(RunStop::Timeout));
    assert_tree_gone(&obs);
}

#[test]
fn grace_by_engine_stream() {
    use agent_brain::engine::StreamFormat;
    assert_eq!(
        agent_brain::roles::after_result_grace(StreamFormat::Agy),
        Duration::from_secs(15)
    );
    assert_eq!(
        agent_brain::roles::after_result_grace(StreamFormat::Claude),
        Duration::from_secs(30)
    );
    assert_eq!(
        agent_brain::roles::after_result_grace(StreamFormat::Text),
        Duration::from_secs(30)
    );
}
