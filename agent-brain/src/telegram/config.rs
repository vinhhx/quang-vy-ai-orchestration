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
