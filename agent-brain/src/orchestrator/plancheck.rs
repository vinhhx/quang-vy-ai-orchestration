//! Kiểm tra plan còn đúng không trước khi Chạy Dev từ hàng chờ (`queued_actions`).
//!
//! Chỉ ĐỌC: không merge, không rebase, không ghi gì vào git hay DB. Ba bước:
//!   (b) commit plan trên nhánh task còn tồn tại và, nếu task đã duyệt, bằng `approved_commit`;
//!   (a) file plan đọc được và `planfile::validate_plan` không có lỗi (kết quả đã tính sẵn
//!       trong `TaskState.git.plan_errors`, cùng thư mục làm việc của task);
//!   (c) không file nào trong bảng "Thay đổi" của plan (`planfile::plan_files`) đã bị sửa trên
//!       nhánh gốc kể từ `base_commit`.
//! Lỗi khi kiểm (git lỗi) là `Unsure`: không chạy Dev và cũng không đẩy task về Lead.

use crate::dashboard::api_tasks::TaskState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanCheck {
    /// Plan còn đúng — được chạy Dev.
    Ok,
    /// Plan không còn khớp (lý do ngắn) — không chạy Dev, đưa về Lead.
    Mismatch(String),
    /// Không kiểm được (git lỗi...) — giữ hàng `waiting`, thử lại ở tick sau.
    Unsure(String),
}

/// Số file đổi tối đa nêu trong lý do (để lý do và ghi chú cho Lead ngắn gọn).
const MAX_LISTED_FILES: usize = 5;

pub fn plan_check(state: &TaskState) -> PlanCheck {
    let task = &state.task;

    // (b) commit plan trên nhánh task.
    let sha = match crate::gitops::plan_commit_on_branch(
        &state.root,
        &task.branch,
        &task.slug,
        task.base_commit.as_deref(),
    ) {
        Err(()) => {
            return PlanCheck::Unsure("không đọc được lịch sử git của nhánh task".to_string())
        }
        Ok(None) => {
            return PlanCheck::Mismatch("nhánh task không còn commit plan".to_string());
        }
        Ok(Some(sha)) => sha,
    };
    if let Some(approved) = state.auto.approved_commit.as_deref() {
        if approved != sha {
            let short = |s: &str| s.chars().take(7).collect::<String>();
            return PlanCheck::Mismatch(format!(
                "commit plan {} khác commit đã duyệt {}",
                short(&sha),
                short(approved)
            ));
        }
    }

    // (a) file plan đọc được và hợp lệ.
    let text = match state.plan_text.as_deref() {
        Some(t) => t,
        None => return PlanCheck::Mismatch("không đọc được file plan của task".to_string()),
    };
    if !state.git.plan_errors.is_empty() {
        let shown: Vec<&str> = state
            .git
            .plan_errors
            .iter()
            .take(MAX_LISTED_FILES)
            .map(String::as_str)
            .collect();
        return PlanCheck::Mismatch(format!("plan không hợp lệ ({})", shown.join(", ")));
    }

    // (c) file plan nêu đã đổi trên nhánh gốc kể từ base_commit.
    let plan = crate::planfile::parse_plan(text);
    let planned: std::collections::BTreeSet<String> = crate::planfile::plan_files(&plan)
        .iter()
        .map(|f| crate::planfile::normalize_plan_path(f))
        .filter(|f| !f.is_empty())
        .collect();
    let base_commit = task.base_commit.as_deref().filter(|b| !b.is_empty());
    let base_branch = crate::workspace::merge_base_of(&state.project, task);
    if let (Some(base_commit), Some(base_branch)) = (base_commit, base_branch) {
        if !planned.is_empty() {
            match crate::gitops::changed_names_between(&state.root, base_commit, &base_branch) {
                Err(e) => {
                    return PlanCheck::Unsure(format!(
                        "không so được nhánh gốc '{}': {}",
                        base_branch,
                        first_line(&e.message)
                    ))
                }
                Ok(changed) => {
                    let hit: Vec<&String> = planned
                        .iter()
                        .filter(|f| changed.contains(f.as_str()))
                        .collect();
                    if !hit.is_empty() {
                        let mut names: Vec<&str> = hit
                            .iter()
                            .take(MAX_LISTED_FILES)
                            .map(|s| s.as_str())
                            .collect();
                        if hit.len() > MAX_LISTED_FILES {
                            names.push("...");
                        }
                        return PlanCheck::Mismatch(format!(
                            "file plan nêu đã đổi trên nhánh {} kể từ lúc tạo task: {}",
                            base_branch,
                            names.join(", ")
                        ));
                    }
                }
            }
        }
    }

    PlanCheck::Ok
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("").trim()
}
