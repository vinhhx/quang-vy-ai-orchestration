//! Logic fixbug: lấy bug từ Trello / Google Sheet vào hàng chờ, ghi ngược Trello (Plan fixbug-sources-engine).

pub mod config;
pub mod envfile;
pub mod gsheet;
pub mod init;
pub mod pull;
pub mod trello;
pub mod writeback;

use crate::error::{BrainError, EXIT_ERROR};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixbugError {
    NotConfigured(String),
    Config(String),
    Trello { status: u16, message: String },
    Sheet(String),
    Io(String),
    Network(String),
    Other(String),
}

impl std::fmt::Display for FixbugError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured(msg) => write!(f, "Chưa cấu hình fixbug: {msg}"),
            Self::Config(msg) => write!(f, "Lỗi cấu hình fixbug: {msg}"),
            Self::Trello { status, message } => write!(f, "Lỗi Trello ({status}): {message}"),
            Self::Sheet(msg) => write!(f, "Lỗi Google Sheet: {msg}"),
            Self::Io(msg) => write!(f, "Lỗi I/O: {msg}"),
            Self::Network(msg) => write!(f, "Lỗi mạng: {msg}"),
            Self::Other(msg) => write!(f, "Lỗi: {msg}"),
        }
    }
}

impl std::error::Error for FixbugError {}

impl From<FixbugError> for BrainError {
    fn from(e: FixbugError) -> Self {
        BrainError::new(e.to_string(), EXIT_ERROR)
    }
}

impl From<std::io::Error> for FixbugError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

pub fn cmd_check(ctx: &crate::app::Ctx, project_query: &str) -> i32 {
    let project = match crate::routing::resolve(project_query, &ctx.projects) {
        Ok(p) => p,
        Err(_) => {
            crate::term::emit(&format!("Không tìm thấy dự án '{project_query}'"));
            return 1;
        }
    };
    let root = std::path::Path::new(&project.path);
    crate::term::emit(&format!("Dự án: {} ({})", project.name_vi, project.id));
    crate::term::emit(&format!("Thư mục: {}", root.display()));

    let cfg = match config::load_config(root) {
        Ok(c) => c,
        Err(e) => {
            crate::term::emit(&format!("Cấu hình fixbug: {e}"));
            return 1;
        }
    };

    let secrets = envfile::load_env(root);
    crate::term::emit("Kiểm tra bí mật (.env.fixbugs):");
    crate::term::emit(&format!(
        "  - TRELLO_API_KEY: {}",
        if secrets.trello_api_key().is_some_and(|k| !k.is_empty()) { "Có" } else { "Thiếu" }
    ));
    crate::term::emit(&format!(
        "  - TRELLO_TOKEN: {}",
        if secrets.trello_token().is_some_and(|k| !k.is_empty()) { "Có" } else { "Thiếu" }
    ));
    crate::term::emit(&format!(
        "  - TRELLO_BOARD_ID: {}",
        if secrets.trello_board_id().is_some_and(|k| !k.is_empty()) { "Có" } else { "Thiếu" }
    ));

    for (i, src) in cfg.sources.iter().enumerate() {
        crate::term::emit(&format!("\nNguồn #{}:", i + 1));
        match src {
            config::SourceConfig::Trello(t) => {
                crate::term::emit("  Loại: Trello");
                let board = t.board.as_deref().or_else(|| secrets.trello_board_id()).unwrap_or("");
                crate::term::emit(&format!("  Board ID: {}", if board.is_empty() { "(chưa có)" } else { board }));
                crate::term::emit(&format!("  Intake lists: {:?}", t.intake_lists));
                crate::term::emit(&format!("  Fixed list: {}", t.fixed_list));
                crate::term::emit(&format!("  Rejected list: {}", t.rejected_list));

                if let (Some(k), Some(tok)) = (secrets.trello_api_key(), secrets.trello_token()) {
                    if !board.is_empty() && !k.is_empty() && !tok.is_empty() {
                        match trello::trello_api_base() {
                            Ok(base) => {
                                let client = trello::TrelloClient::new(&base, k, tok);
                                match client.lists(board) {
                                    Ok(lists) => {
                                        crate::term::emit("  Danh sách cột trên board:");
                                        for l in lists {
                                            let norm = config::normalize_match(&l.name);
                                            let mut role = Vec::new();
                                            if t.intake_lists.iter().any(|il| config::normalize_match(il) == norm) {
                                                role.push("intake");
                                            }
                                            if config::normalize_match(&t.fixed_list) == norm {
                                                role.push("fixed");
                                            }
                                            if config::normalize_match(&t.rejected_list) == norm {
                                                role.push("rejected");
                                            }
                                            if !role.is_empty() {
                                                crate::term::emit(&format!("    [✓] {} ({})", l.name, role.join(", ")));
                                            } else {
                                                crate::term::emit(&format!("    [ ] {}", l.name));
                                            }
                                        }
                                    }
                                    Err(e) => crate::term::emit(&format!("  Lỗi lấy danh sách cột: {e}")),
                                }
                            }
                            Err(e) => crate::term::emit(&format!("  Lỗi API base: {e}")),
                        }
                    }
                }
            }
            config::SourceConfig::Gsheet(g) => {
                crate::term::emit("  Loại: Google Sheet");
                crate::term::emit(&format!("  URL: {}", g.url));
                match gsheet::fetch_sheet_bugs(g, 30) {
                    Ok(bugs) => {
                        crate::term::emit(&format!("  Số dòng cần sửa thỏa mãn điều kiện: {}", bugs.len()));
                    }
                    Err(e) => crate::term::emit(&format!("  Lỗi đọc sheet: {e}")),
                }
            }
        }
    }
    0
}

pub fn cmd_pull(ctx: &mut crate::app::Ctx, project_query: &str, max: Option<usize>) -> i32 {
    let project = match crate::routing::resolve(project_query, &ctx.projects) {
        Ok(p) => p,
        Err(_) => {
            crate::term::emit(&format!("Không tìm thấy dự án '{project_query}'"));
            return 1;
        }
    };
    let project_id = match crate::db::project_row_id(&ctx.conn, &project.id) {
        Ok(id) => id,
        Err(e) => {
            crate::term::emit(&format!("Dự án '{}' chưa có trong CSDL: {e}", project.id));
            return 1;
        }
    };
    let root = std::path::Path::new(&project.path);
    match pull::pull(&mut ctx.conn, root, project_id, &project.name_vi, max) {
        Ok(report) => {
            if report.new.is_empty() {
                crate::term::emit("Không có bug mới nào cần lấy.");
            } else {
                crate::term::emit(&format!("Đã lấy {} bug mới vào hàng chờ intake:", report.new.len()));
                for b in &report.new {
                    crate::term::emit(&format!("  - [{}] {}: {}", b.priority, b.key, b.title));
                }
            }
            if report.skipped_existing > 0 {
                crate::term::emit(&format!("Bỏ qua {} bug đã tồn tại trong hàng chờ.", report.skipped_existing));
            }
            for w in &report.warnings {
                crate::term::emit(&format!("Cảnh báo: {w}"));
            }
            0
        }
        Err(e) => {
            crate::term::emit(&format!("Lỗi khi lấy bug: {e}"));
            1
        }
    }
}

pub fn cmd_list(ctx: &crate::app::Ctx, project_query: &str) -> i32 {
    let project = match crate::routing::resolve(project_query, &ctx.projects) {
        Ok(p) => p,
        Err(_) => {
            crate::term::emit(&format!("Không tìm thấy dự án '{project_query}'"));
            return 1;
        }
    };
    let project_id = match crate::db::project_row_id(&ctx.conn, &project.id) {
        Ok(id) => id,
        Err(e) => {
            crate::term::emit(&format!("Dự án '{}' chưa có trong CSDL: {e}", project.id));
            return 1;
        }
    };
    match crate::db::list_fixbug_refs(&ctx.conn, project_id) {
        Ok(refs) => {
            if refs.is_empty() {
                crate::term::emit(&format!("Dự án '{}' chưa có bug nào được lấy.", project.id));
            } else {
                crate::term::emit(&format!("Danh sách bug dự án '{}' ({} mục):", project.id, refs.len()));
                for r in refs {
                    let it_str = r.intake_id.map(|id| format!("#{id}")).unwrap_or_else(|| "—".to_string());
                    crate::term::emit(&format!("  {} | {:<8} | {:<6} | intake: {:<5} | {}", r.key, r.state, r.kind, it_str, r.title));
                }
            }
            0
        }
        Err(e) => {
            crate::term::emit(&format!("Lỗi truy vấn CSDL: {e}"));
            1
        }
    }
}

pub fn cmd_deployed(
    ctx: &mut crate::app::Ctx,
    project_query: &str,
    keys: &[String],
    note: Option<&str>,
) -> i32 {
    let project = match crate::routing::resolve(project_query, &ctx.projects) {
        Ok(p) => p,
        Err(_) => {
            crate::term::emit(&format!("Không tìm thấy dự án '{project_query}'"));
            return 1;
        }
    };
    let project_id = match crate::db::project_row_id(&ctx.conn, &project.id) {
        Ok(id) => id,
        Err(e) => {
            crate::term::emit(&format!("Dự án '{}' chưa có trong CSDL: {e}", project.id));
            return 1;
        }
    };
    let root = std::path::Path::new(&project.path);
    match writeback::deployed(&mut ctx.conn, root, project_id, keys, note) {
        Ok(items) => {
            for item in items {
                crate::term::emit(&format!("[✓] {}: {}", item.key, item.message));
            }
            0
        }
        Err(e) => {
            crate::term::emit(&format!("Lỗi deployed: {e}"));
            1
        }
    }
}

pub fn cmd_reject(
    ctx: &mut crate::app::Ctx,
    project_query: &str,
    keys: &[String],
    reason: &str,
) -> i32 {
    let project = match crate::routing::resolve(project_query, &ctx.projects) {
        Ok(p) => p,
        Err(_) => {
            crate::term::emit(&format!("Không tìm thấy dự án '{project_query}'"));
            return 1;
        }
    };
    let project_id = match crate::db::project_row_id(&ctx.conn, &project.id) {
        Ok(id) => id,
        Err(e) => {
            crate::term::emit(&format!("Dự án '{}' chưa có trong CSDL: {e}", project.id));
            return 1;
        }
    };
    let root = std::path::Path::new(&project.path);
    match writeback::reject(&mut ctx.conn, root, project_id, keys, reason) {
        Ok(items) => {
            for item in items {
                crate::term::emit(&format!("[✓] {}: {}", item.key, item.message));
            }
            0
        }
        Err(e) => {
            crate::term::emit(&format!("Lỗi reject: {e}"));
            1
        }
    }
}

pub fn cmd_init(ctx: &crate::app::Ctx, project_query: &str, dry_run: bool) -> i32 {
    let project = match crate::routing::resolve(project_query, &ctx.projects) {
        Ok(p) => p,
        Err(_) => {
            crate::term::emit(&format!("Không tìm thấy dự án '{project_query}'"));
            return 1;
        }
    };
    let root = std::path::Path::new(&project.path);
    match init::init(root, dry_run) {
        Ok(content) => {
            if dry_run {
                crate::term::emit("Cấu hình fixbugs.json đề xuất (--dry-run):");
                println!("{content}");
            } else {
                crate::term::emit(&format!(
                    "Đã khởi tạo file cấu hình: {}",
                    config::config_path(root).display()
                ));
            }
            0
        }
        Err(e) => {
            crate::term::emit(&format!("Lỗi init: {e}"));
            1
        }
    }
}
