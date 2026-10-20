mod common;

use common::Sandbox;

#[test]
fn workflows_endpoint_builtins() {
    let sb = Sandbox::new();
    let repo_web = sb.make_repo("web-ban-hang", &[], None, None);
    let repo_kho = sb.make_repo("kho", &[], None, None);

    let mut entry_web = sb.project_entry(&repo_web, "web-ban-hang", "work");
    entry_web["workflow"] = serde_json::json!("nhanh");
    entry_web["name_vi"] = serde_json::json!("web-ban-hang");

    let mut entry_kho = sb.project_entry(&repo_kho, "kho", "work");
    entry_kho.as_object_mut().unwrap().remove("workflow");
    entry_kho["name_vi"] = serde_json::json!("kho");

    sb.write_map(serde_json::json!([entry_web, entry_kho]), &[]);

    let mut srv = sb.serve(&[]);
    let r = srv.get("/api/workflows");
    assert_eq!(r.status, 200);
    let j = r.json();

    let workflows = j["workflows"].as_array().expect("workflows array");
    let names: Vec<&str> = workflows
        .iter()
        .map(|w| w["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["mac-dinh", "nhanh"]);

    let mac_dinh = &workflows[0];
    assert_eq!(mac_dinh["source"], "builtin");
    assert_eq!(mac_dinh["auto"], true);
    assert_eq!(mac_dinh["loops"], true);
    assert_eq!(mac_dinh["limits"], "3 vòng · 60 phút · $20");
    assert_eq!(
        mac_dinh["summary"],
        "plan ⇄\u{FE0E} review → Bạn → code ⇄\u{FE0E} review → QA ⇄\u{FE0E} sửa"
    );
    assert_eq!(mac_dinh["default_for"], serde_json::json!(["kho"]));

    let nhanh = &workflows[1];
    assert_eq!(nhanh["loops"], false);
    assert_eq!(nhanh["limits"], "60 phút · $20");
    assert_eq!(nhanh["default_for"], serde_json::json!(["web-ban-hang"]));

    let errors = j["errors"].as_array().expect("errors array");
    assert!(errors.is_empty(), "expected no errors: {:?}", errors);

    assert_eq!(
        j["project_defaults"]["kho"],
        serde_json::json!({
            "workflow": null,
            "effective": "mac-dinh",
            "note": null
        })
    );

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn workflows_endpoint_file_and_errors() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("test-proj", &[], None, None);
    let entry = sb.project_entry(&repo, "test-proj", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    // Write workflows.json with a valid "chi-phi-thap" and an entry "x" whose max_rounds is 0
    let wf_text = r#"{
  "chi-phi-thap": {
    "stages": ["plan", "gate:you", "implement", "qa"],
    "max_rounds": 1,
    "budget_minutes": 30,
    "budget_usd": 5,
    "auto": false
  },
  "x": {
    "stages": ["plan"],
    "max_rounds": 0
  }
}"#;
    let wf_path = sb.workflows_path();
    if let Some(p) = wf_path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(&wf_path, wf_text).unwrap();

    let mut srv = sb.serve(&[]);
    let r = srv.get("/api/workflows");
    assert_eq!(r.status, 200);
    let j = r.json();

    let workflows = j["workflows"].as_array().expect("workflows array");
    let names: Vec<&str> = workflows
        .iter()
        .map(|w| w["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["mac-dinh", "nhanh", "chi-phi-thap"]);

    let chi_phi = workflows
        .iter()
        .find(|w| w["name"] == "chi-phi-thap")
        .unwrap();
    assert_eq!(chi_phi["source"], "file");

    let errors = j["errors"].as_array().expect("errors array");
    let err_strings: Vec<&str> = errors.iter().map(|e| e.as_str().unwrap()).collect();
    assert!(
        err_strings.iter().any(
            |s| s.contains(r#""max_rounds" phải là số nguyên từ 1 đến 10"#)
                && s.contains("Dòng 11:")
        ),
        "errors: {:?}",
        err_strings
    );

    // Now write a syntax-broken file
    std::fs::write(&wf_path, "{ broken json").unwrap();
    let r_broken = srv.get("/api/workflows");
    assert_eq!(r_broken.status, 200);
    let j_broken = r_broken.json();
    let names_broken: Vec<&str> = j_broken["workflows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["name"].as_str().unwrap())
        .collect();
    assert_eq!(names_broken, vec!["mac-dinh", "nhanh"]);
    let errs_broken = j_broken["errors"].as_array().unwrap();
    assert_eq!(errs_broken.len(), 1);
    assert!(
        errs_broken[0]
            .as_str()
            .unwrap()
            .contains("không phải JSON hợp lệ"),
        "broken error: {:?}",
        errs_broken
    );

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn workflows_endpoint_rereads_file() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("test-proj", &[], None, None);
    let entry = sb.project_entry(&repo, "test-proj", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    let mut srv = sb.serve(&[]);
    let r1 = srv.get("/api/workflows");
    assert_eq!(r1.status, 200);
    let j1 = r1.json();
    let names1: Vec<&str> = j1["workflows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["name"].as_str().unwrap())
        .collect();
    assert!(!names1.contains(&"moi"));

    // Rewrite workflows.json adding "moi"
    let wf_text = r#"{
  "moi": {
    "stages": ["plan", "gate:you", "implement", "qa"],
    "max_rounds": 1,
    "budget_minutes": 10,
    "budget_usd": 2,
    "auto": true
  }
}"#;
    let wf_path = sb.workflows_path();
    if let Some(p) = wf_path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(&wf_path, wf_text).unwrap();

    let r2 = srv.get("/api/workflows");
    assert_eq!(r2.status, 200);
    let j2 = r2.json();
    let names2: Vec<&str> = j2["workflows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["name"].as_str().unwrap())
        .collect();
    assert!(names2.contains(&"moi"), "names2: {:?}", names2);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn unknown_project_default_note() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("p-unknown", &[], None, None);
    let mut entry = sb.project_entry(&repo, "p-unknown", "work");
    entry["workflow"] = serde_json::json!("x");
    sb.write_map(serde_json::json!([entry]), &[]);

    let mut srv = sb.serve(&[]);
    let r = srv.get("/api/workflows");
    assert_eq!(r.status, 200);
    let j = r.json();

    assert_eq!(
        j["project_defaults"]["p-unknown"],
        serde_json::json!({
            "workflow": "x",
            "effective": "mac-dinh",
            "note": "Quy trình 'x' của dự án không có trong workflows.json — dùng mac-dinh."
        })
    );

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn workflows_endpoint_gated() {
    let sb = Sandbox::new();
    let mut srv = sb.serve(&[]);

    // 1. Request without cookie -> 401
    let r_unauth = srv.request("GET", "/api/workflows", &[], b"");
    assert_eq!(r_unauth.status, 401);

    // 2. POST to /api/workflows with cookie and valid Origin -> 405
    let r_post = srv.post_json("/api/workflows", &serde_json::json!({}));
    assert_eq!(r_post.status, 405);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}
