use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

const EDIT_TOOLS: &[&str] = &["Edit", "Write", "MultiEdit", "NotebookEdit"];
const PROTOCOL_FILES: &[&str] = &[".plan.md", ".qa-report.md"];

fn is_protocol_file(p: &str) -> bool {
    let basename = p.rsplit('/').next().unwrap_or(p);
    PROTOCOL_FILES.contains(&p) || PROTOCOL_FILES.contains(&basename)
}

fn posix_normpath(p: &str) -> String {
    if p.is_empty() {
        return ".".to_string();
    }
    let is_abs = p.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in p.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            if let Some(&top) = parts.last() {
                if top != ".." {
                    parts.pop();
                    continue;
                }
            }
            if !is_abs {
                parts.push("..");
            }
        } else {
            parts.push(part);
        }
    }
    if is_abs {
        if parts.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", parts.join("/"))
        }
    } else if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

pub fn normalize_tool_path(p: &str, root: Option<&Path>) -> String {
    let p_nfc: String = p.nfc().collect();
    let p_trimmed = p_nfc.trim();
    if p_trimmed.is_empty() {
        return String::new();
    }

    if let Some(r) = root {
        let r_str = r.to_string_lossy();
        let r_nfc: String = r_str.nfc().collect();
        let root_norm = posix_normpath(r_nfc.trim());
        let mut p_norm = posix_normpath(p_trimmed);

        if p_norm == root_norm {
            return ".".to_string();
        }
        let prefix = format!("{root_norm}/");
        if let Some(rest) = p_norm.strip_prefix(&prefix) {
            return rest.to_string();
        }
        if !p_norm.starts_with('/') {
            if let Some(rest) = p_norm.strip_prefix("./") {
                p_norm = rest.to_string();
            }
            return p_norm;
        }
        if let (Ok(p_path), Ok(r_path)) =
            (std::fs::canonicalize(p_trimmed), std::fs::canonicalize(r))
        {
            if let Ok(rel) = p_path.strip_prefix(&r_path) {
                return rel.to_string_lossy().to_string();
            }
        }
        p_norm
    } else {
        let mut p_norm = posix_normpath(p_trimmed);
        if !p_norm.starts_with('/') {
            if let Some(rest) = p_norm.strip_prefix("./") {
                p_norm = rest.to_string();
            }
            return p_norm;
        }
        p_norm
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub cache_creation_input_tokens: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageSource {
    Result,
    Partial,
    None,
}

impl UsageSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            UsageSource::Result => "result",
            UsageSource::Partial => "partial",
            UsageSource::None => "none",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamEvent {
    pub kind: String,
    pub raw: String,
    pub value: Option<serde_json::Value>,
}

pub fn classify(line: &[u8]) -> Option<StreamEvent> {
    let mut trimmed = line;
    while let Some(&last) = trimmed.last() {
        if last == b'\n' || last == b'\r' {
            trimmed = &trimmed[..trimmed.len() - 1];
        } else {
            break;
        }
    }
    let s = String::from_utf8_lossy(trimmed).to_string();
    if s.trim().is_empty() {
        return None;
    }

    match serde_json::from_str::<serde_json::Value>(&s) {
        Ok(serde_json::Value::Object(map)) => {
            let val = serde_json::Value::Object(map);
            let kind = if let Some(t) = val.get("type").and_then(|v| v.as_str()) {
                match t {
                    "system" => {
                        if let Some(st) = val.get("subtype").and_then(|v| v.as_str()) {
                            format!("system/{st}")
                        } else {
                            "system/?".to_string()
                        }
                    }
                    "assistant" => "assistant".to_string(),
                    "user" => "user".to_string(),
                    "result" => "result".to_string(),
                    "rate_limit_event" => "rate_limit_event".to_string(),
                    other => format!("unknown:{other}"),
                }
            } else {
                "unknown:".to_string()
            };
            Some(StreamEvent {
                kind,
                raw: s,
                value: Some(val),
            })
        }
        _ => Some(StreamEvent {
            kind: "bad_json".to_string(),
            raw: s,
            value: None,
        }),
    }
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub session_id: String,
    pub model: Option<String>,
    pub assistant_turns: i64,
    pub num_turns: Option<i64>,
    pub subtype: Option<String>,
    pub is_error: bool,
    pub result_text: Option<String>,
    pub usage: Usage,
    pub usage_source: UsageSource,
    pub cost_usd: f64,
    pub permission_denials: i64,
    pub total_lines: i64,
    pub bad_lines: i64,
    pub format_ok: bool,
    pub tool_counts: BTreeMap<String, i64>,
    pub reads: Vec<String>,
    pub edits: Vec<String>,
    pub first_edit_file: Option<String>,
    pub turns_before_first_edit: Option<i64>,
    pub reads_outside_plan: Option<Vec<String>>,
    plan_files: Option<Vec<String>>,
    root: Option<PathBuf>,
    seen_msg_ids: BTreeSet<String>,
    msg_usages: BTreeMap<String, Usage>,
    result_seen: bool,
}

impl Summary {
    pub fn new(plan_files: Option<Vec<String>>, root: Option<PathBuf>) -> Self {
        Self {
            session_id: String::new(),
            model: None,
            assistant_turns: 0,
            num_turns: None,
            subtype: None,
            is_error: false,
            result_text: None,
            usage: Usage::default(),
            usage_source: UsageSource::None,
            cost_usd: 0.0,
            permission_denials: 0,
            total_lines: 0,
            bad_lines: 0,
            format_ok: false,
            tool_counts: BTreeMap::new(),
            reads: Vec::new(),
            edits: Vec::new(),
            first_edit_file: None,
            turns_before_first_edit: None,
            reads_outside_plan: None,
            plan_files,
            root,
            seen_msg_ids: BTreeSet::new(),
            msg_usages: BTreeMap::new(),
            result_seen: false,
        }
    }

    pub fn feed(&mut self, line: &[u8]) -> Option<StreamEvent> {
        self.total_lines += 1;
        let ev = classify(line)?;
        if ev.kind == "bad_json" {
            self.bad_lines += 1;
            return Some(ev);
        }

        if let Some(ref val) = ev.value {
            let obj = match val.as_object() {
                Some(o) => o,
                None => return Some(ev),
            };

            if ev.kind == "system/init" {
                if self.session_id.is_empty() {
                    if let Some(s) = obj.get("session_id").and_then(|v| v.as_str()) {
                        self.session_id = s.to_string();
                    }
                }
                if self.model.is_none() {
                    if let Some(m) = obj.get("model").and_then(|v| v.as_str()) {
                        self.model = Some(m.to_string());
                    }
                }
            } else if ev.kind == "assistant" {
                let mut msg_id = None;
                let mut content_list: Option<&Vec<serde_json::Value>> = None;
                let mut msg_usage: Option<Usage> = None;

                if let Some(msg) = obj.get("message").and_then(|m| m.as_object()) {
                    msg_id = msg.get("id").and_then(|v| v.as_str());
                    content_list = msg.get("content").and_then(|v| v.as_array());
                    if self.model.is_none() {
                        if let Some(m) = msg.get("model").and_then(|v| v.as_str()) {
                            self.model = Some(m.to_string());
                        }
                    }
                    if let Some(u) = msg.get("usage").and_then(|v| v.as_object()) {
                        msg_usage = Some(Usage {
                            input_tokens: u
                                .get("input_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0),
                            output_tokens: u
                                .get("output_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0),
                            cache_read_input_tokens: u
                                .get("cache_read_input_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0),
                            cache_creation_input_tokens: u
                                .get("cache_creation_input_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0),
                        });
                    }
                } else if let Some(c) = obj.get("content").and_then(|v| v.as_array()) {
                    content_list = Some(c);
                }

                if msg_id.is_none() {
                    msg_id = obj.get("id").and_then(|v| v.as_str());
                }
                if msg_usage.is_none() {
                    if let Some(u) = obj.get("usage").and_then(|v| v.as_object()) {
                        msg_usage = Some(Usage {
                            input_tokens: u
                                .get("input_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0),
                            output_tokens: u
                                .get("output_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0),
                            cache_read_input_tokens: u
                                .get("cache_read_input_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0),
                            cache_creation_input_tokens: u
                                .get("cache_creation_input_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0),
                        });
                    }
                }

                if let Some(mid) = msg_id {
                    if !self.seen_msg_ids.contains(mid) {
                        self.seen_msg_ids.insert(mid.to_string());
                        self.assistant_turns += 1;
                    }
                    if let Some(u) = msg_usage {
                        self.msg_usages.insert(mid.to_string(), u);
                    }
                } else {
                    self.assistant_turns += 1;
                    if let Some(u) = msg_usage {
                        let anon_id = format!("__anon_{}", self.assistant_turns);
                        self.msg_usages.insert(anon_id, u);
                    }
                }

                if let Some(contents) = content_list {
                    for item in contents {
                        if let Some(item_obj) = item.as_object() {
                            if item_obj.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                                if let Some(name) = item_obj.get("name").and_then(|v| v.as_str()) {
                                    *self.tool_counts.entry(name.to_string()).or_insert(0) += 1;

                                    if let Some(input_obj) =
                                        item_obj.get("input").and_then(|v| v.as_object())
                                    {
                                        let raw_path = input_obj
                                            .get("file_path")
                                            .or_else(|| input_obj.get("notebook_path"))
                                            .or_else(|| input_obj.get("path"))
                                            .and_then(|v| v.as_str());

                                        if let Some(raw) = raw_path {
                                            let norm_p =
                                                normalize_tool_path(raw, self.root.as_deref());
                                            if name == "Read" || name == "Grep" || name == "Glob" {
                                                if !self.reads.contains(&norm_p) {
                                                    self.reads.push(norm_p.clone());
                                                }
                                            } else if EDIT_TOOLS.contains(&name) {
                                                if !self.edits.contains(&norm_p) {
                                                    self.edits.push(norm_p.clone());
                                                }
                                                let is_proto = is_protocol_file(&norm_p);
                                                if !is_proto
                                                    && self.turns_before_first_edit.is_none()
                                                {
                                                    self.turns_before_first_edit =
                                                        Some(self.assistant_turns - 1);
                                                    self.first_edit_file = Some(norm_p);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if ev.kind == "result" {
                self.result_seen = true;
                if self.session_id.is_empty() {
                    if let Some(s) = obj.get("session_id").and_then(|v| v.as_str()) {
                        self.session_id = s.to_string();
                    }
                }
                self.subtype = obj
                    .get("subtype")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                self.is_error = obj
                    .get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                self.num_turns = obj.get("num_turns").and_then(|v| v.as_i64());
                self.cost_usd = obj
                    .get("total_cost_usd")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                self.result_text = obj
                    .get("result")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                if let Some(denials) = obj.get("permission_denials").and_then(|v| v.as_array()) {
                    self.permission_denials = denials.len() as i64;
                }
                if let Some(u) = obj.get("usage").and_then(|v| v.as_object()) {
                    self.usage.input_tokens =
                        u.get("input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                    self.usage.output_tokens =
                        u.get("output_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                    self.usage.cache_read_input_tokens = u
                        .get("cache_read_input_tokens")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    self.usage.cache_creation_input_tokens = u
                        .get("cache_creation_input_tokens")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                }
            }
        }

        Some(ev)
    }

    pub fn finish(&mut self) {
        if self.result_seen {
            self.format_ok = true;
            self.usage_source = UsageSource::Result;
        } else if !self.msg_usages.is_empty() {
            self.format_ok = false;
            self.usage_source = UsageSource::Partial;
            let mut sum_usage = Usage::default();
            for u in self.msg_usages.values() {
                sum_usage.input_tokens += u.input_tokens;
                sum_usage.output_tokens += u.output_tokens;
                sum_usage.cache_read_input_tokens += u.cache_read_input_tokens;
                sum_usage.cache_creation_input_tokens += u.cache_creation_input_tokens;
            }
            self.usage = sum_usage;
        } else {
            self.format_ok = false;
            self.usage_source = UsageSource::None;
        }

        if let Some(ref pf) = self.plan_files {
            let norm_plan_files: BTreeSet<String> = pf
                .iter()
                .map(|f| normalize_tool_path(f, self.root.as_deref()))
                .collect();
            let mut outside = Vec::new();
            for r in &self.reads {
                if !is_protocol_file(r) && !norm_plan_files.contains(r) {
                    outside.push(r.clone());
                }
            }
            self.reads_outside_plan = Some(outside);
        } else {
            self.reads_outside_plan = None;
        }
    }
}

pub fn summarize_file(
    path: &Path,
    plan_files: Option<Vec<String>>,
    root: Option<PathBuf>,
) -> std::io::Result<Summary> {
    use std::io::BufRead;
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let mut summary = Summary::new(plan_files, root);
    let mut buf = Vec::new();
    while reader.read_until(b'\n', &mut buf)? > 0 {
        summary.feed(&buf);
        buf.clear();
    }
    summary.finish();
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_bad_json_array() {
        let ev = classify(b"[1, 2]").expect("event");
        assert_eq!(ev.kind, "bad_json");
        assert_eq!(ev.raw, "[1, 2]");
        assert!(ev.value.is_none());
    }

    #[test]
    fn test_classify_truncated() {
        let ev = classify(b"{\"type\":\"system\",\"sub").expect("event");
        assert_eq!(ev.kind, "bad_json");
        assert_eq!(ev.raw, "{\"type\":\"system\",\"sub");
        assert!(ev.value.is_none());
    }

    #[test]
    fn test_classify_whitespace_none() {
        assert!(classify(b"   \n").is_none());
    }

    #[test]
    fn test_classify_invalid_utf8() {
        let ev = classify(&[0xFF, 0xFE]).expect("event");
        assert_eq!(ev.kind, "bad_json");
        assert!(ev.raw.contains('\u{FFFD}'));
    }

    #[test]
    fn test_classify_trailing_newline_same() {
        let with_nl = classify(b"{\"type\":\"x\"}\n").expect("with nl");
        let without_nl = classify(b"{\"type\":\"x\"}").expect("without nl");
        assert_eq!(with_nl.kind, without_nl.kind);
        assert_eq!(with_nl.raw, without_nl.raw);
    }

    #[test]
    fn test_classify_unknown_type() {
        let ev = classify(b"{\"type\":\"x\"}").expect("event");
        assert_eq!(ev.kind, "unknown:x");
        assert_eq!(ev.raw, "{\"type\":\"x\"}");
    }

    #[test]
    fn test_reads_counts_only_read_not_grep() {
        let mut summary = Summary::new(None, None);
        let line = serde_json::json!({
            "type": "assistant",
            "message": {
                "id": "msg-grep-read",
                "content": [
                    {
                        "type": "tool_use",
                        "name": "Grep",
                        "input": {
                            "path": "src/lib.rs",
                            "pattern": "fn main"
                        }
                    },
                    {
                        "type": "tool_use",
                        "name": "Read",
                        "input": {
                            "file_path": "src/main.rs"
                        }
                    }
                ]
            }
        });
        let raw = serde_json::to_vec(&line).unwrap();
        summary.feed(&raw);
        assert_eq!(summary.reads, vec!["src/main.rs"]);
        assert_eq!(summary.tool_counts.get("Grep"), Some(&1));
        assert_eq!(summary.tool_counts.get("Read"), Some(&1));
    }
}
