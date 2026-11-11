//! The protocol files (plan, QA report, plan review, code review): their names and where they
//! live, in one place (D1). Every other module asks this one for a path.
//!
//! A project may set `protocol_dir` in projects_map.json (default `""` = the repo root, the
//! historical layout). `{slug}` is the only variable and is replaced with the task slug, so
//! `".brain/{slug}"` gives each task its own folder. Every path is relative to the root the task
//! works in (today the project root), so a per-task work directory can be layered on later.

use std::path::Path;

pub const PLAN_FILE: &str = ".plan.md";
pub const QA_REPORT_FILE: &str = ".qa-report.md";
pub const PLAN_REVIEW_FILE: &str = ".plan-review.md";
pub const CODE_REVIEW_FILE: &str = ".code-review.md";
/// The four file names, in the order plan, QA report, plan review, code review.
pub const FILE_NAMES: [&str; 4] = [PLAN_FILE, QA_REPORT_FILE, PLAN_REVIEW_FILE, CODE_REVIEW_FILE];

/// Key of the setting in a projects_map.json entry.
pub const CONFIG_KEY: &str = "protocol_dir";
/// The one variable a `protocol_dir` may hold.
pub const SLUG_VAR: &str = "{slug}";
/// Longest accepted `protocol_dir` (characters, before `{slug}` is replaced).
pub const PROTOCOL_DIR_MAX_CHARS: usize = 120;

/// Default `protocol_dir` of a project whose entry has no `protocol_dir` key and that has a real
/// `.planning/` folder.
pub const DEFAULT_DIR_PLANNING: &str = ".planning/quick/plan-{slug}";
/// Default `protocol_dir` of every other project without a `protocol_dir` key.
pub const DEFAULT_DIR_GSD: &str = ".gsd/quick/plan-{slug}";

/// Prompt placeholders, replaced by `ProtocolPaths::render` wherever a prompt is built.
pub const PLAN_PLACEHOLDER: &str = "{plan_path}";
pub const QA_REPORT_PLACEHOLDER: &str = "{qa_report_path}";
pub const PLAN_REVIEW_PLACEHOLDER: &str = "{plan_review_path}";
pub const CODE_REVIEW_PLACEHOLDER: &str = "{code_review_path}";

/// One of the four protocol files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolFile {
    Plan,
    QaReport,
    PlanReview,
    CodeReview,
}

impl ProtocolFile {
    /// The prompt placeholder naming this file.
    pub fn placeholder(&self) -> &'static str {
        match self {
            ProtocolFile::Plan => PLAN_PLACEHOLDER,
            ProtocolFile::QaReport => QA_REPORT_PLACEHOLDER,
            ProtocolFile::PlanReview => PLAN_REVIEW_PLACEHOLDER,
            ProtocolFile::CodeReview => CODE_REVIEW_PLACEHOLDER,
        }
    }
}

/// Paths of a task's protocol files, relative to the root the task works in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolPaths {
    pub plan: String,
    pub qa_report: String,
    pub plan_review: String,
    pub code_review: String,
}

impl Default for ProtocolPaths {
    /// The default layout: every file at the root.
    fn default() -> Self {
        paths_in("", "")
    }
}

impl ProtocolPaths {
    /// The four paths, in the order of `FILE_NAMES`.
    pub fn all(&self) -> [&str; 4] {
        [&self.plan, &self.qa_report, &self.plan_review, &self.code_review]
    }

    /// The path of one file.
    pub fn get(&self, file: ProtocolFile) -> &str {
        match file {
            ProtocolFile::Plan => &self.plan,
            ProtocolFile::QaReport => &self.qa_report,
            ProtocolFile::PlanReview => &self.plan_review,
            ProtocolFile::CodeReview => &self.code_review,
        }
    }

    /// True when `rel` (relative to the task root, `./` allowed) is one of the four paths.
    pub fn contains(&self, rel: &str) -> bool {
        let r = rel.strip_prefix("./").unwrap_or(rel);
        self.all().contains(&r)
    }

    /// The folder holding the files, relative to the root; `None` for the root itself.
    pub fn dir(&self) -> Option<&str> {
        self.plan
            .rsplit_once('/')
            .map(|(d, _)| d)
            .filter(|d| !d.is_empty())
    }

    /// The report a review kind writes.
    pub fn review(&self, kind: crate::report::ReviewKind) -> &str {
        match kind {
            crate::report::ReviewKind::Plan => &self.plan_review,
            crate::report::ReviewKind::Code => &self.code_review,
        }
    }

    /// `:(exclude)<path>` for each file, to keep them out of a `git diff -- .`.
    pub fn exclude_pathspecs(&self) -> Vec<String> {
        self.all().iter().map(|p| format!(":(exclude){p}")).collect()
    }

    /// The `git diff` command a reviewer or QA runs to see the code changed since the plan
    /// commit, protocol files left out.
    pub fn code_diff_command(&self, plan_commit: &str) -> String {
        let excl: Vec<String> = self
            .exclude_pathspecs()
            .iter()
            .map(|p| format!("'{p}'"))
            .collect();
        format!("git diff {plan_commit}..HEAD -- . {}", excl.join(" "))
    }

    /// `git show <rev>:<plan>` argument.
    pub fn plan_at(&self, rev: &str) -> String {
        format!("{rev}:{}", self.plan)
    }

    /// Replaces the prompt placeholders (`{plan_path}`, `{qa_report_path}`,
    /// `{plan_review_path}`, `{code_review_path}`) with these paths. Any other text, braces
    /// included, is left as is.
    pub fn render(&self, text: &str) -> String {
        if !text.contains('{') {
            return text.to_string();
        }
        text.replace(PLAN_PLACEHOLDER, &self.plan)
            .replace(QA_REPORT_PLACEHOLDER, &self.qa_report)
            .replace(PLAN_REVIEW_PLACEHOLDER, &self.plan_review)
            .replace(CODE_REVIEW_PLACEHOLDER, &self.code_review)
    }

    /// Creates the folder of the files under `root` when it is not the root.
    pub fn ensure_dir(&self, root: &Path) -> std::io::Result<()> {
        match self.dir() {
            Some(d) => std::fs::create_dir_all(root.join(d)),
            None => Ok(()),
        }
    }
}

/// The paths for a `protocol_dir` value and a task slug. An invalid value falls back to the
/// root (callers validate before storing; this keeps a hand-edited map from escaping the repo).
pub fn paths_in(protocol_dir: &str, slug: &str) -> ProtocolPaths {
    let dir = match validate_protocol_dir(protocol_dir) {
        Ok(d) => d.replace(SLUG_VAR, slug),
        Err(_) => String::new(),
    };
    // An empty slug leaves an empty segment: drop it so the path stays well formed.
    let dir: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    let dir = dir.join("/");
    let join = |name: &str| {
        if dir.is_empty() {
            name.to_string()
        } else {
            format!("{dir}/{name}")
        }
    };
    ProtocolPaths {
        plan: join(PLAN_FILE),
        qa_report: join(QA_REPORT_FILE),
        plan_review: join(PLAN_REVIEW_FILE),
        code_review: join(CODE_REVIEW_FILE),
    }
}

/// The paths of a project's task.
pub fn paths(project: &crate::projects::Project, task_slug: &str) -> ProtocolPaths {
    paths_in(&project.protocol_dir, task_slug)
}

/// The paths of the task a branch `task/<slug>` belongs to; any other branch gives an empty slug.
pub fn for_branch(project: &crate::projects::Project, branch: Option<&str>) -> ProtocolPaths {
    let slug = branch
        .and_then(|b| b.strip_prefix("task/"))
        .unwrap_or("");
    paths(project, slug)
}

/// The paths of the task whose branch is checked out at `root`.
pub fn current(project: &crate::projects::Project, root: &Path) -> ProtocolPaths {
    for_branch(project, crate::gitops::current_branch(root).as_deref())
}

/// True when the file name of `p` is one of the protocol file names (wherever it lives).
pub fn is_protocol_file(p: &str) -> bool {
    let basename = p.rsplit('/').next().unwrap_or(p);
    FILE_NAMES.contains(&p) || FILE_NAMES.contains(&basename)
}

/// Checks a `protocol_dir` value and returns it trimmed, without `./` or trailing `/`.
/// Allowed: `""` (the root), or a relative path without `..`, not inside `.git`, without
/// backslashes or control characters, at most `PROTOCOL_DIR_MAX_CHARS` long, where `{slug}` is
/// the only `{…}`.
pub fn validate_protocol_dir(raw: &str) -> Result<String, String> {
    let t = raw.trim();
    if t.chars().count() > PROTOCOL_DIR_MAX_CHARS {
        return Err(format!(
            "\"{CONFIG_KEY}\" dài quá {PROTOCOL_DIR_MAX_CHARS} ký tự."
        ));
    }
    if t.starts_with('/') || t.starts_with('~') {
        return Err(format!(
            "\"{CONFIG_KEY}\" phải là đường dẫn tương đối so với gốc repo."
        ));
    }
    if t.contains('\\') || t.chars().any(|c| c.is_control()) {
        return Err(format!(
            "\"{CONFIG_KEY}\" không được chứa dấu \\ hay ký tự điều khiển."
        ));
    }
    let without_var = t.replace(SLUG_VAR, "");
    if without_var.contains('{') || without_var.contains('}') {
        return Err(format!(
            "\"{CONFIG_KEY}\": chỉ dùng được biến {SLUG_VAR}."
        ));
    }
    let mut parts = Vec::new();
    for seg in t.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return Err(format!("\"{CONFIG_KEY}\" không được chứa \"..\"."));
        }
        parts.push(seg);
    }
    if parts.first().is_some_and(|s| s.eq_ignore_ascii_case(".git")) {
        return Err(format!(
            "\"{CONFIG_KEY}\" không được trỏ vào thư mục .git."
        ));
    }
    Ok(parts.join("/"))
}

/// The `protocol_dir` of a project whose entry has no `protocol_dir` key: per-task folders under
/// the project's knowledge home. A real (non-symlink) `.planning/` folder gives
/// `.planning/quick/plan-{slug}`, anything else `.gsd/quick/plan-{slug}`. Only `.planning/` is
/// looked at (never `.gsd/`), so the answer does not change once the program creates `.gsd/`.
pub fn default_protocol_dir(root: &Path) -> String {
    let planning_is_real_dir = std::fs::symlink_metadata(root.join(".planning"))
        .map(|m| !m.file_type().is_symlink() && m.is_dir())
        .unwrap_or(false);
    if planning_is_real_dir {
        DEFAULT_DIR_PLANNING.to_string()
    } else {
        DEFAULT_DIR_GSD.to_string()
    }
}

/// The `protocol_dir` of a projects_map.json entry; missing or invalid reads as `""`.
/// (The projects loader applies `default_protocol_dir` when the key is absent.)
pub fn protocol_dir_of_value(entry: &serde_json::Value) -> String {
    entry
        .get(CONFIG_KEY)
        .and_then(|v| v.as_str())
        .and_then(|s| validate_protocol_dir(s).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_the_root_layout() {
        let p = ProtocolPaths::default();
        assert_eq!(p.plan, ".plan.md");
        assert_eq!(p.qa_report, ".qa-report.md");
        assert_eq!(p.plan_review, ".plan-review.md");
        assert_eq!(p.code_review, ".code-review.md");
        assert_eq!(p.dir(), None);
        assert_eq!(
            p.code_diff_command("abc"),
            "git diff abc..HEAD -- . ':(exclude).plan.md' ':(exclude).qa-report.md' ':(exclude).plan-review.md' ':(exclude).code-review.md'"
        );
        assert_eq!(paths_in("", "x"), p);
        assert_eq!(paths_in("  ", "x"), p);
    }

    #[test]
    fn slug_dir_is_substituted() {
        let p = paths_in(".brain/{slug}", "sua-app");
        assert_eq!(p.plan, ".brain/sua-app/.plan.md");
        assert_eq!(p.qa_report, ".brain/sua-app/.qa-report.md");
        assert_eq!(p.dir(), Some(".brain/sua-app"));
        assert!(p.contains("./.brain/sua-app/.plan.md"));
        assert!(!p.contains(".plan.md"));
        assert_eq!(p.plan_at("abc"), "abc:.brain/sua-app/.plan.md");
        assert_eq!(
            p.exclude_pathspecs()[0],
            ":(exclude).brain/sua-app/.plan.md"
        );
        // no slug (not on a task branch): the empty segment is dropped
        assert_eq!(paths_in(".brain/{slug}", "").plan, ".brain/.plan.md");
        assert_eq!(paths_in("./docs/", "x").plan, "docs/.plan.md");
    }

    #[test]
    fn invalid_dir_falls_back_to_root() {
        assert_eq!(paths_in("../x", "s").plan, ".plan.md");
        assert_eq!(paths_in("/abs", "s").plan, ".plan.md");
    }

    #[test]
    fn validate_rules() {
        assert_eq!(validate_protocol_dir("").unwrap(), "");
        assert_eq!(validate_protocol_dir(" .brain/{slug}/ ").unwrap(), ".brain/{slug}");
        assert!(validate_protocol_dir("/etc").is_err());
        assert!(validate_protocol_dir("~/x").is_err());
        assert!(validate_protocol_dir("a/../b").is_err());
        assert!(validate_protocol_dir("..").is_err());
        assert!(validate_protocol_dir(".git").is_err());
        assert!(validate_protocol_dir("./.GIT/x").is_err());
        assert!(validate_protocol_dir("a\\b").is_err());
        assert!(validate_protocol_dir("a/{task}").is_err());
        assert!(validate_protocol_dir("a/{slug").is_err());
        assert!(validate_protocol_dir(&"a".repeat(PROTOCOL_DIR_MAX_CHARS + 1)).is_err());
        assert!(validate_protocol_dir(".github/plans").is_ok());
    }

    #[test]
    fn render_replaces_only_placeholders() {
        let p = paths_in(".brain/{slug}", "t");
        let out = p.render("Ghi {plan_path}, {qa_report_path}, {plan_review_path}, {code_review_path}; giữ {slug} và {x}.");
        assert_eq!(
            out,
            "Ghi .brain/t/.plan.md, .brain/t/.qa-report.md, .brain/t/.plan-review.md, .brain/t/.code-review.md; giữ {slug} và {x}."
        );
        assert_eq!(ProtocolPaths::default().render("{plan_path}"), ".plan.md");
    }

    #[test]
    fn protocol_file_by_name() {
        assert!(is_protocol_file(".plan.md"));
        assert!(is_protocol_file(".brain/t/.qa-report.md"));
        assert!(!is_protocol_file("plan.md"));
    }

    #[test]
    fn default_dir_follows_planning_folder() {
        let t = tempfile::tempdir().unwrap();
        // nothing: .gsd
        assert_eq!(default_protocol_dir(t.path()), ".gsd/quick/plan-{slug}");
        // .gsd alone does not change the answer
        std::fs::create_dir_all(t.path().join(".gsd/quick")).unwrap();
        assert_eq!(default_protocol_dir(t.path()), ".gsd/quick/plan-{slug}");
        // a real .planning: .planning
        std::fs::create_dir_all(t.path().join(".planning")).unwrap();
        assert_eq!(default_protocol_dir(t.path()), ".planning/quick/plan-{slug}");
        assert_eq!(
            paths_in(&default_protocol_dir(t.path()), "sua-nut").plan,
            ".planning/quick/plan-sua-nut/.plan.md"
        );
    }

    #[cfg(unix)]
    #[test]
    fn default_dir_ignores_planning_symlink() {
        let t = tempfile::tempdir().unwrap();
        let other = t.path().join("elsewhere");
        std::fs::create_dir_all(&other).unwrap();
        std::os::unix::fs::symlink(&other, t.path().join(".planning")).unwrap();
        assert_eq!(default_protocol_dir(t.path()), ".gsd/quick/plan-{slug}");
        // a plain file named .planning is not a folder either
        let t2 = tempfile::tempdir().unwrap();
        std::fs::write(t2.path().join(".planning"), "x").unwrap();
        assert_eq!(default_protocol_dir(t2.path()), ".gsd/quick/plan-{slug}");
    }

    #[test]
    fn dir_of_value() {
        let v = serde_json::json!({"protocol_dir": ".brain/{slug}"});
        assert_eq!(protocol_dir_of_value(&v), ".brain/{slug}");
        assert_eq!(protocol_dir_of_value(&serde_json::json!({"protocol_dir": "../x"})), "");
        assert_eq!(protocol_dir_of_value(&serde_json::json!({})), "");
    }
}
