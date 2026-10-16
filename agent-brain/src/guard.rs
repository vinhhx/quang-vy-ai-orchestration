use crate::error::BrainResult;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

pub const GUARD_MATCHER: &str = "Read|Edit|Write|MultiEdit|NotebookEdit|Grep|Glob";
pub const WRITE_TOOLS: [&str; 4] = ["Write", "Edit", "MultiEdit", "NotebookEdit"];
pub const READ_TOOLS: [&str; 3] = ["Read", "Grep", "Glob"];

#[derive(Debug, Clone)]
pub struct Policy {
    pub root: PathBuf,
    pub read_allow: Option<Vec<PathBuf>>,
    pub write_allow: Vec<PathBuf>,
    pub log: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub allow: bool,
    pub tool: String,
    pub path: String,
    pub reason: String,
}

fn realpath_nonstrict(p: &Path) -> PathBuf {
    let abs_p = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(p)
    };

    let mut cur = abs_p.clone();
    let mut tail = Vec::new();

    loop {
        if let Ok(canon) = cur.canonicalize() {
            let mut result = canon;
            for part in tail.into_iter().rev() {
                result.push(part);
            }
            return result;
        }

        #[cfg(unix)]
        {
            if cur.is_symlink() {
                if let Ok(target) = std::fs::read_link(&cur) {
                    let next = if target.is_absolute() {
                        target
                    } else {
                        cur.parent().unwrap_or(Path::new("/")).join(target)
                    };
                    let mut resolved = realpath_nonstrict(&next);
                    for part in tail.into_iter().rev() {
                        resolved.push(part);
                    }
                    return resolved;
                }
            }
        }

        if let Some(file_name) = cur.file_name() {
            tail.push(file_name.to_os_string());
            if let Some(parent) = cur.parent() {
                cur = parent.to_path_buf();
            } else {
                break;
            }
        } else {
            break;
        }
    }

    abs_p
}

fn realpath_nfc(p: &Path) -> PathBuf {
    let resolved = realpath_nonstrict(p);
    let s = resolved.to_string_lossy();
    let nfc: String = s.nfc().collect();
    PathBuf::from(nfc)
}

pub fn decide(policy: &Policy, payload: &serde_json::Value) -> Decision {
    let tool_name = payload
        .get("tool_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let tool_input = payload.get("tool_input").and_then(|v| v.as_object());
    let cwd = payload
        .get("cwd")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| policy.root.to_str().unwrap_or(""));

    let is_write = WRITE_TOOLS.contains(&tool_name);
    let is_read = READ_TOOLS.contains(&tool_name);

    if !is_write && !is_read {
        return Decision {
            allow: true,
            tool: tool_name.to_string(),
            path: String::new(),
            reason: format!("tool {tool_name} not guarded"),
        };
    }

    let raw_path: Option<String> = if is_write {
        if tool_name == "NotebookEdit" {
            tool_input
                .and_then(|ti| ti.get("notebook_path"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            tool_input
                .and_then(|ti| ti.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        }
    } else if tool_name == "Grep" || tool_name == "Glob" {
        let p = tool_input
            .and_then(|ti| ti.get("path"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        Some(p.unwrap_or(cwd).to_string())
    } else {
        tool_input
            .and_then(|ti| ti.get("file_path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    let raw_path_str = match raw_path {
        Some(p) => p,
        None => {
            let reason = if is_write {
                "missing path for write-class tool"
            } else {
                "missing file_path for Read"
            };
            return Decision {
                allow: false,
                tool: tool_name.to_string(),
                path: String::new(),
                reason: reason.to_string(),
            };
        }
    };

    let norm_path: String = raw_path_str.nfc().collect();
    let joined = if !Path::new(&norm_path).is_absolute() {
        let base_dir = if !cwd.is_empty() {
            PathBuf::from(cwd)
        } else {
            policy.root.clone()
        };
        base_dir.join(&norm_path)
    } else {
        PathBuf::from(&norm_path)
    };

    let resolved = realpath_nfc(&joined);

    let write_allow: Vec<PathBuf> = policy.write_allow.iter().map(|p| realpath_nfc(p)).collect();
    let read_allow: Option<Vec<PathBuf>> = policy
        .read_allow
        .as_ref()
        .map(|list| list.iter().map(|p| realpath_nfc(p)).collect());

    if is_write {
        if write_allow.contains(&resolved) {
            Decision {
                allow: true,
                tool: tool_name.to_string(),
                path: raw_path_str,
                reason: "in write allowlist".to_string(),
            }
        } else {
            Decision {
                allow: false,
                tool: tool_name.to_string(),
                path: raw_path_str,
                reason: format!("path {} not in write allowlist", resolved.display()),
            }
        }
    } else if read_allow.is_none() {
        Decision {
            allow: true,
            tool: tool_name.to_string(),
            path: raw_path_str,
            reason: "read-free mode".to_string(),
        }
    } else {
        let read_allow_vec = read_allow.unwrap();
        if tool_name == "Grep" || tool_name == "Glob" {
            Decision {
                allow: false,
                tool: tool_name.to_string(),
                path: raw_path_str,
                reason: format!("{tool_name} prohibited when read_allow is set"),
            }
        } else if read_allow_vec.contains(&resolved) {
            Decision {
                allow: true,
                tool: tool_name.to_string(),
                path: raw_path_str,
                reason: "in read allowlist".to_string(),
            }
        } else {
            Decision {
                allow: false,
                tool: tool_name.to_string(),
                path: raw_path_str,
                reason: format!("path {} not in read allowlist", resolved.display()),
            }
        }
    }
}

pub fn sh_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

pub fn write_hook_settings(
    run_dir: &Path,
    root: &Path,
    read_allow: Option<&[PathBuf]>,
    write_allow: &[PathBuf],
    exe: &Path,
) -> BrainResult<PathBuf> {
    std::fs::create_dir_all(run_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(run_dir, std::fs::Permissions::from_mode(0o700));
    }

    let root_resolved = realpath_nfc(root);
    let norm_write: Vec<String> = write_allow
        .iter()
        .map(|p| realpath_nfc(p).to_string_lossy().to_string())
        .collect();
    let norm_read: Option<Vec<String>> = read_allow.map(|list| {
        list.iter()
            .map(|p| realpath_nfc(p).to_string_lossy().to_string())
            .collect()
    });

    let run_dir_resolved = realpath_nfc(run_dir);
    let log_path = run_dir_resolved.join("guard.jsonl");
    let policy_path = run_dir.join("policy.json");
    let settings_path = run_dir.join("settings.json");

    let policy_data = serde_json::json!({
        "root": root_resolved.to_string_lossy(),
        "read_allow": norm_read,
        "write_allow": norm_write,
        "log": log_path.to_string_lossy(),
    });

    std::fs::write(
        &policy_path,
        serde_json::to_string_pretty(&policy_data)
            .map_err(|e| crate::error::BrainError::error(format!("{e}")))?,
    )?;

    let exe_resolved = realpath_nfc(exe);
    let policy_resolved = realpath_nfc(&policy_path);
    let cmd = format!(
        "{} guard --policy {}",
        sh_quote(&exe_resolved.to_string_lossy()),
        sh_quote(&policy_resolved.to_string_lossy()),
    );

    let settings_data = serde_json::json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": GUARD_MATCHER,
                    "hooks": [
                        {
                            "type": "command",
                            "command": cmd,
                        }
                    ],
                }
            ]
        }
    });

    std::fs::write(
        &settings_path,
        serde_json::to_string_pretty(&settings_data)
            .map_err(|e| crate::error::BrainError::error(format!("{e}")))?,
    )?;

    Ok(settings_path)
}

fn append_log(log_path: &Path, decision: &Decision) {
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let entry = serde_json::json!({
        "ts": chrono::Utc::now().to_rfc3339(),
        "tool": decision.tool,
        "path": decision.path,
        "decision": if decision.allow { "allow" } else { "block" },
        "reason": decision.reason,
    });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        use std::io::Write;
        if let Ok(line) = serde_json::to_string(&entry) {
            let _ = writeln!(f, "{line}");
        }
    }
}

pub fn cmd_guard(policy: Option<&Path>) -> i32 {
    let policy_path = match policy {
        Some(p) => p,
        None => {
            crate::term::emit("agent-brain guard: thiếu đối số --policy");
            return 2;
        }
    };

    let policy_str = match std::fs::read_to_string(policy_path) {
        Ok(s) => s,
        Err(e) => {
            crate::term::emit(&format!("agent-brain guard: không thể đọc policy: {e}"));
            return 2;
        }
    };

    let policy_val: serde_json::Value = match serde_json::from_str(&policy_str) {
        Ok(v) => v,
        Err(e) => {
            crate::term::emit(&format!("agent-brain guard: không thể đọc policy: {e}"));
            return 2;
        }
    };

    let policy_obj = match policy_val.as_object() {
        Some(o) => o,
        None => {
            crate::term::emit("agent-brain guard: policy không phải object");
            return 2;
        }
    };

    let root = policy_obj
        .get("root")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_default();

    let log = policy_obj
        .get("log")
        .and_then(|v| v.as_str())
        .map(PathBuf::from);

    let read_allow = if let Some(v) = policy_obj.get("read_allow") {
        if v.is_null() {
            None
        } else if let Some(arr) = v.as_array() {
            Some(
                arr.iter()
                    .filter_map(|x| x.as_str().map(PathBuf::from))
                    .collect(),
            )
        } else {
            None
        }
    } else {
        None
    };

    let write_allow = if let Some(arr) = policy_obj.get("write_allow").and_then(|v| v.as_array()) {
        arr.iter()
            .filter_map(|x| x.as_str().map(PathBuf::from))
            .collect()
    } else {
        Vec::new()
    };

    let pol = Policy {
        root,
        read_allow,
        write_allow,
        log: log.clone(),
    };

    let mut raw_stdin = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut raw_stdin).is_err() {
        crate::term::emit("agent-brain guard: stdin không hợp lệ");
        return 2;
    }

    if raw_stdin.trim().is_empty() {
        return 2;
    }

    let payload: serde_json::Value = match serde_json::from_str(&raw_stdin) {
        Ok(v) => v,
        Err(e) => {
            crate::term::emit(&format!("agent-brain guard: stdin không hợp lệ: {e}"));
            return 2;
        }
    };

    if !payload.is_object() {
        return 2;
    }

    let decision = decide(&pol, &payload);

    if let Some(ref log_path) = pol.log {
        append_log(log_path, &decision);
    }

    if !decision.allow {
        let display_path = if decision.path.is_empty() {
            "<none>"
        } else {
            &decision.path
        };
        crate::term::emit(&format!(
            "agent-brain guard: chặn {} {} — ngoài danh sách được phép của vai trò (xem .plan.md)",
            decision.tool, display_path
        ));
        2
    } else {
        0
    }
}
