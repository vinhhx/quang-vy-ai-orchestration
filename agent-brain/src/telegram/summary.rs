//! Task summary file sent before the "QA đạt" message: what was asked, how it was done and
//! the result. Deterministic (no LLM): built from `.plan.md`, the git history of the task
//! branch, `.qa-report.md` and the runs. Never includes diff content.

use crate::dashboard::api_tasks::TaskState;
use crate::{gitops, planfile, report};
use std::path::Path;

/// Longest objective copied from the plan, in characters.
pub const OBJECTIVE_MAX: usize = 3000;
/// Longest QA conclusion copied from `.qa-report.md`, in characters.
pub const QA_MAX: usize = 8000;
pub const COMMITS_MAX: usize = 50;
pub const DIFF_FILES_MAX: usize = 100;
pub const PLAN_FILES_MAX: usize = 50;
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
    /// `.qa-report.md` already reduced to front matter line plus body.
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

fn qa_text(root: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(root.join(report::QA_REPORT_FILE)).ok()?;
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

/// Collects the facts of the task (git, `.plan.md`, `.qa-report.md`) and builds the file:
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
        qa_report: qa_text(&s.root),
        total_line: super::render::summary_line(s, now),
    };
    (file_name(&s.task.slug), build(&input))
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
}
