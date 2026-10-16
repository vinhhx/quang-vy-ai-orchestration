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

pub fn posix_normpath(_p: &str) -> String {
    unimplemented!("posix_normpath stub")
}

pub fn realpath_nonstrict(_p: &std::path::Path) -> std::path::PathBuf {
    unimplemented!("realpath_nonstrict stub")
}

pub fn normalize_plan_path(_p: &str) -> String {
    unimplemented!("normalize_plan_path stub")
}

pub fn section_text(_text: &str, _name: &str) -> String {
    unimplemented!("section_text stub")
}

pub fn parse_plan(_text: &str) -> Plan {
    unimplemented!("parse_plan stub")
}

pub fn validate_plan(_plan: &Plan, _root: Option<&std::path::Path>) -> Vec<String> {
    unimplemented!("validate_plan stub")
}

pub fn plan_files(_plan: &Plan) -> Vec<String> {
    unimplemented!("plan_files stub")
}

pub fn has_blockers(_plan: &Plan) -> bool {
    unimplemented!("has_blockers stub")
}

pub fn render_qa_skeleton(_task_title: &str, _meta: &serde_json::Value) -> String {
    unimplemented!("render_qa_skeleton stub")
}

pub fn parse_qa_verdict(_text: &str) -> Option<&'static str> {
    unimplemented!("parse_qa_verdict stub")
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Will test hand-rolled splitter
        unimplemented!("table splitter stub");
    }
}
