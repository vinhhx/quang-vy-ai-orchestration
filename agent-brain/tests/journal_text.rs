//! Phase 7 plan 12 (WR-05, WR-06, WR-08, IN-08): what the text pipeline lets through and what
//! `/tim` returns. The index comes from a real `journal export --no-push`; `serve` runs against
//! the fake Telegram API; nothing contacts api.telegram.org. Texts and projects are invented.

#![allow(dead_code, unused_imports)]

mod common;
#[allow(dead_code)]
#[path = "support/journal.rs"]
mod jsupport;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use common::{wait_until, Sandbox, Serve};
use jsupport::*;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use tg::{write_config, FakeTelegram, PAIRED_CHAT};

const CONNECTED: &str = "Telegram: đã kết nối (@ab_fake_bot)";
const WEB_BASE: &str =
    "https://github.com/example-owner-with-a-long-name/example-vault-with-a-long-name";
const NOT_FOUND: &str = "Không tìm thấy mục nào trong nhật ký cho: ";
const DAY_FILE: &str = "Nhật ký/2026/01/2026-01-15.md";

struct Fixture {
    // Kept alive for the whole test: the sandbox owns every temp directory.
    sb: Sandbox,
    tg: FakeTelegram,
    srv: Serve,
}

impl Fixture {
    fn finish(mut self) {
        self.srv.stop(libc::SIGINT, Duration::from_secs(10));
    }
}

fn stdout_of(srv: &Serve) -> String {
    srv.stdout.lock().unwrap().clone()
}

fn stderr_of(srv: &Serve) -> String {
    std::fs::read_to_string(&srv.stderr_path).unwrap_or_default()
}

fn wait_polling(tg: &FakeTelegram, srv: &Serve) {
    assert!(
        wait_until(Duration::from_secs(10), || stdout_of(srv)
            .contains(CONNECTED)),
        "no connected line; stdout:\n{}\nstderr:\n{}",
        stdout_of(srv),
        stderr_of(srv)
    );
    assert!(
        wait_until(Duration::from_secs(10), || tg
            .calls("getUpdates")
            .iter()
            .any(|r| r["json"]["timeout"].as_i64().unwrap_or(0) > 0)),
        "no long poll; records:\n{:#?}\nstderr:\n{}",
        tg.records(),
        stderr_of(srv)
    );
}

fn text_of(rec: &Value) -> &str {
    rec["json"]["text"].as_str().unwrap_or("")
}

fn sent_to(tg: &FakeTelegram, chat: i64) -> Vec<Value> {
    tg.calls("sendMessage")
        .into_iter()
        .filter(|r| r["json"]["chat_id"].as_i64() == Some(chat))
        .collect()
}

/// Sends `text` from the paired chat and returns the text of the one reply it produces.
fn ask(f: &Fixture, text: &str) -> String {
    let before = sent_to(&f.tg, PAIRED_CHAT).len();
    f.tg.push_update(f.tg.message_update(PAIRED_CHAT, PAIRED_CHAT, "private", text));
    let mut reply = None;
    wait_until(Duration::from_secs(15), || {
        let all = sent_to(&f.tg, PAIRED_CHAT);
        if all.len() > before {
            reply = Some(text_of(&all[before]).to_string());
            true
        } else {
            false
        }
    });
    match reply {
        Some(r) => r,
        None => panic!(
            "no reply to {text:?}; records:\n{:#?}\nstderr:\n{}",
            f.tg.records(),
            stderr_of(&f.srv)
        ),
    }
}

/// A sandbox with the journal section (`extra` may add keys) and the projects `du-an-a`.
fn sandbox(extra: impl FnOnce(&mut Value)) -> (Sandbox, Connection) {
    let sb = Sandbox::new();
    let remote = bare_remote(&sb, "vault");
    let mut section = journal_section(&remote);
    section["web_base_url"] = json!(WEB_BASE);
    extra(&mut section);
    sb.write_config(json!({ "journal": section }));
    let conn = setup_projects(&sb, &[("du-an-a", None)]);
    (sb, conn)
}

/// One finished (or failed) dev run on 2026-01-15 for `task`.
fn run_on_15(conn: &Connection, task: i64, status: &str) {
    seed_run(
        conn,
        task,
        "dev",
        status,
        None,
        local_ms(2026, 1, 15, 9, 0, 0),
        Some(local_ms(2026, 1, 15, 9, 20, 0)),
        Some(1),
    );
}

/// Exports with `--no-push`, then starts `serve` against the fake Telegram.
fn export_and_serve(sb: Sandbox) -> Fixture {
    let out = export(&sb, &["--no-push"], local_ms(2026, 1, 15, 21, 0, 0));
    assert_eq!(out.code, 0, "{}\n{}", out.stdout, out.stderr);
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let srv = tg.serve(&sb, &[]);
    wait_polling(&tg, &srv);
    Fixture { sb, tg, srv }
}

/// Every file of the vault clone (outside `.git`): relative path and text.
fn vault_files(root: &Path) -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                walk(&p, root, out);
            } else if let Ok(text) = std::fs::read_to_string(&p) {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().to_string();
                out.push((rel, text));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

fn vault_text(sb: &Sandbox) -> String {
    let files = vault_files(&clone_dir(sb));
    assert!(files.iter().any(|(p, _)| p == DAY_FILE), "{files:?}");
    files
        .iter()
        .map(|(p, t)| format!("== {p}\n{t}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// WR-08: a proposal title is shown by the vault only up to `callout_title_chars`; a word that
/// exists only after that cut is not indexed and not returned.
#[test]
fn tim_does_not_index_title_text_the_vault_cuts() {
    let (sb, conn) = sandbox(|_| {});
    // 140 characters at most; `tukhoaxanhla` starts after the 60-character vault cut.
    let title = "Rà soát tài liệu hướng dẫn cài đặt môi trường làm việc cho nhóm mới vào \
                 tukhoaxanhla rồi kiểm tra lại toàn bộ cuoicungtitle xong";
    let word_at = title
        .find("tukhoaxanhla")
        .map(|b| title[..b].chars().count());
    assert!(word_at.unwrap() > 70, "{word_at:?}");
    assert!(title.chars().count() <= 140, "{}", title.chars().count());
    let task = seed_task(&conn, "du-an-a", "ra-soat", "Việc phụ trợ khác hẳn");
    seed_proposal(&conn, task, title);
    run_on_15(&conn, task, "succeeded");
    drop(conn);

    let f = export_and_serve(sb);
    let vault = vault_text(&f.sb);
    assert!(vault.contains("Rà soát tài liệu"), "{vault}");
    for word in ["tukhoaxanhla", "cuoicungtitle"] {
        assert!(!vault.contains(word), "vault shows {word}:\n{vault}");
    }

    // the hit shows the vault's text only
    let reply = ask(&f, "/tim soat");
    assert!(reply.contains("du-an-a — Rà soát tài liệu"), "{reply}");
    for word in ["tukhoaxanhla", "cuoicungtitle"] {
        assert!(!reply.contains(word), "reply shows {word}: {reply}");
    }
    // and the words beyond the cut find nothing (a not-found reply only echoes the query)
    for word in ["tukhoaxanhla", "cuoicungtitle"] {
        let reply = ask(&f, &format!("/tim {word}"));
        assert!(reply.starts_with(NOT_FOUND), "{word}: {reply}");
        assert!(!reply.contains("du-an-a"), "{word}: {reply}");
    }
    f.finish();
}

/// IN-08: more matching results than fit in one Telegram message: the reply stops at a result
/// boundary, says how many were left out, and no URL is cut.
#[test]
fn tim_reply_stops_at_a_result_boundary() {
    let (sb, conn) = sandbox(|s| {
        s["title_max_chars"] = json!(400);
        s["callout_title_chars"] = json!(200);
        s["search_max_results"] = json!(20);
    });
    let filler = "Rà soát tài liệu hướng dẫn cài đặt môi trường ".repeat(12);
    for i in 0..22 {
        let title = format!("baocaotuan {i:02} {filler}");
        let title = title.chars().take(400).collect::<String>();
        assert_eq!(title.chars().count(), 400);
        let task = seed_task(&conn, "du-an-a", &format!("bao-cao-{i:02}"), &title);
        seed_proposal(&conn, task, &title);
        run_on_15(&conn, task, "succeeded");
    }
    drop(conn);

    let f = export_and_serve(sb);
    let reply = ask(&f, "/tim baocaotuan");
    let n = reply.chars().count();
    assert!(n < 4096, "{n} characters");

    let lines: Vec<&str> = reply.lines().collect();
    let urls: Vec<&&str> = lines.iter().filter(|l| l.starts_with("https://")).collect();
    let shown = urls.len();
    assert!(shown >= 1 && shown < 20, "{shown} results listed:\n{reply}");
    for u in &urls {
        assert!(
            u.starts_with(WEB_BASE) && u.ends_with("2026-01-15.md"),
            "{u}"
        );
    }
    assert_eq!(
        lines[0],
        format!("Nhật ký — {shown} kết quả cho: baocaotuan"),
        "{reply}"
    );
    assert_eq!(
        lines.last().copied(),
        Some(format!("… và {} kết quả khác", 20 - shown).as_str()),
        "{reply}"
    );
    // every listed result is a date/title line followed by its whole link
    let titles = lines.iter().filter(|l| l.contains(" · du-an-a — ")).count();
    assert_eq!(titles, shown, "{reply}");
    f.finish();
}

/// WR-05 and WR-06 end to end: ordinary text with a date and `CI/CD` survives in the vault and in
/// `/tim`; a URL, a host and a path leak into no vault file, no digest and no `/tim` reply.
#[test]
fn ordinary_slash_text_survives_and_real_paths_and_hosts_do_not_leak() {
    let (sb, conn) = sandbox(|_| {});
    let ok_title = "Cấu hình CI/CD cho bản phát hành ngày 15/01";
    let t1 = seed_task(&conn, "du-an-a", "cau-hinh-ci", ok_title);
    seed_proposal(&conn, t1, ok_title);
    run_on_15(&conn, t1, "succeeded");

    let bad_title =
        "Mở https://intranet.example.co/x?token=abc rồi sửa /home/someone/app/config.php \
                     trên git.example.co";
    let t2 = seed_task(&conn, "du-an-a", "mo-trang", bad_title);
    seed_proposal(&conn, t2, bad_title);
    run_on_15(&conn, t2, "failed");

    // the digest text of that day, from the same collector and summary the exporter uses
    let tz = chrono::FixedOffset::east_opt(7 * 3600).unwrap();
    let day = chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let cfg = agent_brain::journal::config::JournalConfig::default();
    let model =
        agent_brain::journal::collect::collect(&conn, &sb.read_map(), &cfg, &[], day, &tz).unwrap();
    let summary = agent_brain::journal::model::summarize(&model, day);
    let digest = agent_brain::journal::digest::digest_text(&summary, &cfg, None);
    drop(conn);
    assert!(digest.contains("lỗi — du-an-a — Mở"), "{digest}");

    let f = export_and_serve(sb);
    let vault = vault_text(&f.sb);
    assert!(vault.contains(ok_title), "{vault}");
    assert!(vault.contains("ngày 15/01"), "{vault}");

    let reply_ci = ask(&f, "/tim CI");
    assert!(
        reply_ci.contains(&format!("du-an-a — {ok_title} · ")),
        "{reply_ci}"
    );
    let reply_open = ask(&f, "/tim sua");
    assert!(reply_open.contains("du-an-a — Mở"), "{reply_open}");

    // searching for the leaked words finds nothing (a not-found reply only echoes the query)
    for word in ["intranet", "someone", "config"] {
        let reply = ask(&f, &format!("/tim {word}"));
        assert!(reply.starts_with(NOT_FOUND), "{word}: {reply}");
        assert!(!reply.contains("du-an-a"), "{word}: {reply}");
    }
    let replies = [reply_ci, reply_open];
    for (name, text) in [("vault", &vault), ("digest", &digest)]
        .into_iter()
        .chain(replies.iter().map(|r| ("/tim reply", r)))
    {
        for leak in [
            "intranet",
            "example.co",
            "token=abc",
            "/home",
            "someone",
            "config.php",
        ] {
            let seen = text.contains(leak);
            assert!(!seen, "{name} leaks {leak}:\n{text}");
        }
    }
    f.finish();
}
