//! Engine `dsh` (D-35): DeepSeek Harness trong container, hộp đen chỉ văn bản.
//!
//! Lệnh chạy là `docker exec -w <thư mục dự án> <container> sh -c '<đoạn bọc cố định>' sh
//! <profile> <prompt>`: prompt chỉ đi qua tham số vị trí `$2`, không bao giờ nằm trong đoạn lệnh
//! của shell (T-06-85). Mỗi lượt chạy có một marker riêng (`ab-dsh-<12 hex>`) nằm trong dòng
//! lệnh của `sh`, để dừng đúng các tiến trình của lượt đó bên trong container (không bao giờ theo
//! tên, không bao giờ đụng vào chính container).

use crate::config::{Config, DshDef};
use crate::engine::{Engine, EnginePlan, StreamFormat};
use crate::error::{BrainError, BrainResult};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// What a dsh run persists so a kill (or a restart of the serve process) can find and remove
/// the container process it started.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DshRun {
    pub docker: String,
    pub container: String,
    /// Unique token of this run, used to find the container-side process again.
    pub marker: String,
}

pub const MARKER_PREFIX: &str = "ab-dsh-";

/// Number of hex digits after the prefix.
const MARKER_HEX_LEN: usize = 12;

/// The only `docker inspect` form agent-brain ever runs: the mount list. A bare inspect would
/// print the container environment, secrets included (T-06-89).
const INSPECT_MOUNTS_ARGS: [&str; 3] = ["inspect", "--format", "{{json .Mounts}}"];

/// Cap for every short docker call (mount list, process list, signal).
const DOCKER_CALL_CAP: Duration = Duration::from_secs(5);

/// Envs of the docker client that never come from a project profile or an engine key.
const STRIPPED_EXACT: [&str; 3] = [
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR",
    "CLAUDE_CODE_SUBAGENT_MODEL",
];

/// D-37 refusal text; the same wording as `teams::catalog_errors`.
fn non_dev_message(id: &str) -> String {
    format!("Engine '{id}' là kiểu dsh, chỉ dùng cho vai trò dev.")
}

/// A fresh marker: the prefix and 12 lowercase hex digits from `/dev/urandom`.
pub fn new_marker() -> String {
    let mut bytes = [0u8; MARKER_HEX_LEN / 2];
    let filled = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .is_ok();
    if !filled {
        // No random device: still unique per process and per call.
        use std::time::{SystemTime, UNIX_EPOCH};
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mixed = nanos ^ (u64::from(std::process::id()) << 32) ^ n.rotate_left(17);
        let n_bytes = bytes.len();
        bytes.copy_from_slice(&mixed.to_le_bytes()[..n_bytes]);
    }
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{MARKER_PREFIX}{hex}")
}

/// True for `ab-dsh-` followed by exactly 12 lowercase hex digits. Every signal path checks this
/// first: an empty or odd marker would match unrelated processes.
pub fn is_valid_marker(marker: &str) -> bool {
    match marker.strip_prefix(MARKER_PREFIX) {
        Some(hex) => {
            hex.len() == MARKER_HEX_LEN
                && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        }
        None => false,
    }
}

/// The shell text given to `sh -c`: constant but for the marker. The `:` builtin carries the
/// marker into the command line; `dsh` is not the last command, so `sh` stays its parent for the
/// whole run (A18). The prompt and the profile arrive as `$2` and `$1`.
pub fn wrapper_script(marker: &str) -> String {
    format!(": {marker}; dsh --profile \"$1\" \"$2\"; rc=$?; exit $rc")
}

/// The full docker argv of one run (argv[0] is the docker command).
pub fn build_argv(def: &DshDef, cwd: &Path, marker: &str, prompt: &str) -> Vec<String> {
    vec![
        def.docker.clone(),
        "exec".to_string(),
        "-w".to_string(),
        cwd.to_string_lossy().to_string(),
        def.container.clone(),
        "sh".to_string(),
        "-c".to_string(),
        wrapper_script(marker),
        "sh".to_string(),
        def.dsh_profile.clone(),
        prompt.to_string(),
    ]
}

/// `docker inspect` printed `{{json .Mounts}}`: true when a bind mount with the same source and
/// destination, read-write, contains `cwd` (path-component prefix, so `/mnt/data` does not cover
/// `/mnt/database`).
pub fn same_path_bind_covers(mounts_json: &str, cwd: &Path) -> bool {
    let Ok(serde_json::Value::Array(mounts)) =
        serde_json::from_str::<serde_json::Value>(mounts_json.trim())
    else {
        return false;
    };
    mounts.iter().any(|m| {
        let kind = m.get("Type").and_then(|v| v.as_str());
        let src = m.get("Source").and_then(|v| v.as_str());
        let dst = m.get("Destination").and_then(|v| v.as_str());
        let rw = m.get("RW").and_then(|v| v.as_bool());
        match (kind, src, dst, rw) {
            (Some("bind"), Some(src), Some(dst), Some(true)) => {
                !dst.is_empty() && src == dst && cwd.starts_with(dst)
            }
            _ => false,
        }
    })
}

/// The process env with every name that could carry a key or pick an account removed. Used for
/// docker calls that have no run env of their own (a kill from another process).
fn scrubbed_current_env() -> BTreeMap<String, String> {
    std::env::vars()
        .filter(|(k, _)| {
            !(k.starts_with("ANTHROPIC_")
                || STRIPPED_EXACT.contains(&k.as_str())
                || k.ends_with("_KEY")
                || k.ends_with("_TOKEN")
                || k.ends_with("_SECRET"))
        })
        .collect()
}

/// Runs the docker client with `args`, stdin from `/dev/null`, at most `cap`. `Some((success,
/// stdout))` once it exited; `None` when it could not start or ran over the cap (then only that
/// client process, which agent-brain started, is ended).
fn docker_capture(
    docker: &str,
    args: &[String],
    env: &BTreeMap<String, String>,
    cap: Duration,
) -> Option<(bool, String)> {
    let mut child = Command::new(docker)
        .args(args)
        .env_clear()
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        let _ = tx.send(String::from_utf8_lossy(&buf).to_string());
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if start.elapsed() < cap => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let out = rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
    Some((status.success(), out))
}

/// ENG-09: the container must see the project at the same path, read-write, or dsh would edit a
/// different tree (T-06-90). Runs before any run row exists.
pub fn preflight(def: &DshDef, cwd: &Path) -> BrainResult<()> {
    preflight_env(def, cwd, &scrubbed_current_env())
}

fn preflight_env(def: &DshDef, cwd: &Path, env: &BTreeMap<String, String>) -> BrainResult<()> {
    let mut args: Vec<String> = INSPECT_MOUNTS_ARGS.iter().map(|s| s.to_string()).collect();
    args.push(def.container.clone());
    match docker_capture(&def.docker, &args, env, DOCKER_CALL_CAP) {
        Some((true, out)) => {
            if same_path_bind_covers(&out, cwd) {
                Ok(())
            } else {
                Err(BrainError::error(format!(
                    "Thư mục {} không nằm trong thư mục mà container {} thấy cùng đường dẫn — engine dsh không chạy được trên dự án này.",
                    cwd.display(),
                    def.container
                )))
            }
        }
        _ => Err(BrainError::error(format!(
            "Không đọc được danh sách mount của container {} — container chưa chạy hoặc lệnh docker lỗi; engine dsh không chạy được.",
            def.container
        ))),
    }
}

/// One stdout line of dsh as the event stored in `events.raw_json`.
pub fn text_event(line: &str) -> String {
    serde_json::json!({"dsh": "text", "text": line}).to_string()
}

/// The run plan of a dsh engine: Dev only (D-37), refused before any run row on a project the
/// container cannot see at the same path.
pub fn plan(
    cfg: &Config,
    id: &str,
    def: &DshDef,
    role: &str,
    claude_argv: &[String],
    base_env: BTreeMap<String, String>,
    cwd: &Path,
) -> BrainResult<EnginePlan> {
    if role != "dev" {
        return Err(BrainError::error(non_dev_message(id)));
    }

    // The docker client sees the project profile's env and nothing that picks an account or
    // carries a key (T-06-45 / D-32).
    let mut env = base_env;
    let keys = cfg.engines.all_key_envs();
    env.retain(|k, _| {
        !(k.starts_with("ANTHROPIC_")
            || STRIPPED_EXACT.contains(&k.as_str())
            || keys.iter().any(|name| name == k))
    });

    preflight_env(def, cwd, &env)?;
    warn_leftover(def, &env);

    let (input, prompt) = crate::engine::claude_argv_parts(claude_argv)
        .ok_or_else(|| BrainError::error("Không dựng được lệnh dsh từ lệnh claude"))?;
    let text = format!("{prompt}\n\n---\n{input}");
    let marker = new_marker();
    let argv = build_argv(def, cwd, &marker, &text);

    Ok(EnginePlan {
        engine: Engine::Named(id.to_string()),
        db_engine: id.to_string(),
        display: id.to_string(),
        argv,
        env,
        model_label: def.dsh_profile.clone(),
        stream: StreamFormat::Text,
        stdout_file: false,
        redact: Vec::new(),
        response_file: None,
        native: false,
        unverified: false,
        dsh: Some(DshRun {
            docker: def.docker.clone(),
            container: def.container.clone(),
            marker,
        }),
    })
}

/// One row of the container's process list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcRow {
    pub pid: i32,
    pub ppid: i32,
    pub pgid: i32,
    pub args: String,
}

/// Parses `ps -ww -eo pid=,ppid=,pgid=,args=`: three numbers (leading spaces allowed) and the
/// rest of the line as the arguments. Blank and malformed lines are skipped.
pub fn parse_ps(snapshot: &str) -> Vec<ProcRow> {
    snapshot
        .lines()
        .filter_map(|line| {
            let mut rest = line.trim_start();
            let mut nums = [0i32; 3];
            for n in nums.iter_mut() {
                let end = rest.find(char::is_whitespace)?;
                *n = rest[..end].parse().ok()?;
                rest = rest[end..].trim_start();
            }
            Some(ProcRow {
                pid: nums[0],
                ppid: nums[1],
                pgid: nums[2],
                args: rest.trim_end().to_string(),
            })
        })
        .collect()
}

fn args_tokens(args: &str) -> impl Iterator<Item = &str> {
    args.split(|c: char| c.is_whitespace() || c == ';')
        .filter(|t| !t.is_empty())
}

/// A zombie keeps its row until its parent reaps it; it can neither be signalled nor matter.
fn is_defunct(row: &ProcRow) -> bool {
    row.args.ends_with("<defunct>")
}

/// The first 100 characters of the arguments: what a re-match compares (S6 `tree`).
fn args_head(args: &str) -> String {
    args.chars().take(100).collect()
}

/// The processes of the run that carries `marker`: the rows with a token EQUAL to the marker,
/// their descendants by parent pid, and every `before` row that is still present with the same
/// pid and the same first 100 characters of arguments (orphans re-parented by a kill). A pid that
/// now runs something else is not selected (PID reuse). Sorted by pid; pid 1 is never included.
pub fn marker_subtree(rows: &[ProcRow], marker: &str, before: &[ProcRow]) -> Vec<ProcRow> {
    let live: Vec<&ProcRow> = rows.iter().filter(|r| !is_defunct(r)).collect();
    let mut selected: std::collections::BTreeSet<i32> = live
        .iter()
        .filter(|r| args_tokens(&r.args).any(|t| t == marker))
        .map(|r| r.pid)
        .collect();
    loop {
        let mut grew = false;
        for r in &live {
            if selected.contains(&r.ppid) && selected.insert(r.pid) {
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    for b in before {
        if let Some(r) = live.iter().find(|r| r.pid == b.pid) {
            if args_head(&r.args) == args_head(&b.args) {
                selected.insert(r.pid);
            }
        }
    }
    let mut out: Vec<ProcRow> = live
        .into_iter()
        .filter(|r| r.pid > 1 && selected.contains(&r.pid))
        .cloned()
        .collect();
    out.sort_by_key(|r| r.pid);
    out.dedup_by_key(|r| r.pid);
    out
}

fn ps_args(container: &str) -> Vec<String> {
    [
        "exec",
        container,
        "ps",
        "-ww",
        "-eo",
        "pid=,ppid=,pgid=,args=",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn ps_snapshot_env(run: &DshRun, env: &BTreeMap<String, String>) -> Option<String> {
    match docker_capture(&run.docker, &ps_args(&run.container), env, DOCKER_CALL_CAP) {
        Some((true, out)) => Some(out),
        _ => None,
    }
}

/// The container's process list, or `None` when the container cannot be reached.
pub fn ps_snapshot(run: &DshRun) -> Option<String> {
    ps_snapshot_env(run, &scrubbed_current_env())
}

/// Sends `sig` (`-TERM` / `-KILL`) to exactly these pids inside the container.
fn signal_pids(run: &DshRun, env: &BTreeMap<String, String>, sig: &str, pids: &[i32]) {
    if pids.is_empty() {
        return;
    }
    let mut args = vec![
        "exec".to_string(),
        run.container.clone(),
        "kill".to_string(),
        sig.to_string(),
    ];
    args.extend(pids.iter().map(|p| p.to_string()));
    let _ = docker_capture(&run.docker, &args, env, DOCKER_CALL_CAP);
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StopReport {
    /// Pids recorded from the marker's subtree and sent TERM.
    pub signalled: Vec<i32>,
    /// Pids still carrying the run after TERM, the grace period and KILL.
    pub survivors: Vec<i32>,
}

/// ENG-09 / D-35: stops one dsh run INSIDE the container. A host-side kill of the docker client
/// leaves the container process running (S6 4a), so this always runs first: snapshot, record the
/// marker's subtree, `kill -TERM` those pids, poll up to `grace` re-matching every pid by pid and
/// arguments, `kill -KILL` what re-matched, final snapshot. Nothing is signalled by name and the
/// container itself is never touched. An unreachable container or an invalid marker signals
/// nothing.
pub fn stop_in_container(run: &DshRun, grace: Duration) -> StopReport {
    let mut report = StopReport::default();
    if !is_valid_marker(&run.marker) {
        return report;
    }
    let env = scrubbed_current_env();
    let Some(first) = ps_snapshot_env(run, &env) else {
        return report;
    };
    let recorded = marker_subtree(&parse_ps(&first), &run.marker, &[]);
    if recorded.is_empty() {
        return report;
    }
    report.signalled = recorded.iter().map(|r| r.pid).collect();
    signal_pids(run, &env, "-TERM", &report.signalled);

    let start = Instant::now();
    let poll = Duration::from_millis(200);
    // `None` while the container cannot be listed: then nothing is re-matched, so nothing more
    // is signalled.
    let mut alive: Option<Vec<ProcRow>>;
    loop {
        std::thread::sleep(poll.min(grace.max(Duration::from_millis(1))));
        alive = ps_snapshot_env(run, &env)
            .map(|s| marker_subtree(&parse_ps(&s), &run.marker, &recorded));
        if alive.as_ref().is_some_and(|a| a.is_empty()) || start.elapsed() >= grace {
            break;
        }
    }

    match alive {
        Some(a) if a.is_empty() => {}
        Some(a) => {
            let pids: Vec<i32> = a.iter().map(|r| r.pid).collect();
            signal_pids(run, &env, "-KILL", &pids);
            std::thread::sleep(Duration::from_millis(300));
            report.survivors = match ps_snapshot_env(run, &env) {
                Some(s) => marker_subtree(&parse_ps(&s), &run.marker, &recorded)
                    .iter()
                    .map(|r| r.pid)
                    .collect(),
                None => pids,
            };
        }
        None => report.survivors = report.signalled.clone(),
    }
    report
}

/// A process of an earlier run (a marker token on its command line) is still in the container:
/// it is reported and never signalled (a residual after SIGKILL of agent-brain).
fn warn_leftover(def: &DshDef, env: &BTreeMap<String, String>) {
    let run = DshRun {
        docker: def.docker.clone(),
        container: def.container.clone(),
        marker: String::new(),
    };
    let Some(snapshot) = ps_snapshot_env(&run, env) else {
        return;
    };
    let pids: Vec<String> = parse_ps(&snapshot)
        .iter()
        .filter(|r| !is_defunct(r) && args_tokens(&r.args).any(|t| t.starts_with(MARKER_PREFIX)))
        .map(|r| r.pid.to_string())
        .collect();
    if !pids.is_empty() {
        crate::term::emit(&format!(
            "Cảnh báo: container {} còn tiến trình của run dsh trước (pid {}) — agent-brain không tự dừng chúng.",
            def.container,
            pids.join(", ")
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cfg(extra: serde_json::Value) -> Config {
        let mut v: serde_json::Value =
            serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap();
        if let Some(o) = extra.as_object() {
            for (k, val) in o {
                v.as_object_mut().unwrap().insert(k.clone(), val.clone());
            }
        }
        crate::config::config_from_value(&v).unwrap()
    }

    #[test]
    fn wrapper_exact() {
        assert_eq!(
            wrapper_script("ab-dsh-0123456789ab"),
            ": ab-dsh-0123456789ab; dsh --profile \"$1\" \"$2\"; rc=$?; exit $rc"
        );

        let a = new_marker();
        let b = new_marker();
        assert!(is_valid_marker(&a), "{a}");
        assert!(is_valid_marker(&b), "{b}");
        assert_ne!(a, b);
        assert!(!is_valid_marker("ab-dsh-0123456789abc"));
        assert!(!is_valid_marker("ab-dsh-0123456789AB"));
        assert!(!is_valid_marker(""));

        let def = DshDef {
            docker: "docker".to_string(),
            container: "dsh-x".to_string(),
            dsh_profile: "headless".to_string(),
        };
        let prompt = "làm việc; rm -rf / $(x) \"q\" '--'\n\n---\nviệc";
        let argv = build_argv(
            &def,
            Path::new("/mnt/data/p"),
            "ab-dsh-0123456789ab",
            prompt,
        );
        assert_eq!(argv.last().unwrap(), prompt);
        assert_eq!(argv.len(), 11);
        assert_eq!(
            &argv[..10],
            [
                "docker",
                "exec",
                "-w",
                "/mnt/data/p",
                "dsh-x",
                "sh",
                "-c",
                wrapper_script("ab-dsh-0123456789ab").as_str(),
                "sh",
                "headless"
            ]
        );
        // the prompt is a positional argument, never part of the shell text
        assert!(!argv[7].contains("rm -rf"));
        assert!(!argv[7].contains(prompt));
    }

    #[test]
    fn mounts_rules() {
        let bind = |src: &str, dst: &str, rw: bool| {
            format!(
                r#"[{{"Type":"bind","Source":"{src}","Destination":"{dst}","Mode":"","RW":{rw},"Propagation":"rprivate"}}]"#
            )
        };
        let cwd = Path::new("/mnt/data/IDE/x");
        assert!(same_path_bind_covers(
            &bind("/mnt/data", "/mnt/data", true),
            cwd
        ));
        assert!(same_path_bind_covers(&bind("/", "/", true), cwd));
        // path-component prefix, not a string prefix
        assert!(!same_path_bind_covers(
            &bind("/mnt/data", "/mnt/data", true),
            Path::new("/mnt/database/x")
        ));
        // source differs from destination
        assert!(!same_path_bind_covers(
            &bind("/srv/data", "/mnt/data", true),
            cwd
        ));
        // read-only
        assert!(!same_path_bind_covers(
            &bind("/mnt/data", "/mnt/data", false),
            cwd
        ));
        // a volume, not a bind
        assert!(!same_path_bind_covers(
            r#"[{"Type":"volume","Source":"/mnt/data","Destination":"/mnt/data","RW":true}]"#,
            cwd
        ));
        // one covering mount among several
        let many = r#"[{"Type":"bind","Source":"/etc","Destination":"/etc","RW":true},
            {"Type":"bind","Source":"/mnt/data","Destination":"/mnt/data","RW":true}]"#;
        assert!(same_path_bind_covers(many, cwd));
        // malformed
        assert!(!same_path_bind_covers("not json", cwd));
        assert!(!same_path_bind_covers("null", cwd));
        assert!(!same_path_bind_covers("{}", cwd));
        assert!(!same_path_bind_covers("", cwd));
    }

    #[test]
    fn plan_refuses_non_dev() {
        // The docker command does not exist: a refusal must come before any docker call.
        let cfg = test_cfg(serde_json::json!({"engines": [
            {"id": "dsh", "kind": "dsh", "docker": "/khong/co/docker-test", "container": "dsh-x"},
        ]}));
        let def = cfg.engines.get("dsh").unwrap().dsh().unwrap().clone();
        for role in ["lead", "qa", "plan-review", "code-review"] {
            let e = plan(
                &cfg,
                "dsh",
                &def,
                role,
                &[],
                BTreeMap::new(),
                Path::new("/tmp"),
            )
            .unwrap_err();
            assert_eq!(
                e.message,
                "Engine 'dsh' là kiểu dsh, chỉ dùng cho vai trò dev."
            );
        }
    }

    fn row(pid: i32, ppid: i32, args: &str) -> ProcRow {
        ProcRow {
            pid,
            ppid,
            pgid: pid,
            args: args.to_string(),
        }
    }

    #[test]
    fn parse_ps_rows() {
        let snap = "    1     0     1 /sbin/init\n  4242  4200  4242 sh -c : ab-dsh-0123456789ab; dsh --profile \"$1\" \"$2\"; rc=$?; exit $rc sh headless a   very  long    prompt\n\n   77\t1\t77\tsleep 30\nnot a row\n  12 34\n";
        let rows = parse_ps(snap);
        assert_eq!(rows.len(), 3, "{rows:?}");
        assert_eq!(rows[0], row(1, 0, "/sbin/init"));
        assert_eq!(
            (rows[1].pid, rows[1].ppid, rows[1].pgid),
            (4242, 4200, 4242)
        );
        assert!(rows[1].args.starts_with("sh -c : ab-dsh-0123456789ab;"));
        assert!(rows[1].args.ends_with("a   very  long    prompt"));
        assert_eq!(rows[2].args, "sleep 30");
        assert!(parse_ps("").is_empty());
    }

    #[test]
    fn marker_subtree_rules() {
        let m = "ab-dsh-0123456789ab";
        let sh = format!("sh -c : {m}; dsh --profile \"$1\" \"$2\"; rc=$?; exit $rc sh p x");
        let rows = vec![
            row(1, 0, "/sbin/init"),
            row(100, 1, &sh),
            row(101, 100, "dsh --profile headless x"),
            row(102, 101, "sleep 120"),
            row(103, 102, "sleep 121"),
            row(200, 1, "sleep 999"),
            // a different marker that merely starts with ours
            row(300, 1, "sh -c : ab-dsh-0123456789abcd; sleep 5"),
            row(301, 300, "sleep 5"),
        ];
        let sel = marker_subtree(&rows, m, &[]);
        let pids = |v: &[ProcRow]| v.iter().map(|r| r.pid).collect::<Vec<_>>();
        assert_eq!(pids(&sel), [100, 101, 102, 103]);

        // an orphan re-parented to init stays selected through `before`
        let after = vec![
            row(1, 0, "/sbin/init"),
            row(102, 1, "sleep 120"),
            row(200, 1, "sleep 999"),
        ];
        assert_eq!(pids(&marker_subtree(&after, m, &sel)), [102]);

        // the same pid now running something else is NOT selected (PID reuse)
        let reused = vec![row(1, 0, "/sbin/init"), row(102, 1, "tail -f /var/log/x")];
        assert!(marker_subtree(&reused, m, &sel).is_empty());

        // a zombie is never selected
        let zombie = vec![row(100, 1, &sh), row(101, 100, "[dsh] <defunct>")];
        assert_eq!(pids(&marker_subtree(&zombie, m, &[])), [100]);

        // pid 1 is never selected, even when a `before` row names it
        let init = vec![row(1, 0, "/sbin/init")];
        assert!(marker_subtree(&init, m, &init).is_empty());

        // the marker must be a whole token: inside another word it does not count
        let embedded = vec![row(400, 1, &format!("echo x{m}y"))];
        assert!(marker_subtree(&embedded, m, &[]).is_empty());
    }

    #[test]
    fn stop_ignores_invalid_marker() {
        // An empty or malformed marker would match unrelated processes: nothing is even run.
        for marker in ["", "ab-dsh-", "ab-dsh-XYZ", "sleep"] {
            let run = DshRun {
                docker: "/khong/co/docker-test".to_string(),
                container: "dsh-x".to_string(),
                marker: marker.to_string(),
            };
            assert_eq!(
                stop_in_container(&run, Duration::from_millis(10)),
                StopReport::default()
            );
        }
    }

    #[test]
    fn text_event_is_json_text() {
        let v: serde_json::Value = serde_json::from_str(&text_event("a \"b\"\tc")).unwrap();
        assert_eq!(v["dsh"], "text");
        assert_eq!(v["text"], "a \"b\"\tc");
    }
}
