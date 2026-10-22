//! Forwarded Telegram messages (Phase 05.4, INTAKE-05/06).
//!
//! A forwarded message is written by a third party, so everything read here is
//! untrusted data: it is shown to the paired user and, once picked, quoted inside
//! the nonce-delimited block of `compose::compose_forwarded`. Only the text and
//! the caption are used; images and files are never downloaded.
//!
//! The author and the original time come from `forward_origin` (Bot API 7.0+).
//! The ☑/☐ state lives in the server-held list; buttons carry only the flow id
//! and an index into it.

use super::callback::{encode_intake, IntakeCb, IntakeOp};
use super::compose::Quote;
use super::render::{self, Button};
use serde_json::Value;

/// Most messages in one batch; later ones are ignored (one `OVER_CAP` reply).
pub const MAX_FORWARDS: usize = 20;
/// The collection window never stays open longer than this after the first forward.
pub const COLLECT_MAX_MS: i64 = 10_000;
/// The window closes after this long without a new forward.
pub const DEFAULT_QUIET_MS: u64 = 2_000;
pub const OVER_CAP: &str = "Chỉ nhận tối đa 20 tin một lần — các tin sau bị bỏ qua.";
/// Reply when no forwarded message of a batch has text (`{k}` = skipped count).
pub const NOTHING_USABLE_FMT: &str = "Không có tin nào có chữ — bỏ qua {k} tin.";
/// Sender label when `forward_origin` names nobody usable.
pub const UNKNOWN_SENDER: &str = "(không rõ người gửi)";

const QUIET_ENV: &str = "AGENT_BRAIN_TELEGRAM_INTAKE_QUIET_MS";
const QUIET_MIN_MS: u64 = 100;
const QUIET_MAX_MS: u64 = 10_000;
/// Longest sender label, and its length on a toggle button.
const SENDER_MAX: usize = 80;
const BUTTON_SENDER_MAX: usize = 20;
/// Characters of each message shown in the selection list.
const PREVIEW_CHARS: usize = 60;

/// One collected forwarded message.
#[derive(Debug, Clone, PartialEq)]
pub struct ForwardItem {
    pub sender: String,
    pub at: String,
    pub text: String,
    /// The text is the caption of a photo or file.
    pub media: bool,
    pub selected: bool,
}

/// The quiet window in ms: `AGENT_BRAIN_TELEGRAM_INTAKE_QUIET_MS` (clamped to
/// 100..=10000), else `DEFAULT_QUIET_MS`.
pub fn quiet_ms() -> u64 {
    quiet_ms_from(std::env::var(QUIET_ENV).ok().as_deref())
}

fn quiet_ms_from(value: Option<&str>) -> u64 {
    match value.and_then(|v| v.trim().parse::<u64>().ok()) {
        Some(v) => v.clamp(QUIET_MIN_MS, QUIET_MAX_MS),
        None => DEFAULT_QUIET_MS,
    }
}

/// The message's `forward_origin`, when it is an object.
pub fn forward_origin(msg: &Value) -> Option<&Value> {
    msg.get("forward_origin").filter(|v| v.is_object())
}

/// `text` on one line, trimmed, cut to `max` characters (ending with `…` when cut).
fn cut(text: &str, max: usize) -> String {
    let flat = text.replace(['\r', '\n'], " ");
    let flat = flat.trim();
    if flat.chars().count() <= max {
        return flat.to_string();
    }
    let mut out: String = flat.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("").trim()
}

/// `title (signature)`, or the title alone when there is no signature.
fn chat_label(chat: &Value, signature: &str) -> String {
    let title = str_of(chat, "title");
    match (title.is_empty(), signature.is_empty()) {
        (true, _) => String::new(),
        (false, true) => title.to_string(),
        (false, false) => format!("{title} ({signature})"),
    }
}

/// Who wrote the forwarded message: user `first [last] (@username)`, hidden
/// user `sender_user_name`, chat or channel `title (author_signature)`, else
/// `(không rõ người gửi)`. At most 80 characters, one line.
pub fn sender_label(origin: &Value) -> String {
    let label = match origin.get("type").and_then(Value::as_str) {
        Some("user") => {
            let user = &origin["sender_user"];
            let name = [str_of(user, "first_name"), str_of(user, "last_name")]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(" ");
            match (name.is_empty(), str_of(user, "username")) {
                (true, _) => String::new(),
                (false, "") => name,
                (false, u) => format!("{name} (@{u})"),
            }
        }
        Some("hidden_user") => str_of(origin, "sender_user_name").to_string(),
        Some("chat") => chat_label(&origin["sender_chat"], str_of(origin, "author_signature")),
        Some("channel") => chat_label(&origin["chat"], str_of(origin, "author_signature")),
        _ => String::new(),
    };
    let label = cut(&label, SENDER_MAX);
    if label.is_empty() {
        UNKNOWN_SENDER.to_string()
    } else {
        label
    }
}

/// Unix seconds as local `YYYY-MM-DD HH:MM`, `--` when out of range.
pub fn when_label(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "--".to_string())
}

/// The usable content of a forwarded message: its text, else its caption
/// (`media`), else `None` (nothing to quote). Selected by default.
pub fn item_of(msg: &Value) -> Option<ForwardItem> {
    let nonempty = |key: &str| {
        msg.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let (text, media) = match nonempty("text") {
        Some(t) => (t, false),
        None => (nonempty("caption")?, true),
    };
    let origin = forward_origin(msg).cloned().unwrap_or(Value::Null);
    let at = origin
        .get("date")
        .and_then(Value::as_i64)
        .map(when_label)
        .unwrap_or_else(|| "--".to_string());
    Some(ForwardItem {
        sender: sender_label(&origin),
        at,
        text,
        media,
        selected: true,
    })
}

fn selected_count(items: &[ForwardItem]) -> usize {
    items.iter().filter(|i| i.selected).count()
}

/// The selection message: a header, one line per message, the skipped count.
pub fn selection_text(items: &[ForwardItem], skipped: usize) -> String {
    let mut lines = vec![format!(
        "Đã nhận {} tin chuyển tiếp — chọn tin cần gộp thành một task:",
        items.len()
    )];
    for (i, item) in items.iter().enumerate() {
        let preview: String = item
            .text
            .replace(['\r', '\n'], " ")
            .chars()
            .take(PREVIEW_CHARS)
            .collect();
        lines.push(format!(
            "[{}] {} · {} · {}",
            i + 1,
            item.sender,
            item.at,
            preview
        ));
    }
    if skipped > 0 {
        lines.push(format!("(bỏ qua {skipped} tin không có chữ)"));
    }
    render::clip(&lines.join("\n"), render::MAX_TEXT)
}

fn button(label: String, op: IntakeOp, flow: &str, index: Option<usize>) -> Button {
    Button {
        label,
        data: encode_intake(&IntakeCb {
            op,
            flow: flow.to_string(),
            index,
        }),
    }
}

/// One ☑/☐ toggle per message, then `Chọn hết` + `Bỏ hết`, `Tiếp tục (k tin)`
/// and `Huỷ`. The data carries only the flow id and the message's index.
pub fn selection_keyboard(flow: &str, items: &[ForwardItem]) -> Vec<Vec<Button>> {
    let mut rows: Vec<Vec<Button>> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let mark = if item.selected { "☑" } else { "☐" };
            let label = format!(
                "{mark} {} · {}",
                i + 1,
                cut(&item.sender, BUTTON_SENDER_MAX)
            );
            vec![button(label, IntakeOp::Toggle, flow, Some(i))]
        })
        .collect();
    rows.push(vec![
        button("Chọn hết".to_string(), IntakeOp::All, flow, None),
        button("Bỏ hết".to_string(), IntakeOp::None, flow, None),
    ]);
    rows.push(vec![button(
        format!("Tiếp tục ({} tin)", selected_count(items)),
        IntakeOp::Go,
        flow,
        None,
    )]);
    rows.push(vec![button(
        "Huỷ".to_string(),
        IntakeOp::Cancel,
        flow,
        None,
    )]);
    rows
}

/// The selected messages, in order, as quotes for the composer.
pub fn to_quotes(items: &[ForwardItem]) -> Vec<Quote> {
    items
        .iter()
        .filter(|i| i.selected)
        .map(|i| Quote {
            sender: i.sender.clone(),
            at: i.at.clone(),
            text: i.text.clone(),
            media: i.media,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telegram::callback::{decode_intake, MAX_LEN};
    use serde_json::json;

    #[test]
    fn sender_label_of_each_origin() {
        let user = json!({"type": "user", "date": 1, "sender_user": {
            "id": 1, "is_bot": false, "first_name": "Nguyễn", "last_name": "An", "username": "an_ng"}});
        assert_eq!(sender_label(&user), "Nguyễn An (@an_ng)");
        let bare = json!({"type": "user", "date": 1, "sender_user": {"id": 1, "first_name": "An"}});
        assert_eq!(sender_label(&bare), "An");
        let hidden = json!({"type": "hidden_user", "date": 1, "sender_user_name": "Bình"});
        assert_eq!(sender_label(&hidden), "Bình");
        let chat = json!({"type": "chat", "date": 1,
            "sender_chat": {"id": -5, "type": "group", "title": "Nhóm Dev"},
            "author_signature": "Lan"});
        assert_eq!(sender_label(&chat), "Nhóm Dev (Lan)");
        let chat_nosig = json!({"type": "chat", "date": 1,
            "sender_chat": {"id": -5, "type": "group", "title": "Nhóm Dev"}});
        assert_eq!(sender_label(&chat_nosig), "Nhóm Dev");
        let channel = json!({"type": "channel", "date": 1, "message_id": 7,
            "chat": {"id": -100, "type": "channel", "title": "Kênh Dự án"},
            "author_signature": "Chi"});
        assert_eq!(sender_label(&channel), "Kênh Dự án (Chi)");

        // Missing or unknown type, or nothing usable: unknown sender.
        for o in [
            json!({"date": 1, "sender_user_name": "X"}),
            json!({"type": "robot", "date": 1}),
            json!({"type": "hidden_user", "date": 1}),
            json!({"type": "user", "date": 1, "sender_user": {"id": 1}}),
            Value::Null,
        ] {
            assert_eq!(sender_label(&o), UNKNOWN_SENDER, "{o}");
        }

        // One line, at most 80 characters.
        let long = json!({"type": "hidden_user", "date": 1,
            "sender_user_name": format!("a\nb{}", "x".repeat(200))});
        let got = sender_label(&long);
        assert_eq!(got.chars().count(), 80, "{got}");
        assert!(!got.contains('\n') && got.starts_with("a b") && got.ends_with('…'));
    }

    #[test]
    fn when_label_is_local_time() {
        use chrono::TimeZone;
        let secs = 1_792_652_580;
        let want = chrono::Local
            .timestamp_opt(secs, 0)
            .single()
            .expect("a local time")
            .format("%Y-%m-%d %H:%M")
            .to_string();
        assert_eq!(when_label(secs), want);
        assert_eq!(when_label(i64::MAX), "--");
    }

    #[test]
    fn item_of_text_caption_or_nothing() {
        let origin =
            json!({"type": "hidden_user", "date": 1_792_652_580, "sender_user_name": "Bình"});
        let text =
            json!({"message_id": 1, "text": "  Thêm kiểm tra email ", "forward_origin": origin});
        let item = item_of(&text).expect("a text item");
        assert_eq!(
            item,
            ForwardItem {
                sender: "Bình".to_string(),
                at: when_label(1_792_652_580),
                text: "Thêm kiểm tra email".to_string(),
                media: false,
                selected: true,
            }
        );
        let photo = json!({"message_id": 2, "caption": "Ảnh lỗi", "forward_origin": origin,
            "photo": [{"file_id": "f1", "file_unique_id": "u1", "width": 1, "height": 1}]});
        let item = item_of(&photo).expect("a caption item");
        assert_eq!((item.text.as_str(), item.media), ("Ảnh lỗi", true));
        let bare = json!({"message_id": 3, "forward_origin": origin,
            "photo": [{"file_id": "f1", "file_unique_id": "u1", "width": 1, "height": 1}]});
        assert_eq!(item_of(&bare), None);
        let blank =
            json!({"message_id": 4, "text": "   ", "caption": "", "forward_origin": origin});
        assert_eq!(item_of(&blank), None);
        // No origin: still data, with an unknown sender and time.
        let plain = json!({"message_id": 5, "text": "x"});
        let item = item_of(&plain).unwrap();
        assert_eq!(
            (item.sender.as_str(), item.at.as_str()),
            (UNKNOWN_SENDER, "--")
        );
    }

    fn items(n: usize) -> Vec<ForwardItem> {
        (0..n)
            .map(|i| ForwardItem {
                sender: format!("Người gửi có tên rất dài số {i}"),
                at: "2026-10-21 14:03".to_string(),
                text: format!("tin {i}\n{}", "x".repeat(100)),
                media: false,
                selected: i % 2 == 0,
            })
            .collect()
    }

    #[test]
    fn selection_keyboard_data_decodes_and_fits() {
        let list = items(MAX_FORWARDS);
        let rows = selection_keyboard("0123abcd", &list);
        assert_eq!(rows.len(), MAX_FORWARDS + 3);
        for (i, row) in rows.iter().take(MAX_FORWARDS).enumerate() {
            let cb = decode_intake(&row[0].data).expect("decodes");
            assert_eq!(cb.op, IntakeOp::Toggle);
            assert_eq!((cb.flow.as_str(), cb.index), ("0123abcd", Some(i)));
            let mark = if i % 2 == 0 { "☑" } else { "☐" };
            assert!(
                row[0].label.starts_with(&format!("{mark} {} · ", i + 1)),
                "{}",
                row[0].label
            );
        }
        let labels: Vec<&str> = rows[MAX_FORWARDS..]
            .iter()
            .flatten()
            .map(|b| b.label.as_str())
            .collect();
        assert_eq!(
            labels,
            vec!["Chọn hết", "Bỏ hết", "Tiếp tục (10 tin)", "Huỷ"]
        );
        for b in rows.iter().flatten() {
            assert!(b.data.len() <= MAX_LEN, "{}", b.data);
            let cb = decode_intake(&b.data).expect("decodes");
            assert_eq!(cb.flow, "0123abcd");
            // Never any text or state in the data.
            assert!(
                !b.data.contains('☑') && !b.data.contains("tin"),
                "{}",
                b.data
            );
        }
    }

    #[test]
    fn selection_text_lists_messages() {
        let mut list = items(2);
        list[1].selected = true;
        let text = selection_text(&list, 1);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "Đã nhận 2 tin chuyển tiếp — chọn tin cần gộp thành một task:"
        );
        let preview: String = format!("tin 0 {}", "x".repeat(100))
            .chars()
            .take(60)
            .collect();
        assert_eq!(
            lines[1],
            format!("[1] Người gửi có tên rất dài số 0 · 2026-10-21 14:03 · {preview}")
        );
        assert!(lines[2].starts_with("[2] "));
        assert_eq!(lines[3], "(bỏ qua 1 tin không có chữ)");
        assert_eq!(lines.len(), 4);
        assert!(!selection_text(&list, 0).contains("bỏ qua"));
    }

    #[test]
    fn to_quotes_keeps_selected_in_order() {
        let list = items(5);
        let quotes = to_quotes(&list);
        assert_eq!(quotes.len(), 3);
        assert!(quotes[0].text.starts_with("tin 0"));
        assert!(quotes[1].text.starts_with("tin 2"));
        assert!(quotes[2].text.starts_with("tin 4"));
    }

    #[test]
    fn quiet_window_env_is_clamped() {
        assert_eq!(quiet_ms_from(None), DEFAULT_QUIET_MS);
        assert_eq!(quiet_ms_from(Some("300")), 300);
        assert_eq!(quiet_ms_from(Some(" 300 ")), 300);
        assert_eq!(quiet_ms_from(Some("5")), 100);
        assert_eq!(quiet_ms_from(Some("999999")), 10_000);
        assert_eq!(quiet_ms_from(Some("abc")), DEFAULT_QUIET_MS);
        assert_eq!(quiet_ms_from(Some("-1")), DEFAULT_QUIET_MS);
    }
}
