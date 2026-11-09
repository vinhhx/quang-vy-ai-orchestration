//! Lấy bug từ Trello và Google Sheet vào hàng chờ intake (Plan fixbug-sources-engine).
//!
//! Tải danh sách bug mới, kiểm tra trùng lặp với `fixbug_refs`, đưa vào `intake`
//! với nguồn `trello` hoặc `sheet` và ghi lại ref trong cùng một transaction.

use super::config::{load_config, normalize_match, SourceConfig};
use super::envfile::load_env;
use super::gsheet::fetch_sheet_bugs;
use super::trello::{trello_api_base, TrelloClient};
use super::{FixbugError, QA_DATA_CLOSE, QA_DATA_OPEN};
use crate::db::{fixbug_ref_exists, insert_fixbug_ref, insert_intake};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const MAX_REQUEST_CHARS: usize = 8000;
pub const DEFAULT_MAX_NEW: usize = 10;
/// Số ảnh tối đa tải cho mỗi card.
pub const MAX_IMAGES_PER_CARD: usize = 5;
/// Cỡ tối đa của một ảnh tải về (5 MiB).
pub const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;
/// Độ dài tối đa của tên file ảnh đã làm sạch.
const MAX_IMAGE_NAME: usize = 60;
pub const IMAGES_NOT_IGNORED: &str =
    "Thư mục ảnh fix-bugs/images/ chưa nằm trong .gitignore — bỏ qua tải ảnh";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewBugInfo {
    pub key: String,
    pub title: String,
    pub priority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullReport {
    pub new: Vec<NewBugInfo>,
    pub skipped_existing: usize,
    pub warnings: Vec<String>,
}

/// Ảnh upload của card Trello, tải khi bug được nhận vào hàng chờ.
pub struct PendingImage {
    pub name: String,
    pub url: String,
    pub bytes: Option<u64>,
}

pub struct BugCandidate {
    pub kind: &'static str,
    pub ext_id: String,
    pub key: String,
    pub title: String,
    pub url: String,
    pub priority: String,
    /// Dòng đầu của request (ngoài khối dữ liệu QA).
    pub header: String,
    /// Mô tả, bình luận… do QA ghi (bọc trong khối dữ liệu QA).
    pub qa_body: String,
    /// Đính kèm không tải (`(tên, URL)`), liệt kê trong khối.
    pub attachments: Vec<(String, String)>,
    /// Ảnh upload sẽ tải (chỉ card Trello).
    pub images: Vec<PendingImage>,
    /// Chỉ số client Trello dùng để tải ảnh.
    pub client_idx: Option<usize>,
    pub sort_order: i64,
}

/// Mã ngẫu nhiên 8 hex cho khối dữ liệu QA.
fn new_nonce() -> String {
    crate::telegram::compose::new_hex_id().unwrap_or_else(|_| {
        let n = crate::db::now_ms() as u64 ^ u64::from(std::process::id());
        format!("{:08x}", (n & 0xffff_ffff) as u32)
    })
}

/// Vô hiệu dòng đóng khối giả: nếu nội dung QA chứa `<<<HET_DU_LIEU_QA` thì mọi `<<<`
/// trong nội dung thành `‹‹‹`.
pub fn neutralize_qa(content: &str) -> String {
    if content.contains(QA_DATA_CLOSE.trim_end()) {
        content.replace("<<<", "‹‹‹")
    } else {
        content.to_string()
    }
}

/// `header` + khối dữ liệu QA bọc `qa_body` (cắt để cả request ≤ `max_chars`, khối luôn đóng).
pub fn build_request(header: &str, qa_body: &str, nonce: &str, max_chars: usize) -> String {
    let open = format!("{QA_DATA_OPEN}{nonce}");
    let close = format!("{QA_DATA_CLOSE}{nonce}");
    let body = neutralize_qa(qa_body.trim_end());
    let overhead = header.chars().count() + open.chars().count() + close.chars().count() + 3;
    let room = max_chars.saturating_sub(overhead).max(40);
    let body = truncate_request(&body, room);
    format!("{header}\n{open}\n{body}\n{close}")
}

/// Tên file ảnh: chữ thường, chỉ giữ `[a-z0-9._-]` (ký tự khác thành `-`), tối đa 60 ký tự.
pub fn sanitize_image_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        let c = if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' || c == '-' {
            c
        } else {
            '-'
        };
        if c == '-' && out.ends_with('-') {
            continue;
        }
        out.push(c);
    }
    let trimmed = out.trim_matches(|c| c == '-' || c == '.');
    let cut: String = trimmed.chars().take(MAX_IMAGE_NAME).collect();
    if cut.is_empty() {
        "anh".to_string()
    } else {
        cut
    }
}

/// `true` khi `rel` (đường dẫn tương đối từ gốc repo) bị git bỏ qua.
fn git_ignored(project_root: &Path, rel: &str) -> bool {
    std::process::Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(["check-ignore", "-q", "--", rel])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|st| st.success())
}

/// Tải ảnh của một card vào `<home>/fix-bugs/images/<KEY>/`; trả đường dẫn tương đối từ gốc
/// repo của ảnh đã tải. Lỗi từng ảnh chỉ thành cảnh báo.
fn download_images(
    client: &TrelloClient,
    project_root: &Path,
    key: &str,
    images: &[PendingImage],
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let home = crate::knowledge::find_or_default(project_root);
    let key_dir = sanitize_image_name(key).to_uppercase();
    let dir: PathBuf = home.dir.join("fix-bugs").join("images").join(&key_dir);
    let mut out = Vec::new();
    for (i, img) in images.iter().enumerate() {
        let file = format!("{}-{}", i + 1, sanitize_image_name(&img.name));
        let dest = dir.join(&file);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            warnings.push(format!("{key}: không tạo được thư mục ảnh ({e})"));
            return out;
        }
        match client.download(&img.url, &dest, MAX_IMAGE_BYTES) {
            Ok(_) => out.push(format!("{}/fix-bugs/images/{key_dir}/{file}", home.rel)),
            Err(e) => warnings.push(format!("{key}: không tải được ảnh '{}' ({e})", img.name)),
        }
    }
    out
}

/// Cắt chuỗi request nếu vượt quá 8000 ký tự và gắn thêm "\n…(đã cắt)".
pub fn truncate_request(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let cut_mark = "\n…(đã cắt)";
        let take_len = max_chars.saturating_sub(cut_mark.chars().count());
        let mut out: String = s.chars().take(take_len).collect();
        out.push_str(cut_mark);
        out
    }
}

/// Lấy bug từ Trello và Google Sheet cho dự án, lọc bug mới và ghi vào hàng chờ intake.
pub fn pull(
    conn: &mut Connection,
    project_root: &Path,
    project_id: i64,
    project_name: &str,
    max_new: Option<usize>,
) -> Result<PullReport, FixbugError> {
    let config = load_config(project_root)?;
    let secrets = load_env(project_root);
    let limit = max_new.unwrap_or(DEFAULT_MAX_NEW);

    let mut candidates = Vec::new();
    let mut warnings = Vec::new();
    let mut clients: Vec<TrelloClient> = Vec::new();

    for src in &config.sources {
        match src {
            SourceConfig::Trello(t_cfg) => {
                let board_id = t_cfg
                    .board
                    .as_deref()
                    .or_else(|| secrets.trello_board_id())
                    .unwrap_or("");
                if board_id.is_empty() {
                    return Err(FixbugError::Config(
                        "Cấu hình Trello thiếu board id và không có TRELLO_BOARD_ID trong .env.fixbugs"
                            .to_string(),
                    ));
                }

                let key = secrets.trello_api_key().unwrap_or("");
                let token = secrets.trello_token().unwrap_or("");
                if key.is_empty() || token.is_empty() {
                    return Err(FixbugError::Config(
                        "Thiếu TRELLO_API_KEY hoặc TRELLO_TOKEN trong .env.fixbugs".to_string(),
                    ));
                }

                let base = trello_api_base()?;
                clients.push(TrelloClient::new(&base, key, token));
                let client_idx = clients.len() - 1;
                let client = &clients[client_idx];

                let all_lists = client.lists(board_id)?;
                let mut list_map = HashMap::new(); // list_id -> list_name
                let mut list_norm_name_to_id = HashMap::new(); // normalized_name -> list_id

                for l in &all_lists {
                    list_map.insert(l.id.clone(), l.name.clone());
                    list_norm_name_to_id.insert(normalize_match(&l.name), l.id.clone());
                }

                // Kiểm tra các list trong cấu hình có tồn tại trên board không
                for intake_list_name in &t_cfg.intake_lists {
                    let norm = normalize_match(intake_list_name);
                    if !list_norm_name_to_id.contains_key(&norm) {
                        warnings.push(format!(
                            "List '{intake_list_name}' trong cấu hình không tồn tại trên board Trello"
                        ));
                    }
                }

                let all_cards = client.cards(board_id)?;

                let norm_intake_lists: Vec<String> = t_cfg
                    .intake_lists
                    .iter()
                    .map(|l| normalize_match(l))
                    .collect();

                let norm_filter_labels: Vec<String> =
                    t_cfg.labels.iter().map(|l| normalize_match(l)).collect();

                let norm_urgent_labels: Vec<String> = t_cfg
                    .priority_labels
                    .urgent
                    .iter()
                    .map(|l| normalize_match(l))
                    .collect();

                let norm_low_labels: Vec<String> = t_cfg
                    .priority_labels
                    .low
                    .iter()
                    .map(|l| normalize_match(l))
                    .collect();

                for card in all_cards {
                    // 1. Lọc theo list
                    let list_name = list_map.get(&card.id_list).map(|s| s.as_str()).unwrap_or("");
                    let norm_card_list = normalize_match(list_name);
                    if !norm_intake_lists.iter().any(|l| l == &norm_card_list) {
                        continue;
                    }

                    // 2. Lọc theo labels nếu cấu hình có khai báo labels
                    if !norm_filter_labels.is_empty() {
                        let has_label = card.labels.iter().any(|lbl| {
                            let norm_name = normalize_match(&lbl.name);
                            norm_filter_labels.iter().any(|fl| fl == &norm_name)
                        });
                        if !has_label {
                            continue;
                        }
                    }

                    // 3. Tính độ ưu tiên (urgent thắng low; không khớp -> normal)
                    let is_urgent = card.labels.iter().any(|lbl| {
                        let norm = normalize_match(&lbl.name);
                        norm_urgent_labels.iter().any(|u| u == &norm)
                    });
                    let is_low = card.labels.iter().any(|lbl| {
                        let norm = normalize_match(&lbl.name);
                        norm_low_labels.iter().any(|l| l == &norm)
                    });
                    let priority = if is_urgent {
                        "urgent"
                    } else if is_low {
                        "low"
                    } else {
                        "normal"
                    };

                    let key_str = format!("T-{}", card.id_short);

                    // 4. Lấy comments từ Trello API (bỏ comment có prefix, lấy tối đa 10 mới nhất)
                    let comments = client.comments(&card.id).unwrap_or_default();
                    let prefix = &t_cfg.comment_prefix;
                    let filtered_comments: Vec<_> = comments
                        .into_iter()
                        .filter(|c| prefix.is_empty() || !c.text.starts_with(prefix))
                        .take(10)
                        .collect();

                    let header = format!(
                        "Sửa bug {key_str} từ Trello của dự án {project_name}: {}\nLink: {}",
                        card.name, card.short_url
                    );
                    let mut qa_body = format!(
                        "--- Mô tả ---\n{}",
                        if card.desc.trim().is_empty() {
                            "(không có mô tả)"
                        } else {
                            card.desc.trim()
                        }
                    );

                    if !filtered_comments.is_empty() {
                        qa_body.push_str("\n--- Bình luận ---\n");
                        for c in &filtered_comments {
                            qa_body.push_str(&format!("- [{date} - {author}]: {text}\n",
                                date = c.date,
                                author = c.author,
                                text = c.text.trim()
                            ));
                        }
                    }

                    let mut images = Vec::new();
                    let mut attachments = Vec::new();
                    for att in &card.attachments {
                        let is_image = att.is_upload == Some(true)
                            && att
                                .mime_type
                                .as_deref()
                                .is_some_and(|m| m.starts_with("image/"));
                        if is_image && att.bytes.is_some_and(|b| b > MAX_IMAGE_BYTES) {
                            warnings.push(format!(
                                "{key_str}: ảnh '{}' lớn hơn 5 MiB — không tải",
                                att.name
                            ));
                            attachments.push((att.name.clone(), att.url.clone()));
                        } else if is_image && images.len() < MAX_IMAGES_PER_CARD {
                            images.push(PendingImage {
                                name: att.name.clone(),
                                url: att.url.clone(),
                                bytes: att.bytes,
                            });
                        } else {
                            attachments.push((att.name.clone(), att.url.clone()));
                        }
                    }

                    candidates.push(BugCandidate {
                        kind: "trello",
                        ext_id: card.id,
                        key: key_str,
                        title: card.name,
                        url: card.short_url,
                        priority: priority.to_string(),
                        header,
                        qa_body,
                        attachments,
                        images,
                        client_idx: Some(client_idx),
                        sort_order: card.id_short,
                    });
                }
            }
            SourceConfig::Gsheet(g_cfg) => {
                let bugs = fetch_sheet_bugs(g_cfg, 30)?;
                for bug in bugs {
                    let header = format!(
                        "Sửa bug {} từ Google Sheet của dự án {project_name}: {}\nLink: {}",
                        bug.key, bug.title, bug.url
                    );
                    let mut qa_body = format!("--- Mô tả ---\n{}", bug.content.trim());

                    if !bug.group.is_empty() || !bug.reporter.is_empty() {
                        qa_body.push_str("\n--- Hạng mục / Người báo ---\n");
                        if !bug.group.is_empty() {
                            qa_body.push_str(&format!("Hạng mục: {}\n", bug.group));
                        }
                        if !bug.reporter.is_empty() {
                            qa_body.push_str(&format!("Người báo: {}\n", bug.reporter));
                        }
                    }

                    let attachments = if bug.attachment.is_empty() {
                        Vec::new()
                    } else {
                        vec![(String::new(), bug.attachment.clone())]
                    };

                    candidates.push(BugCandidate {
                        kind: "sheet",
                        ext_id: bug.ext_id,
                        key: bug.key,
                        title: bug.title,
                        url: bug.url,
                        priority: bug.priority,
                        header,
                        qa_body,
                        attachments,
                        images: Vec::new(),
                        client_idx: None,
                        sort_order: bug.row_idx as i64,
                    });
                }
            }
        }
    }

    // Lọc trùng với database fixbug_refs
    let mut to_insert = Vec::new();
    let mut skipped_existing = 0;

    for candidate in candidates {
        let exists = fixbug_ref_exists(conn, project_id, candidate.kind, &candidate.ext_id)
            .map_err(|e| FixbugError::Other(e.to_string()))?;
        if exists {
            skipped_existing += 1;
        } else {
            to_insert.push(candidate);
        }
    }

    // Sắp xếp: cũ nhất trước (Trello theo idShort tăng dần, Sheet theo số thứ tự dòng tăng dần)
    to_insert.sort_by_key(|c| c.sort_order);

    // Giới hạn max_new
    if to_insert.len() > limit {
        to_insert.truncate(limit);
    }

    // Tải ảnh (chỉ bug được nhận) và dựng request với khối dữ liệu QA
    let images_allowed = if to_insert.iter().any(|c| !c.images.is_empty()) {
        let home = crate::knowledge::find_or_default(project_root);
        let probe = format!("{}/fix-bugs/images/x.png", home.rel);
        let ok = git_ignored(project_root, &probe);
        if !ok {
            warnings.push(IMAGES_NOT_IGNORED.to_string());
        }
        ok
    } else {
        false
    };

    let mut requests = Vec::with_capacity(to_insert.len());
    for c in &mut to_insert {
        let mut image_paths = Vec::new();
        if images_allowed {
            if let Some(client) = c.client_idx.and_then(|i| clients.get(i)) {
                image_paths = download_images(client, project_root, &c.key, &c.images, &mut warnings);
            }
        } else {
            for img in c.images.drain(..) {
                c.attachments.push((img.name, img.url));
            }
        }
        let mut body = c.qa_body.trim_end().to_string();
        if !image_paths.is_empty() {
            body.push_str("\n--- Ảnh ---\n");
            for p in &image_paths {
                body.push_str(&format!("- {p}\n"));
            }
        }
        if !c.attachments.is_empty() {
            body.push_str("\n--- Đính kèm ---\n");
            for (name, url) in &c.attachments {
                if name.is_empty() {
                    body.push_str(&format!("- {url}\n"));
                } else {
                    body.push_str(&format!("- {name}: {url}\n"));
                }
            }
        }
        requests.push(build_request(&c.header, &body, &new_nonce(), MAX_REQUEST_CHARS));
    }

    // Ghi vào intake và fixbug_refs trong cùng một transaction
    let tx = conn
        .transaction()
        .map_err(|e| FixbugError::Other(e.to_string()))?;

    let mut new_bugs = Vec::new();

    for (c, request) in to_insert.into_iter().zip(requests) {
        // Một bug = một task: intake của bug không được tách (split = false).
        let intake_id = insert_intake(
            &tx,
            c.kind,
            Some(project_id),
            &request,
            &c.priority,
            false,
        )
        .map_err(|e| FixbugError::Other(e.to_string()))?;

        insert_fixbug_ref(
            &tx,
            project_id,
            c.kind,
            &c.ext_id,
            &c.key,
            &c.title,
            Some(&c.url),
            Some(intake_id),
        )
        .map_err(|e| FixbugError::Other(e.to_string()))?;

        new_bugs.push(NewBugInfo {
            key: c.key,
            title: c.title,
            priority: c.priority,
        });
    }

    tx.commit().map_err(|e| FixbugError::Other(e.to_string()))?;

    Ok(PullReport {
        new: new_bugs,
        skipped_existing,
        warnings,
    })
}
