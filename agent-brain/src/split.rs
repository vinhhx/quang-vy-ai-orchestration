//! Module điều phối chia việc (split) từ task cha thành các task con.

use rusqlite::Connection;
use std::path::{Path, PathBuf};

use crate::db::{NewSplitItem, SplitItemRow, TaskRow};
use crate::error::BrainResult;
use crate::planfile::{self, SplitItem};
use crate::protocol_paths::ProtocolPaths;

/// Kết quả trạng thái của một việc con.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildOutcome {
    Pending,
    Passed,
    Failed(String),
    Cancelled,
}

/// Đọc plan đã commit từ git, trả về danh sách việc con nếu là plan tách việc.
pub fn plan_split_items(
    root: &Path,
    protocol: &ProtocolPaths,
    plan_commit: &str,
) -> BrainResult<Option<Vec<SplitItem>>> {
    let text = match crate::gitops::git(root, &["show", &protocol.plan_at(plan_commit)]) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let plan = planfile::parse_plan(&text);
    if planfile::is_split_plan(&plan) {
        Ok(Some(planfile::split_items(&plan)))
    } else {
        Ok(None)
    }
}

/// Hiện thực hoá danh sách việc con vào CSDL và chuyển trạng thái task cha thành `waiting_children`.
/// Gọi lại lần hai không tạo thêm dòng (idempotent).
pub fn materialize(
    conn: &Connection,
    parent: &TaskRow,
    plan_commit: &str,
    items: &[SplitItem],
) -> BrainResult<usize> {
    let new_items: Vec<NewSplitItem> = items
        .iter()
        .map(|it| NewSplitItem {
            idx: it.idx,
            title: it.title.clone(),
            request: it.request.clone(),
            after_idx: it.after,
        })
        .collect();
    let inserted = crate::db::insert_split_items(conn, parent.id, plan_commit, &new_items)?;
    crate::db::set_auto_state(conn, parent.id, Some("waiting_children"))?;
    crate::db::set_auto_driver(conn, parent.id, None, None)?;
    Ok(inserted)
}

/// Xác định kết quả của một việc con từ trạng thái hàng split_item và task con trong CSDL.
pub fn child_outcome(conn: &Connection, item: &SplitItemRow) -> ChildOutcome {
    match item.state.as_str() {
        "failed" | "skipped" => {
            let reason = item
                .reason
                .clone()
                .unwrap_or_else(|| "Thất bại".to_string());
            ChildOutcome::Failed(reason)
        }
        "cancelled" => ChildOutcome::Cancelled,
        "waiting" => ChildOutcome::Pending,
        "started" => {
            let Some(child_id) = item.child_task_id else {
                return ChildOutcome::Pending;
            };

            let child_status: Option<String> = conn
                .query_row(
                    "SELECT status FROM tasks WHERE id = ?1",
                    [child_id],
                    |r| r.get(0),
                )
                .ok();
            let Some(status) = child_status else {
                return ChildOutcome::Pending;
            };

            if crate::tasks::is_success_status(&status) {
                return ChildOutcome::Passed;
            }

            let auto = crate::db::task_auto(conn, child_id)
                .ok()
                .flatten()
                .unwrap_or_default();
            let aj = crate::autopilot::decide::AutoJson::parse(auto.auto_json.as_deref());

            // Task con bị hủy hẳn
            if auto.auto_state.as_deref() == Some("ended") && aj.cancelled {
                return ChildOutcome::Cancelled;
            }

            // Dừng autopilot và không có dòng quota_stops -> Thất bại
            if auto.auto_state.as_deref() == Some("stopped") {
                let has_quota_stop = crate::db::get_quota_stop(conn, child_id)
                    .ok()
                    .flatten()
                    .is_some();
                if !has_quota_stop {
                    let reason = aj
                        .stop
                        .as_ref()
                        .map(|s| s.reason.as_str().to_string())
                        .or(aj.last_error.clone())
                        .unwrap_or_else(|| "Dừng autopilot".to_string());
                    return ChildOutcome::Failed(reason);
                }
            }

            // auto_state = 'done' mà không thành công -> Thất bại
            if auto.auto_state.as_deref() == Some("done") {
                return ChildOutcome::Failed("Xong nhưng không qua QA".to_string());
            }

            // Còn lại (running, gate, waiting_human, interrupted, stopped vì quota, NULL) -> Pending
            ChildOutcome::Pending
        }
        _ => ChildOutcome::Pending,
    }
}

/// Kiểm tra xem mọi việc con của task cha đã xong (không còn Pending).
pub fn settled(conn: &Connection, parent_id: i64) -> BrainResult<bool> {
    let items = crate::db::split_items_of(conn, parent_id)?;
    if items.is_empty() {
        return Ok(false);
    }
    for item in &items {
        if child_outcome(conn, item) == ChildOutcome::Pending {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Hủy các việc con đang ở trạng thái `waiting`.
pub fn cancel_waiting(conn: &Connection, parent_id: i64) -> BrainResult<usize> {
    let now = crate::db::now_ms();
    let n = conn.execute(
        "UPDATE split_items SET state = 'cancelled', reason = 'Task cha đã hủy', updated_at = ?1
         WHERE parent_id = ?2 AND state = 'waiting'",
        rusqlite::params![now, parent_id],
    )?;
    Ok(n)
}

/// Soạn thảo nội dung markdown của báo cáo gộp.
pub fn build_report(conn: &Connection, parent: &TaskRow) -> BrainResult<String> {
    let items = crate::db::split_items_of(conn, parent.id)?;
    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_cancelled = 0;
    let total = items.len();

    let mut table_rows = Vec::new();
    let mut highlighted_errors = Vec::new();
    let mut child_report_files = Vec::new();

    let (_proj_slug, proj_path): (String, String) = conn.query_row(
        "SELECT slug, path FROM projects WHERE id = ?1",
        [parent.project_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    for item in &items {
        let outcome = child_outcome(conn, item);
        let (task_col, child_slug, child_branch, child_workdir) = if let Some(cid) = item.child_task_id {
            let row: Option<(String, String, Option<String>)> = conn
                .query_row(
                    "SELECT slug, branch, workdir FROM tasks WHERE id = ?1",
                    [cid],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .ok();
            if let Some((slug, branch, workdir)) = row {
                (format!("#{cid} {slug}"), Some(slug), Some(branch), workdir)
            } else {
                (format!("#{cid}"), None, None, None)
            }
        } else {
            ("-".to_string(), None, None, None)
        };

        let result_str = match &outcome {
            ChildOutcome::Passed => {
                total_passed += 1;
                "Đạt"
            }
            ChildOutcome::Failed(_) => {
                if item.state == "skipped" {
                    "Bỏ qua"
                } else {
                    total_failed += 1;
                    "Không đạt"
                }
            }
            ChildOutcome::Cancelled => {
                total_cancelled += 1;
                "Hủy"
            }
            ChildOutcome::Pending => "Chờ",
        };

        let task_root = child_workdir
            .as_ref()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .unwrap_or_else(|| PathBuf::from(&proj_path));

        let child_protocol = child_slug
            .as_ref()
            .map(|s| crate::protocol_paths::paths_in("", s));

        // Ghi chú trong bảng
        let mut note = match &outcome {
            ChildOutcome::Passed => {
                let mut note_text = "Đạt QA".to_string();
                if let Some(ref proto) = child_protocol {
                    let qa_file = task_root.join(&proto.qa_report);
                    if let Ok(raw) = std::fs::read_to_string(&qa_file) {
                        let conclusion = planfile::section_text(&raw, "Kết luận");
                        if !conclusion.is_empty() {
                            if let Some(first) = conclusion.lines().map(str::trim).find(|l| !l.is_empty()) {
                                note_text = first.to_string();
                            }
                        }
                    }
                }
                note_text
            }
            ChildOutcome::Failed(reason) => reason.clone(),
            ChildOutcome::Cancelled => "Hủy".to_string(),
            ChildOutcome::Pending => "Đang chờ".to_string(),
        };

        note = note.replace('|', "/");
        if note.chars().count() > 200 {
            note = note.chars().take(197).collect::<String>() + "...";
        }

        table_rows.push(format!(
            "| {} | {} | {} | {} | {} |",
            item.idx, item.title, task_col, result_str, note
        ));

        // Lỗi nổi bật cho các việc không đạt
        if let ChildOutcome::Failed(ref reason) = outcome {
            let mut error_lines = Vec::new();
            if let Some(ref proto) = child_protocol {
                let qa_file = task_root.join(&proto.qa_report);
                if let Ok(raw) = std::fs::read_to_string(&qa_file) {
                    for line in raw.lines().take(15) {
                        error_lines.push(line.to_string());
                    }
                }
            }
            if error_lines.is_empty() {
                if let Some(cid) = item.child_task_id {
                    let last_err: Option<String> = conn
                        .query_row(
                            "SELECT error FROM runs WHERE task_id = ?1 AND error IS NOT NULL ORDER BY id DESC LIMIT 1",
                            [cid],
                            |r| r.get(0),
                        )
                        .ok();
                    if let Some(err) = last_err {
                        for line in err.lines().take(15) {
                            error_lines.push(line.to_string());
                        }
                    }
                }
            }
            if error_lines.is_empty() {
                error_lines.push(reason.clone());
            }

            highlighted_errors.push(format!(
                "### Việc #{}: {} ({})\n```\n{}\n```",
                item.idx,
                item.title,
                task_col,
                error_lines.join("\n")
            ));
        }

        // File báo cáo/sản phẩm của task con
        if let Some(ref proto) = child_protocol {
            let plan_path = task_root.join(&proto.plan);
            let mut plan_text_opt = std::fs::read_to_string(&plan_path).ok();
            if plan_text_opt.is_none() {
                if let Some(ref branch) = child_branch {
                    plan_text_opt = crate::gitops::git(&task_root, &["show", &proto.plan_at(branch)]).ok();
                }
            }
            if let Some(plan_text) = plan_text_opt {
                let child_plan = planfile::parse_plan(&plan_text);
                for f in planfile::report_files(&child_plan) {
                    child_report_files.push(f);
                }
                for f in planfile::product_files(&child_plan) {
                    child_report_files.push(f);
                }
            }
        }
    }

    let total_cost: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(cost_usd), 0.0) FROM runs WHERE task_id IN (SELECT child_task_id FROM split_items WHERE parent_id = ?1 AND child_task_id IS NOT NULL)",
            [parent.id],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let mut report = String::new();
    report.push_str(&format!("# Báo cáo gộp: {}\n\n", parent.title));
    report.push_str(&format!(
        "- **Tổng kết**: Đạt {total_passed} / Không đạt {total_failed} / Hủy {total_cancelled} / Tổng {total}\n"
    ));
    report.push_str(&format!("- **Tổng chi phí**: ${total_cost:.4}\n\n"));

    report.push_str("| # | Việc con | Task | Kết quả | Ghi chú |\n");
    report.push_str("|---|---|---|---|---|\n");
    for row in table_rows {
        report.push_str(&row);
        report.push('\n');
    }
    report.push('\n');

    report.push_str("## Lỗi nổi bật\n\n");
    if highlighted_errors.is_empty() {
        report.push_str("Không có lỗi.\n\n");
    } else {
        for err_sec in highlighted_errors {
            report.push_str(&err_sec);
            report.push_str("\n\n");
        }
    }

    report.push_str("## File báo cáo của task con\n\n");
    if child_report_files.is_empty() {
        report.push_str("Không có file báo cáo.\n");
    } else {
        child_report_files.sort();
        child_report_files.dedup();
        for f in child_report_files {
            report.push_str(&format!("- {f}\n"));
        }
    }

    Ok(report)
}

/// Tổng hợp kết quả các task con: lưu báo cáo vào thư mục answers,
/// đặt trạng thái task cha thành `split_done` và `auto_state = 'done'`.
pub fn aggregate(conn: &Connection, parent: &TaskRow) -> BrainResult<PathBuf> {
    let report = build_report(conn, parent)?;
    let proj_slug: String = conn.query_row(
        "SELECT slug FROM projects WHERE id = ?1",
        [parent.project_id],
        |r| r.get(0),
    )?;
    let path = crate::answers::save(&proj_slug, parent.id, &parent.slug, &report)?;
    crate::db::set_task_status(conn, parent.id, "split_done")?;
    crate::db::set_auto_state(conn, parent.id, Some("done"))?;
    crate::db::set_auto_driver(conn, parent.id, None, None)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{open, project_row_id, sync_projects, upsert_task};
    use crate::projects::Project;

    fn fresh_task() -> (tempfile::TempDir, Connection, TaskRow) {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = open(&dir.path().join("brain.db")).expect("open db");
        let project = Project {
            id: "demo".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            name_vi: "Demo".to_string(),
            aliases: vec![],
            kind: "code".to_string(),
            profile: "default".to_string(),
            repo: None,
            missing: false,
            hidden: false,
            stack: vec![],
            read_sources: vec![],
            protocol_dir: String::new(),
            workspace: Default::default(),
        };
        sync_projects(&conn, &[project]).expect("sync projects");
        let project_id = project_row_id(&conn, "demo").expect("project id");
        let _task_id =
            upsert_task(&conn, project_id, "parent", "Cha", "task/parent", Some("main"), None)
                .expect("upsert task");
        let parent = crate::db::find_task_by_branch(&conn, project_id, "task/parent")
            .unwrap()
            .unwrap();
        (dir, conn, parent)
    }

    #[test]
    fn test_materialize_and_idempotency() {
        let (_dir, conn, parent) = fresh_task();
        let items = vec![
            SplitItem {
                idx: 1,
                title: "Viec 1".to_string(),
                request: "Req 1".to_string(),
                after: None,
            },
            SplitItem {
                idx: 2,
                title: "Viec 2".to_string(),
                request: "Req 2".to_string(),
                after: Some(1),
            },
        ];
        let n = materialize(&conn, &parent, "commit_sha", &items).unwrap();
        assert_eq!(n, 2);

        let auto = crate::db::task_auto(&conn, parent.id).unwrap().unwrap();
        assert_eq!(auto.auto_state.as_deref(), Some("waiting_children"));
        assert_eq!(auto.auto_pid, None);

        // Calling again does not insert duplicates
        let n2 = materialize(&conn, &parent, "commit_sha", &items).unwrap();
        assert_eq!(n2, 0);

        let stored = crate::db::split_items_of(&conn, parent.id).unwrap();
        assert_eq!(stored.len(), 2);
    }

    #[test]
    fn test_child_outcomes_and_settled() {
        let (_dir, conn, parent) = fresh_task();
        let items = vec![
            SplitItem {
                idx: 1,
                title: "V1".to_string(),
                request: "R1".to_string(),
                after: None,
            },
            SplitItem {
                idx: 2,
                title: "V2".to_string(),
                request: "R2".to_string(),
                after: Some(1),
            },
        ];
        materialize(&conn, &parent, "sha", &items).unwrap();

        // Ban đầu cả 2 waiting -> Pending -> chưa settled
        assert!(!settled(&conn, parent.id).unwrap());

        let list = crate::db::split_items_of(&conn, parent.id).unwrap();
        assert_eq!(child_outcome(&conn, &list[0]), ChildOutcome::Pending);

        // Tạo task con 1 và đánh dấu qua QA
        let c1 = upsert_task(&conn, parent.project_id, "c1", "C1", "task/c1", Some("main"), None)
            .unwrap();
        crate::db::set_task_parent(&conn, c1, parent.id).unwrap();
        crate::db::set_split_item_started(&conn, list[0].id, c1).unwrap();
        crate::db::set_task_status(&conn, c1, "qa_pass").unwrap();

        let list2 = crate::db::split_items_of(&conn, parent.id).unwrap();
        assert_eq!(child_outcome(&conn, &list2[0]), ChildOutcome::Passed);
        assert!(!settled(&conn, parent.id).unwrap()); // V2 vẫn waiting

        // Hàng 2 skipped do lý do
        crate::db::set_split_item_state(&conn, list2[1].id, "skipped", Some("Bỏ qua")).unwrap();
        let list3 = crate::db::split_items_of(&conn, parent.id).unwrap();
        assert_eq!(child_outcome(&conn, &list3[1]), ChildOutcome::Failed("Bỏ qua".to_string()));

        // Mọi item đều Passed hoặc Failed -> settled = true
        assert!(settled(&conn, parent.id).unwrap());
    }

    #[test]
    fn test_cancel_waiting() {
        let (_dir, conn, parent) = fresh_task();
        let items = vec![
            SplitItem {
                idx: 1,
                title: "V1".to_string(),
                request: "R1".to_string(),
                after: None,
            },
            SplitItem {
                idx: 2,
                title: "V2".to_string(),
                request: "R2".to_string(),
                after: Some(1),
            },
        ];
        materialize(&conn, &parent, "sha", &items).unwrap();

        let cancelled = cancel_waiting(&conn, parent.id).unwrap();
        assert_eq!(cancelled, 2);

        let list = crate::db::split_items_of(&conn, parent.id).unwrap();
        assert_eq!(child_outcome(&conn, &list[0]), ChildOutcome::Cancelled);
        assert_eq!(child_outcome(&conn, &list[1]), ChildOutcome::Cancelled);
        assert!(settled(&conn, parent.id).unwrap());
    }

    #[test]
    fn test_build_report_and_aggregate() {
        let (_dir, conn, parent) = fresh_task();
        let items = vec![
            SplitItem {
                idx: 1,
                title: "Đăng nhập".to_string(),
                request: "E2E đăng nhập".to_string(),
                after: None,
            },
        ];
        materialize(&conn, &parent, "sha", &items).unwrap();

        let list = crate::db::split_items_of(&conn, parent.id).unwrap();
        let c1 = upsert_task(&conn, parent.project_id, "c1", "C1", "task/c1", Some("main"), None)
            .unwrap();
        crate::db::set_task_parent(&conn, c1, parent.id).unwrap();
        crate::db::set_split_item_started(&conn, list[0].id, c1).unwrap();
        crate::db::set_task_status(&conn, c1, "qa_pass").unwrap();

        let report = build_report(&conn, &parent).unwrap();
        assert!(report.contains("# Báo cáo gộp: Cha"));
        assert!(report.contains("Đạt 1 / Không đạt 0 / Hủy 0 / Tổng 1"));
        assert!(report.contains("| 1 | Đăng nhập | #"));

        let path = aggregate(&conn, &parent).unwrap();
        assert!(path.is_file());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Báo cáo gộp: Cha"));

        let updated_parent = crate::db::find_task_by_branch(&conn, parent.project_id, "task/parent")
            .unwrap()
            .unwrap();
        assert_eq!(updated_parent.status, "split_done");
        let auto = crate::db::task_auto(&conn, parent.id).unwrap().unwrap();
        assert_eq!(auto.auto_state.as_deref(), Some("done"));
    }
}
