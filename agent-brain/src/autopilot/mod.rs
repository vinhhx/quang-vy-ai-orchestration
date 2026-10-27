//! Autopilot (AUTO-05): driver nhỏ nhận một task, lặp `decide` và chạy từng bước
//! bằng một tiến trình con `agent-brain run …`; dừng ở cổng duyệt plan, khi xong
//! hoặc khi gặp lý do dừng. Driver không tự giám sát claude — tiến trình con làm việc đó.

pub mod claim;
pub mod decide;
pub mod view;

use crate::app::Ctx;
use crate::autopilot::claim::ClaimOpts;
use crate::autopilot::decide::{
    AutoJson, Decision, Loop, Replan, Stage, StageRun, StopInfo, StopReason,
};
use crate::error::{
    BrainError, BrainResult, EXIT_BLOCKED, EXIT_INTERRUPTED, EXIT_OK, EXIT_PROTOCOL, EXIT_QUOTA,
    EXIT_WAITING,
};
use crate::workflows::Workflow;
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Trần an toàn số bước một lần chạy driver được bắt đầu.
pub const MAX_STAGES_PER_RUN: usize = 50;
/// Biến môi trường driver đặt cho tiến trình con: id task của autopilot.
pub const AUTOPILOT_TASK_ENV: &str = "AGENT_BRAIN_AUTOPILOT_TASK";

const POLL: Duration = Duration::from_millis(200);

/// Driver nhận SIGINT / SIGTERM / SIGHUP: handler chỉ đặt cờ này.
static SIGNALED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_sig: libc::c_int) {
    SIGNALED.store(true, Ordering::SeqCst);
}

/// Cài handler cho SIGINT, SIGTERM, SIGHUP (một lần, trước khi chạy bước nào).
fn install_signal_handlers() {
    let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
    unsafe {
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
        libc::signal(libc::SIGHUP, handler);
    }
}

fn signaled() -> bool {
    SIGNALED.load(Ordering::SeqCst)
}

/// Tham số dòng lệnh `agent-brain auto`.
#[derive(Debug, Clone, Default)]
pub struct AutoOpts {
    pub start: bool,
    pub args: Vec<String>,
    pub name: Option<String>,
    pub workflow: Option<String>,
    pub more_round: bool,
    pub replan: bool,
    pub note: Option<String>,
    pub origin: Option<String>,
    pub team: Option<String>,
}

/// Kết quả một tiến trình con.
#[derive(Debug, Clone, Default)]
struct ChildOutcome {
    exit_code: Option<i32>,
    /// Dòng stderr đầu tiên có ích: bỏ banner `[agent-brain]`, `Cảnh báo …` và dòng tiến độ (IN-06).
    first_err_line: Option<String>,
    /// Dòng stderr không rỗng cuối cùng.
    last_err_line: Option<String>,
    interrupted: bool,
}

/// Dòng tiến độ của vai trò: `task N, run M` hoặc `run M`.
fn is_progress_line(line: &str) -> bool {
    fn digits(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
    }
    if let Some(rest) = line.strip_prefix("task ") {
        return match rest.split_once(", run ") {
            Some((t, r)) => digits(t) && digits(r),
            None => false,
        };
    }
    line.strip_prefix("run ").is_some_and(digits)
}

/// Dòng stderr của con có thể làm chi tiết `run lỗi` (IN-06).
fn is_error_candidate(line: &str) -> bool {
    !line.is_empty()
        && !line.starts_with("[agent-brain]")
        && !line.starts_with("Cảnh báo")
        && !is_progress_line(line)
}

/// Argv của tiến trình con cho một bước (vị trí sau `--`).
pub fn stage_args(sr: &StageRun, slug: &str, project: &str, title: &str) -> Vec<String> {
    let round = sr.round.to_string();
    let mut a: Vec<String> = Vec::new();
    match sr.stage {
        Stage::Plan => {
            a.extend(["run", "lead", "--name", slug, "--round", &round].map(String::from));
            if sr.review_input {
                a.push("--review".to_string());
            }
            if let Some(note) = sr.note.as_deref() {
                a.push(format!("--note={note}"));
            }
            a.push("--".to_string());
            a.push(project.to_string());
            a.push(title.to_string());
        }
        Stage::PlanReview => {
            a.extend(["run", "plan-review", "--round", &round, "--", project].map(String::from));
        }
        Stage::Implement => {
            a.extend(["run", "dev", "--round", &round, "--", project].map(String::from));
        }
        Stage::CodeReview => {
            a.extend(["run", "code-review", "--round", &round, "--", project].map(String::from));
        }
        Stage::Qa => {
            a.extend(["run", "qa", "--round", &round, "--", project].map(String::from));
        }
        Stage::Fix => {
            let fix_for = sr.fix_for.map(|f| f.cli_arg()).unwrap_or("code-review");
            a.extend(
                [
                    "run", "dev", "--fix", fix_for, "--round", &round, "--", project,
                ]
                .map(String::from),
            );
        }
    }
    a
}

fn loop_of_stage(sr_stage: Stage, fix_qa: bool) -> Loop {
    match sr_stage {
        Stage::Plan | Stage::PlanReview => Loop::Plan,
        Stage::Implement | Stage::CodeReview => Loop::Build,
        Stage::Qa => Loop::Qa,
        Stage::Fix => {
            if fix_qa {
                Loop::Qa
            } else {
                Loop::Build
            }
        }
    }
}

/// StopInfo `run lỗi` không gắn run (bước không tạo run, lỗi git, lỗi driver).
fn run_loi(detail: String, sr: Option<&StageRun>) -> StopInfo {
    StopInfo {
        reason: StopReason::RunLoi,
        loop_name: sr.map(|s| {
            loop_of_stage(
                s.stage,
                s.fix_for == Some(crate::autopilot::decide::FixFor::Qa),
            )
        }),
        stage: sr.map(|s| s.stage),
        round: sr.map(|s| s.round),
        cap: None,
        run_id: None,
        role: sr.map(|s| s.stage.role().to_string()),
        at_ms: crate::db::now_ms(),
        detail_line: Some(detail),
        used_ms: None,
        used_usd: None,
        estimated: None,
        limit_minutes: None,
        limit_usd: None,
    }
}

/// Nhánh đang checkout không phải nhánh của task → StopInfo `run lỗi` (CR-01):
/// mọi vai trò suy ra task từ HEAD, nên chạy bước trên nhánh khác là làm việc của task khác.
fn branch_stop(
    root: &std::path::Path,
    task: &crate::db::TaskRow,
    sr: &StageRun,
) -> Option<StopInfo> {
    let cur = crate::gitops::current_branch(root);
    if cur.as_deref() == Some(task.branch.as_str()) {
        return None;
    }
    Some(run_loi(
        branch_mismatch_text(&task.branch, cur.as_deref()),
        Some(sr),
    ))
}

fn branch_mismatch_text(branch: &str, cur: Option<&str>) -> String {
    format!(
        "Không ở nhánh task '{branch}' (nhánh hiện tại: '{}') — không đoán",
        cur.unwrap_or("None")
    )
}

/// Chạy `agent-brain <args>` làm tiến trình con (PDEATHSIG, stdout kế thừa, stderr chép lại).
fn spawn_stage(ctx: &mut Ctx, task_id: Option<i64>, args: &[String]) -> BrainResult<ChildOutcome> {
    use std::os::unix::process::CommandExt;

    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    if let Some(tid) = task_id {
        cmd.env(AUTOPILOT_TASK_ENV, tid.to_string());
    } else {
        cmd.env_remove(AUTOPILOT_TASK_ENV);
    }
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::piped());

    // Con ở cùng nhóm tiến trình với driver: Ctrl+C ở terminal tới cả hai; driver
    // chuyển SIGTERM để con dừng êm, không dừng cưỡng bức (WR-01).
    // Driver chết thì con nhận SIGTERM (PR_SET_PDEATHSIG), không chạy mồ côi.
    let driver_pid = unsafe { libc::getpid() };
    unsafe {
        cmd.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != driver_pid {
                return Err(std::io::Error::other("driver exited before child exec"));
            }
            Ok(())
        });
    }

    let mut child = cmd.spawn()?;
    let stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut first: Option<String> = None;
        let mut last: Option<String> = None;
        if let Some(s) = stderr {
            let mut r = std::io::BufReader::new(s);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match r.read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf);
                        let line = line.trim_end_matches(['\n', '\r']);
                        crate::term::emit(line);
                        let t = line.trim();
                        if first.is_none() && is_error_candidate(t) {
                            first = Some(t.to_string());
                        }
                        if !t.is_empty() {
                            last = Some(t.to_string());
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        (first, last)
    });

    // Chờ con: tín hiệu tới driver → chuyển một SIGTERM cho con rồi chờ nó tự dừng
    // (con dừng claude của nó). Ctrl+C ở terminal tới cả hai; driver chuyển SIGTERM
    // để con dừng êm, không dừng cưỡng bức — SIGINT thứ hai sẽ thành SIGKILL ở runner
    // (WR-01). Cờ kill trên task → runner::cmd_kill một lần (D-04).
    let child_pid = child.id() as libc::pid_t;
    let mut forwarded = false;
    let mut kill_done = false;
    let status = loop {
        if let Some(st) = child.try_wait()? {
            break st;
        }
        if signaled() && !forwarded {
            unsafe {
                libc::kill(child_pid, libc::SIGTERM);
            }
            forwarded = true;
        }
        if let Some(tid) = task_id {
            if !kill_done && !forwarded && kill_flag_set(ctx, tid) {
                if let Ok(Some(run)) = crate::db::running_run_for_task(&ctx.conn, tid) {
                    kill_done = true;
                    // Kill của người khác đang chạy (đã ghi kill_requested_at) — chỉ chờ con.
                    if run.kill_requested_at.is_none() {
                        match crate::runner::cmd_kill(ctx, tid) {
                            Ok(code) => crate::term::emit(&format!(
                                "Autopilot: đã yêu cầu dừng run của task {tid} (mã {code})"
                            )),
                            Err(e) => crate::term::emit(&format!(
                                "Autopilot: không dừng được run của task {tid}: {}",
                                e.message
                            )),
                        }
                    }
                }
            }
        }
        std::thread::sleep(POLL);
    };

    let (first_err_line, last_err_line) = reader.join().unwrap_or((None, None));
    Ok(ChildOutcome {
        exit_code: status.code(),
        first_err_line,
        last_err_line,
        interrupted: signaled(),
    })
}

/// Task có cờ dừng loại `kill` (Kill trong lúc autopilot chạy).
fn kill_flag_set(ctx: &Ctx, task_id: i64) -> bool {
    match crate::db::task_auto(&ctx.conn, task_id) {
        Ok(Some(a)) => a.auto_stop_at.is_some() && a.auto_stop_kind.as_deref() == Some("kill"),
        _ => false,
    }
}

fn max_run_id(ctx: &Ctx, task_id: i64) -> BrainResult<i64> {
    Ok(crate::db::runs_for_task(&ctx.conn, task_id)?
        .iter()
        .map(|r| r.id)
        .max()
        .unwrap_or(0))
}

/// Mã thoát của con không phải lỗi của vai trò: 0, 3, 4, 5 (hết quota), 6 (chờ nhân sự), 130.
pub(crate) fn child_exit_is_neutral(code: i32) -> bool {
    [
        EXIT_OK,
        EXIT_PROTOCOL,
        EXIT_BLOCKED,
        EXIT_QUOTA,
        EXIT_WAITING,
        EXIT_INTERRUPTED,
    ]
    .contains(&code)
}

/// Con thoát với mã lỗi (không phải 0, 3, 4, 5 = hết quota, 6 = chờ nhân sự, 130) trong khi run mới nhất của task
/// `succeeded` mà chưa có outcome: vai trò hỏng sau run (vd. commit bị hook từ chối).
/// Dừng `run lỗi` gắn run đó, không để `decide` chạy lại bước (WR-02, D-27).
/// Run failed / timed_out / killed hoặc đã có outcome thì để `decide` (D2) xử lý.
fn child_error_stop(
    ctx: &Ctx,
    task_id: i64,
    sr: &StageRun,
    outcome: &ChildOutcome,
) -> BrainResult<Option<StopInfo>> {
    let code = match outcome.exit_code {
        Some(c) if !child_exit_is_neutral(c) => c,
        _ => return Ok(None),
    };
    let run = match crate::db::latest_run_for_task(&ctx.conn, task_id)? {
        Some(r) if r.status == "succeeded" && r.outcome.is_none() => r,
        _ => return Ok(None),
    };
    let mut info = run_loi(
        outcome
            .last_err_line
            .clone()
            .unwrap_or_else(|| format!("Bước {} thoát với mã {code}", sr.stage.as_str())),
        Some(sr),
    );
    info.run_id = Some(run.id);
    info.role = Some(run.role);
    Ok(Some(info))
}

fn stop_with(ctx: &Ctx, task_id: i64, slug: &str, mut aj: AutoJson, info: StopInfo) -> i32 {
    let line = view::terminal_line(&info, slug, task_id);
    aj.stop = Some(info);
    if let Err(e) = claim::release(&ctx.conn, task_id, "stopped", &aj) {
        crate::term::emit(&e.message);
    }
    crate::term::emit(&line);
    EXIT_BLOCKED
}

/// Vòng lặp driver: đọc dữ kiện, `decide`, chạy bước kế tiếp (task đã được nhận).
fn run_loop(ctx: &mut Ctx, task_id: i64, resume_first: bool) -> BrainResult<i32> {
    let task = crate::db::get_task(&ctx.conn, task_id)?
        .ok_or_else(|| BrainError::error(format!("Không tìm thấy task {task_id}")))?;
    let (proj_slug, _profile) = claim::task_project(&ctx.conn, task_id)?
        .ok_or_else(|| BrainError::error(format!("Không tìm thấy dự án của task {task_id}")))?;
    let project = ctx
        .projects
        .iter()
        .find(|p| p.id == proj_slug)
        .cloned()
        .ok_or_else(|| {
            BrainError::error(format!(
                "Dự án '{proj_slug}' không còn trong projects_map.json"
            ))
        })?;
    let root = crate::projects::require_runnable(&project, &ctx.cfg)?;
    let auto0 = crate::db::task_auto(&ctx.conn, task_id)?.unwrap_or_default();
    let wf = auto0
        .workflow_json
        .as_deref()
        .and_then(Workflow::from_snapshot)
        .ok_or_else(|| {
            BrainError::error(format!(
                "Task {task_id} tạo trước Phase 5.2 — không có quy trình để chạy autopilot."
            ))
        })?;

    let mut resume = resume_first;
    for _ in 0..MAX_STAGES_PER_RUN {
        // Tín hiệu giữa hai bước: không bắt đầu bước mới.
        if signaled() {
            return Ok(interrupted(ctx, task_id));
        }
        let auto = crate::db::task_auto(&ctx.conn, task_id)?.unwrap_or_default();
        let mut aj = AutoJson::parse(auto.auto_json.as_deref());
        let runs = crate::db::runs_for_task(&ctx.conn, task_id)?;
        let plan_commit = match crate::gitops::plan_commit_on_branch(
            &root,
            &task.branch,
            &task.slug,
            task.base_commit.as_deref(),
        ) {
            Ok(pc) => pc,
            Err(()) => {
                let info = run_loi(
                    format!("Không đọc được lịch sử git của nhánh {}", task.branch),
                    None,
                );
                return Ok(stop_with(ctx, task_id, &task.slug, aj, info));
            }
        };
        let facts = decide::build_facts(
            &wf,
            &runs,
            &auto,
            &aj,
            plan_commit.as_deref(),
            &ctx.cfg.pricing,
            crate::db::now_ms(),
            resume,
        );
        resume = false;

        match decide::decide(&wf, &facts) {
            Decision::Run(sr) => {
                // Trước mọi bước: đúng nhánh task, nếu không thì dừng, không đoán (CR-01).
                if let Some(info) = branch_stop(&root, &task, &sr) {
                    return Ok(stop_with(ctx, task_id, &task.slug, aj, info));
                }
                if crate::pricing::exhausted(&facts.budget_usage, &facts.budget_limits)
                    && aj.bypass_stages > 0
                {
                    aj.bypass_stages -= 1;
                }
                if sr.stage == Stage::Plan {
                    if sr.round == 1 {
                        aj.grants.clear();
                    }
                    aj.replan = None;
                }
                crate::db::set_auto_json(&ctx.conn, task_id, Some(&aj.to_json()))?;
                crate::term::emit(&format!(
                    "Autopilot · task {task_id} · {} vòng {}",
                    sr.stage.as_str(),
                    sr.round
                ));

                let before = max_run_id(ctx, task_id)?;
                let args = stage_args(&sr, &task.slug, &project.id, &task.title);
                let outcome = match spawn_stage(ctx, Some(task_id), &args) {
                    Ok(o) => o,
                    Err(e) => {
                        let info = run_loi(e.message.clone(), Some(&sr));
                        return Ok(stop_with(ctx, task_id, &task.slug, aj, info));
                    }
                };
                if outcome.interrupted {
                    return Ok(interrupted(ctx, task_id));
                }
                // D-19: bước Dev do người làm không tạo run nào và không lỗi — nhả task chờ `Xong`.
                if outcome.exit_code == Some(EXIT_WAITING) {
                    claim::release(&ctx.conn, task_id, "waiting_human", &aj)?;
                    crate::term::emit(&format!(
                        "Autopilot · task {task_id} · chờ nhân sự làm — bấm Xong khi họ báo xong."
                    ));
                    return Ok(EXIT_OK);
                }
                let after = max_run_id(ctx, task_id)?;
                if after <= before {
                    let detail = outcome
                        .first_err_line
                        .clone()
                        .unwrap_or_else(|| format!("Bước {} không tạo run nào", sr.stage.as_str()));
                    let info = run_loi(detail, Some(&sr));
                    return Ok(stop_with(ctx, task_id, &task.slug, aj, info));
                }
                if let Some(info) = child_error_stop(ctx, task_id, &sr, &outcome)? {
                    return Ok(stop_with(ctx, task_id, &task.slug, aj, info));
                }
            }
            Decision::Gate => {
                claim::release(&ctx.conn, task_id, "gate", &aj)?;
                crate::term::emit(&view::gate_line(task_id));
                return Ok(EXIT_OK);
            }
            Decision::Done => {
                claim::release(&ctx.conn, task_id, "done", &aj)?;
                crate::term::emit(&view::done_line(task_id, wf.has("qa")));
                return Ok(EXIT_OK);
            }
            Decision::Stop(info) => {
                return Ok(stop_with(ctx, task_id, &task.slug, aj, info));
            }
        }
    }

    let aj = AutoJson::parse(
        crate::db::task_auto(&ctx.conn, task_id)?
            .unwrap_or_default()
            .auto_json
            .as_deref(),
    );
    let info = run_loi(
        format!("Autopilot dừng an toàn sau {MAX_STAGES_PER_RUN} bước trong một lần chạy"),
        None,
    );
    Ok(stop_with(ctx, task_id, &task.slug, aj, info))
}

/// Driver bị ngắt: ghi `interrupted`, xóa pid, trả 130.
fn interrupted(ctx: &Ctx, task_id: i64) -> i32 {
    let aj = crate::db::task_auto(&ctx.conn, task_id)
        .ok()
        .flatten()
        .map(|a| AutoJson::parse(a.auto_json.as_deref()))
        .unwrap_or_default();
    if let Err(e) = claim::release(&ctx.conn, task_id, "interrupted", &aj) {
        crate::term::emit(&e.message);
    }
    crate::term::emit(&format!(
        "Autopilot đã dừng (bị ngắt) — task {task_id}; chạy tiếp: agent-brain auto {task_id}"
    ));
    crate::error::EXIT_INTERRUPTED
}

/// Chạy vòng lặp; lỗi nội bộ của driver vẫn kết thúc bằng `stopped` + `run lỗi`.
fn drive(ctx: &mut Ctx, task_id: i64, resume_first: bool) -> BrainResult<i32> {
    match run_loop(ctx, task_id, resume_first) {
        Ok(code) => Ok(code),
        Err(e) => {
            let slug = crate::db::get_task(&ctx.conn, task_id)
                .ok()
                .flatten()
                .map(|t| t.slug)
                .unwrap_or_default();
            let aj = crate::db::task_auto(&ctx.conn, task_id)
                .ok()
                .flatten()
                .map(|a| AutoJson::parse(a.auto_json.as_deref()))
                .unwrap_or_default();
            let first = e.message.lines().next().unwrap_or("").to_string();
            crate::term::emit(&e.message);
            Ok(stop_with(ctx, task_id, &slug, aj, run_loi(first, None)))
        }
    }
}

fn claim_task(ctx: &mut Ctx, task_id: i64, opts: &ClaimOpts) -> BrainResult<AutoJson> {
    let (slug, profile) = claim::task_project(&ctx.conn, task_id)?
        .ok_or_else(|| BrainError::error(format!("Không tìm thấy task {task_id}")))?;
    claim::claim(&mut ctx.conn, task_id, &slug, &profile, opts)
}

/// `agent-brain auto` — `--start` tạo task bằng bước Lead đầu tiên, còn lại tiếp tục task có sẵn.
pub fn cmd_auto(ctx: &mut Ctx, opts: &AutoOpts) -> BrainResult<i32> {
    install_signal_handlers();
    if !opts.start && opts.origin.is_some() {
        return Err(BrainError::error("--origin chỉ dùng với --start"));
    }
    if !opts.start && opts.team.is_some() {
        return Err(BrainError::error("--team chỉ dùng với --start"));
    }
    if opts.start {
        if opts.more_round || opts.replan || opts.note.is_some() {
            return Err(BrainError::error(
                "--more-round, --replan và --note không dùng với --start",
            ));
        }
        if opts.args.len() != 2 {
            return Err(BrainError::error("auto --start cần <dự án> và <yêu cầu>"));
        }
        let project_q = opts.args[0].as_str();
        let request = opts.args[1].trim();
        if request.is_empty() {
            return Err(BrainError::error("auto --start cần <dự án> và <yêu cầu>"));
        }
        let project = crate::projects::select_project(project_q, &ctx.map)?;
        crate::projects::require_runnable(&project, &ctx.cfg)?;

        let loaded = crate::workflows::load(&ctx.cfg);
        let resolved = crate::workflows::resolve(
            &loaded,
            opts.workflow.as_deref(),
            crate::projects::workflow_of(&ctx.map, &project.id).as_deref(),
        )?;
        if let Some(ref n) = resolved.note {
            crate::term::emit(n);
        }

        // TEAM-03: an unknown explicit team is refused before anything is spawned. Without
        // --team the Lead child applies the project default (or mac-dinh) itself.
        let team_name = match opts.team.as_deref() {
            Some(requested) => {
                let (teams_loaded, _staff) = crate::teams::load();
                Some(
                    crate::teams::resolve(&teams_loaded, Some(requested), None)?
                        .team
                        .name,
                )
            }
            None => None,
        };

        let slug = crate::routing::slugify(opts.name.as_deref().unwrap_or(request));
        if slug.is_empty() {
            return Err(BrainError::error(
                "Không thể tạo tên task từ yêu cầu — hãy truyền cờ --name <slug>",
            ));
        }
        if let Some(reason) =
            claim::project_busy_reason(&ctx.conn, &project.id, &project.profile, None)?
        {
            return Err(BrainError::error(reason));
        }

        let branch = format!("task/{slug}");
        let proj_row_id = crate::db::project_row_id(&ctx.conn, &project.id)?;
        let existing = crate::db::find_task_by_branch(&ctx.conn, proj_row_id, &branch)?;
        let before = match existing.as_ref() {
            Some(t) => max_run_id(ctx, t.id)?,
            None => 0,
        };

        let mut args: Vec<String> = [
            "run",
            "lead",
            "--name",
            &slug,
            "--workflow",
            &resolved.workflow.name,
            "--round",
            "1",
        ]
        .map(String::from)
        .to_vec();
        if let Some(origin) = opts.origin.as_deref() {
            args.push("--origin".to_string());
            args.push(origin.to_string());
        }
        if let Some(team) = team_name.as_deref() {
            args.push("--team".to_string());
            args.push(team.to_string());
        }
        args.push("--".to_string());
        args.push(project.id.clone());
        args.push(request.to_string());
        crate::term::emit(&format!("Autopilot · task mới {slug} · plan vòng 1"));
        let outcome = spawn_stage(ctx, None, &args)?;

        let task = match crate::db::find_task_by_branch(&ctx.conn, proj_row_id, &branch)? {
            Some(t) => t,
            None => {
                if outcome.interrupted {
                    return Ok(crate::error::EXIT_INTERRUPTED);
                }
                return Err(BrainError::error(
                    "Lead không tạo được task — xem lỗi ở trên",
                ));
            }
        };
        if outcome.interrupted {
            return Ok(interrupted(ctx, task.id));
        }

        let aj = claim_task(ctx, task.id, &ClaimOpts::default())?;
        let sr = StageRun {
            stage: Stage::Plan,
            round: 1,
            fix_for: None,
            review_input: false,
            note: None,
            fresh_plan: true,
        };
        let after = max_run_id(ctx, task.id)?;
        if after <= before {
            let detail = outcome
                .first_err_line
                .clone()
                .unwrap_or_else(|| "Bước plan không tạo run nào".to_string());
            return Ok(stop_with(
                ctx,
                task.id,
                &task.slug,
                aj,
                run_loi(detail, Some(&sr)),
            ));
        }
        if let Some(info) = child_error_stop(ctx, task.id, &sr, &outcome)? {
            return Ok(stop_with(ctx, task.id, &task.slug, aj, info));
        }
        return drive(ctx, task.id, false);
    }

    if opts.name.is_some() || opts.workflow.is_some() {
        return Err(BrainError::error(
            "--name và --workflow chỉ dùng với --start",
        ));
    }
    if opts.args.len() != 1 {
        return Err(BrainError::error("auto cần <ID task>"));
    }
    let task_id: i64 = opts.args[0]
        .trim()
        .parse()
        .map_err(|_| BrainError::error("auto cần <ID task>"))?;

    let note = opts
        .note
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if let Some(n) = note.as_deref() {
        if n.chars().count() > crate::roles::lead::MAX_NOTE_CHARS {
            return Err(BrainError::error("Ghi chú quá dài (tối đa 500 ký tự)"));
        }
    }
    if opts.note.is_some() && !opts.replan {
        return Err(BrainError::error("--note chỉ dùng với --replan"));
    }

    let copts = ClaimOpts {
        more_round: opts.more_round,
        replan: opts.replan.then(|| Replan { note: note.clone() }),
    };
    claim_task(ctx, task_id, &copts)?;
    drive(ctx, task_id, true)
}

/// `human done` sau một vi phạm giao thức (D-20): nhận lại task và chạy vòng lặp KHÔNG ở chế độ
/// resume, để D2 đọc run người vừa ghi (`protocol_violation`) và ghi lý do dừng thay vì chạy lại
/// bước Dev.
pub fn stop_after_human_violation(ctx: &mut Ctx, task_id: i64) -> BrainResult<i32> {
    install_signal_handlers();
    claim_task(ctx, task_id, &ClaimOpts::default())?;
    drive(ctx, task_id, false)
}

/// `agent-brain approve` — ghi duyệt plan cho commit plan hiện tại; với quy trình
/// `auto: true` driver chạy tiếp ngay trong tiến trình này (trừ khi `--no-continue`).
pub fn cmd_approve(ctx: &mut Ctx, task_id: i64, no_continue: bool) -> BrainResult<i32> {
    let task = crate::db::get_task(&ctx.conn, task_id)?
        .ok_or_else(|| BrainError::error(format!("Không tìm thấy task {task_id}")))?;
    let (proj_slug, profile) = claim::task_project(&ctx.conn, task_id)?
        .ok_or_else(|| BrainError::error(format!("Không tìm thấy dự án của task {task_id}")))?;
    let project = ctx
        .projects
        .iter()
        .find(|p| p.id == proj_slug)
        .cloned()
        .ok_or_else(|| {
            BrainError::error(format!(
                "Dự án '{proj_slug}' không còn trong projects_map.json"
            ))
        })?;
    let root = crate::projects::require_runnable(&project, &ctx.cfg)?;

    let plan_commit = claim::approve_check(&ctx.conn, &task, &root).map_err(BrainError::error)?;

    let auto = crate::db::task_auto(&ctx.conn, task_id)?.unwrap_or_default();
    let wf_auto = auto
        .workflow_json
        .as_deref()
        .and_then(Workflow::from_snapshot)
        .map(|w| w.auto)
        .unwrap_or(false);
    // Autopilot đã tắt cho task (`ended`): duyệt chỉ ghi nhận, không chạy tiếp (D-15 sửa đổi).
    let ended = auto.auto_state.as_deref() == Some("ended");
    let cont = wf_auto && !no_continue && !ended;
    if cont {
        // Kiểm tra bận và nhánh trước khi ghi, để một lần từ chối không để lại nửa chừng.
        if let Some(reason) =
            claim::project_busy_reason(&ctx.conn, &proj_slug, &profile, Some(task_id))?
        {
            return Err(BrainError::error(reason));
        }
        let cur = crate::gitops::current_branch(&root);
        if cur.as_deref() != Some(task.branch.as_str()) {
            return Err(BrainError::error(format!(
                "{}. Duyệt mà không chạy tiếp: agent-brain approve {task_id} --no-continue",
                branch_mismatch_text(&task.branch, cur.as_deref())
            )));
        }
    }

    let at = claim::approve_record(&ctx.conn, task_id, &plan_commit)?;
    let sha7: String = plan_commit.chars().take(7).collect();
    crate::term::emit(&format!(
        "Đã duyệt plan {sha7} lúc {} (task {task_id}).",
        view::fmt_hhmm(at)
    ));
    if !cont {
        if wf_auto && !no_continue && ended {
            crate::term::emit(&format!(
                "Autopilot đã tắt cho task {task_id} — chỉ ghi nhận duyệt. Bật lại: agent-brain auto {task_id}"
            ));
        }
        return Ok(EXIT_OK);
    }

    install_signal_handlers();
    claim::claim(
        &mut ctx.conn,
        task_id,
        &proj_slug,
        &profile,
        &ClaimOpts::default(),
    )?;
    drive(ctx, task_id, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_exit_is_neutral_ignores_waiting() {
        for code in [
            EXIT_OK,
            EXIT_PROTOCOL,
            EXIT_BLOCKED,
            EXIT_QUOTA,
            EXIT_WAITING,
            EXIT_INTERRUPTED,
        ] {
            assert!(child_exit_is_neutral(code), "{code}");
        }
        assert_eq!(EXIT_WAITING, 6);
        for code in [1, 2, 7, 137] {
            assert!(!child_exit_is_neutral(code), "{code}");
        }
    }
}
