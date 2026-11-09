//! Task summary file sent before the "QA đạt" message: what was asked, how it was done and
//! the result. Deterministic (no LLM): built from the plan file, the git history of the task
//! branch, the QA report and the runs. Never includes diff content.

use crate::dashboard::api_tasks::TaskState;
use crate::{gitops, planfile, report};
use std::path::{Path, PathBuf};

/// Longest objective copied from the plan, in characters.
pub const OBJECTIVE_MAX: usize = 3000;
/// Longest QA conclusion copied from the QA report, in characters.
pub const QA_MAX: usize = 8000;
pub const COMMITS_MAX: usize = 50;
pub const DIFF_FILES_MAX: usize = 100;
pub const PLAN_FILES_MAX: usize = 50;
pub const MAX_REPORT_FILE_SIZE: usize = 1024 * 1024;
/// Largest file a bot may upload through the Bot API (50 MB).
pub const TELEGRAM_UPLOAD_MAX_BYTES: u64 = 50 * 1024 * 1024;
/// Extensions sent with `sendVideo`.
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "webm"];
const CUT_MARK: &str = "… (đã cắt)";

/// `git diff --numstat` reduced to totals and a per-file list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiffStat {
    pub files: usize,
    pub added: u64,
    pub removed: u64,
    /// `(path, added, removed)`; binary files count as 0/0.
    pub per_file: Vec<(String, u64, u64)>,
}

/// Plain data of the summary; `build` turns it into Markdown.
#[derive(Debug, Clone, Default)]
pub struct SummaryInput {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub project: String,
    pub branch: String,
    pub workflow: Option<String>,
    pub team: Option<String>,
    pub request: Option<String>,
    pub objective: Option<String>,
    pub plan_files: Vec<String>,
    /// `(role, engine, status word, minutes)`
    pub runs: Vec<(String, String, String, i64)>,
    /// `<hash7> <subject>`; `None` = could not be read.
    pub commits: Option<Vec<String>>,
    pub stat: Option<DiffStat>,
    /// The QA report already reduced to front matter line plus body.
    pub qa_report: Option<String>,
    pub total_line: String,
}

/// `text` cut to `max` characters (never inside a character), `CUT_MARK` appended when cut.
fn cut_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('\n');
    out.push_str(CUT_MARK);
    out
}

/// Embedded text must not add headings of its own: `## x` becomes `### x`.
fn demote(text: &str) -> String {
    text.lines()
        .map(|l| {
            if l.starts_with('#') {
                format!("##{l}")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

pub fn file_name(slug: &str) -> String {
    if super::render::slug_ok(slug) {
        format!("{slug}.summary.md")
    } else {
        "summary.md".to_string()
    }
}

pub fn build(input: &SummaryInput) -> String {
    let mut o = String::new();
    let title = input.title.lines().next().unwrap_or("").trim();
    o.push_str(&format!("# Tóm tắt task {}: {}\n\n", input.id, title));
    o.push_str(&format!(
        "- Dự án: {}\n- Nhánh: {}\n",
        input.project, input.branch
    ));
    o.push_str(&format!(
        "- Quy trình: {}\n",
        input.workflow.as_deref().unwrap_or("(không có quy trình)")
    ));
    o.push_str(&format!(
        "- Đội hình: {}\n\n",
        input.team.as_deref().unwrap_or("(không có đội hình)")
    ));

    o.push_str("## Yêu cầu\n\n");
    match input.request.as_deref().filter(|r| !r.trim().is_empty()) {
        Some(r) => o.push_str(r.trim()),
        None => o.push_str(title),
    }
    o.push_str("\n\n");

    o.push_str("## Cách thực hiện\n\n");
    match input.objective.as_deref().filter(|r| !r.trim().is_empty()) {
        Some(obj) => o.push_str(&demote(&cut_chars(obj.trim(), OBJECTIVE_MAX))),
        None => o.push_str("(không có mục tiêu của plan)"),
    }
    o.push_str("\n\n");
    if input.plan_files.is_empty() {
        o.push_str("(không có danh sách file của plan)\n\n");
    } else {
        o.push_str(&format!("File theo plan ({}):\n", input.plan_files.len()));
        for f in input.plan_files.iter().take(PLAN_FILES_MAX) {
            o.push_str(&format!("- {f}\n"));
        }
        if input.plan_files.len() > PLAN_FILES_MAX {
            o.push_str(&format!(
                "- … và {} file nữa\n",
                input.plan_files.len() - PLAN_FILES_MAX
            ));
        }
        o.push('\n');
    }
    if input.runs.is_empty() {
        o.push_str("(không có run)\n\n");
    } else {
        o.push_str("| Vai | Engine | Trạng thái | Phút |\n|---|---|---|---|\n");
        for (role, engine, status, minutes) in &input.runs {
            o.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                cell(role),
                cell(engine),
                cell(status),
                minutes
            ));
        }
        o.push('\n');
    }

    o.push_str("## Kết quả\n\n");
    o.push_str("### Commit\n\n");
    match &input.commits {
        Some(c) if !c.is_empty() => {
            for line in c.iter().take(COMMITS_MAX) {
                o.push_str(&format!("- {}\n", line.replace('\n', " ")));
            }
            if c.len() > COMMITS_MAX {
                o.push_str(&format!("- … và {} commit nữa\n", c.len() - COMMITS_MAX));
            }
        }
        _ => o.push_str("(không có commit)\n"),
    }
    o.push_str("\n### Thay đổi\n\n");
    match &input.stat {
        Some(st) if st.files > 0 => {
            o.push_str(&format!(
                "{} file · +{} · −{}\n\n",
                st.files, st.added, st.removed
            ));
            for (path, a, r) in st.per_file.iter().take(DIFF_FILES_MAX) {
                o.push_str(&format!("- {path} (+{a} −{r})\n"));
            }
            if st.per_file.len() > DIFF_FILES_MAX {
                o.push_str(&format!(
                    "- … và {} file nữa\n",
                    st.per_file.len() - DIFF_FILES_MAX
                ));
            }
        }
        _ => o.push_str("(không có thống kê thay đổi)\n"),
    }
    o.push_str("\n### Kết luận QA\n\n");
    match input.qa_report.as_deref().filter(|r| !r.trim().is_empty()) {
        Some(q) => o.push_str(&demote(&cut_chars(q.trim(), QA_MAX))),
        None => o.push_str("(không có báo cáo QA)"),
    }
    o.push_str("\n\n");

    o.push_str("## Tổng\n\n");
    o.push_str(&input.total_line);
    o.push('\n');
    o
}

/// `git diff --numstat` output into totals (a binary file shows `-` and counts 0).
pub fn parse_numstat(out: &str) -> DiffStat {
    let mut st = DiffStat::default();
    for line in out.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(a), Some(r), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let a = a.parse::<u64>().unwrap_or(0);
        let r = r.parse::<u64>().unwrap_or(0);
        st.files += 1;
        st.added += a;
        st.removed += r;
        st.per_file.push((path.to_string(), a, r));
    }
    st
}

fn qa_text(root: &Path, protocol: &crate::protocol_paths::ProtocolPaths) -> Option<String> {
    let raw = std::fs::read_to_string(root.join(&protocol.qa_report)).ok()?;
    let mut out = String::new();
    let mut body_from = 0;
    if let Some(fm) = report::front_matter(&raw) {
        let mut parts = Vec::new();
        for key in ["specs", "failing", "not_automatable"] {
            if let Some(v) = fm.get(key) {
                parts.push(format!("{key}: {v}"));
            }
        }
        if !parts.is_empty() {
            out.push_str(&parts.join(" · "));
            out.push_str("\n\n");
        }
        body_from = fm.end_line;
    }
    let body: Vec<&str> = raw.lines().skip(body_from).collect();
    out.push_str(body.join("\n").trim());
    Some(out)
}

/// Collects the facts of the task (git, plan file, QA report) and builds the file:
/// `(file name, content)`. A missing or failing source only leaves its `(không có …)` line.
pub fn document(s: &TaskState) -> (String, String) {
    let meta = s.plan_text.as_deref().and_then(planfile::read_meta);
    let request = meta
        .as_ref()
        .and_then(|m| m.get("request"))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let (objective, plan_files) = match s.plan_text.as_deref() {
        Some(text) => {
            let plan = planfile::parse_plan(text);
            (
                plan.sections.get("Mục tiêu").cloned(),
                planfile::plan_files(&plan),
            )
        }
        None => (None, Vec::new()),
    };
    let now = crate::db::now_ms();
    let runs = s
        .runs
        .iter()
        .map(|r| {
            let status = match &r.outcome {
                Some(o) if !o.is_empty() => format!("{}/{}", r.status, o),
                _ => r.status.clone(),
            };
            let ms = (r.ended_at.unwrap_or(now) - r.started_at).max(0);
            (r.role.clone(), r.engine.clone(), status, ms / 60_000)
        })
        .collect();
    let base = s
        .task
        .base_commit
        .as_deref()
        .filter(|b| gitops::is_hex_sha(b));
    let commits = base
        .and_then(|b| gitops::commit_subjects(&s.root, b, "HEAD").ok())
        .map(|v| {
            v.into_iter()
                .map(|(sha, subj)| format!("{} {}", sha.chars().take(7).collect::<String>(), subj))
                .collect::<Vec<_>>()
        });
    let stat = base.and_then(|b| {
        let range = format!("{b}..HEAD");
        gitops::git(&s.root, &["diff", "--numstat", &range])
            .ok()
            .map(|o| parse_numstat(&o))
    });
    let input = SummaryInput {
        id: s.task.id,
        title: s.task.title.clone(),
        slug: s.task.slug.clone(),
        project: s.project.id.clone(),
        branch: s.task.branch.clone(),
        workflow: s.workflow.as_ref().map(|w| w.name.clone()),
        team: s.team.as_ref().map(|t| t.name.clone()),
        request,
        objective,
        plan_files,
        runs,
        commits,
        stat,
        qa_report: qa_text(&s.root, &s.protocol()),
        total_line: super::render::summary_line(s, now),
    };
    (file_name(&s.task.slug), build(&input))
}

/// Collects task report files to be sent over Telegram upon QA pass:
/// `(display filename, file bytes)`.
pub fn report_documents(s: &TaskState) -> Vec<(String, Vec<u8>)> {
    let plan = s.plan_text.as_deref().map(planfile::parse_plan);
    let has_report_sec = plan
        .as_ref()
        .map(|p| p.sections.keys().any(|k| k.starts_with(planfile::REPORT_SECTION)))
        .unwrap_or(false);

    let candidates: Vec<String> = if has_report_sec {
        plan.as_ref().map(planfile::report_files).unwrap_or_default()
    } else {
        let base = s
            .task
            .base_commit
            .as_deref()
            .filter(|b| gitops::is_hex_sha(b));
        match base {
            Some(b) => {
                let range = format!("{b}..{}", s.task.branch);
                let diff_out = gitops::git(&s.root, &["diff", "--name-only", "--diff-filter=A", &range])
                    .unwrap_or_default();
                let p_files = plan.as_ref().map(planfile::plan_files).unwrap_or_default();
                let mut list = Vec::new();
                for line in diff_out.lines() {
                    let raw = line.trim();
                    if raw.is_empty() {
                        continue;
                    }
                    let norm = planfile::normalize_plan_path(raw);
                    let lower = norm.to_lowercase();
                    if !lower.ends_with(".md") {
                        continue;
                    }
                    let base_name = norm.rsplit('/').next().unwrap_or(&norm);
                    if crate::protocol_paths::is_protocol_file(&norm)
                        || base_name == crate::roles::dev::HANDOVER_FILE
                    {
                        continue;
                    }
                    if p_files.contains(&norm) && !list.contains(&norm) {
                        list.push(norm);
                    }
                }
                list
            }
            None => Vec::new(),
        }
    };

    // 2. Safety filter
    let mut filtered = Vec::new();
    for path in candidates {
        if report::sensitive_path(&path) {
            continue;
        }
        let lower = path.to_lowercase();
        if !(lower.ends_with(".md") || lower.ends_with(".txt") || lower.ends_with(".csv")) {
            continue;
        }
        if !filtered.contains(&path) {
            filtered.push(path);
        }
        if filtered.len() == 5 {
            break;
        }
    }

    // 3. Read content from git branch
    let mut kept = Vec::new();
    for path in filtered {
        let bytes = match gitops::show_file_bytes(&s.root, &s.task.branch, &path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if bytes.len() > MAX_REPORT_FILE_SIZE {
            crate::term::emit(&format!("Telegram: bỏ file báo cáo {path} — lớn hơn 1 MiB"));
            continue;
        }
        kept.push((path, bytes));
    }

    // 4. Display filename
    let mut name_counts = std::collections::HashMap::new();
    for (path, _) in &kept {
        let base = Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path.as_str());
        *name_counts.entry(base.to_string()).or_insert(0usize) += 1;
    }

    let mut results = Vec::new();
    for (path, bytes) in kept {
        let base = Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path.as_str())
            .to_string();
        let display_name = if name_counts.get(&base).copied().unwrap_or(0) > 1 {
            let parent = Path::new(&path)
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if !parent.is_empty() {
                format!("{parent}-{base}")
            } else {
                base
            }
        } else {
            base
        };
        results.push((display_name, bytes));
    }
    results
}

/// How a product file goes to Telegram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductKind {
    /// `sendVideo` (`.mp4`, `.mov`, `.webm`).
    Video,
    /// `sendDocument`.
    Document,
    /// Larger than `TELEGRAM_UPLOAD_MAX_BYTES`: only a text with its path is sent.
    TooLarge,
}

/// One `## Sản phẩm` file ready to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductSend {
    /// Absolute path, resolved through symlinks (inside the project root).
    pub path: PathBuf,
    /// Display name: the base name of the plan entry.
    pub filename: String,
    pub size: u64,
    pub kind: ProductKind,
}

/// The `## Sản phẩm` files of a passed task, read from the working tree at `s.root`, with the
/// same checks as the QA product check (exists, regular file, more than 0 bytes, inside the
/// canonical project root after symlinks) plus the hard denylist of paths. Missing files are
/// skipped (the QA check already stopped such a task).
pub fn product_files_for_send(s: &TaskState) -> Vec<ProductSend> {
    let mut commits: Vec<&str> = Vec::new();
    if let Some(a) = s.auto.approved_commit.as_deref() {
        commits.push(a);
    }
    let files = crate::roles::qa::task_product_files(
        &s.root,
        &commits,
        s.plan_text.as_deref(),
        &s.protocol(),
    );
    let root_real = s.root.canonicalize().ok();
    let mut out = Vec::new();
    for rel in files.iter().take(planfile::MAX_PRODUCT_FILES) {
        if crate::guard::grant_denied_rel(rel) {
            continue;
        }
        let Ok(found) = crate::roles::qa::check_product(&s.root, rel) else {
            continue;
        };
        let resolved_rel = root_real
            .as_ref()
            .and_then(|r| found.path.strip_prefix(r).ok())
            .map(|p| p.to_string_lossy().to_string());
        match resolved_rel {
            Some(r) if !crate::guard::grant_denied_rel(&r) => {}
            _ => continue,
        }
        let filename = Path::new(rel)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(rel.as_str())
            .to_string();
        let ext = Path::new(rel)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let kind = if found.size > TELEGRAM_UPLOAD_MAX_BYTES {
            ProductKind::TooLarge
        } else if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
            ProductKind::Video
        } else {
            ProductKind::Document
        };
        out.push(ProductSend {
            path: found.path,
            filename,
            size: found.size,
            kind,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> SummaryInput {
        SummaryInput {
            id: 7,
            title: "sửa app".to_string(),
            slug: "sua-app".to_string(),
            project: "web".to_string(),
            branch: "task/sua-app".to_string(),
            workflow: Some("mac-dinh".to_string()),
            team: Some("doi-a".to_string()),
            request: Some("Sửa nút đăng nhập".to_string()),
            objective: Some("Làm nút chạy đúng.".to_string()),
            plan_files: vec!["src/a.rs".to_string()],
            runs: vec![("dev".into(), "claude".into(), "done/ok".into(), 3)],
            commits: Some(vec!["abc1234 feat: x".to_string()]),
            stat: Some(DiffStat {
                files: 1,
                added: 4,
                removed: 1,
                per_file: vec![("src/a.rs".to_string(), 4, 1)],
            }),
            qa_report: Some("specs: 2\n\n## Kết luận\nĐạt".to_string()),
            total_line: "Tổng: 1 run · 3 phút · $0.10".to_string(),
        }
    }

    fn pos(text: &str, needle: &str) -> usize {
        text.find(needle)
            .unwrap_or_else(|| panic!("missing {needle}"))
    }

    #[test]
    fn five_sections_in_order() {
        let t = build(&full());
        assert!(t.starts_with("# Tóm tắt task 7: sửa app"));
        let marks = ["## Yêu cầu", "## Cách thực hiện", "## Kết quả", "## Tổng"];
        let mut last = 0;
        for m in marks {
            let p = pos(&t, m);
            assert!(p >= last, "{m}");
            last = p;
        }
        assert!(t.contains("Sửa nút đăng nhập"));
        assert!(t.contains("| dev | claude | done/ok | 3 |"));
        assert!(t.contains("- abc1234 feat: x"));
        assert!(t.contains("1 file · +4 · −1"));
        assert!(t.contains("### Kết luận\nĐạt"), "{t}");
        assert!(t.trim_end().ends_with("Tổng: 1 run · 3 phút · $0.10"));
    }

    #[test]
    fn missing_facts_give_one_line_each() {
        let mut i = full();
        i.request = None;
        i.objective = None;
        i.plan_files.clear();
        i.runs.clear();
        i.commits = None;
        i.stat = None;
        i.qa_report = None;
        i.workflow = None;
        i.team = None;
        let t = build(&i);
        assert!(t.contains("## Yêu cầu\n\nsửa app"));
        for m in [
            "(không có mục tiêu của plan)",
            "(không có danh sách file của plan)",
            "(không có run)",
            "(không có commit)",
            "(không có thống kê thay đổi)",
            "(không có báo cáo QA)",
        ] {
            assert_eq!(t.matches(m).count(), 1, "{m}\n{t}");
        }
        for m in ["## Yêu cầu", "## Cách thực hiện", "## Kết quả", "## Tổng"] {
            assert!(t.contains(m));
        }
    }

    #[test]
    fn limits_and_utf8() {
        let mut i = full();
        i.objective = Some("ẳ".repeat(3500));
        i.qa_report = Some("ẵ".repeat(9000));
        i.commits = Some((0..60).map(|n| format!("h{n:06} s")).collect());
        i.plan_files = (0..60).map(|n| format!("f{n}.rs")).collect();
        i.stat = Some(DiffStat {
            files: 120,
            added: 1,
            removed: 1,
            per_file: (0..120).map(|n| (format!("p{n}"), 1, 0)).collect(),
        });
        let t = build(&i);
        assert_eq!(t.matches('ẳ').count(), 3000);
        assert_eq!(t.matches('ẵ').count(), 8000);
        assert_eq!(t.matches(CUT_MARK).count(), 2);
        assert!(t.contains("- h000049 s"));
        assert!(!t.contains("- h000050 s"));
        assert!(t.contains("- … và 10 commit nữa"));
        assert!(t.contains("- f49.rs") && !t.contains("- f50.rs"));
        assert!(t.contains("- p99 (+1 −0)") && !t.contains("- p100 "));
    }

    #[test]
    fn embedded_headings_are_demoted() {
        let mut i = full();
        i.objective = Some("## Lạc\nx".to_string());
        let t = build(&i);
        assert!(t.contains("### Lạc"));
        assert_eq!(t.matches("\n## Kết quả").count(), 1);
    }

    #[test]
    fn numstat_parse() {
        let st = parse_numstat("3\t1\ta.rs\n-\t-\timg.png\n\nbad\n");
        assert_eq!(st.files, 2);
        assert_eq!((st.added, st.removed), (3, 1));
        assert_eq!(st.per_file[1], ("img.png".to_string(), 0, 0));
    }

    #[test]
    fn file_names() {
        assert_eq!(file_name("sua-app"), "sua-app.summary.md");
        assert_eq!(file_name("Sua App"), "summary.md");
        assert_eq!(file_name(""), "summary.md");
    }

    fn test_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(dir.path())
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .unwrap();
        assert!(status.success());
        dir
    }

    fn test_commit(root: &Path, files: &[(&str, &[u8])], msg: &str) -> String {
        for (rel, content) in files {
            let p = root.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, content).unwrap();
        }
        let status = std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .unwrap();
        assert!(status.success());

        let output = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                msg,
            ])
            .current_dir(root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .unwrap();
        assert!(output.status.success());
        gitops::head_sha(root).unwrap()
    }

    fn test_task_state(root: &Path, branch: &str, base: Option<&str>, plan: Option<&str>) -> TaskState {
        let task = crate::db::TaskRow {
            id: 1,
            project_id: 1,
            slug: "test-task".to_string(),
            title: "test task".to_string(),
            branch: branch.to_string(),
            base_branch: Some("main".to_string()),
            base_commit: base.map(|s| s.to_string()),
            status: "planned".to_string(),
            created_at: 0,
            updated_at: 0,
            level: "vua".to_string(),
            workdir: None,
        };
        let project = crate::projects::Project {
            id: "test-proj".to_string(),
            path: root.to_string_lossy().to_string(),
            name_vi: "Dự án test".to_string(),
            aliases: Vec::new(),
            kind: "code".to_string(),
            profile: "default".to_string(),
            repo: None,
            missing: false,
            stack: vec![],
            read_sources: vec![],
            protocol_dir: String::new(),
            workspace: Default::default(),
        };
        let (teams, staff) = crate::teams::load();
        TaskState {
            task,
            project,
            root: root.to_path_buf(),
            git: crate::dashboard::taskstate::GitFacts {
                current_branch: Some(branch.to_string()),
                ..Default::default()
            },
            plan_text: plan.map(|s| s.to_string()),
            plan_source: Some("branch"),
            plan_dirty: false,
            busy: None,
            auto: crate::db::TaskAuto::default(),
            aj: crate::autopilot::decide::AutoJson::default(),
            workflow: None,
            driver_alive: false,
            runs: Vec::new(),
            gate: None,
            view: crate::dashboard::taskstate::TaskView {
                steps: Vec::new(),
                banner: None,
                group: String::new(),
                current_step: String::new(),
                actions: Vec::new(),
                controls: Vec::new(),
                auto: crate::dashboard::taskstate::AutoView {
                    state: String::new(),
                    legacy: false,
                    workflow_auto: false,
                    stop_reason: None,
                    stop_word: None,
                    stop_detail: None,
                    stopped_in: None,
                    stop_requested: false,
                },
            },
            rev: "rev1".to_string(),
            origin: None,
            project_telegram_ok: false,
            team_json: None,
            team: None,
            teams,
            staff,
            plan_diff_summary: None,
            quota_teams: Vec::new(),
            fixbug_key: None,
        }
    }

    #[test]
    fn test_report_documents_cases() {
        let repo = test_repo();
        let root = repo.path();
        let base_sha = test_commit(root, &[("README.md", b"# initial\n")], "initial commit");

        // Create branch task/test
        gitops::create_branch(root, "task/test").unwrap();
        // Switch to branch task/test
        let status = std::process::Command::new("git")
            .args(["checkout", "task/test"])
            .current_dir(root)
            .status()
            .unwrap();
        assert!(status.success());

        // Case 1: Plan có `## Gửi báo cáo` với `reports/a.md`, đã commit trên nhánh task:
        // trả 1 phần tử ("a.md", nội dung).
        test_commit(root, &[("reports/a.md", b"report a content")], "add report a");
        let plan_case1 = "# Plan: t\n## Mục tiêu\nx\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | reports/a.md | - | x | y |\n## Không được đụng tới\n-\n## Cách kiểm tra\n-\n## Vướng mắc\n## Gửi báo cáo\n- `reports/a.md`\n";
        let s1 = test_task_state(root, "task/test", Some(&base_sha), Some(plan_case1));
        let docs1 = report_documents(&s1);
        assert_eq!(docs1.len(), 1);
        assert_eq!(docs1[0].0, "a.md");
        assert_eq!(docs1[0].1, b"report a content");

        // Case 4: Worktree có nội dung khác chưa commit: vẫn trả nội dung ở nhánh (đọc từ git).
        std::fs::write(root.join("reports/a.md"), b"modified in worktree").unwrap();
        let docs4 = report_documents(&s1);
        assert_eq!(docs4.len(), 1);
        assert_eq!(docs4[0].1, b"report a content");
        // Revert worktree file
        std::fs::write(root.join("reports/a.md"), b"report a content").unwrap();

        // Case 2: Không có mục; nhánh task thêm mới `reports/b.md` (có trong Thay đổi), sửa `README.md` và thêm `src/x.php`: chỉ trả `b.md`.
        test_commit(root, &[
            ("reports/b.md", b"report b content"),
            ("README.md", b"# updated readme\n"),
            ("src/x.php", b"<?php echo 1;"),
        ], "add files");
        let plan_case2 = "# Plan: t\n## Mục tiêu\nx\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | reports/b.md | - | x | y |\n| 2 | README.md | - | x | y |\n| 3 | src/x.php | - | x | y |\n## Không được đụng tới\n-\n## Cách kiểm tra\n-\n## Vướng mắc\n";
        let s2 = test_task_state(root, "task/test", Some(&base_sha), Some(plan_case2));
        let docs2 = report_documents(&s2);
        assert_eq!(docs2.len(), 1);
        assert_eq!(docs2[0].0, "b.md");
        assert_eq!(docs2[0].1, b"report b content");

        // Case 3: File lớn hơn 1 MiB: bị bỏ qua.
        let big_bytes = vec![b'x'; MAX_REPORT_FILE_SIZE + 10];
        test_commit(root, &[("reports/big.md", &big_bytes)], "add big report");
        let plan_case3 = "# Plan: t\n## Mục tiêu\nx\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | reports/big.md | - | x | y |\n## Không được đụng tới\n-\n## Cách kiểm tra\n-\n## Vướng mắc\n## Gửi báo cáo\n- `reports/big.md`\n";
        let s3 = test_task_state(root, "task/test", Some(&base_sha), Some(plan_case3));
        let docs3 = report_documents(&s3);
        assert!(docs3.is_empty(), "big file must be skipped: {docs3:?}");
    }

    #[test]
    fn product_files_for_send_kinds_and_checks() {
        let repo = test_repo();
        let root = repo.path();
        test_commit(root, &[(".gitignore", b"out/\n")], "initial commit");
        std::fs::create_dir_all(root.join("out")).unwrap();
        std::fs::write(root.join("out/v.MP4"), b"video").unwrap();
        std::fs::write(root.join("out/r.pdf"), b"%PDF").unwrap();
        std::fs::write(root.join("out/empty.webm"), b"").unwrap();
        let big = std::fs::File::create(root.join("out/big.mov")).unwrap();
        big.set_len(TELEGRAM_UPLOAD_MAX_BYTES + 1).unwrap();
        drop(big);
        let other = tempfile::tempdir().unwrap();
        std::fs::write(other.path().join("x.mp4"), b"x").unwrap();
        std::os::unix::fs::symlink(other.path().join("x.mp4"), root.join("out/away.mp4")).unwrap();
        let plan = "# Plan: t\n## Mục tiêu\nx\n## Thay đổi\n| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n| 1 | a.txt | - | x | y |\n## Không được đụng tới\n-\n## Cách kiểm tra\n-\n## Vướng mắc\n## Thư mục được ghi\n- `out/**`\n## Sản phẩm\n- `out/v.MP4`\n- `out/missing.mp4`\n- `out/r.pdf`\n- `out/big.mov`\n- `out/empty.webm`\n- `out/away.mp4`\n";
        let s = test_task_state(root, "main", None, Some(plan));
        let got = product_files_for_send(&s);
        let names: Vec<(&str, ProductKind, u64)> = got
            .iter()
            .map(|p| (p.filename.as_str(), p.kind, p.size))
            .collect();
        // at most 5 entries are looked at: `out/away.mp4` is the 6th
        assert_eq!(
            names,
            vec![
                ("v.MP4", ProductKind::Video, 5),
                ("r.pdf", ProductKind::Document, 4),
                (
                    "big.mov",
                    ProductKind::TooLarge,
                    TELEGRAM_UPLOAD_MAX_BYTES + 1
                ),
            ]
        );
        let root_real = root.canonicalize().unwrap();
        assert!(got
            .iter()
            .all(|p| p.path.is_absolute() && p.path.starts_with(&root_real)));

        // a symlink out of the root is never sent
        let plan2 = plan.replace("- `out/v.MP4`\n- `out/missing.mp4`\n", "");
        let s2 = test_task_state(root, "main", None, Some(&plan2));
        assert!(product_files_for_send(&s2)
            .iter()
            .all(|p| p.filename != "away.mp4"));
        // no product section: nothing
        let s3 = test_task_state(root, "main", None, Some("# Plan: t\n## Mục tiêu\nx\n"));
        assert!(product_files_for_send(&s3).is_empty());
    }
}
