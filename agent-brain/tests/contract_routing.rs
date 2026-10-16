mod common;

use agent_brain::projects::Project;
use agent_brain::routing::{normalize, resolve, slugify, RouteKind};
use common::Sandbox;
use std::path::Path;

#[test]
fn normalize_contract() {
    let fixture_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/contract/normalize.json");
    let fixture_str = std::fs::read_to_string(&fixture_path).expect("read normalize.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&fixture_str).expect("parse normalize.json");

    for case in fixture["cases"].as_array().unwrap() {
        let case_id = case["id"].as_str().unwrap();
        let input = case["input"].as_str().unwrap();
        let expected_normalized = case["normalized"].as_str().unwrap();
        let expected_slug = case["slug"].as_str().unwrap();

        assert_eq!(
            normalize(input),
            expected_normalized,
            "normalize failed on case {}",
            case_id
        );
        assert_eq!(
            slugify(input),
            expected_slug,
            "slugify failed on case {}",
            case_id
        );
    }
}

#[test]
fn normalize_unicode_mn_only() {
    // Review finding 1: drop only Unicode category Mn, not Mc or Me.
    // Python normalize("ा") == "ा"
    // Python normalize("क़ि हिन्दी") == "कि हिनदी"
    assert_eq!(normalize("ा"), "ा");
    assert_eq!(normalize("क़ि हिन्दी"), "कि हिनदी");
}

#[test]
fn p1_01_p1_04_routing_contract() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/contract/routing.json");
    let fixture_str = std::fs::read_to_string(&fixture_path).expect("read routing.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&fixture_str).expect("parse routing.json");

    let projects: Vec<Project> = fixture["projects"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Project::from_value)
        .collect();

    for case in fixture["cases"].as_array().unwrap() {
        let case_id = case["id"].as_str().unwrap();
        let query = case["query"].as_str().unwrap();
        let expect = &case["expect"];
        let expect_kind = expect["kind"].as_str().unwrap();

        if expect_kind == "match" {
            let matched = resolve(query, &projects)
                .unwrap_or_else(|e| panic!("Case {} expected match, got {:?}", case_id, e));
            assert_eq!(
                matched.id,
                expect["project"].as_str().unwrap(),
                "Case {} matched id mismatch",
                case_id
            );
        } else {
            let err = resolve(query, &projects)
                .expect_err(&format!("Case {} expected error, got match", case_id));
            let actual_kind = match err.kind {
                RouteKind::None => "none",
                RouteKind::Ambiguous => "ambiguous",
            };
            assert_eq!(
                actual_kind, expect_kind,
                "Case {} error kind mismatch",
                case_id
            );
            let candidate_ids: Vec<&str> = err.candidates.iter().map(|p| p.id.as_str()).collect();
            let expect_candidates: Vec<&str> = expect["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c.as_str().unwrap())
                .collect();
            assert_eq!(
                candidate_ids, expect_candidates,
                "Case {} candidates mismatch",
                case_id
            );
        }
    }
}

#[test]
fn p1_03_ambiguous_name_stops_with_list() {
    let sb = Sandbox::new();
    let repo1 = sb.make_repo("proj-a", &[], None, None);
    let repo2 = sb.make_repo("proj-b", &[], None, None);
    let mut e1 = sb.project_entry(&repo1, "proj-a", "work");
    e1["aliases"] = serde_json::json!(["shop"]);
    let mut e2 = sb.project_entry(&repo2, "proj-b", "work");
    e2["aliases"] = serde_json::json!(["shop"]);
    sb.write_map(serde_json::json!([e1, e2]), &[]);

    let res = sb.run(&["run", "lead", "shop", "x"], None);
    assert_eq!(res.code, 2);
    assert!(
        res.stdout
            .starts_with("Tên \"shop\" khớp 2 dự án — dừng, không chạy agent."),
        "stdout should start with ambiguous header:\n{}",
        res.stdout
    );
    assert!(res.stdout.contains("proj-a"));
    assert!(res.stdout.contains("proj-b"));
    assert!(sb.records().is_empty());
}

#[test]
fn p1_04_unknown_name_lists_all() {
    let sb = Sandbox::new();
    let repo1 = sb.make_repo("proj-a", &[], None, None);
    let repo2 = sb.make_repo("proj-b", &[], None, None);
    let e1 = sb.project_entry(&repo1, "proj-a", "work");
    let e2 = sb.project_entry(&repo2, "proj-b", "work");
    sb.write_map(serde_json::json!([e1, e2]), &[]);

    let res = sb.run(&["run", "lead", "không có", "x"], None);
    assert_eq!(res.code, 2);
    assert!(
        res.stdout
            .contains("Không tìm thấy dự án \"không có\" — dừng, không chạy agent."),
        "stdout should contain unknown header:\n{}",
        res.stdout
    );
    assert!(res.stdout.contains("proj-a"));
    assert!(res.stdout.contains("proj-b"));
    assert!(sb.records().is_empty());

    let sb_empty = Sandbox::new();
    sb_empty.write_map(serde_json::json!([]), &[]);
    let res_empty = sb_empty.run(&["run", "lead", "không có", "x"], None);
    assert_eq!(res_empty.code, 2);
    assert!(
        res_empty
            .stdout
            .contains("(trống) — chạy: agent-brain projects scan --root <thư mục>"),
        "stdout should contain empty hint:\n{}",
        res_empty.stdout
    );
}
