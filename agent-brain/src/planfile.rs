use unicode_normalization::UnicodeNormalization;

pub const PLAN_TEMPLATE: &str = include_str!("../assets/templates/plan.md");
pub const QA_TEMPLATE: &str = include_str!("../assets/templates/qa-report.md");

fn sort_json(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut btree = std::collections::BTreeMap::new();
            for (k, val) in map {
                btree.insert(k.clone(), sort_json(val));
            }
            serde_json::to_value(btree).unwrap()
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sort_json).collect())
        }
        other => other.clone(),
    }
}

pub fn meta_line(meta: &serde_json::Value) -> String {
    let sorted = sort_json(meta);
    let dumped = serde_json::to_string(&sorted).unwrap_or_else(|_| "{}".to_string());
    let escaped = dumped.replace('>', "\\u003e");
    format!("<!-- brain-meta: {escaped} -->")
}

pub fn read_meta(text: &str) -> Option<serde_json::Value> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?s)<!--\s*brain-meta:\s*(.*?)\s*-->").unwrap());
    let caps = re.captures(text)?;
    let val: serde_json::Value = serde_json::from_str(&caps[1]).ok()?;
    if val.is_object() {
        Some(val)
    } else {
        None
    }
}

pub fn set_meta(text: &str, meta: &serde_json::Value) -> String {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?s)<!--\s*brain-meta:\s*.*?-->").unwrap());
    let ml = meta_line(meta);
    if let Some(mat) = re.find(text) {
        let mut s = String::with_capacity(text.len() + ml.len());
        s.push_str(&text[..mat.start()]);
        s.push_str(&ml);
        s.push_str(&text[mat.end()..]);
        return s;
    }

    let mut lines = Vec::new();
    let mut start = 0;
    for (i, c) in text.char_indices() {
        if c == '\n' {
            lines.push(&text[start..=i]);
            start = i + 1;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }

    if lines.is_empty() {
        return format!("{ml}\n");
    }

    let mut first_line = lines[0].to_string();
    if !first_line.ends_with('\n') {
        first_line.push('\n');
    }
    let rest = lines[1..].join("");
    format!("{first_line}{ml}\n{rest}")
}

pub fn render_plan_skeleton(task_title: &str, meta: &serde_json::Value) -> String {
    let raw_first_line = task_title.lines().next().unwrap_or(task_title);
    let title_100: String = raw_first_line.chars().take(100).collect();
    let norm_title: String = title_100.nfc().collect();
    let lines: Vec<&str> = PLAN_TEMPLATE.lines().collect();
    let first_line = lines
        .first()
        .copied()
        .unwrap_or("")
        .replace("<tên task>", &norm_title);
    let ml = meta_line(meta);
    let mut new_lines = vec![first_line, ml];
    if lines.len() > 1 {
        new_lines.extend(lines[1..].iter().map(|s| s.to_string()));
    }
    new_lines.join("\n") + "\n"
}

pub const PLAN_SECTIONS: [&str; 5] = [
    "Mục tiêu",
    "Thay đổi",
    "Không được đụng tới",
    "Cách kiểm tra",
    "Vướng mắc",
];
pub const QA_SECTIONS: [&str; 4] = [
    "Thay đổi có khớp plan không (từng mục)",
    "Sửa ngoài plan (nếu có)",
    "Rủi ro",
    "Kết luận",
];
pub const QA_VERDICTS: [&str; 2] = ["Đạt", "Cần sửa"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub file: String,
    pub where_: String,
    pub what: String,
    pub why: String,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub title: String,
    pub meta: Option<serde_json::Value>,
    pub sections: std::collections::BTreeMap<String, String>,
    pub changes: Vec<Change>,
}

pub fn posix_normpath(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let (initial_slashes, tail) = if !path.starts_with('/') {
        ("", path)
    } else if !path[1..].starts_with('/') || path.starts_with("///") {
        ("/", &path[1..])
    } else {
        ("//", &path[2..])
    };

    let mut comps: Vec<&str> = Vec::new();
    for comp in tail.split('/') {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp != ".."
            || (initial_slashes.is_empty() && comps.is_empty())
            || (!comps.is_empty() && comps.last() == Some(&".."))
        {
            comps.push(comp);
        } else if !comps.is_empty() {
            comps.pop();
        }
    }

    let joined = comps.join("/");
    let res = format!("{initial_slashes}{joined}");
    if res.is_empty() {
        ".".to_string()
    } else {
        res
    }
}

pub fn realpath_nonstrict(p: &std::path::Path) -> std::path::PathBuf {
    let abs_p = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/"))
            .join(p)
    };

    let mut cur = abs_p.clone();
    let mut tail = Vec::new();

    loop {
        if let Ok(canon) = cur.canonicalize() {
            let mut result = canon;
            for part in tail.into_iter().rev() {
                result.push(part);
            }
            return result;
        }

        #[cfg(unix)]
        {
            if cur.is_symlink() {
                if let Ok(target) = std::fs::read_link(&cur) {
                    let next = if target.is_absolute() {
                        target
                    } else {
                        cur.parent()
                            .unwrap_or(std::path::Path::new("/"))
                            .join(target)
                    };
                    let mut resolved = realpath_nonstrict(&next);
                    for part in tail.into_iter().rev() {
                        resolved.push(part);
                    }
                    return resolved;
                }
            }
        }

        if let Some(file_name) = cur.file_name() {
            tail.push(file_name.to_os_string());
            if let Some(parent) = cur.parent() {
                cur = parent.to_path_buf();
            } else {
                break;
            }
        } else {
            break;
        }
    }

    abs_p
}

pub fn normalize_plan_path(p: &str) -> String {
    let s = p
        .trim()
        .trim_matches('`')
        .trim_matches(|c| c == '\'' || c == '"')
        .trim();
    if s.is_empty() {
        return String::new();
    }
    let nfc: String = s.nfc().collect();
    let norm = posix_normpath(&nfc);
    if let Some(stripped) = norm.strip_prefix("./") {
        stripped.to_string()
    } else {
        norm
    }
}

fn split_table_row(line: &str) -> Option<Vec<String>> {
    let s = line.trim();
    if !s.starts_with('|') || s.chars().filter(|&c| c == '|').count() < 2 {
        return None;
    }
    let mut content = s;
    if content.starts_with('|') {
        content = &content[1..];
    }
    if content.ends_with('|') {
        content = &content[..content.len() - 1];
    }

    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next_c) = chars.peek() {
                if next_c == '|' {
                    chars.next();
                    current.push('|');
                    continue;
                }
            }
            current.push('\\');
        } else if c == '|' {
            cells.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(c);
        }
    }
    cells.push(current.trim().to_string());
    Some(cells)
}

fn is_separator_row(cells: &[String]) -> bool {
    static SEP_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = SEP_RE.get_or_init(|| regex::Regex::new(r"^:?-+:?$").unwrap());
    !cells.is_empty() && cells.iter().all(|c| re.is_match(c))
}

fn is_header_row(cells: &[String]) -> bool {
    cells.len() >= 2 && cells[0] == "#" && cells[1].eq_ignore_ascii_case("file")
}

pub fn section_text(text: &str, name: &str) -> String {
    let norm_name: String = name.nfc().collect();
    let norm_name = norm_name.trim();
    let mut in_section = false;
    let mut section_lines = Vec::new();

    for line in text.lines() {
        if line.starts_with("## ") || line.starts_with("# ") {
            let heading_raw = line.trim_start_matches('#').trim();
            let heading: String = heading_raw.nfc().collect();
            if in_section {
                break;
            }
            if heading.starts_with(norm_name) {
                in_section = true;
                continue;
            }
        } else if in_section {
            section_lines.push(line);
        }
    }

    if !in_section {
        return String::new();
    }
    section_lines.join("\n").trim().to_string()
}

pub fn parse_plan(text: &str) -> Plan {
    let meta = read_meta(text);

    // Title
    let mut title = String::new();
    for line in text.lines() {
        let line_clean = line.trim();
        let lower = line_clean.to_lowercase();
        if lower.starts_with("# plan:") {
            title = line_clean[7..].trim().nfc().collect();
            break;
        } else if line_clean.starts_with("# ") && !lower.starts_with("# qa:") {
            title = line_clean[2..].trim().nfc().collect();
            break;
        }
    }

    // Sections
    let mut sections = std::collections::BTreeMap::new();
    for line in text.lines() {
        if line.starts_with("## ") {
            let heading: String = line[3..].trim().nfc().collect();
            let mut matched = false;
            for &sec in &PLAN_SECTIONS {
                if heading.starts_with(sec) {
                    sections
                        .entry(sec.to_string())
                        .or_insert_with(|| section_text(text, sec));
                    matched = true;
                    break;
                }
            }
            if !matched {
                sections
                    .entry(heading.clone())
                    .or_insert_with(|| section_text(text, &heading));
            }
        }
    }

    // Changes from Thay đổi
    let mut changes = Vec::new();
    let thay_doi_text = sections.get("Thay đổi").map(|s| s.as_str()).unwrap_or("");
    for line in thay_doi_text.lines() {
        if let Some(cells) = split_table_row(line) {
            if is_header_row(&cells) || is_separator_row(&cells) {
                continue;
            }
            if cells.len() < 5 {
                continue;
            }
            let file_cell = cells[1].trim();
            let where_cell = cells[2].clone();
            let what_cell = cells[3].clone();
            let why_cell = cells[4].clone();

            if file_cell.is_empty() {
                changes.push(Change {
                    file: String::new(),
                    where_: where_cell,
                    what: what_cell,
                    why: why_cell,
                });
            } else {
                for sub_f in file_cell.split(',') {
                    let norm_f = normalize_plan_path(sub_f);
                    changes.push(Change {
                        file: norm_f,
                        where_: where_cell.clone(),
                        what: what_cell.clone(),
                        why: why_cell.clone(),
                    });
                }
            }
        }
    }

    Plan {
        title,
        meta,
        sections,
        changes,
    }
}

pub fn path_error(f: &str, root: Option<&std::path::Path>) -> Option<String> {
    if f.is_empty() {
        return None;
    }
    if f.starts_with('/') {
        Some(format!("absolute_path:{f}"))
    } else if f == "~" || f.starts_with("~/") {
        Some(format!("home_path:{f}"))
    } else if f == ".." || f.starts_with("../") || f.contains("/../") || f.ends_with("/..") {
        Some(format!("parent_traversal:{f}"))
    } else if f == ".git" || f.starts_with(".git/") || f.contains("/.git/") || f.ends_with("/.git")
    {
        Some(format!("git_dir:{f}"))
    } else if crate::stream::PROTOCOL_FILES
        .iter()
        .any(|proto| f == *proto || f.ends_with(&format!("/{proto}")))
    {
        Some(format!("protocol_file:{f}"))
    } else if normalize_plan_path(f) == "." {
        Some(format!("not_a_file:{f}"))
    } else if f
        .chars()
        .any(|c| c == '*' || c == '?' || c == '[' || c == ']')
    {
        Some(format!("glob_path:{f}"))
    } else if let Some(root_path) = root {
        let root_resolved = realpath_nonstrict(root_path);
        let target_resolved = realpath_nonstrict(&root_path.join(f));
        if target_resolved.strip_prefix(&root_resolved).is_err() {
            Some(format!("outside_root:{f}"))
        } else {
            None
        }
    } else {
        None
    }
}

pub fn validate_plan(plan: &Plan, root: Option<&std::path::Path>) -> Vec<String> {
    let mut errors = Vec::new();

    // 1. missing_title
    if plan.title.trim().is_empty() {
        errors.push("missing_title".to_string());
    }

    // 2. missing_section
    for sec in &PLAN_SECTIONS {
        if !plan.sections.contains_key(*sec) {
            errors.push(format!("missing_section:{sec}"));
        }
    }

    // Check table rows in Thay đổi
    let thay_doi_text = plan
        .sections
        .get("Thay đổi")
        .map(|s| s.as_str())
        .unwrap_or("");
    let mut data_rows_count = 0;
    let mut bad_rows = Vec::new();
    let mut empty_cells = Vec::new();

    for line in thay_doi_text.lines() {
        if let Some(cells) = split_table_row(line) {
            if is_header_row(&cells) || is_separator_row(&cells) {
                continue;
            }

            data_rows_count += 1;
            if cells.len() < 5 {
                bad_rows.push(data_rows_count);
                continue;
            }

            let file_cell = cells[1].trim();
            if file_cell.is_empty() {
                empty_cells.push(data_rows_count);
            } else {
                for sub_f in file_cell.split(',') {
                    if normalize_plan_path(sub_f).is_empty() {
                        empty_cells.push(data_rows_count);
                        break;
                    }
                }
            }
        }
    }

    for r in &bad_rows {
        errors.push(format!("bad_row:{r}"));
    }
    for r in &empty_cells {
        errors.push(format!("empty_file_cell:{r}"));
    }

    // 3. no_changes
    if data_rows_count == 0
        || (plan.changes.is_empty() && bad_rows.is_empty() && empty_cells.is_empty())
    {
        errors.push("no_changes".to_string());
    }

    // 4. File path errors for each change
    let mut seen_outside_root = std::collections::HashSet::new();
    for change in &plan.changes {
        let f = &change.file;
        if f.is_empty() {
            continue;
        }
        if let Some(err) = path_error(f, root) {
            if err.starts_with("outside_root:") {
                if seen_outside_root.insert(f.clone()) {
                    errors.push(err);
                }
            } else {
                errors.push(err);
            }
        }
    }

    // 5. optional run grants (skill mode): absent sections add no error
    errors.extend(validate_grants(&run_grants(plan), root));

    errors
}

/// What an approved plan lets the Dev do beyond its files (skill mode, D-49). Every list is empty
/// for a plan without the four optional sections, and then nothing changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Grants {
    /// `## Skill sử dụng` — informational (name and path as written).
    pub skills: Vec<String>,
    /// `## Lệnh được chạy` — command patterns, as written between the backticks.
    pub commands: Vec<String>,
    /// `## Thư mục được đọc` — directory globs.
    pub read_dirs: Vec<String>,
    /// `## Thư mục được ghi` — directory globs.
    pub write_dirs: Vec<String>,
}

impl Grants {
    /// True when the plan declares nothing that changes the Dev's rights.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty() && self.read_dirs.is_empty() && self.write_dirs.is_empty()
    }
}

pub const GRANT_SECTION_SKILLS: &str = "Skill sử dụng";
pub const GRANT_SECTION_COMMANDS: &str = "Lệnh được chạy";
pub const GRANT_SECTION_READ: &str = "Thư mục được đọc";
pub const GRANT_SECTION_WRITE: &str = "Thư mục được ghi";
/// At most this many entries per grant section.
pub const MAX_GRANT_ITEMS: usize = 20;

fn grant_section<'a>(plan: &'a Plan, name: &str) -> &'a str {
    plan.sections
        .iter()
        .find(|(k, _)| k.starts_with(name))
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

/// The backticked value of each bullet line (`- ` or `* `); HTML comments are dropped and a
/// bullet without a backticked value grants nothing. For skills the text after the value is kept.
fn grant_items(body: &str, keep_rest: bool) -> Vec<String> {
    static COMMENT_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = COMMENT_RE.get_or_init(|| regex::Regex::new(r"(?s)<!--.*?-->").unwrap());
    let body = re.replace_all(body, "");
    let mut out = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) else {
            continue;
        };
        let rest = rest.trim();
        let Some(open) = rest.find('`') else {
            continue;
        };
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else {
            continue;
        };
        let value: String = after[..close].trim().nfc().collect();
        if value.is_empty() {
            continue;
        }
        if keep_rest {
            let tail = after[close + 1..].trim();
            if tail.is_empty() {
                out.push(value);
            } else {
                out.push(format!("{value} {tail}"));
            }
        } else {
            out.push(value);
        }
    }
    out
}

/// Reads the four optional grant sections of a plan.
pub fn run_grants(plan: &Plan) -> Grants {
    Grants {
        skills: grant_items(grant_section(plan, GRANT_SECTION_SKILLS), true),
        commands: grant_items(grant_section(plan, GRANT_SECTION_COMMANDS), false),
        read_dirs: grant_items(grant_section(plan, GRANT_SECTION_READ), false),
        write_dirs: grant_items(grant_section(plan, GRANT_SECTION_WRITE), false),
    }
}

/// Characters a granted command may never hold (shell constructs).
const GRANT_COMMAND_FORBIDDEN_CHARS: &[char] =
    &[';', '|', '&', '$', '(', ')', '<', '>', '`', '{', '}'];

/// Programs a granted command may never start with, whatever the plan says.
const FORBIDDEN_PROGRAMS: &[&str] = &[
    "sudo",
    "su",
    "doas",
    "ssh",
    "scp",
    "sftp",
    "nc",
    "ncat",
    "curl",
    "wget",
    "ftp",
    "telnet",
    "dd",
    "mkfs",
    "mount",
    "umount",
    "chmod",
    "chown",
    "eval",
    "exec",
    "env",
    "sh",
    "bash",
    "zsh",
    "dash",
    "fish",
    "xargs",
    "find",
    "nohup",
    "setsid",
    "docker",
    "kubectl",
    "systemctl",
    "crontab",
    "kill",
    "pkill",
    "killall",
    "rsync",
];

/// Interpreters: a granted pattern must name a script (not `-c …` and not a bare `*`).
const INTERPRETER_PROGRAMS: &[&str] = &[
    "python", "python3", "node", "perl", "ruby", "php", "deno", "bun", "npx", "npm", "pip", "pip3",
    "lua", "awk", "sed",
];

/// True when the (lowercase) program basename is an interpreter that runs a script.
fn is_interpreter_base(base: &str) -> bool {
    INTERPRETER_PROGRAMS.contains(&base) || base.starts_with("python") || base.starts_with("node")
}

/// The words of a command that are EXECUTED: the program, and for an interpreter the first word
/// after it that does not start with `-` (the script). Other words are data.
pub fn executed_words<'a>(words: &[&'a str]) -> Vec<&'a str> {
    let Some(first) = words.first() else {
        return Vec::new();
    };
    let mut out = vec![*first];
    if is_interpreter_base(&program_basename(first)) {
        if let Some(script) = words[1..].iter().find(|w| !w.starts_with('-')) {
            out.push(*script);
        }
    }
    out
}

fn program_basename(program: &str) -> String {
    let base = program.rsplit('/').next().unwrap_or(program);
    base.to_lowercase()
}

/// Why `program args…` is never allowed (shared by the plan check and the guard hook); `None`
/// when the program is not on the hard denylist. `args` are the words after the program.
pub fn is_forbidden_program(program: &str, args: &[&str]) -> Option<String> {
    let base = program_basename(program);
    if FORBIDDEN_PROGRAMS.contains(&base.as_str()) {
        return Some(format!("chương trình {base} bị cấm"));
    }
    if base == "rm" {
        let risky = args.iter().any(|a| {
            *a == "--recursive"
                || *a == "--force"
                || (a.starts_with('-')
                    && !a.starts_with("--")
                    && a[1..].chars().any(|c| c == 'r' || c == 'R' || c == 'f'))
        });
        if risky {
            return Some("rm kèm -r/-R/-f bị cấm".to_string());
        }
    }
    None
}

/// Why a granted command pattern is refused, `None` when it is acceptable. The pattern is one
/// command; `*` may only be the whole last word.
pub fn grant_command_error(pattern: &str) -> Option<String> {
    let p = pattern.trim();
    if p.is_empty() {
        return Some("lệnh trống".to_string());
    }
    if p.contains('\n') || p.contains('\r') || p.chars().any(|c| c.is_control() && c != '\t') {
        return Some("lệnh có xuống dòng hoặc ký tự điều khiển".to_string());
    }
    if let Some(c) = p
        .chars()
        .find(|c| GRANT_COMMAND_FORBIDDEN_CHARS.contains(c))
    {
        return Some(format!("lệnh có ký tự cấm ({c})"));
    }
    let words: Vec<&str> = p.split_whitespace().collect();
    for (i, w) in words.iter().enumerate() {
        if w.contains('*') && !(*w == "*" && i + 1 == words.len() && i > 0) {
            return Some(
                "`*` chỉ được đứng riêng làm từ cuối cùng (ví dụ `python scripts/x.py *`); không viết `*.srt` hay `file*` — hãy liệt kê đúng tên file hoặc dùng `*` đứng riêng"
                    .to_string(),
            );
        }
        if w.contains('?') || w.contains('[') || w.contains(']') || w.contains('\\') {
            return Some("lệnh có ký tự đại diện hoặc dấu \\".to_string());
        }
        if w.contains("..") && w.split('/').any(|c| c == "..") {
            return Some("lệnh có `..`".to_string());
        }
        if w.starts_with('/') || w.starts_with('~') || w.contains("=/") || w.contains("=~") {
            return Some("lệnh có đường dẫn tuyệt đối hoặc bắt đầu bằng ~".to_string());
        }
    }
    let program = words[0];
    if program.starts_with('-') || program.contains('=') {
        return Some("từ đầu tiên phải là tên chương trình".to_string());
    }
    let args: Vec<&str> = words[1..].to_vec();
    if let Some(reason) = is_forbidden_program(program, &args) {
        return Some(reason);
    }
    let base = program_basename(program);
    if base == "git" {
        let no_star = words
            .iter()
            .filter(|w| **w != "*")
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        if let Err(reason) = crate::guard::bash_command_check(&no_star) {
            return Some(reason);
        }
        if !words.get(1).is_some_and(|w| !w.starts_with('-')) {
            return Some("git cần lệnh con rõ ràng".to_string());
        }
    }
    if is_interpreter_base(&base) {
        match words.get(1) {
            None => return Some("trình thông dịch phải kèm đường dẫn script".to_string()),
            Some(w) if *w == "*" || w.starts_with('-') => {
                return Some(
                    "trình thông dịch phải kèm đường dẫn script, không `-c`/`*`".to_string(),
                )
            }
            _ => {}
        }
    }
    None
}

/// The directory a grant glob stands for: the glob without a trailing `/**`, `**` or `/`. Only
/// a trailing `/**` may hold `*`.
pub fn grant_dir_base(pattern: &str) -> String {
    let p = pattern.trim();
    let p = p.strip_suffix("**").unwrap_or(p);
    let p = p.trim_end_matches('/');
    normalize_plan_path(p)
}

/// Why a directory grant is refused, `None` when acceptable. Applies to read and write dirs.
pub fn grant_dir_error(pattern: &str, root: Option<&std::path::Path>) -> Option<String> {
    let p = pattern.trim();
    if p.is_empty() {
        return Some("thư mục trống".to_string());
    }
    if p.chars().any(|c| c.is_control()) {
        return Some("thư mục có ký tự điều khiển".to_string());
    }
    if p.starts_with('/') {
        return Some("đường dẫn tuyệt đối".to_string());
    }
    if p.starts_with('~') {
        return Some("đường dẫn bắt đầu bằng ~".to_string());
    }
    if p.split('/').any(|c| c == "..") {
        return Some("đường dẫn có `..`".to_string());
    }
    let stem = p.strip_suffix("**").unwrap_or(p);
    if stem.contains(['*', '?', '[', ']', '\\']) {
        return Some("chỉ cho phép `**` ở cuối (dạng `thư-mục/**`)".to_string());
    }
    if p.ends_with("**") && !(stem.is_empty() || stem.ends_with('/')) {
        return Some("`**` phải đứng sau dấu `/`".to_string());
    }
    let base = grant_dir_base(p);
    if base.is_empty() || base == "." {
        return Some("không được cấp cả gốc dự án".to_string());
    }
    let lower = base.to_lowercase();
    for c in lower.split('/') {
        if c == ".git" || c == ".planning" || c == ".claude" || c.starts_with(".env") {
            return Some(format!("thư mục nhạy cảm ({c})"));
        }
    }
    if crate::report::sensitive_path(&base) || crate::report::sensitive_path(&format!("{base}/x")) {
        return Some("đường dẫn nhạy cảm".to_string());
    }
    if let Some(r) = root {
        let root_resolved = realpath_nonstrict(r);
        let target = realpath_nonstrict(&r.join(&base));
        if target.strip_prefix(&root_resolved).is_err() {
            return Some("thư mục nằm ngoài dự án".to_string());
        }
    }
    None
}

/// Vietnamese errors for the grant sections of a plan (empty when it has none or all are fine).
pub fn validate_grants(g: &Grants, root: Option<&std::path::Path>) -> Vec<String> {
    let mut errors = Vec::new();
    for (name, list) in [
        (GRANT_SECTION_COMMANDS, &g.commands),
        (GRANT_SECTION_READ, &g.read_dirs),
        (GRANT_SECTION_WRITE, &g.write_dirs),
    ] {
        if list.len() > MAX_GRANT_ITEMS {
            errors.push(format!(
                "Mục \"{name}\" có {} dòng — tối đa {MAX_GRANT_ITEMS}",
                list.len()
            ));
        }
    }
    for c in &g.commands {
        if let Some(e) = grant_command_error(c) {
            errors.push(format!("Lệnh được chạy `{c}` bị từ chối: {e}"));
        }
    }
    for d in &g.read_dirs {
        if let Some(e) = grant_dir_error(d, root) {
            errors.push(format!("Thư mục được đọc `{d}` bị từ chối: {e}"));
        }
    }
    for d in &g.write_dirs {
        if let Some(e) = grant_dir_error(d, root) {
            errors.push(format!("Thư mục được ghi `{d}` bị từ chối: {e}"));
        }
    }
    // a command may not run a file the Dev can write itself
    let write_bases: Vec<String> = g
        .write_dirs
        .iter()
        .filter(|d| grant_dir_error(d, None).is_none())
        .map(|d| grant_dir_base(d))
        .collect();
    for c in &g.commands {
        if grant_command_error(c).is_some() {
            continue;
        }
        let words: Vec<&str> = c.split_whitespace().collect();
        for w in executed_words(&words) {
            if w == "*" || w.starts_with('-') {
                continue;
            }
            let n = normalize_plan_path(w);
            if write_bases
                .iter()
                .any(|b| n == *b || n.starts_with(&format!("{b}/")))
            {
                errors.push(format!(
                    "Lệnh được chạy `{c}` chạy file nằm trong thư mục được ghi — Dev có thể sửa file đó trước khi chạy"
                ));
                break;
            }
        }
    }
    errors
}

pub fn plan_files(plan: &Plan) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for c in &plan.changes {
        if !c.file.is_empty() && seen.insert(c.file.clone()) {
            result.push(c.file.clone());
        }
    }
    result
}

pub fn has_blockers(plan: &Plan) -> bool {
    static COMMENT_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = COMMENT_RE.get_or_init(|| regex::Regex::new(r"(?s)<!--.*?-->").unwrap());
    let body = plan
        .sections
        .get("Vướng mắc")
        .map(|s| s.as_str())
        .unwrap_or("");
    let no_comments = re.replace_all(body, "");
    !no_comments.trim().is_empty()
}

pub fn render_qa_skeleton(task_title: &str, meta: &serde_json::Value) -> String {
    let raw_first_line = task_title.lines().next().unwrap_or(task_title);
    let title_100: String = raw_first_line.chars().take(100).collect();
    let norm_title: String = title_100.nfc().collect();
    let lines: Vec<&str> = QA_TEMPLATE.lines().collect();
    let first_line = lines
        .first()
        .copied()
        .unwrap_or("")
        .replace("<tên task>", &norm_title);
    let ml = meta_line(meta);
    let mut new_lines = vec![first_line, ml];
    if lines.len() > 1 {
        new_lines.extend(lines[1..].iter().map(|s| s.to_string()));
    }
    new_lines.join("\n") + "\n"
}

pub fn parse_qa_verdict(text: &str) -> Option<&'static str> {
    static BULLET_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let bullet_re =
        BULLET_RE.get_or_init(|| regex::Regex::new(r"^[>\s*+-]+(?:\d+\.)?\s*").unwrap());

    static PREFIX_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let prefix_re =
        PREFIX_RE.get_or_init(|| regex::Regex::new(r"(?i)^(?:kết luận|ket luan)\s*:\s*").unwrap());

    let kt_body = section_text(text, "Kết luận");
    if kt_body.is_empty() {
        return None;
    }

    let mut found_verdicts = std::collections::BTreeSet::new();
    for raw_line in kt_body.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let after_bullet = bullet_re.replace(trimmed, "");
        let after_prefix = prefix_re.replace(&after_bullet, "");
        let cleaned = after_prefix
            .trim_matches(|c: char| matches!(c, '`' | '*' | '"' | '\'' | '_' | ' ' | '\t'));
        let norm: String = cleaned.nfc().collect();
        let lower = norm.to_lowercase();
        if lower == "đạt" {
            found_verdicts.insert("Đạt");
        } else if lower == "cần sửa" {
            found_verdicts.insert("Cần sửa");
        }
    }

    if found_verdicts.len() == 1 {
        found_verdicts.into_iter().next()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_error_codes() {
        assert_eq!(
            path_error("/etc/x", None),
            Some("absolute_path:/etc/x".to_string())
        );
        assert_eq!(path_error("~/x", None), Some("home_path:~/x".to_string()));
        assert_eq!(
            path_error("../x", None),
            Some("parent_traversal:../x".to_string())
        );
        assert_eq!(
            path_error("a/../../x", None),
            Some("parent_traversal:a/../../x".to_string())
        );
        assert_eq!(
            path_error(".git/config", None),
            Some("git_dir:.git/config".to_string())
        );
        assert_eq!(
            path_error(".plan-review.md", None),
            Some("protocol_file:.plan-review.md".to_string())
        );
        assert_eq!(
            path_error("sub/.code-review.md", None),
            Some("protocol_file:sub/.code-review.md".to_string())
        );
        assert_eq!(path_error(".", None), Some("not_a_file:.".to_string()));
        assert_eq!(
            path_error("src/*.py", None),
            Some("glob_path:src/*.py".to_string())
        );

        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();
        let outside_file = dir2.path().join("outside.py");
        std::fs::write(&outside_file, "print(1)").unwrap();

        #[cfg(unix)]
        {
            let symlink_path = dir1.path().join("out_symlink");
            std::os::unix::fs::symlink(dir2.path(), &symlink_path).unwrap();
            assert_eq!(
                path_error("out_symlink/outside.py", Some(dir1.path())),
                Some("outside_root:out_symlink/outside.py".to_string())
            );
        }

        assert_eq!(path_error("src/app.py", Some(dir1.path())), None);
    }

    #[test]
    fn test_posix_normpath_cases() {
        assert_eq!(posix_normpath("."), ".");
        assert_eq!(posix_normpath("./x"), "x");
        assert_eq!(posix_normpath("a/../b"), "b");
        assert_eq!(posix_normpath("a//b/"), "a/b");
        assert_eq!(posix_normpath("../a"), "../a");
    }

    #[test]
    fn test_realpath_nonstrict_missing_file_under_symlinked_dir() {
        let td = tempfile::tempdir().unwrap();
        let real_dir = td.path().join("real_dir");
        std::fs::create_dir_all(&real_dir).unwrap();
        let link_dir = td.path().join("link_dir");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

        let missing = link_dir.join("missing_file.txt");
        let resolved = realpath_nonstrict(&missing);
        let expected = real_dir.canonicalize().unwrap().join("missing_file.txt");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_table_splitter_escaped_pipe() {
        let row = r"| 1 | src/a\|b.py | toàn bộ | fix | ok |";
        let cells = split_table_row(row).expect("row should parse");
        assert_eq!(cells.len(), 5);
        assert_eq!(cells[1], "src/a|b.py");
    }

    // ---- skill mode (D-49): grants ----

    const PLAN_BASE: &str = "# Plan: t\n<!-- brain-meta: {\"task\":\"t\"} -->\n## Mục tiêu\nx\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | a.txt | - | x | y |\n## Không được đụng tới\n-\n## Cách kiểm tra\n-\n## Vướng mắc\n";

    fn plan_with(extra: &str) -> Plan {
        parse_plan(&format!("{PLAN_BASE}{extra}"))
    }

    #[test]
    fn grants_parse_all_four_sections() {
        let p = plan_with(
            "## Skill sử dụng\n- `fake-idea` (.agents/skills/fake-idea/SKILL.md)\n## Lệnh được chạy\n- `python scripts/create.py *`\n- bỏ qua dòng không có nháy\n## Thư mục được đọc\n- `.gsd/**`\n- `scripts/**`\n## Thư mục được ghi\n- `.gsd/phases/**`\n- `projects/**`\n",
        );
        let g = run_grants(&p);
        assert_eq!(
            g.skills,
            vec!["fake-idea (.agents/skills/fake-idea/SKILL.md)"]
        );
        assert_eq!(g.commands, vec!["python scripts/create.py *"]);
        assert_eq!(g.read_dirs, vec![".gsd/**", "scripts/**"]);
        assert_eq!(g.write_dirs, vec![".gsd/phases/**", "projects/**"]);
        assert!(!g.is_empty());
        assert!(
            validate_plan(&p, None).is_empty(),
            "{:?}",
            validate_plan(&p, None)
        );
    }

    #[test]
    fn old_plan_has_empty_grants_and_unchanged_validation() {
        let p = parse_plan(PLAN_BASE);
        assert!(run_grants(&p).is_empty());
        assert_eq!(run_grants(&p), Grants::default());
        assert!(validate_plan(&p, None).is_empty());
        // an empty grant section (template-like) grants nothing
        let p2 = plan_with("## Lệnh được chạy\n<!-- không có -->\n");
        assert!(run_grants(&p2).is_empty());
        assert!(validate_plan(&p2, None).is_empty());
    }

    #[test]
    fn grant_commands_refused() {
        for bad in [
            "python a.py; id",
            "python a.py | sh",
            "python a.py && id",
            "python a.py & id",
            "python a.py $(id)",
            "python a.py $HOME",
            "python a.py `id`",
            "python a.py < in",
            "python a.py > out",
            "python (a.py)",
            "python {a,b}.py",
            "python ../a.py",
            "python a/../b.py",
            "python /abs/a.py",
            "/usr/bin/python a.py",
            "~/bin/tool",
            "python ~/a.py",
            "python a.py\nid",
            "python *",
            "python -c *",
            "python3 -c print",
            "python a*.py",
            "node",
            "*",
            "sudo ls",
            "su root",
            "doas ls",
            "ssh h",
            "scp a b",
            "sftp h",
            "nc h 1",
            "ncat h 1",
            "curl http://x",
            "wget http://x",
            "ftp h",
            "telnet h",
            "dd if=a of=b",
            "mkfs x",
            "mount x",
            "umount x",
            "chmod 777 x",
            "chown a x",
            "eval x",
            "exec x",
            "env A=1 x",
            "sh run.sh",
            "bash run.sh",
            "zsh run.sh",
            "dash run.sh",
            "fish run.sh",
            "xargs ls",
            "find . -delete",
            "nohup x",
            "setsid x",
            "rm -rf x",
            "rm -r x",
            "rm -R x",
            "rm -f x",
            "rm --recursive x",
            "rm -fr x",
            "docker run x",
            "kubectl get",
            "systemctl stop x",
            "git push origin",
            "git remote add x y",
            "git config a b",
            "git -c a=b log",
            "A=1 python a.py",
        ] {
            assert!(grant_command_error(bad).is_some(), "should refuse {bad:?}");
        }
        for ok in [
            "python scripts/create_project.py *",
            "python3 scripts/a.py --flag",
            "node scripts/x.js *",
            "./scripts/run.sh *",
            "scripts/run.sh",
            "rm out/a.txt",
            "git status",
            "git add *",
            "make-thing --name x *",
        ] {
            assert_eq!(grant_command_error(ok), None, "should accept {ok:?}");
        }
    }

    #[test]
    fn grant_dirs_refused_and_accepted() {
        let td = tempfile::tempdir().unwrap();
        for bad in [
            "../x",
            "a/../b",
            "/etc",
            "~/x",
            "~",
            ".git",
            ".git/**",
            "a/.git/hooks",
            ".planning/**",
            ".claude/**",
            ".claude/skills/**",
            ".env",
            ".env.local/**",
            "a/.env",
            ".github/**",
            "key.pem",
            "**",
            "/**",
            ".",
            "./",
            "",
            "a/*/b",
            "a/**/b",
            "a*",
            "src/**x",
            ".vscode/**",
        ] {
            assert!(
                grant_dir_error(bad, Some(td.path())).is_some(),
                "should refuse {bad:?}"
            );
        }
        for ok in [
            ".gsd/**",
            "scripts/**",
            "projects",
            "projects/",
            "a/b/c/**",
            "out/./x/**",
        ] {
            assert_eq!(
                grant_dir_error(ok, Some(td.path())),
                None,
                "should accept {ok:?}"
            );
        }
        assert_eq!(grant_dir_base(".gsd/phases/**"), ".gsd/phases");
        assert_eq!(grant_dir_base("projects/"), "projects");
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            std::os::unix::fs::symlink(outside.path(), td.path().join("lnk")).unwrap();
            assert!(grant_dir_error("lnk/**", Some(td.path())).is_some());
        }
    }

    #[test]
    fn validate_plan_reports_vietnamese_grant_errors_and_limits() {
        let p = plan_with(
            "## Lệnh được chạy\n- `sudo ls`\n- `python a.py; id`\n## Thư mục được ghi\n- `.git/**`\n- `../x/**`\n## Thư mục được đọc\n- `/etc/**`\n",
        );
        let errs = validate_plan(&p, None);
        assert!(errs
            .iter()
            .any(|e| e.contains("Lệnh được chạy `sudo ls` bị từ chối")));
        assert!(errs.iter().any(|e| e.contains("ký tự cấm")));
        assert!(errs
            .iter()
            .any(|e| e.contains("Thư mục được ghi `.git/**`")));
        assert!(errs
            .iter()
            .any(|e| e.contains("Thư mục được ghi `../x/**`")));
        assert!(errs
            .iter()
            .any(|e| e.contains("Thư mục được đọc `/etc/**`")));

        let many: String = (0..21).map(|i| format!("- `d{i}/**`\n")).collect();
        let p2 = plan_with(&format!("## Thư mục được ghi\n{many}"));
        let errs2 = validate_plan(&p2, None);
        assert!(errs2.iter().any(|e| e.contains("tối đa 20")), "{errs2:?}");
        let many_c: String = (0..21).map(|i| format!("- `tool{i} *`\n")).collect();
        let p3 = plan_with(&format!("## Lệnh được chạy\n{many_c}"));
        assert!(validate_plan(&p3, None)
            .iter()
            .any(|e| e.contains("tối đa 20")));
    }

    #[test]
    fn command_may_not_run_a_file_in_a_write_dir() {
        let p = plan_with(
            "## Lệnh được chạy\n- `python out/run.py *`\n## Thư mục được ghi\n- `out/**`\n",
        );
        let errs = validate_plan(&p, None);
        assert!(
            errs.iter().any(|e| e.contains("thư mục được ghi")),
            "{errs:?}"
        );
        let p2 = plan_with(
            "## Lệnh được chạy\n- `python scripts/run.py *`\n## Thư mục được ghi\n- `out/**`\n",
        );
        assert!(validate_plan(&p2, None).is_empty());
    }

    fn cmd_errs(cmd: &str, write: &str) -> Vec<String> {
        let p = plan_with(&format!(
            "## Lệnh được chạy\n- `{cmd}`\n## Thư mục được ghi\n- `{write}`\n"
        ));
        validate_plan(&p, None)
    }

    #[test]
    fn only_the_executed_file_may_not_live_in_a_write_dir() {
        // (a) data argument inside the write dir is fine
        assert!(cmd_errs(
            "python scripts/parse_srt.py projects/x/input/a.srt",
            "projects/x/**"
        )
        .is_empty());
        // (e) data argument after a flag
        assert!(cmd_errs(
            "python scripts/a.py --out projects/x/o.txt",
            "projects/x/**"
        )
        .is_empty());
        // (b) the script itself in the write dir
        let b = cmd_errs("python projects/x/run.py", "projects/x/**");
        assert!(b.iter().any(|e| e.contains("thư mục được ghi")), "{b:?}");
        // (c) the program itself in the write dir
        let c = cmd_errs("projects/x/tool arg", "projects/x/**");
        assert!(c.iter().any(|e| e.contains("thư mục được ghi")), "{c:?}");
        // (d) -m / -c stay refused
        assert!(!cmd_errs("python -m mod", "projects/x/**").is_empty());
        assert!(!cmd_errs("python -c x", "projects/x/**").is_empty());
        // (f) glued star gets the new message
        let f = cmd_errs("python scripts/a.py subtitles.srt*", "projects/x/**");
        assert!(f.iter().any(|e| e.contains("đứng riêng")), "{f:?}");
        let f2 = cmd_errs("python scripts/a.py *.srt", "projects/x/**");
        assert!(f2.iter().any(|e| e.contains("đứng riêng")), "{f2:?}");
    }

    #[test]
    fn srt_like_plan_validates() {
        let p = plan_with(
            "## Skill sử dụng\n- `fake-idea` (.agents/skills/fake-idea/SKILL.md)\n## Lệnh được chạy\n- `python scripts/create_project.py ganh-nang-tren-vai`\n- `python scripts/parse_srt.py projects/ganh-nang-tren-vai/input/subtitles.srt`\n## Thư mục được đọc\n- `scripts/**`\n## Thư mục được ghi\n- `.gsd/phases/ganh-nang-tren-vai/**`\n- `projects/ganh-nang-tren-vai/**`\n",
        );
        let errs = validate_plan(&p, None);
        assert!(errs.is_empty(), "{errs:?}");
    }
}
