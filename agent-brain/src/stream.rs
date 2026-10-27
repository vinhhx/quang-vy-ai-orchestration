use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

const EDIT_TOOLS: &[&str] = &["Edit", "Write", "MultiEdit", "NotebookEdit"];
#[rustfmt::skip]
pub const PROTOCOL_FILES: &[&str] = &[".plan.md", ".qa-report.md", ".plan-review.md", ".code-review.md"];

pub fn is_protocol_file(p: &str) -> bool {
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
            } else if let Some(e) = val.get("event").and_then(|v| v.as_str()) {
                // agy keys its events by `event` (S7), not `type`.
                format!("agy/{e}")
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
    /// A `rate_limit_event` with `status: rejected` was seen (stays set once seen).
    pub rate_limit_rejected: bool,
    /// `resetsAt` (Unix seconds) of the rejected event, as milliseconds; fractions are truncated.
    pub rate_limit_reset_at_ms: Option<i64>,
    pub rate_limit_type: Option<String>,
    /// `isUsingOverage` of the last `rate_limit_event` that carried it.
    pub using_overage: bool,
    /// `api_error_status` of the final `result` (HTTP status that ended the conversation).
    pub api_error_status: Option<i64>,
    pub terminal_reason: Option<String>,
    /// `error` / `error_status` of the last `system/api_retry`.
    pub last_retry_error: Option<String>,
    pub last_retry_status: Option<i64>,
    /// agy `result.status` (`SUCCESS` | `ERROR` | ...); `None` without an agy result.
    pub agy_status: Option<String>,
    pub agy_response: Option<String>,
    pub agy_error: Option<String>,
    /// `init.permission_mode` of an agy run.
    pub agy_permission_mode: Option<String>,
    /// Raw thinking tokens of an agy run (folded into `usage.output_tokens` for cost).
    pub thinking_tokens: i64,
    plan_files: Option<Vec<String>>,
    root: Option<PathBuf>,
    seen_msg_ids: BTreeSet<String>,
    msg_usages: BTreeMap<String, Usage>,
    agy_step_thinking: BTreeMap<i64, i64>,
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
            rate_limit_rejected: false,
            rate_limit_reset_at_ms: None,
            rate_limit_type: None,
            using_overage: false,
            api_error_status: None,
            terminal_reason: None,
            last_retry_error: None,
            last_retry_status: None,
            agy_status: None,
            agy_response: None,
            agy_error: None,
            agy_permission_mode: None,
            thinking_tokens: 0,
            plan_files,
            root,
            seen_msg_ids: BTreeSet::new(),
            msg_usages: BTreeMap::new(),
            agy_step_thinking: BTreeMap::new(),
            result_seen: false,
        }
    }

    /// A final `result` event was seen.
    pub fn result_seen(&self) -> bool {
        self.result_seen
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
                                            if name == "Read" {
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
            } else if ev.kind == "rate_limit_event" {
                if let Some(info) = obj.get("rate_limit_info").and_then(|v| v.as_object()) {
                    if let Some(over) = info.get("isUsingOverage").and_then(|v| v.as_bool()) {
                        self.using_overage = over;
                    }
                    if info.get("status").and_then(|v| v.as_str()) == Some("rejected") {
                        self.rate_limit_rejected = true;
                        if let Some(t) = info.get("rateLimitType").and_then(|v| v.as_str()) {
                            self.rate_limit_type = Some(t.to_string());
                        }
                        // Unix seconds, integer or fractional: truncate toward zero, store ms.
                        let secs = info.get("resetsAt").and_then(|v| {
                            v.as_i64().or_else(|| v.as_f64().map(|f| f.trunc() as i64))
                        });
                        if let Some(s) = secs {
                            self.rate_limit_reset_at_ms = Some(s.saturating_mul(1000));
                        }
                    }
                }
            } else if ev.kind == "system/api_retry" {
                if let Some(e) = obj.get("error").and_then(|v| v.as_str()) {
                    self.last_retry_error = Some(e.to_string());
                }
                self.last_retry_status = obj.get("error_status").and_then(|v| v.as_i64());
            } else if ev.kind == "result" {
                self.result_seen = true;
                self.api_error_status = obj.get("api_error_status").and_then(|v| v.as_i64());
                self.terminal_reason = obj
                    .get("terminal_reason")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
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
            } else if ev.kind == "agy/init" {
                if self.session_id.is_empty() {
                    if let Some(c) = obj.get("conversation_id").and_then(|v| v.as_str()) {
                        self.session_id = c.to_string();
                    }
                }
                self.agy_permission_mode = obj
                    .get("init")
                    .and_then(|i| i.get("permission_mode"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            } else if ev.kind == "agy/step_update" {
                if let Some(step) = obj.get("step_update").and_then(|v| v.as_object()) {
                    let done = step.get("state").and_then(|v| v.as_str()) == Some("DONE");
                    let idx = step
                        .get("step_index")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(-1);
                    if done {
                        if let Some(u) = step.get("usage").and_then(|v| v.as_object()) {
                            let (usage, thinking) = agy_usage(u);
                            self.msg_usages.insert(format!("agy-step-{idx}"), usage);
                            self.agy_step_thinking.insert(idx, thinking);
                        }
                    }
                }
            } else if ev.kind == "agy/result" {
                if let Some(r) = obj.get("result").and_then(|v| v.as_object()) {
                    self.result_seen = true;
                    let status = r.get("status").and_then(|v| v.as_str()).unwrap_or("");
                    self.agy_status = Some(status.to_string());
                    self.is_error = status != "SUCCESS";
                    self.agy_response = r
                        .get("response")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    self.result_text = self.agy_response.clone();
                    self.agy_error = r
                        .get("error")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());
                    self.num_turns = r.get("num_turns").and_then(|v| v.as_i64());
                    if self.session_id.is_empty() {
                        if let Some(c) = r
                            .get("conversation_id")
                            .and_then(|v| v.as_str())
                            .filter(|c| !c.is_empty())
                        {
                            self.session_id = c.to_string();
                        }
                    }
                    self.cost_usd = 0.0;
                    if let Some(u) = r.get("usage").and_then(|v| v.as_object()) {
                        let (usage, thinking) = agy_usage(u);
                        self.usage = usage;
                        self.thinking_tokens = thinking;
                    }
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
            self.thinking_tokens = self.agy_step_thinking.values().sum();
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

/// agy `usage` -> (usage with thinking folded into output, raw thinking tokens). agy reports no
/// cache writes.
fn agy_usage(u: &serde_json::Map<String, serde_json::Value>) -> (Usage, i64) {
    let n = |k: &str| u.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
    let thinking = n("thinking_tokens");
    (
        Usage {
            input_tokens: n("input_tokens"),
            output_tokens: n("output_tokens") + thinking,
            cache_read_input_tokens: n("cache_read_tokens"),
            cache_creation_input_tokens: 0,
        },
        thinking,
    )
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

    fn feed_all(lines: &[&str]) -> Summary {
        let mut s = Summary::new(None, None);
        for l in lines {
            s.feed(l.as_bytes());
        }
        s.finish();
        s
    }

    #[test]
    fn rate_limit_fields() {
        let s = feed_all(&[
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","resetsAt":1792310400,"rateLimitType":"seven_day","isUsingOverage":false}}"#,
        ]);
        assert!(!s.rate_limit_rejected, "allowed_warning is not a rejection");
        assert_eq!(s.rate_limit_reset_at_ms, None);
        assert_eq!(s.rate_limit_type, None);

        let s = feed_all(&[
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1792310400,"rateLimitType":"five_hour","isUsingOverage":false}}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","isUsingOverage":true}}"#,
        ]);
        assert!(s.rate_limit_rejected, "a rejection stays set");
        assert_eq!(s.rate_limit_reset_at_ms, Some(1_792_310_400_000));
        assert_eq!(s.rate_limit_type.as_deref(), Some("five_hour"));
        assert!(s.using_overage, "isUsingOverage is the last value seen");

        // A fractional resetsAt is truncated toward zero; no float is stored.
        let s = feed_all(&[
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1792310400.9,"isUsingOverage":false}}"#,
        ]);
        assert_eq!(s.rate_limit_reset_at_ms, Some(1_792_310_400_000));

        // Unknown shapes never panic and change nothing.
        let s = feed_all(&[
            r#"{"type":"rate_limit_event"}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":"x"}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":7,"resetsAt":"soon","isUsingOverage":"no"}}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":"soon"}}"#,
        ]);
        assert!(s.rate_limit_rejected);
        assert_eq!(s.rate_limit_reset_at_ms, None);
        assert!(!s.using_overage);
    }

    #[test]
    fn api_retry_fields() {
        let s = feed_all(&[
            r#"{"type":"system","subtype":"api_retry","attempt":1,"max_retries":5,"retry_delay_ms":10,"error_status":429,"error":"rate_limit"}"#,
        ]);
        assert_eq!(s.last_retry_error.as_deref(), Some("rate_limit"));
        assert_eq!(s.last_retry_status, Some(429));
        assert!(!s.result_seen());

        let s = feed_all(&[
            r#"{"type":"system","subtype":"api_retry","error_status":null,"error":"overloaded"}"#,
            r#"{"type":"system","subtype":"api_retry","error":7}"#,
        ]);
        assert_eq!(s.last_retry_error.as_deref(), Some("overloaded"));
        assert_eq!(s.last_retry_status, None);
    }

    #[test]
    fn result_error_fields() {
        let s = feed_all(&[
            r#"{"type":"result","subtype":"success","is_error":true,"api_error_status":402,"terminal_reason":"blocking_limit","result":"x"}"#,
        ]);
        assert!(s.result_seen());
        assert!(s.is_error);
        assert_eq!(s.api_error_status, Some(402));
        assert_eq!(s.terminal_reason.as_deref(), Some("blocking_limit"));

        let s = feed_all(&[
            r#"{"type":"result","subtype":"success","is_error":false,"api_error_status":null}"#,
        ]);
        assert!(s.result_seen());
        assert_eq!(s.api_error_status, None);
        assert_eq!(s.terminal_reason, None);
    }

    #[test]
    fn agy_events_classified() {
        let init = br#"{"event":"init","conversation_id":"c1","init":{"permission_mode":"request-review"}}"#;
        assert_eq!(classify(init).unwrap().kind, "agy/init");
        let step = br#"{"event":"step_update","step_update":{"step_index":0,"state":"DONE"}}"#;
        assert_eq!(classify(step).unwrap().kind, "agy/step_update");
        let res = br#"{"event":"result","result":{"status":"SUCCESS"}}"#;
        assert_eq!(classify(res).unwrap().kind, "agy/result");
        // A claude line keeps its own kinds.
        assert_eq!(classify(br#"{"type":"result"}"#).unwrap().kind, "result");
    }

    #[test]
    fn agy_summary_from_result() {
        let s = feed_all(&[
            r#"{"event":"init","conversation_id":"c1","init":{"permission_mode":"request-review"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":1,"state":"ACTIVE","step_type":"agent_response"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":1,"state":"DONE","step_type":"agent_response","usage":{"input_tokens":100,"output_tokens":10,"thinking_tokens":2,"cache_read_tokens":0}}}"#,
            r#"{"event":"result","result":{"conversation_id":"c1","status":"SUCCESS","response":"xong","num_turns":1,"usage":{"input_tokens":120,"output_tokens":30,"thinking_tokens":5,"cache_read_tokens":40,"total_tokens":150}}}"#,
        ]);
        assert!(s.result_seen());
        assert_eq!(s.session_id, "c1");
        assert_eq!(s.agy_permission_mode.as_deref(), Some("request-review"));
        assert_eq!(s.agy_status.as_deref(), Some("SUCCESS"));
        assert_eq!(s.agy_response.as_deref(), Some("xong"));
        assert_eq!(s.result_text.as_deref(), Some("xong"));
        assert!(!s.is_error);
        assert_eq!(s.num_turns, Some(1));
        assert_eq!(s.usage.input_tokens, 120);
        assert_eq!(s.usage.output_tokens, 35);
        assert_eq!(s.usage.cache_read_input_tokens, 40);
        assert_eq!(s.usage.cache_creation_input_tokens, 0);
        assert_eq!(s.thinking_tokens, 5);
        assert_eq!(s.usage_source, UsageSource::Result);
    }

    #[test]
    fn agy_summary_error_and_partial() {
        let s = feed_all(&[
            r#"{"event":"result","result":{"conversation_id":"","status":"ERROR","response":"","error":"boom","num_turns":0}}"#,
        ]);
        assert!(s.is_error);
        assert_eq!(s.agy_status.as_deref(), Some("ERROR"));
        assert_eq!(s.agy_error.as_deref(), Some("boom"));

        // No result: partial usage from DONE steps, with their thinking tokens.
        let s = feed_all(&[
            r#"{"event":"step_update","step_update":{"step_index":1,"state":"DONE","usage":{"input_tokens":10,"output_tokens":4,"thinking_tokens":1,"cache_read_tokens":2}}}"#,
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"DONE","usage":{"input_tokens":20,"output_tokens":6,"thinking_tokens":3,"cache_read_tokens":0}}}"#,
        ]);
        assert!(!s.result_seen());
        assert_eq!(s.usage_source, UsageSource::Partial);
        assert_eq!(s.usage.input_tokens, 30);
        assert_eq!(s.usage.output_tokens, 14);
        assert_eq!(s.thinking_tokens, 4);
    }
}
