use crate::config::Config;
use crate::error::{BrainError, BrainResult};
use std::collections::BTreeMap;
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

fn skip_whitespace(chars: &mut std::iter::Peekable<std::str::Chars>, line: &mut usize) {
    while let Some(&c) = chars.peek() {
        if c == '\n' {
            *line += 1;
            chars.next();
        } else if c.is_ascii_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
}

fn parse_string(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    line: &mut usize,
) -> Option<String> {
    if chars.peek() != Some(&'"') {
        return None;
    }
    chars.next(); // consume opening quote
    let mut s = String::new();
    let mut escape = false;
    while let Some(&c) = chars.peek() {
        chars.next();
        if c == '\n' {
            *line += 1;
        }
        if escape {
            s.push(c);
            escape = false;
        } else if c == '\\' {
            escape = true;
        } else if c == '"' {
            return Some(s);
        } else {
            s.push(c);
        }
    }
    Some(s)
}

fn skip_value(chars: &mut std::iter::Peekable<std::str::Chars>, line: &mut usize) {
    skip_whitespace(chars, line);
    match chars.peek() {
        Some('"') => {
            parse_string(chars, line);
        }
        Some('{') => {
            chars.next();
            let mut depth = 1;
            while let Some(&c) = chars.peek() {
                chars.next();
                if c == '\n' {
                    *line += 1;
                } else if c == '"' {
                    let mut escape = false;
                    while let Some(&sc) = chars.peek() {
                        chars.next();
                        if sc == '\n' {
                            *line += 1;
                        }
                        if escape {
                            escape = false;
                        } else if sc == '\\' {
                            escape = true;
                        } else if sc == '"' {
                            break;
                        }
                    }
                } else if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
        }
        Some('[') => {
            chars.next();
            let mut depth = 1;
            while let Some(&c) = chars.peek() {
                chars.next();
                if c == '\n' {
                    *line += 1;
                } else if c == '"' {
                    let mut escape = false;
                    while let Some(&sc) = chars.peek() {
                        chars.next();
                        if sc == '\n' {
                            *line += 1;
                        }
                        if escape {
                            escape = false;
                        } else if sc == '\\' {
                            escape = true;
                        } else if sc == '"' {
                            break;
                        }
                    }
                } else if c == '[' {
                    depth += 1;
                } else if c == ']' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
        }
        _ => {
            while let Some(&c) = chars.peek() {
                if c == ',' || c == '}' || c == ']' || c.is_ascii_whitespace() {
                    break;
                }
                chars.next();
            }
        }
    }
}

pub fn key_lines(text: &str) -> BTreeMap<String, usize> {
    let mut map = BTreeMap::new();
    let mut chars = text.chars().peekable();
    let mut line = 1;

    skip_whitespace(&mut chars, &mut line);
    if chars.peek() != Some(&'{') {
        return map;
    }
    chars.next(); // consume '{'

    loop {
        skip_whitespace(&mut chars, &mut line);
        match chars.peek() {
            None | Some('}') => {
                break;
            }
            Some(',') => {
                chars.next();
                continue;
            }
            Some('"') => {
                let entry_line = line;
                let entry_name = match parse_string(&mut chars, &mut line) {
                    Some(s) => s,
                    None => break,
                };
                map.insert(entry_name.clone(), entry_line);

                skip_whitespace(&mut chars, &mut line);
                if chars.peek() == Some(&':') {
                    chars.next();
                }
                skip_whitespace(&mut chars, &mut line);

                if chars.peek() == Some(&'{') {
                    chars.next(); // consume '{'
                    loop {
                        skip_whitespace(&mut chars, &mut line);
                        match chars.peek() {
                            None | Some('}') => {
                                if chars.peek() == Some(&'}') {
                                    chars.next();
                                }
                                break;
                            }
                            Some(',') => {
                                chars.next();
                                continue;
                            }
                            Some('"') => {
                                let field_line = line;
                                let field_name = match parse_string(&mut chars, &mut line) {
                                    Some(s) => s,
                                    None => break,
                                };
                                map.insert(format!("{entry_name}/{field_name}"), field_line);

                                skip_whitespace(&mut chars, &mut line);
                                if chars.peek() == Some(&':') {
                                    chars.next();
                                }
                                skip_whitespace(&mut chars, &mut line);

                                if field_name == "stages" && chars.peek() == Some(&'[') {
                                    chars.next(); // consume '['
                                    let mut elem_idx = 0;
                                    loop {
                                        skip_whitespace(&mut chars, &mut line);
                                        match chars.peek() {
                                            None | Some(']') => {
                                                if chars.peek() == Some(&']') {
                                                    chars.next();
                                                }
                                                break;
                                            }
                                            Some(',') => {
                                                chars.next();
                                                continue;
                                            }
                                            _ => {
                                                let elem_line = line;
                                                map.insert(
                                                    format!("{entry_name}/stages/{elem_idx}"),
                                                    elem_line,
                                                );
                                                skip_value(&mut chars, &mut line);
                                                elem_idx += 1;
                                            }
                                        }
                                    }
                                } else {
                                    skip_value(&mut chars, &mut line);
                                }
                            }
                            _ => {
                                skip_value(&mut chars, &mut line);
                            }
                        }
                    }
                } else {
                    skip_value(&mut chars, &mut line);
                }
            }
            _ => {
                skip_value(&mut chars, &mut line);
            }
        }
    }

    map
}

fn validate_entry(
    name: &str,
    val: &serde_json::Value,
    lines: &BTreeMap<String, usize>,
    cfg: &Config,
    source: &str,
) -> Result<Workflow, Vec<String>> {
    let mut entry_errors = Vec::new();
    let entry_line = lines.get(name).copied().unwrap_or(1);

    if !validate_name(name) {
        entry_errors.push(format!(
            "Dòng {entry_line}: tên quy trình \"{name}\" không hợp lệ (chỉ chữ thường, số và dấu gạch ngang, tối đa 40 ký tự)."
        ));
    }

    let obj = match val.as_object() {
        Some(o) => o,
        None => {
            entry_errors.push(format!(
                "Dòng {entry_line}: quy trình \"{name}\" phải là một object."
            ));
            return Err(entry_errors);
        }
    };

    for (k, _) in obj {
        if k != "stages"
            && k != "loops"
            && k != "max_rounds"
            && k != "budget_minutes"
            && k != "budget_usd"
            && k != "auto"
        {
            let l = lines
                .get(&format!("{name}/{k}"))
                .copied()
                .unwrap_or(entry_line);
            entry_errors.push(format!(
                "Dòng {l}: quy trình \"{name}\" có khóa lạ \"{k}\"."
            ));
        }
    }

    let stages_line = lines
        .get(&format!("{name}/stages"))
        .copied()
        .unwrap_or(entry_line);
    let stages_val = obj.get("stages");
    let mut stages: Vec<String> = Vec::new();

    let stages_ok = match stages_val {
        Some(serde_json::Value::Array(arr)) if !arr.is_empty() => {
            let all_strings = arr.iter().all(|v| v.is_string());
            if all_strings {
                stages = arr
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect();
                true
            } else {
                false
            }
        }
        _ => false,
    };

    if !stages_ok {
        entry_errors.push(format!(
            "Dòng {stages_line}: quy trình \"{name}\" thiếu \"stages\" (danh sách giai đoạn)."
        ));
    } else {
        let mut seen = std::collections::HashSet::new();
        let mut last_canonical_idx = 0;
        let mut first = true;

        for (i, tok) in stages.iter().enumerate() {
            let elem_line = lines
                .get(&format!("{name}/stages/{i}"))
                .copied()
                .unwrap_or(stages_line);

            if !STAGES.contains(&tok.as_str()) {
                entry_errors.push(format!(
                    "Dòng {elem_line}: quy trình \"{name}\": giai đoạn \"{tok}\" không hợp lệ (dùng: plan, plan-review, gate:you, implement, code-review, qa, fix)."
                ));
            } else {
                if seen.contains(tok) {
                    entry_errors.push(format!(
                        "Dòng {elem_line}: quy trình \"{name}\": giai đoạn \"{tok}\" bị lặp."
                    ));
                } else {
                    seen.insert(tok.clone());
                }

                let canon_idx = STAGES.iter().position(|&s| s == tok.as_str()).unwrap();
                if first {
                    last_canonical_idx = canon_idx;
                    first = false;
                } else if canon_idx < last_canonical_idx {
                    entry_errors.push(format!(
                        "Dòng {elem_line}: quy trình \"{name}\": giai đoạn \"{tok}\" sai thứ tự (đúng: plan → plan-review → gate:you → implement → code-review → qa → fix)."
                    ));
                } else {
                    last_canonical_idx = canon_idx;
                }
            }
        }

        let has_plan = stages.iter().any(|s| s == "plan");
        let has_plan_or_review = stages.iter().any(|s| s == "plan" || s == "plan-review");
        let has_later = stages
            .iter()
            .any(|s| s == "implement" || s == "code-review" || s == "qa" || s == "fix");
        let has_gate = stages.iter().any(|s| s == "gate:you");

        if has_plan_or_review && has_later && !has_gate {
            entry_errors.push(format!(
                "Dòng {stages_line}: quy trình \"{name}\" phải có \"gate:you\" giữa bước plan và bước code/QA — autopilot luôn dừng ở bước bạn duyệt plan."
            ));
        }

        if has_gate && !has_plan {
            let gate_i = stages.iter().position(|s| s == "gate:you").unwrap();
            let gate_line = lines
                .get(&format!("{name}/stages/{gate_i}"))
                .copied()
                .unwrap_or(stages_line);
            entry_errors.push(format!(
                "Dòng {gate_line}: quy trình \"{name}\": \"gate:you\" cần có \"plan\" trước đó."
            ));
        }

        let has_qa = stages.iter().any(|s| s == "qa");
        let has_fix = stages.iter().any(|s| s == "fix");
        if has_fix && !has_qa {
            let fix_i = stages.iter().position(|s| s == "fix").unwrap();
            let fix_line = lines
                .get(&format!("{name}/stages/{fix_i}"))
                .copied()
                .unwrap_or(stages_line);
            entry_errors.push(format!(
                "Dòng {fix_line}: quy trình \"{name}\": \"fix\" cần có \"qa\"."
            ));
        }
    }

    let mut loops = Vec::new();
    if let Some(loops_val) = obj.get("loops") {
        let loops_line = lines
            .get(&format!("{name}/loops"))
            .copied()
            .unwrap_or(entry_line);
        let mut loops_err = false;

        match loops_val.as_object() {
            Some(lobj) => {
                let has_stage = |s: &str| stages.iter().any(|st| st == s);

                for (k, v) in lobj {
                    match k.as_str() {
                        "plan" => {
                            if v.as_str() != Some("plan-review")
                                || !has_stage("plan")
                                || !has_stage("plan-review")
                            {
                                loops_err = true;
                            }
                        }
                        "implement" => {
                            if v.as_str() != Some("code-review")
                                || !has_stage("implement")
                                || !has_stage("code-review")
                            {
                                loops_err = true;
                            }
                        }
                        "qa" => {
                            if v.as_str() != Some("fix") || !has_stage("qa") || !has_stage("fix") {
                                loops_err = true;
                            }
                        }
                        _ => {
                            loops_err = true;
                        }
                    }
                }

                if loops_err {
                    entry_errors.push(format!(
                        "Dòng {loops_line}: quy trình \"{name}\": \"loops\" chỉ nhận \"plan\": \"plan-review\", \"implement\": \"code-review\", \"qa\": \"fix\" và cả hai giai đoạn phải có trong \"stages\"."
                    ));
                } else {
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
            }
            None => {
                entry_errors.push(format!(
                    "Dòng {loops_line}: quy trình \"{name}\": \"loops\" chỉ nhận \"plan\": \"plan-review\", \"implement\": \"code-review\", \"qa\": \"fix\" và cả hai giai đoạn phải có trong \"stages\"."
                ));
            }
        }
    } else {
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

    let max_rounds = if let Some(v) = obj.get("max_rounds") {
        let l = lines
            .get(&format!("{name}/max_rounds"))
            .copied()
            .unwrap_or(entry_line);
        match v.as_i64() {
            Some(n) if n >= 1 && n <= 10 && v.is_number() => n as u32,
            _ => {
                entry_errors.push(format!(
                    "Dòng {l}: \"max_rounds\" phải là số nguyên từ 1 đến 10."
                ));
                cfg.autopilot.max_rounds
            }
        }
    } else {
        cfg.autopilot.max_rounds
    };

    let budget_minutes = if let Some(v) = obj.get("budget_minutes") {
        let l = lines
            .get(&format!("{name}/budget_minutes"))
            .copied()
            .unwrap_or(entry_line);
        match v.as_i64() {
            Some(n) if n >= 1 && n <= 1440 && v.is_number() => n as u32,
            _ => {
                entry_errors.push(format!(
                    "Dòng {l}: \"budget_minutes\" phải là số nguyên từ 1 đến 1440."
                ));
                cfg.autopilot.budget_minutes
            }
        }
    } else {
        cfg.autopilot.budget_minutes
    };

    let budget_usd = if let Some(v) = obj.get("budget_usd") {
        let l = lines
            .get(&format!("{name}/budget_usd"))
            .copied()
            .unwrap_or(entry_line);
        match v.as_f64() {
            Some(x) if x > 0.0 && x <= 1000.0 => x,
            _ => {
                entry_errors.push(format!(
                    "Dòng {l}: \"budget_usd\" phải là số lớn hơn 0 và không quá 1000."
                ));
                cfg.autopilot.budget_usd
            }
        }
    } else {
        cfg.autopilot.budget_usd
    };

    let auto = if let Some(v) = obj.get("auto") {
        let l = lines
            .get(&format!("{name}/auto"))
            .copied()
            .unwrap_or(entry_line);
        match v.as_bool() {
            Some(b) => b,
            None => {
                entry_errors.push(format!("Dòng {l}: \"auto\" phải là true hoặc false."));
                false
            }
        }
    } else {
        false
    };

    if !entry_errors.is_empty() {
        return Err(entry_errors);
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
    let dummy_lines = BTreeMap::new();
    let mut res = Vec::new();
    for (name, val) in obj {
        if let Ok(wf) = validate_entry(name, val, &dummy_lines, cfg, "builtin") {
            res.push(wf);
        }
    }
    res
}

pub fn load_from(text: Option<&str>, cfg: &Config) -> LoadedWorkflows {
    let mut workflows = load_builtins(cfg);
    let mut errors = Vec::new();

    if let Some(file_text) = text {
        if file_text.len() as u64 > MAX_FILE_BYTES {
            errors.push("workflows.json lớn quá 256 KiB — bỏ qua file.".to_string());
            return LoadedWorkflows { workflows, errors };
        }

        let parsed_val: serde_json::Value = match serde_json::from_str(file_text) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!(
                    "Dòng {}, cột {}: workflows.json không phải JSON hợp lệ.",
                    e.line(),
                    e.column()
                ));
                return LoadedWorkflows { workflows, errors };
            }
        };

        let obj = match parsed_val.as_object() {
            Some(o) => o,
            None => {
                errors.push(
                    "Dòng 1: workflows.json phải là một object, mỗi khóa là tên một quy trình."
                        .to_string(),
                );
                return LoadedWorkflows { workflows, errors };
            }
        };

        let lines = key_lines(file_text);

        for (name, val) in obj {
            let is_override = workflows.iter().any(|w| w.name == *name);
            let src = if is_override { "override" } else { "file" };

            match validate_entry(name, val, &lines, cfg, src) {
                Ok(wf) => {
                    if let Some(pos) = workflows.iter().position(|w| w.name == *name) {
                        workflows[pos] = wf;
                    } else {
                        workflows.push(wf);
                    }
                }
                Err(mut errs) => {
                    errors.append(&mut errs);
                }
            }
        }
    }

    LoadedWorkflows { workflows, errors }
}

pub fn load_path(path: &Path, cfg: &Config) -> LoadedWorkflows {
    if !path.exists() {
        return load_from(None, cfg);
    }
    match std::fs::metadata(path) {
        Ok(m) => {
            if m.len() > MAX_FILE_BYTES {
                let mut lw = load_from(None, cfg);
                lw.errors
                    .push("workflows.json lớn quá 256 KiB — bỏ qua file.".to_string());
                return lw;
            }
        }
        Err(e) => {
            let mut lw = load_from(None, cfg);
            lw.errors
                .push(format!("Không đọc được workflows.json: {e}"));
            return lw;
        }
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

        let r = resolve(&loaded, Some("nhanh"), Some("mac-dinh")).unwrap();
        assert_eq!(r.workflow.name, "nhanh");
        assert_eq!(r.note, None);

        let r = resolve(&loaded, None, Some("nhanh")).unwrap();
        assert_eq!(r.workflow.name, "nhanh");
        assert_eq!(r.note, None);

        let r = resolve(&loaded, None, None).unwrap();
        assert_eq!(r.workflow.name, "mac-dinh");
        assert_eq!(r.note, None);

        let r = resolve(&loaded, None, Some("x")).unwrap();
        assert_eq!(r.workflow.name, "mac-dinh");
        assert_eq!(
            r.note,
            Some(
                "Quy trình 'x' của dự án không có trong workflows.json — dùng mac-dinh."
                    .to_string()
            )
        );

        let err = resolve(&loaded, Some("khong-co"), None).unwrap_err();
        assert!(err
            .to_string()
            .starts_with("Không có quy trình 'khong-co' — các quy trình có: mac-dinh, nhanh"));

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

    #[test]
    fn test_file_adds_after_builtins() {
        let cfg = test_cfg();
        let json = r#"{
  "chi-phi-thap": {
    "stages": ["plan", "plan-review", "gate:you", "implement", "qa"],
    "budget_usd": 0.05,
    "auto": true
  }
}"#;
        let loaded = load_from(Some(json), &cfg);
        assert_eq!(loaded.names(), vec!["mac-dinh", "nhanh", "chi-phi-thap"]);
        assert!(loaded.errors.is_empty());

        let cpt = loaded.get("chi-phi-thap").unwrap();
        assert_eq!(cpt.source, "file");
        assert_eq!(cpt.budget_usd, 0.05);
        assert_eq!(cpt.max_rounds, 3);
        assert_eq!(cpt.loops, vec!["plan"]);
        assert!(cpt.auto);
    }

    #[test]
    fn test_file_overrides_builtin() {
        let cfg = test_cfg();
        let json = r#"{
  "nhanh": {
    "stages": ["plan", "gate:you", "implement", "qa"],
    "auto": true,
    "max_rounds": 2
  }
}"#;
        let loaded = load_from(Some(json), &cfg);
        assert_eq!(loaded.names(), vec!["mac-dinh", "nhanh"]);
        assert!(loaded.errors.is_empty());

        let nhanh = loaded.get("nhanh").unwrap();
        assert_eq!(nhanh.source, "override");
        assert!(nhanh.auto);
        assert_eq!(nhanh.max_rounds, 2);
    }

    #[test]
    fn test_invalid_override_keeps_builtin() {
        let cfg = test_cfg();
        let json = r#"{
  "mac-dinh": {
    "stages": ["plan", "implement"]
  }
}"#;
        let loaded = load_from(Some(json), &cfg);
        assert_eq!(loaded.names(), vec!["mac-dinh", "nhanh"]);
        assert_eq!(loaded.errors.len(), 1);
        assert!(loaded.errors[0].contains(
            "quy trình \"mac-dinh\" phải có \"gate:you\" giữa bước plan và bước code/QA"
        ));

        let mac = loaded.get("mac-dinh").unwrap();
        assert_eq!(mac.source, "builtin");
        assert_eq!(mac.stages.len(), 7);
    }

    #[test]
    fn test_syntax_error_drops_file() {
        let cfg = test_cfg();
        let text = "{\n  \"a\": {\n    \"stages\": [\n";
        let expected_err = serde_json::from_str::<serde_json::Value>(text).unwrap_err();
        let loaded = load_from(Some(text), &cfg);

        assert_eq!(loaded.names(), vec!["mac-dinh", "nhanh"]);
        assert_eq!(loaded.errors.len(), 1);
        let expected_msg = format!(
            "Dòng {}, cột {}: workflows.json không phải JSON hợp lệ.",
            expected_err.line(),
            expected_err.column()
        );
        assert_eq!(loaded.errors[0], expected_msg);
    }

    #[test]
    fn test_semantic_errors_name_key_and_line() {
        let cfg = test_cfg();
        let text = "{\n  \"valid\": {\n    \"stages\": [\"plan\", \"gate:you\", \"implement\", \"qa\"]\n  },\n  \"x\": {\n    \"stages\": [\n      \"plan\",\n      \"gate:you\",\n      \"implement\",\n      \"qa\"\n    ],\n    \"max_rounds\": 0,\n    \"auto\": false\n  }\n}";
        let loaded = load_from(Some(text), &cfg);

        assert_eq!(loaded.names(), vec!["mac-dinh", "nhanh", "valid"]);
        assert_eq!(loaded.errors.len(), 1);
        assert_eq!(
            loaded.errors[0],
            "Dòng 12: \"max_rounds\" phải là số nguyên từ 1 đến 10."
        );
        assert!(loaded.get("x").is_none());
        assert!(loaded.get("valid").is_some());
    }

    #[test]
    fn test_unknown_key_rejected() {
        let cfg = test_cfg();
        let text = "{\n  \"y\": {\n    \"stages\": [\"plan\", \"gate:you\", \"implement\", \"qa\"],\n    \"skip_gate\": true\n  }\n}";
        let loaded = load_from(Some(text), &cfg);

        assert_eq!(loaded.names(), vec!["mac-dinh", "nhanh"]);
        assert_eq!(loaded.errors.len(), 1);
        assert_eq!(
            loaded.errors[0],
            "Dòng 4: quy trình \"y\" có khóa lạ \"skip_gate\"."
        );
        assert!(loaded.get("y").is_none());
    }

    #[test]
    fn test_stage_rules_unknown() {
        let cfg = test_cfg();
        let text = "{\n  \"w\": {\n    \"stages\": [\n      \"plan\",\n      \"gate:you\",\n      \"deploy\"\n    ]\n  }\n}";
        let loaded = load_from(Some(text), &cfg);

        assert!(loaded.get("w").is_none());
        assert_eq!(loaded.errors.len(), 1);
        assert_eq!(
            loaded.errors[0],
            "Dòng 6: quy trình \"w\": giai đoạn \"deploy\" không hợp lệ (dùng: plan, plan-review, gate:you, implement, code-review, qa, fix)."
        );
    }

    #[test]
    fn test_stage_rules_duplicate() {
        let cfg = test_cfg();
        let text = "{\n  \"w\": {\n    \"stages\": [\n      \"plan\",\n      \"gate:you\",\n      \"qa\",\n      \"qa\"\n    ]\n  }\n}";
        let loaded = load_from(Some(text), &cfg);

        assert!(loaded.get("w").is_none());
        assert_eq!(loaded.errors.len(), 1);
        assert_eq!(
            loaded.errors[0],
            "Dòng 7: quy trình \"w\": giai đoạn \"qa\" bị lặp."
        );
    }

    #[test]
    fn test_stage_rules_order() {
        let cfg = test_cfg();
        let text =
            "{\n  \"w\": {\n    \"stages\": [\n      \"implement\",\n      \"plan\"\n    ]\n  }\n}";
        let loaded = load_from(Some(text), &cfg);

        assert!(loaded.get("w").is_none());
        assert!(loaded.errors.iter().any(|e| e == "Dòng 5: quy trình \"w\": giai đoạn \"plan\" sai thứ tự (đúng: plan → plan-review → gate:you → implement → code-review → qa → fix)."));
    }

    #[test]
    fn test_stage_rules_gate() {
        let cfg = test_cfg();
        // Missing gate
        let text1 = "{\n  \"w1\": {\n    \"stages\": [\"plan\", \"implement\"]\n  }\n}";
        let loaded1 = load_from(Some(text1), &cfg);
        assert_eq!(
            loaded1.errors[0],
            "Dòng 3: quy trình \"w1\" phải có \"gate:you\" giữa bước plan và bước code/QA — autopilot luôn dừng ở bước bạn duyệt plan."
        );

        // Gate without plan
        let text2 = "{\n  \"w2\": {\n    \"stages\": [\n      \"gate:you\",\n      \"implement\"\n    ]\n  }\n}";
        let loaded2 = load_from(Some(text2), &cfg);
        assert_eq!(
            loaded2.errors[0],
            "Dòng 4: quy trình \"w2\": \"gate:you\" cần có \"plan\" trước đó."
        );
    }

    #[test]
    fn test_stage_rules_fix_without_qa() {
        let cfg = test_cfg();
        let text = "{\n  \"w\": {\n    \"stages\": [\n      \"plan\",\n      \"gate:you\",\n      \"implement\",\n      \"fix\"\n    ]\n  }\n}";
        let loaded = load_from(Some(text), &cfg);
        assert_eq!(
            loaded.errors[0],
            "Dòng 7: quy trình \"w\": \"fix\" cần có \"qa\"."
        );
    }

    #[test]
    fn test_loops_rules() {
        let cfg = test_cfg();
        // Valid custom loops
        let text1 = r#"{
  "w1": {
    "stages": ["plan", "plan-review", "gate:you", "implement", "code-review"],
    "loops": {"plan": "plan-review"}
  }
}"#;
        let loaded1 = load_from(Some(text1), &cfg);
        assert!(loaded1.errors.is_empty());
        let w1 = loaded1.get("w1").unwrap();
        assert_eq!(w1.loops, vec!["plan"]);

        // Invalid loops value
        let text2 = "{\n  \"w2\": {\n    \"stages\": [\"plan\", \"gate:you\", \"implement\", \"qa\"],\n    \"loops\": {\"plan\": \"qa\"}\n  }\n}";
        let loaded2 = load_from(Some(text2), &cfg);
        assert_eq!(
            loaded2.errors[0],
            "Dòng 4: quy trình \"w2\": \"loops\" chỉ nhận \"plan\": \"plan-review\", \"implement\": \"code-review\", \"qa\": \"fix\" và cả hai giai đoạn phải có trong \"stages\"."
        );
    }

    #[test]
    fn test_number_and_bool_rules() {
        let cfg = test_cfg();
        let text = "{\n  \"w\": {\n    \"stages\": [\"plan\", \"gate:you\", \"implement\", \"qa\"],\n    \"budget_minutes\": 1441,\n    \"budget_usd\": 0,\n    \"auto\": \"yes\"\n  }\n}";
        let loaded = load_from(Some(text), &cfg);
        assert_eq!(loaded.errors.len(), 3);
        assert!(loaded
            .errors
            .contains(&"Dòng 4: \"budget_minutes\" phải là số nguyên từ 1 đến 1440.".to_string()));
        assert!(loaded.errors.contains(
            &"Dòng 5: \"budget_usd\" phải là số lớn hơn 0 và không quá 1000.".to_string()
        ));
        assert!(loaded
            .errors
            .contains(&"Dòng 6: \"auto\" phải là true hoặc false.".to_string()));

        let text2 = "{\n  \"w\": {\n    \"stages\": [\"plan\", \"gate:you\", \"implement\", \"qa\"],\n    \"budget_usd\": 1000.5\n  }\n}";
        let loaded2 = load_from(Some(text2), &cfg);
        assert_eq!(
            loaded2.errors[0],
            "Dòng 4: \"budget_usd\" phải là số lớn hơn 0 và không quá 1000."
        );
    }

    #[test]
    fn test_name_rule() {
        let cfg = test_cfg();
        let text = "{\n  \"Bad_Name\": {\n    \"stages\": [\"plan\", \"gate:you\", \"implement\", \"qa\"]\n  }\n}";
        let loaded = load_from(Some(text), &cfg);
        assert_eq!(
            loaded.errors[0],
            "Dòng 2: tên quy trình \"Bad_Name\" không hợp lệ (chỉ chữ thường, số và dấu gạch ngang, tối đa 40 ký tự)."
        );
    }

    #[test]
    fn test_top_level_not_object() {
        let cfg = test_cfg();
        let loaded = load_from(Some("[]"), &cfg);
        assert_eq!(
            loaded.errors[0],
            "Dòng 1: workflows.json phải là một object, mỗi khóa là tên một quy trình."
        );
    }

    #[test]
    fn test_too_large_file() {
        let cfg = test_cfg();
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("workflows.json");
        let content = " ".repeat(262_145);
        std::fs::write(&file_path, content).unwrap();

        let loaded = load_path(&file_path, &cfg);
        assert_eq!(loaded.names(), vec!["mac-dinh", "nhanh"]);
        assert_eq!(
            loaded.errors,
            vec!["workflows.json lớn quá 256 KiB — bỏ qua file.".to_string()]
        );
    }

    #[test]
    fn test_key_lines_scanner() {
        let text = r#"{
  "a": {
    "desc": "hello \"world\"",
    "nested": {
      "foo": 1
    },
    "stages": [
      "plan",
      "gate:you"
    ],
    "budget_usd": 10.0
  }
}"#;
        let lines = key_lines(text);
        assert_eq!(lines.get("a"), Some(&2));
        assert_eq!(lines.get("a/stages"), Some(&7));
        assert_eq!(lines.get("a/stages/1"), Some(&9));
        assert_eq!(lines.get("a/budget_usd"), Some(&11));
    }
}
