use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};
use rusqlite::Connection;

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
