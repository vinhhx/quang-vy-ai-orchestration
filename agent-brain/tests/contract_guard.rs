mod common;

use std::fs;
use std::path::PathBuf;
use unicode_normalization::UnicodeNormalization;

const EXPECTED_IDS: &[&str] = &[
    "G-read-allowed",
    "G-read-denied",
    "G-relative-path",
    "G-parent-traversal",
    "G-symlink-outside",
    "G-edit-allowed",
    "G-write-new-outside",
    "G-multiedit-allowed",
    "G-multiedit-blocked",
    "G-notebookedit-allowed",
    "G-notebookedit-blocked",
    "G-grep-blocked",
    "G-glob-blocked",
    "G-readfree-read-any",
    "G-readfree-grep",
    "G-readfree-write-outside",
    "G-bash-not-guarded",
    "G-malformed-stdin",
    "G-missing-policy",
    "G-write-missing-path",
    "G-nfd-path",
];

fn substitute(val: &serde_json::Value, root_str: &str, outside_str: &str) -> serde_json::Value {
    match val {
        serde_json::Value::String(s) => serde_json::Value::String(
            s.replace("{root}", root_str)
                .replace("{outside}", outside_str),
        ),
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|item| substitute(item, root_str, outside_str))
                .collect(),
        ),
        serde_json::Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (k, v) in map {
                new_map.insert(k.clone(), substitute(v, root_str, outside_str));
            }
            serde_json::Value::Object(new_map)
        }
        other => other.clone(),
    }
}

#[test]
fn guard_fixture_ids() {
    let contract_path = common::repo_root().join("tests/contract/guard.json");
    let content = fs::read_to_string(&contract_path).expect("read guard.json");
    let data: serde_json::Value = serde_json::from_str(&content).expect("parse guard.json");
    let cases = data
        .get("cases")
        .and_then(|v| v.as_array())
        .expect("cases array");
    let ids: Vec<&str> = cases
        .iter()
        .filter_map(|c| c.get("id").and_then(|v| v.as_str()))
        .collect();
    assert_eq!(ids, EXPECTED_IDS);
}

#[test]
fn guard_contract_cases() {
    let contract_path = common::repo_root().join("tests/contract/guard.json");
    let content = fs::read_to_string(&contract_path).expect("read guard.json");
    let data: serde_json::Value = serde_json::from_str(&content).expect("parse guard.json");

    let layout = data.get("layout").expect("layout");
    let default_policy = data.get("default_policy").expect("default_policy");
    let cases = data.get("cases").and_then(|v| v.as_array()).expect("cases");

    for case in cases {
        let cid = case.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let temp_path = temp_dir.path().canonicalize().expect("canonicalize");
        let root = temp_path.join("root");
        let outside = temp_path.join("outside");
        fs::create_dir_all(&root).expect("create root");
        fs::create_dir_all(&outside).expect("create outside");

        if let Some(files) = layout.get("files").and_then(|v| v.as_object()) {
            for (rel_name, text_val) in files {
                let target_file = root.join(rel_name);
                fs::create_dir_all(target_file.parent().unwrap()).expect("create parent");
                fs::write(&target_file, text_val.as_str().unwrap()).expect("write file");
            }
        }

        if let Some(outside_files) = layout.get("outside_files").and_then(|v| v.as_object()) {
            for (rel_name, text_val) in outside_files {
                let target_file = outside.join(rel_name);
                fs::create_dir_all(target_file.parent().unwrap()).expect("create parent");
                fs::write(&target_file, text_val.as_str().unwrap()).expect("write file");
            }
        }

        if let Some(symlinks) = layout.get("symlinks").and_then(|v| v.as_object()) {
            for (link_name, target_tpl) in symlinks {
                let target_str = target_tpl
                    .as_str()
                    .unwrap()
                    .replace("{root}", &root.to_string_lossy())
                    .replace("{outside}", &outside.to_string_lossy());
                let link_file = root.join(link_name);
                fs::create_dir_all(link_file.parent().unwrap()).expect("create parent");
                #[cfg(unix)]
                std::os::unix::fs::symlink(target_str, link_file).expect("symlink");
            }
        }

        let case_policy = case.get("policy").unwrap_or(default_policy);
        let log_path = temp_path.join("guard.jsonl");

        let policy_file = if let Some(p) = case.get("policy_path").and_then(|v| v.as_str()) {
            PathBuf::from(
                p.replace("{root}", &root.to_string_lossy())
                    .replace("{outside}", &outside.to_string_lossy()),
            )
        } else {
            let p_file = temp_path.join("policy.json");
            let read_allow: Option<Vec<String>> = case_policy.get("read_allow").and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    Some(
                        v.as_array()
                            .unwrap()
                            .iter()
                            .map(|e| root.join(e.as_str().unwrap()).to_string_lossy().to_string())
                            .collect(),
                    )
                }
            });
            let write_allow: Vec<String> = case_policy
                .get("write_allow")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|e| root.join(e.as_str().unwrap()).to_string_lossy().to_string())
                        .collect()
                })
                .unwrap_or_default();

            let policy_data = serde_json::json!({
                "root": root.to_string_lossy(),
                "read_allow": read_allow,
                "write_allow": write_allow,
                "log": log_path.to_string_lossy(),
            });
            fs::write(&p_file, serde_json::to_string_pretty(&policy_data).unwrap())
                .expect("write policy.json");
            p_file
        };

        let stdin_data = if let Some(raw) = case.get("stdin_raw").and_then(|v| v.as_str()) {
            raw.replace("{root}", &root.to_string_lossy())
                .replace("{outside}", &outside.to_string_lossy())
        } else {
            let mut payload = substitute(
                case.get("payload").unwrap(),
                &root.to_string_lossy(),
                &outside.to_string_lossy(),
            );
            if case.get("normalize_payload_path").and_then(|v| v.as_str()) == Some("NFD") {
                if let Some(tool_input) = payload
                    .get_mut("tool_input")
                    .and_then(|v| v.as_object_mut())
                {
                    if let Some(fp) = tool_input.get("file_path").and_then(|v| v.as_str()) {
                        let nfd: String = fp.nfd().collect();
                        tool_input.insert("file_path".to_string(), serde_json::Value::String(nfd));
                    }
                }
            }
            serde_json::to_string(&payload).unwrap()
        };

        let db_path = temp_path.join("guard_test.db");
        let mut cmd = std::process::Command::new(common::bin());
        cmd.arg("guard")
            .arg("--policy")
            .arg(&policy_file)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", std::env::var("HOME").unwrap_or_default())
            .env("AGENT_BRAIN_DB", &db_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().expect("spawn guard");
        {
            use std::io::Write;
            let mut stdin = child.stdin.take().expect("stdin");
            stdin.write_all(stdin_data.as_bytes()).expect("write stdin");
        }
        let output = child.wait_with_output().expect("wait output");

        let expect = case.get("expect").expect("expect block");
        let expected_exit = expect.get("exit").and_then(|v| v.as_i64()).unwrap() as i32;
        assert_eq!(
            output.status.code(),
            Some(expected_exit),
            "Case {cid}: exit code mismatch. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        if let Some(stderr_contains) = expect.get("stderr_contains").and_then(|v| v.as_array()) {
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            for fragment in stderr_contains {
                let frag_str = fragment.as_str().unwrap();
                assert!(
                    stderr_str.contains(frag_str),
                    "Case {cid}: stderr '{}' missing fragment '{}'",
                    stderr_str,
                    frag_str
                );
            }
        }

        if let Some(expected_decision) = expect.get("decision").and_then(|v| v.as_str()) {
            assert!(
                log_path.is_file(),
                "Case {cid}: expected log file {log_path:?} to exist"
            );
            let log_content = fs::read_to_string(&log_path).expect("read log file");
            let lines: Vec<&str> = log_content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .collect();
            assert!(
                !lines.is_empty(),
                "Case {cid}: expected log records in {log_path:?}"
            );
            let last_record: serde_json::Value =
                serde_json::from_str(lines.last().unwrap()).expect("parse log json");
            assert_eq!(
                last_record.get("decision").and_then(|v| v.as_str()),
                Some(expected_decision),
                "Case {cid}: decision mismatch"
            );
            if let Some(expected_tool) = expect.get("tool").and_then(|v| v.as_str()) {
                assert_eq!(
                    last_record.get("tool").and_then(|v| v.as_str()),
                    Some(expected_tool),
                    "Case {cid}: tool mismatch"
                );
            }
        }
    }
}

#[test]
fn guard_never_opens_db() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let temp_path = temp_dir.path().canonicalize().expect("canonicalize");
    let db_path = temp_path.join("guard_never_opens.db");
    let policy_file = temp_path.join("policy.json");
    let policy_data = serde_json::json!({
        "root": temp_path.to_string_lossy(),
        "read_allow": null,
        "write_allow": [],
        "log": temp_path.join("guard.jsonl").to_string_lossy(),
    });
    fs::write(&policy_file, serde_json::to_string(&policy_data).unwrap())
        .expect("write policy.json");

    assert!(
        !db_path.exists(),
        "DB path must not exist before guard runs"
    );

    let mut cmd = std::process::Command::new(common::bin());
    cmd.arg("guard")
        .arg("--policy")
        .arg(&policy_file)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", std::env::var("HOME").unwrap_or_default())
        .env("AGENT_BRAIN_DB", &db_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().expect("spawn guard");
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("stdin");
        let payload = serde_json::json!({
            "tool_name": "Read",
            "tool_input": {"file_path": "foo.py"},
            "cwd": temp_path.to_string_lossy(),
        });
        stdin
            .write_all(serde_json::to_string(&payload).unwrap().as_bytes())
            .expect("write stdin");
    }
    let output = child.wait_with_output().expect("wait output");
    assert_eq!(output.status.code(), Some(0));
    assert!(!db_path.exists(), "DB path must not exist after guard runs");
}

#[test]
fn hook_settings_shape_and_quoting() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let temp_path = temp_dir.path().canonicalize().expect("canonicalize");
    let root = temp_path.join("root");
    fs::create_dir_all(root.join("src")).expect("create src");
    let app_py = root.join("src").join("app.py");
    fs::write(&app_py, "def app(): pass\n").expect("write app.py");
    let other_py = root.join("other.py");
    fs::write(&other_py, "# other\n").expect("write other.py");

    let run_dir = temp_path.join("run dir 'q'");
    let exe = common::bin();

    let settings_path = agent_brain::guard::write_hook_settings(
        &run_dir,
        &root,
        Some(&[app_py.clone()]),
        &[app_py.clone()],
        &exe,
    )
    .expect("write_hook_settings");

    assert!(settings_path.is_file());
    let settings_content = fs::read_to_string(&settings_path).expect("read settings.json");
    let settings_json: serde_json::Value =
        serde_json::from_str(&settings_content).expect("parse settings.json");

    let hooks = settings_json
        .get("hooks")
        .and_then(|h| h.get("PreToolUse"))
        .and_then(|v| v.as_array())
        .expect("PreToolUse array");
    assert_eq!(hooks.len(), 1);
    assert_eq!(
        hooks[0].get("matcher").and_then(|v| v.as_str()),
        Some("Read|Edit|Write|MultiEdit|NotebookEdit|Grep|Glob|Bash")
    );
    let inner_hooks = hooks[0]
        .get("hooks")
        .and_then(|v| v.as_array())
        .expect("inner hooks array");
    assert_eq!(inner_hooks.len(), 1);
    assert_eq!(
        inner_hooks[0].get("type").and_then(|v| v.as_str()),
        Some("command")
    );
    let cmd_str = inner_hooks[0]
        .get("command")
        .and_then(|v| v.as_str())
        .expect("command string");

    let policy_path = run_dir.join("policy.json");
    assert!(policy_path.is_file());
    let policy_content = fs::read_to_string(&policy_path).expect("read policy.json");
    let policy_json: serde_json::Value =
        serde_json::from_str(&policy_content).expect("parse policy.json");
    assert_eq!(
        policy_json.get("log").and_then(|v| v.as_str()),
        Some(run_dir.join("guard.jsonl").to_string_lossy().as_ref())
    );

    // Run command string with sh -c and Read payload for src/app.py -> exits 0
    let payload_allowed = serde_json::json!({
        "tool_name": "Read",
        "tool_input": {"file_path": app_py.to_string_lossy()},
        "cwd": root.to_string_lossy(),
    });
    let mut child = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd_str)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn sh -c");
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("stdin");
        stdin
            .write_all(serde_json::to_string(&payload_allowed).unwrap().as_bytes())
            .expect("write stdin");
    }
    let res = child.wait_with_output().expect("wait output");
    assert_eq!(res.status.code(), Some(0));

    // Run command string with sh -c and Read payload for other.py -> exits 2
    let payload_blocked = serde_json::json!({
        "tool_name": "Read",
        "tool_input": {"file_path": other_py.to_string_lossy()},
        "cwd": root.to_string_lossy(),
    });
    let mut child = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd_str)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn sh -c");
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("stdin");
        stdin
            .write_all(serde_json::to_string(&payload_blocked).unwrap().as_bytes())
            .expect("write stdin");
    }
    let res = child.wait_with_output().expect("wait output");
    assert_eq!(res.status.code(), Some(2));
}

#[test]
fn read_free_mode_when_read_allow_none() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let temp_path = temp_dir.path().canonicalize().expect("canonicalize");
    let root = temp_path.join("root");
    fs::create_dir_all(&root).expect("create root");
    let run_dir = temp_path.join("run");
    let exe = common::bin();

    agent_brain::guard::write_hook_settings(&run_dir, &root, None, &[], &exe)
        .expect("write_hook_settings");

    let policy_path = run_dir.join("policy.json");
    let policy_content = fs::read_to_string(&policy_path).expect("read policy.json");
    let policy_json: serde_json::Value =
        serde_json::from_str(&policy_content).expect("parse policy.json");
    assert!(policy_json.get("read_allow").unwrap().is_null());
}
