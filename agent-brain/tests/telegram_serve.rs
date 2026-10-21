mod common;
#[allow(dead_code)]
#[path = "support/telegram.rs"]
mod tg;

use common::{wait_until, Sandbox, Serve};
use std::time::Duration;
use tg::{write_config, FakeTelegram, FAKE_TOKEN, PAIRED_CHAT};

fn stdout_of(srv: &Serve) -> String {
    srv.stdout.lock().unwrap().clone()
}

fn stderr_of(srv: &Serve) -> String {
    std::fs::read_to_string(&srv.stderr_path).unwrap_or_default()
}

#[test]
fn serve_connects_to_fake_telegram() {
    let sb = Sandbox::new();
    let tg = FakeTelegram::start(&sb);
    write_config(&sb, PAIRED_CHAT, 0o600);
    let mut srv = tg.serve(&sb, &[]);

    let connected = "Telegram: đã kết nối (@ab_fake_bot)";
    assert!(
        wait_until(Duration::from_secs(10), || stdout_of(&srv)
            .contains(connected)),
        "no connected line; stdout:\n{}\nstderr:\n{}",
        stdout_of(&srv),
        stderr_of(&srv)
    );

    let out = stdout_of(&srv);
    let url_pos = out
        .find("http://127.0.0.1:")
        .expect("URL line in serve stdout");
    let tg_pos = out.find(connected).expect("connected line");
    assert!(url_pos < tg_pos, "URL line must come first:\n{out}");
    assert!(
        out.lines().next().unwrap_or("").contains("/?token="),
        "first stdout line must stay the dashboard URL:\n{out}"
    );

    let calls = tg.calls("getMe");
    assert!(!calls.is_empty(), "fake Telegram recorded no getMe");
    for c in &calls {
        assert_eq!(c["token"].as_str(), Some(FAKE_TOKEN));
    }

    assert!(!out.contains(FAKE_TOKEN), "token leaked to stdout");
    assert!(
        !stderr_of(&srv).contains(FAKE_TOKEN),
        "token leaked to stderr"
    );

    assert_eq!(srv.get("/").status, 200);
    assert_eq!(srv.stop(libc::SIGINT, Duration::from_secs(5)), Some(0));
}
