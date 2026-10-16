use agent_brain::planfile::{
    has_blockers, meta_line, parse_plan, parse_qa_verdict, plan_files, read_meta,
    render_qa_skeleton, validate_plan, QA_SECTIONS,
};
use std::path::Path;

#[test]
fn plan_fixture_has_24_cases() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/contract/plan.json");
    let fixture_str = std::fs::read_to_string(&fixture_path).expect("read plan.json");
    let fixture: serde_json::Value = serde_json::from_str(&fixture_str).expect("parse plan.json");
    let cases = fixture["cases"].as_array().expect("cases array");
    assert_eq!(cases.len(), 24);
}

#[test]
fn plan_contract_cases() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/contract/plan.json");
    let fixture_str = std::fs::read_to_string(&fixture_path).expect("read plan.json");
    let fixture: serde_json::Value = serde_json::from_str(&fixture_str).expect("parse plan.json");
    assert_eq!(fixture["version"], 1);

    let cases = fixture["cases"].as_array().expect("cases array");
    assert_eq!(cases.len(), 24);

    let mut failures = Vec::new();

    for case in cases {
        let case_id = case["id"].as_str().unwrap();
        let markdown = case["markdown"].as_str().unwrap();
        let expect = &case["expect"];

        let expected_files: Vec<String> = expect["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let expected_has_blockers = expect["has_blockers"].as_bool().unwrap();
        let expected_valid = expect["valid"].as_bool().unwrap();
        let expected_error_codes: Vec<String> = expect["error_codes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        let plan = parse_plan(markdown);

        let files = plan_files(&plan);
        if files != expected_files {
            failures.push(format!(
                "{case_id}: files mismatch: expected {expected_files:?}, got {files:?}"
            ));
        }

        let blockers = has_blockers(&plan);
        if blockers != expected_has_blockers {
            failures.push(format!(
                "{case_id}: has_blockers mismatch: expected {expected_has_blockers}, got {blockers}"
            ));
        }

        let temp_dir;
        let root = if let Some(layout) = case.get("root_layout") {
            let td = tempfile::tempdir().unwrap();
            let root_path = td.path().join("repo");
            std::fs::create_dir_all(&root_path).unwrap();
            let outside_dir = td.path().join("outside");
            std::fs::create_dir_all(&outside_dir).unwrap();
            std::fs::write(outside_dir.join("target.py"), "print('outside')").unwrap();

            if let Some(files) = layout.get("files").and_then(|v| v.as_array()) {
                for f in files {
                    let f_str = f.as_str().unwrap();
                    let fp = root_path.join(f_str);
                    if let Some(parent) = fp.parent() {
                        std::fs::create_dir_all(parent).unwrap();
                    }
                    std::fs::write(fp, "ok").unwrap();
                }
            }

            if let Some(symlinks) = layout.get("symlinks").and_then(|v| v.as_object()) {
                for (link_name, link_target) in symlinks {
                    let target_str = link_target.as_str().unwrap();
                    let lp = root_path.join(link_name);
                    if let Some(parent) = lp.parent() {
                        std::fs::create_dir_all(parent).unwrap();
                    }
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::symlink;
                        if target_str.starts_with("../outside") {
                            symlink(outside_dir.join("target.py"), &lp).unwrap();
                        } else {
                            symlink(target_str, &lp).unwrap();
                        }
                    }
                }
            }
            temp_dir = Some(td);
            Some(root_path)
        } else {
            temp_dir = None;
            None
        };

        let errors = validate_plan(&plan, root.as_deref());
        let is_valid = errors.is_empty();
        if is_valid != expected_valid {
            failures.push(format!(
                "{case_id}: valid mismatch: expected {expected_valid}, got {is_valid}"
            ));
        }
        if errors != expected_error_codes {
            failures.push(format!(
                "{case_id}: error_codes mismatch: expected {expected_error_codes:?}, got {errors:?}"
            ));
        }

        drop(temp_dir);
    }

    assert!(
        failures.is_empty(),
        "Plan contract failures ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn qa_report_contract_cases() {
    let fixture_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/contract/qa_report.json");
    let fixture_str = std::fs::read_to_string(&fixture_path).expect("read qa_report.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&fixture_str).expect("parse qa_report.json");
    assert_eq!(fixture["version"], 1);

    let cases = fixture["cases"].as_array().expect("cases array");
    assert_eq!(cases.len(), 8);

    let mut failures = Vec::new();

    for case in cases {
        let case_id = case["id"].as_str().unwrap();
        let markdown = case["markdown"].as_str().unwrap();
        let expect = &case["expect"];
        let expected_verdict = expect["verdict"].as_str();

        let actual_verdict = parse_qa_verdict(markdown);
        if actual_verdict != expected_verdict {
            failures.push(format!(
                "{case_id}: verdict mismatch: expected {expected_verdict:?}, got {actual_verdict:?}"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "QA contract failures ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn qa_skeleton_matches_python_shape() {
    let meta = serde_json::json!({
        "task": "t",
        "plan_commit": "abc",
    });
    let rendered = render_qa_skeleton("Thêm nút", &meta);
    assert!(
        rendered.starts_with("# QA: Thêm nút\n<!-- brain-meta: "),
        "Must start with '# QA: Thêm nút\\n<!-- brain-meta: ', got:\n{rendered}"
    );

    let mut last_pos = 0;
    for sec in QA_SECTIONS {
        let heading = format!("## {sec}");
        if let Some(pos) = rendered[last_pos..].find(&heading) {
            last_pos += pos + heading.len();
        } else {
            panic!("Missing QA section '{heading}' in order. Rendered:\n{rendered}");
        }
    }

    assert!(
        rendered.ends_with('\n'),
        "Rendered QA skeleton must end with newline"
    );
}

#[test]
fn reads_python_meta_line() {
    let python_meta_line =
        r#"<!-- brain-meta: {"base_branch": "main", "request": "a > b", "task": "t"} -->"#;
    let read = read_meta(python_meta_line).expect("must parse python meta line");
    assert_eq!(read["base_branch"], "main");
    assert_eq!(read["request"], "a > b");
    assert_eq!(read["task"], "t");

    let ml = meta_line(&read);
    assert!(ml.contains(r"\u003e"));

    let payload = ml
        .strip_prefix("<!-- brain-meta: ")
        .unwrap()
        .strip_suffix(" -->")
        .unwrap();
    assert!(
        !payload.contains('>'),
        "Payload should not have raw '>': {payload}"
    );

    let roundtrip = read_meta(&ml).expect("must roundtrip rust meta line");
    assert_eq!(roundtrip, read);
}
