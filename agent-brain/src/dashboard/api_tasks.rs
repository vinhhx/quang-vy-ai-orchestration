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
    /// Đội hình cho task mới (D-12); không có thì dùng đội hình của dự án rồi `mac-dinh`.
    #[serde(default)]
    pub team: Option<String>,
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

/// Thân yêu cầu của đổi đội hình và chạy lại bước hết quota: tên đội hình và `rev` tùy chọn.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TeamBody {
    team: String,
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
        .route("/api/tasks/{id}/team", post(post_change_team))
        .route("/api/tasks/{id}/quota-rerun", post(post_quota_rerun))
        .route("/api/tasks/{id}/human/done", post(post_human_done))
        .route("/api/tasks/{id}/human/return", post(post_human_return))
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

/// Nơi giao việc: dashboard (Giao việc) hay Telegram. Chỉ mã phía máy chủ chọn giá
/// trị này; thân yêu cầu HTTP không có trường nào để một trình duyệt tự nhận là Telegram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Dashboard,
    Telegram,
}

/// Đầu vào của đường tạo task dùng chung (dashboard và bot Telegram).
#[derive(Debug, Clone)]
pub struct StartRequest<'a> {
    pub project: &'a str,
    pub request: &'a str,
    pub name: Option<&'a str>,
    pub workflow: Option<&'a str>,
    /// Đội hình yêu cầu (D-12); `None` = đội hình của dự án rồi `mac-dinh`.
    pub team: Option<&'a str>,
    pub origin: Origin,
    /// Chạy mọi kiểm tra tới hết `lead_preflight` rồi dừng, không bắt đầu job nào.
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StartInfo {
    pub project: Project,
    pub slug: String,
    pub branch: String,
    pub workflow: String,
    pub auto: bool,
    pub warning: Option<String>,
    pub replan: bool,
    /// Tên đội hình đã chọn cho task (body > đội hình dự án > `mac-dinh`).
    pub team: String,
    /// Ghi chú khi đội hình của dự án không có trong teams.json và phải dùng `mac-dinh`.
    pub team_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StartOutcome {
    /// Dry run: mọi kiểm tra đạt, chưa có job.
    Ready(StartInfo),
    Started {
        job_id: u64,
        info: StartInfo,
    },
    Invalid(String),
    ProjectRefused {
        status: u16,
        body: serde_json::Value,
    },
    PreflightRefused {
        reason: String,
        kind: String,
    },
    Busy(String),
    Unavailable,
}

/// Vector đối số của job giao việc; positional (dự án, yêu cầu) luôn sau `--` nên một
/// yêu cầu bắt đầu bằng `-` vẫn là dữ liệu.
pub fn new_task_args(
    auto: bool,
    slug: &str,
    workflow: &str,
    project_id: &str,
    request: &str,
    origin: Origin,
) -> (&'static str, Vec<String>) {
    new_task_args_team(auto, slug, workflow, None, project_id, request, origin)
}

/// Như `new_task_args`, thêm `--team <tên>` ngay sau cặp `--workflow` khi có đội hình.
pub fn new_task_args_team(
    auto: bool,
    slug: &str,
    workflow: &str,
    team: Option<&str>,
    project_id: &str,
    request: &str,
    origin: Origin,
) -> (&'static str, Vec<String>) {
    let (role, head): (&'static str, [&str; 2]) = if auto {
        ("auto", ["auto", "--start"])
    } else {
        ("lead", ["run", "lead"])
    };
    let mut args: Vec<String> = head.iter().map(|s| s.to_string()).collect();
    args.extend(
        ["--name", slug, "--workflow", workflow]
            .iter()
            .map(|s| s.to_string()),
    );
    if let Some(team) = team {
        args.push("--team".to_string());
        args.push(team.to_string());
    }
    // Nguồn task đi theo argv (trước `--`) để `run lead` / `auto --start` lưu vào brain.db.
    if origin == Origin::Telegram {
        args.push("--origin".to_string());
        args.push("telegram".to_string());
    }
    args.extend(["--", project_id, request].iter().map(|s| s.to_string()));
    (role, args)
}

/// Toàn bộ đường Giao việc: kiểm tra đầu vào → dự án → quy trình → preflight → job.
/// Hàm thường (không có kiểu axum) để dashboard và bot Telegram dùng chung.
pub fn start_new_task(st: &AppState, req: &StartRequest) -> BrainResult<StartOutcome> {
    let trimmed_req = req.request.trim();
    if trimmed_req.is_empty() {
        return Ok(StartOutcome::Invalid(
            "Yêu cầu không được để trống".to_string(),
        ));
    }
    if trimmed_req.chars().count() > MAX_REQUEST_CHARS {
        return Ok(StartOutcome::Invalid(format!(
            "Yêu cầu quá dài (tối đa {MAX_REQUEST_CHARS} ký tự)"
        )));
    }

    let name_or_req = req.name.unwrap_or(trimmed_req);
    let slug = crate::routing::slugify(name_or_req);
    if slug.is_empty() {
        return Ok(StartOutcome::Invalid(
            "Không thể tạo tên task từ yêu cầu — hãy truyền cờ --name <slug>".to_string(),
        ));
    }

    let map = crate::projects::load_map(&st.map_path)?;
    let (project, proj_root) = match resolve_project(&map, &st.cfg, req.project) {
        Ok(p) => p,
        Err(refusal) => {
            return Ok(StartOutcome::ProjectRefused {
                status: refusal.status.as_u16(),
                body: refusal.body,
            });
        }
    };

    // D-14 / D-24: body > project default > mac-dinh; an unknown name is
    // refused before any job starts.
    let loaded = crate::workflows::load(&st.cfg);
    let project_default = crate::projects::workflow_of(&map, &project.id);
    let workflow =
        match crate::workflows::resolve(&loaded, req.workflow, project_default.as_deref()) {
            Ok(r) => r.workflow,
            Err(e) => return Ok(StartOutcome::Invalid(e.message)),
        };

    // TEAM-03 / D-12: body > project default > mac-dinh; an unknown explicit team is
    // refused before any job starts.
    let (teams_loaded, _staff) = crate::teams::load();
    let team = match crate::teams::resolve(
        &teams_loaded,
        req.team,
        crate::projects::team_of(&map, &project.id).as_deref(),
    ) {
        Ok(r) => r,
        Err(e) => return Ok(StartOutcome::Invalid(e.message)),
    };

    // ENG-10 / D-36 / D-42: a team that names a profile-restricted engine cannot be bound to a
    // project of another profile. Decided here, before the preflight, any job, branch or row.
    // Giao việc, Telegram `/task` and a forbidden project default all pass through this function.
    if let Some(msg) = crate::teams::team_profile_refusal(
        &team.team,
        &st.cfg.engines,
        &project.id,
        &project.profile,
    ) {
        return Ok(StartOutcome::Invalid(msg));
    }

    let conn = crate::db::open(&st.db_path)?;
    let pf = lead_preflight(&conn, &st.jobs, &project, &proj_root, &slug)?;
    if !pf.can_start {
        return Ok(StartOutcome::PreflightRefused {
            reason: pf.reason.unwrap_or_default(),
            kind: pf.kind.unwrap_or_default(),
        });
    }

    let branch = format!("task/{slug}");
    // OD-8: a Telegram request may neither re-plan a checked-out task branch nor
    // reuse a branch that already has a task row; the dashboard keeps both.
    if req.origin == Origin::Telegram {
        if pf.replan {
            return Ok(StartOutcome::PreflightRefused {
                reason: format!(
                    "Nhánh {branch} đang được checkout — task giao từ Telegram không lập lại plan của task có sẵn"
                ),
                kind: "exists".to_string(),
            });
        }
        // An Err here means the project has no row yet, so it has no task either.
        let has_task = match crate::db::project_row_id(&conn, &project.id) {
            Ok(pid) => crate::db::find_task_by_branch(&conn, pid, &branch)?.is_some(),
            Err(_) => false,
        };
        if has_task {
            return Ok(StartOutcome::PreflightRefused {
                reason: format!("Đã có task cho nhánh {branch} — dùng tên khác"),
                kind: "exists".to_string(),
            });
        }
    }
    let info = StartInfo {
        project,
        slug,
        branch,
        workflow: workflow.name.clone(),
        auto: workflow.auto,
        warning: pf.warning,
        replan: pf.replan,
        team: team.team.name.clone(),
        team_note: team.note.clone(),
    };
    if req.dry_run {
        return Ok(StartOutcome::Ready(info));
    }

    let (role, args) = new_task_args_team(
        workflow.auto,
        &info.slug,
        &workflow.name,
        Some(&info.team),
        &info.project.id,
        trimmed_req,
        req.origin,
    );
    let spec = crate::dashboard::jobs::JobSpec {
        role: role.to_string(),
        project: info.project.id.clone(),
        profile: info.project.profile.clone(),
        branch: Some(info.branch.clone()),
        task_id: None,
        args,
    };

    match st.jobs.start(spec) {
        Ok(job_id) => Ok(StartOutcome::Started { job_id, info }),
        Err(e) => {
            if e.message.contains("Dashboard đang dừng") {
                Ok(StartOutcome::Unavailable)
            } else {
                Ok(StartOutcome::Busy(e.message))
            }
        }
    }
}

fn json_response(status: StatusCode, body: serde_json::Value) -> Response {
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

/// Ánh xạ kết quả `start_new_task` sang đúng các mã trạng thái và thân JSON của
/// `POST /api/tasks` trước khi tách hàm.
fn start_response(outcome: StartOutcome) -> Response {
    match outcome {
        StartOutcome::Started { job_id, info } => {
            let mut resp_val = serde_json::json!({
                "job_id": job_id,
                "project": info.project.id,
                "slug": info.slug,
                "branch": info.branch,
                "workflow": info.workflow,
                "auto": info.auto,
                "team": info.team,
            });
            if let Some(w) = info.warning {
                resp_val["warning"] = serde_json::json!(w);
            }
            if let Some(n) = info.team_note {
                resp_val["team_note"] = serde_json::json!(n);
            }
            json_response(StatusCode::ACCEPTED, resp_val)
        }
        StartOutcome::Ready(_) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Lỗi nội bộ: dry run trong giao việc",
        ),
        StartOutcome::Invalid(m) => json_error(StatusCode::UNPROCESSABLE_ENTITY, &m),
        StartOutcome::ProjectRefused { status, body } => json_response(
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            body,
        ),
        StartOutcome::PreflightRefused { reason, kind } => json_response(
            StatusCode::CONFLICT,
            serde_json::json!({ "error": reason, "kind": kind }),
        ),
        StartOutcome::Busy(m) => json_response(
            StatusCode::CONFLICT,
            serde_json::json!({ "error": m, "kind": "busy" }),
        ),
        StartOutcome::Unavailable => json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({ "error": "Dashboard đang dừng — không nhận việc mới" }),
        ),
    }
}

async fn create_task(State(st): State<Arc<AppState>>, Json(body): Json<NewTask>) -> Response {
    let res = blocking(move || {
        start_new_task(
            &st,
            &StartRequest {
                project: &body.project,
                request: &body.request,
                name: body.name.as_deref(),
                workflow: body.workflow.as_deref(),
                team: body.team.as_deref(),
                origin: Origin::Dashboard,
                dry_run: false,
            },
        )
    })
    .await;

    match res {
        Ok(outcome) => start_response(outcome),
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
        // staff.json is read once for the whole list.
        let staff = crate::staff::load();

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
            // Reviews are not among the three roles above; a quota stop of one still counts.
            let latest_run = crate::db::latest_run_for_task(&conn, task.id)?;
            // A person's pending step (06-16): mode and display name, so the overview banner and
            // group match the task detail.
            let team_json = crate::db::task_team_json(&conn, task.id)?;
            let human = crate::teams::TaskTeam::parse(team_json.as_deref())
                .and_then(|t| t.wait)
                .map(|w| (w.mode.clone(), staff.display_name_of(&w.staff)));
            let auto_facts = AutoFacts {
                workflow: workflow.as_ref(),
                auto: Some(&auto),
                auto_json: Some(&aj),
                driver_alive: claim::driver_alive(&auto),
                running: running.as_ref(),
                gate: None,
                plan_dirty: false,
                latest_run: latest_run.as_ref(),
                human_mode: human.as_ref().map(|(m, _)| m.as_str()),
                human_name: human.as_ref().map(|(_, n)| n.as_str()),
            };
            let view = taskstate::derive_with(&facts, &auto_facts);
            // Tổng quan shows the reason line of a quota stop for manual tasks (UI-SPEC).
            let quota_detail = latest_run
                .as_ref()
                .filter(|r| r.outcome.as_deref() == Some(crate::quota::OUTCOME_QUOTA))
                .map(|r| crate::quota::stop_detail(r.id, &r.role, &r.engine));

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
                "human_mode": human.as_ref().map(|(m, _)| m.as_str()),
                "human_name": human.as_ref().map(|(_, n)| n.as_str()),
                "quota_detail": quota_detail,
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
    /// `tasks.origin` (05.4): `Some("telegram")` khi task do Telegram giao.
    pub origin: Option<String>,
    /// Task do Telegram giao và dự án còn trong `telegram_projects` (đọc file lúc dựng; fail closed).
    /// Luôn `false` khi task không do Telegram giao.
    pub project_telegram_ok: bool,
    /// `tasks.team_json` nguyên văn (`None` = task tạo trước Phase 6); là giá trị cũ của CAS.
    pub team_json: Option<String>,
    /// `team_json` đã đọc được (`None` = task cũ hoặc dữ liệu hỏng).
    pub team: Option<crate::teams::TaskTeam>,
    /// teams.json đang có (cộng đội hình có sẵn).
    pub teams: crate::teams::LoadedTeams,
    pub staff: crate::staff::LoadedStaff,
}

/// Token đổi khi trạng thái task, run mới nhất, autopilot, cờ dừng, duyệt, commit plan hoặc
/// `team_json` đổi.
pub fn task_rev(
    task: &TaskRow,
    auto: &TaskAuto,
    latest: Option<&RunRow>,
    plan_commit: Option<&str>,
    team_json: Option<&str>,
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
    team_json.hash(&mut h);
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

/// Mọi đường dơ đều thuộc việc dở của Dev: file trong bảng Thay đổi của plan, file giao thức
/// hoặc `.handover.md` (cùng luật với `roles::dev::cmd_dev` khi tiếp tục sau hết quota).
fn dirty_is_continuation(plan_text: &str, dirty: &[String]) -> bool {
    use unicode_normalization::UnicodeNormalization;
    let plan = crate::planfile::parse_plan(plan_text);
    let plan_set: std::collections::BTreeSet<String> = crate::planfile::plan_files(&plan)
        .iter()
        .map(|f| crate::planfile::normalize_plan_path(f).nfc().collect())
        .collect();
    dirty.iter().all(|p| {
        let n: String = p.nfc().collect();
        plan_set.contains(&n)
            || crate::stream::PROTOCOL_FILES.contains(&n.as_str())
            || n == ".handover.md"
    })
}

/// Đọc toàn bộ trạng thái task (Err(thông báo) = task / dự án không có; dashboard trả 404).
pub fn load_task_state(
    st: &AppState,
    conn: &rusqlite::Connection,
    task_id: i64,
) -> BrainResult<Result<TaskState, String>> {
    let task = match crate::db::get_task(conn, task_id)? {
        Some(t) => t,
        None => return Ok(Err(format!("Không tìm thấy task {task_id}"))),
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
        None => return Ok(Err(format!("Không tìm thấy dự án {project_slug}"))),
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

    let mut git = GitFacts {
        current_branch,
        dirty,
        plan_exists,
        plan_blockers,
        plan_errors,
        plan_commit,
        code_after_plan,
        continuation_ok: false,
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

    // D-16 / ENG-03: the same rule `run dev` applies (06-07) — the latest Dev run stopped on
    // quota, and every dirty path is a plan file, a protocol file or `.handover.md`. Then the
    // partial diff is work to continue, so Chạy Dev stays enabled instead of refusing the tree.
    git.continuation_ok = is_on_branch
        && !git.dirty.is_empty()
        && latest_of(&runs, "dev").map_or(false, |r| {
            r.outcome.as_deref() == Some(crate::quota::OUTCOME_QUOTA)
        })
        && plan_text
            .as_deref()
            .map_or(false, |text| dirty_is_continuation(text, &git.dirty));

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

    let team_json = crate::db::task_team_json(conn, task.id)?;
    let team = crate::teams::TaskTeam::parse(team_json.as_deref());
    let (teams, staff) = crate::teams::load();
    // D-29: only a step that is still waiting blocks a team change; a rejected one does not.
    let waiting_staff = team
        .as_ref()
        .and_then(|t| t.wait.as_ref())
        .filter(|w| w.mode == "waiting")
        .map(|w| staff.display_name_of(&w.staff));
    // 06-16: mode and display name of any human marker (waiting or rejected).
    let human_wait = team
        .as_ref()
        .and_then(|t| t.wait.as_ref())
        .map(|w| (w.mode.clone(), staff.display_name_of(&w.staff)));

    let mut view = {
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
            latest_run: runs.iter().max_by_key(|r| (r.started_at, r.id)),
            human_mode: human_wait.as_ref().map(|(m, _)| m.as_str()),
            human_name: human_wait.as_ref().map(|(_, n)| n.as_str()),
        };
        taskstate::derive_with(&facts, &auto_facts)
    };
    if let Some((mode, name)) = human_wait.as_ref() {
        // D-20: Xong / Trả lại exist whenever a person's step is pending; their `enabled` and
        // `reason` carry the refusals the shared `human_action` repeats server-side.
        let live = if running_of(&runs).is_some() {
            Some("Đang có run chạy — chờ run xong hoặc bấm Kill")
        } else if driver_alive {
            Some("Autopilot đang chạy task này — chờ autopilot dừng")
        } else {
            None
        };
        let on_branch = (git.current_branch.as_deref() != Some(task.branch.as_str())).then(|| {
            format!(
                "Không ở nhánh task '{}' (nhánh hiện tại: '{}') — không đoán",
                task.branch,
                git.current_branch.as_deref().unwrap_or("None")
            )
        });
        view.actions
            .push(taskstate::build_human_done(mode, name, on_branch, live));
        view.actions
            .push(taskstate::build_human_return(mode, name, live));
    }
    view.actions.push(taskstate::build_change_team(
        running_of(&runs),
        driver_alive,
        waiting_staff.as_deref(),
    ));
    let latest = runs.iter().max_by_key(|r| (r.started_at, r.id));
    let mut quota_rerun = taskstate::build_quota_rerun(latest, running_of(&runs), driver_alive);
    // Anything else holding the project (another task, a job just handed over) also refuses it.
    if quota_rerun.enabled {
        if let Some(b) = busy.as_ref() {
            quota_rerun.enabled = false;
            quota_rerun.reason = Some(b.clone());
        }
    }
    view.actions.push(quota_rerun);

    let rev = task_rev(
        &task,
        &auto,
        latest,
        git.plan_commit.as_deref(),
        team_json.as_deref(),
    );
    let origin = crate::db::task_origin(conn, task.id)?;
    let project_telegram_ok = origin.as_deref() == Some(crate::db::ORIGIN_TELEGRAM)
        && crate::telegram::intake::project_in_allowlist(&st.map_path, &project.id);

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
        origin,
        project_telegram_ok,
        team_json,
        team,
        teams,
        staff,
    }))
}

/// Tên đội hình hiện tại của task: tên đã lưu, hoặc `mac-dinh` cho task cũ (không có
/// `team_json`) và cho dữ liệu hỏng không đọc được.
fn current_team_name(state: &TaskState) -> String {
    state
        .team
        .as_ref()
        .map(|t| t.name.clone())
        .unwrap_or_else(|| crate::teams::DEFAULT_TEAM.to_string())
}

/// Kết quả đổi đội hình của task, không gắn với HTTP: dashboard (`change_team_response`) và bot
/// Telegram dùng chung một hàm (D-13, D-29).
#[derive(Debug, Clone, PartialEq)]
pub enum TeamOutcome {
    /// Đã ghi tên đội hình mới cùng một dòng lịch sử.
    Changed { from: String, to: String },
    /// Từ chối với lý do và loại (`busy`, `not_allowed`).
    Refused { reason: String, kind: &'static str },
    /// `rev` không khớp hoặc `team_json` đã đổi trong lúc ghi (CAS thua).
    Stale,
    /// Task / dự án không có.
    NotFound(String),
    /// Tên đội hình không có trong teams.json.
    Invalid(String),
}

fn source_label(source: ActionSource) -> &'static str {
    match source {
        ActionSource::Dashboard => "dashboard",
        ActionSource::Telegram => "telegram",
    }
}

fn unknown_team_message(state: &TaskState, team: &str) -> String {
    format!(
        "Không có đội hình '{team}'. Đội hình có: {}",
        state.teams.names().join(", ")
    )
}

/// Ghi `to` làm đội hình của task kèm một dòng lịch sử `{at, by, from, to}` bằng CAS trên
/// `team_json` (giá trị cũ là giá trị `state` đã đọc). `Some(from)` khi ghi được, `None` khi
/// CAS thua. Task cũ (NULL) đi từ `mac-dinh` với lịch sử rỗng; `wait` được giữ nguyên.
fn write_team_change(
    conn: &rusqlite::Connection,
    state: &TaskState,
    to: &str,
    source: ActionSource,
) -> BrainResult<Option<String>> {
    let from = current_team_name(state);
    let mut new_team = state
        .team
        .clone()
        .unwrap_or_else(|| crate::teams::TaskTeam::new(&from));
    new_team.name = to.to_string();
    new_team.history.push(crate::teams::TeamChange {
        at: crate::db::now_ms(),
        by: source_label(source).to_string(),
        from: from.clone(),
        to: to.to_string(),
    });
    let wrote = crate::db::set_task_team_cas(
        conn,
        state.task.id,
        state.team_json.as_deref(),
        &new_team.to_json(),
    )?;
    Ok(wrote.then_some(from))
}

/// Đổi đội hình của task (D-13, D-29). Trạng thái được đọc lại; thứ tự kiểm tra: task có →
/// `rev` → nút `change_team` bật → đội hình có trong teams.json → khác đội hình hiện tại →
/// ghi bằng CAS. Đội hình mới áp dụng từ bước kế tiếp; các run đã chạy giữ engine của chúng.
pub fn change_team(
    st: &AppState,
    task_id: i64,
    team: &str,
    rev: Option<&str>,
    source: ActionSource,
) -> BrainResult<TeamOutcome> {
    let team = team.trim();
    let conn = crate::db::open(&st.db_path)?;
    let state = match load_task_state(st, &conn, task_id)? {
        Ok(s) => s,
        Err(msg) => return Ok(TeamOutcome::NotFound(msg)),
    };
    if let Some(r) = rev {
        if r != state.rev {
            return Ok(TeamOutcome::Stale);
        }
    }
    if let Some(refusal) = change_team_refusal(&state) {
        return Ok(refusal);
    }
    let Some(picked) = state.teams.get(team) else {
        return Ok(TeamOutcome::Invalid(unknown_team_message(&state, team)));
    };
    // ENG-10 / D-36 / D-42: refused before the team is written.
    if let Some(msg) = crate::teams::team_profile_refusal(
        picked,
        &st.cfg.engines,
        &state.project.id,
        &state.project.profile,
    ) {
        return Ok(TeamOutcome::Invalid(msg));
    }
    if team == current_team_name(&state) {
        return Ok(TeamOutcome::Refused {
            reason: SAME_TEAM_MSG.to_string(),
            kind: "not_allowed",
        });
    }
    match write_team_change(&conn, &state, team, source)? {
        Some(from) => Ok(TeamOutcome::Changed {
            from,
            to: team.to_string(),
        }),
        None => Ok(TeamOutcome::Stale),
    }
}

const SAME_TEAM_MSG: &str = "Chọn đội hình khác đội hình hiện tại.";

/// Từ chối của nút `change_team` (run đang chạy, autopilot, người đang chờ) hoặc `None`.
fn change_team_refusal(state: &TaskState) -> Option<TeamOutcome> {
    let action = state.view.actions.iter().find(|a| a.id == "change_team")?;
    if action.enabled {
        return None;
    }
    let reason = action.reason.clone().unwrap_or_default();
    let live = reason == taskstate::CHANGE_TEAM_RUN_LIVE
        || reason == taskstate::CHANGE_TEAM_AUTOPILOT
        || state.busy.as_deref() == Some(reason.as_str());
    Some(TeamOutcome::Refused {
        reason,
        kind: if live { "busy" } else { "not_allowed" },
    })
}

fn change_team_response(o: TeamOutcome) -> Response {
    match o {
        TeamOutcome::Changed { from, to } => json_resp(
            StatusCode::OK,
            serde_json::json!({ "ok": true, "from": from, "to": to }),
        ),
        TeamOutcome::Refused { reason, kind } => conflict(&reason, kind),
        TeamOutcome::Stale => conflict(STALE_MSG, "stale"),
        TeamOutcome::NotFound(msg) => json_error(StatusCode::NOT_FOUND, &msg),
        TeamOutcome::Invalid(msg) => json_error(StatusCode::UNPROCESSABLE_ENTITY, &msg),
    }
}

async fn post_change_team(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(body): Json<TeamBody>,
) -> Response {
    let res = blocking(move || {
        change_team(
            &st,
            task_id,
            &body.team,
            body.rev.as_deref(),
            ActionSource::Dashboard,
        )
    })
    .await;
    match res {
        Ok(o) => change_team_response(o),
        Err(e) => internal_error("đổi đội hình của task", &e),
    }
}

/// Run mới nhất của task (mọi vai trò) khi nó kết thúc với outcome `quota` và không có driver
/// autopilot đang chạy; `None` trong mọi trường hợp khác.
fn quota_run(state: &TaskState) -> Option<&RunRow> {
    if state.driver_alive {
        return None;
    }
    state
        .runs
        .iter()
        .max_by_key(|r| (r.started_at, r.id))
        .filter(|r| r.outcome.as_deref() == Some(crate::quota::OUTCOME_QUOTA))
}

/// Tên nút của bước thường để `Thử lại sau` trỏ tới: autopilot đã dừng → `Chạy thêm 1 vòng`;
/// ngược lại nút đang bật của Dev (`Chạy Dev`), QA (`Chạy QA`) hoặc Lead (`Chạy lại Lead`).
/// Bước review không có nút riêng nên không có tên.
fn quota_retry_label(state: &TaskState, run: &RunRow) -> Option<String> {
    if state.view.auto.state == "stopped" {
        if let Some(c) = state
            .view
            .controls
            .iter()
            .find(|c| c.id == "auto_more_round")
        {
            return Some(c.label.clone());
        }
    }
    let id = match run.role.as_str() {
        "dev" => "run_dev",
        "qa" => "run_qa",
        "lead" => "relead",
        _ => return None,
    };
    state
        .view
        .actions
        .iter()
        .find(|a| a.id == id && a.enabled)
        .map(|a| a.label.clone())
}

/// `{run_id, role, stage, step_name, engine, detail, retry_label}` của UI-SPEC khi bước gần
/// nhất của task dừng vì hết quota (D-15, D-18); `None` khi không có quota stop.
pub fn quota_view(state: &TaskState) -> Option<serde_json::Value> {
    let run = quota_run(state)?;
    Some(serde_json::json!({
        "run_id": run.id,
        "role": run.role,
        "stage": run.stage,
        "step_name": crate::quota::step_name(run.stage.as_deref(), &run.role),
        "engine": run.engine,
        "detail": crate::quota::stop_detail(run.id, &run.role, &run.engine),
        "retry_label": quota_retry_label(state, run),
    }))
}

/// `human {mode, staff_id, staff_name, notes, step, step_name, round, branch, since_ms,
/// waiting_ms, report_paths, push {ok, command, error}}` của UI-SPEC khi team document của task
/// có marker `wait` (D-21, HUM-01); `None` khi không có. `notes` chỉ có khi nhân sự có ghi chú.
pub fn human_view(state: &TaskState, now_ms: i64) -> Option<serde_json::Value> {
    let wait = state.team.as_ref()?.wait.as_ref()?;
    let entry = state.staff.get(&wait.staff);
    let mut v = serde_json::json!({
        "mode": wait.mode,
        "staff_id": wait.staff,
        "staff_name": state.staff.display_name_of(&wait.staff),
        "step": wait.stage,
        "step_name": crate::quota::step_name(Some(wait.stage.as_str()), "dev"),
        "round": wait.round,
        "branch": wait.branch,
        "since_ms": wait.since_ms,
        "waiting_ms": (now_ms - wait.since_ms).max(0),
        "report_paths": wait.report_paths,
        "push": wait.push.as_ref().map(|p| serde_json::json!({
            "ok": p.ok,
            "command": p.command,
            "error": p.error,
        })),
    });
    if let Some(notes) = entry
        .and_then(|s| s.notes.as_deref())
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        v["notes"] = serde_json::json!(notes);
    }
    Some(v)
}

/// Xong / Trả lại (D-20): một hàm dùng chung cho hai route của dashboard (không có nút Telegram).
/// Trạng thái được đọc lại; thứ tự kiểm tra: task có → `rev` → nút có mặt (có marker) → nút bật →
/// bắt đầu job `human done|return {id}`. `action_id` là `human_done` hoặc `human_return`.
pub fn human_action(
    st: &AppState,
    task_id: i64,
    action_id: &str,
    rev: Option<&str>,
) -> BrainResult<ControlOutcome> {
    let sub = match action_id {
        "human_done" => "done",
        "human_return" => "return",
        _ => {
            return Ok(ControlOutcome::Refused {
                reason: NOT_ALLOWED_MSG.to_string(),
                kind: "not_allowed",
            })
        }
    };
    let conn = crate::db::open(&st.db_path)?;
    let state = match load_task_state(st, &conn, task_id)? {
        Ok(s) => s,
        Err(msg) => return Ok(ControlOutcome::NotFound(msg)),
    };
    if let Some(r) = rev {
        if r != state.rev {
            return Ok(ControlOutcome::Stale);
        }
    }
    let action = match state.view.actions.iter().find(|a| a.id == action_id) {
        Some(a) => a.clone(),
        None => {
            return Ok(ControlOutcome::Refused {
                reason: NOT_ALLOWED_MSG.to_string(),
                kind: "not_allowed",
            })
        }
    };
    if !action.enabled {
        let reason = action.reason.unwrap_or_default();
        let live = reason.starts_with("Đang có run") || reason.starts_with("Autopilot đang chạy");
        let kind = if live || state.busy.as_deref() == Some(reason.as_str()) {
            "busy"
        } else {
            "not_allowed"
        };
        return Ok(ControlOutcome::Refused { reason, kind });
    }
    let args = vec!["human".to_string(), sub.to_string(), task_id.to_string()];
    match start_job_outcome(st, task_job(&state, "dev", args)) {
        Ok(job_id) => Ok(ControlOutcome::Started { job_id }),
        Err(outcome) => Ok(outcome),
    }
}

async fn post_human(
    st: Arc<AppState>,
    task_id: i64,
    action_id: &'static str,
    rev: Option<String>,
) -> Response {
    let res = blocking(move || human_action(&st, task_id, action_id, rev.as_deref())).await;
    match res {
        Ok(o) => control_response(o),
        Err(e) => internal_error("xử lý nút của bước do người làm", &e),
    }
}

async fn post_human_done(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(body): Json<ControlBody>,
) -> Response {
    post_human(st, task_id, "human_done", body.rev).await
}

async fn post_human_return(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(body): Json<ControlBody>,
) -> Response {
    post_human(st, task_id, "human_return", body.rev).await
}

/// Job chạy lại bước đã dừng vì hết quota: autopilot còn bật thì `auto {id}` (quota run bị bỏ
/// khỏi dữ kiện khi resume nên cùng giai đoạn, cùng vòng chạy lại dưới đội hình mới); không
/// thì lệnh riêng của bước. `None` khi vai trò của run không có lệnh chạy lại.
fn quota_rerun_job(state: &TaskState, run: &RunRow) -> Option<(&'static str, Vec<String>)> {
    if taskstate::autopilot_continues(state.workflow.as_ref(), Some(&state.auto)) {
        return Some(("auto", vec!["auto".to_string(), state.task.id.to_string()]));
    }
    let project = state.project.id.clone();
    let run_args = |role: &'static str, mid: &[&str]| -> (&'static str, Vec<String>) {
        let mut a = vec!["run".to_string(), role.to_string()];
        a.extend(mid.iter().map(|s| s.to_string()));
        a.push("--".to_string());
        a.push(project.clone());
        (role, a)
    };
    match run.role.as_str() {
        "lead" => Some(lead_return_args(
            &state.task,
            &state.project.id,
            false,
            None,
        )),
        "plan-review" => Some(run_args("plan-review", &[])),
        "code-review" => Some(run_args("code-review", &[])),
        "qa" => Some(run_args("qa", &[])),
        "dev" => {
            if run.stage.as_deref() == Some("fix") {
                let fix_for = decide::RunFact::from_row(run)
                    .fix_for
                    .map(|f| f.cli_arg())
                    .unwrap_or("qa");
                Some(run_args("dev", &["--fix", fix_for]))
            } else {
                Some(run_args("dev", &[]))
            }
        }
        _ => None,
    }
}

/// Chạy lại bước vừa dừng vì hết quota bằng đội hình `team` (D-16, D-17, D-18, QUOTA-01).
/// Đội hình chọn trở thành đội hình của task từ bước này (lịch sử ghi theo `source`, kể cả khi
/// trùng đội hình hiện tại), rồi bước chạy lại thành một job. Không bao giờ tự đổi đội hình
/// hay engine, không phải duyệt plan. Thứ tự kiểm tra: task có → `rev` → nút `quota_rerun` bật
/// (run gần nhất là quota stop, không gì đang chạy) → đội hình có → bước có lệnh chạy lại và
/// nút thường của bước (nếu có) không từ chối → ghi đội hình bằng CAS → bắt đầu job. Job không
/// bắt đầu được sau khi đã ghi (chỉ còn cuộc đua tắt dashboard) thì đội hình vẫn đổi (đã có
/// trong lịch sử) và trả từ chối.
pub fn quota_rerun(
    st: &AppState,
    task_id: i64,
    team: &str,
    rev: Option<&str>,
    source: ActionSource,
) -> BrainResult<ControlOutcome> {
    let team = team.trim();
    let conn = crate::db::open(&st.db_path)?;
    let state = match load_task_state(st, &conn, task_id)? {
        Ok(s) => s,
        Err(msg) => return Ok(ControlOutcome::NotFound(msg)),
    };
    if let Some(r) = rev {
        if r != state.rev {
            return Ok(ControlOutcome::Stale);
        }
    }

    let refusal = |reason: String| {
        let kind = if state.busy.as_deref() == Some(reason.as_str())
            || reason.starts_with("Đang có run")
            || reason.starts_with("Autopilot đang chạy")
        {
            "busy"
        } else {
            "not_allowed"
        };
        ControlOutcome::Refused { reason, kind }
    };
    let action = match state.view.actions.iter().find(|a| a.id == "quota_rerun") {
        Some(a) => a,
        None => {
            return Ok(ControlOutcome::Refused {
                reason: NOT_ALLOWED_MSG.to_string(),
                kind: "not_allowed",
            })
        }
    };
    if !action.enabled {
        return Ok(refusal(action.reason.clone().unwrap_or_default()));
    }
    let Some(picked) = state.teams.get(team) else {
        return Ok(ControlOutcome::Invalid(unknown_team_message(&state, team)));
    };
    // ENG-10 / D-36 / D-42: refused before the team is written and before any job is queued.
    if let Some(msg) = crate::teams::team_profile_refusal(
        picked,
        &st.cfg.engines,
        &state.project.id,
        &state.project.profile,
    ) {
        return Ok(ControlOutcome::Invalid(msg));
    }

    let Some(run) = state
        .runs
        .iter()
        .max_by_key(|r| (r.started_at, r.id))
        .filter(|r| r.outcome.as_deref() == Some(crate::quota::OUTCOME_QUOTA))
    else {
        return Ok(refusal(taskstate::QUOTA_RERUN_NOT_QUOTA.to_string()));
    };
    let Some((role, args)) = quota_rerun_job(&state, run) else {
        return Ok(ControlOutcome::Refused {
            reason: NOT_ALLOWED_MSG.to_string(),
            kind: "not_allowed",
        });
    };

    // The step's own button knows whether the job can start (right branch, tree, plan); a refusal
    // there must come before the team is written, or the team would change for nothing.
    let own_id = if role == "auto" {
        "auto_more_round"
    } else {
        match run.role.as_str() {
            "dev" => "run_dev",
            "qa" => "run_qa",
            "lead" => "relead",
            _ => "",
        }
    };
    let own = state
        .view
        .actions
        .iter()
        .chain(state.view.controls.iter())
        .find(|a| a.id == own_id);
    if let Some(a) = own {
        if !a.enabled {
            return Ok(refusal(a.reason.clone().unwrap_or_default()));
        }
    }

    if write_team_change(&conn, &state, team, source)?.is_none() {
        return Ok(ControlOutcome::Stale);
    }
    match start_job_outcome(st, task_job(&state, role, args)) {
        Ok(job_id) => Ok(ControlOutcome::Started { job_id }),
        Err(outcome) => Ok(outcome),
    }
}

async fn post_quota_rerun(
    State(st): State<Arc<AppState>>,
    AxumPath(task_id): AxumPath<i64>,
    Json(body): Json<TeamBody>,
) -> Response {
    let res = blocking(move || {
        quota_rerun(
            &st,
            task_id,
            &body.team,
            body.rev.as_deref(),
            ActionSource::Dashboard,
        )
    })
    .await;
    match res {
        Ok(o) => control_response(o),
        Err(e) => internal_error("chạy lại bước hết quota", &e),
    }
}

/// `{name, summary, description, missing, legacy}` của UI-SPEC: task cũ (NULL) = `mac-dinh`,
/// mọi vai trò chạy bằng claude; tên không còn trong teams.json = `missing`.
pub fn team_view(state: &TaskState) -> serde_json::Value {
    match state.team.as_ref() {
        None if state.team_json.is_none() => serde_json::json!({
            "name": crate::teams::DEFAULT_TEAM,
            "summary": "tất cả: claude",
            "description": serde_json::Value::Null,
            "missing": false,
            "legacy": true,
        }),
        None => serde_json::json!({
            "name": crate::teams::DEFAULT_TEAM,
            "summary": serde_json::Value::Null,
            "description": serde_json::Value::Null,
            "missing": true,
            "legacy": false,
        }),
        Some(t) => match state.teams.get(&t.name) {
            Some(team) => serde_json::json!({
                "name": t.name,
                "summary": team.summary(&state.staff),
                "description": team.description,
                "missing": false,
                "legacy": false,
            }),
            None => serde_json::json!({
                "name": t.name,
                "summary": serde_json::Value::Null,
                "description": serde_json::Value::Null,
                "missing": true,
                "legacy": false,
            }),
        },
    }
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
            Err(msg) => return Ok(json_error(StatusCode::NOT_FOUND, &msg)),
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

            let mut plan_json = serde_json::json!({
                "path": ".plan.md",
                "source": state.plan_source.unwrap_or("worktree"),
                "commit": state.git.plan_commit,
                "dirty": state.plan_dirty,
                "title": plan.title,
                "sections": sections,
                "changes": changes,
                "blockers": state.git.plan_blockers,
                "errors": state.git.plan_errors,
            });
            // Skill mode: shown before Duyệt plan; the key is absent for a plan without grants.
            let grants = crate::planfile::run_grants(&plan);
            if !grants.is_empty() {
                plan_json["grants"] = serde_json::json!({
                    "skills": grants.skills,
                    "commands": grants.commands,
                    "read_dirs": grants.read_dirs,
                    "write_dirs": grants.write_dirs,
                });
            }
            Some(plan_json)
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
            "team": team_view(&state),
            "quota": quota_view(&state),
            "human": human_view(&state, crate::db::now_ms()),
            "team_history": state
                .team
                .as_ref()
                .map(|t| t.history.clone())
                .unwrap_or_default(),
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
            Err(msg) => return Ok(json_error(StatusCode::NOT_FOUND, &msg)),
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

        let mut view = pipeline::derive_response(&facts, &flow);

        // D-14: a card without a run names the engine its role will use under the task's team;
        // a task with no team (or a team no longer in teams.json) runs everything on claude.
        let team = state.team.as_ref().and_then(|t| state.teams.get(&t.name));
        let planned: std::collections::BTreeMap<String, String> =
            ["lead", "plan-review", "dev", "code-review", "qa"]
                .iter()
                .map(|role| {
                    let engine = team
                        .map(|t| t.engine_for(role).to_config_string())
                        .unwrap_or_else(|| crate::engine::ENGINE_CLAUDE.to_string());
                    (role.to_string(), engine)
                })
                .collect();
        pipeline::set_planned_engines(&mut view, &planned);

        // 06-16: a person's pending step is not a run; the pipeline shows it from the marker.
        if let Some(wait) = state.team.as_ref().and_then(|t| t.wait.as_ref()) {
            pipeline::apply_human_wait(
                &mut view,
                wait,
                &state.staff.display_name_of(&wait.staff),
                now,
            );
        }

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

/// Đối số của Trả về Lead / Chạy lại Lead (05.2 D-17 / D-30): autopilot còn bật
/// thì `auto {id} --replan`, ngược lại Lead tay `run lead --name {slug} -- {dự án} {tiêu đề}`.
/// Ghi chú (đã chuẩn hóa) luôn là một phần tử argv `--note=…`, không qua shell.
pub fn lead_return_args(
    task: &TaskRow,
    project_id: &str,
    autopilot: bool,
    note: Option<&str>,
) -> (&'static str, Vec<String>) {
    if autopilot {
        let mut a = vec![
            "auto".to_string(),
            task.id.to_string(),
            "--replan".to_string(),
        ];
        if let Some(n) = note {
            a.push(format!("--note={n}"));
        }
        ("auto", a)
    } else {
        let mut a = vec![
            "run".to_string(),
            "lead".to_string(),
            "--name".to_string(),
            task.slug.clone(),
        ];
        if let Some(n) = note {
            a.push(format!("--note={n}"));
        }
        a.push("--".to_string());
        a.push(project_id.to_string());
        a.push(task.title.clone());
        ("lead", a)
    }
}

/// Hành động của task (Chạy Dev / Chạy QA / Chạy lại Lead). Trạng thái được đọc
/// lại; chỉ chạy khi hành động có mặt và đang bật. `rev` (tùy chọn) phải khớp;
/// ghi chú chỉ dùng cho `relead`. Dashboard truyền `None` cho cả hai.
pub fn run_task_action(
    st: &AppState,
    task_id: i64,
    action_id: &str,
    role: &str,
    rev: Option<&str>,
    note: Option<&str>,
) -> BrainResult<ControlOutcome> {
    let note = match normalize_note(note) {
        Ok(n) => n,
        Err(msg) => return Ok(ControlOutcome::Invalid(msg)),
    };

    let conn = crate::db::open(&st.db_path)?;
    let state = match load_task_state(st, &conn, task_id)? {
        Ok(s) => s,
        Err(msg) => return Ok(ControlOutcome::NotFound(msg)),
    };

    if let Some(r) = rev {
        if r != state.rev {
            return Ok(ControlOutcome::Stale);
        }
    }

    let task = &state.task;
    let project = &state.project;

    let action = match state.view.actions.iter().find(|a| a.id == action_id) {
        Some(a) => a.clone(),
        None => {
            return Ok(ControlOutcome::Refused {
                reason: NOT_ALLOWED_MSG.to_string(),
                kind: "not_allowed",
            });
        }
    };

    if !action.enabled {
        let reason = action.reason.unwrap_or_default();
        let is_busy = state.busy.as_ref().map_or(false, |b| b == &reason);
        let kind = if is_busy { "busy" } else { "not_allowed" };
        return Ok(ControlOutcome::Refused { reason, kind });
    }
    // Chạy lại Lead khi autopilot còn bật (quy trình `auto: true`, chưa `ended`):
    // autopilot lập lại plan từ khung mới (`auto {id} --replan`) rồi review plan
    // như thường; autopilot đã tắt thì chạy Lead tay.
    let replan_auto = action_id == "relead"
        && taskstate::autopilot_continues(state.workflow.as_ref(), Some(&state.auto));
    let (role, args): (&str, Vec<String>) = match role {
        "lead" => {
            let note = if action_id == "relead" {
                note.as_deref()
            } else {
                None
            };
            lead_return_args(task, &project.id, replan_auto, note)
        }
        "auto" => lead_return_args(task, &project.id, true, None),
        "dev" => (
            "dev",
            vec![
                "run".to_string(),
                "dev".to_string(),
                "--".to_string(),
                project.id.clone(),
            ],
        ),
        "qa" => (
            "qa",
            vec![
                "run".to_string(),
                "qa".to_string(),
                "--".to_string(),
                project.id.clone(),
            ],
        ),
        other => (other, vec![]),
    };

    match start_job_outcome(st, task_job(&state, role, args)) {
        Ok(job_id) => Ok(ControlOutcome::Started { job_id }),
        Err(outcome) => Ok(outcome),
    }
}

async fn post_task_action(
    st: Arc<AppState>,
    task_id: i64,
    action_id: &'static str,
    role: &'static str,
) -> Response {
    let res = blocking(move || run_task_action(&st, task_id, action_id, role, None, None)).await;

    match res {
        Ok(o) => control_response(o),
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

/// Bắt đầu một job; lỗi được đổi thành `Unavailable` (dashboard đang dừng, 503)
/// hoặc `Refused` loại `busy` (409).
fn start_job_outcome(
    st: &AppState,
    spec: crate::dashboard::jobs::JobSpec,
) -> Result<u64, ControlOutcome> {
    match st.jobs.start(spec) {
        Ok(job_id) => Ok(job_id),
        Err(e) => {
            if e.message.contains("Dashboard đang dừng") {
                Err(ControlOutcome::Unavailable)
            } else {
                Err(ControlOutcome::Refused {
                    reason: e.message,
                    kind: "busy",
                })
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

/// Kết quả của một nút điều khiển / hành động task, không gắn với HTTP: dashboard
/// (`control_response`) và bot Telegram cùng gọi một đường xử lý (05.3 D-02).
#[derive(Debug, Clone, PartialEq)]
pub enum ControlOutcome {
    /// Duyệt plan: ghi nhận duyệt cho commit plan; `job_id` khi autopilot chạy tiếp.
    Approved {
        approved_at: i64,
        plan_commit: String,
        job_id: Option<u64>,
    },
    /// Một job đã bắt đầu (Trả về Lead, Bật autopilot, Chạy thêm 1 vòng, Dev / QA / Lead).
    Started { job_id: u64 },
    /// Dừng mềm: đã ghi cờ dừng cho driver đang chạy.
    StopRequested,
    /// Autopilot đã tắt (`ended`); `at_gate` khi tắt ở cổng duyệt plan.
    Ended { at_gate: bool },
    /// `rev` không khớp hoặc trạng thái đã đổi trong lúc ghi.
    Stale,
    /// Từ chối với lý do và loại (`busy`, `not_allowed`, `duplicate`).
    Refused { reason: String, kind: &'static str },
    /// Dashboard đang dừng — không nhận việc mới.
    Unavailable,
    /// Task / dự án không có.
    NotFound(String),
    /// Đầu vào không hợp lệ (ghi chú quá dài).
    Invalid(String),
}

/// Ghi chú cho Lead (05.2 D-17): cắt khoảng trắng, rỗng = không có ghi chú;
/// dài hơn `MAX_NOTE_CHARS` ký tự thì từ chối.
pub fn normalize_note(note: Option<&str>) -> Result<Option<String>, String> {
    let note = note
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if let Some(n) = note.as_deref() {
        if n.chars().count() > crate::roles::lead::MAX_NOTE_CHARS {
            return Err("Ghi chú quá dài (tối đa 500 ký tự)".to_string());
        }
    }
    Ok(note)
}

/// Nút điều khiển của task (Duyệt plan, Trả về Lead, Bật / Dừng autopilot,
/// Chạy thêm 1 vòng, Dừng hẳn). Trạng thái được đọc lại từ DB và git; chỉ làm
/// khi nút có mặt và đang bật. Thứ tự kiểm tra: ghi chú → trạng thái → `rev` →
/// nút có mặt → nút bật → hành động.
pub fn run_control(
    st: &AppState,
    task_id: i64,
    control: &str,
    rev: Option<&str>,
    note: Option<&str>,
) -> BrainResult<ControlOutcome> {
    run_control_as(st, task_id, control, rev, note, ActionSource::Dashboard)
}

/// Nơi một hành động task đến từ đâu (05.4): dashboard (127.0.0.1 + token) hoặc
/// bot Telegram. Chỉ mã phía máy chủ chọn nguồn; không có trường request nào chọn nó.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionSource {
    Dashboard,
    Telegram,
}

/// Lý do từ chối duyệt plan của task do Telegram giao khi dự án không còn trong `telegram_projects`.
pub const TELEGRAM_APPROVE_REFUSED: &str =
    "Dự án không còn trong danh sách Telegram — duyệt plan trên dashboard.";

/// Như `run_control`, nhưng biết nguồn hành động: duyệt plan của task do Telegram giao
/// chỉ được từ Telegram khi dự án còn trong `telegram_projects` (D-48, kiểm lại lúc bấm); dashboard và CLI vẫn duyệt được.
pub fn run_control_as(
    st: &AppState,
    task_id: i64,
    control: &str,
    rev: Option<&str>,
    note: Option<&str>,
    source: ActionSource,
) -> BrainResult<ControlOutcome> {
    // Ghi chú (chỉ Trả về Lead): cắt khoảng trắng, rỗng = không có ghi chú.
    let note = match normalize_note(note) {
        Ok(n) => n,
        Err(msg) => return Ok(ControlOutcome::Invalid(msg)),
    };

    let conn = crate::db::open(&st.db_path)?;
    let state = match load_task_state(st, &conn, task_id)? {
        Ok(s) => s,
        Err(msg) => return Ok(ControlOutcome::NotFound(msg)),
    };

    if let Some(r) = rev {
        if r != state.rev {
            return Ok(ControlOutcome::Stale);
        }
    }

    let ctl = match state.view.controls.iter().find(|c| c.id == control) {
        Some(c) => c.clone(),
        None => {
            let (reason, kind) = absent_control(control, &state);
            return Ok(ControlOutcome::Refused { reason, kind });
        }
    };
    if !ctl.enabled {
        let reason = ctl.reason.unwrap_or_default();
        let kind = if state.busy.as_deref() == Some(reason.as_str()) {
            "busy"
        } else {
            "not_allowed"
        };
        return Ok(ControlOutcome::Refused { reason, kind });
    }

    let id_str = task_id.to_string();
    match control {
        "approve" => {
            if source == ActionSource::Telegram
                && state.origin.as_deref() == Some(crate::db::ORIGIN_TELEGRAM)
                && !state.project_telegram_ok
            {
                return Ok(ControlOutcome::Refused {
                    reason: TELEGRAM_APPROVE_REFUSED.to_string(),
                    kind: "not_allowed",
                });
            }
            let plan_commit = match claim::approve_check(&conn, &state.task, &state.root) {
                Ok(pc) => pc,
                Err(reason) => {
                    return Ok(ControlOutcome::Refused {
                        reason,
                        kind: "not_allowed",
                    })
                }
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
            // Autopilot còn bật: chạy tiếp; đã tắt (`ended`) / quy trình tay: chỉ ghi nhận.
            let job_id =
                if taskstate::autopilot_continues(state.workflow.as_ref(), Some(&state.auto)) {
                    let spec = task_job(&state, "auto", vec!["auto".to_string(), id_str]);
                    match start_job_outcome(st, spec) {
                        Ok(j) => Some(j),
                        Err(outcome) => {
                            // Không để lại duyệt nửa chừng khi autopilot không bắt đầu được;
                            // chỉ hoàn lại khi duyệt vẫn là của yêu cầu này (WR-04).
                            claim::revert_approval(&conn, task_id, &plan_commit, prev)?;
                            return Ok(outcome);
                        }
                    }
                } else {
                    None
                };
            Ok(ControlOutcome::Approved {
                approved_at,
                plan_commit,
                job_id,
            })
        }
        "return_lead" => {
            // D-17 / D-30: lập lại plan (ghi chú là một phần tử argv); bỏ duyệt
            // chỉ sau khi job đã bắt đầu — job bị từ chối thì giữ nguyên duyệt (WR-04).
            let (role, args) = lead_return_args(
                &state.task,
                &state.project.id,
                taskstate::autopilot_continues(state.workflow.as_ref(), Some(&state.auto)),
                note.as_deref(),
            );
            match start_job_outcome(st, task_job(&state, role, args)) {
                Ok(job_id) => {
                    crate::db::clear_task_approval(&conn, task_id)?;
                    Ok(ControlOutcome::Started { job_id })
                }
                Err(outcome) => Ok(outcome),
            }
        }
        "auto_start" | "auto_more_round" => {
            let mut args = vec!["auto".to_string(), id_str];
            if control == "auto_more_round" {
                args.push("--more-round".to_string());
            }
            match start_job_outcome(st, task_job(&state, "auto", args)) {
                Ok(job_id) => Ok(ControlOutcome::Started { job_id }),
                Err(outcome) => Ok(outcome),
            }
        }
        "auto_stop" => {
            if state.driver_alive {
                // Dừng mềm: run đang chạy chạy xong, autopilot không bắt đầu bước mới.
                crate::db::request_auto_stop(&conn, task_id, "soft")?;
                return Ok(ControlOutcome::StopRequested);
            }
            // Autopilot chờ ở cổng duyệt plan (không có driver): tắt autopilot cho
            // task (`ended`). Không bỏ qua cổng — vẫn phải bấm Duyệt plan (D-03);
            // Bật autopilot bật lại. Chỉ ghi khi task vẫn `gate` (WR-04).
            if !claim::end_autopilot(&conn, task_id, "gate", &state.aj)? {
                return Ok(ControlOutcome::Stale);
            }
            Ok(ControlOutcome::Ended { at_gate: true })
        }
        "auto_end" => {
            // Dừng hẳn: giữ auto_json.stop làm lịch sử; Bật autopilot chạy lại sau.
            // Chỉ ghi khi task vẫn `stopped` và không có driver (WR-04).
            if !claim::end_autopilot(&conn, task_id, "stopped", &state.aj)? {
                return Ok(ControlOutcome::Stale);
            }
            Ok(ControlOutcome::Ended { at_gate: false })
        }
        _ => Ok(ControlOutcome::Refused {
            reason: NOT_ALLOWED_MSG.to_string(),
            kind: "not_allowed",
        }),
    }
}

/// Đổi `ControlOutcome` thành phản hồi JSON của dashboard (mã và thân như trước 05.3).
fn control_response(o: ControlOutcome) -> Response {
    match o {
        ControlOutcome::Approved {
            approved_at,
            plan_commit,
            job_id,
        } => json_resp(
            StatusCode::OK,
            serde_json::json!({
                "approved_at": approved_at,
                "plan_commit": plan_commit,
                "job_id": job_id,
            }),
        ),
        ControlOutcome::Started { job_id } => json_resp(
            StatusCode::ACCEPTED,
            serde_json::json!({ "job_id": job_id }),
        ),
        ControlOutcome::StopRequested => {
            json_resp(StatusCode::OK, serde_json::json!({ "ok": true }))
        }
        ControlOutcome::Ended { at_gate: true } => json_resp(
            StatusCode::OK,
            serde_json::json!({ "ok": true, "ended": true }),
        ),
        ControlOutcome::Ended { at_gate: false } => {
            json_resp(StatusCode::OK, serde_json::json!({ "ok": true }))
        }
        ControlOutcome::Stale => conflict(STALE_MSG, "stale"),
        ControlOutcome::Refused { reason, kind } => conflict(&reason, kind),
        ControlOutcome::Unavailable => json_resp(
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({ "error": "Dashboard đang dừng — không nhận việc mới" }),
        ),
        ControlOutcome::NotFound(msg) => json_error(StatusCode::NOT_FOUND, &msg),
        ControlOutcome::Invalid(msg) => json_error(StatusCode::UNPROCESSABLE_ENTITY, &msg),
    }
}

/// Route của các nút điều khiển: chạy `run_control` ngoài luồng async rồi đổi
/// kết quả thành JSON.
async fn post_control(
    st: Arc<AppState>,
    task_id: i64,
    control: &'static str,
    rev: Option<String>,
    note: Option<String>,
) -> Response {
    let res =
        blocking(move || run_control(&st, task_id, control, rev.as_deref(), note.as_deref())).await;

    match res {
        Ok(o) => control_response(o),
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
