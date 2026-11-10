//! Mục `journal` của config.json (Phase 7, D-07, D-08, D-11, D-18). Thiếu mục = tắt, mọi khóa
//! có mặc định; `journal_group` của từng dự án đọc lỏng từ projects_map.json (D-19).

use crate::error::{BrainError, BrainResult};
use serde_json::Value;
use std::path::PathBuf;

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

fn get_u64(o: &serde_json::Map<String, Value>, key: &str) -> BrainResult<Option<u64>> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .map(Some)
            .ok_or_else(|| err(format!("Cấu hình journal.{key} phải là số nguyên không âm"))),
    }
}

fn get_i64(o: &serde_json::Map<String, Value>, key: &str) -> BrainResult<Option<i64>> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_i64()
            .map(Some)
            .ok_or_else(|| err(format!("Cấu hình journal.{key} phải là số nguyên"))),
    }
}

fn parse_digest_time(s: &str) -> Option<(u32, u32)> {
    let (h, m) = s.split_once(':')?;
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    Some((h, m))
}

fn parse_groups(v: &Value) -> BrainResult<Vec<GroupDef>> {
    let arr = v
        .as_array()
        .ok_or_else(|| err("Cấu hình journal.groups phải là danh sách {id, label}".to_string()))?;
    let mut out = Vec::new();
    for g in arr {
        let id = g.get("id").and_then(|x| x.as_str());
        let label = g.get("label").and_then(|x| x.as_str());
        match (id, label) {
            (Some(id), Some(label)) => out.push(GroupDef {
                id: id.to_string(),
                label: label.to_string(),
            }),
            _ => {
                return Err(err(
                    "Cấu hình journal.groups phải là danh sách {id, label}".to_string()
                ))
            }
        }
    }
    if out.is_empty() {
        return Err(err(
            "Cấu hình journal.groups phải có ít nhất một nhóm".to_string()
        ));
    }
    Ok(out)
}

/// Đọc mục `journal` (thiếu hoặc `null` = mặc định, tắt).
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
        cfg.repo_url = Some(s.to_string());
    }
    if let Some(s) = get_str(o, "branch")? {
        cfg.branch = s.to_string();
    }
    if let Some(s) = get_str(o, "clone_dir")? {
        cfg.clone_dir = crate::config::expand_tilde(s, &crate::config::home_dir());
    }
    if let Some(n) = get_u64(o, "interval_minutes")? {
        cfg.interval_minutes = n as u32;
    }
    if let Some(b) = get_bool(o, "digest_enabled")? {
        cfg.digest_enabled = b;
    }
    if let Some(s) = get_str(o, "digest_time")? {
        cfg.digest_time = parse_digest_time(s)
            .ok_or_else(|| err("Cấu hình journal.digest_time phải có dạng HH:MM".to_string()))?;
    }
    if let Some(n) = get_u64(o, "push_fail_notify_after")? {
        cfg.push_fail_notify_after = n as u32;
    }
    if let Some(n) = get_u64(o, "title_max_chars")? {
        cfg.title_max_chars = n as usize;
    }
    if let Some(n) = get_u64(o, "callout_title_chars")? {
        cfg.callout_title_chars = n as usize;
    }
    if let Some(n) = get_u64(o, "git_timeout_seconds")? {
        cfg.git_timeout_seconds = n;
    }
    if let Some(n) = get_u64(o, "ssh_connect_timeout_seconds")? {
        cfg.ssh_connect_timeout_seconds = n;
    }
    if let Some(s) = get_str(o, "author_name")? {
        cfg.author_name = s.to_string();
    }
    if let Some(s) = get_str(o, "author_email")? {
        cfg.author_email = s.to_string();
    }
    if let Some(g) = o.get("groups").filter(|g| !g.is_null()) {
        cfg.groups = parse_groups(g)?;
    }
    if let Some(n) = get_i64(o, "utc_offset_minutes")? {
        cfg.utc_offset_minutes = Some(n as i32);
    }
    if let Some(s) = get_str(o, "web_base_url")? {
        cfg.web_base_url = Some(s.to_string());
    }
    if let Some(n) = get_u64(o, "project_page_months")? {
        cfg.project_page_months = n as u32;
    }
    if let Some(n) = get_u64(o, "search_max_results")? {
        cfg.search_max_results = n as usize;
    }
    Ok(cfg)
}

/// Nhóm của dự án `project_slug` theo khóa `journal_group` trong projects_map.json; không có
/// hoặc không khớp nhóm nào → nhóm đầu tiên (D-08).
pub fn journal_group_of<'a>(
    map: &Value,
    project_slug: &str,
    cfg: &'a JournalConfig,
) -> &'a GroupDef {
    let id = map
        .get("projects")
        .and_then(|p| p.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(project_slug))
        })
        .and_then(|entry| entry.get("journal_group"))
        .and_then(|g| g.as_str());
    match id.and_then(|id| cfg.groups.iter().find(|g| g.id == id)) {
        Some(g) => g,
        None => cfg.default_group(),
    }
}
