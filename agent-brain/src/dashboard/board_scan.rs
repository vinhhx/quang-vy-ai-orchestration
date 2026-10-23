//! Bảng công việc: đọc file GSD của dự án (chỉ trong `.planning/`, không theo symlink,
//! có giới hạn kích thước, số file và thời gian) rồi đưa ra `Facts` cho `board::derive`.
//!
//! Mọi byte đi qua `open_regular`: `symlink_metadata` phải báo file thường, mở bằng
//! `O_NOFOLLOW | O_NONBLOCK`, kiểm tra lại trên handle đã mở, đọc qua `Read::take`.

use crate::dashboard::board::{
    clean_text, Facts, PhaseFact, PhaseNum, PlanFact, RoadmapPlanLine, Warning,
};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub const ROADMAP_MAX_BYTES: u64 = 1_048_576;
pub const STATE_READ_BYTES: u64 = 16_384;
pub const PLAN_READ_BYTES: u64 = 32_768;
pub const LINE_MAX_CHARS: usize = 4_096;
pub const MAX_PHASE_DIRS: usize = 500;
pub const MAX_PLAN_FILES: usize = 2_000;
pub const MAX_DEPS_PER_PLAN: usize = 64;
pub const MAX_REQS_PER_PLAN: usize = 50;
pub const SCAN_BUDGET: Duration = Duration::from_secs(2);
pub const TITLE_MAX_CHARS: usize = 200;
pub const WARNING_MAX_CHARS: usize = 300;

const ROADMAP_REL: &str = ".planning/ROADMAP.md";

#[derive(Debug, Clone)]
pub struct Caps {
    pub roadmap_max_bytes: u64,
    pub state_read_bytes: u64,
    pub plan_read_bytes: u64,
    pub max_phase_dirs: usize,
    pub max_plan_files: usize,
    pub budget: Duration,
}

impl Default for Caps {
    fn default() -> Self {
        Caps {
            roadmap_max_bytes: ROADMAP_MAX_BYTES,
            state_read_bytes: STATE_READ_BYTES,
            plan_read_bytes: PLAN_READ_BYTES,
            max_phase_dirs: MAX_PHASE_DIRS,
            max_plan_files: MAX_PLAN_FILES,
            budget: SCAN_BUDGET,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReadError {
    Missing,
    NotRegular,
    TooLarge,
    Io,
}

impl ReadError {
    /// Lý do tiếng Việt, dùng trong cảnh báo và trạng thái trống.
    pub fn detail(&self) -> &'static str {
        match self {
            ReadError::Missing => "không tìm thấy file",
            ReadError::NotRegular => {
                "không phải file thường (symlink hoặc file đặc biệt bị từ chối)"
            }
            ReadError::TooLarge => "file lớn hơn 1 MiB",
            ReadError::Io => "lỗi đọc file",
        }
    }
}

/// Mở một file thường để đọc. Từ chối symlink, thư mục, FIFO, thiết bị.
pub fn open_regular(path: &Path) -> Result<(std::fs::File, u64), ReadError> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ReadError::Missing),
        Err(_) => return Err(ReadError::Io),
    };
    if !meta.file_type().is_file() {
        return Err(ReadError::NotRegular);
    }
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ReadError::Missing),
        Err(e) if e.raw_os_error() == Some(libc::ELOOP) => return Err(ReadError::NotRegular),
        Err(_) => return Err(ReadError::Io),
    };
    // Kiểm tra lại trên handle đã mở: đóng khe hở giữa kiểm tra và mở.
    let opened = file.metadata().map_err(|_| ReadError::Io)?;
    if !opened.file_type().is_file() {
        return Err(ReadError::NotRegular);
    }
    Ok((file, opened.len()))
}

fn read_capped(file: std::fs::File, cap: u64) -> Result<String, ReadError> {
    let mut buf = Vec::new();
    file.take(cap)
        .read_to_end(&mut buf)
        .map_err(|_| ReadError::Io)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Đọc tối đa `cap` byte đầu của file.
pub fn read_head(path: &Path, cap: u64) -> Result<String, ReadError> {
    let (file, _) = open_regular(path)?;
    read_capped(file, cap)
}

/// Đọc cả file; `TooLarge` khi dài hơn `max`.
pub fn read_whole(path: &Path, max: u64) -> Result<String, ReadError> {
    let (file, len) = open_regular(path)?;
    if len > max {
        return Err(ReadError::TooLarge);
    }
    read_capped(file, max)
}

/// Thư mục thật: `symlink_metadata` báo thư mục và không phải symlink.
pub fn is_real_dir(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(m) => m.file_type().is_dir() && !m.file_type().is_symlink(),
        Err(_) => false,
    }
}

fn plan_name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d+(?:\.\d+)*)-(\d+)-PLAN\.md$").expect("plan name regex"))
}

/// `05.4-03-PLAN.md` → (`05.4-03`, phase 5.4, số plan 3).
pub fn plan_file_id(file_name: &str) -> Option<(String, PhaseNum, u64)> {
    let caps = plan_name_re().captures(file_name)?;
    let phase = PhaseNum::parse(caps.get(1)?.as_str())?;
    let number = caps.get(2)?.as_str().parse::<u64>().ok()?;
    let id = file_name.strip_suffix("-PLAN.md")?.to_string();
    Some((id, phase, number))
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlanFm {
    pub phase: Option<String>,
    pub plan: Option<String>,
    pub wave: Option<u32>,
    pub depends_on: Vec<String>,
    pub requirements: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FmError {
    NoFrontMatter,
    Unterminated,
    BadList(String),
}

impl FmError {
    pub fn message(&self) -> String {
        match self {
            FmError::NoFrontMatter => "không có front matter (dòng đầu phải là ---)".to_string(),
            FmError::Unterminated => "front matter không có dòng --- đóng".to_string(),
            FmError::BadList(key) => format!("{key} không phải danh sách một dòng dạng [a, b]"),
        }
    }
}

/// Bỏ ` #ghi chú` ở cuối, chỉ khi nằm ngoài dấu nháy.
fn strip_comment(s: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut prev_space = true;
    for (i, c) in s.char_indices() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c == '#' && prev_space {
                    return &s[..i];
                }
            }
        }
        prev_space = c.is_whitespace();
    }
    s
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    let bytes = t.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        return t[1..t.len() - 1].to_string();
    }
    t.to_string()
}

fn scalar(raw: &str) -> String {
    unquote(strip_comment(raw).trim())
}

/// Tách theo dấu phẩy ngoài dấu nháy, trim, bỏ nháy, bỏ phần rỗng.
fn split_list(inner: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in inner.chars() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                    cur.push(c);
                } else if c == ',' {
                    parts.push(std::mem::take(&mut cur));
                } else {
                    cur.push(c);
                }
            }
        }
    }
    parts.push(cur);
    parts
        .into_iter()
        .map(|p| unquote(&p))
        .filter(|p| !p.is_empty())
        .collect()
}

/// Số dòng nối tiếp tối đa của một danh sách `[a,\n b]` trải trên nhiều dòng.
const FM_LIST_CONTINUATION_MAX: usize = 20;

/// Dòng cột 0 dạng `khoá:` — bắt đầu một khoá mới của front matter.
fn looks_like_key(line: &str) -> bool {
    let mut chars = line.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    for c in chars {
        if c == ':' {
            return true;
        }
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return false;
        }
    }
    false
}

/// Đọc giá trị danh sách của `key`: `[a, b]` một hoặc nhiều dòng, khối `- mục` ở các dòng sau,
/// `null` / `~` / rỗng là danh sách rỗng. Trả về danh sách và số dòng tiếp theo đã dùng.
fn list_value(key: &str, raw: &str, rest: &[&str]) -> Result<(Vec<String>, usize), FmError> {
    let v = strip_comment(raw).trim();
    if v.is_empty() {
        // Danh sách dạng khối: các dòng `- mục` ngay sau khoá (thụt vào hoặc sát lề).
        let mut items: Vec<String> = Vec::new();
        let mut used = 0usize;
        for line in rest {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                used += 1;
                continue;
            }
            match t.strip_prefix('-') {
                Some(item) if item.is_empty() || item.starts_with(char::is_whitespace) => {
                    let value = scalar(item);
                    if !value.is_empty() {
                        items.push(value);
                    }
                    used += 1;
                }
                _ => break,
            }
        }
        return Ok((items, used));
    }
    if v == "null" || v == "~" {
        return Ok((Vec::new(), 0));
    }
    if v.starts_with('[') {
        let mut text = v.to_string();
        let mut used = 0usize;
        while !text.ends_with(']') {
            let next = rest.get(used).copied();
            match next {
                Some(line) if used < FM_LIST_CONTINUATION_MAX && !looks_like_key(line) => {
                    text.push(' ');
                    text.push_str(strip_comment(line).trim());
                    used += 1;
                }
                _ => return Err(FmError::BadList(key.to_string())),
            }
        }
        let inner = &text[1..text.len() - 1];
        return Ok((split_list(inner), used));
    }
    // Giá trị đơn không có ngoặc: một phần tử.
    Ok((vec![unquote(v)], 0))
}

fn cap_list(key: &str, mut list: Vec<String>, cap: usize, notes: &mut Vec<String>) -> Vec<String> {
    if list.len() > cap {
        list.truncate(cap);
        notes.push(format!("{key} có hơn {cap} mục — chỉ dùng {cap} mục đầu"));
    }
    list
}

/// Tập con của YAML front matter: chỉ đọc `phase`, `plan`, `wave`, `depends_on`, `requirements`.
pub fn parse_plan_front_matter(text: &str) -> Result<PlanFm, FmError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    match lines.next() {
        Some(first) if first.trim_end() == "---" => {}
        _ => return Err(FmError::NoFrontMatter),
    }
    let mut block: Vec<&str> = Vec::new();
    let mut closed = false;
    for line in lines {
        if line.trim_end() == "---" {
            closed = true;
            break;
        }
        block.push(line);
    }
    if !closed {
        return Err(FmError::Unterminated);
    }

    let mut fm = PlanFm::default();
    let mut seen: Vec<&str> = Vec::new();
    let mut at = 0usize;
    while at < block.len() {
        let line = block[at];
        at += 1;
        let first = match line.chars().next() {
            Some(c) => c,
            None => continue,
        };
        if first == ' ' || first == '\t' || first == '#' {
            continue;
        }
        if !(first.is_ascii_alphabetic() || first == '_') {
            continue;
        }
        let colon = match line.find(':') {
            Some(i) => i,
            None => continue,
        };
        let key = &line[..colon];
        if !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            continue;
        }
        let value = &line[colon + 1..];
        let known = ["phase", "plan", "wave", "depends_on", "requirements"];
        let Some(&name) = known.iter().find(|k| **k == key) else {
            continue;
        };
        if seen.contains(&name) {
            continue; // khoá lặp: lần đầu thắng
        }
        seen.push(name);
        match name {
            "phase" => fm.phase = Some(scalar(value)),
            "plan" => fm.plan = Some(scalar(value)),
            "wave" => {
                let raw = scalar(value);
                match raw.parse::<u32>() {
                    Ok(w) => fm.wave = Some(w),
                    Err(_) => {
                        if !raw.is_empty() && raw != "null" && raw != "~" {
                            fm.notes.push(format!("wave không phải số nguyên: {raw}"));
                        }
                    }
                }
            }
            "depends_on" => {
                let (list, used) = list_value(name, value, &block[at..])?;
                at += used;
                fm.depends_on = cap_list(name, list, MAX_DEPS_PER_PLAN, &mut fm.notes);
            }
            _ => {
                let (list, used) = list_value(name, value, &block[at..])?;
                at += used;
                fm.requirements = cap_list(name, list, MAX_REQS_PER_PLAN, &mut fm.notes);
            }
        }
    }
    Ok(fm)
}

pub struct RoadmapScan {
    pub phases: Vec<PhaseFact>,
    pub warnings: Vec<Warning>,
}

fn phase_heading_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^#{3,4}\s+Phase\s+(\d+(?:\.\d+)*):\s*(.+?)\s*$").expect("phase heading regex")
    })
}

fn checkbox_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^-\s+\[( |x|X)\]\s+\*\*Phase\s+(\d+(?:\.\d+)*):").expect("checkbox regex")
    })
}

fn depends_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\*\*Depends on(?:\*\*:?|:\*\*)\s*(.*)$").expect("depends regex")
    })
}

fn phase_ref_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"Phase\s+(\d+(?:\.\d+)*)").expect("phase ref regex"))
}

fn plan_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s*-\s+\[( |x|X)\]\s+(\d+(?:\.\d+)*-\d+)(?:-PLAN\.md)?(.*)$")
            .expect("plan line regex")
    })
}

fn wave_suffix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\s*(?:[—–-]\s*wave\s+\d+|\(wave\s+\d+\))\s*$").expect("wave suffix regex")
    })
}

fn summary_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<summary>(.*?)</summary>").expect("summary regex"))
}

fn summary_milestone_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(v\d+(?:\.\d+)*)\s+(.*)$").expect("summary milestone regex"))
}

fn milestone_heading_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^###\s+\S*\s*(v\d+(?:\.\d+)*)\s+(.+?)(?:\s+\(.*\))?$")
            .expect("milestone heading regex")
    })
}

fn push_warning(warnings: &mut Vec<Warning>, path: &str, message: &str) {
    warnings.push(Warning {
        path: clean_text(path, WARNING_MAX_CHARS),
        message: clean_text(message, WARNING_MAX_CHARS),
    });
}

/// `<summary>✅ v1.0 MVP (Phases 1-2) - SHIPPED …</summary>` → `v1.0 MVP`.
fn milestone_from_summary(line: &str) -> Option<String> {
    let inner = summary_re().captures(line)?.get(1)?.as_str();
    let caps = summary_milestone_re().captures(inner)?;
    let version = caps.get(1)?.as_str();
    let rest = caps.get(2).map(|m| m.as_str()).unwrap_or("");
    let cut = [rest.find('('), rest.find(" - ")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(rest.len());
    let full = format!("{version} {}", rest[..cut].trim());
    Some(clean_text(&full, TITLE_MAX_CHARS))
}

/// `### 🚧 v1.1 Xuất báo cáo (In Progress)` → `v1.1 Xuất báo cáo`.
fn milestone_from_heading(line: &str) -> Option<String> {
    let caps = milestone_heading_re().captures(line)?;
    let full = format!("{} {}", caps.get(1)?.as_str(), caps.get(2)?.as_str());
    Some(clean_text(&full, TITLE_MAX_CHARS))
}

/// Chữ của dòng plan trong ROADMAP: bỏ dấu phân cách đầu và hậu tố `— wave N` / `(wave N)`.
fn plan_line_title(rest: &str) -> String {
    let t = rest.trim().trim_start_matches(|c: char| {
        c == ':' || c == '—' || c == '–' || c == '-' || c.is_whitespace()
    });
    let t = wave_suffix_re().replace(t, "");
    clean_text(t.trim(), TITLE_MAX_CHARS)
}

pub fn parse_roadmap(text: &str) -> RoadmapScan {
    let mut phases: Vec<PhaseFact> = Vec::new();
    let mut warnings: Vec<Warning> = Vec::new();
    let mut top_checks: Vec<(PhaseNum, bool)> = Vec::new();
    let mut milestone: Option<String> = None;
    let mut current: Option<usize> = None;
    for (idx, line) in text.lines().enumerate() {
        if line.chars().count() > LINE_MAX_CHARS {
            push_warning(
                &mut warnings,
                ROADMAP_REL,
                &format!("bỏ qua dòng {}: dài hơn {LINE_MAX_CHARS} ký tự", idx + 1),
            );
            continue;
        }
        if line.contains("<summary>") {
            if let Some(m) = milestone_from_summary(line) {
                milestone = Some(m);
                current = None;
            }
            continue;
        }
        if line.contains("</details>") {
            milestone = None;
            current = None;
            continue;
        }
        if let Some(caps) = phase_heading_re().captures(line) {
            let Some(num) = caps.get(1).and_then(|m| PhaseNum::parse(m.as_str())) else {
                current = None;
                continue;
            };
            let raw_name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let name = raw_name
                .strip_suffix("(INSERTED)")
                .unwrap_or(raw_name)
                .trim();
            phases.push(PhaseFact {
                num,
                name: clean_text(name, TITLE_MAX_CHARS),
                milestone: milestone.clone(),
                depends_on: Vec::new(),
                checkbox: None,
                roadmap_plans: Vec::new(),
            });
            current = Some(phases.len() - 1);
            continue;
        }
        if let Some(m) = milestone_from_heading(line) {
            milestone = Some(m);
            current = None;
            continue;
        }
        if let Some(caps) = checkbox_re().captures(line) {
            let checked = caps.get(1).map(|m| m.as_str() != " ").unwrap_or(false);
            if let Some(num) = caps.get(2).and_then(|m| PhaseNum::parse(m.as_str())) {
                top_checks.push((num, checked));
            }
            continue;
        }
        let Some(cur) = current else {
            continue;
        };
        if let Some(caps) = depends_re().captures(line) {
            let rest = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            // Chỉ đọc phần trước dấu `(` đầu tiên: phần trong ngoặc nhắc phase khác.
            let head = rest.split('(').next().unwrap_or("");
            if head.trim_start().starts_with("Nothing") {
                continue;
            }
            for found in phase_ref_re().captures_iter(head) {
                let raw = found.get(1).map(|m| m.as_str()).unwrap_or("");
                match PhaseNum::parse(raw) {
                    Some(n) => phases[cur].depends_on.push(n),
                    None => push_warning(
                        &mut warnings,
                        ROADMAP_REL,
                        &format!("số phase không hợp lệ: {raw}"),
                    ),
                }
            }
            continue;
        }
        if let Some(caps) = plan_line_re().captures(line) {
            let checked = caps.get(1).map(|m| m.as_str() != " ").unwrap_or(false);
            let id = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let rest = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            phases[cur].roadmap_plans.push(RoadmapPlanLine {
                id: id.to_string(),
                title: plan_line_title(rest),
                checked,
            });
        }
    }
    for ph in &mut phases {
        if let Some((_, checked)) = top_checks.iter().find(|(n, _)| *n == ph.num) {
            ph.checkbox = Some(*checked);
        }
    }
    RoadmapScan { phases, warnings }
}

/// Câu đầu của `<objective>`: dòng không trống đầu tiên sau dòng chứa `<objective>`, bỏ dấu
/// backtick, gộp khoảng trắng, cắt tại `. ` đầu tiên hoặc 120 ký tự.
fn objective_title(head: &str) -> Option<String> {
    let mut lines = head.lines();
    lines.find(|l| l.contains("<objective>"))?;
    let line = lines.find(|l| !l.trim().is_empty())?;
    if line.trim_start().starts_with("</objective>") {
        return None;
    }
    let plain: String = line.chars().filter(|c| *c != '`').collect();
    let collapsed = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    let first = match collapsed.find(". ") {
        Some(i) => &collapsed[..i],
        None => collapsed.as_str(),
    };
    let cut: String = first.chars().take(120).collect();
    let cut = cut.trim();
    if cut.is_empty() {
        None
    } else {
        Some(cut.to_string())
    }
}

fn state_body_phase_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^Phase:\s*(\d+(?:\.\d+)*)\b").expect("state phase regex"))
}

/// Phase hiện tại trong STATE.md: khoá `current_phase` của front matter (cột 0, có hoặc không có
/// nháy); không có thì dòng thân `Phase: 05.5 — …`; không đọc được gì thì `None`.
pub fn parse_state(text: &str) -> Option<PhaseNum> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines().peekable();
    if lines.peek().is_some_and(|l| l.trim_end() == "---") {
        lines.next();
        for line in lines.by_ref() {
            if line.trim_end() == "---" {
                break;
            }
            if let Some(rest) = line.strip_prefix("current_phase:") {
                if let Some(num) = PhaseNum::parse(&scalar(rest)) {
                    return Some(num);
                }
            }
        }
    }
    for line in lines {
        if let Some(caps) = state_body_phase_re().captures(line) {
            return caps.get(1).and_then(|m| PhaseNum::parse(m.as_str()));
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct ScanRefusal {
    pub reason: &'static str,
    pub detail: String,
}

fn out_of_time(facts: &mut Facts) {
    facts.truncated = true;
    push_warning(
        &mut facts.warnings,
        ".planning/phases",
        "dừng quét: quá thời gian đọc 2 giây — bảng có thể thiếu hạng mục",
    );
}

/// Đọc `.planning/` của dự án. `root` là thư mục dự án đã tin cậy (từ `Project::root()`).
pub fn scan(root: &Path, caps: &Caps, deadline: Instant) -> Result<Facts, ScanRefusal> {
    let planning = root.join(".planning");
    let roadmap_text =
        read_whole(&planning.join("ROADMAP.md"), caps.roadmap_max_bytes).map_err(|e| {
            ScanRefusal {
                reason: "unreadable",
                detail: e.detail().to_string(),
            }
        })?;
    let rs = parse_roadmap(&roadmap_text);
    let known_phases: HashSet<PhaseNum> = rs.phases.iter().map(|p| p.num.clone()).collect();
    let mut roadmap_titles: HashMap<String, String> = HashMap::new();
    for line in rs.phases.iter().flat_map(|p| p.roadmap_plans.iter()) {
        if !line.title.is_empty() {
            roadmap_titles
                .entry(line.id.clone())
                .or_insert_with(|| line.title.clone());
        }
    }
    let mut unlisted_dirs: HashSet<String> = HashSet::new();
    let mut facts = Facts {
        phases: rs.phases,
        warnings: rs.warnings,
        ..Facts::default()
    };

    match read_head(&planning.join("STATE.md"), caps.state_read_bytes) {
        Ok(text) => facts.current_phase = parse_state(&text),
        Err(ReadError::Missing) => {}
        Err(_) => push_warning(
            &mut facts.warnings,
            ".planning/STATE.md",
            "STATE.md không đọc được — dùng phase đầu tiên chưa xong",
        ),
    }

    let phases_dir = planning.join("phases");
    match std::fs::symlink_metadata(&phases_dir) {
        Err(_) => return Ok(facts),
        Ok(_) if !is_real_dir(&phases_dir) => {
            push_warning(
                &mut facts.warnings,
                ".planning/phases",
                "bỏ qua .planning/phases: symlink hoặc không phải thư mục thật",
            );
            return Ok(facts);
        }
        Ok(_) => {}
    }

    // Liệt kê thư mục phase: chỉ thư mục thật, sắp theo tên.
    let mut dir_names: Vec<String> = Vec::new();
    let entries = match std::fs::read_dir(&phases_dir) {
        Ok(e) => e,
        Err(_) => {
            push_warning(
                &mut facts.warnings,
                ".planning/phases",
                "không đọc được thư mục .planning/phases",
            );
            return Ok(facts);
        }
    };
    let mut seen_entries = 0usize;
    for entry in entries.flatten() {
        seen_entries += 1;
        if seen_entries % 256 == 0 && Instant::now() >= deadline {
            out_of_time(&mut facts);
            break;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_real_dir(&entry.path()) {
            dir_names.push(name);
        } else {
            push_warning(
                &mut facts.warnings,
                &format!(".planning/phases/{name}"),
                "bỏ qua thư mục: symlink hoặc không phải thư mục thật",
            );
        }
    }
    dir_names.sort();
    if dir_names.len() > caps.max_phase_dirs {
        dir_names.truncate(caps.max_phase_dirs);
        facts.truncated = true;
        push_warning(
            &mut facts.warnings,
            ".planning/phases",
            &format!("dừng quét: quá {} thư mục phase", caps.max_phase_dirs),
        );
    }

    let mut plan_count = 0usize;
    'dirs: for dir in &dir_names {
        let dir_path = phases_dir.join(dir);
        let rel_dir = format!(".planning/phases/{dir}");
        let entries = match std::fs::read_dir(&dir_path) {
            Ok(e) => e,
            Err(_) => {
                push_warning(&mut facts.warnings, &rel_dir, "không đọc được thư mục");
                continue;
            }
        };
        let mut files: Vec<String> = Vec::new();
        let mut listed = 0usize;
        for entry in entries.flatten() {
            listed += 1;
            if listed % 256 == 0 && Instant::now() >= deadline {
                out_of_time(&mut facts);
                break 'dirs;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if plan_file_id(&name).is_some() {
                files.push(name);
            }
        }
        files.sort();

        for file in files {
            if Instant::now() >= deadline {
                out_of_time(&mut facts);
                break 'dirs;
            }
            if plan_count >= caps.max_plan_files {
                facts.truncated = true;
                push_warning(
                    &mut facts.warnings,
                    ".planning/phases",
                    &format!("dừng quét: quá {} file plan", caps.max_plan_files),
                );
                break 'dirs;
            }
            let Some((id, phase, number)) = plan_file_id(&file) else {
                continue;
            };
            let rel = format!("{rel_dir}/{file}");
            let head = match read_head(&dir_path.join(&file), caps.plan_read_bytes) {
                Ok(t) => t,
                Err(e) => {
                    push_warning(&mut facts.warnings, &rel, e.detail());
                    continue;
                }
            };
            plan_count += 1;

            if !known_phases.contains(&phase) && unlisted_dirs.insert(dir.clone()) {
                push_warning(
                    &mut facts.warnings,
                    &rel_dir,
                    &format!(
                        "thư mục phase {dir} không có trong ROADMAP.md (vẫn hiện thành epic riêng)"
                    ),
                );
            }

            let mut fm_wave = None;
            let mut depends_on = Vec::new();
            let mut requirements = Vec::new();
            match parse_plan_front_matter(&head) {
                Ok(fm) => {
                    fm_wave = fm.wave;
                    depends_on = fm.depends_on;
                    requirements = fm.requirements;
                    for note in &fm.notes {
                        push_warning(&mut facts.warnings, &rel, note);
                    }
                }
                Err(e) => {
                    push_warning(
                        &mut facts.warnings,
                        &rel,
                        &format!(
                            "front matter lỗi: {} — hạng mục vẫn hiện, không có phụ thuộc",
                            e.message()
                        ),
                    );
                }
            }

            let summary = dir_path.join(format!("{id}-SUMMARY.md"));
            let done = std::fs::symlink_metadata(&summary)
                .map(|m| m.file_type().is_file() && !m.file_type().is_symlink())
                .unwrap_or(false);

            // Tiêu đề: dòng plan trong ROADMAP, rồi câu đầu của <objective>, rồi mã plan.
            let raw_title = roadmap_titles
                .get(&id)
                .cloned()
                .or_else(|| objective_title(&head))
                .unwrap_or_else(|| id.clone());
            let mut title = clean_text(&raw_title, TITLE_MAX_CHARS);
            if title.is_empty() {
                title = clean_text(&id, TITLE_MAX_CHARS);
            }

            facts.plans.push(PlanFact {
                title,
                id,
                phase,
                number,
                fm_wave,
                depends_on,
                requirements,
                done,
                path: clean_text(&rel, WARNING_MAX_CHARS),
            });
        }
    }
    Ok(facts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_file_id_accepts_only_plan_files() {
        let (id, phase, n) = plan_file_id("05.4-03-PLAN.md").expect("plan id");
        assert_eq!(id, "05.4-03");
        assert_eq!(phase.display(), "05.4");
        assert_eq!(n, 3);
        assert!(plan_file_id("05.4-03-SUMMARY.md").is_none());
        assert!(plan_file_id("05.4-CONTEXT.md").is_none());
        assert!(plan_file_id("01-02-03-PLAN.md").is_none());
        assert!(plan_file_id("x-01-PLAN.md").is_none());
    }

    #[test]
    fn front_matter_skips_nested_blocks() {
        let text = "---\nphase: 01-nen-tang\nplan: 02\ntype: execute\nwave: 2\ndepends_on: [01-01]\nfiles_modified:\n  - src/app.py\nrequirements: [DEMO-01, DEMO-02]\nmust_haves:\n  truths:\n    - \"dòng có dấu hai chấm: và --- ở giữa\"\n---\n\n<objective>x</objective>\n";
        let fm = parse_plan_front_matter(text).expect("fm");
        assert_eq!(fm.phase.as_deref(), Some("01-nen-tang"));
        assert_eq!(fm.plan.as_deref(), Some("02"));
        assert_eq!(fm.wave, Some(2));
        assert_eq!(fm.depends_on, ["01-01"]);
        assert_eq!(fm.requirements, ["DEMO-01", "DEMO-02"]);
        assert!(fm.notes.is_empty());
    }

    #[test]
    fn front_matter_list_shapes() {
        let bare = parse_plan_front_matter("---\ndepends_on: [02-01, 02-02]\n---\n").unwrap();
        assert_eq!(bare.depends_on, ["02-01", "02-02"]);
        let quoted =
            parse_plan_front_matter("---\ndepends_on: [\"05.2-01\", \"05.2-02\"]\n---\n").unwrap();
        assert_eq!(quoted.depends_on, ["05.2-01", "05.2-02"]);
        let empty = parse_plan_front_matter("---\ndepends_on: []\n---\n").unwrap();
        assert!(empty.depends_on.is_empty());
        let commented =
            parse_plan_front_matter("---\ndepends_on: [01-01] # ghi chú\n---\n").unwrap();
        assert_eq!(commented.depends_on, ["01-01"]);
    }

    #[test]
    fn front_matter_errors() {
        assert_eq!(
            parse_plan_front_matter("khong co\n"),
            Err(FmError::NoFrontMatter)
        );
        assert_eq!(
            parse_plan_front_matter("---\nwave: 1\n"),
            Err(FmError::Unterminated)
        );
        assert_eq!(
            parse_plan_front_matter("---\ndepends_on: [01-01\n---\n"),
            Err(FmError::BadList("depends_on".to_string()))
        );
    }

    #[test]
    fn front_matter_bom_crlf_and_bad_wave() {
        let text = "\u{feff}---\r\nwave: hai\r\ndepends_on: [01-01]\r\n---\r\n";
        let fm = parse_plan_front_matter(text).unwrap();
        assert_eq!(fm.wave, None);
        assert_eq!(fm.notes, ["wave không phải số nguyên: hai"]);
        assert_eq!(fm.depends_on, ["01-01"]);
    }

    #[test]
    fn front_matter_caps_and_first_key_wins() {
        let deps: Vec<String> = (0..70).map(|i| format!("01-{i:02}")).collect();
        let text = format!(
            "---\ndepends_on: [{}]\nwave: 1\nwave: 9\n---\n",
            deps.join(", ")
        );
        let fm = parse_plan_front_matter(&text).unwrap();
        assert_eq!(fm.depends_on.len(), MAX_DEPS_PER_PLAN);
        assert_eq!(fm.wave, Some(1));
        assert_eq!(fm.notes, ["depends_on có hơn 64 mục — chỉ dùng 64 mục đầu"]);
    }

    #[test]
    fn roadmap_headings_and_long_lines() {
        let long = format!("### Phase 9: {}", "x".repeat(LINE_MAX_CHARS + 1));
        let text = format!(
            "# Roadmap\n### Phase 1: Nền tảng\n#### Phase 05.5: Bảng (INSERTED)\n{long}\n## Phase 2: sai cấp\n"
        );
        let rs = parse_roadmap(&text);
        let seen: Vec<(String, String)> = rs
            .phases
            .iter()
            .map(|p| (p.num.display(), p.name.clone()))
            .collect();
        assert_eq!(
            seen,
            [
                ("01".to_string(), "Nền tảng".to_string()),
                ("05.5".to_string(), "Bảng".to_string())
            ]
        );
        assert_eq!(rs.warnings.len(), 1);
        assert_eq!(rs.warnings[0].message, "bỏ qua dòng 4: dài hơn 4096 ký tự");
        assert_eq!(rs.warnings[0].path, ".planning/ROADMAP.md");
    }

    #[test]
    fn state_current_phase_quoted_bare_and_absent() {
        let phase = |t: &str| parse_state(t).map(|p| p.display());
        assert_eq!(
            phase("---\ncurrent_phase: \"05.5\"\ncurrent_phase_name: x\n---\n").as_deref(),
            Some("05.5")
        );
        assert_eq!(phase("---\ncurrent_phase: 2\n---\n").as_deref(), Some("02"));
        assert_eq!(
            phase("---\ncurrent_phase: '3' # ghi chú\n---\n").as_deref(),
            Some("03")
        );
        assert_eq!(phase("---\ncurrent_phase_name: x\n---\n"), None);
        assert_eq!(phase("current_phase: 2\n"), None);
        assert_eq!(phase("---\nname: x\n---\ncurrent_phase: 2\n"), None);
        assert_eq!(phase("---\ncurrent_phase: tam\n---\n"), None);
        assert_eq!(phase(""), None);
    }

    #[test]
    fn open_regular_refuses_symlink_and_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let real = dir.path().join("real.md");
        std::fs::write(&real, "hello").unwrap();
        let link = dir.path().join("link.md");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert_eq!(read_head(&real, 3).unwrap(), "hel");
        assert_eq!(read_whole(&real, 10).unwrap(), "hello");
        assert_eq!(read_whole(&real, 4), Err(ReadError::TooLarge));
        assert_eq!(read_head(&link, 10), Err(ReadError::NotRegular));
        assert_eq!(read_head(dir.path(), 10), Err(ReadError::NotRegular));
        assert_eq!(
            read_head(&dir.path().join("none.md"), 10),
            Err(ReadError::Missing)
        );
        assert!(is_real_dir(dir.path()));
        assert!(!is_real_dir(&real));
        let dir_link = dir.path().join("dlink");
        std::os::unix::fs::symlink(dir.path(), &dir_link).unwrap();
        assert!(!is_real_dir(&dir_link));
    }

    fn tree(plans: usize) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let phase = dir.path().join(".planning/phases/01-a");
        std::fs::create_dir_all(&phase).unwrap();
        std::fs::write(dir.path().join(".planning/ROADMAP.md"), "### Phase 1: A\n").unwrap();
        for i in 1..=plans {
            std::fs::write(
                phase.join(format!("01-{i:02}-PLAN.md")),
                "---\nwave: 1\ndepends_on: []\n---\n",
            )
            .unwrap();
        }
        dir
    }

    fn far() -> Instant {
        Instant::now() + Duration::from_secs(60)
    }

    #[test]
    fn scan_stops_at_plan_file_cap() {
        let dir = tree(5);
        let caps = Caps {
            max_plan_files: 3,
            ..Caps::default()
        };
        let facts = scan(dir.path(), &caps, far()).expect("scan");
        assert_eq!(facts.plans.len(), 3);
        assert!(facts.truncated);
        assert!(facts
            .warnings
            .iter()
            .any(|w| w.message == "dừng quét: quá 3 file plan"));
    }

    #[test]
    fn scan_stops_after_deadline() {
        let dir = tree(2);
        let facts = scan(dir.path(), &Caps::default(), Instant::now()).expect("scan");
        assert!(facts.plans.is_empty());
        assert!(facts.truncated);
        assert!(facts
            .warnings
            .iter()
            .any(|w| w.message.starts_with("dừng quét: quá thời gian đọc 2 giây")));
    }

    #[test]
    fn scan_skips_symlinked_phase_dir_and_summary() {
        let dir = tree(1);
        let phases = dir.path().join(".planning/phases");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(
            outside.path().join("09-01-PLAN.md"),
            "---\nwave: 1\ndepends_on: []\n---\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path(), phases.join("09-out")).unwrap();
        // SUMMARY là symlink thì tính là chưa xong
        let secret = outside.path().join("s.md");
        std::fs::write(&secret, "x").unwrap();
        std::os::unix::fs::symlink(&secret, phases.join("01-a/01-01-SUMMARY.md")).unwrap();

        let facts = scan(dir.path(), &Caps::default(), far()).expect("scan");
        assert_eq!(facts.plans.len(), 1);
        assert_eq!(facts.plans[0].id, "01-01");
        assert!(!facts.plans[0].done);
        assert!(facts.warnings.iter().any(|w| {
            w.path == ".planning/phases/09-out"
                && w.message == "bỏ qua thư mục: symlink hoặc không phải thư mục thật"
        }));
    }

    #[test]
    fn scan_keeps_plan_with_bad_front_matter() {
        let dir = tree(1);
        std::fs::write(
            dir.path().join(".planning/phases/01-a/01-01-PLAN.md"),
            "không có front matter\n",
        )
        .unwrap();
        let facts = scan(dir.path(), &Caps::default(), far()).expect("scan");
        assert_eq!(facts.plans.len(), 1);
        assert!(facts.plans[0].depends_on.is_empty());
        assert_eq!(facts.warnings.len(), 1);
        assert_eq!(
            facts.warnings[0].path,
            ".planning/phases/01-a/01-01-PLAN.md"
        );
        assert!(facts.warnings[0].message.starts_with("front matter lỗi: "));
    }

    #[test]
    fn scan_refuses_missing_or_symlinked_roadmap() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".planning")).unwrap();
        let err = scan(dir.path(), &Caps::default(), far()).expect_err("missing");
        assert_eq!(err.reason, "unreadable");
        assert_eq!(err.detail, "không tìm thấy file");
        let target = dir.path().join("elsewhere.md");
        std::fs::write(&target, "### Phase 1: X\n").unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join(".planning/ROADMAP.md")).unwrap();
        let err = scan(dir.path(), &Caps::default(), far()).expect_err("symlink");
        assert!(err.detail.starts_with("không phải file thường"));
    }

    fn nums(list: &[PhaseNum]) -> Vec<String> {
        list.iter().map(|n| n.display()).collect()
    }

    #[test]
    fn roadmap_depends_on_ignores_parentheticals() {
        let text = "### Phase 3: Ba\n**Depends on**: Phase 2 (và kết quả S1, S4, S5, S8 + quyết định tài khoản từ Phase 1)\n### Phase 1: Một\n**Depends on**: Nothing (first phase)\n### Phase 4: Bốn\n**Depends on:** Phase 3, Phase 05.2 (ghi chú Phase 1)\n### Phase 5: Năm\n**Depends on**: Phase 4 (kết quả Phase 2)\n**Depends on**: Phase 3\n";
        let rs = parse_roadmap(text);
        assert!(rs.warnings.is_empty());
        let by = |n: &str| {
            rs.phases
                .iter()
                .find(|p| p.num.display() == n)
                .expect("phase")
        };
        assert_eq!(nums(&by("03").depends_on), ["02"]);
        assert!(by("01").depends_on.is_empty());
        assert_eq!(nums(&by("04").depends_on), ["03", "05.2"]);
        assert_eq!(nums(&by("05").depends_on), ["04", "03"]);
    }

    #[test]
    fn roadmap_depends_on_bad_number_is_a_warning() {
        let text = "### Phase 2: Hai\n**Depends on**: Phase 1234567\n";
        let rs = parse_roadmap(text);
        assert!(rs.phases[0].depends_on.is_empty());
        assert_eq!(rs.warnings.len(), 1);
        assert_eq!(rs.warnings[0].message, "số phase không hợp lệ: 1234567");
        assert_eq!(rs.warnings[0].path, ".planning/ROADMAP.md");
    }

    #[test]
    fn roadmap_checkboxes_top_list() {
        let text = "## Phases\n\n- [x] **Phase 1: Một** - xong\n- [ ] **Phase 2: Hai** - đang làm\n- [X] **Phase 3: Ba**\n\n### Phase 1: Một\n### Phase 2: Hai\n### Phase 3: Ba\n### Phase 4: Bốn\n";
        let rs = parse_roadmap(text);
        let boxes: Vec<(String, Option<bool>)> = rs
            .phases
            .iter()
            .map(|p| (p.num.display(), p.checkbox))
            .collect();
        assert_eq!(
            boxes,
            [
                ("01".to_string(), Some(true)),
                ("02".to_string(), Some(false)),
                ("03".to_string(), Some(true)),
                ("04".to_string(), None),
            ]
        );
    }

    #[test]
    fn roadmap_plan_lines_both_shapes() {
        let text = "### Phase 5.4: X\nPlans:\n- [x] 05.4-01-PLAN.md — Shared tiện ích (wave 1)\n- [x] 02-01-PLAN.md — Tracer: đường chạy đầu — wave 1\n- [x] 05.2-32-PLAN.md\n- [ ] 01-01: Mô tả mẫu\n- [ ] 01-02 - Chữ sau gạch\n- [ ] ghi chú không phải plan\n";
        let rs = parse_roadmap(text);
        let lines: Vec<(String, String, bool)> = rs.phases[0]
            .roadmap_plans
            .iter()
            .map(|l| (l.id.clone(), l.title.clone(), l.checked))
            .collect();
        assert_eq!(
            lines,
            [
                ("05.4-01".to_string(), "Shared tiện ích".to_string(), true),
                (
                    "02-01".to_string(),
                    "Tracer: đường chạy đầu".to_string(),
                    true
                ),
                ("05.2-32".to_string(), String::new(), true),
                ("01-01".to_string(), "Mô tả mẫu".to_string(), false),
                ("01-02".to_string(), "Chữ sau gạch".to_string(), false),
            ]
        );
        // Dòng plan trước mọi tiêu đề phase không thuộc phase nào.
        let early = parse_roadmap("- [x] 01-01: sớm\n### Phase 1: A\n");
        assert!(early.phases[0].roadmap_plans.is_empty());
    }

    #[test]
    fn roadmap_milestones_details_and_headings() {
        let text = "<details>\n<summary>✅ v1.0 MVP (Phases 1-2) - SHIPPED 2026-10-01</summary>\n\n### Phase 1: Một\n### Phase 2: Hai\n\n</details>\n\n### 🚧 v1.1 Xuất báo cáo (In Progress)\n\n#### Phase 3: Ba\n#### Phase 4: Bốn\n\n### Phase 9: Ngoài\n";
        let rs = parse_roadmap(text);
        let ms: Vec<(String, Option<String>)> = rs
            .phases
            .iter()
            .map(|p| (p.num.display(), p.milestone.clone()))
            .collect();
        assert_eq!(
            ms,
            [
                ("01".to_string(), Some("v1.0 MVP".to_string())),
                ("02".to_string(), Some("v1.0 MVP".to_string())),
                ("03".to_string(), Some("v1.1 Xuất báo cáo".to_string())),
                ("04".to_string(), Some("v1.1 Xuất báo cáo".to_string())),
                ("09".to_string(), Some("v1.1 Xuất báo cáo".to_string())),
            ]
        );
        // </details> xoá milestone đang đặt.
        let cleared = parse_roadmap("<details><summary>v2.0 Lớn - xong</summary>\n### Phase 1: A\n</details>\n### Phase 2: B\n");
        assert_eq!(cleared.phases[0].milestone.as_deref(), Some("v2.0 Lớn"));
        assert_eq!(cleared.phases[1].milestone, None);
        // Không có phần milestone nào: không phase nào có milestone.
        let none = parse_roadmap(
            "### Phase 1: A\n<details><summary>Chi tiết</summary>\n</details>\n### Phase 2: B\n",
        );
        assert!(none.phases.iter().all(|p| p.milestone.is_none()));
    }

    #[test]
    fn roadmap_heading_levels_and_inserted() {
        let text = "## Phase 1: sai cấp\n### Phase 2: Ba cấp\n#### Phase 2.1: Bốn cấp (INSERTED)\n##### Phase 3: năm cấp\n### Phase 05.10: Thập phân\n";
        let rs = parse_roadmap(text);
        let seen: Vec<(String, String)> = rs
            .phases
            .iter()
            .map(|p| (p.num.display(), p.name.clone()))
            .collect();
        assert_eq!(
            seen,
            [
                ("02".to_string(), "Ba cấp".to_string()),
                ("02.1".to_string(), "Bốn cấp".to_string()),
                ("05.10".to_string(), "Thập phân".to_string()),
            ]
        );
    }

    #[test]
    fn title_chain_roadmap_objective_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let phase = dir.path().join(".planning/phases/01-a");
        std::fs::create_dir_all(&phase).unwrap();
        std::fs::write(
            dir.path().join(".planning/ROADMAP.md"),
            "### Phase 1: A\n- [ ] 01-01-PLAN.md — Tiêu đề từ ROADMAP — wave 1\n- [ ] 01-02-PLAN.md\n- [ ] 01-03-PLAN.md\n",
        )
        .unwrap();
        let fm = "---\nwave: 1\ndepends_on: []\n---\n\n";
        std::fs::write(
            phase.join("01-01-PLAN.md"),
            format!("{fm}<objective>\nKhông dùng. Vì ROADMAP thắng.\n</objective>\n"),
        )
        .unwrap();
        std::fs::write(
            phase.join("01-02-PLAN.md"),
            format!(
                "{fm}<objective>\n\n  Ghi   luồng `CSV`\ttheo lô. Chi tiết khác.\n</objective>\n"
            ),
        )
        .unwrap();
        std::fs::write(
            phase.join("01-03-PLAN.md"),
            format!("{fm}không có mục tiêu\n"),
        )
        .unwrap();
        let facts = scan(dir.path(), &Caps::default(), far()).expect("scan");
        let titles: Vec<(String, String)> = facts
            .plans
            .iter()
            .map(|p| (p.id.clone(), p.title.clone()))
            .collect();
        assert_eq!(
            titles,
            [
                ("01-01".to_string(), "Tiêu đề từ ROADMAP".to_string()),
                ("01-02".to_string(), "Ghi luồng CSV theo lô".to_string()),
                ("01-03".to_string(), "01-03".to_string()),
            ]
        );
        let long = format!("{fm}<objective>\n{}\n", "x".repeat(300));
        assert_eq!(objective_title(&long).unwrap().chars().count(), 120);
        assert_eq!(objective_title("<objective>\n</objective>\n"), None);
        assert_eq!(objective_title("không có thẻ\n"), None);
    }

    const FM_OK: &str = "---\nwave: 1\ndepends_on: []\n---\n";

    #[test]
    fn read_symlink_and_dir_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let real = dir.path().join("that.md");
        std::fs::write(&real, "noi dung").unwrap();
        let link = dir.path().join("link.md");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert_eq!(open_regular(&link).err(), Some(ReadError::NotRegular));
        assert_eq!(open_regular(dir.path()).err(), Some(ReadError::NotRegular));
        assert_eq!(
            open_regular(&dir.path().join("khong-co.md")).err(),
            Some(ReadError::Missing)
        );
        assert!(open_regular(&real).is_ok());
    }

    #[test]
    fn read_fifo_refused_without_blocking() {
        use std::os::unix::ffi::OsStrExt;
        let dir = tree(2);
        let fifo = dir.path().join(".planning/phases/01-a/01-03-PLAN.md");
        let c_path = std::ffi::CString::new(fifo.as_os_str().as_bytes()).expect("cstring");
        // SAFETY: `c_path` là chuỗi C hợp lệ, sống suốt lời gọi.
        let rc = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
        assert_eq!(rc, 0, "mkfifo");
        let began = Instant::now();
        assert_eq!(read_head(&fifo, 100), Err(ReadError::NotRegular));
        let facts = scan(dir.path(), &Caps::default(), far()).expect("scan");
        assert!(
            began.elapsed() < Duration::from_secs(1),
            "scan bị treo vì FIFO"
        );
        let ids: Vec<&str> = facts.plans.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["01-01", "01-02"]);
        let on_fifo: Vec<&Warning> = facts
            .warnings
            .iter()
            .filter(|w| w.path == ".planning/phases/01-a/01-03-PLAN.md")
            .collect();
        assert_eq!(on_fifo.len(), 1);
        assert!(on_fifo[0].message.contains("không phải file thường"));
    }

    #[test]
    fn read_head_caps_at_32k() {
        let dir = tree(2);
        let phase = dir.path().join(".planning/phases/01-a");
        // 40 KiB, front matter đóng trong 2 KiB đầu: đọc được
        let filler = "x".repeat(40 * 1024);
        std::fs::write(
            phase.join("01-01-PLAN.md"),
            format!("---\nwave: 3\ndepends_on: [01-02]\n---\n{filler}\n"),
        )
        .unwrap();
        // dòng --- đóng nằm sau byte 32.768: lỗi front matter, hạng mục vẫn còn
        std::fs::write(
            phase.join("01-02-PLAN.md"),
            format!(
                "---\nwave: 3\ndepends_on: [01-01]\nghi_chu: {}\n---\n",
                "y".repeat(33_000)
            ),
        )
        .unwrap();
        let head = read_head(&phase.join("01-01-PLAN.md"), PLAN_READ_BYTES).unwrap();
        assert_eq!(head.len() as u64, PLAN_READ_BYTES);
        let facts = scan(dir.path(), &Caps::default(), far()).expect("scan");
        let p1 = &facts.plans[0];
        assert_eq!((p1.id.as_str(), p1.fm_wave), ("01-01", Some(3)));
        assert_eq!(p1.depends_on, ["01-02"]);
        let p2 = &facts.plans[1];
        assert_eq!(p2.id, "01-02");
        assert!(p2.depends_on.is_empty());
        assert!(facts.warnings.iter().any(|w| {
            w.path == ".planning/phases/01-a/01-02-PLAN.md"
                && w.message.starts_with("front matter lỗi")
        }));
    }

    #[test]
    fn read_whole_too_large() {
        let dir = tempfile::tempdir().expect("tempdir");
        let big = dir.path().join("lon.md");
        std::fs::write(&big, vec![b'a'; 1_048_577]).unwrap();
        assert_eq!(
            read_whole(&big, ROADMAP_MAX_BYTES),
            Err(ReadError::TooLarge)
        );
        let exact = dir.path().join("vua.md");
        std::fs::write(&exact, vec![b'a'; 1_048_576]).unwrap();
        assert_eq!(
            read_whole(&exact, ROADMAP_MAX_BYTES).unwrap().len(),
            1_048_576
        );
    }

    #[test]
    fn caps_plan_files_truncate() {
        let dir = tree(5);
        let caps = Caps {
            max_plan_files: 3,
            ..Caps::default()
        };
        let facts = scan(dir.path(), &caps, far()).expect("scan");
        assert_eq!(facts.plans.len(), 3);
        assert!(facts.truncated);
        assert!(facts
            .warnings
            .iter()
            .any(|w| w.message == "dừng quét: quá 3 file plan"));
    }

    #[test]
    fn caps_phase_dirs_truncate() {
        let dir = tree(1);
        let second = dir.path().join(".planning/phases/02-b");
        std::fs::create_dir_all(&second).unwrap();
        std::fs::write(second.join("02-01-PLAN.md"), FM_OK).unwrap();
        let caps = Caps {
            max_phase_dirs: 1,
            ..Caps::default()
        };
        let facts = scan(dir.path(), &caps, far()).expect("scan");
        let ids: Vec<&str> = facts.plans.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["01-01"]);
        assert!(facts.truncated);
        assert!(facts
            .warnings
            .iter()
            .any(|w| w.message == "dừng quét: quá 1 thư mục phase"));
    }

    #[test]
    fn caps_deadline_truncates() {
        let dir = tree(3);
        let facts = scan(dir.path(), &Caps::default(), Instant::now()).expect("scan");
        assert!(facts.plans.is_empty());
        assert!(facts.truncated);
        assert!(facts
            .warnings
            .iter()
            .any(|w| w.message.starts_with("dừng quét: quá thời gian đọc 2 giây")));
    }

    #[test]
    fn roadmap_long_line_skipped() {
        let long = format!("### Phase 9: {}", "n".repeat(5_000));
        let text = format!("### Phase 1: Một\n{long}\n### Phase 2: Hai\n");
        let rs = parse_roadmap(&text);
        let seen: Vec<String> = rs.phases.iter().map(|p| p.num.display()).collect();
        assert_eq!(seen, ["01", "02"]);
        assert_eq!(rs.warnings.len(), 1);
        assert_eq!(rs.warnings[0].message, "bỏ qua dòng 2: dài hơn 4096 ký tự");
        assert_eq!(rs.warnings[0].path, ".planning/ROADMAP.md");
    }

    #[test]
    fn fm_list_caps() {
        let many = |n: usize, p: &str| {
            (0..n)
                .map(|i| format!("{p}-{i:02}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let text = format!(
            "---\ndepends_on: [{}]\nrequirements: [{}]\n---\n",
            many(70, "01"),
            many(60, "REQ")
        );
        let fm = parse_plan_front_matter(&text).unwrap();
        assert_eq!(fm.depends_on.len(), 64);
        assert_eq!(fm.requirements.len(), 50);
        assert_eq!(fm.depends_on[63], "01-63");
        assert_eq!(fm.requirements[49], "REQ-49");
        assert_eq!(
            fm.notes,
            [
                "depends_on có hơn 64 mục — chỉ dùng 64 mục đầu",
                "requirements có hơn 50 mục — chỉ dùng 50 mục đầu"
            ]
        );
    }

    #[test]
    fn title_control_chars_and_cap() {
        let dir = tree(3);
        let phase = dir.path().join(".planning/phases/01-a");
        std::fs::write(
            dir.path().join(".planning/ROADMAP.md"),
            format!(
                "### Phase 1: A\n- [ ] 01-01-PLAN.md — chữ\u{7}điều\tkhiển\n- [ ] 01-02-PLAN.md — {}\n",
                "y".repeat(300)
            ),
        )
        .unwrap();
        std::fs::write(
            phase.join("01-03-PLAN.md"),
            format!("{FM_OK}\n<objective>\nGhi\u{1}luồng. Câu sau.\n</objective>\n"),
        )
        .unwrap();
        let facts = scan(dir.path(), &Caps::default(), far()).expect("scan");
        assert_eq!(facts.plans[0].title, "chữ điều khiển");
        assert_eq!(facts.plans[1].title.chars().count(), 200);
        assert_eq!(facts.plans[2].title, "Ghi luồng");
        assert!(facts
            .plans
            .iter()
            .all(|p| !p.title.chars().any(|c| c.is_control())));
    }

    #[test]
    fn scan_phase_dir_symlink_ignored() {
        let dir = tree(1);
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("09-01-PLAN.md"), FM_OK).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join(".planning/phases/09-out"))
            .unwrap();
        let facts = scan(dir.path(), &Caps::default(), far()).expect("scan");
        let ids: Vec<&str> = facts.plans.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["01-01"]);
        assert!(facts.warnings.iter().any(|w| {
            w.path == ".planning/phases/09-out"
                && w.message == "bỏ qua thư mục: symlink hoặc không phải thư mục thật"
        }));
    }

    #[test]
    fn scan_summary_symlink_counts_absent() {
        let dir = tree(2);
        let phase = dir.path().join(".planning/phases/01-a");
        let outside = tempfile::tempdir().expect("outside");
        let real = outside.path().join("tom-tat.md");
        std::fs::write(&real, "# xong\n").unwrap();
        std::os::unix::fs::symlink(&real, phase.join("01-01-SUMMARY.md")).unwrap();
        std::fs::write(phase.join("01-02-SUMMARY.md"), "# xong\n").unwrap();
        let facts = scan(dir.path(), &Caps::default(), far()).expect("scan");
        let done: Vec<(&str, bool)> = facts
            .plans
            .iter()
            .map(|p| (p.id.as_str(), p.done))
            .collect();
        assert_eq!(done, [("01-01", false), ("01-02", true)]);
    }

    #[test]
    fn fm_bom_and_crlf() {
        let plain = "---\nphase: 01-x\nplan: 01\nwave: 2\ndepends_on: [01-01, \"01-02\"]\nrequirements: [A-1]\n---\n\n<objective>x</objective>\n";
        let crlf = format!("\u{feff}{}", plain.replace('\n', "\r\n"));
        let a = parse_plan_front_matter(plain).unwrap();
        let b = parse_plan_front_matter(&crlf).unwrap();
        assert_eq!(a, b);
        assert_eq!(b.wave, Some(2));
        assert_eq!(b.depends_on, ["01-01", "01-02"]);
        assert_eq!(b.phase.as_deref(), Some("01-x"));
    }

    #[test]
    fn fm_block_list() {
        let text = "---\ndepends_on:\n  - 01-01\n  - \"01-02\"\nrequirements:\n- 'R-1'\n- R-2 # ghi chú\nwave: 3\n---\n";
        let fm = parse_plan_front_matter(text).unwrap();
        assert_eq!(fm.depends_on, ["01-01", "01-02"]);
        assert_eq!(fm.requirements, ["R-1", "R-2"]);
        assert_eq!(fm.wave, Some(3));
        // khoá trống không có mục nào theo sau là danh sách rỗng
        let empty = parse_plan_front_matter("---\ndepends_on:\nwave: 1\n---\n").unwrap();
        assert!(empty.depends_on.is_empty());
        assert_eq!(empty.wave, Some(1));
    }

    #[test]
    fn fm_single_quotes_and_no_space() {
        let q = parse_plan_front_matter("---\ndepends_on: ['x-01']\n---\n").unwrap();
        assert_eq!(q.depends_on, ["x-01"]);
        let tight = parse_plan_front_matter("---\nrequirements: [SPK-01,SPK-02]\n---\n").unwrap();
        assert_eq!(tight.requirements, ["SPK-01", "SPK-02"]);
        let comma = parse_plan_front_matter("---\ndepends_on: [\"a, b\", 'c']\n---\n").unwrap();
        assert_eq!(comma.depends_on, ["a, b", "c"]);
    }

    #[test]
    fn fm_inline_list_over_lines() {
        let two = parse_plan_front_matter("---\ndepends_on: [01-01,\n  01-02]\n---\n").unwrap();
        assert_eq!(two.depends_on, ["01-01", "01-02"]);
        let bad = |closing_line: usize| {
            let mut text = String::from("---\ndepends_on: [01-01,\n");
            for i in 1..=closing_line {
                if i == closing_line {
                    text.push_str("  01-09]\n");
                } else {
                    text.push_str("  01-02,\n");
                }
            }
            text.push_str("---\n");
            parse_plan_front_matter(&text)
        };
        assert!(bad(20).is_ok());
        assert_eq!(bad(21), Err(FmError::BadList("depends_on".to_string())));
        assert_eq!(
            parse_plan_front_matter("---\ndepends_on: [01-01\nwave: 2\n---\n"),
            Err(FmError::BadList("depends_on".to_string()))
        );
    }

    #[test]
    fn fm_comments_and_null() {
        let c = parse_plan_front_matter("---\ndepends_on: [01-01] # ghi chú\nwave: 2 # hai\n---\n")
            .unwrap();
        assert_eq!(c.depends_on, ["01-01"]);
        assert_eq!(c.wave, Some(2));
        let n = parse_plan_front_matter("---\ndepends_on: null\nrequirements: ~\n---\n").unwrap();
        assert!(n.depends_on.is_empty());
        assert!(n.requirements.is_empty());
        let hash_in_quotes = parse_plan_front_matter("---\ndepends_on: [\"a #b\"]\n---\n").unwrap();
        assert_eq!(hash_in_quotes.depends_on, ["a #b"]);
    }

    #[test]
    fn fm_nested_blocks_ignored() {
        let text = "---\nphase: 01-x\nwave: 1\ndepends_on: [01-01]\nmust_haves:\n  truths:\n    - \"dòng có: dấu hai chấm\"\n    - \"--- trong ngoặc\"\n  depends_on: [9-9]\n  wave: 7\nestimate:\n  tokens: 100\n  wave: 8\nuser_setup:\n  - service: \"x: y\"\n    depends_on: [8-8]\nrequirements: [A-1]\n---\n";
        let fm = parse_plan_front_matter(text).unwrap();
        assert_eq!(fm.wave, Some(1));
        assert_eq!(fm.depends_on, ["01-01"]);
        assert_eq!(fm.requirements, ["A-1"]);
        assert_eq!(fm.phase.as_deref(), Some("01-x"));
        assert!(fm.notes.is_empty());
    }

    #[test]
    fn fm_missing_and_unterminated() {
        assert_eq!(
            parse_plan_front_matter("wave: 1\n---\n"),
            Err(FmError::NoFrontMatter)
        );
        assert_eq!(parse_plan_front_matter(""), Err(FmError::NoFrontMatter));
        assert_eq!(
            parse_plan_front_matter("---\nwave: 1\ndepends_on: []\n"),
            Err(FmError::Unterminated)
        );
    }

    #[test]
    fn fm_wave_not_integer() {
        let fm = parse_plan_front_matter("---\nwave: 2a\ndepends_on: [01-01]\n---\n").unwrap();
        assert_eq!(fm.wave, None);
        assert_eq!(fm.notes, ["wave không phải số nguyên: 2a"]);
        assert_eq!(fm.depends_on, ["01-01"]);
    }

    #[test]
    fn fm_duplicate_key_first_wins() {
        let fm = parse_plan_front_matter(
            "---\nwave: 1\nwave: 9\ndepends_on: [01-01]\ndepends_on: [01-02]\n---\n",
        )
        .unwrap();
        assert_eq!(fm.wave, Some(1));
        assert_eq!(fm.depends_on, ["01-01"]);
    }

    #[test]
    fn state_quoted_bare_body() {
        let phase = |t: &str| parse_state(t).map(|p| p.display());
        assert_eq!(
            phase("---\ncurrent_phase: \"05.5\"\n---\nPhase: 03\n").as_deref(),
            Some("05.5")
        );
        assert_eq!(phase("---\ncurrent_phase: 5\n---\n").as_deref(), Some("05"));
        assert_eq!(
            phase("---\nname: x\n---\n# State\n\nPhase: 05.5 — Work Board (INSERTED)\n").as_deref(),
            Some("05.5")
        );
        assert_eq!(phase("# State\nPhase: 2\nPlan: 1\n").as_deref(), Some("02"));
        assert_eq!(
            phase("---\ncurrent_phase: tam\n---\nPhase: 4 — x\n").as_deref(),
            Some("04")
        );
        assert_eq!(phase("---\nname: x\n---\nkhông có gì\n"), None);
        assert_eq!(phase("Phase: tam\n"), None);
        assert_eq!(phase("\u{0}\u{1}\u{2}"), None);
    }

    #[test]
    fn scan_warns_once_for_dir_missing_from_roadmap() {
        let dir = tree(2);
        let other = dir.path().join(".planning/phases/07-ngoai");
        std::fs::create_dir_all(&other).unwrap();
        for n in ["07-01-PLAN.md", "07-02-PLAN.md"] {
            std::fs::write(other.join(n), "---\nwave: 1\ndepends_on: []\n---\n").unwrap();
        }
        let facts = scan(dir.path(), &Caps::default(), far()).expect("scan");
        assert_eq!(facts.plans.len(), 4);
        assert_eq!(facts.warnings.len(), 1);
        assert_eq!(facts.warnings[0].path, ".planning/phases/07-ngoai");
        let msg = &facts.warnings[0].message;
        assert!(msg.starts_with("thư mục phase 07-ngoai không có trong ROADMAP.md"));
        assert!(msg.ends_with("(vẫn hiện thành epic riêng)"));
    }
}
