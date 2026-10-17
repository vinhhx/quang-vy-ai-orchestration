//! Task endpoints: giao việc, preflight guards, busy rules, and resolve gate.

use crate::config::Config;
use crate::dashboard::jobs::JobManager;
use crate::dashboard::{blocking, json_error, AppState};
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
        .route("/api/tasks", post(create_task))
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

#[derive(serde::Deserialize)]
struct PreflightQuery {
    project: Option<String>,
    slug: Option<String>,
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
