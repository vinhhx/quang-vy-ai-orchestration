//! D1: protocol files under a configured `protocol_dir` (`.brain/{slug}`).
mod common;
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::plan_text;
use std::path::{Path, PathBuf};

const DIR: &str = ".brain/{slug}";
const TASK: &str = "sua-app";
const PLAN: &str = ".brain/sua-app/.plan.md";
const QA: &str = ".brain/sua-app/.qa-report.md";

fn setup(sb: &Sandbox) -> PathBuf {
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", "work");
    entry["protocol_dir"] = serde_json::json!(DIR);
    sb.write_map(serde_json::json!([entry]), &[]);
    repo
}

fn argv_of(rec: &serde_json::Value) -> Vec<String> {
    rec["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

fn after<'a>(argv: &'a [String], flag: &str) -> &'a str {
    let i = argv.iter().position(|a| a == flag).unwrap();
    &argv[i + 1]
}

fn run_lead(sb: &Sandbox, repo: &Path) {
    let rows = [("src/app.py", "app()", "trả về 2", "demo")];
    let sc = serde_json::json!({"ops": [
        {"op": "read", "path": PLAN},
        {"op": "write", "path": PLAN, "content": plan_text("Kế hoạch", &rows, "python3 -m unittest", ""), "keep_meta": true}
    ]});
    let res = sb.run(&["run", "lead", "web-ban-hang", "sửa app"], Some(sc));
    assert_eq!(res.code, 0, "Lead: {}\n{}", res.stderr, res.stdout);
    assert!(repo.join(PLAN).is_file());
}

#[test]
fn lead_writes_and_commits_plan_in_protocol_dir() {
    let sb = Sandbox::new();
    let repo = setup(&sb);
    run_lead(&sb, &repo);

    assert!(!repo.join(".plan.md").exists(), "no plan at the root");
    assert_eq!(
        sb.git(&repo, &["log", "-1", "--format=%s"]).trim(),
        format!("{TASK}: plan")
    );
    let files = sb.git(&repo, &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]);
    assert_eq!(files.trim(), PLAN);

    let recs = sb.records();
    let argv = argv_of(&recs[recs.len() - 1]);
    assert!(argv.contains(&format!("Write(./{PLAN})")), "{argv:?}");
    assert!(argv.contains(&format!("Edit(./{PLAN})")));
    let repo_str = repo.to_string_lossy();
    assert!(argv.contains(&format!("Write(/{repo_str}/{PLAN})")));
    assert!(!argv.iter().any(|a| a == "Write(./.plan.md)"));

    // Placeholders are rendered with the task's real paths, in the input and the system prompt.
    let input = after(&argv, "-p");
    let system = after(&argv, "--append-system-prompt");
    assert!(input.contains(PLAN), "{input}");
    assert!(system.contains(&format!("Ghi {PLAN} theo đúng mẫu")), "{system}");
    for text in [input, system] {
        assert!(!text.contains("{plan_path}"));
        assert!(!text.contains("{qa_report_path}"));
    }
}

#[test]
fn dev_and_qa_read_the_plan_from_protocol_dir() {
    let sb = Sandbox::new();
    let repo = setup(&sb);
    run_lead(&sb, &repo);
    let plan_sha = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();

    let sc_dev = serde_json::json!({"ops": [
        {"op": "read", "path": PLAN},
        {"op": "read", "path": "src/app.py"},
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
        {"op": "git", "args": ["add", "src/app.py"]},
        {"op": "git", "args": ["commit", "-m", format!("{TASK}: sửa app")]}
    ]});
    let res = sb.run(&["run", "dev", "web-ban-hang"], Some(sc_dev));
    assert_eq!(res.code, 0, "Dev: {}\n{}", res.stderr, res.stdout);
    let recs = sb.records();
    let dev = &recs[recs.len() - 1];
    assert!(
        dev["blocked"].as_array().map(|b| b.is_empty()).unwrap_or(true),
        "Dev reads the plan where it is: {dev}"
    );
    let argv = argv_of(dev);
    assert!(argv.contains(&format!("Read(./{PLAN})")), "{argv:?}");
    let input = after(&argv, "-p");
    assert!(input.contains(&format!("Đọc {PLAN} rồi sửa")), "{input}");
    assert!(after(&argv, "--append-system-prompt").contains(&format!("Chỉ đọc {PLAN}")));

    let qa_body = "# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- src/app.py: khớp.\n## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông.\n## Kết luận\nĐạt\n";
    let sc_qa = serde_json::json!({"ops": [
        {"op": "read", "path": PLAN},
        {"op": "write", "path": QA, "content": qa_body, "keep_meta": true}
    ]});
    let res = sb.run(&["run", "qa", "web-ban-hang"], Some(sc_qa));
    assert_eq!(res.code, 0, "QA: {}\n{}", res.stderr, res.stdout);
    assert!(res.stdout.contains("Kết luận QA: Đạt"));
    let files = sb.git(&repo, &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]);
    assert_eq!(files.trim(), QA);
    assert!(!repo.join(".qa-report.md").exists());

    let recs = sb.records();
    let argv = argv_of(&recs[recs.len() - 1]);
    assert!(argv.contains(&format!("Write(./{QA})")));
    let input = after(&argv, "-p");
    assert!(input.contains(&format!("git diff {plan_sha}..HEAD")));
    assert!(input.contains(&format!("':(exclude){PLAN}'")), "{input}");
    assert!(input.contains(&format!("Ghi {QA} theo khung")));
}

#[test]
fn guard_blocks_dev_writing_the_plan_at_the_root() {
    let sb = Sandbox::new();
    let repo = setup(&sb);
    run_lead(&sb, &repo);

    let sc_dev = serde_json::json!({"ops": [
        {"op": "write", "path": ".plan.md", "content": "# Plan: nhầm chỗ\n"},
        {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
        {"op": "git", "args": ["add", "src/app.py"]},
        {"op": "git", "args": ["commit", "-m", format!("{TASK}: sửa app")]}
    ]});
    let _ = sb.run(&["run", "dev", "web-ban-hang"], Some(sc_dev));
    assert!(!repo.join(".plan.md").exists(), "the root plan write was refused");
    let recs = sb.records();
    let blocked = recs[recs.len() - 1]["blocked"].as_array().cloned().unwrap_or_default();
    let root_plan = repo.join(".plan.md");
    assert!(
        blocked.iter().any(|b| b["path"].as_str().map(Path::new) == Some(root_plan.as_path())
            || b["path"].as_str().is_some_and(|p| p.ends_with("/.plan.md") && !p.contains(".brain"))),
        "{blocked:?}"
    );
}

#[test]
fn protocol_dir_endpoint_validates_and_stores() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("proj-pd", &[], None, None);
    let entry = sb.project_entry(&repo, "proj-pd", "work");
    sb.write_map(serde_json::json!([entry]), &[]);
    let mut srv = sb.serve(&[]);

    let j = srv.get("/api/projects").json();
    assert_eq!(j["projects"][0]["protocol_dir"], "");

    for bad in ["../x", "/abs", ".git/plans", "a/{task}", "a\\b"] {
        let r = srv.put_json(
            "/api/projects/proj-pd/protocol-dir",
            &serde_json::json!({"protocol_dir": bad}),
        );
        assert_eq!(r.status, 400, "{bad}");
    }
    let r = srv.put_json(
        "/api/projects/nope/protocol-dir",
        &serde_json::json!({"protocol_dir": ".brain"}),
    );
    assert_eq!(r.status, 404);
    let r = srv.put_json(
        "/api/projects/proj-pd/protocol-dir",
        &serde_json::json!({"protocol_dir": ".brain", "etag": "wrong"}),
    );
    assert_eq!(r.status, 409);

    let r = srv.put_json(
        "/api/projects/proj-pd/protocol-dir",
        &serde_json::json!({"protocol_dir": " ./.brain/{slug}/ "}),
    );
    assert_eq!(r.status, 200, "{}", String::from_utf8_lossy(&r.body));
    assert_eq!(sb.read_map()["projects"][0]["protocol_dir"], ".brain/{slug}");
    assert_eq!(srv.get("/api/projects").json()["projects"][0]["protocol_dir"], ".brain/{slug}");

    let r = srv.put_json(
        "/api/projects/proj-pd/protocol-dir",
        &serde_json::json!({"protocol_dir": ""}),
    );
    assert_eq!(r.status, 200);
    assert!(sb.read_map()["projects"][0].get("protocol_dir").is_none());

    let exit = srv.stop(libc::SIGINT, std::time::Duration::from_secs(5));
    assert_eq!(exit, Some(0));
}
