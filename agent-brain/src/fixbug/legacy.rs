//! Nhập trạng thái của skill `/fix-bugs` cũ vào `fixbug_refs` (Plan fixbug-trial A5).
//!
//! Chỉ đọc `<home>/fix-bugs/tasks/*/state.json` (không đọc README.md hay images/ vì có dữ
//! liệu cá nhân), không sửa file nào của skill cũ và không gọi Trello.

use super::{legacy_state_files, FixbugError, LEGACY_NOTE_PREFIX};
use crate::db::{fixbug_ref_exists, insert_fixbug_ref_with_state};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Cách xử lý bug cũ còn mở chưa bắt đầu sửa (`need_info`, `plan_ready`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OpenMode {
    /// Ghi ref `pulled` — agent-brain không kéo lại (mặc định).
    #[default]
    Skip,
    /// Không ghi ref — `fixbug pull` kéo bug này thành task mới của agent-brain.
    Take,
}

impl std::str::FromStr for OpenMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "skip" => Ok(Self::Skip),
            "take" => Ok(Self::Take),
            other => Err(format!("--open chỉ nhận 'skip' hoặc 'take' (nhận '{other}')")),
        }
    }
}

/// Việc sẽ làm với một bug cũ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyAction {
    /// Ghi ref với trạng thái này.
    Write(String),
    /// Không ghi ref: để `fixbug pull` kéo thành task mới.
    Take,
    /// Ref đã có (theo `project_id, kind, ext_id`).
    Exists,
}

impl LegacyAction {
    pub fn label(&self) -> String {
        match self {
            Self::Write(state) => state.clone(),
            Self::Take => "(không ghi — để pull)".to_string(),
            Self::Exists => "(đã có ref — bỏ qua)".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyItem {
    pub key: String,
    pub old_state: String,
    pub action: LegacyAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LegacyReport {
    pub items: Vec<LegacyItem>,
    /// Số ref ghi mới theo state cũ.
    pub written_by_state: BTreeMap<String, usize>,
    /// Số bug bỏ qua (ref đã có, state.json hỏng hoặc thiếu trường).
    pub skipped: usize,
    /// KEY để `fixbug pull` kéo (`--open take`).
    pub take_keys: Vec<String>,
    /// KEY skill cũ đang giữ nhánh/worktree (luôn ghi `pulled`).
    pub held_keys: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LegacyState {
    #[serde(default)]
    key: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    trello_card_id: Option<String>,
    #[serde(default)]
    trello_url: Option<String>,
    #[serde(default)]
    state: String,
}

/// Trạng thái ref cho state cũ; `None` = không ghi ref (để pull kéo).
/// Trả thêm `true` khi skill cũ đang giữ nhánh/worktree của bug.
pub fn map_state(old: &str, open: OpenMode) -> Result<(Option<&'static str>, bool), String> {
    match old {
        "done" => Ok((Some("deployed"), false)),
        "rejected" | "duplicate" => Ok((Some("rejected"), false)),
        "need_info" | "plan_ready" => match open {
            OpenMode::Skip => Ok((Some("pulled"), false)),
            OpenMode::Take => Ok((None, false)),
        },
        "approved" | "review" | "ready_to_merge" | "merged" | "blocked" | "parked" => {
            Ok((Some("pulled"), true))
        }
        other => Err(format!("state '{other}' không nhận ra")),
    }
}

/// Đọc state.json của skill cũ và ghi ref (trừ khi `dry_run`).
pub fn import_legacy(
    conn: &mut Connection,
    project_root: &Path,
    project_id: i64,
    open: OpenMode,
    dry_run: bool,
) -> Result<LegacyReport, FixbugError> {
    let mut report = LegacyReport::default();
    let files = legacy_state_files(project_root);
    let tx = conn
        .transaction()
        .map_err(|e| FixbugError::Other(e.to_string()))?;

    for path in files {
        let dir_name = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let parsed: LegacyState = match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => {
                report.skipped += 1;
                report
                    .warnings
                    .push(format!("tasks/{dir_name}/state.json đọc không được ({e}) — bỏ qua"));
                continue;
            }
        };
        let card_id = parsed
            .trello_card_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let key = parsed.key.trim().to_string();
        let Some(card_id) = card_id.filter(|_| !key.is_empty()) else {
            report.skipped += 1;
            report.warnings.push(format!(
                "tasks/{dir_name}/state.json thiếu key hoặc trello_card_id — bỏ qua"
            ));
            continue;
        };
        let old_state = parsed.state.trim().to_string();
        let (target, held) = match map_state(&old_state, open) {
            Ok(v) => v,
            Err(e) => {
                report.skipped += 1;
                report.warnings.push(format!("{key}: {e} — bỏ qua"));
                continue;
            }
        };

        let exists = fixbug_ref_exists(&tx, project_id, "trello", card_id)
            .map_err(|e| FixbugError::Other(e.to_string()))?;
        let action = if exists {
            report.skipped += 1;
            LegacyAction::Exists
        } else {
            match target {
                None => {
                    report.take_keys.push(key.clone());
                    LegacyAction::Take
                }
                Some(state) => {
                    if held {
                        report.held_keys.push(key.clone());
                    }
                    if !dry_run {
                        let title = if parsed.title.trim().is_empty() {
                            key.clone()
                        } else {
                            parsed.title.trim().to_string()
                        };
                        let note = format!("{LEGACY_NOTE_PREFIX}{old_state}");
                        insert_fixbug_ref_with_state(
                            &tx,
                            project_id,
                            "trello",
                            card_id,
                            &key,
                            &title,
                            parsed.trello_url.as_deref().filter(|u| !u.trim().is_empty()),
                            state,
                            Some(&note),
                        )
                        .map_err(|e| FixbugError::Other(e.to_string()))?;
                    }
                    *report.written_by_state.entry(old_state.clone()).or_insert(0) += 1;
                    LegacyAction::Write(state.to_string())
                }
            }
        };
        report.items.push(LegacyItem {
            key,
            old_state,
            action,
        });
    }

    if dry_run {
        drop(tx);
    } else {
        tx.commit().map_err(|e| FixbugError::Other(e.to_string()))?;
    }
    Ok(report)
}

/// Các dòng in ra terminal cho `fixbug import-legacy`.
pub fn report_lines(report: &LegacyReport, dry_run: bool) -> Vec<String> {
    let mut out = Vec::new();
    if report.items.is_empty() && report.warnings.is_empty() {
        out.push("Không có state.json nào của skill /fix-bugs cũ.".to_string());
        return out;
    }
    if dry_run {
        out.push("Chạy thử (--dry-run), không ghi gì:".to_string());
        out.push("KEY | state cũ | sẽ ghi".to_string());
        for it in &report.items {
            out.push(format!("{} | {} | {}", it.key, it.old_state, it.action.label()));
        }
    }
    let total: usize = report.written_by_state.values().sum();
    let verb = if dry_run { "Sẽ ghi" } else { "Đã ghi" };
    out.push(format!("{verb} {total} ref mới."));
    for (state, n) in &report.written_by_state {
        out.push(format!("  - {state}: {n}"));
    }
    out.push(format!("Bỏ qua: {}", report.skipped));
    if !report.take_keys.is_empty() {
        out.push(format!(
            "Để 'fixbug pull' kéo thành task mới (take): {}",
            report.take_keys.join(", ")
        ));
    }
    if !report.held_keys.is_empty() {
        out.push(format!(
            "Cảnh báo: skill cũ đang giữ nhánh/worktree của {} — luôn ghi 'pulled', xử lý tiếp bằng skill cũ.",
            report.held_keys.join(", ")
        ));
    }
    for w in &report.warnings {
        out.push(format!("Cảnh báo: {w}"));
    }
    out
}

/// KEY của các bug skill cũ mà `import-legacy` luôn ghi ref (mọi state trừ `need_info` /
/// `plan_ready`) nhưng chưa có ref theo `trello_card_id`. Rỗng = đã nhập đủ, cho pull.
/// Kiểm theo card, không theo `note`, vì `deployed` / `reject` ghi đè `note` của ref.
/// `need_info` / `plan_ready` không chặn: với `--open take` chúng cố ý không có ref.
pub fn unimported_keys(
    conn: &Connection,
    project_root: &Path,
    project_id: i64,
) -> Result<Vec<String>, FixbugError> {
    let mut out = Vec::new();
    for path in legacy_state_files(project_root) {
        let Ok(parsed) = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<LegacyState>(&t).map_err(|e| e.to_string()))
        else {
            continue;
        };
        let card_id = parsed.trello_card_id.as_deref().map(str::trim).unwrap_or("");
        let key = parsed.key.trim();
        if card_id.is_empty() || key.is_empty() {
            continue;
        }
        let always_written = matches!(
            map_state(parsed.state.trim(), OpenMode::Take),
            Ok((Some(_), _))
        );
        if !always_written {
            continue;
        }
        let exists = fixbug_ref_exists(conn, project_id, "trello", card_id)
            .map_err(|e| FixbugError::Other(e.to_string()))?;
        if !exists {
            out.push(key.to_string());
        }
    }
    Ok(out)
}
