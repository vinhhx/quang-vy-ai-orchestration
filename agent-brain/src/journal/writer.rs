//! Ghi tệp vào bản clone (D-22): chỉ đường dẫn được quản lý, chỉ ghi đè tệp có dòng đánh dấu,
//! ghi qua tệp tạm cùng thư mục rồi `rename` (không bao giờ để lại tệp ghi dở).

use super::model::{JOURNAL_DIR, MARKER, PROJECTS_DIR, TODAY_PATH};
use crate::error::{BrainError, BrainResult};
use std::collections::BTreeMap;
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriteReport {
    /// Tệp đã ghi mới hoặc thay đổi.
    pub changed: Vec<String>,
    /// Tệp đã xóa (07-05).
    pub removed: Vec<String>,
    /// Tệp ở đường dẫn quản lý nhưng không có dòng đánh dấu: không động tới.
    pub foreign: Vec<String>,
}

/// `Today.md`, hoặc nằm dưới `Nhật ký/` hay `Dự án/`.
pub fn is_managed_path(rel: &str) -> bool {
    rel == TODAY_PATH
        || rel.starts_with(&format!("{JOURNAL_DIR}/"))
        || rel.starts_with(&format!("{PROJECTS_DIR}/"))
}

/// Một trong 12 dòng đầu bằng đúng `MARKER`.
pub fn has_marker(content: &str) -> bool {
    content.lines().take(12).any(|l| l == MARKER)
}

fn check_rel(rel: &str) -> BrainResult<()> {
    let bad = rel.is_empty()
        || rel.starts_with('/')
        || rel.contains('\\')
        || rel.contains('\0')
        || rel.split('/').any(|seg| seg == ".." || seg.is_empty())
        || rel.contains("..")
        || rel.starts_with(".obsidian/")
        || rel.starts_with(".git/")
        || !rel.nfc().eq(rel.chars())
        || !is_managed_path(rel);
    if bad {
        return Err(BrainError::error(format!(
            "Nhật ký: từ chối ghi đường dẫn không hợp lệ: {rel}"
        )));
    }
    Ok(())
}

fn write_atomic(target: &Path, text: &str) -> BrainResult<()> {
    let dir = target
        .parent()
        .ok_or_else(|| BrainError::error("Nhật ký: đường dẫn tệp không có thư mục cha"))?;
    std::fs::create_dir_all(dir)?;
    let name = target
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| BrainError::error("Nhật ký: tên tệp không hợp lệ"))?;
    let tmp = dir.join(format!(".{name}.journal-tmp"));
    std::fs::write(&tmp, text.as_bytes())?;
    if let Err(e) = std::fs::rename(&tmp, target) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Ghi `files` (đường dẫn tương đối → nội dung) vào `clone_dir`. Mọi đường dẫn được kiểm tra
/// trước khi ghi tệp đầu tiên.
pub fn apply(clone_dir: &Path, files: &BTreeMap<String, String>) -> BrainResult<WriteReport> {
    for rel in files.keys() {
        check_rel(rel)?;
    }
    let mut report = WriteReport::default();
    for (rel, text) in files {
        let target = clone_dir.join(rel);
        if target.exists() {
            let existing = std::fs::read(&target)?;
            if existing == text.as_bytes() {
                continue;
            }
            let owned = std::str::from_utf8(&existing)
                .map(has_marker)
                .unwrap_or(false);
            if !owned {
                report.foreign.push(rel.clone());
                continue;
            }
        }
        write_atomic(&target, text)?;
        report.changed.push(rel.clone());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_skips_identical_and_leaves_foreign() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = BTreeMap::new();
        let body = format!("---\ndate: x\n---\n{MARKER}\n# a\n");
        files.insert("Nhật ký/2026/01/2026-01-15.md".to_string(), body.clone());
        files.insert("Today.md".to_string(), body.clone());
        std::fs::write(dir.path().join("Today.md"), "của người dùng\n").unwrap();

        let r = apply(dir.path(), &files).unwrap();
        assert_eq!(r.changed, vec!["Nhật ký/2026/01/2026-01-15.md".to_string()]);
        assert_eq!(r.foreign, vec!["Today.md".to_string()]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("Today.md")).unwrap(),
            "của người dùng\n"
        );

        let r2 = apply(dir.path(), &files).unwrap();
        assert!(r2.changed.is_empty());
        let leftovers: Vec<_> = std::fs::read_dir(dir.path().join("Nhật ký/2026/01"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".journal-tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn refuses_bad_paths() {
        let dir = tempfile::tempdir().unwrap();
        for bad in [
            "",
            "/etc/x.md",
            "Nhật ký/../x.md",
            "Nhật ký\\x.md",
            ".obsidian/a.json",
            ".git/config",
            "khac/x.md",
            "Nha\u{0323}\u{0302}t ký/x.md",
        ] {
            let mut files = BTreeMap::new();
            files.insert(bad.to_string(), "x\n".to_string());
            assert!(
                apply(dir.path(), &files).is_err(),
                "{bad:?} phải bị từ chối"
            );
        }
    }
}
