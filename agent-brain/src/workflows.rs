use crate::config::Config;
use crate::error::{BrainError, BrainResult};
use std::path::Path;

pub const BUILTIN_WORKFLOWS_JSON: &str = include_str!("../assets/workflows.default.json");
pub const STAGES: [&str; 7] = [
    "plan",
    "plan-review",
    "gate:you",
    "implement",
    "code-review",
    "qa",
    "fix",
];
pub const DEFAULT_WORKFLOW: &str = "mac-dinh";
pub const MAX_FILE_BYTES: u64 = 262_144;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Workflow {
    pub name: String,
    pub stages: Vec<String>,
    pub loops: Vec<String>,
    pub max_rounds: u32,
    pub budget_minutes: u32,
    pub budget_usd: f64,
    pub auto: bool,
    pub source: String,
}

impl Workflow {
    pub fn has(&self, stage: &str) -> bool {
        self.stages.iter().any(|s| s == stage)
    }

    pub fn has_loop(&self, l: &str) -> bool {
        self.loops.iter().any(|x| x == l)
    }

    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        let mut skip_plan_review = false;
        let mut skip_code_review = false;
        let mut skip_fix = false;

        for &stage in &STAGES {
            if !self.has(stage) {
                continue;
            }
            match stage {
                "plan" => {
                    if self.has_loop("plan") {
                        parts.push("plan ⇄\u{FE0E} review".to_string());
                        skip_plan_review = true;
                    } else {
                        parts.push("plan".to_string());
                    }
                }
                "plan-review" => {
                    if !skip_plan_review {
                        parts.push("review plan".to_string());
                    }
                }
                "gate:you" => {
                    parts.push("Bạn".to_string());
                }
                "implement" => {
                    if self.has_loop("build") {
                        parts.push("code ⇄\u{FE0E} review".to_string());
                        skip_code_review = true;
                    } else {
                        parts.push("code".to_string());
                    }
                }
                "code-review" => {
                    if !skip_code_review {
                        parts.push("review code".to_string());
                    }
                }
                "qa" => {
                    if self.has_loop("qa") {
                        parts.push("QA ⇄\u{FE0E} sửa".to_string());
                        skip_fix = true;
                    } else {
                        parts.push("QA".to_string());
                    }
                }
                "fix" => {
                    if !skip_fix {
                        parts.push("sửa".to_string());
                    }
                }
                _ => {}
            }
        }
        parts.join(" → ")
    }

    pub fn limits_text(&self) -> String {
        let usd = if (self.budget_usd.round() - self.budget_usd).abs() < 1e-9 {
            format!("${:.0}", self.budget_usd)
        } else {
            format!("${:.2}", self.budget_usd)
        };
        if self.loops.is_empty() {
            format!("{} phút · {}", self.budget_minutes, usd)
        } else {
            format!(
                "{} vòng · {} phút · {}",
                self.max_rounds, self.budget_minutes, usd
            )
        }
    }

    pub fn to_snapshot(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_snapshot(s: &str) -> Option<Workflow> {
        serde_json::from_str(s).ok()
    }
}

#[derive(Debug, Clone, Default)]
pub struct LoadedWorkflows {
    pub workflows: Vec<Workflow>,
    pub errors: Vec<String>,
}

impl LoadedWorkflows {
    pub fn get(&self, name: &str) -> Option<&Workflow> {
        self.workflows.iter().find(|w| w.name == name)
    }

    pub fn names(&self) -> Vec<String> {
        self.workflows.iter().map(|w| w.name.clone()).collect()
    }
}

pub fn validate_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 40 {
        return false;
    }
    match bytes[0] {
        b'a'..=b'z' | b'0'..=b'9' => {}
        _ => return false,
    }
    for &b in &bytes[1..] {
        match b {
            b'a'..=b'z' | b'0'..=b'9' | b'-' => {}
            _ => return false,
        }
    }
    true
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub workflow: Workflow,
    pub note: Option<String>,
}

pub fn resolve(
    loaded: &LoadedWorkflows,
    cli: Option<&str>,
    project_default: Option<&str>,
) -> BrainResult<Resolved> {
    if let Some(name) = cli {
        if !validate_name(name) {
            return Err(BrainError::error(format!(
                "Tên quy trình '{name}' không hợp lệ"
            )));
        }
        if let Some(wf) = loaded.get(name) {
            return Ok(Resolved {
                workflow: wf.clone(),
                note: None,
            });
        }
        let available = loaded.names().join(", ");
        return Err(BrainError::error(format!(
            "Không có quy trình '{name}' — các quy trình có: {available}"
        )));
    }

    if let Some(p_name) = project_default {
        let trimmed = p_name.trim();
        if !trimmed.is_empty() {
            if let Some(wf) = loaded.get(trimmed) {
                return Ok(Resolved {
                    workflow: wf.clone(),
                    note: None,
                });
            } else {
                let def = loaded.get(DEFAULT_WORKFLOW).ok_or_else(|| {
                    BrainError::error(format!(
                        "Không tìm thấy quy trình mặc định '{DEFAULT_WORKFLOW}'"
                    ))
                })?;
                return Ok(Resolved {
                    workflow: def.clone(),
                    note: Some(format!(
                        "Quy trình '{trimmed}' của dự án không có trong workflows.json — dùng {DEFAULT_WORKFLOW}."
                    )),
                });
            }
        }
    }

    let def = loaded.get(DEFAULT_WORKFLOW).ok_or_else(|| {
        BrainError::error(format!(
            "Không tìm thấy quy trình mặc định '{DEFAULT_WORKFLOW}'"
        ))
    })?;
    Ok(Resolved {
        workflow: def.clone(),
        note: None,
    })
}

fn parse_entry(
    name: &str,
    val: &serde_json::Value,
    cfg: &Config,
    source: &str,
) -> Result<Workflow, Vec<String>> {
    let mut errors = Vec::new();

    let obj = match val.as_object() {
        Some(o) => o,
        None => {
            errors.push(format!("quy trình \"{name}\" phải là một object."));
            return Err(errors);
        }
    };

    let mut stages = Vec::new();
    if let Some(stages_val) = obj.get("stages") {
        if let Some(arr) = stages_val.as_array() {
            for v in arr {
                if let Some(s) = v.as_str() {
                    stages.push(s.to_string());
                }
            }
        }
    }

    let mut loops = Vec::new();
    if let Some(loops_val) = obj.get("loops") {
        if let Some(lobj) = loops_val.as_object() {
            if lobj.get("plan").and_then(|v| v.as_str()) == Some("plan-review") {
                loops.push("plan".to_string());
            }
            if lobj.get("implement").and_then(|v| v.as_str()) == Some("code-review") {
                loops.push("build".to_string());
            }
            if lobj.get("qa").and_then(|v| v.as_str()) == Some("fix") {
                loops.push("qa".to_string());
            }
        }
    } else {
        // Derive from stage presence
        let has_stage = |s: &str| stages.iter().any(|st| st == s);
        if has_stage("plan") && has_stage("plan-review") {
            loops.push("plan".to_string());
        }
        if has_stage("implement") && has_stage("code-review") {
            loops.push("build".to_string());
        }
        if has_stage("qa") && has_stage("fix") {
            loops.push("qa".to_string());
        }
    }

    let max_rounds = obj
        .get("max_rounds")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(cfg.autopilot.max_rounds);

    let budget_minutes = obj
        .get("budget_minutes")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(cfg.autopilot.budget_minutes);

    let budget_usd = obj
        .get("budget_usd")
        .and_then(|v| v.as_f64())
        .unwrap_or(cfg.autopilot.budget_usd);

    let auto = obj.get("auto").and_then(|v| v.as_bool()).unwrap_or(false);

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(Workflow {
        name: name.to_string(),
        stages,
        loops,
        max_rounds,
        budget_minutes,
        budget_usd,
        auto,
        source: source.to_string(),
    })
}

fn load_builtins(cfg: &Config) -> Vec<Workflow> {
    let parsed: serde_json::Value =
        serde_json::from_str(BUILTIN_WORKFLOWS_JSON).expect("valid built-in workflows JSON");
    let obj = parsed.as_object().expect("built-in JSON is object");
    let mut res = Vec::new();
    for (name, val) in obj {
        if let Ok(wf) = parse_entry(name, val, cfg, "builtin") {
            res.push(wf);
        }
    }
    res
}

pub fn load_from(text: Option<&str>, cfg: &Config) -> LoadedWorkflows {
    let builtins = load_builtins(cfg);
    #[allow(unused_mut)]
    let mut workflows = builtins;
    #[allow(unused_mut)]
    let mut errors = Vec::new();

    if let Some(_file_text) = text {
        // Full file handling and validation lands in Task 2
    }

    LoadedWorkflows { workflows, errors }
}

pub fn load_path(path: &Path, cfg: &Config) -> LoadedWorkflows {
    if !path.exists() {
        return load_from(None, cfg);
    }
    match std::fs::read_to_string(path) {
        Ok(text) => load_from(Some(&text), cfg),
        Err(e) => {
            let mut lw = load_from(None, cfg);
            lw.errors
                .push(format!("Không đọc được workflows.json: {e}"));
            lw
        }
    }
}

pub fn load(cfg: &Config) -> LoadedWorkflows {
    let p = crate::config::config_dir().join("workflows.json");
    load_path(&p, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cfg() -> Config {
        crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn test_builtins_only() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        assert_eq!(loaded.names(), vec!["mac-dinh", "nhanh"]);
        assert!(loaded.errors.is_empty());

        let mac = loaded.get("mac-dinh").unwrap();
        assert_eq!(mac.stages, STAGES);
        assert_eq!(mac.loops, vec!["plan", "build", "qa"]);
        assert!(mac.auto);
        assert_eq!(mac.source, "builtin");
        assert_eq!(mac.max_rounds, 3);
        assert_eq!(mac.budget_minutes, 60);
        assert_eq!(mac.budget_usd, 20.0);

        let nhanh = loaded.get("nhanh").unwrap();
        assert_eq!(nhanh.stages, vec!["plan", "gate:you", "implement", "qa"]);
        assert!(nhanh.loops.is_empty());
        assert!(!nhanh.auto);
        assert_eq!(nhanh.source, "builtin");
        assert_eq!(nhanh.max_rounds, 3);
        assert_eq!(nhanh.budget_minutes, 60);
        assert_eq!(nhanh.budget_usd, 20.0);
    }

    #[test]
    fn test_config_defaults_apply() {
        let mut cfg = test_cfg();
        cfg.autopilot.max_rounds = 5;
        cfg.autopilot.budget_minutes = 30;
        cfg.autopilot.budget_usd = 7.5;

        let loaded = load_from(None, &cfg);
        for name in &["mac-dinh", "nhanh"] {
            let wf = loaded.get(name).unwrap();
            assert_eq!(wf.max_rounds, 5);
            assert_eq!(wf.budget_minutes, 30);
            assert_eq!(wf.budget_usd, 7.5);
        }
    }

    #[test]
    fn test_summaries() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let mac = loaded.get("mac-dinh").unwrap();
        assert_eq!(
            mac.summary(),
            "plan ⇄\u{FE0E} review → Bạn → code ⇄\u{FE0E} review → QA ⇄\u{FE0E} sửa"
        );
        let nhanh = loaded.get("nhanh").unwrap();
        assert_eq!(nhanh.summary(), "plan → Bạn → code → QA");

        let lone = Workflow {
            name: "lone".to_string(),
            stages: vec!["code-review".to_string()],
            loops: vec![],
            max_rounds: 3,
            budget_minutes: 60,
            budget_usd: 20.0,
            auto: false,
            source: "builtin".to_string(),
        };
        assert_eq!(lone.summary(), "review code");

        let custom = Workflow {
            name: "custom".to_string(),
            stages: vec![
                "plan".to_string(),
                "plan-review".to_string(),
                "gate:you".to_string(),
            ],
            loops: vec![],
            max_rounds: 3,
            budget_minutes: 60,
            budget_usd: 20.0,
            auto: false,
            source: "builtin".to_string(),
        };
        assert_eq!(custom.summary(), "plan → review plan → Bạn");
    }

    #[test]
    fn test_limits() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let mac = loaded.get("mac-dinh").unwrap();
        assert_eq!(mac.limits_text(), "3 vòng · 60 phút · $20");
        let nhanh = loaded.get("nhanh").unwrap();
        assert_eq!(nhanh.limits_text(), "60 phút · $20");

        let mut low_budget = nhanh.clone();
        low_budget.budget_usd = 0.05;
        assert_eq!(low_budget.limits_text(), "60 phút · $0.05");
    }

    #[test]
    fn test_resolve_order() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);

        // cli Some("nhanh"), project Some("mac-dinh") -> nhanh, note None
        let r = resolve(&loaded, Some("nhanh"), Some("mac-dinh")).unwrap();
        assert_eq!(r.workflow.name, "nhanh");
        assert_eq!(r.note, None);

        // cli None, project Some("nhanh") -> nhanh
        let r = resolve(&loaded, None, Some("nhanh")).unwrap();
        assert_eq!(r.workflow.name, "nhanh");
        assert_eq!(r.note, None);

        // cli None, None -> mac-dinh
        let r = resolve(&loaded, None, None).unwrap();
        assert_eq!(r.workflow.name, "mac-dinh");
        assert_eq!(r.note, None);

        // cli None, Some("x") -> mac-dinh with note
        let r = resolve(&loaded, None, Some("x")).unwrap();
        assert_eq!(r.workflow.name, "mac-dinh");
        assert_eq!(
            r.note,
            Some(
                "Quy trình 'x' của dự án không có trong workflows.json — dùng mac-dinh."
                    .to_string()
            )
        );

        // cli Some("khong-co"), None -> Err starting with "Không có quy trình 'khong-co' — các quy trình có: mac-dinh, nhanh"
        let err = resolve(&loaded, Some("khong-co"), None).unwrap_err();
        assert!(err
            .to_string()
            .starts_with("Không có quy trình 'khong-co' — các quy trình có: mac-dinh, nhanh"));

        // cli Some("Bad Name"), None -> Err "Tên quy trình 'Bad Name' không hợp lệ"
        let err = resolve(&loaded, Some("Bad Name"), None).unwrap_err();
        assert_eq!(err.to_string(), "Tên quy trình 'Bad Name' không hợp lệ");
    }

    #[test]
    fn test_snapshot_round_trip() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        for name in &["mac-dinh", "nhanh"] {
            let wf = loaded.get(name).unwrap();
            let snap = wf.to_snapshot();
            let round_trip = Workflow::from_snapshot(&snap);
            assert_eq!(round_trip, Some(wf.clone()));
        }
        assert_eq!(Workflow::from_snapshot("not json"), None);
    }

    #[test]
    fn test_validate_name() {
        assert!(validate_name("mac-dinh"));
        assert!(validate_name("nhanh"));
        assert!(validate_name("workflow-1"));
        assert!(validate_name("a"));
        assert!(!validate_name(""));
        assert!(!validate_name("-leading"));
        assert!(!validate_name("Bad_Name"));
        assert!(!validate_name("Bad Name"));
        assert!(!validate_name(&"a".repeat(41)));
        assert!(validate_name(&"a".repeat(40)));
    }
}
