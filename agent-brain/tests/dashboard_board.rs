mod common;

use common::Sandbox;
use serde_json::{json, Value};
use std::time::Duration;

/// Một file plan có dạng như plan GSD thật: front matter có khối lồng và dòng chứa `:` và `---`.
fn plan_md(dir: &str, plan: &str, wave: u32, deps: &str) -> String {
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
Câu đầu của mục tiêu. Câu sau.\n\
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
