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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
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
    sb.write_config(json!({"engines": [
        gateway_entry("gw", "https://gw.invalid/x", "GW_KEY"),
    ]}));
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
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
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
        &[("agent-brain stdout", &res.stdout), ("agent-brain stderr", &res.stderr)],
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
    assert!(events_with_mark >= 1, "events.raw_json lacks the redacted echo");
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
                "api_error_status": 429, "terminal_reason": "blocking_limit",
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
    assert_eq!(res.code, 5, "quota exit: {}\n{}", res.stderr, res.stdout);

    let (_, outcome, error, _) = run_row(&sb, "dev");
    assert_eq!(outcome.as_deref(), Some("quota"));
    let error = error.unwrap_or_default();
    assert!(error.contains(REDACTED), "runs.error not redacted");
    assert_absent_everywhere(
        &sb,
        GW_KEY,
        &[("agent-brain stdout", &res.stdout), ("agent-brain stderr", &res.stderr)],
    );
}
