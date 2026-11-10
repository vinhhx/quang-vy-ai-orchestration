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
/// `[ ] ! # ` < > | * ~ = % $ ^ { } @ \`, thành khoảng trắng. Dấu `/` chỉ được giữ khi đứng giữa
/// hai ký tự chữ/số và từ chứa nó không phải đường dẫn (`CI/CD`, `15/01`); xem `is_path_token`.
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
/// Tên máy chủ (WR-05). Luật: một chuỗi là máy chủ khi có ít nhất hai nhãn cách nhau bằng dấu
/// chấm, nhãn cuối chỉ gồm chữ cái và dài từ 2 ký tự (TLD nào cũng được, không còn danh sách ngắn),
/// kèm `:cổng` tùy chọn. Không coi là máy chủ: số phiên bản (`v1.2`, nhãn cuối là số), nhãn cuối
/// một chữ cái (`e.g`, `i.e`, `a.b`) và tên tệp có đúng một dấu chấm với đuôi quen thuộc
/// (`node.js`, `report.pdf`, xem `FILE_EXTENSIONS`; chỉ khi không có cổng). Dương tính giả đã biết:
/// `ASP.NET`, `Dr.Smith`, hai câu dính nhau thiếu khoảng trắng sau dấu chấm (`xong.Sau`), tên tệp
/// nhiều dấu chấm (`jquery.min.js`). Âm tính giả đã biết: tên miền một dấu chấm có TLD trùng
/// đuôi tệp (`intranet.sh`, `corp.rs`). Che nhầm an toàn hơn để lọt máy chủ nội bộ (D-03).
static RE_HOST: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)\b(?:[a-z0-9-]+\.)+[a-z]{2,}(?::\d{1,5})?\b"));
/// Đuôi tệp thường gặp: tên chỉ có một dấu chấm với các đuôi này không bị coi là máy chủ.
const FILE_EXTENSIONS: &[&str] = &[
    "js", "ts", "tsx", "jsx", "mjs", "py", "php", "rs", "go", "rb", "java", "kt", "md", "txt",
    "json", "yml", "yaml", "toml", "ini", "cfg", "conf", "html", "css", "svg", "pdf", "png", "jpg",
    "jpeg", "gif", "ico", "csv", "sql", "log", "sh", "lock", "xml", "doc", "docx", "xls", "xlsx",
    "ppt", "pptx", "gz", "tmp", "bak",
];

/// `true` khi khớp của `RE_HOST` là một máy chủ cần che (xem chú thích của `RE_HOST`).
fn is_host_match(m: &str) -> bool {
    if m.contains(':') {
        return true;
    }
    let lower = m.to_lowercase();
    let mut labels = lower.split('.');
    let first = labels.next().unwrap_or("");
    let rest: Vec<&str> = labels.collect();
    let single_dot = rest.len() == 1;
    let last = rest.last().copied().unwrap_or(first);
    !(single_dot && FILE_EXTENSIONS.contains(&last))
}

static RE_WIN_PATH: LazyLock<Regex> = LazyLock::new(|| re(r"\b[A-Za-z]:\\\S*"));
static RE_HOME_PATH: LazyLock<Regex> = LazyLock::new(|| re(r"~/\S*"));
static RE_ABS_PATH: LazyLock<Regex> = LazyLock::new(|| re(r#"(^|[\s(\["'=,;])/\S+"#));
/// Mọi từ có dấu `/` ở giữa; `is_path_token` quyết định đó có phải đường dẫn (WR-06).
static RE_REL_PATH: LazyLock<Regex> = LazyLock::new(|| re(r"\S*[^\s/]/+[^\s/]\S*"));

/// Một vế của cặp một dấu `/` còn được giữ: chữ cái tối đa 6 ký tự (`CI`, `CD`, `và`, `hoặc`) hoặc
/// số tối đa 4 chữ số (`24`, `7`, `2026`).
fn is_short_side(s: &str) -> bool {
    let n = s.chars().count();
    n > 0
        && ((n <= 6 && s.chars().all(char::is_alphabetic))
            || (n <= 4 && s.chars().all(|c| c.is_ascii_digit())))
}

/// `true` khi từ có dấu `/` là đường dẫn cần che (WR-06). Không phải đường dẫn (giữ nguyên): đúng
/// một dấu `/` giữa hai vế ngắn (`CI/CD`, `I/O`, `A/B`, `và/hoặc`, `24/7`, `50/50`, `15/01`), hoặc
/// ngày kiểu `15/01/2026` (hai dấu, ba số). Còn lại là đường dẫn: từ hai dấu `/` trở lên, có đuôi
/// tệp (`a/b.php`), có vế dài hoặc lẫn ký tự khác (`task/sua-loi-dang-nhap`, `./a`, `C:/x`).
/// Đường bắt đầu bằng `/`, `~` hay ổ đĩa đã bị các luật trước che. Dương tính giả/âm tính giả đã
/// biết: `src/app` (hai vế ngắn) được giữ; `v1/v2` (lẫn chữ và số) bị che.
fn is_path_token(token: &str) -> bool {
    let core = token
        .trim_start_matches(['(', '[', '{', '"', '\'', '`'])
        .trim_end_matches([')', ']', '}', '"', '\'', '`', '.', ',', ';', ':', '!', '?']);
    let slashes = core.matches('/').count();
    match slashes {
        1 => core
            .split_once('/')
            .is_none_or(|(a, b)| !(is_short_side(a) && is_short_side(b))),
        2 => !core
            .split('/')
            .all(|p| !p.is_empty() && p.len() <= 4 && p.chars().all(|c| c.is_ascii_digit())),
        _ => true,
    }
}
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
    let t = RE_HOST
        .replace_all(&t, |c: &regex::Captures| {
            if is_host_match(&c[0]) {
                WORD_ADDR.to_string()
            } else {
                c[0].to_string()
            }
        })
        .into_owned();
    let t = sub(&RE_WIN_PATH, t, WORD_PATH);
    let t = sub(&RE_HOME_PATH, t, WORD_PATH);
    let t = RE_ABS_PATH
        .replace_all(&t, |c: &regex::Captures| format!("{}{WORD_PATH}", &c[1]))
        .into_owned();
    let t = RE_REL_PATH
        .replace_all(&t, |c: &regex::Captures| {
            if is_path_token(&c[0]) {
                WORD_PATH.to_string()
            } else {
                c[0].to_string()
            }
        })
        .into_owned();
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
    // `/` chỉ còn ở giữa hai ký tự chữ/số (`CI/CD`, `15/01`): các từ như vậy đã qua `is_path_token`.
    // Dấu `/` đầu, cuối hay đứng đôi không bao giờ qua được.
    let chars: Vec<char> = replaced.chars().collect();
    let allowed: String = chars
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let inner_slash = c == '/'
                && i > 0
                && chars[i - 1].is_alphanumeric()
                && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric());
            if c.is_alphanumeric()
                || c == ' '
                || c == ELLIPSIS
                || ALLOWED_PUNCT.contains(c)
                || inner_slash
            {
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

/// Luật của cổng, theo thứ tự báo cáo (sau `known-value`). Tệp vault không bao giờ chứa URL hay
/// đường dẫn tuyệt đối do cách dựng, nên các luật này có tín hiệu cao; luật chuỗi dài và đường
/// dẫn tương đối của bộ lọc chữ không có ở đây (slug và wikilink sẽ vướng).
static GATE_RULES: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    vec![
        ("url", re(r"(?i)[a-z][a-z0-9+.-]*://")),
        (
            "abs-path",
            re(r"/home/|/root/|/mnt/|/Users/|~/|\b[A-Za-z]:\\"),
        ),
        ("pem", re("-----BEGIN")),
        (
            "token-shape",
            re(&format!(
                r"{TOKEN_SHAPES}|(?i:\bbearer\s+[A-Za-z0-9._~+/=-]{{16,}})"
            )),
        ),
        ("jwt", re(JWT_SHAPE)),
        ("telegram-token", re(TELEGRAM_SHAPE)),
        ("key-assignment", re(KEY_ASSIGN)),
        ("email", re(r"[\w.+-]+@[\w-]+\.[\w.-]+")),
    ]
});

/// Đường dẫn dùng trong báo cáo: giá trị secret đã biết và mọi chuỗi giống secret bị che.
fn mask_path(path: &str, secrets: &[String]) -> String {
    let hidden = crate::secrets::REDACTED;
    replace_secret_shapes(
        crate::secrets::redact_str(path, secrets),
        hidden,
        hidden,
        hidden,
    )
}

/// Cổng cuối (JRN-03): quét nội dung và đường dẫn mọi tệp đã dựng. Có kết quả = không ghi gì.
/// Mỗi (tệp, luật) một kết quả; kết quả chỉ có đường dẫn đã che và tên luật, không bao giờ có
/// đoạn khớp.
pub fn scan_generated(files: &BTreeMap<String, String>, secrets: &[String]) -> Vec<SecretHit> {
    let mut hits = Vec::new();
    for (path, body) in files {
        let mut rules: Vec<&'static str> = Vec::new();
        if crate::secrets::contains_secret(body.as_bytes(), secrets)
            || crate::secrets::contains_secret(path.as_bytes(), secrets)
        {
            rules.push("known-value");
        }
        for (name, rule) in GATE_RULES.iter() {
            if rule.is_match(body) || rule.is_match(path) {
                rules.push(name);
            }
        }
        if rules.is_empty() {
            continue;
        }
        let shown = mask_path(path, secrets);
        for rule in rules {
            hits.push(SecretHit {
                path: shown.clone(),
                rule: rule.to_string(),
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

    fn gate(files: &[(&str, String)], secrets: &[String]) -> Vec<(String, String)> {
        let map: BTreeMap<String, String> = files
            .iter()
            .map(|(p, b)| (p.to_string(), b.clone()))
            .collect();
        scan_generated(&map, secrets)
            .into_iter()
            .map(|h| (h.path, h.rule))
            .collect()
    }

    #[test]
    fn gate_rules_fire() {
        let jwt = format!(
            "{}.{}.{}",
            tok("eyJ", "hbGciOiJIUzI1NiJ9"),
            "eyJzdWIiOiIxMjM0NTY",
            "c2lnbmF0dXJlMTIz"
        );
        let cases: Vec<(&str, String)> = vec![
            ("url", "https://x.example/a".to_string()),
            ("abs-path", "/home/u/a".to_string()),
            ("pem", "-----BEGIN KEY-----".to_string()),
            ("token-shape", tok("ghp_", BODY35)),
            ("jwt", jwt),
            ("telegram-token", format!("123456789:{BODY35}")),
            ("key-assignment", "secret = abcdefgh12".to_string()),
            ("email", "a@b.vn".to_string()),
        ];
        for (rule, text) in &cases {
            let path = format!("Nhật ký/{rule}.md");
            let hits = gate(&[(path.as_str(), format!("trước {text} sau\n"))], &[]);
            assert_eq!(hits, vec![(path.clone(), rule.to_string())], "{rule}");
        }

        // the same rules apply to the path
        let path = "Dự án/a@b.vn.md";
        let hits = gate(&[(path, "sạch\n".to_string())], &[]);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].1, "email");

        // a day file in the tracer format and a long project path are clean
        let day = "---\ndate: 2026-01-15\ntasks: 1\ntags: [nhat-ky]\n---\n\
<!-- Tệp sinh tự động từ agent-brain — không sửa tay, lần xuất sau sẽ ghi đè. -->\n\
# Thứ Năm 15/01/2026 — 1 việc (1 xong)\n\n## Video (1)\n\
> [!success]- ✓ du-an-a — Dựng video giới thiệu sản phẩm mẫu\n\
> **Trạng thái:** xong · 34 phút\n\
> **Dự án:** [[Dự án/du-an-a|du-an-a]] · **Nhóm:** Video · **Số bước chạy:** 3 · **Vòng:** 1\n\
> **Vai đã chạy:** Lead, Dev, QA\n>\n> Dựng video giới thiệu sản phẩm mẫu thêm phụ đề\n\n---\n\
Tháng: [[2026-01]]\n";
        let clean = gate(
            &[
                ("Nhật ký/2026/01/2026-01-15.md", day.to_string()),
                (
                    "Dự án/automation-test-e2e-very-long-name-1.md",
                    "sạch\n".to_string(),
                ),
            ],
            &[],
        );
        assert!(clean.is_empty(), "{clean:?}");
    }

    #[test]
    fn gate_report_never_holds_the_value() {
        let gh = tok("ghp_", BODY35);
        let hits = gate(&[(&format!("Dự án/{gh}.md"), "sạch\n".to_string())], &[]);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].1, "token-shape");
        assert!(!hits[0].0.contains(BODY35), "{hits:?}");

        let known = "khoa-bi-mat-ba-24680".to_string();
        let hits = gate(
            &[("Today.md", format!("có {known} và https://x.example\n"))],
            std::slice::from_ref(&known),
        );
        let rules: Vec<&str> = hits.iter().map(|h| h.1.as_str()).collect();
        assert_eq!(rules, vec!["known-value", "url"]);
    }

    #[test]
    fn hosts_of_any_alphabetic_tld_are_redacted() {
        for host in [
            "git.example.co",
            "jira.example.ai",
            "wiki.example.me",
            "intranet.example.tech",
            "host.corp.site",
            "x.example.uk",
            "x.example.de",
            "x.example.edu.vn",
            "wiki.example.online:8443",
            "Intranet.Example.CO",
        ] {
            let out = clean(&format!("mở {host} xem"));
            assert_eq!(out, "mở (địa chỉ) xem", "{host} -> {out}");
        }
    }

    #[test]
    fn non_hosts_survive() {
        for raw in [
            "viết bằng node.js và Vue.js",
            "bản v1.2 và 2.5.1",
            "ví dụ e.g. như vậy, i.e. thế",
            "gửi report.pdf và ảnh logo.png",
            "sửa a.b cho đúng",
            "sửa config.php và main.rs",
            "giá 3.5 triệu",
        ] {
            assert_eq!(clean(raw), raw, "{raw}");
        }
        // a port makes even a file-looking name a host
        assert_eq!(clean("chạy node.js:3000 rồi"), "chạy (địa chỉ) rồi");
    }

    #[test]
    fn single_slash_text_survives() {
        for raw in [
            "Họp ngày 15/01 về dự án",
            "Trực 24/7 cho khách",
            "Chia 50/50 chi phí",
            "Cấu hình CI/CD mới",
            "Kiểm tra I/O đĩa",
            "Chạy thử A/B cho trang chủ",
            "Chọn email và/hoặc điện thoại",
            "Hạn 15/01/2026 chốt",
            "Làm xong (CI/CD), rồi nghỉ.",
        ] {
            assert_eq!(clean(raw), raw, "{raw}");
        }
    }

    #[test]
    fn real_paths_are_still_redacted() {
        for (raw, banned) in [
            ("xem /etc/x nhé", "etc"),
            ("xem ~/a nhé", "~"),
            ("xem ./a/b nhé", "./"),
            ("xem ../a nhé", ".."),
            (r"xem C:\x\y nhé", "C:"),
            ("xem src/app/config.php nhé", "config"),
            ("xem a/b.php nhé", "b.php"),
            ("xem task/sua-loi-dang-nhap nhé", "sua-loi"),
            ("xem (src/app/config.php) nhé", "config"),
            ("xem a//b nhé", "a//b"),
            ("xem 1/2/3/4 nhé", "1/2"),
        ] {
            let out = clean(raw);
            assert!(out.contains("(đường dẫn)"), "{raw} -> {out}");
            assert!(!out.contains(banned), "{raw} -> {out}");
            assert!(!out.contains('/'), "{raw} -> {out}");
        }
    }

    #[test]
    fn date_survives_beside_path_and_url() {
        let out = clean(
            "ngày 15/01 sửa /home/someone/app/config.php rồi mở https://intranet.example.co/x?token=abc \
             và git.example.co, hạn 24/7",
        );
        assert_eq!(
            out,
            "ngày 15/01 sửa (đường dẫn) rồi mở (liên kết) và (địa chỉ), hạn 24/7"
        );
    }

    #[test]
    fn slash_never_survives_at_the_edge_or_doubled() {
        for raw in ["a/ b", "a /b", "src/", "a // b", "//x", "x//", "/"] {
            let out = clean(raw);
            assert!(!out.contains('/'), "{raw} -> {out}");
        }
        assert_eq!(clean("a/b"), "a/b");
    }

    #[test]
    fn relaxed_rules_keep_secret_and_code_handling() {
        // known secret still replaced first, even when it sits inside a slash/host-looking word
        let secret = "khoa-bi-mat-12345".to_string();
        let out = sanitize_text(
            &format!("15/01 dùng {secret}/x và host.example.me"),
            140,
            &[secret.clone()],
        );
        assert!(!out.as_str().contains("khoa-bi-mat"), "{out}");
        assert!(out.as_str().starts_with("15/01 dùng "), "{out}");
        assert!(!out.as_str().contains("example"), "{out}");
        // diff / fence / PEM cut still happens before the allow-list, one line, length cut last
        assert_eq!(
            clean("Sửa CI/CD\ndiff --git a/x.php b/x.php\n+x"),
            "Sửa CI/CD (đã bỏ phần mã)"
        );
        assert_eq!(clean("Họp 15/01\n```\nx\n```"), "Họp 15/01 (đã bỏ phần mã)");
        assert_eq!(
            clean("Mã 24/7\n-----BEGIN KEY-----\nabc"),
            "Mã 24/7 (đã ẩn)"
        );
        let long = "CI/CD ".repeat(60);
        let cut = sanitize_text(&long, 40, &[]);
        assert!(cut.as_str().chars().count() <= 40, "{cut}");
        assert!(cut.as_str().ends_with('…'), "{cut}");
        // markup next to a slash word is still neutralized
        let out = clean("[[CI/CD]] #tag `A/B` <b>I/O</b>");
        for bad in ["[", "]", "#", "`", "<", ">"] {
            assert!(!out.contains(bad), "{bad} in {out}");
        }
        // a word still wrapped in an HTML tag has two slashes and is treated as a path (safe side)
        assert_eq!(out, "CI/CD tag A/B (đường dẫn)");
    }

    #[test]
    fn gate_accepts_slash_words_in_day_text() {
        let body =
            "# Thứ Năm 15/01/2026\n> [!success]- ✓ du-an-a — Cấu hình CI/CD ngày 15/01 24/7\n";
        let hits = gate(&[("Nhật ký/2026/01/2026-01-15.md", body.to_string())], &[]);
        assert!(hits.is_empty(), "{hits:?}");
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
