//! C2: checks that a project's target environment is up before a run (and, optionally, while it
//! runs). The checks come from the project's `preflight` entry in `projects_map.json`; a project
//! without one behaves exactly as before.
//!
//! - `http`: `url` or `url_env` (read from THIS process's environment, never from a `.env*` file),
//!   `expect_status`, `timeout_seconds`. `http://` is checked with a plain TCP connection and a
//!   minimal HTTP/1.1 request; `https://` with the curl the crate already uses (Trello), the URL
//!   handed over on stdin so it never shows in a process list.
//! - `tcp`: `host`, `port`, `timeout_seconds`.
//!
//! Messages and logs only ever show `scheme://host[:port]` of a URL, never its path or query.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Most checks a project can list.
pub const MAX_CHECKS: usize = 5;
/// `timeout_seconds` range of one check.
pub const MIN_TIMEOUT_SECS: u64 = 1;
pub const MAX_TIMEOUT_SECS: u64 = 60;
pub const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_TCP_TIMEOUT_SECS: u64 = 5;
/// Most entries in `expect_status`.
pub const MAX_EXPECT_STATUS: usize = 10;
/// `preflight_retry_minutes` upper bound (0 = no automatic rerun).
pub const MAX_RETRY_MINUTES: u64 = 1440;
/// `preflight_interval_seconds` range when on (0 = off).
pub const MIN_INTERVAL_SECS: u64 = 30;
pub const MAX_INTERVAL_SECS: u64 = 3600;
/// Most roles in `preflight_roles`.
pub const MAX_ROLES: usize = 10;
/// Roles checked when `preflight_roles` is missing.
pub const DEFAULT_ROLES: &[&str] = &["dev", "qa"];
/// Consecutive failed checks during a run that stop it (`preflight_interval_seconds`).
pub const FAILS_TO_STOP: u32 = 2;
/// Run outcome of a run stopped because the environment went down while it ran.
pub const OUTCOME_ENV_DOWN: &str = "env:down";

const MAX_URL_LEN: usize = 2048;
const MAX_HOST_LEN: usize = 253;
const MAX_ENV_NAME_LEN: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlSource {
    Literal(String),
    Env(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    Http {
        url: UrlSource,
        /// Empty = any 2xx or 3xx.
        expect_status: Vec<u16>,
        timeout_seconds: u64,
    },
    Tcp {
        host: String,
        port: u16,
        timeout_seconds: u64,
    },
}

/// The project's preflight settings. `Default` = nothing configured (no check anywhere).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preflight {
    pub checks: Vec<Check>,
    pub roles: Vec<String>,
    pub retry_minutes: u64,
    pub interval_seconds: u64,
}

impl Default for Preflight {
    fn default() -> Self {
        Preflight {
            checks: Vec::new(),
            roles: DEFAULT_ROLES.iter().map(|s| s.to_string()).collect(),
            retry_minutes: 0,
            interval_seconds: 0,
        }
    }
}

impl Preflight {
    /// Checks run before a run of `role`.
    pub fn applies_to(&self, role: &str) -> bool {
        !self.checks.is_empty() && self.roles.iter().any(|r| r == role)
    }
}

fn valid_env_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_ENV_NAME_LEN
        && s.bytes()
            .next()
            .is_some_and(|b| b.is_ascii_uppercase() || b == b'_')
        && s
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

fn valid_host(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_HOST_LEN
        && s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b':'))
}

fn valid_role(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
}

fn timeout_of(obj: &serde_json::Map<String, serde_json::Value>, default: u64, at: &str) -> Result<u64, String> {
    match obj.get("timeout_seconds") {
        None | Some(serde_json::Value::Null) => Ok(default),
        Some(v) => match v.as_u64() {
            Some(n) if (MIN_TIMEOUT_SECS..=MAX_TIMEOUT_SECS).contains(&n) => Ok(n),
            _ => Err(format!(
                "{at}: timeout_seconds phải là số nguyên {MIN_TIMEOUT_SECS}–{MAX_TIMEOUT_SECS}."
            )),
        },
    }
}

/// One `preflight` entry. `at` names it in error messages (`preflight[0]`).
pub fn parse_check(v: &serde_json::Value, at: &str) -> Result<Check, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| format!("{at}: mỗi mục phải là một object."))?;
    match obj.get("kind").and_then(|k| k.as_str()) {
        Some("http") => {
            let url = obj.get("url").and_then(|u| u.as_str()).map(str::trim);
            let env = obj.get("url_env").and_then(|u| u.as_str()).map(str::trim);
            let url = match (url.filter(|s| !s.is_empty()), env.filter(|s| !s.is_empty())) {
                (Some(_), Some(_)) => {
                    return Err(format!("{at}: chỉ dùng một trong url hoặc url_env."));
                }
                (None, None) => return Err(format!("{at}: http cần url hoặc url_env.")),
                (Some(u), None) => {
                    if u.len() > MAX_URL_LEN {
                        return Err(format!("{at}: url dài quá {MAX_URL_LEN} ký tự."));
                    }
                    if parse_url(u).is_none() {
                        return Err(format!(
                            "{at}: url phải bắt đầu bằng http:// hoặc https:// và có tên máy."
                        ));
                    }
                    UrlSource::Literal(u.to_string())
                }
                (None, Some(e)) => {
                    if !valid_env_name(e) {
                        return Err(format!(
                            "{at}: url_env phải là tên biến môi trường (chữ in hoa, số, _)."
                        ));
                    }
                    UrlSource::Env(e.to_string())
                }
            };
            let expect_status = match obj.get("expect_status") {
                None | Some(serde_json::Value::Null) => Vec::new(),
                Some(serde_json::Value::Array(arr)) => {
                    if arr.len() > MAX_EXPECT_STATUS {
                        return Err(format!(
                            "{at}: expect_status tối đa {MAX_EXPECT_STATUS} mã."
                        ));
                    }
                    let mut out = Vec::new();
                    for c in arr {
                        match c.as_u64() {
                            Some(n) if (100..=599).contains(&n) => out.push(n as u16),
                            _ => {
                                return Err(format!(
                                    "{at}: expect_status chỉ chứa mã HTTP 100–599."
                                ));
                            }
                        }
                    }
                    out
                }
                Some(_) => return Err(format!("{at}: expect_status phải là danh sách mã HTTP.")),
            };
            let timeout_seconds = timeout_of(obj, DEFAULT_HTTP_TIMEOUT_SECS, at)?;
            Ok(Check::Http {
                url,
                expect_status,
                timeout_seconds,
            })
        }
        Some("tcp") => {
            let host = obj
                .get("host")
                .and_then(|h| h.as_str())
                .map(str::trim)
                .unwrap_or("");
            if !valid_host(host) {
                return Err(format!("{at}: tcp cần host hợp lệ (tên máy hoặc địa chỉ IP)."));
            }
            let port = match obj.get("port").and_then(|p| p.as_u64()) {
                Some(p) if (1..=65535).contains(&p) => p as u16,
                _ => return Err(format!("{at}: port phải là số 1–65535.")),
            };
            let timeout_seconds = timeout_of(obj, DEFAULT_TCP_TIMEOUT_SECS, at)?;
            Ok(Check::Tcp {
                host: host.to_string(),
                port,
                timeout_seconds,
            })
        }
        _ => Err(format!("{at}: kind phải là \"http\" hoặc \"tcp\".")),
    }
}

/// Strict parse of the four project keys (`preflight`, `preflight_roles`,
/// `preflight_retry_minutes`, `preflight_interval_seconds`) read from `obj` — the dashboard
/// endpoint's validation. A missing key takes its default.
pub fn parse_settings(obj: &serde_json::Value) -> Result<Preflight, Vec<String>> {
    let mut errs = Vec::new();
    let mut out = Preflight::default();

    match obj.get("preflight") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Array(arr)) => {
            if arr.len() > MAX_CHECKS {
                errs.push(format!("preflight tối đa {MAX_CHECKS} mục."));
            } else {
                for (i, v) in arr.iter().enumerate() {
                    match parse_check(v, &format!("preflight[{i}]")) {
                        Ok(c) => out.checks.push(c),
                        Err(e) => errs.push(e),
                    }
                }
            }
        }
        Some(_) => errs.push("preflight phải là danh sách.".to_string()),
    }

    match obj.get("preflight_roles") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Array(arr)) => {
            if arr.len() > MAX_ROLES {
                errs.push(format!("preflight_roles tối đa {MAX_ROLES} vai."));
            } else {
                let mut roles = Vec::new();
                for r in arr {
                    match r.as_str().map(str::trim) {
                        Some(s) if valid_role(s) => {
                            if !roles.iter().any(|x: &String| x == s) {
                                roles.push(s.to_string());
                            }
                        }
                        _ => {
                            errs.push(
                                "preflight_roles chỉ chứa tên vai (chữ thường và dấu -)."
                                    .to_string(),
                            );
                            break;
                        }
                    }
                }
                out.roles = roles;
            }
        }
        Some(_) => errs.push("preflight_roles phải là danh sách tên vai.".to_string()),
    }

    match obj.get("preflight_retry_minutes") {
        None | Some(serde_json::Value::Null) => {}
        Some(v) => match v.as_u64() {
            Some(n) if n <= MAX_RETRY_MINUTES => out.retry_minutes = n,
            _ => errs.push(format!(
                "preflight_retry_minutes phải là số nguyên 0–{MAX_RETRY_MINUTES}."
            )),
        },
    }

    match obj.get("preflight_interval_seconds") {
        None | Some(serde_json::Value::Null) => {}
        Some(v) => match v.as_u64() {
            Some(0) => out.interval_seconds = 0,
            Some(n) if (MIN_INTERVAL_SECS..=MAX_INTERVAL_SECS).contains(&n) => {
                out.interval_seconds = n
            }
            _ => errs.push(format!(
                "preflight_interval_seconds phải là 0 (tắt) hoặc {MIN_INTERVAL_SECS}–{MAX_INTERVAL_SECS}."
            )),
        },
    }

    if errs.is_empty() {
        Ok(out)
    } else {
        Err(errs)
    }
}

/// Lenient read for runs: each key that fails validation keeps its default, each bad check is
/// left out (with a warning on the terminal), so a hand edit never blocks every run.
pub fn read_lenient(obj: &serde_json::Value) -> Preflight {
    if let Ok(p) = parse_settings(obj) {
        return p;
    }
    let mut out = Preflight::default();
    if let Some(arr) = obj.get("preflight").and_then(|v| v.as_array()) {
        for (i, v) in arr.iter().take(MAX_CHECKS).enumerate() {
            match parse_check(v, &format!("preflight[{i}]")) {
                Ok(c) => out.checks.push(c),
                Err(e) => crate::term::emit(&format!("Cảnh báo: bỏ qua kiểm môi trường — {e}")),
            }
        }
    }
    for key in [
        "preflight_roles",
        "preflight_retry_minutes",
        "preflight_interval_seconds",
    ] {
        if let Some(v) = obj.get(key) {
            let one = serde_json::json!({ key: v.clone() });
            if let Ok(p) = parse_settings(&one) {
                match key {
                    "preflight_roles" => out.roles = p.roles,
                    "preflight_retry_minutes" => out.retry_minutes = p.retry_minutes,
                    _ => out.interval_seconds = p.interval_seconds,
                }
            } else {
                crate::term::emit(&format!("Cảnh báo: {key} không hợp lệ — dùng mặc định"));
            }
        }
    }
    out
}

/// The JSON form of the settings as stored in `projects_map.json` (keys left out at default).
pub fn to_json_fields(p: &Preflight) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    if !p.checks.is_empty() {
        let arr: Vec<serde_json::Value> = p.checks.iter().map(check_json).collect();
        m.insert("preflight".to_string(), serde_json::Value::Array(arr));
    }
    let default_roles: Vec<String> = DEFAULT_ROLES.iter().map(|s| s.to_string()).collect();
    if p.roles != default_roles {
        m.insert("preflight_roles".to_string(), serde_json::json!(p.roles));
    }
    if p.retry_minutes > 0 {
        m.insert(
            "preflight_retry_minutes".to_string(),
            serde_json::json!(p.retry_minutes),
        );
    }
    if p.interval_seconds > 0 {
        m.insert(
            "preflight_interval_seconds".to_string(),
            serde_json::json!(p.interval_seconds),
        );
    }
    m
}

fn check_json(c: &Check) -> serde_json::Value {
    match c {
        Check::Http {
            url,
            expect_status,
            timeout_seconds,
        } => {
            let mut m = serde_json::Map::new();
            m.insert("kind".to_string(), serde_json::json!("http"));
            match url {
                UrlSource::Literal(u) => m.insert("url".to_string(), serde_json::json!(u)),
                UrlSource::Env(e) => m.insert("url_env".to_string(), serde_json::json!(e)),
            };
            if !expect_status.is_empty() {
                m.insert("expect_status".to_string(), serde_json::json!(expect_status));
            }
            m.insert("timeout_seconds".to_string(), serde_json::json!(timeout_seconds));
            serde_json::Value::Object(m)
        }
        Check::Tcp {
            host,
            port,
            timeout_seconds,
        } => serde_json::json!({
            "kind": "tcp",
            "host": host,
            "port": port,
            "timeout_seconds": timeout_seconds,
        }),
    }
}

/// Parts of an `http(s)://` URL. `None` when it is not one.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Url {
    https: bool,
    host: String,
    port: u16,
    explicit_port: bool,
    path: String,
}

fn parse_url(s: &str) -> Option<Url> {
    let (https, rest) = match s.strip_prefix("http://") {
        Some(r) => (false, r),
        None => (true, s.strip_prefix("https://")?),
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let mut path = rest[end..].to_string();
    if let Some(i) = path.find('#') {
        path.truncate(i);
    }
    if path.is_empty() || path.starts_with('?') {
        path.insert(0, '/');
    }
    // user:pass@host is never used (and never shown).
    let hostport = authority.rsplit('@').next().unwrap_or(authority);
    let (host, port) = if let Some(after) = hostport.strip_prefix('[') {
        let close = after.find(']')?;
        let host = &after[..close];
        let tail = &after[close + 1..];
        let port = match tail.strip_prefix(':') {
            Some(p) => Some(p.parse::<u16>().ok()?),
            None if tail.is_empty() => None,
            None => return None,
        };
        (host.to_string(), port)
    } else {
        match hostport.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), Some(p.parse::<u16>().ok()?)),
            None => (hostport.to_string(), None),
        }
    };
    if host.is_empty() || !valid_host(&host) || path.bytes().any(|b| b.is_ascii_control() || b == b' ') {
        return None;
    }
    Some(Url {
        https,
        host,
        port: port.unwrap_or(if https { 443 } else { 80 }),
        explicit_port: port.is_some(),
        path,
    })
}

/// `scheme://host[:port]` of a URL: the only part of it messages show.
pub fn url_origin(s: &str) -> String {
    match parse_url(s) {
        Some(u) => {
            let scheme = if u.https { "https" } else { "http" };
            let host = if u.host.contains(':') {
                format!("[{}]", u.host)
            } else {
                u.host.clone()
            };
            if u.explicit_port {
                format!("{scheme}://{host}:{}", u.port)
            } else {
                format!("{scheme}://{host}")
            }
        }
        None => "(url không hợp lệ)".to_string(),
    }
}

fn connect(host: &str, port: u16, timeout: Duration) -> Result<TcpStream, String> {
    let addrs: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|_| "không phân giải được tên máy".to_string())?
        .collect();
    if addrs.is_empty() {
        return Err("không phân giải được tên máy".to_string());
    }
    let mut last = String::from("không kết nối được");
    for a in addrs {
        match TcpStream::connect_timeout(&a, timeout) {
            Ok(s) => return Ok(s),
            Err(e) => {
                last = match e.kind() {
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
                        "hết thời gian chờ kết nối".to_string()
                    }
                    std::io::ErrorKind::ConnectionRefused => "bị từ chối kết nối".to_string(),
                    _ => "không kết nối được".to_string(),
                }
            }
        }
    }
    Err(last)
}

/// Minimal HTTP/1.1 GET over a plain TCP connection; returns the status code.
fn http_status_plain(u: &Url, timeout: Duration) -> Result<u16, String> {
    let mut stream = connect(&u.host, u.port, timeout)?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let host_header = if u.host.contains(':') {
        format!("[{}]", u.host)
    } else {
        u.host.clone()
    };
    let host_header = if u.explicit_port {
        format!("{host_header}:{}", u.port)
    } else {
        host_header
    };
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {host_header}\r\nUser-Agent: agent-brain-preflight\r\nAccept: */*\r\nConnection: close\r\n\r\n",
        u.path
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|_| "không gửi được yêu cầu".to_string())?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 512];
    while !buf.contains(&b'\n') && buf.len() < 4096 {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) => {
                return Err(match e.kind() {
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
                        "hết thời gian chờ trả lời".to_string()
                    }
                    _ => "kết nối bị ngắt".to_string(),
                });
            }
        }
    }
    let line = String::from_utf8_lossy(&buf);
    let first = line.lines().next().unwrap_or("");
    let mut parts = first.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some(v), Some(code)) if v.starts_with("HTTP/") => code
            .parse::<u16>()
            .map_err(|_| "trả lời không phải HTTP".to_string()),
        _ => Err("trả lời không phải HTTP".to_string()),
    }
}

/// `https://`: the crate's curl, the URL on stdin (`-K -`) so it never shows in a process list.
fn http_status_curl(url: &str, timeout_secs: u64) -> Result<u16, String> {
    let mut cmd = std::process::Command::new(crate::fixbug::trello::curl_bin());
    cmd.args([
        "-q",
        "-s",
        "-o",
        "/dev/null",
        "-w",
        "%{http_code}",
        "--max-time",
        &timeout_secs.to_string(),
        "--proto",
        "=http,https",
        "-K",
        "-",
    ]);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());
    let mut child = cmd.spawn().map_err(|_| "không chạy được curl".to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let cfg = format!("url = {}\n", crate::fixbug::trello::quote_cfg(url));
        let _ = stdin.write_all(cfg.as_bytes());
    }
    let out = child
        .wait_with_output()
        .map_err(|_| "không chờ được curl".to_string())?;
    let code = String::from_utf8_lossy(&out.stdout).trim().parse::<u16>().unwrap_or(0);
    if code > 0 {
        return Ok(code);
    }
    Err(match out.status.code() {
        Some(6) => "không phân giải được tên máy".to_string(),
        Some(7) => "không kết nối được".to_string(),
        Some(28) => "hết thời gian chờ".to_string(),
        Some(35) | Some(51) | Some(60) => "lỗi TLS/chứng chỉ".to_string(),
        Some(c) => format!("curl lỗi (mã {c})"),
        None => "curl bị dừng".to_string(),
    })
}

fn status_ok(code: u16, expect: &[u16]) -> bool {
    if expect.is_empty() {
        (200..400).contains(&code)
    } else {
        expect.contains(&code)
    }
}

/// A short name of the check for messages (`http://host:port`, `$NAME (https://host)`, `tcp host:port`).
fn label(c: &Check, env: &dyn Fn(&str) -> Option<String>) -> String {
    match c {
        Check::Http { url, .. } => match url {
            UrlSource::Literal(u) => url_origin(u),
            UrlSource::Env(name) => match env(name) {
                Some(u) => format!("${name} ({})", url_origin(&u)),
                None => format!("${name}"),
            },
        },
        Check::Tcp { host, port, .. } => format!("tcp {host}:{port}"),
    }
}

/// Runs one check with `env` as the environment lookup. `Err` carries a message without any URL
/// path or query.
pub fn run_check_with(c: &Check, env: &dyn Fn(&str) -> Option<String>) -> Result<(), String> {
    let name = label(c, env);
    match c {
        Check::Http {
            url,
            expect_status,
            timeout_seconds,
        } => {
            let raw = match url {
                UrlSource::Literal(u) => u.clone(),
                UrlSource::Env(n) => match env(n).map(|s| s.trim().to_string()) {
                    Some(v) if !v.is_empty() => v,
                    _ => {
                        return Err(format!(
                            "{name}: biến môi trường chưa đặt trong tiến trình agent-brain"
                        ))
                    }
                },
            };
            let Some(u) = parse_url(&raw) else {
                return Err(format!("{name}: url không hợp lệ"));
            };
            let timeout = Duration::from_secs(*timeout_seconds);
            let code = if u.https {
                http_status_curl(&raw, *timeout_seconds)
            } else {
                http_status_plain(&u, timeout)
            }
            .map_err(|e| format!("{name}: {e}"))?;
            if status_ok(code, expect_status) {
                Ok(())
            } else {
                let want = if expect_status.is_empty() {
                    "2xx/3xx".to_string()
                } else {
                    expect_status
                        .iter()
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                Err(format!("{name}: trả mã {code} (cần {want})"))
            }
        }
        Check::Tcp {
            host,
            port,
            timeout_seconds,
        } => connect(host, *port, Duration::from_secs(*timeout_seconds))
            .map(|_| ())
            .map_err(|e| format!("{name}: {e}")),
    }
}

/// Runs every check in order; the first failure is the answer.
pub fn run_checks_with(checks: &[Check], env: &dyn Fn(&str) -> Option<String>) -> Result<(), String> {
    for c in checks {
        run_check_with(c, env)?;
    }
    Ok(())
}

/// [`run_checks_with`] on this process's environment.
pub fn run_checks(checks: &[Check]) -> Result<(), String> {
    run_checks_with(checks, &|name: &str| std::env::var(name).ok())
}

/// The message of a refused run (no run is created).
pub fn refusal_message(reason: &str) -> String {
    format!("Môi trường chưa sẵn sàng — {reason}. Không tạo run.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn serve_once(status: &'static str) -> (u16, std::thread::JoinHandle<()>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                let _ = s.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\n\r\n").as_bytes());
            }
        });
        (port, h)
    }

    fn dead_port() -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn missing_config_is_default_and_applies_nowhere() {
        let p = parse_settings(&serde_json::json!({"id": "x"})).unwrap();
        assert_eq!(p, Preflight::default());
        assert!(!p.applies_to("dev"));
        assert!(!p.applies_to("qa"));
        assert_eq!(p.roles, vec!["dev", "qa"]);
        assert!(to_json_fields(&p).is_empty());
    }

    #[test]
    fn validation_limits() {
        let six: Vec<_> = (0..6)
            .map(|_| serde_json::json!({"kind": "tcp", "host": "h", "port": 1}))
            .collect();
        assert!(parse_settings(&serde_json::json!({"preflight": six})).is_err());
        for bad in [
            serde_json::json!({"preflight": [{"kind": "ftp"}]}),
            serde_json::json!({"preflight": [{"kind": "http"}]}),
            serde_json::json!({"preflight": [{"kind": "http", "url": "ftp://x"}]}),
            serde_json::json!({"preflight": [{"kind": "http", "url": "http://a", "url_env": "B"}]}),
            serde_json::json!({"preflight": [{"kind": "http", "url_env": "lower"}]}),
            serde_json::json!({"preflight": [{"kind": "http", "url": "http://a", "timeout_seconds": 0}]}),
            serde_json::json!({"preflight": [{"kind": "http", "url": "http://a", "timeout_seconds": 61}]}),
            serde_json::json!({"preflight": [{"kind": "http", "url": "http://a", "expect_status": [99]}]}),
            serde_json::json!({"preflight": [{"kind": "tcp", "host": "a", "port": 0}]}),
            serde_json::json!({"preflight": [{"kind": "tcp", "host": "a b", "port": 1}]}),
            serde_json::json!({"preflight_roles": ["Dev"]}),
            serde_json::json!({"preflight_retry_minutes": 1441}),
            serde_json::json!({"preflight_interval_seconds": 10}),
            serde_json::json!({"preflight_interval_seconds": 3601}),
            serde_json::json!({"preflight": {}}),
        ] {
            assert!(parse_settings(&bad).is_err(), "{bad}");
        }
        let ok = parse_settings(&serde_json::json!({
            "preflight": [
                {"kind": "http", "url": "https://h.example/x?y=1", "expect_status": [200, 302]},
                {"kind": "tcp", "host": "127.0.0.1", "port": 443, "timeout_seconds": 5}
            ],
            "preflight_roles": ["qa"],
            "preflight_retry_minutes": 10,
            "preflight_interval_seconds": 60
        }))
        .unwrap();
        assert_eq!(ok.checks.len(), 2);
        assert!(ok.applies_to("qa") && !ok.applies_to("dev"));
        assert_eq!(ok.retry_minutes, 10);
        assert_eq!(ok.interval_seconds, 60);
        // Round trip through the stored form.
        let back = parse_settings(&serde_json::Value::Object(to_json_fields(&ok))).unwrap();
        assert_eq!(back, ok);
    }

    #[test]
    fn lenient_read_drops_bad_entries() {
        let p = read_lenient(&serde_json::json!({
            "preflight": [{"kind": "tcp", "host": "h", "port": 1}, {"kind": "nope"}],
            "preflight_retry_minutes": -1
        }));
        assert_eq!(p.checks.len(), 1);
        assert_eq!(p.retry_minutes, 0);
    }

    #[test]
    fn origin_hides_path_query_and_credentials() {
        assert_eq!(url_origin("https://u:p@h.example/a/b?token=s3cret"), "https://h.example");
        assert_eq!(url_origin("http://127.0.0.1:8080/x"), "http://127.0.0.1:8080");
        assert_eq!(url_origin("http://[::1]:81/"), "http://[::1]:81");
    }

    #[test]
    fn http_alive_and_dead() {
        let (port, h) = serve_once("200 OK");
        let c = Check::Http {
            url: UrlSource::Literal(format!("http://127.0.0.1:{port}/health?k=secretvalue")),
            expect_status: vec![200],
            timeout_seconds: 5,
        };
        assert_eq!(run_check_with(&c, &no_env), Ok(()));
        h.join().unwrap();

        let (port, h) = serve_once("503 Service Unavailable");
        let c = Check::Http {
            url: UrlSource::Literal(format!("http://127.0.0.1:{port}/health?k=secretvalue")),
            expect_status: vec![200, 302],
            timeout_seconds: 5,
        };
        let err = run_check_with(&c, &no_env).unwrap_err();
        h.join().unwrap();
        assert!(err.contains("503"), "{err}");
        assert!(!err.contains("secretvalue") && !err.contains("/health"), "{err}");

        let port = dead_port();
        let c = Check::Http {
            url: UrlSource::Literal(format!("http://127.0.0.1:{port}/p?k=secretvalue")),
            expect_status: vec![],
            timeout_seconds: 2,
        };
        let err = run_check_with(&c, &no_env).unwrap_err();
        assert!(err.starts_with(&format!("http://127.0.0.1:{port}:")), "{err}");
        assert!(!err.contains("secretvalue"), "{err}");
    }

    #[test]
    fn url_env_reads_the_process_env_and_never_prints_the_url() {
        let port = dead_port();
        let full = format!("http://127.0.0.1:{port}/deep/path?token=topsecret");
        let full2 = full.clone();
        let env = move |n: &str| (n == "AB_TEST_TARGET").then(|| full2.clone());
        let c = Check::Http {
            url: UrlSource::Env("AB_TEST_TARGET".to_string()),
            expect_status: vec![],
            timeout_seconds: 2,
        };
        let err = run_check_with(&c, &env).unwrap_err();
        assert!(!err.contains("topsecret") && !err.contains("/deep"), "{err}");
        assert!(err.contains("$AB_TEST_TARGET"), "{err}");
        assert!(!err.contains(&full));

        let err = run_check_with(&c, &no_env).unwrap_err();
        assert!(err.contains("chưa đặt"), "{err}");
    }

    #[test]
    fn tcp_alive_and_dead() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let c = Check::Tcp {
            host: "127.0.0.1".to_string(),
            port,
            timeout_seconds: 2,
        };
        assert_eq!(run_check_with(&c, &no_env), Ok(()));
        drop(l);
        let c = Check::Tcp {
            host: "127.0.0.1".to_string(),
            port: dead_port(),
            timeout_seconds: 2,
        };
        assert!(run_check_with(&c, &no_env).is_err());
    }
}
