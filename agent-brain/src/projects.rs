use crate::config::Config;
use crate::error::{BrainError, BrainResult};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub id: String,
    pub path: String,
    pub name_vi: String,
    pub aliases: Vec<String>,
    pub kind: String, /* map key "type" */
    pub profile: String,
    pub repo: Option<String>,
    pub missing: bool,
    pub stack: Vec<String>,
    pub read_sources: Vec<String>,
    /// Folder of the protocol files, relative to the repo root (`protocol_dir`, D1); `""` = root.
    pub protocol_dir: String,
    /// How tasks share the repo (D2): base branch, gate hold, checkout on approve.
    pub workspace: WorkspaceOpts,
}

/// Key of the project's base branch for new tasks (D2.1).
pub const BASE_BRANCH_KEY: &str = "base_branch";
/// Key of the gate hold option (D2.3a).
pub const HOLD_AT_GATE_KEY: &str = "hold_project_at_gate";
/// Key of the checkout-on-approve option (D2.3b).
pub const AUTO_CHECKOUT_KEY: &str = "auto_checkout_on_approve";
/// Longest accepted `base_branch` (characters).
pub const BASE_BRANCH_MAX_CHARS: usize = 120;

/// The project's workspace options (D2). A project without the keys gets `Default`: new tasks
/// branch from the repo's current branch, a task waiting at the plan gate holds the project,
/// and approving checks out the task branch when the tree is clean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceOpts {
    /// `base_branch`: the branch new tasks start from; `None` = the current branch at creation.
    pub base_branch: Option<String>,
    /// `hold_project_at_gate`: a task waiting at the plan gate counts as busy for other tasks.
    pub hold_project_at_gate: bool,
    /// `auto_checkout_on_approve`: approving off the task branch with a clean tree checks it out.
    pub auto_checkout_on_approve: bool,
}

impl Default for WorkspaceOpts {
    fn default() -> Self {
        WorkspaceOpts {
            base_branch: None,
            hold_project_at_gate: true,
            auto_checkout_on_approve: true,
        }
    }
}

impl WorkspaceOpts {
    /// Reads the options from a projects_map.json entry. Missing or invalid values keep the
    /// default (the dashboard validates before storing).
    pub fn from_value(entry: &serde_json::Value) -> WorkspaceOpts {
        let d = WorkspaceOpts::default();
        WorkspaceOpts {
            base_branch: entry
                .get(BASE_BRANCH_KEY)
                .and_then(|v| v.as_str())
                .and_then(|s| validate_base_branch(s).ok())
                .filter(|s| !s.is_empty()),
            hold_project_at_gate: entry
                .get(HOLD_AT_GATE_KEY)
                .and_then(|v| v.as_bool())
                .unwrap_or(d.hold_project_at_gate),
            auto_checkout_on_approve: entry
                .get(AUTO_CHECKOUT_KEY)
                .and_then(|v| v.as_bool())
                .unwrap_or(d.auto_checkout_on_approve),
        }
    }

    /// The branch a new task starts from: the configured `base_branch`, else the repo's
    /// current branch at `root`.
    pub fn base_branch_at(&self, root: &Path) -> Option<String> {
        self.base_branch
            .clone()
            .or_else(|| crate::gitops::current_branch(root))
    }
}

/// Checks a `base_branch` value and returns it trimmed; `""` = not set. Refuses what git would
/// refuse as a branch name (spaces, control characters, `..`, `~^:?*[\\`, a leading `-` or
/// `/`, a trailing `/`, `.` or `.lock`, `@{`).
pub fn validate_base_branch(raw: &str) -> Result<String, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(String::new());
    }
    let bad = || Err(format!("\"{BASE_BRANCH_KEY}\" không phải tên nhánh git hợp lệ."));
    if t.chars().count() > BASE_BRANCH_MAX_CHARS {
        return Err(format!(
            "\"{BASE_BRANCH_KEY}\" dài quá {BASE_BRANCH_MAX_CHARS} ký tự."
        ));
    }
    if t.chars().any(|c| c.is_control() || c.is_whitespace() || "~^:?*[\\".contains(c)) {
        return bad();
    }
    if t.contains("..") || t.contains("@{") || t.contains("//") || t == "@" {
        return bad();
    }
    if t.starts_with('-') || t.starts_with('/') || t.ends_with('/') || t.ends_with('.') {
        return bad();
    }
    if t.split('/').any(|seg| seg.starts_with('.') || seg.ends_with(".lock")) {
        return bad();
    }
    Ok(t.to_string())
}

/// Refusal when the project's `base_branch` (D2.1) is not a local branch of the repo.
pub fn base_branch_missing_text(base: &str) -> String {
    format!(
        "Nhánh gốc '{base}' ({BASE_BRANCH_KEY}) của dự án không có trong repo — sửa cấu hình dự án hoặc tạo nhánh đó"
    )
}

/// The workspace options of a project in the map; `Default` when it is not there.
pub fn workspace_of(map: &serde_json::Value, project_id: &str) -> WorkspaceOpts {
    map.get("projects")
        .and_then(|p| p.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(project_id))
        })
        .map(WorkspaceOpts::from_value)
        .unwrap_or_default()
}

impl Project {
    pub fn from_value(v: &serde_json::Value) -> Option<Project> {
        let obj = v.as_object()?;
        let id = obj
            .get("id")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let path = obj
            .get("path")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let name_vi = obj
            .get("name_vi")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let aliases = obj
            .get("aliases")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let kind = obj
            .get("type")
            .and_then(|s| s.as_str())
            .unwrap_or("code")
            .to_string();
        let profile = obj
            .get("profile")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let repo = obj
            .get("repo")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        let missing = obj
            .get("missing")
            .and_then(|b| b.as_bool())
            .unwrap_or(false);
        let stack = obj
            .get("stack")
            .and_then(|s| s.as_array())
            .map(|arr| {
                let mut valid = Vec::new();
                for item in arr {
                    if let Some(s) = item.as_str() {
                        if crate::prompts::valid_name(s) {
                            valid.push(s.to_string());
                        } else {
                            crate::term::emit(&format!(
                                "Bỏ qua stack '{s}' không hợp lệ của dự án '{id}'"
                            ));
                        }
                    }
                }
                valid
            })
            .unwrap_or_default();

        let read_sources = obj
            .get("read_sources")
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Some(Project {
            id,
            path,
            name_vi,
            aliases,
            kind,
            profile,
            repo,
            missing,
            stack,
            read_sources,
            protocol_dir: crate::protocol_paths::protocol_dir_of_value(v),
            workspace: WorkspaceOpts::from_value(v),
        })
    }

    pub fn root(&self) -> PathBuf {
        let p = crate::config::expand_tilde(&self.path, &crate::config::home_dir());
        std::fs::canonicalize(&p).unwrap_or(p)
    }
}

pub fn projects_of(map: &serde_json::Value) -> Vec<Project> {
    if let Some(arr) = map.get("projects").and_then(|p| p.as_array()) {
        arr.iter().filter_map(Project::from_value).collect()
    } else {
        Vec::new()
    }
}

pub fn read_source_paths(
    map: &serde_json::Value,
    project: &Project,
) -> (Vec<(String, PathBuf)>, Vec<String>) {
    let mut sources = Vec::new();
    let mut warnings = Vec::new();

    let all_projects = projects_of(map);
    let cur_root = project.root();
    let cur_real = std::fs::canonicalize(&cur_root).unwrap_or_else(|_| cur_root.clone());

    for src_id in &project.read_sources {
        if src_id == &project.id {
            warnings.push(format!("Bỏ qua dự án nguồn trùng chính nó: '{src_id}'"));
            continue;
        }
        let src_proj = match all_projects.iter().find(|p| p.id == *src_id) {
            Some(p) => p,
            None => {
                warnings.push(format!("Bỏ qua dự án nguồn không tồn tại: '{src_id}'"));
                continue;
            }
        };
        if src_proj.missing {
            warnings.push(format!("Bỏ qua dự án nguồn bị missing: '{src_id}'"));
            continue;
        }
        let src_root = src_proj.root();
        let src_real = match std::fs::canonicalize(&src_root) {
            Ok(p) => p,
            Err(_) => {
                warnings.push(format!(
                    "Bỏ qua dự án nguồn có thư mục không tồn tại: '{src_id}' ({})",
                    src_root.display()
                ));
                continue;
            }
        };
        if src_real == cur_real || src_real.starts_with(&cur_real) || cur_real.starts_with(&src_real) {
            warnings.push(format!(
                "Bỏ qua dự án nguồn '{src_id}' vì thư mục chồng lấn với dự án hiện tại"
            ));
            continue;
        }
        sources.push((src_id.clone(), src_real));
    }

    (sources, warnings)
}

pub fn read_sources_prompt_block(
    map: &serde_json::Value,
    project: &Project,
) -> (String, Vec<String>) {
    let (paths, warnings) = read_source_paths(map, project);
    if paths.is_empty() {
        return (String::new(), warnings);
    }
    let mut block = String::from("## Dự án nguồn (chỉ đọc)\n");
    for (id, path) in &paths {
        let kn = match crate::knowledge::find(path) {
            Some(home) => format!(" — thư mục tri thức: {}", home.rel),
            None => String::new(),
        };
        block.push_str(&format!("- {}: {}{}\n", id, path.display(), kn));
    }
    block.push_str("Mọi vai của task (Lead, Dev, QA, Review) đều đọc được các thư mục này bằng đường dẫn tuyệt đối.\n");
    block.push_str("Đọc bằng đường dẫn tuyệt đối. Không ghi, không chạy lệnh trong các thư mục này, không đọc file .env*.");
    (block, warnings)
}

pub fn workflow_of(map: &serde_json::Value, project_id: &str) -> Option<String> {
    let arr = map.get("projects").and_then(|p| p.as_array())?;
    for p in arr {
        if p.get("id").and_then(|v| v.as_str()) == Some(project_id) {
            let wf = p.get("workflow").and_then(|v| v.as_str())?;
            let trimmed = wf.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// The project's `timeouts: {role: seconds}` (C1). Missing, malformed or out-of-range entries
/// are left out, so a project without the key keeps the workflow / config limits.
pub fn timeouts_of(map: &serde_json::Value, project_id: &str) -> BTreeMap<String, u64> {
    let Some(arr) = map.get("projects").and_then(|p| p.as_array()) else {
        return BTreeMap::new();
    };
    let Some(v) = arr
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(project_id))
        .and_then(|p| p.get("timeouts"))
    else {
        return BTreeMap::new();
    };
    let Some(obj) = v.as_object() else {
        return BTreeMap::new();
    };
    obj.iter()
        .filter_map(|(role, secs)| {
            let one = serde_json::json!({ role.clone(): secs.clone() });
            crate::config::parse_role_timeouts(&one).ok()
        })
        .flatten()
        .collect()
}

/// C4: what a project pins for one role: a department member (`role_members`) or an engine
/// (`role_engines`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RolePin {
    Member(String),
    Engine(String),
}

/// C4: the project's `role_members: {role: member id}` and `role_engines: {role: engine name}`.
/// A project without them pins nothing and the team / department pick decides as before.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RolePins {
    pub members: BTreeMap<String, String>,
    pub engines: BTreeMap<String, String>,
}

impl RolePins {
    pub fn is_empty(&self) -> bool {
        self.members.is_empty() && self.engines.is_empty()
    }

    /// The pin of `role`; an error when the file pins both a member and an engine for it.
    pub fn pin_for(&self, role: &str) -> Result<Option<RolePin>, String> {
        match (self.members.get(role), self.engines.get(role)) {
            (Some(_), Some(_)) => Err(format!(
                "Dự án ghim cả thành viên (role_members) lẫn engine (role_engines) cho vai {role} — chỉ được ghim một trong hai."
            )),
            (Some(m), None) => Ok(Some(RolePin::Member(m.clone()))),
            (None, Some(e)) => Ok(Some(RolePin::Engine(e.clone()))),
            (None, None) => Ok(None),
        }
    }
}

fn parse_pin_object(
    key: &str,
    v: &serde_json::Value,
    check: &dyn Fn(&str) -> bool,
    errors: &mut Vec<String>,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if v.is_null() {
        return out;
    }
    let Some(obj) = v.as_object() else {
        errors.push(format!(
            "\"{key}\" phải là object {{vai: tên}} (vai: {})",
            crate::teams::ROLES.join(", ")
        ));
        return out;
    };
    for (role, val) in obj {
        if !crate::teams::ROLES.contains(&role.as_str()) {
            errors.push(format!(
                "\"{key}\": vai \"{role}\" không hợp lệ (dùng: {})",
                crate::teams::ROLES.join(", ")
            ));
            continue;
        }
        match val.as_str().map(str::trim) {
            Some(name) if check(name) => {
                out.insert(role.clone(), name.to_string());
            }
            _ => errors.push(format!("\"{key}.{role}\" không phải tên hợp lệ.")),
        }
    }
    out
}

fn pin_member_ok(name: &str) -> bool {
    crate::staff::validate_id(name)
}

fn pin_engine_ok(name: &str) -> bool {
    matches!(
        crate::engine::Engine::parse(name),
        Some(crate::engine::Engine::Claude) | Some(crate::engine::Engine::Named(_))
    )
}

/// C4: `role_members` / `role_engines` as the dashboard sends them. Every role must be one of
/// `teams::ROLES`, a name must be a valid id, and a role may be pinned in only one of the two.
pub fn parse_role_pins(
    members: &serde_json::Value,
    engines: &serde_json::Value,
) -> Result<RolePins, Vec<String>> {
    let mut errors = Vec::new();
    let pins = RolePins {
        members: parse_pin_object("role_members", members, &pin_member_ok, &mut errors),
        engines: parse_pin_object("role_engines", engines, &pin_engine_ok, &mut errors),
    };
    for role in pins.members.keys() {
        if pins.engines.contains_key(role) {
            errors.push(format!(
                "Vai {role} chỉ được ghim thành viên hoặc engine, không cả hai."
            ));
        }
    }
    if errors.is_empty() {
        Ok(pins)
    } else {
        Err(errors)
    }
}

/// C4: the project's role pins as stored. Invalid entries are left out; a role pinned in both
/// maps is kept so the run seam refuses it with a clear message (`RolePins::pin_for`).
pub fn role_pins_of(map: &serde_json::Value, project_id: &str) -> RolePins {
    let Some(obj) = map
        .get("projects")
        .and_then(|p| p.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(project_id))
        })
    else {
        return RolePins::default();
    };
    let null = serde_json::Value::Null;
    let mut ignored = Vec::new();
    RolePins {
        members: parse_pin_object(
            "role_members",
            obj.get("role_members").unwrap_or(&null),
            &pin_member_ok,
            &mut ignored,
        ),
        engines: parse_pin_object(
            "role_engines",
            obj.get("role_engines").unwrap_or(&null),
            &pin_engine_ok,
            &mut ignored,
        ),
    }
}

/// The project's environment preflight (C2): `preflight`, `preflight_roles`,
/// `preflight_retry_minutes`, `preflight_interval_seconds`. A project without them (or not in
/// the map) gets `Preflight::default()`: no check anywhere.
pub fn preflight_of(map: &serde_json::Value, project_id: &str) -> crate::preflight::Preflight {
    let Some(obj) = map
        .get("projects")
        .and_then(|p| p.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(project_id))
        })
    else {
        return crate::preflight::Preflight::default();
    };
    crate::preflight::read_lenient(obj)
}

/// The project's default team name (D-12), trimmed; None when missing or blank.
pub fn team_of(map: &serde_json::Value, project_id: &str) -> Option<String> {
    let arr = map.get("projects").and_then(|p| p.as_array())?;
    for p in arr {
        if p.get("id").and_then(|v| v.as_str()) == Some(project_id) {
            let team = p.get("team").and_then(|v| v.as_str())?;
            let trimmed = team.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

pub fn triage_split_of(map: &serde_json::Value, project_id: &str) -> bool {
    let arr = match map.get("projects").and_then(|p| p.as_array()) {
        Some(a) => a,
        None => return true,
    };
    for p in arr {
        if p.get("id").and_then(|v| v.as_str()) == Some(project_id) {
            return p.get("triage_split").and_then(|v| v.as_bool()).unwrap_or(true);
        }
    }
    true
}

pub fn load_map(path: &Path) -> BrainResult<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::json!({"scan_roots": [], "projects": []}));
    }
    let content = std::fs::read_to_string(path).map_err(|e| {
        BrainError::error(format!(
            "Không đọc được {}: {} — sửa file rồi chạy lại (agent-brain không ghi đè file này)",
            path.display(),
            e
        ))
    })?;
    let mut data: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        BrainError::error(format!(
            "Không đọc được {}: {} — sửa file rồi chạy lại (agent-brain không ghi đè file này)",
            path.display(),
            e
        ))
    })?;
    let obj = data.as_object_mut().ok_or_else(|| {
        BrainError::error(format!(
            "Không đọc được {}: phải là JSON object — sửa file rồi chạy lại (agent-brain không ghi đè file này)",
            path.display()
        ))
    })?;

    if let Some(p) = obj.get("projects") {
        if !p.is_array() {
            return Err(BrainError::error(format!(
                "Không đọc được {}: 'projects' phải là danh sách — sửa file rồi chạy lại (agent-brain không ghi đè file này)",
                path.display()
            )));
        }
    } else {
        obj.insert("projects".to_string(), serde_json::json!([]));
    }

    if let Some(s) = obj.get("scan_roots") {
        if !s.is_array() {
            return Err(BrainError::error(format!(
                "Không đọc được {}: 'scan_roots' phải là danh sách — sửa file rồi chạy lại (agent-brain không ghi đè file này)",
                path.display()
            )));
        }
    } else {
        obj.insert("scan_roots".to_string(), serde_json::json!([]));
    }

    Ok(data)
}

pub fn select_project(query: &str, map: &serde_json::Value) -> BrainResult<Project> {
    let projects = projects_of(map);
    match crate::routing::resolve(query, &projects) {
        Ok(p) => Ok(p),
        Err(err) => {
            let header = if err.kind == crate::routing::RouteKind::Ambiguous {
                format!(
                    "Tên \"{}\" khớp {} dự án — dừng, không chạy agent. Gọi lại bằng id chính xác:",
                    query,
                    err.candidates.len()
                )
            } else {
                format!(
                    "Không tìm thấy dự án \"{}\" — dừng, không chạy agent. Danh sách dự án:",
                    query
                )
            };
            let mut lines = vec![header];
            if err.candidates.is_empty() {
                lines.push(
                    "  (trống) — chạy: agent-brain projects scan --root <thư mục>".to_string(),
                );
            } else {
                for c in &err.candidates {
                    let sources_str = if c.read_sources.is_empty() {
                        String::new()
                    } else {
                        format!(" [nguồn: {}]", c.read_sources.join(", "))
                    };
                    lines.push(format!("  {}\t{}\t{}{}", c.id, c.name_vi, c.path, sources_str));
                }
            }
            Err(BrainError::new(
                lines.join("\n"),
                crate::error::EXIT_ROUTING,
            ))
        }
    }
}

pub fn require_runnable(project: &Project, cfg: &Config) -> BrainResult<PathBuf> {
    if project.missing {
        return Err(BrainError::error(format!(
            "Dự án '{}' bị đánh dấu missing — không thể chạy agent",
            project.id
        )));
    }
    if project.path.trim().is_empty() {
        return Err(BrainError::error(format!(
            "Dự án '{}' không có đường dẫn hợp lệ",
            project.id
        )));
    }
    let home = crate::config::home_dir();
    let proj_path = crate::config::expand_tilde(&project.path, &home);
    let proj_path = std::fs::canonicalize(&proj_path).unwrap_or(proj_path);
    if !proj_path.is_dir() {
        return Err(BrainError::error(format!(
            "Đường dẫn dự án '{}' không tồn tại: {}",
            project.id,
            proj_path.display()
        )));
    }
    if project.kind == "idea" {
        crate::gitops::ensure_shadow(&proj_path)?;
    }
    if !crate::gitops::is_repo_root(&proj_path) {
        return Err(BrainError::error(format!(
            "Thư mục dự án '{}' không phải repo git: {}",
            project.id,
            proj_path.display()
        )));
    }
    crate::profile::check_profile(&project.profile, &cfg.profiles)?;
    Ok(proj_path)
}

fn to_realpath(p: &str) -> String {
    let path = Path::new(p);
    if let Ok(canon) = std::fs::canonicalize(path) {
        canon.to_string_lossy().to_string()
    } else if path.is_absolute() {
        path.to_string_lossy().to_string()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(path).to_string_lossy().to_string()
    } else {
        p.to_string()
    }
}

pub fn clean_remote_url(raw: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^(https?://)[^@/?#]+@(.*)$").unwrap());
    re.replace(raw, "${1}${2}").to_string()
}

pub fn git_remote(path: &Path) -> Option<String> {
    let raw = crate::gitops::git(path, &["remote", "get-url", "origin"]).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(clean_remote_url(trimmed))
}

pub fn unique_id_from_projects(projects: &[serde_json::Value], base: &str, path: &Path) -> String {
    let abs_path = to_realpath(&path.to_string_lossy());
    let mut used_ids: HashMap<String, String> = HashMap::new();
    for p in projects {
        if let Some(pid) = p.get("id").and_then(|id| id.as_str()) {
            let ppath = p.get("path").and_then(|pth| pth.as_str()).unwrap_or("");
            used_ids.insert(pid.to_string(), to_realpath(ppath));
        }
    }

    if !used_ids.contains_key(base) || used_ids[base] == abs_path {
        return base.to_string();
    }
    let mut i = 2;
    loop {
        let candidate = format!("{base}-{i}");
        if !used_ids.contains_key(&candidate) || used_ids[&candidate] == abs_path {
            return candidate;
        }
        i += 1;
    }
}

pub fn unique_id(data: &serde_json::Value, base: &str, path: &Path) -> String {
    let empty = Vec::new();
    let projs = data
        .get("projects")
        .and_then(|p| p.as_array())
        .map(|v| v.as_slice())
        .unwrap_or(&empty);
    unique_id_from_projects(projs, base, path)
}

pub fn new_entry_from_projects(
    projects: &[serde_json::Value],
    path: &Path,
    kind: &str,
) -> serde_json::Value {
    let abs_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let abs_str = abs_path.to_string_lossy().to_string();
    let base = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let pid = unique_id_from_projects(projects, &base, &abs_path);
    let repo = if kind == "code" {
        git_remote(&abs_path)
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null)
    } else {
        serde_json::Value::Null
    };

    let mut map = serde_json::Map::new();
    map.insert("id".to_string(), serde_json::Value::String(pid));
    map.insert("path".to_string(), serde_json::Value::String(abs_str));
    map.insert("name_vi".to_string(), serde_json::Value::String(base));
    map.insert("aliases".to_string(), serde_json::json!([]));
    map.insert(
        "type".to_string(),
        serde_json::Value::String(kind.to_string()),
    );
    map.insert("repo".to_string(), repo);
    map.insert(
        "profile".to_string(),
        serde_json::Value::String(String::new()),
    );
    map.insert("missing".to_string(), serde_json::Value::Bool(false));
    serde_json::Value::Object(map)
}

pub fn new_entry(data: &serde_json::Value, path: &Path, kind: &str) -> serde_json::Value {
    let empty = Vec::new();
    let projs = data
        .get("projects")
        .and_then(|p| p.as_array())
        .map(|v| v.as_slice())
        .unwrap_or(&empty);
    new_entry_from_projects(projs, path, kind)
}

pub fn save_map(path: &Path, data: &serde_json::Value) -> BrainResult<()> {
    let parent = match path.parent() {
        Some(p) => p,
        None => Path::new("."),
    };
    if !parent.exists() {
        std::fs::create_dir_all(parent).map_err(|e| {
            BrainError::error(format!("Không thể tạo thư mục {}: {}", parent.display(), e))
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }
    }

    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_path = parent.join(format!("projects_map_{}_{}.tmp", pid, nanos));

    let write_res = (|| -> std::io::Result<()> {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp_path)?;
        let json_str = serde_json::to_string_pretty(data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        f.write_all(json_str.as_bytes())?;
        f.write_all(b"\n")?;
        f.flush()?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(BrainError::error(format!(
            "Không thể ghi {}: {}",
            tmp_path.display(),
            e
        )));
    }

    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(BrainError::error(format!(
            "Không thể đổi tên {} thành {}: {}",
            tmp_path.display(),
            path.display(),
            e
        )));
    }

    Ok(())
}

pub fn detect_stack(root: &Path) -> Vec<String> {
    let mut stacks = Vec::new();
    let has_artisan = root.join("artisan").exists();
    let has_yii = root.join("yii").exists()
        || root.join("protected").join("yiic").exists()
        || (root.join("config").join("web.php").exists()
            && root.join("vendor").join("yiisoft").exists());
    let has_composer = root.join("composer.json").exists();
    let has_package_json = root.join("package.json").exists();
    let has_video_srt = root.join(".agents").join("skills").join("srt-idea").exists()
        || root.join("scripts").join("build_video.py").exists();
    let has_cargo = root.join("Cargo.toml").exists();
    let has_python = root.join("pyproject.toml").exists() || root.join("requirements.txt").exists();

    if has_artisan {
        stacks.push("laravel".to_string());
    }
    if has_yii {
        stacks.push("yii2".to_string());
    }
    if has_composer && !has_artisan && !has_yii {
        stacks.push("php".to_string());
    }
    if has_package_json {
        stacks.push("node".to_string());
    }
    if has_video_srt {
        stacks.push("video-srt".to_string());
    }
    if has_cargo {
        stacks.push("rust".to_string());
    }
    if has_python {
        stacks.push("python".to_string());
    }
    stacks
}

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub added: usize,
    pub missing: usize,
    pub restored: usize,
    pub unchanged: usize,
    pub bad_roots: Vec<String>,
    pub suffixed: Vec<(String, String, String)>,
    pub stack_suggested: usize,
}

pub fn scan(data: &mut serde_json::Value) -> ScanReport {
    let scan_roots = data
        .get("scan_roots")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let mut bad_roots = Vec::new();
    let mut valid_roots = Vec::new();
    for r_val in &scan_roots {
        let r_str = r_val.as_str().unwrap_or("");
        let p = Path::new(r_str);
        if p.is_dir() {
            let res = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            valid_roots.push(res);
        } else {
            bad_roots.push(r_str.to_string());
        }
    }

    if data.get("projects").is_none() {
        data["projects"] = serde_json::json!([]);
    }

    let mut projects_by_realpath: HashMap<String, usize> = HashMap::new();
    if let Some(arr) = data.get("projects").and_then(|p| p.as_array()) {
        for (i, p) in arr.iter().enumerate() {
            if let Some(pth) = p.get("path").and_then(|s| s.as_str()) {
                if !pth.is_empty() {
                    projects_by_realpath.insert(to_realpath(pth), i);
                }
            }
        }
    }

    let mut added_count = 0;
    let mut restored_count = 0;
    let mut unchanged_count = 0;
    let mut suffixed_ids = Vec::new();
    let mut scanned_realpaths = HashSet::new();
    let mut stack_suggested = 0;

    for r in &valid_roots {
        let entries = match std::fs::read_dir(r) {
            Ok(rd) => {
                let mut list = Vec::new();
                for entry_res in rd {
                    if let Ok(e) = entry_res {
                        list.push(e);
                    }
                }
                list.sort_by_key(|e| e.file_name());
                list
            }
            Err(_) => {
                bad_roots.push(r.to_string_lossy().to_string());
                continue;
            }
        };

        for entry in entries {
            let entry_path = entry.path();
            let is_dir = entry_path.is_dir();
            if !is_dir {
                continue;
            }
            let child_path = std::fs::canonicalize(&entry_path).unwrap_or(entry_path);
            let git_path = child_path.join(".git");
            if !git_path.exists() {
                continue;
            }
            let real_child = to_realpath(&child_path.to_string_lossy());
            scanned_realpaths.insert(real_child.clone());

            let projects = data
                .get_mut("projects")
                .and_then(|p| p.as_array_mut())
                .unwrap();

            if let Some(&idx) = projects_by_realpath.get(&real_child) {
                if let Some(proj) = projects.get_mut(idx) {
                    let repo_val = git_remote(&child_path)
                        .map(serde_json::Value::String)
                        .unwrap_or(serde_json::Value::Null);
                    proj["repo"] = repo_val;
                    let was_missing = proj
                        .get("missing")
                        .and_then(|b| b.as_bool())
                        .unwrap_or(false);
                    if was_missing {
                        proj["missing"] = serde_json::Value::Bool(false);
                        restored_count += 1;
                    } else {
                        unchanged_count += 1;
                    }
                }
            } else {
                let base_name = child_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let new_proj = new_entry_from_projects(projects, &child_path, "code");
                let new_id = new_proj
                    .get("id")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                if new_id != base_name {
                    suffixed_ids.push((
                        base_name,
                        new_id,
                        child_path.to_string_lossy().to_string(),
                    ));
                }
                projects.push(new_proj);
                projects_by_realpath.insert(real_child, projects.len() - 1);
                added_count += 1;
            }
        }
    }

    let projects = data
        .get_mut("projects")
        .and_then(|p| p.as_array_mut())
        .unwrap();

    for p in projects.iter_mut() {
        let p_path = match p.get("path").and_then(|s| s.as_str()) {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };
        let real_p = to_realpath(p_path);
        if scanned_realpaths.contains(&real_p) {
            continue;
        }
        let is_dir = Path::new(&real_p).is_dir();
        if !is_dir {
            p["missing"] = serde_json::Value::Bool(true);
        } else {
            let was_missing = p.get("missing").and_then(|b| b.as_bool()).unwrap_or(false);
            if was_missing {
                p["missing"] = serde_json::Value::Bool(false);
                restored_count += 1;
            } else {
                unchanged_count += 1;
            }
        }
    }

    let total_missing = projects
        .iter()
        .filter(|p| p.get("missing").and_then(|b| b.as_bool()).unwrap_or(false))
        .count();

    for p in projects.iter_mut() {
        if p.get("stack").is_none() {
            if let Some(pth) = p.get("path").and_then(|s| s.as_str()) {
                let p_dir = Path::new(pth);
                if p_dir.is_dir() {
                    let detected = detect_stack(p_dir);
                    if !detected.is_empty() {
                        p["stack"] = serde_json::Value::Array(
                            detected.into_iter().map(serde_json::Value::String).collect(),
                        );
                        stack_suggested += 1;
                    }
                }
            }
        }
    }

    ScanReport {
        added: added_count,
        missing: total_missing,
        restored: restored_count,
        unchanged: unchanged_count,
        bad_roots,
        suffixed: suffixed_ids,
        stack_suggested,
    }
}

pub fn add_project(
    data: &mut serde_json::Value,
    path: &Path,
    kind: Option<&str>,
) -> BrainResult<(serde_json::Value, bool)> {
    let abs_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let real_path = to_realpath(&abs_path.to_string_lossy());

    if let Some(projects) = data.get_mut("projects").and_then(|p| p.as_array_mut()) {
        for p in projects.iter_mut() {
            let ppath = p.get("path").and_then(|s| s.as_str()).unwrap_or("");
            if to_realpath(ppath) == real_path {
                if p.get("missing").and_then(|b| b.as_bool()).unwrap_or(false) {
                    p["missing"] = serde_json::Value::Bool(false);
                }
                if p.get("stack").is_none() {
                    let detected = detect_stack(&abs_path);
                    if !detected.is_empty() {
                        p["stack"] = serde_json::Value::Array(
                            detected.into_iter().map(serde_json::Value::String).collect(),
                        );
                    }
                }
                return Ok((p.clone(), false));
            }
        }
    }

    let actual_kind = match kind {
        Some(k) if !matches!(k, "code" | "media" | "idea") => {
            return Err(BrainError::error(format!(
                "Loại dự án '{k}' không hợp lệ — chọn một trong: code, media, idea"
            )));
        }
        Some(k) => k,
        None => {
            if abs_path.join(".git").exists() {
                "code"
            } else {
                "media"
            }
        }
    };

    let mut entry = new_entry(data, &abs_path, actual_kind);
    if entry.get("stack").is_none() {
        let detected = detect_stack(&abs_path);
        if !detected.is_empty() {
            entry["stack"] = serde_json::Value::Array(
                detected.into_iter().map(serde_json::Value::String).collect(),
            );
        }
    }
    if let Some(projects) = data.get_mut("projects").and_then(|p| p.as_array_mut()) {
        projects.push(entry.clone());
    } else {
        data["projects"] = serde_json::json!([entry.clone()]);
    }

    Ok((entry, true))
}

pub fn find_duplicates(projects: &[serde_json::Value]) -> Vec<(String, Vec<String>)> {
    let mut key_to_ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in projects {
        let pid = match p.get("id").and_then(|s| s.as_str()) {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };
        let mut keys = BTreeSet::new();
        let k_id = crate::routing::normalize(pid);
        if !k_id.is_empty() {
            keys.insert(k_id);
        }
        if let Some(name_vi) = p.get("name_vi").and_then(|s| s.as_str()) {
            let k_name = crate::routing::normalize(name_vi);
            if !k_name.is_empty() {
                keys.insert(k_name);
            }
        }
        if let Some(aliases) = p.get("aliases").and_then(|a| a.as_array()) {
            for a in aliases {
                if let Some(alias_str) = a.as_str() {
                    let k_alias = crate::routing::normalize(alias_str);
                    if !k_alias.is_empty() {
                        keys.insert(k_alias);
                    }
                }
            }
        }
        for k in keys {
            let ids = key_to_ids.entry(k).or_default();
            if !ids.iter().any(|existing| existing == pid) {
                ids.push(pid.to_string());
            }
        }
    }

    let mut duplicates: Vec<(String, Vec<String>)> = key_to_ids
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .collect();
    duplicates.sort_by(|a, b| a.0.cmp(&b.0));
    duplicates
}

pub fn cmd_scan(roots: &[String]) -> BrainResult<i32> {
    for r in roots {
        let p = Path::new(r);
        let r_path = if p.is_absolute() {
            p.to_path_buf()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(p)
        } else {
            p.to_path_buf()
        };
        let r_canon = std::fs::canonicalize(&r_path).unwrap_or(r_path);
        if !r_canon.is_dir() {
            return Err(BrainError::error(format!(
                "Thư mục không tồn tại: {}",
                r_canon.display()
            )));
        }
    }

    let map_p = crate::config::map_path();
    let mut data = load_map(&map_p)?;

    let mut existing_roots: Vec<String> = data
        .get("scan_roots")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let mut roots_set: HashSet<String> = existing_roots.iter().map(|r| to_realpath(r)).collect();

    for r in roots {
        let p = Path::new(r);
        let r_abs = if p.is_absolute() {
            p.to_path_buf()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(p)
        } else {
            p.to_path_buf()
        };
        let r_canon = std::fs::canonicalize(&r_abs).unwrap_or(r_abs);
        let r_abs_str = r_canon.to_string_lossy().to_string();
        let r_real = to_realpath(&r_abs_str);
        if !roots_set.contains(&r_real) {
            existing_roots.push(r_abs_str);
            roots_set.insert(r_real);
        }
    }

    if existing_roots.is_empty() {
        return Err(BrainError::error(
            "Chưa có scan_roots — chạy: agent-brain projects scan --root <thư mục>",
        ));
    }

    data["scan_roots"] = serde_json::Value::Array(
        existing_roots
            .iter()
            .map(|s| serde_json::Value::String(s.clone()))
            .collect(),
    );

    let report = scan(&mut data);
    save_map(&map_p, &data)?;

    for br in &report.bad_roots {
        crate::term::emit(&format!(
            "CẢNH BÁO: thư mục gốc không tồn tại, bỏ qua: {br}"
        ));
    }

    let num_roots = existing_roots.len().saturating_sub(report.bad_roots.len());
    let stack_msg = if report.stack_suggested > 0 {
        format!(", {} gợi ý stack", report.stack_suggested)
    } else {
        String::new()
    };
    crate::term::emit_out(&format!(
        "Đã quét {} thư mục gốc: +{} mới, {} thiếu (missing), {} khôi phục, {} không đổi{}",
        num_roots, report.added, report.missing, report.restored, report.unchanged, stack_msg
    ));

    for (base, new_id, _path) in &report.suffixed {
        crate::term::emit_out(&format!("Trùng id: đặt id là '{new_id}' thay cho '{base}'"));
    }

    if let Some(projects_arr) = data.get("projects").and_then(|p| p.as_array()) {
        for (key, ids) in find_duplicates(projects_arr) {
            let ids_str = ids.join(", ");
            crate::term::emit(&format!("CẢNH BÁO: tên/alias trùng \"{key}\": {ids_str}"));
        }
    }

    Ok(0)
}

pub fn cmd_add(path: &str, kind: Option<&str>) -> BrainResult<i32> {
    let p = Path::new(path);
    let target_path = if p.is_absolute() {
        p.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(p)
    } else {
        p.to_path_buf()
    };
    let target_path = std::fs::canonicalize(&target_path).unwrap_or(target_path);
    if !target_path.is_dir() {
        return Err(BrainError::error(format!(
            "Thư mục không tồn tại: {}",
            target_path.display()
        )));
    }

    let map_p = crate::config::map_path();
    let mut data = load_map(&map_p)?;

    let (entry, created) = add_project(&mut data, &target_path, kind)?;
    save_map(&map_p, &data)?;

    let id = entry.get("id").and_then(|i| i.as_str()).unwrap_or("");
    if created {
        crate::term::emit_out(&format!("Đã thêm {id}"));
    } else {
        crate::term::emit_out(&format!("Đã có {id} — giữ nguyên các trường sửa tay"));
    }

    if let Some(projects_arr) = data.get("projects").and_then(|p| p.as_array()) {
        for (key, ids) in find_duplicates(projects_arr) {
            let ids_str = ids.join(", ");
            crate::term::emit(&format!("CẢNH BÁO: tên/alias trùng \"{key}\": {ids_str}"));
        }
    }

    Ok(0)
}

pub fn cmd_list() -> BrainResult<i32> {
    let map_path = crate::config::map_path();
    let map = load_map(&map_path)?;
    let projects = projects_of(&map);
    if projects.is_empty() {
        crate::term::emit_out("Chưa có dự án nào trong projects_map.json");
        return Ok(0);
    }
    for p in projects {
        let sources_str = if p.read_sources.is_empty() {
            String::new()
        } else {
            format!(" [nguồn: {}]", p.read_sources.join(", "))
        };
        crate::term::emit_out(&format!(
            "{}\t{}\t{}{}",
            p.id, p.name_vi, p.path, sources_str
        ));
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_options_default_and_parse() {
        let d = WorkspaceOpts::default();
        assert_eq!(d.base_branch, None);
        assert!(d.hold_project_at_gate);
        assert!(d.auto_checkout_on_approve);
        assert_eq!(WorkspaceOpts::from_value(&serde_json::json!({"id": "x"})), d);
        let set = WorkspaceOpts::from_value(&serde_json::json!({
            "id": "x", "base_branch": " develop ", "hold_project_at_gate": false,
            "auto_checkout_on_approve": false
        }));
        assert_eq!(set.base_branch.as_deref(), Some("develop"));
        assert!(!set.hold_project_at_gate);
        assert!(!set.auto_checkout_on_approve);
        // Invalid values keep the defaults.
        let bad = WorkspaceOpts::from_value(&serde_json::json!({
            "id": "x", "base_branch": "a..b", "hold_project_at_gate": "no"
        }));
        assert_eq!(bad, d);
        let map = serde_json::json!({"projects": [{"id": "x", "base_branch": "rel/1"}]});
        assert_eq!(workspace_of(&map, "x").base_branch.as_deref(), Some("rel/1"));
        assert_eq!(workspace_of(&map, "y"), d);
    }

    #[test]
    fn base_branch_validation() {
        assert_eq!(validate_base_branch("  ").unwrap(), "");
        assert_eq!(validate_base_branch("feature/x-1").unwrap(), "feature/x-1");
        for bad in ["a b", "-x", "/x", "x/", "x.", "a..b", "x.lock", "a/.b", "a~1", "a:b", "x@{1}", "a//b"] {
            assert!(validate_base_branch(bad).is_err(), "{bad}");
        }
        assert!(validate_base_branch(&"a".repeat(BASE_BRANCH_MAX_CHARS + 1)).is_err());
    }

    #[test]
    fn role_pins_parse_and_read() {
        use serde_json::json;
        let pins = parse_role_pins(&json!({"dev": "dev-a"}), &json!({"qa": "claude"})).unwrap();
        assert_eq!(pins.pin_for("dev").unwrap(), Some(RolePin::Member("dev-a".into())));
        assert_eq!(pins.pin_for("qa").unwrap(), Some(RolePin::Engine("claude".into())));
        assert_eq!(pins.pin_for("lead").unwrap(), None);
        assert!(parse_role_pins(&json!({"dev": "a"}), &json!({"dev": "b"})).is_err());
        assert!(parse_role_pins(&json!({"khac": "a"}), &json!(null)).is_err());
        assert!(parse_role_pins(&json!(null), &json!({"dev": "người:x"})).is_err());
        assert!(parse_role_pins(&json!(null), &json!(null)).unwrap().is_empty());

        let map = json!({"projects": [
            {"id": "a", "role_members": {"dev": "dev-a", "khac": "x"}, "role_engines": {"dev": "e1"}},
            {"id": "b"}
        ]});
        let a = role_pins_of(&map, "a");
        assert_eq!(a.members.len(), 1);
        assert!(a.pin_for("dev").is_err());
        assert!(role_pins_of(&map, "b").is_empty());
        assert!(role_pins_of(&map, "missing").is_empty());
    }

    #[test]
    fn timeouts_of_reads_valid_entries_only() {
        let map = serde_json::json!({"projects": [
            {"id": "a", "timeouts": {"dev": 3600, "qa": 30, "khac": 600, "lead": "x"}},
            {"id": "b"}
        ]});
        let a = timeouts_of(&map, "a");
        assert_eq!(a.len(), 1);
        assert_eq!(a.get("dev"), Some(&3600));
        assert!(timeouts_of(&map, "b").is_empty());
        assert!(timeouts_of(&map, "missing").is_empty());
    }

    #[test]
    fn preflight_of_missing_is_default() {
        let map = serde_json::json!({"projects": [
            {"id": "a", "preflight": [{"kind": "tcp", "host": "127.0.0.1", "port": 9}],
             "preflight_retry_minutes": 5},
            {"id": "b"}
        ]});
        let a = preflight_of(&map, "a");
        assert_eq!(a.checks.len(), 1);
        assert_eq!(a.retry_minutes, 5);
        assert!(a.applies_to("dev"));
        assert_eq!(preflight_of(&map, "b"), crate::preflight::Preflight::default());
        assert_eq!(preflight_of(&map, "zz"), crate::preflight::Preflight::default());
    }

    #[test]
    fn test_workflow_of() {
        let map = serde_json::json!({
            "projects": [
                {"id": "a", "workflow": "nhanh"},
                {"id": "b"},
                {"id": "c", "workflow": "  "}
            ]
        });
        assert_eq!(workflow_of(&map, "a"), Some("nhanh".to_string()));
        assert_eq!(workflow_of(&map, "b"), None);
        assert_eq!(workflow_of(&map, "c"), None);
        assert_eq!(workflow_of(&map, "unknown"), None);
    }

    #[test]
    fn team_of_reads_key() {
        let map = serde_json::json!({
            "projects": [
                {"id": "web", "team": " tiet-kiem "},
                {"id": "b"},
                {"id": "c", "team": "  "},
                {"id": "d", "team": 3}
            ]
        });
        assert_eq!(team_of(&map, "web"), Some("tiet-kiem".to_string()));
        assert_eq!(team_of(&map, "b"), None);
        assert_eq!(team_of(&map, "c"), None);
        assert_eq!(team_of(&map, "d"), None);
        assert_eq!(team_of(&map, "unknown"), None);
        assert_eq!(team_of(&serde_json::json!({}), "web"), None);
    }
    fn proj(kind: &str, path: &Path) -> Project {
        Project {
            id: "p".into(),
            path: path.to_string_lossy().to_string(),
            name_vi: "p".into(),
            aliases: vec![],
            kind: kind.into(),
            repo: None,
            profile: "work".into(),
            missing: false,
            stack: vec![],
            read_sources: vec![],
            protocol_dir: String::new(),
            workspace: Default::default(),
        }
    }

    #[test]
    fn require_runnable_idea_vs_code_vs_media() {
        let data = tempfile::tempdir().unwrap();
        let _g = crate::gitops::SHADOW_TEST_ENV
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let old = std::env::var_os("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", data.path());
        let dir = tempfile::tempdir().unwrap();
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();

        let code = require_runnable(&proj("code", dir.path()), &cfg);
        let media = require_runnable(&proj("media", dir.path()), &cfg);
        let idea = require_runnable(&proj("idea", dir.path()), &cfg);
        let has_git = dir.path().join(".git").exists();

        match old {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
        assert!(code
            .unwrap_err()
            .to_string()
            .contains("không phải repo git"));
        assert!(media
            .unwrap_err()
            .to_string()
            .contains("không phải repo git"));
        assert!(idea.is_ok(), "{:?}", idea.err());
        assert!(!has_git);
    }

    #[test]
    fn add_project_rejects_unknown_type() {
        let dir = tempfile::tempdir().unwrap();
        let mut data = serde_json::json!({"projects": []});
        let err = add_project(&mut data, dir.path(), Some("foo")).unwrap_err();
        assert!(err.to_string().contains("code, media, idea"));
        let (e, created) = add_project(&mut data, dir.path(), Some("idea")).unwrap();
        assert!(created);
        assert_eq!(e["type"], "idea");
        assert!(e["repo"].is_null());
    }

    #[test]
    fn test_detect_stack_variants() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path();

        // 1. Laravel: artisan
        let laravel_dir = p.join("laravel");
        std::fs::create_dir_all(&laravel_dir).unwrap();
        std::fs::write(laravel_dir.join("artisan"), "").unwrap();
        assert_eq!(detect_stack(&laravel_dir), vec!["laravel"]);

        // 2. Yii2: yii
        let yii_dir = p.join("yii");
        std::fs::create_dir_all(&yii_dir).unwrap();
        std::fs::write(yii_dir.join("yii"), "").unwrap();
        assert_eq!(detect_stack(&yii_dir), vec!["yii2"]);

        // 3. Yii2: protected/yiic
        let yii_yiic = p.join("yiic");
        std::fs::create_dir_all(yii_yiic.join("protected")).unwrap();
        std::fs::write(yii_yiic.join("protected").join("yiic"), "").unwrap();
        assert_eq!(detect_stack(&yii_yiic), vec!["yii2"]);

        // 4. Yii2: config/web.php + vendor/yiisoft
        let yii_conf = p.join("yiiconf");
        std::fs::create_dir_all(yii_conf.join("config")).unwrap();
        std::fs::create_dir_all(yii_conf.join("vendor").join("yiisoft")).unwrap();
        std::fs::write(yii_conf.join("config").join("web.php"), "").unwrap();
        assert_eq!(detect_stack(&yii_conf), vec!["yii2"]);

        // 5. composer.json without artisan or yii -> php
        let php_dir = p.join("php");
        std::fs::create_dir_all(&php_dir).unwrap();
        std::fs::write(php_dir.join("composer.json"), "").unwrap();
        assert_eq!(detect_stack(&php_dir), vec!["php"]);

        // 6. package.json -> node
        let node_dir = p.join("node");
        std::fs::create_dir_all(&node_dir).unwrap();
        std::fs::write(node_dir.join("package.json"), "").unwrap();
        assert_eq!(detect_stack(&node_dir), vec!["node"]);

        // 7. video-srt: .agents/skills/srt-idea
        let srt_dir1 = p.join("srt1");
        std::fs::create_dir_all(srt_dir1.join(".agents").join("skills").join("srt-idea")).unwrap();
        assert_eq!(detect_stack(&srt_dir1), vec!["video-srt"]);

        // 8. video-srt: scripts/build_video.py
        let srt_dir2 = p.join("srt2");
        std::fs::create_dir_all(srt_dir2.join("scripts")).unwrap();
        std::fs::write(srt_dir2.join("scripts").join("build_video.py"), "").unwrap();
        assert_eq!(detect_stack(&srt_dir2), vec!["video-srt"]);

        // 9. Cargo.toml -> rust
        let rust_dir = p.join("rust");
        std::fs::create_dir_all(&rust_dir).unwrap();
        std::fs::write(rust_dir.join("Cargo.toml"), "").unwrap();
        assert_eq!(detect_stack(&rust_dir), vec!["rust"]);

        // 10. pyproject.toml -> python
        let py_dir1 = p.join("py1");
        std::fs::create_dir_all(&py_dir1).unwrap();
        std::fs::write(py_dir1.join("pyproject.toml"), "").unwrap();
        assert_eq!(detect_stack(&py_dir1), vec!["python"]);

        // 11. requirements.txt -> python
        let py_dir2 = p.join("py2");
        std::fs::create_dir_all(&py_dir2).unwrap();
        std::fs::write(py_dir2.join("requirements.txt"), "").unwrap();
        assert_eq!(detect_stack(&py_dir2), vec!["python"]);

        // 12. Combined: artisan + package.json -> laravel, node
        let combo = p.join("combo");
        std::fs::create_dir_all(&combo).unwrap();
        std::fs::write(combo.join("artisan"), "").unwrap();
        std::fs::write(combo.join("package.json"), "").unwrap();
        assert_eq!(detect_stack(&combo), vec!["laravel", "node"]);
    }

    #[test]
    fn test_scan_only_fills_missing_stack() {
        let tmp = tempfile::tempdir().unwrap();
        let proj_dir = tmp.path().join("proj-rust");
        std::fs::create_dir_all(&proj_dir).unwrap();
        std::fs::write(proj_dir.join("Cargo.toml"), "").unwrap();

        // 1. Existing project with user-chosen stack: ["custom"] -> remains ["custom"]
        let mut data = serde_json::json!({
            "scan_roots": [tmp.path().to_str().unwrap()],
            "projects": [
                {
                    "id": "proj-custom",
                    "path": proj_dir.to_str().unwrap(),
                    "stack": ["custom"]
                },
                {
                    "id": "proj-nostack",
                    "path": proj_dir.to_str().unwrap()
                }
            ]
        });

        let rep = scan(&mut data);
        assert_eq!(rep.stack_suggested, 1);
        let projs = data["projects"].as_array().unwrap();
        assert_eq!(projs[0]["stack"], serde_json::json!(["custom"]));
        assert_eq!(projs[1]["stack"], serde_json::json!(["rust"]));
    }
}
