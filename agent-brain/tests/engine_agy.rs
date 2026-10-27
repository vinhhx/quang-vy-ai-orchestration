//! Phase 6 plan 09: Antigravity (`agy`) as a headless engine. Every test runs against the fake
//! agy (tests/fakes/fake_agy.py); the real binary is never called.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{RunOut, Sandbox};
use serde_json::{json, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fake_agy() -> PathBuf {
    common::repo_root()
        .join("tests")
        .join("fakes")
        .join("fake_agy.py")
        .canonicalize()
        .expect("canonicalize fake_agy.py")
}

/// Points `engines.agy.command` at the fake agy.
fn agy_config(sb: &Sandbox) {
    sb.write_config(json!({"engines": {"agy": {"command": fake_agy().to_str().unwrap()}}}));
}

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

/// Environment of one fake agy call: scenario and record files under the sandbox.
struct AgyEnv {
    scenario: String,
    record: String,
}

impl AgyEnv {
    fn new(sb: &Sandbox, scenario: Value) -> AgyEnv {
        let scenario_path = sb.root.join("fake_agy_scenario.json");
        let record_path = sb.root.join("fake_agy_record.jsonl");
        std::fs::write(&scenario_path, serde_json::to_string(&scenario).unwrap()).unwrap();
        let _ = std::fs::remove_file(&record_path);
        AgyEnv {
            scenario: scenario_path.to_str().unwrap().to_string(),
            record: record_path.to_str().unwrap().to_string(),
        }
    }

    fn vars(&self) -> Vec<(&str, &str)> {
        vec![
            ("FAKE_AGY_SCENARIO", self.scenario.as_str()),
            ("FAKE_AGY_RECORD", self.record.as_str()),
        ]
    }

    fn records(&self) -> Vec<Value> {
        match std::fs::read_to_string(&self.record) {
            Ok(c) => c
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| serde_json::from_str(l).unwrap())
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}

fn usage() -> Value {
    json!({
        "input_tokens": 120,
        "output_tokens": 30,
        "thinking_tokens": 5,
        "cache_read_tokens": 40,
        "total_tokens": 150
    })
}

/// init -> DONE agent_response step -> result, shaped like the S7 stream.
fn agy_lines(response: &str, status: &str, permission_mode: &str) -> Vec<Value> {
    vec![
        json!({"event": "init", "conversation_id": "c1",
               "init": {"cwd": "/x", "tools": ["write_to_file"], "permission_mode": permission_mode}}),
        json!({"event": "step_update", "step_update": {
            "conversation_id": "c1", "step_index": 1, "state": "DONE",
            "step_type": "agent_response", "usage": usage()}}),
        json!({"event": "result", "result": {
            "conversation_id": "c1", "status": status, "response": response,
            "num_turns": 1, "usage": usage()}}),
    ]
}

fn lead_plan_text() -> String {
    protocol::plan_text(
        "Kế hoạch",
        &[("src/app.py", "app()", "trả về 2", "demo")],
        "python3 -m unittest",
        "",
    )
}

fn write_doi_agy(sb: &Sandbox, roles: Value) {
    sb.write_teams(json!({"doi-agy": {"description": "Lead bằng agy", "roles": roles}}));
}

/// Repo `web` without a workflow entry (server default: mac-dinh), profile `work`.
fn setup_project(sb: &Sandbox, profile: &str) -> PathBuf {
    let repo = sb.make_repo(
        "web",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web", profile);
    entry.as_object_mut().unwrap().remove("workflow");
    sb.write_map(json!([entry]), &[]);
    repo
}

fn run_lead(sb: &Sandbox, env: &AgyEnv) -> RunOut {
    sb.run_env(
        &[
            "run",
            "lead",
            "--team",
            "doi-agy",
            "--name",
            "sua-app",
            "--",
            "web",
            "sửa app",
        ],
        None,
        &env.vars(),
    )
}

struct RunRow {
    engine: String,
    status: String,
    outcome: Option<String>,
    usage_source: Option<String>,
    input: i64,
    output: i64,
    cache_read: i64,
    session_id: Option<String>,
    metrics: Value,
    run_dir: String,
}

fn run_row(sb: &Sandbox, role: &str) -> RunRow {
    sb.db()
        .query_row(
            "SELECT engine, status, outcome, usage_source, input_tokens, output_tokens, \
             cache_read_tokens, session_id, metrics_json, run_dir \
             FROM runs WHERE role = ?1 ORDER BY id LIMIT 1",
            [role],
            |r| {
                let metrics: Option<String> = r.get(8)?;
                Ok(RunRow {
                    engine: r.get(0)?,
                    status: r.get(1)?,
                    outcome: r.get(2)?,
                    usage_source: r.get(3)?,
                    input: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    output: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    cache_read: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
                    session_id: r.get(7)?,
                    metrics: serde_json::from_str(&metrics.unwrap_or_else(|| "{}".into())).unwrap(),
                    run_dir: r.get::<_, Option<String>>(9)?.unwrap_or_default(),
                })
            },
        )
        .unwrap_or_else(|e| panic!("no {role} run row: {e}"))
}

// ---------------------------------------------------------------------------
// Task 1: agy as Lead
// ---------------------------------------------------------------------------

#[test]
fn agy_lead_tracer() {
    let sb = Sandbox::new();
    agy_config(&sb);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    let repo = setup_project(&sb, "work");

    let env = AgyEnv::new(
        &sb,
        json!({
            "lines": agy_lines(&lead_plan_text(), "SUCCESS", "request-review"),
            "delay_ms": 30,
            "final_newline": false,
        }),
    );
    let res = run_lead(&sb, &env);
    assert_ok(&res, "run lead (agy)");
    assert!(
        res.stderr.contains("· engine agy ·"),
        "header lacks the engine: {}",
        res.stderr
    );

    // The plan is committed with the plan body, written by agent-brain from the response.
    let subjects = sb.git(&repo, &["log", "--format=%s"]);
    assert!(
        subjects.lines().any(|l| l == "sua-app: plan"),
        "no plan commit: {subjects}"
    );
    let plan = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    assert!(
        plan.contains("| 1 | src/app.py | app() | trả về 2 | demo |"),
        "{plan}"
    );

    let row = run_row(&sb, "lead");
    assert_eq!(row.engine, "agy");
    assert_eq!(row.status, "succeeded");
    assert_eq!(row.usage_source.as_deref(), Some("result"));
    assert_eq!(row.input, 120);
    assert_eq!(row.output, 35, "30 output + 5 thinking");
    assert_eq!(row.cache_read, 40);
    assert_eq!(row.session_id.as_deref(), Some("c1"));
    assert_eq!(row.metrics["thinking_tokens"], json!(5), "{}", row.metrics);
}

#[test]
fn agy_stream_file_exact() {
    let sb = Sandbox::new();
    agy_config(&sb);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    setup_project(&sb, "work");

    let plan = lead_plan_text();
    let raw: Vec<String> = agy_lines(&plan, "SUCCESS", "request-review")
        .iter()
        .map(|v| serde_json::to_string(v).unwrap())
        .collect();
    let env = AgyEnv::new(
        &sb,
        json!({"lines": [], "raw_lines": raw, "delay_ms": 200, "final_newline": false}),
    );
    let res = run_lead(&sb, &env);
    assert_ok(&res, "run lead (agy, exact stream)");

    let row = run_row(&sb, "lead");
    let bytes = std::fs::read(PathBuf::from(&row.run_dir).join("stream.jsonl")).unwrap();
    let expected = raw.join("\n");
    assert_eq!(
        String::from_utf8_lossy(&bytes),
        expected,
        "stream.jsonl must equal what agy printed: no newline added, none lost"
    );
    // The last line (no newline) was still parsed: the result gave the usage.
    assert_eq!(row.usage_source.as_deref(), Some("result"));
}

#[test]
fn agy_env_and_argv_hygiene() {
    let sb = Sandbox::new();
    agy_config(&sb);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    // personal sets CLAUDE_CONFIG_DIR in the claude env: agy must not receive it.
    setup_project(&sb, "personal");

    let env = AgyEnv::new(
        &sb,
        json!({"lines": agy_lines(&lead_plan_text(), "SUCCESS", "request-review")}),
    );
    let mut vars = env.vars();
    vars.push(("DEEPSEEK_API_KEY", "ds-secret-value"));
    vars.push(("ANTHROPIC_MODEL", "x"));
    let res = sb.run_env(
        &[
            "run",
            "lead",
            "--team",
            "doi-agy",
            "--name",
            "sua-app",
            "--",
            "web",
            "sửa app",
        ],
        None,
        &vars,
    );
    assert_ok(&res, "run lead (agy, hygiene)");

    let recs = env.records();
    assert_eq!(recs.len(), 1, "one agy call");
    let argv: Vec<&str> = recs[0]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(argv[0], "-p");
    for want in ["--output-format", "stream-json", "--print-timeout", "15s"] {
        assert!(argv.contains(&want), "argv lacks {want}: {argv:?}");
    }
    for bad in ["--mode", "--sandbox", "--dangerously-skip-permissions"] {
        assert!(!argv.contains(&bad), "a lead must not get {bad}");
    }
    assert!(
        !argv.contains(&"--max-turns"),
        "agy has no turn limit flag: {argv:?}"
    );

    let names: Vec<&str> = recs[0]["env_names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"FAKE_AGY_RECORD"),
        "env was not passed: {names:?}"
    );
    for bad in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_MODEL",
        "CLAUDE_CONFIG_DIR",
        "DEEPSEEK_API_KEY",
    ] {
        assert!(!names.contains(&bad), "agy received {bad}: {names:?}");
    }
}

#[test]
fn agy_error_status_fails() {
    let sb = Sandbox::new();
    agy_config(&sb);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    setup_project(&sb, "work");

    // agy exits 0 with an ERROR result: the run still fails.
    let env = AgyEnv::new(
        &sb,
        json!({"status": "ERROR", "error": "boom", "response": ""}),
    );
    let res = run_lead(&sb, &env);
    assert_ne!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let row = run_row(&sb, "lead");
    assert_eq!(row.engine, "agy");
    assert_eq!(row.status, "failed");
}
