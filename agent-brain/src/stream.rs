use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
    seen_msg_ids: BTreeSet<String>,
    result_seen: bool,
}

impl Summary {
    pub fn new(_plan_files: Option<Vec<String>>, _root: Option<PathBuf>) -> Self {
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
            seen_msg_ids: BTreeSet::new(),
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

                if let Some(msg) = obj.get("message").and_then(|m| m.as_object()) {
                    msg_id = msg.get("id").and_then(|v| v.as_str());
                    content_list = msg.get("content").and_then(|v| v.as_array());
                    if self.model.is_none() {
                        if let Some(m) = msg.get("model").and_then(|v| v.as_str()) {
                            self.model = Some(m.to_string());
                        }
                    }
                } else if let Some(c) = obj.get("content").and_then(|v| v.as_array()) {
                    content_list = Some(c);
                }

                if msg_id.is_none() {
                    msg_id = obj.get("id").and_then(|v| v.as_str());
                }

                if let Some(mid) = msg_id {
                    if !self.seen_msg_ids.contains(mid) {
                        self.seen_msg_ids.insert(mid.to_string());
                        self.assistant_turns += 1;
                    }
                } else {
                    self.assistant_turns += 1;
                }

                if let Some(contents) = content_list {
                    for item in contents {
                        if let Some(item_obj) = item.as_object() {
                            if item_obj.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                                if let Some(name) = item_obj.get("name").and_then(|v| v.as_str()) {
                                    *self.tool_counts.entry(name.to_string()).or_insert(0) += 1;
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
        } else {
            self.format_ok = false;
            self.usage_source = UsageSource::None;
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
