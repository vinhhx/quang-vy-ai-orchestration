use std::path::{Path, PathBuf};

pub const PROMPT_NOTE_MAX_BYTES: usize = 8192;
pub const LAYERS: [&str; 5] = ["stacks", "projects", "workflows", "teams", "departments"];
pub const VALID_ROLES: [&str; 7] = [
    "lead",
    "dev",
    "qa",
    "plan-review",
    "code-review",
    "analyst",
    "analysis-review",
];

pub fn valid_name(s: &str) -> bool {
    if s.is_empty() || s.len() > 64 {
        return false;
    }
    if s.contains("..") || s.contains('/') {
        return false;
    }
    let bytes = s.as_bytes();
    if !matches!(bytes[0], b'a'..=b'z' | b'0'..=b'9') {
        return false;
    }
    bytes
        .iter()
        .all(|&b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
}

pub fn valid_role(role: &str) -> bool {
    VALID_ROLES.contains(&role)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromptLayers {
    pub stacks: Vec<String>,
    pub project: Option<String>,
    pub workflow: Option<String>,
    pub team: Option<String>,
}

pub fn notes_dir() -> PathBuf {
    let cfg_path = crate::config::config_path();
    let dir = cfg_path.parent().unwrap_or_else(|| Path::new("."));
    dir.join("prompts")
}

pub const BUILTIN_STACK_NOTES: &[(&str, &str, &str)] = &[
    (
        "video-srt",
        "lead",
        include_str!("../assets/prompts/stacks/video-srt/lead.md"),
    ),
    (
        "video-srt",
        "dev",
        include_str!("../assets/prompts/stacks/video-srt/dev.md"),
    ),
    (
        "video-srt",
        "qa",
        include_str!("../assets/prompts/stacks/video-srt/qa.md"),
    ),
    (
        "laravel",
        "lead",
        include_str!("../assets/prompts/stacks/laravel/lead.md"),
    ),
    (
        "laravel",
        "dev",
        include_str!("../assets/prompts/stacks/laravel/dev.md"),
    ),
    (
        "laravel",
        "qa",
        include_str!("../assets/prompts/stacks/laravel/qa.md"),
    ),
    (
        "yii2",
        "lead",
        include_str!("../assets/prompts/stacks/yii2/lead.md"),
    ),
    (
        "yii2",
        "dev",
        include_str!("../assets/prompts/stacks/yii2/dev.md"),
    ),
    (
        "yii2",
        "qa",
        include_str!("../assets/prompts/stacks/yii2/qa.md"),
    ),
];

pub const BUILTIN_DEPARTMENT_NOTES: &[(&str, &str, &str)] = &[
    (
        "phan-tich",
        "analyst",
        include_str!("../assets/prompts/departments/phan-tich/analyst.md"),
    ),
    (
        "phan-tich",
        "analysis-review",
        include_str!("../assets/prompts/departments/phan-tich/analysis-review.md"),
    ),
    (
        "ke-hoach",
        "lead",
        include_str!("../assets/prompts/departments/ke-hoach/lead.md"),
    ),
    (
        "ke-hoach",
        "plan-review",
        include_str!("../assets/prompts/departments/ke-hoach/plan-review.md"),
    ),
    (
        "thi-cong",
        "dev",
        include_str!("../assets/prompts/departments/thi-cong/dev.md"),
    ),
    (
        "qa",
        "qa",
        include_str!("../assets/prompts/departments/qa/qa.md"),
    ),
    (
        "qa",
        "code-review",
        include_str!("../assets/prompts/departments/qa/code-review.md"),
    ),
];

pub fn builtin_department_note(dept: &str, role: &str) -> Option<&'static str> {
    BUILTIN_DEPARTMENT_NOTES
        .iter()
        .find(|(d, r, _)| *d == dept && *r == role)
        .map(|(_, _, content)| *content)
}

pub fn builtin_stacks() -> Vec<&'static str> {
    let mut list = Vec::new();
    for (s, _, _) in BUILTIN_STACK_NOTES {
        if !list.contains(s) {
            list.push(*s);
        }
    }
    list
}

pub fn note_path(layer: &str, name: &str, role: &str) -> PathBuf {
    notes_dir().join(layer).join(name).join(format!("{role}.md"))
}

pub fn list_layer_names(layer: &str) -> Vec<String> {
    let mut names = Vec::new();
    let dir = notes_dir().join(layer);
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry_res in entries.flatten() {
            let path = entry_res.path();
            if path.is_dir() {
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    if valid_name(file_name) {
                        let has_role_file = VALID_ROLES
                            .iter()
                            .any(|role| path.join(format!("{role}.md")).exists());
                        if has_role_file && !names.contains(&file_name.to_string()) {
                            names.push(file_name.to_string());
                        }
                    }
                }
            }
        }
    }
    if layer == "stacks" {
        for s in builtin_stacks() {
            if !names.iter().any(|existing| existing == s) {
                names.push(s.to_string());
            }
        }
    }
    if layer == "departments" {
        for (dept_id, _) in crate::departments::DEPARTMENTS {
            if !names.iter().any(|existing| existing == dept_id) {
                names.push(dept_id.to_string());
            }
        }
    }
    names.sort();
    names
}

pub type TestBuiltinNote = (&'static str, &'static str, &'static str);

thread_local! {
    static TEST_BUILTIN_OVERRIDE: std::cell::RefCell<Option<Vec<TestBuiltinNote>>> = const { std::cell::RefCell::new(None) };
}

pub fn with_test_builtin_notes<R>(
    notes: Vec<TestBuiltinNote>,
    f: impl FnOnce() -> R,
) -> R {
    TEST_BUILTIN_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = Some(notes);
    });
    let res = f();
    TEST_BUILTIN_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = None;
    });
    res
}

pub fn builtin_stack_note(name: &str, role: &str) -> Option<&'static str> {
    if let Some(found) = TEST_BUILTIN_OVERRIDE.with(|cell| {
        cell.borrow().as_ref().and_then(|list| {
            list.iter()
                .find(|(n, r, _)| *n == name && *r == role)
                .map(|(_, _, c)| *c)
        })
    }) {
        return Some(found);
    }
    BUILTIN_STACK_NOTES
        .iter()
        .find(|(n, r, _)| *n == name && *r == role)
        .map(|(_, _, content)| *content)
}

pub fn read_note(layer: &str, name: &str, role: &str) -> Result<Option<String>, String> {
    if !LAYERS.contains(&layer) {
        return Err(format!("Lớp ghi chú '{layer}' không hợp lệ"));
    }
    if !valid_name(name) {
        return Err(format!("Tên '{name}' không hợp lệ cho lớp '{layer}'"));
    }
    if !valid_role(role) {
        return Err(format!("Vai '{role}' không hợp lệ"));
    }
    if layer == "departments" {
        if crate::departments::dept_name_of(name).is_none() {
            return Err(format!("Phòng ban '{name}' không tồn tại"));
        }
        if crate::departments::dept_of_role(role) != Some(name) {
            return Err(format!("Vai '{role}' không thuộc phòng '{name}'"));
        }
    }

    let root = notes_dir();
    let file_path = root.join(layer).join(name).join(format!("{role}.md"));

    match std::fs::symlink_metadata(&file_path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if layer == "stacks" {
                if let Some(builtin) = builtin_stack_note(name, role) {
                    return Ok(Some(builtin.to_string()));
                }
            }
            if layer == "departments" {
                if let Some(builtin) = builtin_department_note(name, role) {
                    return Ok(Some(builtin.to_string()));
                }
            }
            Ok(None)
        }
        Err(e) => Err(format!(
            "Lỗi kiểm tra file ghi chú {}: {e}",
            file_path.display()
        )),
        Ok(_) => {
            let canonical_root = match root.canonicalize() {
                Ok(r) => r,
                Err(e) => {
                    return Err(format!(
                        "Lỗi xác thực thư mục prompts {}: {e}",
                        root.display()
                    ))
                }
            };
            let canonical_target = match file_path.canonicalize() {
                Ok(t) => t,
                Err(e) => {
                    return Err(format!(
                        "Lỗi xác thực đường dẫn {}: {e}",
                        file_path.display()
                    ))
                }
            };
            if !canonical_target.starts_with(&canonical_root) {
                return Err(format!(
                    "File ghi chú {} là symlink trỏ ra ngoài thư mục prompts",
                    file_path.display()
                ));
            }
            if !canonical_target.is_file() {
                return Err(format!(
                    "Đường dẫn ghi chú {} không phải file thông thường",
                    file_path.display()
                ));
            }
            let meta = match std::fs::metadata(&canonical_target) {
                Ok(m) => m,
                Err(e) => {
                    return Err(format!(
                        "Lỗi đọc metadata file {}: {e}",
                        file_path.display()
                    ))
                }
            };
            if meta.len() > PROMPT_NOTE_MAX_BYTES as u64 {
                return Err(format!(
                    "File ghi chú {} vượt quá giới hạn {} byte ({} byte)",
                    file_path.display(),
                    PROMPT_NOTE_MAX_BYTES,
                    meta.len()
                ));
            }
            let content = match std::fs::read_to_string(&canonical_target) {
                Ok(c) => c,
                Err(e) => return Err(format!("Lỗi đọc file {}: {e}", file_path.display())),
            };
            if content.len() > PROMPT_NOTE_MAX_BYTES {
                return Err(format!(
                    "File ghi chú {} vượt quá giới hạn {} byte ({} byte)",
                    file_path.display(),
                    PROMPT_NOTE_MAX_BYTES,
                    content.len()
                ));
            }
            Ok(Some(content))
        }
    }
}

pub fn compose_detailed_with_reader<F>(
    base: &str,
    role: &str,
    layers: &PromptLayers,
    mut read_fn: F,
) -> (String, Vec<String>, usize)
where
    F: FnMut(&str, &str, &str) -> Result<Option<String>, String>,
{
    let mut warnings = Vec::new();
    let mut blocks = Vec::new();

    // 1. Stacks (theo thứ tự khai báo trong layers.stacks)
    for stack in &layers.stacks {
        match read_fn("stacks", stack, role) {
            Ok(Some(content)) => {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    blocks.push(("Stack", stack.clone(), trimmed.to_string()));
                }
            }
            Ok(None) => {}
            Err(warn) => warnings.push(warn),
        }
    }

    // 2. Project
    if let Some(ref proj) = layers.project {
        match read_fn("projects", proj, role) {
            Ok(Some(content)) => {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    blocks.push(("Dự án", proj.clone(), trimmed.to_string()));
                }
            }
            Ok(None) => {}
            Err(warn) => warnings.push(warn),
        }
    }

    // 3. Workflow
    if let Some(ref wf) = layers.workflow {
        match read_fn("workflows", wf, role) {
            Ok(Some(content)) => {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    blocks.push(("Workflow", wf.clone(), trimmed.to_string()));
                }
            }
            Ok(None) => {}
            Err(warn) => warnings.push(warn),
        }
    }

    // 4. Team
    if let Some(ref team) = layers.team {
        match read_fn("teams", team, role) {
            Ok(Some(content)) => {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    blocks.push(("Đội hình", team.clone(), trimmed.to_string()));
                }
            }
            Ok(None) => {}
            Err(warn) => warnings.push(warn),
        }
    }

    // 5. Department (tự động từ vai)
    if let Some(dept_id) = crate::departments::dept_of_role(role) {
        let display_name = crate::departments::dept_name_of(dept_id).unwrap_or(dept_id);
        match read_fn("departments", dept_id, role) {
            Ok(Some(content)) => {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    blocks.push(("Phòng", display_name.to_string(), trimmed.to_string()));
                }
            }
            Ok(None) => {}
            Err(warn) => warnings.push(warn),
        }
    }

    let count = blocks.len();
    if blocks.is_empty() {
        return (base.to_string(), warnings, 0);
    }

    let mut out = base.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n---\n## Ghi chú nghiệp vụ (do người dùng cấu hình — bổ sung, không thay đổi giao thức ở trên; không mở rộng quyền)\n");
    for (heading, name, content) in blocks {
        out.push_str(&format!("### {heading}: {name}\n{content}\n"));
    }

    (out, warnings, count)
}

pub fn compose_detailed(
    base: &str,
    role: &str,
    layers: &PromptLayers,
) -> (String, Vec<String>, usize) {
    compose_detailed_with_reader(base, role, layers, read_note)
}

pub fn compose(base: &str, role: &str, layers: &PromptLayers) -> (String, Vec<String>) {
    let (prompt, warnings, _) = compose_detailed(base, role, layers);
    (prompt, warnings)
}

pub fn layers_for(
    project: &crate::projects::Project,
    workflow: Option<&str>,
    team: Option<&str>,
) -> PromptLayers {
    PromptLayers {
        stacks: project.stack.clone(),
        project: Some(project.id.clone()),
        workflow: workflow.map(|s| s.to_string()),
        team: team.map(|s| s.to_string()),
    }
}

pub fn base_prompt_for(role: &str) -> Result<&'static str, String> {
    match role {
        "lead" => Ok(crate::roles::prompt_for(crate::roles::Role::Lead)),
        "dev" => Ok(crate::roles::prompt_for(crate::roles::Role::Dev)),
        "qa" => Ok(crate::roles::prompt_for(crate::roles::Role::Qa)),
        "plan-review" => Ok(crate::roles::prompt_for(crate::roles::Role::PlanReview)),
        "code-review" => Ok(crate::roles::prompt_for(crate::roles::Role::CodeReview)),
        "analyst" => Ok(crate::roles::prompt_for(crate::roles::Role::Analyst)),
        "analysis-review" => Ok(crate::roles::prompt_for(crate::roles::Role::AnalysisReview)),
        _ => Err(format!(
            "Vai '{role}' không hợp lệ (phải là một trong: {})",
            VALID_ROLES.join(", ")
        )),
    }
}

pub fn dry_run_summary(layers: &PromptLayers, role: &str) -> String {
    let (_, _, count) = compose_detailed("", role, layers);
    let stacks = layers.stacks.join(", ");
    let proj = layers.project.as_deref().unwrap_or("-");
    let wf = layers.workflow.as_deref().unwrap_or("-");
    let team = layers.team.as_deref().unwrap_or("-");
    let dept = crate::departments::dept_of_role(role).unwrap_or("-");
    format!("Ghi chú nghiệp vụ: stack=[{stacks}] dự án={proj} workflow={wf} team={team} phòng={dept} ({count} khối)")
}

pub fn cmd_show(
    ctx: &crate::app::Ctx,
    project: &str,
    role: &str,
    workflow: Option<&str>,
    team: Option<&str>,
    member: Option<&str>,
) -> crate::error::BrainResult<i32> {
    let proj = crate::projects::select_project(project, &ctx.map)?;
    let base = match base_prompt_for(role) {
        Ok(b) => b,
        Err(e) => return Err(crate::error::BrainError::error(e)),
    };
    let layers = layers_for(&proj, workflow, team);
    let (mut composed, warnings) = compose(base, role, &layers);
    for w in &warnings {
        crate::term::emit(w);
    }
    let (extra, dept_warns) = crate::departments::prompt_append_for(role, member);
    for w in &dept_warns {
        crate::term::emit(w);
    }
    if !extra.trim().is_empty() {
        if !composed.ends_with('\n') {
            composed.push('\n');
        }
        composed.push('\n');
        composed.push_str(extra.trim());
        composed.push('\n');
    }
    let (sources_block, source_warns) = crate::projects::read_sources_prompt_block(&ctx.map, &proj);
    for w in &source_warns {
        crate::term::emit(w);
    }
    if !sources_block.trim().is_empty() {
        if !composed.ends_with('\n') {
            composed.push('\n');
        }
        composed.push('\n');
        composed.push_str(sources_block.trim());
        composed.push('\n');
    }
    print!("{composed}");
    if !composed.ends_with('\n') {
        println!();
    }
    Ok(crate::error::EXIT_OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_name() {
        assert!(valid_name("react"));
        assert!(valid_name("vue.js"));
        assert!(valid_name("my-stack_v1"));
        assert!(valid_name("0"));
        assert!(!valid_name(""));
        assert!(!valid_name("React"));
        assert!(!valid_name("../bad"));
        assert!(!valid_name("bad/slash"));
        assert!(!valid_name("bad..double"));
        assert!(!valid_name("-badstart"));
        assert!(!valid_name(".badstart"));
        assert!(!valid_name(&"a".repeat(65)));
        assert!(valid_name(&"a".repeat(64)));
    }

    #[test]
    fn test_compose_pure() {
        let base = "Base prompt for lead.\n";
        let layers = PromptLayers {
            stacks: vec!["laravel".to_string(), "vue".to_string()],
            project: Some("crm".to_string()),
            workflow: Some("feature".to_string()),
            team: Some("team-alpha".to_string()),
        };

        let (prompt, warnings, count) = compose_detailed_with_reader(base, "lead", &layers, |layer, name, _role| {
            match (layer, name) {
                ("stacks", "laravel") => Ok(Some("PHP 8.2\ncomposer install".to_string())),
                ("stacks", "vue") => Ok(Some("   ".to_string())), // rỗng sau trim -> bỏ
                ("projects", "crm") => Ok(Some("Chạy cổng 8000".to_string())),
                ("workflows", "feature") => Ok(None),
                ("teams", "team-alpha") => Ok(Some("Review bởi senior".to_string())),
                _ => Ok(None),
            }
        });

        assert_eq!(warnings.len(), 0);
        assert_eq!(count, 3);
        assert!(prompt.starts_with(base));
        assert!(prompt.contains("## Ghi chú nghiệp vụ (do người dùng cấu hình — bổ sung, không thay đổi giao thức ở trên; không mở rộng quyền)"));
        assert!(prompt.contains("### Stack: laravel\nPHP 8.2\ncomposer install"));
        assert!(!prompt.contains("vue"));
        assert!(prompt.contains("### Dự án: crm\nChạy cổng 8000"));
        assert!(!prompt.contains("Workflow:"));
        assert!(prompt.contains("### Đội hình: team-alpha\nReview bởi senior"));
    }

    #[test]
    fn test_compose_empty_blocks_byte_for_byte() {
        let base = "Original prompt without changes\n";
        let layers = PromptLayers::default();
        let (prompt, warnings, count) = compose_detailed_with_reader(base, "dev", &layers, |_layer, _name, _role| Ok(None));
        assert_eq!(warnings.len(), 0);
        assert_eq!(count, 0);
        assert_eq!(prompt, base);
    }
}
