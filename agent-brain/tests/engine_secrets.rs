//! Phase 6 plan 21 (ENG-05, D-32): the key of an engine never reaches a file, a database row, a
//! terminal or a job output, whatever the model echoes and however the run ends. All of it runs
//! against the fake claude with planted values; a failure names files and tables, never a value.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{RunOut, Sandbox};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const GW_KEY: &str = "test-planted-gw-key-0123456789";
const REDACTED: &str = "[đã ẩn]";

/// A text for a failure message: both planted keys masked, so a failing test never prints one.
fn safe(text: &str) -> String {
    text.replace(GW_KEY, "<key>").replace(GW_B_KEY, "<key-b>")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code,
        0,
        "{what} failed: stderr: {}\nstdout: {}",
        safe(&res.stderr),
        safe(&res.stdout)
    );
}

fn fake_cmd() -> String {
    Sandbox::fake_claude().to_str().unwrap().to_string()
}

fn gateway_entry(id: &str, base_url: &str, key_env: &str) -> Value {
    json!({
        "id": id,
        "kind": "anthropic",
        "command": fake_cmd(),
        "base_url": base_url,
        "key_env": key_env,
        "models": {
            "lead": "m-lead",
            "dev": "m-dev",
            "qa": "m-qa"
        },
        "verified": "test"
    })
}

fn app_repo(sb: &Sandbox, name: &str) -> PathBuf {
    sb.make_repo(
        name,
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    )
}

/// Config with one gateway entry `gw`, team `doi-gw` (Dev on `gw`) and project `web-ban-hang`
/// whose default team is `doi-gw`.
fn gw_setup(sb: &Sandbox) -> PathBuf {
    gw_setup_with(sb, json!({}))
}

/// `gw_setup` with more top-level config keys (for example `timeout_seconds`).
fn gw_setup_with(sb: &Sandbox, extra: Value) -> PathBuf {
    let mut cfg = json!({"engines": [
        gateway_entry("gw", "https://gw.invalid/x", "GW_KEY"),
    ]});
    for (k, v) in extra.as_object().unwrap() {
        cfg[k] = v.clone();
    }
    sb.write_config(cfg);
    sb.write_teams(json!({"doi-gw": {
        "description": "Dev qua gateway.",
        "roles": {"dev": "gw"}
    }}));
    let repo = app_repo(sb, "web-ban-hang");
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    let obj = entry.as_object_mut().unwrap();
    obj.remove("workflow");
    obj.insert("team".to_string(), json!("doi-gw"));
    sb.write_map(Value::Array(vec![entry]), &[]);
    repo
}

fn lead_plan_entry() -> Value {
    protocol::lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])
}

fn dev_ops_prefix() -> Vec<Value> {
    vec![
        json!({"op": "read", "path": ".plan.md"}),
        json!({"op": "read", "path": "src/app.py"}),
        json!({"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"}),
    ]
}

fn dev_ops_suffix() -> Vec<Value> {
    vec![
        json!({"op": "git", "args": ["add", "src/app.py"]}),
        json!({"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}),
    ]
}

fn echo_op(var: &str, to: &str) -> Value {
    json!({"op": "echo_env", "var": var, "to": to})
}

fn gw_env() -> Vec<(&'static str, &'static str)> {
    vec![("GW_KEY", GW_KEY)]
}

fn run_lead(sb: &Sandbox, env: &[(&str, &str)]) {
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
        Some(lead_plan_entry()),
        env,
    );
    assert_ok(&res, "run lead");
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

/// Paths (relative to `dir`) of the files under `dir` that hold `needle`. Never the value.
fn file_hits(dir: &Path, needle: &str) -> Vec<String> {
    let mut files = Vec::new();
    files_under(dir, &mut files);
    files.sort();
    files
        .into_iter()
        .filter(|p| {
            std::fs::read(p)
                .map(|b| contains(&b, needle.as_bytes()))
                .unwrap_or(false)
        })
        .map(|p| p.strip_prefix(dir).unwrap_or(&p).display().to_string())
        .collect()
}

/// `table.column` of every database column holding `needle`. Never the value.
fn db_hits(sb: &Sandbox, needle: &str) -> Vec<String> {
    let db = sb.db();
    let tables: Vec<String> = {
        let mut st = db
            .prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            )
            .unwrap();
        let rows = st.query_map([], |r| r.get::<_, String>(0)).unwrap();
        rows.map(|r| r.unwrap()).collect()
    };
    let mut hits = Vec::new();
    for t in tables {
        let cols: Vec<String> = {
            let Ok(mut st) = db.prepare(&format!("PRAGMA table_info(\"{t}\")")) else {
                continue;
            };
            let rows = st.query_map([], |r| r.get::<_, String>(1)).unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        for c in cols {
            let n: i64 = db
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM \"{t}\" WHERE instr(CAST(\"{c}\" AS TEXT), ?1) > 0"
                    ),
                    [needle],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if n > 0 {
                hits.push(format!("{t}.{c}"));
            }
        }
    }
    hits
}

/// Every place a run of the sandbox could have left `needle`: run dirs, database, the output of
/// the agent-brain processes handed in as `outputs`.
fn assert_absent_everywhere(sb: &Sandbox, needle: &str, outputs: &[(&str, &str)]) {
    let files = file_hits(&sb.runs, needle);
    assert!(files.is_empty(), "key found in run dir files: {files:?}");
    let rows = db_hits(sb, needle);
    assert!(rows.is_empty(), "key found in database columns: {rows:?}");
    for (what, text) in outputs {
        assert!(!text.contains(needle), "key found in the {what}");
    }
}

/// The run directory of the newest run with `role`.
fn run_dir_of(sb: &Sandbox, role: &str) -> PathBuf {
    let dir: String = sb
        .db()
        .query_row(
            "SELECT run_dir FROM runs WHERE role = ?1 ORDER BY id DESC LIMIT 1",
            [role],
            |r| r.get(0),
        )
        .unwrap_or_else(|e| panic!("no {role} run row: {e}"));
    PathBuf::from(dir)
}

/// `(status, outcome, error, metrics)` of the newest run with `role`.
fn run_row(sb: &Sandbox, role: &str) -> (String, Option<String>, Option<String>, Value) {
    let (status, outcome, error, metrics): (String, Option<String>, Option<String>, Option<String>) =
        sb.db()
            .query_row(
                "SELECT status, outcome, error, metrics_json FROM runs WHERE role = ?1 ORDER BY id DESC LIMIT 1",
                [role],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap_or_else(|e| panic!("no {role} run row: {e}"));
    let metrics = serde_json::from_str(&metrics.unwrap_or_else(|| "{}".to_string())).unwrap();
    (status, outcome, error, metrics)
}

// ---------------------------------------------------------------------------
// Task 1: redaction before every sink
// ---------------------------------------------------------------------------

#[test]
fn eng05_key_absent_everywhere() {
    let sb = Sandbox::new();
    gw_setup(&sb);
    let env = gw_env();
    run_lead(&sb, &env);

    let mut ops = dev_ops_prefix();
    ops.push(echo_op("ANTHROPIC_AUTH_TOKEN", "stdout"));
    ops.push(echo_op("ANTHROPIC_AUTH_TOKEN", "stderr"));
    ops.extend(dev_ops_suffix());
    let res = sb.run_env(
        &["run", "dev", "--", "web-ban-hang"],
        Some(json!({ "ops": ops })),
        &env,
    );
    assert_ok(&res, "run dev");
    assert!(
        res.stderr.contains("· engine gw · model m-dev"),
        "the run was not on the gateway engine: {}",
        res.stderr
    );

    assert_absent_everywhere(
        &sb,
        GW_KEY,
        &[
            ("agent-brain stdout", &res.stdout),
            ("agent-brain stderr", &res.stderr),
        ],
    );

    // The echo really happened and was scrubbed, not skipped.
    let dev_dir = run_dir_of(&sb, "dev");
    let stream = std::fs::read_to_string(dev_dir.join("stream.jsonl")).unwrap();
    assert!(
        stream.contains(&format!("echo_env: {REDACTED}")),
        "stream.jsonl lacks the redacted echo"
    );
    let stderr_txt = std::fs::read_to_string(dev_dir.join("stderr.txt")).unwrap();
    assert!(
        stderr_txt.contains(&format!("echo_env: {REDACTED}")),
        "stderr.txt lacks the redacted echo"
    );
    let events_with_mark: i64 = sb
        .db()
        .query_row(
            "SELECT COUNT(*) FROM events WHERE instr(raw_json, ?1) > 0",
            [REDACTED],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        events_with_mark >= 1,
        "events.raw_json lacks the redacted echo"
    );
}

#[test]
fn eng05_quota_detail_redacted() {
    let sb = Sandbox::new();
    gw_setup(&sb);
    let env = gw_env();
    run_lead(&sb, &env);

    // A gateway answers a quota error whose text quotes the caller's key.
    let text = format!("You've hit your session limit for key {GW_KEY}");
    let sc = json!({
        "events": [
            {"type": "system", "subtype": "init", "session_id": "fake-gw", "model": "fake"},
            {
                "type": "result", "subtype": "success", "is_error": true,
                "terminal_reason": "blocking_limit",
                "result": text, "num_turns": 1, "session_id": "fake-gw",
                "total_cost_usd": 0,
                "usage": {"input_tokens": 0, "output_tokens": 0,
                          "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0},
                "permission_denials": []
            }
        ],
        "exit_code": 1,
    });
    let res = sb.run_env(&["run", "dev", "--", "web-ban-hang"], Some(sc), &env);
    assert_eq!(
        res.code,
        5,
        "quota exit: {}\n{}",
        safe(&res.stderr),
        safe(&res.stdout)
    );

    let (_, outcome, error, _) = run_row(&sb, "dev");
    assert_eq!(outcome.as_deref(), Some("quota"));
    let error = error.unwrap_or_default();
    assert!(error.contains(REDACTED), "runs.error not redacted");
    assert_absent_everywhere(
        &sb,
        GW_KEY,
        &[
            ("agent-brain stdout", &res.stdout),
            ("agent-brain stderr", &res.stderr),
        ],
    );
}

// ---------------------------------------------------------------------------
// Task 2: kill, timeout, dashboard job output, two keys, no cost without secrets
// ---------------------------------------------------------------------------

const GW_B_KEY: &str = "test-planted-gw-b-key-0123456789";

/// A Dev scenario that echoes the gateway token on both streams and then stays alive.
fn echo_then_sleep_scenario() -> Value {
    json!({
        "ops": [
            echo_op("ANTHROPIC_AUTH_TOKEN", "stdout"),
            echo_op("ANTHROPIC_AUTH_TOKEN", "stderr"),
        ],
        "sleep_after": 30,
    })
}

/// Starts `agent-brain run dev` for project `web-ban-hang` with piped output.
fn spawn_dev(sb: &Sandbox, scenario: Value, env: &[(&str, &str)]) -> std::process::Child {
    sb.write_scenario(scenario);
    let mut cmd = sb.command(&["run", "dev", "--", "web-ban-hang"], env);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.spawn().expect("spawn agent-brain run dev")
}

/// Waits until the Dev run is `running` and its stream already holds the redacted echo.
fn wait_dev_streaming(sb: &Sandbox) -> i64 {
    let mut task_id = None;
    let ok = common::wait_until(std::time::Duration::from_secs(20), || {
        let Ok(db) = rusqlite::Connection::open(&sb.db_path) else {
            return false;
        };
        let row: rusqlite::Result<(Option<i64>, String, Option<String>)> = db.query_row(
            "SELECT task_id, status, run_dir FROM runs WHERE role = 'dev' ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        );
        let Ok((tid, status, Some(dir))) = row else {
            return false;
        };
        let stream =
            std::fs::read_to_string(PathBuf::from(dir).join("stream.jsonl")).unwrap_or_default();
        if status == "running" && stream.contains(REDACTED) {
            task_id = tid;
            return true;
        }
        false
    });
    assert!(ok, "the Dev run never showed its echo in stream.jsonl");
    task_id.expect("dev run has a task")
}

fn finish(child: std::process::Child) -> (Option<i32>, String, String) {
    let out = child.wait_with_output().expect("wait for agent-brain");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn eng05_killed_run_redacted() {
    let sb = Sandbox::new();
    gw_setup(&sb);
    let env = gw_env();
    run_lead(&sb, &env);

    let child = spawn_dev(&sb, echo_then_sleep_scenario(), &env);
    let task_id = wait_dev_streaming(&sb);
    let killed = sb.run(&["kill", &task_id.to_string()], None);
    assert_eq!(
        killed.code,
        0,
        "kill: {}\n{}",
        safe(&killed.stderr),
        safe(&killed.stdout)
    );
    let (_, out, err) = finish(child);

    let (status, _, error, _) = run_row(&sb, "dev");
    assert_eq!(status, "killed");
    assert_absent_everywhere(
        &sb,
        GW_KEY,
        &[
            ("run stdout", &out),
            ("run stderr", &err),
            ("kill stdout", &killed.stdout),
            ("kill stderr", &killed.stderr),
        ],
    );
    assert!(
        !error.unwrap_or_default().contains(GW_KEY),
        "key found in runs.error"
    );
    // The stderr line the child wrote before it died was rewritten, not dropped.
    let stderr_txt = std::fs::read_to_string(run_dir_of(&sb, "dev").join("stderr.txt")).unwrap();
    assert!(
        stderr_txt.contains(REDACTED),
        "stderr.txt lost the redacted echo"
    );
}

#[test]
fn eng05_timeout_run_redacted() {
    let sb = Sandbox::new();
    gw_setup_with(
        &sb,
        json!({"timeout_seconds": {
            "lead": 30, "dev": 2, "qa": 30, "inspect": 30, "plan-review": 30, "code-review": 30
        }}),
    );
    let env = gw_env();
    run_lead(&sb, &env);

    let res = sb.run_env(
        &["run", "dev", "--", "web-ban-hang"],
        Some(echo_then_sleep_scenario()),
        &env,
    );
    assert_ne!(res.code, 0, "a timed-out dev must not succeed");
    let (status, _, _, _) = run_row(&sb, "dev");
    assert_eq!(status, "timed_out");
    assert_absent_everywhere(
        &sb,
        GW_KEY,
        &[
            ("agent-brain stdout", &res.stdout),
            ("agent-brain stderr", &res.stderr),
        ],
    );
    let dev_dir = run_dir_of(&sb, "dev");
    let stream = std::fs::read_to_string(dev_dir.join("stream.jsonl")).unwrap();
    assert!(
        stream.contains(REDACTED),
        "stream.jsonl lost the redacted echo"
    );
    let stderr_txt = std::fs::read_to_string(dev_dir.join("stderr.txt")).unwrap();
    assert!(
        stderr_txt.contains(REDACTED),
        "stderr.txt lost the redacted echo"
    );
}

#[test]
fn eng05_job_output_redacted() {
    let sb = Sandbox::new();
    gw_setup(&sb);
    let env = gw_env();
    run_lead(&sb, &env);

    let mut ops = dev_ops_prefix();
    ops.push(echo_op("ANTHROPIC_AUTH_TOKEN", "stdout"));
    ops.push(echo_op("ANTHROPIC_AUTH_TOKEN", "stderr"));
    ops.extend(dev_ops_suffix());
    sb.write_scenario(json!({ "ops": ops }));

    let srv = sb.serve(&env);
    let resp = srv.post_json("/api/tasks/1/dev", &json!({}));
    assert_eq!(resp.status, 202, "start dev job");
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    let mut job = Value::Null;
    let ok = common::wait_until(std::time::Duration::from_secs(30), || {
        let r = srv.get(&format!("/api/jobs/{job_id}"));
        if r.status == 200 && r.json()["state"] == "exited" {
            job = r.json();
            return true;
        }
        false
    });
    assert!(ok, "the dev job did not exit");
    assert_eq!(job["exit_code"], json!(0), "dev job exit");

    let output = job["output"].to_string();
    assert!(!output.contains(GW_KEY), "key found in the job output");
    assert!(
        output.contains(REDACTED),
        "the job output lacks the redacted echo"
    );
    let serve_err = std::fs::read_to_string(&srv.stderr_path).unwrap_or_default();
    let serve_out = srv.stdout.lock().unwrap().clone();
    assert_absent_everywhere(
        &sb,
        GW_KEY,
        &[("serve stderr", &serve_err), ("serve stdout", &serve_out)],
    );
    // Every dashboard read of the task and its runs is clean too.
    for path in ["/api/tasks/1", "/api/tasks", "/api/projects"] {
        let body = srv.get(path).text();
        assert!(!body.contains(GW_KEY), "key found in GET {path}");
    }
}

fn fake_agy() -> PathBuf {
    common::repo_root()
        .join("tests")
        .join("fakes")
        .join("fake_agy.py")
        .canonicalize()
        .expect("canonicalize fake_agy.py")
}

fn rec_env_names(rec: &Value) -> Vec<String> {
    rec["env_names"]
        .as_array()
        .unwrap_or_else(|| panic!("record has no env_names: {rec}"))
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn eng05_two_keys_isolated() {
    let sb = Sandbox::new();
    sb.write_config(json!({"engines": [
        gateway_entry("gw", "https://gw.invalid/x", "GW_KEY"),
        gateway_entry("gw-b", "https://gw-b.invalid/x", "GW_B_KEY"),
        {"id": "agy", "kind": "agy", "command": fake_agy().to_str().unwrap()},
    ]}));
    sb.write_teams(json!({
        "doi-gw": {"description": "Dev trên gw.", "roles": {"dev": "gw"}},
        "doi-agy": {"description": "Dev trên agy.", "roles": {"dev": "agy"}},
    }));
    let repo_gw = app_repo(&sb, "web-ban-hang");
    let repo_agy = app_repo(&sb, "web-agy");
    let entry = |repo: &Path, id: &str, team: &str| {
        let mut e = sb.project_entry(repo, id, "work");
        let o = e.as_object_mut().unwrap();
        o.remove("workflow");
        o.insert("team".to_string(), json!(team));
        e
    };
    sb.write_map(
        Value::Array(vec![
            entry(&repo_gw, "web-ban-hang", "doi-gw"),
            entry(&repo_agy, "web-agy", "doi-agy"),
        ]),
        &[],
    );

    // Both keys are exported in the shell that runs every command.
    let env = [("GW_KEY", GW_KEY), ("GW_B_KEY", GW_B_KEY)];

    // Lead on claude (web-ban-hang): tries to echo key B, which is not in its env.
    let mut lead_ops = lead_plan_entry()["ops"].as_array().unwrap().clone();
    lead_ops.extend(vec![
        echo_op("GW_B_KEY", "stdout"),
        echo_op("GW_KEY", "stdout"),
        echo_op("GW_B_KEY", "stderr"),
    ]);
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
        Some(json!({ "ops": lead_ops })),
        &env,
    );
    assert_ok(&res, "run lead (claude)");
    let mut outputs: Vec<(String, String)> = vec![
        ("lead stdout".into(), res.stdout.clone()),
        ("lead stderr".into(), res.stderr.clone()),
    ];

    // Dev on gw: its own key is redacted, key B is not in its env at all.
    let mut dev_ops = dev_ops_prefix();
    dev_ops.push(echo_op("ANTHROPIC_AUTH_TOKEN", "stdout"));
    dev_ops.push(echo_op("GW_B_KEY", "stdout"));
    dev_ops.push(echo_op("GW_B_KEY", "stderr"));
    dev_ops.extend(dev_ops_suffix());
    let res = sb.run_env(
        &["run", "dev", "--", "web-ban-hang"],
        Some(json!({ "ops": dev_ops })),
        &env,
    );
    assert_ok(&res, "run dev (gw)");
    outputs.push(("gw dev stdout".into(), res.stdout.clone()));
    outputs.push(("gw dev stderr".into(), res.stderr.clone()));

    // Lead on claude (web-agy), then Dev on the fake agy.
    let res = sb.run_env(
        &[
            "run",
            "lead",
            "--name",
            "sua-app",
            "--",
            "web-agy",
            "sửa app",
        ],
        Some(lead_plan_entry()),
        &env,
    );
    assert_ok(&res, "run lead (web-agy)");
    outputs.push(("agy lead stdout".into(), res.stdout.clone()));
    outputs.push(("agy lead stderr".into(), res.stderr.clone()));

    let agy_scenario = sb.root.join("fake_agy_scenario.json");
    let agy_record = sb.root.join("fake_agy_record.jsonl");
    std::fs::write(
        &agy_scenario,
        serde_json::to_string(&json!({"ops": [
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"}
        ]}))
        .unwrap(),
    )
    .unwrap();
    let mut agy_env: Vec<(&str, &str)> = env.to_vec();
    agy_env.push(("FAKE_AGY_SCENARIO", agy_scenario.to_str().unwrap()));
    agy_env.push(("FAKE_AGY_RECORD", agy_record.to_str().unwrap()));
    let res = sb.run_env(&["run", "dev", "--", "web-agy"], None, &agy_env);
    assert_ok(&res, "run dev (agy)");
    outputs.push(("agy dev stdout".into(), res.stdout.clone()));
    outputs.push(("agy dev stderr".into(), res.stderr.clone()));

    // Engines of the four runs, as recorded.
    let engines: Vec<String> = {
        let db = sb.db();
        let mut st = db.prepare("SELECT engine FROM runs ORDER BY id").unwrap();
        let rows = st.query_map([], |r| r.get::<_, String>(0)).unwrap();
        rows.map(|r| r.unwrap()).collect()
    };
    assert_eq!(engines, ["claude", "gw", "claude", "agy"]);

    // Env names: nobody but the gw run holds a token, and nobody holds a key variable.
    let recs = sb.records();
    assert_eq!(recs.len(), 3, "claude records");
    for rec in &recs {
        let names = rec_env_names(rec);
        assert!(!names.iter().any(|n| n == "GW_KEY" || n == "GW_B_KEY"));
    }
    let has_token = |rec: &Value| {
        rec_env_names(rec)
            .iter()
            .any(|n| n == "ANTHROPIC_AUTH_TOKEN")
    };
    assert!(!has_token(&recs[0]) && has_token(&recs[1]) && !has_token(&recs[2]));
    let agy_records: Vec<Value> = std::fs::read_to_string(&agy_record)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(agy_records.len(), 1);
    let names = rec_env_names(&agy_records[0]);
    assert!(!names
        .iter()
        .any(|n| n == "GW_KEY" || n == "GW_B_KEY" || n == "ANTHROPIC_AUTH_TOKEN"));

    // Key B is in no sink of any of the four runs.
    let outs: Vec<(&str, &str)> = outputs
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    assert_absent_everywhere(&sb, GW_B_KEY, &outs);
    // Key A is in no sink either (the gw run echoed it and it was redacted).
    assert_absent_everywhere(&sb, GW_KEY, &outs);
    assert!(file_hits(&repo_gw, GW_B_KEY).is_empty() && file_hits(&repo_agy, GW_B_KEY).is_empty());
}

#[test]
fn claude_stream_unchanged_without_secrets() {
    let sb = Sandbox::new();
    protocol::setup_project(&sb, "work");
    let lines = [
        "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s1\",\"model\":\"fake\"}",
        "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"xin chào [đã ẩn] — dòng có ký tự đặc biệt\\t\"}]}}",
        "not json at all",
        "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":1,\"session_id\":\"s1\",\"total_cost_usd\":0,\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"cache_read_input_tokens\":0,\"cache_creation_input_tokens\":0},\"permission_denials\":[]}",
    ];
    // The key variable is exported, but a claude run carries no key: nothing is rewritten.
    let _ = sb.run_env(
        &["run", "lead", "--", "web-ban-hang", "sửa app"],
        Some(json!({ "raw_lines": lines })),
        &gw_env(),
    );
    let stream = std::fs::read(run_dir_of(&sb, "lead").join("stream.jsonl")).unwrap();
    let expected = format!("{}\n", lines.join("\n"));
    assert!(
        stream == expected.as_bytes(),
        "stream.jsonl differs from the fake's stdout"
    );
}

// ---------------------------------------------------------------------------
// Task 3: the post-run scan before any commit, and the dry run
// ---------------------------------------------------------------------------

/// Subjects of every commit of the repo, newest first.
fn commit_subjects(sb: &Sandbox, repo: &Path) -> Vec<String> {
    sb.git(repo, &["log", "--all", "--format=%s"])
        .lines()
        .map(|l| l.to_string())
        .collect()
}

fn leak_message(id: &str, paths: &str) -> String {
    format!("Phát hiện key của engine {id} trong file: {paths} — không commit.")
}

#[test]
fn eng05_key_written_to_plan_not_committed() {
    let sb = Sandbox::new();
    sb.write_config(json!({"engines": [
        gateway_entry("gw", "https://gw.invalid/x", "GW_KEY"),
    ]}));
    sb.write_teams(json!({"doi-gw": {
        "description": "Lead qua gateway.",
        "roles": {"lead": "gw"}
    }}));
    let repo = app_repo(&sb, "web-ban-hang");
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    let obj = entry.as_object_mut().unwrap();
    obj.remove("workflow");
    obj.insert("team".to_string(), json!("doi-gw"));
    sb.write_map(Value::Array(vec![entry]), &[]);

    // The Lead writes a valid plan and then copies its token into it.
    let mut ops = lead_plan_entry()["ops"].as_array().unwrap().clone();
    ops.push(json!({
        "op": "echo_env", "var": "ANTHROPIC_AUTH_TOKEN", "to": "file",
        "path": ".plan.md", "append": true
    }));
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
        Some(json!({ "ops": ops })),
        &gw_env(),
    );
    assert_eq!(
        res.code,
        3,
        "exit: {}\n{}",
        safe(&res.stderr),
        safe(&res.stdout)
    );
    let all = format!("{}{}", res.stdout, res.stderr);
    assert!(
        all.contains(&leak_message("gw", ".plan.md")),
        "the leak message is missing"
    );
    assert!(
        all.contains("Xóa key khỏi file (và khỏi lịch sử git nếu đã commit) rồi chạy lại."),
        "the leak message lacks its instruction"
    );
    assert!(!all.contains(GW_KEY), "key found in the CLI output");

    let subjects = commit_subjects(&sb, &repo);
    assert!(
        !subjects.iter().any(|s| s == "sua-app: plan"),
        "the plan was committed: {subjects:?}"
    );
    let (_, outcome, _, metrics) = run_row(&sb, "lead");
    assert_eq!(outcome.as_deref(), Some("protocol_violation"));
    assert_eq!(metrics["secret_leak"], json!([".plan.md"]));
    assert_absent_everywhere(&sb, GW_KEY, &[]);
}

#[test]
fn eng05_dev_file_with_key_not_committed() {
    let sb = Sandbox::new();
    let repo = gw_setup(&sb);
    let env = gw_env();
    run_lead(&sb, &env);
    let before = sb.git(&repo, &["rev-parse", "HEAD"]);

    let mut ops = dev_ops_prefix();
    ops.push(json!({
        "op": "echo_env", "var": "ANTHROPIC_AUTH_TOKEN", "to": "file",
        "path": "src/app.py", "append": true
    }));
    let res = sb.run_env(
        &["run", "dev", "--", "web-ban-hang"],
        Some(json!({ "ops": ops })),
        &env,
    );
    assert_eq!(
        res.code,
        3,
        "exit: {}\n{}",
        safe(&res.stderr),
        safe(&res.stdout)
    );
    let all = format!("{}{}", res.stdout, res.stderr);
    assert!(all.contains(&leak_message("gw", "src/app.py")));
    assert!(!all.contains(GW_KEY), "key found in the CLI output");

    assert_eq!(
        sb.git(&repo, &["rev-parse", "HEAD"]),
        before,
        "agent-brain committed after a key leak"
    );
    let (_, outcome, _, metrics) = run_row(&sb, "dev");
    assert_eq!(outcome.as_deref(), Some("protocol_violation"));
    assert_eq!(metrics["secret_leak"], json!(["src/app.py"]));
    assert_absent_everywhere(&sb, GW_KEY, &[]);
}

#[test]
fn eng05_committed_key_detected() {
    // The agent commits the key itself: the scan also reads what changed since the run's base.
    let sb = Sandbox::new();
    let repo = gw_setup(&sb);
    let env = gw_env();
    run_lead(&sb, &env);

    let mut ops = dev_ops_prefix();
    ops.push(json!({
        "op": "echo_env", "var": "ANTHROPIC_AUTH_TOKEN", "to": "file",
        "path": "src/app.py", "append": true
    }));
    ops.extend(dev_ops_suffix());
    let res = sb.run_env(
        &["run", "dev", "--", "web-ban-hang"],
        Some(json!({ "ops": ops })),
        &env,
    );
    assert_eq!(
        res.code,
        3,
        "exit: {}\n{}",
        safe(&res.stderr),
        safe(&res.stdout)
    );
    let all = format!("{}{}", res.stdout, res.stderr);
    assert!(all.contains(&leak_message("gw", "src/app.py")));
    assert!(!all.contains(GW_KEY), "key found in the CLI output");
    let (_, outcome, _, metrics) = run_row(&sb, "dev");
    assert_eq!(outcome.as_deref(), Some("protocol_violation"));
    assert_eq!(metrics["secret_leak"], json!(["src/app.py"]));
}

#[test]
fn dry_run_has_no_key() {
    let sb = Sandbox::new();
    gw_setup(&sb);
    let env = gw_env();
    run_lead(&sb, &env);

    for role in ["dev", "lead"] {
        let args: Vec<&str> = if role == "dev" {
            vec!["run", "dev", "--dry-run", "--", "web-ban-hang"]
        } else {
            vec!["run", "lead", "--dry-run", "--", "web-ban-hang", "sửa app"]
        };
        let res = sb.run_env(&args, None, &env);
        assert_eq!(
            res.code,
            0,
            "dry run {role}: {}\n{}",
            safe(&res.stderr),
            safe(&res.stdout)
        );
        let all = format!("{}{}", res.stdout, res.stderr);
        assert!(!all.contains(GW_KEY), "key found in the {role} dry run");
        assert!(!all.is_empty(), "the {role} dry run printed nothing");
    }
}
