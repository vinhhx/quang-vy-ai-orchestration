mod common;

use common::Sandbox;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

fn team_names(j: &Value) -> Vec<String> {
    j["teams"]
        .as_array()
        .expect("teams array")
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

fn find<'a>(j: &'a Value, list: &str, key: &str, name: &str) -> &'a Value {
    j[list]
        .as_array()
        .unwrap_or_else(|| panic!("{list} array"))
        .iter()
        .find(|t| t[key] == name)
        .unwrap_or_else(|| panic!("{name} not in {list}: {j}"))
}

fn is_hex16(s: &str) -> bool {
    s.len() == 16 && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

fn stop(mut srv: common::Serve) {
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

fn doi_a_entry() -> Value {
    json!({
        "name": "doi-a",
        "description": "Thử",
        "roles": {
            "lead": "claude",
            "dev": "agy",
            "qa": "claude",
            "plan_review": "claude",
            "code_review": "claude"
        }
    })
}

#[test]
fn teams_get_save_get() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);

    let r = srv.get("/api/teams");
    assert_eq!(r.status, 200);
    let j = r.json();
    assert_eq!(j["version"], "absent");
    assert_eq!(j["errors"], json!([]));
    assert_eq!(team_names(&j), ["mac-dinh", "tiet-kiem"]);
    assert_eq!(j["builtin_names"], json!(["mac-dinh", "tiet-kiem"]));
    for t in j["teams"].as_array().unwrap() {
        assert_eq!(t["source"], "builtin");
    }
    let tk = find(&j, "teams", "name", "tiet-kiem");
    assert_eq!(tk["roles"]["dev"], "deepseek");
    assert_eq!(
        tk["summary"],
        "claude: lead, qa, review plan, review code · deepseek: dev"
    );

    let body = json!({"version": "absent", "create": true, "entry": doi_a_entry()});
    let r = srv.post_json("/api/teams/save", &body);
    assert_eq!(r.status, 200, "{}", r.text());
    let saved = r.json();
    assert_eq!(saved["ok"], true);
    let a = find(&saved, "teams", "name", "doi-a");
    assert_eq!(a["source"], "file");
    assert_eq!(
        a["summary"],
        "claude: lead, qa, review plan, review code · agy: dev"
    );
    let version = saved["version"].as_str().unwrap().to_string();
    assert!(is_hex16(&version), "version: {version}");

    let path = sb.teams_path();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.ends_with('\n'));
    let on_disk: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        on_disk,
        json!({"doi-a": {"description": "Thử", "roles": {
            "lead": "claude", "dev": "agy", "qa": "claude",
            "plan-review": "claude", "code-review": "claude"
        }}})
    );
    let leftovers: Vec<String> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "temp files left: {leftovers:?}");

    let again = srv.get("/api/teams").json();
    assert_eq!(again["version"], version.as_str());
    assert!(team_names(&again).contains(&"doi-a".to_string()));

    stop(srv);
}

// ---- Task 2: team rules, delete, conflicts, staff ----

fn team_entry(name: &str, description: &str, roles: Value) -> Value {
    let mut full = json!({
        "lead": "claude", "dev": "claude", "qa": "claude",
        "plan_review": "claude", "code_review": "claude"
    });
    for (k, v) in roles.as_object().unwrap() {
        full[k] = v.clone();
    }
    json!({"name": name, "description": description, "roles": full})
}

fn save_team(srv: &common::Serve, version: &str, create: bool, entry: Value) -> common::HttpResp {
    srv.post_json(
        "/api/teams/save",
        &json!({"version": version, "create": create, "entry": entry}),
    )
}

fn delete_team(srv: &common::Serve, version: &str, name: &str) -> common::HttpResp {
    srv.post_json(
        "/api/teams/delete",
        &json!({"version": version, "name": name}),
    )
}

fn teams_version(srv: &common::Serve) -> String {
    srv.get("/api/teams").json()["version"]
        .as_str()
        .unwrap()
        .to_string()
}

fn refusals(j: &Value) -> Vec<(String, String)> {
    j["errors"]
        .as_array()
        .unwrap_or_else(|| panic!("errors array in {j}"))
        .iter()
        .map(|e| {
            (
                e["field"].as_str().unwrap_or("").to_string(),
                e["message"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn one_refusal(r: &common::HttpResp) -> (String, String) {
    assert_eq!(r.status, 422, "{}", r.text());
    let j = r.json();
    assert_eq!(j["error"], "Không lưu được đội hình");
    let all = refusals(&j);
    assert_eq!(all.len(), 1, "{all:?}");
    all.into_iter().next().unwrap()
}

fn pair(field: &str, message: &str) -> (String, String) {
    (field.to_string(), message.to_string())
}

fn disk_json(path: &std::path::Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

const READONLY_TEAM: &str = "Đội hình có sẵn chỉ đọc — bấm Nhân bản để tạo bản riêng.";

#[test]
fn teams_save_rules() {
    let sb = Sandbox::new();
    sb.write_staff(json!({"ha": {"display_name": "Hà"}}));
    let srv = sb.serve(&[]);
    let v = teams_version(&srv);
    assert_eq!(v, "absent");

    let r = save_team(
        &srv,
        &v,
        true,
        team_entry("doi-x", "x", json!({"qa": "deepseek"})),
    );
    assert_eq!(r.status, 422);
    assert!(refusals(&r.json()).contains(&pair("qa", "QA phải dùng engine claude.")));

    let r = save_team(
        &srv,
        &v,
        true,
        team_entry("doi-x", "x", json!({"lead": "người:ha"})),
    );
    assert_eq!(
        one_refusal(&r),
        pair("lead", "Engine người chỉ dùng cho vai trò dev.")
    );

    // Hyphenated roles come back under the request's underscore names.
    let r = save_team(
        &srv,
        &v,
        true,
        team_entry("doi-x", "x", json!({"plan_review": "người:ha"})),
    );
    assert_eq!(one_refusal(&r).0, "plan_review");

    let r = save_team(&srv, &v, true, team_entry("mac-dinh", "x", json!({})));
    assert_eq!(
        one_refusal(&r),
        pair(
            "name",
            "'mac-dinh' là đội hình có sẵn, không ghi đè được. Đặt tên khác."
        )
    );

    let r = save_team(&srv, &v, true, team_entry("Doi-A", "x", json!({})));
    assert_eq!(
        one_refusal(&r),
        pair(
            "name",
            "Tên chỉ gồm chữ thường không dấu, số và dấu gạch ngang (ví dụ tiet-kiem)."
        )
    );

    let r = save_team(&srv, &v, true, team_entry("doi-x", "   ", json!({})));
    assert_eq!(
        one_refusal(&r),
        pair("description", "Nhập mô tả ngắn để chọn đội hình cho đúng.")
    );

    // Nothing was written by any refusal.
    assert!(!sb.teams_path().exists());

    let r = save_team(
        &srv,
        &v,
        true,
        team_entry("doi-a", "Thử", json!({"dev": "agy"})),
    );
    assert_eq!(r.status, 200, "{}", r.text());
    let v2 = r.json()["version"].as_str().unwrap().to_string();
    let r = save_team(&srv, &v2, true, team_entry("doi-a", "Thử", json!({})));
    assert_eq!(one_refusal(&r), pair("name", "Đã có đội hình tên 'doi-a'."));

    // A team whose dev is a person saves, and the summary names the person.
    let r = save_team(
        &srv,
        &v2,
        true,
        team_entry("doi-b", "Có người", json!({"dev": "người:ha"})),
    );
    assert_eq!(r.status, 200, "{}", r.text());
    let j = r.json();
    let b = find(&j, "teams", "name", "doi-b");
    assert_eq!(
        b["summary"],
        "claude: lead, qa, review plan, review code · người Hà: dev"
    );

    // Unknown body keys are refused by the body shape.
    let r = srv.post_json(
        "/api/teams/save",
        &json!({"version": v2, "create": true, "entry": {"name": "q", "command": "rm"}}),
    );
    assert!(r.status == 422 || r.status == 400, "{}", r.status);

    stop(srv);
}

#[test]
fn teams_update_and_override() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);
    let v = teams_version(&srv);

    let r = save_team(
        &srv,
        &v,
        true,
        team_entry("doi-a", "Thử", json!({"dev": "agy"})),
    );
    assert_eq!(r.status, 200, "{}", r.text());
    let v = r.json()["version"].as_str().unwrap().to_string();

    // A built-in-only team is read-only.
    let r = save_team(&srv, &v, false, team_entry("tiet-kiem", "x", json!({})));
    assert_eq!(one_refusal(&r), pair("name", READONLY_TEAM));
    // Unknown name.
    let r = save_team(&srv, &v, false, team_entry("x", "x", json!({})));
    assert_eq!(
        one_refusal(&r),
        pair("name", "Không có đội hình 'x' trong teams.json.")
    );

    // Update keeps the name and changes the entry.
    let r = save_team(
        &srv,
        &v,
        false,
        team_entry("doi-a", "Thử 2", json!({"dev": "deepseek"})),
    );
    assert_eq!(r.status, 200, "{}", r.text());
    let j = r.json();
    let a = find(&j, "teams", "name", "doi-a");
    assert_eq!(a["roles"]["dev"], "deepseek");
    assert_eq!(a["description"], "Thử 2");
    assert_eq!(
        disk_json(&sb.teams_path())["doi-a"]["roles"]["dev"],
        "deepseek"
    );

    // A hand-made override of a built-in stays editable and deletable.
    sb.write_teams(json!({
        "doi-a": {"description": "Thử", "roles": {"dev": "agy"}},
        "mac-dinh": {"description": "Ghi đè", "roles": {"dev": "deepseek"}}
    }));
    let g = srv.get("/api/teams").json();
    assert_eq!(find(&g, "teams", "name", "mac-dinh")["source"], "override");
    let v = g["version"].as_str().unwrap().to_string();

    let r = save_team(
        &srv,
        &v,
        false,
        team_entry("mac-dinh", "Ghi đè 2", json!({"dev": "agy"})),
    );
    assert_eq!(r.status, 200, "{}", r.text());
    let j = r.json();
    let md = find(&j, "teams", "name", "mac-dinh");
    assert_eq!(md["source"], "override");
    assert_eq!(md["description"], "Ghi đè 2");
    let v = j["version"].as_str().unwrap().to_string();

    // Creating over an existing override is a duplicate, not a built-in clash.
    let r = save_team(&srv, &v, true, team_entry("mac-dinh", "x", json!({})));
    assert_eq!(
        one_refusal(&r),
        pair("name", "Đã có đội hình tên 'mac-dinh'.")
    );

    let r = delete_team(&srv, &v, "mac-dinh");
    assert_eq!(r.status, 200, "{}", r.text());
    let j = r.json();
    let md = find(&j, "teams", "name", "mac-dinh");
    assert_eq!(md["source"], "builtin");
    assert_eq!(
        md["description"],
        "Mọi vai trò chạy bằng claude theo profile của dự án."
    );

    stop(srv);
}

#[test]
fn teams_delete_rules() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);
    let v = teams_version(&srv);
    let r = save_team(
        &srv,
        &v,
        true,
        team_entry("doi-a", "Thử", json!({"dev": "agy"})),
    );
    let v = r.json()["version"].as_str().unwrap().to_string();

    let r = delete_team(&srv, &v, "tiet-kiem");
    assert_eq!(one_refusal(&r), pair("name", READONLY_TEAM));
    let r = delete_team(&srv, &v, "x");
    assert_eq!(
        one_refusal(&r),
        pair("name", "Không có đội hình 'x' trong teams.json.")
    );

    let r = delete_team(&srv, &v, "doi-a");
    assert_eq!(r.status, 200, "{}", r.text());
    let j = r.json();
    assert_eq!(j["ok"], true);
    assert_eq!(team_names(&j), ["mac-dinh", "tiet-kiem"]);
    assert_eq!(disk_json(&sb.teams_path()), json!({}));

    stop(srv);
}

#[test]
fn teams_conflict_keeps_file() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);
    let v = teams_version(&srv);
    let r = save_team(&srv, &v, true, team_entry("doi-a", "Thử", json!({})));
    let stale = r.json()["version"].as_str().unwrap().to_string();

    // Someone edits the file after the dashboard read it.
    sb.write_teams(json!({
        "doi-a": {"description": "Thử", "roles": {}},
        "doi-tay": {"description": "Viết tay", "roles": {}}
    }));
    let before = std::fs::read(sb.teams_path()).unwrap();

    let r = save_team(&srv, &stale, true, team_entry("doi-moi", "Mới", json!({})));
    assert_eq!(r.status, 409, "{}", r.text());
    let j = r.json();
    assert_eq!(j["code"], "conflict");
    assert_eq!(j["error"], "teams.json vừa bị sửa ở nơi khác");
    assert_eq!(std::fs::read(sb.teams_path()).unwrap(), before);

    let r = delete_team(&srv, &stale, "doi-a");
    assert_eq!(r.status, 409);
    assert_eq!(r.json()["code"], "conflict");
    assert_eq!(std::fs::read(sb.teams_path()).unwrap(), before);

    // With the fresh version the same write goes through.
    let fresh = teams_version(&srv);
    let r = save_team(&srv, &fresh, true, team_entry("doi-moi", "Mới", json!({})));
    assert_eq!(r.status, 200, "{}", r.text());

    stop(srv);
}

#[test]
fn teams_invalid_file_blocks_writes() {
    let sb = Sandbox::new();
    sb.write_teams(json!({"doi-x": {"description": "x", "roles": {"qa": "agy"}}}));
    let srv = sb.serve(&[]);
    let before = std::fs::read(sb.teams_path()).unwrap();

    let g = srv.get("/api/teams");
    assert_eq!(g.status, 200);
    let gj = g.json();
    let errors: Vec<String> = gj["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_str().unwrap().to_string())
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].ends_with("đội hình 'doi-x': QA phải dùng engine claude."));
    assert_eq!(team_names(&gj), ["mac-dinh", "tiet-kiem"]);
    let v = gj["version"].as_str().unwrap().to_string();

    let r = save_team(&srv, &v, true, team_entry("doi-ok", "Tốt", json!({})));
    assert_eq!(r.status, 409, "{}", r.text());
    let j = r.json();
    assert_eq!(j["code"], "invalid_file");
    assert_eq!(
        j["error"],
        "teams.json đang có lỗi — sửa file trong editor rồi bấm Đọc lại; dashboard không ghi đè file lỗi."
    );
    assert_eq!(j["errors"], gj["errors"]);

    let r = delete_team(&srv, &v, "doi-x");
    assert_eq!(r.status, 409);
    assert_eq!(r.json()["code"], "invalid_file");

    assert_eq!(std::fs::read(sb.teams_path()).unwrap(), before);
    stop(srv);
}

#[test]
fn teams_untouched_entries_preserved() {
    let sb = Sandbox::new();
    let dir = sb.teams_path().parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        sb.teams_path(),
        "{\n  \"z-doi\":   {\"description\": \"Z\", \"roles\": {\"dev\": \"deepseek\"}},\n  \"a-doi\": {\"description\": \"A\", \"roles\": {}}\n}\n",
    )
    .unwrap();
    let srv = sb.serve(&[]);
    let v = teams_version(&srv);

    let r = save_team(
        &srv,
        &v,
        false,
        team_entry("a-doi", "A2", json!({"dev": "agy"})),
    );
    assert_eq!(r.status, 200, "{}", r.text());

    let disk = disk_json(&sb.teams_path());
    let keys: Vec<&String> = disk.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["z-doi", "a-doi"]);
    assert_eq!(
        disk["z-doi"],
        json!({"description": "Z", "roles": {"dev": "deepseek"}})
    );
    assert_eq!(disk["a-doi"]["description"], "A2");
    assert_eq!(disk["a-doi"]["roles"]["dev"], "agy");

    // Deleting one entry leaves the other's value and the order intact.
    let v = r.json()["version"].as_str().unwrap().to_string();
    let r = delete_team(&srv, &v, "a-doi");
    assert_eq!(r.status, 200, "{}", r.text());
    assert_eq!(
        disk_json(&sb.teams_path()),
        json!({"z-doi": {"description": "Z", "roles": {"dev": "deepseek"}}})
    );

    stop(srv);
}

fn save_staff(srv: &common::Serve, version: &str, create: bool, entry: Value) -> common::HttpResp {
    srv.post_json(
        "/api/staff/save",
        &json!({"version": version, "create": create, "entry": entry}),
    )
}

fn delete_staff(srv: &common::Serve, version: &str, id: &str) -> common::HttpResp {
    srv.post_json("/api/staff/delete", &json!({"version": version, "id": id}))
}

fn staff_version(srv: &common::Serve) -> String {
    srv.get("/api/staff").json()["version"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn staff_crud_and_usage() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);

    let g = srv.get("/api/staff");
    assert_eq!(g.status, 200);
    let gj = g.json();
    assert_eq!(gj["staff"], json!([]));
    assert_eq!(gj["version"], "absent");
    assert_eq!(gj["errors"], json!([]));

    let sv = gj["version"].as_str().unwrap().to_string();
    let r = save_staff(
        &srv,
        &sv,
        true,
        json!({"id": "ha", "display_name": "Hà", "notes": "Zalo 09xx"}),
    );
    assert_eq!(r.status, 200, "{}", r.text());
    let j = r.json();
    assert_eq!(j["ok"], true);
    let ha = find(&j, "staff", "id", "ha");
    assert_eq!(ha["display_name"], "Hà");
    assert_eq!(ha["notes"], "Zalo 09xx");
    assert_eq!(ha["used_in"], json!([]));
    let sv = j["version"].as_str().unwrap().to_string();
    assert!(is_hex16(&sv));
    let staff_mode = std::fs::metadata(sb.staff_path())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(staff_mode, 0o600);

    // Duplicate, bad id, blank name.
    let r = save_staff(&srv, &sv, true, json!({"id": "ha", "display_name": "Hà 2"}));
    assert_eq!(r.status, 422);
    assert_eq!(r.json()["error"], "Không lưu được nhân sự");
    assert_eq!(refusals(&r.json()), [pair("id", "Đã có nhân sự mã 'ha'.")]);
    let r = save_staff(&srv, &sv, true, json!({"id": "Ha", "display_name": "Hà"}));
    assert_eq!(
        refusals(&r.json()),
        [pair(
            "id",
            "Mã chỉ gồm chữ thường không dấu, số và dấu gạch ngang."
        )]
    );
    let r = save_staff(&srv, &sv, true, json!({"id": "an", "display_name": " "}));
    assert_eq!(
        refusals(&r.json()),
        [pair("display_name", "Nhập tên hiển thị.")]
    );

    // Update keeps the id.
    let r = save_staff(
        &srv,
        &sv,
        false,
        json!({"id": "ha", "display_name": "Hà Nội", "notes": ""}),
    );
    assert_eq!(r.status, 200, "{}", r.text());
    let j = r.json();
    assert_eq!(find(&j, "staff", "id", "ha")["display_name"], "Hà Nội");
    assert_eq!(find(&j, "staff", "id", "ha")["notes"], "");
    let sv = j["version"].as_str().unwrap().to_string();
    let r = save_staff(
        &srv,
        &sv,
        false,
        json!({"id": "khong-co", "display_name": "X"}),
    );
    assert_eq!(
        refusals(&r.json()),
        [pair("id", "Không có nhân sự 'khong-co' trong staff.json.")]
    );

    // A team that gives dev to the person.
    let tv = teams_version(&srv);
    let r = save_team(
        &srv,
        &tv,
        true,
        team_entry("doi-b", "Có người", json!({"dev": "người:ha"})),
    );
    assert_eq!(r.status, 200, "{}", r.text());
    let tv = r.json()["version"].as_str().unwrap().to_string();
    let g = srv.get("/api/staff").json();
    assert_eq!(find(&g, "staff", "id", "ha")["used_in"], json!(["doi-b"]));

    // Deleting a person that a team uses is refused.
    let r = delete_staff(&srv, &sv, "ha");
    assert_eq!(r.status, 422, "{}", r.text());
    assert_eq!(
        refusals(&r.json()),
        [pair(
            "id",
            "Đang dùng trong đội hình: doi-b. Đổi engine của vai trò đó trước rồi xóa."
        )]
    );
    assert!(std::fs::read_to_string(sb.staff_path())
        .unwrap()
        .contains("\"ha\""));

    let r = delete_team(&srv, &tv, "doi-b");
    assert_eq!(r.status, 200, "{}", r.text());
    let r = delete_staff(&srv, &sv, "khong-co");
    assert_eq!(
        refusals(&r.json()),
        [pair("id", "Không có nhân sự 'khong-co' trong staff.json.")]
    );
    let r = delete_staff(&srv, &sv, "ha");
    assert_eq!(r.status, 200, "{}", r.text());
    assert_eq!(r.json()["staff"], json!([]));

    // A stale token is a conflict and writes nothing.
    let before = std::fs::read(sb.staff_path()).unwrap();
    let r = save_staff(&srv, &sv, true, json!({"id": "an", "display_name": "An"}));
    assert_eq!(r.status, 409, "{}", r.text());
    assert_eq!(r.json()["code"], "conflict");
    assert_eq!(r.json()["error"], "file nhân sự vừa bị sửa ở nơi khác");
    assert_eq!(std::fs::read(sb.staff_path()).unwrap(), before);

    // An invalid staff.json is never overwritten.
    std::fs::write(sb.staff_path(), r#"{"Ha": {"display_name": "Hà"}}"#).unwrap();
    let before = std::fs::read(sb.staff_path()).unwrap();
    let sv = staff_version(&srv);
    let r = save_staff(&srv, &sv, true, json!({"id": "an", "display_name": "An"}));
    assert_eq!(r.status, 409, "{}", r.text());
    let j = r.json();
    assert_eq!(j["code"], "invalid_file");
    assert_eq!(
        j["error"],
        "staff.json đang có lỗi — sửa file trong editor rồi bấm Đọc lại; dashboard không ghi đè file lỗi."
    );
    assert_eq!(j["errors"].as_array().unwrap().len(), 1);
    assert_eq!(std::fs::read(sb.staff_path()).unwrap(), before);

    stop(srv);
}
