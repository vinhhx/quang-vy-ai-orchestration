//! Phase 7 plan 04 (JRN-03, ROADMAP criterion 3): hostile task text leaves nothing in the
//! vault, and a secret that reaches the output anyway stops the export before any write, commit
//! or push. Everything is invented; key-shaped strings are built at run time from a prefix and a
//! body so that no complete token literal sits in the repository.

mod common;
#[allow(dead_code)]
#[path = "support/journal.rs"]
mod jsupport;

use common::Sandbox;
use jsupport::*;
use serde_json::json;
use std::path::Path;

const DAY: &str = "Nhật ký/2026/01/2026-01-15.md";
const GH_BODY: &str = "Zq9xLm4Vb7Tn2Kd8Rw5Ys3Hc6Pf1Jg0Ae7";
const ANT_BODY: &str = "Hx3Kp8Wd2Qm6Rt9Vc4Nb7Lz1Sf5Yg0Ja8";

fn now() -> i64 {
    local_ms(2026, 1, 15, 20, 0, 0)
}

/// Every file of a checkout (outside `.git`): relative path and text.
fn all_files(root: &Path) -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                walk(&p, root, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().to_string();
                out.push((rel, std::fs::read_to_string(&p).unwrap_or_default()));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

fn three_runs(conn: &rusqlite::Connection, task: i64) {
    for (role, outcome, from, to) in [
        ("lead", None, (9, 0), (9, 5)),
        ("dev", None, (9, 10), (9, 30)),
        ("qa", Some("qa:pass"), (9, 31), (9, 40)),
    ] {
        seed_run(
            conn,
            task,
            role,
            "succeeded",
            outcome,
            local_ms(2026, 1, 15, from.0, from.1, 0),
            Some(local_ms(2026, 1, 15, to.0, to.1, 0)),
            Some(1),
        );
    }
}

#[test]
fn hostile_request_leaves_nothing_in_vault() {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    let mut section = journal_section(&remote);
    section["title_max_chars"] = json!(400);
    section["callout_title_chars"] = json!(200);
    sb.write_config(json!({"journal": section}));
    let conn = setup_projects(&sb, &[("du-an-a", None)]);

    let gh = format!("{}{}", "ghp_", GH_BODY);
    let ant = format!("{}{}", "sk-ant-", ANT_BODY);
    let request = format!(
        "Sửa trang chủ theo mẫu mới, cấu hình ở /home/someone/app/config/database.php, xem \
         https://intranet.example.local/admin?token=abc123, đặt DEEPSEEK_API_KEY=abcd1234efgh5678, \
         khóa {gh} và {ant}, ghi chú ![[ghi chu rieng]] <script>x</script> %%c%% {{{{t}}}} nhánh \
         task/sua-loi\ndiff --git a/x.php b/x.php\n@@ -1 +1 @@\n-cu\n+moi\n"
    );
    let task = seed_task(&conn, "du-an-a", "sua-trang-chu", &request);
    three_runs(&conn, task);

    let out = export(&sb, &[], now());
    assert_eq!(
        out.code, 0,
        "stdout: {}\nstderr: {}",
        out.stdout, out.stderr
    );
    assert_eq!(remote_commits(&sb, &remote).len(), 1);

    let co = checkout(&sb, &remote);
    let files = all_files(&co);
    assert!(files.iter().any(|(p, _)| p == DAY), "{files:?}");
    let day = &files.iter().find(|(p, _)| p == DAY).unwrap().1;
    for word in ["(đường dẫn)", "(liên kết)", "(đã ẩn)", "(đã bỏ phần mã)"] {
        assert!(day.contains(word), "{word} missing in:\n{day}");
    }
    for (path, body) in &files {
        for bad in [
            "intranet",
            "example.local",
            "/home/",
            "database.php",
            "abcd1234efgh5678",
            "DEEPSEEK_API_KEY",
            GH_BODY,
            ANT_BODY,
            "diff --git",
            "@@",
            "![[ghi",
            "<script",
            "%%",
            "{{",
            "task/sua-loi",
        ] {
            assert!(!path.contains(bad), "{bad:?} in path {path}");
            assert!(!body.contains(bad), "{bad:?} in {path}:\n{body}");
        }
    }
}

#[test]
fn planted_secret_blocks_export() {
    const VALUE: &str = "zq7plantedsecret9x";
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    sb.write_config(json!({"journal": journal_section(&remote)}));
    // The project id is not run through `sanitize_text`; it is the value the gate must catch.
    let conn = setup_projects(&sb, &[(VALUE, None)]);
    let task = seed_task(
        &conn,
        VALUE,
        "dung-video",
        "Dựng video giới thiệu sản phẩm mẫu",
    );
    three_runs(&conn, task);

    let now_s = now().to_string();
    let out = sb.run_env(
        &["journal", "export"],
        None,
        &[
            (agent_brain::journal::NOW_ENV, now_s.as_str()),
            ("AB_TEST_JOURNAL_TOKEN", VALUE),
        ],
    );
    assert_eq!(
        out.code, 1,
        "stdout: {}\nstderr: {}",
        out.stdout, out.stderr
    );
    assert!(out.stderr.contains("known-value"), "{}", out.stderr);
    assert!(!out.stdout.contains(VALUE), "{}", out.stdout);
    assert!(!out.stderr.contains(VALUE), "{}", out.stderr);

    assert!(remote_commits(&sb, &remote).is_empty());
    let clone = clone_dir(&sb);
    assert!(!clone.join("Nhật ký").exists());
    assert!(!clone.join("Dự án").exists());
    assert!(
        !clone.join(".git").exists(),
        "gate must stop before the clone"
    );
}
