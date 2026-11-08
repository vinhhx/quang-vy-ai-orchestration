//! Phòng ban, thành viên và chọn người theo độ khó (departments.json).
//!
//! Người dùng tổ chức thành 5 phòng ban cố định, mỗi phòng có nhiều thành viên.
//! Mỗi thành viên là một engine kèm model do người dùng cấu hình và độ khó hỗ trợ (de, vua, kho).

use crate::config::Config;
use crate::engine::{profile_refusal, readiness, Engine};
use crate::error::BrainResult;
use crate::staff::validate_id;
use crate::teams::FieldError;
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MAX_FILE_BYTES: u64 = 262_144;
pub const MAX_NAME_CHARS: usize = 64;
pub const MAX_MODEL_CHARS: usize = 64;
pub const MAX_NOTES_CHARS: usize = 500;
pub const MAX_RULES_CHARS: usize = 1000;
pub const MAX_DEPT_SKILL_FILE_BYTES: u64 = 64 * 1024;
pub const MAX_DEPT_SKILL_TOTAL_BYTES: usize = 24 * 1024;

pub const DEPARTMENTS: &[(&str, &str)] = &[
    ("phan-tich", "Phòng phân tích"),
    ("ke-hoach", "Phòng kế hoạch"),
    ("thi-cong", "Phòng thi công"),
    ("qa", "Phòng QA"),
    ("bao-mat", "Phòng security"),
];

pub fn dept_name_of(id: &str) -> Option<&'static str> {
    DEPARTMENTS
        .iter()
        .find(|(d_id, _)| *d_id == id)
        .map(|(_, name)| *name)
}

pub fn dept_of_role(role: &str) -> Option<&'static str> {
    match role {
        "analyst" | "analysis-review" => Some("phan-tich"),
        "lead" | "plan-review" => Some("ke-hoach"),
        "dev" => Some("thi-cong"),
        "qa" | "code-review" => Some("qa"),
        _ => None,
    }
}

pub fn dept_roles(dept_id: &str) -> &'static [&'static str] {
    match dept_id {
        "phan-tich" => &["analyst", "analysis-review"],
        "ke-hoach" => &["lead", "plan-review"],
        "thi-cong" => &["dev"],
        "qa" => &["qa", "code-review"],
        _ => &[],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    De,
    Vua,
    Kho,
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::De => "de",
            Level::Vua => "vua",
            Level::Kho => "kho",
        }
    }

    pub fn display_vi(&self) -> &'static str {
        match self {
            Level::De => "Dễ",
            Level::Vua => "Vừa",
            Level::Kho => "Khó",
        }
    }

    pub fn parse(s: &str) -> Option<Level> {
        match s.trim().to_lowercase().as_str() {
            "de" => Some(Level::De),
            "vua" => Some(Level::Vua),
            "kho" => Some(Level::Kho),
            _ => None,
        }
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub id: String,
    pub department: String,
    pub display_name: String,
    pub engine: Engine,
    pub model: Option<String>,
    pub levels: Vec<Level>,
    pub notes: Option<String>,
    pub rules: Option<String>,
}

impl serde::Serialize for Member {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Member", 8)?;
        s.serialize_field("id", &self.id)?;
        s.serialize_field("department", &self.department)?;
        s.serialize_field("display_name", &self.display_name)?;
        s.serialize_field("engine", &self.engine.to_config_string())?;
        s.serialize_field("model", &self.model)?;
        s.serialize_field("levels", &self.levels)?;
        s.serialize_field("notes", &self.notes)?;
        s.serialize_field("rules", &self.rules)?;
        s.end()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoadedDepartments {
    pub members: Vec<Member>,
    pub errors: Vec<String>,
}

impl LoadedDepartments {
    pub fn get(&self, id: &str) -> Option<&Member> {
        self.members.iter().find(|m| m.id == id)
    }

    pub fn members_of(&self, dept: &str) -> Vec<&Member> {
        self.members.iter().filter(|m| m.department == dept).collect()
    }

    pub fn has_members(&self) -> bool {
        !self.members.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    pub member: Member,
    pub reason: String,
}

pub fn departments_path() -> PathBuf {
    if let Ok(dir) = std::env::var("AGENT_BRAIN_CONFIG_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join("departments.json");
        }
    }
    crate::config::config_dir().join("departments.json")
}

fn field_error(field: &str, message: impl Into<String>) -> FieldError {
    FieldError {
        field: field.to_string(),
        message: message.into(),
    }
}

fn is_valid_model_str(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= MAX_MODEL_CHARS
        && model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' || c == ':' || c == '/')
}

/// Validate một thành viên trong phòng ban. Tách riêng để tái sử dụng ở dashboard.
pub fn validate_entry(
    dept_id: &str,
    member_id: &str,
    value: &serde_json::Value,
    cfg: &Config,
) -> Result<Member, Vec<FieldError>> {
    let mut errors = Vec::new();

    if dept_name_of(dept_id).is_none() {
        errors.push(field_error(
            "department",
            format!("Phòng ban không hợp lệ: {dept_id}"),
        ));
    }

    if !validate_id(member_id) {
        errors.push(field_error(
            "id",
            "Mã chỉ gồm chữ thường không dấu, số và dấu gạch ngang (tối đa 32 ký tự)",
        ));
    }

    let obj = match value.as_object() {
        Some(o) => o,
        None => {
            errors.push(field_error("entry", "Giá trị của thành viên phải là một object"));
            return Err(errors);
        }
    };

    // display_name
    let display_name = match obj.get("display_name").and_then(|v| v.as_str()) {
        Some(name) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                errors.push(field_error("display_name", "Tên hiển thị không được để trống"));
                String::new()
            } else if trimmed.chars().count() > MAX_NAME_CHARS {
                errors.push(field_error(
                    "display_name",
                    format!("Tên hiển thị quá dài (tối đa {MAX_NAME_CHARS} ký tự)"),
                ));
                trimmed.to_string()
            } else {
                trimmed.to_string()
            }
        }
        None => {
            errors.push(field_error("display_name", "Thiếu trường display_name"));
            String::new()
        }
    };

    // engine
    let engine = match obj.get("engine").and_then(|v| v.as_str()) {
        Some(eng_str) => {
            let eng_trimmed = eng_str.trim();
            if eng_trimmed.starts_with("người:") || eng_trimmed == "người" {
                errors.push(field_error(
                    "engine",
                    "Thành viên phòng ban không thể là người thật",
                ));
                Engine::Claude
            } else if eng_trimmed == "claude" {
                Engine::Claude
            } else if let Some(def) = cfg.engines.get(eng_trimmed) {
                if def.kind() == crate::config::EngineKind::Dsh {
                    errors.push(field_error(
                        "engine",
                        "Engine dsh không được hỗ trợ cho phòng ban",
                    ));
                }
                Engine::Named(eng_trimmed.to_string())
            } else {
                errors.push(field_error(
                    "engine",
                    format!("Engine không tồn tại trong cấu hình: {eng_trimmed}"),
                ));
                Engine::Named(eng_trimmed.to_string())
            }
        }
        None => {
            errors.push(field_error("engine", "Thiếu trường engine"));
            Engine::Claude
        }
    };

    // model
    let model = match obj.get("model") {
        Some(v) => match v.as_str() {
            Some(m) => {
                let trimmed = m.trim();
                if trimmed.is_empty() {
                    None
                } else if !is_valid_model_str(trimmed) {
                    errors.push(field_error(
                        "model",
                        format!("Model '{trimmed}' không hợp lệ (tối đa {MAX_MODEL_CHARS} ký tự gồm a-z, 0-9, .-_:/)"),
                    ));
                    Some(trimmed.to_string())
                } else {
                    Some(trimmed.to_string())
                }
            }
            None => {
                errors.push(field_error("model", "Trường model phải là chuỗi"));
                None
            }
        },
        None => None,
    };

    // levels
    let mut parsed_levels = Vec::new();
    match obj.get("levels") {
        Some(v) => match v.as_array() {
            Some(arr) => {
                if arr.is_empty() {
                    errors.push(field_error("levels", "Mảng levels không được để rỗng"));
                } else {
                    for item in arr {
                        match item.as_str().and_then(Level::parse) {
                            Some(lvl) => {
                                if parsed_levels.contains(&lvl) {
                                    errors.push(field_error(
                                        "levels",
                                        format!("Trùng mức độ khó: {}", lvl.as_str()),
                                    ));
                                } else {
                                    parsed_levels.push(lvl);
                                }
                            }
                            None => {
                                errors.push(field_error(
                                    "levels",
                                    format!("Mức độ khó không hợp lệ: {item:?} (chỉ nhận 'de', 'vua', 'kho')"),
                                ));
                            }
                        }
                    }
                }
            }
            None => {
                errors.push(field_error("levels", "Trường levels phải là một mảng"));
            }
        },
        None => {
            errors.push(field_error("levels", "Thiếu trường levels"));
        }
    };

    // notes
    let notes = match obj.get("notes") {
        Some(v) => match v.as_str() {
            Some(n) => {
                let trimmed = n.trim();
                if trimmed.is_empty() {
                    None
                } else if trimmed.chars().count() > MAX_NOTES_CHARS {
                    errors.push(field_error(
                        "notes",
                        format!("Ghi chú quá dài (tối đa {MAX_NOTES_CHARS} ký tự)"),
                    ));
                    Some(trimmed.to_string())
                } else {
                    Some(trimmed.to_string())
                }
            }
            None => {
                errors.push(field_error("notes", "Trường notes phải là chuỗi"));
                None
            }
        },
        None => None,
    };

    // rules
    let rules = match obj.get("rules") {
        Some(v) => match v.as_str() {
            Some(n) => {
                let trimmed = n.trim();
                if trimmed.is_empty() {
                    None
                } else if trimmed.chars().count() > MAX_RULES_CHARS {
                    errors.push(field_error(
                        "rules",
                        format!("Rules riêng quá dài (tối đa {MAX_RULES_CHARS} ký tự)"),
                    ));
                    Some(trimmed.to_string())
                } else {
                    Some(trimmed.to_string())
                }
            }
            None => {
                errors.push(field_error("rules", "Trường rules phải là chuỗi"));
                None
            }
        },
        None => None,
    };

    if errors.is_empty() {
        Ok(Member {
            id: member_id.to_string(),
            department: dept_id.to_string(),
            display_name,
            engine,
            model,
            levels: parsed_levels,
            notes,
            rules,
        })
    } else {
        Err(errors)
    }
}

pub fn load_from(text: &str, cfg: &Config) -> LoadedDepartments {
    let mut members = Vec::new();
    let mut errors = Vec::new();

    let root: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            return LoadedDepartments {
                members: Vec::new(),
                errors: vec![format!("Lỗi cú pháp JSON: {e}")],
            };
        }
    };

    let obj = match root.as_object() {
        Some(o) => o,
        None => {
            return LoadedDepartments {
                members: Vec::new(),
                errors: vec!["File departments.json phải là một JSON object".to_string()],
            };
        }
    };

    let mut seen_ids = BTreeMap::new();

    for (k, v) in obj {
        if dept_name_of(k).is_none() {
            errors.push(format!("Khóa phòng ban không hợp lệ: {k}"));
            continue;
        }

        let dept_obj = match v.as_object() {
            Some(o) => o,
            None => {
                errors.push(format!("Phòng ban '{k}' phải là một object"));
                continue;
            }
        };

        let members_val = match dept_obj.get("members") {
            Some(m) => m,
            None => continue, // Phòng không khai báo members -> hợp lệ, phòng rỗng
        };

        let members_map = match members_val.as_object() {
            Some(m) => m,
            None => {
                errors.push(format!("Trường 'members' của phòng '{k}' phải là một object"));
                continue;
            }
        };

        for (m_id, m_val) in members_map {
            if let Some(prev_dept) = seen_ids.get(m_id) {
                errors.push(format!(
                    "Mã thành viên bị trùng: '{m_id}' (đã có ở phòng '{prev_dept}')"
                ));
                continue;
            }

            match validate_entry(k, m_id, m_val, cfg) {
                Ok(member) => {
                    seen_ids.insert(m_id.clone(), k.clone());
                    members.push(member);
                }
                Err(errs) => {
                    let msg = errs
                        .into_iter()
                        .map(|e| format!("{}: {}", e.field, e.message))
                        .collect::<Vec<_>>()
                        .join(", ");
                    errors.push(format!("Thành viên '{m_id}' (phòng {k}): {msg}"));
                }
            }
        }
    }

    LoadedDepartments { members, errors }
}

pub fn load_path(path: &Path, cfg: &Config) -> LoadedDepartments {
    if !path.exists() {
        return LoadedDepartments::default();
    }

    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            return LoadedDepartments {
                members: Vec::new(),
                errors: vec![format!("Không thể đọc thông tin file {}: {e}", path.display())],
            };
        }
    };

    if meta.len() > MAX_FILE_BYTES {
        return LoadedDepartments {
            members: Vec::new(),
            errors: vec![format!(
                "File {} vượt quá giới hạn {} bytes",
                path.display(),
                MAX_FILE_BYTES
            )],
        };
    }

    match std::fs::read_to_string(path) {
        Ok(text) => load_from(&text, cfg),
        Err(e) => LoadedDepartments {
            members: Vec::new(),
            errors: vec![format!("Không thể đọc file {}: {e}", path.display())],
        },
    }
}

pub fn load() -> LoadedDepartments {
    match crate::config::load_config() {
        Ok(cfg) => load_path(&departments_path(), &cfg),
        Err(e) => LoadedDepartments {
            members: Vec::new(),
            errors: vec![format!("Không thể đọc cấu hình: {}", e.message)],
        },
    }
}

/// Họ model: viết thường, bỏ đuôi `-low` / `-medium` / `-high`.
/// Ví dụ: `claude-opus-5-5-high` và `claude-opus-5-5` cùng họ.
pub fn model_family(model: &str) -> String {
    let lower = model.to_lowercase();
    for suffix in &["-low", "-medium", "-high"] {
        if let Some(stripped) = lower.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    lower
}

/// Chọn thành viên phòng ban cho một vai trò của task.
pub fn pick(
    conn: &Connection,
    cfg: &Config,
    task_id: i64,
    role: &str,
    level: Level,
    project_profile: &str,
) -> BrainResult<Option<Pick>> {
    let loaded = load_path(&departments_path(), cfg);
    pick_from(conn, cfg, &loaded, task_id, role, level, project_profile)
}

/// A member's engine is `san_sang` in `statuses` and `profile_refusal` does not refuse it for
/// `role` on a project of `project_profile` (people are never picked from a department).
pub fn member_usable(
    cfg: &Config,
    statuses: &[crate::engine::EngineStatus],
    member: &Member,
    role: &str,
    project_profile: &str,
) -> bool {
    let name = member.engine.as_db_str();
    let ready = statuses
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.readiness == crate::engine::Readiness::SanSang.key())
        .unwrap_or(false);
    let not_refused = match &member.engine {
        Engine::Claude => true,
        Engine::Named(id) => match cfg.engines.get(id) {
            Some(def) => profile_refusal(def, "", project_profile, role).is_none(),
            None => false,
        },
        Engine::Human(_) => false,
    };
    ready && not_refused
}

/// The members `pick_from` chooses among: those of `dept_members` with `level` (every member when
/// none has it) whose engine is usable (`member_usable`).
pub fn ready_candidates<'a>(
    cfg: &Config,
    statuses: &[crate::engine::EngineStatus],
    dept_members: &[&'a Member],
    role: &str,
    level: Level,
    project_profile: &str,
) -> Vec<&'a Member> {
    let mut candidates: Vec<&Member> = dept_members
        .iter()
        .copied()
        .filter(|m| m.levels.contains(&level))
        .collect();
    if candidates.is_empty() {
        candidates = dept_members.to_vec();
    }
    candidates.retain(|m| member_usable(cfg, statuses, m, role, project_profile));
    candidates
}

/// Chọn thành viên phòng ban từ LoadedDepartments đã nạp sẵn.
pub fn pick_from(
    conn: &Connection,
    cfg: &Config,
    loaded: &LoadedDepartments,
    task_id: i64,
    role: &str,
    level: Level,
    project_profile: &str,
) -> BrainResult<Option<Pick>> {
    let dept_id = match dept_of_role(role) {
        Some(d) => d,
        None => return Ok(None),
    };

    let dept_name = dept_name_of(dept_id).unwrap_or(dept_id);
    let dept_members = loaded.members_of(dept_id);
    if dept_members.is_empty() {
        return Ok(None);
    }

    let env = crate::profile::current_env();
    let statuses = readiness(cfg, &env, 0);
    let usable = |member: &Member| member_usable(cfg, &statuses, member, role, project_profile);

    // 2. Giữ người cũ: task đã có run cùng vai với member_id còn trong file,
    // engine sẵn sàng và có mức độ khó tương ứng -> chọn lại thành viên đó.
    if let Ok(Some(prev_member_id)) = crate::db::last_member_for(conn, task_id, role) {
        if let Some(prev) = dept_members.iter().find(|m| m.id == prev_member_id) {
            if prev.levels.contains(&level) && usable(prev) {
                return Ok(Some(Pick {
                    member: (*prev).clone(),
                    reason: format!("{} · giữ người cũ", dept_name.to_lowercase()),
                }));
            }
        }
    }

    // 3. Ứng viên là thành viên của phòng có level của task (không có ai thì mọi thành viên
    // của phòng), bỏ người có engine không sẵn sàng hoặc bị profile_refusal từ chối.
    let mut candidates =
        ready_candidates(cfg, &statuses, &dept_members, role, level, project_profile);

    if candidates.is_empty() {
        return Ok(None);
    }

    // 4. Độc lập khi review:
    // - plan-review -> so với thành viên của run lead thành công gần nhất của task.
    // - code-review hoặc qa -> so với run dev gần nhất.
    // - analysis-review -> so với run analyst gần nhất.
    let target_role_opt = match role {
        "plan-review" => Some(("lead", true)),
        "code-review" | "qa" => Some(("dev", false)),
        "analysis-review" => Some(("analyst", false)),
        _ => None,
    };

    if let Some((target_role, only_succeeded)) = target_role_opt {
        let query = if only_succeeded {
            "SELECT model FROM runs WHERE task_id = ?1 AND role = ?2 AND status = 'succeeded' ORDER BY id DESC LIMIT 1"
        } else {
            "SELECT model FROM runs WHERE task_id = ?1 AND role = ?2 ORDER BY id DESC LIMIT 1"
        };
        let target_model: Option<String> = conn
            .query_row(query, rusqlite::params![task_id, target_role], |r| r.get(0))
            .ok()
            .flatten();

        if let Some(ref t_mod) = target_model {
            let target_family = model_family(t_mod);
            let diff_candidates: Vec<&Member> = candidates
                .iter()
                .copied()
                .filter(|m| {
                    if let Some(ref m_mod) = m.model {
                        model_family(m_mod) != target_family
                    } else {
                        // Nếu member không ghi model rõ, kiểm tra engine name xem có trùng họ không
                        let eng_str = m.engine.as_db_str();
                        model_family(eng_str) != target_family
                    }
                })
                .collect();

            if !diff_candidates.is_empty() {
                candidates = diff_candidates;
            }
        }
    }

    // 5. Xếp hạng theo thứ tự:
    // 1) Ít run running nhất (theo runs.member_id)
    // 2) Điểm trung bình cao nhất. Thành viên có dưới 5 điểm tính là 3.0.
    // 3) Thứ tự trong file (dept_members order)
    struct CandidateRank<'a> {
        member: &'a Member,
        running_count: i64,
        avg_score: f64,
        original_idx: usize,
    }

    let mut ranked = Vec::with_capacity(candidates.len());
    for (idx, m) in candidates.into_iter().enumerate() {
        let running_count = crate::db::running_count_by_member(conn, &m.id).unwrap_or(0);
        let (avg, count) = crate::db::member_score_avg(conn, &m.id).unwrap_or((0.0, 0));
        let effective_score = if count < 5 { 3.0 } else { avg };
        ranked.push(CandidateRank {
            member: m,
            running_count,
            avg_score: effective_score,
            original_idx: idx,
        });
    }

    ranked.sort_by(|a, b| {
        a.running_count
            .cmp(&b.running_count)
            .then_with(|| {
                b.avg_score
                    .partial_cmp(&a.avg_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.original_idx.cmp(&b.original_idx))
    });

    let best = ranked.first().map(|r| r.member).unwrap();
    let reason = format!(
        "{} · mức {} · ít việc nhất",
        dept_name.to_lowercase(),
        level.display_vi().to_lowercase()
    );

    Ok(Some(Pick {
        member: (*best).clone(),
        reason,
    }))
}

pub fn cmd_list(ctx: &crate::app::Ctx) -> BrainResult<i32> {
    let p = departments_path();
    let loaded = load_path(&p, &ctx.cfg);
    let env = crate::profile::current_env();
    let statuses = crate::engine::readiness(&ctx.cfg, &env, 0);

    for (dept_id, dept_name) in DEPARTMENTS {
        println!("{dept_name} ({dept_id}):");
        let members = loaded.members_of(dept_id);
        if members.is_empty() {
            println!("  (chưa có thành viên)");
        } else {
            for m in members {
                let levels_str = m
                    .levels
                    .iter()
                    .map(|l| l.display_vi())
                    .collect::<Vec<_>>()
                    .join(", ");
                let engine_name = m.engine.as_db_str();
                let status_word = statuses
                    .iter()
                    .find(|s| s.name == engine_name)
                    .map(|s| s.word)
                    .unwrap_or("chưa rõ");

                let (avg, count) = crate::db::member_score_avg(&ctx.conn, &m.id).unwrap_or((0.0, 0));
                let score_str = if count == 0 {
                    "- (0 lượt)".to_string()
                } else {
                    format!("{:.1} ({} lượt)", avg, count)
                };

                let run_count: i64 = ctx
                    .conn
                    .query_row(
                        "SELECT COUNT(*) FROM runs WHERE member_id = ?1",
                        rusqlite::params![m.id],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);

                println!(
                    "  - {} | {} | {} | model: {} | mức: {} | {} | điểm: {} | {} run",
                    m.id,
                    m.display_name,
                    engine_name,
                    m.model.as_deref().unwrap_or("-"),
                    levels_str,
                    status_word,
                    score_str,
                    run_count
                );
            }
        }
        println!();
    }

    if !loaded.errors.is_empty() {
        println!("Lỗi đọc file {}:", p.display());
        for err in &loaded.errors {
            println!("  ! {err}");
        }
    }

    Ok(0)
}

pub fn cmd_check(ctx: &crate::app::Ctx) -> BrainResult<i32> {
    let p = departments_path();
    if !p.exists() {
        println!("Chưa có file cấu hình phòng ban: {}", p.display());
        println!("Để tạo cấu hình mẫu, hãy sao chép từ assets:");
        if let Some(parent) = p.parent() {
            println!("  mkdir -p {}", parent.display());
        }
        println!("  cp <agent-brain>/assets/departments.example.json {}", p.display());
        return Ok(0);
    }

    let loaded = load_path(&p, &ctx.cfg);
    if !loaded.errors.is_empty() {
        eprintln!("File {} có lỗi:", p.display());
        for err in &loaded.errors {
            eprintln!("  ! {err}");
        }
        return Ok(1);
    }

    println!("File {} hợp lệ ({} thành viên).", p.display(), loaded.members.len());
    Ok(0)
}

pub fn cmd_pick(ctx: &crate::app::Ctx, task_id: i64, role: &str) -> BrainResult<i32> {
    let level_str = crate::db::task_level(&ctx.conn, task_id)?;
    let level = Level::parse(&level_str).unwrap_or(Level::Vua);
    let project_profile: String = ctx
        .conn
        .query_row(
            "SELECT p.profile FROM tasks t JOIN projects p ON p.id = t.project_id WHERE t.id = ?1",
            rusqlite::params![task_id],
            |r| r.get(0),
        )
        .map_err(|e| crate::error::BrainError::error(format!("Không tìm thấy task {task_id}: {e}")))?;

    match pick(&ctx.conn, &ctx.cfg, task_id, role, level, &project_profile)? {
        Some(pick) => {
            println!("Đã chọn: {} ({})", pick.member.display_name, pick.member.id);
            println!(
                "  Phòng ban: {}",
                dept_name_of(&pick.member.department).unwrap_or(&pick.member.department)
            );
            println!("  Lý do: {}", pick.reason);
            println!("  Engine: {}", pick.member.engine.as_db_str());
            println!("  Model: {}", pick.member.model.as_deref().unwrap_or("-"));
        }
        None => {
            println!(
                "Không có thành viên phòng ban phù hợp cho vai trò '{role}' (task mức {}) — sẽ dùng engine mặc định của đội hình.",
                level.display_vi()
            );
        }
    }

    Ok(0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeptSkill {
    pub name: String,
    pub description: String,
    pub body: String,
    pub path: PathBuf,
}

pub fn dept_skills_dir(dept: &str) -> PathBuf {
    if let Ok(dir) = std::env::var("AGENT_BRAIN_CONFIG_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join("departments").join(dept).join("skills");
        }
    }
    crate::config::config_dir().join("departments").join(dept).join("skills")
}

pub fn dept_skills(dept: &str) -> Vec<DeptSkill> {
    let dir = dept_skills_dir(dept);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut list = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let skill_file = entry.path().join("SKILL.md");
        let Ok(meta) = std::fs::symlink_metadata(&skill_file) else {
            continue;
        };
        if !meta.file_type().is_file() || meta.len() > MAX_DEPT_SKILL_FILE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&skill_file) else {
            continue;
        };
        let (fm_name, fm_desc) = crate::skills::front_matter(&text);
        let folder_name = entry.file_name().to_string_lossy().to_string();
        let name = fm_name.filter(|n| !n.trim().is_empty()).unwrap_or(folder_name);
        let description = fm_desc.unwrap_or_else(|| crate::skills::first_line(&text));

        list.push(DeptSkill {
            name,
            description,
            body: text,
            path: skill_file,
        });
    }

    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

pub fn format_dept_skills(dept: &str) -> (String, Vec<String>) {
    let skills = dept_skills(dept);
    if skills.is_empty() {
        return (String::new(), Vec::new());
    }

    let dept_name = dept_name_of(dept).unwrap_or(dept);
    let mut out = format!("## Skill của phòng {dept_name} (playbook dùng chung — bổ sung, không mở rộng quyền)\n");
    let mut warnings = Vec::new();
    let mut current_bytes = out.len();
    let mut budget_exceeded = false;

    for skill in &skills {
        let block = format!("### Skill: {}\n{}\n", skill.name, skill.body.trim());
        if !budget_exceeded && current_bytes + block.len() <= MAX_DEPT_SKILL_TOTAL_BYTES {
            out.push_str(&block);
            current_bytes += block.len();
        } else {
            if !budget_exceeded {
                budget_exceeded = true;
            }
            let summary = if skill.description.is_empty() {
                format!("- {}\n", skill.name)
            } else {
                format!("- {}: {}\n", skill.name, skill.description)
            };
            out.push_str(&summary);
            warnings.push(format!(
                "Skill '{}' của phòng {} vượt quá giới hạn 24 KiB nên chỉ đưa tên và mô tả",
                skill.name, dept_name
            ));
        }
    }

    (out, warnings)
}

pub fn prompt_append_for(role: &str, member_id: Option<&str>) -> (String, Vec<String>) {
    let Some(dept_id) = dept_of_role(role) else {
        return (String::new(), Vec::new());
    };

    let (mut out, warnings) = format_dept_skills(dept_id);

    if let Some(m_id) = member_id {
        let loaded = load();
        if let Some(m) = loaded.get(m_id) {
            if let Some(ref rules) = m.rules {
                let trimmed = rules.trim();
                if !trimmed.is_empty() {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&format!(
                        "## Rules riêng của thành viên {}\n{}\n",
                        m.display_name, trimmed
                    ));
                }
            }
        }
    }

    (out, warnings)
}

pub fn prompt_append_for_member(role: &str, member: Option<&Member>) -> (String, Vec<String>) {
    let Some(dept_id) = dept_of_role(role) else {
        return (String::new(), Vec::new());
    };

    let (mut out, warnings) = format_dept_skills(dept_id);

    if let Some(m) = member {
        if let Some(ref rules) = m.rules {
            let trimmed = rules.trim();
            if !trimmed.is_empty() {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&format!(
                    "## Rules riêng của thành viên {}\n{}\n",
                    m.display_name, trimmed
                ));
            }
        }
    }

    (out, warnings)
}
