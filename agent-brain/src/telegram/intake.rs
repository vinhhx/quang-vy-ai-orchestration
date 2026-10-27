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
//!
//! After `Tạo task` the started job is watched from the poll thread (`tick`, no
//! extra thread): the user is told the task id once its row exists, or the job's
//! last output lines when it ends before any row exists (INTAKE-10).
//!
//! Forwarded messages (INTAKE-05/06) open a flow too: they are collected in a
//! short window (`Collecting`, closed by `tick`), shown as one ☑/☐ list
//! (`Select`), then go through the same project, workflow and confirm steps. The
//! request is composed at `Tạo task` by `compose::compose_forwarded`, which puts
//! the forwarded text in a nonce-delimited untrusted block; a text typed on the
//! confirm screen is the optional instruction, outside the block.

use super::bot::{self, PollState, HELP_TEXT, INVALID_BUTTON};
use super::callback::{decode_intake, encode_intake, IntakeCb, IntakeOp};
use super::compose;
use super::forward::{self, ForwardItem};
use super::render::{self, Button};
use super::transport::LONG_POLL_SECS;
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
/// Confirm-screen line of a forwarded flow: a typed text becomes the instruction.
pub const ASK_INSTRUCTION: &str =
    "Gửi một tin chữ để thêm việc cần làm (không bắt buộc), hoặc bấm Tạo task.";

/// Most started jobs watched at once; the oldest is dropped first.
pub const MAX_WATCH: usize = 20;
/// A watched job without a task row is dropped after this long (10 minutes).
pub const WATCH_MAX_MS: i64 = 600_000;
/// Sent once the task row of a started job exists (`{id}`, `{branch}`).
pub const CREATED_FMT: &str = "Đã tạo task {id} · {branch}. Plan đang được lập; khi xong sẽ có tin chờ duyệt — duyệt plan trên dashboard.";
/// First line of the report of a job that ended before its task row existed.
pub const FAILED_PREFIX: &str = "Không tạo được task ({branch}):";
const NO_ERROR_LINES: &str = "(không có thông báo lỗi)";
/// Refusal when the daily cap is reached (`{N}` = the cap).
pub const CAP_REACHED_FMT: &str =
    "Đã đủ {N} task giao từ Telegram hôm nay — giao tiếp trên dashboard hoặc chờ sang ngày mai.";
/// Refusal when the daily cap is 0.
pub const CAP_OFF: &str =
    "Giao việc từ Telegram đang tắt (telegram_max_tasks_per_day = 0 trong config.json).";
/// Output lines of a failed job sent to the chat, and the length of each.
const FAILED_LINES: usize = 3;
const FAILED_LINE_MAX: usize = 200;

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
    /// Forwarded messages are being collected; no message is shown yet.
    Collecting {
        first_ms: i64,
        last_ms: i64,
    },
    /// The ☑/☐ list of collected forwarded messages is shown.
    Select,
    Project,
    Workflow,
    Confirm,
}

/// Where the request text came from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Typed(String),
    /// Forwarded messages (untrusted data), composed into one request at confirm.
    Forwarded {
        items: Vec<ForwardItem>,
        /// Forwarded messages without text or caption.
        skipped: usize,
        /// `forward::OVER_CAP` was already sent for this flow.
        over_cap_told: bool,
        /// The text typed on the confirm screen, outside the untrusted block.
        instruction: Option<String>,
        /// Nonce of the block delimiters.
        nonce: String,
        /// When the first forward arrived (names the default branch).
        started_ms: i64,
    },
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

/// A job started by `Tạo task`, watched until its task row exists or it ends.
#[derive(Debug, Clone, PartialEq)]
pub struct Watch {
    pub job_id: u64,
    /// Project id (the `projects.slug` of the row).
    pub project: String,
    pub branch: String,
    pub started_ms: i64,
}

impl Intake {
    pub fn expired(&self, now_ms: i64) -> bool {
        now_ms >= self.expires_at_ms
    }

    /// The request handed to `start_new_task`: the typed text, or the selected
    /// forwarded messages composed into the untrusted block.
    fn request(&self) -> Result<String, String> {
        match &self.source {
            Source::Typed(text) => Ok(text.clone()),
            Source::Forwarded {
                items,
                instruction,
                nonce,
                ..
            } => compose::compose_forwarded(
                &forward::to_quotes(items),
                instruction.as_deref(),
                nonce,
            ),
        }
    }

    /// One line describing the request on the project screen.
    fn preview(&self) -> String {
        match &self.source {
            Source::Typed(text) => short(first_line(text), TITLE_MAX),
            Source::Forwarded { items, .. } => {
                compose::source_tag(items.iter().filter(|i| i.selected).count())
            }
        }
    }

    /// The text the branch name is made from: the typed request, the forwarded
    /// flow's instruction, else `tin chuyen tiep {YYYYMMDD} {HHMM}`.
    fn slug_source(&self) -> String {
        match &self.source {
            Source::Typed(text) => text.clone(),
            Source::Forwarded {
                instruction: Some(text),
                ..
            } => text.clone(),
            Source::Forwarded { started_ms, .. } => {
                let stamp = chrono::DateTime::from_timestamp_millis(*started_ms)
                    .map(|dt| {
                        dt.with_timezone(&chrono::Local)
                            .format("%Y%m%d %H%M")
                            .to_string()
                    })
                    .unwrap_or_default();
                format!("tin chuyen tiep {stamp}")
            }
        }
    }

    fn is_forwarded(&self) -> bool {
        matches!(self.source, Source::Forwarded { .. })
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

/// The `getUpdates` timeout: 1 second while a started job is watched (so its
/// outcome is reported within seconds) or forwarded messages are being collected
/// (so the quiet window is checked about once a second); the long poll otherwise.
pub fn poll_timeout_secs(state: &PollState) -> u64 {
    let collecting = state
        .intake
        .as_ref()
        .is_some_and(|f| matches!(f.stage, Stage::Collecting { .. }));
    if state.watch.is_empty() && !collecting {
        LONG_POLL_SECS
    } else {
        1
    }
}

/// Start of the local day of `now`, in ms (the earliest instant when ambiguous).
/// A day whose midnight falls in a DST gap starts at the first hour that exists.
pub fn local_midnight_ms(now: chrono::DateTime<chrono::Local>) -> i64 {
    use chrono::TimeZone;
    let date = now.date_naive();
    (0..=3)
        .filter_map(|h| date.and_hms_opt(h, 0, 0))
        .find_map(|t| chrono::Local.from_local_datetime(&t).earliest())
        .map(|t| t.timestamp_millis())
        .unwrap_or_else(|| now.timestamp_millis())
}

/// Why a new Telegram task is refused by the daily cap, if it is (OD-2): the
/// cap is 0, or the Telegram-origin rows created since local midnight plus the
/// watched jobs without a row yet reach it. A database error refuses too.
pub fn cap_refusal(ctx: &BotCtx, state: &PollState) -> Option<String> {
    let cap = ctx.st.cfg.telegram_max_tasks_per_day;
    if cap == 0 {
        return Some(CAP_OFF.to_string());
    }
    let counted = crate::db::open(&ctx.st.db_path).and_then(|conn| {
        let since = local_midnight_ms(chrono::Local::now());
        let rows = crate::db::count_tasks_since(&conn, crate::db::ORIGIN_TELEGRAM, since)?;
        let starting = state
            .watch
            .iter()
            .filter(|w| task_row_id(&conn, w).is_none())
            .count();
        Ok(rows.saturating_add(starting as i64))
    });
    match counted {
        Ok(n) if n >= i64::from(cap) => Some(CAP_REACHED_FMT.replace("{N}", &cap.to_string())),
        Ok(_) => None,
        Err(e) => {
            crate::term::emit(&format!(
                "Telegram: lỗi đếm task giao từ Telegram hôm nay ({})",
                e.message
            ));
            Some(INTERNAL.to_string())
        }
    }
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
    let text = format!("{CHOOSE_PROJECT}\n«{}»", f.preview());
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
    ];
    match &f.source {
        Source::Typed(text) => {
            lines.push(format!("Yêu cầu: {}", short(first_line(text), TITLE_MAX)));
        }
        Source::Forwarded {
            items, instruction, ..
        } => {
            let k = items.iter().filter(|i| i.selected).count();
            let todo = instruction
                .as_deref()
                .unwrap_or(compose::DEFAULT_INSTRUCTION);
            lines.push(format!("Tin chuyển tiếp: {k} tin đã chọn"));
            lines.push(format!(
                "Việc cần làm: {}",
                short(first_line(todo), TITLE_MAX)
            ));
        }
    }
    if let Some(w) = &info.warning {
        lines.push(format!("Lưu ý: {w}"));
    }
    if f.is_forwarded() {
        lines.push(ASK_INSTRUCTION.to_string());
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
    let candidates = slug_candidates(&f.slug_source());
    let request = f.request()?;
    for candidate in &candidates {
        let req = StartRequest {
            project: project_id,
            request: &request,
            name: Some(candidate),
            workflow: f.workflow_name(),
            team: None,
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
        "huy" => match state.intake.as_ref().map(|f| f.message_id.is_some()) {
            Some(true) => close(ctx, state, CANCELLED),
            // Forwards still being collected have no message to edit yet.
            Some(false) => {
                close(ctx, state, CANCELLED);
                bot::reply(ctx, CANCELLED);
            }
            None => bot::reply(ctx, NO_FLOW),
        },
        _ => bot::reply(ctx, HELP_TEXT),
    }
}

/// The projects a new flow may offer (OD-1), as buttons. The error is the text
/// to reply. A `telegram_projects` id the map does not know is warned once per
/// serve (INTAKE-03).
fn allowed_offers(ctx: &BotCtx, state: &mut PollState) -> Result<Vec<Offer>, String> {
    let map = load_map(&ctx.st.map_path).map_err(|e| format!("{REFUSED_PREFIX}{}", e.message))?;
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
    let (projects, _) = allowed_projects(&map, &ctx.st.cfg)?;
    Ok(projects
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
        .collect())
}

/// Opens a typed flow: `/task <request>` or bare `/task`.
fn start_flow(ctx: &Arc<BotCtx>, state: &mut PollState, rest: &str) {
    // The daily cap is checked before anything is shown (OD-2).
    if let Some(refusal) = cap_refusal(ctx, state) {
        close(ctx, state, REPLACED);
        bot::reply(ctx, &refusal);
        audit("từ chối (giới hạn ngày)");
        return;
    }
    let offers = match allowed_offers(ctx, state) {
        Ok(offers) => offers,
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

/// The request of a flow waiting for it, or the instruction of a forwarded flow
/// on its confirm screen. `true` when `text` was taken.
pub(crate) fn on_text(ctx: &Arc<BotCtx>, state: &mut PollState, text: &str) -> bool {
    let now = crate::db::now_ms();
    let instructing = state
        .intake
        .as_ref()
        .is_some_and(|f| f.is_forwarded() && f.stage == Stage::Confirm && !f.expired(now));
    if instructing {
        let instruction = text.trim();
        if !instruction.is_empty() {
            if let Some(flow) = state.intake.take() {
                state.intake = set_instruction(ctx, flow, instruction);
            }
        }
        return true;
    }
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

/// OD-7: `instruction` becomes the forwarded flow's instruction (outside the
/// untrusted block); the branch is looked up again from it and the confirm screen
/// is redrawn. Returns the flow when it stays open.
fn set_instruction(ctx: &Arc<BotCtx>, mut f: Intake, instruction: &str) -> Option<Intake> {
    let (mid, project_id) = match (f.message_id, f.project_id()) {
        (Some(m), Some(p)) => (m, p.to_string()),
        _ => return Some(f),
    };
    let old = match &mut f.source {
        Source::Forwarded { instruction: i, .. } => i.replace(instruction.to_string()),
        Source::Typed(_) => return Some(f),
    };
    // A request over the size limit keeps the old instruction (never truncated).
    if let Err(reason) = f.request() {
        if let Source::Forwarded { instruction: i, .. } = &mut f.source {
            *i = old;
        }
        bot::reply(ctx, &reason);
        return Some(f);
    }
    match find_slug(ctx, &f, &project_id) {
        Ok((slug, info)) => {
            f.slug = Some(slug);
            f.expires_at_ms = deadline(crate::db::now_ms());
            let (text, keyboard) = confirm_screen(&f, &info);
            bot::edit(ctx, mid, &text, keyboard);
            audit(&format!(
                "thêm việc cần làm ({} ký tự)",
                instruction.chars().count()
            ));
            Some(f)
        }
        Err(reason) => {
            bot::edit(
                ctx,
                mid,
                &format!("{REFUSED_PREFIX}{reason}"),
                render::no_keyboard(),
            );
            audit("từ chối (preflight)");
            None
        }
    }
}

/// What a press did to the flow.
enum Next {
    /// Moved on: the flow stays open and its clock restarts.
    Advanced,
    /// Nothing changed (invalid press): the flow stays open as it was.
    Kept,
    /// Over: cancelled or refused.
    Closed,
    /// Over: a job started and is watched until its task row exists.
    Started(Watch),
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
    match press(ctx, state, &mut flow, cq_id, message_id, &cb) {
        Next::Advanced => {
            flow.expires_at_ms = deadline(crate::db::now_ms());
            state.intake = Some(flow);
        }
        Next::Kept => state.intake = Some(flow),
        Next::Closed => {}
        Next::Started(w) => push_watch(state, w),
    }
}

/// Adds a started job to the watch list, dropping the oldest at `MAX_WATCH`.
fn push_watch(state: &mut PollState, w: Watch) {
    while state.watch.len() >= MAX_WATCH {
        let old = state.watch.remove(0);
        audit(&format!("thôi theo dõi job {} (quá nhiều job)", old.job_id));
    }
    state.watch.push(w);
}

fn press(
    ctx: &Arc<BotCtx>,
    state: &PollState,
    f: &mut Intake,
    cq_id: &str,
    mid: i64,
    cb: &IntakeCb,
) -> Next {
    match (cb.op, f.stage.clone()) {
        (IntakeOp::Cancel, _) => {
            bot::edit(ctx, mid, CANCELLED, render::no_keyboard());
            bot::answer(ctx, cq_id, "Đã huỷ", false);
            audit("đã huỷ");
            Next::Closed
        }
        (IntakeOp::Toggle, Stage::Select) => select(ctx, f, cq_id, mid, Pick::One(cb.index)),
        (IntakeOp::All, Stage::Select) => select(ctx, f, cq_id, mid, Pick::All(true)),
        (IntakeOp::None, Stage::Select) => select(ctx, f, cq_id, mid, Pick::All(false)),
        (IntakeOp::Go, Stage::Select) => go(ctx, f, cq_id, mid),
        (IntakeOp::Project, Stage::Project) => pick_project(ctx, f, cq_id, mid, cb.index),
        (IntakeOp::Workflow, Stage::Workflow) => pick_workflow(ctx, f, cq_id, mid, cb.index),
        (IntakeOp::Confirm, Stage::Confirm) => confirm(ctx, state, f, cq_id, mid),
        _ => {
            bot::answer(ctx, cq_id, INVALID_BUTTON, false);
            Next::Kept
        }
    }
}

/// A change to the ☑/☐ state of a forwarded flow.
enum Pick {
    /// Flip one message (its index).
    One(Option<usize>),
    /// Select (`true`) or clear every message.
    All(bool),
}

/// The selection message of a forwarded flow, redrawn from the server-held list.
fn show_selection(ctx: &BotCtx, f: &Intake, mid: i64) {
    if let Source::Forwarded { items, skipped, .. } = &f.source {
        bot::edit(
            ctx,
            mid,
            &forward::selection_text(items, *skipped),
            render::keyboard_json(&forward::selection_keyboard(&f.id, items)),
        );
    }
}

fn select(ctx: &BotCtx, f: &mut Intake, cq_id: &str, mid: i64, pick: Pick) -> Next {
    let items = match &mut f.source {
        Source::Forwarded { items, .. } => items,
        Source::Typed(_) => {
            bot::answer(ctx, cq_id, INVALID_BUTTON, false);
            return Next::Kept;
        }
    };
    let answer = match pick {
        Pick::One(index) => match index.and_then(|i| items.get_mut(i)) {
            Some(item) => {
                item.selected = !item.selected;
                if item.selected {
                    "Đã chọn"
                } else {
                    "Đã bỏ chọn"
                }
            }
            None => {
                bot::answer(ctx, cq_id, INVALID_BUTTON, false);
                return Next::Kept;
            }
        },
        Pick::All(on) => {
            for item in items.iter_mut() {
                item.selected = on;
            }
            if on {
                "Đã chọn hết"
            } else {
                "Đã bỏ hết"
            }
        }
    };
    show_selection(ctx, f, mid);
    bot::answer(ctx, cq_id, answer, false);
    Next::Advanced
}

/// `Tiếp tục`: at least one message selected and the composed request within
/// the size limit, then the project screen.
fn go(ctx: &BotCtx, f: &mut Intake, cq_id: &str, mid: i64) -> Next {
    let k = match &f.source {
        Source::Forwarded { items, .. } => items.iter().filter(|i| i.selected).count(),
        Source::Typed(_) => 0,
    };
    if k == 0 {
        bot::answer(ctx, cq_id, compose::NONE_SELECTED, true);
        return Next::Kept;
    }
    if let Err(reason) = f.request() {
        bot::answer(ctx, cq_id, &reason, true);
        return Next::Kept;
    }
    f.stage = Stage::Project;
    let (text, keyboard) = project_screen(f);
    bot::edit(ctx, mid, &text, keyboard);
    bot::answer(ctx, cq_id, &format!("Đã chọn {k} tin"), false);
    audit(&format!("đã chọn {k} tin chuyển tiếp"));
    Next::Advanced
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

/// `Tạo task`: the allowlist is re-read, the daily cap is checked again, a last
/// dry run checks the stored slug, then exactly one job starts through
/// `start_new_task`.
fn confirm(ctx: &Arc<BotCtx>, state: &PollState, f: &mut Intake, cq_id: &str, mid: i64) -> Next {
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
    if let Some(refusal) = cap_refusal(ctx, state) {
        bot::edit(ctx, mid, &refusal, render::no_keyboard());
        bot::answer(ctx, cq_id, &refusal, true);
        audit("từ chối (giới hạn ngày)");
        return Next::Closed;
    }

    let request = match f.request() {
        Ok(r) => r,
        Err(reason) => return refuse(ctx, cq_id, mid, &reason, "yêu cầu"),
    };
    let mut req = StartRequest {
        project: &project_id,
        request: &request,
        name: Some(&slug),
        workflow: f.workflow_name(),
        team: None,
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
            Next::Started(Watch {
                job_id,
                project: info.project.id,
                branch: info.branch,
                started_ms: crate::db::now_ms(),
            })
        }
        Ok(other) => {
            let (reason, kind) = refusal_of(other);
            refuse(ctx, cq_id, mid, &reason, &kind)
        }
        Err(e) => refuse(ctx, cq_id, mid, &e.message, "nội bộ"),
    }
}

/// A forwarded message: data, never a command (it may start with `/task`). It is
/// added to the open forwarded flow while that flow collects or shows its list;
/// otherwise it opens a new forwarded flow (OD-4).
pub(crate) fn on_forward(ctx: &Arc<BotCtx>, state: &mut PollState, msg: &Value) {
    let now = crate::db::now_ms();
    let item = forward::item_of(msg);
    let open = state.intake.as_ref().is_some_and(|f| {
        f.is_forwarded()
            && !f.expired(now)
            && matches!(f.stage, Stage::Collecting { .. } | Stage::Select)
    });
    if !open {
        start_forwarded(ctx, state, item, now);
        return;
    }
    let f = match state.intake.as_mut() {
        Some(f) => f,
        None => return,
    };
    let mut changed = false;
    let mut tell_over_cap = false;
    if let Source::Forwarded {
        items,
        skipped,
        over_cap_told,
        ..
    } = &mut f.source
    {
        match item {
            Some(item) if items.len() < forward::MAX_FORWARDS => {
                items.push(item);
                changed = true;
            }
            Some(_) => {
                tell_over_cap = !*over_cap_told;
                *over_cap_told = true;
            }
            None => {
                *skipped += 1;
                changed = true;
            }
        }
    }
    if tell_over_cap {
        bot::reply(ctx, forward::OVER_CAP);
    }
    f.expires_at_ms = deadline(now);
    match &mut f.stage {
        Stage::Collecting { last_ms, .. } => *last_ms = now,
        _ => {
            if let (true, Some(mid)) = (changed, f.message_id) {
                show_selection(ctx, f, mid);
            }
        }
    }
}

/// Opens a forwarded flow with its first message (or one skipped message). The
/// allowlist and the daily cap are checked when the window closes (`tick`).
fn start_forwarded(ctx: &Arc<BotCtx>, state: &mut PollState, item: Option<ForwardItem>, now: i64) {
    let ids = compose::new_hex_id().and_then(|flow| Ok((flow, compose::new_hex_id()?)));
    let (flow_id, nonce) = match ids {
        Ok(ids) => ids,
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
    let skipped = usize::from(item.is_none());
    state.intake = Some(Intake {
        id: flow_id,
        stage: Stage::Collecting {
            first_ms: now,
            last_ms: now,
        },
        source: Source::Forwarded {
            items: item.into_iter().collect(),
            skipped,
            over_cap_told: false,
            instruction: None,
            nonce,
            started_ms: now,
        },
        offers: Vec::new(),
        project: None,
        default_workflow: String::new(),
        workflows: Vec::new(),
        workflow: None,
        slug: None,
        message_id: None,
        expires_at_ms: deadline(now),
    });
    audit("nhận tin chuyển tiếp");
}

/// Closes the collection window of a forwarded flow after `forward::quiet_ms()`
/// without a new forward, or `COLLECT_MAX_MS` after the first one: nothing usable
/// → one reply; refused by the cap or the allowlist → one reply; otherwise the
/// selection message is sent (synchronously, to learn its id).
fn close_window(ctx: &Arc<BotCtx>, state: &mut PollState, now_ms: i64) {
    let due = state.intake.as_ref().is_some_and(|f| match f.stage {
        Stage::Collecting { first_ms, last_ms } => {
            now_ms.saturating_sub(last_ms) >= forward::quiet_ms() as i64
                || now_ms.saturating_sub(first_ms) >= forward::COLLECT_MAX_MS
        }
        _ => false,
    });
    if !due {
        return;
    }
    let mut f = match state.intake.take() {
        Some(f) => f,
        None => return,
    };
    let (n, skipped) = match &f.source {
        Source::Forwarded { items, skipped, .. } => (items.len(), *skipped),
        Source::Typed(_) => return,
    };
    if n == 0 {
        bot::reply(
            ctx,
            &forward::NOTHING_USABLE_FMT.replace("{k}", &skipped.to_string()),
        );
        audit(&format!("bỏ qua {skipped} tin chuyển tiếp không có chữ"));
        return;
    }
    if let Some(refusal) = cap_refusal(ctx, state) {
        bot::reply(ctx, &refusal);
        audit("từ chối (giới hạn ngày)");
        return;
    }
    f.offers = match allowed_offers(ctx, state) {
        Ok(offers) => offers,
        Err(msg) => {
            bot::reply(ctx, &msg);
            audit("từ chối (telegram_projects)");
            return;
        }
    };
    let (text, keyboard) = match &f.source {
        Source::Forwarded { items, skipped, .. } => (
            forward::selection_text(items, *skipped),
            render::keyboard_json(&forward::selection_keyboard(&f.id, items)),
        ),
        Source::Typed(_) => return,
    };
    match bot::send_message(ctx, &text, keyboard) {
        Some(message_id) => {
            f.message_id = Some(message_id);
            f.stage = Stage::Select;
            f.expires_at_ms = deadline(now_ms);
            state.intake = Some(f);
            audit(&format!("mở danh sách {n} tin chuyển tiếp"));
        }
        None => audit("không gửi được danh sách tin chuyển tiếp"),
    }
}

/// Ends an expired flow, closes the collection window of forwarded messages and
/// reports the outcome of watched jobs (called after
/// each `getUpdates` and before each update, on the poll thread).
pub(crate) fn tick(ctx: &Arc<BotCtx>, state: &mut PollState, now_ms: i64) {
    if let Some(f) = take_expired(state, now_ms) {
        if let Some(message_id) = f.message_id {
            bot::edit(ctx, message_id, EXPIRED, render::no_keyboard());
        }
        audit("hết hạn");
    }
    close_window(ctx, state, now_ms);
    watch_tick(ctx, state, now_ms);
}

/// Id of the task row of a watched job, if it exists.
fn task_row_id(conn: &rusqlite::Connection, w: &Watch) -> Option<i64> {
    let project = crate::db::project_row_id(conn, &w.project).ok()?;
    crate::db::find_task_by_branch(conn, project, &w.branch)
        .ok()
        .flatten()
        .map(|t| t.id)
}

/// `CREATED_FMT` for task `id` on `branch`.
fn created_text(id: i64, branch: &str) -> String {
    CREATED_FMT
        .replace("{id}", &id.to_string())
        .replace("{branch}", branch)
}

/// `FAILED_PREFIX` and the last (at most `FAILED_LINES`) non-empty output lines.
fn failed_text(branch: &str, output: &[String]) -> String {
    let mut lines: Vec<String> = output
        .iter()
        .rev()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .take(FAILED_LINES)
        .map(|l| short(l, FAILED_LINE_MAX))
        .collect();
    lines.reverse();
    let body = if lines.is_empty() {
        NO_ERROR_LINES.to_string()
    } else {
        lines.join("\n")
    };
    format!("{}\n{body}", FAILED_PREFIX.replace("{branch}", branch))
}

/// One pass over the watched jobs: a task row → `Đã tạo task`; the job gone or
/// exited without a row → the failure report; older than `WATCH_MAX_MS` →
/// dropped. Each job gets at most one message.
fn watch_tick(ctx: &Arc<BotCtx>, state: &mut PollState, now_ms: i64) {
    if state.watch.is_empty() {
        return;
    }
    let conn = match crate::db::open(&ctx.st.db_path) {
        Ok(c) => {
            state.watch_db_failed = false;
            c
        }
        Err(e) => {
            if !state.watch_db_failed {
                crate::term::emit(&format!(
                    "Telegram: lỗi mở CSDL khi theo dõi job giao việc ({})",
                    e.message
                ));
                state.watch_db_failed = true;
            }
            return;
        }
    };
    let mut keep = Vec::new();
    for w in std::mem::take(&mut state.watch) {
        if let Some(id) = task_row_id(&conn, &w) {
            bot::send_message(ctx, &created_text(id, &w.branch), render::no_keyboard());
            audit(&format!("job {} đã tạo task {id}", w.job_id));
            continue;
        }
        let job = ctx.st.jobs.get(w.job_id);
        if job.as_ref().is_none_or(|j| j.state == "exited") {
            // The row may have been written just before the job ended.
            if let Some(id) = task_row_id(&conn, &w) {
                bot::send_message(ctx, &created_text(id, &w.branch), render::no_keyboard());
                audit(&format!("job {} đã tạo task {id}", w.job_id));
                continue;
            }
            let output = job.map(|j| j.output).unwrap_or_default();
            bot::send_message(ctx, &failed_text(&w.branch, &output), render::no_keyboard());
            audit(&format!("job {} dừng trước khi có task", w.job_id));
            continue;
        }
        if now_ms.saturating_sub(w.started_ms) >= WATCH_MAX_MS {
            audit(&format!(
                "thôi theo dõi job {} (10 phút chưa có task)",
                w.job_id
            ));
            continue;
        }
        keep.push(w);
    }
    state.watch = keep;
}

/// Closes the open flow, if any, showing `text` on its message without buttons.
pub(crate) fn close(ctx: &Arc<BotCtx>, state: &mut PollState, text: &str) {
    if let Some(f) = state.intake.take() {
        if let Some(message_id) = f.message_id {
            bot::edit(ctx, message_id, text, render::no_keyboard());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn flow(expires_at_ms: i64) -> Intake {
        Intake {
            id: "0123abcd".to_string(),
            stage: Stage::Project,
            source: Source::Typed("sửa app".to_string()),
            offers: Vec::new(),
            project: None,
            default_workflow: String::new(),
            workflows: Vec::new(),
            workflow: None,
            slug: None,
            message_id: Some(9),
            expires_at_ms,
        }
    }

    fn cmd(name: &str, rest: &str) -> Option<(String, String)> {
        Some((name.to_string(), rest.to_string()))
    }

    #[test]
    fn parse_command_rules() {
        assert_eq!(parse_command("/task sửa app"), cmd("task", "sửa app"));
        assert_eq!(parse_command("/task@ab_fake_bot x y"), cmd("task", "x y"));
        assert_eq!(parse_command("/task"), cmd("task", ""));
        assert_eq!(parse_command("/task\ndòng 2"), cmd("task", "dòng 2"));
        assert_eq!(parse_command("/huy"), cmd("huy", ""));
        assert_eq!(parse_command("  /huy  "), cmd("huy", ""));
        assert_eq!(
            parse_command("/task dòng 1\ndòng 2"),
            cmd("task", "dòng 1\ndòng 2")
        );
        let long = format!("/{}", "a".repeat(33));
        let max = format!("/{}", "a".repeat(32));
        assert_eq!(parse_command(&max), cmd(&"a".repeat(32), ""));
        for bad in [
            "/Task x",
            "/create-task x",
            "/",
            "task x",
            long.as_str(),
            "/task@ x",
            "/task@bad-bot x",
            "/tásk x",
            "",
        ] {
            assert_eq!(parse_command(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn slug_candidates_rules() {
        assert_eq!(
            slug_candidates("sửa app"),
            vec![
                "sua-app",
                "sua-app-2",
                "sua-app-3",
                "sua-app-4",
                "sua-app-5"
            ]
        );
        // Only the first line names the branch.
        assert_eq!(slug_candidates("sửa app\nchi tiết")[0], "sua-app");
        let emoji = slug_candidates("🙂");
        assert_eq!(emoji[0], DEFAULT_SLUG);
        assert!(emoji[0].starts_with("viec-tu-telegram"));
        assert_eq!(emoji[4], "viec-tu-telegram-5");

        let request = "sửa trang thanh toán của web bán hàng cho khách lẻ ngay";
        let request = format!("{request}{}", "x".repeat(60 - request.chars().count()));
        assert_eq!(request.chars().count(), 60);
        let all = slug_candidates(&request);
        assert_eq!(all.len(), 5);
        for (i, s) in all.iter().enumerate() {
            assert!(s.len() <= crate::routing::SLUG_MAX_LEN, "{s}");
            let stem = if i == 0 {
                s.as_str()
            } else {
                s.strip_suffix(&format!("-{}", i + 1)).expect("suffix")
            };
            assert!(!stem.is_empty() && !stem.ends_with('-'), "{s}");
        }
        // A cut that lands on a hyphen drops it before the suffix.
        let hyphen = format!("{}-b", "a".repeat(46));
        assert_eq!(slug_candidates(&hyphen)[1], format!("{}-2", "a".repeat(46)));
    }

    #[test]
    fn take_expired_uses_clock() {
        let mut st = PollState::default();
        assert!(take_expired(&mut st, 5_000).is_none());
        st.intake = Some(flow(1_000));
        assert!(take_expired(&mut st, 999).is_none());
        assert!(st.intake.is_some());
        let taken = take_expired(&mut st, 1_000).expect("expired at its deadline");
        assert_eq!(taken.id, "0123abcd");
        assert!(st.intake.is_none());
        assert!(flow(1_000).expired(1_001));
        assert!(!flow(1_000).expired(999));
    }

    fn git_repo(dir: &std::path::Path) {
        let status = std::process::Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(dir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .expect("git init");
        assert!(status.success());
    }

    fn entry(id: &str, path: &std::path::Path, kind: &str, missing: bool) -> Value {
        json!({
            "id": id,
            "path": path.to_str().unwrap(),
            "name_vi": id,
            "aliases": [],
            "type": kind,
            "profile": "work",
            "missing": missing,
        })
    }

    #[test]
    fn allowed_projects_rules() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let m = tempfile::tempdir().unwrap();
        let plain = tempfile::tempdir().unwrap();
        git_repo(a.path());
        git_repo(b.path());
        git_repo(m.path());
        let projects = json!([
            entry("a", a.path(), "code", false),
            entry("b-missing", b.path(), "code", true),
            entry("media-x", m.path(), "media", false),
            entry("not-a-repo", plain.path(), "code", false),
        ]);

        // Fail closed: no key, an empty list, a non-array or no string entries.
        let no_key = json!({"scan_roots": [], "projects": projects});
        assert_eq!(
            allowed_projects(&no_key, &cfg).unwrap_err(),
            NO_ALLOWLIST.to_string()
        );
        for list in [json!([]), json!("a"), json!([1, null])] {
            let map = json!({"projects": projects, "telegram_projects": list});
            assert_eq!(
                allowed_projects(&map, &cfg).unwrap_err(),
                NO_ALLOWLIST.to_string(),
                "{list}"
            );
        }

        let map = json!({
            "projects": projects,
            "telegram_projects": ["a", "khong-co", "b-missing"],
        });
        let (offers, unknown) = allowed_projects(&map, &cfg).unwrap();
        assert_eq!(
            offers.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            vec!["a"]
        );
        assert_eq!(unknown, vec!["khong-co".to_string()]);

        // Media and unrunnable projects are never offered.
        let map = json!({
            "projects": projects,
            "telegram_projects": ["khong-co", "b-missing", "media-x", "not-a-repo"],
        });
        assert_eq!(
            allowed_projects(&map, &cfg).unwrap_err(),
            NONE_RUNNABLE.to_string()
        );
        let map = json!({"projects": projects, "telegram_projects": ["khong-co"]});
        assert_eq!(
            allowed_projects(&map, &cfg).unwrap_err(),
            NONE_RUNNABLE.to_string()
        );
    }

    #[test]
    fn help_text_tells_how_to_give_work() {
        assert!(HELP_TEXT.contains("/task"), "{HELP_TEXT}");
        assert!(HELP_TEXT.contains("/huy"), "{HELP_TEXT}");
        assert!(
            !HELP_TEXT.contains("không giao việc mới"),
            "the help line still says the bot takes no new work: {HELP_TEXT}"
        );
    }

    #[test]
    fn local_midnight_is_start_of_local_day() {
        use chrono::TimeZone;
        let at = |h, m, sec| {
            chrono::Local
                .with_ymd_and_hms(2026, 10, 22, h, m, sec)
                .earliest()
                .expect("a local time")
        };
        let midnight = at(0, 0, 0).timestamp_millis();
        assert_eq!(local_midnight_ms(at(14, 3, 0)), midnight);
        assert_eq!(local_midnight_ms(at(23, 59, 59)), midnight);
        assert_eq!(local_midnight_ms(at(0, 0, 0)), midnight);
    }

    #[test]
    fn forwarded_needs_an_origin_object() {
        assert!(is_forwarded(
            &json!({"text": "/task x", "forward_origin": {"type": "user"}})
        ));
        assert!(!is_forwarded(&json!({"text": "/task x"})));
        assert!(!is_forwarded(
            &json!({"text": "/task x", "forward_origin": "user"})
        ));
    }
}
