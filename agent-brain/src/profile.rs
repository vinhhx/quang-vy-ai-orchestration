use crate::config::ProfileCfg;
use crate::error::{BrainError, BrainResult};
use std::collections::BTreeMap;

pub const NESTED_SESSION_VARS: [&str; 11] = [
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "CLAUDE_VC_ENV",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
];

pub fn check_profile(profile: &str, profiles: &BTreeMap<String, ProfileCfg>) -> BrainResult<()> {
    if profile.trim().is_empty() {
        return Err(BrainError::error(
            "Dự án chưa đặt profile (work|personal) — sửa projects_map.json rồi chạy lại",
        ));
    }
    if !profiles.contains_key(profile) {
        let names = profiles.keys().cloned().collect::<Vec<_>>().join(", ");
        return Err(BrainError::error(format!(
            "Profile '{profile}' không hợp lệ. Các profile đã cấu hình: {names}"
        )));
    }
    Ok(())
}

pub fn posix_normpath(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let is_abs = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            if let Some(last) = parts.last() {
                if *last != ".." {
                    parts.pop();
                    continue;
                }
            }
            if !is_abs {
                parts.push("..");
            }
        } else {
            parts.push(seg);
        }
    }
    if is_abs {
        format!("/{}", parts.join("/"))
    } else if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// Expands a leading `~` against `HOME` of `env` (falling back to the real home directory) and
/// normalises the result; any other path is returned unchanged.
pub fn expand_home(dir: &str, env: &BTreeMap<String, String>) -> String {
    if let Some(rest) = dir.strip_prefix('~') {
        let home = env
            .get("HOME")
            .cloned()
            .unwrap_or_else(|| crate::config::home_dir().to_string_lossy().to_string());
        let remainder = rest.trim_start_matches(['/', '\\']);
        let joined = format!("{}/{}", home.trim_end_matches(['/', '\\']), remainder);
        posix_normpath(&joined)
    } else {
        dir.to_string()
    }
}

pub fn build_env(
    profile: &str,
    profiles: &BTreeMap<String, ProfileCfg>,
    base_env: &BTreeMap<String, String>,
) -> BrainResult<BTreeMap<String, String>> {
    check_profile(profile, profiles)?;
    let mut env = base_env.clone();
    for var in NESTED_SESSION_VARS {
        env.remove(var);
    }
    let prof_cfg = &profiles[profile];
    for var in &prof_cfg.unset_env {
        env.remove(var);
    }
    match &prof_cfg.claude_config_dir {
        None => {
            env.remove("CLAUDE_CONFIG_DIR");
        }
        Some(dir_str) => {
            let dir_val = expand_home(dir_str, &env);
            env.insert("CLAUDE_CONFIG_DIR".to_string(), dir_val);
        }
    }
    env.insert(
        "CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR".to_string(),
        "1".to_string(),
    );
    Ok(env)
}

pub fn describe_profile(profile: &str, profiles: &BTreeMap<String, ProfileCfg>) -> String {
    if let Some(prof_cfg) = profiles.get(profile) {
        if let Some(ref d) = prof_cfg.claude_config_dir {
            format!("CLAUDE_CONFIG_DIR={d}")
        } else {
            "CLAUDE_CONFIG_DIR không đặt → mặc định ~/.claude".to_string()
        }
    } else {
        "CLAUDE_CONFIG_DIR không đặt → mặc định ~/.claude".to_string()
    }
}

pub fn current_env() -> BTreeMap<String, String> {
    std::env::vars().collect()
}
