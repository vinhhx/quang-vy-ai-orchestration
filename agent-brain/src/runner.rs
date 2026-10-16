use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

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

pub fn cmd_kill(_ctx: &mut Ctx, _task_id: i64) -> BrainResult<i32> {
    Err(BrainError::not_available("kill"))
}

pub struct RunRequest {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub stderr_path: PathBuf,
    pub stream_path: PathBuf,
    pub timeout: Duration,
    pub kill_grace: Duration,
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

    let stderr_file = std::fs::File::create(&req.stderr_path)?;

    let mut cmd = tokio::process::Command::new(argv0);
    cmd.args(&req.argv[1..]);
    cmd.current_dir(&req.cwd);
    cmd.env_clear();
    cmd.envs(&req.env);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
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

    if let Err(e) = obs.on_spawn(pid, pgid, pid_start) {
        let _ = child.start_kill();
        let _ = child.wait().await;
        return Err(e);
    }

    let mut stream_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&req.stream_path)?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| BrainError::error("Không lấy được stdout của tiến trình con"))?;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let reader_handle = tokio::spawn(async move {
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
    });

    let start_time = std::time::Instant::now();
    let sleep_fut = tokio::time::sleep(req.timeout);
    tokio::pin!(sleep_fut);
    let mut timed_out = false;
    let mut child_exit = None;

    loop {
        tokio::select! {
            Some(line) = rx.recv() => {
                use std::io::Write;
                let _ = stream_file.write_all(&line);
                let _ = stream_file.flush();
                obs.on_line(&line);
            }
            status = child.wait(), if child_exit.is_none() => {
                child_exit = Some(status?);
                break;
            }
            _ = &mut sleep_fut, if !timed_out => {
                timed_out = true;
                obs.on_notice(&format!("Quá thời gian {}s — dừng run", req.timeout.as_secs()));
                let _ = child.start_kill();
            }
        }
    }

    if child_exit.is_none() {
        child_exit = Some(child.wait().await?);
    }

    let drain_deadline = tokio::time::sleep(Duration::from_secs(2));
    tokio::pin!(drain_deadline);
    loop {
        tokio::select! {
            Some(line) = rx.recv() => {
                use std::io::Write;
                let _ = stream_file.write_all(&line);
                let _ = stream_file.flush();
                obs.on_line(&line);
            }
            _ = &mut drain_deadline => {
                break;
            }
            else => break,
        }
    }
    reader_handle.abort();

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

    let status = if timed_out {
        RunStatus::TimedOut
    } else if exit_status.success() {
        RunStatus::Succeeded
    } else if obs.kill_requested() {
        RunStatus::Killed
    } else {
        RunStatus::Failed
    };

    Ok(RunOutcome {
        status,
        exit_code,
        elapsed,
    })
}
