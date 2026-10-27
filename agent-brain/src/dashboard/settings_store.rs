//! Ghi an toàn teams.json / workflows.json / staff.json từ dashboard (D-10, SET-02..04).
//! Một khóa ghi cho cả tiến trình, so phiên bản (version) ngay trước khi ghi,
//! ghi file tạm 0600 rồi rename và fsync cả thư mục.

use crate::error::{BrainError, BrainResult};
use crate::teams::FieldError;
use std::hash::{DefaultHasher, Hasher};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

pub const VERSION_ABSENT: &str = "absent";

static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// `absent` for a missing file, else 16 lowercase hex digits of the bytes' hash.
/// Tokens are only compared inside one serve process (like `task_rev`), so the
/// std hasher is enough.
pub fn file_version(bytes: Option<&[u8]>) -> String {
    match bytes {
        None => VERSION_ABSENT.to_string(),
        Some(b) => {
            let mut hasher = DefaultHasher::new();
            hasher.write(b);
            format!("{:016x}", hasher.finish())
        }
    }
}

/// `(text, version)`; a missing file reads as `(None, "absent")`.
pub fn read_doc(path: &Path) -> BrainResult<(Option<String>, String)> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((None, VERSION_ABSENT.to_string()));
        }
        Err(e) => {
            return Err(BrainError::error(format!(
                "Không đọc được {}: {e}",
                path.display()
            )));
        }
    };
    let version = file_version(Some(&bytes));
    match String::from_utf8(bytes) {
        Ok(text) => Ok((Some(text), version)),
        Err(_) => Err(BrainError::error(format!(
            "{} không phải văn bản UTF-8",
            path.display()
        ))),
    }
}

#[derive(Debug)]
pub enum WriteError {
    /// The file changed since the caller read it.
    Conflict,
    /// `mutate` refused the change.
    Refused(Vec<FieldError>),
    Io(String),
}

fn tmp_name_for(path: &Path) -> String {
    let stem = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "settings".to_string());
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(".{stem}_{}_{nanos}.tmp", std::process::id())
}

/// Temp file in the same directory (mode 0600, `create_new`), write, flush, fsync,
/// rename, then fsync the directory. The temp file is removed on any failure.
fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    if !parent.exists() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Không thể tạo thư mục {}: {e}", parent.display()))?;
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }

    let tmp_path = parent.join(tmp_name_for(path));

    let write_res = (|| -> std::io::Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true).mode(0o600);
        let mut f = opts.open(&tmp_path)?;
        f.write_all(data)?;
        f.flush()?;
        f.sync_all()?;
        Ok(())
    })();
    if let Err(e) = write_res {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!("Không thể ghi {}: {e}", tmp_path.display()));
    }

    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!(
            "Không thể đổi tên {} thành {}: {e}",
            tmp_path.display(),
            path.display()
        ));
    }

    std::fs::File::open(parent)
        .and_then(|d| d.sync_all())
        .map_err(|e| format!("Không thể fsync thư mục {}: {e}", parent.display()))?;
    Ok(())
}

/// Read-modify-write of a JSON object file under the process-wide write lock.
/// The file is re-read here; a version that differs from `expected_version` is a
/// `Conflict` and nothing is written. `absent` starts from an empty object. Callers
/// refuse files that fail to load before calling this. Returns the new version.
pub fn write_object(
    path: &Path,
    expected_version: &str,
    mutate: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>) -> Result<(), Vec<FieldError>>,
) -> Result<String, WriteError> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    let bytes = match std::fs::read(path) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(WriteError::Io(format!(
                "Không đọc được {}: {e}",
                path.display()
            )));
        }
    };
    if file_version(bytes.as_deref()) != expected_version {
        return Err(WriteError::Conflict);
    }

    let mut map = match bytes {
        None => serde_json::Map::new(),
        Some(b) => match serde_json::from_slice::<serde_json::Value>(&b) {
            Ok(serde_json::Value::Object(m)) => m,
            Ok(_) => {
                return Err(WriteError::Io(format!(
                    "{} không phải object",
                    path.display()
                )))
            }
            Err(e) => {
                return Err(WriteError::Io(format!(
                    "{} không phải JSON hợp lệ: {e}",
                    path.display()
                )))
            }
        },
    };

    mutate(&mut map).map_err(WriteError::Refused)?;

    let mut out = serde_json::to_string_pretty(&serde_json::Value::Object(map))
        .map_err(|e| WriteError::Io(format!("Lỗi serialize JSON: {e}")))?;
    out.push('\n');

    atomic_write(path, out.as_bytes()).map_err(WriteError::Io)?;
    Ok(file_version(Some(out.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn version_absent_and_hash() {
        assert_eq!(file_version(None), "absent");
        assert_eq!(file_version(None), VERSION_ABSENT);
        let a = file_version(Some(b"{}"));
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
        assert_eq!(a, file_version(Some(b"{}")));
        assert_ne!(a, file_version(Some(b"{ }")));
        // An empty file is a file, not an absent one.
        assert_ne!(file_version(Some(b"")), VERSION_ABSENT);

        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.json");
        assert_eq!(read_doc(&p).unwrap(), (None, "absent".to_string()));
        std::fs::write(&p, "{}").unwrap();
        assert_eq!(read_doc(&p).unwrap(), (Some("{}".to_string()), a));
    }

    #[test]
    fn write_conflict_detected() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("teams.json");

        let v1 = write_object(&p, "absent", |m| {
            m.insert("a".to_string(), serde_json::json!(1));
            Ok(())
        })
        .unwrap();
        assert_ne!(v1, "absent");

        // A stale token is refused and the file is unchanged.
        let before = std::fs::read(&p).unwrap();
        let res = write_object(&p, "absent", |m| {
            m.insert("b".to_string(), serde_json::json!(2));
            Ok(())
        });
        assert!(matches!(res, Err(WriteError::Conflict)), "{res:?}");
        assert_eq!(std::fs::read(&p).unwrap(), before);

        // The current token works and keeps key order.
        let v2 = write_object(&p, &v1, |m| {
            m.insert("z".to_string(), serde_json::json!(3));
            m.insert("a".to_string(), serde_json::json!(4));
            Ok(())
        })
        .unwrap();
        assert_eq!(read_doc(&p).unwrap().1, v2);
        let parsed: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["a", "z"]);
        assert_eq!(parsed["a"], 4);

        // A refusal writes nothing.
        let before = std::fs::read(&p).unwrap();
        let res = write_object(&p, &v2, |_| {
            Err(vec![FieldError {
                field: "name".to_string(),
                message: "không".to_string(),
            }])
        });
        match res {
            Err(WriteError::Refused(errs)) => assert_eq!(errs[0].message, "không"),
            other => panic!("expected Refused, got {other:?}"),
        }
        assert_eq!(std::fs::read(&p).unwrap(), before);

        // A file that is not an object is an Io error, never overwritten.
        std::fs::write(&p, "[]").unwrap();
        let v3 = read_doc(&p).unwrap().1;
        let res = write_object(&p, &v3, |_| Ok(()));
        assert!(matches!(res, Err(WriteError::Io(_))), "{res:?}");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "[]");
    }

    #[test]
    fn write_creates_0600_and_no_tmp() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("cfg").join("agent-brain");
        let p = nested.join("teams.json");

        write_object(&p, "absent", |m| {
            m.insert("doi".to_string(), serde_json::json!({"description": "d"}));
            Ok(())
        })
        .unwrap();

        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let dir_mode = std::fs::metadata(&nested).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.ends_with("}\n"));
        let names: Vec<String> = std::fs::read_dir(&nested)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, ["teams.json"]);
    }
}
