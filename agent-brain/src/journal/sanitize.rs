//! Bộ lọc chữ đi ra vault (D-03, D-05, D-23) và cổng chặn secret trước khi ghi (JRN-03).
//!
//! Mọi chuỗi lấy từ brain.db phải đi qua `sanitize_text` để thành `SafeText`; `render` chỉ nhận
//! `SafeText`, nên chữ chưa lọc không thể lọt vào mẫu. `scan_generated` quét lại toàn bộ tệp đã
//! dựng (cả đường dẫn) trước khi ghi tệp đầu tiên.

use regex::{NoExpand, Regex};
use std::collections::BTreeMap;
use std::sync::LazyLock;
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

const WORD_LINK: &str = "(liên kết)";
const WORD_EMAIL: &str = "(email)";
const WORD_HIDDEN: &str = "(đã ẩn)";
const WORD_ADDR: &str = "(địa chỉ)";
const WORD_PATH: &str = "(đường dẫn)";
const WORD_CODE: &str = "(đã bỏ phần mã)";

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("mẫu regex cố định của bộ lọc nhật ký phải hợp lệ")
}

/// Dòng bắt đầu của một diff dán vào (sau khoảng trắng đầu dòng tùy ý).
static RE_DIFF_LINE: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?m)^[ \t]*(?:diff --git |--- a/|\+\+\+ b/|@@ -|index [0-9a-f]{7,}\.\.)")
});
static RE_URL: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\b[a-z][a-z0-9+.-]*://\S+|\bwww\.\S+"));
static RE_EMAIL: LazyLock<Regex> = LazyLock::new(|| re(r"[\w.+-]+@[\w-]+\.[\w.-]+"));
/// Tên có chứa key/token/secret/password… rồi `:` hoặc `=` và giá trị (cả tên lẫn giá trị bị thay).
const KEY_ASSIGN: &str = r#"(?i)[A-Za-z0-9_.-]*(?:api[_-]?key|access[_-]?key|authorization|password|passwd|pwd|secret|token|key)["']?\s*[:=]\s*(?:(?:bearer|basic)\s+)?\S+"#;
static RE_KEY_ASSIGN: LazyLock<Regex> = LazyLock::new(|| re(KEY_ASSIGN));
static RE_BEARER: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{8,}"));
/// Dạng token của các nhà cung cấp (không gồm JWT và token bot Telegram).
const TOKEN_SHAPES: &str = r"\bsk-ant-[A-Za-z0-9_-]{8,}|\bsk-[A-Za-z0-9_-]{16,}|\bgh[pousr]_[A-Za-z0-9]{20,}|\bgithub_pat_[A-Za-z0-9_]{20,}|\bglpat-[A-Za-z0-9_-]{16,}|\bxox[baprs]-[A-Za-z0-9-]{10,}|\bAKIA[0-9A-Z]{16}|\bAIza[0-9A-Za-z_-]{35}";
const JWT_SHAPE: &str = r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*";
const TELEGRAM_SHAPE: &str = r"\b\d{6,}:[A-Za-z0-9_-]{30,}";
static RE_SHAPES: LazyLock<Regex> =
    LazyLock::new(|| re(&format!("{TOKEN_SHAPES}|{JWT_SHAPE}|{TELEGRAM_SHAPE}")));
static RE_IPV4: LazyLock<Regex> = LazyLock::new(|| {
    re(r"\b\d{1,3}(?:\.\d{1,3}){3}(?::\d{1,5})?\b|(?i:\blocalhost(?::\d{1,5})?\b)")
});
static RE_HOST: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)\b(?:[a-z0-9-]+\.)+(?:com|net|org|vn|io|dev|app|local|internal|lan|corp|test|cloud|xyz|info|biz)(?::\d{1,5})?\b",
    )
});
static RE_WIN_PATH: LazyLock<Regex> = LazyLock::new(|| re(r"\b[A-Za-z]:\\\S*"));
static RE_HOME_PATH: LazyLock<Regex> = LazyLock::new(|| re(r"~/\S*"));
static RE_ABS_PATH: LazyLock<Regex> = LazyLock::new(|| re(r#"(^|[\s(\["'=,;])/\S+"#));
static RE_REL_PATH: LazyLock<Regex> = LazyLock::new(|| re(r"\S*[^\s/]/+[^\s/]\S*"));
static RE_ENV_NAME: LazyLock<Regex> = LazyLock::new(|| re(r"\b[A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+\b"));
static RE_BLOB: LazyLock<Regex> = LazyLock::new(|| re(r"[A-Za-z0-9+/=_-]{24,}"));

fn sub(re: &Regex, text: String, word: &str) -> String {
    re.replace_all(&text, NoExpand(word)).into_owned()
}

/// Thay URL, e-mail, `khóa = giá trị`, Bearer và các dạng token bằng từ tương ứng. Dùng chung
/// cho bộ lọc chữ và cho việc che đường dẫn trong báo cáo của cổng.
fn replace_secret_shapes(text: String, link: &str, email: &str, hidden: &str) -> String {
    let t = sub(&RE_URL, text, link);
    let t = sub(&RE_EMAIL, t, email);
    let t = sub(&RE_KEY_ASSIGN, t, hidden);
    let t = sub(&RE_BEARER, t, hidden);
    sub(&RE_SHAPES, t, hidden)
}

/// Bước 6 của `sanitize_text`: thay địa chỉ, đường dẫn, tên biến môi trường và chuỗi dài giống
/// khóa bằng từ chỉ loại (D-05).
fn replace_patterns(text: String) -> String {
    let t = replace_secret_shapes(text, WORD_LINK, WORD_EMAIL, WORD_HIDDEN);
    let t = sub(&RE_IPV4, t, WORD_ADDR);
    let t = sub(&RE_HOST, t, WORD_ADDR);
    let t = sub(&RE_WIN_PATH, t, WORD_PATH);
    let t = sub(&RE_HOME_PATH, t, WORD_PATH);
    let t = RE_ABS_PATH
        .replace_all(&t, |c: &regex::Captures| format!("{}{WORD_PATH}", &c[1]))
        .into_owned();
    let t = sub(&RE_REL_PATH, t, WORD_PATH);
    let t = sub(&RE_ENV_NAME, t, WORD_HIDDEN);
    RE_BLOB
        .replace_all(&t, |c: &regex::Captures| {
            let m = &c[0];
            let letter = m.chars().any(|ch| ch.is_ascii_alphabetic());
            let digit = m.chars().any(|ch| ch.is_ascii_digit());
            if letter && digit {
                WORD_HIDDEN.to_string()
            } else {
                m.to_string()
            }
        })
        .into_owned()
}

/// Cắt từ diff/khối mã (hoặc khối PEM) đầu tiên trở đi; trả phần còn lại và từ thay thế.
fn cut_code_or_pem(text: &str) -> (&str, Option<&'static str>) {
    let code_at = [RE_DIFF_LINE.find(text).map(|m| m.start()), text.find("```")]
        .into_iter()
        .flatten()
        .min();
    let pem_at = text.find("-----BEGIN");
    match (code_at, pem_at) {
        (Some(c), Some(p)) if p < c => (&text[..p], Some(WORD_HIDDEN)),
        (Some(c), _) => (&text[..c], Some(WORD_CODE)),
        (None, Some(p)) => (&text[..p], Some(WORD_HIDDEN)),
        (None, None) => (text, None),
    }
}

/// NFC → bỏ ký tự điều khiển (trừ xuống dòng)/zero-width/bidi/BOM → cắt từ diff, khối mã hoặc
/// PEM trở đi → mọi chuỗi khoảng trắng (cả xuống dòng) thành một khoảng trắng → thay giá trị
/// secret đã biết → thay URL, e-mail, khóa, token, địa chỉ, đường dẫn, tên biến môi trường và
/// chuỗi dài giống khóa bằng từ chỉ loại → chỉ giữ chữ, số, khoảng trắng, `ALLOWED_PUNCT` và `…`
/// → gộp khoảng trắng → cắt `max_chars`.
pub fn sanitize_text(raw: &str, max_chars: usize, secrets: &[String]) -> SafeText {
    let nfc: String = raw.nfc().collect();
    let visible: String = nfc
        .chars()
        .filter_map(|c| {
            if c == '\n' || c == '\r' {
                Some('\n')
            } else if c.is_whitespace() {
                Some(' ')
            } else if is_invisible(c) {
                None
            } else {
                Some(c)
            }
        })
        .collect();
    let (head, replaced) = cut_code_or_pem(&visible);
    let mut one_line = collapse_spaces(head);
    if let Some(word) = replaced {
        one_line.push(' ');
        one_line.push_str(word);
    }
    let redacted = crate::secrets::redact_str(&one_line, secrets);
    let replaced = replace_patterns(redacted);
    let allowed: String = replaced
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

    // Token-shaped strings are built at run time from a prefix and a body, so no complete
    // key-shaped literal sits in the repository.
    const BODY35: &str = "Zq9xLm4Vb7Tn2Kd8Rw5Ys3Hc6Pf1Jg0Ae7";
    const HEX40: &str = "3f9a1c07be52d8a4c6e0917fb3a25d84c1e6f07a";

    fn tok(prefix: &str, body: &str) -> String {
        format!("{prefix}{body}")
    }

    fn clean(raw: &str) -> String {
        sanitize_text(raw, 140, &[]).as_str().to_string()
    }

    #[test]
    fn plain_vietnamese_unchanged() {
        for t in [
            "Sửa lỗi đăng nhập trang quản trị",
            "Dựng video giới thiệu sản phẩm mẫu",
        ] {
            assert_eq!(clean(t), t);
        }
    }

    #[test]
    fn markup_is_neutralized() {
        let inputs = [
            "![[ghi chu rieng]]",
            "[[x]]",
            "#tag",
            "`a` `b`",
            "---",
            "<script>alert(1)</script>",
            "%%c%%",
            "$$x$$",
            "{{t}}",
            "^block",
            "> trich",
            "| a | b |",
            "== x ==",
            "~~x~~",
        ];
        for raw in inputs {
            let out = clean(&format!("trước {raw} sau"));
            for bad in [
                "[[", "]]", "![", "#", "`", "<", ">", "%", "$", "{", "}", "^", "|", "=", "~",
            ] {
                assert!(!out.contains(bad), "{bad:?} còn trong {out:?} (từ {raw:?})");
            }
        }
    }

    #[test]
    fn paths_urls_hosts_emails_redacted() {
        let out = clean("xem /home/someone/app/config/database.php nhé");
        assert!(out.contains("(đường dẫn)"), "{out}");
        assert!(!out.contains("home") && !out.contains("database"), "{out}");

        let out = clean("mở https://intranet.example.local/admin?token=abc123 xem");
        assert!(out.contains("(liên kết)"), "{out}");
        assert!(
            !out.contains("intranet") && !out.contains("abc123"),
            "{out}"
        );

        for raw in [
            r"C:\Users\x\a.txt",
            "~/.ssh/id_rsa",
            "task/sua-loi-dang-nhap",
        ] {
            let out = clean(&format!("tệp {raw} đó"));
            assert!(out.contains("(đường dẫn)"), "{raw} -> {out}");
            assert!(!out.contains("Users") && !out.contains("ssh") && !out.contains("sua-loi"));
        }
        for raw in [
            "10.0.0.5:8080",
            "localhost:3000",
            "db.internal",
            "api.example.vn",
        ] {
            let out = clean(&format!("chạy {raw} rồi"));
            assert!(out.contains("(địa chỉ)"), "{raw} -> {out}");
            assert!(!out.contains("8080") && !out.contains("3000") && !out.contains("internal"));
            assert!(!out.contains("example"), "{out}");
        }
        let out = clean("gửi ai@example.vn nhé");
        assert!(out.contains("(email)"), "{out}");
        assert!(!out.contains("example"), "{out}");
    }

    #[test]
    fn secret_shapes_redacted() {
        let jwt_head = "hbGciOiJIUzI1NiJ9";
        let jwt_mid = "eyJzdWIiOiIxMjM0NTY";
        let jwt_sig = "c2lnbmF0dXJlMTIz";
        let jwt = format!("{}.{}.{}", tok("eyJ", jwt_head), jwt_mid, jwt_sig);
        let telegram = format!("123456789:{BODY35}");
        let cases: Vec<(String, Vec<&str>)> = vec![
            (tok("sk-ant-", BODY35), vec![BODY35]),
            (tok("sk-", BODY35), vec![BODY35]),
            (tok("ghp_", BODY35), vec![BODY35]),
            (tok("github_pat_", BODY35), vec![BODY35]),
            (tok("glpat-", BODY35), vec![BODY35]),
            (
                tok("xoxb-", "123456789012-abcdefghij"),
                vec!["123456789012"],
            ),
            (tok("AKIA", "Q7M2X9B4C8D1E5F6"), vec!["Q7M2X9B4C8D1E5F6"]),
            (tok("AIza", BODY35), vec![BODY35]),
            (jwt, vec![jwt_head, jwt_mid, jwt_sig]),
            (telegram, vec![BODY35, "123456789"]),
            (
                "DEEPSEEK_API_KEY=abcd1234efgh5678".to_string(),
                vec!["abcd1234efgh5678", "DEEPSEEK_API_KEY"],
            ),
            ("password: hunter2hunter2".to_string(), vec!["hunter2"]),
            (
                "Bearer abcdefghijklmnop1234".to_string(),
                vec!["abcdefghijklmnop1234"],
            ),
            (HEX40.to_string(), vec![HEX40]),
        ];
        for (raw, banned) in cases {
            let out = clean(&format!("dùng {raw} nhé"));
            for b in banned {
                assert!(!out.contains(b), "{b} còn trong {out:?} (từ {raw})");
            }
            assert!(out.starts_with("dùng ") && out.ends_with(" nhé"), "{out}");
        }
    }

    #[test]
    fn diff_and_code_are_cut() {
        let diff = "Sửa lỗi như sau\ndiff --git a/x.php b/x.php\n@@ -1 +1 @@\n-cu\n+moi";
        assert_eq!(clean(diff), "Sửa lỗi như sau (đã bỏ phần mã)");
        let indented = "Sửa lỗi như sau\r\n  index 3f9a1c0..b3a25d8 100644\n+x";
        assert_eq!(clean(indented), "Sửa lỗi như sau (đã bỏ phần mã)");
        let fence = "Sửa lỗi như sau\n```php\necho 1;\n```\nxong";
        assert_eq!(clean(fence), "Sửa lỗi như sau (đã bỏ phần mã)");
        let hunk = "Đổi\n--- a/x\n+++ b/x\n@@ -1 +1 @@";
        assert_eq!(clean(hunk), "Đổi (đã bỏ phần mã)");
    }

    #[test]
    fn redact_before_cut() {
        let key = tok("sk-ant-", "Zq9xLm4Vb7Tn2Kd8Rw5Ys3Hc6Pf1Jg0Ae7");
        let mut raw = "từ ".repeat(43); // 129 chars + 1 space below
        raw.push(' ');
        assert_eq!(raw.chars().count(), 130);
        raw.push_str(&key);
        raw.push_str(&" nội dung dài ".repeat(280));
        assert!(raw.chars().count() > 4000);
        let out = sanitize_text(&raw, 140, &[]);
        assert!(out.as_str().chars().count() <= 140, "{out}");
        let body: Vec<char> = key["sk-ant-".len()..].chars().collect();
        for w in body.windows(6) {
            let sub: String = w.iter().collect();
            assert!(!out.as_str().contains(&sub), "{sub} còn trong {out}");
        }
    }

    #[test]
    fn pem_block_removed() {
        let raw = "Dùng khóa này\n-----BEGIN RSA PRIVATE KEY-----\nMIIEabc\nxyz\n-----END RSA PRIVATE KEY-----";
        assert_eq!(clean(raw), "Dùng khóa này (đã ẩn)");
    }

    #[test]
    fn zero_width_and_bidi_removed() {
        assert_eq!(clean("chữ\u{200B}mẫu\u{202E}nè"), "chữmẫunè");
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
