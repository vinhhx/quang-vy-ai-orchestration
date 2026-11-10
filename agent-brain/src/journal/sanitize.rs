//! Bộ lọc chữ đi ra vault (D-03, D-05, D-23) và cổng chặn secret trước khi ghi (JRN-03).
//!
//! Mọi chuỗi lấy từ brain.db phải đi qua `sanitize_text` để thành `SafeText`; `render` chỉ nhận
//! `SafeText`, nên chữ chưa lọc không thể lọt vào mẫu. `scan_generated` quét lại toàn bộ tệp đã
//! dựng (cả đường dẫn) trước khi ghi tệp đầu tiên.

use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

/// Dấu câu được giữ lại (ngoài chữ, số, khoảng trắng và `…`). Mọi ký tự khác, trong đó có
/// `[ ] ! # ` < > | * ~ = % $ ^ { } @ / \`, thành khoảng trắng.
pub const ALLOWED_PUNCT: &str = ".,;:()&+-_'\"?";

const ELLIPSIS: char = '…';

/// Chữ đã lọc: một dòng, đã cắt, không còn giá trị secret đã biết, không còn cú pháp
/// Markdown/Obsidian. Chỉ tạo được qua `sanitize_text` (hoặc cắt từ một `SafeText`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SafeText(String);

impl SafeText {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Bản cắt còn tối đa `max_chars` ký tự (thêm `…` khi cắt), cùng quy tắc với `sanitize_text`.
    pub fn cut(&self, max_chars: usize) -> SafeText {
        SafeText(cut_chars(&self.0, max_chars))
    }
}

impl std::fmt::Display for SafeText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn is_invisible(c: char) -> bool {
    c.is_control()
        || ('\u{200B}'..='\u{200F}').contains(&c)
        || ('\u{202A}'..='\u{202E}').contains(&c)
        || ('\u{2066}'..='\u{2069}').contains(&c)
        || c == '\u{FEFF}'
}

fn collapse_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cắt còn tối đa `max_chars` ký tự; khi phải cắt thì ưu tiên cắt ở khoảng trắng cuối cùng nằm
/// sau 70 % giới hạn và thêm `…` (tính cả `…` vào giới hạn).
pub fn cut_chars(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars {
        return s.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    let keep = max_chars - 1;
    let prefix: Vec<char> = s.chars().take(keep).collect();
    let min_at = (max_chars * 7).div_ceil(10);
    let mut end = keep;
    // `prefix[i] == ' '` → giữ `prefix[..i]`; một khoảng trắng ngay sau phần giữ cũng được tính.
    let space_after = s.chars().nth(keep) == Some(' ');
    if !space_after {
        if let Some(i) = (min_at..keep).rev().find(|&i| prefix[i] == ' ') {
            end = i;
        }
    }
    let mut out: String = prefix[..end].iter().collect();
    while out.ends_with(' ') {
        out.pop();
    }
    out.push(ELLIPSIS);
    out
}

/// NFC → bỏ ký tự điều khiển/zero-width/bidi/BOM → mọi chuỗi khoảng trắng (cả xuống dòng) thành
/// một khoảng trắng → thay giá trị secret đã biết → chỉ giữ chữ, số, khoảng trắng,
/// `ALLOWED_PUNCT` và `…` → gộp khoảng trắng → cắt `max_chars`.
pub fn sanitize_text(raw: &str, max_chars: usize, secrets: &[String]) -> SafeText {
    let nfc: String = raw.nfc().collect();
    let visible: String = nfc
        .chars()
        .map(|c| {
            if c.is_whitespace() {
                ' '
            } else if is_invisible(c) {
                '\u{0}'
            } else {
                c
            }
        })
        .filter(|c| *c != '\u{0}')
        .collect();
    let one_line = collapse_spaces(&visible);
    let redacted = crate::secrets::redact_str(&one_line, secrets);
    let allowed: String = redacted
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == ELLIPSIS || ALLOWED_PUNCT.contains(c) {
                c
            } else {
                ' '
            }
        })
        .collect();
    let collapsed = collapse_spaces(&allowed);
    SafeText(cut_chars(collapsed.trim(), max_chars))
}

/// Một lần chặn của cổng secret: chỉ có đường dẫn (đã che giá trị) và tên luật, không bao giờ
/// có giá trị.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretHit {
    pub path: String,
    pub rule: String,
}

/// Cổng cuối (JRN-03): quét nội dung và đường dẫn mọi tệp đã dựng. Có kết quả = không ghi gì.
pub fn scan_generated(files: &BTreeMap<String, String>, secrets: &[String]) -> Vec<SecretHit> {
    let mut hits = Vec::new();
    for (path, body) in files {
        if crate::secrets::contains_secret(body.as_bytes(), secrets)
            || crate::secrets::contains_secret(path.as_bytes(), secrets)
        {
            hits.push(SecretHit {
                path: crate::secrets::redact_str(path, secrets),
                rule: "known-value".to_string(),
            });
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_known_value_and_redacts_path() {
        let a = "khoa-bi-mat-mot-12345".to_string();
        let b = "khoa-bi-mat-hai-67890".to_string();
        let secrets = vec![a.clone(), b.clone()];
        let mut files = BTreeMap::new();
        files.insert(
            "Nhật ký/2026/01/2026-01-15.md".to_string(),
            format!("nội dung có {a} ở giữa\n"),
        );
        files.insert(format!("Dự án/{b}.md"), "nội dung sạch\n".to_string());
        files.insert("Today.md".to_string(), "sạch\n".to_string());
        let hits = scan_generated(&files, &secrets);
        assert_eq!(hits.len(), 2, "{hits:?}");
        for h in &hits {
            assert!(!h.path.contains(&a) && !h.path.contains(&b), "{h:?}");
            assert_eq!(h.rule, "known-value");
        }
    }

    #[test]
    fn sanitize_makes_one_short_line_without_markup() {
        let raw = "Dòng một [[x]] #tag `mã`\r\nDòng hai\t![[nhung]] <b>đậm</b> %%c%% $$m$$ {{t}}\u{200B}\u{202E}";
        let s = sanitize_text(raw, 140, &[]);
        assert!(!s.as_str().contains('\n'));
        for bad in [
            "[", "]", "#", "`", "<", ">", "%", "$", "{", "}", "!", "\u{200B}",
        ] {
            assert!(!s.as_str().contains(bad), "{bad} còn trong {s}");
        }
        assert!(s.as_str().starts_with("Dòng một x tag mã Dòng hai"), "{s}");
    }

    #[test]
    fn sanitize_redacts_known_secret_and_cuts() {
        let secret = "gia-tri-bi-mat-123".to_string();
        let s = sanitize_text(&format!("dùng {secret} nhé"), 140, &[secret.clone()]);
        assert!(!s.as_str().contains(&secret));
        assert!(s.as_str().contains("đã ẩn"), "{s}");

        let long = "từ ".repeat(100);
        let cut = sanitize_text(&long, 40, &[]);
        assert!(cut.as_str().chars().count() <= 40, "{cut}");
        assert!(cut.as_str().ends_with('…'));
        assert!(!cut.as_str().ends_with(" …"));
    }

    #[test]
    fn short_text_is_not_cut() {
        assert_eq!(
            sanitize_text("Dựng video giới thiệu sản phẩm mẫu", 60, &[]).as_str(),
            "Dựng video giới thiệu sản phẩm mẫu"
        );
        assert_eq!(cut_chars("abc", 3), "abc");
        assert_eq!(cut_chars("abcdef", 4), "abc…");
    }
}
