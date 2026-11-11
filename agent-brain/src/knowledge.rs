//! Thư mục tri thức dự án (.gsd -> .planning -> .agent-brain).
//!
//! Mỗi dự án có một thư mục tri thức để các agent bám theo tech stack,
//! nghiệp vụ và lưu nhật ký tính năng/bug. Thứ tự ưu tiên tìm kiếm:
//! 1. `.gsd/`
//! 2. `.planning/`
//! 3. `.agent-brain/`
//!
//! Bỏ qua symlink và các entry không phải thư mục thật. Thư mục `.gsd/` chỉ có `quick/`
//! (chương trình tự tạo để chứa file giao thức theo task) KHÔNG tính là thư mục tri thức.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HomeKind {
    Gsd,
    Planning,
    AgentBrain,
}

impl HomeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gsd => "gsd",
            Self::Planning => "planning",
            Self::AgentBrain => "agent-brain",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    pub kind: HomeKind,
    pub dir: PathBuf,
    pub rel: &'static str,
}

const CANDIDATES: &[(HomeKind, &str)] = &[
    (HomeKind::Gsd, ".gsd"),
    (HomeKind::Planning, ".planning"),
    (HomeKind::AgentBrain, ".agent-brain"),
];

/// `.gsd/` mà chỉ chứa `quick/` là do chương trình tạo cho file giao thức theo task, không phải
/// thư mục tri thức. Một `.gsd/` rỗng hay có mục khác (README.md, phases/…) do người dùng đặt vẫn tính.
fn gsd_only_quick(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(rd) => {
            let names: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
            !names.is_empty() && names.iter().all(|n| n == "quick")
        }
        Err(_) => false,
    }
}

/// Tìm thư mục tri thức đầu tiên tồn tại theo thứ tự ưu tiên (.gsd -> .planning -> .agent-brain).
/// Bỏ qua symlink và thứ không phải thư mục; `.gsd/` chỉ có `quick/` cũng bị bỏ qua.
pub fn find(root: &Path) -> Option<Home> {
    for (kind, rel) in CANDIDATES {
        let p = root.join(rel);
        if let Ok(meta) = std::fs::symlink_metadata(&p) {
            if !meta.file_type().is_symlink() && meta.is_dir() {
                if *kind == HomeKind::Gsd && gsd_only_quick(&p) {
                    continue;
                }
                let dir = if p.is_absolute() {
                    p
                } else if let Ok(cwd) = std::env::current_dir() {
                    cwd.join(&p)
                } else {
                    p
                };
                return Some(Home {
                    kind: *kind,
                    dir,
                    rel,
                });
            }
        }
    }
    None
}

/// Trả về thư mục tri thức tìm được, hoặc mặc định `root/.agent-brain` nếu chưa tồn tại.
pub fn find_or_default(root: &Path) -> Home {
    find(root).unwrap_or_else(|| {
        let dir = if root.is_absolute() {
            root.join(".agent-brain")
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(root).join(".agent-brain")
        } else {
            root.join(".agent-brain")
        };
        Home {
            kind: HomeKind::AgentBrain,
            dir,
            rel: ".agent-brain",
        }
    })
}

/// Trả về đường dẫn file tài liệu mở đầu tương ứng nếu tồn tại:
/// - `.gsd/README.md`
/// - `.planning/PROJECT.md`
/// - `.agent-brain/README.md`
pub fn readme(home: &Home) -> Option<PathBuf> {
    let target = match home.kind {
        HomeKind::Gsd => home.dir.join("README.md"),
        HomeKind::Planning => home.dir.join("PROJECT.md"),
        HomeKind::AgentBrain => home.dir.join("README.md"),
    };
    if target.is_file() {
        Some(target)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_only_planning() {
        let temp = tempfile::tempdir().unwrap();
        let plan_dir = temp.path().join(".planning");
        std::fs::create_dir_all(&plan_dir).unwrap();

        let home = find(temp.path()).expect("found .planning");
        assert_eq!(home.kind, HomeKind::Planning);
        assert_eq!(home.rel, ".planning");
        assert!(home.dir.is_absolute());
        assert_eq!(home.dir, plan_dir);
    }

    #[test]
    fn test_find_gsd_and_planning_prefers_gsd() {
        let temp = tempfile::tempdir().unwrap();
        let gsd_dir = temp.path().join(".gsd");
        let plan_dir = temp.path().join(".planning");
        std::fs::create_dir_all(&gsd_dir).unwrap();
        std::fs::create_dir_all(&plan_dir).unwrap();

        let home = find(temp.path()).expect("found .gsd");
        assert_eq!(home.kind, HomeKind::Gsd);
        assert_eq!(home.rel, ".gsd");
        assert_eq!(home.dir, gsd_dir);
    }

    #[test]
    fn test_find_gsd_with_only_quick_is_not_a_home() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".gsd/quick/plan-x")).unwrap();
        let ab = temp.path().join(".agent-brain");
        std::fs::create_dir_all(&ab).unwrap();

        // `.gsd/` made by the program (only `quick/`) must not shadow `.agent-brain/`.
        let home = find(temp.path()).expect("found .agent-brain");
        assert_eq!(home.kind, HomeKind::AgentBrain);
        assert_eq!(home.dir, ab);

        // Nothing else there: no home at all (and default stays .agent-brain).
        let only = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(only.path().join(".gsd/quick")).unwrap();
        assert!(find(only.path()).is_none());
        assert_eq!(find_or_default(only.path()).kind, HomeKind::AgentBrain);

        // `.gsd/` with any other entry is a home again.
        std::fs::write(temp.path().join(".gsd/README.md"), "# GSD").unwrap();
        assert_eq!(find(temp.path()).unwrap().kind, HomeKind::Gsd);

        // An empty `.gsd/` was made by the user: still a home, as before.
        let empty = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(empty.path().join(".gsd")).unwrap();
        assert_eq!(find(empty.path()).unwrap().kind, HomeKind::Gsd);
    }

    #[test]
    fn test_find_symlink_is_ignored() {
        let temp = tempfile::tempdir().unwrap();
        let foreign = temp.path().join("foreign_dir");
        std::fs::create_dir_all(&foreign).unwrap();

        let gsd_link = temp.path().join(".gsd");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&foreign, &gsd_link).unwrap();

        // Chỉ có symlink .gsd -> None
        assert!(find(temp.path()).is_none());

        // Thêm .planning thật -> tìm được .planning
        let plan_dir = temp.path().join(".planning");
        std::fs::create_dir_all(&plan_dir).unwrap();
        let home = find(temp.path()).expect("found .planning despite .gsd symlink");
        assert_eq!(home.kind, HomeKind::Planning);
    }

    #[test]
    fn test_find_empty_and_default() {
        let temp = tempfile::tempdir().unwrap();
        assert!(find(temp.path()).is_none());

        let home = find_or_default(temp.path());
        assert_eq!(home.kind, HomeKind::AgentBrain);
        assert_eq!(home.rel, ".agent-brain");
        assert_eq!(home.dir, temp.path().join(".agent-brain"));
        assert!(home.dir.is_absolute());
    }

    #[test]
    fn test_readme_detection() {
        let temp = tempfile::tempdir().unwrap();
        let gsd = temp.path().join(".gsd");
        std::fs::create_dir_all(&gsd).unwrap();
        let home_gsd = find(temp.path()).unwrap();
        assert!(readme(&home_gsd).is_none());

        let gsd_readme = gsd.join("README.md");
        std::fs::write(&gsd_readme, "# GSD Readme").unwrap();
        assert_eq!(readme(&home_gsd), Some(gsd_readme));

        let temp2 = tempfile::tempdir().unwrap();
        let planning = temp2.path().join(".planning");
        std::fs::create_dir_all(&planning).unwrap();
        let home_plan = find(temp2.path()).unwrap();
        assert!(readme(&home_plan).is_none());

        let plan_project = planning.join("PROJECT.md");
        std::fs::write(&plan_project, "# Project").unwrap();
        assert_eq!(readme(&home_plan), Some(plan_project));

        let temp3 = tempfile::tempdir().unwrap();
        let ab = temp3.path().join(".agent-brain");
        std::fs::create_dir_all(&ab).unwrap();
        let home_ab = find(temp3.path()).unwrap();
        let ab_readme = ab.join("README.md");
        std::fs::write(&ab_readme, "# AB Readme").unwrap();
        assert_eq!(readme(&home_ab), Some(ab_readme));
    }
}
