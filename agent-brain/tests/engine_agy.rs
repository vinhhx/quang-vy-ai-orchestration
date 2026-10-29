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
    assert_eq!(&argv[argv.len() - 2..], ["--mode", "accept-edits"]);
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
