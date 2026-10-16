mod common;

use common::Sandbox;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn format_path(template: &str, root_paths: &HashMap<String, PathBuf>) -> String {
    let mut result = template.to_string();
    for (k, v) in root_paths {
        let pat = format!("{{{}}}", k);
        result = result.replace(&pat, v.to_str().unwrap());
    }
    result
}

fn materialize_layout(sb: &Sandbox, layout: &serde_json::Value) -> HashMap<String, PathBuf> {
    let mut root_paths = HashMap::new();
    if let Some(roots_spec) = layout.get("roots").and_then(|r| r.as_object()) {
        for (r_name, folders) in roots_spec {
            let r_dir = sb.root.join("roots").join(r_name);
            std::fs::create_dir_all(&r_dir).expect("create root dir");
            let r_resolved = r_dir.canonicalize().unwrap_or(r_dir.clone());
            root_paths.insert(r_name.clone(), r_resolved);
            if let Some(folders_obj) = folders.as_object() {
                for (f_name, f_spec) in folders_obj {
                    let f_dir = r_dir.join(f_name);
                    std::fs::create_dir_all(&f_dir).expect("create folder dir");
                    if f_spec.get("git").and_then(|g| g.as_bool()).unwrap_or(false) {
                        let remote = f_spec.get("remote").and_then(|r| r.as_str());
                        sb.make_repo(f_name, &[], Some(&r_dir), remote);
                    }
                    if let Some(children) = f_spec.get("children").and_then(|c| c.as_object()) {
                        for (c_name, c_spec) in children {
                            let c_dir = f_dir.join(c_name);
                            std::fs::create_dir_all(&c_dir).expect("create child dir");
                            if c_spec.get("git").and_then(|g| g.as_bool()).unwrap_or(false) {
                                let remote = c_spec.get("remote").and_then(|r| r.as_str());
                                sb.make_repo(c_name, &[], Some(&f_dir), remote);
                            }
                        }
                    }
                }
            }
        }
    }
    root_paths
}

fn run_case(case_id: &str) {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/contract/scan.json");
    let fixture_str = std::fs::read_to_string(&fixture_path).expect("read scan.json");
    let fixture: serde_json::Value = serde_json::from_str(&fixture_str).expect("parse scan.json");
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == case_id)
        .unwrap_or_else(|| panic!("case {} not found", case_id));

    let sb = Sandbox::new();
    let root_paths = materialize_layout(&sb, &case["layout"]);

    let mut all_stderr = Vec::new();
    let mut actual_exit_codes = Vec::new();

    if let Some(steps) = case.get("steps").and_then(|s| s.as_array()) {
        for step in steps {
            let op = step["op"].as_str().unwrap();
            match op {
                "scan" => {
                    let mut args = vec!["projects", "scan"];
                    let mut root_args = Vec::new();
                    if let Some(roots) = step.get("roots").and_then(|r| r.as_array()) {
                        for r in roots {
                            let r_name = r.as_str().unwrap();
                            root_args.push(root_paths[r_name].to_str().unwrap().to_string());
                        }
                    }
                    for r_arg in &root_args {
                        args.push("--root");
                        args.push(r_arg.as_str());
                    }
                    let res = sb.run(&args, None);
                    actual_exit_codes.push(res.code);
                    all_stderr.push(res.stderr);
                }
                "add" => {
                    let raw_path = format_path(step["path"].as_str().unwrap(), &root_paths);
                    let mut args = vec!["projects", "add", raw_path.as_str()];
                    let type_arg;
                    if let Some(t) = step.get("type").and_then(|t| t.as_str()) {
                        type_arg = t;
                        args.push("--type");
                        args.push(type_arg);
                    }
                    let res = sb.run(&args, None);
                    actual_exit_codes.push(res.code);
                    all_stderr.push(res.stderr);
                }
                "edit" => {
                    let map_path = sb.map_path();
                    let content = std::fs::read_to_string(&map_path).expect("read map");
                    let mut data: serde_json::Value =
                        serde_json::from_str(&content).expect("parse map");
                    let proj_id = step["project"].as_str().unwrap();
                    let mut found = false;
                    if let Some(projects) = data.get_mut("projects").and_then(|p| p.as_array_mut())
                    {
                        for p in projects {
                            if p.get("id").and_then(|id| id.as_str()) == Some(proj_id) {
                                if let Some(obj) = p.as_object_mut() {
                                    for (k, v) in step["set"].as_object().unwrap() {
                                        obj.insert(k.clone(), v.clone());
                                    }
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }
                    assert!(found, "Project '{}' not found for edit", proj_id);
                    std::fs::write(&map_path, serde_json::to_string_pretty(&data).unwrap())
                        .expect("write map");
                    actual_exit_codes.push(0);
                }
                "mkrepo" => {
                    let r_dir = &root_paths[step["root"].as_str().unwrap()];
                    let remote = step.get("remote").and_then(|r| r.as_str());
                    sb.make_repo(step["name"].as_str().unwrap(), &[], Some(r_dir), remote);
                    actual_exit_codes.push(0);
                }
                "rmdir" => {
                    let target =
                        PathBuf::from(format_path(step["path"].as_str().unwrap(), &root_paths));
                    if target.is_dir() {
                        std::fs::remove_dir_all(&target).expect("remove dir");
                    }
                    actual_exit_codes.push(0);
                }
                "mkdir" => {
                    let target =
                        PathBuf::from(format_path(step["path"].as_str().unwrap(), &root_paths));
                    std::fs::create_dir_all(&target).expect("create dir");
                    if step.get("git").and_then(|g| g.as_bool()).unwrap_or(false) {
                        sb.make_repo(
                            target.file_name().unwrap().to_str().unwrap(),
                            &[],
                            target.parent(),
                            None,
                        );
                    }
                    actual_exit_codes.push(0);
                }
                other => panic!("unknown op: {}", other),
            }
        }
    }

    let expect = &case["expect"];
    if let Some(exp_codes) = expect.get("exit_codes").and_then(|c| c.as_array()) {
        let exp_codes_vec: Vec<i32> = exp_codes
            .iter()
            .map(|c| c.as_i64().unwrap() as i32)
            .collect();
        assert_eq!(actual_exit_codes, exp_codes_vec, "Exit codes mismatch");
    }

    let final_data = if sb.map_path().exists() {
        let content = std::fs::read_to_string(sb.map_path()).expect("read final map");
        serde_json::from_str(&content).expect("parse final map")
    } else {
        serde_json::json!({"scan_roots": [], "projects": []})
    };

    let mut projects_by_id = HashMap::new();
    if let Some(arr) = final_data.get("projects").and_then(|p| p.as_array()) {
        for p in arr {
            if let Some(id) = p.get("id").and_then(|id| id.as_str()) {
                projects_by_id.insert(id.to_string(), p.clone());
            }
        }
    }

    if let Some(exp_projects) = expect.get("projects").and_then(|p| p.as_object()) {
        for (exp_id, exp_fields) in exp_projects {
            assert!(
                projects_by_id.contains_key(exp_id),
                "Project '{}' should be in map",
                exp_id
            );
            let actual_proj = &projects_by_id[exp_id];
            for (k, v) in exp_fields.as_object().unwrap() {
                let expected_val = if let Some(s) = v.as_str() {
                    serde_json::Value::String(format_path(s, &root_paths))
                } else {
                    v.clone()
                };
                assert_eq!(
                    actual_proj.get(k),
                    Some(&expected_val),
                    "Field '{}' on project '{}' mismatch: {:?} != {:?}",
                    k,
                    exp_id,
                    actual_proj.get(k),
                    expected_val
                );
            }
        }
    }

    if let Some(absent) = expect.get("absent").and_then(|a| a.as_array()) {
        for abs_id in absent {
            let abs_str = abs_id.as_str().unwrap();
            assert!(
                !projects_by_id.contains_key(abs_str),
                "Project '{}' should be absent",
                abs_str
            );
        }
    }

    let combined_stderr = all_stderr.join("\n");
    if let Some(warnings) = expect.get("warnings").and_then(|w| w.as_array()) {
        for warn in warnings {
            let key = warn["key"].as_str().unwrap();
            let prefix = format!("CẢNH BÁO: tên/alias trùng \"{}\":", key);
            assert!(
                combined_stderr.contains(&prefix),
                "Expected warning prefix '{}' in stderr:\n{}",
                prefix,
                combined_stderr
            );
            for wid in warn["ids"].as_array().unwrap() {
                let wid_str = wid.as_str().unwrap();
                assert!(
                    combined_stderr.contains(wid_str),
                    "Expected id '{}' in warning in stderr:\n{}",
                    wid_str,
                    combined_stderr
                );
            }
        }
    }
}

#[test]
fn p1_05_scan_keeps_hand_edits() {
    run_case("P1-05");
}

#[test]
fn route_06_alias_warned() {
    run_case("ROUTE-06-alias");
}

#[test]
fn route_06_alias_vs_id_warned() {
    run_case("ROUTE-06-alias-vs-id");
}

#[test]
fn d06_depth_one_only() {
    run_case("D-06-depth");
}

#[test]
fn d06_missing_then_restored() {
    run_case("D-06-missing-restore");
}

#[test]
fn id_collision_suffixed() {
    run_case("R-id-collision");
}

#[test]
fn remote_credentials_stripped() {
    run_case("R-remote-credentials");
}

#[test]
fn d05_add_media() {
    run_case("D-05-add-media");
}

#[test]
fn d05_add_existing_keeps_edits() {
    run_case("D-05-add-existing");
}

#[test]
fn unknown_keys_preserved() {
    run_case("R-unknown-keys");
}

#[test]
fn scan_fixture_ids_all_covered() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/contract/scan.json");
    let fixture_str = std::fs::read_to_string(&fixture_path).expect("read scan.json");
    let fixture: serde_json::Value = serde_json::from_str(&fixture_str).expect("parse scan.json");
    let ids: Vec<&str> = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    let expected = vec![
        "P1-05",
        "ROUTE-06-alias",
        "ROUTE-06-alias-vs-id",
        "D-06-depth",
        "D-06-missing-restore",
        "R-id-collision",
        "R-remote-credentials",
        "D-05-add-media",
        "D-05-add-existing",
        "R-unknown-keys",
    ];
    assert_eq!(ids, expected);
}

#[test]
fn map_written_like_python() {
    let sb = Sandbox::new();
    let root = sb.root.join("roots");
    std::fs::create_dir_all(&root).unwrap();
    sb.make_repo("web-bán-hàng", &[], Some(&root), None);

    let res = sb.run(
        &["projects", "scan", "--root", root.to_str().unwrap()],
        None,
    );
    assert_eq!(res.code, 0, "scan failed: {}", res.stderr);

    let map_bytes = std::fs::read(sb.map_path()).expect("read map");
    assert!(map_bytes.ends_with(b"\n"), "Map file must end with newline");
    let map_str = String::from_utf8(map_bytes).expect("valid utf-8");
    assert!(
        map_str.contains("\"name_vi\": \"web-bán-hàng\""),
        "Map must contain unescaped Vietnamese name_vi"
    );
    assert!(
        !map_str.contains("\\u"),
        "Map must not contain \\u escape sequences"
    );
    assert!(
        map_str.contains("  \"projects\": ["),
        "Map must be 2-space indented"
    );
}

#[test]
fn scan_missing_root_exits_1_map_untouched() {
    let sb = Sandbox::new();
    let missing_1 = sb.root.join("does_not_exist");
    let res = sb.run(
        &["projects", "scan", "--root", missing_1.to_str().unwrap()],
        None,
    );
    assert_eq!(res.code, 1);
    assert!(
        res.stderr
            .contains(&format!("Thư mục không tồn tại: {}", missing_1.display())),
        "stderr should contain missing root message: {}",
        res.stderr
    );
    assert!(!sb.map_path().exists(), "map file should not exist yet");

    let valid_root = sb.root.join("valid_root");
    std::fs::create_dir_all(&valid_root).unwrap();
    let res_init = sb.run(
        &["projects", "scan", "--root", valid_root.to_str().unwrap()],
        None,
    );
    assert_eq!(res_init.code, 0);
    assert!(sb.map_path().exists());
    let before_bytes = std::fs::read(sb.map_path()).unwrap();

    let missing_2 = sb.root.join("another_missing");
    let res2 = sb.run(
        &["projects", "scan", "--root", missing_2.to_str().unwrap()],
        None,
    );
    assert_eq!(res2.code, 1);
    assert!(res2
        .stderr
        .contains(&format!("Thư mục không tồn tại: {}", missing_2.display())));
    let after_bytes = std::fs::read(sb.map_path()).unwrap();
    assert_eq!(
        before_bytes, after_bytes,
        "map file should remain byte-identical"
    );
}

#[test]
fn scan_without_roots_exits_1() {
    let sb = Sandbox::new();
    let res = sb.run(&["projects", "scan"], None);
    assert_eq!(res.code, 1);
    assert!(
        res.stderr
            .contains("Chưa có scan_roots — chạy: agent-brain projects scan --root <thư mục>"),
        "stderr: {}",
        res.stderr
    );
}

#[test]
fn unreadable_map_left_untouched() {
    let sb = Sandbox::new();
    let map_path = sb.map_path();
    if let Some(p) = map_path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(&map_path, "{bad").unwrap();

    let root = sb.root.join("roots");
    std::fs::create_dir_all(&root).unwrap();

    let res = sb.run(
        &["projects", "scan", "--root", root.to_str().unwrap()],
        None,
    );
    assert_eq!(res.code, 1);
    assert!(
        res.stderr.contains("Không đọc được"),
        "stderr: {}",
        res.stderr
    );
    let content = std::fs::read_to_string(&map_path).unwrap();
    assert_eq!(content, "{bad", "unreadable map must not be modified");
}
