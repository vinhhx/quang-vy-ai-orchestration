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
            let dir_val = if dir_str.starts_with('~') {
                let home = env
                    .get("HOME")
                    .cloned()
                    .unwrap_or_else(|| crate::config::home_dir().to_string_lossy().to_string());
                let remainder = dir_str[1..].trim_start_matches(['/', '\\']);
                let joined = std::path::PathBuf::from(home).join(remainder);
                joined.to_string_lossy().to_string()
            } else {
                dir_str.clone()
            };
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
