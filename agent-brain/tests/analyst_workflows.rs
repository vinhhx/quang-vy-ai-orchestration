use agent_brain::workflows::{load_from, validate_entry, STAGES};
use std::collections::BTreeMap;

fn test_cfg() -> agent_brain::config::Config {
    agent_brain::config::config_from_value(
        &serde_json::from_str(agent_brain::config::DEFAULT_CONFIG_JSON).unwrap(),
    )
    .unwrap()
}

#[test]
fn builtins_phan_tich_and_phan_tich_review_valid() {
    let cfg = test_cfg();
    let loaded = load_from(None, &cfg);
    assert!(loaded.errors.is_empty());

    let pt = loaded.get("phan-tich").expect("phan-tich workflow exists");
    assert_eq!(pt.kind, "analysis");
    assert!(!pt.review);
    assert!(!pt.auto);
    assert!(pt.stages.is_empty());
    assert!(pt.loops.is_empty());
    assert_eq!(pt.summary(), "phân tích");

    let ptr = loaded.get("phan-tich-review").expect("phan-tich-review workflow exists");
    assert_eq!(ptr.kind, "analysis");
    assert!(ptr.review);
    assert!(!ptr.auto);
    assert!(ptr.stages.is_empty());
    assert!(ptr.loops.is_empty());
    assert_eq!(ptr.summary(), "phân tích ⇄ review");

    // Existing workflows unchanged
    let mac = loaded.get("mac-dinh").unwrap();
    assert_eq!(mac.kind, "code");
    assert_eq!(mac.stages, STAGES);
    assert_eq!(
        mac.summary(),
        "plan ⇄\u{FE0E} review → Bạn → code ⇄\u{FE0E} review → QA ⇄\u{FE0E} sửa"
    );

    let nhanh = loaded.get("nhanh").unwrap();
    assert_eq!(nhanh.kind, "code");
    assert_eq!(nhanh.summary(), "plan → Bạn → code → QA");
}

#[test]
fn analysis_validation_rules() {
    let cfg = test_cfg();
    let lines = BTreeMap::new();

    // 1. stages non-empty in analysis -> error
    let v_stages = serde_json::json!({
        "kind": "analysis",
        "stages": ["plan"]
    });
    let err_stages = validate_entry("wf-bad-stages", &v_stages, &lines, &cfg, "test").unwrap_err();
    assert!(err_stages.iter().any(|e| e.contains("quy trình phân tích không được có \"stages\"")));

    // 2. auto: true in analysis -> error
    let v_auto = serde_json::json!({
        "kind": "analysis",
        "auto": true
    });
    let err_auto = validate_entry("wf-bad-auto", &v_auto, &lines, &cfg, "test").unwrap_err();
    assert!(err_auto.iter().any(|e| e.contains("quy trình phân tích không hỗ trợ \"auto\": true (phải là false)")));

    // 3. loops in analysis -> error
    let v_loops = serde_json::json!({
        "kind": "analysis",
        "loops": {"plan": "plan-review"}
    });
    let err_loops = validate_entry("wf-bad-loops", &v_loops, &lines, &cfg, "test").unwrap_err();
    assert!(err_loops.iter().any(|e| e.contains("quy trình phân tích không được có khóa \"loops\"")));

    // 4. review in code -> error
    let v_review = serde_json::json!({
        "kind": "code",
        "stages": ["plan", "gate:you", "implement", "qa"],
        "review": true
    });
    let err_rev = validate_entry("wf-bad-code-rev", &v_review, &lines, &cfg, "test").unwrap_err();
    assert!(err_rev.iter().any(|e| e.contains("quy trình code không được có khóa \"review\"")));
}
