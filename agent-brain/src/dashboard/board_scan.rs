//! Bảng công việc: đọc file GSD của dự án (chỉ trong `.planning/`, không theo symlink,
//! có giới hạn kích thước, số file và thời gian) rồi đưa ra `Facts` cho `board::derive`.
//!
//! Mọi byte đi qua `open_regular`: `symlink_metadata` phải báo file thường, mở bằng
//! `O_NOFOLLOW | O_NONBLOCK`, kiểm tra lại trên handle đã mở, đọc qua `Read::take`.

use crate::dashboard::board::{clean_text, Facts, PhaseFact, PhaseNum, PlanFact, Warning};
use regex::Regex;
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

fn list_value(key: &str, raw: &str) -> Result<Vec<String>, FmError> {
    let v = strip_comment(raw).trim();
    if v.is_empty() || v == "null" || v == "~" {
        return Ok(Vec::new());
    }
    if let Some(rest) = v.strip_prefix('[') {
        return match rest.strip_suffix(']') {
            Some(inner) => Ok(split_list(inner)),
            None => Err(FmError::BadList(key.to_string())),
        };
    }
    // Giá trị đơn không có ngoặc: một phần tử.
    Ok(vec![unquote(v)])
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
    for line in block {
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
                let list = list_value(name, value)?;
                fm.depends_on = cap_list(name, list, MAX_DEPS_PER_PLAN, &mut fm.notes);
            }
            _ => {
                let list = list_value(name, value)?;
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

fn push_warning(warnings: &mut Vec<Warning>, path: &str, message: &str) {
    warnings.push(Warning {
        path: clean_text(path, WARNING_MAX_CHARS),
        message: clean_text(message, WARNING_MAX_CHARS),
    });
}

pub fn parse_roadmap(text: &str) -> RoadmapScan {
    let mut phases: Vec<PhaseFact> = Vec::new();
    let mut warnings: Vec<Warning> = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if line.chars().count() > LINE_MAX_CHARS {
            push_warning(
                &mut warnings,
                ROADMAP_REL,
                &format!("bỏ qua dòng {}: dài hơn {LINE_MAX_CHARS} ký tự", idx + 1),
            );
            continue;
        }
        let Some(caps) = phase_heading_re().captures(line) else {
            continue;
        };
        let Some(num) = caps.get(1).and_then(|m| PhaseNum::parse(m.as_str())) else {
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
            milestone: None,
            depends_on: Vec::new(),
            checkbox: None,
            roadmap_plans: Vec::new(),
        });
    }
    RoadmapScan { phases, warnings }
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
    let mut facts = Facts {
        phases: rs.phases,
        warnings: rs.warnings,
        ..Facts::default()
    };

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

            facts.plans.push(PlanFact {
                title: clean_text(&id, TITLE_MAX_CHARS),
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
}
