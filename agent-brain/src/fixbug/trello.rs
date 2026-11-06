//! Client gọi Trello REST API qua tiến trình curl (Plan fixbug-sources-engine).
//!
//! Bí mật (key, token) chỉ truyền qua stdin của curl (-K -), không bao giờ
//! nằm trong argv của tiến trình. Mọi phản hồi hoặc lỗi từ Trello đều được
//! redact qua `crate::secrets::redact_str`.

use super::FixbugError;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

pub const DEFAULT_TRELLO_API: &str = "https://api.trello.com";
const LOOPBACK_HTTP: &str = "http://127.0.0.1:";

/// `true` chỉ khi `v` là `http://127.0.0.1:<cổng>` (có thể kèm `/đường-dẫn`).
pub fn is_loopback_http(v: &str) -> bool {
    let Some(rest) = v.strip_prefix(LOOPBACK_HTTP) else {
        return false;
    };
    let port = rest.split_once('/').map_or(rest, |(port, _)| port);
    !port.is_empty()
        && port.len() <= 5
        && port.bytes().all(|b| b.is_ascii_digit())
        && port.parse::<u16>().is_ok_and(|p| p > 0)
}

/// Lấy base URL của Trello API từ env `AGENT_BRAIN_TRELLO_API` (tests) hoặc mặc định.
pub fn trello_api_base() -> Result<String, FixbugError> {
    match std::env::var("AGENT_BRAIN_TRELLO_API") {
        Ok(v) if !v.is_empty() => {
            if v.starts_with("https://") || is_loopback_http(&v) {
                Ok(v.trim_end_matches('/').to_string())
            } else {
                Err(FixbugError::Config(
                    "AGENT_BRAIN_TRELLO_API chỉ nhận https://… hoặc http://127.0.0.1:<cổng>"
                        .to_string(),
                ))
            }
        }
        _ => Ok(DEFAULT_TRELLO_API.to_string()),
    }
}

pub fn curl_bin() -> String {
    match std::env::var("AGENT_BRAIN_CURL") {
        Ok(v) if !v.is_empty() => v,
        _ => "curl".to_string(),
    }
}

/// Double-quoted curl config value: `\` và `"` thoát; newline, CR và tab
/// ghi thành dạng escape 2 ký tự mà parser config của curl hiểu.
pub fn quote_cfg(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn curl_argv(max_secs: u64, base: &str) -> Vec<String> {
    let proto = if is_loopback_http(base) {
        "=http,https"
    } else {
        "=https"
    };
    vec![
        "-q".to_string(),
        "-sS".to_string(),
        "--max-time".to_string(),
        max_secs.to_string(),
        "--proto".to_string(),
        proto.to_string(),
        "-K".to_string(),
        "-".to_string(),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrelloList {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub closed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrelloLabel {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrelloAttachment {
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(rename = "isUpload")]
    pub is_upload: Option<bool>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrelloCard {
    pub id: String,
    #[serde(rename = "idShort")]
    pub id_short: i64,
    pub name: String,
    #[serde(default)]
    pub desc: String,
    #[serde(rename = "idList")]
    pub id_list: String,
    #[serde(default)]
    pub labels: Vec<TrelloLabel>,
    #[serde(rename = "shortUrl")]
    pub short_url: String,
    #[serde(rename = "dateLastActivity")]
    pub date_last_activity: Option<String>,
    #[serde(default)]
    pub attachments: Vec<TrelloAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrelloComment {
    pub date: String,
    pub author: String,
    pub text: String,
}

pub struct TrelloClient {
    base: String,
    key: String,
    token: String,
    curl: String,
}

impl TrelloClient {
    pub fn new(base: &str, key: &str, token: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            key: key.trim().to_string(),
            token: token.trim().to_string(),
            curl: curl_bin(),
        }
    }

    fn secrets(&self) -> Vec<String> {
        let mut v = Vec::new();
        if !self.key.is_empty() {
            v.push(self.key.clone());
        }
        if !self.token.is_empty() {
            v.push(self.token.clone());
        }
        v
    }

    fn redact(&self, text: &str) -> String {
        crate::secrets::redact_str(text, &self.secrets())
    }

    fn run_curl(&self, cfg: &str, max_secs: u64) -> Result<(u16, String), FixbugError> {
        let mut cmd = std::process::Command::new(&self.curl);
        cmd.args(curl_argv(max_secs, &self.base));
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        let mut child = cmd.spawn().map_err(|e| {
            FixbugError::Io(format!("Không chạy được lệnh curl ({}): {e}", self.curl))
        })?;

        let stdin_data = cfg.as_bytes().to_vec();
        let stdin_thread = child.stdin.take().map(|mut stdin| {
            std::thread::spawn(move || {
                let _ = stdin.write_all(&stdin_data);
                drop(stdin);
            })
        });

        let stdout_thread = child.stdout.take().map(|mut out| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = out.read_to_end(&mut buf);
                buf
            })
        });

        let stderr_thread = child.stderr.take().map(|mut err| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = err.read_to_end(&mut buf);
                buf
            })
        });

        let start = Instant::now();
        let limit = Duration::from_secs(max_secs.saturating_add(5));
        let outcome = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Ok(st),
                Ok(None) => {}
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(FixbugError::Network(format!("Không chờ được curl: {e}")));
                }
            }
            if start.elapsed() > limit {
                let _ = child.kill();
                let _ = child.wait();
                break Err(FixbugError::Network("Quá thời gian chờ Trello API".to_string()));
            }
            std::thread::sleep(Duration::from_millis(50));
        };

        if let Some(t) = stdin_thread {
            let _ = t.join();
        }
        let stdout_bytes = stdout_thread
            .and_then(|t| t.join().ok())
            .unwrap_or_default();
        let stderr_bytes = stderr_thread
            .and_then(|t| t.join().ok())
            .unwrap_or_default();

        let status = outcome?;
        let stdout_str = String::from_utf8_lossy(&stdout_bytes).into_owned();

        // Tách HTTP_STATUS từ cuối output (được ghi bởi write-out)
        let (body, http_code) = match stdout_str.rfind("\nHTTP_STATUS:") {
            Some(idx) => {
                let body = &stdout_str[..idx];
                let code_str = stdout_str[idx + "\nHTTP_STATUS:".len()..].trim();
                let code = code_str.parse::<u16>().unwrap_or(0);
                (body.to_string(), code)
            }
            None => {
                if !status.success() {
                    let err_msg = String::from_utf8_lossy(&stderr_bytes).into_owned();
                    return Err(FixbugError::Network(self.redact(&format!(
                        "Lỗi curl (exit {:?}): {err_msg}",
                        status.code()
                    ))));
                }
                (stdout_str, 200)
            }
        };

        if (200..300).contains(&http_code) {
            Ok((http_code, body))
        } else {
            let clean_msg = self.redact(body.trim());
            Err(FixbugError::Trello {
                status: http_code,
                message: if clean_msg.is_empty() {
                    format!("HTTP {http_code}")
                } else {
                    clean_msg
                },
            })
        }
    }

    /// `GET /1/boards/{board}/lists?fields=name,closed`
    pub fn lists(&self, board_id: &str) -> Result<Vec<TrelloList>, FixbugError> {
        let sep = if self.base.contains('?') { "&" } else { "?" };
        let url = format!(
            "{}/1/boards/{board_id}/lists{sep}fields=name,closed&key={}&token={}",
            self.base, self.key, self.token
        );
        let cfg = format!("url = {}\nwrite-out = \"\\nHTTP_STATUS:%{{http_code}}\"\n", quote_cfg(&url));
        let (_, body) = self.run_curl(&cfg, 30)?;
        serde_json::from_str(&body).map_err(|e| {
            FixbugError::Trello {
                status: 200,
                message: format!("Phản hồi lists không phải JSON hợp lệ: {e}"),
            }
        })
    }

    /// `GET /1/boards/{board}/cards?fields=idShort,name,desc,idList,labels,shortUrl,dateLastActivity&attachments=true&attachment_fields=name,url,isUpload,mimeType`
    pub fn cards(&self, board_id: &str) -> Result<Vec<TrelloCard>, FixbugError> {
        let sep = if self.base.contains('?') { "&" } else { "?" };
        let url = format!(
            "{}/1/boards/{board_id}/cards{sep}fields=idShort,name,desc,idList,labels,shortUrl,dateLastActivity&attachments=true&attachment_fields=name,url,isUpload,mimeType&key={}&token={}",
            self.base, self.key, self.token
        );
        let cfg = format!("url = {}\nwrite-out = \"\\nHTTP_STATUS:%{{http_code}}\"\n", quote_cfg(&url));
        let (_, body) = self.run_curl(&cfg, 30)?;
        serde_json::from_str(&body).map_err(|e| {
            FixbugError::Trello {
                status: 200,
                message: format!("Phản hồi cards không phải JSON hợp lệ: {e}"),
            }
        })
    }

    /// `GET /1/cards/{id}/actions?filter=commentCard`
    pub fn comments(&self, card_id: &str) -> Result<Vec<TrelloComment>, FixbugError> {
        let sep = if self.base.contains('?') { "&" } else { "?" };
        let url = format!(
            "{}/1/cards/{card_id}/actions{sep}filter=commentCard&key={}&token={}",
            self.base, self.key, self.token
        );
        let cfg = format!("url = {}\nwrite-out = \"\\nHTTP_STATUS:%{{http_code}}\"\n", quote_cfg(&url));
        let (_, body) = self.run_curl(&cfg, 30)?;
        let raw: Vec<serde_json::Value> = serde_json::from_str(&body).map_err(|e| {
            FixbugError::Trello {
                status: 200,
                message: format!("Phản hồi comments không phải JSON hợp lệ: {e}"),
            }
        })?;

        let mut comments = Vec::new();
        for item in raw {
            let date = item
                .get("date")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let author = item
                .get("memberCreator")
                .and_then(|m| m.get("fullName").or_else(|| m.get("username")))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            let text = item
                .get("data")
                .and_then(|d| d.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            comments.push(TrelloComment { date, author, text });
        }
        Ok(comments)
    }

    /// `POST /1/cards/{id}/actions/comments`
    pub fn add_comment(
        &self,
        card_id: &str,
        text: &str,
        comment_prefix: &str,
    ) -> Result<(), FixbugError> {
        let full_text = if comment_prefix.is_empty() {
            text.to_string()
        } else {
            format!("{comment_prefix} {text}")
        };
        let sep = if self.base.contains('?') { "&" } else { "?" };
        let url = format!(
            "{}/1/cards/{card_id}/actions/comments{sep}key={}&token={}",
            self.base, self.key, self.token
        );
        let cfg = format!(
            "url = {}\nrequest = \"POST\"\nheader = \"Content-Type: application/x-www-form-urlencoded\"\ndata-urlencode = {}\nwrite-out = \"\\nHTTP_STATUS:%{{http_code}}\"\n",
            quote_cfg(&url),
            quote_cfg(&format!("text={full_text}"))
        );
        self.run_curl(&cfg, 30)?;
        Ok(())
    }

    /// `PUT /1/cards/{id}?idList=…`
    pub fn move_card(&self, card_id: &str, list_id: &str) -> Result<(), FixbugError> {
        let sep = if self.base.contains('?') { "&" } else { "?" };
        let url = format!(
            "{}/1/cards/{card_id}{sep}idList={list_id}&key={}&token={}",
            self.base, self.key, self.token
        );
        let cfg = format!(
            "url = {}\nrequest = \"PUT\"\nheader = \"Content-Length: 0\"\nwrite-out = \"\\nHTTP_STATUS:%{{http_code}}\"\n",
            quote_cfg(&url)
        );
        self.run_curl(&cfg, 30)?;
        Ok(())
    }

    /// Lấy (list_id, list_name) của card bằng `GET /1/cards/{id}?fields=idList,closed` và tra tên list.
    pub fn card_list_name(&self, card_id: &str, board_id: Option<&str>) -> Result<(String, String), FixbugError> {
        let sep = if self.base.contains('?') { "&" } else { "?" };
        let url = format!(
            "{}/1/cards/{card_id}{sep}fields=idList,closed&key={}&token={}",
            self.base, self.key, self.token
        );
        let cfg = format!("url = {}\nwrite-out = \"\\nHTTP_STATUS:%{{http_code}}\"\n", quote_cfg(&url));
        let (_, body) = self.run_curl(&cfg, 30)?;
        let card_val: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            FixbugError::Trello {
                status: 200,
                message: format!("Phản hồi card không phải JSON hợp lệ: {e}"),
            }
        })?;

        let list_id = card_val
            .get("idList")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if list_id.is_empty() {
            return Err(FixbugError::Trello {
                status: 200,
                message: format!("Card {card_id} không có trường idList"),
            });
        }

        // Tra tên list qua GET /1/lists/{idList}?fields=name
        let list_url = format!(
            "{}/1/lists/{list_id}{sep}fields=name&key={}&token={}",
            self.base, self.key, self.token
        );
        let list_cfg = format!("url = {}\nwrite-out = \"\\nHTTP_STATUS:%{{http_code}}\"\n", quote_cfg(&list_url));
        match self.run_curl(&list_cfg, 30) {
            Ok((_, list_body)) => {
                if let Ok(l_val) = serde_json::from_str::<serde_json::Value>(&list_body) {
                    if let Some(name) = l_val.get("name").and_then(|n| n.as_str()) {
                        return Ok((list_id, name.to_string()));
                    }
                }
            }
            Err(_) => {
                // Nếu tra riêng thất bại nhưng có board_id, thử lấy từ lists(board)
                if let Some(b) = board_id {
                    if let Ok(all_lists) = self.lists(b) {
                        for l in all_lists {
                            if l.id == list_id {
                                return Ok((list_id, l.name));
                            }
                        }
                    }
                }
            }
        }

        Ok((list_id.clone(), list_id))
    }
}
