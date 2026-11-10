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

/// Nội dung `.gitignore` tạo khi vault chưa có (không bao giờ ghi đè tệp đã có): bỏ qua cả
/// `.obsidian/` (dữ liệu plugin trên điện thoại có thể chứa token) và thùng rác của Obsidian.
pub const GITIGNORE_BOOTSTRAP: &str = ".obsidian/\n.trash/\n";

/// Tệp lớn hơn mức này không phải tệp sinh tự động: không đọc, không xóa.
const MAX_OWNED_BYTES: u64 = 4 * 1024 * 1024;

fn read_marked(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_file() && m.len() <= MAX_OWNED_BYTES => {}
        _ => return false,
    }
    std::fs::read(path)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .map(|s| has_marker(&s))
        .unwrap_or(false)
}

/// Tệp thường (không theo symlink, bỏ mọi tên bắt đầu bằng `.`) dưới `rel_dir` của `root`.
fn walk_files(root: &Path, rel_dir: &str, out: &mut Vec<String>) -> BrainResult<()> {
    let entries = match std::fs::read_dir(root.join(rel_dir)) {
        Ok(it) => it,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    for entry in entries {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let rel = format!("{rel_dir}/{name}");
        let ft = entry.file_type()?;
        if ft.is_dir() {
            walk_files(root, &rel, out)?;
        } else if ft.is_file() {
            out.push(rel);
        }
    }
    Ok(())
}

/// Tệp đang có trong các gốc được quản lý (`Today.md`, `Nhật ký/`, `Dự án/`); gốc là symlink
/// thì bỏ qua cả gốc.
fn managed_files_on_disk(clone_dir: &Path) -> BrainResult<Vec<String>> {
    let mut out = Vec::new();
    if let Ok(m) = std::fs::symlink_metadata(clone_dir.join(TODAY_PATH)) {
        if m.file_type().is_file() {
            out.push(TODAY_PATH.to_string());
        }
    }
    for root in [JOURNAL_DIR, PROJECTS_DIR] {
        match std::fs::symlink_metadata(clone_dir.join(root)) {
            Ok(m) if m.file_type().is_dir() => walk_files(clone_dir, root, &mut out)?,
            _ => {}
        }
    }
    Ok(out)
}

/// Xóa thư mục rỗng bên dưới `Nhật ký/` (không bao giờ xóa chính thư mục gốc).
fn prune_empty_dirs(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut empty = true;
    for entry in entries.flatten() {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir && prune_empty_dirs(&entry.path()) {
            let _ = std::fs::remove_dir(entry.path());
            continue;
        }
        empty = false;
    }
    empty
}

/// Ghi `files` (đường dẫn tương đối → nội dung) vào `clone_dir`. Mọi đường dẫn được kiểm tra
/// trước khi ghi tệp đầu tiên. Sau đó (D-22): xóa tệp cũ trong các gốc được quản lý khi tệp
/// không còn trong `files` và có dòng đánh dấu; tệp không có dấu, `.obsidian/`, `.git/` và mọi
/// thư mục khác không bao giờ bị đọc để xóa, ghi hay xóa. `.gitignore` chỉ được tạo khi chưa có.
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

    let mut removed_any = false;
    for rel in managed_files_on_disk(clone_dir)? {
        if files.contains_key(&rel) || !is_managed_path(&rel) {
            continue;
        }
        let path = clone_dir.join(&rel);
        if read_marked(&path) {
            std::fs::remove_file(&path)?;
            report.removed.push(rel);
            removed_any = true;
        }
    }
    if removed_any {
        let root = clone_dir.join(JOURNAL_DIR);
        if std::fs::symlink_metadata(&root)
            .map(|m| m.file_type().is_dir())
            .unwrap_or(false)
        {
            prune_empty_dirs(&root);
        }
    }

    let gitignore = clone_dir.join(".gitignore");
    if std::fs::symlink_metadata(&gitignore).is_err() {
        write_atomic(&gitignore, GITIGNORE_BOOTSTRAP)?;
        report.changed.push(".gitignore".to_string());
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
        assert_eq!(
            r.changed,
            vec![
                "Nhật ký/2026/01/2026-01-15.md".to_string(),
                ".gitignore".to_string()
            ]
        );
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

    fn marked(title: &str) -> String {
        format!("---\ndate: x\n---\n{MARKER}\n# {title}\n")
    }

    fn put(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn one_day() -> BTreeMap<String, String> {
        let mut files = BTreeMap::new();
        files.insert("Nhật ký/2026/01/2026-01-15.md".to_string(), marked("mới"));
        files
    }

    #[test]
    fn stale_marker_file_is_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let stale = "Nhật ký/2025/12/2025-12-01.md";
        put(dir.path(), stale, &marked("cũ"));

        let r = apply(dir.path(), &one_day()).unwrap();
        assert_eq!(r.removed, vec![stale.to_string()]);
        assert!(!dir.path().join(stale).exists());
        // Thư mục rỗng dưới `Nhật ký/` được dọn, gốc thì giữ.
        assert!(!dir.path().join("Nhật ký/2025").exists());
        assert!(dir.path().join("Nhật ký").is_dir());
        assert!(dir.path().join("Nhật ký/2026/01/2026-01-15.md").is_file());
    }

    #[test]
    fn marker_less_file_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let note = "Dự án/ghi-chu-cua-toi.md";
        put(dir.path(), note, "ghi chú tay của tôi\n");
        put(dir.path(), "Today.md", "trang của tôi\n");
        let mut files = one_day();
        files.insert("Today.md".to_string(), marked("hôm nay"));

        let r = apply(dir.path(), &files).unwrap();
        assert!(r.removed.is_empty(), "{r:?}");
        assert_eq!(r.foreign, vec!["Today.md".to_string()]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(note)).unwrap(),
            "ghi chú tay của tôi\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("Today.md")).unwrap(),
            "trang của tôi\n"
        );
    }

    #[test]
    fn obsidian_dir_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let app = format!("{{\"a\": 1}}\n{MARKER}\n");
        put(dir.path(), ".obsidian/app.json", &app);
        put(dir.path(), ".obsidian/Nhật ký/x.md", &marked("x"));
        put(dir.path(), "Nhật ký/.obsidian/y.md", &marked("y"));
        put(dir.path(), "Ghi chú/z.md", &marked("z"));

        let r = apply(dir.path(), &one_day()).unwrap();
        assert!(r.removed.is_empty(), "{r:?}");
        assert!(!r.changed.iter().any(|p| p.starts_with(".obsidian")));
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".obsidian/app.json")).unwrap(),
            app
        );
        assert!(dir.path().join(".obsidian/Nhật ký/x.md").is_file());
        assert!(dir.path().join("Nhật ký/.obsidian/y.md").is_file());
        assert!(dir.path().join("Ghi chú/z.md").is_file());
    }

    #[test]
    fn gitignore_bootstrapped_only_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let r = apply(dir.path(), &one_day()).unwrap();
        assert!(r.changed.contains(&".gitignore".to_string()), "{r:?}");
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".gitignore")).unwrap(),
            ".obsidian/\n.trash/\n"
        );

        let other = tempfile::tempdir().unwrap();
        let mine = ".obsidian/workspace*.json\n.obsidian/cache\n.trash/\n";
        std::fs::write(other.path().join(".gitignore"), mine).unwrap();
        let r = apply(other.path(), &one_day()).unwrap();
        assert!(!r.changed.contains(&".gitignore".to_string()), "{r:?}");
        assert_eq!(
            std::fs::read(other.path().join(".gitignore")).unwrap(),
            mine.as_bytes()
        );
    }

    #[test]
    fn symlink_in_managed_root_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        put(outside.path(), "ngoai.md", &marked("ngoài"));
        put(outside.path(), "sau/trong.md", &marked("trong"));
        std::fs::create_dir_all(dir.path().join("Dự án")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("ngoai.md"),
            dir.path().join("Dự án/lien-ket.md"),
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path().join("sau"), dir.path().join("Dự án/thu-muc"))
            .unwrap();

        let r = apply(dir.path(), &one_day()).unwrap();
        assert!(r.removed.is_empty(), "{r:?}");
        assert!(std::fs::symlink_metadata(dir.path().join("Dự án/lien-ket.md")).is_ok());
        assert!(outside.path().join("ngoai.md").is_file());
        assert!(outside.path().join("sau/trong.md").is_file());
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
