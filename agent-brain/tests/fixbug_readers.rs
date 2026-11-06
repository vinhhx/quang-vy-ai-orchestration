#[path = "support/fixbug.rs"]
mod support_fixbug;

use agent_brain::fixbug::config::{parse_gsheet_url, FixbugConfig, SourceConfig};
use agent_brain::fixbug::envfile::parse_env_content;
use agent_brain::fixbug::gsheet::{
    fetch_sheet_bugs, fnv1a_64, hash_group_content, parse_csv, parse_sheet_csv,
};
use agent_brain::fixbug::trello::TrelloClient;
use agent_brain::fixbug::FixbugError;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::sync::Mutex;
use support_fixbug::{FakeGsheet, FakeTrello};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_config_parse_example() {
    let json_text = r#"{
      "sources": [
        { "type": "trello",
          "board": null,
          "intake_lists": ["Bugs"],
          "fixed_list": "Testing",
          "rejected_list": "Reject",
          "labels": [],
          "priority_labels": { "urgent": ["high"], "low": [] },
          "comment_prefix": "[agent-brain]" },
        { "type": "gsheet",
          "url": "https://docs.google.com/spreadsheets/d/abc-123/edit?gid=456#gid=456",
          "columns": { "id": "STT", "group": "Hạng mục", "content": "Nội dung phản hồi",
                       "status": "Trạng thái", "priority": "Mức độ Ưu tiên",
                       "attachment": "Mô tả/chụp màn hình", "reporter": "Nhân sự feedback" },
          "todo_status": [""],
          "priority_values": { "urgent": ["Cao", "High", "Gấp"], "low": ["Thấp", "Low"] } }
      ]
    }"#;

    let cfg: FixbugConfig = serde_json::from_str(json_text).expect("parse config");
    assert_eq!(cfg.sources.len(), 2);
    match &cfg.sources[0] {
        SourceConfig::Trello(t) => {
            assert_eq!(t.board, None);
            assert_eq!(t.intake_lists, vec!["Bugs"]);
            assert_eq!(t.fixed_list, "Testing");
            assert_eq!(t.rejected_list, "Reject");
            assert_eq!(t.comment_prefix, "[agent-brain]");
        }
        _ => panic!("Expected Trello source"),
    }

    match &cfg.sources[1] {
        SourceConfig::Gsheet(g) => {
            assert_eq!(g.columns.id, "STT");
            assert_eq!(g.columns.content, "Nội dung phản hồi");
            assert_eq!(g.columns.status, "Trạng thái");
            assert_eq!(g.todo_status, vec![""]);
        }
        _ => panic!("Expected Gsheet source"),
    }
}

#[test]
fn test_config_deny_unknown_fields() {
    let json_text = r#"{
      "sources": [
        { "type": "trello", "unknown_key": "val" }
      ]
    }"#;
    let err = serde_json::from_str::<FixbugConfig>(json_text).unwrap_err();
    assert!(err.to_string().contains("unknown field `unknown_key`"));
}

#[test]
fn test_config_gsheet_urls() {
    let urls = vec![
        ("https://docs.google.com/spreadsheets/d/sheet_id_1/edit?gid=100#gid=100", "sheet_id_1", "100"),
        ("https://docs.google.com/spreadsheets/d/sheet_id_2/edit#gid=200", "sheet_id_2", "200"),
        ("https://docs.google.com/spreadsheets/d/sheet_id_3/htmlview", "sheet_id_3", "0"),
        ("https://docs.google.com/spreadsheets/d/sheet_id_4/export?format=csv&gid=400", "sheet_id_4", "400"),
    ];

    for (url, expected_id, expected_gid) in urls {
        let (id, gid) = parse_gsheet_url(url).expect("valid sheet url");
        assert_eq!(id, expected_id);
        assert_eq!(gid, expected_gid);
    }

    let bad_host = "https://other.domain.com/spreadsheets/d/abc/edit";
    let err = parse_gsheet_url(bad_host).unwrap_err();
    assert!(err.to_string().contains("docs.google.com"));
}

#[test]
fn test_envfile_parse_and_debug_mask() {
    let content = r#"
    # Comment line
    TRELLO_API_KEY=key_12345678
    export TRELLO_TOKEN='token_abcdefgh'
    TRELLO_BOARD_ID="board_98765432"
    EMPTY=
    export UNQUOTED=some_value
    "#;

    let secrets = parse_env_content(content);
    assert_eq!(secrets.trello_api_key(), Some("key_12345678"));
    assert_eq!(secrets.trello_token(), Some("token_abcdefgh"));
    assert_eq!(secrets.trello_board_id(), Some("board_98765432"));
    assert_eq!(secrets.get("EMPTY"), Some(""));
    assert_eq!(secrets.get("UNQUOTED"), Some("some_value"));

    let debug_out = format!("{secrets:?}");
    assert_eq!(debug_out, "FixbugSecrets(<ẩn>)");
    assert!(!debug_out.contains("key_12345678"));
    assert!(!debug_out.contains("token_abcdefgh"));
}

#[test]
fn test_csv_rfc4180_edge_cases() {
    let csv = "\u{feff}\"col1\",\"col2,with,comma\",\"col3\"\"quoted\"\"\"\r\n\"line1\nline2\",\"val2\",\"val3\"\r\n";
    let rows = parse_csv(csv);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], vec!["col1", "col2,with,comma", "col3\"quoted\""]);
    assert_eq!(rows[1], vec!["line1\nline2", "val2", "val3"]);
}

#[test]
fn test_gsheet_parse_and_fnv1a() {
    assert_eq!(fnv1a_64(b""), 0xcbf29ce484222325);
    let hash8 = hash_group_content("Admin", "Nút lưu lỗi");
    assert_eq!(hash8.len(), 8);

    let csv = "STT,Nhân sự feedback,Hạng mục,Nội dung phản hồi,Mô tả/chụp màn hình,Mức độ Ưu tiên,Phân loại,Trạng thái\n\
1,QA 1,Giao diện,Nút bấm lệch,,Cao,,Hoàn Thành\n\
2,QA 1,,Font chữ quá nhỏ,,Thấp,,\n\
,QA 2,Chức năng,Lỗi 500 khi submit,,Gấp,,\n\
4,QA 2,,Từ chối,,,,Từ chối thực hiện\n";

    let config = agent_brain::fixbug::config::GsheetConfig {
        url: "https://docs.google.com/spreadsheets/d/abc/edit?gid=10".to_string(),
        columns: Default::default(),
        todo_status: vec!["".to_string()],
        priority_values: Default::default(),
    };

    let bugs = parse_sheet_csv(csv, &config, &config.url, "10").unwrap();
    assert_eq!(bugs.len(), 2);

    // Row 2: key S-2, carry-forward "Giao diện", priority "low"
    assert_eq!(bugs[0].key, "S-2");
    assert_eq!(bugs[0].ext_id, "10:2");
    assert_eq!(bugs[0].group, "Giao diện");
    assert_eq!(bugs[0].priority, "low");

    // Row 3: empty STT -> key S-h..., group "Chức năng", priority "urgent"
    assert!(bugs[1].key.starts_with("S-h"));
    assert!(bugs[1].ext_id.starts_with("10:h"));
    assert_eq!(bugs[1].group, "Chức năng");
    assert_eq!(bugs[1].priority, "urgent");
}

#[test]
fn test_trello_client_with_fake() {
    let _env_lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let fake = FakeTrello::start(temp.path());

    fake.set_data(json!({
        "lists": [
            {"id": "l_bugs", "name": "Bugs", "closed": false},
            {"id": "l_test", "name": "Testing", "closed": false},
            {"id": "l_rej", "name": "Reject", "closed": false}
        ],
        "cards": [
            {
                "id": "c_1",
                "idShort": 42,
                "name": "Lỗi đăng nhập",
                "desc": "Không đăng nhập được bằng Google",
                "idList": "l_bugs",
                "labels": [{"id": "lbl_1", "name": "high"}],
                "shortUrl": "https://trello.com/c/c_1",
                "attachments": []
            }
        ],
        "comments": {
            "c_1": [
                {"date": "2026-11-06T10:00:00Z", "author": "QA Tester", "text": "Đã tái hiện trên Chrome"}
            ]
        }
    }));

    let client = TrelloClient::new(&fake.url(), "fake_key_123456", "fake_token_123456");

    // 1. lists
    let lists = client.lists("b_1").unwrap();
    assert_eq!(lists.len(), 3);
    assert_eq!(lists[0].name, "Bugs");

    // 2. cards
    let cards = client.cards("b_1").unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].id_short, 42);
    assert_eq!(cards[0].name, "Lỗi đăng nhập");

    // 3. comments
    let comments = client.comments("c_1").unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].author, "QA Tester");
    assert_eq!(comments[0].text, "Đã tái hiện trên Chrome");

    // 4. add_comment
    client.add_comment("c_1", "Đang xử lý", "[agent-brain]").unwrap();
    let updated_comments = client.comments("c_1").unwrap();
    assert_eq!(updated_comments.len(), 2);
    assert_eq!(updated_comments[1].text, "[agent-brain] Đang xử lý");

    // 5. card_list_name
    let (cur_list_id, cur_list_name) = client.card_list_name("c_1", Some("b_1")).unwrap();
    assert_eq!(cur_list_id, "l_bugs");
    assert_eq!(cur_list_name, "Bugs");

    // 6. move_card
    client.move_card("c_1", "l_test").unwrap();
    let (new_list_id, new_list_name) = client.card_list_name("c_1", Some("b_1")).unwrap();
    assert_eq!(new_list_id, "l_test");
    assert_eq!(new_list_name, "Testing");
}

#[test]
fn test_trello_secrets_never_in_argv_and_redacted_on_error() {
    let _env_lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let fake = FakeTrello::start(temp.path());

    let key = "secret_key_88888888";
    let token = "secret_token_99999999";

    // Tạo curl wrapper script ghi lại argv vào file
    let argv_log = temp.path().join("curl_argv.log");
    let wrapper_path = temp.path().join("curl_wrapper.sh");
    let script_content = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\nexec /usr/bin/curl \"$@\"\n",
        argv_log.display()
    );
    std::fs::write(&wrapper_path, script_content).unwrap();
    let mut perms = std::fs::metadata(&wrapper_path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&wrapper_path, perms).unwrap();

    // Thiết lập AGENT_BRAIN_CURL trỏ tới wrapper
    std::env::set_var("AGENT_BRAIN_CURL", wrapper_path.to_str().unwrap());

    let client = TrelloClient::new(&fake.url(), key, token);
    let _ = client.lists("board_test");

    // Kiểm tra file log: key và token KHÔNG được xuất hiện trong argv
    let logged_argv = std::fs::read_to_string(&argv_log).unwrap_or_default();
    assert!(!logged_argv.is_empty(), "Curl wrapper phải được gọi");
    assert!(!logged_argv.contains(key), "Key không được nằm trong argv");
    assert!(!logged_argv.contains(token), "Token không được nằm trong argv");

    // Dọn dẹp env AGENT_BRAIN_CURL
    std::env::remove_var("AGENT_BRAIN_CURL");

    // Kiểm tra lỗi 401 echo token: token phải được ẩn trong thông điệp lỗi
    fake.set_behavior(json!([
        {
            "path": "/1/boards/board_err/lists",
            "times": 1,
            "status": 401,
            "body": format!("Unauthorized invalid token {token}")
        }
    ]));

    let err = client.lists("board_err").unwrap_err();
    match err {
        FixbugError::Trello { status, message } => {
            assert_eq!(status, 401);
            assert!(
                !message.contains(token),
                "Token không được xuất hiện trong message lỗi: {message}"
            );
            assert!(
                message.contains("[đã ẩn]"),
                "Message lỗi phải chứa [đã ẩn]: {message}"
            );
        }
        other => panic!("Kỳ vọng FixbugError::Trello, nhận: {:?}", other),
    }
}

#[test]
fn test_gsheet_client_fetch_with_fake() {
    let _env_lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let fake = FakeGsheet::start(temp.path());

    let csv_data = "STT,Nhân sự feedback,Hạng mục,Nội dung phản hồi,Mô tả/chụp màn hình,Mức độ Ưu tiên,Phân loại,Trạng thái\n\
1,QA 1,UI,Lỗi màu nền,,Cao,,Hoàn Thành\n\
2,QA 1,,Lỗi thanh cuộn,,Thấp,,\n\
3,QA 2,Backend,Crash server,,Gấp,,\n";

    fake.set_csv(csv_data);

    // Ghi đè AGENT_BRAIN_GSHEET_BASE để trỏ tới fake server
    std::env::set_var("AGENT_BRAIN_GSHEET_BASE", fake.url());

    // Chú ý: url trong GsheetConfig phải có host docs.google.com nếu dùng parse_gsheet_url thông thường,
    // nhưng khi test với AGENT_BRAIN_GSHEET_BASE, config.url có thể vẫn là docs.google.com,
    // client sẽ ghép {AGENT_BRAIN_GSHEET_BASE}/spreadsheets/d/{id}/export...
    let config_valid_host = agent_brain::fixbug::config::GsheetConfig {
        url: "https://docs.google.com/spreadsheets/d/mock_sheet_123/edit?gid=999".to_string(),
        columns: Default::default(),
        todo_status: vec!["".to_string()],
        priority_values: Default::default(),
    };

    let bugs = fetch_sheet_bugs(&config_valid_host, 10).expect("fetch sheet bugs");
    std::env::remove_var("AGENT_BRAIN_GSHEET_BASE");

    assert_eq!(bugs.len(), 2);
    assert_eq!(bugs[0].key, "S-2");
    assert_eq!(bugs[0].group, "UI");
    assert_eq!(bugs[0].priority, "low");

    assert_eq!(bugs[1].key, "S-3");
    assert_eq!(bugs[1].group, "Backend");
    assert_eq!(bugs[1].priority, "urgent");
}
