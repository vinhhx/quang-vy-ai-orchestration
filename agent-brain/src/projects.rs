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

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub added: usize,
    pub missing: usize,
    pub restored: usize,
    pub unchanged: usize,
    pub bad_roots: Vec<String>,
    pub suffixed: Vec<(String, String, String)>,
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

    ScanReport {
        added: added_count,
        missing: total_missing,
        restored: restored_count,
        unchanged: unchanged_count,
        bad_roots,
        suffixed: suffixed_ids,
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

    let entry = new_entry(data, &abs_path, actual_kind);
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
    crate::term::emit_out(&format!(
        "Đã quét {} thư mục gốc: +{} mới, {} thiếu (missing), {} khôi phục, {} không đổi",
        num_roots, report.added, report.missing, report.restored, report.unchanged
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
