use crate::config;
use crate::error::{BrainError, BrainResult};
use std::path::PathBuf;

pub fn path(project_id: &str, task_id: i64, slug: &str) -> PathBuf {
    config::data_dir()
        .join("answers")
        .join(project_id)
        .join(format!("{task_id}-{slug}.md"))
}

pub fn save(project_id: &str, task_id: i64, slug: &str, content: &str) -> BrainResult<PathBuf> {
    let dest = path(project_id, task_id, slug);
    if let Some(parent) = dest.parent() {
        if !parent.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(parent)
                    .map_err(|e| BrainError::error(format!("Không tạo được thư mục answers: {e}")))?;
            }
            #[cfg(not(unix))]
            {
                std::fs::create_dir_all(parent)
                    .map_err(|e| BrainError::error(format!("Không tạo được thư mục answers: {e}")))?;
            }
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }
    }

    let tmp_dest = dest.with_extension("tmp");

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp_dest)
            .map_err(|e| BrainError::error(format!("Không tạo được file tạm answer: {e}")))?;
        file.write_all(content.as_bytes())
            .map_err(|e| BrainError::error(format!("Không ghi được file answer: {e}")))?;
        file.flush()
            .map_err(|e| BrainError::error(format!("Không flush được file answer: {e}")))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&tmp_dest, content)
            .map_err(|e| BrainError::error(format!("Không ghi được file answer: {e}")))?;
    }

    std::fs::rename(&tmp_dest, &dest)
        .map_err(|e| BrainError::error(format!("Không đổi tên được file answer: {e}")))?;

    Ok(dest)
}

pub fn read(project_id: &str, task_id: i64, slug: &str) -> BrainResult<Option<String>> {
    let dest = path(project_id, task_id, slug);
    if !dest.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&dest)
        .map_err(|e| BrainError::error(format!("Không đọc được file answer: {e}")))?;
    Ok(Some(content))
}
