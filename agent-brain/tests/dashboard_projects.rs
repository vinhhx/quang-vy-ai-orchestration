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
