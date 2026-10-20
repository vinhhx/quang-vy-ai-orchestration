//! Task endpoints: giao việc, preflight guards, busy rules, resolve gate,
//! task list/detail, derived steps and dev/qa/relead starts.

use crate::autopilot::claim;
use crate::autopilot::decide::{self, AutoJson, GateState};
use crate::config::Config;
use crate::dashboard::jobs::{JobManager, JobView};
use crate::dashboard::pipeline;
use crate::dashboard::taskstate::{self, AutoFacts, GitFacts, TaskFacts, TaskView};
use crate::dashboard::{blocking, json_error, AppState};
use crate::db::{RunRow, TaskAuto, TaskRow};
use crate::error::BrainResult;
use crate::projects::Project;
use crate::workflows::Workflow;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const MAX_REQUEST_CHARS: usize = 20_000;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewTask {
    pub project: String,
    pub request: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub workflow: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionBody {}

/// Thân yêu cầu của các nút điều khiển autopilot / cổng duyệt; `rev` (tùy chọn)
/// là `rev` của task mà màn hình đang hiển thị.
#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ControlBody {
    #[serde(default)]
    rev: Option<String>,
}

/// Thân yêu cầu của Trả về Lead: ghi chú tùy chọn cho Lead và `rev`.
#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ReturnBody {
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    rev: Option<String>,
}

const STALE_MSG: &str = "Trạng thái task đã thay đổi — màn hình sẽ tải lại.";
const NOT_ALLOWED_MSG: &str = "Hành động không áp dụng cho trạng thái hiện tại";

#[derive(serde::Deserialize)]
struct ListTasksQuery {
    profile: Option<String>,
    limit: Option<usize>,
}

#[derive(serde::Serialize)]
pub struct LeadPreflight {
    pub project: String,
    pub profile: String,
    pub current_branch: Option<String>,
    pub slug: String,
    pub branch: String,
    pub replan: bool,
    pub dirty: Vec<String>,
    pub branch_exists: bool,
    pub warning: Option<String>,
    pub busy: Option<String>,
    pub can_start: bool,
    pub reason: Option<String>,
    pub kind: Option<String>,
}

pub struct Refusal {
    pub status: StatusCode,
    pub body: serde_json::Value,
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        (
            self.status,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            Json(self.body),
        )
            .into_response()
    }
}

pub fn resolve_project(
    map: &serde_json::Value,
    cfg: &Config,
    query: &str,
) -> Result<(Project, PathBuf), Refusal> {
    let projects = crate::projects::projects_of(map);
    let project = match crate::routing::resolve(query, &projects) {
        Ok(p) => p,
        Err(err) => {
            let error_msg = match crate::projects::select_project(query, map) {
                Ok(_) => String::new(),
                Err(e) => e.message,
            };
            let kind = match err.kind {
                crate::routing::RouteKind::Ambiguous => "ambiguous",
                crate::routing::RouteKind::None => "none",
            };
            let candidates: Vec<serde_json::Value> = err
                .candidates
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "id": p.id,
                        "name_vi": p.name_vi,
                        "path": p.path,
                        "profile": p.profile,
                    })
                })
                .collect();
            return Err(Refusal {
                status: StatusCode::CONFLICT,
                body: serde_json::json!({
                    "error": error_msg,
                    "kind": kind,
                    "candidates": candidates,
                }),
            });
        }
    };

    let proj_root = match crate::projects::require_runnable(&project, cfg) {
        Ok(r) => r,
        Err(e) => {
            return Err(Refusal {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                body: serde_json::json!({
                    "error": e.message,
                }),
            });
        }
    };

    Ok((project, proj_root))
}

pub fn busy_reason(
    conn: &rusqlite::Connection,
    jobs: &JobManager,
    project: &Project,
) -> BrainResult<Option<String>> {
    let running = crate::db::running_runs(conn)?;
    for r in &running {
        if let (Some(pid), Some(pid_start)) = (r.pid, r.pid_start) {
            let proc_id = crate::procs::ProcId {
                pid: pid as i32,
                start: pid_start as u64,
            };
            if crate::procs::alive(proc_id) {
                let project_slug: Option<String> = conn
                    .query_row(
                        "SELECT slug FROM projects WHERE id = ?1",
                        rusqlite::params![r.project_id],
                        |row| row.get(0),
                    )
                    .ok();
                let tid_str = r
                    .task_id
                    .map(|tid| tid.to_string())
                    .unwrap_or_else(|| "-".to_string());
                if project_slug.as_deref() == Some(&project.id) {
                    return Ok(Some(format!(
                        "Dự án {} đang có run {} ({}, task {tid_str}) chạy — chờ run xong hoặc bấm Kill",
                        project.id, r.id, r.role
                    )));
                }
            }
        }
    }

    // RESEARCH Pitfall 5 / AUTO-05: a live autopilot driver (also one started
    // from a terminal, unknown to the JobManager) holds its project / profile.
    if let Some(reason) = claim::live_driver_reason(conn, &project.id, &project.profile, None)? {
        return Ok(Some(reason));
    }

    let reserved = jobs.reserved();
    if reserved.iter().any(|(p, _)| p == &project.id) {
        return Ok(Some(format!(
            "Dự án {} đang có việc vừa giao từ dashboard — chờ run xong",
            project.id
        )));
    }

    if project.profile == "personal" {
        for r in &running {
            if r.profile == "personal" {
                if let (Some(pid), Some(pid_start)) = (r.pid, r.pid_start) {
                    let proc_id = crate::procs::ProcId {
                        pid: pid as i32,
                        start: pid_start as u64,
                    };
                    if crate::procs::alive(proc_id) {
                        let tid_str = r
                            .task_id
                            .map(|tid| tid.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        return Ok(Some(format!(
                            "Profile personal đang có run {} chạy (task {tid_str}) — không chạy song song trên cùng thư mục cấu hình (E-06)",
                            r.id
                        )));
                    }
                }
            }
        }
        if reserved.iter().any(|(_, prof)| prof == "personal") {
            return Ok(Some(
                "Profile personal đang có việc vừa giao từ dashboard — không chạy song song trên cùng thư mục cấu hình (E-06)".to_string(),
            ));
        }
    }

    Ok(None)
}

pub fn busy_reason_for_task(
    conn: &rusqlite::Connection,
    jobs: &JobManager,
    project: &Project,
    task_id: i64,
    branch: &str,
) -> BrainResult<Option<String>> {
    let running = crate::db::running_runs(conn)?;
    for r in &running {
        if r.task_id == Some(task_id) {
            continue;
        }
        if let (Some(pid), Some(pid_start)) = (r.pid, r.pid_start) {
            let proc_id = crate::procs::ProcId {
                pid: pid as i32,
                start: pid_start as u64,
            };
            if crate::procs::alive(proc_id) {
                let project_slug: Option<String> = conn
                    .query_row(
                        "SELECT slug FROM projects WHERE id = ?1",
                        rusqlite::params![r.project_id],
                        |row| row.get(0),
                    )
                    .ok();
                let tid_str = r
                    .task_id
                    .map(|tid| tid.to_string())
                    .unwrap_or_else(|| "-".to_string());
                if project_slug.as_deref() == Some(&project.id) {
                    return Ok(Some(format!(
                        "Dự án {} đang có run {} ({}, task {tid_str}) chạy — chờ run xong hoặc bấm Kill",
                        project.id, r.id, r.role
                    )));
                }
            }
        }
    }

    // A live driver of another task holds the project (or the personal profile).
    if let Some(reason) =
        claim::live_driver_reason(conn, &project.id, &project.profile, Some(task_id))?
    {
        return Ok(Some(reason));
    }

    let active_jobs = jobs.list();
    let this_task_job = active_jobs.iter().find(|j| {
        j.state == "running" && (j.task_id == Some(task_id) || j.branch.as_deref() == Some(branch))
    });

    let reserved = jobs.reserved();
    for (p, _) in &reserved {
        if p == &project.id {
            if let Some(tj) = this_task_job {
                if &tj.project == p {
                    continue;
                }
            }
            return Ok(Some(format!(
                "Dự án {} đang có việc vừa giao từ dashboard — chờ run xong",
                project.id
            )));
        }
    }

    if project.profile == "personal" {
        for r in &running {
            if r.task_id == Some(task_id) {
                continue;
            }
            if r.profile == "personal" {
                if let (Some(pid), Some(pid_start)) = (r.pid, r.pid_start) {
                    let proc_id = crate::procs::ProcId {
                        pid: pid as i32,
                        start: pid_start as u64,
                    };
                    if crate::procs::alive(proc_id) {
                        let tid_str = r
                            .task_id
                            .map(|tid| tid.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        return Ok(Some(format!(
                            "Profile personal đang có run {} chạy (task {tid_str}) — không chạy song song trên cùng thư mục cấu hình (E-06)",
                            r.id
                        )));
                    }
                }
            }
        }
        for (p, prof) in &reserved {
            if prof == "personal" {
                if let Some(tj) = this_task_job {
                    if &tj.project == p {
                        continue;
                    }
                }
                return Ok(Some(
                    "Profile personal đang có việc vừa giao từ dashboard — không chạy song song trên cùng thư mục cấu hình (E-06)".to_string(),
                ));
            }
        }
    }

    Ok(None)
}

pub fn lead_preflight(
    conn: &rusqlite::Connection,
    jobs: &JobManager,
    project: &Project,
    root: &Path,
    slug: &str,
) -> BrainResult<LeadPreflight> {
    let current = crate::gitops::current_branch(root);
    let branch = format!("task/{slug}");
    let replan = current.as_deref() == Some(&branch);
    let mut dirty_set = crate::gitops::changed_paths(root)?;
    if replan {
        dirty_set.remove(".plan.md");
    }
    let mut dirty: Vec<String> = dirty_set.into_iter().collect();
    dirty.sort();

    let branch_exists = !replan && crate::gitops::branch_exists(root, &branch);

    let warning = if !replan {
        if let Some(ref cb) = current {
            if cb.starts_with("task/") {
                Some(format!(
                    "Cảnh báo: Nhánh mới '{branch}' được tạo từ nhánh task '{cb}' (stacking)"
                ))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let busy = busy_reason(conn, jobs, project)?;

    let (can_start, reason, kind) = if let Some(ref b) = busy {
        (false, Some(b.clone()), Some("busy".to_string()))
    } else if !dirty.is_empty() {
        let r = format!(
            "Working tree có thay đổi chưa commit — dừng, không tự stash:\n{}",
            dirty.join("\n")
        );
        (false, Some(r), Some("dirty".to_string()))
    } else if branch_exists {
        let r = format!(
            "Nhánh '{branch}' đã tồn tại nhưng chưa được checkout — hãy checkout nhánh đó hoặc dùng cờ --name khác"
        );
        (false, Some(r), Some("branch_exists".to_string()))
    } else {
        (true, None, None)
    };

    Ok(LeadPreflight {
        project: project.id.clone(),
        profile: project.profile.clone(),
        current_branch: current,
        slug: slug.to_string(),
        branch,
        replan,
        dirty,
        branch_exists,
        warning,
        busy,
        can_start,
        reason,
        kind,
    })
}

pub fn routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/api/tasks", get(list_tasks).post(create_task))
        .route("/api/tasks/{id}", get(get_task))
        .route("/api/tasks/{id}/pipeline", get(get_pipeline))
        .route("/api/tasks/{id}/dev", post(post_task_dev))
        .route("/api/tasks/{id}/qa", post(post_task_qa))
        .route("/api/tasks/{id}/relead", post(post_task_relead))
        .route("/api/tasks/{id}/approve", post(post_approve))
        .route("/api/tasks/{id}/return", post(post_return))
        .route("/api/tasks/{id}/auto", post(post_auto))
        .route("/api/tasks/{id}/auto/stop", post(post_auto_stop))
        .route(
            "/api/tasks/{id}/auto/more-round",
            post(post_auto_more_round),
        )
        .route("/api/tasks/{id}/auto/end", post(post_auto_end))
        .route("/api/preflight", get(get_preflight))
        .route("/api/slug", get(get_slug))
        .route("/api/jobs/{id}", get(get_job))
}

#[derive(serde::Deserialize)]
struct SlugQuery {
    text: Option<String>,
}

async fn get_slug(Query(query): Query<SlugQuery>) -> Response {
    let slug = crate::routing::slugify(query.text.as_deref().unwrap_or(""));
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )],
        Json(serde_json::json!({ "slug": slug })),
    )
        .into_response()
}

async fn get_preflight(
    State(st): State<Arc<AppState>>,
    Query(query): Query<PreflightQuery>,
) -> Response {
    let raw_slug = query.slug.unwrap_or_default();
    let slug = crate::routing::slugify(&raw_slug);
    if slug.is_empty() {
        return json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Không thể tạo tên task từ yêu cầu — hãy truyền cờ --name <slug>",
        );
    }
    let project_query = query.project.unwrap_or_default();

    let res = blocking(move || {
        let map = crate::projects::load_map(&st.map_path)?;
        let (project, proj_root) = match resolve_project(&map, &st.cfg, &project_query) {
            Ok(p) => p,
            Err(refusal) => return Ok(refusal.into_response()),
        };
        let conn = crate::db::open(&st.db_path)?;
        let pf = lead_preflight(&conn, &st.jobs, &project, &proj_root, &slug)?;
        Ok((
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            Json(pf),
        )
            .into_response())
    })
    .await;

    match res {
        Ok(resp) => resp,
        Err(e) => internal_error("kiểm tra trước khi giao việc", &e),
    }
}

#[derive(serde::Deserialize)]
struct PreflightQuery {
    project: Option<String>,
    slug: Option<String>,
}

async fn create_task(State(st): State<Arc<AppState>>, Json(body): Json<NewTask>) -> Response {
    let trimmed_req = body.request.trim();
    if trimmed_req.is_empty() {
        return json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Yêu cầu không được để trống",
        );
    }
    if trimmed_req.chars().count() > MAX_REQUEST_CHARS {
        return json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("Yêu cầu quá dài (tối đa {MAX_REQUEST_CHARS} ký tự)"),
        );
    }

    let name_or_req = body.name.as_deref().unwrap_or(trimmed_req);
    let slug = crate::routing::slugify(name_or_req);
    if slug.is_empty() {
        return json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Không thể tạo tên task từ yêu cầu — hãy truyền cờ --name <slug>",
        );
    }

    let trimmed_req = trimmed_req.to_string();
    let res = blocking(move || {
        let map = crate::projects::load_map(&st.map_path)?;
        let (project, proj_root) = match resolve_project(&map, &st.cfg, &body.project) {
            Ok(p) => p,
            Err(refusal) => return Ok(refusal.into_response()),
        };

        // D-14 / D-24: body > project default > mac-dinh; an unknown name is
        // refused before any job starts.
        let loaded = crate::workflows::load(&st.cfg);
        let project_default = crate::projects::workflow_of(&map, &project.id);
        let workflow = match crate::workflows::resolve(
            &loaded,
            body.workflow.as_deref(),
            project_default.as_deref(),
        ) {
            Ok(r) => r.workflow,
            Err(e) => {
                return Ok(json_error(StatusCode::UNPROCESSABLE_ENTITY, &e.message));
            }
        };

        let conn = crate::db::open(&st.db_path)?;
        let pf = lead_preflight(&conn, &st.jobs, &project, &proj_root, &slug)?;
        if !pf.can_start {
            return Ok((
                StatusCode::CONFLICT,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({
                    "error": pf.reason.unwrap_or_default(),
                    "kind": pf.kind.unwrap_or_default(),
                })),
            )
                .into_response());
        }

        let branch = format!("task/{slug}");
        // Argument vector, positionals after `--` (a request starting with `-` stays data).
        let (role, args) = if workflow.auto {
            (
                "auto",
                vec![
                    "auto".to_string(),
                    "--start".to_string(),
                    "--name".to_string(),
                    slug.clone(),
                    "--workflow".to_string(),
                    workflow.name.clone(),
                    "--".to_string(),
                    project.id.clone(),
                    trimmed_req,
                ],
            )
        } else {
            (
                "lead",
                vec![
                    "run".to_string(),
                    "lead".to_string(),
                    "--name".to_string(),
                    slug.clone(),
                    "--workflow".to_string(),
                    workflow.name.clone(),
                    "--".to_string(),
                    project.id.clone(),
                    trimmed_req,
                ],
            )
        };
        let spec = crate::dashboard::jobs::JobSpec {
            role: role.to_string(),
            project: project.id.clone(),
            profile: project.profile.clone(),
            branch: Some(branch.clone()),
            task_id: None,
            args,
        };

        match st.jobs.start(spec) {
            Ok(job_id) => {
                let mut resp_val = serde_json::json!({
                    "job_id": job_id,
                    "project": project.id,
                    "slug": slug,
                    "branch": branch,
                    "workflow": workflow.name,
                    "auto": workflow.auto,
                });
                if let Some(w) = pf.warning {
                    resp_val["warning"] = serde_json::json!(w);
                }
                Ok((
                    StatusCode::ACCEPTED,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "application/json; charset=utf-8",
                    )],
                    Json(resp_val),
                )
                    .into_response())
            }
            Err(e) => {
                if e.message.contains("Dashboard đang dừng") {
                    Ok((
                        StatusCode::SERVICE_UNAVAILABLE,
                        [(
                            axum::http::header::CONTENT_TYPE,
                            "application/json; charset=utf-8",
                        )],
                        Json(serde_json::json!({
                            "error": "Dashboard đang dừng — không nhận việc mới"
                        })),
                    )
                        .into_response())
                } else {
                    Ok((
                        StatusCode::CONFLICT,
                        [(
                            axum::http::header::CONTENT_TYPE,
                            "application/json; charset=utf-8",
                        )],
                        Json(serde_json::json!({
                            "error": e.message,
                            "kind": "busy",
                        })),
                    )
                        .into_response())
                }
            }
        }
    })
    .await;

    match res {
        Ok(resp) => resp,
        Err(e) => internal_error("giao việc", &e),
    }
}

async fn list_tasks(
    State(st): State<Arc<AppState>>,
    Query(query): Query<ListTasksQuery>,
) -> Response {
    if let Some(ref p) = query.profile {
        if !p.is_empty() {
            if let Err(e) = crate::profile::check_profile(p, &st.cfg.profiles) {
                return json_error(StatusCode::BAD_REQUEST, &e.message);
            }
        }
    }

    let lim = query.limit.unwrap_or(100).clamp(1, 500);

    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT tasks.id, tasks.project_id, tasks.slug, tasks.title, tasks.branch,
                    tasks.base_branch, tasks.base_commit, tasks.status, tasks.created_at, tasks.updated_at,
                    projects.slug, projects.name_vi, projects.profile
             FROM tasks
             JOIN projects ON tasks.project_id = projects.id
             WHERE (?1 IS NULL OR projects.profile = ?1)
             ORDER BY tasks.updated_at DESC
             LIMIT ?2",
        )?;

        let prof_param = query.profile.as_ref().filter(|s| !s.is_empty());
        let mut rows = stmt.query(rusqlite::params![prof_param, lim as i64])?;

        let mut task_items = Vec::new();
        let now_ms = crate::db::now_ms();

        while let Some(r) = rows.next()? {
            let task = TaskRow {
                id: r.get(0)?,
                project_id: r.get(1)?,
                slug: r.get(2)?,
                title: r.get(3)?,
                branch: r.get(4)?,
                base_branch: r.get(5)?,
                base_commit: r.get(6)?,
                status: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
            };
            let proj_slug: String = r.get(10)?;
            let proj_name: String = r.get(11)?;
            let proj_profile: String = r.get(12)?;

            let lead_run = crate::db::latest_run_for_task_role(&conn, task.id, "lead")?;
            let dev_run = crate::db::latest_run_for_task_role(&conn, task.id, "dev")?;
            let qa_run = crate::db::latest_run_for_task_role(&conn, task.id, "qa")?;

            let facts = TaskFacts {
                task: &task,
                lead: lead_run.as_ref(),
                dev: dev_run.as_ref(),
                qa: qa_run.as_ref(),
                git: &GitFacts::default(),
                busy: None,
                now_ms,
            };
            // Autopilot state without git: gate unknown, enough for banner, group and badge.
            let auto = crate::db::task_auto(&conn, task.id)?.unwrap_or_default();
            let aj = AutoJson::parse(auto.auto_json.as_deref());
            let workflow = auto
                .workflow_json
                .as_deref()
                .and_then(Workflow::from_snapshot);
            let running = crate::db::running_run_for_task(&conn, task.id)?;
            let auto_facts = AutoFacts {
                workflow: workflow.as_ref(),
                auto: Some(&auto),
                auto_json: Some(&aj),
                driver_alive: claim::driver_alive(&auto),
                running: running.as_ref(),
                gate: None,
                plan_dirty: false,
            };
            let view = taskstate::derive_with(&facts, &auto_facts);

            task_items.push(serde_json::json!({
                "id": task.id,
                "project": proj_slug,
                "project_name": proj_name,
                "profile": proj_profile,
                "title": task.title,
                "slug": task.slug,
                "branch": task.branch,
                "status": task.status,
                "banner": view.banner,
                "group": view.group,
                "current_step": view.current_step,
                "steps": view.steps,
                "auto_state": view.auto.state,
                "stop_reason": view.auto.stop_reason,
                "stop_detail": view.auto.stop_detail,
                "created_at": task.created_at,
                "updated_at": task.updated_at,
            }));
        }

        let mut busy_list = Vec::new();
        for r in crate::db::running_runs(&conn)? {
            if let (Some(pid), Some(pid_start)) = (r.pid, r.pid_start) {
                let proc_id = crate::procs::ProcId {
                    pid: pid as i32,
                    start: pid_start as u64,
                };
                if crate::procs::alive(proc_id) {
                    let p_slug: Option<String> = conn
                        .query_row(
                            "SELECT slug FROM projects WHERE id = ?1",
                            rusqlite::params![r.project_id],
                            |row| row.get(0),
                        )
                        .ok();
                    if let Some(slug) = p_slug {
                        busy_list.push(serde_json::json!({
                            "project": slug,
                            "run_id": r.id,
                            "task_id": r.task_id,
                            "profile": r.profile,
                            "role": r.role,
                        }));
                    }
                }
            }
        }

        Ok((
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            Json(serde_json::json!({
                "tasks": task_items,
                "busy": busy_list,
            })),
        )
            .into_response())
    })
    .await;

    match res {
        Ok(resp) => resp,
        Err(e) => internal_error("đọc danh sách task", &e),
    }
}

/// Mọi dữ kiện của một task mà màn hình task và các nút hành động cùng dùng:
/// git, plan, busy, autopilot, view suy ra và `rev`.
pub struct TaskState {
    pub task: TaskRow,
    pub project: Project,
    pub root: PathBuf,
    pub git: GitFacts,
    pub plan_text: Option<String>,
    pub plan_source: Option<&'static str>,
    pub plan_dirty: bool,
    pub busy: Option<String>,
    pub auto: TaskAuto,
    pub aj: AutoJson,
    pub workflow: Option<Workflow>,
    pub driver_alive: bool,
    pub runs: Vec<RunRow>,
    pub gate: Option<GateState>,
    pub view: TaskView,
    pub rev: String,
}

/// Token đổi khi trạng thái task, run mới nhất, autopilot, cờ dừng, duyệt hoặc commit plan đổi.
pub fn task_rev(
    task: &TaskRow,
    auto: &TaskAuto,
    latest: Option<&RunRow>,
    plan_commit: Option<&str>,
) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    task.status.hash(&mut h);
    latest.map(|r| r.id).hash(&mut h);
    latest.map(|r| r.status.as_str()).hash(&mut h);
    auto.auto_state.hash(&mut h);
    auto.auto_stop_at.hash(&mut h);
    auto.approved_commit.hash(&mut h);
    plan_commit.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn latest_of<'a>(runs: &'a [RunRow], role: &str) -> Option<&'a RunRow> {
    runs.iter()
        .filter(|r| r.role == role)
        .max_by_key(|r| (r.started_at, r.id))
}

fn running_of(runs: &[RunRow]) -> Option<&RunRow> {
    runs.iter()
        .filter(|r| r.status == "running")
        .max_by_key(|r| (r.started_at, r.id))
}

/// Đọc toàn bộ trạng thái task (Err(Response) = 404 JSON cho task / dự án không có).
pub fn load_task_state(
    st: &AppState,
    conn: &rusqlite::Connection,
    task_id: i64,
) -> BrainResult<Result<TaskState, Response>> {
    let task = match crate::db::get_task(conn, task_id)? {
        Some(t) => t,
        None => {
            return Ok(Err(json_error(
                StatusCode::NOT_FOUND,
                &format!("Không tìm thấy task {task_id}"),
            )));
        }
    };

    let map = crate::projects::load_map(&st.map_path)?;
    let projects = crate::projects::projects_of(&map);
    let project_slug: String = conn.query_row(
        "SELECT slug FROM projects WHERE id = ?1",
        rusqlite::params![task.project_id],
        |r| r.get(0),
    )?;
    let project = match projects.into_iter().find(|p| p.id == project_slug) {
        Some(p) => p,
        None => {
            return Ok(Err(json_error(
                StatusCode::NOT_FOUND,
                &format!("Không tìm thấy dự án {project_slug}"),
            )));
        }
    };

    let root = PathBuf::from(&project.path);
    let current_branch = crate::gitops::current_branch(&root);
    let is_on_branch = current_branch.as_deref() == Some(task.branch.as_str());
    let dirty_set = crate::gitops::changed_paths(&root).unwrap_or_default();
    let mut dirty: Vec<String> = dirty_set.into_iter().collect();
    dirty.sort();
    let plan_dirty = is_on_branch && dirty.iter().any(|p| p == ".plan.md");

    let (plan_text, plan_source) = if is_on_branch {
        let p_path = root.join(".plan.md");
        if p_path.is_file() {
            (std::fs::read_to_string(&p_path).ok(), Some("worktree"))
        } else {
            (None, None)
        }
    } else {
        let show_arg = format!("{}:.plan.md", task.branch);
        match crate::gitops::git(&root, &["show", &show_arg]) {
            Ok(text) => (Some(text), Some("branch")),
            Err(_) => (None, None),
        }
    };

    // 05.1 WR-01: chỉ tìm commit plan trên nhánh task, giới hạn từ base_commit.
    let plan_commit = crate::gitops::plan_commit_on_branch(
        &root,
        &task.branch,
        &task.slug,
        task.base_commit.as_deref(),
    )
    .ok()
    .flatten();

    // RESEARCH Pitfall 6: commit báo cáo review không tính là thay đổi code.
    let code_after_plan = match (is_on_branch, plan_commit.as_deref()) {
        (true, Some(pc)) => match crate::gitops::diff_code_names(&root, pc, "HEAD") {
            Ok(diff) => !diff.is_empty(),
            Err(_) => false,
        },
        _ => false,
    };

    let (plan_exists, plan_blockers, plan_errors) = if let Some(ref text) = plan_text {
        let plan = crate::planfile::parse_plan(text);
        let blockers = crate::planfile::has_blockers(&plan);
        let errors = if plan_source == Some("worktree") {
            crate::planfile::validate_plan(&plan, Some(&root))
        } else {
            Vec::new()
        };
        (true, blockers, errors)
    } else {
        (false, false, Vec::new())
    };

    let git = GitFacts {
        current_branch,
        dirty,
        plan_exists,
        plan_blockers,
        plan_errors,
        plan_commit,
        code_after_plan,
    };

    let busy = busy_reason_for_task(conn, &st.jobs, &project, task.id, &task.branch)?;

    let auto = crate::db::task_auto(conn, task.id)?.unwrap_or_default();
    let aj = AutoJson::parse(auto.auto_json.as_deref());
    let workflow = auto
        .workflow_json
        .as_deref()
        .and_then(Workflow::from_snapshot);
    let driver_alive = claim::driver_alive(&auto);
    let runs = crate::db::runs_for_task(conn, task.id)?;
    let now = crate::db::now_ms();

    let gate = workflow.as_ref().map(|wf| {
        let facts = decide::build_facts(
            wf,
            &runs,
            &auto,
            &aj,
            git.plan_commit.as_deref(),
            &st.cfg.pricing,
            now,
            false,
        );
        decide::gate_state(wf, &facts)
    });

    let view = {
        let facts = TaskFacts {
            task: &task,
            lead: latest_of(&runs, "lead"),
            dev: latest_of(&runs, "dev"),
            qa: latest_of(&runs, "qa"),
            git: &git,
            busy: busy.clone(),
            now_ms: now,
        };
        let auto_facts = AutoFacts {
            workflow: workflow.as_ref(),
            auto: Some(&auto),
            auto_json: Some(&aj),
            driver_alive,
            running: running_of(&runs),
            gate,
            plan_dirty,
        };
        taskstate::derive_with(&facts, &auto_facts)
    };

    let latest = runs.iter().max_by_key(|r| (r.started_at, r.id));
    let rev = task_rev(&task, &auto, latest, git.plan_commit.as_deref());

    Ok(Ok(TaskState {
        task,
        project,
        root,
        git,
        plan_text,
        plan_source,
        plan_dirty,
        busy,
        auto,
        aj,
        workflow,
        driver_alive,
        runs,
        gate,
        view,
        rev,
    }))
}

/// PlanFacts cho cổng duyệt suy ra (task trước 5.2) và thẻ `ban` của 05.1.
fn plan_facts(root: &Path, task: &TaskRow, runs: &[RunRow]) -> pipeline::PlanFacts {
    match crate::gitops::plan_commit_on_branch(
        root,
        &task.branch,
        &task.slug,
        task.base_commit.as_deref(),
    ) {
        Err(()) => pipeline::PlanFacts::Unknown,
        Ok(None) => pipeline::PlanFacts::NoCommit,
        Ok(Some(sha)) => {
            let at = crate::gitops::commit_time_ms(root, &sha);
            // 05.1 WR-02: lỗi git khi xét tổ tiên -> Unknown, không coi là "chưa duyệt".
            match pipeline::approving_dev_run_checked(runs, &sha, at, |a, d| {
                crate::gitops::ancestry(root, a, d)
            }) {
                Ok(approved) => pipeline::PlanFacts::Commit {
                    sha,
                    at_ms: at,
                    approved_by: approved,
                },
                Err(()) => pipeline::PlanFacts::Unknown,
            }
        }
    }
}

async fn get_task(State(st): State<Arc<AppState>>, AxumPath(id): AxumPath<i64>) -> Response {
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let state = match load_task_state(&st, &conn, id)? {
            Ok(s) => s,
            Err(resp) => return Ok(resp),
        };
        let task = &state.task;
        let project = &state.project;
        let proj_root = &state.root;
        let is_on_branch = state.git.current_branch.as_deref() == Some(task.branch.as_str());

        let plan_obj = if let Some(ref text) = state.plan_text {
            let plan = crate::planfile::parse_plan(text);
            let sections: Vec<serde_json::Value> = crate::planfile::PLAN_SECTIONS
                .iter()
                .map(|name| {
                    serde_json::json!({
                        "name": name,
                        "text": crate::planfile::section_text(text, name),
                    })
                })
                .collect();
            let changes: Vec<serde_json::Value> = plan
                .changes
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "file": c.file,
                        "where": c.where_,
                        "what": c.what,
                        "why": c.why,
                    })
                })
                .collect();

            Some(serde_json::json!({
                "path": ".plan.md",
                "source": state.plan_source.unwrap_or("worktree"),
                "commit": state.git.plan_commit,
                "dirty": state.plan_dirty,
                "title": plan.title,
                "sections": sections,
                "changes": changes,
                "blockers": state.git.plan_blockers,
                "errors": state.git.plan_errors,
            }))
        } else {
            None
        };

        let (qa_text, qa_source) = if is_on_branch {
            let q_path = proj_root.join(".qa-report.md");
            if q_path.is_file() {
                (std::fs::read_to_string(&q_path).ok(), Some("worktree"))
            } else {
                (None, None)
            }
        } else {
            let show_arg = format!("{}:.qa-report.md", task.branch);
            match crate::gitops::git(proj_root, &["show", &show_arg]) {
                Ok(text) => (Some(text), Some("branch")),
                Err(_) => (None, None),
            }
        };

        let qa_obj = if let Some(ref text) = qa_text {
            let verdict = crate::planfile::parse_qa_verdict(text);
            let sections: Vec<serde_json::Value> = crate::planfile::QA_SECTIONS
                .iter()
                .map(|name| {
                    serde_json::json!({
                        "name": name,
                        "text": crate::planfile::section_text(text, name),
                    })
                })
                .collect();
            Some(serde_json::json!({
                "path": ".qa-report.md",
                "source": qa_source.unwrap_or("worktree"),
                "verdict": verdict,
                "sections": sections,
            }))
        } else {
            None
        };

        let view = &state.view;

        let mut runs_stmt = conn.prepare(
            "SELECT id, role, status, outcome, input_tokens, output_tokens,
                    cache_read_tokens, cache_write_tokens, cost_usd, started_at, ended_at,
                    stage, round
             FROM runs WHERE task_id = ?1 ORDER BY started_at ASC, id ASC",
        )?;
        let run_rows: Vec<serde_json::Value> = runs_stmt
            .query_map(rusqlite::params![id], |r| {
                Ok(serde_json::json!({
                    "id": r.get::<_, i64>(0)?,
                    "role": r.get::<_, String>(1)?,
                    "status": r.get::<_, String>(2)?,
                    "outcome": r.get::<_, Option<String>>(3)?,
                    "input_tokens": r.get::<_, i64>(4)?,
                    "output_tokens": r.get::<_, i64>(5)?,
                    "cache_read_tokens": r.get::<_, i64>(6)?,
                    "cache_write_tokens": r.get::<_, i64>(7)?,
                    "cost_usd": r.get::<_, f64>(8)?,
                    "started_at": r.get::<_, i64>(9)?,
                    "ended_at": r.get::<_, Option<i64>>(10)?,
                    "stage": r.get::<_, Option<String>>(11)?,
                    "round": r.get::<_, Option<i64>>(12)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let job_view: Option<JobView> = st.jobs.list().into_iter().find(|j| {
            j.state == "running"
                && (j.task_id == Some(task.id) || j.branch.as_deref() == Some(&task.branch))
        });

        let workflow_json = match state.workflow.as_ref() {
            Some(wf) => serde_json::json!({
                "name": wf.name,
                "summary": wf.summary(),
                "auto": wf.auto,
            }),
            None => serde_json::Value::Null,
        };

        let resp_json = serde_json::json!({
            "id": task.id,
            "project_id": task.project_id,
            "slug": task.slug,
            "title": task.title,
            "branch": task.branch,
            "base_branch": task.base_branch,
            "base_commit": task.base_commit,
            "status": task.status,
            "created_at": task.created_at,
            "updated_at": task.updated_at,
            "project": {
                "id": project.id,
                "name_vi": project.name_vi,
                "profile": project.profile,
                "path": project.path,
            },
            "steps": view.steps,
            "banner": view.banner,
            "group": view.group,
            "current_step": view.current_step,
            "actions": view.actions,
            "controls": view.controls,
            "auto": view.auto,
            "workflow": workflow_json,
            "rev": state.rev,
            "plan": plan_obj,
            "qa": qa_obj,
            "runs": run_rows,
            "job": job_view,
        });

        Ok((
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            Json(resp_json),
        )
            .into_response())
    })
    .await;

    match res {
        Ok(resp) => resp,
        Err(e) => internal_error("đọc task", &e),
    }
}

async fn get_pipeline(State(st): State<Arc<AppState>>, AxumPath(id): AxumPath<i64>) -> Response {
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let state = match load_task_state(&st, &conn, id)? {
            Ok(s) => s,
            Err(resp) => return Ok(resp),
        };

        let plan = plan_facts(&state.root, &state.task, &state.runs);
        let now = crate::db::now_ms();
        let running = running_of(&state.runs);
        let task_facts = TaskFacts {
            task: &state.task,
            lead: latest_of(&state.runs, "lead"),
            dev: latest_of(&state.runs, "dev"),
            qa: latest_of(&state.runs, "qa"),
            git: &state.git,
            busy: None,
            now_ms: now,
        };
        // 05.1 IN-05: một định nghĩa "bước hiện tại".
        let current_step = taskstate::current_step_of(&task_facts, running);
        let facts = pipeline::PipelineFacts {
            task: &state.task,
            runs: &state.runs,
            plan: &plan,
            current_step: &current_step,
            now_ms: now,
        };

        let plan_commit_at = state
            .git
            .plan_commit
            .as_deref()
            .and_then(|sha| crate::gitops::commit_time_ms(&state.root, sha));

        let next_card = match state.workflow.as_ref() {
            Some(wf) if state.driver_alive && running.is_none() => {
                let dfacts = decide::build_facts(
                    wf,
                    &state.runs,
                    &state.auto,
                    &state.aj,
                    state.git.plan_commit.as_deref(),
                    &st.cfg.pricing,
                    now,
                    false,
                );
                match decide::decide(wf, &dfacts) {
                    decide::Decision::Run(sr) => Some(sr.card_key().to_string()),
                    _ => None,
                }
            }
            _ => None,
        };

        let budget = state
            .workflow
            .as_ref()
            .map(|_| crate::pricing::budget_usage(&state.runs, &st.cfg.pricing, now));

        let flow = pipeline::FlowFacts {
            workflow: state.workflow.as_ref(),
            auto: Some(&state.auto),
            auto_json: Some(&state.aj),
            gate: state.gate,
            plan_commit_at,
            driver_alive: state.driver_alive,
            next_card,
            budget,
        };

        let view = pipeline::derive_response(&facts, &flow);

        Ok((
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            Json(view),
        )
            .into_response())
    })
    .await;

    match res {
        Ok(resp) => resp,
        Err(e) => {
            crate::term::emit(&format!("Lỗi GET /api/tasks/{id}/pipeline: {}", e.message));
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Lỗi nội bộ khi đọc pipeline của task {id}"),
            )
        }
    }
}

async fn post_task_action(
    st: Arc<AppState>,
    task_id: i64,
    action_id: &'static str,
    role: &'static str,
) -> Response {
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let state = match load_task_state(&st, &conn, task_id)? {
            Ok(s) => s,
            Err(resp) => return Ok(resp),
        };
        let task = &state.task;
        let project = &state.project;
        let busy_reason_str = state.busy.clone();

        let target_action = state.view.actions.iter().find(|a| a.id == action_id);
        let action = match target_action {
            Some(a) => a.clone(),
            None => {
                return Ok((
                    StatusCode::CONFLICT,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "application/json; charset=utf-8",
                    )],
                    Json(serde_json::json!({
                        "error": "Hành động không áp dụng cho trạng thái hiện tại",
                        "kind": "not_allowed",
                    })),
                )
                    .into_response());
            }
        };

        if !action.enabled {
            let reason = action.reason.unwrap_or_default();
            let is_busy = busy_reason_str.as_ref().map_or(false, |b| b == &reason);
            let kind = if is_busy { "busy" } else { "not_allowed" };
            return Ok((
                StatusCode::CONFLICT,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({
                    "error": reason,
                    "kind": kind,
                })),
            )
                .into_response());
        }
        // Chạy lại Lead trên quy trình `auto: true`: autopilot lập lại plan từ khung
        // mới (`auto {id} --replan`, không ghi chú) rồi review plan như thường.
        let replan_auto =
            action_id == "relead" && state.workflow.as_ref().map_or(false, |w| w.auto);
        let role = if replan_auto { "auto" } else { role };
        let args = match role {
            "auto" => vec![
                "auto".to_string(),
                task.id.to_string(),
                "--replan".to_string(),
            ],
            "dev" => vec![
                "run".to_string(),
                "dev".to_string(),
                "--".to_string(),
                project.id.clone(),
            ],
            "qa" => vec![
                "run".to_string(),
                "qa".to_string(),
                "--".to_string(),
                project.id.clone(),
            ],
            "lead" => vec![
                "run".to_string(),
                "lead".to_string(),
                "--name".to_string(),
                task.slug.clone(),
                "--".to_string(),
                project.id.clone(),
                task.title.clone(),
            ],
            _ => vec![],
        };

        let spec = crate::dashboard::jobs::JobSpec {
            role: role.to_string(),
            project: project.id.clone(),
            profile: project.profile.clone(),
            branch: Some(task.branch.clone()),
            task_id: Some(task.id),
            args,
        };

        match st.jobs.start(spec) {
            Ok(job_id) => Ok((
                StatusCode::ACCEPTED,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({ "job_id": job_id })),
            )
                .into_response()),
            Err(e) => {
                if e.message.contains("Dashboard đang dừng") {
                    Ok((
                        StatusCode::SERVICE_UNAVAILABLE,
                        [(
                            axum::http::header::CONTENT_TYPE,
                            "application/json; charset=utf-8",
                        )],
                        Json(serde_json::json!({
                            "error": "Dashboard đang dừng — không nhận việc mới"
                        })),
                    )
                        .into_response())
                } else {
                    Ok((
                        StatusCode::CONFLICT,
                        [(
                            axum::http::header::CONTENT_TYPE,
                            "application/json; charset=utf-8",
                        )],
                        Json(serde_json::json!({
                            "error": e.message,
                            "kind": "busy",
                        })),
                    )
                        .into_response())
                }
            }
        }
    })
    .await;

    match res {
        Ok(resp) => resp,
        Err(e) => internal_error("chạy hành động của task", &e),
    }
}

async fn post_task_dev(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(_body): Json<ActionBody>,
) -> Response {
    post_task_action(st, task_id, "run_dev", "dev").await
}

async fn post_task_qa(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(_body): Json<ActionBody>,
) -> Response {
    post_task_action(st, task_id, "run_qa", "qa").await
}

async fn post_task_relead(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(_body): Json<ActionBody>,
) -> Response {
    post_task_action(st, task_id, "relead", "lead").await
}

/// Lỗi 500 với thân cố định (W-3): chi tiết (đường dẫn, lỗi SQLite) chỉ in ra
/// terminal đang chạy `agent-brain serve`, không gửi về trình duyệt.
fn internal_error(what: &str, e: &crate::error::BrainError) -> Response {
    crate::term::emit(&format!("Lỗi nội bộ ({what}): {}", e.message));
    json_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        &format!("Lỗi nội bộ khi {what} — xem terminal đang chạy agent-brain serve"),
    )
}

fn json_resp(status: StatusCode, body: serde_json::Value) -> Response {
    (
        status,
        [(
            axum::http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )],
        Json(body),
    )
        .into_response()
}

fn conflict(error: &str, kind: &str) -> Response {
    json_resp(
        StatusCode::CONFLICT,
        serde_json::json!({ "error": error, "kind": kind }),
    )
}

/// Bắt đầu một job; lỗi được đổi thành 503 (dashboard đang dừng) hoặc 409 busy.
fn start_job(st: &AppState, spec: crate::dashboard::jobs::JobSpec) -> Result<u64, Response> {
    match st.jobs.start(spec) {
        Ok(job_id) => Ok(job_id),
        Err(e) => {
            if e.message.contains("Dashboard đang dừng") {
                Err(json_resp(
                    StatusCode::SERVICE_UNAVAILABLE,
                    serde_json::json!({ "error": "Dashboard đang dừng — không nhận việc mới" }),
                ))
            } else {
                Err(conflict(&e.message, "busy"))
            }
        }
    }
}

/// JobSpec cho một job của task (vị trí sau `--` trong `args`).
fn task_job(state: &TaskState, role: &str, args: Vec<String>) -> crate::dashboard::jobs::JobSpec {
    crate::dashboard::jobs::JobSpec {
        role: role.to_string(),
        project: state.project.id.clone(),
        profile: state.project.profile.clone(),
        branch: Some(state.task.branch.clone()),
        task_id: Some(state.task.id),
        args,
    }
}

/// Lý do (và kind) khi nút điều khiển không có trong trạng thái hiện tại:
/// `duplicate` khi trạng thái đã đi qua bước mà nút này làm, ngược lại `not_allowed`.
fn absent_control(control: &str, state: &TaskState) -> (String, &'static str) {
    let legacy = state.workflow.is_none();
    match control {
        "approve" if state.gate == Some(GateState::Approved) => {
            let at = state
                .auto
                .approved_at
                .map(crate::autopilot::view::fmt_hhmm)
                .unwrap_or_else(|| "--:--".to_string());
            (
                format!("Plan hiện tại đã được duyệt lúc {at} — không duyệt lại"),
                "duplicate",
            )
        }
        "auto_start" if !legacy && state.driver_alive => {
            ("Autopilot đang chạy task này".to_string(), "duplicate")
        }
        "auto_more_round" | "auto_end" if !legacy && state.view.auto.state != "stopped" => {
            ("Autopilot không ở trạng thái dừng".to_string(), "duplicate")
        }
        "auto_stop" if !legacy && !state.driver_alive => {
            ("Autopilot không chạy".to_string(), "duplicate")
        }
        _ => (NOT_ALLOWED_MSG.to_string(), "not_allowed"),
    }
}

/// Nút điều khiển của task (Duyệt plan, Trả về Lead, Bật / Dừng autopilot,
/// Chạy thêm 1 vòng, Dừng hẳn). Trạng thái được đọc lại trên server; chỉ làm
/// khi nút có mặt và đang bật.
async fn post_control(
    st: Arc<AppState>,
    task_id: i64,
    control: &'static str,
    rev: Option<String>,
    note: Option<String>,
) -> Response {
    // Ghi chú (chỉ Trả về Lead): cắt khoảng trắng, rỗng = không có ghi chú.
    let note = note
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if let Some(n) = note.as_deref() {
        if n.chars().count() > crate::roles::lead::MAX_NOTE_CHARS {
            return json_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Ghi chú quá dài (tối đa 500 ký tự)",
            );
        }
    }

    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let state = match load_task_state(&st, &conn, task_id)? {
            Ok(s) => s,
            Err(resp) => return Ok(resp),
        };

        if let Some(r) = rev.as_deref() {
            if r != state.rev {
                return Ok(json_resp(
                    StatusCode::CONFLICT,
                    serde_json::json!({ "error": STALE_MSG, "kind": "stale" }),
                ));
            }
        }

        let ctl = match state.view.controls.iter().find(|c| c.id == control) {
            Some(c) => c.clone(),
            None => {
                let (reason, kind) = absent_control(control, &state);
                return Ok(conflict(&reason, kind));
            }
        };
        if !ctl.enabled {
            let reason = ctl.reason.unwrap_or_default();
            let kind = if state.busy.as_deref() == Some(reason.as_str()) {
                "busy"
            } else {
                "not_allowed"
            };
            return Ok(conflict(&reason, kind));
        }

        let id_str = task_id.to_string();
        let wf_auto = state.workflow.as_ref().map_or(false, |w| w.auto);
        match control {
            "approve" => {
                let plan_commit = match claim::approve_check(&conn, &state.task, &state.root) {
                    Ok(pc) => pc,
                    Err(reason) => return Ok(conflict(&reason, "not_allowed")),
                };
                // Duyệt trước khi ghi, để hoàn lại khi autopilot không bắt đầu được.
                let prev = match (
                    state.auto.approved_at,
                    state.auto.approved_commit.as_deref(),
                ) {
                    (Some(at), Some(c)) => Some((at, c)),
                    _ => None,
                };
                let approved_at = claim::approve_record(&conn, task_id, &plan_commit)?;
                let job_id = if wf_auto {
                    let spec = task_job(&state, "auto", vec!["auto".to_string(), id_str]);
                    match start_job(&st, spec) {
                        Ok(j) => Some(j),
                        Err(resp) => {
                            // Không để lại duyệt nửa chừng khi autopilot không bắt đầu được;
                            // chỉ hoàn lại khi duyệt vẫn là của yêu cầu này (WR-04).
                            claim::revert_approval(&conn, task_id, &plan_commit, prev)?;
                            return Ok(resp);
                        }
                    }
                } else {
                    None
                };
                Ok(json_resp(
                    StatusCode::OK,
                    serde_json::json!({
                        "approved_at": approved_at,
                        "plan_commit": plan_commit,
                        "job_id": job_id,
                    }),
                ))
            }
            "return_lead" => {
                // D-17 / D-30: lập lại plan (ghi chú là một phần tử argv); bỏ duyệt
                // chỉ sau khi job đã bắt đầu — job bị từ chối thì giữ nguyên duyệt (WR-04).
                let (role, args) = if wf_auto {
                    let mut a = vec!["auto".to_string(), id_str, "--replan".to_string()];
                    if let Some(n) = note.as_deref() {
                        a.push(format!("--note={n}"));
                    }
                    ("auto", a)
                } else {
                    let mut a = vec![
                        "run".to_string(),
                        "lead".to_string(),
                        "--name".to_string(),
                        state.task.slug.clone(),
                    ];
                    if let Some(n) = note.as_deref() {
                        a.push(format!("--note={n}"));
                    }
                    a.push("--".to_string());
                    a.push(state.project.id.clone());
                    a.push(state.task.title.clone());
                    ("lead", a)
                };
                match start_job(&st, task_job(&state, role, args)) {
                    Ok(job_id) => {
                        crate::db::clear_task_approval(&conn, task_id)?;
                        Ok(json_resp(
                            StatusCode::ACCEPTED,
                            serde_json::json!({ "job_id": job_id }),
                        ))
                    }
                    Err(resp) => Ok(resp),
                }
            }
            "auto_start" | "auto_more_round" => {
                let mut args = vec!["auto".to_string(), id_str];
                if control == "auto_more_round" {
                    args.push("--more-round".to_string());
                }
                match start_job(&st, task_job(&state, "auto", args)) {
                    Ok(job_id) => Ok(json_resp(
                        StatusCode::ACCEPTED,
                        serde_json::json!({ "job_id": job_id }),
                    )),
                    Err(resp) => Ok(resp),
                }
            }
            "auto_stop" => {
                // Dừng mềm: run đang chạy chạy xong, autopilot không bắt đầu bước mới.
                crate::db::request_auto_stop(&conn, task_id, "soft")?;
                Ok(json_resp(StatusCode::OK, serde_json::json!({ "ok": true })))
            }
            "auto_end" => {
                // Dừng hẳn: giữ auto_json.stop làm lịch sử; Bật autopilot chạy lại sau.
                // Chỉ ghi khi task vẫn `stopped` và không có driver (WR-04).
                if !claim::end_autopilot(&conn, task_id, "stopped", &state.aj)? {
                    return Ok(json_resp(
                        StatusCode::CONFLICT,
                        serde_json::json!({ "error": STALE_MSG, "kind": "stale" }),
                    ));
                }
                Ok(json_resp(StatusCode::OK, serde_json::json!({ "ok": true })))
            }
            _ => Ok(conflict(NOT_ALLOWED_MSG, "not_allowed")),
        }
    })
    .await;

    match res {
        Ok(resp) => resp,
        Err(e) => internal_error("xử lý nút điều khiển của task", &e),
    }
}

async fn post_approve(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(body): Json<ControlBody>,
) -> Response {
    post_control(st, task_id, "approve", body.rev, None).await
}

async fn post_return(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(body): Json<ReturnBody>,
) -> Response {
    post_control(st, task_id, "return_lead", body.rev, body.note).await
}

async fn post_auto(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(body): Json<ControlBody>,
) -> Response {
    post_control(st, task_id, "auto_start", body.rev, None).await
}

async fn post_auto_stop(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(body): Json<ControlBody>,
) -> Response {
    post_control(st, task_id, "auto_stop", body.rev, None).await
}

async fn post_auto_more_round(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(body): Json<ControlBody>,
) -> Response {
    post_control(st, task_id, "auto_more_round", body.rev, None).await
}

async fn post_auto_end(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(body): Json<ControlBody>,
) -> Response {
    post_control(st, task_id, "auto_end", body.rev, None).await
}

async fn get_job(State(st): State<Arc<AppState>>, AxumPath(job_id): AxumPath<u64>) -> Response {
    let view = match st.jobs.get(job_id) {
        Some(v) => v,
        None => {
            return json_error(
                StatusCode::NOT_FOUND,
                &format!("Không tìm thấy job {job_id}"),
            );
        }
    };

    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let mut task_id = view.task_id;
        if task_id.is_none() {
            if let Some(ref branch) = view.branch {
                if let Ok(p_row_id) = crate::db::project_row_id(&conn, &view.project) {
                    if let Ok(Some(task)) = crate::db::find_task_by_branch(&conn, p_row_id, branch)
                    {
                        task_id = Some(task.id);
                    }
                }
            }
        }

        let mut run_id = None;
        if let Some(tid) = task_id {
            if let Ok(Some(run)) = crate::db::latest_run_for_task(&conn, tid) {
                run_id = Some(run.id);
            }
        }

        let mut val = serde_json::to_value(&view)
            .map_err(|e| crate::error::BrainError::error(e.to_string()))?;
        val["task_id"] = serde_json::json!(task_id);
        val["run_id"] = serde_json::json!(run_id);

        Ok((
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            Json(val),
        )
            .into_response())
    })
    .await;

    match res {
        Ok(resp) => resp,
        Err(e) => internal_error("đọc job", &e),
    }
}
