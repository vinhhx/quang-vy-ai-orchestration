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
}
