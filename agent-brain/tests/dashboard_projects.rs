mod common;

use common::Sandbox;

#[test]
fn projects_list_and_profile_filter() {
    let sb = Sandbox::new();
    let repo_w = sb.make_repo("proj-work", &[], None, None);
    let repo_p = sb.make_repo("proj-personal", &[], None, None);

    let mut entry_w = sb.project_entry(&repo_w, "proj-work", "work");
    entry_w["aliases"] = serde_json::json!(["shared-alias", "work-alias"]);
    entry_w["name_vi"] = serde_json::json!("Dự án Work");

    let mut entry_p = sb.project_entry(&repo_p, "proj-personal", "personal");
    entry_p["aliases"] = serde_json::json!(["shared-alias", "pers-alias"]);
    entry_p["name_vi"] = serde_json::json!("Dự án Personal");

    sb.write_map(serde_json::json!([entry_w, entry_p]), &[]);

    let mut srv = sb.serve(&[]);

    // 1. GET /api/projects with no profile -> lists all projects
    let r = srv.get("/api/projects");
    assert_eq!(r.status, 200);
    let j = r.json();

    let etag = j["etag"].as_str().expect("etag string");
    assert!(!etag.is_empty(), "etag should not be empty");

    let projects = j["projects"].as_array().expect("projects array");
    assert_eq!(projects.len(), 2);
    for p in projects {
        assert!(p.get("id").is_some());
        assert!(p.get("name_vi").is_some());
        assert!(p.get("aliases").is_some());
        assert!(p.get("path").is_some());
        assert!(p.get("type").is_some());
        assert!(p.get("profile").is_some());
        assert!(p.get("missing").is_some());
    }

    // duplicates list contains shared-alias across the two projects
    let duplicates = j["duplicates"].as_array().expect("duplicates array");
    assert_eq!(duplicates.len(), 1);
    assert_eq!(duplicates[0]["key"], "shared alias");
    let dup_ids = duplicates[0]["ids"].as_array().expect("ids array");
    assert_eq!(dup_ids.len(), 2);

    // 2. GET /api/projects?profile=work -> returns only work project
    let r_work = srv.get("/api/projects?profile=work");
    assert_eq!(r_work.status, 200);
    let j_work = r_work.json();
    let projs_work = j_work["projects"].as_array().expect("projects array");
    assert_eq!(projs_work.len(), 1);
    assert_eq!(projs_work[0]["id"], "proj-work");
    assert_eq!(projs_work[0]["profile"], "work");
    // duplicates are still computed over unfiltered map
    assert_eq!(j_work["duplicates"].as_array().unwrap().len(), 1);

    // 3. GET /api/projects?profile=bogus -> 400
    let r_bad = srv.get("/api/projects?profile=bogus");
    assert_eq!(r_bad.status, 400);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn suggest_endpoint() {
    let sb = Sandbox::new();
    let repo_w = sb.make_repo("web-ban-hang", &[], None, None);
    let repo_p = sb.make_repo("app-personal", &[], None, None);

    let mut entry_w = sb.project_entry(&repo_w, "web-ban-hang", "work");
    entry_w["name_vi"] = serde_json::json!("Web bán hàng");
    entry_w["aliases"] = serde_json::json!(["shop"]);

    let mut entry_p = sb.project_entry(&repo_p, "app-personal", "personal");
    entry_p["name_vi"] = serde_json::json!("App cá nhân");
    entry_p["aliases"] = serde_json::json!(["shop"]);

    let mut entry_m = sb.project_entry(&repo_w, "missing-proj", "work");
    entry_m["name_vi"] = serde_json::json!("Dự án thiếu");
    entry_m["path"] = serde_json::json!(sb.root.join("nonexistent-dir").to_string_lossy());
    entry_m["missing"] = serde_json::json!(true);

    let mut entries = vec![entry_w, entry_p, entry_m];

    // Add 12 dummy projects to test SUGGEST_LIMIT (at most 10 returned)
    for i in 1..=12 {
        let dummy_id = format!("extra-proj-{i:02}");
        let repo_d = sb.make_repo(&dummy_id, &[], None, None);
        let mut entry_d = sb.project_entry(&repo_d, &dummy_id, "work");
        entry_d["name_vi"] = serde_json::json!(format!("Dự án phụ {i}"));
        entries.push(entry_d);
    }

    sb.write_map(serde_json::Value::Array(entries), &[]);

    let mut srv = sb.serve(&[]);

    // 1. q=ban returns web-ban-hang
    let r = srv.get("/api/projects/suggest?q=ban");
    assert_eq!(r.status, 200);
    let j = r.json();
    let items = j["items"].as_array().expect("items array");
    assert!(!items.is_empty());
    assert_eq!(items[0]["id"], "web-ban-hang");
    assert_eq!(items[0]["name_vi"], "Web bán hàng");
    assert_eq!(items[0]["rank"], "contains");

    // 2. q=shop&profile=personal excludes work project
    let r_pers = srv.get("/api/projects/suggest?q=shop&profile=personal");
    assert_eq!(r_pers.status, 200);
    let j_pers = r_pers.json();
    let items_pers = j_pers["items"].as_array().expect("items array");
    assert_eq!(items_pers.len(), 1);
    assert_eq!(items_pers[0]["id"], "app-personal");
    assert_eq!(items_pers[0]["profile"], "personal");
    assert_eq!(items_pers[0]["rank"], "exact");

    // 3. Project with missing: true is returned with missing true
    let r_miss = srv.get("/api/projects/suggest?q=thieu");
    assert_eq!(r_miss.status, 200);
    let j_miss = r_miss.json();
    let items_miss = j_miss["items"].as_array().expect("items array");
    assert_eq!(items_miss.len(), 1);
    assert_eq!(items_miss[0]["id"], "missing-proj");
    assert_eq!(items_miss[0]["missing"], true);

    // 4. Empty query returns at most SUGGEST_LIMIT (10) items
    let r_all = srv.get("/api/projects/suggest");
    assert_eq!(r_all.status, 200);
    let j_all = r_all.json();
    let items_all = j_all["items"].as_array().expect("items array");
    assert_eq!(items_all.len(), 10);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn project_status_zero_token() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("web-ban-hang", &[], None, None);

    let entry = sb.project_entry(&repo, "web-ban-hang", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    let mut srv = sb.serve(&[]);

    // 1. GET /api/projects/web-ban-hang/status
    let r = srv.get("/api/projects/web-ban-hang/status");
    assert_eq!(r.status, 200);
    let j = r.json();

    assert_eq!(j["project"], "web-ban-hang");
    assert_eq!(j["branch"], "main");
    assert_eq!(j["clean"], true);
    assert!(j["task"].is_null());

    let log = j["log"].as_array().expect("log array");
    assert!(!log.is_empty(), "log should have init commit");
    assert!(log[0].as_str().unwrap().contains("init"));

    // Check zero token: SQLite runs table is empty and fake-claude record file absent
    let conn = sb.db();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM runs", [], |row| row.get(0))
        .expect("count runs");
    assert_eq!(count, 0, "runs table must remain empty");
    assert!(
        sb.records().is_empty(),
        "no claude process should be started"
    );

    // 2. Unknown id -> 404
    let r_not_found = srv.get("/api/projects/unknown-proj/status");
    assert_eq!(r_not_found.status, 404);
    assert!(r_not_found
        .text()
        .contains("Không tìm thấy dự án unknown-proj"));

    // 3. Project whose path is not a repo -> 422
    let not_repo_dir = sb.root.join("not-a-repo");
    std::fs::create_dir_all(&not_repo_dir).expect("create not-a-repo dir");
    let not_repo_entry = serde_json::json!({
        "id": "not-repo",
        "path": not_repo_dir.to_string_lossy(),
        "name_vi": "Không phải repo",
        "aliases": [],
        "type": "code",
        "profile": "work",
        "missing": false,
    });
    sb.write_map(serde_json::json!([entry, not_repo_entry]), &[]);

    let r_not_repo = srv.get("/api/projects/not-repo/status");
    assert_eq!(r_not_repo.status, 422);
    assert!(r_not_repo.text().contains("không phải repo git"));

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn dash12_alias_edit_preserves_other_keys() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("proj-a", &[], None, None);

    let entry = serde_json::json!({
        "id": "proj-a",
        "path": repo.to_string_lossy(),
        "name_vi": "Dự án A",
        "aliases": ["old-alias"],
        "type": "code",
        "profile": "work",
        "repo": null,
        "missing": false,
        "note": "giữ nguyên"
    });
    sb.write_map(serde_json::json!([entry]), &[]);

    let mut srv = sb.serve(&[]);

    let r_list = srv.get("/api/projects");
    assert_eq!(r_list.status, 200);
    let current_etag = r_list.json()["etag"].as_str().unwrap().to_string();

    let put_body = serde_json::json!({
        "aliases": ["  Shop ", "shop", "", "cửa hàng"],
        "etag": current_etag
    });
    let r_put = srv.put_json("/api/projects/proj-a", &put_body);
    assert_eq!(r_put.status, 200);
    let j_put = r_put.json();
    assert!(j_put.get("warnings").is_some());

    // Check file on disk:
    let map_data = sb.read_map();
    let projs = map_data["projects"].as_array().unwrap();
    assert_eq!(projs.len(), 1);
    let p0 = &projs[0];
    let aliases: Vec<&str> = p0["aliases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(aliases, vec!["Shop", "cửa hàng"]);
    assert_eq!(p0["note"], "giữ nguyên");
    assert_eq!(p0["id"], "proj-a");
    assert_eq!(p0["name_vi"], "Dự án A");
    assert_eq!(p0["profile"], "work");
    assert_eq!(p0["type"], "code");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(sb.map_path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    // Check SQLite: projects.aliases_json updated
    let conn = sb.db();
    let aliases_json: String = conn
        .query_row(
            "SELECT aliases_json FROM projects WHERE id = 'proj-a'",
            [],
            |r| r.get(0),
        )
        .expect("query aliases_json");
    assert!(aliases_json.contains("Shop"));
    assert!(aliases_json.contains("cửa hàng"));

    // GET /api/projects/suggest?q=cua hang finds the project
    let r_sug = srv.get("/api/projects/suggest?q=cua hang");
    assert_eq!(r_sug.status, 200);
    let items = r_sug.json()["items"].as_array().unwrap().clone();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "proj-a");
    assert_eq!(items[0]["rank"], "exact");

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn dash12_alias_edit_conflict_409() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("proj-a", &[], None, None);
    let entry = sb.project_entry(&repo, "proj-a", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    let mut srv = sb.serve(&[]);

    let r_list = srv.get("/api/projects");
    assert_eq!(r_list.status, 200);
    let old_etag = r_list.json()["etag"].as_str().unwrap().to_string();

    // Rewrite file on disk
    let mut e_new = sb.project_entry(&repo, "proj-a", "work");
    e_new["aliases"] = serde_json::json!(["hand-edited"]);
    sb.write_map(serde_json::json!([e_new]), &[]);

    // PUT with old_etag
    let put_body = serde_json::json!({
        "aliases": ["from-dashboard"],
        "etag": old_etag
    });
    let r_put = srv.put_json("/api/projects/proj-a", &put_body);
    assert_eq!(r_put.status, 409);
    let j_put = r_put.json();
    assert!(j_put["error"]
        .as_str()
        .unwrap()
        .contains("thay đổi ở nơi khác"));
    let fresh_projects = j_put["projects"].as_array().expect("fresh projects");
    assert_eq!(fresh_projects.len(), 1);
    assert_eq!(fresh_projects[0]["aliases"][0], "hand-edited");

    // File still holds the hand edit
    let map_data = sb.read_map();
    assert_eq!(
        map_data["projects"][0]["aliases"],
        serde_json::json!(["hand-edited"])
    );

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn dash12_duplicate_alias_warns_not_refuses() {
    let sb = Sandbox::new();
    let repo_a = sb.make_repo("proj-a", &[], None, None);
    let repo_b = sb.make_repo("proj-b", &[], None, None);

    let mut e_a = sb.project_entry(&repo_a, "proj-a", "work");
    e_a["aliases"] = serde_json::json!(["shop"]);
    let e_b = sb.project_entry(&repo_b, "proj-b", "work");
    sb.write_map(serde_json::json!([e_a, e_b]), &[]);

    let mut srv = sb.serve(&[]);

    let r_list = srv.get("/api/projects");
    assert_eq!(r_list.status, 200);
    let etag = r_list.json()["etag"].as_str().unwrap().to_string();

    let put_body = serde_json::json!({
        "aliases": ["shop"],
        "etag": etag
    });
    let r_put = srv.put_json("/api/projects/proj-b", &put_body);
    assert_eq!(r_put.status, 200);
    let j_put = r_put.json();
    let warnings = j_put["warnings"].as_array().expect("warnings array");
    let warn_str = warnings
        .iter()
        .filter_map(|w| w.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        warn_str.contains("tên/alias trùng \"shop\""),
        "warnings should mention duplicate shop, got: {warn_str}"
    );

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn d15_profile_not_editable() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("proj-a", &[], None, None);
    let entry = sb.project_entry(&repo, "proj-a", "work");
    sb.write_map(serde_json::json!([entry]), &[]);

    let mut srv = sb.serve(&[]);

    let r_list = srv.get("/api/projects");
    assert_eq!(r_list.status, 200);
    let etag = r_list.json()["etag"].as_str().unwrap().to_string();

    // PUT body with extra "profile" field -> 422
    let bad_body = serde_json::json!({
        "aliases": ["alias1"],
        "etag": etag,
        "profile": "personal"
    });
    let r_bad = srv.put_json("/api/projects/proj-a", &bad_body);
    assert_eq!(r_bad.status, 422);

    // Profile remains work
    let map_data = sb.read_map();
    assert_eq!(map_data["projects"][0]["profile"], "work");

    // PUT on unknown id -> 404
    let not_found_body = serde_json::json!({
        "aliases": ["alias1"],
        "etag": etag
    });
    let r_404 = srv.put_json("/api/projects/unknown-proj", &not_found_body);
    assert_eq!(r_404.status, 404);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn dash12_rescan_runs_cli_scan() {
    let sb = Sandbox::new();
    let repos_dir = sb.repos.clone();
    sb.write_map(serde_json::json!([]), &[repos_dir.to_str().unwrap()]);

    let mut srv = sb.serve(&[]);

    // Create a new git repo in scan_roots after serve starts
    let _repo_new = sb.make_repo("new-scanned-proj", &[], None, None);

    let r_scan = srv.post_json("/api/projects/scan", &serde_json::json!({}));
    assert_eq!(r_scan.status, 200);
    let j_scan = r_scan.json();
    assert_eq!(j_scan["exit_code"], 0);
    let stdout = j_scan["stdout"].as_array().expect("stdout array");
    let stdout_str = stdout
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stdout_str.contains("Đã quét"),
        "stdout should contain 'Đã quét', got: {stdout_str}"
    );

    // GET /api/projects now lists the new project
    let r_projs = srv.get("/api/projects");
    assert_eq!(r_projs.status, 200);
    let j_projs = r_projs.json();
    let projs = j_projs["projects"].as_array().expect("projects array");
    assert!(
        projs.iter().any(|p| p["id"] == "new-scanned-proj"),
        "projects should include new-scanned-proj"
    );

    // SQLite has its row
    let conn = sb.db();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projects WHERE id = 'new-scanned-proj'",
            [],
            |row| row.get(0),
        )
        .expect("count project row");
    assert_eq!(count, 1, "new-scanned-proj row must exist in SQLite");

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn dash12_rescan_without_roots_shows_cli_error() {
    let sb = Sandbox::new();
    sb.write_map(serde_json::json!([]), &[]);

    let mut srv = sb.serve(&[]);

    let r_scan = srv.post_json("/api/projects/scan", &serde_json::json!({}));
    assert_eq!(r_scan.status, 200);
    let j_scan = r_scan.json();
    assert_eq!(j_scan["exit_code"], 1);
    let stderr = j_scan["stderr"].as_array().expect("stderr array");
    let stderr_str = stderr
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stderr_str.contains("Chưa có scan_roots"),
        "stderr should contain 'Chưa có scan_roots', got: {stderr_str}"
    );

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}
