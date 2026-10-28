//! Phase 6 plan 23: `dsh` (the DeepSeek Harness container) as a black-box Dev engine through
//! `docker exec`. Every test runs against the fake docker (tests/fakes/fake_docker.py) and the
//! fake dsh (tests/fakes/fake_dsh.py); the real docker CLI and the real container are never
//! touched. Cleanup is by recorded PID only.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{RunOut, Sandbox};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fake_path(name: &str) -> PathBuf {
    common::repo_root()
        .join("tests")
        .join("fakes")
        .join(name)
        .canonicalize()
        .unwrap_or_else(|e| panic!("canonicalize {name}: {e}"))
}

fn dsh_entry() -> Value {
    json!({
        "id": "dsh",
        "kind": "dsh",
        "docker": fake_path("fake_docker.py").to_str().unwrap(),
        "container": "fake-dsh",
        "dsh_profile": "headless",
        "allowed_profiles": ["personal"]
    })
}

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

/// Environment of one fake docker + fake dsh call: a bin dir whose `dsh` is the fake, the
/// scenario and the two record files.
struct Dsh {
    bin: PathBuf,
    scenario: PathBuf,
    docker_record: PathBuf,
    dsh_record: PathBuf,
}

impl Dsh {
    fn new(sb: &Sandbox, scenario: Value) -> Dsh {
        let bin = sb.root.join("dsh-bin");
        std::fs::create_dir_all(&bin).unwrap();
        let link = bin.join("dsh");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(fake_path("fake_dsh.py"), &link).unwrap();
        let d = Dsh {
            bin,
            scenario: sb.root.join("fake_dsh_scenario.json"),
            docker_record: sb.root.join("fake_docker_record.jsonl"),
            dsh_record: sb.root.join("fake_dsh_record.jsonl"),
        };
        d.set_scenario(&scenario);
        let _ = std::fs::remove_file(&d.docker_record);
        let _ = std::fs::remove_file(&d.dsh_record);
        d
    }

    fn set_scenario(&self, scenario: &Value) {
        std::fs::write(&self.scenario, serde_json::to_string(scenario).unwrap()).unwrap();
    }

    fn vars(&self, extra: &[(&str, &str)]) -> Vec<(String, String)> {
        let path = format!(
            "{}:{}",
            self.bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut v = vec![
            ("PATH".to_string(), path),
            (
                "FAKE_DSH_SCENARIO".to_string(),
                self.scenario.to_str().unwrap().to_string(),
            ),
            (
                "FAKE_DSH_RECORD".to_string(),
                self.dsh_record.to_str().unwrap().to_string(),
            ),
            (
                "FAKE_DOCKER_RECORD".to_string(),
                self.docker_record.to_str().unwrap().to_string(),
            ),
        ];
        for (k, val) in extra {
            v.push((k.to_string(), val.to_string()));
        }
        v
    }

    fn docker_records(&self) -> Vec<Value> {
        read_jsonl(&self.docker_record)
    }

    fn dsh_records(&self) -> Vec<Value> {
        read_jsonl(&self.dsh_record)
    }

    /// Records of the fake docker whose first argv word is `sub`.
    fn docker_calls(&self, sub: &str) -> Vec<Value> {
        self.docker_records()
            .into_iter()
            .filter(|r| r["argv"][0] == json!(sub))
            .collect()
    }
}

fn refs(v: &[(String, String)]) -> Vec<(&str, &str)> {
    v.iter()
        .map(|(k, val)| (k.as_str(), val.as_str()))
        .collect()
}

fn read_jsonl(path: &Path) -> Vec<Value> {
    match std::fs::read_to_string(path) {
        Ok(c) => c
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn argv_of(rec: &Value) -> Vec<String> {
    rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

fn env_names_of(rec: &Value) -> Vec<String> {
    rec["env_names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

fn gateway_entry(id: &str, key_env: &str) -> Value {
    json!({
        "id": id,
        "kind": "anthropic",
        "command": Sandbox::fake_claude().to_str().unwrap(),
        "base_url": "https://gateway.invalid/proxyllm",
        "key_env": key_env,
        "models": {"dev": "deepseek/deepseek-chat"},
        "verified": "test"
    })
}

/// A personal project `web-ban-hang` whose default team is `doi-dsh` (dev on dsh); a claude Lead
/// run has written and committed the plan. Returns the repo path.
fn setup(sb: &Sandbox, extra_engines: Vec<Value>, lead_env: &[(String, String)]) -> PathBuf {
    setup_plan(sb, extra_engines, "dsh", &["src/app.py"], lead_env)
}

/// Same as `setup`, with the Dev engine of team `doi-dsh` and the files the plan lists chosen by
/// the caller.
fn setup_plan(
    sb: &Sandbox,
    extra_engines: Vec<Value>,
    dev_engine: &str,
    plan_files: &[&str],
    lead_env: &[(String, String)],
) -> PathBuf {
    let mut engines = vec![dsh_entry()];
    engines.extend(extra_engines);
    sb.write_config(json!({ "engines": engines }));
    sb.write_teams(
        json!({"doi-dsh": {"description": "Dev trên dsh", "roles": {"dev": dev_engine}}}),
    );
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "personal");
    let obj = entry.as_object_mut().unwrap();
    obj.remove("workflow");
    obj.insert("team".to_string(), json!("doi-dsh"));
    sb.write_map(json!([entry]), &[]);
    let rows: Vec<(&str, &str, &str, &str)> = plan_files
        .iter()
        .map(|f| (*f, "toàn file", "sửa", "demo"))
        .collect();
    let res = sb.run_env(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(protocol::lead_scenario(&rows)),
        &refs(lead_env),
    );
    assert_ok(&res, "run lead (claude)");
    repo
}

fn write_app() -> Value {
    json!({"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"})
}

fn run_dev(sb: &Sandbox, env: &[(String, String)]) -> RunOut {
    sb.run_env(&["run", "dev", "--", "web-ban-hang"], None, &refs(env))
}

struct DevRow {
    id: i64,
    engine: String,
    status: String,
    model: Option<String>,
    usage_source: String,
    input: i64,
    output: i64,
    cost: f64,
    metrics: Value,
}

fn dev_row(sb: &Sandbox) -> DevRow {
    sb.db()
        .query_row(
            "SELECT id, engine, status, model, usage_source, input_tokens, output_tokens, \
             cost_usd, metrics_json FROM runs WHERE role = 'dev' ORDER BY id DESC LIMIT 1",
            [],
            |r| {
                let metrics: Option<String> = r.get(8)?;
                Ok(DevRow {
                    id: r.get(0)?,
                    engine: r.get(1)?,
                    status: r.get(2)?,
                    model: r.get(3)?,
                    usage_source: r.get(4)?,
                    input: r.get(5)?,
                    output: r.get(6)?,
                    cost: r.get(7)?,
                    metrics: serde_json::from_str(&metrics.unwrap_or_else(|| "{}".into())).unwrap(),
                })
            },
        )
        .expect("no dev run row")
}

fn dev_run_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM runs WHERE role = 'dev'", [], |r| {
            r.get(0)
        })
        .unwrap()
}

fn is_marker(m: &str) -> bool {
    m.len() == "ab-dsh-".len() + 12
        && m.starts_with("ab-dsh-")
        && m["ab-dsh-".len()..]
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// Pids of live local processes whose command line carries `needle`.
fn procs_with(needle: &str) -> Vec<i32> {
    let me = std::process::id() as i32;
    let mut out = Vec::new();
    for entry in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        if pid == me {
            continue;
        }
        if let Ok(raw) = std::fs::read(format!("/proc/{pid}/cmdline")) {
            if String::from_utf8_lossy(&raw).contains(needle) {
                out.push(pid);
            }
        }
    }
    out
}

fn read_pids(pidfile: &Path, timeout: Duration) -> Option<(i32, i32)> {
    let mut pids = None;
    common::wait_until(timeout, || {
        if let Ok(c) = std::fs::read_to_string(pidfile) {
            let p: Vec<i32> = c
                .split_whitespace()
                .filter_map(|x| x.parse().ok())
                .collect();
            if p.len() == 2 {
                pids = Some((p[0], p[1]));
                return true;
            }
        }
        false
    });
    pids
}

/// Waits for the dev run to be `running` with its marker persisted, returns the marker.
fn wait_marker(sb: &Sandbox) -> String {
    let mut marker = String::new();
    common::wait_until(Duration::from_secs(15), || {
        let found: Option<String> = sb
            .db()
            .query_row(
                "SELECT json_extract(metrics_json, '$.dsh.marker') FROM runs \
                 WHERE role = 'dev' AND status = 'running' ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(None);
        match found {
            Some(m) => {
                marker = m;
                true
            }
            None => false,
        }
    });
    assert!(is_marker(&marker), "no valid marker persisted: {marker:?}");
    marker
}

fn assert_gone(marker: &str, pids: (i32, i32)) {
    let gone = common::wait_until(Duration::from_secs(10), || {
        procs_with(marker).is_empty()
            && agent_brain::procs::proc_id(pids.0).is_none()
            && agent_brain::procs::proc_id(pids.1).is_none()
    });
    assert!(
        gone,
        "marker processes {:?}; pid {} alive {}; pid {} alive {}",
        procs_with(marker),
        pids.0,
        agent_brain::procs::proc_id(pids.0).is_some(),
        pids.1,
        agent_brain::procs::proc_id(pids.1).is_some()
    );
}

/// Kills (by recorded pid) what a failed kill test left behind, so a red test does not leak.
fn guard_pids(pids: (i32, i32)) -> common::ProcGuard {
    let mut v = Vec::new();
    for p in [pids.0, pids.1] {
        if let Some(id) = agent_brain::procs::proc_id(p) {
            v.push(id);
        }
    }
    common::ProcGuard(v)
}

fn wrapper(marker: &str) -> String {
    format!(": {marker}; dsh --profile \"$1\" \"$2\"; rc=$?; exit $rc")
}

// ---------------------------------------------------------------------------
// Task 1 (tracer): a Dev run through the fake docker
// ---------------------------------------------------------------------------

#[test]
fn dsh_dev_tracer() {
    let sb = Sandbox::new();
    let dsh = Dsh::new(
        &sb,
        json!({"ops": [write_app()], "lines": ["đọc plan", "sửa src/app.py"]}),
    );
    let repo = setup(&sb, vec![], &[]);

    let res = run_dev(&sb, &dsh.vars(&[]));
    assert_ok(&res, "run dev (dsh)");

    let row = dev_row(&sb);
    assert_eq!(row.engine, "dsh");
    assert_eq!(row.status, "succeeded");
    assert_eq!(row.usage_source, "none");
    assert_eq!((row.input, row.output), (0, 0));
    assert_eq!(row.cost, 0.0);
    assert_eq!(row.model.as_deref(), Some("headless"));
    assert!(
        res.stderr.contains("· engine dsh · model headless"),
        "stderr: {}",
        res.stderr
    );
    assert!(
        !res.stderr.contains("dòng JSON lỗi"),
        "stderr: {}",
        res.stderr
    );

    // each stdout line is one `dsh/text` event with the line as its text
    let db = sb.db();
    let mut stmt = db
        .prepare("SELECT kind, raw_json FROM events WHERE run_id = ?1 ORDER BY seq")
        .unwrap();
    let events: Vec<(String, String)> = stmt
        .query_map([row.id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(events.len(), 2, "{events:?}");
    let texts: Vec<String> = events
        .iter()
        .map(|(k, raw)| {
            assert_eq!(k, "dsh/text");
            let v: Value = serde_json::from_str(raw).unwrap();
            assert_eq!(v["dsh"], json!("text"));
            v["text"].as_str().unwrap().to_string()
        })
        .collect();
    assert_eq!(texts, ["đọc plan", "sửa src/app.py"]);
    // shown as plain assistant text in the terminal
    assert!(res.stderr.contains("đọc plan"), "stderr: {}", res.stderr);

    // stream.jsonl keeps the raw text
    let run_dir: String = db
        .query_row("SELECT run_dir FROM runs WHERE id = ?1", [row.id], |r| {
            r.get(0)
        })
        .unwrap();
    let stream = std::fs::read_to_string(Path::new(&run_dir).join("stream.jsonl")).unwrap();
    assert_eq!(stream, "đọc plan\nsửa src/app.py\n");

    let dsh_meta = &row.metrics["dsh"];
    assert!(
        is_marker(dsh_meta["marker"].as_str().unwrap()),
        "{dsh_meta}"
    );
    assert_eq!(dsh_meta["container"], json!("fake-dsh"));
    assert_eq!(
        dsh_meta["docker"],
        json!(fake_path("fake_docker.py").to_str().unwrap())
    );

    let subject = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert!(subject.starts_with("sua-app: "), "head subject: {subject}");
    let files = sb.git(&repo, &["show", "--name-only", "--format=", "HEAD"]);
    assert!(files.lines().any(|l| l == "src/app.py"), "{files}");
}

#[test]
fn dsh_argv_and_stdin() {
    let sb = Sandbox::new();
    let dsh = Dsh::new(&sb, json!({"ops": [write_app()]}));
    let repo = setup(&sb, vec![], &[]);

    let res = run_dev(&sb, &dsh.vars(&[]));
    assert_ok(&res, "run dev (dsh)");
    let marker = dev_row(&sb).metrics["dsh"]["marker"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(is_marker(&marker));

    // Order of the docker calls: the mount preflight, the leftover check (`exec ... ps`), then
    // the one run exec, which is the only `exec` with `-w`.
    let subs: Vec<String> = dsh
        .docker_records()
        .iter()
        .map(|r| r["argv"][0].as_str().unwrap().to_string())
        .collect();
    assert_eq!(subs, ["inspect", "exec", "exec"], "{subs:?}");
    let execs = dsh.docker_calls("exec");
    assert_eq!(argv_of(&execs[0])[2], "ps");
    let first = argv_of(&execs[1]);
    let root = repo.to_str().unwrap();
    assert_eq!(first.len(), 10, "{first:?}");
    assert_eq!(
        &first[..9],
        [
            "exec",
            "-w",
            root,
            "fake-dsh",
            "sh",
            "-c",
            wrapper(&marker).as_str(),
            "sh",
            "headless"
        ]
    );
    let prompt = &first[9];
    assert!(prompt.contains("\n\n---\n"), "prompt: {prompt}");
    // the prompt travels as a positional argument, never inside the shell text
    assert!(!first[6].contains(prompt.as_str()));
    assert!(!first[6].contains("---"));

    let recs = dsh.dsh_records();
    assert_eq!(recs.len(), 1);
    assert_eq!(
        argv_of(&recs[0]),
        ["--profile", "headless", prompt.as_str()]
    );
    assert_eq!(recs[0]["stdin_is_null"], json!(true));
    assert_eq!(recs[0]["cwd"], json!(root));
}

#[test]
fn dsh_env_has_no_keys() {
    let sb = Sandbox::new();
    let dsh = Dsh::new(&sb, json!({"ops": [write_app()]}));
    setup(
        &sb,
        vec![
            gateway_entry("gw", "GW_KEY"),
            json!({
                "id": "deepseek", "kind": "anthropic",
                "command": Sandbox::fake_claude().to_str().unwrap(),
                "base_url": "https://api.deepseek.invalid/anthropic",
                "key_env": "DEEPSEEK_API_KEY",
                "models": {"dev": "sonnet"}
            }),
        ],
        &[],
    );

    let env = dsh.vars(&[
        ("DEEPSEEK_API_KEY", "test-planted-ds-key-0123456789"),
        ("GW_KEY", "test-planted-gw-key-0123456789"),
        ("ANTHROPIC_MODEL", "some-model"),
    ]);
    let res = run_dev(&sb, &env);
    assert_ok(&res, "run dev (dsh)");

    let mut all = dsh.docker_records();
    all.extend(dsh.dsh_records());
    assert!(!all.is_empty());
    for rec in &all {
        for name in env_names_of(rec) {
            assert!(
                !(name == "DEEPSEEK_API_KEY"
                    || name == "GW_KEY"
                    || name.starts_with("ANTHROPIC_")
                    || name == "CLAUDE_CONFIG_DIR"
                    || name == "CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR"
                    || name == "CLAUDE_CODE_SUBAGENT_MODEL"),
                "{name} reached {rec}"
            );
        }
    }
}

#[test]
fn dsh_mount_preflight() {
    let sb = Sandbox::new();
    let dsh = Dsh::new(&sb, json!({"ops": [write_app()]}));
    setup(&sb, vec![], &[]);

    let mounts = json!([{
        "Type": "bind", "Source": "/nonexistent-mount", "Destination": "/nonexistent-mount",
        "Mode": "", "RW": true, "Propagation": "rprivate"
    }])
    .to_string();
    let res = run_dev(&sb, &dsh.vars(&[("FAKE_DOCKER_MOUNTS", mounts.as_str())]));
    assert_eq!(
        res.code, 1,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        res.stderr
            .contains("không nằm trong thư mục mà container fake-dsh thấy cùng đường dẫn"),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(dev_run_count(&sb), 0, "no run row for a refused step");
    assert!(dsh.docker_calls("exec").is_empty(), "nothing was executed");
    let inspects = dsh.docker_calls("inspect");
    assert_eq!(inspects.len(), 1);
    assert_eq!(
        argv_of(&inspects[0]),
        ["inspect", "--format", "{{json .Mounts}}", "fake-dsh"]
    );

    // a same-path read-write bind of the parent directory passes
    let ok_mounts = json!([{
        "Type": "bind", "Source": "/", "Destination": "/",
        "Mode": "", "RW": true, "Propagation": "rprivate"
    }])
    .to_string();
    let res = run_dev(
        &sb,
        &dsh.vars(&[("FAKE_DOCKER_MOUNTS", ok_mounts.as_str())]),
    );
    assert_ok(&res, "run dev (dsh, same-path bind)");
}

#[test]
fn fake_docker_ps_status() {
    let sb = Sandbox::new();
    let rec = sb.root.join("ps_record.jsonl");
    let run = |status: Option<&str>, args: &[&str]| -> (i32, String) {
        let mut cmd = std::process::Command::new(fake_path("fake_docker.py"));
        cmd.args(args);
        cmd.env_clear();
        if let Ok(p) = std::env::var("PATH") {
            cmd.env("PATH", p);
        }
        cmd.env("FAKE_DOCKER_RECORD", &rec);
        if let Some(s) = status {
            cmd.env("FAKE_DOCKER_STATUS", s);
        }
        cmd.stdin(std::process::Stdio::null());
        let out = cmd.output().unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
        )
    };
    let probe = [
        "ps",
        "--filter",
        "name=^fake-dsh$",
        "--format",
        "{{.Status}}",
    ];

    assert_eq!(run(None, &probe), (0, "Up 1 minute\n".to_string()));
    assert_eq!(
        run(Some("Exited (0) 2 minutes ago"), &probe),
        (0, "Exited (0) 2 minutes ago\n".to_string())
    );
    assert_eq!(run(Some(""), &probe), (0, String::new()));
    // either option order
    assert_eq!(
        run(
            None,
            &[
                "ps",
                "--format",
                "{{.Status}}",
                "--filter",
                "name=^fake-dsh$"
            ]
        ),
        (0, "Up 1 minute\n".to_string())
    );
    // the fake emulates only the probe shape
    let (code, _) = run(None, &["ps", "--filter", "name=^fake-dsh$"]);
    assert_eq!(code, 2);

    let recs = read_jsonl(&rec);
    assert_eq!(recs.len(), 5);
    for r in &recs {
        assert_eq!(r["argv"][0], json!("ps"));
    }
}

// ---------------------------------------------------------------------------
// Task 2: stopping a run stops it inside the container
// ---------------------------------------------------------------------------

/// `run dev` as a child process with its output in files, so it does not interleave with the
/// test harness output.
fn spawn_dev(sb: &Sandbox, env: &[(String, String)]) -> std::process::Child {
    let _ = std::fs::remove_file(&sb.scenario_file);
    let out = std::fs::File::create(sb.root.join("dev.stdout.txt")).unwrap();
    let err = std::fs::File::create(sb.root.join("dev.stderr.txt")).unwrap();
    let mut cmd = sb.command(&["run", "dev", "--", "web-ban-hang"], &refs(env));
    cmd.stdout(out).stderr(err);
    cmd.spawn().expect("spawn agent-brain")
}

fn sleeping_scenario(pidfile: &Path) -> Value {
    json!({
        "lines": ["bắt đầu"],
        "sleep_s": 30,
        "spawn_child": true,
        "pidfile": pidfile.to_str().unwrap(),
    })
}

#[test]
fn dsh_kill_inside_container() {
    let sb = Sandbox::new();
    let pidfile = sb.root.join("dsh.pid");
    let dsh = Dsh::new(&sb, sleeping_scenario(&pidfile));
    setup(&sb, vec![], &[]);

    let mut child = spawn_dev(&sb, &dsh.vars(&[]));
    let pids = read_pids(&pidfile, Duration::from_secs(15)).expect("fake dsh wrote no pidfile");
    let _guard = guard_pids(pids);
    let marker = wait_marker(&sb);
    assert!(!procs_with(&marker).is_empty(), "no container-side process");

    let task_id: i64 = sb
        .db()
        .query_row("SELECT id FROM tasks ORDER BY id DESC LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    let kill = sb.run(&["kill", &task_id.to_string()], None);
    assert_eq!(
        kill.code, 0,
        "kill failed: {}\n{}",
        kill.stderr, kill.stdout
    );
    assert!(
        kill.stderr.contains("fake-dsh"),
        "the kill reports the container: {}",
        kill.stderr
    );
    let _ = child.wait();

    assert_eq!(dev_row(&sb).status, "killed");
    assert_gone(&marker, pids);
}

#[test]
fn dsh_timeout_kills_inside_container() {
    let sb = Sandbox::new();
    let pidfile = sb.root.join("dsh.pid");
    let dsh = Dsh::new(&sb, sleeping_scenario(&pidfile));
    setup(&sb, vec![], &[]);
    // the full map: a partial `timeout_seconds` would drop the other roles' values
    let mut cfg: Value =
        serde_json::from_str(&std::fs::read_to_string(&sb.config_path).unwrap()).unwrap();
    cfg["timeout_seconds"] = json!({
        "lead": 30, "dev": 3, "qa": 30, "inspect": 30, "plan-review": 30, "code-review": 30
    });
    std::fs::write(&sb.config_path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();

    let res = run_dev(&sb, &dsh.vars(&[]));
    assert_ne!(res.code, 0, "stderr: {}", res.stderr);
    let pids = read_pids(&pidfile, Duration::from_secs(5)).expect("fake dsh wrote no pidfile");
    let _guard = guard_pids(pids);
    let row = dev_row(&sb);
    assert_eq!(row.status, "timed_out");
    let marker = row.metrics["dsh"]["marker"].as_str().unwrap().to_string();
    assert!(is_marker(&marker));
    assert_gone(&marker, pids);
}

#[test]
fn dsh_sigint_kills_inside_container() {
    let sb = Sandbox::new();
    let pidfile = sb.root.join("dsh.pid");
    let dsh = Dsh::new(&sb, sleeping_scenario(&pidfile));
    setup(&sb, vec![], &[]);

    let mut child = spawn_dev(&sb, &dsh.vars(&[]));
    let pids = read_pids(&pidfile, Duration::from_secs(15)).expect("fake dsh wrote no pidfile");
    let _guard = guard_pids(pids);
    let marker = wait_marker(&sb);

    unsafe {
        libc::kill(child.id() as i32, libc::SIGINT);
    }
    let _ = child.wait();

    assert_eq!(dev_row(&sb).status, "killed");
    assert_gone(&marker, pids);
}

#[test]
fn dsh_leftover_warns() {
    use std::os::unix::process::CommandExt;

    let sb = Sandbox::new();
    let dsh = Dsh::new(&sb, json!({"ops": [write_app()]}));
    setup(&sb, vec![], &[]);

    // A process of an earlier run: a marker token on its command line. The trailing command keeps
    // the shell from replacing itself with `sleep`.
    let mut old = std::process::Command::new("sh");
    old.args(["-c", ": ab-dsh-000000000000; sleep 30; exit 0"]);
    old.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    old.process_group(0);
    let mut old = old.spawn().unwrap();
    let old_pid = old.id() as i32;
    let sh_id = agent_brain::procs::proc_id(old_pid).expect("proc_id of the old shell");
    let mut recorded = vec![sh_id];
    let seen = common::wait_until(Duration::from_secs(5), || {
        let d = agent_brain::procs::descendants(old_pid);
        if d.is_empty() {
            false
        } else {
            recorded.extend(d);
            true
        }
    });
    assert!(seen, "the old shell did not start its sleep");
    let _guard = common::ProcGuard(recorded.clone());

    let res = run_dev(&sb, &dsh.vars(&[]));
    assert_ok(&res, "run dev (dsh, leftover)");
    assert!(
        res.stderr
            .contains("Cảnh báo: container fake-dsh còn tiến trình của run dsh trước"),
        "stderr: {}",
        res.stderr
    );
    assert!(
        res.stderr.contains(&old_pid.to_string()),
        "the warning names the pid: {}",
        res.stderr
    );
    // never signalled by agent-brain
    assert!(
        agent_brain::procs::alive(sh_id),
        "the old process was killed"
    );

    // the test ends it by its recorded pids
    for p in &recorded {
        agent_brain::procs::signal_proc(*p, libc::SIGKILL);
    }
    let _ = old.wait();
}

// ---------------------------------------------------------------------------
// Plan 06-25 Task 1 (tracer): dsh Dev diff checks and the `dsh:empty` stop
// ---------------------------------------------------------------------------

const DSH_EMPTY_MESSAGE: &str = "dsh không sửa file nào trong lượt này — có thể container không ghi được vào dự án hoặc dsh chỉ trả lời. Xem log rồi chạy lại hoặc đổi đội hình.";

/// `(status, outcome, error, metrics)` of the newest dev run.
fn dev_outcome(sb: &Sandbox) -> (String, Option<String>, Option<String>, Value) {
    let (status, outcome, error, metrics): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sb
        .db()
        .query_row(
            "SELECT status, outcome, error, metrics_json FROM runs WHERE role = 'dev' \
             ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("no dev run row");
    let metrics = serde_json::from_str(&metrics.unwrap_or_else(|| "{}".to_string())).unwrap();
    (status, outcome, error, metrics)
}

#[test]
fn dsh_dev_outcomes() {
    // (a) dsh prints a line and edits nothing: empty-diff stop, nothing committed.
    {
        let sb = Sandbox::new();
        let dsh = Dsh::new(&sb, json!({"lines": ["chỉ trả lời, không sửa gì"]}));
        let repo = setup(&sb, vec![], &[]);
        let before = sb.git(&repo, &["rev-parse", "HEAD"]);
        let res = run_dev(&sb, &dsh.vars(&[]));
        assert_eq!(
            res.code, 4,
            "stderr: {}\nstdout: {}",
            res.stderr, res.stdout
        );
        assert!(
            res.stdout.contains(DSH_EMPTY_MESSAGE),
            "stdout: {}",
            res.stdout
        );
        let (status, outcome, _, _) = dev_outcome(&sb);
        assert_eq!(status, "succeeded");
        assert_eq!(outcome.as_deref(), Some("dsh:empty"));
        assert_eq!(sb.git(&repo, &["rev-parse", "HEAD"]), before, "no commit");
    }

    // (b) the plan lists `.planning/ghi-chu.md` and dsh writes it: still a protocol violation.
    {
        let sb = Sandbox::new();
        let dsh = Dsh::new(
            &sb,
            json!({"ops": [{"op": "write", "path": ".planning/ghi-chu.md", "content": "x\n"}]}),
        );
        let repo = setup_plan(&sb, vec![], "dsh", &[".planning/ghi-chu.md"], &[]);
        let before = sb.git(&repo, &["rev-parse", "HEAD"]);
        let res = run_dev(&sb, &dsh.vars(&[]));
        assert_eq!(
            res.code, 3,
            "stderr: {}\nstdout: {}",
            res.stderr, res.stdout
        );
        assert!(
            res.stdout.contains(".planning/ghi-chu.md"),
            "stdout: {}",
            res.stdout
        );
        assert_eq!(dev_outcome(&sb).1.as_deref(), Some("protocol_violation"));
        assert_eq!(sb.git(&repo, &["rev-parse", "HEAD"]), before, "no commit");
    }

    // (c) dsh writes a file outside the plan.
    {
        let sb = Sandbox::new();
        let dsh = Dsh::new(
            &sb,
            json!({"ops": [
                write_app(),
                {"op": "write", "path": "src/other.py", "content": "y = 1\n"}
            ]}),
        );
        setup(&sb, vec![], &[]);
        let res = run_dev(&sb, &dsh.vars(&[]));
        assert_eq!(
            res.code, 3,
            "stderr: {}\nstdout: {}",
            res.stderr, res.stdout
        );
        assert!(
            res.stdout.contains("src/other.py"),
            "stdout: {}",
            res.stdout
        );
        assert_eq!(dev_outcome(&sb).1.as_deref(), Some("protocol_violation"));
    }

    // (d) a secret-looking path is a violation even when the plan lists it.
    {
        let sb = Sandbox::new();
        let dsh = Dsh::new(
            &sb,
            json!({"ops": [{"op": "write", "path": ".env", "content": "A=1\n"}]}),
        );
        setup_plan(&sb, vec![], "dsh", &[".env"], &[]);
        let res = run_dev(&sb, &dsh.vars(&[]));
        assert_eq!(
            res.code, 3,
            "stderr: {}\nstdout: {}",
            res.stderr, res.stdout
        );
        assert_eq!(dev_outcome(&sb).1.as_deref(), Some("protocol_violation"));
    }
}

/// An agy-kind entry whose id is not `agy` gets the same empty-diff stop as `agy` (D-35: the
/// check keys on the engine kind, not the id).
#[test]
fn dsh_agy_kind_other_id_gets_agy_empty() {
    let sb = Sandbox::new();
    let fake_agy = fake_path("fake_agy.py");
    let scenario = sb.root.join("fake_agy_scenario.json");
    std::fs::write(&scenario, "{}").unwrap();
    let agy2 = json!({"id": "agy2", "kind": "agy", "command": fake_agy.to_str().unwrap()});
    let repo = setup_plan(&sb, vec![agy2], "agy2", &["src/app.py"], &[]);
    let before = sb.git(&repo, &["rev-parse", "HEAD"]);
    let res = run_dev(
        &sb,
        &[(
            "FAKE_AGY_SCENARIO".to_string(),
            scenario.to_str().unwrap().to_string(),
        )],
    );
    assert_eq!(
        res.code, 4,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        res.stdout
            .contains("agy không sửa file nào — có thể bị từ chối quyền (xem stderr.txt của run)."),
        "stdout: {}",
        res.stdout
    );
    assert_eq!(dev_row(&sb).engine, "agy2");
    assert_eq!(dev_outcome(&sb).1.as_deref(), Some("agy:empty"));
    assert_eq!(sb.git(&repo, &["rev-parse", "HEAD"]), before, "no commit");
}

fn review_report(verdict: &str) -> String {
    format!(
        "---\nverdict: {verdict}\nblocking: 0\nadvisory: 0\nfiles: []\n---\n# Review: sửa app\n## Mục chặn (blocking)\n## Góp ý (advisory)\n"
    )
}

/// Personal project `web-ban-hang` on team `doi-dsh` (Dev on dsh), no plan yet, for `auto`.
fn setup_auto(sb: &Sandbox) {
    sb.write_config(json!({ "engines": [dsh_entry()] }));
    sb.write_teams(json!({"doi-dsh": {"description": "Dev trên dsh", "roles": {"dev": "dsh"}}}));
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "personal");
    let obj = entry.as_object_mut().unwrap();
    obj.remove("workflow");
    obj.insert("team".to_string(), json!("doi-dsh"));
    sb.write_map(json!([entry]), &[]);
}

/// `auto --start` (Lead plans, plan-review approves, the driver waits at the gate) then
/// `approve 1` (the driver runs Dev on dsh). Returns the `approve` result.
fn drive_to_dev(sb: &Sandbox, env: &[(String, String)]) -> RunOut {
    let scenario = json!({"by_role": {
        "lead": [protocol::lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])],
        "plan-review": [{"ops": [{
            "op": "write",
            "path": ".plan-review.md",
            "content": review_report("approve"),
        }]}],
    }});
    let res = sb.run_env(
        &[
            "auto",
            "--start",
            "--name",
            "sua-app",
            "--workflow",
            "mac-dinh",
            "--",
            "web-ban-hang",
            "sửa app",
        ],
        Some(scenario.clone()),
        &refs(env),
    );
    assert_ok(&res, "auto --start");
    sb.run_env(&["approve", "1"], Some(scenario), &refs(env))
}

/// `(auto_state, auto_json)` of task 1.
fn auto_state(sb: &Sandbox) -> (Option<String>, Value) {
    let (state, aj): (Option<String>, Option<String>) = sb
        .db()
        .query_row(
            "SELECT auto_state, auto_json FROM tasks WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    (
        state,
        serde_json::from_str(&aj.unwrap_or_else(|| "{}".into())).unwrap(),
    )
}

/// `(role, outcome)` of every run, oldest first.
fn run_roles(sb: &Sandbox) -> Vec<(String, Option<String>)> {
    let conn = sb.db();
    let mut stmt = conn
        .prepare("SELECT role, outcome FROM runs ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

#[test]
fn dsh_empty_stops_autopilot() {
    let sb = Sandbox::new();
    let dsh = Dsh::new(&sb, json!({"lines": ["chỉ trả lời, không sửa gì"]}));
    setup_auto(&sb);
    let res = drive_to_dev(&sb, &dsh.vars(&[]));
    assert_eq!(
        res.code, 4,
        "driver stop exits 4; stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let (state, aj) = auto_state(&sb);
    assert_eq!(state.as_deref(), Some("stopped"), "aj: {aj}");
    assert_eq!(aj["stop"]["reason"], "vuong_mac", "aj: {aj}");
    assert_eq!(aj["stop"]["role"], "dev");
    assert_eq!(aj["stop"]["detail_line"], json!(DSH_EMPTY_MESSAGE));

    // nothing started after the empty dev run
    let roles = run_roles(&sb);
    assert_eq!(roles.last().unwrap().0, "dev", "{roles:?}");
    assert_eq!(roles.last().unwrap().1.as_deref(), Some("dsh:empty"));
    assert_eq!(
        roles.iter().filter(|r| r.0 == "dev").count(),
        1,
        "{roles:?}"
    );
    assert!(
        roles.iter().all(|r| r.0 != "qa" && r.0 != "code-review"),
        "{roles:?}"
    );
}
