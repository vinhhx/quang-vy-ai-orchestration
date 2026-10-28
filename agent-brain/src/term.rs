use crate::stream::{StreamEvent, Summary};
use std::time::Duration;

/// Emits a line to stderr, swallowing all write errors (including EPIPE / BrokenPipe and EIO) without panicking.
pub fn emit(line: &str) {
    use std::io::Write;
    let mut stderr = std::io::stderr();
    let _ = writeln!(stderr, "{line}");
}

/// Emits a line to stdout, swallowing all write errors (including EPIPE / BrokenPipe and EIO) without panicking.
pub fn emit_out(line: &str) {
    use std::io::Write;
    let mut stdout = std::io::stdout();
    let _ = writeln!(stdout, "{line}");
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

pub fn clock() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

pub fn event_text(ev: &StreamEvent, verbose: bool) -> String {
    if ev.kind == "bad_json" {
        let trunc = truncate_chars(&ev.raw, 80);
        return format!("dòng JSON lỗi (lưu thô): {trunc}");
    }

    let val = match &ev.value {
        Some(v) => v,
        None => return ev.kind.clone(),
    };

    if ev.kind == "system/init" {
        let session = val.get("session_id").and_then(|s| s.as_str()).unwrap_or("");
        let model = val.get("model").and_then(|m| m.as_str()).unwrap_or("");
        format!("init · session {session} · model {model}")
    } else if ev.kind.starts_with("system/") {
        let subtype = val
            .get("subtype")
            .and_then(|s| s.as_str())
            .unwrap_or_else(|| ev.kind.strip_prefix("system/").unwrap_or("?"));
        format!("system {subtype}")
    } else if ev.kind == "assistant" {
        let mut blocks = Vec::new();
        let contents = val
            .get("message")
            .and_then(|m| m.get("content"))
            .or_else(|| val.get("content"))
            .and_then(|c| c.as_array());

        if let Some(contents) = contents {
            for item in contents {
                if let Some(item_obj) = item.as_object() {
                    let b_type = item_obj.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match b_type {
                        "text" => {
                            let text = item_obj.get("text").and_then(|t| t.as_str()).unwrap_or("");
                            if verbose {
                                blocks.push(format!("assistant: {text}"));
                            } else {
                                let single_line = text.replace(['\n', '\r'], " ");
                                let trunc = truncate_chars(&single_line, 100);
                                blocks.push(format!("assistant: {trunc}"));
                            }
                        }
                        "tool_use" => {
                            let name = item_obj.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            let input = item_obj.get("input");
                            let target = input
                                .and_then(|inp| {
                                    inp.get("file_path")
                                        .or_else(|| inp.get("notebook_path"))
                                        .or_else(|| inp.get("path"))
                                        .or_else(|| inp.get("command"))
                                        .or_else(|| inp.get("pattern"))
                                })
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let trunc = truncate_chars(target, 100);
                            if trunc.is_empty() {
                                blocks.push(format!("tool {name}"));
                            } else {
                                blocks.push(format!("tool {name} {trunc}"));
                            }
                        }
                        "thinking" => {
                            blocks.push("thinking".to_string());
                        }
                        _ => {
                            blocks.push("assistant".to_string());
                        }
                    }
                }
            }
        }
        if blocks.is_empty() {
            "assistant".to_string()
        } else {
            blocks.join(" | ")
        }
    } else if ev.kind == "user" {
        let mut blocks = Vec::new();
        let contents = val
            .get("message")
            .and_then(|m| m.get("content"))
            .or_else(|| val.get("content"))
            .and_then(|c| c.as_array());

        if let Some(contents) = contents {
            for item in contents {
                if let Some(item_obj) = item.as_object() {
                    let b_type = item_obj.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if b_type == "tool_result" {
                        let is_err = item_obj
                            .get("is_error")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let content_str = if let Some(s) =
                            item_obj.get("content").and_then(|c| c.as_str())
                        {
                            s.to_string()
                        } else if let Some(arr) = item_obj.get("content").and_then(|c| c.as_array())
                        {
                            arr.iter()
                                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                                .collect::<Vec<_>>()
                                .join("")
                        } else {
                            String::new()
                        };
                        let len = content_str.chars().count();
                        let status_str = if is_err { "lỗi" } else { "ok" };
                        if verbose && !content_str.is_empty() {
                            blocks.push(format!(
                                "kết quả tool: {status_str} ({len} ký tự) · {content_str}"
                            ));
                        } else {
                            blocks.push(format!("kết quả tool: {status_str} ({len} ký tự)"));
                        }
                    }
                }
            }
        }
        if blocks.is_empty() {
            "user".to_string()
        } else {
            blocks.join(" | ")
        }
    } else if ev.kind == "result" {
        let subtype = val.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
        let num_turns = val.get("num_turns").and_then(|n| n.as_i64()).unwrap_or(0);
        let cost = val
            .get("total_cost_usd")
            .and_then(|c| c.as_f64())
            .unwrap_or(0.0);
        format!("result {subtype} · {num_turns} turn · ${cost:.4}")
    } else if ev.kind == "rate_limit_event" {
        let status = val
            .get("rate_limit_info")
            .and_then(|r| r.get("status"))
            .and_then(|s| s.as_str())
            .unwrap_or("");
        format!("rate_limit {status}")
    } else if ev.kind == "dsh/text" {
        let text = val.get("text").and_then(|t| t.as_str()).unwrap_or("");
        if verbose {
            text.to_string()
        } else {
            let single_line = text.replace(['\n', '\r'], " ");
            truncate_chars(&single_line, 200)
        }
    } else if ev.kind == "agy/init" {
        let mode = val
            .get("init")
            .and_then(|i| i.get("permission_mode"))
            .and_then(|m| m.as_str())
            .unwrap_or("");
        format!("init · agy · {mode}")
    } else if ev.kind == "agy/step_update" {
        let step = val.get("step_update");
        let idx = step
            .and_then(|s| s.get("step_index"))
            .and_then(|i| i.as_i64())
            .unwrap_or(0);
        let step_type = step
            .and_then(|s| s.get("step_type"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let state = step
            .and_then(|s| s.get("state"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        format!("bước {idx} · {step_type} · {state}")
    } else if ev.kind == "agy/result" {
        let status = val
            .get("result")
            .and_then(|r| r.get("status"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        format!("kết quả · {status}")
    } else {
        ev.kind.clone()
    }
}

pub fn event_line(ev: &StreamEvent, verbose: bool) -> String {
    format!("{} {}", clock(), event_text(ev, verbose))
}

pub fn summary_line(
    run_id: i64,
    task_id: Option<i64>,
    status: &str,
    s: &Summary,
    elapsed: Duration,
) -> String {
    let task_part = match task_id {
        Some(tid) => format!("task {tid}"),
        None => "không có task".to_string(),
    };
    let mut base = format!(
        "Kết thúc run {run_id} ({task_part}): {status} · {} turn · token vào {}, ra {}, cache đọc {}, cache ghi {} · ~{:.4} USD tương đương · {:.1}s",
        s.assistant_turns,
        s.usage.input_tokens,
        s.usage.output_tokens,
        s.usage.cache_read_input_tokens,
        s.usage.cache_creation_input_tokens,
        s.cost_usd,
        elapsed.as_secs_f64()
    );
    if let Some(t) = s.turns_before_first_edit {
        base.push_str(&format!(" · {t} turn trước lần sửa đầu"));
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{classify, summarize_file};
    use std::path::PathBuf;

    #[test]
    fn test_s1_init_and_tool_use_lines() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let s1_path = manifest_dir.join("../spikes/s1/raw/s1.jsonl");
        let content = std::fs::read_to_string(&s1_path).expect("read s1.jsonl");
        let lines: Vec<&str> = content.lines().collect();

        // Line 3 (0-indexed 2) is the init line
        let init_ev = classify(lines[2].as_bytes()).expect("classify init");
        let init_txt = event_text(&init_ev, false);
        assert!(
            init_txt.starts_with("init · session 1794dfc4-1acc-4f78-afc7-5dd721fe31d6"),
            "got: {init_txt}"
        );

        // Line 4 (0-indexed 3) is the tool_use line
        let tool_ev = classify(lines[3].as_bytes()).expect("classify tool_use");
        let tool_txt = event_text(&tool_ev, false);
        assert!(tool_txt.starts_with("tool Bash"), "got: {tool_txt}");
    }

    #[test]
    fn agy_events_render() {
        let render = |l: &str| event_text(&classify(l.as_bytes()).unwrap(), false);
        assert_eq!(
            render(
                r#"{"event":"init","conversation_id":"c1","init":{"permission_mode":"request-review"}}"#
            ),
            "init · agy · request-review"
        );
        assert_eq!(
            render(
                r#"{"event":"step_update","step_update":{"step_index":3,"state":"DONE","step_type":"tool"}}"#
            ),
            "bước 3 · tool · DONE"
        );
        assert_eq!(
            render(r#"{"event":"result","result":{"status":"SUCCESS"}}"#),
            "kết quả · SUCCESS"
        );
    }

    #[test]
    fn test_truncate_chars_unicode() {
        assert_eq!(truncate_chars("đđđđ", 2), "đđ");
    }

    #[test]
    fn test_summary_line_kpi_text() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let fixture_path =
            manifest_dir.join("../tests/fixtures/stream/dev_two_reads_then_edit.jsonl");
        let summary = summarize_file(
            &fixture_path,
            Some(vec!["src/app.py".to_string()]),
            Some(PathBuf::from("/work/proj")),
        )
        .expect("summarize fixture");

        let line = summary_line(1, Some(1), "succeeded", &summary, Duration::from_secs(3));
        assert!(
            line.contains("2 turn trước lần sửa đầu"),
            "summary line missing KPI text: {line}"
        );
    }
}
