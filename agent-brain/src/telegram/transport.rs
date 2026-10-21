//! Telegram Bot API transport through the system `curl` (D-04).
//!
//! The bot token travels only inside the `-K -` config written to curl's
//! stdin; it is never an argv element (argv is world-readable via /proc).
//! curl's stderr is drained and dropped: it can echo the URL (and the token).

use serde_json::Value;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const DEFAULT_API: &str = "https://api.telegram.org";
pub const LONG_POLL_SECS: u64 = 25;

const LOOPBACK_HTTP: &str = "http://127.0.0.1:";

/// `AGENT_BRAIN_TELEGRAM_API` override (tests) or the real API. Only https:// or
/// the loopback http base is accepted, so the token never leaves the machine in clear text.
pub fn api_base() -> Result<String, String> {
    match std::env::var("AGENT_BRAIN_TELEGRAM_API") {
        Ok(v) if !v.is_empty() => {
            if v.starts_with("https://") || v.starts_with(LOOPBACK_HTTP) {
                Ok(v.trim_end_matches('/').to_string())
            } else {
                Err(
                    "AGENT_BRAIN_TELEGRAM_API chỉ nhận https://… hoặc http://127.0.0.1:<cổng>"
                        .to_string(),
                )
            }
        }
        _ => Ok(DEFAULT_API.to_string()),
    }
}

pub fn curl_bin() -> String {
    match std::env::var("AGENT_BRAIN_CURL") {
        Ok(v) if !v.is_empty() => v,
        _ => "curl".to_string(),
    }
}

pub fn check_curl(bin: &str) -> Result<(), String> {
    let res = std::process::Command::new(bin)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match res {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "Không tìm thấy lệnh curl ({bin}) — cài curl rồi chạy lại agent-brain serve"
        )),
        Err(e) => Err(format!("Không chạy được lệnh curl ({bin}): {e}")),
    }
}

/// Double-quoted curl config value: `\` and `"` escaped; newline, CR and tab
/// written as the two-character escapes curl's config parser understands.
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

#[derive(Debug, Clone)]
pub enum Body {
    Json(Value),
    Upload {
        fields: Vec<(String, String)>,
        file_field: String,
        filename: String,
        content: String,
    },
}

/// Characters that would break curl's `-F` mini-syntax (`;` `,` `"` `\` and
/// control characters) are replaced in the upload filename.
fn safe_form_word(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == ';' || c == ',' || c == '"' || c == '\\' || c == '=' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect()
}

pub fn build_config(url: &str, body: &Body) -> String {
    let mut lines = vec![format!("url = {}", quote_cfg(url))];
    match body {
        Body::Json(v) => {
            lines.push(format!(
                "header = {}",
                quote_cfg("Content-Type: application/json")
            ));
            lines.push(format!("header = {}", quote_cfg("Expect:")));
            let compact = serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string());
            lines.push(format!("data-raw = {}", quote_cfg(&compact)));
        }
        Body::Upload {
            fields,
            file_field,
            filename,
            ..
        } => {
            lines.push(format!("header = {}", quote_cfg("Expect:")));
            for (k, v) in fields {
                lines.push(format!("form-string = {}", quote_cfg(&format!("{k}={v}"))));
            }
            lines.push(format!(
                "form = {}",
                quote_cfg(&format!(
                    "{}=@upload.md;filename={};type=text/markdown",
                    safe_form_word(file_field),
                    safe_form_word(filename)
                ))
            ));
        }
    }
    let mut s = lines.join("\n");
    s.push('\n');
    s
}

/// curl argv. `-q` (must be first) skips any `~/.curlrc`, so a user curl config
/// can never add tracing/logging of the request (which carries the token).
pub fn curl_argv(max_secs: u64, base: &str) -> Vec<String> {
    let proto = if base.starts_with(LOOPBACK_HTTP) {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    Network(String),
    Api {
        code: i64,
        description: String,
        retry_after: Option<u64>,
    },
    Stopped,
    Spawn(String),
    BadResponse(String),
}

impl CallError {
    pub fn text(&self) -> String {
        match self {
            CallError::Network(s) => s.clone(),
            CallError::Api {
                code, description, ..
            } => format!("Telegram trả lỗi {code}: {description}"),
            CallError::Stopped => "đã dừng".to_string(),
            CallError::Spawn(s) => s.clone(),
            CallError::BadResponse(s) => format!("phản hồi Telegram không hợp lệ ({s})"),
        }
    }

    pub fn is_unauthorized(&self) -> bool {
        matches!(self, CallError::Api { code: 401, .. })
    }

    /// `Some(ms)` when the call should be retried after `ms`, `None` when retrying is pointless.
    pub fn retry_delay_ms(&self, failures: u32, base_ms: u64) -> Option<u64> {
        match self {
            CallError::Network(_) | CallError::BadResponse(_) => {
                Some(backoff_ms(failures, base_ms))
            }
            CallError::Api {
                code, retry_after, ..
            } => {
                if *code == 429 {
                    Some(
                        retry_after
                            .map(|s| s.saturating_mul(1000))
                            .unwrap_or_else(|| backoff_ms(failures, base_ms)),
                    )
                } else if (400..500).contains(code) {
                    None
                } else {
                    Some(backoff_ms(failures, base_ms))
                }
            }
            CallError::Stopped | CallError::Spawn(_) => None,
        }
    }
}

/// `base * 2^(failures-1)`, capped at `12 * base` (5 s → 10 → 20 → 40 → 60 s).
pub fn backoff_ms(failures: u32, base_ms: u64) -> u64 {
    let exp = failures.max(1).saturating_sub(1).min(20);
    base_ms
        .saturating_mul(1u64 << exp)
        .min(base_ms.saturating_mul(12))
}

pub fn base_backoff_ms() -> u64 {
    match std::env::var("AGENT_BRAIN_TELEGRAM_BACKOFF_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
    {
        Some(v) => v.clamp(100, 5000),
        None => 5000,
    }
}

pub fn redact(s: &str, token: &str) -> String {
    if token.is_empty() {
        return s.to_string();
    }
    s.replace(token, "<token>")
}

fn curl_exit_text(code: Option<i32>) -> String {
    match code {
        Some(6) => "không phân giải được tên máy chủ Telegram".to_string(),
        Some(7) => "không kết nối được tới Telegram".to_string(),
        Some(28) => "quá thời gian chờ Telegram".to_string(),
        Some(35) | Some(60) => "lỗi TLS khi kết nối Telegram".to_string(),
        Some(n) => format!("curl lỗi mã {n}"),
        None => "curl bị dừng bởi tín hiệu".to_string(),
    }
}

pub struct Transport {
    token: String,
    base: String,
    curl: String,
    upload_dir: PathBuf,
    stop: Arc<AtomicBool>,
}

impl std::fmt::Debug for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transport")
            .field("token", &"<ẩn>")
            .field("base", &self.base)
            .field("curl", &self.curl)
            .field("upload_dir", &self.upload_dir)
            .finish()
    }
}

fn create_private_dir(p: &Path) -> std::io::Result<()> {
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(p)
}

impl Transport {
    pub fn new(
        token: String,
        base: String,
        curl: String,
        upload_dir: PathBuf,
        stop: Arc<AtomicBool>,
    ) -> Transport {
        Transport {
            token,
            base,
            curl,
            upload_dir,
            stop,
        }
    }

    /// Calls `method`; returns the `result` field of a `{ok: true}` answer.
    pub fn call(&self, method: &str, body: &Body, max_secs: u64) -> Result<Value, CallError> {
        if self.stop.load(Ordering::SeqCst) {
            return Err(CallError::Stopped);
        }
        let url = format!("{}/bot{}/{}", self.base, self.token, method);
        let cfg = build_config(&url, body);

        let workdir = match body {
            Body::Upload { content, .. } => match self.prepare_upload(content) {
                Ok(d) => Some(d),
                Err(e) => {
                    return Err(CallError::Spawn(format!(
                        "không ghi được file tạm để gửi Telegram: {e}"
                    )))
                }
            },
            Body::Json(_) => None,
        };

        let res = self.run_curl(&cfg, workdir.as_deref(), max_secs);
        if let Some(d) = workdir {
            let _ = std::fs::remove_dir_all(d);
        }
        res
    }

    fn prepare_upload(&self, content: &str) -> std::io::Result<PathBuf> {
        create_private_dir(&self.upload_dir)?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = self
            .upload_dir
            .join(format!("call-{}-{nanos}", std::process::id()));
        let mut b = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        b.create(&dir)?;
        let write = (|| -> std::io::Result<()> {
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut f = opts.open(dir.join("upload.md"))?;
            f.write_all(content.as_bytes())?;
            f.flush()?;
            Ok(())
        })();
        if let Err(e) = write {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e);
        }
        Ok(dir)
    }

    fn run_curl(
        &self,
        cfg: &str,
        workdir: Option<&Path>,
        max_secs: u64,
    ) -> Result<Value, CallError> {
        let mut cmd = std::process::Command::new(&self.curl);
        cmd.args(curl_argv(max_secs, &self.base));
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if let Some(d) = workdir {
            cmd.current_dir(d);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
            let server_pid = unsafe { libc::getpid() };
            unsafe {
                cmd.pre_exec(move || {
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::getppid() != server_pid {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "server exited before child exec",
                        ));
                    }
                    Ok(())
                });
            }
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(CallError::Spawn(format!(
                    "Không tìm thấy lệnh curl ({}) — cài curl rồi chạy lại agent-brain serve",
                    self.curl
                )))
            }
            Err(e) => {
                return Err(CallError::Spawn(format!(
                    "Không chạy được lệnh curl ({}): {e}",
                    self.curl
                )))
            }
        };

        // Config (with the token) goes to stdin on a helper thread, then stdin is closed.
        let stdin_thread = child.stdin.take().map(|mut stdin| {
            let data = cfg.as_bytes().to_vec();
            std::thread::spawn(move || {
                let _ = stdin.write_all(&data);
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
        // stderr is drained and dropped: never logged, never returned.
        let stderr_thread = child.stderr.take().map(|mut err| {
            std::thread::spawn(move || {
                let mut sink = [0u8; 4096];
                while let Ok(n) = err.read(&mut sink) {
                    if n == 0 {
                        break;
                    }
                }
            })
        });

        let start = Instant::now();
        let limit = Duration::from_secs(max_secs.saturating_add(5));
        let outcome: Result<std::process::ExitStatus, CallError> = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Ok(st),
                Ok(None) => {}
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(CallError::Network(format!("không chờ được curl: {e}")));
                }
            }
            if self.stop.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                break Err(CallError::Stopped);
            }
            if start.elapsed() > limit {
                let _ = child.kill();
                let _ = child.wait();
                break Err(CallError::Network("quá thời gian chờ Telegram".to_string()));
            }
            std::thread::sleep(Duration::from_millis(50));
        };

        if let Some(t) = stdin_thread {
            let _ = t.join();
        }
        let stdout = stdout_thread
            .and_then(|t| t.join().ok())
            .unwrap_or_default();
        if let Some(t) = stderr_thread {
            let _ = t.join();
        }

        let status = outcome?;
        if !status.success() {
            return Err(CallError::Network(curl_exit_text(status.code())));
        }

        let v: Value = match serde_json::from_slice(&stdout) {
            Ok(v) => v,
            Err(_) => {
                return Err(CallError::BadResponse(format!(
                    "{} byte không phải JSON",
                    stdout.len()
                )))
            }
        };
        match v.get("ok").and_then(|o| o.as_bool()) {
            Some(true) => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
            Some(false) => {
                let code = v.get("error_code").and_then(|c| c.as_i64()).unwrap_or(0);
                let description = v
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();
                let retry_after = v
                    .get("parameters")
                    .and_then(|p| p.get("retry_after"))
                    .and_then(|r| r.as_u64());
                Err(CallError::Api {
                    code,
                    description: redact(&description, &self.token),
                    retry_after,
                })
            }
            None => Err(CallError::BadResponse("thiếu trường ok".to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "4242:fake-token-for-tests";

    #[test]
    fn argv_never_carries_the_token() {
        for base in [DEFAULT_API, "http://127.0.0.1:9"] {
            let argv = curl_argv(LONG_POLL_SECS + 10, base);
            assert!(argv.iter().all(|a| !a.contains(TOKEN)), "{argv:?}");
            let n = argv.len();
            assert_eq!(&argv[n - 2..], &["-K".to_string(), "-".to_string()]);
        }
        assert!(curl_argv(5, DEFAULT_API).contains(&"=https".to_string()));
        assert!(curl_argv(5, "http://127.0.0.1:9").contains(&"=http,https".to_string()));
    }

    #[test]
    fn config_carries_url_with_token() {
        let url = format!("{DEFAULT_API}/bot{TOKEN}/getMe");
        let cfg = build_config(&url, &Body::Json(serde_json::json!({})));
        assert!(
            cfg.contains(&format!(
                "url = \"https://api.telegram.org/bot{TOKEN}/getMe\""
            )),
            "{cfg}"
        );
        assert!(cfg.contains("data-raw = \"{}\""), "{cfg}");
        let up = build_config(
            &url,
            &Body::Upload {
                fields: vec![("caption".to_string(), "a \"b\"; type=x".to_string())],
                file_field: "document".to_string(),
                filename: "x.plan.md".to_string(),
                content: "c".to_string(),
            },
        );
        assert!(
            up.contains(r#"form-string = "caption=a \"b\"; type=x""#),
            "{up}"
        );
        assert!(
            up.contains(r#"form = "document=@upload.md;filename=x.plan.md;type=text/markdown""#),
            "{up}"
        );
    }

    #[test]
    fn quote_cfg_escapes() {
        assert_eq!(quote_cfg(r#"a\b"c"#), r#""a\\b\"c""#);
        assert_eq!(quote_cfg("x\ny\r\tz"), r#""x\ny\r\tz""#);
    }

    #[test]
    fn backoff_doubles_to_sixty_seconds() {
        let got: Vec<u64> = (1..=6).map(|f| backoff_ms(f, 5000)).collect();
        assert_eq!(got, vec![5000, 10000, 20000, 40000, 60000, 60000]);
    }

    #[test]
    fn retry_delay_by_error_kind() {
        let e429 = CallError::Api {
            code: 429,
            description: "Too Many Requests".into(),
            retry_after: Some(7),
        };
        assert_eq!(e429.retry_delay_ms(1, 5000), Some(7000));
        let e400 = CallError::Api {
            code: 400,
            description: "Bad Request".into(),
            retry_after: None,
        };
        assert_eq!(e400.retry_delay_ms(1, 5000), None);
        let e502 = CallError::Api {
            code: 502,
            description: "Bad Gateway".into(),
            retry_after: None,
        };
        assert_eq!(e502.retry_delay_ms(2, 5000), Some(10000));
        assert_eq!(
            CallError::Network("x".into()).retry_delay_ms(3, 5000),
            Some(20000)
        );
        assert_eq!(CallError::Stopped.retry_delay_ms(1, 5000), None);
        assert_eq!(CallError::Spawn("x".into()).retry_delay_ms(1, 5000), None);
        let e401 = CallError::Api {
            code: 401,
            description: "Unauthorized".into(),
            retry_after: None,
        };
        assert!(e401.is_unauthorized());
        assert!(!e400.is_unauthorized());
    }

    #[test]
    fn env_overrides() {
        std::env::set_var("AGENT_BRAIN_TELEGRAM_API", "http://example.com");
        assert!(api_base().is_err());
        std::env::set_var("AGENT_BRAIN_TELEGRAM_API", "http://127.0.0.1:9/");
        assert_eq!(api_base().unwrap(), "http://127.0.0.1:9");
        std::env::set_var("AGENT_BRAIN_TELEGRAM_API", "https://tg.example/");
        assert_eq!(api_base().unwrap(), "https://tg.example");
        std::env::remove_var("AGENT_BRAIN_TELEGRAM_API");
        assert_eq!(api_base().unwrap(), DEFAULT_API);

        std::env::set_var("AGENT_BRAIN_TELEGRAM_BACKOFF_MS", "1");
        assert_eq!(base_backoff_ms(), 100);
        std::env::set_var("AGENT_BRAIN_TELEGRAM_BACKOFF_MS", "999999");
        assert_eq!(base_backoff_ms(), 5000);
        std::env::remove_var("AGENT_BRAIN_TELEGRAM_BACKOFF_MS");
        assert_eq!(base_backoff_ms(), 5000);
    }

    #[test]
    fn redact_replaces_token() {
        let s = format!("bad url /bot{TOKEN}/getMe and {TOKEN}");
        let r = redact(&s, TOKEN);
        assert!(!r.contains(TOKEN));
        assert_eq!(r.matches("<token>").count(), 2);
    }

    #[test]
    fn transport_debug_hides_token() {
        let t = Transport::new(
            TOKEN.into(),
            DEFAULT_API.into(),
            "curl".into(),
            PathBuf::from("/nonexistent"),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(!format!("{t:?}").contains(TOKEN));
    }
}
