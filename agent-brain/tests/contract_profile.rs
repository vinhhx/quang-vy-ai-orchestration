use agent_brain::config::{config_from_value, ProfileCfg};
use agent_brain::profile::{build_env, check_profile};
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn p1_10_profile_env_contract() {
    let fixture_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/contract/profile_env.json");
    let fixture_str = std::fs::read_to_string(&fixture_path).expect("read profile_env.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&fixture_str).expect("parse profile_env.json");

    let profiles: BTreeMap<String, ProfileCfg> =
        serde_json::from_value(fixture["profiles"].clone()).unwrap();

    for case in fixture["cases"].as_array().unwrap() {
        let case_id = case["id"].as_str().unwrap();
        let prof_str = case.get("profile").and_then(|p| p.as_str()).unwrap_or("");
        let base_env: BTreeMap<String, String> = case["base_env"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
            .collect();
        let expect = &case["expect"];

        if let Some(err_type) = expect.get("error").and_then(|e| e.as_str()) {
            assert_eq!(err_type, "profile");
            let res = build_env(prof_str, &profiles, &base_env);
            assert!(res.is_err(), "Case {} should fail build_env", case_id);
            assert_eq!(res.unwrap_err().code, 1);
            let check_res = check_profile(prof_str, &profiles);
            assert!(
                check_res.is_err(),
                "Case {} should fail check_profile",
                case_id
            );
            assert_eq!(check_res.unwrap_err().code, 1);
        } else {
            let env = build_env(prof_str, &profiles, &base_env).expect("build_env failed");
            if let Some(absent) = expect.get("absent").and_then(|a| a.as_array()) {
                for k in absent {
                    let var = k.as_str().unwrap();
                    assert!(
                        !env.contains_key(var),
                        "Case {}: expected var '{}' to be absent",
                        case_id,
                        var
                    );
                }
            }
            if let Some(equals) = expect.get("equals").and_then(|e| e.as_object()) {
                for (k, v) in equals {
                    let val = v.as_str().unwrap();
                    assert_eq!(
                        env.get(k).map(|s| s.as_str()),
                        Some(val),
                        "Case {}: expected var '{}' == '{}', got {:?}",
                        case_id,
                        k,
                        val,
                        env.get(k)
                    );
                }
            }
            assert_eq!(
                env.get("CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR")
                    .map(|s| s.as_str()),
                Some("1"),
                "Case {}: CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR should be 1",
                case_id
            );
        }
    }
}

#[test]
fn config_validation_messages() {
    let res = config_from_value(&serde_json::json!({}));
    assert_eq!(res.unwrap_err().message, "Cấu hình thiếu trường 'models'");

    let res = config_from_value(&serde_json::json!({
        "models": { "lead": "opus", "qa": "sonnet" },
        "profiles": { "work": {}, "personal": {} }
    }));
    assert_eq!(
        res.unwrap_err().message,
        "Cấu hình thiếu model cho vai trò 'dev'"
    );

    let res = config_from_value(&serde_json::json!({
        "models": { "lead": "opus", "dev": "sonnet", "qa": "sonnet" }
    }));
    assert_eq!(res.unwrap_err().message, "Cấu hình thiếu trường 'profiles'");

    let res = config_from_value(&serde_json::json!({
        "models": { "lead": "opus", "dev": "sonnet", "qa": "sonnet" },
        "profiles": { "work": {} }
    }));
    assert_eq!(
        res.unwrap_err().message,
        "Cấu hình thiếu profile 'personal'"
    );

    let cfg = config_from_value(&serde_json::json!({
        "models": { "lead": "opus", "dev": "sonnet", "qa": "sonnet" },
        "profiles": { "work": {}, "personal": {} }
    }))
    .unwrap();
    assert_eq!(cfg.inspect_model, "sonnet");
    assert_eq!(cfg.kill_grace_secs, 3);
    assert_eq!(cfg.inspect_max_diff_bytes, 100_000);
}

#[test]
fn build_env_tilde_normpath() {
    // Review finding 2: normalise tilde-expanded config dir like Python's
    // os.path.normpath(os.path.join(home, dir[1:].lstrip("/\\")))
    let mut profiles = BTreeMap::new();
    profiles.insert(
        "trailing".to_string(),
        ProfileCfg {
            claude_config_dir: Some("~/.claude-orca/".to_string()),
            unset_env: vec![],
        },
    );
    profiles.insert(
        "traversal".to_string(),
        ProfileCfg {
            claude_config_dir: Some("~/a/../b".to_string()),
            unset_env: vec![],
        },
    );
    profiles.insert(
        "home_trailing".to_string(),
        ProfileCfg {
            claude_config_dir: Some("~/.claude-orca".to_string()),
            unset_env: vec![],
        },
    );

    let base_env_normal = BTreeMap::from([("HOME".to_string(), "/home/u".to_string())]);
    let env1 = build_env("trailing", &profiles, &base_env_normal).unwrap();
    assert_eq!(
        env1.get("CLAUDE_CONFIG_DIR").unwrap(),
        "/home/u/.claude-orca"
    );

    let env2 = build_env("traversal", &profiles, &base_env_normal).unwrap();
    assert_eq!(env2.get("CLAUDE_CONFIG_DIR").unwrap(), "/home/u/b");

    let base_env_slash = BTreeMap::from([("HOME".to_string(), "/h/".to_string())]);
    let env3 = build_env("home_trailing", &profiles, &base_env_slash).unwrap();
    assert_eq!(env3.get("CLAUDE_CONFIG_DIR").unwrap(), "/h/.claude-orca");
}
