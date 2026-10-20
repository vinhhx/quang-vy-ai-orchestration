use crate::error::{BrainError, BrainResult};
use std::collections::{HashMap, VecDeque};
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const OUTPUT_LINES: usize = 200;

#[derive(Clone, Debug)]
pub struct JobSpec {
    pub role: String,
    pub project: String,
    pub profile: String,
    pub branch: Option<String>,
    pub task_id: Option<i64>,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct JobView {
    pub job_id: u64,
    pub role: String,
    pub project: String,
    pub profile: String,
    pub branch: Option<String>,
    pub task_id: Option<i64>,
    pub pid: i32,
    pub state: String, /* running | exited */
    pub exit_code: Option<i32>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub output: Vec<String>,
}

struct InnerJob {
    view: JobView,
    proc_id: Option<crate::procs::ProcId>,
    output_ring: Arc<Mutex<VecDeque<String>>>,
}

struct ManagerState {
    next_id: u64,
    shutting_down: bool,
    reservations: HashMap<String, String>, // project -> profile
    jobs: HashMap<u64, InnerJob>,
}

struct SpawnRequest {
    args: Vec<String>,
    reply: Sender<std::io::Result<std::process::Child>>,
}

pub struct JobManager {
    exe: PathBuf,
    spawn_tx: Sender<SpawnRequest>,
    state: Arc<Mutex<ManagerState>>,
}

impl JobManager {
    pub fn new(exe: PathBuf) -> Arc<JobManager> {
        let (spawn_tx, spawn_rx) = channel::<SpawnRequest>();
        let exe_clone = exe.clone();

        std::thread::Builder::new()
            .name("ab-spawner".to_string())
            .spawn(move || {
                for req in spawn_rx {
                    let mut cmd = std::process::Command::new(&exe_clone);
                    cmd.args(&req.args);
                    cmd.stdin(std::process::Stdio::null());
                    cmd.stdout(std::process::Stdio::piped());
                    cmd.stderr(std::process::Stdio::piped());

                    #[cfg(unix)]
                    {
                        use std::os::unix::process::CommandExt;
                        cmd.process_group(0);
                        let server_pid = unsafe { libc::getpid() };
                        unsafe {
                            cmd.pre_exec(move || {
                                if libc::prctl(
                                    libc::PR_SET_PDEATHSIG,
                                    libc::SIGTERM as libc::c_ulong,
                                ) != 0
                                {
                                    return Err(std::io::Error::last_os_error());
                                }
                                if libc::getppid() != server_pid {
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::Other,
                                        "server exited before child exec",
                                    ));
                                }
                                Ok(())
                            });
                        }
                    }

                    let res = cmd.spawn();
                    let _ = req.reply.send(res);
                }
            })
            .expect("spawn ab-spawner thread");

        let state = Arc::new(Mutex::new(ManagerState {
            next_id: 1,
            shutting_down: false,
            reservations: HashMap::new(),
            jobs: HashMap::new(),
        }));

        Arc::new(JobManager {
            exe,
            spawn_tx,
            state,
        })
    }

    pub fn start(&self, spec: JobSpec) -> BrainResult<u64> {
        let (job_id, reply_rx) = {
            let mut state = self.state.lock().unwrap();
            if state.shutting_down {
                return Err(BrainError::error(
                    "Dashboard đang dừng — không nhận việc mới",
                ));
            }
            if state.reservations.contains_key(&spec.project) {
                return Err(BrainError::error(format!(
                    "Dự án {} đang có việc vừa giao từ dashboard — chờ run xong",
                    spec.project
                )));
            }

            let job_id = state.next_id;
            state.next_id += 1;
            state
                .reservations
                .insert(spec.project.clone(), spec.profile.clone());

            let (reply_tx, reply_rx) = channel();
            let req = SpawnRequest {
                args: spec.args.clone(),
                reply: reply_tx,
            };

            if let Err(e) = self.spawn_tx.send(req) {
                state.reservations.remove(&spec.project);
                return Err(BrainError::error(format!(
                    "Lỗi gửi yêu cầu tới spawner: {e}"
                )));
            }
            (job_id, reply_rx)
        };

        let spawn_res = reply_rx
            .recv()
            .map_err(|e| BrainError::error(format!("Spawner không phản hồi: {e}")))?;

        let mut child = match spawn_res {
            Ok(c) => c,
            Err(e) => {
                let mut state = self.state.lock().unwrap();
                state.reservations.remove(&spec.project);
                if e.kind() == std::io::ErrorKind::NotFound
                    || self.exe.to_string_lossy().ends_with(" (deleted)")
                {
                    return Err(BrainError::error(
                        "Binary đã bị thay thế — khởi động lại agent-brain serve",
                    ));
                }
                return Err(BrainError::error(format!(
                    "Không chạy được agent-brain: {e}"
                )));
            }
        };

        let pid = child.id() as i32;
        let mut proc_id = crate::procs::proc_id(pid);
        if proc_id.is_none() {
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_millis(200) {
                if let Some(p) = crate::procs::proc_id(pid) {
                    proc_id = Some(p);
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        let output_ring = Arc::new(Mutex::new(VecDeque::with_capacity(OUTPUT_LINES)));
        let started_at = crate::db::now_ms();

        let view = JobView {
            job_id,
            role: spec.role.clone(),
            project: spec.project.clone(),
            profile: spec.profile.clone(),
            branch: spec.branch.clone(),
            task_id: spec.task_id,
            pid,
            state: "running".to_string(),
            exit_code: None,
            started_at,
            ended_at: None,
            output: Vec::new(),
        };

        {
            let mut state = self.state.lock().unwrap();
            state.jobs.insert(
                job_id,
                InnerJob {
                    view,
                    proc_id,
                    output_ring: output_ring.clone(),
                },
            );
        }

        let child_stdout = child.stdout.take();
        let child_stderr = child.stderr.take();
        let ring_out = output_ring.clone();
        let ring_err = output_ring.clone();

        // D-20: dòng `Autopilot …` của job cũng hiện ở terminal của serve.
        let t_out = std::thread::spawn(move || {
            if let Some(stdout) = child_stdout {
                let reader = std::io::BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    let echo = line
                        .starts_with("Autopilot ")
                        .then(|| format!("[job {job_id}] {line}"));
                    {
                        let mut b = ring_out.lock().unwrap();
                        if b.len() >= OUTPUT_LINES {
                            b.pop_front();
                        }
                        b.push_back(line);
                    }
                    if let Some(e) = echo {
                        crate::term::emit(&e);
                    }
                }
            }
        });

        let t_err = std::thread::spawn(move || {
            if let Some(stderr) = child_stderr {
                let reader = std::io::BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    let echo = line
                        .starts_with("Autopilot ")
                        .then(|| format!("[job {job_id}] {line}"));
                    {
                        let mut b = ring_err.lock().unwrap();
                        if b.len() >= OUTPUT_LINES {
                            b.pop_front();
                        }
                        b.push_back(line);
                    }
                    if let Some(e) = echo {
                        crate::term::emit(&e);
                    }
                }
            }
        });

        let state_clone = self.state.clone();
        let project_name = spec.project.clone();
        std::thread::spawn(move || {
            let status = child.wait();
            let _ = t_out.join();
            let _ = t_err.join();

            let exit_code = match status {
                Ok(st) => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        if let Some(c) = st.code() {
                            Some(c)
                        } else if let Some(sig) = st.signal() {
                            Some(128 + sig)
                        } else {
                            None
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        st.code()
                    }
                }
                Err(_) => None,
            };

            let ended_at = crate::db::now_ms();
            let final_output: Vec<String> = {
                let ring = output_ring.lock().unwrap();
                ring.iter().cloned().collect()
            };

            let mut state = state_clone.lock().unwrap();
            state.reservations.remove(&project_name);
            if let Some(job) = state.jobs.get_mut(&job_id) {
                job.view.state = "exited".to_string();
                job.view.exit_code = exit_code;
                job.view.ended_at = Some(ended_at);
                job.view.output = final_output;
            }
        });

        Ok(job_id)
    }

    pub fn get(&self, job_id: u64) -> Option<JobView> {
        let state = self.state.lock().unwrap();
        let job = state.jobs.get(&job_id)?;
        let mut view = job.view.clone();
        if view.state == "running" {
            let ring = job.output_ring.lock().unwrap();
            view.output = ring.iter().cloned().collect();
        }
        Some(view)
    }

    pub fn list(&self) -> Vec<JobView> {
        let state = self.state.lock().unwrap();
        let mut keys: Vec<u64> = state.jobs.keys().copied().collect();
        keys.sort_unstable();
        keys.into_iter()
            .filter_map(|id| {
                let job = state.jobs.get(&id)?;
                let mut view = job.view.clone();
                if view.state == "running" {
                    let ring = job.output_ring.lock().unwrap();
                    view.output = ring.iter().cloned().collect();
                }
                Some(view)
            })
            .collect()
    }

    pub fn reserved(&self) -> Vec<(String, String)> {
        let state = self.state.lock().unwrap();
        state
            .reservations
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn live_pids(&self) -> Vec<crate::procs::ProcId> {
        let state = self.state.lock().unwrap();
        state
            .jobs
            .values()
            .filter(|j| j.view.state == "running")
            .filter_map(|j| j.proc_id)
            .filter(|p| crate::procs::alive(*p))
            .collect()
    }

    pub fn shutdown(&self, grace: Duration) {
        {
            let mut state = self.state.lock().unwrap();
            state.shutting_down = true;
        }

        let pids = self.live_pids();
        for p in &pids {
            crate::procs::signal_proc(*p, libc::SIGINT);
        }

        let start = std::time::Instant::now();
        while start.elapsed() < grace {
            if !self.live_pids().iter().any(|p| crate::procs::alive(*p)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        for p in self.live_pids() {
            crate::procs::kill_tree(p, Duration::ZERO, &std::sync::atomic::AtomicBool::new(true));
        }
    }

    pub fn kill_all_now(&self) {
        {
            let mut state = self.state.lock().unwrap();
            state.shutting_down = true;
        }
        for p in self.live_pids() {
            crate::procs::kill_tree(p, Duration::ZERO, &std::sync::atomic::AtomicBool::new(true));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_jobs_manager_basic() {
        let mgr = JobManager::new(PathBuf::from("/bin/sh"));
        let spec = JobSpec {
            role: "test".into(),
            project: "p1".into(),
            profile: "work".into(),
            branch: None,
            task_id: None,
            args: vec!["-c".into(), "echo a; echo b 1>&2; exit 3".into()],
        };
        let job_id = mgr.start(spec).expect("start job");
        let start = Instant::now();
        let mut job = None;
        while start.elapsed() < Duration::from_secs(10) {
            if let Some(j) = mgr.get(job_id) {
                if j.state == "exited" {
                    job = Some(j);
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let j = job.expect("job should have exited");
        assert_eq!(j.exit_code, Some(3));
        assert!(j.output.contains(&"a".to_string()));
        assert!(j.output.contains(&"b".to_string()));
        assert_eq!(j.state, "exited");
        assert!(!mgr.reserved().iter().any(|(p, _)| p == "p1"));
    }

    #[test]
    fn test_jobs_manager_reservation_and_shutdown() {
        let mgr = JobManager::new(PathBuf::from("/bin/sh"));
        let spec1 = JobSpec {
            role: "test".into(),
            project: "p2".into(),
            profile: "work".into(),
            branch: None,
            task_id: None,
            args: vec![
                "-c".into(),
                "exec python3 -c 'import time; time.sleep(30)'".into(),
            ],
        };
        let _job_id = mgr.start(spec1).expect("start job 1");

        let spec2 = JobSpec {
            role: "test".into(),
            project: "p2".into(),
            profile: "work".into(),
            branch: None,
            task_id: None,
            args: vec!["-c".into(), "echo fail".into()],
        };
        let err = mgr.start(spec2).unwrap_err();
        assert!(err
            .message
            .contains("Dự án p2 đang có việc vừa giao từ dashboard"));

        mgr.shutdown(Duration::from_secs(1));
        assert!(mgr.live_pids().is_empty());
    }
}
