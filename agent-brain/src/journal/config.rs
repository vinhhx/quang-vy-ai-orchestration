//! Mục `journal` của config.json (Phase 7, D-07, D-08, D-11, D-18). Thiếu mục = tắt, mọi khóa
//! có mặc định; `journal_group` của từng dự án đọc lỏng từ projects_map.json (D-19).

use crate::error::{BrainError, BrainResult};
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Một nhóm của nhật ký (`journal.groups`); nhóm đầu tiên là nhóm mặc định (D-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupDef {
    pub id: String,
    pub label: String,
}

/// Múi giờ tính ngày (D-12): giờ máy, hoặc độ lệch cố định khi có `utc_offset_minutes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalTz {
    Local,
    Fixed(chrono::FixedOffset),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalConfig {
    pub enabled: bool,
    pub repo_url: Option<String>,
    pub branch: String,
    pub clone_dir: PathBuf,
    pub interval_minutes: u32,
    pub digest_enabled: bool,
    pub digest_time: (u32, u32),
    pub push_fail_notify_after: u32,
    pub title_max_chars: usize,
    pub callout_title_chars: usize,
    pub git_timeout_seconds: u64,
    pub ssh_connect_timeout_seconds: u64,
    pub author_name: String,
    pub author_email: String,
    pub groups: Vec<GroupDef>,
    pub utc_offset_minutes: Option<i32>,
    pub web_base_url: Option<String>,
    pub project_page_months: u32,
    pub search_max_results: usize,
}

fn default_groups() -> Vec<GroupDef> {
    [
        ("cong-viec", "Công việc"),
        ("video", "Video"),
        ("ho-tro", "Hỗ trợ bộ phận khác"),
    ]
    .iter()
    .map(|(id, label)| GroupDef {
        id: id.to_string(),
        label: label.to_string(),
    })
    .collect()
}

impl Default for JournalConfig {
    fn default() -> Self {
        JournalConfig {
            enabled: false,
            repo_url: None,
            branch: "main".to_string(),
            clone_dir: crate::config::data_dir().join("journal"),
            interval_minutes: 30,
            digest_enabled: true,
            digest_time: (18, 0),
            push_fail_notify_after: 3,
            title_max_chars: 140,
            callout_title_chars: 60,
            git_timeout_seconds: 60,
            ssh_connect_timeout_seconds: 15,
            author_name: "agent-brain journal".to_string(),
            author_email: "journal@agent-brain.invalid".to_string(),
            groups: default_groups(),
            utc_offset_minutes: None,
            web_base_url: None,
            project_page_months: 3,
            search_max_results: 8,
        }
    }
}

impl JournalConfig {
    pub fn tz(&self) -> JournalTz {
        match self
            .utc_offset_minutes
            .and_then(|m| chrono::FixedOffset::east_opt(m * 60))
        {
            Some(off) => JournalTz::Fixed(off),
            None => JournalTz::Local,
        }
    }

    /// Nhóm mặc định = nhóm đầu tiên (`parse` không cho danh sách rỗng).
    pub fn default_group(&self) -> &GroupDef {
        &self.groups[0]
    }
}

fn err(msg: String) -> BrainError {
    BrainError::error(msg)
}

fn get_bool(o: &serde_json::Map<String, Value>, key: &str) -> BrainResult<Option<bool>> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_bool()
            .map(Some)
            .ok_or_else(|| err(format!("Cấu hình journal.{key} phải là boolean"))),
    }
}

fn get_str<'a>(o: &'a serde_json::Map<String, Value>, key: &str) -> BrainResult<Option<&'a str>> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_str()
            .map(Some)
            .ok_or_else(|| err(format!("Cấu hình journal.{key} phải là chuỗi"))),
    }
}

fn range_err(key: &str, lo: i64, hi: i64) -> BrainError {
    err(format!(
        "Cấu hình journal.{key} phải là số nguyên từ {lo} đến {hi}"
    ))
}

/// Số nguyên trong `[lo, hi]`; sai kiểu hoặc ngoài khoảng → lỗi nêu tên khóa và khoảng.
fn get_range(
    o: &serde_json::Map<String, Value>,
    key: &str,
    lo: i64,
    hi: i64,
) -> BrainResult<Option<i64>> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => match v.as_i64() {
            Some(n) if (lo..=hi).contains(&n) => Ok(Some(n)),
            _ => Err(range_err(key, lo, hi)),
        },
    }
}

/// `HH:MM`, 00:00 đến 23:59, đúng hai chữ số mỗi phần.
fn parse_digest_time(s: &str) -> Option<(u32, u32)> {
    let (h, m) = s.split_once(':')?;
    if h.len() != 2 || m.len() != 2 || !(h.chars().chain(m.chars())).all(|c| c.is_ascii_digit()) {
        return None;
    }
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    (h < 24 && m < 60).then_some((h, m))
}

fn has_space_or_control(s: &str) -> bool {
    s.chars().any(|c| c.is_whitespace() || c.is_control())
}

fn scp_like(s: &str) -> bool {
    let Some((user, rest)) = s.split_once('@') else {
        return false;
    };
    let Some((host, path)) = rest.split_once(':') else {
        return false;
    };
    !user.is_empty()
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        && !path.is_empty()
        && !path.starts_with('-')
}

/// `git@host:path`, `ssh://`, `https://`, `file://` hoặc đường dẫn tuyệt đối; không rỗng,
/// không khoảng trắng/ký tự điều khiển, không bắt đầu bằng `-`, không chứa `::` (T-07-06).
fn valid_repo_url(s: &str) -> bool {
    if s.is_empty() || has_space_or_control(s) || s.starts_with('-') || s.contains("::") {
        return false;
    }
    for scheme in ["ssh://", "https://", "file://"] {
        if let Some(rest) = s.strip_prefix(scheme) {
            return !rest.is_empty();
        }
    }
    if s.contains("://") {
        return false;
    }
    s.starts_with('/') || scp_like(s)
}

/// `[A-Za-z0-9._/-]{1,100}`, không bắt đầu bằng `-` hay `/`, không chứa `..` hay `//`,
/// không kết thúc bằng `/` hay `.lock`.
fn valid_branch(s: &str) -> bool {
    (1..=100).contains(&s.len())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
        && !s.starts_with('-')
        && !s.starts_with('/')
        && !s.ends_with('/')
        && !s.ends_with(".lock")
        && !s.contains("..")
        && !s.contains("//")
}

fn valid_author(s: &str) -> bool {
    !s.trim().is_empty() && !s.contains('<') && !s.contains('>') && !s.chars().any(char::is_control)
}

const MAX_GROUPS: usize = 10;
const MAX_GROUP_LABEL_CHARS: usize = 60;

fn parse_groups(v: &Value) -> BrainResult<Vec<GroupDef>> {
    let shape = || {
        err(format!(
            "Cấu hình journal.groups phải là danh sách 1–{MAX_GROUPS} nhóm {{\"id\", \"label\"}}"
        ))
    };
    let arr = v.as_array().ok_or_else(shape)?;
    if arr.is_empty() || arr.len() > MAX_GROUPS {
        return Err(shape());
    }
    let mut out: Vec<GroupDef> = Vec::new();
    for g in arr {
        let id = g.get("id").and_then(|x| x.as_str()).ok_or_else(shape)?;
        let label = g.get("label").and_then(|x| x.as_str()).ok_or_else(shape)?;
        let id_ok = (1..=40).contains(&id.len())
            && id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !id_ok {
            return Err(err(format!(
                "Cấu hình journal.groups: id '{id}' phải gồm chữ thường a-z, số, dấu - (1–40 ký tự)"
            )));
        }
        if out.iter().any(|g| g.id == id) {
            return Err(err(format!("Cấu hình journal.groups: id '{id}' bị trùng")));
        }
        let label_ok = !label.trim().is_empty()
            && label.chars().count() <= MAX_GROUP_LABEL_CHARS
            && !label
                .chars()
                .any(|c| c.is_control() || matches!(c, '[' | ']' | '|' | '#'));
        if !label_ok {
            return Err(err(format!(
                "Cấu hình journal.groups: nhãn của '{id}' phải khác rỗng, tối đa {MAX_GROUP_LABEL_CHARS} ký tự, không chứa [ ] | # hay xuống dòng"
            )));
        }
        out.push(GroupDef {
            id: id.to_string(),
            label: label.trim().to_string(),
        });
    }
    Ok(out)
}

/// Đọc mục `journal` (thiếu hoặc `null` = mặc định, tắt). Mọi khóa đều tùy chọn; giá trị sai
/// kiểu hoặc ngoài khoảng là lỗi nêu tên khóa (JRN-07).
pub fn parse(v: Option<&Value>) -> BrainResult<JournalConfig> {
    let mut cfg = JournalConfig::default();
    let o = match v {
        None | Some(Value::Null) => return Ok(cfg),
        Some(Value::Object(o)) => o,
        Some(_) => return Err(err("Cấu hình journal phải là object".to_string())),
    };
    if let Some(b) = get_bool(o, "enabled")? {
        cfg.enabled = b;
    }
    if let Some(s) = get_str(o, "repo_url")? {
        if !valid_repo_url(s) {
            return Err(err(
                "Cấu hình journal.repo_url không hợp lệ (dùng git@host:đường-dẫn, ssh://, https://, file:// hoặc đường dẫn tuyệt đối; không khoảng trắng, không bắt đầu bằng -, không chứa ::)".to_string(),
            ));
        }
        cfg.repo_url = Some(s.to_string());
    }
    if let Some(s) = get_str(o, "branch")? {
        if !valid_branch(s) {
            return Err(err(
                "Cấu hình journal.branch phải gồm 1–100 ký tự A-Z a-z 0-9 . _ / -, không bắt đầu bằng - hay /, không chứa .. và không kết thúc bằng .lock".to_string(),
            ));
        }
        cfg.branch = s.to_string();
    }
    if let Some(s) = get_str(o, "clone_dir")? {
        if s.trim().is_empty() || s.chars().any(char::is_control) {
            return Err(err(
                "Cấu hình journal.clone_dir phải là đường dẫn khác rỗng".to_string(),
            ));
        }
        cfg.clone_dir = crate::config::expand_tilde(s, &crate::config::home_dir());
    }
    if let Some(n) = get_range(o, "interval_minutes", 1, 1440)? {
        cfg.interval_minutes = n as u32;
    }
    if let Some(b) = get_bool(o, "digest_enabled")? {
        cfg.digest_enabled = b;
    }
    if let Some(s) = get_str(o, "digest_time")? {
        cfg.digest_time = parse_digest_time(s).ok_or_else(|| {
            err("Cấu hình journal.digest_time phải có dạng HH:MM (00:00 đến 23:59)".to_string())
        })?;
    }
    if let Some(n) = get_range(o, "push_fail_notify_after", 1, 100)? {
        cfg.push_fail_notify_after = n as u32;
    }
    if let Some(n) = get_range(o, "title_max_chars", 40, 400)? {
        cfg.title_max_chars = n as usize;
    }
    if let Some(n) = get_range(o, "callout_title_chars", 20, 200)? {
        cfg.callout_title_chars = n as usize;
    }
    if let Some(n) = get_range(o, "git_timeout_seconds", 10, 600)? {
        cfg.git_timeout_seconds = n as u64;
    }
    if let Some(n) = get_range(o, "ssh_connect_timeout_seconds", 5, 120)? {
        cfg.ssh_connect_timeout_seconds = n as u64;
    }
    if let Some(s) = get_str(o, "author_name")? {
        if !valid_author(s) {
            return Err(err(
                "Cấu hình journal.author_name phải khác rỗng, không chứa < > hay xuống dòng"
                    .to_string(),
            ));
        }
        cfg.author_name = s.to_string();
    }
    if let Some(s) = get_str(o, "author_email")? {
        if !valid_author(s) {
            return Err(err(
                "Cấu hình journal.author_email phải khác rỗng, không chứa < > hay xuống dòng"
                    .to_string(),
            ));
        }
        cfg.author_email = s.to_string();
    }
    if let Some(g) = o.get("groups").filter(|g| !g.is_null()) {
        cfg.groups = parse_groups(g)?;
    }
    if let Some(n) = get_range(o, "utc_offset_minutes", -720, 840)? {
        cfg.utc_offset_minutes = Some(n as i32);
    }
    if let Some(s) = get_str(o, "web_base_url")? {
        let ok =
            s.strip_prefix("https://").is_some_and(|r| !r.is_empty()) && !has_space_or_control(s);
        if !ok {
            return Err(err(
                "Cấu hình journal.web_base_url phải bắt đầu bằng https:// và không có khoảng trắng"
                    .to_string(),
            ));
        }
        cfg.web_base_url = Some(s.trim_end_matches('/').to_string());
    }
    if let Some(n) = get_range(o, "project_page_months", 1, 120)? {
        cfg.project_page_months = n as u32;
    }
    if let Some(n) = get_range(o, "search_max_results", 1, 20)? {
        cfg.search_max_results = n as usize;
    }
    Ok(cfg)
}

/// Cảnh báo `journal_group` lạ đúng một lần cho mỗi (dự án, id) trong một tiến trình.
fn warn_unknown_group_once(slug: &str, id: &str, label: &str) {
    static SEEN: OnceLock<Mutex<HashSet<(String, String)>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let first = match seen.lock() {
        Ok(mut set) => set.insert((slug.to_string(), id.to_string())),
        Err(_) => false,
    };
    if first {
        crate::term::emit(&format!(
            "Cảnh báo: journal_group '{id}' của dự án {slug} không có trong journal.groups — dùng nhóm {label}"
        ));
    }
}

/// Nhóm của dự án `project_slug` theo khóa `journal_group` trong projects_map.json (D-07, D-19).
/// Đọc lỏng như `preflight::read_lenient`: không có khóa → nhóm đầu tiên, im lặng; id không có
/// trong `journal.groups` → nhóm đầu tiên kèm một cảnh báo (D-08). Không bao giờ lỗi.
pub fn journal_group_of<'a>(
    map: &Value,
    project_slug: &str,
    cfg: &'a JournalConfig,
) -> &'a GroupDef {
    let raw = map
        .get("projects")
        .and_then(|p| p.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(project_slug))
        })
        .and_then(|entry| entry.get("journal_group"))
        .filter(|g| !g.is_null());
    let Some(raw) = raw else {
        return cfg.default_group();
    };
    let id = match raw.as_str() {
        Some(s) => s.to_string(),
        None => raw.to_string(),
    };
    match cfg.groups.iter().find(|g| g.id == id) {
        Some(g) => g,
        None => {
            let def = cfg.default_group();
            warn_unknown_group_once(project_slug, &id, &def.label);
            def
        }
    }
}

/// `https://github.com/{owner}/{repo}` suy từ `repo_url` (`git@github.com:o/r.git`,
/// `ssh://git@github.com/o/r.git`, `https://github.com/o/r(.git)`); `web_base_url` thắng.
pub fn web_base(cfg: &JournalConfig) -> Option<String> {
    if let Some(b) = &cfg.web_base_url {
        return Some(b.trim_end_matches('/').to_string());
    }
    let url = cfg.repo_url.as_deref()?;
    let path = [
        "git@github.com:",
        "ssh://git@github.com/",
        "https://github.com/",
    ]
    .iter()
    .find_map(|p| url.strip_prefix(p))?;
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    (ok(owner) && ok(repo)).then(|| format!("https://github.com/{owner}/{repo}"))
}

/// Liên kết github.com tới tệp `rel_path` của vault trên nhánh cấu hình (D-14).
pub fn blob_url(cfg: &JournalConfig, rel_path: &str) -> Option<String> {
    web_base(cfg).map(|b| format!("{b}/blob/{}/{}", cfg.branch, percent_encode_path(rel_path)))
}

/// RFC 3986: giữ `A-Za-z0-9-._~` và `/`, mọi byte UTF-8 khác thành `%XX` (chữ hoa).
pub fn percent_encode_path(p: &str) -> String {
    let mut out = String::with_capacity(p.len() * 3);
    for b in p.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn with(key: &str, val: Value) -> BrainResult<JournalConfig> {
        let mut o = serde_json::Map::new();
        o.insert(key.to_string(), val);
        parse(Some(&Value::Object(o)))
    }

    fn assert_rejected(key: &str, val: Value) {
        match with(key, val.clone()) {
            Ok(_) => panic!("journal.{key} = {val} phải bị từ chối"),
            Err(e) => assert!(
                e.message.contains(&format!("journal.{key}")),
                "{key}: {}",
                e.message
            ),
        }
    }

    fn assert_accepted(key: &str, val: Value) {
        if let Err(e) = with(key, val.clone()) {
            panic!("journal.{key} = {val} phải hợp lệ: {}", e.message);
        }
    }

    #[test]
    fn absent_section_is_off() {
        let d = JournalConfig::default();
        assert_eq!(parse(None).unwrap(), d);
        assert_eq!(parse(Some(&json!({}))).unwrap(), d);
        assert!(!d.enabled);
        assert_eq!(d.repo_url, None);
        assert_eq!(d.branch, "main");
        assert_eq!(d.interval_minutes, 30);
        assert_eq!(d.digest_time, (18, 0));
        assert_eq!(d.push_fail_notify_after, 3);
        assert_eq!(d.title_max_chars, 140);
        let labels: Vec<&str> = d.groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, vec!["Công việc", "Video", "Hỗ trợ bộ phận khác"]);
        assert_eq!(d.default_group().id, "cong-viec");
    }

    #[test]
    fn ranges_rejected() {
        let ranges: &[(&str, i64, i64)] = &[
            ("interval_minutes", 1, 1440),
            ("push_fail_notify_after", 1, 100),
            ("title_max_chars", 40, 400),
            ("callout_title_chars", 20, 200),
            ("git_timeout_seconds", 10, 600),
            ("ssh_connect_timeout_seconds", 5, 120),
            ("utc_offset_minutes", -720, 840),
            ("project_page_months", 1, 120),
            ("search_max_results", 1, 20),
        ];
        for (key, lo, hi) in ranges {
            assert_accepted(key, json!(lo));
            assert_accepted(key, json!(hi));
            assert_rejected(key, json!(lo - 1));
            assert_rejected(key, json!(hi + 1));
            assert_rejected(key, json!("10"));
        }
        assert_eq!(
            with("digest_time", json!("00:00")).unwrap().digest_time,
            (0, 0)
        );
        assert_eq!(
            with("digest_time", json!("23:59")).unwrap().digest_time,
            (23, 59)
        );
        for bad in ["24:00", "18:5", "1800", "18:60", ""] {
            assert_rejected("digest_time", json!(bad));
        }
    }

    #[test]
    fn repo_url_injection_rejected() {
        for bad in [
            "-oProxyCommand=x",
            "ext::sh -c touch",
            "git@github.com:o/r.git x",
            "git@github.com:o/r.git\n",
            "ftp://h/r",
            "repo.git",
            "",
        ] {
            assert_rejected("repo_url", json!(bad));
        }
        for good in [
            "git@github.com:owner/vault.git",
            "ssh://git@github.com/owner/vault.git",
            "https://github.com/owner/vault",
            "file:///tmp/v.git",
            "/tmp/v.git",
        ] {
            assert_accepted("repo_url", json!(good));
        }
    }

    #[test]
    fn branch_rejected() {
        for bad in ["-x", "a..b", "a b", "x.lock", ""] {
            assert_rejected("branch", json!(bad));
        }
        assert_rejected("branch", json!("a".repeat(101)));
        assert_accepted("branch", json!("main"));
        assert_accepted("branch", json!("release/journal-1"));
        assert_accepted("branch", json!("a".repeat(100)));
    }

    #[test]
    fn groups_validated() {
        let g = |id: &str, label: &str| json!({"id": id, "label": label});
        assert_rejected("groups", json!([]));
        let many: Vec<Value> = (0..11).map(|i| g(&format!("g{i}"), "Nhóm")).collect();
        assert_rejected("groups", Value::Array(many));
        assert_rejected("groups", json!([g("a", "A"), g("a", "B")]));
        assert_rejected("groups", json!([g("Cong Viec", "Công việc")]));
        assert_rejected("groups", json!([g("a", "")]));
        for bad_label in ["Nhóm [a]", "Nhóm | a", "Nhóm #a", "Nhóm\na"] {
            assert_rejected("groups", json!([g("a", bad_label)]));
        }
        let cfg = with(
            "groups",
            json!([g("video", "Video"), g("cong-viec", "Công việc")]),
        )
        .unwrap();
        assert_eq!(cfg.groups[0].id, "video");
        assert_eq!(cfg.default_group().label, "Video");
    }

    #[test]
    fn journal_group_lenient() {
        let cfg = JournalConfig::default();
        let map = json!({"projects": [
            {"id": "du-an-a", "journal_group": "video"},
            {"id": "du-an-b"},
            {"id": "du-an-c", "journal_group": "khong-co"},
        ]});
        assert_eq!(journal_group_of(&map, "du-an-a", &cfg).label, "Video");
        assert_eq!(journal_group_of(&map, "du-an-b", &cfg).label, "Công việc");
        assert_eq!(journal_group_of(&map, "du-an-c", &cfg).label, "Công việc");
        assert_eq!(journal_group_of(&map, "du-an-x", &cfg).label, "Công việc");
    }

    #[test]
    fn blob_url_percent_encodes() {
        let cfg = with(
            "repo_url",
            json!("git@github.com:example-owner/example-vault.git"),
        )
        .unwrap();
        assert_eq!(
            blob_url(&cfg, "Nhật ký/2026/11/2026-11-10.md").as_deref(),
            Some("https://github.com/example-owner/example-vault/blob/main/Nh%E1%BA%ADt%20k%C3%BD/2026/11/2026-11-10.md")
        );
        assert_eq!(
            blob_url(&cfg, "Dự án/du-an-a.md").as_deref(),
            Some("https://github.com/example-owner/example-vault/blob/main/D%E1%BB%B1%20%C3%A1n/du-an-a.md")
        );
        assert_eq!(percent_encode_path("a-b._~/c d"), "a-b._~/c%20d");
    }

    #[test]
    fn web_base_from_remote_forms() {
        for url in [
            "git@github.com:o/r.git",
            "ssh://git@github.com/o/r.git",
            "https://github.com/o/r",
            "https://github.com/o/r.git",
        ] {
            let cfg = with("repo_url", json!(url)).unwrap();
            assert_eq!(
                web_base(&cfg).as_deref(),
                Some("https://github.com/o/r"),
                "{url}"
            );
        }
        let local = with("repo_url", json!("/tmp/v.git")).unwrap();
        assert_eq!(web_base(&local), None);
        assert_eq!(blob_url(&local, "Today.md"), None);

        let explicit = parse(Some(&json!({
            "repo_url": "git@github.com:o/r.git",
            "web_base_url": "https://git.example.test/o/r"
        })))
        .unwrap();
        assert_eq!(
            web_base(&explicit).as_deref(),
            Some("https://git.example.test/o/r")
        );
        assert_eq!(
            blob_url(&explicit, "Today.md").as_deref(),
            Some("https://git.example.test/o/r/blob/main/Today.md")
        );
        assert_rejected("web_base_url", json!("http://x/o/r"));
        assert_rejected("web_base_url", json!("https://x/o r"));
    }
}
