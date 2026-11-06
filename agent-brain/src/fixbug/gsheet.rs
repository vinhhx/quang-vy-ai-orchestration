//! Tải và phân tích Google Sheet CSV (Plan fixbug-sources-engine).
//!
//! Tải CSV qua `curl` subprocess (base ghi đè bằng `AGENT_BRAIN_GSHEET_BASE`,
//! theo redirect `-L`, tối đa 5 MB).
//! Tự phân tích CSV theo RFC 4180 và tính hash FNV-1a 64-bit mà không cần thêm crate.

use super::config::{normalize_match, parse_gsheet_url, GsheetConfig};
use super::trello::{curl_argv, is_loopback_http, quote_cfg};
use super::FixbugError;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

pub const DEFAULT_GSHEET_BASE: &str = "https://docs.google.com";
pub const MAX_CSV_BYTES: usize = 5 * 1024 * 1024; // 5 MB

/// Lấy base URL cho Google Sheet từ env `AGENT_BRAIN_GSHEET_BASE` hoặc mặc định.
pub fn gsheet_base() -> Result<String, FixbugError> {
    match std::env::var("AGENT_BRAIN_GSHEET_BASE") {
        Ok(v) if !v.is_empty() => {
            if v.starts_with("https://") || is_loopback_http(&v) {
                Ok(v.trim_end_matches('/').to_string())
            } else {
                Err(FixbugError::Config(
                    "AGENT_BRAIN_GSHEET_BASE chỉ nhận https://… hoặc http://127.0.0.1:<cổng>"
                        .to_string(),
                ))
            }
        }
        _ => Ok(DEFAULT_GSHEET_BASE.to_string()),
    }
}

/// Thuật toán băm FNV-1a 64-bit tự viết (không dùng crate ngoài).
/// Offset basis: 0xcbf29ce484222325, Prime: 0x100000001b3.
pub fn fnv1a_64(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Trả về 8 ký tự hex đầu tiên của FNV-1a 64-bit(group + "\n" + content).
pub fn hash_group_content(group: &str, content: &str) -> String {
    let payload = format!("{}\n{}", group, content);
    let hash = fnv1a_64(payload.as_bytes());
    let hex_full = format!("{:016x}", hash);
    hex_full[..8].to_string()
}

/// Parser CSV RFC 4180 tự viết:
/// - Xử lý BOM UTF-8 đầu file (\u{feff})
/// - Hỗ trợ trường trong ngoặc kép "...", dấu "" thoát trong ngoặc kép
/// - Hỗ trợ xuống dòng và dấu phẩy trong ô được bọc ngoặc kép
/// - Trả về ma trận dòng/cột dạng chuỗi
pub fn parse_csv(input: &str) -> Vec<Vec<String>> {
    let s = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut rows = Vec::new();
    let mut current_row = Vec::new();
    let mut current_field = String::new();
    let mut in_quotes = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    // Cặp "" bên trong ngoặc kép -> 1 ký tự "
                    chars.next();
                    current_field.push('"');
                } else {
                    // Kết thúc trường ngoặc kép
                    in_quotes = false;
                }
            } else {
                current_field.push(c);
            }
        } else {
            match c {
                '"' => {
                    in_quotes = true;
                }
                ',' => {
                    current_row.push(current_field);
                    current_field = String::new();
                }
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    current_row.push(current_field);
                    current_field = String::new();
                    rows.push(current_row);
                    current_row = Vec::new();
                }
                '\n' => {
                    current_row.push(current_field);
                    current_field = String::new();
                    rows.push(current_row);
                    current_row = Vec::new();
                }
                other => {
                    current_field.push(other);
                }
            }
        }
    }

    if !current_field.is_empty() || !current_row.is_empty() {
        current_row.push(current_field);
        rows.push(current_row);
    }

    rows
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetBug {
    pub ext_id: String,
    pub key: String,
    pub title: String,
    pub content: String,
    pub group: String,
    pub reporter: String,
    pub attachment: String,
    pub priority: String, // "urgent", "low", hoặc "normal"
    pub row_idx: usize,
    pub url: String,
}

/// Phân tích dữ liệu text CSV tải từ Google Sheet thành danh sách SheetBug.
pub fn parse_sheet_csv(
    csv_text: &str,
    config: &GsheetConfig,
    sheet_url: &str,
    gid: &str,
) -> Result<Vec<SheetBug>, FixbugError> {
    let rows = parse_csv(csv_text);
    if rows.is_empty() {
        return Err(FixbugError::Sheet(
            "File CSV tải từ Google Sheet rỗng".to_string(),
        ));
    }

    let headers = &rows[0];

    // Tìm index cho từng cột
    let find_col = |name: &str| -> Option<usize> {
        let target = normalize_match(name);
        headers
            .iter()
            .position(|h| normalize_match(h) == target)
    };

    let headers_list = headers
        .iter()
        .map(|h| format!("'{h}'"))
        .collect::<Vec<_>>()
        .join(", ");

    let id_col_idx = find_col(&config.columns.id).ok_or_else(|| {
        FixbugError::Sheet(format!(
            "Thiếu cột bắt buộc '{}' trong Google Sheet. Các cột hiện có: [{}]",
            config.columns.id, headers_list
        ))
    })?;

    let content_col_idx = find_col(&config.columns.content).ok_or_else(|| {
        FixbugError::Sheet(format!(
            "Thiếu cột bắt buộc '{}' trong Google Sheet. Các cột hiện có: [{}]",
            config.columns.content, headers_list
        ))
    })?;

    let status_col_idx = find_col(&config.columns.status).ok_or_else(|| {
        FixbugError::Sheet(format!(
            "Thiếu cột bắt buộc '{}' trong Google Sheet. Các cột hiện có: [{}]",
            config.columns.status, headers_list
        ))
    })?;

    let group_col_idx = config.columns.group.as_deref().and_then(find_col);
    let priority_col_idx = config.columns.priority.as_deref().and_then(find_col);
    let attachment_col_idx = config.columns.attachment.as_deref().and_then(find_col);
    let reporter_col_idx = config.columns.reporter.as_deref().and_then(find_col);

    let normalized_todo_status: Vec<String> = config
        .todo_status
        .iter()
        .map(|s| normalize_match(s))
        .collect();

    let normalized_urgent_values: Vec<String> = config
        .priority_values
        .urgent
        .iter()
        .map(|s| normalize_match(s))
        .collect();

    let normalized_low_values: Vec<String> = config
        .priority_values
        .low
        .iter()
        .map(|s| normalize_match(s))
        .collect();

    let mut bugs = Vec::new();
    let mut current_group = String::new();

    for (idx, row) in rows.iter().enumerate().skip(1) {
        // Cập nhật group với carry-forward (dù dòng đó có được nhận hay không)
        if let Some(c_idx) = group_col_idx {
            if let Some(val) = row.get(c_idx) {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    current_group = trimmed.to_string();
                }
            }
        }

        let status_val = row.get(status_col_idx).map(|s| s.trim()).unwrap_or("");
        let norm_status = normalize_match(status_val);

        // Chỉ nhận dòng có status thuộc todo_status
        if !normalized_todo_status.iter().any(|s| s == &norm_status) {
            continue;
        }

        let content_val = row.get(content_col_idx).map(|s| s.trim()).unwrap_or("");
        // Chỉ nhận dòng có content khác rỗng
        if content_val.is_empty() {
            continue;
        }

        let id_val = row.get(id_col_idx).map(|s| s.trim()).unwrap_or("");
        let (ext_id, key) = if !id_val.is_empty() {
            (format!("{gid}:{id_val}"), format!("S-{id_val}"))
        } else {
            let h8 = hash_group_content(&current_group, content_val);
            (format!("{gid}:h{h8}"), format!("S-h{h8}"))
        };

        let priority_val = priority_col_idx
            .and_then(|c_idx| row.get(c_idx))
            .map(|s| s.trim())
            .unwrap_or("");
        let norm_prio = normalize_match(priority_val);
        let priority = if normalized_urgent_values.iter().any(|u| u == &norm_prio) {
            "urgent".to_string()
        } else if normalized_low_values.iter().any(|l| l == &norm_prio) {
            "low".to_string()
        } else {
            "normal".to_string()
        };

        let attachment = attachment_col_idx
            .and_then(|c_idx| row.get(c_idx))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let reporter = reporter_col_idx
            .and_then(|c_idx| row.get(c_idx))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        // Tạo tiêu đề ngắn gọn
        let first_line = content_val.lines().next().unwrap_or("").trim();
        let title = if current_group.is_empty() {
            first_line.to_string()
        } else {
            format!("{}: {}", current_group, first_line)
        };
        let title_clean = if title.chars().count() > 150 {
            format!("{}…", title.chars().take(149).collect::<String>())
        } else {
            title
        };

        bugs.push(SheetBug {
            ext_id,
            key,
            title: title_clean,
            content: content_val.to_string(),
            group: current_group.clone(),
            reporter,
            attachment,
            priority,
            row_idx: idx + 1, // 1-based row index (giống số dòng trên Sheet)
            url: sheet_url.to_string(),
        });
    }

    Ok(bugs)
}

/// Tải file CSV từ Google Sheet qua lệnh curl subprocess.
pub fn fetch_sheet_csv(
    base: &str,
    sheet_id: &str,
    gid: &str,
    max_secs: u64,
) -> Result<String, FixbugError> {
    let curl = super::trello::curl_bin();
    let sep = if base.contains('?') { "&" } else { "?" };
    let export_url = format!("{base}/spreadsheets/d/{sheet_id}/export{sep}format=csv&gid={gid}");

    let mut args = curl_argv(max_secs, base);
    // Google Sheet export cần đi theo redirect (-L)
    args.insert(1, "-L".to_string());

    let mut cmd = std::process::Command::new(&curl);
    cmd.args(&args);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = cmd.spawn().map_err(|e| {
        FixbugError::Io(format!("Không chạy được lệnh curl ({curl}): {e}"))
    })?;

    let cfg_content = format!(
        "url = {}\nwrite-out = \"\\nHTTP_STATUS:%{{http_code}}\"\n",
        quote_cfg(&export_url)
    );

    let stdin_bytes = cfg_content.into_bytes();
    let stdin_thread = child.stdin.take().map(|mut stdin| {
        std::thread::spawn(move || {
            let _ = stdin.write_all(&stdin_bytes);
            drop(stdin);
        })
    });

    let stdout_thread = child.stdout.take().map(|mut out| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 8192];
            let mut total = 0;
            while let Ok(n) = out.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                total += n;
                if total > MAX_CSV_BYTES + 200 {
                    // Vượt quá 5 MB
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            (buf, total)
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
            break Err(FixbugError::Network(
                "Quá thời gian tải Google Sheet CSV".to_string(),
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    if let Some(t) = stdin_thread {
        let _ = t.join();
    }
    let (stdout_bytes, total_read) = stdout_thread
        .and_then(|t| t.join().ok())
        .unwrap_or_default();
    let stderr_bytes = stderr_thread
        .and_then(|t| t.join().ok())
        .unwrap_or_default();

    let status = outcome?;

    if total_read > MAX_CSV_BYTES {
        return Err(FixbugError::Sheet(format!(
            "Dữ liệu Google Sheet CSV vượt quá giới hạn 5 MB (đã đọc {total_read} bytes)"
        )));
    }

    let stdout_str = String::from_utf8_lossy(&stdout_bytes).into_owned();
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
                return Err(FixbugError::Network(format!(
                    "Lỗi curl tải Google Sheet (exit {:?}): {err_msg}",
                    status.code()
                )));
            }
            (stdout_str, 200)
        }
    };

    if (200..300).contains(&http_code) {
        Ok(body)
    } else {
        Err(FixbugError::Sheet(format!(
            "Tải CSV thất bại (HTTP {http_code}): {}",
            body.trim()
        )))
    }
}

/// Tải và phân tích các bug cần sửa từ Google Sheet cấu hình.
pub fn fetch_sheet_bugs(
    config: &GsheetConfig,
    max_secs: u64,
) -> Result<Vec<SheetBug>, FixbugError> {
    let (sheet_id, gid) = parse_gsheet_url(&config.url)?;
    let base = gsheet_base()?;
    let csv_text = fetch_sheet_csv(&base, &sheet_id, &gid, max_secs)?;
    parse_sheet_csv(&csv_text, config, &config.url, &gid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_64_known_values() {
        assert_eq!(fnv1a_64(b""), 0xcbf29ce484222325);
        let h8 = hash_group_content("Admin", "Nút lưu bị hỏng");
        assert_eq!(h8.len(), 8);
    }

    #[test]
    fn parse_csv_rfc4180() {
        let input = "a,b,c\r\n1,\"2,3\",4\r\n5,\"line1\nline2\",\"quote\"\"test\"\"\r\n\"\r\n";
        let rows = parse_csv(input);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], vec!["a", "b", "c"]);
        assert_eq!(rows[1], vec!["1", "2,3", "4"]);
        assert_eq!(rows[2], vec!["5", "line1\nline2", "quote\"test\"\r\n"]);
    }

    #[test]
    fn parse_csv_with_bom() {
        let input = "\u{feff}id,name\n1,Alpha";
        let rows = parse_csv(input);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["id", "name"]);
    }

    #[test]
    fn parse_sheet_carry_forward_and_filtering() {
        let csv = "STT,Nhân sự feedback,Hạng mục,Nội dung phản hồi,Mô tả/chụp màn hình,Mức độ Ưu tiên,Phân loại,Trạng thái\n\
1,QA A,Giao diện,Nút bấm lệch,,Cao,,Hoàn Thành\n\
2,QA A,,Font chữ quá nhỏ,,Thấp,,\n\
,QA B,Chức năng,Lỗi 500 khi submit,,Gấp,,\n\
4,QA B,,Chức năng bị từ chối,,,,Từ chối thực hiện\n\
5,QA C,,,,,,,";

        let config = GsheetConfig {
            url: "https://docs.google.com/spreadsheets/d/abc/edit?gid=123".to_string(),
            columns: Default::default(),
            todo_status: vec!["".to_string()],
            priority_values: Default::default(),
        };

        let bugs = parse_sheet_csv(csv, &config, &config.url, "123").unwrap();
        // Dòng 1: Hoàn Thành -> bỏ qua
        // Dòng 2: Trạng thái trống, group carry forward từ "Giao diện", priority Thấp -> low, key S-2
        // Dòng 3: Trạng thái trống, STT trống, group "Chức năng", priority Gấp -> urgent, key S-h...
        // Dòng 4: Từ chối thực hiện -> bỏ qua
        // Dòng 5: Nội dung phản hồi trống -> bỏ qua
        assert_eq!(bugs.len(), 2);

        assert_eq!(bugs[0].key, "S-2");
        assert_eq!(bugs[0].ext_id, "123:2");
        assert_eq!(bugs[0].group, "Giao diện");
        assert_eq!(bugs[0].priority, "low");

        assert!(bugs[1].key.starts_with("S-h"));
        assert!(bugs[1].ext_id.starts_with("123:h"));
        assert_eq!(bugs[1].group, "Chức năng");
        assert_eq!(bugs[1].priority, "urgent");
    }

    #[test]
    fn parse_sheet_missing_required_column() {
        let csv = "STT,Nhân sự feedback,Hạng mục\n1,QA,Admin";
        let config = GsheetConfig {
            url: "https://docs.google.com/spreadsheets/d/abc/edit".to_string(),
            columns: Default::default(),
            todo_status: vec!["".to_string()],
            priority_values: Default::default(),
        };
        let err = parse_sheet_csv(csv, &config, &config.url, "0").unwrap_err();
        assert!(err.to_string().contains("Nội dung phản hồi"));
    }
}
