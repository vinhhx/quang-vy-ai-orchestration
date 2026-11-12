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
               "init": {"tools": ["write_to_file"], "permission_mode": permission_mode}}),
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

#[test]
fn agy_lead_empty_response_is_agy_empty() {
    let sb = Sandbox::new();
    agy_config(&sb);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    setup_project(&sb, "work");

    let env = AgyEnv::new(
        &sb,
        json!({
            "lines": agy_lines("", "SUCCESS", "request-review"),
        }),
    );
    let res = run_lead(&sb, &env);
    assert_ne!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    let combined_out = format!("{}\n{}", res.stdout, res.stderr);
    assert!(
        combined_out.contains("agy không trả kế hoạch nào"),
        "expected empty lead message in output: {combined_out}"
    );

    let row = run_row(&sb, "lead");
    assert_eq!(row.engine, "agy");
    assert_eq!(row.status, "succeeded");
    assert_eq!(row.outcome.as_deref(), Some("agy:empty"));
}

#[test]
fn agy_lead_prompt_has_git_context() {
    let sb = Sandbox::new();
    agy_config(&sb);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    let repo = setup_project(&sb, "work");
    let branch = sb.git(&repo, &["branch", "--show-current"]).trim().to_string();

    let env = AgyEnv::new(
        &sb,
        json!({
            "lines": agy_lines(&lead_plan_text(), "SUCCESS", "request-review"),
        }),
    );
    let res = run_lead(&sb, &env);
    assert_ok(&res, "run lead (agy, git context)");

    let recs = env.records();
    assert_eq!(recs.len(), 1, "one agy call");
    let argv: Vec<&str> = recs[0]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let p_idx = argv.iter().position(|&x| x == "-p").expect("argv has -p");
    let prompt = argv[p_idx + 1];

    assert!(
        prompt.contains("## Ngữ cảnh git"),
        "prompt lacks git context header: {prompt}"
    );
    assert!(
        prompt.contains("### git status --short"),
        "prompt lacks git status: {prompt}"
    );
    assert!(
        prompt.contains("### git log -n 20 --stat --date=iso"),
        "prompt lacks git log: {prompt}"
    );
    assert!(
        prompt.contains(&branch),
        "prompt lacks current branch '{branch}': {prompt}"
    );
}

#[test]
fn claude_lead_prompt_has_no_git_context() {
    let sb = Sandbox::new();
    setup_project(&sb, "work");

    let scenario = protocol::lead_scenario(&[("src/app.py", "1-5", "sửa app", "yêu cầu")]);
    let res = sb.run(&["run", "lead", "web", "sửa app"], Some(scenario));
    assert_ok(&res, "run lead (claude)");

    let recs = sb.records();
    assert_eq!(recs.len(), 1, "one claude call");
    let argv: Vec<&str> = recs[0]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let p_idx = argv.iter().position(|&x| x == "-p").expect("argv has -p");
    let prompt = argv[p_idx + 1];

    assert!(
        !prompt.contains("## Ngữ cảnh git"),
        "claude lead prompt must not contain git context: {prompt}"
    );
}

// ---------------------------------------------------------------------------
// Task 2: agy Dev
// ---------------------------------------------------------------------------

/// A plan whose table lists `files`, written by an agy Lead; leaves the repo on the task branch
/// with the plan committed. `roles` is the team's per-role engine map.
fn plan_task(sb: &Sandbox, roles: Value, files: &[&str]) -> PathBuf {
    agy_config(sb);
    write_doi_agy(sb, roles);
    let repo = setup_project(sb, "work");
    let rows: Vec<(&str, &str, &str, &str)> = files
        .iter()
        .map(|f| (*f, "toàn file", "sửa", "demo"))
        .collect();
    let plan = protocol::plan_text("Kế hoạch", &rows, "python3 -m unittest", "");
    let env = AgyEnv::new(
        sb,
        json!({"lines": agy_lines(&plan, "SUCCESS", "request-review")}),
    );
    let res = run_lead(sb, &env);
    assert_ok(&res, "run lead (agy, plan)");
    repo
}

fn write_op(path: &str, content: &str) -> Value {
    json!({"op": "write", "path": path, "content": content})
}

fn run_dev(sb: &Sandbox, scenario: Value) -> (RunOut, AgyEnv) {
    let env = AgyEnv::new(sb, scenario);
    let res = sb.run_env(&["run", "dev", "web"], None, &env.vars());
    (res, env)
}

fn argv_of(rec: &Value) -> Vec<String> {
    rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

fn dev_roles() -> Value {
    json!({"lead": "agy", "dev": "agy"})
}

#[test]
fn agy_dev_edits_in_scope() {
    let sb = Sandbox::new();
    let repo = plan_task(&sb, dev_roles(), &["src/a.txt"]);

    let (res, env) = run_dev(&sb, json!({"ops": [write_op("src/a.txt", "a\n")]}));
    assert_ok(&res, "run dev (agy)");

    let subject = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert!(subject.starts_with("sua-app: "), "head subject: {subject}");
    let files = sb.git(&repo, &["show", "--name-only", "--format=", "HEAD"]);
    assert!(files.lines().any(|l| l == "src/a.txt"), "{files}");

    let row = run_row(&sb, "dev");
    assert_eq!(row.engine, "agy");
    assert_eq!(row.status, "succeeded");

    let recs = env.records();
    assert_eq!(recs.len(), 1);
    let argv = argv_of(&recs[0]);
    assert!(
        argv.windows(2).any(|w| w == ["--mode", "accept-edits"]),
        "{argv:?}"
    );
    assert_eq!(recs[0]["permission"], json!("accept-edits"));
}

#[test]
fn agy_dev_out_of_scope_violation() {
    let sb = Sandbox::new();
    let repo = plan_task(&sb, dev_roles(), &["src/a.txt"]);

    let (res, _env) = run_dev(
        &sb,
        json!({"ops": [write_op("src/a.txt", "a\n"), write_op("src/b.txt", "b\n")]}),
    );
    assert_eq!(
        res.code, 3,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(res.stdout.contains("src/b.txt"), "stdout: {}", res.stdout);
    assert_eq!(
        run_row(&sb, "dev").outcome.as_deref(),
        Some("protocol_violation")
    );
    // Nothing was committed or reverted: the files stay for a person to look at.
    assert!(repo.join("src/b.txt").is_file());
}

#[test]
fn agy_dev_planning_and_secret_paths() {
    // `.planning/` is never agy's, even when the plan lists it.
    let sb = Sandbox::new();
    plan_task(&sb, dev_roles(), &[".planning/x.md"]);
    let (res, _env) = run_dev(&sb, json!({"ops": [write_op(".planning/x.md", "x\n")]}));
    assert_eq!(
        res.code, 3,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        res.stdout.contains(".planning/x.md"),
        "stdout: {}",
        res.stdout
    );
    assert_eq!(
        run_row(&sb, "dev").outcome.as_deref(),
        Some("protocol_violation")
    );

    // A secret-looking / CI path is a violation too.
    let sb = Sandbox::new();
    plan_task(&sb, dev_roles(), &[".github/workflows/ci.yml"]);
    let (res, _env) = run_dev(
        &sb,
        json!({"ops": [write_op(".github/workflows/ci.yml", "on: push\n")]}),
    );
    assert_eq!(
        res.code, 3,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        res.stdout.contains(".github/workflows/ci.yml"),
        "stdout: {}",
        res.stdout
    );
}

#[test]
fn agy_dev_empty_diff_suspicious() {
    let sb = Sandbox::new();
    let repo = plan_task(&sb, dev_roles(), &["src/a.txt"]);
    let before = sb.git(&repo, &["rev-parse", "HEAD"]);

    // SUCCESS and no ops: nothing changed.
    let (res, _env) = run_dev(&sb, json!({}));
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
    let row = run_row(&sb, "dev");
    assert_eq!(row.status, "succeeded");
    assert_eq!(row.outcome.as_deref(), Some("agy:empty"));
    assert_eq!(sb.git(&repo, &["rev-parse", "HEAD"]), before, "no commit");
}

#[test]
fn claude_dev_unchanged_for_planning_path() {
    // Only the Lead is agy: the Dev is claude, which keeps today's behaviour for `.planning/`.
    let sb = Sandbox::new();
    let repo = plan_task(&sb, json!({"lead": "agy"}), &[".planning/x.md"]);

    let scenario = json!({"ops": [write_op(".planning/x.md", "x\n")]});
    let res = sb.run_env(&["run", "dev", "web"], Some(scenario), &[]);
    assert_ok(&res, "run dev (claude, .planning path)");
    assert_eq!(run_row(&sb, "dev").engine, "claude");
    let files = sb.git(&repo, &["show", "--name-only", "--format=", "HEAD"]);
    assert!(files.lines().any(|l| l == ".planning/x.md"), "{files}");
}

// ---------------------------------------------------------------------------
// Task 3: quota, timeout, permission_mode check, kill
// ---------------------------------------------------------------------------

#[test]
fn agy_quota_stops() {
    let sb = Sandbox::new();
    agy_config(&sb);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    let repo = setup_project(&sb, "work");

    let env = AgyEnv::new(
        &sb,
        json!({
            "status": "ERROR",
            "error": "HTTP 429 RESOURCE_EXHAUSTED: Resource has been exhausted (e.g. check quota).",
            "response": "",
            "exit_code": 1,
        }),
    );
    let res = run_lead(&sb, &env);
    assert_eq!(
        res.code, 5,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );

    let row = run_row(&sb, "lead");
    assert_eq!(row.engine, "agy");
    assert_eq!(row.outcome.as_deref(), Some("quota"));
    assert_eq!(
        row.metrics["quota"]["engine"],
        json!("agy"),
        "{}",
        row.metrics
    );
    assert_eq!(
        row.metrics["quota"]["signal"],
        json!("agy.result.error"),
        "{}",
        row.metrics
    );
    // A quota stop writes nothing and commits nothing.
    let subjects = sb.git(&repo, &["log", "--format=%s"]);
    assert!(
        !subjects.lines().any(|l| l == "sua-app: plan"),
        "{subjects}"
    );
}

#[test]
fn agy_permission_mode_mismatch_warns() {
    let sb = Sandbox::new();
    agy_config(&sb);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    setup_project(&sb, "work");

    // The fake reports a mode other than the one the probe showed for a flag-less run.
    let env = AgyEnv::new(
        &sb,
        json!({
            "permission_mode": "always-proceed",
            "response": lead_plan_text(),
        }),
    );
    let res = run_lead(&sb, &env);
    assert_ok(&res, "run lead (agy, mode mismatch)");
    assert!(
        res.stderr.contains(
            "Cảnh báo: agy chạy với permission_mode always-proceed, mong đợi request-review"
        ),
        "stderr: {}",
        res.stderr
    );
    let row = run_row(&sb, "lead");
    assert_eq!(
        row.metrics["agy"],
        json!({"permission_mode": "always-proceed", "expected": "request-review"}),
        "{}",
        row.metrics
    );

    // The expected mode raises no warning and stores nothing.
    let sb = Sandbox::new();
    agy_config(&sb);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    setup_project(&sb, "work");
    let env = AgyEnv::new(&sb, json!({"response": lead_plan_text()}));
    let res = run_lead(&sb, &env);
    assert_ok(&res, "run lead (agy, expected mode)");
    assert!(
        !res.stderr.contains("Cảnh báo: agy chạy với"),
        "{}",
        res.stderr
    );
    assert!(run_row(&sb, "lead").metrics.get("agy").is_none());
}

#[test]
fn agy_timeout_mapping() {
    // S7b: a `--print-timeout` expiry is status SUCCESS, exit 0 and an empty response; the stderr
    // line is the only signal.
    let sb = Sandbox::new();
    agy_config(&sb);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    setup_project(&sb, "work");
    let env = AgyEnv::new(
        &sb,
        json!({
            "stderr": "[agy] print timeout after 5s with turn in progress; returning partial output\n",
            "response": "",
        }),
    );
    let res = run_lead(&sb, &env);
    assert_ne!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        res.stderr.contains("quá thời gian"),
        "stderr: {}",
        res.stderr
    );
    let row = run_row(&sb, "lead");
    assert_eq!(row.status, "timed_out");

    // A plain non-zero exit stays `failed`.
    let sb = Sandbox::new();
    agy_config(&sb);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    setup_project(&sb, "work");
    let env = AgyEnv::new(
        &sb,
        json!({"stderr": "boom\n", "exit_code": 2, "response": ""}),
    );
    let res = run_lead(&sb, &env);
    assert_ne!(res.code, 0);
    assert_eq!(run_row(&sb, "lead").status, "failed");
}

#[test]
fn agy_kill_s4() {
    use std::time::Duration;

    let sb = Sandbox::new();
    agy_config(&sb);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    setup_project(&sb, "work");

    let pidfile = sb.root.join("agy.pid");
    let env = AgyEnv::new(
        &sb,
        json!({
            "sleep_s": 30,
            "spawn_child": true,
            "pidfile": pidfile.to_str().unwrap(),
            "response": "",
        }),
    );
    let mut run_child = sb.spawn_env(
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
    );

    // The fake wrote "{agy pid} {child pid}".
    let mut pids: Option<(i32, i32)> = None;
    common::wait_until(Duration::from_secs(10), || {
        if let Ok(c) = std::fs::read_to_string(&pidfile) {
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
    let (agy_pid, child_pid) = pids.expect("fake agy did not write its pidfile");
    let procs = vec![
        agent_brain::procs::proc_id(agy_pid).expect("agy proc_id"),
        agent_brain::procs::proc_id(child_pid).expect("child proc_id"),
    ];
    let _guard = common::ProcGuard(procs.clone());

    // The run row is `running` once the process is recorded.
    let running = common::wait_until(Duration::from_secs(10), || {
        sb.db()
            .query_row(
                "SELECT COUNT(*) FROM runs WHERE status = 'running'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n == 1)
            .unwrap_or(false)
    });
    assert!(running, "no running run row");
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
    let _ = run_child.wait();

    assert_eq!(run_row(&sb, "lead").status, "killed");
    std::thread::sleep(Duration::from_secs(3));
    for p in &procs {
        assert!(
            !agent_brain::procs::alive(*p),
            "process {p:?} survived the kill"
        );
    }
}

// ---------------------------------------------------------------------------
// agy that answered but does not exit (an MCP child keeps it alive)
// ---------------------------------------------------------------------------

/// The fake agy config plus a 1 s kill grace and, when given, a short Lead timeout.
fn agy_config_with(sb: &Sandbox, lead_timeout: Option<u64>) {
    let mut cfg = json!({
        "engines": {"agy": {"command": fake_agy().to_str().unwrap()}},
        "kill_grace_secs": 1,
    });
    if let Some(t) = lead_timeout {
        cfg["timeout_seconds"] = json!({"lead": t, "dev": 30, "qa": 30, "inspect": 30});
    }
    sb.write_config(cfg);
}

/// Runs the agy Lead and, while it runs, records the fake agy and its sleeping child from the
/// pidfile ("{agy pid} {child pid}"). Returns the output, those processes and the run time.
fn run_lead_watching(
    sb: &Sandbox,
    env: &AgyEnv,
    pidfile: &std::path::Path,
) -> (RunOut, Vec<agent_brain::procs::ProcId>, std::time::Duration) {
    run_watching(
        sb,
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
        env,
        pidfile,
    )
}

/// `run_lead_watching` for any agent-brain command.
fn run_watching(
    sb: &Sandbox,
    args: &[&str],
    env: &AgyEnv,
    pidfile: &std::path::Path,
) -> (RunOut, Vec<agent_brain::procs::ProcId>, std::time::Duration) {
    use std::time::{Duration, Instant};
    let t0 = Instant::now();
    let mut cmd = sb.command(args, &env.vars());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let child = cmd.spawn().expect("spawn run lead");
    let mut procs = Vec::new();
    common::wait_until(Duration::from_secs(10), || {
        let text = std::fs::read_to_string(pidfile).unwrap_or_default();
        let ids: Vec<agent_brain::procs::ProcId> = text
            .split_whitespace()
            .filter_map(|x| x.parse::<i32>().ok())
            .filter_map(agent_brain::procs::proc_id)
            .collect();
        if ids.len() == 2 {
            procs = ids;
            true
        } else {
            false
        }
    });
    let guard = common::ProcGuard(procs.clone());
    let output = child.wait_with_output().expect("wait run lead");
    let elapsed = t0.elapsed();
    drop(guard);
    let out = RunOut {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    };
    (out, procs, elapsed)
}

fn assert_all_dead(procs: &[agent_brain::procs::ProcId]) {
    assert_eq!(procs.len(), 2, "the fake agy and its child were recorded");
    for p in procs {
        assert!(!agent_brain::procs::alive(*p), "process {p:?} survived");
    }
}

fn run_exit_code(sb: &Sandbox) -> Option<i64> {
    sb.db()
        .query_row(
            "SELECT exit_code FROM runs WHERE role = 'lead' ORDER BY id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn agy_lead_hung_after_result_is_stopped_and_plan_written() {
    use std::time::Duration;

    let sb = Sandbox::new();
    agy_config_with(&sb, None);
    write_doi_agy(&sb, json!({"lead": "agy"}));
    let repo = setup_project(&sb, "work");

    let pidfile = sb.root.join("agy.pid");
    let env = AgyEnv::new(
        &sb,
        json!({
            "lines": agy_lines(&lead_plan_text(), "SUCCESS", "request-review"),
            "spawn_child": true,
            "pidfile": pidfile.to_str().unwrap(),
            "hang_s": 600,
        }),
    );
    let (res, procs, elapsed) = run_lead_watching(&sb, &env, &pidfile);
    assert_ok(&res, "run lead (agy hung after result)");

    // 15 s grace + 1 s kill grace + the 2 s drain, far below the 1800 s Lead timeout.
    assert!(
        elapsed < Duration::from_secs(30),
        "run took {elapsed:?}: {}",
        res.stderr
    );
    assert!(
        res.stderr
            .contains("Run đã trả kết quả nhưng tiến trình chưa thoát sau 15s"),
        "stderr: {}",
        res.stderr
    );
    let plan = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    assert!(
        plan.contains("| 1 | src/app.py | app() | trả về 2 | demo |"),
        "{plan}"
    );
    let subjects = sb.git(&repo, &["log", "--format=%s"]);
    assert!(
        subjects.lines().any(|l| l == "sua-app: plan"),
        "no plan commit: {subjects}"
    );
    let row = run_row(&sb, "lead");
    assert_eq!(row.status, "succeeded");
    assert_eq!(row.usage_source.as_deref(), Some("result"));
    // The real exit code of the stopped agy is kept for the log.
    assert_ne!(run_exit_code(&sb), Some(0));
    assert_all_dead(&procs);
}

#[test]
fn agy_timeout_after_success_result_uses_the_answer() {
    let sb = Sandbox::new();
    // The 3 s timeout fires before the 15 s grace after the result.
    agy_config_with(&sb, Some(3));
    write_doi_agy(&sb, json!({"lead": "agy"}));
    let repo = setup_project(&sb, "work");

    let pidfile = sb.root.join("agy.pid");
    let env = AgyEnv::new(
        &sb,
        json!({
            "lines": agy_lines(&lead_plan_text(), "SUCCESS", "request-review"),
            "spawn_child": true,
            "pidfile": pidfile.to_str().unwrap(),
            "hang_s": 600,
        }),
    );
    let (res, procs, _) = run_lead_watching(&sb, &env, &pidfile);
    assert_all_dead(&procs);
    assert_ok(&res, "run lead (agy timed out after a SUCCESS result)");
    assert!(
        res.stderr
            .contains("Run quá thời gian nhưng agy đã trả kết quả SUCCESS trước đó"),
        "stderr: {}",
        res.stderr
    );
    let plan = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    assert!(
        plan.contains("| 1 | src/app.py | app() | trả về 2 | demo |"),
        "{plan}"
    );
    assert_eq!(run_row(&sb, "lead").status, "succeeded");
}

#[test]
fn agy_print_timeout_then_hang_stays_timed_out() {
    let sb = Sandbox::new();
    agy_config_with(&sb, Some(3));
    write_doi_agy(&sb, json!({"lead": "agy"}));
    setup_project(&sb, "work");

    let pidfile = sb.root.join("agy.pid");
    let env = AgyEnv::new(
        &sb,
        json!({
            "stderr": "[agy] print timeout after 5s with turn in progress; returning partial output\n",
            "response": "",
            "spawn_child": true,
            "pidfile": pidfile.to_str().unwrap(),
            "hang_s": 600,
        }),
    );
    let (res, procs, _) = run_lead_watching(&sb, &env, &pidfile);
    assert_all_dead(&procs);
    assert_ne!(
        res.code, 0,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        !res.stderr.contains("dùng kết quả này"),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(run_row(&sb, "lead").status, "timed_out");
}

#[test]
fn agy_timeout_without_result_stays_timed_out() {
    let sb = Sandbox::new();
    agy_config_with(&sb, Some(3));
    write_doi_agy(&sb, json!({"lead": "agy"}));
    setup_project(&sb, "work");

    let pidfile = sb.root.join("agy.pid");
    let env = AgyEnv::new(
        &sb,
        json!({
            "sleep_s": 600,
            "spawn_child": true,
            "pidfile": pidfile.to_str().unwrap(),
        }),
    );
    let (res, procs, _) = run_lead_watching(&sb, &env, &pidfile);
    assert_all_dead(&procs);
    assert_ne!(res.code, 0);
    assert_eq!(run_row(&sb, "lead").status, "timed_out");
}

// ---------------------------------------------------------------------------
// agy Dev tool steps checked against the plan's permissions
// ---------------------------------------------------------------------------

/// One agy tool step about to run, shaped like a real agy 1.x Dev log line.
fn active_tool(idx: i64, tool: &str, params: Value) -> Value {
    json!({"event": "step_update", "step_update": {
        "conversation_id": "c1", "step_index": idx, "state": "ACTIVE", "step_type": "tool",
        "tool_name": tool, "tool_info": {"name": tool, "parameters": params}}})
}

fn dev_error(sb: &Sandbox) -> Option<String> {
    sb.db()
        .query_row(
            "SELECT error FROM runs WHERE role = 'dev' ORDER BY id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn agy_dev_forbidden_command_stops_the_run() {
    use std::time::Duration;

    let sb = Sandbox::new();
    let repo = plan_task(&sb, dev_roles(), &["src/a.txt"]);

    let pidfile = sb.root.join("agy-dev.pid");
    let env = AgyEnv::new(
        &sb,
        json!({
            "lines": [
                {"event": "init", "conversation_id": "c1",
                 "init": {"cwd": repo.to_str().unwrap(), "permission_mode": "accept-edits"}},
                active_tool(2, "read_file", json!({"Path": repo.join(".plan.md").to_str().unwrap()})),
                active_tool(4, "run_command", json!({"CommandLine": "python3 -c 'print(1)'"})),
            ],
            "spawn_child": true,
            "pidfile": pidfile.to_str().unwrap(),
            "hang_s": 600,
        }),
    );
    let (res, procs, elapsed) = run_watching(&sb, &["run", "dev", "web"], &env, &pidfile);
    assert_eq!(
        res.code, 3,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        elapsed < Duration::from_secs(15),
        "the run was not stopped at the step: {elapsed:?}"
    );
    assert!(
        res.stderr.contains(
            "agy Dev dùng công cụ ngoài quyền của plan: run_command {\"CommandLine\":\"python3 -c 'print(1)'\"}"
        ),
        "stderr: {}",
        res.stderr
    );
    let row = run_row(&sb, "dev");
    assert_eq!(row.status, "succeeded");
    assert_eq!(row.outcome.as_deref(), Some("protocol_violation"));
    let err = dev_error(&sb).unwrap_or_default();
    assert!(err.contains("python3 -c"), "runs.error: {err}");
    assert_all_dead(&procs);

    // the decision is in the run's guard log, like a hook decision
    let log = std::fs::read_to_string(PathBuf::from(&row.run_dir).join("guard.jsonl")).unwrap();
    assert!(log.contains("\"decision\":\"block\""), "{log}");
}

#[test]
fn agy_dev_write_outside_plan_stops_the_run() {
    let sb = Sandbox::new();
    let repo = plan_task(&sb, dev_roles(), &["src/a.txt"]);

    let env = AgyEnv::new(
        &sb,
        json!({
            "lines": [
                active_tool(3, "write_file", json!({"Path": repo.join("src/other.txt").to_str().unwrap()})),
            ],
            "hang_s": 600,
        }),
    );
    let res = sb.run_env(&["run", "dev", "web"], None, &env.vars());
    assert_eq!(
        res.code, 3,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        res.stderr
            .contains("agy Dev dùng công cụ ngoài quyền của plan: write_file"),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(
        run_row(&sb, "dev").outcome.as_deref(),
        Some("protocol_violation")
    );
}

#[test]
fn agy_dev_allowed_tools_do_not_stop_the_run() {
    let sb = Sandbox::new();
    let repo = plan_task(&sb, dev_roles(), &["src/a.txt"]);

    let mut lines = vec![
        json!({"event": "init", "conversation_id": "c1",
               "init": {"cwd": repo.to_str().unwrap(), "permission_mode": "accept-edits"}}),
        active_tool(
            2,
            "read_file",
            json!({"Path": repo.join(".plan.md").to_str().unwrap(), "StartLine": 1}),
        ),
        active_tool(
            4,
            "write_file",
            json!({"Path": repo.join("src/a.txt").to_str().unwrap()}),
        ),
        // the same step again (its DONE echo, or a repeat) is not checked twice
        active_tool(
            4,
            "write_file",
            json!({"Path": repo.join("src/a.txt").to_str().unwrap()}),
        ),
        active_tool(
            6,
            "run_command",
            json!({"CommandLine": "git status --short"}),
        ),
        active_tool(
            8,
            "manage_task",
            json!({"Action": "status", "TaskId": "t-1"}),
        ),
        active_tool(10, "ask_question", json!({"Question": "?"})),
    ];
    lines.extend(
        agy_lines("xong", "SUCCESS", "accept-edits")
            .into_iter()
            .skip(1),
    );
    let (res, _env) = run_dev(
        &sb,
        json!({"lines": lines, "ops": [write_op("src/a.txt", "a\n")]}),
    );
    assert_ok(&res, "run dev (agy, allowed tools)");
    assert!(
        !res.stderr.contains("ngoài quyền của plan"),
        "stderr: {}",
        res.stderr
    );
    let row = run_row(&sb, "dev");
    assert_eq!(row.status, "succeeded");
    assert_ne!(row.outcome.as_deref(), Some("protocol_violation"));
    let files = sb.git(&repo, &["show", "--name-only", "--format=", "HEAD"]);
    assert!(files.lines().any(|l| l == "src/a.txt"), "{files}");
}

// ---------------------------------------------------------------------------
// A skill-mode plan (shaped like the `srt` video studio) run by an agy Dev: the plan's grants
// reach the run's policy and are checked step by step.
// ---------------------------------------------------------------------------

const STUDIO_GRANTS: &str = "\n## Skill sử dụng\n- `studio` (.agents/skills/studio/SKILL.md)\n\n## Lệnh được chạy\n- `.venv/bin/python scripts/build_video.py *`\n\n## Thư mục được đọc\n- `scripts/**`\n\n## Thư mục được ghi\n- `projects/x/**`\n";

/// Repo `web` with a studio-like layout and an approved plan (written by an agy Lead) that
/// lists `src/app.py` and declares `STUDIO_GRANTS`; Lead and Dev run on agy.
fn studio_task(sb: &Sandbox) -> PathBuf {
    agy_config(sb);
    write_doi_agy(sb, dev_roles());
    let repo = sb.make_repo(
        "web",
        &[
            ("src/app.py", "def app():\n    return 1\n"),
            ("scripts/build_video.py", "print('build')\n"),
            (
                ".agents/skills/studio/SKILL.md",
                "---\nname: studio\ndescription: dựng video\n---\n",
            ),
            ("projects/x/brief.md", "# x\n"),
        ],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web", "work");
    entry.as_object_mut().unwrap().remove("workflow");
    sb.write_map(json!([entry]), &[]);
    let mut plan = protocol::plan_text(
        "Kế hoạch",
        &[("src/app.py", "toàn file", "sửa", "demo")],
        "python3 -m unittest",
        "",
    );
    plan.push_str(STUDIO_GRANTS);
    let env = AgyEnv::new(
        sb,
        json!({"lines": agy_lines(&plan, "SUCCESS", "request-review")}),
    );
    let res = run_lead(sb, &env);
    assert_ok(&res, "run lead (agy, studio plan)");
    repo
}

fn studio_dev_lines(repo: &std::path::Path, steps: Vec<Value>) -> Vec<Value> {
    let mut lines = vec![json!({"event": "init", "conversation_id": "c1",
        "init": {"cwd": repo.to_str().unwrap(), "permission_mode": "accept-edits"}})];
    lines.extend(steps);
    lines
}

#[test]
fn agy_dev_skill_plan_gets_grants_and_runs_its_command() {
    let sb = Sandbox::new();
    let repo = studio_task(&sb);
    let p = |rel: &str| repo.join(rel).to_str().unwrap().to_string();

    let mut lines = studio_dev_lines(
        &repo,
        vec![
            active_tool(2, "read_file", json!({"Path": p("scripts/build_video.py")})),
            active_tool(3, "list_dir", json!({"DirectoryPath": p("scripts")})),
            active_tool(
                4,
                "run_command",
                json!({"CommandLine": ".venv/bin/python scripts/build_video.py --project projects/x --stage all"}),
            ),
            active_tool(6, "write_file", json!({"Path": p("projects/x/out.txt")})),
            active_tool(8, "edit_file", json!({"Path": p("src/app.py")})),
        ],
    );
    lines.extend(
        agy_lines("xong", "SUCCESS", "accept-edits")
            .into_iter()
            .skip(1),
    );
    let (res, env) = run_dev(
        &sb,
        json!({"lines": lines, "ops": [
            write_op("src/app.py", "def app():\n    return 2\n"),
            write_op("projects/x/out.txt", "video\n"),
        ]}),
    );
    assert_ok(&res, "run dev (agy, studio plan)");
    assert!(
        !res.stderr.contains("ngoài quyền của plan"),
        "stderr: {}",
        res.stderr
    );
    assert!(
        !res.stdout.contains("chỉ áp dụng cho engine"),
        "stdout: {}",
        res.stdout
    );

    // the input carries the grants note; no claude allow-rule reaches agy
    let recs = env.records();
    let input = argv_of(recs.last().unwrap()).join("\n");
    assert!(input.contains("Plan có khai báo Lệnh được chạy"), "{input}");
    assert!(input.contains("command not found"), "{input}");
    assert!(!input.contains("Bash("), "{input}");

    let row = run_row(&sb, "dev");
    assert_eq!(row.status, "succeeded");
    assert_eq!(row.outcome, None);
    let policy: Value = serde_json::from_str(
        &std::fs::read_to_string(PathBuf::from(&row.run_dir).join("policy.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        policy["bash_allow"],
        json!([".venv/bin/python scripts/build_video.py *"])
    );
    assert!(
        policy["read_dirs"][0]
            .as_str()
            .unwrap()
            .ends_with("/scripts"),
        "{policy}"
    );
    assert!(
        policy["write_dirs"][0]
            .as_str()
            .unwrap()
            .ends_with("/projects/x"),
        "{policy}"
    );

    // the write under the declared dir is committed with the plan file
    let files = sb.git(&repo, &["show", "--name-only", "--format=", "HEAD"]);
    assert!(files.lines().any(|l| l == "projects/x/out.txt"), "{files}");
    assert!(files.lines().any(|l| l == "src/app.py"), "{files}");
}

#[test]
fn agy_dev_skill_plan_still_stops_other_commands_and_writes() {
    for (step, needle) in [
        (
            json!({"CommandLine": "python3 -c 'x'"}),
            "run_command {\"CommandLine\":\"python3 -c 'x'\"}",
        ),
        (
            json!({"CommandLine": "cat << 'EOF' > /tmp/a.py\nimport os\nEOF"}),
            "run_command",
        ),
    ] {
        let sb = Sandbox::new();
        let repo = studio_task(&sb);
        let lines = studio_dev_lines(&repo, vec![active_tool(4, "run_command", step)]);
        let (res, _env) = run_dev(&sb, json!({"lines": lines, "hang_s": 600}));
        assert_eq!(
            res.code, 3,
            "stderr: {}\nstdout: {}",
            res.stderr, res.stdout
        );
        assert!(
            res.stderr.contains(&format!(
                "agy Dev dùng công cụ ngoài quyền của plan: {needle}"
            )),
            "stderr: {}",
            res.stderr
        );
        assert_eq!(
            run_row(&sb, "dev").outcome.as_deref(),
            Some("protocol_violation")
        );
    }

    // a write into the read-only directory
    let sb = Sandbox::new();
    let repo = studio_task(&sb);
    let lines = studio_dev_lines(
        &repo,
        vec![active_tool(
            6,
            "write_file",
            json!({"Path": repo.join("scripts/a.py").to_str().unwrap()}),
        )],
    );
    let (res, _env) = run_dev(&sb, json!({"lines": lines, "hang_s": 600}));
    assert_eq!(
        res.code, 3,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        res.stderr
            .contains("agy Dev dùng công cụ ngoài quyền của plan: write_file"),
        "stderr: {}",
        res.stderr
    );
    assert_eq!(
        run_row(&sb, "dev").outcome.as_deref(),
        Some("protocol_violation")
    );
}

#[test]
fn agy_dev_without_grants_still_allows_only_git() {
    let sb = Sandbox::new();
    let repo = plan_task(&sb, dev_roles(), &["src/a.txt"]);
    let lines = studio_dev_lines(
        &repo,
        vec![active_tool(
            4,
            "run_command",
            json!({"CommandLine": ".venv/bin/python scripts/build_video.py --stage all"}),
        )],
    );
    let (res, _env) = run_dev(&sb, json!({"lines": lines, "hang_s": 600}));
    assert_eq!(
        res.code, 3,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        res.stderr
            .contains("lệnh không nằm trong danh sách Lệnh được chạy"),
        "stderr: {}",
        res.stderr
    );
    let row = run_row(&sb, "dev");
    let policy: Value = serde_json::from_str(
        &std::fs::read_to_string(PathBuf::from(&row.run_dir).join("policy.json")).unwrap(),
    )
    .unwrap();
    assert!(policy.get("bash_allow").is_none(), "{policy}");
}

// ---------------------------------------------------------------------------
// C4: agy workspace root, wrong cwd guard, Dev engine preview for the Lead, project pins
// ---------------------------------------------------------------------------

fn canon(p: &str) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p))
}

/// `web` reads the source project `lib`; Lead and Dev on agy; the plan lists `src/a.txt`.
fn plan_task_with_source(sb: &Sandbox) -> (PathBuf, PathBuf) {
    agy_config(sb);
    write_doi_agy(sb, dev_roles());
    let repo = sb.make_repo(
        "web",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let src = sb.make_repo("lib", &[("lib.py", "X = 1\n")], None, None);
    let mut entry = sb.project_entry(&repo, "web", "work");
    entry.as_object_mut().unwrap().remove("workflow");
    entry["read_sources"] = json!(["lib"]);
    let mut src_entry = sb.project_entry(&src, "lib", "work");
    src_entry.as_object_mut().unwrap().remove("workflow");
    sb.write_map(json!([entry, src_entry]), &[]);
    let plan = protocol::plan_text(
        "Kế hoạch",
        &[("src/a.txt", "toàn file", "sửa", "demo")],
        "python3 -m unittest",
        "",
    );
    let env = AgyEnv::new(
        sb,
        json!({"lines": agy_lines(&plan, "SUCCESS", "request-review")}),
    );
    let res = run_lead(sb, &env);
    assert_ok(&res, "run lead (agy, plan with source)");
    (repo, src)
}

#[test]
fn agy_dev_workspace_root_comes_before_sources_and_init_cwd_matches() {
    let sb = Sandbox::new();
    let (repo, src) = plan_task_with_source(&sb);

    let (res, env) = run_dev(&sb, json!({"ops": [write_op("src/a.txt", "a\n")]}));
    assert_ok(&res, "run dev (agy, with source)");

    let recs = env.records();
    assert_eq!(recs.len(), 1);
    let add_dirs: Vec<PathBuf> = recs[0]["add_dirs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| canon(v.as_str().unwrap()))
        .collect();
    let repo_c = canon(repo.to_str().unwrap());
    let src_c = canon(src.to_str().unwrap());
    assert_eq!(add_dirs.first(), Some(&repo_c), "root is not first: {add_dirs:?}");
    let src_at = add_dirs.iter().position(|d| d == &src_c);
    assert!(src_at.is_some_and(|i| i > 0), "source missing or before root: {add_dirs:?}");
    // the real working directory of agy is the project root, and so is init.cwd in the stream
    assert_eq!(canon(recs[0]["cwd"].as_str().unwrap()), repo_c);
    let row = run_row(&sb, "dev");
    let stream =
        std::fs::read_to_string(PathBuf::from(&row.run_dir).join("stream.jsonl")).unwrap();
    let init: Value = stream
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["event"] == "init")
        .expect("init event in stream");
    assert_eq!(canon(init["init"]["cwd"].as_str().unwrap()), repo_c);
    assert_eq!(row.status, "succeeded");
    assert_ne!(row.outcome.as_deref(), Some("protocol_violation"));
    assert!(row.metrics.get("violation").is_none(), "{}", row.metrics);
}

#[test]
fn agy_dev_reporting_another_cwd_is_a_wrong_cwd_violation() {
    let sb = Sandbox::new();
    let repo = plan_task(&sb, dev_roles(), &["src/a.txt"]);
    let elsewhere = sb.root.join("noi-khac");
    std::fs::create_dir_all(&elsewhere).unwrap();

    let env = AgyEnv::new(&sb, json!({"ops": [write_op("src/a.txt", "a\n")]}));
    let mut vars = env.vars();
    vars.push(("FAKE_AGY_CWD", elsewhere.to_str().unwrap()));
    let res = sb.run_env(&["run", "dev", "web"], None, &vars);
    assert_eq!(res.code, 3, "stderr: {}\nstdout: {}", res.stderr, res.stdout);
    assert!(
        res.stderr.contains("không phải gốc dự án"),
        "stderr: {}",
        res.stderr
    );
    let row = run_row(&sb, "dev");
    assert_eq!(row.outcome.as_deref(), Some("protocol_violation"));
    assert_eq!(row.metrics["violation"]["kind"], json!("wrong_cwd"), "{}", row.metrics);
    assert_eq!(
        canon(row.metrics["violation"]["cwd"].as_str().unwrap()),
        canon(elsewhere.to_str().unwrap())
    );
    let err = dev_error(&sb).unwrap_or_default();
    assert!(err.contains("không phải gốc dự án"), "runs.error: {err}");
    // the run was stopped before anything was committed
    let subject = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(subject.trim(), "sua-app: plan");
}

fn lead_prompt(sb: &Sandbox) -> String {
    let row = run_row(sb, "lead");
    std::fs::read_to_string(PathBuf::from(&row.run_dir).join("prompt.md")).unwrap()
}

#[test]
fn lead_prompt_names_the_dev_engine_and_its_note() {
    let sb = Sandbox::new();
    plan_task(&sb, dev_roles(), &["src/a.txt"]);
    let prompt = lead_prompt(&sb);
    assert!(
        prompt.contains("Dev dự kiến chạy bằng engine agy (agy)."),
        "{prompt}"
    );
    let note = std::fs::read_to_string(
        common::repo_root().join("agent-brain/assets/prompts/engine-notes/agy.md"),
    )
    .unwrap();
    assert!(prompt.contains(note.lines().next().unwrap()), "{prompt}");
    // the preview wrote nothing: only the Lead run exists
    let runs: i64 = sb
        .db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(runs, 1);
}

#[test]
fn lead_prompt_without_engine_note_for_claude_dev() {
    let sb = Sandbox::new();
    plan_task(&sb, json!({"lead": "agy", "dev": "claude"}), &["src/a.txt"]);
    let prompt = lead_prompt(&sb);
    assert!(
        prompt.contains("Dev dự kiến chạy bằng engine anthropic (claude)."),
        "{prompt}"
    );
    assert!(!prompt.contains("Lưu ý khi Dev chạy bằng agy"), "{prompt}");
}

/// Adds `role_engines` to project `web` of the map.
fn pin_engine(sb: &Sandbox, role: &str, engine: &str) {
    let path = sb.map_path();
    let mut map: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for p in map["projects"].as_array_mut().unwrap() {
        if p["id"] == "web" {
            p["role_engines"] = json!({role: engine});
        }
    }
    std::fs::write(&path, serde_json::to_string_pretty(&map).unwrap()).unwrap();
}

#[test]
fn project_engine_pin_wins_over_the_team() {
    let sb = Sandbox::new();
    let repo = plan_task(&sb, json!({"lead": "agy", "dev": "claude"}), &["src/a.txt"]);
    pin_engine(&sb, "dev", "agy");

    let (res, env) = run_dev(&sb, json!({"ops": [write_op("src/a.txt", "a\n")]}));
    assert_ok(&res, "run dev (pinned to agy)");
    assert_eq!(run_row(&sb, "dev").engine, "agy");
    assert_eq!(env.records().len(), 1);
    assert!(repo.join("src/a.txt").is_file());
}

#[test]
fn project_pin_not_ready_refuses_without_fallback() {
    let sb = Sandbox::new();
    plan_task(&sb, dev_roles(), &["src/a.txt"]);
    sb.write_config(json!({"engines": [
        {"id": "agy", "kind": "agy", "command": fake_agy().to_str().unwrap()},
        {"id": "agy-hong", "kind": "agy", "command": "/khong/co/agy-hong"}
    ]}));
    pin_engine(&sb, "dev", "agy-hong");

    let (res, env) = run_dev(&sb, json!({"ops": [write_op("src/a.txt", "a\n")]}));
    assert_ne!(res.code, 0, "stdout: {}", res.stdout);
    assert!(
        res.stderr.contains("agy-hong") && res.stderr.contains("chưa sẵn sàng"),
        "stderr: {}",
        res.stderr
    );
    assert!(env.records().is_empty(), "agy ran anyway");
    let dev_runs: i64 = sb
        .db()
        .query_row("SELECT COUNT(*) FROM runs WHERE role = 'dev'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(dev_runs, 0);
}

#[test]
fn role_pins_api_saves_validates_and_checks_etag() {
    let sb = Sandbox::new();
    agy_config(&sb);
    let _repo = setup_project(&sb, "work");
    let srv = sb.serve(&[]);

    let resp = srv.put_json(
        "/api/projects/web/role-pins",
        &json!({"role_engines": {"dev": "agy"}}),
    );
    assert_eq!(resp.status, 200, "{}", resp.text());
    let web = resp.json()["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "web")
        .cloned()
        .unwrap();
    assert_eq!(web["role_engines"], json!({"dev": "agy"}));
    assert_eq!(web["role_members"], json!({}));

    for bad in [
        json!({"role_engines": {"dev": "agy"}, "role_members": {"dev": "ai-do"}}),
        json!({"role_engines": {"khac": "agy"}}),
        json!({"role_engines": {"dev": "khong-co"}}),
        json!({"role_members": {"dev": "khong-co"}}),
    ] {
        let resp = srv.put_json("/api/projects/web/role-pins", &bad);
        assert_eq!(resp.status, 400, "{bad}: {}", resp.text());
    }

    let resp = srv.put_json(
        "/api/projects/web/role-pins",
        &json!({"role_engines": {}, "etag": "cu"}),
    );
    assert_eq!(resp.status, 409, "{}", resp.text());

    let resp = srv.put_json("/api/projects/web/role-pins", &json!({"role_engines": {}}));
    assert_eq!(resp.status, 200, "{}", resp.text());
    let map: Value = serde_json::from_str(
        &std::fs::read_to_string(sb.map_path()).unwrap(),
    )
    .unwrap();
    let web = map["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "web")
        .unwrap();
    assert!(web.get("role_engines").is_none(), "{web}");
}
