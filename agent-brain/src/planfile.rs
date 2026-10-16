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
