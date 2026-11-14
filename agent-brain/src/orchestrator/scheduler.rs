//! Bộ lập lịch điều phối (Scheduler) cho orchestrator:
//! - Duyệt proposal approved theo thứ tự ưu tiên (urgent > normal > low), tối đa max_parallel.
//! - Task hết quota không tính vào slot.
//! - Quét task dừng vì quota ghi vào bảng quota_stops (cột notified giữ nguyên 0, thông báo Telegram do telegram/detect.rs đảm nhận tự động).
//! - Tự động tiếp tục (auto_resume) khi auto_resume_quota = true và đã qua resume_after_minutes (chỉ một lần mỗi lần dừng).
//! - Không tự đổi engine.

use crate::dashboard::api_tasks::{
    find_task_action, is_busy_reason, load_task_state, queued_action_label, queued_action_role,
    run_control_as, run_task_action, start_new_task, ActionSource, ControlOutcome, Origin,
    StartOutcome, StartRequest,
};
use crate::orchestrator::plancheck::{plan_check, PlanCheck};
use crate::dashboard::AppState;
use crate::error::{BrainError, BrainResult};
use crate::tracing;

/// Tính số task đang chạy trong hệ thống: bao gồm các task có run hoặc auto_state đang 'running' trong CSDL,
/// và các job đang chạy trong JobManager (không tính trùng).
pub(crate) fn active_task_count(st: &AppState, conn: &rusqlite::Connection) -> BrainResult<u32> {
    let mut active_task_ids = std::collections::HashSet::new();
    let mut stmt = conn.prepare(
        "SELECT task_id FROM runs WHERE status = 'running' AND task_id IS NOT NULL
         UNION
         SELECT id FROM tasks WHERE auto_state = 'running'",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    for id in rows {
        active_task_ids.insert(id?);
    }

    let mut unassociated_jobs = 0;
    for job in st.jobs.list() {
        if job.state == "running" {
            if let Some(tid) = job.task_id {
                active_task_ids.insert(tid);
            } else {
                unassociated_jobs += 1;
            }
        }
    }

    Ok((active_task_ids.len() + unassociated_jobs) as u32)
}

/// Nhịp xử lý của bộ lập lịch: gọi định kỳ bởi orchestrator loop
pub fn schedule_tick(st: &AppState) -> BrainResult<()> {
    // C2: environment reruns follow the project's own `preflight_retry_minutes`, with or without
    // the orchestrator. Nothing is opened unless some project sets it.
    let map = crate::projects::load_map(&st.map_path).unwrap_or_default();
    if any_env_retry(&map) {
        let conn = crate::db::open(&st.db_path)?;
        handle_env_stops(st, &conn, &map, crate::db::now_ms())?;
    }

    // Hành động xếp hàng chờ slot chạy khi dự án hết bận, kể cả khi orchestrator tắt
    // (như `handle_env_stops`): đó là việc người dùng đã yêu cầu tay.
    {
        let conn = crate::db::open(&st.db_path)?;
        handle_queued_actions(st, &conn)?;
        if let Err(e) = handle_split_items(st, &conn) {
            crate::term::emit(&format!("Hàng chờ việc con: lỗi — {}", e.message));
        }
        // Tự dọn worktree task xong và nhánh đã merge (cũng khi orchestrator tắt); lỗi chỉ ghi
        // ra terminal, không làm hỏng tick.
        if let Err(e) = crate::orchestrator::housekeeping::run_if_due(st, &conn, crate::db::now_ms()) {
            crate::term::emit(&format!("Dọn tự động: lỗi — {}", e.message));
        }
    }

    let orch = st.orch();
    if !orch.enabled {
        return Ok(());
    }

    let conn = crate::db::open(&st.db_path)?;
    let max_parallel = orch.max_parallel;

    // (b) & (c): Quét các task dừng vì quota để ghi vào quota_stops và xử lý auto_resume nếu có
    handle_quota_stops(st, &conn)?;

    // (a): Bắt đầu các proposal approved theo thứ tự ưu tiên
    let mut active = active_task_count(st, &conn)?;
    let approved_props = crate::db::list_approved_proposals(&conn)?;
    let workflows = crate::workflows::load(&st.cfg);

    for prop in approved_props {
        if active >= max_parallel {
            break;
        }

        // Xử lý chuỗi việc (after_proposal_id):
        let mut prior_answer = None;
        if let Some(prior_id) = prop.after_proposal_id {
            match crate::db::get_proposal(&conn, prior_id)? {
                Some(prior_p) => {
                    if prior_p.status == "rejected" || prior_p.status == "failed" {
                        crate::db::set_proposal_failed(
                            &conn,
                            prop.id,
                            &format!("Việc trước (#{prior_id}) không có kết quả"),
                        )?;
                        continue;
                    }
                    if prior_p.status != "started" {
                        // Việc trước chưa bắt đầu xong: bỏ qua lượt này
                        continue;
                    }
                    // Việc trước đã started: kiểm tra task của nó
                    let prior_task_id = match prior_p.task_id {
                        Some(tid) => tid,
                        None => continue,
                    };
                    struct PriorTaskInfo {
                        status: String,
                        task_slug: String,
                        project_slug: String,
                        auto_state: Option<String>,
                        auto_json_str: Option<String>,
                    }
                    let prior_task: Option<PriorTaskInfo> = conn
                        .query_row(
                            "SELECT t.status, t.slug, p.slug, t.auto_state, t.auto_json FROM tasks t
                             JOIN projects p ON p.id = t.project_id
                             WHERE t.id = ?1",
                            [prior_task_id],
                            |r| {
                                Ok(PriorTaskInfo {
                                    status: r.get(0)?,
                                    task_slug: r.get(1)?,
                                    project_slug: r.get(2)?,
                                    auto_state: r.get(3)?,
                                    auto_json_str: r.get(4)?,
                                })
                            },
                        )
                        .ok();

                    match prior_task {
                        Some(PriorTaskInfo {
                            status,
                            task_slug: t_slug,
                            project_slug: p_slug,
                            auto_state,
                            auto_json_str,
                        }) => {
                            let is_success = crate::tasks::is_success_status(&status);
                            if is_success {
                                // B2: an analysis hands its answer on as a document; code must
                                // first be merged into its base branch.
                                let prior_kind = workflows
                                    .get(&prior_p.workflow)
                                    .map(|w| w.kind.as_str())
                                    .unwrap_or("code");
                                if prior_kind == "analysis" || status == "answered" {
                                    if status == "answered" {
                                        match crate::answers::read(&p_slug, prior_task_id, &t_slug) {
                                            Ok(Some(ans)) => {
                                                prior_answer = Some((prior_task_id, ans));
                                            }
                                            _ => {
                                                // Chưa đọc được câu trả lời: bỏ qua chờ
                                                continue;
                                            }
                                        }
                                    }
                                } else if let Some(wait) =
                                    code_prior_wait(&conn, &map, &p_slug, prior_task_id)?
                                {
                                    if prop.error.as_deref() != Some(wait.as_str()) {
                                        crate::db::set_proposal_error(&conn, prop.id, &wait)?;
                                        // Wakes the Telegram detector so the wait is reported now.
                                        let _ = st.hints.send(0);
                                    }
                                    continue;
                                }
                            } else {
                                let is_ended = auto_state.as_deref() == Some("ended")
                                    || (auto_state.is_none() && matches!(status.as_str(), "failed" | "killed" | "timed_out" | "lost" | "rejected"));
                                if is_ended {
                                    crate::db::set_proposal_failed(
                                        &conn,
                                        prop.id,
                                        &format!("Việc trước (#{prior_id}) không có kết quả"),
                                    )?;
                                    continue;
                                }
                                if auto_state.as_deref() == Some("stopped") {
                                    let aj = crate::autopilot::decide::AutoJson::parse(auto_json_str.as_deref());
                                    let reason_str = aj
                                        .stop
                                        .as_ref()
                                        .map(|s| crate::autopilot::view::reason_word(s.reason))
                                        .unwrap_or("dừng");
                                    let wait_msg = format!("Chờ task #{prior_task_id} (đang dừng: {reason_str})");
                                    crate::db::set_proposal_error(&conn, prop.id, &wait_msg)?;
                                }
                                continue;
                            }
                        }
                        None => {
                            crate::db::set_proposal_failed(
                                &conn,
                                prop.id,
                                &format!("Việc trước (#{prior_id}) không có kết quả"),
                            )?;
                            continue;
                        }
                    }
                }
                None => {
                    crate::db::set_proposal_failed(
                        &conn,
                        prop.id,
                        &format!("Việc trước (#{prior_id}) không có kết quả"),
                    )?;
                    continue;
                }
            }
        }

        let project_slug: String = match conn.query_row(
            "SELECT slug FROM projects WHERE id = ?1",
            rusqlite::params![prop.project_id],
            |r| r.get(0),
        ) {
            Ok(s) => s,
            Err(_) => {
                crate::db::set_proposal_failed(&conn, prop.id, "Dự án không tồn tại trong CSDL")?;
                continue;
            }
        };

        if let Some((prior_tid, ref ans_content)) = prior_answer {
            let doc_name = format!("phan-tich-{prior_tid}.md");
            let doc_source = format!("answer:{prior_tid}");
            let secrets = crate::inputs::collect_secrets(st);
            if let Err(e) = crate::inputs::add_to_proposal(
                &conn,
                prop.id,
                &doc_name,
                &doc_source,
                ans_content,
                &secrets,
            ) {
                let err_msg = format!("Không chuyển được tài liệu {doc_name}: {e}");
                tracing::warn!("{err_msg}");
                crate::db::set_proposal_failed(&conn, prop.id, &err_msg)?;
                continue;
            }
        }

        let title_slug = crate::routing::slugify(&prop.title);
        let name_opt = if title_slug.is_empty() {
            None
        } else {
            Some(title_slug.as_str())
        };

        let req = StartRequest {
            project: &project_slug,
            request: &prop.request,
            name: name_opt,
            workflow: Some(&prop.workflow),
            level: Some(&prop.level),
            team: Some(&prop.team),
            inputs: None,
            origin: Origin::Dashboard,
            dry_run: false,
        };

        match start_new_task(st, &req)? {
            StartOutcome::Started { task_id, .. } => {
                active += 1;
                if let Err(e) = crate::inputs::move_proposal_inputs_to_task(&conn, prop.id, task_id) {
                    tracing::warn!("Lỗi chuyển tài liệu từ proposal #{} sang task #{}: {}", prop.id, task_id, e);
                }
                crate::db::set_proposal_started(&conn, prop.id, Some(task_id))?;
                if prop.error.is_some() {
                    // The wait that held it back ("Chờ task #N …") no longer applies.
                    conn.execute(
                        "UPDATE proposals SET error = NULL WHERE id = ?1",
                        rusqlite::params![prop.id],
                    )?;
                }
            }
            StartOutcome::PreflightRefused { reason, kind } => {
                if kind == "busy" {
                    // Dự án đang bận (có job chạy): tạm thời, giữ approved và thử proposal tiếp theo
                    continue;
                }
                crate::db::set_proposal_failed(&conn, prop.id, &reason)?;
            }
            StartOutcome::Busy(_) => {
                // Tạm thời bận, giữ approved và thử proposal tiếp theo
                continue;
            }
            StartOutcome::Invalid(reason) => {
                crate::db::set_proposal_failed(&conn, prop.id, &reason)?;
            }
            StartOutcome::ProjectRefused { body, .. } => {
                let err_str = body
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Dự án bị từ chối");
                crate::db::set_proposal_failed(&conn, prop.id, err_str)?;
            }
            StartOutcome::Unavailable => {
                break;
            }
            StartOutcome::Ready(_) => {
                break;
            }
        }
    }

    Ok(())
}

/// Prefix of the proposal error written while a chained proposal waits for the merge of the
/// earlier code task (B2); `telegram::detect::ChainWaits` reports it once (B3).
pub const CHAIN_WAIT_PREFIX: &str = "Chờ task #";

/// B2: why a proposal chained after a finished code task must still wait, `None` = it may run.
/// `base` is the earlier task's merge base, `branch_exists` / `merged` its branch's state in
/// the project repo.
pub fn code_wait_reason(
    prior_task_id: i64,
    branch: &str,
    base: Option<&str>,
    branch_exists: bool,
    merged: bool,
) -> Option<String> {
    let Some(base) = base else {
        return Some(format!(
            "Task #{prior_task_id} không có nhánh gốc — merge tay rồi duyệt lại"
        ));
    };
    if !branch_exists {
        return Some(format!(
            "{CHAIN_WAIT_PREFIX}{prior_task_id} được merge vào {base} (nhánh {branch} không còn — coi như chưa merge)"
        ));
    }
    if !merged {
        return Some(format!(
            "{CHAIN_WAIT_PREFIX}{prior_task_id} được merge vào {base}"
        ));
    }
    None
}

/// Reads the earlier code task and its project, then applies [`code_wait_reason`].
fn code_prior_wait(
    conn: &rusqlite::Connection,
    map: &serde_json::Value,
    project_slug: &str,
    prior_task_id: i64,
) -> BrainResult<Option<String>> {
    let Some(task) = crate::db::get_task(conn, prior_task_id)? else {
        return Ok(Some(format!(
            "Task #{prior_task_id} không có nhánh gốc — merge tay rồi duyệt lại"
        )));
    };
    let Some(project) = crate::projects::projects_of(map)
        .into_iter()
        .find(|p| p.id == project_slug)
    else {
        return Ok(Some(format!(
            "Task #{prior_task_id}: dự án {project_slug} không còn trong danh sách dự án"
        )));
    };
    let base = crate::workspace::merge_base_of(&project, &task);
    let root = project.root();
    let exists = crate::gitops::branch_exists(&root, &task.branch);
    let merged = base
        .as_deref()
        .is_some_and(|b| crate::gitops::is_merged_into(&root, &task.branch, b));
    Ok(code_wait_reason(
        prior_task_id,
        &task.branch,
        base.as_deref(),
        exists,
        merged,
    ))
}

/// Deletes the `quota_stops` rows of tasks no longer stopped on quota (`db::quota_stop_is_stale`),
/// except the tasks in `keep` (the ones the current scan found stopped). Returns how many went.
fn prune_stale_quota_stops(conn: &rusqlite::Connection, keep: &[i64]) -> BrainResult<usize> {
    let mut removed = 0;
    for qs in crate::db::list_quota_stops(conn)? {
        if keep.contains(&qs.task_id) {
            continue;
        }
        if crate::db::quota_stop_is_stale(conn, qs.task_id)?
            && crate::db::delete_quota_stop(conn, qs.task_id)?
        {
            removed += 1;
        }
    }
    Ok(removed)
}

fn handle_quota_stops(st: &AppState, conn: &rusqlite::Connection) -> BrainResult<()> {
    let now = crate::db::now_ms();

    // Quét các task dừng vì quota: auto_stop_kind = 'het_quota' hoặc run cuối có outcome = 'quota'
    // Lưu ý: Chỉ xét task không đang chạy (auto_state != 'running' hoặc NULL)
    let mut stmt = conn.prepare(
        "SELECT t.id, t.auto_stop_at, COALESCE(t.auto_stop_kind, ''), t.project_id, t.branch, t.slug,
                p.slug AS project_slug, p.profile AS project_profile,
                r.engine, r.ended_at, r.outcome
         FROM tasks t
         JOIN projects p ON p.id = t.project_id
         LEFT JOIN runs r ON r.id = (
             SELECT id FROM runs WHERE task_id = t.id ORDER BY started_at DESC, id DESC LIMIT 1
         )
         WHERE (t.auto_stop_kind = 'het_quota' OR r.outcome = 'quota')
           AND (t.auto_state IS NULL OR t.auto_state != 'running')
           AND EXISTS (
               SELECT 1 FROM proposals pr WHERE pr.task_id = t.id AND pr.status = 'started'
           )",
    )?;

    struct QuotaCandidate {
        task_id: i64,
        stopped_at: i64,
        engine: String,
        project_slug: String,
        project_profile: String,
        branch: String,
    }

    let candidates: Vec<QuotaCandidate> = stmt
        .query_map([], |row| {
            let task_id: i64 = row.get(0)?;
            let auto_stop_at: Option<i64> = row.get(1)?;
            let project_slug: String = row.get(6)?;
            let project_profile: String = row.get(7)?;
            let branch: String = row.get(4)?;
            let run_engine: Option<String> = row.get(8)?;
            let run_ended_at: Option<i64> = row.get(9)?;

            let engine = run_engine.unwrap_or_else(|| "claude".to_string());
            let stopped_at = auto_stop_at
                .filter(|&t| t > 0)
                .or_else(|| run_ended_at.filter(|&t| t > 0))
                .unwrap_or(now);

            Ok(QuotaCandidate {
                task_id,
                stopped_at,
                engine,
                project_slug,
                project_profile,
                branch,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // (b) Ghi quota_stops khi phát hiện dừng lần đầu.
    // Cột notified để nguyên 0: thông báo Telegram do telegram/detect.rs đảm nhận tự động.
    for cand in &candidates {
        crate::db::insert_quota_stop(conn, cand.task_id, cand.stopped_at, &cand.engine)?;
    }

    // (b2) Dọn dòng quota_stops cũ của task đã được chạy lại (bằng tay, dashboard, Telegram),
    // không phụ thuộc auto_resume_quota.
    let keep: Vec<i64> = candidates.iter().map(|c| c.task_id).collect();
    prune_stale_quota_stops(conn, &keep)?;

    // (c) auto_resume chỉ khi auto_resume_quota = true và đã qua resume_after_minutes
    let orch = st.orch();
    if orch.auto_resume_quota {
        let resume_after_ms = orch.resume_after_minutes as i64 * 60 * 1000;
        let quota_stops = crate::db::list_quota_stops(conn)?;

        for qs in quota_stops {
            // C3: never before the engine's own reset time when the run reported one.
            let reset_at = crate::db::latest_quota_reset_at(conn, qs.task_id);
            if now >= crate::quota::resume_due_ms(qs.stopped_at, resume_after_ms, reset_at) {
                // Giới hạn auto_resume chỉ cho task có trong proposals (proposals.task_id = tasks.id và proposals.status = started)
                let has_started_proposal: bool = conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM proposals WHERE task_id = ?1 AND status = 'started')",
                        rusqlite::params![qs.task_id],
                        |r| r.get(0),
                    )
                    .unwrap_or(false);
                if !has_started_proposal {
                    let _ = crate::db::delete_quota_stop(conn, qs.task_id);
                    continue;
                }

                // Kiểm tra xem task có đang chạy job không
                let already_running = st
                    .jobs
                    .list()
                    .iter()
                    .any(|j| j.task_id == Some(qs.task_id) && j.state == "running");
                if already_running {
                    continue;
                }

                let cand_opt = candidates.iter().find(|c| c.task_id == qs.task_id);
                let (proj_slug, proj_prof, branch) = if let Some(c) = cand_opt {
                    (
                        c.project_slug.clone(),
                        c.project_profile.clone(),
                        c.branch.clone(),
                    )
                } else {
                    let info: Option<(String, String, String)> = conn
                        .query_row(
                            "SELECT p.slug, p.profile, t.branch FROM tasks t JOIN projects p ON p.id = t.project_id WHERE t.id = ?1",
                            rusqlite::params![qs.task_id],
                            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                        )
                        .ok();
                    match info {
                        Some(i) => i,
                        None => continue,
                    }
                };

                let ws = crate::projects::workspace_of(
                    &crate::projects::load_map(&st.map_path).unwrap_or_default(),
                    &proj_slug,
                );
                let spec = crate::dashboard::jobs::JobSpec {
                    role: "auto".to_string(),
                    project: proj_slug,
                    profile: proj_prof,
                    branch: Some(branch),
                    task_id: Some(qs.task_id),
                    args: vec!["auto".to_string(), qs.task_id.to_string()],
                    max_parallel: ws.max_parallel_job_slots(),
                };

                // Chạy tiếp qua đường autopilot hiện có; xóa khỏi quota_stops để chỉ chạy 1 lần mỗi lần dừng
                match st.jobs.start(spec) {
                    Ok(_) => {
                        crate::db::delete_quota_stop(conn, qs.task_id)?;
                        let _ = conn.execute(
                            "UPDATE tasks SET auto_stop_kind = NULL WHERE id = ?1",
                            rusqlite::params![qs.task_id],
                        );
                    }
                    Err(e) => {
                        crate::term::emit(&format!(
                            "Lỗi auto_resume task {}: {}",
                            qs.task_id, e.message
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

/// Some project in the map asks for automatic reruns after an environment stop.
fn any_env_retry(map: &serde_json::Value) -> bool {
    map.get("projects")
        .and_then(|p| p.as_array())
        .is_some_and(|arr| {
            arr.iter().any(|p| {
                p.get("preflight_retry_minutes")
                    .and_then(|v| v.as_u64())
                    .is_some_and(|n| n > 0)
            })
        })
}

/// When each env-stopped task was last re-checked (in memory: a serve restart re-checks at the
/// next due time counted from the stop).
fn env_checks() -> &'static std::sync::Mutex<std::collections::HashMap<i64, i64>> {
    static CHECKS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<i64, i64>>> =
        std::sync::OnceLock::new();
    CHECKS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// C2, the same path as `handle_quota_stops`' auto resume: every task the autopilot stopped with
/// `moi_truong_chua_san` on a project with `preflight_retry_minutes > 0` is re-checked once per
/// that period; when the environment answers again, `auto <task>` starts as a dashboard job and
/// `auto_json.env_rerun_at` records it (Telegram reports it). Returns the tasks restarted.
pub fn handle_env_stops(
    st: &AppState,
    conn: &rusqlite::Connection,
    map: &serde_json::Value,
    now: i64,
) -> BrainResult<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, p.slug, p.profile, t.branch, t.auto_json
         FROM tasks t JOIN projects p ON p.id = t.project_id
         WHERE t.auto_state = 'stopped'
           AND json_extract(t.auto_json, '$.stop.reason') = 'moi_truong_chua_san'",
    )?;
    let rows: Vec<(i64, String, String, String, Option<String>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut checks = env_checks().lock().unwrap_or_else(|e| e.into_inner());
    let stopped: std::collections::HashSet<i64> = rows.iter().map(|r| r.0).collect();
    checks.retain(|id, _| stopped.contains(id));

    let mut restarted = Vec::new();
    for (task_id, slug, profile, branch, auto_json) in rows {
        let pf = crate::projects::preflight_of(map, &slug);
        if pf.retry_minutes == 0 || pf.checks.is_empty() {
            continue;
        }
        let mut aj = crate::autopilot::decide::AutoJson::parse(auto_json.as_deref());
        let stopped_at = aj.stop.as_ref().map(|s| s.at_ms).unwrap_or(now);
        let last = checks.get(&task_id).copied().unwrap_or(stopped_at);
        if now < last + pf.retry_minutes as i64 * 60_000 {
            continue;
        }
        let running = st
            .jobs
            .list()
            .iter()
            .any(|j| j.task_id == Some(task_id) && j.state == "running");
        if running {
            continue;
        }
        let ws = crate::projects::workspace_of(map, &slug);
        if crate::autopilot::claim::busy_reason_ws(conn, &slug, &profile, None, &ws)?
            .is_some()
        {
            continue;
        }
        checks.insert(task_id, now);
        if let Err(reason) = crate::preflight::run_checks(&pf.checks) {
            crate::term::emit(&format!(
                "Task {task_id}: môi trường vẫn chưa sẵn sàng — {reason}; kiểm lại sau {} phút",
                pf.retry_minutes
            ));
            continue;
        }
        // Written before the job starts: the new driver's claim keeps it (and must not be
        // overwritten afterwards). Put back if the job does not start.
        let previous = auto_json.clone();
        aj.env_rerun_at = Some(now);
        crate::db::set_auto_json(conn, task_id, Some(&aj.to_json()))?;
        let spec = crate::dashboard::jobs::JobSpec {
            role: "auto".to_string(),
            project: slug,
            profile,
            branch: Some(branch),
            task_id: Some(task_id),
            args: vec!["auto".to_string(), task_id.to_string()],
            max_parallel: ws.max_parallel_job_slots(),
        };
        match st.jobs.start(spec) {
            Ok(_) => {
                checks.remove(&task_id);
                crate::term::emit(&format!(
                    "Task {task_id}: môi trường đã sẵn sàng — chạy lại autopilot"
                ));
                restarted.push(task_id);
            }
            Err(e) => {
                let _ = crate::db::set_auto_json(conn, task_id, previous.as_deref());
                crate::term::emit(&format!(
                    "Lỗi chạy lại task {task_id} sau khi môi trường sẵn sàng: {}",
                    e.message
                ));
            }
        }
    }
    Ok(restarted)
}

/// Lý do khi hành động đã biến mất khỏi trạng thái task.
pub const QUEUED_STATE_CHANGED: &str = "Task đã đổi trạng thái — không chạy nữa";

/// Cảnh báo đã in cho mỗi hàng chờ (id → câu), để một lỗi kéo dài không in lại mỗi 5 giây.
fn queue_warned() -> &'static std::sync::Mutex<std::collections::HashMap<i64, String>> {
    static WARNED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<i64, String>>> =
        std::sync::OnceLock::new();
    WARNED.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn warn_queue_once(queued_id: i64, task_id: i64, text: &str) {
    let mut warned = queue_warned().lock().unwrap_or_else(|e| e.into_inner());
    if warned.get(&queued_id).map(String::as_str) == Some(text) {
        return;
    }
    warned.insert(queued_id, text.to_string());
    crate::term::emit(&format!("Task {task_id}: {text}"));
}

/// Hàng chờ chạy lại bước của task có sẵn khi dự án bận. Mỗi tick duyệt các hàng `waiting`
/// từ cũ tới mới; hàng chạy được thì chạy bằng đúng `run_task_action` / `run_control_as`,
/// hàng còn bận thì giữ nguyên. Mỗi dự án bắt đầu tối đa MỘT hành động mỗi tick. Với `run_dev`
/// kiểm plan trước (`plan_check`): lệch thì đổi hàng thành `relead` kèm lý do (chạy ở tick sau),
/// không chắc thì giữ `waiting`. Trả id các task đã bắt đầu.
pub fn handle_queued_actions(st: &AppState, conn: &rusqlite::Connection) -> BrainResult<Vec<i64>> {
    let rows = crate::db::waiting_actions(conn)?;
    {
        // Quên cảnh báo của hàng không còn `waiting`.
        let live: std::collections::HashSet<i64> = rows.iter().map(|r| r.id).collect();
        queue_warned()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|id, _| live.contains(id));
    }

    let mut started_projects: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut started_tasks = Vec::new();

    for row in rows {
        let task_id = row.task_id;
        let state = match load_task_state(st, conn, task_id) {
            Ok(Ok(s)) => s,
            Ok(Err(_)) => {
                finish_queued(st, conn, &row, "failed", QUEUED_STATE_CHANGED);
                continue;
            }
            Err(e) => {
                warn_queue_once(
                    row.id,
                    task_id,
                    &format!("không đọc được trạng thái task — {}", e.message),
                );
                continue;
            }
        };

        let action = match find_task_action(&state, &row.action_id) {
            Some(a) => a,
            None => {
                finish_queued(st, conn, &row, "failed", QUEUED_STATE_CHANGED);
                continue;
            }
        };
        if !action.enabled {
            let reason = action.reason.clone().unwrap_or_default();
            if is_busy_reason(&state, &reason) {
                continue; // vẫn bận: giữ `waiting`
            }
            // Dev bị khoá vì chính plan (vd. thiếu mục) cũng là plan không còn khớp: đưa về Lead
            // thay vì bỏ hàng.
            if row.action_id == "run_dev" {
                if let PlanCheck::Mismatch(why) = plan_check(&state) {
                    send_dev_back_to_lead(st, conn, &row, &why);
                    continue;
                }
            }
            let text = if reason.is_empty() {
                QUEUED_STATE_CHANGED.to_string()
            } else {
                reason
            };
            finish_queued(st, conn, &row, "failed", &text);
            continue;
        }
        if started_projects.contains(&state.project.id) {
            continue; // dự án này đã nhận một hành động ở tick này
        }

        if row.action_id == "run_dev" {
            match plan_check(&state) {
                PlanCheck::Ok => {}
                PlanCheck::Unsure(why) => {
                    warn_queue_once(
                        row.id,
                        task_id,
                        &format!(
                            "chưa kiểm được plan trước khi chạy Dev từ hàng chờ — {why}; thử lại sau"
                        ),
                    );
                    continue;
                }
                PlanCheck::Mismatch(why) => {
                    send_dev_back_to_lead(st, conn, &row, &why);
                    continue;
                }
            }
        }

        let source = if row.source == ActionSource::Telegram.as_str() {
            ActionSource::Telegram
        } else {
            ActionSource::Dashboard
        };
        let outcome = match queued_action_role(&row.action_id) {
            Some(role) => {
                run_task_action(st, task_id, &row.action_id, role, None, row.note.as_deref())
            }
            None => run_control_as(st, task_id, &row.action_id, None, row.note.as_deref(), source),
        };
        let label = queued_action_label(&row.action_id);
        match outcome {
            Ok(ControlOutcome::Started { .. }) => {
                finish_queued(st, conn, &row, "started", "");
                crate::term::emit(&format!("Task {task_id}: {label} chạy từ hàng chờ"));
                started_projects.insert(state.project.id.clone());
                started_tasks.push(task_id);
            }
            Ok(ControlOutcome::Refused { kind: "busy", .. }) | Ok(ControlOutcome::Unavailable) => {}
            Ok(ControlOutcome::Refused { reason, .. }) => {
                finish_queued(st, conn, &row, "failed", &reason);
            }
            Ok(ControlOutcome::Invalid(msg)) | Ok(ControlOutcome::NotFound(msg)) => {
                finish_queued(st, conn, &row, "failed", &msg);
            }
            Ok(_) => {
                // `Stale` (không truyền `rev` nên không xảy ra) hoặc kết quả không thuộc luồng này.
                finish_queued(st, conn, &row, "failed", QUEUED_STATE_CHANGED);
            }
            Err(e) => warn_queue_once(
                row.id,
                task_id,
                &format!("{label} từ hàng chờ lỗi — {}", e.message),
            ),
        }
    }
    Ok(started_tasks)
}

/// Plan không còn khớp: hàng `run_dev` đổi thành `relead` kèm ghi chú cho Lead (chạy ở tick
/// sau); Dev không chạy.
fn send_dev_back_to_lead(
    st: &AppState,
    conn: &rusqlite::Connection,
    row: &crate::db::QueuedAction,
    why: &str,
) {
    let note = clip_note(&format!("Plan không còn khớp: {why}"));
    match crate::db::retarget_queued(conn, row.id, "relead", Some(&note), Some(why)) {
        Ok(_) => {
            crate::term::emit(&format!(
                "Task {}: plan không còn khớp ({why}) — không chạy Dev, chạy lại Lead từ hàng chờ",
                row.task_id
            ));
            let _ = st.hints.send(0);
        }
        Err(e) => warn_queue_once(
            row.id,
            row.task_id,
            &format!("không ghi được hàng chờ — {}", e.message),
        ),
    }
}

/// Ghi trạng thái cuối của một hàng chờ, báo màn Task cập nhật; lỗi ghi chỉ in ra terminal.
fn finish_queued(
    st: &AppState,
    conn: &rusqlite::Connection,
    row: &crate::db::QueuedAction,
    status: &str,
    reason: &str,
) {
    let reason = if reason.is_empty() { None } else { Some(reason) };
    match crate::db::set_queued_status(conn, row.id, status, reason) {
        Ok(_) => {
            if status == "failed" {
                crate::term::emit(&format!(
                    "Task {}: {} trong hàng chờ bị bỏ — {}",
                    row.task_id,
                    queued_action_label(&row.action_id),
                    reason.unwrap_or("")
                ));
            }
            let _ = st.hints.send(0);
        }
        Err(e) => crate::term::emit(&format!(
            "Task {}: không ghi được trạng thái hàng chờ — {}",
            row.task_id, e.message
        )),
    }
}

/// Cắt ghi chú cho vừa `MAX_NOTE_CHARS` (`normalize_note` từ chối ghi chú dài hơn).
fn clip_note(note: &str) -> String {
    let max = crate::roles::lead::MAX_NOTE_CHARS;
    if note.chars().count() <= max {
        return note.to_string();
    }
    let mut s: String = note.chars().take(max.saturating_sub(1)).collect();
    s.push('…');
    s
}

/// Sinh slug ứng viên cho việc con từ slug cha, số thứ tự `idx` và số lượt chạy lại `rerun_num`.
/// Kết quả luôn ổn định qua `routing::slugify`, độ dài <= SLUG_MAX_LEN, khác slug cha khi parent_slug != "task".
pub(crate) fn child_slug_candidate(parent_slug: &str, idx: usize, rerun_num: usize) -> String {
    let suffix = if rerun_num <= 1 {
        format!("-c{idx}")
    } else {
        format!("-c{idx}-r{rerun_num}")
    };
    let max_head_len = crate::routing::SLUG_MAX_LEN.saturating_sub(suffix.len());
    let head = if parent_slug.len() <= max_head_len {
        parent_slug
    } else {
        &parent_slug[..max_head_len]
    };
    let mut head = head.trim_end_matches('-');
    if head.is_empty() {
        head = "task";
    }
    format!("{head}{suffix}")
}

/// `<slug cha>-c<idx>`, `-r2`, `-r3`, … : slug chưa có hàng trong DB và chưa có nhánh trong repo.
fn pick_child_slug(
    conn: &rusqlite::Connection,
    project: &crate::projects::Project,
    parent_slug: &str,
    idx: usize,
) -> BrainResult<String> {
    let project_row_id = crate::db::project_row_id(conn, &project.id)?;
    let root = project.root();

    let mut rerun_num = 1;
    loop {
        if rerun_num > 1000 {
            return Err(BrainError::error(format!(
                "Không tìm được slug trống cho việc con #{idx} của task {parent_slug}"
            )));
        }
        let candidate = child_slug_candidate(parent_slug, idx, rerun_num);
        let mut free = candidate != parent_slug;
        if free {
            for prefix in ["task", "analysis"] {
                let branch = format!("{prefix}/{candidate}");
                if crate::db::find_task_by_branch(conn, project_row_id, &branch)?.is_some()
                    || crate::gitops::branch_exists(&root, &branch)
                {
                    free = false;
                    break;
                }
            }
        }
        if free {
            return Ok(candidate);
        }
        rerun_num += 1;
    }
}

/// Khởi động các việc con từ bảng `split_items` khi có slot và đủ điều kiện phụ thuộc.
pub fn handle_split_items(st: &AppState, conn: &rusqlite::Connection) -> BrainResult<()> {
    let orch = st.orch();
    let limit = orch.max_parallel;
    let mut active = active_task_count(st, conn)?;

    let map = crate::projects::load_map(&st.map_path)?;
    let projects = crate::projects::projects_of(&map);

    let waiting_items = crate::db::waiting_split_items(conn)?;

    for item in waiting_items {
        if active >= limit {
            break;
        }

        let Some(parent_task) = crate::db::get_task(conn, item.parent_id)? else {
            continue;
        };

        let parent_auto = crate::db::task_auto(conn, item.parent_id)?.unwrap_or_default();
        let parent_aj = crate::autopilot::decide::AutoJson::parse(parent_auto.auto_json.as_deref());

        // Task cha không còn auto_state = 'waiting_children' hoặc đã hủy → bỏ qua
        // (item giữ nguyên trừ khi cha bị hủy: cancelled).
        if parent_aj.cancelled
            || (parent_auto.auto_state.as_deref() == Some("ended") && parent_aj.cancelled)
        {
            crate::db::set_split_item_state(conn, item.id, "cancelled", Some("Task cha đã hủy"))?;
            continue;
        }

        if parent_auto.auto_state.as_deref() != Some("waiting_children") {
            continue;
        }

        // Có after_idx: lấy outcome của item đó; Pending → bỏ qua lượt này;
        // Failed/Cancelled → item skipped, lý do Việc #<after_idx> không đạt; Passed → đi tiếp.
        if let Some(after_idx) = item.after_idx {
            let parent_items = crate::db::split_items_of(conn, item.parent_id)?;
            let prior = parent_items.iter().find(|it| it.idx == after_idx as usize);
            match prior {
                None => {
                    crate::db::set_split_item_state(
                        conn,
                        item.id,
                        "skipped",
                        Some(&format!("Việc #{after_idx} không đạt")),
                    )?;
                    continue;
                }
                Some(prior_item) => {
                    let outcome = crate::split::child_outcome(conn, prior_item);
                    match outcome {
                        crate::split::ChildOutcome::Pending => {
                            continue;
                        }
                        crate::split::ChildOutcome::Failed(_)
                        | crate::split::ChildOutcome::Cancelled => {
                            crate::db::set_split_item_state(
                                conn,
                                item.id,
                                "skipped",
                                Some(&format!("Việc #{after_idx} không đạt")),
                            )?;
                            continue;
                        }
                        crate::split::ChildOutcome::Passed => {
                            // Nếu workflow là code, việc sau cần code của việc trước:
                            // chỉ đi tiếp khi nhánh task con trước đã merge vào nhánh gốc
                            let parent_wf = parent_auto
                                .workflow_json
                                .as_deref()
                                .and_then(crate::workflows::Workflow::from_snapshot);
                            let is_code = parent_wf.as_ref().map_or(true, |w| w.kind == "code");
                            if is_code {
                                if let Some(prior_tid) = prior_item.child_task_id {
                                    let project_slug: String = conn.query_row(
                                        "SELECT slug FROM projects WHERE id = ?1",
                                        rusqlite::params![parent_task.project_id],
                                        |r| r.get(0),
                                    )?;
                                    if let Some(_wait) =
                                        code_prior_wait(conn, &map, &project_slug, prior_tid)?
                                    {
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let project_slug: String = match conn.query_row(
            "SELECT slug FROM projects WHERE id = ?1",
            rusqlite::params![parent_task.project_id],
            |r| r.get(0),
        ) {
            Ok(s) => s,
            Err(_) => {
                crate::db::set_split_item_state(
                    conn,
                    item.id,
                    "failed",
                    Some("Dự án của task cha không tồn tại"),
                )?;
                continue;
            }
        };

        let Some(project) = projects.iter().find(|p| p.id == project_slug) else {
            crate::db::set_split_item_state(
                conn,
                item.id,
                "failed",
                Some("Dự án không còn trong danh sách"),
            )?;
            continue;
        };

        let child_slug = match pick_child_slug(conn, project, &parent_task.slug, item.idx) {
            Ok(s) => s,
            Err(e) => {
                crate::db::set_split_item_state(conn, item.id, "failed", Some(&e.message))?;
                continue;
            }
        };

        let child_branch = format!("task/{child_slug}");
        let project_row_id = crate::db::project_row_id(conn, &project.id)?;
        if crate::db::find_task_by_branch(conn, project_row_id, &child_branch)?.is_some() {
            crate::db::set_split_item_state(
                conn,
                item.id,
                "failed",
                Some(&format!("Nhánh {child_branch} đã tồn tại")),
            )?;
            continue;
        }

        let child_request = format!(
            "{}\n\n{}\n\n(Việc con #{} của task #{} \"{}\")",
            item.title, item.request, item.idx, item.parent_id, parent_task.title
        );

        let parent_wf = parent_auto
            .workflow_json
            .as_deref()
            .and_then(crate::workflows::Workflow::from_snapshot);
        let wf_name = parent_wf.as_ref().map(|w| w.name.as_str());

        let team_json = crate::db::task_team_json(conn, item.parent_id)?;
        let team = crate::teams::TaskTeam::parse(team_json.as_deref()).map(|t| t.name);

        let parent_inputs: Vec<crate::inputs::InputItem> =
            crate::inputs::list_for_task(conn, item.parent_id)
                .unwrap_or_default()
                .into_iter()
                .map(|r| crate::inputs::InputItem {
                    name: r.name,
                    content: r.content,
                })
                .collect();
        let inputs_opt = if parent_inputs.is_empty() {
            None
        } else {
            Some(parent_inputs.as_slice())
        };

        let parent_origin = match crate::db::task_origin(conn, item.parent_id)?.as_deref() {
            Some(crate::db::ORIGIN_TELEGRAM) => crate::dashboard::api_tasks::Origin::Telegram,
            _ => crate::dashboard::api_tasks::Origin::Dashboard,
        };

        let start_req = crate::dashboard::api_tasks::StartRequest {
            project: &project_slug,
            request: &child_request,
            name: Some(&child_slug),
            workflow: wf_name,
            level: Some(&parent_task.level),
            team: team.as_deref(),
            inputs: inputs_opt,
            origin: parent_origin,
            dry_run: false,
        };

        match crate::dashboard::api_tasks::start_new_task(st, &start_req) {
            Ok(crate::dashboard::api_tasks::StartOutcome::Started {
                task_id: child_task_id,
                ..
            }) => {
                if child_task_id == item.parent_id {
                    crate::db::set_split_item_state(
                        conn,
                        item.id,
                        "failed",
                        Some(&format!("Slug việc con trùng task cha #{child_task_id}")),
                    )?;
                    continue;
                }
                let existing_parent = crate::db::task_parent(conn, child_task_id)?;
                if existing_parent.is_some() && existing_parent != Some(item.parent_id) {
                    crate::db::set_split_item_state(
                        conn,
                        item.id,
                        "failed",
                        Some(&format!("Slug việc con trùng task #{child_task_id}")),
                    )?;
                    continue;
                }
                let in_split_item: bool = conn
                    .query_row(
                        "SELECT 1 FROM split_items WHERE child_task_id = ?1 AND id != ?2 LIMIT 1",
                        rusqlite::params![child_task_id, item.id],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);
                if in_split_item {
                    crate::db::set_split_item_state(
                        conn,
                        item.id,
                        "failed",
                        Some(&format!("Slug việc con trùng task #{child_task_id}")),
                    )?;
                    continue;
                }

                crate::db::set_task_parent(conn, child_task_id, item.parent_id)?;
                crate::db::set_task_request(conn, child_task_id, &item.request)?;
                crate::db::set_split_item_started(conn, item.id, child_task_id)?;
                active += 1;
            }
            Ok(crate::dashboard::api_tasks::StartOutcome::Busy(_)) => {
                continue;
            }
            Ok(crate::dashboard::api_tasks::StartOutcome::PreflightRefused { ref kind, .. })
                if kind == "busy" =>
            {
                continue;
            }
            Ok(crate::dashboard::api_tasks::StartOutcome::PreflightRefused { reason, .. })
            | Ok(crate::dashboard::api_tasks::StartOutcome::Invalid(reason)) => {
                crate::db::set_split_item_state(conn, item.id, "failed", Some(&reason))?;
            }
            Ok(crate::dashboard::api_tasks::StartOutcome::ProjectRefused { body, .. }) => {
                let reason = body
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Dự án từ chối")
                    .to_string();
                crate::db::set_split_item_state(conn, item.id, "failed", Some(&reason))?;
            }
            Ok(crate::dashboard::api_tasks::StartOutcome::Unavailable) => {
                crate::db::set_split_item_state(
                    conn,
                    item.id,
                    "failed",
                    Some("Dashboard không sẵn sàng"),
                )?;
            }
            Ok(crate::dashboard::api_tasks::StartOutcome::Ready(_)) => {}
            Err(e) => {
                crate::db::set_split_item_state(conn, item.id, "failed", Some(&e.message))?;
            }
        }
    }

    // 3. Với mỗi parents_waiting_children: nếu split::settled -> split::aggregate
    let parents = crate::db::parents_waiting_children(conn)?;
    for parent_id in parents {
        match crate::split::settled(conn, parent_id) {
            Ok(true) => {
                if let Ok(Some(parent_task)) = crate::db::get_task(conn, parent_id) {
                    match crate::split::aggregate(conn, &parent_task) {
                        Ok(path) => {
                            crate::term::emit(&format!(
                                "Tách việc xong: task #{} -> {}",
                                parent_id,
                                path.display()
                            ));
                            let _ = st.hints.send(0);
                        }
                        Err(e) => {
                            crate::term::emit(&format!(
                                "Tổng hợp việc con task {parent_id}: lỗi — {}",
                                e.message
                            ));
                        }
                    }
                }
            }
            Ok(false) => {}
            Err(e) => {
                crate::term::emit(&format!(
                    "Kiểm tra settled việc con task {parent_id}: lỗi — {}",
                    e.message
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_db() -> (tempfile::TempDir, rusqlite::Connection, i64) {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = crate::db::open(&dir.path().join("brain.db")).expect("open db");
        let project = crate::projects::Project {
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
        crate::db::sync_projects(&conn, &[project]).expect("sync projects");
        let project_id = crate::db::project_row_id(&conn, "demo").expect("project id");
        (dir, conn, project_id)
    }

    fn task_run(
        conn: &rusqlite::Connection,
        project_id: i64,
        task_id: i64,
        started_at: i64,
        outcome: &str,
    ) {
        let id = crate::db::insert_run(
            conn,
            &crate::db::NewRun {
                task_id: Some(task_id),
                project_id,
                role: "qa".to_string(),
                profile: "default".to_string(),
                model: None,
                pid: 1,
                pgid: 1,
                pid_start: 1,
                base_commit: None,
                run_dir: None,
            },
        )
        .expect("insert run");
        conn.execute(
            "UPDATE runs SET started_at = ?1, status = 'succeeded', outcome = ?2 WHERE id = ?3",
            rusqlite::params![started_at, outcome, id],
        )
        .expect("update run");
    }

    #[test]
    fn prune_drops_quota_stops_of_tasks_run_again() {
        let (_dir, conn, pid) = fresh_db();
        let quota = crate::db::upsert_task(&conn, pid, "a", "A", "task/a", None, None).unwrap();
        let rerun = crate::db::upsert_task(&conn, pid, "b", "B", "task/b", None, None).unwrap();
        let marked = crate::db::upsert_task(&conn, pid, "c", "C", "task/c", None, None).unwrap();
        let kept = crate::db::upsert_task(&conn, pid, "d", "D", "task/d", None, None).unwrap();
        for t in [quota, rerun, marked, kept] {
            crate::db::insert_quota_stop(&conn, t, 1_000, "claude").unwrap();
            task_run(&conn, pid, t, 1_000, "quota");
        }
        // run again by hand and finished
        task_run(&conn, pid, rerun, 2_000, "qa:fix");
        // the autopilot still marks it het_quota
        task_run(&conn, pid, marked, 2_000, "qa:fix");
        conn.execute(
            "UPDATE tasks SET auto_stop_kind = 'het_quota' WHERE id = ?1",
            rusqlite::params![marked],
        )
        .unwrap();
        // in the current scan's candidates: never pruned here
        task_run(&conn, pid, kept, 2_000, "qa:fix");

        assert_eq!(prune_stale_quota_stops(&conn, &[kept]).unwrap(), 1);
        let left: Vec<i64> = crate::db::list_quota_stops(&conn)
            .unwrap()
            .into_iter()
            .map(|q| q.task_id)
            .collect();
        assert!(left.contains(&quota));
        assert!(!left.contains(&rerun));
        assert!(left.contains(&marked));
        assert!(left.contains(&kept));
    }

    #[test]
    fn child_slug_candidate_rules() {
        let parent = "viet-test-case-va-test-playwright-cho-toan-bo-r2";
        assert_eq!(parent.len(), 48);

        // slug cha 48 ký tự, idx 1 -> kết quả <= 48, kết thúc bằng -c1, khác slug cha, slugify(x) == x
        let c1 = child_slug_candidate(parent, 1, 1);
        assert!(c1.len() <= crate::routing::SLUG_MAX_LEN);
        assert!(c1.ends_with("-c1"));
        assert_ne!(c1, parent);
        assert_eq!(crate::routing::slugify(&c1), c1);

        // idx 20 và rerun -r3 cũng thỏa các điều kiện trên
        let c20_r3 = child_slug_candidate(parent, 20, 3);
        assert!(c20_r3.len() <= crate::routing::SLUG_MAX_LEN);
        assert!(c20_r3.ends_with("-c20-r3"));
        assert_ne!(c20_r3, parent);
        assert_eq!(crate::routing::slugify(&c20_r3), c20_r3);

        // slug cha ngắn (abc) -> abc-c1 (giữ hành vi cũ)
        let c_short = child_slug_candidate("abc", 1, 1);
        assert_eq!(c_short, "abc-c1");
        assert_eq!(crate::routing::slugify(&c_short), c_short);
    }
}
