use super::{blocking, json_error, AppState};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, watch};

pub const PAGE: i64 = 500;

pub fn row_kind(ev_kind: &str, text: &str) -> &'static str {
    if ev_kind.starts_with("system/")
        || ev_kind == "system"
        || ev_kind == "rate_limit_event"
        || ev_kind.starts_with("unknown:")
    {
        "system"
    } else if ev_kind == "bad_json" {
        "error"
    } else if ev_kind == "user" {
        "tool_result"
    } else if ev_kind == "result" || ev_kind == "agy/result" {
        "result"
    } else if ev_kind == "assistant" {
        if text.starts_with("tool ") {
            "tool"
        } else {
            "assistant"
        }
    } else {
        "system"
    }
}

pub fn row_json(seq: i64, ts: i64, ev_kind: &str, raw: &str) -> serde_json::Value {
    let text = if let Some(ev) = crate::stream::classify(raw.as_bytes()) {
        crate::term::event_text(&ev, false)
    } else {
        raw.to_string()
    };
    let kind = row_kind(ev_kind, &text);
    let time_str = chrono::DateTime::from_timestamp_millis(ts)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "00:00:00".to_string());
    serde_json::json!({
        "seq": seq,
        "ts": ts,
        "time": time_str,
        "kind": kind,
        "ev": ev_kind,
        "text": text
    })
}

pub fn status_json(run: &crate::db::RunRow) -> serde_json::Value {
    serde_json::json!({
        "run_id": run.id,
        "task_id": run.task_id,
        "role": run.role,
        "status": run.status,
        "outcome": run.outcome,
        "exit_code": run.exit_code,
        "input_tokens": run.input_tokens,
        "output_tokens": run.output_tokens,
        "cache_read_tokens": run.cache_read_tokens,
        "cache_write_tokens": run.cache_write_tokens,
        "cost_usd": run.cost_usd,
        "started_at": run.started_at,
        "ended_at": run.ended_at,
    })
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct StatusSnapshot {
    status: String,
    outcome: Option<String>,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
}

impl StatusSnapshot {
    fn from_run(r: &crate::db::RunRow) -> Self {
        Self {
            status: r.status.clone(),
            outcome: r.outcome.clone(),
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            cache_read_tokens: r.cache_read_tokens,
            cache_write_tokens: r.cache_write_tokens,
        }
    }
}

async fn wait_for_hint(
    hint_rx: &mut broadcast::Receiver<i64>,
    shutdown_rx: &mut watch::Receiver<bool>,
    run_id: i64,
) -> bool {
    let deadline = tokio::time::sleep(Duration::from_secs(15));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            res = shutdown_rx.changed() => {
                if res.is_err() || *shutdown_rx.borrow() {
                    return false;
                }
            }
            res = hint_rx.recv() => {
                match res {
                    Ok(id) => {
                        if id == run_id || id == 0 {
                            return true;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        return true;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        return false;
                    }
                }
            }
            _ = &mut deadline => {
                return true;
            }
        }
    }
}

struct StreamState {
    st: Arc<AppState>,
    run_id: i64,
    cursor: i64,
    buf: VecDeque<Event>,
    last_status: Option<StatusSnapshot>,
    hint_rx: broadcast::Receiver<i64>,
    shutdown_rx: watch::Receiver<bool>,
    done: bool,
}

pub async fn stream(
    State(st): State<Arc<AppState>>,
    Path(run_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let last_event_id: i64 = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let hint_rx = st.hints.subscribe();
    let shutdown_rx = st.shutdown.clone();

    let db_p = st.db_path.clone();
    let initial_run_res = blocking(move || {
        let conn = crate::db::open(&db_p)?;
        crate::db::get_run(&conn, run_id)
    })
    .await;

    let initial_run = match initial_run_res {
        Ok(Some(run)) => run,
        _ => {
            return json_error(
                StatusCode::NOT_FOUND,
                &format!("Không tìm thấy run {run_id}"),
            );
        }
    };

    let mut initial_buf = VecDeque::new();
    initial_buf.push_back(
        Event::default()
            .event("status")
            .data(status_json(&initial_run).to_string()),
    );
    let initial_snap = StatusSnapshot::from_run(&initial_run);

    let state = StreamState {
        st,
        run_id,
        cursor: last_event_id,
        buf: initial_buf,
        last_status: Some(initial_snap),
        hint_rx,
        shutdown_rx,
        done: false,
    };

    let sse_stream = futures_util::stream::unfold(state, |mut s| async move {
        loop {
            if let Some(ev) = s.buf.pop_front() {
                return Some((Ok::<Event, std::convert::Infallible>(ev), s));
            }
            if s.done {
                return None;
            }

            let db_p = s.st.db_path.clone();
            let r_id = s.run_id;
            let cur = s.cursor;
            let read_res = blocking(move || {
                let conn = crate::db::open(&db_p)?;
                let run = crate::db::get_run(&conn, r_id)?;
                let mut stmt = conn.prepare(
                    "SELECT seq, ts, kind, raw_json FROM events WHERE run_id = ?1 AND seq > ?2 ORDER BY seq LIMIT 500",
                )?;
                let rows = stmt
                    .query_map([r_id, cur], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((run, rows))
            })
            .await;

            let (run_opt, rows) = match read_res {
                Ok(pair) => pair,
                Err(e) => {
                    crate::term::emit(&format!("Lỗi đọc DB trong SSE stream: {e}"));
                    return None;
                }
            };

            let run = match run_opt {
                Some(r) => r,
                None => {
                    return None;
                }
            };

            let rows_len = rows.len();
            for (seq, ts, kind, raw) in rows {
                s.cursor = seq;
                let val = row_json(seq, ts, &kind, &raw);
                s.buf
                    .push_back(Event::default().id(seq.to_string()).data(val.to_string()));
            }

            let curr_snap = StatusSnapshot::from_run(&run);
            if s.last_status.as_ref() != Some(&curr_snap) {
                s.last_status = Some(curr_snap);
                s.buf.push_back(
                    Event::default()
                        .event("status")
                        .data(status_json(&run).to_string()),
                );
            }

            let page_full = rows_len == PAGE as usize;
            if run.status != "running" && !page_full {
                s.buf.push_back(
                    Event::default()
                        .event("end")
                        .data(status_json(&run).to_string()),
                );
                s.done = true;
            }

            if !s.buf.is_empty() {
                continue;
            }

            if *s.shutdown_rx.borrow() {
                return None;
            }

            let should_continue = wait_for_hint(&mut s.hint_rx, &mut s.shutdown_rx, s.run_id).await;
            if !should_continue {
                return None;
            }
        }
    });

    Sse::new(sse_stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_kind() {
        assert_eq!(row_kind("system/init", "init"), "system");
        assert_eq!(row_kind("system/anything", "foo"), "system");
        assert_eq!(row_kind("system", "foo"), "system");
        assert_eq!(row_kind("rate_limit_event", "foo"), "system");
        assert_eq!(row_kind("unknown:something", "foo"), "system");
        assert_eq!(row_kind("bad_json", "foo"), "error");
        assert_eq!(row_kind("user", "foo"), "tool_result");
        assert_eq!(row_kind("result", "foo"), "result");
        assert_eq!(row_kind("assistant", "tool Bash ls"), "tool");
        assert_eq!(row_kind("assistant", "assistant: hello"), "assistant");
    }

    #[test]
    fn agy_row_kinds() {
        assert_eq!(row_kind("agy/result", "kết quả · SUCCESS"), "result");
        assert_eq!(
            row_kind("agy/init", "init · agy · request-review"),
            "system"
        );
        assert_eq!(
            row_kind("agy/step_update", "bước 1 · tool · DONE"),
            "system"
        );
        let raw = r#"{"event":"result","result":{"status":"ERROR"}}"#;
        let v = row_json(3, 1700000000200, "agy/result", raw);
        assert_eq!(v["kind"], "result");
        assert_eq!(v["text"], "kết quả · ERROR");
    }

    #[test]
    fn test_row_json_init() {
        let raw = r#"{"type":"system","subtype":"init","session_id":"s","model":"m"}"#;
        let v = row_json(1, 1700000000000, "system/init", raw);
        assert_eq!(v["kind"], "system");
        assert_eq!(v["ev"], "system/init");
        assert_eq!(v["text"], "init · session s · model m");
        assert_eq!(v["seq"], 1);
        assert_eq!(v["ts"], 1700000000000i64);
        assert!(v.get("raw_json").is_none());
    }

    #[test]
    fn test_row_json_thinking_tokens() {
        let raw = r#"{"type":"system","subtype":"thinking_tokens"}"#;
        let v = row_json(2, 1700000000100, "system/thinking_tokens", raw);
        assert_eq!(v["kind"], "system");
        assert_eq!(v["ev"], "system/thinking_tokens");
    }
}
