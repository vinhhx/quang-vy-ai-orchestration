pub const PLAN_REVIEW_FILE: &str = ".plan-review.md";
pub const CODE_REVIEW_FILE: &str = ".code-review.md";
pub const QA_REPORT_FILE: &str = ".qa-report.md";
pub const MAX_NAMED_FILES: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewKind {
    Plan,
    Code,
}

impl ReviewKind {
    pub fn file(&self) -> &'static str {
        match self {
            ReviewKind::Plan => PLAN_REVIEW_FILE,
            ReviewKind::Code => CODE_REVIEW_FILE,
        }
    }

    pub fn role(&self) -> &'static str {
        match self {
            ReviewKind::Plan => "plan-review",
            ReviewKind::Code => "code-review",
        }
    }

    pub fn ok_verdict(&self) -> &'static str {
        match self {
            ReviewKind::Plan => "approve",
            ReviewKind::Code => "pass",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            ReviewKind::Plan => "Review plan",
            ReviewKind::Code => "Review code",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FrontMatter {
    pub fields: Vec<(String, String)>,
    pub end_line: usize,
}

impl FrontMatter {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

pub fn front_matter(text: &str) -> Option<FrontMatter> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut lines = text.lines().enumerate();
    let (_, first_line) = lines.next()?;
    if first_line.trim_end() != "---" {
        return None;
    }

    let mut fields = Vec::new();
    let mut end_line = None;

    for (idx, line) in lines {
        let trimmed_end = line.trim_end();
        if trimmed_end == "---" {
            end_line = Some(idx + 1); // 1-based line number
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            if key.is_empty() || !key.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                return None;
            }
            let val = v.trim();
            fields.push((key.to_string(), val.to_string()));
        } else {
            return None;
        }
    }

    let end_line = end_line?;
    Some(FrontMatter { fields, end_line })
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewReport {
    pub verdict: String,
    pub blocking: i64,
    pub advisory: i64,
    pub files: Vec<String>,
}

pub fn parse_review(text: &str, kind: ReviewKind) -> Result<ReviewReport, String> {
    let fm = front_matter(text)
        .ok_or_else(|| "thiếu khối front matter (--- ... ---) ở đầu file".to_string())?;

    let verdict_str = fm.get("verdict").unwrap_or("").trim();
    if verdict_str.is_empty() {
        return Err("thiếu verdict".to_string());
    }

    let ok_v = kind.ok_verdict();
    if verdict_str != ok_v && verdict_str != "revise" {
        return Err(format!(
            "verdict \"{verdict_str}\" không hợp lệ (dùng {ok_v} hoặc revise)"
        ));
    }

    let blocking_str = fm.get("blocking").unwrap_or("");
    let blocking = match blocking_str.parse::<i64>() {
        Ok(n) if n >= 0 => n,
        _ => return Err("\"blocking\" phải là số nguyên không âm".to_string()),
    };

    let advisory_str = fm.get("advisory").unwrap_or("");
    let advisory = match advisory_str.parse::<i64>() {
        Ok(n) if n >= 0 => n,
        _ => return Err("\"advisory\" phải là số nguyên không âm".to_string()),
    };

    if verdict_str == "revise" && blocking == 0 {
        return Err("verdict revise cần ít nhất 1 mục chặn (blocking)".to_string());
    }

    if verdict_str != "revise" && blocking > 0 {
        return Err(format!("có {blocking} mục chặn nên verdict phải là revise"));
    }

    let files = if let Some(files_str) = fm.get("files") {
        serde_json::from_str::<Vec<String>>(files_str)
            .map_err(|_| "\"files\" phải là danh sách JSON các đường dẫn".to_string())?
    } else {
        Vec::new()
    };

    Ok(ReviewReport {
        verdict: verdict_str.to_string(),
        blocking,
        advisory,
        files,
    })
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct QaCounts {
    pub specs: Option<i64>,
    pub failing: Option<i64>,
    pub not_automatable: Option<i64>,
    pub files: Vec<String>,
}

pub fn parse_qa_counts(text: &str) -> QaCounts {
    let fm = match front_matter(text) {
        Some(fm) => fm,
        None => return QaCounts::default(),
    };

    let specs = fm
        .get("specs")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|&n| n >= 0);
    let failing = fm
        .get("failing")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|&n| n >= 0);
    let not_automatable = fm
        .get("not_automatable")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|&n| n >= 0);
    let files = fm
        .get("files")
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default();

    QaCounts {
        specs,
        failing,
        not_automatable,
        files,
    }
}

pub fn review_metrics(r: &ReviewReport) -> serde_json::Value {
    serde_json::json!({
        "verdict": r.verdict,
        "blocking": r.blocking,
        "advisory": r.advisory,
    })
}

pub fn qa_metrics(c: &QaCounts) -> serde_json::Value {
    serde_json::json!({
        "specs": c.specs,
        "failing": c.failing,
        "not_automatable": c.not_automatable,
    })
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NamedFiles {
    pub files: Vec<String>,
    pub rejected: Vec<(String, String)>,
}

pub fn named_files(report_text: &str, root: &std::path::Path) -> NamedFiles {
    named_files_with(report_text, root, None)
}

/// Files a review / QA report names, for widening the fix-mode write set (D-12).
///
/// Front-matter `files` are the explicit way to name a file (new files included).
/// Backticked body tokens are a heuristic: when `changed` is `Some`, a body token
/// is kept only if that file already changed since the plan commit; otherwise it
/// is rejected as `not_in_diff:{path}` (WR-06).
pub fn named_files_with(
    report_text: &str,
    root: &std::path::Path,
    changed: Option<&std::collections::BTreeSet<String>>,
) -> NamedFiles {
    let fm_opt = front_matter(report_text);
    // (path, from_body)
    let mut raw_candidates: Vec<(String, bool)> = Vec::new();

    if let Some(ref fm) = fm_opt {
        if let Some(files_str) = fm.get("files") {
            if let Ok(files_list) = serde_json::from_str::<Vec<String>>(files_str) {
                raw_candidates.extend(files_list.into_iter().map(|f| (f, false)));
            }
        }
    }

    let body_text = if let Some(ref fm) = fm_opt {
        let mut lines = report_text.lines();
        for _ in 0..fm.end_line {
            lines.next();
        }
        lines.collect::<Vec<_>>().join("\n")
    } else {
        report_text.to_string()
    };

    static BACKTICK_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let backtick_re = BACKTICK_RE.get_or_init(|| regex::Regex::new(r"`([^`\n]+)`").unwrap());

    static SUFFIX_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let suffix_re = SUFFIX_RE.get_or_init(|| regex::Regex::new(r":\d+(?:-\d+)?$").unwrap());

    const KNOWN_EXTS: &[&str] = &[
        ".rs", ".py", ".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx", ".json", ".toml", ".yaml",
        ".yml", ".md", ".html", ".css", ".scss", ".sh", ".go", ".java", ".kt", ".swift", ".c",
        ".h", ".cc", ".cpp", ".hpp", ".rb", ".php", ".sql", ".txt", ".vue", ".svelte", ".xml",
        ".ini", ".cfg",
    ];

    for caps in backtick_re.captures_iter(&body_text) {
        let token = caps[1].trim();
        let stripped = suffix_re.replace(token, "");
        let s = stripped.trim();
        if !s.contains(char::is_whitespace)
            && (s.contains('/') || KNOWN_EXTS.iter().any(|ext| s.ends_with(ext)))
        {
            raw_candidates.push((s.to_string(), true));
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut candidates = Vec::new();
    for (raw, from_body) in raw_candidates {
        let norm = crate::planfile::normalize_plan_path(&raw);
        if norm.is_empty() {
            continue;
        }
        if seen.insert(norm.clone()) {
            candidates.push((norm, from_body));
        }
    }

    let mut files = Vec::new();
    let mut rejected = Vec::new();

    for (f, from_body) in candidates {
        // 1. planfile::path_error
        if let Some(err) = crate::planfile::path_error(&f, Some(root)) {
            rejected.push((f, err));
            continue;
        }

        // 2. sensitive
        let basename = f.rsplit('/').next().unwrap_or(&f);
        let is_sensitive = basename.starts_with(".env")
            || basename.ends_with(".pem")
            || basename.ends_with(".key")
            || basename.starts_with("id_rsa")
            || basename.starts_with("id_ed25519")
            || f == ".claude"
            || f.starts_with(".claude/")
            || f.contains("/.claude/");

        if is_sensitive {
            rejected.push((f.clone(), format!("sensitive:{f}")));
            continue;
        }

        // 3. body tokens only for files already changed since the plan commit (WR-06)
        if from_body {
            if let Some(set) = changed {
                if !set.contains(&f) {
                    rejected.push((f.clone(), format!("not_in_diff:{f}")));
                    continue;
                }
            }
        }

        // 4. untracked
        let p = root.join(&f);
        if std::fs::symlink_metadata(&p).is_ok() && !crate::gitops::is_tracked(root, &f) {
            rejected.push((f.clone(), format!("untracked:{f}")));
            continue;
        }

        // 5. cap
        if files.len() < MAX_NAMED_FILES {
            files.push(f);
        } else {
            rejected.push((f.clone(), format!("too_many:{f}")));
        }
    }

    NamedFiles { files, rejected }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_review_ok() {
        let text_plan = "---\nverdict: revise\nblocking: 1\nadvisory: 4\nfiles: [\"src/a.py\", \"tests/t.py\"]\n---\n# Review plan: x\n";
        let report = parse_review(text_plan, ReviewKind::Plan).unwrap();
        assert_eq!(
            report,
            ReviewReport {
                verdict: "revise".to_string(),
                blocking: 1,
                advisory: 4,
                files: vec!["src/a.py".to_string(), "tests/t.py".to_string()],
            }
        );

        let text_approve = "---\nverdict: approve\nblocking: 0\nadvisory: 2\n---\n";
        let report_app = parse_review(text_approve, ReviewKind::Plan).unwrap();
        assert_eq!(
            report_app,
            ReviewReport {
                verdict: "approve".to_string(),
                blocking: 0,
                advisory: 2,
                files: vec![],
            }
        );

        let text_code = "---\nverdict: pass\nblocking: 0\nadvisory: 0\n---\n";
        let report_code = parse_review(text_code, ReviewKind::Code).unwrap();
        assert_eq!(report_code.verdict, "pass");
    }

    #[test]
    fn parse_review_errors() {
        // no front matter
        let err = parse_review("# Just a title\n", ReviewKind::Plan).unwrap_err();
        assert_eq!(err, "thiếu khối front matter (--- ... ---) ở đầu file");

        // empty verdict
        let err = parse_review(
            "---\nverdict:\nblocking: 0\nadvisory: 0\n---\n",
            ReviewKind::Plan,
        )
        .unwrap_err();
        assert_eq!(err, "thiếu verdict");

        // Plan with verdict pass
        let err = parse_review(
            "---\nverdict: pass\nblocking: 0\nadvisory: 0\n---\n",
            ReviewKind::Plan,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "verdict \"pass\" không hợp lệ (dùng approve hoặc revise)"
        );

        // blocking -1 or x
        let err = parse_review(
            "---\nverdict: approve\nblocking: -1\nadvisory: 0\n---\n",
            ReviewKind::Plan,
        )
        .unwrap_err();
        assert_eq!(err, "\"blocking\" phải là số nguyên không âm");
        let err = parse_review(
            "---\nverdict: approve\nblocking: x\nadvisory: 0\n---\n",
            ReviewKind::Plan,
        )
        .unwrap_err();
        assert_eq!(err, "\"blocking\" phải là số nguyên không âm");

        // advisory -1 or x
        let err = parse_review(
            "---\nverdict: approve\nblocking: 0\nadvisory: -1\n---\n",
            ReviewKind::Plan,
        )
        .unwrap_err();
        assert_eq!(err, "\"advisory\" phải là số nguyên không âm");
        let err = parse_review(
            "---\nverdict: approve\nblocking: 0\nadvisory: x\n---\n",
            ReviewKind::Plan,
        )
        .unwrap_err();
        assert_eq!(err, "\"advisory\" phải là số nguyên không âm");

        // revise with blocking 0
        let err = parse_review(
            "---\nverdict: revise\nblocking: 0\nadvisory: 2\n---\n",
            ReviewKind::Plan,
        )
        .unwrap_err();
        assert_eq!(err, "verdict revise cần ít nhất 1 mục chặn (blocking)");

        // approve with blocking 2
        let err = parse_review(
            "---\nverdict: approve\nblocking: 2\nadvisory: 1\n---\n",
            ReviewKind::Plan,
        )
        .unwrap_err();
        assert_eq!(err, "có 2 mục chặn nên verdict phải là revise");

        // files: "src/a.py" (not a JSON array)
        let err = parse_review(
            "---\nverdict: approve\nblocking: 0\nadvisory: 0\nfiles: \"src/a.py\"\n---\n",
            ReviewKind::Plan,
        )
        .unwrap_err();
        assert_eq!(err, "\"files\" phải là danh sách JSON các đường dẫn");

        // front matter without closing ---
        let err = parse_review(
            "---\nverdict: approve\nblocking: 0\nadvisory: 0\n",
            ReviewKind::Plan,
        )
        .unwrap_err();
        assert_eq!(err, "thiếu khối front matter (--- ... ---) ở đầu file");
    }

    #[test]
    fn front_matter_strictness() {
        // body line before closing ---
        let text = "---\nverdict: approve\nSome prose\nblocking: 0\n---\n";
        assert!(front_matter(text).is_none());

        // blank lines inside are allowed
        let text_blanks = "---\nverdict: approve\n\nblocking: 0\n\nadvisory: 1\n---\n";
        let fm = front_matter(text_blanks).unwrap();
        assert_eq!(fm.get("verdict"), Some("approve"));
        assert_eq!(fm.get("blocking"), Some("0"));
        assert_eq!(fm.get("advisory"), Some("1"));
        assert_eq!(fm.end_line, 7);

        // BOM accepted
        let text_bom = "\u{FEFF}---\nverdict: approve\nblocking: 0\nadvisory: 0\n---\n";
        let fm_bom = front_matter(text_bom).unwrap();
        assert_eq!(fm_bom.get("verdict"), Some("approve"));
        assert_eq!(fm_bom.end_line, 5);
    }

    #[test]
    fn qa_counts() {
        let text = "---\nspecs: 3\nfailing: 1\n---\n# QA: x";
        let qc = parse_qa_counts(text);
        assert_eq!(
            qc,
            QaCounts {
                specs: Some(3),
                failing: Some(1),
                not_automatable: None,
                files: vec![],
            }
        );

        let no_fm = "# QA: x\n";
        let qc_no_fm = parse_qa_counts(no_fm);
        assert_eq!(qc_no_fm, QaCounts::default());

        let invalid_counts = "---\nspecs: abc\nfailing: -2\n---\n";
        let qc_inv = parse_qa_counts(invalid_counts);
        assert_eq!(qc_inv.specs, None);
        assert_eq!(qc_inv.failing, None);

        let metrics = qa_metrics(&qc);
        assert_eq!(
            metrics,
            serde_json::json!({
                "specs": 3,
                "failing": 1,
                "not_automatable": serde_json::Value::Null,
            })
        );
        assert_eq!(metrics["specs"].as_i64(), Some(3));
        assert_eq!(metrics["failing"].as_i64(), Some(1));
    }

    #[test]
    fn review_kind_helpers() {
        assert_eq!(ReviewKind::Plan.file(), PLAN_REVIEW_FILE);
        assert_eq!(ReviewKind::Code.file(), CODE_REVIEW_FILE);
        assert_eq!(ReviewKind::Plan.role(), "plan-review");
        assert_eq!(ReviewKind::Code.role(), "code-review");
        assert_eq!(ReviewKind::Plan.ok_verdict(), "approve");
        assert_eq!(ReviewKind::Code.ok_verdict(), "pass");
        assert_eq!(ReviewKind::Plan.title(), "Review plan");
        assert_eq!(ReviewKind::Code.title(), "Review code");
    }

    #[test]
    fn review_metrics_shape() {
        let report = ReviewReport {
            verdict: "revise".to_string(),
            blocking: 1,
            advisory: 4,
            files: vec![],
        };
        let metrics = review_metrics(&report);
        assert_eq!(
            metrics,
            serde_json::json!({
                "verdict": "revise",
                "blocking": 1,
                "advisory": 4,
            })
        );
        assert_eq!(metrics["verdict"].as_str(), Some("revise"));
        assert_eq!(metrics["blocking"].as_i64(), Some(1));
        assert_eq!(metrics["advisory"].as_i64(), Some(4));
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

    fn test_commit(root: &std::path::Path, files: &[(&str, &str)], msg: &str) {
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
    }

    #[test]
    fn named_files_union_and_suffix() {
        let dir = test_repo();
        let root = dir.path();
        test_commit(root, &[("src/a.py", "# a"), ("src/b.py", "# b")], "init");

        let report = "---\nverdict: revise\nblocking: 1\nadvisory: 0\nfiles: [\"src/a.py\"]\n---\nSửa `src/b.py:12` và `tests/t_b.py`; xem `ghi chú dài`\n";
        let nf = named_files(report, root);
        assert_eq!(
            nf.files,
            vec![
                "src/a.py".to_string(),
                "src/b.py".to_string(),
                "tests/t_b.py".to_string()
            ]
        );
        assert!(nf.rejected.is_empty(), "rejected was: {:?}", nf.rejected);
    }

    #[test]
    fn named_files_body_needs_diff() {
        let dir = test_repo();
        let root = dir.path();
        test_commit(root, &[("src/a.py", "# a"), ("src/b.py", "# b")], "init");

        let report = "---\nverdict: revise\nblocking: 1\nadvisory: 0\nfiles: [\"src/new.py\"]\n---\nSửa `src/a.py:3` và `src/b.py`\n";
        let changed: std::collections::BTreeSet<String> =
            ["src/a.py".to_string()].into_iter().collect();

        let nf = named_files_with(report, root, Some(&changed));
        assert_eq!(
            nf.files,
            vec!["src/new.py".to_string(), "src/a.py".to_string()]
        );
        assert!(
            nf.rejected
                .contains(&("src/b.py".to_string(), "not_in_diff:src/b.py".to_string())),
            "rejected was: {:?}",
            nf.rejected
        );

        let nf_all = named_files_with(report, root, None);
        assert_eq!(
            nf_all.files,
            vec![
                "src/new.py".to_string(),
                "src/a.py".to_string(),
                "src/b.py".to_string()
            ]
        );
        assert!(nf_all.rejected.is_empty());
        assert_eq!(named_files(report, root), nf_all);
    }

    #[test]
    fn named_files_rejections() {
        let dir = test_repo();
        let root = dir.path();
        test_commit(root, &[("src/main.rs", "// main")], "init");

        let outside_dir = tempfile::tempdir().unwrap();
        let outside_file = outside_dir.path().join("x.py");
        std::fs::write(&outside_file, "print(1)").unwrap();

        #[cfg(unix)]
        {
            let symlink_path = root.join("out");
            std::os::unix::fs::symlink(outside_dir.path(), &symlink_path).unwrap();
            let status = std::process::Command::new("git")
                .args(["add", "out"])
                .current_dir(root)
                .env("GIT_TERMINAL_PROMPT", "0")
                .status()
                .unwrap();
            assert!(status.success());
        }

        std::fs::write(root.join("secret.txt"), "shhh").unwrap();

        let report = r#"---
verdict: revise
blocking: 1
advisory: 0
files: ["/etc/passwd", "~/x", "../x.py", ".git/config", ".plan.md", "src/*.py", ".env", "config/.env.local", "keys/server.pem", "deploy.key", "id_rsa", "id_ed25519.pub", ".claude/settings.json", "secret.txt", "out/x.py"]
---
# Report
"#;
        let nf = named_files(report, root);
        assert!(
            nf.files.is_empty(),
            "files should be empty but was: {:?}",
            nf.files
        );

        let rejected_codes: Vec<String> = nf.rejected.into_iter().map(|(_, r)| r).collect();
        assert!(rejected_codes
            .iter()
            .any(|c| c.starts_with("absolute_path:")));
        assert!(rejected_codes.iter().any(|c| c.starts_with("home_path:")));
        assert!(rejected_codes
            .iter()
            .any(|c| c.starts_with("parent_traversal:")));
        assert!(rejected_codes.iter().any(|c| c.starts_with("git_dir:")));
        assert!(rejected_codes
            .iter()
            .any(|c| c.starts_with("protocol_file:")));
        assert!(rejected_codes.iter().any(|c| c.starts_with("glob_path:")));

        let sensitive_count = rejected_codes
            .iter()
            .filter(|c| c.starts_with("sensitive:"))
            .count();
        assert_eq!(
            sensitive_count, 7,
            "expected 7 sensitive rejections, got: {:?}",
            rejected_codes
        );

        assert!(rejected_codes.iter().any(|c| c.starts_with("untracked:")));
        assert!(rejected_codes
            .iter()
            .any(|c| c.starts_with("outside_root:")));
    }

    #[test]
    fn named_files_cap() {
        let dir = test_repo();
        let root = dir.path();

        let candidates: Vec<String> = (0..25).map(|i| format!("src/f{i}.py")).collect();
        let files_json = serde_json::to_string(&candidates).unwrap();
        let report =
            format!("---\nverdict: revise\nblocking: 1\nadvisory: 0\nfiles: {files_json}\n---\n");

        let nf = named_files(&report, root);
        assert_eq!(nf.files.len(), 20);
        assert_eq!(nf.files, candidates[..20]);
        assert_eq!(nf.rejected.len(), 5);
        for (f, reason) in &nf.rejected {
            assert!(
                reason.starts_with("too_many:"),
                "expected too_many, got {reason}"
            );
            assert!(f.starts_with("src/f"));
        }
    }
}
