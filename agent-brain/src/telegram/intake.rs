//! Task intake from Telegram (Phase 05.4): the paired user's `/task` flow.
//!
//! The paired user's typed request is trusted input (authz runs before anything
//! here). Work is created only through the dashboard's own `start_new_task`
//! (`Origin::Telegram`), after the user chose an allowlisted project and a
//! workflow and confirmed; this file holds the only call site. Flow state is in
//! memory: a restart of serve loses it and old buttons answer the stale text.
//!
//! One interaction is open at a time (OD-5): a new `/task` replaces the open
//! flow, opening a flow closes an open `Trả về Lead` note prompt, and a flow
//! expires `INTAKE_SECS` after the last interaction. The terminal audit lines
//! carry counts, project, branch and job id, never the request text.

use super::bot::{self, PollState, HELP_TEXT, INVALID_BUTTON};
use super::callback::{decode_intake, encode_intake, IntakeCb, IntakeOp};
use super::compose;
use super::render::{self, Button};
use super::BotCtx;
use crate::config::Config;
use crate::dashboard::api_tasks::{start_new_task, Origin, StartInfo, StartOutcome, StartRequest};
use crate::projects::{load_map, projects_of, require_runnable, workflow_of, Project};
use serde_json::Value;
use std::sync::Arc;

/// How long a flow stays open after its last interaction (OD-5).
pub const INTAKE_SECS: i64 = 600;
/// Most projects offered on the project screen.
pub const MAX_OFFERS: usize = 30;
/// Most workflows offered on the workflow screen (besides the default).
pub const MAX_WORKFLOWS: usize = 20;

pub const ASK_REQUEST: &str =
    "Gửi yêu cầu cho task mới (một tin nhắn), hoặc bấm Huỷ. Hết hạn sau 10 phút.";
pub const CHOOSE_PROJECT: &str = "Chọn dự án cho task:";
pub const CHOOSE_WORKFLOW: &str = "Chọn quy trình";
pub const CONFIRM_TITLE: &str = "Xác nhận giao việc";
pub const CONFIRM_FOOTER: &str = "Plan của task giao từ Telegram chỉ duyệt trên dashboard.";
pub const ACCEPTED: &str = "Đã nhận — đang lập plan";
pub const CANCELLED: &str = "Đã huỷ — không tạo task.";
pub const REPLACED: &str = "Đã huỷ — có yêu cầu mới.";
pub const EXPIRED: &str = "Hết thời gian (10 phút) — không tạo task.";
pub const STALE_FLOW: &str = "Yêu cầu đã hết hạn — gõ /task để bắt đầu lại";
pub const NO_FLOW: &str = "Không có yêu cầu nào đang mở.";
pub const NO_ALLOWLIST: &str = "Chưa có telegram_projects trong projects_map.json — thêm id dự án được phép để giao việc từ Telegram.";
pub const NONE_RUNNABLE: &str = "Không có dự án nào trong telegram_projects chạy được — kiểm tra id và đường dẫn trong projects_map.json.";
pub const REFUSED_PREFIX: &str = "Không giao được: ";

const INTERNAL: &str = "Lỗi nội bộ — xem terminal đang chạy agent-brain serve";
const UNAVAILABLE: &str = "Dashboard đang dừng — không nhận việc mới";
const DEFAULT_SLUG: &str = "viec-tu-telegram";
/// Longest label of a project button, and longest quoted request line.
const LABEL_MAX: usize = 40;
const TITLE_MAX: usize = 120;
/// Slug suffixes tried when a branch already exists: `-2` … `-5`.
const LAST_SUFFIX: usize = 5;

#[derive(Debug, Clone, PartialEq)]
pub enum Stage {
    AwaitRequest,
    Project,
    Workflow,
    Confirm,
}

/// Where the request text came from. Plan 05.4-07 adds collected forwards.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Typed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    pub id: String,
    pub label: String,
}

/// The open `/task` flow.
#[derive(Debug, Clone)]
pub struct Intake {
    /// 8-hex id in every button of the flow.
    pub id: String,
    pub stage: Stage,
    pub source: Source,
    /// Projects offered, in `telegram_projects` order; buttons carry an index.
    pub offers: Vec<Offer>,
    pub project: Option<usize>,
    pub default_workflow: String,
    pub workflows: Vec<String>,
    /// Index into `workflows`; `None` is the project's default.
    pub workflow: Option<usize>,
    pub slug: Option<String>,
    /// The flow's message (edited through every stage).
    pub message_id: Option<i64>,
    pub expires_at_ms: i64,
}

impl Intake {
    pub fn expired(&self, now_ms: i64) -> bool {
        now_ms >= self.expires_at_ms
    }

    fn request(&self) -> &str {
        match &self.source {
            Source::Typed(text) => text,
        }
    }

    fn project_id(&self) -> Option<&str> {
        self.project
            .and_then(|i| self.offers.get(i))
            .map(|o| o.id.as_str())
    }

    fn workflow_name(&self) -> Option<&str> {
        self.workflow
            .and_then(|i| self.workflows.get(i))
            .map(String::as_str)
    }
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// `(name, rest)` of a command message. Only the first token is read: it must
/// start with `/`, may end in `@bot`, and the name follows the BotCommand rule
/// (1-32 of `a-z`, `0-9`, `_`). `rest` is the remaining text, trimmed.
pub fn parse_command(text: &str) -> Option<(String, String)> {
    let text = text.trim_start();
    let end = text.find(char::is_whitespace).unwrap_or(text.len());
    let (token, rest) = text.split_at(end);
    let body = token.strip_prefix('/')?;
    let (name, bot) = match body.split_once('@') {
        Some((n, b)) => (n, Some(b)),
        None => (body, None),
    };
    if let Some(b) = bot {
        if b.is_empty() || !b.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
            return None;
        }
    }
    let valid = !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'_'));
    valid.then(|| (name.to_string(), rest.trim().to_string()))
}

/// A forwarded message carries an object `forward_origin`; it is data, never a command.
pub fn is_forwarded(msg: &Value) -> bool {
    msg.get("forward_origin").is_some_and(Value::is_object)
}

/// The ids of `telegram_projects` (string entries, in order, without repeats).
fn allowlist_ids(map: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(arr) = map.get("telegram_projects").and_then(Value::as_array) {
        for id in arr.iter().filter_map(Value::as_str) {
            if !out.iter().any(|o| o == id) {
                out.push(id.to_string());
            }
        }
    }
    out
}

/// Ids of `telegram_projects` that are not in the map's `projects`.
fn unknown_ids(map: &Value) -> Vec<String> {
    let known = projects_of(map);
    allowlist_ids(map)
        .into_iter()
        .filter(|id| !known.iter().any(|p| &p.id == id))
        .collect()
}

/// OD-1: projects Telegram may offer — listed in `telegram_projects`, present,
/// not `media`, not missing and runnable — in list order, at most `MAX_OFFERS`,
/// plus the listed ids the map does not know. Fails closed: no list, no flow.
pub fn allowed_projects(map: &Value, cfg: &Config) -> Result<(Vec<Project>, Vec<String>), String> {
    let ids = allowlist_ids(map);
    if ids.is_empty() {
        return Err(NO_ALLOWLIST.to_string());
    }
    let known = projects_of(map);
    let mut offers: Vec<Project> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    for id in &ids {
        match known.iter().find(|p| &p.id == id) {
            None => unknown.push(id.clone()),
            Some(p) => {
                if offers.len() < MAX_OFFERS
                    && p.kind != "media"
                    && !p.missing
                    && require_runnable(p, cfg).is_ok()
                {
                    offers.push(p.clone());
                }
            }
        }
    }
    if offers.is_empty() {
        return Err(NONE_RUNNABLE.to_string());
    }
    Ok((offers, unknown))
}

/// Branch-name candidates for a request (OD-8): the slug of its first line,
/// then `-2` … `-5`, every one within `SLUG_MAX_LEN`.
pub fn slug_candidates(request: &str) -> Vec<String> {
    let line = first_line(request);
    let mut base = crate::routing::slugify(line);
    if base.is_empty() {
        base = DEFAULT_SLUG.to_string();
    }
    let mut out = vec![base.clone()];
    for n in 2..=LAST_SUFFIX {
        let suffix = format!("-{n}");
        let room = crate::routing::SLUG_MAX_LEN.saturating_sub(suffix.len());
        let cut: String = base.chars().take(room).collect();
        out.push(format!("{}{suffix}", cut.trim_end_matches('-')));
    }
    out
}

/// Takes the open flow off `state` when it has expired at `now_ms`.
pub fn take_expired(state: &mut PollState, now_ms: i64) -> Option<Intake> {
    if state.intake.as_ref().is_some_and(|f| f.expired(now_ms)) {
        state.intake.take()
    } else {
        None
    }
}

/// First non-empty line of a request, trimmed.
fn first_line(request: &str) -> &str {
    request.trim().lines().next().unwrap_or("").trim()
}

/// `text` cut to `max` characters, ending with `…` when cut. One line, so it is
/// safe on a button label.
fn short(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn deadline(now_ms: i64) -> i64 {
    now_ms.saturating_add(INTAKE_SECS * 1000)
}

fn button(label: &str, op: IntakeOp, flow: &str, index: Option<usize>) -> Button {
    Button {
        label: label.to_string(),
        data: encode_intake(&IntakeCb {
            op,
            flow: flow.to_string(),
            index,
        }),
    }
}

fn cancel_row(flow: &str) -> Vec<Button> {
    vec![button("Huỷ", IntakeOp::Cancel, flow, None)]
}

fn project_screen(f: &Intake) -> (String, Value) {
    let text = format!(
        "{CHOOSE_PROJECT}\n«{}»",
        short(first_line(f.request()), TITLE_MAX)
    );
    let mut rows: Vec<Vec<Button>> = f
        .offers
        .iter()
        .enumerate()
        .map(|(i, o)| vec![button(&o.label, IntakeOp::Project, &f.id, Some(i))])
        .collect();
    rows.push(cancel_row(&f.id));
    (text, render::keyboard_json(&rows))
}

fn workflow_screen(f: &Intake, project_id: &str) -> (String, Value) {
    let text = format!("{CHOOSE_WORKFLOW} · dự án {project_id}");
    let mut rows = vec![vec![button(
        &format!("Mặc định ({})", f.default_workflow),
        IntakeOp::Workflow,
        &f.id,
        Some(0),
    )]];
    for (i, name) in f.workflows.iter().enumerate() {
        rows.push(vec![button(name, IntakeOp::Workflow, &f.id, Some(i + 1))]);
    }
    rows.push(cancel_row(&f.id));
    (text, render::keyboard_json(&rows))
}

fn confirm_screen(f: &Intake, info: &StartInfo) -> (String, Value) {
    let mut lines = vec![
        CONFIRM_TITLE.to_string(),
        format!(
            "Dự án: {} · profile {}",
            info.project.id, info.project.profile
        ),
        format!(
            "Quy trình: {} ({})",
            info.workflow,
            if info.auto { "tự động" } else { "tay" }
        ),
        format!("Nhánh: {}", info.branch),
        format!("Yêu cầu: {}", short(first_line(f.request()), TITLE_MAX)),
    ];
    if let Some(w) = &info.warning {
        lines.push(format!("Lưu ý: {w}"));
    }
    lines.push(CONFIRM_FOOTER.to_string());
    let keyboard = render::keyboard_json(&[
        vec![button("Tạo task", IntakeOp::Confirm, &f.id, None)],
        cancel_row(&f.id),
    ]);
    (lines.join("\n"), keyboard)
}

/// What the user is told for a creation outcome that did not start a job, and
/// a short kind for the terminal audit line.
fn refusal_of(outcome: StartOutcome) -> (String, String) {
    match outcome {
        StartOutcome::ProjectRefused { body, .. } => (
            body.get("error")
                .and_then(Value::as_str)
                .unwrap_or("Dự án không dùng được")
                .to_string(),
            "dự án".to_string(),
        ),
        StartOutcome::PreflightRefused { reason, kind } => (reason, kind),
        StartOutcome::Busy(text) => (text, "busy".to_string()),
        StartOutcome::Invalid(text) => (text, "invalid".to_string()),
        StartOutcome::Unavailable => (UNAVAILABLE.to_string(), "unavailable".to_string()),
        StartOutcome::Ready(_) | StartOutcome::Started { .. } => {
            (INTERNAL.to_string(), "internal".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Creation path (the only call site of `start_new_task`)
// ---------------------------------------------------------------------------

/// Dry runs over the slug candidates (OD-8): the first one whose branch and task
/// row are free. A taken name moves on; every other refusal ends the search.
fn find_slug(ctx: &BotCtx, f: &Intake, project_id: &str) -> Result<(String, StartInfo), String> {
    let candidates = slug_candidates(f.request());
    for candidate in &candidates {
        let req = StartRequest {
            project: project_id,
            request: f.request(),
            name: Some(candidate),
            workflow: f.workflow_name(),
            origin: Origin::Telegram,
            dry_run: true,
        };
        match start_new_task(&ctx.st, &req) {
            Ok(StartOutcome::Ready(info)) => return Ok((info.slug.clone(), info)),
            Ok(StartOutcome::PreflightRefused { kind, .. })
                if kind == "branch_exists" || kind == "exists" => {}
            Ok(other) => return Err(refusal_of(other).0),
            Err(e) => return Err(e.message),
        }
    }
    let base = &candidates[0];
    Err(format!(
        "Đã có nhánh task/{base} … task/{base}-{LAST_SUFFIX} — gõ /task với yêu cầu khác"
    ))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn audit(text: &str) {
    crate::term::emit_out(&format!("Telegram: giao việc — {text}"));
}

/// A command from the paired chat (already parsed; forwards never get here).
pub(crate) fn on_command(ctx: &Arc<BotCtx>, state: &mut PollState, name: &str, rest: &str) {
    match name {
        "task" => start_flow(ctx, state, rest),
        "huy" => {
            if state.intake.is_some() {
                close(ctx, state, CANCELLED);
            } else {
                bot::reply(ctx, NO_FLOW);
            }
        }
        _ => bot::reply(ctx, HELP_TEXT),
    }
}

/// Opens a typed flow: `/task <request>` or bare `/task`.
fn start_flow(ctx: &Arc<BotCtx>, state: &mut PollState, rest: &str) {
    let map = match load_map(&ctx.st.map_path) {
        Ok(m) => m,
        Err(e) => {
            close(ctx, state, REPLACED);
            bot::reply(ctx, &format!("{REFUSED_PREFIX}{}", e.message));
            return;
        }
    };
    // An id the map does not know is warned once per serve (INTAKE-03).
    let fresh: Vec<String> = unknown_ids(&map)
        .into_iter()
        .filter(|id| !state.warned_ids.contains(id))
        .collect();
    if !fresh.is_empty() {
        crate::term::emit(&format!(
            "Telegram: telegram_projects có id không có trong projects_map.json: {}",
            fresh.join(", ")
        ));
        state.warned_ids.extend(fresh);
    }
    let projects = match allowed_projects(&map, &ctx.st.cfg) {
        Ok((projects, _)) => projects,
        Err(msg) => {
            close(ctx, state, REPLACED);
            bot::reply(ctx, &msg);
            return;
        }
    };
    let flow_id = match compose::new_hex_id() {
        Ok(id) => id,
        Err(e) => {
            crate::term::emit(&format!("Telegram: lỗi tạo mã yêu cầu ({e})"));
            bot::reply(ctx, INTERNAL);
            return;
        }
    };

    // One interaction at a time (OD-5): the open note prompt and flow end here.
    if let Some(p) = state.pending.take() {
        bot::edit(ctx, p.prompt_message_id, REPLACED, render::no_keyboard());
    }
    close(ctx, state, REPLACED);

    let request = rest.trim().to_string();
    let offers: Vec<Offer> = projects
        .iter()
        .map(|p| {
            let name = if p.name_vi.trim().is_empty() {
                &p.id
            } else {
                &p.name_vi
            };
            Offer {
                id: p.id.clone(),
                label: short(&format!("{name} · {}", p.profile), LABEL_MAX),
            }
        })
        .collect();
    let awaiting = request.is_empty();
    let mut flow = Intake {
        id: flow_id,
        stage: if awaiting {
            Stage::AwaitRequest
        } else {
            Stage::Project
        },
        source: Source::Typed(request.clone()),
        offers,
        project: None,
        default_workflow: String::new(),
        workflows: Vec::new(),
        workflow: None,
        slug: None,
        message_id: None,
        expires_at_ms: deadline(crate::db::now_ms()),
    };
    let (text, keyboard) = if awaiting {
        (
            ASK_REQUEST.to_string(),
            render::keyboard_json(&[cancel_row(&flow.id)]),
        )
    } else {
        project_screen(&flow)
    };
    if let Some(message_id) = bot::send_message(ctx, &text, keyboard) {
        flow.message_id = Some(message_id);
        state.intake = Some(flow);
        audit(&format!("mở yêu cầu ({} ký tự)", request.chars().count()));
    }
}

/// The request of a flow waiting for it. `true` when `text` was taken.
pub(crate) fn on_text(ctx: &Arc<BotCtx>, state: &mut PollState, text: &str) -> bool {
    let now = crate::db::now_ms();
    let waiting = state
        .intake
        .as_ref()
        .is_some_and(|f| f.stage == Stage::AwaitRequest && !f.expired(now));
    let request = text.trim();
    if !waiting || request.is_empty() {
        return false;
    }
    if let Some(f) = state.intake.as_mut() {
        f.source = Source::Typed(request.to_string());
        f.stage = Stage::Project;
        f.expires_at_ms = deadline(now);
        if let Some(message_id) = f.message_id {
            let (text, keyboard) = project_screen(f);
            bot::edit(ctx, message_id, &text, keyboard);
        }
        audit(&format!("mở yêu cầu ({} ký tự)", request.chars().count()));
    }
    true
}

/// What a press did to the flow.
enum Next {
    /// Moved on: the flow stays open and its clock restarts.
    Advanced,
    /// Nothing changed (invalid press): the flow stays open as it was.
    Kept,
    /// Over: cancelled, refused or created.
    Closed,
}

/// An `i…` button press of the paired chat.
pub(crate) fn on_callback(
    ctx: &Arc<BotCtx>,
    state: &mut PollState,
    cq_id: &str,
    message_id: Option<i64>,
    data: &str,
) {
    let (cb, message_id) = match (decode_intake(data), message_id) {
        (Some(cb), Some(m)) => (cb, m),
        _ => {
            bot::answer(ctx, cq_id, INVALID_BUTTON, false);
            return;
        }
    };
    let now = crate::db::now_ms();
    let current = state
        .intake
        .as_ref()
        .is_some_and(|f| f.id == cb.flow && f.message_id == Some(message_id) && !f.expired(now));
    if !current {
        // Closed, replaced, expired or never known: the message loses its buttons.
        bot::answer(ctx, cq_id, STALE_FLOW, false);
        bot::edit(ctx, message_id, "", render::no_keyboard());
        return;
    }
    let mut flow = match state.intake.take() {
        Some(f) => f,
        None => return,
    };
    match press(ctx, &mut flow, cq_id, message_id, &cb) {
        Next::Advanced => {
            flow.expires_at_ms = deadline(crate::db::now_ms());
            state.intake = Some(flow);
        }
        Next::Kept => state.intake = Some(flow),
        Next::Closed => {}
    }
}

fn press(ctx: &Arc<BotCtx>, f: &mut Intake, cq_id: &str, mid: i64, cb: &IntakeCb) -> Next {
    match (cb.op, f.stage.clone()) {
        (IntakeOp::Cancel, _) => {
            bot::edit(ctx, mid, CANCELLED, render::no_keyboard());
            bot::answer(ctx, cq_id, "Đã huỷ", false);
            audit("đã huỷ");
            Next::Closed
        }
        (IntakeOp::Project, Stage::Project) => pick_project(ctx, f, cq_id, mid, cb.index),
        (IntakeOp::Workflow, Stage::Workflow) => pick_workflow(ctx, f, cq_id, mid, cb.index),
        (IntakeOp::Confirm, Stage::Confirm) => confirm(ctx, f, cq_id, mid),
        _ => {
            bot::answer(ctx, cq_id, INVALID_BUTTON, false);
            Next::Kept
        }
    }
}

/// Ends the flow with a refusal shown on its message.
fn refuse(ctx: &BotCtx, cq_id: &str, mid: i64, reason: &str, kind: &str) -> Next {
    bot::edit(
        ctx,
        mid,
        &format!("{REFUSED_PREFIX}{reason}"),
        render::no_keyboard(),
    );
    bot::answer(ctx, cq_id, reason, true);
    audit(&format!("từ chối ({kind})"));
    Next::Closed
}

fn pick_project(
    ctx: &Arc<BotCtx>,
    f: &mut Intake,
    cq_id: &str,
    mid: i64,
    index: Option<usize>,
) -> Next {
    let i = match index.filter(|i| *i < f.offers.len()) {
        Some(i) => i,
        None => {
            bot::answer(ctx, cq_id, INVALID_BUTTON, false);
            return Next::Kept;
        }
    };
    let project_id = f.offers[i].id.clone();
    let map = match load_map(&ctx.st.map_path) {
        Ok(m) => m,
        Err(e) => return refuse(ctx, cq_id, mid, &e.message, "map"),
    };
    let loaded = crate::workflows::load(&ctx.st.cfg);
    let default =
        match crate::workflows::resolve(&loaded, None, workflow_of(&map, &project_id).as_deref()) {
            Ok(r) => r.workflow.name,
            Err(e) => return refuse(ctx, cq_id, mid, &e.message, "quy trình"),
        };
    f.project = Some(i);
    f.default_workflow = default;
    f.workflows = loaded.names().into_iter().take(MAX_WORKFLOWS).collect();
    f.workflow = None;
    f.stage = Stage::Workflow;
    let (text, keyboard) = workflow_screen(f, &project_id);
    bot::edit(ctx, mid, &text, keyboard);
    bot::answer(ctx, cq_id, "Đã chọn dự án", false);
    Next::Advanced
}

fn pick_workflow(
    ctx: &Arc<BotCtx>,
    f: &mut Intake,
    cq_id: &str,
    mid: i64,
    index: Option<usize>,
) -> Next {
    let i = match index.filter(|i| *i <= f.workflows.len()) {
        Some(i) => i,
        None => {
            bot::answer(ctx, cq_id, INVALID_BUTTON, false);
            return Next::Kept;
        }
    };
    let project_id = match f.project_id() {
        Some(id) => id.to_string(),
        None => {
            bot::answer(ctx, cq_id, INVALID_BUTTON, false);
            return Next::Kept;
        }
    };
    f.workflow = i.checked_sub(1);
    match find_slug(ctx, f, &project_id) {
        Ok((slug, info)) => {
            f.slug = Some(slug);
            f.stage = Stage::Confirm;
            let (text, keyboard) = confirm_screen(f, &info);
            bot::edit(ctx, mid, &text, keyboard);
            bot::answer(ctx, cq_id, "Đã chọn quy trình", false);
            Next::Advanced
        }
        Err(reason) => refuse(ctx, cq_id, mid, &reason, "preflight"),
    }
}

/// `Tạo task`: the allowlist is re-read, a last dry run checks the stored slug,
/// then exactly one job starts through `start_new_task`.
fn confirm(ctx: &Arc<BotCtx>, f: &mut Intake, cq_id: &str, mid: i64) -> Next {
    let (project_id, slug) = match (f.project_id(), f.slug.clone()) {
        (Some(p), Some(s)) => (p.to_string(), s),
        _ => {
            bot::answer(ctx, cq_id, INVALID_BUTTON, false);
            return Next::Kept;
        }
    };
    let map = match load_map(&ctx.st.map_path) {
        Ok(m) => m,
        Err(e) => return refuse(ctx, cq_id, mid, &e.message, "map"),
    };
    let still_allowed = allowed_projects(&map, &ctx.st.cfg)
        .map(|(projects, _)| projects.iter().any(|p| p.id == project_id))
        .unwrap_or(false);
    if !still_allowed {
        return refuse(
            ctx,
            cq_id,
            mid,
            &format!("{project_id} không còn trong telegram_projects"),
            "allowlist",
        );
    }

    let request = f.request().to_string();
    let mut req = StartRequest {
        project: &project_id,
        request: &request,
        name: Some(&slug),
        workflow: f.workflow_name(),
        origin: Origin::Telegram,
        dry_run: true,
    };
    match start_new_task(&ctx.st, &req) {
        Ok(StartOutcome::Ready(_)) => {}
        Ok(other) => {
            let (reason, kind) = refusal_of(other);
            return refuse(ctx, cq_id, mid, &reason, &kind);
        }
        Err(e) => return refuse(ctx, cq_id, mid, &e.message, "nội bộ"),
    }
    req.dry_run = false;
    match start_new_task(&ctx.st, &req) {
        Ok(StartOutcome::Started { job_id, info }) => {
            bot::edit(
                ctx,
                mid,
                &format!(
                    "{ACCEPTED} · dự án {} · nhánh {}",
                    info.project.id, info.branch
                ),
                render::no_keyboard(),
            );
            bot::answer(ctx, cq_id, "Đã giao việc", false);
            audit(&format!(
                "đã giao job {job_id}: dự án {}, nhánh {}",
                info.project.id, info.branch
            ));
            Next::Closed
        }
        Ok(other) => {
            let (reason, kind) = refusal_of(other);
            refuse(ctx, cq_id, mid, &reason, &kind)
        }
        Err(e) => refuse(ctx, cq_id, mid, &e.message, "nội bộ"),
    }
}

/// A forwarded message: data, never a command. Collection arrives in plan 05.4-07;
/// until then the help line is the only reply.
pub(crate) fn on_forward(ctx: &Arc<BotCtx>, _state: &mut PollState, _msg: &Value) {
    bot::reply(ctx, HELP_TEXT);
}

/// Ends an expired flow (called after each `getUpdates` and before each update).
pub(crate) fn tick(ctx: &Arc<BotCtx>, state: &mut PollState, now_ms: i64) {
    if let Some(f) = take_expired(state, now_ms) {
        if let Some(message_id) = f.message_id {
            bot::edit(ctx, message_id, EXPIRED, render::no_keyboard());
        }
        audit("hết hạn");
    }
}

/// Closes the open flow, if any, showing `text` on its message without buttons.
pub(crate) fn close(ctx: &Arc<BotCtx>, state: &mut PollState, text: &str) {
    if let Some(f) = state.intake.take() {
        if let Some(message_id) = f.message_id {
            bot::edit(ctx, message_id, text, render::no_keyboard());
        }
    }
}
