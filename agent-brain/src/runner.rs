use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Succeeded,
    Failed,
    TimedOut,
    Killed,
    Lost,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Succeeded => "succeeded",
            RunStatus::Failed => "failed",
            RunStatus::TimedOut => "timed_out",
            RunStatus::Killed => "killed",
            RunStatus::Lost => "lost",
        }
    }
}

pub fn reconcile_stale_runs(conn: &Connection) -> BrainResult<usize> {
    let running = crate::db::running_runs(conn)?;
    let mut lost_count = 0;
    for run in running {
        let is_stale = match (run.pid, run.pid_start) {
            (Some(pid), Some(start)) if pid > 0 => {
                let proc_id = crate::procs::ProcId {
                    pid: pid as i32,
                    start: start as u64,
                };
                !crate::procs::alive(proc_id)
            }
            _ => true,
        };

        if is_stale {
            crate::db::mark_status(conn, run.id, "lost")?;
            lost_count += 1;
        }
    }
    Ok(lost_count)
}

pub fn cmd_kill(ctx: &mut Ctx, task_id: i64) -> BrainResult<i32> {
    let task = crate::db::get_task(&ctx.conn, task_id)?;
    if task.is_none() {
        return Err(BrainError::error(format!("Không tìm thấy task {task_id}")));
    }

    // D-04 / AUTO-05: Kill cũng dừng autopilot còn sống của task — ghi cờ dừng loại
    // `kill` trước, driver thấy cờ và dừng với lý do `dừng bởi bạn` (không gửi tín hiệu cho driver).
    let driver = crate::db::task_auto(&ctx.conn, task_id)?
        .map(|a| crate::autopilot::claim::driver_alive(&a))
        .unwrap_or(false);
    if driver {
        crate::db::request_auto_stop(&ctx.conn, task_id, "kill")?;
    }

    let running = crate::db::running_run_for_task(&ctx.conn, task_id)?;
    let run = match running {
        Some(r) => r,
        None => {
            if driver {
                crate::term::emit(&format!(
                    "Đã yêu cầu autopilot dừng (task {task_id}) — không có run đang chạy; autopilot dừng trước bước tiếp theo."
                ));
                return Ok(0);
            }
            let latest = crate::db::latest_run_for_task(&ctx.conn, task_id)?;
            let msg = match latest {
                Some(r) => format!(
                    "Task {task_id} không có run đang chạy (run gần nhất: run {} · {})",
                    r.id, r.status
                ),
                None => format!("Task {task_id} không có run đang chạy"),
            };
            return Err(BrainError::error(msg));
        }
    };

    let root = match (run.pid, run.pid_start) {
        (Some(p), Some(s)) if p > 0 => crate::procs::ProcId {
            pid: p as i32,
            start: s as u64,
        },
        _ => {
            crate::db::mark_lost_if_running(&ctx.conn, run.id)?;
            return Err(BrainError::error(format!(
                "Run {} không còn tiến trình — đã đánh dấu lost, không gửi tín hiệu",
                run.id
            )));
        }
    };

    if !crate::procs::alive(root) {
        crate::db::mark_lost_if_running(&ctx.conn, run.id)?;
        return Err(BrainError::error(format!(
            "Run {} không còn tiến trình — đã đánh dấu lost, không gửi tín hiệu",
            run.id
        )));
    }

    crate::db::mark_kill_requested(&ctx.conn, run.id)?;
    crate::term::emit(&format!(
        "Đang dừng task {task_id} (run {}, pid {})...",
        run.id, root.pid
    ));

    let force = std::sync::atomic::AtomicBool::new(false);
    let report =
        crate::procs::kill_tree(root, Duration::from_secs(ctx.cfg.kill_grace_secs), &force);

    let wait_start = std::time::Instant::now();
    let mut still_running = true;
    while wait_start.elapsed() < Duration::from_secs(2) {
        if let Ok(Some(current)) = crate::db::get_run(&ctx.conn, run.id) {
            if current.status != "running" {
                still_running = false;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    if still_running {
        let _ = crate::db::mark_status(&ctx.conn, run.id, "killed");
    }

    if report.survivors.is_empty() {
        crate::term::emit(&format!(
            "Đã dừng task {task_id} (run {}) sau {} ms",
            run.id,
            report.elapsed.as_millis()
        ));
        if driver {
            crate::term::emit(&format!(
                "Autopilot của task {task_id} cũng dừng với lý do \"dừng bởi bạn\"."
            ));
        }
        Ok(0)
    } else {
        let pids: Vec<String> = report.survivors.iter().map(|p| p.pid.to_string()).collect();
        crate::term::emit(&format!(
            "CẢNH BÁO: còn tiến trình sót: {}",
            pids.join(", ")
        ));
        Ok(1)
    }
}

pub struct RunRequest {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub stderr_path: PathBuf,
    pub stream_path: PathBuf,
    pub timeout: Duration,
    pub kill_grace: Duration,
    /// true: the child's stdout is `stream_path` itself (never a pipe) and the runner tails it.
    pub stdout_file: bool,
    /// Secret values of THIS run's engine (ENG-05): scrubbed from every stdout line before it is
    /// written or shown, and from `stderr.txt` when the child is gone. Empty for an engine that
    /// carries no key, and then nothing is copied or rewritten.
    pub redact: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub elapsed: Duration,
}

pub trait RunObserver {
    fn on_spawn(&mut self, pid: i32, pgid: i32, pid_start: u64) -> BrainResult<()>;
    fn on_line(&mut self, line: &[u8]);
    fn on_notice(&mut self, message: &str);
    fn kill_requested(&mut self) -> bool;
}

struct TreeGuard {
    root: crate::procs::ProcId,
    armed: bool,
}

impl Drop for TreeGuard {
    fn drop(&mut self) {
        if self.armed {
            crate::procs::kill_tree(
                self.root,
                Duration::ZERO,
                &std::sync::atomic::AtomicBool::new(true),
            );
        }
    }
}

/// Rewrites `stderr.txt` without the run's secrets when the supervisor is left by any path that
/// did not reach the explicit rewrite (an error return, a panic): the child wrote that file
/// directly, so it is the one sink the line filter cannot cover.
struct StderrGuard<'a> {
    req: &'a RunRequest,
    armed: bool,
}

impl Drop for StderrGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            scrub_stderr_file(self.req);
        }
    }
}

/// Rewrites `stderr.txt` redacted. If the rewrite fails the file is removed: an absent log is
/// better than one that holds a key. Returns the failure text for the caller to report.
fn scrub_stderr_file(req: &RunRequest) -> Option<String> {
    match crate::secrets::redact_file(&req.stderr_path, &req.redact) {
        Ok(()) => None,
        Err(e) => {
            let _ = std::fs::remove_file(&req.stderr_path);
            Some(format!(
                "Không ẩn được key trong stderr.txt ({e}) — đã xóa file để không lưu key"
            ))
        }
    }
}

pub fn supervise(req: &RunRequest, obs: &mut dyn RunObserver) -> BrainResult<RunOutcome> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| BrainError::error(format!("Lỗi khởi tạo tokio runtime: {e}")))?;
    rt.block_on(supervise_async(req, obs))
}

async fn supervise_async(req: &RunRequest, obs: &mut dyn RunObserver) -> BrainResult<RunOutcome> {
    if req.argv.is_empty() {
        return Err(BrainError::error("argv rỗng"));
    }
    let argv0 = &req.argv[0];
    // The stdout file is written by the child itself, so no line passes through the filter. An
    // engine that needs a key must therefore never use that mode.
    if req.stdout_file && !req.redact.is_empty() {
        return Err(BrainError::error(
            "Engine ghi stdout ra file không được mang key — lỗi cấu hình engine",
        ));
    }

    let mut sigint = signal(SignalKind::interrupt())
        .map_err(|e| BrainError::error(format!("Lỗi đăng ký SIGINT: {e}")))?;
    let mut sigterm = signal(SignalKind::terminate())
        .map_err(|e| BrainError::error(format!("Lỗi đăng ký SIGTERM: {e}")))?;
    let mut sighup = signal(SignalKind::hangup())
        .map_err(|e| BrainError::error(format!("Lỗi đăng ký SIGHUP: {e}")))?;

    let stderr_file = std::fs::File::create(&req.stderr_path)?;
    let mut stderr_guard = StderrGuard { req, armed: true };

    let mut cmd = tokio::process::Command::new(argv0);
    cmd.args(&req.argv[1..]);
    cmd.current_dir(&req.cwd);
    cmd.env_clear();
    cmd.envs(&req.env);
    cmd.stdin(std::process::Stdio::null());
    if req.stdout_file {
        use std::os::unix::fs::OpenOptionsExt;
        let out = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&req.stream_path)?;
        cmd.stdout(out);
    } else {
        cmd.stdout(std::process::Stdio::piped());
    }
    cmd.stderr(stderr_file);
    cmd.process_group(0);
    cmd.kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Err(BrainError::error(format!(
                    "Không tìm thấy lệnh claude: {argv0}"
                )));
            }
            return Err(BrainError::error(format!("Không chạy được {argv0}: {e}")));
        }
    };

    let pid = child
        .id()
        .ok_or_else(|| BrainError::error("Không lấy được PID tiến trình con"))?
        as i32;
    let pgid = pid;

    let mut pid_start: u64 = 0;
    for _ in 0..20 {
        if let Some(st) = crate::procs::read_stat(pid) {
            pid_start = st.start;
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let root = crate::procs::ProcId {
        pid,
        start: pid_start,
    };
    let mut guard = TreeGuard { root, armed: true };

    if let Err(e) = obs.on_spawn(pid, pgid, pid_start) {
        let _ = child.start_kill();
        let _ = child.wait().await;
        return Err(e); // `stderr_guard` rewrites stderr.txt on the way out
    }

    // Pipe mode: the runner writes each line to the stream file. File mode: the child already
    // writes it, so the runner only tails it.
    let mut stream_file = if req.stdout_file {
        None
    } else {
        Some(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&req.stream_path)?,
        )
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let tail_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader_handle = if req.stdout_file {
        let path = req.stream_path.clone();
        let done = tail_done.clone();
        tokio::spawn(async move {
            use std::io::{Read, Seek, SeekFrom};
            let mut offset: u64 = 0;
            let mut pending: Vec<u8> = Vec::new();
            loop {
                let finished = done.load(std::sync::atomic::Ordering::Relaxed);
                if let Ok(mut f) = std::fs::File::open(&path) {
                    if f.seek(SeekFrom::Start(offset)).is_ok() {
                        let mut chunk = Vec::new();
                        if let Ok(n) = f.read_to_end(&mut chunk) {
                            offset += n as u64;
                            pending.extend_from_slice(&chunk);
                        }
                    }
                }
                while let Some(pos) = pending.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = pending.drain(..=pos).collect();
                    if tx.send(line).is_err() {
                        return;
                    }
                }
                if finished {
                    if !pending.is_empty() {
                        let _ = tx.send(std::mem::take(&mut pending));
                    }
                    return;
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        })
    } else {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| BrainError::error("Không lấy được stdout của tiến trình con"))?;
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf).await {
                    Ok(0) => break,
                    Ok(_) => {
                        if tx.send(buf.clone()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        })
    };

    #[derive(PartialEq, Eq)]
    enum StopReason {
        Signal,
        Timeout,
    }

    let start_time = std::time::Instant::now();
    let sleep_fut = tokio::time::sleep(req.timeout);
    tokio::pin!(sleep_fut);

    let mut stopping: Option<StopReason> = None;
    let force = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut kill_task: Option<tokio::task::JoinHandle<crate::procs::KillReport>> = None;
    let mut child_exit = None;

    loop {
        tokio::select! {
            line = rx.recv() => {
                match line {
                    Some(line) => {
                        let line = if req.redact.is_empty() {
                            line
                        } else {
                            crate::secrets::redact_line(&line, &req.redact)
                        };
                        if let Some(f) = stream_file.as_mut() {
                            use std::io::Write;
                            let _ = f.write_all(&line);
                            let _ = f.flush();
                        }
                        obs.on_line(&line);
                    }
                    None => {}
                }
            }
            status = child.wait(), if child_exit.is_none() => {
                child_exit = Some(status?);
                break;
            }
            _ = sigint.recv() => {
                if stopping.is_none() {
                    stopping = Some(StopReason::Signal);
                    obs.on_notice(&format!(
                        "Đang dừng run: SIGTERM nhóm tiến trình, chờ {}s (Ctrl+C lần nữa để dừng ngay)",
                        req.kill_grace.as_secs()
                    ));
                    let force_clone = force.clone();
                    let kill_grace = req.kill_grace;
                    kill_task = Some(tokio::task::spawn_blocking(move || {
                        crate::procs::kill_tree(root, kill_grace, &force_clone)
                    }));
                } else if !force.load(std::sync::atomic::Ordering::Relaxed) {
                    force.store(true, std::sync::atomic::Ordering::Relaxed);
                    obs.on_notice("Dừng ngay (SIGKILL)");
                }
            }
            _ = sigterm.recv() => {
                if stopping.is_none() {
                    stopping = Some(StopReason::Signal);
                    obs.on_notice(&format!(
                        "Đang dừng run: SIGTERM nhóm tiến trình, chờ {}s (Ctrl+C lần nữa để dừng ngay)",
                        req.kill_grace.as_secs()
                    ));
                    let force_clone = force.clone();
                    let kill_grace = req.kill_grace;
                    kill_task = Some(tokio::task::spawn_blocking(move || {
                        crate::procs::kill_tree(root, kill_grace, &force_clone)
                    }));
                }
            }
            _ = sighup.recv() => {
                if stopping.is_none() {
                    stopping = Some(StopReason::Signal);
                    obs.on_notice(&format!(
                        "Đang dừng run: SIGTERM nhóm tiến trình, chờ {}s (Ctrl+C lần nữa để dừng ngay)",
                        req.kill_grace.as_secs()
                    ));
                    let force_clone = force.clone();
                    let kill_grace = req.kill_grace;
                    kill_task = Some(tokio::task::spawn_blocking(move || {
                        crate::procs::kill_tree(root, kill_grace, &force_clone)
                    }));
                }
            }
            _ = &mut sleep_fut, if stopping.is_none() => {
                stopping = Some(StopReason::Timeout);
                obs.on_notice(&format!("Quá thời gian {}s — dừng run", req.timeout.as_secs()));
                let force_clone = force.clone();
                let kill_grace = req.kill_grace;
                kill_task = Some(tokio::task::spawn_blocking(move || {
                    crate::procs::kill_tree(root, kill_grace, &force_clone)
                }));
            }
        }
    }

    if child_exit.is_none() {
        child_exit = Some(child.wait().await?);
    }

    tail_done.store(true, std::sync::atomic::Ordering::Relaxed);
    let drain_deadline = tokio::time::sleep(Duration::from_secs(2));
    tokio::pin!(drain_deadline);
    loop {
        tokio::select! {
            line = rx.recv() => {
                match line {
                    Some(line) => {
                        let line = if req.redact.is_empty() {
                            line
                        } else {
                            crate::secrets::redact_line(&line, &req.redact)
                        };
                        if let Some(f) = stream_file.as_mut() {
                            use std::io::Write;
                            let _ = f.write_all(&line);
                            let _ = f.flush();
                        }
                        obs.on_line(&line);
                    }
                    None => {
                        break;
                    }
                }
            }
            _ = &mut drain_deadline => {
                break;
            }
        }
    }
    reader_handle.abort();

    if let Some(task) = kill_task {
        if let Ok(report) = task.await {
            if !report.survivors.is_empty() {
                let pids: Vec<String> =
                    report.survivors.iter().map(|p| p.pid.to_string()).collect();
                obs.on_notice(&format!(
                    "CẢNH BÁO: còn tiến trình sót: {}",
                    pids.join(", ")
                ));
            }
        }
    }
    guard.armed = false;

    // Every exit path (normal, signal, timeout, `agent-brain kill`) reaches this point with the
    // child tree gone, so stderr.txt is final and can be rewritten once.
    stderr_guard.armed = false;
    if let Some(msg) = scrub_stderr_file(req) {
        obs.on_notice(&msg);
    }

    let elapsed = start_time.elapsed();
    let exit_status = child_exit.unwrap();
    let exit_code = exit_status.code().or_else(|| {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            exit_status.signal().map(|s| 128 + s)
        }
        #[cfg(not(unix))]
        {
            None
        }
    });

    let status = match stopping {
        Some(StopReason::Signal) => RunStatus::Killed,
        Some(StopReason::Timeout) => RunStatus::TimedOut,
        None => {
            if obs.kill_requested() {
                RunStatus::Killed
            } else if exit_status.success() {
                RunStatus::Succeeded
            } else {
                RunStatus::Failed
            }
        }
    };

    Ok(RunOutcome {
        status,
        exit_code,
        elapsed,
    })
}
