//! `~/.config/agent-brain/telegram.json`: bot token + paired chat ID (D-08).
//!
//! The file must be 0600. `load` refuses a group/world-readable file and never
//! fixes the mode silently. No message built here ever contains the token or
//! the file content.

use crate::error::{BrainError, BrainResult};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const FILE_NAME: &str = "telegram.json";

pub fn path() -> PathBuf {
    crate::config::config_dir().join(FILE_NAME)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub token: String,
    pub chat_id: i64,
    pub paired_at: i64,
}

impl std::fmt::Debug for TelegramConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelegramConfig")
            .field("token", &"<ẩn>")
            .field("chat_id", &self.chat_id)
            .field("paired_at", &self.paired_at)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    Missing,
    Insecure(String),
    Invalid(String),
    Io(String),
}

impl LoadError {
    /// User-facing Vietnamese text; `None` for `Missing` (no file = bot off, silent).
    pub fn message(&self) -> Option<String> {
        match self {
            LoadError::Missing => None,
            LoadError::Insecure(s) | LoadError::Invalid(s) | LoadError::Io(s) => Some(s.clone()),
        }
    }
}

/// `<digits>:<chars>` with chars from `[A-Za-z0-9_-]`.
pub fn token_shape_ok(token: &str) -> bool {
    let Some((id, secret)) = token.split_once(':') else {
        return false;
    };
    !id.is_empty()
        && id.bytes().all(|b| b.is_ascii_digit())
        && !secret.is_empty()
        && secret
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn load(path: &Path) -> Result<TelegramConfig, LoadError> {
    let shown = path.display().to_string();
    let invalid = || {
        LoadError::Invalid(format!(
            "{shown} không hợp lệ: thiếu hoặc sai khóa token/chat_id"
        ))
    };

    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(LoadError::Missing),
        Err(e) => return Err(LoadError::Io(format!("Không đọc được {shown}: {e}"))),
    };
    if !meta.is_file() {
        return Err(invalid());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(LoadError::Insecure(format!(
                "{shown} có quyền đọc cho nhóm/người khác — chạy: chmod 600 {shown}"
            )));
        }
    }

    let raw = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return Err(LoadError::Io(format!("Không đọc được {shown}: {e}"))),
    };
    // Never echo serde's error: it can quote parts of the file.
    let v: serde_json::Value = serde_json::from_slice(&raw).map_err(|_| invalid())?;
    let token = v
        .get("token")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    if token.is_empty() || !token_shape_ok(&token) {
        return Err(invalid());
    }
    let chat_id = match v.get("chat_id").and_then(|c| c.as_i64()) {
        Some(c) if c != 0 => c,
        _ => return Err(invalid()),
    };
    let paired_at = v.get("paired_at").and_then(|p| p.as_i64()).unwrap_or(0);
    Ok(TelegramConfig {
        token,
        chat_id,
        paired_at,
    })
}

/// Atomic write: 0600 temp file (never briefly world-readable), fsync, rename.
pub fn save(path: &Path, cfg: &TelegramConfig) -> BrainResult<()> {
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    if !parent.exists() {
        let mut b = std::fs::DirBuilder::new();
        b.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        b.create(&parent).map_err(|e| {
            BrainError::error(format!("Không thể tạo thư mục {}: {e}", parent.display()))
        })?;
    }

    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_path = parent.join(format!("telegram_{pid}_{nanos}.tmp"));

    let write_res = (|| -> std::io::Result<()> {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp_path)?;
        let json_str = serde_json::to_string_pretty(cfg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        f.write_all(json_str.as_bytes())?;
        f.write_all(b"\n")?;
        f.flush()?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(BrainError::error(format!(
            "Không thể ghi {}: {e}",
            tmp_path.display()
        )));
    }
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(BrainError::error(format!(
            "Không thể ghi {}: {e}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const TOKEN: &str = "4242:fake-token-for-tests";

    fn tmpdir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let d =
            std::env::temp_dir().join(format!("ab-tgcfg-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(dir: &Path, text: &str, mode: u32) -> PathBuf {
        let p = dir.join(FILE_NAME);
        std::fs::write(&p, text).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
        p
    }

    #[test]
    fn missing_file_is_missing() {
        let d = tmpdir("missing");
        assert_eq!(load(&d.join(FILE_NAME)).unwrap_err(), LoadError::Missing);
        assert_eq!(LoadError::Missing.message(), None);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn group_readable_file_is_refused_not_fixed() {
        let d = tmpdir("insecure");
        let p = write(
            &d,
            &format!(r#"{{"token":"{TOKEN}","chat_id":5,"paired_at":1}}"#),
            0o644,
        );
        let err = load(&p).unwrap_err();
        let msg = err.message().unwrap();
        assert!(matches!(err, LoadError::Insecure(_)));
        assert!(msg.contains("chmod 600"), "{msg}");
        assert!(msg.contains(&p.display().to_string()), "{msg}");
        assert!(!msg.contains(TOKEN));
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn invalid_contents_are_refused_without_echo() {
        let d = tmpdir("invalid");
        for text in [
            r#"{"token":"","chat_id":5}"#,
            r#"{"token":"abc","chat_id":5}"#,
            r#"{"token":"1:x"}"#,
            r#"{"token":"1:x","chat_id":0}"#,
            "not json at all",
        ] {
            let p = write(&d, text, 0o600);
            let err = load(&p).unwrap_err();
            assert!(matches!(err, LoadError::Invalid(_)), "{text}: {err:?}");
            let msg = err.message().unwrap();
            assert!(msg.contains("không hợp lệ"), "{msg}");
            assert!(!msg.contains(text), "{msg}");
        }
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn valid_file_loads() {
        let d = tmpdir("valid");
        let p = write(
            &d,
            &format!(r#"{{"token":"{TOKEN}","chat_id":5555,"paired_at":7}}"#),
            0o600,
        );
        let cfg = load(&p).unwrap();
        assert_eq!(cfg.token, TOKEN);
        assert_eq!(cfg.chat_id, 5555);
        assert_eq!(cfg.paired_at, 7);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn save_writes_0600_and_debug_hides_token() {
        let d = tmpdir("save");
        let p = d.join("sub").join(FILE_NAME);
        let cfg = TelegramConfig {
            token: TOKEN.to_string(),
            chat_id: 42,
            paired_at: 9,
        };
        save(&p, &cfg).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let back = load(&p).unwrap();
        assert_eq!(back.chat_id, 42);
        let dbg = format!("{cfg:?}");
        assert!(!dbg.contains(TOKEN), "{dbg}");
        assert!(dbg.contains("<ẩn>"), "{dbg}");
        let leftovers: Vec<_> = std::fs::read_dir(p.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = std::fs::remove_dir_all(d);
    }
}
