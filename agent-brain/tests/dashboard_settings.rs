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
