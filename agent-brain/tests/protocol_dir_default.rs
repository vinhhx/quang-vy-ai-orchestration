//! Default `protocol_dir`: a project entry without the key keeps each task's protocol files in
//! `.gsd/quick/plan-<slug>/` (or `.planning/quick/plan-<slug>/` when the project has a real
//! `.planning/`); a present key, even `""`, keeps the old layout.
mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::plan_text;
use std::path::{Path, PathBuf};

const TASK: &str = "sua-app";

fn setup(sb: &Sandbox, files: &[(&str, &str)], key: Option<serde_json::Value>) -> PathBuf {
    let mut all: Vec<(&str, &str)> = vec![("src/app.py", "def app():\n    return 1\n")];
    all.extend_from_slice(files);
    let repo = sb.make_repo("web-ban-hang", &all, None, None);
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    if let Some(k) = key {
        entry["protocol_dir"] = k;
    }
    // raw: the entry keeps exactly the keys given here
    sb.write_map_raw(serde_json::json!([entry]), &[]);
    repo
}

fn run_lead(sb: &Sandbox, repo: &Path, plan: &str) {
    let rows = [("src/app.py", "app()", "trả về 2", "demo")];
    let sc = serde_json::json!({"ops": [
        {"op": "read", "path": plan},
        {"op": "write", "path": plan, "content": plan_text("Kế hoạch", &rows, "python3 -m unittest", ""), "keep_meta": true}
    ]});
    let res = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(sc));
    assert_eq!(res.code, 0, "Lead: {}\n{}", res.stderr, res.stdout);
    assert!(repo.join(plan).is_file(), "plan at {plan}");
    assert_eq!(
        sb.git(repo, &["log", "-1", "--format=%s"]).trim(),
        format!("{TASK}: plan")
    );
    let files = sb.git(repo, &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]);
    assert_eq!(files.trim(), plan);
}

#[test]
fn no_key_and_no_planning_uses_gsd_quick_and_creates_the_dir() {
    let sb = Sandbox::new();
    let repo = setup(&sb, &[], None);
    assert!(!repo.join(".gsd").exists());
    let plan = format!(".gsd/quick/plan-{TASK}/.plan.md");
    run_lead(&sb, &repo, &plan);

    assert!(repo.join(format!(".gsd/quick/plan-{TASK}")).is_dir());
    assert!(!repo.join(".plan.md").exists(), "the root stays clean");
    // The program-made `.gsd/` is not a knowledge home.
    assert!(!repo.join(".planning").exists());
    assert!(agent_brain::knowledge::find(&repo).is_none());
}

#[test]
fn no_key_with_planning_uses_planning_quick() {
    let sb = Sandbox::new();
    let repo = setup(&sb, &[(".planning/PROJECT.md", "# p\n")], None);
    let plan = format!(".planning/quick/plan-{TASK}/.plan.md");
    run_lead(&sb, &repo, &plan);

    assert!(!repo.join(".plan.md").exists());
    assert!(!repo.join(".gsd").exists(), "`.planning/` decides; no `.gsd/` is created");
}

#[test]
fn explicit_empty_key_keeps_the_root_layout() {
    let sb = Sandbox::new();
    let repo = setup(&sb, &[], Some(serde_json::json!("")));
    run_lead(&sb, &repo, ".plan.md");
    assert!(!repo.join(".gsd").exists());
}

#[test]
fn explicit_value_wins_over_the_default() {
    let sb = Sandbox::new();
    let repo = setup(&sb, &[(".planning/PROJECT.md", "# p\n")], Some(serde_json::json!(".brain/{slug}")));
    run_lead(&sb, &repo, &format!(".brain/{TASK}/.plan.md"));
    assert!(!repo.join(".planning/quick").exists());
}

#[test]
fn dashboard_shows_the_effective_dir_and_put_still_works() {
    let sb = Sandbox::new();
    setup(&sb, &[], None);
    let mut srv = sb.serve(&[]);

    let j = srv.get("/api/projects").json();
    assert_eq!(j["projects"][0]["protocol_dir"], ".gsd/quick/plan-{slug}");

    let r = srv.put_json(
        "/api/projects/web-ban-hang/protocol-dir",
        &serde_json::json!({"protocol_dir": ".brain/{slug}"}),
    );
    assert_eq!(r.status, 200, "{}", String::from_utf8_lossy(&r.body));
    assert_eq!(sb.read_map()["projects"][0]["protocol_dir"], ".brain/{slug}");
    assert_eq!(srv.get("/api/projects").json()["projects"][0]["protocol_dir"], ".brain/{slug}");

    // `""` removes the key: back to the default.
    let r = srv.put_json(
        "/api/projects/web-ban-hang/protocol-dir",
        &serde_json::json!({"protocol_dir": ""}),
    );
    assert_eq!(r.status, 200);
    assert!(sb.read_map()["projects"][0].get("protocol_dir").is_none());
    assert_eq!(
        srv.get("/api/projects").json()["projects"][0]["protocol_dir"],
        ".gsd/quick/plan-{slug}"
    );
    assert_eq!(srv.stop(libc::SIGINT, std::time::Duration::from_secs(5)), Some(0));
}

/// Lead, Dev and QA all read and write the files in the default folder (nothing blocked).
fn lead_dev_qa_in(planning: bool) {
    let sb = Sandbox::new();
    let files: &[(&str, &str)] = if planning { &[(".planning/PROJECT.md", "# p\n")] } else { &[] };
    let repo = setup(&sb, files, None);
    let home = if planning { ".planning" } else { ".gsd" };
    let plan = format!("{home}/quick/plan-{TASK}/.plan.md");
    let qa = format!("{home}/quick/plan-{TASK}/.qa-report.md");
    run_lead(&sb, &repo, &plan);

    let sc_dev = serde_json::json!({"ops": [
        {"op": "read", "path": plan},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
        {"op": "git", "args": ["add", "src/app.py"]},
        {"op": "git", "args": ["commit", "-m", format!("{TASK}: sửa app")]}
    ]});
    let res = sb.run(&["run", "dev", "web-ban-hang"], Some(sc_dev));
    assert_eq!(res.code, 0, "Dev: {}\n{}", res.stderr, res.stdout);
    let recs = sb.records();
    let dev = &recs[recs.len() - 1];
    assert!(dev["blocked"].as_array().map(|b| b.is_empty()).unwrap_or(true), "{dev}");

    let qa_body = "# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- src/app.py: khớp.\n## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông.\n## Kết luận\nĐạt\n";
    let sc_qa = serde_json::json!({"ops": [
        {"op": "read", "path": plan},
        {"op": "write", "path": qa, "content": qa_body, "keep_meta": true}
    ]});
    let res = sb.run(&["run", "qa", "web-ban-hang"], Some(sc_qa));
    assert_eq!(res.code, 0, "QA: {}\n{}", res.stderr, res.stdout);
    assert!(res.stdout.contains("Kết luận QA: Đạt"));
    assert!(repo.join(&qa).is_file());
    assert!(!repo.join(".qa-report.md").exists());
    assert!(!repo.join(".plan.md").exists());
}

#[test]
fn lead_dev_qa_work_in_gsd_quick() {
    lead_dev_qa_in(false);
}

#[test]
fn lead_dev_qa_work_in_planning_quick() {
    lead_dev_qa_in(true);
}
