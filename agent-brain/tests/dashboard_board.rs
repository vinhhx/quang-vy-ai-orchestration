mod common;

use common::Sandbox;
use serde_json::{json, Value};
use std::time::Duration;

/// Một file plan có dạng như plan GSD thật: front matter có khối lồng và dòng chứa `:` và `---`.
fn plan_md(dir: &str, plan: &str, wave: u32, deps: &str) -> String {
    plan_md_obj(dir, plan, wave, deps, "Câu đầu của mục tiêu. Câu sau.")
}

/// Như `plan_md` nhưng dòng đầu của `<objective>` do người gọi chọn.
fn plan_md_obj(dir: &str, plan: &str, wave: u32, deps: &str, objective: &str) -> String {
    format!(
        "---\n\
phase: {dir}\n\
plan: {plan}\n\
type: execute\n\
wave: {wave}\n\
depends_on: {deps}\n\
files_modified:\n\
  - src/app.py\n\
autonomous: true\n\
requirements: [DEMO-01, DEMO-02]\n\
must_haves:\n\
  truths:\n\
    - \"dòng có dấu hai chấm: và --- ở giữa\"\n\
---\n\
\n\
<objective>\n\
{objective}\n\
</objective>\n"
    )
}

/// Fixture tracer: hai phase có plan, một phase chưa có plan.
fn demo_files() -> Vec<(String, String)> {
    let roadmap = "# Roadmap: Demo\n\n### Phase 1: Nền tảng\n### Phase 2: Xuất báo cáo\n### Phase 3: Hướng dẫn (INSERTED)\n";
    let d1 = "01-nen-tang";
    let d2 = "02-xuat-bao-cao";
    vec![
        (".planning/ROADMAP.md".into(), roadmap.into()),
        (
            format!(".planning/phases/{d1}/01-01-PLAN.md"),
            plan_md(d1, "01", 1, "[]"),
        ),
        (
            format!(".planning/phases/{d1}/01-01-SUMMARY.md"),
            "# xong\n".into(),
        ),
        (
            format!(".planning/phases/{d1}/01-02-PLAN.md"),
            plan_md(d1, "02", 2, "[01-01]"),
        ),
        (
            format!(".planning/phases/{d1}/01-02-SUMMARY.md"),
            "# xong\n".into(),
        ),
        (
            format!(".planning/phases/{d2}/02-01-PLAN.md"),
            plan_md(d2, "01", 1, "[\"01-02\"]"),
        ),
        (
            format!(".planning/phases/{d2}/02-02-PLAN.md"),
            plan_md(d2, "02", 2, "[02-01]"),
        ),
    ]
}

fn as_refs(files: &[(String, String)]) -> Vec<(&str, &str)> {
    files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect()
}

fn item_keys(j: &Value) -> Vec<String> {
    j["items"]
        .as_array()
        .expect("items array")
        .iter()
        .map(|i| i["key"].as_str().expect("key").to_string())
        .collect()
}

fn item<'a>(j: &'a Value, key: &str) -> &'a Value {
    j["items"]
        .as_array()
        .expect("items array")
        .iter()
        .find(|i| i["key"] == key)
        .unwrap_or_else(|| panic!("không có hạng mục {key}"))
}

fn edge_triples(j: &Value) -> Vec<(String, String, String)> {
    j["edges"]
        .as_array()
        .expect("edges array")
        .iter()
        .map(|e| {
            (
                e["from"].as_str().unwrap().to_string(),
                e["to"].as_str().unwrap().to_string(),
                e["kind"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn demo_server(sb: &Sandbox) -> common::Serve {
    let files = demo_files();
    let repo = sb.make_repo("demo", &as_refs(&files), None, None);
    sb.write_map(json!([sb.project_entry(&repo, "demo", "work")]), &[]);
    sb.serve(&[])
}

#[test]
fn board01_tracer_fixture_board() {
    let sb = Sandbox::new();
    let mut srv = demo_server(&sb);

    let r = srv.get("/api/board?project=demo&epic=all");
    assert_eq!(r.status, 200);
    let j = r.json();
    assert_eq!(j["state"], "ok");
    assert_eq!(j["project"]["id"], "demo");
    assert_eq!(item_keys(&j), ["01-01", "03", "01-02", "02-01", "02-02"]);

    assert_eq!(item(&j, "01-01")["status"], "xong");
    assert_eq!(item(&j, "01-02")["status"], "xong");
    assert_eq!(item(&j, "02-01")["status"], "san_sang");
    assert_eq!(item(&j, "02-02")["status"], "bi_chan");
    assert_eq!(item(&j, "03")["status"], "san_sang");

    let ph3 = item(&j, "03");
    assert_eq!(ph3["placeholder"], true);
    assert_eq!(ph3["plan_path"], Value::Null);
    assert_eq!(ph3["title"], "Hướng dẫn");

    assert_eq!(item(&j, "01-01")["wave"], 0);
    assert_eq!(item(&j, "03")["wave"], 0);
    assert_eq!(item(&j, "01-02")["wave"], 1);
    assert_eq!(item(&j, "02-01")["wave"], 2);
    assert_eq!(item(&j, "02-02")["wave"], 3);
    assert_eq!(item(&j, "02-02")["fm_wave"], 2);
    assert_eq!(
        item(&j, "02-01")["plan_path"],
        ".planning/phases/02-xuat-bao-cao/02-01-PLAN.md"
    );

    assert_eq!(
        edge_triples(&j),
        [
            ("01-01".to_string(), "01-02".to_string(), "plan".to_string()),
            ("01-02".to_string(), "02-01".to_string(), "plan".to_string()),
            ("02-01".to_string(), "02-02".to_string(), "plan".to_string()),
        ]
    );

    assert_eq!(
        j["counts"],
        json!({"total": 5, "xong": 2, "san_sang": 2, "bi_chan": 1, "waves": 4})
    );
    let epics = j["epics"].as_array().unwrap();
    let ids: Vec<&str> = epics.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["01", "02", "03"]);
    let tallies: Vec<(u64, u64)> = epics
        .iter()
        .map(|e| (e["total"].as_u64().unwrap(), e["done"].as_u64().unwrap()))
        .collect();
    assert_eq!(tallies, [(2, 2), (2, 0), (1, 0)]);
    assert_eq!(j["warnings"], json!([]));

    let r2 = srv.get("/api/board?project=demo&epic=02");
    assert_eq!(r2.status, 200);
    let j2 = r2.json();
    assert_eq!(j2["epic"], "02");
    assert_eq!(item_keys(&j2), ["02-01", "02-02"]);
    assert_eq!(item(&j2, "02-01")["wave"], 0);
    assert_eq!(item(&j2, "02-01")["status"], "san_sang");
    assert_eq!(item(&j2, "02-02")["wave"], 1);
    assert_eq!(
        edge_triples(&j2),
        [("02-01".to_string(), "02-02".to_string(), "plan".to_string())]
    );

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

fn table_names(sb: &Sandbox) -> Vec<String> {
    let db = sb.db();
    let mut stmt = db
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .expect("prepare");
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query");
    rows.map(|r| r.expect("row")).collect()
}

#[test]
fn board01_gated_read_only() {
    let sb = Sandbox::new();
    let mut srv = demo_server(&sb);

    let unauth = srv.request("GET", "/api/board", &[], b"");
    assert_eq!(unauth.status, 401);

    let post = srv.post_json("/api/board", &json!({}));
    assert_eq!(post.status, 405);

    let before = table_names(&sb);
    let ok = srv.get("/api/board?project=demo&epic=all");
    assert_eq!(ok.status, 200);
    let after = table_names(&sb);
    assert_eq!(before, after);

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

fn demo_server_with_repo(sb: &Sandbox) -> (common::Serve, std::path::PathBuf) {
    let files = demo_files();
    let repo = sb.make_repo("demo", &as_refs(&files), None, None);
    sb.write_map(json!([sb.project_entry(&repo, "demo", "work")]), &[]);
    (sb.serve(&[]), repo)
}

fn error_of(r: &common::HttpResp) -> String {
    r.json()["error"].as_str().unwrap_or("").to_string()
}

#[test]
fn board01_params_rejected() {
    let sb = Sandbox::new();
    let mut srv = demo_server(&sb);

    let long_project = format!("/api/board?project={}", "a".repeat(129));
    let r = srv.get(&long_project);
    assert_eq!(r.status, 400);
    assert_eq!(error_of(&r), "Tham số project quá dài (tối đa 128 ký tự)");

    let r = srv.get(&format!("/api/board?project=demo&epic={}", "1".repeat(33)));
    assert_eq!(r.status, 400);
    assert_eq!(error_of(&r), "Tham số epic quá dài (tối đa 32 ký tự)");

    let r = srv.get(&format!(
        "/api/board?project=demo&milestone={}",
        "m".repeat(65)
    ));
    assert_eq!(r.status, 400);
    assert_eq!(error_of(&r), "Tham số milestone quá dài (tối đa 64 ký tự)");

    let r = srv.get(&format!(
        "/api/board?project=demo&profile={}",
        "p".repeat(33)
    ));
    assert_eq!(r.status, 400);
    assert_eq!(error_of(&r), "Tham số profile quá dài (tối đa 32 ký tự)");

    let r = srv.get("/api/board?project=a%01b");
    assert_eq!(r.status, 400);
    assert_eq!(error_of(&r), "Tham số project chứa ký tự không hợp lệ");

    let r = srv.get("/api/board?project=demo&milestone=v%0A1");
    assert_eq!(r.status, 400);
    assert_eq!(error_of(&r), "Tham số milestone chứa ký tự không hợp lệ");

    let r = srv.get("/api/board?project=demo&epic=x.y");
    assert_eq!(r.status, 400);
    assert_eq!(
        error_of(&r),
        "Tham số epic không hợp lệ: dùng all hoặc số phase như 05.4"
    );

    let r = srv.get("/api/board?project=demo&profile=khac");
    assert_eq!(r.status, 400);
    assert!(
        error_of(&r).contains("Profile 'khac' không hợp lệ"),
        "thông báo: {}",
        error_of(&r)
    );

    let r = srv.get("/api/board?project=khong-co");
    assert_eq!(r.status, 404);
    assert_eq!(error_of(&r), "Không tìm thấy dự án khong-co");

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

fn plain_repo(sb: &Sandbox, name: &str) -> std::path::PathBuf {
    sb.make_repo(name, &[], None, None)
}

fn board_repo(sb: &Sandbox, name: &str) -> std::path::PathBuf {
    let files = demo_files();
    sb.make_repo(name, &as_refs(&files), None, None)
}

#[test]
fn board02_default_project_and_projects_list() {
    let sb = Sandbox::new();
    let no_roadmap = plain_repo(&sb, "no-roadmap");
    let demo = board_repo(&sb, "demo");
    let khac = board_repo(&sb, "khac");
    sb.write_map(
        json!([
            sb.project_entry(&no_roadmap, "no-roadmap", "work"),
            sb.project_entry(&demo, "demo", "work"),
            sb.project_entry(&khac, "khac", "personal"),
        ]),
        &[],
    );
    let mut srv = sb.serve(&[]);

    let j = srv.get("/api/board").json();
    assert_eq!(j["state"], "ok");
    assert_eq!(j["project"]["id"], "demo");
    let list = j["projects"].as_array().unwrap();
    let flags: Vec<(String, bool)> = list
        .iter()
        .map(|p| {
            (
                p["id"].as_str().unwrap().to_string(),
                p["has_board"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        flags,
        [
            ("no-roadmap".to_string(), false),
            ("demo".to_string(), true),
            ("khac".to_string(), true)
        ]
    );

    let j = srv.get("/api/board?profile=personal").json();
    assert_eq!(j["project"]["id"], "khac");
    let ids: Vec<&str> = j["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["khac"]);

    let j = srv.get("/api/board?project=khac&profile=work").json();
    assert_eq!(j["state"], "ok");
    assert_eq!(j["project"]["id"], "khac");

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn board02_no_board_projects() {
    let sb = Sandbox::new();
    let a = plain_repo(&sb, "a");
    sb.write_map(json!([sb.project_entry(&a, "a", "work")]), &[]);
    let mut srv = sb.serve(&[]);

    let r = srv.get("/api/board");
    assert_eq!(r.status, 200);
    let j = r.json();
    assert_eq!(j["state"], "empty");
    assert_eq!(j["reason"], "no_board_projects");
    assert_eq!(j["project"], Value::Null);
    assert_eq!(j["expected_path"], Value::Null);
    assert!(j["message"]
        .as_str()
        .unwrap()
        .starts_with("Chưa có dự án nào có bảng công việc"));
    assert_eq!(j["projects"].as_array().unwrap().len(), 1);
    assert!(j.get("items").is_none());

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn board02_empty_states() {
    let sb = Sandbox::new();

    let no_roadmap = plain_repo(&sb, "no-roadmap");
    let media = board_repo(&sb, "media-proj");
    let mut media_entry = sb.project_entry(&media, "media-proj", "work");
    media_entry["type"] = json!("media");
    let gone_path = sb.root.join("khong-ton-tai");
    let gone_entry = sb.project_entry(&gone_path, "gone", "work");

    // .planning là symlink tới thư mục ngoài repo có ROADMAP.md
    let outside = sb.root.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("ROADMAP.md"), "### Phase 1: Ngoài\n").unwrap();
    let plan_link = plain_repo(&sb, "plan-link");
    std::os::unix::fs::symlink(&outside, plan_link.join(".planning")).unwrap();

    // ROADMAP.md là symlink tới file ngoài repo
    let secret = sb.root.join("secret.md");
    std::fs::write(&secret, "### Phase 1: BÍ MẬT\n").unwrap();
    let road_link = plain_repo(&sb, "road-link");
    std::fs::create_dir_all(road_link.join(".planning")).unwrap();
    std::os::unix::fs::symlink(&secret, road_link.join(".planning/ROADMAP.md")).unwrap();

    // ROADMAP.md lớn hơn 1 MiB
    let big = plain_repo(&sb, "big");
    std::fs::create_dir_all(big.join(".planning")).unwrap();
    std::fs::write(big.join(".planning/ROADMAP.md"), vec![b'x'; 1_048_577]).unwrap();

    sb.write_map(
        json!([
            sb.project_entry(&no_roadmap, "no-roadmap", "work"),
            media_entry,
            gone_entry,
            sb.project_entry(&plan_link, "plan-link", "work"),
            sb.project_entry(&road_link, "road-link", "work"),
            sb.project_entry(&big, "big", "work"),
        ]),
        &[],
    );
    let mut srv = sb.serve(&[]);

    let cases = [
        ("no-roadmap", "no_roadmap"),
        ("media-proj", "media"),
        ("gone", "missing_folder"),
        ("plan-link", "planning_symlink"),
        ("road-link", "unreadable"),
        ("big", "unreadable"),
    ];
    for (id, reason) in cases {
        let r = srv.get(&format!("/api/board?project={id}"));
        assert_eq!(r.status, 200, "{id}");
        let j = r.json();
        assert_eq!(j["state"], "empty", "{id}");
        assert_eq!(j["reason"], reason, "{id}");
        let msg = j["message"].as_str().unwrap_or("");
        assert!(!msg.is_empty(), "{id}: thiếu message");
        assert!(j.get("items").is_none(), "{id}: không được có items");
        assert_eq!(j["project"]["id"], id);
        if reason == "media" {
            assert_eq!(j["expected_path"], Value::Null);
        } else {
            let expected = j["expected_path"].as_str().unwrap_or("");
            assert!(
                expected.ends_with("/.planning/ROADMAP.md"),
                "{id}: expected_path {expected}"
            );
        }
        assert!(!msg.contains("BÍ MẬT"), "{id}: nội dung file lộ ra");
    }
    let j = srv.get("/api/board?project=no-roadmap").json();
    assert!(j["message"]
        .as_str()
        .unwrap()
        .contains(j["expected_path"].as_str().unwrap()));

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn board06_rereads_each_request() {
    let sb = Sandbox::new();
    let (mut srv, repo) = demo_server_with_repo(&sb);

    let j = srv.get("/api/board?project=demo&epic=02").json();
    assert_eq!(item(&j, "02-02")["status"], "bi_chan");

    std::fs::write(
        repo.join(".planning/phases/02-xuat-bao-cao/02-02-SUMMARY.md"),
        "# xong\n",
    )
    .unwrap();

    let j = srv.get("/api/board?project=demo&epic=02").json();
    assert_eq!(item(&j, "02-02")["status"], "xong");

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn board10_default_epic_from_state() {
    let sb = Sandbox::new();
    let demo = board_repo(&sb, "demo");
    let fin_files = vec![
        (
            ".planning/ROADMAP.md".to_string(),
            "### Phase 1: Nền tảng\n".to_string(),
        ),
        (
            ".planning/phases/01-nen-tang/01-01-PLAN.md".to_string(),
            plan_md("01-nen-tang", "01", 1, "[]"),
        ),
        (
            ".planning/phases/01-nen-tang/01-01-SUMMARY.md".to_string(),
            "# xong\n".to_string(),
        ),
    ];
    let fin = sb.make_repo("fin", &as_refs(&fin_files), None, None);
    sb.write_map(
        json!([
            sb.project_entry(&demo, "demo", "work"),
            sb.project_entry(&fin, "fin", "work"),
        ]),
        &[],
    );
    let mut srv = sb.serve(&[]);
    let state_path = demo.join(".planning/STATE.md");
    let board = |srv: &common::Serve, q: &str| -> Value {
        let r = srv.get(&format!("/api/board?project=demo{q}"));
        assert_eq!(r.status, 200);
        r.json()
    };

    // không có STATE.md: epic đầu tiên chưa xong
    let j = board(&srv, "");
    assert_eq!(j["epic"], "02");
    assert_eq!(j["default_epic"], "02");

    // current_phase có nháy
    std::fs::write(&state_path, "---\ncurrent_phase: \"01\"\n---\n# State\n").unwrap();
    let j = board(&srv, "");
    assert_eq!(j["epic"], "01");
    assert_eq!(j["default_epic"], "01");

    // current_phase không nháy
    std::fs::write(&state_path, "---\ncurrent_phase: 3\n---\n").unwrap();
    let j = board(&srv, "");
    assert_eq!(j["epic"], "03");
    std::fs::write(&state_path, "---\ncurrent_phase: 2\n---\n").unwrap();
    let j = board(&srv, "");
    assert_eq!(j["epic"], "02");
    assert_eq!(j["default_epic"], "02");

    // epic rõ ràng thắng mặc định
    std::fs::write(&state_path, "---\ncurrent_phase: \"01\"\n---\n").unwrap();
    let j = board(&srv, "&epic=2");
    assert_eq!(j["epic"], "02");
    assert_eq!(j["default_epic"], "01");
    let j = board(&srv, "&epic=all");
    assert_eq!(j["epic"], "all");
    assert_eq!(item_keys(&j).len(), 5);

    // epic không tồn tại: rơi về mặc định kèm cảnh báo
    let j = board(&srv, "&epic=9");
    assert_eq!(j["epic"], j["default_epic"]);
    assert_eq!(j["epic"], "01");
    let warnings = j["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(
            |w| w["message"] == "Không có epic 09 — đang xem epic mặc định 01"
                && w["path"] == ".planning/ROADMAP.md"
        ),
        "cảnh báo: {warnings:?}"
    );

    // STATE.md là symlink: bỏ qua kèm cảnh báo, dùng epic đầu chưa xong
    std::fs::remove_file(&state_path).unwrap();
    let outside_state = sb.root.join("outside-state.md");
    std::fs::write(&outside_state, "---\ncurrent_phase: \"01\"\n---\n").unwrap();
    std::os::unix::fs::symlink(&outside_state, &state_path).unwrap();
    let j = board(&srv, "");
    assert_eq!(j["epic"], "02");
    let warnings = j["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(|w| w["path"] == ".planning/STATE.md"),
        "cảnh báo: {warnings:?}"
    );

    // mọi hạng mục đã xong: "all"
    let j = srv.get("/api/board?project=fin").json();
    assert_eq!(j["default_epic"], "all");
    assert_eq!(j["epic"], "all");

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

const LO_TRINH_ROADMAP: &str = "# Roadmap: Demo\n\
\n\
## Phases\n\
\n\
- [x] **Phase 1: Nền tảng** - xong\n\
- [ ] **Phase 3: Xuất báo cáo** - đang làm\n\
- [x] **Phase 4: Tài liệu** - đã viết\n\
\n\
<details>\n\
<summary>✅ v1.0 MVP (Phases 1-2) - SHIPPED 2026-10-01</summary>\n\
\n\
### Phase 1: Nền tảng\n\
**Depends on**: Nothing (first phase)\n\
Plans:\n\
- [x] 01-01-PLAN.md — Chốt định dạng CSV — wave 1\n\
- [x] 01-02-PLAN.md — Truy vấn đơn hàng (wave 2)\n\
\n\
### Phase 2: Lưu trữ (INSERTED)\n\
**Depends on**: Phase 1\n\
Plans:\n\
- [x] 02-01: Bảng lưu trữ\n\
- [x] 02-02: Dọn bản cũ\n\
\n\
</details>\n\
\n\
### 🚧 v1.1 Xuất báo cáo (In Progress)\n\
\n\
#### Phase 3: Xuất báo cáo\n\
**Depends on**: Phase 2 (và kết quả khảo sát từ Phase 1)\n\
Plans:\n\
- [ ] 03-01-PLAN.md — Bộ ghi CSV\n\
- [ ] 03-02-PLAN.md\n\
\n\
#### Phase 3.1: Sửa gấp (INSERTED)\n\
**Depends on**: Phase 3\n\
Plans: TBD\n\
\n\
#### Phase 4: Tài liệu\n\
**Depends on**: Phase 3.1\n";

/// Repo `lo-trinh`: ROADMAP theo mẫu GSD (milestone, `####`, phase thập phân, `Depends on` có ngoặc).
fn lo_trinh_files() -> Vec<(String, String)> {
    let d1 = "01-nen-tang";
    let d3 = "03-xuat-bao-cao";
    let d7 = "07-ngoai-lo-trinh";
    vec![
        (".planning/ROADMAP.md".into(), LO_TRINH_ROADMAP.into()),
        (
            format!(".planning/phases/{d1}/01-01-PLAN.md"),
            plan_md(d1, "01", 1, "[]"),
        ),
        (
            format!(".planning/phases/{d1}/01-01-SUMMARY.md"),
            "# xong\n".into(),
        ),
        (
            format!(".planning/phases/{d1}/01-02-PLAN.md"),
            plan_md(d1, "02", 2, "[01-01]"),
        ),
        (
            format!(".planning/phases/{d1}/01-02-SUMMARY.md"),
            "# xong\n".into(),
        ),
        (
            format!(".planning/phases/{d3}/03-01-PLAN.md"),
            plan_md(d3, "01", 1, "[]"),
        ),
        (
            format!(".planning/phases/{d3}/03-02-PLAN.md"),
            plan_md_obj(
                d3,
                "02",
                2,
                "[\"03-01\"]",
                "Ghi luồng CSV theo lô. Chi tiết khác.",
            ),
        ),
        (
            format!(".planning/phases/{d7}/07-01-PLAN.md"),
            plan_md(d7, "01", 1, "[]"),
        ),
    ]
}

fn edge_set(j: &Value) -> Vec<(String, String, String)> {
    let mut v = edge_triples(j);
    v.sort();
    v
}

fn triple(from: &str, to: &str, kind: &str) -> (String, String, String) {
    (from.to_string(), to.to_string(), kind.to_string())
}

#[test]
fn board10_roadmap_drives_links_and_titles() {
    let sb = Sandbox::new();
    let files = lo_trinh_files();
    let repo = sb.make_repo("lo-trinh", &as_refs(&files), None, None);
    sb.write_map(json!([sb.project_entry(&repo, "lo-trinh", "work")]), &[]);
    let mut srv = sb.serve(&[]);

    let r = srv.get("/api/board?project=lo-trinh&epic=all");
    assert_eq!(r.status, 200);
    let j = r.json();
    assert_eq!(j["state"], "ok");

    let epic_ids: Vec<&str> = j["epics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(epic_ids, ["01", "02", "03", "03.1", "04", "07"]);

    let mut keys = item_keys(&j);
    keys.sort();
    assert_eq!(
        keys,
        ["01-01", "01-02", "02", "03-01", "03-02", "03.1", "04", "07-01"]
    );

    let status = |k: &str| item(&j, k)["status"].as_str().unwrap().to_string();
    assert_eq!(status("01-01"), "xong");
    assert_eq!(status("01-02"), "xong");
    assert_eq!(status("02"), "xong");
    assert_eq!(status("03-01"), "san_sang");
    assert_eq!(status("03-02"), "bi_chan");
    assert_eq!(status("03.1"), "bi_chan");
    assert_eq!(status("04"), "xong");
    assert_eq!(status("07-01"), "san_sang");
    for k in ["02", "03.1", "04"] {
        assert_eq!(item(&j, k)["placeholder"], true, "{k}");
    }

    assert_eq!(
        edge_set(&j),
        [
            triple("01-01", "01-02", "plan"),
            triple("01-02", "02", "phase"),
            triple("02", "03-01", "phase"),
            triple("03-01", "03-02", "plan"),
            triple("03-02", "03.1", "phase"),
            triple("03.1", "04", "phase"),
        ]
    );
    assert!(
        !edge_triples(&j)
            .iter()
            .any(|(f, t, _)| f.starts_with("01-") && t == "03-01"),
        "phase 1 không được nối thẳng tới 03-01"
    );

    let waves: Vec<(&str, u64)> = vec![
        ("01-01", 0),
        ("07-01", 0),
        ("01-02", 1),
        ("02", 2),
        ("03-01", 3),
        ("03-02", 4),
        ("03.1", 5),
        ("04", 6),
    ];
    for (k, w) in waves {
        assert_eq!(item(&j, k)["wave"], w, "wave của {k}");
    }

    let titles = [
        ("01-01", "Chốt định dạng CSV"),
        ("01-02", "Truy vấn đơn hàng"),
        ("02", "Lưu trữ"),
        ("03-01", "Bộ ghi CSV"),
        ("03-02", "Ghi luồng CSV theo lô"),
        ("03.1", "Sửa gấp"),
    ];
    for (k, t) in titles {
        assert_eq!(item(&j, k)["title"], t, "tiêu đề của {k}");
    }

    let mut critical_items: Vec<&str> = j["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["critical"] == true)
        .map(|i| i["key"].as_str().unwrap())
        .collect();
    critical_items.sort();
    assert_eq!(critical_items, ["03-01", "03-02", "03.1"]);
    let mut critical_edges: Vec<(String, String)> = j["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["critical"] == true)
        .map(|e| {
            (
                e["from"].as_str().unwrap().to_string(),
                e["to"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    critical_edges.sort();
    assert_eq!(
        critical_edges,
        [
            ("03-01".to_string(), "03-02".to_string()),
            ("03-02".to_string(), "03.1".to_string())
        ]
    );

    let warnings = j["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "cảnh báo: {warnings:?}");
    assert_eq!(warnings[0]["path"], ".planning/phases/07-ngoai-lo-trinh");
    assert_eq!(
        warnings[0]["message"],
        "thư mục phase 07-ngoai-lo-trinh không có trong ROADMAP.md (vẫn hiện thành epic riêng)"
    );

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn board04_milestones_from_roadmap() {
    let sb = Sandbox::new();
    let files = lo_trinh_files();
    let repo = sb.make_repo("lo-trinh", &as_refs(&files), None, None);
    let plain = board_repo(&sb, "demo");
    sb.write_map(
        json!([
            sb.project_entry(&repo, "lo-trinh", "work"),
            sb.project_entry(&plain, "demo", "work"),
        ]),
        &[],
    );
    let mut srv = sb.serve(&[]);

    let j = srv.get("/api/board?project=lo-trinh&epic=all").json();
    assert_eq!(j["milestones"], json!(["v1.0 MVP", "v1.1 Xuất báo cáo"]));
    assert_eq!(j["milestone"], Value::Null);
    let epic_milestone = |id: &str| -> Value {
        j["epics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == id)
            .unwrap_or_else(|| panic!("không có epic {id}"))["milestone"]
            .clone()
    };
    for id in ["01", "02"] {
        assert_eq!(epic_milestone(id), "v1.0 MVP", "epic {id}");
    }
    for id in ["03", "03.1", "04"] {
        assert_eq!(epic_milestone(id), "v1.1 Xuất báo cáo", "epic {id}");
    }
    assert_eq!(epic_milestone("07"), Value::Null);

    let f = srv
        .get("/api/board?project=lo-trinh&epic=all&milestone=v1.0%20MVP")
        .json();
    assert_eq!(f["milestone"], "v1.0 MVP");
    let mut keys = item_keys(&f);
    keys.sort();
    assert_eq!(keys, ["01-01", "01-02", "02"]);
    assert_eq!(item(&f, "01-01")["milestone"], "v1.0 MVP");

    let none = srv.get("/api/board?project=demo&epic=all").json();
    assert_eq!(none["milestones"], json!([]));
    assert_eq!(none["milestone"], Value::Null);

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

const SENTINEL: &str = "BI-MAT-KHONG-DUOC-LO-7f3a";

#[test]
fn board05_symlink_secret_never_leaks() {
    let sb = Sandbox::new();
    let files = vec![
        (
            ".planning/ROADMAP.md".to_string(),
            "### Phase 1: X\n### Phase 2: Liên kết\n".to_string(),
        ),
        (
            ".planning/phases/01-x/01-01-PLAN.md".to_string(),
            plan_md("01-x", "01", 1, "[]"),
        ),
    ];
    let repo = sb.make_repo("hostile", &as_refs(&files), None, None);

    // File và thư mục ngoài repo chứa chuỗi bí mật ở depends_on, requirements và objective.
    let secret_plan = format!(
        "---\nphase: bi-mat\nplan: 02\nwave: 1\ndepends_on: [{SENTINEL}]\nrequirements: [{SENTINEL}]\n---\n\n<objective>\n{SENTINEL}. Câu sau.\n</objective>\n"
    );
    let secret_file = sb.root.join("bi-mat.txt");
    std::fs::write(&secret_file, &secret_plan).unwrap();
    let outside_dir = sb.root.join("ngoai-thu-muc");
    std::fs::create_dir_all(&outside_dir).unwrap();
    std::fs::write(outside_dir.join("02-01-PLAN.md"), &secret_plan).unwrap();

    // Ba loại symlink: file plan, thư mục phase, file SUMMARY.
    std::os::unix::fs::symlink(
        &secret_file,
        repo.join(".planning/phases/01-x/01-02-PLAN.md"),
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside_dir, repo.join(".planning/phases/02-lien-ket")).unwrap();
    std::os::unix::fs::symlink(
        &secret_file,
        repo.join(".planning/phases/01-x/01-01-SUMMARY.md"),
    )
    .unwrap();

    sb.write_map(json!([sb.project_entry(&repo, "hostile", "work")]), &[]);
    let mut srv = sb.serve(&[]);

    let r = srv.get("/api/board?project=hostile&epic=all");
    assert_eq!(r.status, 200);
    assert!(
        !r.text().contains(SENTINEL),
        "chuỗi bí mật lọt vào phản hồi"
    );
    let j = r.json();
    assert_eq!(j["state"], "ok");
    let keys = item_keys(&j);
    assert!(keys.contains(&"01-01".to_string()), "khoá: {keys:?}");
    assert_ne!(item(&j, "01-01")["status"], "xong");
    assert!(!keys.contains(&"01-02".to_string()));
    assert!(!keys.contains(&"02-01".to_string()));

    let warnings = j["warnings"].as_array().unwrap();
    for path in [
        ".planning/phases/01-x/01-02-PLAN.md",
        ".planning/phases/02-lien-ket",
    ] {
        assert!(
            warnings.iter().any(|w| w["path"] == path),
            "thiếu cảnh báo ở {path}: {warnings:?}"
        );
    }
    for w in warnings {
        let path = w["path"].as_str().unwrap();
        assert!(!path.starts_with('/'), "đường dẫn tuyệt đối: {path}");
    }

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}

#[test]
fn board07_broken_parts_listed_rest_intact() {
    let sb = Sandbox::new();
    let d = "01-nen-tang";
    let dup = "01-zz-ban-sao";
    let files = vec![
        (
            ".planning/ROADMAP.md".to_string(),
            "### Phase 1: Nền tảng\n".to_string(),
        ),
        (
            format!(".planning/phases/{d}/01-01-PLAN.md"),
            plan_md(d, "01", 1, "[]"),
        ),
        (
            // không có dòng --- đóng
            format!(".planning/phases/{d}/01-02-PLAN.md"),
            "---\nphase: 01-nen-tang\nplan: 02\nwave: 1\ndepends_on: []\n\n<objective>\nHỏng. Còn lại.\n</objective>\n".to_string(),
        ),
        (
            format!(".planning/phases/{d}/01-03-PLAN.md"),
            plan_md(d, "03", 1, "[01-99]"),
        ),
        (
            format!(".planning/phases/{d}/01-04-PLAN.md"),
            plan_md(d, "04", 2, "[01-05]"),
        ),
        (
            format!(".planning/phases/{d}/01-05-PLAN.md"),
            plan_md(d, "05", 1, "[01-04]"),
        ),
        (
            format!(".planning/phases/{d}/01-06-PLAN.md"),
            plan_md(d, "06", 5, "[]"),
        ),
        (
            format!(".planning/phases/{dup}/01-01-PLAN.md"),
            plan_md(dup, "01", 1, "[]"),
        ),
    ];
    let repo = sb.make_repo("hu-hong", &as_refs(&files), None, None);
    sb.write_map(json!([sb.project_entry(&repo, "hu-hong", "work")]), &[]);
    let mut srv = sb.serve(&[]);

    let r = srv.get("/api/board?project=hu-hong&epic=all");
    assert_eq!(r.status, 200);
    let j = r.json();
    assert_eq!(j["state"], "ok");

    let mut keys = item_keys(&j);
    keys.sort();
    assert_eq!(keys, ["01-01", "01-02", "01-03", "01-04", "01-05", "01-06"]);
    assert_eq!(
        item(&j, "01-01")["plan_path"],
        ".planning/phases/01-nen-tang/01-01-PLAN.md"
    );
    assert_eq!(item(&j, "01-02")["status"], "san_sang");
    assert_eq!(item(&j, "01-03")["status"], "san_sang");
    assert_eq!(item(&j, "01-04")["status"], "bi_chan");
    assert_eq!(item(&j, "01-05")["status"], "san_sang");

    let warnings = j["warnings"].as_array().unwrap();
    let has = |path: &str, needle: &str| {
        warnings
            .iter()
            .any(|w| w["path"] == path && w["message"].as_str().unwrap_or("").contains(needle))
    };
    let p = |name: &str| format!(".planning/phases/{d}/{name}-PLAN.md");
    assert!(
        has(&p("01-02"), "front matter lỗi"),
        "cảnh báo: {warnings:?}"
    );
    assert!(
        has(&p("01-03"), "depends_on trỏ tới plan không tồn tại: 01-99"),
        "cảnh báo: {warnings:?}"
    );
    assert!(
        has(
            &p("01-05"),
            "vòng phụ thuộc: 01-04 cần 01-05 cần 01-04 — đã bỏ phụ thuộc của 01-05 vào 01-04"
        ),
        "cảnh báo: {warnings:?}"
    );
    assert!(
        has(
            &format!(".planning/phases/{dup}/01-01-PLAN.md"),
            "mã plan 01-01 trùng với .planning/phases/01-nen-tang/01-01-PLAN.md"
        ),
        "cảnh báo: {warnings:?}"
    );
    assert!(
        has(&p("01-06"), "wave: ghi 5, tính ra 1"),
        "cảnh báo: {warnings:?}"
    );
    for w in warnings {
        let path = w["path"].as_str().unwrap();
        assert!(path.starts_with(".planning/"), "đường dẫn cảnh báo: {path}");
    }

    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}
