//! Task endpoints: giao việc, preflight guards, busy rules, resolve gate,
//! task list/detail, derived steps and dev/qa/relead starts.

use crate::config::Config;
use crate::dashboard::jobs::{JobManager, JobView};
use crate::dashboard::pipeline;
use crate::dashboard::taskstate::{self, GitFacts, TaskFacts};
use crate::dashboard::{blocking, json_error, AppState};
use crate::db::{RunRow, TaskRow};
use crate::error::BrainResult;
use crate::projects::Project;
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
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionBody {}

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

fn latest_run_per_role(
    conn: &rusqlite::Connection,
    task_id: i64,
    role: &str,
) -> BrainResult<Option<RunRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start,
                status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens,
                output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns,
                assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials,
                usage_source, base_commit, run_dir, error
         FROM runs WHERE task_id = ?1 AND role = ?2 ORDER BY started_at DESC, id DESC LIMIT 1",
    )?;
    let mut rows = stmt.query(rusqlite::params![task_id, role])?;
    if let Some(r) = rows.next()? {
        Ok(Some(RunRow {
            id: r.get(0)?,
            task_id: r.get(1)?,
            project_id: r.get(2)?,
            role: r.get(3)?,
            engine: r.get(4)?,
            profile: r.get(5)?,
            model: r.get(6)?,
            session_id: r.get(7)?,
            pid: r.get(8)?,
            pgid: r.get(9)?,
            pid_start: r.get(10)?,
            status: r.get(11)?,
            outcome: r.get(12)?,
            exit_code: r.get(13)?,
            kill_requested_at: r.get(14)?,
            started_at: r.get(15)?,
            ended_at: r.get(16)?,
            input_tokens: r.get(17)?,
            output_tokens: r.get(18)?,
            cache_read_tokens: r.get(19)?,
            cache_write_tokens: r.get(20)?,
            cost_usd: r.get(21)?,
            num_turns: r.get(22)?,
            assistant_turns: r.get(23)?,
            turns_before_first_edit: r.get(24)?,
            reads_outside_plan: r.get(25)?,
            permission_denials: r.get(26)?,
            usage_source: r.get(27)?,
            base_commit: r.get(28)?,
            run_dir: r.get(29)?,
            error: r.get(30)?,
        }))
    } else {
        Ok(None)
    }
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
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
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
        let spec = crate::dashboard::jobs::JobSpec {
            role: "lead".to_string(),
            project: project.id.clone(),
            profile: project.profile.clone(),
            branch: Some(branch.clone()),
            task_id: None,
            args: vec![
                "run".to_string(),
                "lead".to_string(),
                "--name".to_string(),
                slug.clone(),
                "--".to_string(),
                project.id.clone(),
                trimmed_req,
            ],
        };

        match st.jobs.start(spec) {
            Ok(job_id) => {
                let mut resp_val = serde_json::json!({
                    "job_id": job_id,
                    "project": project.id,
                    "slug": slug,
                    "branch": branch,
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
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
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

            let lead_run = latest_run_per_role(&conn, task.id, "lead")?;
            let dev_run = latest_run_per_role(&conn, task.id, "dev")?;
            let qa_run = latest_run_per_role(&conn, task.id, "qa")?;

            let facts = TaskFacts {
                task: &task,
                lead: lead_run.as_ref(),
                dev: dev_run.as_ref(),
                qa: qa_run.as_ref(),
                git: &GitFacts::default(),
                busy: None,
                now_ms,
            };
            let view = taskstate::derive(&facts);

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
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn get_task(State(st): State<Arc<AppState>>, AxumPath(id): AxumPath<i64>) -> Response {
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let task = match crate::db::get_task(&conn, id)? {
            Some(t) => t,
            None => {
                return Ok(json_error(
                    StatusCode::NOT_FOUND,
                    &format!("Không tìm thấy task {id}"),
                ));
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
                return Ok(json_error(
                    StatusCode::NOT_FOUND,
                    &format!("Không tìm thấy dự án {project_slug}"),
                ));
            }
        };

        let proj_root = PathBuf::from(&project.path);
        let current_branch = crate::gitops::current_branch(&proj_root);
        let is_on_branch = current_branch.as_deref() == Some(&task.branch);
        let dirty_set = crate::gitops::changed_paths(&proj_root).unwrap_or_default();
        let mut dirty: Vec<String> = dirty_set.into_iter().collect();
        dirty.sort();

        let plan_dirty = is_on_branch && dirty.iter().any(|p| p == ".plan.md");

        let (plan_text, plan_source) = if is_on_branch {
            let p_path = proj_root.join(".plan.md");
            if p_path.is_file() {
                (std::fs::read_to_string(&p_path).ok(), Some("worktree"))
            } else {
                (None, None)
            }
        } else {
            let show_arg = format!("{}:.plan.md", task.branch);
            match crate::gitops::git(&proj_root, &["show", &show_arg]) {
                Ok(text) => (Some(text), Some("branch")),
                Err(_) => (None, None),
            }
        };

        let plan_commit = {
            let expected_subj = format!("{}: plan", task.slug);
            let mut commit = None;
            if let Ok(log_out) =
                crate::gitops::git(&proj_root, &["log", "--format=%H %s", &task.branch])
            {
                for line in log_out.lines() {
                    if let Some((sha, subj)) = line.split_once(' ') {
                        if subj.trim() == expected_subj {
                            commit = Some(sha.trim().to_string());
                            break;
                        }
                    }
                }
            }
            commit
        };

        let code_after_plan = if is_on_branch {
            if let Some(ref pc) = plan_commit {
                match crate::gitops::diff_names(&proj_root, pc, "HEAD") {
                    Ok(mut diff) => {
                        diff.remove(".plan.md");
                        diff.remove(".qa-report.md");
                        !diff.is_empty()
                    }
                    Err(_) => false,
                }
            } else {
                false
            }
        } else {
            false
        };

        let (plan_obj, plan_exists, plan_blockers, plan_errors) = if let Some(ref text) = plan_text
        {
            let plan = crate::planfile::parse_plan(text);
            let blockers = crate::planfile::has_blockers(&plan);
            let errors = if plan_source == Some("worktree") {
                crate::planfile::validate_plan(&plan, Some(&proj_root))
            } else {
                Vec::new()
            };
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

            let obj = serde_json::json!({
                "path": ".plan.md",
                "source": plan_source.unwrap(),
                "commit": plan_commit,
                "dirty": plan_dirty,
                "title": plan.title,
                "sections": sections,
                "changes": changes,
                "blockers": blockers,
                "errors": errors,
            });
            (Some(obj), true, blockers, errors)
        } else {
            (None, false, false, Vec::new())
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
            match crate::gitops::git(&proj_root, &["show", &show_arg]) {
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
                "source": qa_source.unwrap(),
                "verdict": verdict,
                "sections": sections,
            }))
        } else {
            None
        };

        let git_facts = GitFacts {
            current_branch,
            dirty,
            plan_exists,
            plan_blockers,
            plan_errors,
            plan_commit,
            code_after_plan,
        };

        let busy_reason_str =
            busy_reason_for_task(&conn, &st.jobs, &project, task.id, &task.branch)?;

        let lead_run = latest_run_per_role(&conn, task.id, "lead")?;
        let dev_run = latest_run_per_role(&conn, task.id, "dev")?;
        let qa_run = latest_run_per_role(&conn, task.id, "qa")?;

        let facts = TaskFacts {
            task: &task,
            lead: lead_run.as_ref(),
            dev: dev_run.as_ref(),
            qa: qa_run.as_ref(),
            git: &git_facts,
            busy: busy_reason_str,
            now_ms: crate::db::now_ms(),
        };

        let view = taskstate::derive(&facts);

        let mut runs_stmt = conn.prepare(
            "SELECT id, role, status, outcome, input_tokens, output_tokens,
                    cache_read_tokens, cache_write_tokens, cost_usd, started_at, ended_at
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
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let job_view: Option<JobView> = st.jobs.list().into_iter().find(|j| {
            j.state == "running"
                && (j.task_id == Some(task.id) || j.branch.as_deref() == Some(&task.branch))
        });

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
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn get_pipeline(State(st): State<Arc<AppState>>, AxumPath(id): AxumPath<i64>) -> Response {
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let task = match crate::db::get_task(&conn, id)? {
            Some(t) => t,
            None => {
                return Ok(json_error(
                    StatusCode::NOT_FOUND,
                    &format!("Không tìm thấy task {id}"),
                ));
            }
        };

        let runs = crate::db::runs_for_task(&conn, id)?;
        let plan = pipeline::PlanFacts::Unknown;
        let now = crate::db::now_ms();
        let latest = |role: &str| {
            runs.iter()
                .filter(|r| r.role == role)
                .max_by_key(|r| (r.started_at, r.id))
        };
        let facts = TaskFacts {
            task: &task,
            lead: latest("lead"),
            dev: latest("dev"),
            qa: latest("qa"),
            git: &GitFacts::default(),
            busy: None,
            now_ms: now,
        };
        let current_step = taskstate::derive(&facts).current_step;
        let view = pipeline::derive(&pipeline::PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &plan,
            current_step: &current_step,
            now_ms: now,
        });

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
        let task = match crate::db::get_task(&conn, task_id)? {
            Some(t) => t,
            None => {
                return Ok((
                    StatusCode::NOT_FOUND,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "application/json; charset=utf-8",
                    )],
                    Json(serde_json::json!({ "error": format!("Không tìm thấy task {task_id}") })),
                )
                    .into_response());
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
                return Ok(json_error(
                    StatusCode::NOT_FOUND,
                    &format!("Không tìm thấy dự án {project_slug}"),
                ));
            }
        };

        let proj_root = PathBuf::from(&project.path);
        let current_branch = crate::gitops::current_branch(&proj_root);
        let is_on_branch = current_branch.as_deref() == Some(&task.branch);
        let dirty_set = crate::gitops::changed_paths(&proj_root).unwrap_or_default();
        let mut dirty: Vec<String> = dirty_set.into_iter().collect();
        dirty.sort();

        let (plan_text, plan_source) = if is_on_branch {
            let p_path = proj_root.join(".plan.md");
            if p_path.is_file() {
                (std::fs::read_to_string(&p_path).ok(), Some("worktree"))
            } else {
                (None, None)
            }
        } else {
            let show_arg = format!("{}:.plan.md", task.branch);
            match crate::gitops::git(&proj_root, &["show", &show_arg]) {
                Ok(text) => (Some(text), Some("branch")),
                Err(_) => (None, None),
            }
        };

        let plan_commit = {
            let expected_subj = format!("{}: plan", task.slug);
            let mut commit = None;
            if let Ok(log_out) =
                crate::gitops::git(&proj_root, &["log", "--format=%H %s", &task.branch])
            {
                for line in log_out.lines() {
                    if let Some((sha, subj)) = line.split_once(' ') {
                        if subj.trim() == expected_subj {
                            commit = Some(sha.trim().to_string());
                            break;
                        }
                    }
                }
            }
            commit
        };

        let code_after_plan = if is_on_branch {
            if let Some(ref pc) = plan_commit {
                match crate::gitops::diff_names(&proj_root, pc, "HEAD") {
                    Ok(mut diff) => {
                        diff.remove(".plan.md");
                        diff.remove(".qa-report.md");
                        !diff.is_empty()
                    }
                    Err(_) => false,
                }
            } else {
                false
            }
        } else {
            false
        };

        let (plan_exists, plan_blockers, plan_errors) = if let Some(ref text) = plan_text {
            let plan = crate::planfile::parse_plan(text);
            let blockers = crate::planfile::has_blockers(&plan);
            let errors = if plan_source == Some("worktree") {
                crate::planfile::validate_plan(&plan, Some(&proj_root))
            } else {
                Vec::new()
            };
            (true, blockers, errors)
        } else {
            (false, false, Vec::new())
        };

        let git_facts = GitFacts {
            current_branch,
            dirty,
            plan_exists,
            plan_blockers,
            plan_errors,
            plan_commit,
            code_after_plan,
        };

        let busy_reason_str =
            busy_reason_for_task(&conn, &st.jobs, &project, task.id, &task.branch)?;

        let lead_run = latest_run_per_role(&conn, task.id, "lead")?;
        let dev_run = latest_run_per_role(&conn, task.id, "dev")?;
        let qa_run = latest_run_per_role(&conn, task.id, "qa")?;

        let facts = TaskFacts {
            task: &task,
            lead: lead_run.as_ref(),
            dev: dev_run.as_ref(),
            qa: qa_run.as_ref(),
            git: &git_facts,
            busy: busy_reason_str.clone(),
            now_ms: crate::db::now_ms(),
        };

        let view = taskstate::derive(&facts);

        let target_action = view.actions.into_iter().find(|a| a.id == action_id);
        let action = match target_action {
            Some(a) => a,
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

        let args = match role {
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
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
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
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}
