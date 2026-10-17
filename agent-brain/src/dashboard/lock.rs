use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const LOCK_FILE: &str = "serve.lock";

#[derive(Debug)]
pub struct ServeLock {
    file: File,
    #[allow(dead_code)]
    path: PathBuf,
}

#[derive(Debug)]
pub enum LockError {
    Held { pid: Option<u32>, port: Option<u16> },
    Io(std::io::Error),
}

impl From<std::io::Error> for LockError {
    fn from(e: std::io::Error) -> Self {
        LockError::Io(e)
    }
}

pub fn acquire(dir: &Path) -> Result<ServeLock, LockError> {
    if !dir.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
                .map_err(LockError::Io)?;
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(dir).map_err(LockError::Io)?;
        }
    }

    let lock_path = dir.join(LOCK_FILE);

    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(&lock_path).map_err(LockError::Io)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&lock_path, std::fs::Permissions::from_mode(0o600));
    }

    #[cfg(unix)]
    {
        use std::io::{Seek, SeekFrom};
        use std::os::unix::io::AsRawFd;

        let res = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if res != 0 {
            let err = std::io::Error::last_os_error();
            let raw_os = err.raw_os_error();
            if raw_os == Some(libc::EWOULDBLOCK) || raw_os == Some(libc::EAGAIN) {
                let _ = file.seek(SeekFrom::Start(0));
                let mut content = String::new();
                let _ = file.read_to_string(&mut content);
                let (pid, port) =
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        let pid = val.get("pid").and_then(|v| v.as_u64()).map(|n| n as u32);
                        let port = val.get("port").and_then(|v| v.as_u64()).map(|n| n as u16);
                        (pid, port)
                    } else {
                        (None, None)
                    };
                return Err(LockError::Held { pid, port });
            } else {
                return Err(LockError::Io(err));
            }
        }
    }

    Ok(ServeLock {
        file,
        path: lock_path,
    })
}

impl ServeLock {
    pub fn write_info(&mut self, pid: u32, port: u16, started_at_ms: i64) -> std::io::Result<()> {
        use std::io::{Seek, SeekFrom};
        self.file.set_len(0)?;
        self.file.seek(SeekFrom::Start(0))?;
        let payload = serde_json::json!({
            "pid": pid,
            "port": port,
            "started_at": started_at_ms,
        });
        serde_json::to_writer(&mut self.file, &payload)?;
        use std::io::Write;
        self.file.flush()?;
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for ServeLock {
    fn drop(&mut self) {
        use std::os::unix::io::AsRawFd;
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_lifecycle() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let dir_path = dir.path();

        let mut lock1 = acquire(dir_path).expect("acquire succeeds");

        // Second acquire should return Held with (None, None) before write_info
        match acquire(dir_path) {
            Err(LockError::Held { pid, port }) => {
                assert_eq!(pid, None);
                assert_eq!(port, None);
            }
            other => panic!("expected LockError::Held, got {:?}", other),
        }

        lock1
            .write_info(54321, 7437, 1700000000000)
            .expect("write_info");

        // Second acquire now returns Held with pid and port
        match acquire(dir_path) {
            Err(LockError::Held { pid, port }) => {
                assert_eq!(pid, Some(54321));
                assert_eq!(port, Some(7437));
            }
            other => panic!("expected LockError::Held with info, got {:?}", other),
        }

        // Check file mode and JSON keys
        let lock_path = dir_path.join(LOCK_FILE);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&lock_path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        let content = std::fs::read_to_string(&lock_path).expect("read lock file");
        let val: serde_json::Value = serde_json::from_str(&content).expect("parse json");
        let obj = val.as_object().expect("json object");
        assert_eq!(obj.len(), 3);
        assert_eq!(obj.get("pid").and_then(|v| v.as_u64()), Some(54321));
        assert_eq!(obj.get("port").and_then(|v| v.as_u64()), Some(7437));
        assert_eq!(
            obj.get("started_at").and_then(|v| v.as_i64()),
            Some(1700000000000)
        );

        // After dropping lock1, acquire succeeds again
        drop(lock1);
        let _lock2 = acquire(dir_path).expect("acquire succeeds again after drop");
    }
}
