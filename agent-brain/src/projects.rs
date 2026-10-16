use crate::config::Config;
use crate::error::{BrainError, BrainResult};
use std::path::{Path, PathBuf};

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

        Some(Project {
            id,
            path,
            name_vi,
            aliases,
            kind,
            profile,
            repo,
            missing,
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
                    lines.push(format!("  {}\t{}\t{}", c.id, c.name_vi, c.path));
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

pub fn cmd_scan(_roots: &[String]) -> BrainResult<i32> {
    Err(BrainError::not_available("projects scan"))
}

pub fn cmd_add(_path: &str, _kind: Option<&str>) -> BrainResult<i32> {
    Err(BrainError::not_available("projects add"))
}
