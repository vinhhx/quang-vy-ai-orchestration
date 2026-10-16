use std::path::Path;

#[test]
fn prompts_equal_phase2() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

    for role in ["lead", "dev", "qa"] {
        let rust_asset_path = repo_root.join(format!("agent-brain/assets/prompts/{role}.md"));
        let python_prompt_path = repo_root.join(format!("brain/prompts/{role}.md"));

        let rust_asset_bytes = std::fs::read(&rust_asset_path).expect("read rust asset prompt");
        let python_prompt_bytes = std::fs::read(&python_prompt_path).expect("read python prompt");

        assert_eq!(
            rust_asset_bytes, python_prompt_bytes,
            "Prompt file bytes mismatch for role '{role}'"
        );

        let constant_bytes = match role {
            "lead" => agent_brain::roles::LEAD_PROMPT.as_bytes(),
            "dev" => agent_brain::roles::DEV_PROMPT.as_bytes(),
            "qa" => agent_brain::roles::QA_PROMPT.as_bytes(),
            _ => unreachable!(),
        };

        assert_eq!(
            constant_bytes, python_prompt_bytes,
            "Embedded prompt constant mismatch for role '{role}'"
        );
    }
}

#[test]
fn templates_equal_phase2() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

    let plan_rust_asset = repo_root.join("agent-brain/assets/templates/plan.md");
    let plan_python = repo_root.join("brain/templates/plan.md");
    let plan_rust_bytes = std::fs::read(&plan_rust_asset).expect("read rust plan template");
    let plan_python_bytes = std::fs::read(&plan_python).expect("read python plan template");

    assert_eq!(plan_rust_bytes, plan_python_bytes);
    assert_eq!(
        agent_brain::planfile::PLAN_TEMPLATE.as_bytes(),
        plan_python_bytes
    );

    let qa_rust_asset = repo_root.join("agent-brain/assets/templates/qa-report.md");
    let qa_python = repo_root.join("brain/templates/qa-report.md");
    let qa_rust_bytes = std::fs::read(&qa_rust_asset).expect("read rust qa-report template");
    let qa_python_bytes = std::fs::read(&qa_python).expect("read python qa-report template");

    assert_eq!(qa_rust_bytes, qa_python_bytes);
    assert_eq!(
        agent_brain::planfile::QA_TEMPLATE.as_bytes(),
        qa_python_bytes
    );
}

#[test]
fn default_config_matches_phase2() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

    let rust_config_path = repo_root.join("agent-brain/assets/config.default.json");
    let python_config_path = repo_root.join("brain/config.json");

    let rust_cfg_str =
        std::fs::read_to_string(&rust_config_path).expect("read config.default.json");
    let python_cfg_str =
        std::fs::read_to_string(&python_config_path).expect("read brain/config.json");

    let rust_cfg: serde_json::Value =
        serde_json::from_str(&rust_cfg_str).expect("parse config.default.json");
    let python_cfg: serde_json::Value =
        serde_json::from_str(&python_cfg_str).expect("parse brain/config.json");

    assert_eq!(rust_cfg["claude_bin"], python_cfg["claude_bin"]);
    assert_eq!(rust_cfg["models"], python_cfg["models"]);
    assert_eq!(rust_cfg["profiles"], python_cfg["profiles"]);
    assert_eq!(rust_cfg["common_args"], python_cfg["common_args"]);

    assert_eq!(
        rust_cfg["timeout_seconds"]["lead"],
        python_cfg["timeout_seconds"]["lead"]
    );
    assert_eq!(
        rust_cfg["timeout_seconds"]["dev"],
        python_cfg["timeout_seconds"]["dev"]
    );
    assert_eq!(
        rust_cfg["timeout_seconds"]["qa"],
        python_cfg["timeout_seconds"]["qa"]
    );
}
