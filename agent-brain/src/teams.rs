//! Đội hình (TEAM-01): `~/.config/agent-brain/teams.json` cộng hai đội hình có sẵn.
//! Mỗi đội hình gán một engine cho năm vai trò; vai nào cũng dùng được engine nào (D-46),
//! engine `người:<mã>` chỉ dành cho dev (D-24). Đội hình và quy trình là hai danh sách riêng (D-11).

use crate::engine::Engine;
use crate::error::{BrainError, BrainResult};
use crate::staff::LoadedStaff;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const BUILTIN_TEAMS_JSON: &str = include_str!("../assets/teams.default.json");
pub const DEFAULT_TEAM: &str = "mac-dinh";
pub const TEAM_PHONG_BAN: &str = "phong-ban";

/// Đội hình mặc định: trả `phong-ban` khi có thành viên phòng ban, ngược lại trả `mac-dinh`.
pub fn default_team_name() -> &'static str {
    if crate::departments::load().has_members() {
        TEAM_PHONG_BAN
    } else {
        DEFAULT_TEAM
    }
}
pub const MAX_FILE_BYTES: u64 = 262_144;
pub const MAX_DESCRIPTION_CHARS: usize = 200;
pub const ROLES: [&str; 5] = ["lead", "dev", "qa", "plan-review", "code-review"];

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

fn field_error(field: &str, message: impl Into<String>) -> FieldError {
    FieldError {
        field: field.to_string(),
        message: message.into(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Team {
    pub name: String,
    pub description: String,
    pub roles: BTreeMap<String, Engine>,
    /// `builtin` | `file` | `override`
    pub source: String,
    pub warnings: Vec<String>,
}

fn role_label(role: &str) -> &str {
    match role {
        "plan-review" => "review plan",
        "code-review" => "review code",
        other => other,
    }
}

impl Team {
    /// A role the team does not name runs on claude.
    pub fn engine_for(&self, role: &str) -> Engine {
        self.roles.get(role).cloned().unwrap_or(Engine::Claude)
    }

    /// Server string of UI-SPEC "Team summary grammar".
    pub fn summary(&self, staff: &LoadedStaff) -> String {
        let mut groups: Vec<(Engine, Vec<&str>)> = Vec::new();
        for role in ROLES {
            let engine = self.engine_for(role);
            match groups.iter_mut().find(|(e, _)| *e == engine) {
                Some((_, labels)) => labels.push(role_label(role)),
                None => groups.push((engine, vec![role_label(role)])),
            }
        }

        if groups.len() == 1 {
            return format!("tất cả: {}", engine_text(&groups[0].0, staff));
        }

        // claude, deepseek, agy, any other engine id ascending, then people in staff-list order,
        // unknown ids last in id order. The summary has no config, so the UI-SPEC grammar's fixed
        // order is kept for the two historical ids.
        groups.sort_by_key(|(engine, _)| match engine {
            Engine::Claude => (0usize, 0usize, String::new()),
            Engine::Named(id) if id == crate::engine::ENGINE_DEEPSEEK => (1, 0, String::new()),
            Engine::Named(id) if id == crate::engine::ENGINE_AGY => (2, 0, String::new()),
            Engine::Named(id) => (3, 0, id.clone()),
            Engine::Human(id) => match staff.staff.iter().position(|s| s.id == *id) {
                Some(idx) => (4, idx, String::new()),
                None => (5, 0, id.clone()),
            },
        });

        groups
            .iter()
            .map(|(engine, labels)| {
                format!("{}: {}", engine_text(engine, staff), labels.join(", "))
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// `{"description": …, "roles": {all five keys}}` in `ROLES` order.
    pub fn to_file_entry(&self) -> serde_json::Value {
        let mut roles = serde_json::Map::new();
        for role in ROLES {
            roles.insert(
                role.to_string(),
                serde_json::Value::String(self.engine_for(role).to_config_string()),
            );
        }
        let mut entry = serde_json::Map::new();
        entry.insert(
            "description".to_string(),
            serde_json::Value::String(self.description.clone()),
        );
        entry.insert("roles".to_string(), serde_json::Value::Object(roles));
        serde_json::Value::Object(entry)
    }

    pub fn uses_staff(&self, id: &str) -> bool {
        self.roles
            .values()
            .any(|e| matches!(e, Engine::Human(h) if h == id))
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoadedTeams {
    pub teams: Vec<Team>,
    pub errors: Vec<String>,
}

impl LoadedTeams {
    pub fn get(&self, name: &str) -> Option<&Team> {
        self.teams.iter().find(|t| t.name == name)
    }

    pub fn names(&self) -> Vec<String> {
        self.teams.iter().map(|t| t.name.clone()).collect()
    }
}

/// `claude` … ; `người:<id>` reads `người {display_name}` (or the id when unknown).
pub fn engine_text(e: &Engine, staff: &LoadedStaff) -> String {
    match e {
        Engine::Human(id) => format!(
            "{} {}",
            crate::engine::ENGINE_HUMAN,
            staff.display_name_of(id)
        ),
        other => other.as_db_str().to_string(),
    }
}

pub fn validate_entry(
    name: &str,
    value: &serde_json::Value,
    staff: &LoadedStaff,
    source: &str,
) -> Result<Team, Vec<FieldError>> {
    let mut errors = Vec::new();

    if !crate::workflows::validate_name(name) {
        errors.push(field_error(
            "name",
            "Tên chỉ gồm chữ thường không dấu, số và dấu gạch ngang (ví dụ tiet-kiem).",
        ));
    }

    let obj = match value.as_object() {
        Some(o) => o,
        None => {
            errors.push(field_error("name", "Đội hình phải là một object."));
            return Err(errors);
        }
    };

    for key in obj.keys() {
        if key != "description" && key != "roles" {
            errors.push(field_error(key, format!("Khóa lạ: {key}")));
        }
    }

    let mut description = String::new();
    match obj.get("description") {
        Some(serde_json::Value::String(s)) if !s.trim().is_empty() => {
            let trimmed = s.trim();
            if trimmed.chars().count() > MAX_DESCRIPTION_CHARS {
                errors.push(field_error(
                    "description",
                    format!("Mô tả dài quá {MAX_DESCRIPTION_CHARS} ký tự."),
                ));
            } else {
                description = trimmed.to_string();
            }
        }
        _ => errors.push(field_error(
            "description",
            "Nhập mô tả ngắn để chọn đội hình cho đúng.",
        )),
    }

    let mut roles: BTreeMap<String, Engine> = ROLES
        .iter()
        .map(|r| (r.to_string(), Engine::Claude))
        .collect();
    let mut warnings = Vec::new();

    match obj.get("roles") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Object(map)) => {
            for (role, raw) in map {
                if !ROLES.contains(&role.as_str()) {
                    errors.push(field_error(role, format!("Vai trò không hợp lệ: {role}")));
                    continue;
                }
                let engine = match raw.as_str().and_then(Engine::parse) {
                    Some(e) => e,
                    None => {
                        let shown = match raw.as_str() {
                            Some(s) => s.to_string(),
                            None => raw.to_string(),
                        };
                        errors.push(field_error(
                            role,
                            format!(
                                "Engine không hợp lệ cho {role}: {shown} (dùng claude, người:<mã> hoặc id của một engine trong config.json)"
                            ),
                        ));
                        continue;
                    }
                };
                if engine.is_human() && role != "dev" {
                    errors.push(field_error(role, "Engine người chỉ dùng cho vai trò dev."));
                    continue;
                }
                if let Engine::Human(id) = &engine {
                    if staff.get(id).is_none() {
                        warnings.push(format!("Nhân sự {id} không còn trong danh sách."));
                    }
                }
                roles.insert(role.clone(), engine);
            }
        }
        Some(_) => errors.push(field_error(
            "roles",
            "Vai trò phải là một object, mỗi khóa là một vai trò.",
        )),
    }

    if errors.is_empty() {
        Ok(Team {
            name: name.to_string(),
            description,
            roles,
            source: source.to_string(),
            warnings,
        })
    } else {
        Err(errors)
    }
}

/// ENG-10 (D-36, D-38, D-42): the first role of `team`, in `ROLES` order, whose configured engine
/// is restricted to profiles that exclude `project_profile`. Returns the `engine::profile_refusal`
/// text unchanged so every bind point shows the same words. An id missing from the list is not
/// this rule's job (D-43 handles it); `claude` and people have no restriction.
pub fn team_profile_refusal(
    team: &Team,
    engines: &crate::config::EnginesCfg,
    project_id: &str,
    project_profile: &str,
) -> Option<String> {
    ROLES.iter().find_map(|role| {
        let Engine::Named(id) = team.engine_for(role) else {
            return None;
        };
        let def = engines.get(&id)?;
        crate::engine::profile_refusal(def, project_id, project_profile, role)
    })
}

/// The team role whose engine a run role uses (`analyst` runs on the Lead's engine,
/// `analysis-review` on the plan reviewer's).
pub fn team_role_of(role: &str) -> &str {
    match role {
        "analyst" => "lead",
        "analysis-review" => "plan-review",
        other => other,
    }
}

/// C3: whether a team can run one role of a task now. One check shared by the quota message
/// buttons (Telegram), the dashboard re-run dialog, `quota_rerun` and the autopilot's team
/// restore: `team_profile_refusal`, then the engine of the role must be runnable now (`sẵn sàng`,
/// or `chưa kiểm` which runs marked, D-40) in `engine::readiness`; on the department team, a department
/// with members needs one usable member for the role and level (`departments::ready_candidates`, the filter of `pick_from`).
pub struct TeamReadiness {
    pub statuses: Vec<crate::engine::EngineStatus>,
    pub departments: crate::departments::LoadedDepartments,
    pub project_id: String,
    pub project_profile: String,
    pub level: crate::departments::Level,
}

impl TeamReadiness {
    /// Readiness read now (local checks only, `engine::readiness`) and departments.json.
    pub fn current(
        cfg: &crate::config::Config,
        project_id: &str,
        project_profile: &str,
        level: crate::departments::Level,
    ) -> TeamReadiness {
        let env = crate::profile::current_env();
        TeamReadiness {
            statuses: crate::engine::readiness(cfg, &env, 0),
            departments: crate::departments::load_path(
                &crate::departments::departments_path(),
                cfg,
            ),
            project_id: project_id.to_string(),
            project_profile: project_profile.to_string(),
            level,
        }
    }

    /// `Some(reason)` when `team` cannot run `role` now; the reason names the engine and what
    /// readiness says is missing (an env var name, never its value).
    pub fn refusal(&self, cfg: &crate::config::Config, team: &Team, role: &str) -> Option<String> {
        if let Some(msg) =
            team_profile_refusal(team, &cfg.engines, &self.project_id, &self.project_profile)
        {
            return Some(msg);
        }
        let engine = team.engine_for(team_role_of(role));
        if team.name == TEAM_PHONG_BAN && !engine.is_human() {
            if let Some(dept) = crate::departments::dept_of_role(role) {
                let members = self.departments.members_of(dept);
                if !members.is_empty() {
                    let ready = crate::departments::ready_candidates(
                        cfg,
                        &self.statuses,
                        &members,
                        role,
                        self.level,
                        &self.project_profile,
                    );
                    if ready.is_empty() {
                        return Some(format!(
                            "Đội hình {}: không có thành viên phòng ban nào sẵn sàng cho vai {role} (độ khó {}).",
                            team.name,
                            self.level.display_vi()
                        ));
                    }
                    return None;
                }
            }
        }
        let name = engine.as_db_str();
        match self.statuses.iter().find(|s| s.name == name) {
            None => Some(format!(
                "Đội hình {}: engine '{name}' của vai {role} chưa khai báo trong config.json.",
                team.name
            )),
            Some(st) if !crate::engine::readiness_allows_run(st.readiness) => {
                let why = st
                    .reason
                    .as_deref()
                    .map(|r| format!(" — {r}"))
                    .unwrap_or_default();
                Some(format!(
                    "Đội hình {}: engine '{name}' của vai {role} chưa sẵn sàng ({}){why}",
                    team.name, st.word
                ))
            }
            Some(_) => None,
        }
    }

    /// Names of the teams of `teams`, in list order, that can run `role` now.
    pub fn ready_names(
        &self,
        cfg: &crate::config::Config,
        teams: &LoadedTeams,
        role: &str,
    ) -> Vec<String> {
        teams
            .teams
            .iter()
            .filter(|t| self.refusal(cfg, t, role).is_none())
            .map(|t| t.name.clone())
            .collect()
    }

    /// D-40: trả về id engine của role nếu engine đó ở trạng thái `chưa kiểm` (chua_kiem), ngược lại None.
    /// Dùng để gắn dấu `chưa kiểm` lên các lựa chọn chạy chưa kiểm chứng (06-29).
    /// Trên `phong-ban` hàm này đọc engine của chính team, không đọc thành viên phòng ban.
    pub fn unverified_engine(&self, team: &Team, role: &str) -> Option<String> {
        let engine = team.engine_for(team_role_of(role));
        if engine.is_human() {
            return None;
        }
        let name = engine.as_db_str();
        let st = self.statuses.iter().find(|s| s.name == name)?;
        if st.readiness == crate::engine::Readiness::ChuaKiem.key() {
            Some(name.to_string())
        } else {
            None
        }
    }
}

/// D-37 / D-43: rules between a team and the engine list in config.json.
///
/// - a non-dev role on a `dsh`-kind id is an error (dsh has no read-only mode);
/// - with `strict`, an id missing from the list is an error too (Settings save). The loader passes
///   `strict = false`, so a hand-edited team that names an unknown id still loads (the built-in
///   `tiet-kiem` keeps `deepseek`) and is only warned about, see `catalog_warnings`.
pub fn catalog_errors(
    team: &Team,
    engines: &crate::config::EnginesCfg,
    strict: bool,
) -> Vec<FieldError> {
    let mut errors = Vec::new();
    for role in ROLES {
        let Engine::Named(id) = team.engine_for(role) else {
            continue;
        };
        match engines.kind_of(&id) {
            Some(crate::config::EngineKind::Dsh) if role != "dev" => errors.push(field_error(
                role,
                format!("Engine '{id}' là kiểu dsh, chỉ dùng cho vai trò dev."),
            )),
            None if strict => errors.push(field_error(
                role,
                format!("Engine '{id}' không có trong config.json (engines)."),
            )),
            _ => {}
        }
    }
    errors
}

/// D-43: one line per engine id the team names that config.json does not declare, in role order,
/// without repeats. Nothing resolves such an id to another endpoint.
pub fn catalog_warnings(team: &Team, engines: &crate::config::EnginesCfg) -> Vec<String> {
    let mut warnings: Vec<String> = Vec::new();
    for role in ROLES {
        let Engine::Named(id) = team.engine_for(role) else {
            continue;
        };
        if engines.get(&id).is_none() {
            let line = format!("Engine '{id}' chưa khai báo trong config.json.");
            if !warnings.contains(&line) {
                warnings.push(line);
            }
        }
    }
    warnings
}

pub fn teams_path() -> PathBuf {
    crate::config::config_dir().join("teams.json")
}

fn load_builtins(staff: &LoadedStaff) -> Vec<Team> {
    let parsed: serde_json::Value = serde_json::from_str(BUILTIN_TEAMS_JSON)
        .expect("teams.default.json có sẵn trong binary phải là JSON hợp lệ");
    let obj = parsed
        .as_object()
        .expect("teams.default.json có sẵn trong binary phải là một object");
    obj.iter()
        .map(|(name, value)| {
            validate_entry(name, value, staff, "builtin")
                .unwrap_or_else(|e| panic!("Đội hình có sẵn '{name}' không hợp lệ: {e:?}"))
        })
        .collect()
}

pub fn load_from(text: Option<&str>, staff: &LoadedStaff) -> LoadedTeams {
    load_from_with(text, staff, None)
}

/// As `load_from`; with `Some(engines)` an entry that breaks the D-37 role rule (`catalog_errors`
/// with `strict = false`) becomes an error line and is not usable.
pub fn load_from_with(
    text: Option<&str>,
    staff: &LoadedStaff,
    engines: Option<&crate::config::EnginesCfg>,
) -> LoadedTeams {
    let mut teams = load_builtins(staff);
    let mut errors = Vec::new();

    let Some(file_text) = text else {
        return LoadedTeams { teams, errors };
    };

    if file_text.len() as u64 > MAX_FILE_BYTES {
        errors.push("teams.json lớn quá 256 KiB — bỏ qua file.".to_string());
        return LoadedTeams { teams, errors };
    }

    let parsed: serde_json::Value = match serde_json::from_str(file_text) {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!(
                "Dòng {}, cột {}: teams.json không phải JSON hợp lệ.",
                e.line(),
                e.column()
            ));
            return LoadedTeams { teams, errors };
        }
    };

    let Some(obj) = parsed.as_object() else {
        errors.push(
            "Dòng 1: teams.json phải là một object, mỗi khóa là tên một đội hình.".to_string(),
        );
        return LoadedTeams { teams, errors };
    };

    let lines = crate::workflows::key_lines(file_text);
    for (name, value) in obj {
        let line = lines.get(name).copied().unwrap_or(1);
        let existing = teams.iter().position(|t| t.name == *name);
        let source = if existing.is_some() {
            "override"
        } else {
            "file"
        };
        match validate_entry(name, value, staff, source) {
            Ok(team) => {
                let catalog = engines
                    .map(|e| catalog_errors(&team, e, false))
                    .unwrap_or_default();
                if catalog.is_empty() {
                    match existing {
                        Some(pos) => teams[pos] = team,
                        None => teams.push(team),
                    }
                } else {
                    for e in catalog {
                        errors.push(format!("Dòng {line}: đội hình '{name}': {}", e.message));
                    }
                }
            }
            Err(errs) => {
                for e in errs {
                    errors.push(format!("Dòng {line}: đội hình '{name}': {}", e.message));
                }
            }
        }
    }

    LoadedTeams { teams, errors }
}

pub fn load_path(path: &Path, staff: &LoadedStaff) -> LoadedTeams {
    load_path_with(path, staff, None)
}

pub fn load_path_with(
    path: &Path,
    staff: &LoadedStaff,
    engines: Option<&crate::config::EnginesCfg>,
) -> LoadedTeams {
    if !path.exists() {
        return load_from_with(None, staff, engines);
    }
    match std::fs::metadata(path) {
        Ok(m) if m.len() > MAX_FILE_BYTES => {
            let mut l = load_from_with(None, staff, engines);
            l.errors
                .push("teams.json lớn quá 256 KiB — bỏ qua file.".to_string());
            return l;
        }
        Ok(_) => {}
        Err(e) => {
            let mut l = load_from_with(None, staff, engines);
            l.errors.push(format!("Không đọc được teams.json: {e}"));
            return l;
        }
    }
    match std::fs::read_to_string(path) {
        Ok(text) => load_from_with(Some(&text), staff, engines),
        Err(e) => {
            let mut l = load_from_with(None, staff, engines);
            l.errors.push(format!("Không đọc được teams.json: {e}"));
            l
        }
    }
}

/// Loads staff first, then teams with that staff and the engines of the configuration (D-37). A
/// configuration that cannot be read falls back to the default engine list, so the teams still
/// load; the caller that needs the configuration reports its own error.
pub fn load() -> (LoadedTeams, LoadedStaff) {
    let staff = crate::staff::load();
    let engines = match crate::config::load_config() {
        Ok(cfg) => cfg.engines,
        Err(_) => crate::config::EnginesCfg::defaults("claude"),
    };
    let teams = load_path_with(&teams_path(), &staff, Some(&engines));
    (teams, staff)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTeam {
    pub team: Team,
    pub note: Option<String>,
}

fn default_team(loaded: &LoadedTeams) -> BrainResult<&Team> {
    let name = default_team_name();
    if let Some(t) = loaded.get(name) {
        return Ok(t);
    }
    loaded.get(DEFAULT_TEAM).ok_or_else(|| {
        BrainError::error(format!("Không tìm thấy đội hình mặc định '{DEFAULT_TEAM}'"))
    })
}

pub fn resolve(
    loaded: &LoadedTeams,
    requested: Option<&str>,
    project_default: Option<&str>,
) -> BrainResult<ResolvedTeam> {
    if let Some(name) = requested {
        let name = name.trim();
        return match loaded.get(name) {
            Some(team) => Ok(ResolvedTeam {
                team: team.clone(),
                note: None,
            }),
            None => Err(BrainError::error(format!(
                "Không có đội hình '{name}'. Đội hình có: {}",
                loaded.names().join(", ")
            ))),
        };
    }

    if let Some(p_name) = project_default {
        let trimmed = p_name.trim();
        if !trimmed.is_empty() {
            return match loaded.get(trimmed) {
                Some(team) => Ok(ResolvedTeam {
                    team: team.clone(),
                    note: None,
                }),
                None => Ok(ResolvedTeam {
                    team: default_team(loaded)?.clone(),
                    note: Some(format!(
                        "Đội hình '{trimmed}' của dự án không có trong teams.json — dùng {DEFAULT_TEAM}."
                    )),
                }),
            };
        }
    }

    Ok(ResolvedTeam {
        team: default_team(loaded)?.clone(),
        note: None,
    })
}

// ---- persisted in tasks.team_json (plan 06-04) ----

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TeamChange {
    pub at: i64,
    /// dashboard | telegram | cli | autopilot (C3 restore)
    pub by: String,
    pub from: String,
    pub to: String,
    /// `quota` when a quota re-run switched the team, `quota_restore` when the autopilot gave the
    /// team back after the quota reset (C3); absent for an ordinary change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// For `reason: quota`: when the stopped engine's quota resets (ms), if the run reported it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_at: Option<i64>,
}

pub const CHANGE_REASON_QUOTA: &str = "quota";
pub const CHANGE_REASON_QUOTA_RESTORE: &str = "quota_restore";

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PushResult {
    pub ok: bool,
    pub command: String,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HumanWait {
    /// waiting | rejected
    pub mode: String,
    pub role: String,
    /// implement | fix
    pub stage: String,
    #[serde(default)]
    pub fix_for: Option<String>,
    #[serde(default)]
    pub round: Option<i64>,
    pub staff: String,
    pub since_ms: i64,
    pub branch: String,
    pub base: String,
    #[serde(default)]
    pub report_paths: Vec<String>,
    #[serde(default)]
    pub push: Option<PushResult>,
    #[serde(default)]
    pub autopilot: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TaskTeam {
    pub name: String,
    #[serde(default)]
    pub history: Vec<TeamChange>,
    #[serde(default)]
    pub wait: Option<HumanWait>,
}

impl TaskTeam {
    pub fn new(name: &str) -> TaskTeam {
        TaskTeam {
            name: name.to_string(),
            history: Vec::new(),
            wait: None,
        }
    }

    pub fn parse(json: Option<&str>) -> Option<TaskTeam> {
        serde_json::from_str(json?).ok()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// C3: the quota switch to undo at `now_ms`: the last history entry when it is a
    /// `reason: quota` change whose `reset_at` has passed, the task still has that entry's team
    /// and no person's step is pending. An entry without a reset time is never undone.
    pub fn quota_restore_target(&self, now_ms: i64) -> Option<&TeamChange> {
        if self.wait.is_some() {
            return None;
        }
        let last = self.history.last()?;
        let due = last.reason.as_deref() == Some(CHANGE_REASON_QUOTA)
            && last.reset_at.is_some_and(|r| now_ms >= r)
            && last.to == self.name
            && last.from != self.name;
        due.then_some(last)
    }

    /// The document after giving the team back to `entry.from` (a `quota_restore` entry by `by`).
    pub fn restored(&self, entry: &TeamChange, by: &str, now_ms: i64) -> TaskTeam {
        let mut next = self.clone();
        next.name = entry.from.clone();
        next.history.push(TeamChange {
            at: now_ms,
            by: by.to_string(),
            from: entry.to.clone(),
            to: entry.from.clone(),
            reason: Some(CHANGE_REASON_QUOTA_RESTORE.to_string()),
            reset_at: None,
        });
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_staff() -> LoadedStaff {
        LoadedStaff::default()
    }

    fn staff_ha() -> LoadedStaff {
        crate::staff::load_from(Some(r#"{"ha": {"display_name": "Hà"}}"#))
    }

    fn names(l: &LoadedTeams) -> Vec<&str> {
        l.teams.iter().map(|t| t.name.as_str()).collect()
    }

    #[test]
    fn builtins_only() {
        let l = load_from(None, &no_staff());
        assert!(l.errors.is_empty());
        assert_eq!(names(&l), ["mac-dinh", "phong-ban", "tiet-kiem"]);
        assert!(l.teams.iter().all(|t| t.source == "builtin"));

        let md = l.get("mac-dinh").unwrap();
        for role in ROLES {
            assert_eq!(md.engine_for(role), Engine::Claude);
        }
        let tk = l.get("tiet-kiem").unwrap();
        assert_eq!(tk.engine_for("dev"), Engine::Named("deepseek".to_string()));
        assert_eq!(tk.engine_for("qa"), Engine::Claude);
        assert_eq!(tk.engine_for("lead"), Engine::Claude);

        assert_eq!(md.summary(&no_staff()), "tất cả: claude");
        assert_eq!(
            tk.summary(&no_staff()),
            "claude: lead, qa, review plan, review code · deepseek: dev"
        );
        // Byte-identical for equal inputs.
        assert_eq!(load_from(None, &no_staff()), l);
    }

    #[test]
    fn file_entries_and_override() {
        let text = r#"{
  "doi-a": {"description": "Thử", "roles": {"dev": "deepseek", "plan-review": "agy"}},
  "mac-dinh": {"description": "Ghi đè", "roles": {}}
}"#;
        let l = load_from(Some(text), &no_staff());
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        assert_eq!(names(&l), ["mac-dinh", "phong-ban", "tiet-kiem", "doi-a"]);
        assert_eq!(l.get("mac-dinh").unwrap().source, "override");
        assert_eq!(l.get("mac-dinh").unwrap().description, "Ghi đè");
        assert_eq!(l.get("tiet-kiem").unwrap().source, "builtin");
        let a = l.get("doi-a").unwrap();
        assert_eq!(a.source, "file");
        assert_eq!(a.engine_for("lead"), Engine::Claude);
        assert_eq!(
            a.engine_for("plan-review"),
            Engine::Named("agy".to_string())
        );
        assert_eq!(
            a.summary(&no_staff()),
            "claude: lead, qa, review code · deepseek: dev · agy: review plan"
        );

        let entry = a.to_file_entry();
        assert_eq!(entry["description"], "Thử");
        let keys: Vec<&String> = entry["roles"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ROLES.iter().collect::<Vec<_>>());
        assert_eq!(entry["roles"]["dev"], "deepseek");
        assert!(!a.uses_staff("ha"));
    }

    #[test]
    fn human_staff_and_warning() {
        let text = r#"{"doi-b": {"description": "Có người", "roles": {"dev": "người:ha"}}}"#;
        let l = load_from(Some(text), &staff_ha());
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        let b = l.get("doi-b").unwrap();
        assert!(b.warnings.is_empty());
        assert_eq!(b.engine_for("dev"), Engine::Human("ha".to_string()));
        assert!(b.summary(&staff_ha()).contains("người Hà"));
        assert_eq!(
            b.summary(&staff_ha()),
            "claude: lead, qa, review plan, review code · người Hà: dev"
        );
        assert!(b.uses_staff("ha"));
        assert!(!b.uses_staff("khac"));
        assert_eq!(b.to_file_entry()["roles"]["dev"], "người:ha");

        let text = r#"{"doi-c": {"description": "Mất người", "roles": {"dev": "người:khong-co"}}}"#;
        let l = load_from(Some(text), &staff_ha());
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        let c = l.get("doi-c").unwrap();
        assert_eq!(c.warnings, ["Nhân sự khong-co không còn trong danh sách."]);
        assert_eq!(
            c.summary(&staff_ha()),
            "claude: lead, qa, review plan, review code · người khong-co: dev"
        );
    }

    fn one_error(entry: &str) -> Vec<String> {
        let text = format!("{{\n{entry}\n}}");
        let l = load_from(Some(&text), &staff_ha());
        assert_eq!(
            names(&l),
            ["mac-dinh", "phong-ban", "tiet-kiem"],
            "bad entry must not load"
        );
        l.errors
    }

    #[test]
    fn entry_errors_keep_others() {
        // A valid entry after an invalid one still loads.
        let text = r#"{
"doi-x": {"description": "Sai", "roles": {"qa": "người:ha"}},
"doi-ok": {"description": "Tốt", "roles": {}}
}"#;
        let l = load_from(Some(text), &no_staff());
        assert_eq!(names(&l), ["mac-dinh", "phong-ban", "tiet-kiem", "doi-ok"]);
        assert_eq!(
            l.errors,
            ["Dòng 2: đội hình 'doi-x': Engine người chỉ dùng cho vai trò dev."]
        );

        // D-46: QA takes any engine, not only claude.
        let text = r#"{"doi-q": {"description": "Q", "roles": {"qa": "deepseek"}}}"#;
        let l = load_from(Some(text), &no_staff());
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        assert_eq!(
            l.get("doi-q").unwrap().engine_for("qa"),
            Engine::Named("deepseek".to_string())
        );

        let e = one_error(r#""doi-x": {"description": "d", "roles": {"lead": "người:ha"}}"#);
        assert_eq!(
            e,
            ["Dòng 2: đội hình 'doi-x': Engine người chỉ dùng cho vai trò dev."]
        );

        let e = one_error(r#""doi-x": {"description": "d", "roles": {"tester": "claude"}}"#);
        assert_eq!(
            e,
            ["Dòng 2: đội hình 'doi-x': Vai trò không hợp lệ: tester"]
        );

        let e = one_error(r#""doi-x": {"description": "d", "roles": {"dev": "GPT 4"}}"#);
        assert_eq!(
            e,
            ["Dòng 2: đội hình 'doi-x': Engine không hợp lệ cho dev: GPT 4 (dùng claude, người:<mã> hoặc id của một engine trong config.json)"]
        );

        let e = one_error(r#""doi-x": {"description": "  ", "roles": {}}"#);
        assert_eq!(
            e,
            ["Dòng 2: đội hình 'doi-x': Nhập mô tả ngắn để chọn đội hình cho đúng."]
        );
        let e = one_error(r#""doi-x": {"roles": {}}"#);
        assert!(e[0].ends_with("Nhập mô tả ngắn để chọn đội hình cho đúng."));

        let long = "m".repeat(201);
        let e = one_error(&format!(r#""doi-x": {{"description": "{long}"}}"#));
        assert_eq!(e, ["Dòng 2: đội hình 'doi-x': Mô tả dài quá 200 ký tự."]);

        // 200 characters (multi-byte) is fine.
        let ok200 = "ế".repeat(200);
        let text = format!(r#"{{"doi-x": {{"description": "{ok200}"}}}}"#);
        let l = load_from(Some(&text), &no_staff());
        assert!(l.errors.is_empty(), "{:?}", l.errors);

        let name_msg = "Tên chỉ gồm chữ thường không dấu, số và dấu gạch ngang (ví dụ tiet-kiem).";
        let e = one_error(r#""Doi-A": {"description": "d"}"#);
        assert_eq!(e, [format!("Dòng 2: đội hình 'Doi-A': {name_msg}")]);
        let long_name = "a".repeat(41);
        let e = one_error(&format!(r#""{long_name}": {{"description": "d"}}"#));
        assert_eq!(e, [format!("Dòng 2: đội hình '{long_name}': {name_msg}")]);
        // 40 bytes is accepted.
        let ok_name = "a".repeat(40);
        let text = format!(r#"{{"{ok_name}": {{"description": "d"}}}}"#);
        assert!(load_from(Some(&text), &no_staff()).get(&ok_name).is_some());
    }

    #[test]
    fn file_level_errors() {
        let l = load_from(Some(r#"{"a": "#), &no_staff());
        assert_eq!(l.errors.len(), 1);
        assert!(l.errors[0].starts_with("Dòng 1, cột"));
        assert!(l.errors[0].ends_with("teams.json không phải JSON hợp lệ."));
        assert_eq!(names(&l), ["mac-dinh", "phong-ban", "tiet-kiem"]);

        let big = " ".repeat(262_145);
        let l = load_from(Some(&big), &no_staff());
        assert_eq!(l.errors, ["teams.json lớn quá 256 KiB — bỏ qua file."]);
        assert_eq!(names(&l), ["mac-dinh", "phong-ban", "tiet-kiem"]);

        let l = load_from(Some("[]"), &no_staff());
        assert_eq!(
            l.errors,
            ["Dòng 1: teams.json phải là một object, mỗi khóa là tên một đội hình."]
        );
        assert_eq!(names(&l), ["mac-dinh", "phong-ban", "tiet-kiem"]);
    }

    #[test]
    fn load_path_missing_and_oversize() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("teams.json");
        let l = load_path(&p, &no_staff());
        assert!(l.errors.is_empty());
        assert_eq!(names(&l), ["mac-dinh", "phong-ban", "tiet-kiem"]);

        std::fs::write(&p, " ".repeat(262_145)).unwrap();
        let l = load_path(&p, &no_staff());
        assert_eq!(l.errors, ["teams.json lớn quá 256 KiB — bỏ qua file."]);
    }

    #[test]
    fn resolve_rules() {
        let text = r#"{"doi-a": {"description": "Thử", "roles": {"dev": "deepseek"}}}"#;
        let l = load_from(Some(text), &no_staff());

        let r = resolve(&l, Some("doi-a"), None).unwrap();
        assert_eq!(r.team.name, "doi-a");
        assert_eq!(r.note, None);

        let err = resolve(&l, Some("khong-co"), None).unwrap_err();
        assert_eq!(
            err.message,
            "Không có đội hình 'khong-co'. Đội hình có: mac-dinh, phong-ban, tiet-kiem, doi-a"
        );

        let r = resolve(&l, None, Some("khong-co")).unwrap();
        assert_eq!(r.team.name, default_team_name());
        assert_eq!(
            r.note.as_deref(),
            Some("Đội hình 'khong-co' của dự án không có trong teams.json — dùng mac-dinh.")
        );

        let r = resolve(&l, None, Some(" doi-a ")).unwrap();
        assert_eq!(r.team.name, "doi-a");
        assert_eq!(r.note, None);

        let r = resolve(&l, None, Some("  ")).unwrap();
        assert_eq!(r.team.name, default_team_name());
        assert_eq!(r.note, None);

        let r = resolve(&l, None, None).unwrap();
        assert_eq!(r.team.name, default_team_name());
        assert_eq!(r.note, None);

        // An explicit request wins over the project default.
        let r = resolve(&l, Some("tiet-kiem"), Some("doi-a")).unwrap();
        assert_eq!(r.team.name, "tiet-kiem");
    }

    fn engines_with_claude_max() -> crate::config::EnginesCfg {
        let mut v: serde_json::Value =
            serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap();
        v["engines"] = serde_json::json!([{
            "id": "claude-max",
            "kind": "anthropic",
            "command": "claude",
            "pin_profile": "personal",
            "models": {"lead": "opus", "dev": "opus"}
        }]);
        crate::config::config_from_value(&v)
            .expect("config with claude-max")
            .engines
    }

    fn team_of(roles: &[(&str, &str)]) -> Team {
        let mut map = serde_json::Map::new();
        for (r, e) in roles {
            map.insert(r.to_string(), serde_json::Value::String(e.to_string()));
        }
        validate_entry(
            "doi-t",
            &serde_json::json!({"description": "t", "roles": map}),
            &no_staff(),
            "file",
        )
        .expect("valid team")
    }

    #[test]
    fn team_profile_refusal_rules() {
        let engines = engines_with_claude_max();

        let dev_max = team_of(&[("dev", "claude-max")]);
        let text = team_profile_refusal(&dev_max, &engines, "web", "work").expect("work refused");
        assert_eq!(
            text,
            "Engine 'claude-max' chỉ dùng cho dự án profile personal — dự án 'web' dùng profile work (ranh giới work/personal, SPK-10). Đổi engine của vai trò dev hoặc chọn đội hình khác."
        );
        assert_eq!(
            team_profile_refusal(&dev_max, &engines, "web", "personal"),
            None
        );

        // The role named is the one that carries the restricted engine.
        let lead_max = team_of(&[("lead", "claude-max")]);
        let text = team_profile_refusal(&lead_max, &engines, "web", "work").unwrap();
        assert!(text.ends_with("Đổi engine của vai trò lead hoặc chọn đội hình khác."));

        // An id that is not in the list is not this rule's job.
        let unknown = team_of(&[("dev", "khong-co")]);
        assert_eq!(
            team_profile_refusal(&unknown, &engines, "web", "work"),
            None
        );

        // An all-claude team and a team on an unrestricted entry are never refused.
        let all_claude = team_of(&[]);
        assert_eq!(
            team_profile_refusal(&all_claude, &engines, "web", "work"),
            None
        );
    }

    #[test]
    fn catalog_rules() {
        let mut v: serde_json::Value =
            serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap();
        v["engines"] = serde_json::json!([
            {"id": "dsh", "kind": "dsh", "container": "fake-dsh"},
            {"id": "deepseek", "kind": "anthropic", "command": "claude",
             "base_url": "https://example.invalid/anthropic", "key_env": "DEEPSEEK_API_KEY",
             "models": {"dev": "deepseek-chat"}}
        ]);
        let engines = crate::config::config_from_value(&v)
            .expect("config with a dsh entry")
            .engines;

        // D-37: dsh on lead is one error on that role; on dev it is accepted.
        let lead_dsh = team_of(&[("lead", "dsh")]);
        assert_eq!(
            catalog_errors(&lead_dsh, &engines, false),
            [FieldError {
                field: "lead".to_string(),
                message: "Engine 'dsh' là kiểu dsh, chỉ dùng cho vai trò dev.".to_string()
            }]
        );
        let dev_dsh = team_of(&[("dev", "dsh")]);
        assert!(catalog_errors(&dev_dsh, &engines, true).is_empty());

        // D-43: an unknown id is no error for the loader, an error for Settings save.
        let unknown = team_of(&[("dev", "khong-co"), ("plan-review", "khong-co")]);
        assert!(catalog_errors(&unknown, &engines, false).is_empty());
        let strict = catalog_errors(&unknown, &engines, true);
        assert_eq!(
            strict
                .iter()
                .map(|e| (e.field.as_str(), e.message.as_str()))
                .collect::<Vec<_>>(),
            [
                (
                    "dev",
                    "Engine 'khong-co' không có trong config.json (engines)."
                ),
                (
                    "plan-review",
                    "Engine 'khong-co' không có trong config.json (engines)."
                )
            ]
        );
        // ... and one warning, however many roles name it.
        assert_eq!(
            catalog_warnings(&unknown, &engines),
            ["Engine 'khong-co' chưa khai báo trong config.json."]
        );
        assert!(catalog_warnings(&dev_dsh, &engines).is_empty());

        // The loader drops a team that breaks D-37 and keeps one with an unknown id.
        let text = r#"{
"doi-sai": {"description": "Sai", "roles": {"lead": "dsh"}},
"doi-cu": {"description": "Cũ", "roles": {"dev": "khong-co"}}
}"#;
        let l = load_from_with(Some(text), &no_staff(), Some(&engines));
        assert_eq!(names(&l), ["mac-dinh", "phong-ban", "tiet-kiem", "doi-cu"]);
        assert_eq!(
            l.errors,
            ["Dòng 2: đội hình 'doi-sai': Engine 'dsh' là kiểu dsh, chỉ dùng cho vai trò dev."]
        );
        // Without the engine list nothing is checked (every pre-existing caller).
        let l = load_from(Some(text), &no_staff());
        assert!(l.errors.is_empty());
    }

    #[test]
    fn task_team_doc() {
        let t = TaskTeam::new("tiet-kiem");
        let json = t.to_json();
        assert_eq!(TaskTeam::parse(Some(&json)), Some(t));
        assert_eq!(TaskTeam::parse(None), None);
        assert_eq!(TaskTeam::parse(Some("x")), None);

        let bare = TaskTeam::parse(Some(r#"{"name": "a"}"#)).unwrap();
        assert_eq!(bare.name, "a");
        assert!(bare.history.is_empty());
        assert!(bare.wait.is_none());

        let mut full = TaskTeam::new("doi-b");
        full.history.push(TeamChange {
            at: 1_700_000_000,
            by: "dashboard".to_string(),
            from: "mac-dinh".to_string(),
            to: "doi-b".to_string(),
            reason: None,
            reset_at: None,
        });
        full.wait = Some(HumanWait {
            mode: "waiting".to_string(),
            role: "dev".to_string(),
            stage: "implement".to_string(),
            fix_for: None,
            round: Some(2),
            staff: "ha".to_string(),
            since_ms: 1_700_000_000_000,
            branch: "task/1".to_string(),
            base: "abc123".to_string(),
            report_paths: vec![".qa-report.md".to_string()],
            push: Some(PushResult {
                ok: false,
                command: "git push origin task/1".to_string(),
                error: Some("từ chối".to_string()),
            }),
            autopilot: true,
        });
        assert_eq!(TaskTeam::parse(Some(&full.to_json())), Some(full));

        // A minimal wait (only required fields) parses with defaults.
        let min = r#"{"name":"x","wait":{"mode":"waiting","role":"dev","stage":"implement","staff":"ha","since_ms":1,"branch":"b","base":"c"}}"#;
        let w = TaskTeam::parse(Some(min)).unwrap().wait.unwrap();
        assert!(w.report_paths.is_empty());
        assert!(w.push.is_none());
        assert!(!w.autopilot);
    }

    // ---- C3: readiness of a team for one role ----

    fn c3_cfg() -> crate::config::Config {
        let mut v: serde_json::Value =
            serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap();
        v["engines"] = serde_json::json!([
            {"id": "gw", "kind": "anthropic", "command": "claude",
             "base_url": "https://gw.invalid/anthropic", "key_env": "GW_TEST_KEY",
             "verified": "e09 ok", "models": {"dev": "m"}},
            {"id": "pinned", "kind": "anthropic", "command": "claude", "pin_profile": "personal",
             "models": {"dev": "m"}}
        ]);
        v["claude_bin"] = serde_json::json!("claude");
        crate::config::config_from_value(&v).unwrap()
    }

    fn c3_readiness_with_env(
        cfg: &crate::config::Config,
        extra: &[(&str, &str)],
        departments: &str,
    ) -> (TeamReadiness, tempfile::TempDir) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("claude");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut env = BTreeMap::new();
        env.insert("PATH".to_string(), dir.path().to_str().unwrap().to_string());
        for (k, v) in extra {
            env.insert((*k).to_string(), (*v).to_string());
        }
        let statuses = crate::engine::readiness_with(cfg, &env, 0, &|_, _| None);
        (
            TeamReadiness {
                statuses,
                departments: crate::departments::load_from(departments, cfg),
                project_id: "p".to_string(),
                project_profile: "work".to_string(),
                level: crate::departments::Level::Vua,
            },
            dir,
        )
    }

    fn c3_readiness(
        cfg: &crate::config::Config,
        key_set: bool,
        departments: &str,
    ) -> (TeamReadiness, tempfile::TempDir) {
        let extra = if key_set {
            vec![("GW_TEST_KEY", "planted-secret-value")]
        } else {
            vec![]
        };
        c3_readiness_with_env(cfg, &extra, departments)
    }

    fn c3_teams() -> LoadedTeams {
        let text = r#"{
            "doi-gw": {"description": "Dev trên gw", "roles": {"dev": "gw"}},
            "doi-lap": {"description": "Dev trên engine không khai báo", "roles": {"dev": "khong-co"}},
            "doi-pin": {"description": "Dev trên engine personal", "roles": {"dev": "pinned"}}
        }"#;
        load_from(Some(text), &no_staff())
    }

    #[test]
    fn c3_team_with_engine_missing_key_is_not_ready() {
        let cfg = c3_cfg();
        let teams = c3_teams();
        let (r, _dir) = c3_readiness(&cfg, false, "{}");

        let why = r
            .refusal(&cfg, teams.get("doi-gw").unwrap(), "dev")
            .unwrap();
        assert!(why.contains("doi-gw") && why.contains("'gw'"), "{why}");
        assert!(
            why.contains("Chưa đặt biến môi trường GW_TEST_KEY."),
            "{why}"
        );
        assert!(!why.contains("planted"), "{why}");
        let why = r
            .refusal(&cfg, teams.get("doi-lap").unwrap(), "dev")
            .unwrap();
        assert!(why.contains("chưa khai báo trong config.json"), "{why}");
        // ENG-10 comes first: a personal-only engine on a work project.
        let why = r
            .refusal(&cfg, teams.get("doi-pin").unwrap(), "dev")
            .unwrap();
        assert!(why.contains("SPK-10"), "{why}");

        // Only the role of the stopped run counts: QA of every team runs on claude.
        // The third built-in team names an engine this engine list does not declare.
        assert_eq!(
            r.ready_names(&cfg, &teams, "dev"),
            vec!["mac-dinh", "phong-ban"]
        );
        assert!(r
            .ready_names(&cfg, &teams, "qa")
            .contains(&"doi-gw".to_string()));

        // With the key set the gateway team is offered.
        let (r, _dir) = c3_readiness(&cfg, true, "{}");
        assert!(r
            .ready_names(&cfg, &teams, "dev")
            .contains(&"doi-gw".to_string()));
        assert_eq!(team_role_of("analyst"), "lead");
        assert_eq!(team_role_of("analysis-review"), "plan-review");
    }

    #[test]
    fn c3_department_team_needs_a_ready_member() {
        let cfg = c3_cfg();
        let teams = c3_teams();
        let phong_ban = teams.get(TEAM_PHONG_BAN).unwrap();
        let only_gw = r#"{"thi-cong": {"members": {"m1": {"display_name": "M1", "engine": "gw", "model": "x", "levels": ["vua"]}}}}"#;
        let (r, _dir) = c3_readiness(&cfg, false, only_gw);
        assert_eq!(r.departments.members.len(), 1, "{:?}", r.departments.errors);
        let why = r.refusal(&cfg, phong_ban, "dev").unwrap();
        assert!(why.contains("không có thành viên phòng ban"), "{why}");
        // A role whose department has no member falls back to the team's own engine.
        assert_eq!(r.refusal(&cfg, phong_ban, "qa"), None);
        let (r, _dir) = c3_readiness(&cfg, true, only_gw);
        assert_eq!(r.refusal(&cfg, phong_ban, "dev"), None);
    }

    #[test]
    fn c3_unverified_engine_is_ready_and_marked() {
        let mut v: serde_json::Value =
            serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap();
        v["engines"] = serde_json::json!([
            {"id": "gw", "kind": "anthropic", "command": "claude",
             "base_url": "https://gw.invalid/anthropic", "key_env": "GW_TEST_KEY",
             "verified": "e09 ok", "models": {"dev": "m"}},
            {"id": "gw-chua", "kind": "anthropic", "command": "claude",
             "base_url": "https://gw-chua.invalid/anthropic", "key_env": "GW_TEST_KEY",
             "models": {"dev": "m"}},
            {"id": "pinned", "kind": "anthropic", "command": "claude", "pin_profile": "personal",
             "models": {"dev": "m"}}
        ]);
        v["claude_bin"] = serde_json::json!("claude");
        let cfg = crate::config::config_from_value(&v).unwrap();

        let teams_text = r#"{
            "doi-chua": {"description": "Dev trên gw-chua", "roles": {"dev": "gw-chua"}}
        }"#;
        let teams = load_from(Some(teams_text), &no_staff());
        let doi_chua = teams.get("doi-chua").unwrap();
        let mac_dinh = teams.get(DEFAULT_TEAM).unwrap();
        let _phong_ban = teams.get(TEAM_PHONG_BAN).unwrap();

        // Key set
        let only_gw_chua = r#"{"thi-cong": {"members": {"m1": {"display_name": "M1", "engine": "gw-chua", "model": "x", "levels": ["vua"]}}}}"#;
        let (r, _dir) = c3_readiness_with_env(
            &cfg,
            &[("GW_TEST_KEY", "planted-secret-value")],
            only_gw_chua,
        );

        let gw_st = r.statuses.iter().find(|s| s.name == "gw-chua").unwrap();
        assert_eq!(gw_st.readiness, crate::engine::Readiness::ChuaKiem.key());
        assert_eq!(r.refusal(&cfg, doi_chua, "dev"), None);
        assert!(r
            .ready_names(&cfg, &teams, "dev")
            .contains(&"doi-chua".to_string()));
        assert_eq!(
            r.unverified_engine(doi_chua, "dev"),
            Some("gw-chua".to_string())
        );
        assert_eq!(r.unverified_engine(doi_chua, "qa"), None);
        assert_eq!(r.unverified_engine(mac_dinh, "dev"), None);
        // phong-ban member readiness is wired in Task 2 (member_usable).

        // Key unset
        let (r_no_key, _dir) = c3_readiness_with_env(&cfg, &[], only_gw_chua);
        let why = r_no_key.refusal(&cfg, doi_chua, "dev").unwrap();
        assert!(why.contains("thiếu key"), "{why}");
        assert!(
            why.contains("Chưa đặt biến môi trường GW_TEST_KEY."),
            "{why}"
        );
        assert!(!r_no_key
            .ready_names(&cfg, &teams, "dev")
            .contains(&"doi-chua".to_string()));
    }

    #[test]
    fn c3_builtin_tiet_kiem_on_default_deepseek() {
        let v: serde_json::Value =
            serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap();
        let cfg = crate::config::config_from_value(&v).unwrap();
        let teams = load_from(None, &no_staff());
        let tiet_kiem = teams.get("tiet-kiem").unwrap();

        // With DEEPSEEK_API_KEY set:
        let (r, _dir) =
            c3_readiness_with_env(&cfg, &[("DEEPSEEK_API_KEY", "planted-key-val")], "{}");
        assert_eq!(
            r.ready_names(&cfg, &teams, "dev"),
            vec!["mac-dinh", "phong-ban", "tiet-kiem"]
        );
        assert_eq!(
            r.unverified_engine(tiet_kiem, "dev"),
            Some("deepseek".to_string())
        );

        // Without DEEPSEEK_API_KEY:
        let (r_no_key, _dir) = c3_readiness_with_env(&cfg, &[], "{}");
        assert_eq!(
            r_no_key.ready_names(&cfg, &teams, "dev"),
            vec!["mac-dinh", "phong-ban"]
        );
        let why = r_no_key.refusal(&cfg, tiet_kiem, "dev").unwrap();
        assert!(why.contains("DEEPSEEK_API_KEY"), "{why}");
        assert!(!why.contains("planted-key-val"), "{why}");
    }

    #[test]
    fn c3_quota_restore_target() {
        let switched = TaskTeam::parse(Some(
            r#"{"name":"doi-b","history":[{"at":1,"by":"telegram","from":"mac-dinh","to":"doi-b","reason":"quota","reset_at":1000}]}"#,
        ))
        .unwrap();
        assert_eq!(switched.quota_restore_target(999), None, "before reset_at");
        let entry = switched.quota_restore_target(1000).unwrap().clone();
        assert_eq!(entry.from, "mac-dinh");
        let back = switched.restored(&entry, "autopilot", 2000);
        assert_eq!(back.name, "mac-dinh");
        let last = back.history.last().unwrap();
        assert_eq!(last.reason.as_deref(), Some(CHANGE_REASON_QUOTA_RESTORE));
        assert_eq!(
            (last.from.as_str(), last.to.as_str()),
            ("doi-b", "mac-dinh")
        );
        // Restored once: the last entry is no longer a quota switch.
        assert_eq!(back.quota_restore_target(5000), None);

        // An ordinary change, a quota switch without reset time, or a team changed since: nothing.
        for raw in [
            r#"{"name":"doi-b","history":[{"at":1,"by":"dashboard","from":"mac-dinh","to":"doi-b"}]}"#,
            r#"{"name":"doi-b","history":[{"at":1,"by":"telegram","from":"mac-dinh","to":"doi-b","reason":"quota"}]}"#,
            r#"{"name":"doi-c","history":[{"at":1,"by":"telegram","from":"mac-dinh","to":"doi-b","reason":"quota","reset_at":1}]}"#,
        ] {
            assert_eq!(
                TaskTeam::parse(Some(raw))
                    .unwrap()
                    .quota_restore_target(9999),
                None,
                "{raw}"
            );
        }
        // Old documents without the new keys still parse and serialize without them.
        let old = TaskTeam::parse(Some(
            r#"{"name":"doi-b","history":[{"at":1,"by":"cli","from":"mac-dinh","to":"doi-b"}]}"#,
        ))
        .unwrap();
        assert!(!old.to_json().contains("reason"), "{}", old.to_json());
    }
}
