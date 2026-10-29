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
            "SELECT aliases_json FROM projects WHERE slug = 'proj-a'",
            [],
            |r| r.get(0),
        )
        .expect("query aliases_json");
    assert!(aliases_json.contains("Shop"));
    assert!(aliases_json.contains("cửa hàng"));

    // GET /api/projects/suggest?q=cua%20hang finds the project
    let r_sug = srv.get("/api/projects/suggest?q=cua%20hang");
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
            "SELECT COUNT(*) FROM projects WHERE slug = 'new-scanned-proj'",
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

// ---- Telegram toggle per project ----

fn patch_map(sb: &Sandbox, f: impl FnOnce(&mut serde_json::Value)) {
    let mut m = sb.read_map();
    f(&mut m);
    std::fs::write(sb.map_path(), serde_json::to_string_pretty(&m).unwrap()).expect("write map");
}

fn tg_etag(srv: &common::Serve) -> String {
    srv.get("/api/projects").json()["etag"]
        .as_str()
        .unwrap()
        .to_string()
}

fn tg_put(srv: &common::Serve, id: &str, enabled: bool, etag: &str) -> common::HttpResp {
    srv.put_json(
        &format!("/api/projects/{id}/telegram"),
        &serde_json::json!({"enabled": enabled, "etag": etag}),
    )
}

#[test]
fn telegram_toggle_flow() {
    let sb = Sandbox::new();
    let repo_a = sb.make_repo("tg-a", &[], None, None);
    let repo_b = sb.make_repo("tg-b", &[], None, None);
    let mut ea = sb.project_entry(&repo_a, "tg-a", "work");
    ea["aliases"] = serde_json::json!(["alpha"]);
    let eb = sb.project_entry(&repo_b, "tg-b", "work");
    let mut em = sb.project_entry(&repo_a, "tg-media", "work");
    em["type"] = serde_json::json!("media");
    let mut ex = sb.project_entry(&repo_a, "tg-gone", "work");
    ex["missing"] = serde_json::json!(true);
    sb.write_map(serde_json::json!([ea, eb, em, ex]), &["/some/root"]);
    patch_map(&sb, |m| {
        m["telegram_projects"] = serde_json::json!([7, "other", {"x": 1}]);
        m["extra_key"] = serde_json::json!({"keep": true});
    });

    let mut srv = sb.serve(&[]);

    // (a) list exposes the new fields
    let j = srv.get("/api/projects").json();
    assert_eq!(j["telegram_max"], 30);
    assert_eq!(j["telegram_count"], 0);
    for p in j["projects"].as_array().unwrap() {
        assert!(p["telegram"].is_boolean());
        assert!(p["telegram_offerable"].is_boolean());
    }
    let by_id = |j: &serde_json::Value, id: &str| {
        j["projects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(by_id(&j, "tg-a")["telegram_offerable"], true);
    assert_eq!(by_id(&j, "tg-media")["telegram_offerable"], false);
    assert_eq!(by_id(&j, "tg-gone")["telegram_offerable"], false);

    // (b) enable appends at the end, keeps other keys, new etag
    let etag0 = tg_etag(&srv);
    let before = sb.read_map();
    let r = tg_put(&srv, "tg-a", true, &etag0);
    assert_eq!(r.status, 200);
    let rj = r.json();
    assert_ne!(rj["etag"].as_str().unwrap(), etag0);
    assert_eq!(by_id(&rj, "tg-a")["telegram"], true);
    assert_eq!(rj["telegram_count"], 1);
    let after = sb.read_map();
    assert_eq!(
        after["telegram_projects"],
        serde_json::json!([7, "other", {"x": 1}, "tg-a"])
    );
    assert_eq!(after["projects"], before["projects"]);
    assert_eq!(after["scan_roots"], before["scan_roots"]);
    assert_eq!(after["extra_key"], before["extra_key"]);
    assert_eq!(
        after["projects"][0]["aliases"],
        serde_json::json!(["alpha"])
    );

    // (c) enabling twice does not duplicate
    let r = tg_put(&srv, "tg-a", true, rj["etag"].as_str().unwrap());
    assert_eq!(r.status, 200);
    assert_eq!(
        sb.read_map()["telegram_projects"],
        serde_json::json!([7, "other", {"x": 1}, "tg-a"])
    );

    // (d) disable removes only this id, keeps order and non-strings
    let etag = tg_etag(&srv);
    assert_eq!(tg_put(&srv, "tg-b", true, &etag).status, 200);
    let etag = tg_etag(&srv);
    assert_eq!(tg_put(&srv, "tg-a", false, &etag).status, 200);
    assert_eq!(
        sb.read_map()["telegram_projects"],
        serde_json::json!([7, "other", {"x": 1}, "tg-b"])
    );
    let etag = tg_etag(&srv);
    assert_eq!(tg_put(&srv, "tg-a", false, &etag).status, 200);
    assert_eq!(
        sb.read_map()["telegram_projects"],
        serde_json::json!([7, "other", {"x": 1}, "tg-b"])
    );

    // (e) stale etag -> 409 with fresh list, file unchanged
    let file_before = std::fs::read_to_string(sb.map_path()).unwrap();
    let r = tg_put(&srv, "tg-a", true, &etag0);
    assert_eq!(r.status, 409);
    assert!(r.json()["projects"].is_array());
    assert_eq!(std::fs::read_to_string(sb.map_path()).unwrap(), file_before);

    // (f) media and missing -> 422, file unchanged
    let etag = tg_etag(&srv);
    let r = tg_put(&srv, "tg-media", true, &etag);
    assert_eq!(r.status, 422);
    assert!(r.json()["error"].as_str().unwrap().contains("media"));
    let r = tg_put(&srv, "tg-gone", true, &etag);
    assert_eq!(r.status, 422);
    assert_eq!(std::fs::read_to_string(sb.map_path()).unwrap(), file_before);

    // (h) unknown id -> 404
    assert_eq!(tg_put(&srv, "nope", true, &etag).status, 404);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn telegram_toggle_limit_30() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("lim", &[], None, None);
    let entries: Vec<serde_json::Value> = (0..31)
        .map(|i| sb.project_entry(&repo, &format!("lim-{i}"), "work"))
        .collect();
    sb.write_map(serde_json::json!(entries), &[]);

    let mut srv = sb.serve(&[]);
    for i in 0..30 {
        let etag = tg_etag(&srv);
        assert_eq!(tg_put(&srv, &format!("lim-{i}"), true, &etag).status, 200);
    }
    let etag = tg_etag(&srv);
    let r = tg_put(&srv, "lim-30", true, &etag);
    assert_eq!(r.status, 422);
    assert!(r.json()["error"].as_str().unwrap().contains("30"));
    assert_eq!(
        sb.read_map()["telegram_projects"].as_array().unwrap().len(),
        30
    );
    // re-enabling one already on is still fine at the limit
    assert_eq!(tg_put(&srv, "lim-0", true, &etag).status, 200);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

// ---- Scan roots editor, add project, folder picker ----

fn roots_put(srv: &common::Serve, roots: &[&str], etag: &str) -> common::HttpResp {
    srv.put_json(
        "/api/projects/scan-roots",
        &serde_json::json!({"roots": roots, "etag": etag}),
    )
}

fn add_post(srv: &common::Serve, path: &str, kind: &str, etag: &str) -> common::HttpResp {
    srv.post_json(
        "/api/projects",
        &serde_json::json!({"path": path, "type": kind, "etag": etag}),
    )
}

#[test]
fn scan_roots_editor() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("sr-a", &[], None, None);
    let entry = sb.project_entry(&repo, "sr-a", "work");
    sb.write_map(serde_json::json!([entry]), &["/old/root"]);
    patch_map(&sb, |m| {
        m["telegram_projects"] = serde_json::json!(["sr-a"]);
    });
    let dir1 = sb.root.join("roots-1");
    let dir2 = sb.root.join("roots-2");
    std::fs::create_dir_all(&dir1).unwrap();
    std::fs::create_dir_all(&dir2).unwrap();
    let d1 = dir1.to_str().unwrap().to_string();
    let d1_dup = format!("{}/", d1);
    let d2 = dir2.to_str().unwrap().to_string();

    let mut srv = sb.serve(&[]);
    let etag = tg_etag(&srv);
    let before = sb.read_map();

    // valid: saved as typed (trimmed), duplicates by canonical path removed, other keys kept
    let padded = format!("  {d1}  ");
    let r = roots_put(&srv, &[&padded, &d1_dup, "", &d2], &etag);
    assert_eq!(r.status, 200, "{}", r.text());
    let rj = r.json();
    assert_eq!(rj["scan_roots"], serde_json::json!([d1, d2]));
    let after = sb.read_map();
    assert_eq!(after["scan_roots"], serde_json::json!([d1, d2]));
    assert_eq!(after["projects"], before["projects"]);
    assert_eq!(after["telegram_projects"], before["telegram_projects"]);

    // route is not swallowed by /api/projects/{id}: the 404 message of that route never shows
    assert_ne!(r.status, 404);

    let file_before = std::fs::read_to_string(sb.map_path()).unwrap();
    let etag = tg_etag(&srv);
    let r = roots_put(&srv, &["/tmp/../etc"], &etag);
    assert_eq!(r.status, 422);
    let r = roots_put(&srv, &[sb.root.join("nope-dir").to_str().unwrap()], &etag);
    assert_eq!(r.status, 422);
    assert!(r.json()["error"]
        .as_str()
        .unwrap()
        .starts_with("Thư mục không tồn tại"));
    let r = roots_put(&srv, &["relative/dir"], &etag);
    assert_eq!(r.status, 422);
    let many: Vec<&str> = (0..21).map(|_| d1.as_str()).collect();
    let r = roots_put(&srv, &many, &etag);
    assert_eq!(r.status, 422);
    let r = roots_put(&srv, &[&d1], "stale-etag");
    assert_eq!(r.status, 409);
    assert_eq!(std::fs::read_to_string(sb.map_path()).unwrap(), file_before);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn add_project_by_path() {
    let sb = Sandbox::new();
    sb.write_map(serde_json::json!([]), &[]);
    let repo = sb.make_repo("ap-git", &[], None, None);
    let plain = sb.root.join("ap-plain");
    std::fs::create_dir_all(&plain).unwrap();
    let plain_s = plain.to_str().unwrap().to_string();
    let repo_s = repo.to_str().unwrap().to_string();

    let mut srv = sb.serve(&[]);

    let etag = tg_etag(&srv);
    let r = add_post(&srv, &repo_s, "code", &etag);
    assert_eq!(r.status, 201, "{}", r.text());
    let rj = r.json();
    assert_eq!(rj["project"]["type"], "code");
    assert!(rj["projects"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == rj["project"]["id"]));
    assert_eq!(sb.read_map()["projects"].as_array().unwrap().len(), 1);

    // same path again -> 200, no duplicate
    let etag = tg_etag(&srv);
    let r = add_post(&srv, &repo_s, "code", &etag);
    assert_eq!(r.status, 200);
    assert_eq!(sb.read_map()["projects"].as_array().unwrap().len(), 1);

    // code without git -> 422
    let etag = tg_etag(&srv);
    let r = add_post(&srv, &plain_s, "code", &etag);
    assert_eq!(r.status, 422);
    assert!(r.json()["error"].as_str().unwrap().contains("git"));
    assert_eq!(sb.read_map()["projects"].as_array().unwrap().len(), 1);

    // unknown type -> 422
    let r = add_post(&srv, &plain_s, "weird", &etag);
    assert_eq!(r.status, 422);

    // idea without git -> 201, no .git, no shadow
    let r = add_post(&srv, &plain_s, "idea", &etag);
    assert_eq!(r.status, 201, "{}", r.text());
    assert_eq!(r.json()["project"]["type"], "idea");
    assert!(!plain.join(".git").exists());
    assert_eq!(std::fs::read_dir(&plain).unwrap().count(), 0);
    assert!(!sb.xdg_data.join("agent-brain").join("shadow").exists());

    // media without git -> 201
    let media = sb.root.join("ap-media");
    std::fs::create_dir_all(&media).unwrap();
    let etag = tg_etag(&srv);
    let r = add_post(&srv, media.to_str().unwrap(), "media", &etag);
    assert_eq!(r.status, 201);

    // stale etag -> 409; relative/.. -> 422
    assert_eq!(add_post(&srv, &plain_s, "idea", "old").status, 409);
    let etag = tg_etag(&srv);
    assert_eq!(add_post(&srv, "rel/path", "idea", &etag).status, 422);
    assert_eq!(add_post(&srv, "/tmp/../tmp", "idea", &etag).status, 422);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}

#[test]
fn fs_dirs_lists_directories_only() {
    let sb = Sandbox::new();
    sb.write_map(serde_json::json!([]), &[]);
    let base = sb.root.join("fsdirs");
    std::fs::create_dir_all(base.join("zeta")).unwrap();
    std::fs::create_dir_all(base.join("alpha/.git")).unwrap();
    std::fs::create_dir_all(base.join(".hidden")).unwrap();
    std::fs::write(base.join("secret.txt"), "x").unwrap();
    let base_s = base.to_str().unwrap().to_string();

    let mut srv = sb.serve(&[]);

    let r = srv.get(&format!("/api/fs/dirs?path={base_s}"));
    assert_eq!(r.status, 200, "{}", r.text());
    let j = r.json();
    assert_eq!(j["truncated"], false);
    assert!(j["parent"].is_string());
    let names: Vec<&str> = j["dirs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["alpha", "zeta"]);
    assert_eq!(j["dirs"][0]["git"], true);
    assert_eq!(j["dirs"][1]["git"], false);
    assert!(!j.to_string().contains("secret.txt"));

    let j = srv
        .get(&format!("/api/fs/dirs?path={base_s}&hidden=1"))
        .json();
    let names: Vec<&str> = j["dirs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec![".hidden", "alpha", "zeta"]);

    // no path -> HOME
    let r = srv.get("/api/fs/dirs");
    assert_eq!(r.status, 200);

    // truncation at 300
    let many = sb.root.join("many");
    for i in 0..305 {
        std::fs::create_dir_all(many.join(format!("d{i:03}"))).unwrap();
    }
    let j = srv
        .get(&format!("/api/fs/dirs?path={}", many.to_str().unwrap()))
        .json();
    assert_eq!(j["dirs"].as_array().unwrap().len(), 300);
    assert_eq!(j["truncated"], true);

    // bad paths
    assert_eq!(srv.get("/api/fs/dirs?path=relative").status, 422);
    assert_eq!(srv.get("/api/fs/dirs?path=/tmp/../etc").status, 422);
    assert_eq!(
        srv.get(&format!("/api/fs/dirs?path={base_s}/missing"))
            .status,
        422
    );
    assert_eq!(
        srv.get(&format!("/api/fs/dirs?path={base_s}/secret.txt"))
            .status,
        422
    );

    // no token -> refused like other routes
    let r = srv.request("GET", &format!("/api/fs/dirs?path={base_s}"), &[], b"");
    assert!(r.status == 401 || r.status == 403, "got {}", r.status);

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}
