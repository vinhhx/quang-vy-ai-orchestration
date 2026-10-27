//! Đội hình (TEAM-01): `~/.config/agent-brain/teams.json` cộng hai đội hình có sẵn.
//! Mỗi đội hình gán một engine cho năm vai trò; QA luôn là claude (D-06),
//! engine `người:<mã>` chỉ dành cho dev (D-24). Đội hình và quy trình là hai danh sách riêng (D-11).

use crate::engine::Engine;
use crate::error::{BrainError, BrainResult};
use crate::staff::LoadedStaff;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const BUILTIN_TEAMS_JSON: &str = include_str!("../assets/teams.default.json");
pub const DEFAULT_TEAM: &str = "mac-dinh";
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

        // claude, deepseek, agy, then people in staff-list order, unknown ids last in id order.
        groups.sort_by_key(|(engine, _)| match engine {
            Engine::Claude => (0usize, 0usize, String::new()),
            Engine::Deepseek => (1, 0, String::new()),
            Engine::Agy => (2, 0, String::new()),
            Engine::Human(id) => match staff.staff.iter().position(|s| s.id == *id) {
                Some(idx) => (3, idx, String::new()),
                None => (4, 0, id.clone()),
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
                                "Engine không hợp lệ cho {role}: {shown} (dùng claude, deepseek, agy hoặc người:<mã>)"
                            ),
                        ));
                        continue;
                    }
                };
                if role == "qa" && engine != Engine::Claude {
                    errors.push(field_error(role, "QA phải dùng engine claude."));
                    continue;
                }
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
            Ok(team) => match existing {
                Some(pos) => teams[pos] = team,
                None => teams.push(team),
            },
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
    if !path.exists() {
        return load_from(None, staff);
    }
    match std::fs::metadata(path) {
        Ok(m) if m.len() > MAX_FILE_BYTES => {
            let mut l = load_from(None, staff);
            l.errors
                .push("teams.json lớn quá 256 KiB — bỏ qua file.".to_string());
            return l;
        }
        Ok(_) => {}
        Err(e) => {
            let mut l = load_from(None, staff);
            l.errors.push(format!("Không đọc được teams.json: {e}"));
            return l;
        }
    }
    match std::fs::read_to_string(path) {
        Ok(text) => load_from(Some(&text), staff),
        Err(e) => {
            let mut l = load_from(None, staff);
            l.errors.push(format!("Không đọc được teams.json: {e}"));
            l
        }
    }
}

/// Loads staff first, then teams with that staff.
pub fn load() -> (LoadedTeams, LoadedStaff) {
    let staff = crate::staff::load();
    let teams = load_path(&teams_path(), &staff);
    (teams, staff)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTeam {
    pub team: Team,
    pub note: Option<String>,
}

fn default_team(loaded: &LoadedTeams) -> BrainResult<&Team> {
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
    /// dashboard | telegram | cli
    pub by: String,
    pub from: String,
    pub to: String,
}

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
        assert_eq!(names(&l), ["mac-dinh", "tiet-kiem"]);
        assert!(l.teams.iter().all(|t| t.source == "builtin"));

        let md = l.get("mac-dinh").unwrap();
        for role in ROLES {
            assert_eq!(md.engine_for(role), Engine::Claude);
        }
        let tk = l.get("tiet-kiem").unwrap();
        assert_eq!(tk.engine_for("dev"), Engine::Deepseek);
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
        assert_eq!(names(&l), ["mac-dinh", "tiet-kiem", "doi-a"]);
        assert_eq!(l.get("mac-dinh").unwrap().source, "override");
        assert_eq!(l.get("mac-dinh").unwrap().description, "Ghi đè");
        assert_eq!(l.get("tiet-kiem").unwrap().source, "builtin");
        let a = l.get("doi-a").unwrap();
        assert_eq!(a.source, "file");
        assert_eq!(a.engine_for("lead"), Engine::Claude);
        assert_eq!(a.engine_for("plan-review"), Engine::Agy);
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
            ["mac-dinh", "tiet-kiem"],
            "bad entry must not load"
        );
        l.errors
    }

    #[test]
    fn entry_errors_keep_others() {
        // A valid entry after an invalid one still loads.
        let text = r#"{
"doi-x": {"description": "Sai", "roles": {"qa": "deepseek"}},
"doi-ok": {"description": "Tốt", "roles": {}}
}"#;
        let l = load_from(Some(text), &no_staff());
        assert_eq!(names(&l), ["mac-dinh", "tiet-kiem", "doi-ok"]);
        assert_eq!(
            l.errors,
            ["Dòng 2: đội hình 'doi-x': QA phải dùng engine claude."]
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

        let e = one_error(r#""doi-x": {"description": "d", "roles": {"dev": "gpt"}}"#);
        assert_eq!(
            e,
            ["Dòng 2: đội hình 'doi-x': Engine không hợp lệ cho dev: gpt (dùng claude, deepseek, agy hoặc người:<mã>)"]
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
        assert_eq!(names(&l), ["mac-dinh", "tiet-kiem"]);

        let big = " ".repeat(262_145);
        let l = load_from(Some(&big), &no_staff());
        assert_eq!(l.errors, ["teams.json lớn quá 256 KiB — bỏ qua file."]);
        assert_eq!(names(&l), ["mac-dinh", "tiet-kiem"]);

        let l = load_from(Some("[]"), &no_staff());
        assert_eq!(
            l.errors,
            ["Dòng 1: teams.json phải là một object, mỗi khóa là tên một đội hình."]
        );
        assert_eq!(names(&l), ["mac-dinh", "tiet-kiem"]);
    }

    #[test]
    fn load_path_missing_and_oversize() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("teams.json");
        let l = load_path(&p, &no_staff());
        assert!(l.errors.is_empty());
        assert_eq!(names(&l), ["mac-dinh", "tiet-kiem"]);

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
            "Không có đội hình 'khong-co'. Đội hình có: mac-dinh, tiet-kiem, doi-a"
        );

        let r = resolve(&l, None, Some("khong-co")).unwrap();
        assert_eq!(r.team.name, "mac-dinh");
        assert_eq!(
            r.note.as_deref(),
            Some("Đội hình 'khong-co' của dự án không có trong teams.json — dùng mac-dinh.")
        );

        let r = resolve(&l, None, Some(" doi-a ")).unwrap();
        assert_eq!(r.team.name, "doi-a");
        assert_eq!(r.note, None);

        let r = resolve(&l, None, Some("  ")).unwrap();
        assert_eq!(r.team.name, "mac-dinh");
        assert_eq!(r.note, None);

        let r = resolve(&l, None, None).unwrap();
        assert_eq!(r.team.name, "mac-dinh");
        assert_eq!(r.note, None);

        // An explicit request wins over the project default.
        let r = resolve(&l, Some("tiet-kiem"), Some("doi-a")).unwrap();
        assert_eq!(r.team.name, "tiet-kiem");
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
}
