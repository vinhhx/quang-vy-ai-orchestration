use super::AppState;
use axum::extract::{Request, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, COOKIE, HOST, LOCATION, ORIGIN,
    SET_COOKIE,
};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

pub const COOKIE_NAME: &str = "agent_brain_token";
pub const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";

pub fn new_token() -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open("/dev/urandom")?;
    let mut buf = [0u8; 32];
    f.read_exact(&mut buf)?;
    let mut s = String::with_capacity(64);
    for b in buf {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", b);
    }
    Ok(s)
}

pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let raw = headers.get(COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|kv| {
        let (k, v) = kv.trim().split_once('=')?;
        if k == name {
            Some(v)
        } else {
            None
        }
    })
}

pub fn host_allowed(host: &str, port: u16) -> bool {
    let expected_ip = format!("127.0.0.1:{port}");
    let expected_lh = format!("localhost:{port}");
    host == expected_ip || host == expected_lh
}

pub fn origin_allowed(origin: &str, port: u16) -> bool {
    let expected_ip = format!("http://127.0.0.1:{port}");
    let expected_lh = format!("http://localhost:{port}");
    origin == expected_ip || origin == expected_lh
}

fn apply_security_headers(res: &mut Response, path: &str) {
    let headers = res.headers_mut();
    headers.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    let cache_val = if path.starts_with("/api/") || path == "/api" {
        "no-store"
    } else {
        "no-cache"
    };
    headers.insert(CACHE_CONTROL, HeaderValue::from_static(cache_val));
}

fn unauthorized_response(path: &str) -> Response {
    const MSG: &str = "Thiếu hoặc sai token — mở URL có token in ở terminal chạy agent-brain serve";
    if path.starts_with("/api/") || path == "/api" {
        let body = serde_json::json!({ "error": MSG });
        (
            StatusCode::UNAUTHORIZED,
            [(CONTENT_TYPE, "application/json; charset=utf-8")],
            axum::Json(body),
        )
            .into_response()
    } else {
        let html = format!(
            "<!doctype html><html lang=\"vi\"><head><meta charset=\"utf-8\"><title>401 Unauthorized</title></head><body><p>{MSG}</p></body></html>"
        );
        (
            StatusCode::UNAUTHORIZED,
            [(CONTENT_TYPE, "text/html; charset=utf-8")],
            html,
        )
            .into_response()
    }
}

pub async fn gate(State(st): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();

    // (a) Host header missing or not allowed -> 421
    let host_ok = req
        .headers()
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .map(|h| host_allowed(h, st.port))
        .unwrap_or(false);
    if !host_ok {
        let mut resp = (
            StatusCode::MISDIRECTED_REQUEST,
            [(CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Host không hợp lệ",
        )
            .into_response();
        apply_security_headers(&mut resp, &path);
        return resp;
    }

    // (b) GET / whose query has token=<v>
    if req.method() == Method::GET && req.uri().path() == "/" {
        let token_param = req.uri().query().and_then(|q| {
            q.split('&').find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                if k == "token" {
                    Some(v)
                } else {
                    None
                }
            })
        });

        if let Some(v) = token_param {
            if ct_eq(v.as_bytes(), st.token.as_bytes()) {
                let cookie_hdr = format!(
                    "{COOKIE_NAME}={}; HttpOnly; SameSite=Strict; Path=/",
                    st.token
                );
                let mut resp = Response::builder()
                    .status(StatusCode::SEE_OTHER)
                    .header(LOCATION, "/")
                    .header(SET_COOKIE, cookie_hdr)
                    .body(axum::body::Body::empty())
                    .unwrap();
                apply_security_headers(&mut resp, "/");
                return resp;
            } else {
                let mut resp = unauthorized_response(&path);
                apply_security_headers(&mut resp, &path);
                return resp;
            }
        }
    }

    // (c) Cookie agent_brain_token missing or not ct_eq -> 401
    let cookie_ok = cookie_value(req.headers(), COOKIE_NAME)
        .map(|c| ct_eq(c.as_bytes(), st.token.as_bytes()))
        .unwrap_or(false);
    if !cookie_ok {
        let mut resp = unauthorized_response(&path);
        apply_security_headers(&mut resp, &path);
        return resp;
    }

    // (d) For methods other than GET/HEAD: Origin check, Content-Type check for POST/PUT
    if req.method() != Method::GET && req.method() != Method::HEAD {
        let origin_ok = req
            .headers()
            .get(ORIGIN)
            .and_then(|v| v.to_str().ok())
            .map(|orig| origin_allowed(orig, st.port))
            .unwrap_or(false);
        if !origin_ok {
            let mut resp = (
                StatusCode::FORBIDDEN,
                [(CONTENT_TYPE, "application/json; charset=utf-8")],
                axum::Json(serde_json::json!({ "error": "Origin không hợp lệ" })),
            )
                .into_response();
            apply_security_headers(&mut resp, &path);
            return resp;
        }

        if req.method() == Method::POST || req.method() == Method::PUT {
            let ct_ok = req
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|ct| ct.starts_with("application/json"))
                .unwrap_or(false);
            if !ct_ok {
                let mut resp = (
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    [(CONTENT_TYPE, "application/json; charset=utf-8")],
                    axum::Json(
                        serde_json::json!({ "error": "Cần Content-Type: application/json" }),
                    ),
                )
                    .into_response();
                apply_security_headers(&mut resp, &path);
                return resp;
            }
        }
    }

    // (e) Call next
    let mut resp = next.run(req).await;
    apply_security_headers(&mut resp, &path);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ct_eq() {
        assert!(ct_eq(b"hello", b"hello"));
        assert!(!ct_eq(b"hello", b"hellp"));
        assert!(!ct_eq(b"hello", b"hello world"));
        assert!(!ct_eq(b"", b"a"));
        assert!(ct_eq(b"", b""));
    }

    #[test]
    fn test_cookie_value() {
        let mut headers = HeaderMap::new();
        assert_eq!(cookie_value(&headers, "test"), None);

        headers.insert(
            COOKIE,
            HeaderValue::from_static("foo=1; agent_brain_token=abc123xyz; bar=2"),
        );
        assert_eq!(
            cookie_value(&headers, "agent_brain_token"),
            Some("abc123xyz")
        );
        assert_eq!(cookie_value(&headers, "foo"), Some("1"));
        assert_eq!(cookie_value(&headers, "bar"), Some("2"));
        assert_eq!(cookie_value(&headers, "other"), None);
    }

    #[test]
    fn test_host_allowed() {
        assert!(host_allowed("127.0.0.1:7437", 7437));
        assert!(host_allowed("localhost:7437", 7437));
        assert!(!host_allowed("127.0.0.1:8080", 7437));
        assert!(!host_allowed("localhost:8080", 7437));
        assert!(!host_allowed("evil.com:7437", 7437));
        assert!(!host_allowed("192.168.1.1:7437", 7437));
    }

    #[test]
    fn test_origin_allowed() {
        assert!(origin_allowed("http://127.0.0.1:7437", 7437));
        assert!(origin_allowed("http://localhost:7437", 7437));
        assert!(!origin_allowed("http://127.0.0.1:8080", 7437));
        assert!(!origin_allowed("http://localhost:8080", 7437));
        assert!(!origin_allowed("https://127.0.0.1:7437", 7437));
        assert!(!origin_allowed("http://evil.com:7437", 7437));
    }

    #[test]
    fn test_new_token() {
        let t1 = new_token().expect("generate token 1");
        let t2 = new_token().expect("generate token 2");
        assert_eq!(t1.len(), 64);
        assert_eq!(t2.len(), 64);
        assert!(t1
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        assert!(t2
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        assert_ne!(t1, t2);
    }
}
