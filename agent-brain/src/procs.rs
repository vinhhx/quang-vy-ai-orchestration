use std::sync::atomic::AtomicBool;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct ProcStat {
    pub state: char,
    pub ppid: i32,
    pub pgrp: i32,
    pub start: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcId {
    pub pid: i32,
    pub start: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KillReport {
    pub recorded: Vec<ProcId>,
    pub survivors: Vec<ProcId>,
    pub escalated: bool,
    pub elapsed: Duration,
}

pub fn read_stat(pid: i32) -> Option<ProcStat> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rp = s.rfind(')')?; // comm may contain ')' and spaces
    let f: Vec<&str> = s.get(rp + 2..)?.split_ascii_whitespace().collect();
    // f[0]=state(field 3), f[1]=ppid(field 4), f[2]=pgrp(field 5), f[19]=starttime(field 22)
    Some(ProcStat {
        state: f.first()?.chars().next()?,
        ppid: f.get(1)?.parse().ok()?,
        pgrp: f.get(2)?.parse().ok()?,
        start: f.get(19)?.parse().ok()?,
    })
}

pub fn proc_id(pid: i32) -> Option<ProcId> {
    let s = read_stat(pid)?;
    Some(ProcId {
        pid,
        start: s.start,
    })
}

pub fn alive(p: ProcId) -> bool {
    matches!(read_stat(p.pid), Some(s) if s.start == p.start && s.state != 'Z' && s.state != 'X')
}

pub fn descendants(_root_pid: i32) -> Vec<ProcId> {
    Vec::new()
}

pub fn signal_group(_root: ProcId, _sig: i32) -> bool {
    false
}

pub fn signal_proc(_p: ProcId, _sig: i32) -> bool {
    false
}

pub fn kill_tree(_root: ProcId, _grace: Duration, _force: &AtomicBool) -> KillReport {
    KillReport {
        recorded: Vec::new(),
        survivors: Vec::new(),
        escalated: false,
        elapsed: Duration::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn test_read_stat_getpid() {
        let pid = unsafe { libc::getpid() };
        let stat = read_stat(pid).expect("read_stat self");
        assert!(stat.state == 'R' || stat.state == 'S');
        assert!(stat.start > 0);
    }

    #[test]
    fn test_read_stat_weird_comm() {
        let tmp = tempfile::tempdir().unwrap();
        let weird = tmp.path().join("we) ird");
        std::fs::copy("/bin/sleep", &weird).unwrap();
        let mut perms = std::fs::metadata(&weird).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o755);
            std::fs::set_permissions(&weird, perms).unwrap();
        }
        let mut child = std::process::Command::new(&weird)
            .arg("10")
            .spawn()
            .unwrap();
        let pid = child.id() as i32;
        let stat = read_stat(pid).expect("read_stat weird");
        assert_eq!(stat.ppid, unsafe { libc::getpid() });
        assert_eq!(stat.pgrp, unsafe { libc::getpgrp() });
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn test_descendants_bash_sleep() {
        let mut child = std::process::Command::new("bash")
            .arg("-c")
            .arg("sleep 30 & wait")
            .spawn()
            .unwrap();
        let pid = child.id() as i32;
        let mut found_sleep = false;
        for _ in 0..40 {
            let descs = descendants(pid);
            for d in &descs {
                if let Ok(comm) = std::fs::read_to_string(format!("/proc/{}/comm", d.pid)) {
                    if comm.trim() == "sleep" {
                        found_sleep = true;
                        break;
                    }
                }
            }
            if found_sleep {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let descs = descendants(pid);
        for d in descs {
            unsafe {
                libc::kill(d.pid, libc::SIGKILL);
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(found_sleep, "descendants did not find sleep process");
    }

    #[test]
    fn test_signal_group_refuses_foreign_pgrp() {
        let mut child = std::process::Command::new("sleep")
            .arg("10")
            .spawn()
            .unwrap();
        let p = proc_id(child.id() as i32).unwrap();
        let refused = !signal_group(p, libc::SIGTERM);
        assert!(refused, "signal_group should refuse foreign group");
        assert!(alive(p), "child should still be alive");
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn test_signal_proc_refuses_wrong_start_time() {
        let mut child = std::process::Command::new("sleep")
            .arg("10")
            .spawn()
            .unwrap();
        let p = proc_id(child.id() as i32).unwrap();
        let fake_p = ProcId {
            pid: p.pid,
            start: p.start + 99999,
        };
        let refused = !signal_proc(fake_p, libc::SIGTERM);
        assert!(refused, "signal_proc should refuse mismatching start time");
        assert!(alive(p), "child should still be alive");
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn test_kill_tree_ignore_mode() {
        let fake_script = repo_root()
            .join("tests")
            .join("fakes")
            .join("fake_claude_tree.py");
        let tmp = tempfile::tempdir().unwrap();
        let pidfile = tmp.path().join("tree.pid");

        let mut cmd = std::process::Command::new("python3");
        cmd.arg(&fake_script);
        cmd.env("FAKE_TREE_PIDFILE", &pidfile);
        cmd.env("FAKE_TREE_MODE", "ignore");
        cmd.env("FAKE_TREE_SLEEP", "317");
        cmd.stdout(std::process::Stdio::piped());
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }
        let mut child = cmd.spawn().expect("spawn fake_claude_tree");
        let root = proc_id(child.id() as i32).expect("proc_id root");

        // Wait for pidfile
        for _ in 0..50 {
            if pidfile.is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(pidfile.is_file(), "pidfile should exist");

        let force = AtomicBool::new(false);
        let report = kill_tree(root, Duration::from_millis(500), &force);
        let _ = child.wait();

        assert!(report.escalated, "ignore mode should escalate to SIGKILL");
        assert!(
            report.survivors.is_empty(),
            "expected no survivors, got {:?}",
            report.survivors
        );
    }

    #[test]
    fn test_kill_tree_coop_mode() {
        let fake_script = repo_root()
            .join("tests")
            .join("fakes")
            .join("fake_claude_tree.py");
        let tmp = tempfile::tempdir().unwrap();
        let pidfile = tmp.path().join("tree.pid");

        let mut cmd = std::process::Command::new("python3");
        cmd.arg(&fake_script);
        cmd.env("FAKE_TREE_PIDFILE", &pidfile);
        cmd.env("FAKE_TREE_MODE", "coop");
        cmd.env("FAKE_TREE_SLEEP", "317");
        cmd.stdout(std::process::Stdio::piped());
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }
        let mut child = cmd.spawn().expect("spawn fake_claude_tree");
        let root = proc_id(child.id() as i32).expect("proc_id root");

        // Wait for pidfile
        for _ in 0..50 {
            if pidfile.is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(pidfile.is_file(), "pidfile should exist");

        let force = AtomicBool::new(false);
        let report = kill_tree(root, Duration::from_secs(2), &force);
        let _ = child.wait();

        assert!(
            !report.escalated,
            "coop mode should not escalate to SIGKILL"
        );
        assert!(
            report.survivors.is_empty(),
            "expected no survivors, got {:?}",
            report.survivors
        );
    }
}
