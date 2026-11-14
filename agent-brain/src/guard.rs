use crate::error::BrainResult;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

pub const GUARD_MATCHER: &str = "Read|Edit|Write|MultiEdit|NotebookEdit|Grep|Glob|Bash";
pub const BASH_MATCHER: &str = "Bash";
pub const WRITE_TOOLS: [&str; 4] = ["Write", "Edit", "MultiEdit", "NotebookEdit"];
pub const READ_TOOLS: [&str; 3] = ["Read", "Grep", "Glob"];

#[derive(Debug, Clone)]
pub struct Policy {
    pub root: PathBuf,
    pub read_allow: Option<Vec<PathBuf>>,
    pub write_allow: Vec<PathBuf>,
    pub log: Option<PathBuf>,
    /// Directories (absolute) the approved plan lets the Dev read (skill mode, D-49). Empty =
    /// none; a policy file without the field reads as empty.
    pub read_dirs: Vec<PathBuf>,
    /// Directories (absolute) the approved plan lets the Dev write.
    pub write_dirs: Vec<PathBuf>,
    /// Single files (absolute) the approved plan lets the Dev read; missing in an old policy.
    pub read_files: Vec<PathBuf>,
    /// Single files (absolute) the approved plan lets the Dev write.
    pub write_files: Vec<PathBuf>,
    /// Command patterns the approved plan lets the Dev run; `None` = no extra rule (old policy).
    pub bash_allow: Option<Vec<String>>,
    /// Read-only source project roots (absolute, realpath).
    pub source_roots: Vec<PathBuf>,
}

/// The skill-mode part of a Dev run's policy, built from an approved plan's grants.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuardGrants {
    pub read_dirs: Vec<PathBuf>,
    pub write_dirs: Vec<PathBuf>,
    pub read_files: Vec<PathBuf>,
    pub write_files: Vec<PathBuf>,
    pub bash_allow: Option<Vec<String>>,
    pub source_roots: Vec<PathBuf>,
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

    if tool_name == "Bash" {
        return decide_bash(policy, tool_input);
    }

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
        for sr in &policy.source_roots {
            let sr_real = realpath_nfc(sr);
            if resolved.starts_with(&sr_real) {
                return Decision {
                    allow: false,
                    tool: tool_name.to_string(),
                    path: raw_path_str,
                    reason: "thuộc dự án nguồn chỉ đọc".to_string(),
                };
            }
        }

        if write_allow.contains(&resolved) {
            Decision {
                allow: true,
                tool: tool_name.to_string(),
                path: raw_path_str,
                reason: "in write allowlist".to_string(),
            }
        } else if under_grant_any(
            &policy.root,
            &policy.write_dirs,
            &policy.write_files,
            &resolved,
        ) {
            Decision {
                allow: true,
                tool: tool_name.to_string(),
                path: raw_path_str,
                reason: "in approved plan write directory".to_string(),
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

        let mut in_source = None;
        for sr in &policy.source_roots {
            let sr_real = realpath_nfc(sr);
            if let Ok(rel) = resolved.strip_prefix(&sr_real) {
                in_source = Some(rel.to_path_buf());
                break;
            }
        }

        if let Some(rel) = in_source {
            if !grant_denied_rel(&rel.to_string_lossy()) {
                return Decision {
                    allow: true,
                    tool: tool_name.to_string(),
                    path: raw_path_str,
                    reason: "in read-only source project".to_string(),
                };
            }
        }

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
        } else if in_linked_node_modules(&policy.root, &resolved) {
            Decision {
                allow: true,
                tool: tool_name.to_string(),
                path: raw_path_str,
                reason: "in linked node_modules".to_string(),
            }
        } else if under_grant_any(
            &policy.root,
            &policy.read_dirs,
            &policy.read_files,
            &resolved,
        ) || under_grant_any(
            &policy.root,
            &policy.write_dirs,
            &policy.write_files,
            &resolved,
        ) {
            Decision {
                allow: true,
                tool: tool_name.to_string(),
                path: raw_path_str,
                reason: "in approved plan directory".to_string(),
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

/// How one agy tool maps onto the claude tool the guard knows: (agy tool name, claude tool name,
/// agy parameter keys holding the path or command, tried in order). Names and keys seen in real
/// agy 1.x run logs: `read_file` / `write_file` / `edit_file` (`Path`) and `run_command`
/// (`CommandLine`); the others are the names the agy docs use for the same tools.
pub const AGY_TOOL_MAP: &[(&str, &str, &[&str])] = &[
    ("run_command", "Bash", &["CommandLine", "Command"]),
    (
        "read_file",
        "Read",
        &["Path", "AbsolutePath", "TargetFile", "File"],
    ),
    (
        "view_file",
        "Read",
        &["AbsolutePath", "Path", "TargetFile", "File"],
    ),
    (
        "write_file",
        "Write",
        &["Path", "TargetFile", "AbsolutePath", "File"],
    ),
    (
        "write_to_file",
        "Write",
        &["TargetFile", "Path", "AbsolutePath", "File"],
    ),
    (
        "edit_file",
        "Edit",
        &["Path", "TargetFile", "AbsolutePath", "File"],
    ),
    (
        "replace_file_content",
        "Edit",
        &["TargetFile", "Path", "AbsolutePath", "File"],
    ),
    (
        "multi_replace_file_content",
        "Edit",
        &["TargetFile", "Path", "AbsolutePath", "File"],
    ),
    // a directory listing / search: judged by `agy_listing_decision` when reads are limited
    (
        "list_dir",
        "Glob",
        &["DirectoryPath", "Path", "SearchDirectory"],
    ),
    (
        "grep_search",
        "Grep",
        &["SearchPath", "SearchDirectory", "Path", "DirectoryPath"],
    ),
    (
        "find_by_name",
        "Glob",
        &["SearchDirectory", "SearchPath", "Path", "DirectoryPath"],
    ),
];

/// agy tools that only ask the user something: nothing to check.
pub const AGY_ASK_TOOLS: &[&str] = &["ask_question", "ask_permission", "ask_custom_permission"];

/// The agy parameter that sets where `run_command` runs.
const AGY_CWD_KEY: &str = "Cwd";

/// C4: violation kind stored when agy runs in another directory than the project root.
pub const VIOLATION_WRONG_CWD: &str = "wrong_cwd";

/// C4: the `init.cwd` of an agy `init` event compared with the project root, both canonicalized
/// (a path that cannot be canonicalized is compared as written). `Some(cwd)` when agy reports a
/// different directory; `None` when it matches or the event carries no `cwd`.
pub fn agy_init_cwd_mismatch(init_event: &serde_json::Value, root: &Path) -> Option<String> {
    let cwd = init_event
        .get("init")
        .and_then(|i| i.get("cwd"))
        .and_then(|v| v.as_str())?;
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    if canon(Path::new(cwd)) == canon(root) {
        None
    } else {
        Some(cwd.to_string())
    }
}

/// The PreToolUse payload claude would send for one agy tool step, for `decide`. A mapped tool
/// becomes its claude tool (`run_command` -> Bash with `command`, file tools -> Read/Write/Edit
/// with `file_path`, listing/search -> Glob/Grep with `path`); a missing parameter stays missing
/// so `decide` refuses a path tool without a path. Any other tool keeps its own name (which
/// `decide` does not guard). `None` for the tools that only ask the user something.
pub fn agy_tool_payload(
    tool_name: &str,
    params: &serde_json::Map<String, serde_json::Value>,
    root: &Path,
) -> Option<serde_json::Value> {
    if AGY_ASK_TOOLS.contains(&tool_name) {
        return None;
    }
    let first = |keys: &[&str]| -> Option<String> {
        keys.iter()
            .find_map(|k| params.get(*k).and_then(|v| v.as_str()))
            .map(str::to_string)
    };
    let root_str = root.to_string_lossy().to_string();
    let Some((_, claude_tool, keys)) = AGY_TOOL_MAP.iter().find(|(n, _, _)| *n == tool_name) else {
        return Some(serde_json::json!({
            "tool_name": tool_name,
            "tool_input": params,
            "cwd": root_str,
        }));
    };
    let value = first(keys);
    let mut input = serde_json::Map::new();
    let key = match *claude_tool {
        "Bash" => "command",
        "Glob" | "Grep" => "path",
        _ => "file_path",
    };
    if let Some(v) = value {
        input.insert(key.to_string(), serde_json::Value::String(v));
    }
    let cwd = if *claude_tool == "Bash" {
        first(&[AGY_CWD_KEY]).unwrap_or(root_str)
    } else {
        root_str
    };
    Some(serde_json::json!({
        "tool_name": claude_tool,
        "tool_input": input,
        "cwd": cwd,
    }))
}

/// The guard's decision on one agy tool step of a Dev run, logged like a hook decision. `None`
/// when there is nothing to check (a tool that only asks the user). On top of `decide`:
/// - every `browser_*` tool is refused (no plan grants a browser);
/// - `run_command` must run in the project root (`Cwd`), because the command check reads
///   relative paths from the root;
/// - a policy without a command list allows only git: agy has no `--allowedTools` layer behind
///   the guard, which is what limits a claude Dev's other commands;
/// - with limited reads, a listing or search step is judged by `agy_listing_decision` (where
///   the plan lets the Dev read) instead of `decide`'s blanket Glob/Grep refusal.
pub fn agy_tool_decision(
    policy: &Policy,
    tool_name: &str,
    params: &serde_json::Map<String, serde_json::Value>,
) -> Option<Decision> {
    let decision = agy_tool_decision_inner(policy, tool_name, params)?;
    if let Some(log_path) = &policy.log {
        append_log(log_path, &decision);
    }
    Some(decision)
}

/// A listing or search step of an agy Dev (`list_dir`, `find_by_name`, `grep_search`) when its
/// reads are limited. `decide` refuses every Glob/Grep then (a claude Dev has no such tool); an
/// agy Dev may look where it may read: inside the project root, not a hard-denied path, and
///   - inside (or exactly) a granted read or write directory, or a granted single file;
///   - exactly a plan file (read or write allowlist);
///   - for a name listing only (`list_dir`, `find_by_name`): the directory that holds a plan
///     file from the Thay đổi table (protocol files such as the plan do not count). A content
///     search there would read the files beside it, so `grep_search` does not get this one.
fn agy_listing_decision(policy: &Policy, tool_name: &str, raw_path: &str) -> Decision {
    let decision = |allow: bool, reason: String| Decision {
        allow,
        tool: tool_name.to_string(),
        path: raw_path.chars().take(BASH_LOG_MAX_CHARS).collect(),
        reason,
    };
    let norm: String = raw_path.nfc().collect();
    let joined = if Path::new(&norm).is_absolute() {
        PathBuf::from(&norm)
    } else {
        policy.root.join(&norm)
    };
    let resolved = realpath_nfc(&joined);

    for sr in &policy.source_roots {
        let sr_real = realpath_nfc(sr);
        if let Ok(rel) = resolved.strip_prefix(&sr_real) {
            if grant_denied_rel(&rel.to_string_lossy()) {
                return decision(
                    false,
                    format!("{} không được xem (đường dẫn bị cấm)", resolved.display()),
                );
            }
            return decision(true, "in read-only source project".to_string());
        }
    }

    let root_real = realpath_nfc(&policy.root);
    let Ok(rel) = resolved.strip_prefix(&root_real) else {
        return decision(false, format!("{} ngoài thư mục dự án", resolved.display()));
    };
    if grant_denied_rel(&rel.to_string_lossy()) {
        return decision(
            false,
            format!("{} không được xem (đường dẫn bị cấm)", resolved.display()),
        );
    }
    let granted_dir = policy
        .read_dirs
        .iter()
        .chain(policy.write_dirs.iter())
        .map(|d| realpath_nfc(d))
        .any(|d| d != root_real && d.starts_with(&root_real) && d == resolved);
    if granted_dir
        || under_grant_any(
            &policy.root,
            &policy.read_dirs,
            &policy.read_files,
            &resolved,
        )
        || under_grant_any(
            &policy.root,
            &policy.write_dirs,
            &policy.write_files,
            &resolved,
        )
    {
        return decision(true, "in approved plan directory".to_string());
    }
    let plan_files: Vec<PathBuf> = policy
        .read_allow
        .iter()
        .flatten()
        .chain(policy.write_allow.iter())
        .map(|p| realpath_nfc(p))
        .collect();
    if plan_files.contains(&resolved) {
        return decision(true, "plan file".to_string());
    }
    if tool_name != "grep_search" {
        let holds_plan_file = plan_files.iter().any(|f| {
            let name = f
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            !crate::stream::is_protocol_file(&name)
                && name != ".handover.md"
                && f.parent() == Some(resolved.as_path())
        });
        if holds_plan_file {
            return decision(true, "directory of a plan file".to_string());
        }
    }
    decision(
        false,
        format!(
            "{} không nằm trong thư mục được đọc/ghi của plan",
            resolved.display()
        ),
    )
}

fn agy_tool_decision_inner(
    policy: &Policy,
    tool_name: &str,
    params: &serde_json::Map<String, serde_json::Value>,
) -> Option<Decision> {
    if tool_name.starts_with("browser_") {
        return Some(Decision {
            allow: false,
            tool: tool_name.to_string(),
            path: String::new(),
            reason: "công cụ trình duyệt không nằm trong quyền của plan".to_string(),
        });
    }
    let payload = agy_tool_payload(tool_name, params, &policy.root)?;
    let claude_tool = payload.get("tool_name").and_then(|v| v.as_str());
    if matches!(claude_tool, Some("Glob") | Some("Grep")) && policy.read_allow.is_some() {
        let raw = payload
            .get("tool_input")
            .and_then(|ti| ti.get("path"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| policy.root.to_string_lossy().to_string());
        return Some(agy_listing_decision(policy, tool_name, &raw));
    }
    if claude_tool == Some("Bash") {
        if let Some(cwd) = params.get(AGY_CWD_KEY).and_then(|v| v.as_str()) {
            let cwd_path = Path::new(cwd);
            let resolved = if cwd_path.is_absolute() {
                realpath_nfc(cwd_path)
            } else {
                realpath_nfc(&policy.root.join(cwd_path))
            };
            if resolved != realpath_nfc(&policy.root) {
                return Some(Decision {
                    allow: false,
                    tool: "Bash".to_string(),
                    path: cwd.chars().take(BASH_LOG_MAX_CHARS).collect(),
                    reason: format!(
                        "lệnh chạy trong thư mục {} — chỉ được chạy ở thư mục gốc dự án",
                        resolved.display()
                    ),
                });
            }
        }
        if policy.bash_allow.is_none() {
            let mut git_only = policy.clone();
            git_only.bash_allow = Some(Vec::new());
            return Some(decide(&git_only, &payload));
        }
    }
    Some(decide(policy, &payload))
}

/// Returns `Some(description)` when an agy tool step is a safe read operation:
/// - `Read`, `Glob`, `Grep` (or agy tools mapping to them) accessing a path inside the project root
///   or a source root, not matching any denied pattern (`.git`, `.planning`, `.env*`, sensitive).
/// - `Bash` (`run_command`) executing a safe read-only command in the project root without shell
///   operators or redirections, where arguments (treated as paths) are also safe read paths.
/// Returns `None` for writes, edits, forbidden commands/paths, or any other tool.
pub fn agy_soft_read(
    policy: &Policy,
    tool_name: &str,
    params: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    if tool_name.starts_with("browser_") {
        return None;
    }
    let payload = agy_tool_payload(tool_name, params, &policy.root)?;
    let claude_tool = payload.get("tool_name").and_then(|v| v.as_str())?;

    match claude_tool {
        "Read" => {
            let raw_path = payload
                .get("tool_input")
                .and_then(|ti| ti.get("file_path").or_else(|| ti.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if is_safe_read_path(policy, raw_path) {
                Some(format!("đọc an toàn: {raw_path}"))
            } else {
                None
            }
        }
        "Glob" | "Grep" => {
            let raw_path = payload
                .get("tool_input")
                .and_then(|ti| ti.get("path").or_else(|| ti.get("file_path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if is_safe_read_path(policy, raw_path) {
                Some(format!("đọc an toàn: {raw_path}"))
            } else {
                None
            }
        }
        "Bash" => {
            if let Some(cwd) = params.get(AGY_CWD_KEY).and_then(|v| v.as_str()) {
                if !cwd.is_empty() {
                    let cwd_path = Path::new(cwd);
                    let resolved = if cwd_path.is_absolute() {
                        realpath_nfc(cwd_path)
                    } else {
                        realpath_nfc(&policy.root.join(cwd_path))
                    };
                    if resolved != realpath_nfc(&policy.root) {
                        return None;
                    }
                }
            }

            let cmd = payload
                .get("tool_input")
                .and_then(|ti| ti.get("command"))
                .and_then(|v| v.as_str())?;
            let cmd = cmd.trim();
            if cmd.is_empty() {
                return None;
            }

            if cmd.contains(';')
                || cmd.contains('|')
                || cmd.contains('&')
                || cmd.contains('>')
                || cmd.contains('<')
                || cmd.contains("$(")
                || cmd.contains('`')
                || cmd.contains('\n')
                || cmd.contains('\r')
            {
                return None;
            }

            let words = match shell_words(cmd) {
                Ok(w) => w,
                Err(_) => return None,
            };
            if words.is_empty() || is_env_assignment(&words[0].text) {
                return None;
            }

            const SAFE_READ_COMMANDS: &[&str] = &[
                "ls", "cat", "head", "tail", "wc", "stat", "file", "tree", "pwd", "grep", "rg",
                "find",
            ];
            let prog = words[0].text.as_str();
            if !SAFE_READ_COMMANDS.contains(&prog) {
                return None;
            }

            if prog == "find" {
                const FORBIDDEN_FIND_FLAGS: &[&str] = &[
                    "-exec", "-execdir", "-delete", "-ok", "-okdir", "-fprint", "-fprint0",
                    "-fprintf", "-fls",
                ];
                if words[1..].iter().any(|w| {
                    FORBIDDEN_FIND_FLAGS.contains(&w.text.as_str())
                        || w.text.starts_with("-exec")
                        || w.text.starts_with("-ok")
                        || w.text.starts_with("-fprint")
                        || w.text.starts_with("-fprintf")
                        || w.text.starts_with("-fls")
                        || w.text == "-delete"
                }) {
                    return None;
                }
            }

            let is_grep_or_rg = prog == "grep" || prog == "rg";
            let mut seen_grep_pattern = false;

            for w in &words[1..] {
                if w.text.starts_with('-') {
                    if prog == "grep" {
                        if w.text == "--dereference-recursive"
                            || (!w.text.starts_with("--") && w.text[1..].contains('R'))
                        {
                            return None;
                        }
                    } else if prog == "rg" {
                        if w.text == "--follow"
                            || (!w.text.starts_with("--") && w.text[1..].contains('L'))
                        {
                            return None;
                        }
                    } else if prog == "find" {
                        if w.text == "-L" || w.text == "-follow" || w.text == "-H" {
                            return None;
                        }
                    } else if prog == "tree" {
                        if w.text == "-l"
                            || (!w.text.starts_with("--") && w.text[1..].contains('l'))
                        {
                            return None;
                        }
                    } else if prog == "ls" {
                        if w.text == "-L"
                            || (!w.text.starts_with("--") && w.text[1..].contains('L'))
                        {
                            return None;
                        }
                    }
                    continue;
                }

                if w.glob
                    || w.text
                        .chars()
                        .any(|c| matches!(c, '*' | '?' | '[' | ']' | '{' | '}'))
                {
                    return None;
                }

                if is_grep_or_rg && !seen_grep_pattern {
                    seen_grep_pattern = true;
                    continue;
                }

                if !is_safe_read_path(policy, &w.text) {
                    return None;
                }
            }

            Some(format!("lệnh chỉ đọc an toàn: {cmd}"))
        }
        _ => None,
    }
}

fn is_safe_read_path(policy: &Policy, raw_path: &str) -> bool {
    let norm: String = raw_path.nfc().collect();
    let joined = if Path::new(&norm).is_absolute() {
        PathBuf::from(&norm)
    } else {
        policy.root.join(&norm)
    };
    let resolved = realpath_nfc(&joined);

    for sr in &policy.source_roots {
        let sr_real = realpath_nfc(sr);
        if let Ok(rel) = resolved.strip_prefix(&sr_real) {
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            return !grant_denied_rel(&rel_str);
        }
    }

    let root_real = realpath_nfc(&policy.root);
    if let Ok(rel) = resolved.strip_prefix(&root_real) {
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        return !grant_denied_rel(&rel_str);
    }

    false
}

/// True when the root-relative path is one a directory grant never covers: anything under
/// `.git` or `.planning`, any `.env*` component, and the sensitive paths of `report`.
pub fn grant_denied_rel(rel: &str) -> bool {
    let lower = rel.to_lowercase();
    lower
        .split('/')
        .any(|c| c == ".git" || c == ".planning" || c.starts_with(".env"))
        || crate::report::sensitive_path(&lower)
}

/// True when `<root>/node_modules` is a symlink (the link a task worktree gets to the project's
/// `node_modules`) and `resolved` (already resolved through symlinks) lies under the folder it
/// points to. Read only: the caller uses it for `Read` and never for writes. Nothing else of the
/// link target's surroundings opens up, a target not named `node_modules` grants nothing, and
/// `.git`, `.planning` and `.env*` components stay denied.
fn in_linked_node_modules(root: &Path, resolved: &Path) -> bool {
    let link = root.join("node_modules");
    if !std::fs::symlink_metadata(&link).is_ok_and(|m| m.file_type().is_symlink()) {
        return false;
    }
    let target = realpath_nfc(&link);
    if target.file_name().is_none_or(|n| n != "node_modules") || !target.is_dir() {
        return false;
    }
    let Ok(rel) = resolved.strip_prefix(&target) else {
        return false;
    };
    let lower = rel.to_string_lossy().to_lowercase();
    !lower
        .split('/')
        .any(|c| c == ".git" || c == ".planning" || c.starts_with(".env"))
}

/// True when `resolved` (already resolved through symlinks) lies under one of the granted
/// directories. A directory that resolves outside the root (a symlink) or to the root itself
/// grants nothing, and the hard denylist of paths holds whatever a directory says.
fn under_grant_dirs(root: &Path, dirs: &[PathBuf], resolved: &Path) -> bool {
    if dirs.is_empty() {
        return false;
    }
    let root_real = realpath_nfc(root);
    let Ok(rel) = resolved.strip_prefix(&root_real) else {
        return false;
    };
    if grant_denied_rel(&rel.to_string_lossy()) {
        return false;
    }
    dirs.iter().any(|d| {
        let d_real = realpath_nfc(d);
        d_real != root_real
            && d_real.starts_with(&root_real)
            && resolved.starts_with(&d_real)
            && resolved != d_real
    })
}

/// True when `resolved` is exactly one of the granted single files: it must lie inside the root
/// (not be the root), not be a hard-denied path, and be that very path, never a sibling, a
/// parent or a directory.
fn is_grant_file(root: &Path, files: &[PathBuf], resolved: &Path) -> bool {
    if files.is_empty() {
        return false;
    }
    let root_real = realpath_nfc(root);
    let Ok(rel) = resolved.strip_prefix(&root_real) else {
        return false;
    };
    if rel.as_os_str().is_empty() || grant_denied_rel(&rel.to_string_lossy()) || resolved.is_dir() {
        return false;
    }
    files.iter().any(|f| {
        let f_real = realpath_nfc(f);
        f_real != root_real && f_real.starts_with(&root_real) && f_real == resolved
    })
}

/// A directory grant or a single-file grant covers `resolved`.
fn under_grant_any(root: &Path, dirs: &[PathBuf], files: &[PathBuf], resolved: &Path) -> bool {
    under_grant_dirs(root, dirs, resolved) || is_grant_file(root, files, resolved)
}

/// Maximum number of characters of a Bash command kept in a decision / log line.
const BASH_LOG_MAX_CHARS: usize = 200;

fn decide_bash(
    policy: &Policy,
    tool_input: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Decision {
    let command = match tool_input
        .and_then(|ti| ti.get("command"))
        .and_then(|v| v.as_str())
    {
        Some(c) => c,
        None => {
            return Decision {
                allow: false,
                tool: "Bash".to_string(),
                path: String::new(),
                reason: "thiếu lệnh Bash".to_string(),
            };
        }
    };
    let shown: String = command.chars().take(BASH_LOG_MAX_CHARS).collect();
    let checked = bash_command_check_with(command, Some(&policy.root)).and_then(|()| match &policy.bash_allow {
        Some(allow) => bash_grant_check(command, allow, policy),
        None => Ok(()),
    });
    match checked {
        Ok(()) => Decision {
            allow: true,
            tool: "Bash".to_string(),
            path: shown,
            reason: "lệnh Bash được phép".to_string(),
        },
        Err(reason) => Decision {
            allow: false,
            tool: "Bash".to_string(),
            path: shown,
            reason,
        },
    }
}

/// True when the words of a command match an approved pattern: every word before a final `*`
/// equal, and anything after it; without `*` the words must be equal.
fn command_matches(pattern: &str, words: &[ShellWord]) -> bool {
    let pat: Vec<&str> = pattern.split_whitespace().collect();
    if pat.is_empty() {
        return false;
    }
    let (fixed, open) = match pat.last() {
        Some(&"*") if pat.len() > 1 => (&pat[..pat.len() - 1], true),
        _ => (&pat[..], false),
    };
    if words.len() < fixed.len() || (!open && words.len() != fixed.len()) {
        return false;
    }
    fixed
        .iter()
        .zip(words.iter())
        .all(|(p, w)| *p == w.text.as_str())
}

/// True when `command` (one shell command line) matches the approved `pattern` by the same rule
/// the guard applies to a Dev's Bash call (`command_matches`). A command the guard can not split
/// into plain words never matches.
pub fn command_matches_grant(pattern: &str, command: &str) -> bool {
    match shell_words(command) {
        Ok(words) => command_matches(pattern, &words),
        Err(_) => false,
    }
}

/// Skill mode (D-49): with an approved command list, a non-git command must be one plain command
/// that matches an approved pattern and does not start with a denied program. Runs after
/// `bash_command_check`, so the git policy and the one-command rule already held.
fn bash_grant_check(command: &str, allow: &[String], policy: &Policy) -> Result<(), String> {
    let words = shell_words(command)?;
    let Some(first) = words.first() else {
        return Err("lệnh Bash trống".to_string());
    };
    // `git add` / `git commit` / read-only git stay as the git policy decides
    if word_basename(&first.text) == "git" {
        return Ok(());
    }
    let prog = first.text.as_str();
    if prog.starts_with('/') || prog.starts_with('~') || prog.split('/').any(|c| c == "..") {
        return Err(format!(
            "chương trình {prog} có đường dẫn tuyệt đối hoặc `..` — không được phép"
        ));
    }
    if let Some(w) = words.iter().find(|w| w.glob) {
        return Err(format!(
            "lệnh có ký tự glob ngoài dấu nháy ({}) — hãy đặt đối số trong dấu nháy đơn",
            w.text
        ));
    }
    let args: Vec<&str> = words[1..].iter().map(|w| w.text.as_str()).collect();
    if let Some(reason) = crate::planfile::is_forbidden_program(prog, &args) {
        return Err(format!("{reason} — không được phép"));
    }
    if allow.iter().any(|p| command_matches(p, &words)) {
        // the program and the script an interpreter runs may not live where the Dev can write
        let all: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        for w in crate::planfile::executed_words(&all) {
            if w == "*" {
                continue;
            }
            let resolved = realpath_nfc(&policy.root.join(w));
            if under_grant_any(
                &policy.root,
                &policy.write_dirs,
                &policy.write_files,
                &resolved,
            ) {
                return Err("lệnh chạy file trong thư mục được ghi — không được phép".to_string());
            }
        }
        Ok(())
    } else {
        Err("lệnh không nằm trong danh sách Lệnh được chạy của plan đã duyệt".to_string())
    }
}

/// Checks one Bash command line against the guard's git policy (W-1).
/// `Err` carries the Vietnamese reason shown to the model.
pub fn bash_command_check(command: &str) -> Result<(), String> {
    bash_command_check_with(command, None)
}

/// Checks one Bash command line with optional repository root for current branch & ref verification.
pub fn bash_command_check_with(command: &str, root: Option<&Path>) -> Result<(), String> {
    let words = shell_words(command)?;
    let Some(first) = words.first() else {
        return Ok(());
    };
    if is_env_assignment(&first.text) {
        return Err(format!(
            "lệnh Bash bắt đầu bằng gán biến môi trường ({}) — không được phép",
            first.text
        ));
    }
    let base = word_basename(&first.text);
    if base.starts_with("git-") {
        return Err(format!(
            "gọi git qua {base} không được phép — dùng một lệnh `git <lệnh>` đơn"
        ));
    }
    if base != "git" {
        if WRAPPER_COMMANDS.contains(&base.as_str())
            && words[1..].iter().any(|w| w.text.contains("git"))
        {
            return Err(format!(
                "lệnh git chạy qua {base} không được phép — dùng một lệnh `git <lệnh>` đơn"
            ));
        }
        // Not git: claude's own permission rules decide.
        return Ok(());
    }
    if let Some(w) = words.iter().find(|w| w.glob) {
        return Err(format!(
            "lệnh git có ký tự glob ngoài dấu nháy ({}) — hãy đặt đối số trong dấu nháy đơn",
            w.text
        ));
    }
    git_policy_check_with(&words[1..], root, false)
}

/// Grant template check (planfile::grant_command_error mode B): ignores required arguments like
/// push refspecs, merge/pull flags, checkout positional count, or commit message.
pub fn git_grant_check(words: &[&str]) -> Result<(), String> {
    if words.is_empty() {
        return Err("lệnh git trống".to_string());
    }
    let mut words_slice = words;
    if words_slice.first() == Some(&"git") {
        words_slice = &words_slice[1..];
    }
    if words_slice.last() == Some(&"*") {
        words_slice = &words_slice[..words_slice.len() - 1];
    }
    let joined = words_slice.join(" ");
    let parsed = shell_words(&joined)?;
    git_policy_check_with(&parsed, None, true)
}

/// The only git global option allowed before the subcommand, besides `-P`.
const GIT_GLOBAL_LONG_ALLOWED: &str = "no-pager";

/// Git subcommands permitted by the guard hook: read-only inspection, network fetch,
/// and working-tree / index modifications when granted.
pub const GIT_WRITE_SUBCOMMANDS: &[&str] = &["add", "commit"];

/// Git subcommands permitted only when declared in the approved plan's grants (Dev only).
pub const GIT_DEV_GRANT_SUBCOMMANDS: &[&str] = &["push", "pull", "merge", "checkout", "switch"];

pub const GIT_READ_SUBCOMMANDS: &[&str] = &[
    "diff",
    "log",
    "show",
    "status",
    "branch",
    "tag",
    "remote",
    "for-each-ref",
    "show-ref",
    "rev-list",
    "rev-parse",
    "merge-base",
    "merge-tree",
    "ls-tree",
    "ls-files",
    "cat-file",
    "blame",
    "describe",
    "shortlog",
    "name-rev",
    "reflog",
    "grep",
    "whatchanged",
    "range-diff",
    "cherry",
];

#[macro_export]
macro_rules! with_git_read_only_tools {
    ($($extra:expr),* $(,)?) => {
        &[
            $($extra,)*
            "Bash(git diff *)",
            "Bash(git log *)",
            "Bash(git show *)",
            "Bash(git status *)",
            "Bash(git branch *)",
            "Bash(git branch)",
            "Bash(git tag *)",
            "Bash(git tag)",
            "Bash(git remote *)",
            "Bash(git remote)",
            "Bash(git for-each-ref *)",
            "Bash(git show-ref *)",
            "Bash(git rev-list *)",
            "Bash(git rev-parse *)",
            "Bash(git merge-base *)",
            "Bash(git merge-tree *)",
            "Bash(git ls-tree *)",
            "Bash(git ls-files *)",
            "Bash(git cat-file *)",
            "Bash(git blame *)",
            "Bash(git describe *)",
            "Bash(git shortlog *)",
            "Bash(git name-rev *)",
            "Bash(git reflog *)",
            "Bash(git reflog)",
            "Bash(git grep *)",
            "Bash(git whatchanged *)",
            "Bash(git range-diff *)",
            "Bash(git cherry *)",
        ]
    };
}

pub const GIT_READ_ONLY_TOOLS: &[&str] = with_git_read_only_tools!();

/// Long options of diff / show / whatchanged / range-diff that write a file,
/// run an external program or read files outside the repository.
const GIT_DIFF_FORBIDDEN: &[&str] = &["output", "ext-diff", "textconv", "no-index"];
/// Long options for log / rev-list / shortlog, forbidding stdin hang in addition.
const GIT_LOG_LIKE_FORBIDDEN: &[&str] = &["output", "ext-diff", "textconv", "no-index", "stdin"];
/// Long options for grep, forbidding pager and reading pattern from file.
const GIT_GREP_FORBIDDEN: &[&str] = &[
    "output",
    "ext-diff",
    "textconv",
    "no-index",
    "open-files-in-pager",
    "file",
];
/// Long options for blame, forbidding contents override.
const GIT_BLAME_FORBIDDEN: &[&str] = &["output", "ext-diff", "textconv", "no-index", "contents"];
/// Long options for cat-file, forbidding filters.
const GIT_CAT_FILE_FORBIDDEN: &[&str] = &["output", "ext-diff", "textconv", "no-index", "filters"];
/// Long options of commit that read the message from a file, open an
/// editor or an interactive prompt (a run would hang until its timeout), or
/// rewrite an existing commit (`--amend`, e.g. the approved plan commit).
const GIT_COMMIT_FORBIDDEN: &[&str] = &[
    "file",
    "template",
    "pathspec-from-file",
    "amend",
    "patch",
    "edit",
    "reedit-message",
    "interactive",
];
/// Long options of add that add ignored files, read pathspecs from a file,
/// or open an editor or an interactive prompt.
const GIT_ADD_FORBIDDEN: &[&str] = &[
    "force",
    "pathspec-from-file",
    "patch",
    "interactive",
    "edit",
];
/// Exact long option names that share a prefix with a forbidden name but
/// are harmless.
const GIT_LONG_EXACT_ALLOWED: &[&str] = &["text", "no-ext-diff", "no-textconv"];

fn git_forbidden_err(sub: &str, opt: &str) -> String {
    format!("git {sub} {opt} ghi file hoặc chạy chương trình ngoài — không được phép")
}

/// Refusal of a `git commit` without a message source: git would open an
/// editor and the run would hang until its timeout.
const GIT_COMMIT_NO_MESSAGE_ERR: &str =
    "git commit cần -m \"…\" (một dòng); không mở trình soạn thảo";

/// True when `name` (a long option name as typed) selects `full`: a prefix
/// of it at least `min` characters long, the way git resolves
/// abbreviations.
fn long_abbrev_of(name: &str, full: &str, min: usize) -> bool {
    name.len() >= min && full.starts_with(name)
}

/// True when a `--fixup` value creates a commit without an editor: the
/// plain `--fixup=<commit>` form. `amend:` / `reword:` (any `<word>:`
/// form) open an editor.
fn plain_fixup_value(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.is_empty() && !v.contains(':'))
}

pub const PROTECTED_BRANCHES: &[&str] = &["main", "master"];

pub fn is_protected_branch(name: &str) -> bool {
    PROTECTED_BRANCHES
        .iter()
        .any(|p| p.eq_ignore_ascii_case(name))
}

fn is_valid_git_ref_or_remote_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let valid_chars = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '/' || c == '-');
    let bad_start = name.starts_with(['-', '/', '.']);
    let bad_contains = name.contains("..") || name.contains(':') || name.contains('+') || name.contains("@{");
    valid_chars && !bad_start && !bad_contains
}

fn current_git_branch(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["symbolic-ref", "--short", "-q", "HEAD"])
        .output()
        .ok()?;
    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() {
            return Some(branch);
        }
    }
    None
}

const GIT_FETCH_LONG_ALLOWED: &[&str] = &[
    "all", "prune", "prune-tags", "tags", "no-tags", "quiet", "verbose", "dry-run",
];

fn git_fetch_err(detail: &str) -> String {
    format!("git fetch chỉ được cập nhật nhánh theo dõi từ remote đã cấu hình (ví dụ: git fetch --all --prune, git fetch origin) — {detail}")
}

fn check_fetch(args: &[ShellWord]) -> Result<(), String> {
    for word in args {
        let w = word.text.as_str();
        if w == "--" {
            continue;
        }
        if let Some(long) = w.strip_prefix("--") {
            let mut parts = long.splitn(2, '=');
            let name = parts.next().unwrap_or("");
            if !GIT_FETCH_LONG_ALLOWED.contains(&name) {
                return Err(git_fetch_err(&format!("tùy chọn `{w}` không được phép")));
            }
            continue;
        }
        if let Some(cluster) = w.strip_prefix('-') {
            if cluster.is_empty() {
                continue;
            }
            for ch in cluster.chars() {
                if !matches!(ch, 'p' | 'P' | 'q' | 'v' | 't') {
                    return Err(git_fetch_err(&format!("tùy chọn `-{ch}` không được phép")));
                }
            }
            continue;
        }
        // Positional argument
        if w.is_empty() {
            continue;
        }
        if !is_valid_git_ref_or_remote_name(w) {
            return Err(git_fetch_err(&format!("đối số `{w}` không hợp lệ")));
        }
    }
    Ok(())
}

const GIT_BRANCH_LONG_ALLOWED: &[&str] = &[
    "all",
    "remotes",
    "verbose",
    "list",
    "merged",
    "no-merged",
    "contains",
    "no-contains",
    "points-at",
    "sort",
    "format",
    "color",
    "no-color",
    "column",
    "no-column",
    "show-current",
    "ignore-case",
    "abbrev",
    "no-abbrev",
    "quiet",
];

fn check_branch(args: &[ShellWord]) -> Result<(), String> {
    let mut has_list_flag = false;
    let mut positional_count = 0;
    let mut j = 0;
    while j < args.len() {
        let w = args[j].text.as_str();
        j += 1;
        if w == "--" {
            while j < args.len() {
                positional_count += 1;
                j += 1;
            }
            break;
        }
        if let Some(long) = w.strip_prefix("--") {
            let mut parts = long.splitn(2, '=');
            let name = parts.next().unwrap_or("");
            let inline_val = parts.next();
            if !GIT_BRANCH_LONG_ALLOWED.contains(&name) {
                return Err(format!(
                    "tùy chọn của git branch `{w}` không được phép — chỉ cho phép liệt kê nhánh"
                ));
            }
            if matches!(
                name,
                "list"
                    | "all"
                    | "remotes"
                    | "merged"
                    | "no-merged"
                    | "contains"
                    | "no-contains"
                    | "points-at"
            ) {
                has_list_flag = true;
            }
            if inline_val.is_none() {
                if name == "sort" || name == "format" {
                    if j < args.len() {
                        j += 1;
                    }
                } else if matches!(
                    name,
                    "merged" | "no-merged" | "contains" | "no-contains" | "points-at"
                ) {
                    if let Some(next) = args.get(j) {
                        if !next.text.starts_with('-') {
                            j += 1;
                        }
                    }
                }
            }
            continue;
        }
        if let Some(cluster) = w.strip_prefix('-') {
            if cluster.is_empty() {
                continue;
            }
            for ch in cluster.chars() {
                if !matches!(ch, 'a' | 'r' | 'v' | 'l' | 'i' | 'q') {
                    return Err(format!(
                        "tùy chọn ngắn của git branch `-{ch}` không được phép — chỉ cho phép liệt kê nhánh"
                    ));
                }
                if matches!(ch, 'a' | 'r' | 'l') {
                    has_list_flag = true;
                }
            }
            continue;
        }
        positional_count += 1;
    }
    if positional_count > 0 && !has_list_flag {
        return Err("git branch cần một trong các tùy chọn liệt kê (--list, -l, -a, -r, --merged…) khi có đối số — không tạo hoặc đổi tên nhánh".to_string());
    }
    Ok(())
}

const GIT_TAG_LONG_ALLOWED: &[&str] = &[
    "list",
    "sort",
    "format",
    "contains",
    "no-contains",
    "merged",
    "no-merged",
    "points-at",
    "column",
    "no-column",
    "color",
    "ignore-case",
];

fn check_tag(args: &[ShellWord]) -> Result<(), String> {
    let mut has_list_flag = false;
    let mut positional_count = 0;
    let mut j = 0;
    while j < args.len() {
        let w = args[j].text.as_str();
        j += 1;
        if w == "--" {
            while j < args.len() {
                positional_count += 1;
                j += 1;
            }
            break;
        }
        if let Some(long) = w.strip_prefix("--") {
            let mut parts = long.splitn(2, '=');
            let name = parts.next().unwrap_or("");
            let inline_val = parts.next();
            if !GIT_TAG_LONG_ALLOWED.contains(&name) {
                return Err(format!(
                    "tùy chọn của git tag `{w}` không được phép — chỉ cho phép liệt kê tag"
                ));
            }
            if matches!(
                name,
                "list" | "contains" | "no-contains" | "merged" | "no-merged" | "points-at"
            ) {
                has_list_flag = true;
            }
            if inline_val.is_none() {
                if name == "sort" || name == "format" {
                    if j < args.len() {
                        j += 1;
                    }
                } else if matches!(
                    name,
                    "contains" | "no-contains" | "merged" | "no-merged" | "points-at"
                ) {
                    if let Some(next) = args.get(j) {
                        if !next.text.starts_with('-') {
                            j += 1;
                        }
                    }
                }
            }
            continue;
        }
        if let Some(cluster) = w.strip_prefix('-') {
            if cluster.is_empty() {
                continue;
            }
            for ch in cluster.chars() {
                if !matches!(ch, 'l' | 'n' | 'i' | '0'..='9') {
                    return Err(format!(
                        "tùy chọn ngắn của git tag `-{ch}` không được phép — chỉ cho phép liệt kê tag"
                    ));
                }
                if ch == 'l' {
                    has_list_flag = true;
                }
            }
            continue;
        }
        positional_count += 1;
    }
    if positional_count > 0 && !has_list_flag {
        return Err("git tag cần `-l`/`--list` hoặc tùy chọn liệt kê khi có đối số — không tạo hoặc xoá tag".to_string());
    }
    Ok(())
}

fn check_remote(args: &[ShellWord]) -> Result<(), String> {
    if args.is_empty() {
        return Ok(());
    }
    let first = &args[0].text;
    if first == "-v" || first == "--verbose" {
        for w in &args[1..] {
            if w.text != "-v" && w.text != "--verbose" {
                return Err(format!(
                    "git remote {} không được phép — chỉ cho phép `git remote`, `git remote -v`, `git remote get-url <tên>`",
                    w.text
                ));
            }
        }
        return Ok(());
    }
    if let Some(cluster) = first.strip_prefix('-') {
        if !cluster.is_empty() && cluster.chars().all(|c| c == 'v') {
            for w in &args[1..] {
                if w.text != "-v" && w.text != "--verbose" {
                    return Err(format!(
                        "git remote {} không được phép — chỉ cho phép `git remote`, `git remote -v`, `git remote get-url <tên>`",
                        w.text
                    ));
                }
            }
            return Ok(());
        }
    }
    if first == "get-url" {
        let rest = &args[1..];
        let mut name: Option<&str> = None;
        for w in rest {
            if w.text == "--push" || w.text == "--all" {
                continue;
            }
            if w.text.starts_with('-') {
                return Err(format!(
                    "tùy chọn của git remote get-url `{}` không được phép",
                    w.text
                ));
            }
            if name.is_none() {
                name = Some(&w.text);
            } else {
                return Err(format!(
                    "git remote get-url chỉ nhận một tên remote — `{}` không được phép",
                    w.text
                ));
            }
        }
        if name.is_none() {
            return Err("git remote get-url cần tên remote (ví dụ origin)".to_string());
        }
        return Ok(());
    }
    Err(format!(
        "git remote {first} không được phép — chỉ cho phép `git remote`, `git remote -v`, `git remote get-url <tên>`"
    ))
}

const GIT_PUSH_LONG_ALLOWED: &[&str] = &[
    "set-upstream",
    "dry-run",
    "quiet",
    "verbose",
    "porcelain",
    "atomic",
];

fn check_push(args: &[ShellWord], is_grant_mode: bool) -> Result<(), String> {
    let mut positionals = Vec::new();
    for word in args {
        let w = word.text.as_str();
        if w == "--" {
            return Err("git push không nhận `--`".to_string());
        }
        if let Some(long) = w.strip_prefix("--") {
            let mut parts = long.splitn(2, '=');
            let name = parts.next().unwrap_or("");
            let inline_val = parts.next();
            if !GIT_PUSH_LONG_ALLOWED.contains(&name) {
                return Err(format!("tùy chọn của git push `{w}` không được phép"));
            }
            if inline_val.is_some() {
                return Err(format!("tùy chọn của git push `{w}` không nhận giá trị"));
            }
            continue;
        }
        if let Some(cluster) = w.strip_prefix('-') {
            if cluster.is_empty() {
                continue;
            }
            for ch in cluster.chars() {
                if !matches!(ch, 'u' | 'n' | 'q' | 'v') {
                    return Err(format!("tùy chọn ngắn của git push `-{ch}` không được phép"));
                }
            }
            continue;
        }
        positionals.push(w);
    }

    if positionals.is_empty() {
        if is_grant_mode {
            return Err("git push cần tên remote (ví dụ git push origin <nhánh>)".to_string());
        }
        return Err("git push cần remote và refspec (ví dụ git push origin <nhánh>)".to_string());
    }

    let remote = positionals[0];
    if !is_valid_git_ref_or_remote_name(remote) {
        return Err(format!("remote của git push `{remote}` không hợp lệ"));
    }

    let refspecs = &positionals[1..];
    if refspecs.is_empty() {
        if is_grant_mode {
            return Ok(());
        }
        return Err("git push cần ít nhất một refspec (ví dụ git push origin <nhánh>)".to_string());
    }

    for refspec in refspecs {
        if refspec.starts_with('+') {
            return Err(format!(
                "refspec của git push `{refspec}` không được bắt đầu bằng `+` (cấm force push)"
            ));
        }
        let (src, dst) = if let Some(idx) = refspec.find(':') {
            let src = &refspec[..idx];
            let dst = &refspec[idx + 1..];
            if src.is_empty() {
                return Err("không được xoá nhánh từ xa (refspec có src rỗng)".to_string());
            }
            if dst.is_empty() || dst.contains(':') {
                return Err(format!("refspec của git push `{refspec}` không hợp lệ"));
            }
            (src, dst)
        } else {
            (*refspec, *refspec)
        };

        if !refspec.contains(':') && src == "HEAD" {
            return Err("ghi rõ tên nhánh đích, ví dụ git push origin HEAD:<nhánh>".to_string());
        }

        for part in [src, dst] {
            if part.starts_with("refs/") && !part.starts_with("refs/heads/") {
                return Err(format!(
                    "refspec `{part}` không được phép — chỉ cho phép push lên refs/heads/"
                ));
            }
        }

        if !is_valid_git_ref_or_remote_name(src) {
            return Err(format!("nhánh nguồn `{src}` không hợp lệ"));
        }
        if !is_valid_git_ref_or_remote_name(dst) {
            return Err(format!("nhánh đích `{dst}` không hợp lệ"));
        }

        let stripped_dst = dst.strip_prefix("refs/heads/").unwrap_or(dst);
        if is_protected_branch(stripped_dst) {
            return Err(format!(
                "không được push lên nhánh {stripped_dst} (main/master được bảo vệ)"
            ));
        }
    }

    Ok(())
}

const GIT_PULL_LONG_ALLOWED: &[&str] = &[
    "no-rebase",
    "no-ff",
    "no-edit",
    "quiet",
    "verbose",
    "prune",
    "all",
    "no-tags",
    "stat",
    "no-stat",
];

fn check_pull(
    args: &[ShellWord],
    root: Option<&Path>,
    is_grant_mode: bool,
) -> Result<(), String> {
    let mut has_no_rebase = false;
    let mut has_no_ff = false;
    let mut positionals = Vec::new();

    for word in args {
        let w = word.text.as_str();
        if w == "--" {
            return Err("git pull không nhận `--`".to_string());
        }
        if let Some(long) = w.strip_prefix("--") {
            let mut parts = long.splitn(2, '=');
            let name = parts.next().unwrap_or("");
            let inline_val = parts.next();
            if !GIT_PULL_LONG_ALLOWED.contains(&name) {
                return Err(format!("tùy chọn của git pull `{w}` không được phép"));
            }
            if inline_val.is_some() {
                return Err(format!("tùy chọn của git pull `{w}` không nhận giá trị"));
            }
            if name == "no-rebase" {
                has_no_rebase = true;
            } else if name == "no-ff" {
                has_no_ff = true;
            }
            continue;
        }
        if let Some(cluster) = w.strip_prefix('-') {
            if cluster.is_empty() {
                continue;
            }
            for ch in cluster.chars() {
                if !matches!(ch, 'q' | 'v' | 'p') {
                    return Err(format!("tùy chọn ngắn của git pull `-{ch}` không được phép"));
                }
            }
            continue;
        }
        if !is_valid_git_ref_or_remote_name(w) {
            return Err(format!("đối số của git pull `{w}` không hợp lệ"));
        }
        positionals.push(w);
    }

    if !is_grant_mode && (!has_no_rebase || !has_no_ff) {
        return Err(
            "git pull phải có --no-rebase --no-ff (ví dụ git pull --no-rebase --no-ff origin <nhánh>)"
                .to_string(),
        );
    }

    if let Some(root) = root {
        if let Some(branch) = current_git_branch(root) {
            if is_protected_branch(&branch) {
                return Err(format!(
                    "không merge/pull khi đang ở nhánh {branch} (main/master được bảo vệ)"
                ));
            }
        }
    }

    Ok(())
}

const GIT_MERGE_LONG_ALLOWED: &[&str] = &[
    "no-ff",
    "no-edit",
    "no-commit",
    "squash",
    "stat",
    "no-stat",
    "quiet",
    "verbose",
    "abort",
    "allow-unrelated-histories",
    "log",
    "no-log",
    "message",
];

fn check_merge(
    args: &[ShellWord],
    root: Option<&Path>,
    is_grant_mode: bool,
) -> Result<(), String> {
    let mut has_no_ff = false;
    let mut has_squash = false;
    let mut has_abort = false;
    let mut positionals = Vec::new();
    let mut j = 0;

    while j < args.len() {
        let w = args[j].text.as_str();
        j += 1;
        if w == "--" {
            return Err("git merge không nhận `--`".to_string());
        }
        if let Some(long) = w.strip_prefix("--") {
            let mut parts = long.splitn(2, '=');
            let name = parts.next().unwrap_or("");
            let inline_val = parts.next();
            if !GIT_MERGE_LONG_ALLOWED.contains(&name) {
                return Err(format!("tùy chọn của git merge `{w}` không được phép"));
            }
            if name == "abort" {
                has_abort = true;
            } else if name == "no-ff" {
                has_no_ff = true;
            } else if name == "squash" {
                has_squash = true;
            } else if name == "message" && inline_val.is_none() && j < args.len() {
                j += 1;
            }
            continue;
        }
        if let Some(cluster) = w.strip_prefix('-') {
            if cluster.is_empty() {
                continue;
            }
            for (k, ch) in cluster.char_indices() {
                if ch == 'm' {
                    if k + ch.len_utf8() == cluster.len() && j < args.len() {
                        j += 1;
                    }
                    break;
                }
                if !matches!(ch, 'q' | 'v' | 'n') {
                    return Err(format!("tùy chọn ngắn của git merge `-{ch}` không được phép"));
                }
            }
            continue;
        }
        if !is_valid_git_ref_or_remote_name(w) {
            return Err(format!("tên nhánh/ref của git merge `{w}` không hợp lệ"));
        }
        positionals.push(w);
    }

    if has_abort {
        if !positionals.is_empty() {
            return Err("git merge --abort không nhận đối số".to_string());
        }
        return Ok(());
    }

    if !is_grant_mode {
        if !has_no_ff && !has_squash {
            return Err(
                "git merge phải có --no-ff hoặc --squash (ví dụ git merge --no-ff <nhánh>)"
                    .to_string(),
            );
        }
        if positionals.is_empty() {
            return Err("git merge cần tên nhánh hoặc ref để merge (ví dụ git merge --no-ff origin/main)".to_string());
        }
    }

    if let Some(root) = root {
        if let Some(branch) = current_git_branch(root) {
            if is_protected_branch(&branch) {
                return Err(format!(
                    "không merge/pull khi đang ở nhánh {branch} (main/master được bảo vệ)"
                ));
            }
        }
    }

    Ok(())
}

const GIT_CHECKOUT_LONG_ALLOWED: &[&str] = &["quiet", "detach", "track", "no-track"];

fn check_checkout(
    args: &[ShellWord],
    root: Option<&Path>,
    is_grant_mode: bool,
) -> Result<(), String> {
    let mut new_branch: Option<String> = None;
    let mut has_track = false;
    let mut positionals = Vec::new();
    let mut j = 0;

    while j < args.len() {
        let w = args[j].text.as_str();
        j += 1;
        if w == "--" {
            return Err("git checkout không được dùng `--` (chỉ dùng để chuyển/tạo nhánh, không đụng tới file)".to_string());
        }
        if let Some(long) = w.strip_prefix("--") {
            let mut parts = long.splitn(2, '=');
            let name = parts.next().unwrap_or("");
            let inline_val = parts.next();
            if !GIT_CHECKOUT_LONG_ALLOWED.contains(&name) {
                return Err(format!("tùy chọn của git checkout `{w}` không được phép"));
            }
            if inline_val.is_some() {
                return Err(format!("tùy chọn của git checkout `{w}` không nhận giá trị"));
            }
            if name == "track" {
                has_track = true;
            }
            continue;
        }
        if let Some(cluster) = w.strip_prefix('-') {
            if cluster.is_empty() {
                continue;
            }
            for (k, ch) in cluster.char_indices() {
                if ch == 'b' {
                    let rest_cluster = &cluster[k + ch.len_utf8()..];
                    let b_val = if !rest_cluster.is_empty() {
                        rest_cluster.to_string()
                    } else if j < args.len() {
                        let val = args[j].text.clone();
                        j += 1;
                        val
                    } else {
                        return Err("tùy chọn -b của git checkout cần tên nhánh mới".to_string());
                    };
                    new_branch = Some(b_val);
                    break;
                }
                if ch == 't' {
                    has_track = true;
                } else if ch == 'q' {
                    // ok
                } else {
                    return Err(format!("tùy chọn ngắn của git checkout `-{ch}` không được phép"));
                }
            }
            continue;
        }
        positionals.push(w);
    }

    if has_track && new_branch.is_none() {
        return Err("tùy chọn --track/-t chỉ được dùng đi kèm -b".to_string());
    }

    if let Some(nb) = &new_branch {
        if !is_valid_git_ref_or_remote_name(nb) {
            return Err(format!("tên nhánh mới `{nb}` không hợp lệ"));
        }
        if is_protected_branch(nb) {
            return Err(format!("không được tạo nhánh {nb} (main/master được bảo vệ)"));
        }
        if !is_grant_mode && positionals.len() > 1 {
            return Err("git checkout -b chỉ nhận tối đa 1 ref nguồn".to_string());
        }
    } else if !is_grant_mode && positionals.len() != 1 {
        return Err("git checkout cần đúng 1 tên nhánh/ref (hoặc dùng -b để tạo nhánh)".to_string());
    }

    for r in &positionals {
        if !is_valid_git_ref_or_remote_name(r) {
            return Err(format!("đối số `{r}` của git checkout không hợp lệ"));
        }
        if let Some(root) = root {
            if root.join(r).exists() {
                return Err(format!(
                    "checkout {r} trùng tên file — không được dùng checkout để khôi phục file"
                ));
            }
            let verify_status = std::process::Command::new("git")
                .arg("-C")
                .arg(root)
                .args(["rev-parse", "--verify", "--quiet", &format!("{r}^{{commit}}")])
                .output();
            let is_ref = verify_status.map(|o| o.status.success()).unwrap_or(false);
            if !is_ref {
                return Err(format!(
                    "checkout chỉ dùng để chuyển nhánh — {r} không phải nhánh/commit"
                ));
            }
        }
    }

    Ok(())
}

const GIT_SWITCH_LONG_ALLOWED: &[&str] = &["create", "detach", "track", "no-track", "quiet"];

fn check_switch(
    args: &[ShellWord],
    _root: Option<&Path>,
    is_grant_mode: bool,
) -> Result<(), String> {
    let mut new_branch: Option<String> = None;
    let mut has_track = false;
    let mut positionals = Vec::new();
    let mut j = 0;

    while j < args.len() {
        let w = args[j].text.as_str();
        j += 1;
        if w == "--" {
            return Err("git switch không nhận `--`".to_string());
        }
        if let Some(long) = w.strip_prefix("--") {
            let mut parts = long.splitn(2, '=');
            let name = parts.next().unwrap_or("");
            let inline_val = parts.next();
            if !GIT_SWITCH_LONG_ALLOWED.contains(&name) {
                return Err(format!("tùy chọn của git switch `{w}` không được phép"));
            }
            if name == "create" {
                let c_val = if let Some(val) = inline_val {
                    val.to_string()
                } else if j < args.len() {
                    let val = args[j].text.clone();
                    j += 1;
                    val
                } else {
                    return Err("tùy chọn --create của git switch cần tên nhánh mới".to_string());
                };
                new_branch = Some(c_val);
            } else {
                if inline_val.is_some() {
                    return Err(format!("tùy chọn của git switch `{w}` không nhận giá trị"));
                }
                if name == "track" {
                    has_track = true;
                }
            }
            continue;
        }
        if let Some(cluster) = w.strip_prefix('-') {
            if cluster.is_empty() {
                continue;
            }
            for (k, ch) in cluster.char_indices() {
                if ch == 'c' {
                    let rest_cluster = &cluster[k + ch.len_utf8()..];
                    let c_val = if !rest_cluster.is_empty() {
                        rest_cluster.to_string()
                    } else if j < args.len() {
                        let val = args[j].text.clone();
                        j += 1;
                        val
                    } else {
                        return Err("tùy chọn -c của git switch cần tên nhánh mới".to_string());
                    };
                    new_branch = Some(c_val);
                    break;
                }
                if ch == 't' {
                    has_track = true;
                } else if ch == 'q' {
                    // ok
                } else {
                    return Err(format!("tùy chọn ngắn của git switch `-{ch}` không được phép"));
                }
            }
            continue;
        }
        positionals.push(w);
    }

    if has_track && new_branch.is_none() {
        return Err("tùy chọn --track/-t chỉ được dùng đi kèm -c/--create".to_string());
    }

    if let Some(nb) = &new_branch {
        if !is_valid_git_ref_or_remote_name(nb) {
            return Err(format!("tên nhánh mới `{nb}` không hợp lệ"));
        }
        if is_protected_branch(nb) {
            return Err(format!("không được tạo nhánh {nb} (main/master được bảo vệ)"));
        }
        if !is_grant_mode && positionals.len() > 1 {
            return Err("git switch -c chỉ nhận tối đa 1 ref nguồn".to_string());
        }
    } else if !is_grant_mode && positionals.len() != 1 {
        return Err("git switch cần đúng 1 tên nhánh (hoặc dùng -c để tạo nhánh)".to_string());
    }

    for r in &positionals {
        if !is_valid_git_ref_or_remote_name(r) {
            return Err(format!("đối số `{r}` của git switch không hợp lệ"));
        }
    }

    Ok(())
}

/// The git option policy (W-1) on the words after `git`.
fn git_policy_check_with(
    args: &[ShellWord],
    root: Option<&Path>,
    is_grant_mode: bool,
) -> Result<(), String> {
    let mut i = 0;
    while i < args.len() && args[i].text.starts_with('-') {
        let opt = args[i].text.as_str();
        let allowed = opt == "-P" || opt.strip_prefix("--") == Some(GIT_GLOBAL_LONG_ALLOWED);
        if !allowed {
            return Err(format!("tùy chọn toàn cục của git {opt} không được phép"));
        }
        i += 1;
    }
    let sub = args.get(i).map(|w| w.text.as_str()).unwrap_or("");
    let is_allowed = GIT_READ_SUBCOMMANDS.contains(&sub)
        || GIT_WRITE_SUBCOMMANDS.contains(&sub)
        || sub == "fetch"
        || GIT_DEV_GRANT_SUBCOMMANDS.contains(&sub);
    if !is_allowed {
        return Err(format!(
            "git {sub} không được phép — chỉ dùng lệnh git chỉ đọc, git fetch, add/commit, hoặc push/pull/merge/checkout/switch khi plan khai; reset, rebase, restore, clean, stash, xoá nhánh/tag luôn bị cấm"
        ));
    }

    let rest = &args[i + 1..];

    if sub == "push" {
        return check_push(rest, is_grant_mode);
    }
    if sub == "pull" {
        return check_pull(rest, root, is_grant_mode);
    }
    if sub == "merge" {
        return check_merge(rest, root, is_grant_mode);
    }
    if sub == "checkout" {
        return check_checkout(rest, root, is_grant_mode);
    }
    if sub == "switch" {
        return check_switch(rest, root, is_grant_mode);
    }
    if sub == "fetch" {
        return check_fetch(rest);
    }
    if sub == "branch" {
        return check_branch(rest);
    }
    if sub == "tag" {
        return check_tag(rest);
    }
    if sub == "remote" {
        return check_remote(rest);
    }
    if sub == "reflog" {
        if let Some(first_pos) = rest.iter().find(|w| !w.text.starts_with('-') && w.text != "--") {
            if first_pos.text != "show" {
                return Err(format!(
                    "git reflog {} không được phép — chỉ cho phép `git reflog` hoặc `git reflog show`",
                    first_pos.text
                ));
            }
        }
    }

    let forbidden_long: &[&str] = match sub {
        "diff" | "show" | "whatchanged" | "range-diff" => GIT_DIFF_FORBIDDEN,
        "log" | "rev-list" | "shortlog" => GIT_LOG_LIKE_FORBIDDEN,
        "grep" => GIT_GREP_FORBIDDEN,
        "blame" => GIT_BLAME_FORBIDDEN,
        "cat-file" => GIT_CAT_FILE_FORBIDDEN,
        "commit" => GIT_COMMIT_FORBIDDEN,
        "add" => GIT_ADD_FORBIDDEN,
        _ => &[],
    };

    let rest = &args[i + 1..];
    let mut j = 0;
    // `git commit` needs a message source (-m, --message, -C,
    // --reuse-message, plain --fixup); without one git opens an editor.
    let mut has_message = false;
    // after `--` every word is a pathspec: it is still refused when it
    // looks like a forbidden option, but never counts as a message source
    let mut after_dashdash = false;
    while j < rest.len() {
        let word = rest[j].text.as_str();
        j += 1;
        if word == "--" {
            // pathspec separator: keep scanning (a later option-like
            // pathspec is refused too, which is safe)
            after_dashdash = true;
            continue;
        }
        if let Some(long) = word.strip_prefix("--") {
            let mut parts = long.splitn(2, '=');
            let name = parts.next().unwrap_or("");
            let value = parts.next();
            if name.is_empty() || GIT_LONG_EXACT_ALLOWED.contains(&name) {
                continue;
            }
            // git accepts every unambiguous abbreviation of a long option,
            // so any non-empty prefix of a forbidden name counts.
            if forbidden_long.iter().any(|f| f.starts_with(name)) {
                return Err(git_forbidden_err(sub, word));
            }
            if sub == "commit" && !after_dashdash {
                if long_abbrev_of(name, "message", 3) || long_abbrev_of(name, "reuse-message", 3) {
                    has_message = true;
                } else if long_abbrev_of(name, "fixup", 3) {
                    // `--fixup=<commit>` or `--fixup <commit>`
                    let value = value.or_else(|| rest.get(j).map(|w| w.text.as_str()));
                    if plain_fixup_value(value) {
                        has_message = true;
                    }
                }
            }
            continue;
        }
        let Some(cluster) = word.strip_prefix('-') else {
            continue;
        };
        if cluster.is_empty() {
            continue;
        }
        match sub {
            "grep" => {
                if cluster.contains(['O', 'f']) {
                    return Err(git_forbidden_err(sub, word));
                }
            }
            "blame" => {
                if cluster.contains('S') {
                    return Err(git_forbidden_err(sub, word));
                }
            }
            "commit" => {
                for (k, ch) in cluster.char_indices() {
                    match ch {
                        // -F / -t read a file; -p / -e / -c open a prompt
                        // or an editor
                        'F' | 't' | 'p' | 'e' | 'c' => {
                            return Err(git_forbidden_err(sub, word));
                        }
                        'm' => {
                            if !after_dashdash {
                                has_message = true;
                            }
                            // the rest of the cluster is the message; when
                            // `m` ends the cluster the next word is
                            if k + ch.len_utf8() == cluster.len() {
                                j += 1;
                            }
                            break;
                        }
                        'C' => {
                            // reuses a commit's message, no editor; takes
                            // an attached value: stop the scan (the next
                            // word is still checked)
                            if !after_dashdash {
                                has_message = true;
                            }
                            break;
                        }
                        // these take an attached value: stop the scan
                        // (the next word is still checked)
                        'u' | 'S' => break,
                        _ => {}
                    }
                }
            }
            "add" => {
                // -f adds ignored files; -p / -i / -e open a prompt or an
                // editor
                if cluster.contains(['f', 'p', 'i', 'e']) {
                    return Err(git_forbidden_err(sub, word));
                }
            }
            _ => {}
        }
    }
    if sub == "commit" && !has_message && !is_grant_mode {
        return Err(GIT_COMMIT_NO_MESSAGE_ERR.to_string());
    }
    Ok(())
}

/// Commands that run another command given as their arguments. A git call
/// behind one of them would escape the git policy, so it is refused.
const WRAPPER_COMMANDS: &[&str] = &[
    "!", "builtin", "command", "exec", "eval", "env", "time", "timeout", "nice", "nohup", "ionice",
    "stdbuf", "setsid", "sudo", "doas", "su", "runuser", "xargs", "find", "parallel", "watch",
    "script", "unbuffer", "flock", "chroot", "nsenter", "taskset", "chrt", "strace", "ltrace",
    "busybox", "sh", "bash", "dash", "zsh", "ksh", "mksh", "fish", "python", "python3", "perl",
    "ruby", "node",
];

/// Characters that start a shell construct outside quotes: command lists,
/// pipes, redirections, subshells, brace groups / brace expansion,
/// command substitution and parameter expansion.
const SHELL_OPERATORS: &[char] = &[';', '&', '|', '<', '>', '(', ')', '{', '}', '`', '$'];

const ONE_COMMAND_RULE: &str = "chỉ cho phép một lệnh git đơn trên một dòng";

/// One shell word after quote removal and unescaping. `glob` is set when
/// the word holds an unquoted `*`, `?` or `[`, which bash would expand
/// against file names after this check.
#[derive(Debug, Clone, PartialEq)]
struct ShellWord {
    text: String,
    glob: bool,
}

fn operator_err(c: char) -> String {
    format!("lệnh Bash có toán tử shell ({c}) — {ONE_COMMAND_RULE}")
}

fn unterminated_err() -> String {
    format!("lệnh Bash có dấu nháy chưa đóng — {ONE_COMMAND_RULE}")
}

/// Splits a Bash command the way bash would for one simple command, or
/// refuses it when it holds anything beyond one plain command: a newline
/// (inside quotes too), an unquoted operator / redirection / brace,
/// `$` or backtick expansion (also inside double quotes), a comment, an
/// unterminated quote or a trailing backslash.
fn shell_words(command: &str) -> Result<Vec<ShellWord>, String> {
    if command.contains('\n') || command.contains('\r') {
        return Err(format!("lệnh Bash có xuống dòng — {ONE_COMMAND_RULE}"));
    }
    if command.chars().any(|c| c.is_control() && c != '\t') {
        return Err(format!(
            "lệnh Bash có ký tự điều khiển — {ONE_COMMAND_RULE}"
        ));
    }

    let mut words = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    let mut glob = false;
    let mut chars = command.chars();

    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' => {
                if started {
                    words.push(ShellWord {
                        text: std::mem::take(&mut cur),
                        glob,
                    });
                    started = false;
                    glob = false;
                }
            }
            '\\' => match chars.next() {
                Some(n) => {
                    cur.push(n);
                    started = true;
                }
                None => {
                    return Err(format!(
                        "lệnh Bash kết thúc bằng dấu \\ — {ONE_COMMAND_RULE}"
                    ));
                }
            },
            '\'' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(n) => cur.push(n),
                        None => return Err(unterminated_err()),
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(n @ ('"' | '\\' | '$' | '`')) => cur.push(n),
                            Some(n) => {
                                cur.push('\\');
                                cur.push(n);
                            }
                            None => return Err(unterminated_err()),
                        },
                        Some(n @ ('$' | '`')) => return Err(operator_err(n)),
                        Some(n) => cur.push(n),
                        None => return Err(unterminated_err()),
                    }
                }
            }
            '#' if !started => return Err(operator_err('#')),
            c if SHELL_OPERATORS.contains(&c) => return Err(operator_err(c)),
            '*' | '?' | '[' => {
                glob = true;
                started = true;
                cur.push(c);
            }
            _ => {
                started = true;
                cur.push(c);
            }
        }
    }
    if started {
        words.push(ShellWord { text: cur, glob });
    }
    Ok(words)
}

/// `NAME=…`, `NAME+=…` or `NAME[i]=…`: bash treats a leading word of this
/// shape as an environment assignment for the command that follows.
fn is_env_assignment(word: &str) -> bool {
    let b = word.as_bytes();
    if b.is_empty() || !(b[0].is_ascii_alphabetic() || b[0] == b'_') {
        return false;
    }
    let mut i = 1;
    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
        i += 1;
    }
    if i < b.len() && b[i] == b'[' {
        match b[i..].iter().position(|&x| x == b']') {
            Some(p) => i += p + 1,
            None => return false,
        }
    }
    if i < b.len() && b[i] == b'+' {
        i += 1;
    }
    i < b.len() && b[i] == b'='
}

fn word_basename(word: &str) -> String {
    Path::new(word)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
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
    source_roots: &[PathBuf],
) -> BrainResult<PathBuf> {
    let grants = GuardGrants {
        source_roots: source_roots.to_vec(),
        ..GuardGrants::default()
    };
    write_hook_settings_with_matcher(
        run_dir,
        root,
        read_allow,
        write_allow,
        exe,
        GUARD_MATCHER,
        &grants,
    )
}

/// `write_hook_settings` plus the directory and command grants of an approved plan (D-49).
pub fn write_hook_settings_grants(
    run_dir: &Path,
    root: &Path,
    read_allow: Option<&[PathBuf]>,
    write_allow: &[PathBuf],
    exe: &Path,
    grants: &GuardGrants,
    source_roots: &[PathBuf],
) -> BrainResult<PathBuf> {
    let mut g = grants.clone();
    for sr in source_roots {
        if !g.source_roots.contains(sr) {
            g.source_roots.push(sr.clone());
        }
    }
    write_hook_settings_with_matcher(
        run_dir,
        root,
        read_allow,
        write_allow,
        exe,
        GUARD_MATCHER,
        &g,
    )
}

/// Settings whose only PreToolUse matcher is `Bash`: the git policy for a
/// role that has no write guard (Lead, QA). File tools stay with claude's
/// own allow rules.
pub fn write_bash_hook_settings(run_dir: &Path, root: &Path, exe: &Path) -> BrainResult<PathBuf> {
    write_hook_settings_with_matcher(
        run_dir,
        root,
        None,
        &[],
        exe,
        BASH_MATCHER,
        &GuardGrants::default(),
    )
}

fn write_hook_settings_with_matcher(
    run_dir: &Path,
    root: &Path,
    read_allow: Option<&[PathBuf]>,
    write_allow: &[PathBuf],
    exe: &Path,
    matcher: &str,
    grants: &GuardGrants,
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

    let mut policy_data = serde_json::json!({
        "root": root_resolved.to_string_lossy(),
        "read_allow": norm_read,
        "write_allow": norm_write,
        "log": log_path.to_string_lossy(),
    });
    // new fields only when a plan grants something: a run without grants writes the old policy
    if !grants.read_files.is_empty() {
        policy_data["read_files"] = serde_json::json!(grants
            .read_files
            .iter()
            .map(|p| realpath_nfc(p).to_string_lossy().to_string())
            .collect::<Vec<_>>());
    }
    if !grants.read_dirs.is_empty() {
        policy_data["read_dirs"] = serde_json::json!(grants
            .read_dirs
            .iter()
            .map(|p| realpath_nfc(p).to_string_lossy().to_string())
            .collect::<Vec<_>>());
    }
    if !grants.write_files.is_empty() {
        policy_data["write_files"] = serde_json::json!(grants
            .write_files
            .iter()
            .map(|p| realpath_nfc(p).to_string_lossy().to_string())
            .collect::<Vec<_>>());
    }
    if !grants.write_dirs.is_empty() {
        policy_data["write_dirs"] = serde_json::json!(grants
            .write_dirs
            .iter()
            .map(|p| realpath_nfc(p).to_string_lossy().to_string())
            .collect::<Vec<_>>());
    }
    if let Some(list) = &grants.bash_allow {
        policy_data["bash_allow"] = serde_json::json!(list);
    }
    if !grants.source_roots.is_empty() {
        policy_data["source_roots"] = serde_json::json!(grants
            .source_roots
            .iter()
            .map(|p| realpath_nfc(p).to_string_lossy().to_string())
            .collect::<Vec<_>>());
    }

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
                    "matcher": matcher,
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

/// Reads a run's `policy.json` (written by `write_hook_settings*`). `Err` is the text shown after
/// `agent-brain guard: `.
pub fn load_policy(policy_path: &Path) -> Result<Policy, String> {
    let policy_str =
        std::fs::read_to_string(policy_path).map_err(|e| format!("không thể đọc policy: {e}"))?;

    let policy_val: serde_json::Value =
        serde_json::from_str(&policy_str).map_err(|e| format!("không thể đọc policy: {e}"))?;

    let policy_obj = policy_val
        .as_object()
        .ok_or_else(|| "policy không phải object".to_string())?;

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

    let path_list = |key: &str| -> Vec<PathBuf> {
        policy_obj
            .get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(PathBuf::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    // a missing or null `bash_allow` is the old policy; any other shape fails closed (nothing
    // but git is allowed)
    let bash_allow = match policy_obj.get("bash_allow") {
        None | Some(serde_json::Value::Null) => None,
        Some(v) => Some(
            v.as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        ),
    };

    Ok(Policy {
        root,
        read_allow,
        write_allow,
        log,
        read_dirs: path_list("read_dirs"),
        write_dirs: path_list("write_dirs"),
        read_files: path_list("read_files"),
        write_files: path_list("write_files"),
        bash_allow,
        source_roots: path_list("source_roots"),
    })
}

pub fn cmd_guard(policy: Option<&Path>) -> i32 {
    let policy_path = match policy {
        Some(p) => p,
        None => {
            crate::term::emit("agent-brain guard: thiếu đối số --policy");
            return 2;
        }
    };

    let pol = match load_policy(policy_path) {
        Ok(p) => p,
        Err(e) => {
            crate::term::emit(&format!("agent-brain guard: {e}"));
            return 2;
        }
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

    if !decision.allow && decision.tool == "Bash" {
        crate::term::emit(&format!(
            "agent-brain guard: chặn lệnh Bash '{}' — {}",
            decision.path, decision.reason
        ));
        2
    } else if !decision.allow {
        let display_path = if decision.path.is_empty() {
            "<none>"
        } else {
            &decision.path
        };
        crate::term::emit(&format!(
            "agent-brain guard: chặn {} {} — ngoài danh sách được phép của vai trò (xem file plan của task)",
            decision.tool, display_path
        ));
        2
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_table(allowed: &[&str], refused: &[&str]) {
        for cmd in allowed {
            assert!(
                bash_command_check(cmd).is_ok(),
                "expected allowed: {cmd:?} -> {:?}",
                bash_command_check(cmd)
            );
        }
        for cmd in refused {
            assert!(
                bash_command_check(cmd).is_err(),
                "expected refused: {cmd:?}"
            );
        }
    }

    #[test]
    fn bash_shell_table() {
        let allowed = [
            "git status --porcelain",
            "git diff abc123..HEAD -- . ':(exclude).plan.md' ':(exclude).qa-report.md'",
            "git log --format='%H %s' -5",
            "git show HEAD:.plan-review.md",
            "git show 'HEAD@{1}'",
            "git commit -m \"sua-app: sửa app\"",
            "git commit -m sua-app: sửa app",
            "\"git\" status",
            "/usr/bin/git status",
            "ls -la",
            // extra: an escaped `$` inside double quotes is literal; a non-git
            // glob and a `#` inside a word are left to claude's own rules
            "git log \"a\\$b\"",
            "ls *.py",
            "git log a#b",
            "",
            "   ",
        ];
        let refused = [
            "git log > out.txt",
            "git log < in.txt",
            "git diff | sh",
            "git log; rm -rf x",
            "git log && id",
            "git log & id",
            "git log $(id)",
            "git log $HOME",
            "git log \"$HOME\"",
            "git log `id`",
            "git log (x)",
            "git diff {--output=../x,HEAD}",
            "git log #x",
            "git log \"a",
            "git log 'a",
            "git commit -m \"a\nb\"",
            "git status\nid",
            "git status \\\n--output=x",
            "GIT_EXTERNAL_DIFF=x git diff",
            "X=1 ls",
            // extra (stricter than the plan's table)
            "git status\r",
            "git log \"`id`\"",
            "git status \\",
            "X+=1 git status",
            "X[0]=1 git status",
            "git show HEAD@{1}",
            "git diff --outpu?=x",
            "git add src/*.py",
            "git diff '--output=x'",
            "git diff \\-\\-output=x",
            "timeout 5 git diff --output=x",
            "env git status",
            "command git status",
            "sh -c 'git diff --output=x'",
            "xargs git diff",
            "/usr/lib/git-core/git-diff --output=x",
            "git log \u{7}",
        ];
        check_table(&allowed, &refused);

        // quote removal and unescaping give the words bash would pass on
        let w = shell_words("git log --format='%H %s' \"a\\\"b\" c\\ d '' x").unwrap();
        let texts: Vec<&str> = w.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["git", "log", "--format=%H %s", "a\"b", "c d", "", "x"]
        );
        let err = bash_command_check("git log && id").unwrap_err();
        assert!(err.contains("toán tử shell (&)"), "{err}");
    }

    #[test]
    fn bash_git_option_table() {
        let allowed = [
            "git status --porcelain",
            "git diff abc123..HEAD -- . ':(exclude).plan.md'",
            "git --no-pager diff",
            "git -P log",
            "git log -c",
            "git log --oneline",
            "git log --output-indicator-new=+",
            "git diff --text",
            "git diff --no-ext-diff --no-textconv",
            "git add src/app.py",
            "git add -A",
            "git commit -m \"sua-app: sửa app\"",
            "git commit -m -Fix-me",
            "git commit -mF",
            "git commit -am \"x\"",
            "git commit --fixup=HEAD",
            // extra
            "git --no-pager -P log --format='%H %s' -5",
            "git log --format='%H %s' -5",
            "git show HEAD:.plan-review.md",
            "git commit -m --file=/x",
            "git commit -C HEAD",
            "git diff --stat",
            "git add -u",
            // 05.2-32: message sources and harmless commit / add forms
            "git commit -m \"x\"",
            "git commit -am \"x\"",
            "git commit --message=\"x\"",
            "git commit -C HEAD",
            "git commit --allow-empty -m \"x\"",
            "git add -A",
            "git add -u",
            "git add src/a.py",
            // extra
            "git commit --message x",
            "git commit --mes=x",
            "git commit --reuse-message=HEAD",
            "git commit -i -m x src/a.py",
            "git commit --include -m x src/a.py",
            "git commit --all --quiet --no-verify --author=\"A <a@b>\" -m x",
            "git commit --only -m x src/a.py",
            "git commit --no-edit -m x",
            "git commit -m --amend",
            "git commit --fixup HEAD",
            "git add --all",
            "git add --intent-to-add src/a.py",
        ];
        let refused = [
            "git diff --output=/tmp/x",
            "git diff --output /tmp/x",
            "git log --outp=/tmp/x",
            "git log --o=/tmp/x",
            "git diff '--output=x'",
            "git diff \\-\\-output=x",
            "git show --ext-diff HEAD",
            "git show --ext HEAD",
            "git log -p --textconv",
            "git log --textc",
            "git diff --no-index a b",
            "git diff --no-i a b",
            "git -c core.pager=sh log",
            "git -C /etc log",
            "git --git-dir=/x log",
            "git --work-tree=/x status",
            "git --exec-path=/x log",
            "git --config-env=a=b log",
            "git push",
            "git config core.pager sh",
            "git commit -F /etc/passwd",
            "git commit --file=/x",
            "git commit --fil=/x",
            "git commit -aF /x",
            "git commit -Fm x",
            "git commit -t /x",
            "git commit --template=/x",
            "git commit --te=/x",
            "git add -f .env",
            "git add --force .env",
            "git add --f .env",
            "git add -fA",
            "git add --pathspec-from-file=/x",
            // extra
            "git",
            "git --no-pager",
            "git -p log",
            "git --paginate log",
            "git --no-pager=x diff",
            "git --o diff",
            "git log -- --output=x",
            "git diff --no- a b",
            "git show --t HEAD",
            "git commit -um -F /x",
            "git commit -m -- -F /x",
            "git commit -C -F /x",
            "git commit --message x -F /x",
            "git commit --pathspec-from-file=/x",
            "git add -Af .env",
            "git add --pathspec-f=/x",
            "git Diff",
            // 05.2-32: forms that open an editor or a prompt, and --amend
            "git add -p",
            "git add -i src/a.py",
            "git add --patch",
            "git add --inter",
            "git add -e x",
            "git commit",
            "git commit -a",
            "git commit --amend -m \"x\"",
            "git commit --am -m \"x\"",
            "git commit -em \"x\"",
            "git commit -p -m \"x\"",
            "git commit -c HEAD",
            "git commit --reedit-message=HEAD",
            // extra
            "git add -Ap",
            "git add --edit x",
            "git add --interactive",
            "git commit --amend --no-edit",
            "git commit --amend -C HEAD",
            "git commit -m x --amend",
            "git commit --edit -m x",
            "git commit --e -m x",
            "git commit --patch -m x",
            "git commit --interactive -m x",
            "git commit --re=HEAD",
            "git commit -ac HEAD",
            "git commit -ap -m x",
            "git commit --me",
            "git commit --squash=HEAD",
            "git commit --fixup=amend:HEAD",
            "git commit --fixup=reword:HEAD",
            "git commit --fixup amend:HEAD",
            "git commit --fixup",
            "git commit -- -m x",
            "git commit src/a.py",
        ];
        check_table(&allowed, &refused);

        assert_eq!(
            bash_command_check("git commit -a").unwrap_err(),
            "git commit cần -m \"…\" (một dòng); không mở trình soạn thảo"
        );

        assert_eq!(
            bash_command_check("git reset").unwrap_err(),
            "git reset không được phép — chỉ dùng lệnh git chỉ đọc, git fetch, add/commit, hoặc push/pull/merge/checkout/switch khi plan khai; reset, rebase, restore, clean, stash, xoá nhánh/tag luôn bị cấm"
        );
        assert_eq!(
            bash_command_check("git push").unwrap_err(),
            "git push cần remote và refspec (ví dụ git push origin <nhánh>)"
        );
        assert_eq!(
            bash_command_check("git -c a=b log").unwrap_err(),
            "tùy chọn toàn cục của git -c không được phép"
        );
        assert_eq!(
            bash_command_check("git diff --ext-diff").unwrap_err(),
            "git diff --ext-diff ghi file hoặc chạy chương trình ngoài — không được phép"
        );
    }

    // ---- skill mode (D-49): grants ----

    fn grant_policy(root: &Path) -> Policy {
        Policy {
            root: root.to_path_buf(),
            read_allow: Some(vec![root.join(".plan.md")]),
            write_allow: vec![root.join(".plan.md")],
            log: None,
            read_dirs: vec![root.join("scripts")],
            write_dirs: vec![root.join("out")],
            read_files: vec![],
            write_files: vec![],
            bash_allow: Some(vec!["python scripts/make.py *".to_string()]),
            source_roots: vec![],
        }
    }

    fn file_payload(tool: &str, root: &Path, rel: &str) -> serde_json::Value {
        serde_json::json!({
            "tool_name": tool,
            "tool_input": { "file_path": root.join(rel).to_string_lossy() },
            "cwd": root.to_string_lossy(),
        })
    }

    fn bash_payload(cmd: &str) -> serde_json::Value {
        serde_json::json!({ "tool_name": "Bash", "tool_input": { "command": cmd } })
    }

    #[test]
    fn grant_dirs_allow_inside_and_block_outside() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("out/sub")).unwrap();
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        let pol = grant_policy(&root);
        assert!(decide(&pol, &file_payload("Write", &root, "out/sub/new.txt")).allow);
        assert!(decide(&pol, &file_payload("Edit", &root, "out/a/b/c.txt")).allow);
        assert!(!decide(&pol, &file_payload("Write", &root, "other/x.txt")).allow);
        assert!(!decide(&pol, &file_payload("Write", &root, "scripts/make.py")).allow);
        assert!(decide(&pol, &file_payload("Read", &root, "scripts/make.py")).allow);
        assert!(decide(&pol, &file_payload("Read", &root, "out/x")).allow);
        assert!(!decide(&pol, &file_payload("Read", &root, "src/x")).allow);
        // the plan files still work
        assert!(decide(&pol, &file_payload("Write", &root, ".plan.md")).allow);
        // `..` out of the directory
        let dd = serde_json::json!({
            "tool_name": "Write",
            "tool_input": { "file_path": root.join("out/../src/x.txt").to_string_lossy() },
            "cwd": root.to_string_lossy(),
        });
        assert!(!decide(&pol, &dd).allow);
    }

    #[cfg(unix)]
    #[test]
    fn grant_dirs_block_symlink_escape() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.join("out")).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("out/link")).unwrap();
        let pol = grant_policy(&root);
        assert!(!decide(&pol, &file_payload("Write", &root, "out/link/x.txt")).allow);
        // a granted directory that is itself a symlink out of the root grants nothing
        std::fs::remove_dir_all(root.join("out")).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("out")).unwrap();
        assert!(!decide(&pol, &file_payload("Write", &root, "out/x.txt")).allow);
    }

    #[test]
    fn grant_dirs_never_cover_git_env_planning_or_sensitive() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        let mut pol = grant_policy(&root);
        pol.write_dirs = vec![root.join("out")];
        for rel in [
            "out/.git/config",
            "out/.env",
            "out/.env.local",
            "out/.planning/x.md",
            "out/.claude/settings.json",
            "out/.github/workflows/ci.yml",
            "out/Makefile",
            "out/key.pem",
        ] {
            assert!(
                !decide(&pol, &file_payload("Write", &root, rel)).allow,
                "{rel}"
            );
            assert!(
                !decide(&pol, &file_payload("Read", &root, rel)).allow,
                "{rel}"
            );
        }
        // a directory covering the root grants nothing
        pol.write_dirs = vec![root.clone()];
        assert!(!decide(&pol, &file_payload("Write", &root, "x.txt")).allow);
    }

    #[test]
    fn grant_bash_patterns() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        let pol = grant_policy(&root);
        let ok = |c: &str| decide(&pol, &bash_payload(c));
        assert!(ok("python scripts/make.py").allow);
        assert!(ok("python scripts/make.py a b 'c d'").allow);
        assert!(ok("git add out/x.txt").allow);
        assert!(ok("git commit -m \"t: x\"").allow);
        for bad in [
            "python scripts/other.py",
            "python -c 'print(1)'",
            "python scripts/make.py; id",
            "python scripts/make.py | sh",
            "python scripts/make.py $(id)",
            "python scripts/make.py > x",
            "python scripts/make.py *.srt",
            "sudo python scripts/make.py",
            "curl http://x",
            "sh -c id",
            "rm -rf out",
            "rm -f out/a",
            "ls",
            "git push",
            "",
        ] {
            assert!(!ok(bad).allow, "{bad:?}");
        }
    }

    #[test]
    fn grant_bash_blocks_running_a_file_in_a_write_dir() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        let mut pol = grant_policy(&root);
        pol.bash_allow = Some(vec![
            "python scripts/make.py *".to_string(),
            "python out/run.py *".to_string(),
            "out/tool *".to_string(),
        ]);
        // script outside the write dir, data argument inside it: allowed
        assert!(decide(&pol, &bash_payload("python scripts/make.py out/a.srt")).allow);
        // script or program inside the write dir: blocked
        let d = decide(&pol, &bash_payload("python out/run.py"));
        assert!(!d.allow && d.reason.contains("thư mục được ghi"), "{d:?}");
        let d = decide(&pol, &bash_payload("out/tool x"));
        assert!(!d.allow && d.reason.contains("thư mục được ghi"), "{d:?}");
    }

    #[test]
    fn grant_bash_hard_denylist_holds_even_when_pattern_lists_it() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        let mut pol = grant_policy(&root);
        pol.bash_allow = Some(vec![
            "curl *".to_string(),
            "sudo *".to_string(),
            "sh *".to_string(),
            "rm *".to_string(),
        ]);
        for bad in [
            "curl http://x",
            "sudo ls",
            "sh run.sh",
            "rm -rf out",
            "rm -r out",
        ] {
            assert!(!decide(&pol, &bash_payload(bad)).allow, "{bad:?}");
        }
        assert!(decide(&pol, &bash_payload("rm out/a.txt")).allow);
    }

    #[test]
    fn policy_without_new_fields_behaves_as_before() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        let mut pol = grant_policy(&root);
        pol.read_dirs.clear();
        pol.write_dirs.clear();
        pol.bash_allow = None;
        assert!(!decide(&pol, &file_payload("Write", &root, "out/x.txt")).allow);
        assert!(!decide(&pol, &file_payload("Read", &root, "scripts/x")).allow);
        assert!(decide(&pol, &bash_payload("ls")).allow);
        assert!(decide(&pol, &bash_payload("python anything.py")).allow);
        assert!(!decide(&pol, &bash_payload("git push")).allow);
    }

    #[test]
    fn policy_file_round_trip_old_and_new() {
        let td = tempfile::tempdir().unwrap();
        let run = td.path().join("run");
        let root = td.path().join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let exe = std::env::current_exe().unwrap();
        write_hook_settings(&run, &root, None, &[], &exe, &[]).unwrap();
        let old: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(run.join("policy.json")).unwrap())
                .unwrap();
        assert!(old.get("read_dirs").is_none());
        assert!(old.get("write_dirs").is_none());
        assert!(old.get("bash_allow").is_none());
        let g = GuardGrants {
            read_dirs: vec![root.join("a")],
            write_dirs: vec![root.join("b")],
            bash_allow: Some(vec!["python x.py *".to_string()]),
            ..GuardGrants::default()
        };
        write_hook_settings_grants(&run, &root, None, &[], &exe, &g, &[]).unwrap();
        let new: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(run.join("policy.json")).unwrap())
                .unwrap();
        assert_eq!(new["bash_allow"][0], "python x.py *");
        assert!(new["write_dirs"][0].as_str().unwrap().ends_with("/b"));
    }

    // ---- single-file grants ----

    fn file_grant_policy(root: &Path) -> Policy {
        let mut pol = grant_policy(root);
        pol.read_dirs.clear();
        pol.write_dirs.clear();
        pol.read_files = vec![root.join("scripts/create.py"), root.join(".env")];
        pol.write_files = vec![root.join("out/result.txt")];
        pol
    }

    #[test]
    fn grant_read_file_allows_only_that_file() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::write(root.join("scripts/create.py"), "x").unwrap();
        std::fs::write(root.join("scripts/other.py"), "x").unwrap();
        let pol = file_grant_policy(&root);
        assert!(decide(&pol, &file_payload("Read", &root, "scripts/create.py")).allow);
        assert!(!decide(&pol, &file_payload("Read", &root, "scripts/other.py")).allow);
        assert!(!decide(&pol, &file_payload("Read", &root, "scripts")).allow);
        assert!(!decide(&pol, &file_payload("Read", &root, "other.py")).allow);
        assert!(!decide(&pol, &file_payload("Write", &root, "scripts/create.py")).allow);
    }

    #[test]
    fn grant_write_file_allows_only_that_file() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("out")).unwrap();
        let pol = file_grant_policy(&root);
        assert!(decide(&pol, &file_payload("Write", &root, "out/result.txt")).allow);
        assert!(decide(&pol, &file_payload("Read", &root, "out/result.txt")).allow);
        assert!(!decide(&pol, &file_payload("Write", &root, "out/sibling.txt")).allow);
        assert!(!decide(&pol, &file_payload("Write", &root, "out")).allow);
        assert!(!decide(&pol, &file_payload("Write", &root, "result.txt")).allow);
    }

    #[test]
    fn grant_file_entries_never_cover_sensitive_paths() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join(".planning")).unwrap();
        let mut pol = file_grant_policy(&root);
        pol.read_files.extend([
            root.join(".git/config"),
            root.join(".planning/STATE.md"),
            root.join("keys/id_rsa"),
            root.clone(),
        ]);
        pol.write_files
            .extend([root.join(".env.local"), root.join(".git/x")]);
        for rel in [".env", ".git/config", ".planning/STATE.md", "keys/id_rsa"] {
            assert!(
                !decide(&pol, &file_payload("Read", &root, rel)).allow,
                "{rel}"
            );
        }
        for rel in [".env.local", ".git/x"] {
            assert!(
                !decide(&pol, &file_payload("Write", &root, rel)).allow,
                "{rel}"
            );
        }
        assert!(!decide(&pol, &file_payload("Read", &root, "")).allow);
    }

    #[test]
    fn grant_file_symlink_escape_blocked() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("f.txt"), "x").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.path().join("f.txt"), root.join("link.txt"))
                .unwrap();
            let mut pol = grant_policy(&root);
            pol.read_files = vec![root.join("link.txt")];
            assert!(!decide(&pol, &file_payload("Read", &root, "link.txt")).allow);
        }
    }

    #[test]
    fn policy_file_round_trip_with_file_grants() {
        let td = tempfile::tempdir().unwrap();
        let run = td.path().join("run");
        let root = td.path().join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let exe = std::env::current_exe().unwrap();
        let g = GuardGrants {
            read_files: vec![root.join("a.md")],
            write_files: vec![root.join("b.md")],
            ..GuardGrants::default()
        };
        write_hook_settings_grants(&run, &root, None, &[], &exe, &g, &[]).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(run.join("policy.json")).unwrap())
                .unwrap();
        assert!(v["read_files"][0].as_str().unwrap().ends_with("/a.md"));
        assert!(v["write_files"][0].as_str().unwrap().ends_with("/b.md"));
        assert!(v.get("read_dirs").is_none());
    }

    #[test]
    fn agy_init_cwd_mismatch_compares_canonical_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("web");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let ev = |cwd: &str| serde_json::json!({"event": "init", "init": {"cwd": cwd}});
        assert_eq!(agy_init_cwd_mismatch(&ev(root.to_str().unwrap()), &root), None);
        let dotted = format!("{}/sub/..", root.display());
        assert_eq!(agy_init_cwd_mismatch(&ev(&dotted), &root), None);
        let sub = root.join("sub");
        assert_eq!(
            agy_init_cwd_mismatch(&ev(sub.to_str().unwrap()), &root),
            Some(sub.to_string_lossy().to_string())
        );
        assert_eq!(
            agy_init_cwd_mismatch(&serde_json::json!({"event": "init", "init": {}}), &root),
            None
        );
    }

    // ---- agy Dev tool steps ----

    fn agy_params(pairs: &[(&str, &str)]) -> serde_json::Map<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string())))
            .collect()
    }

    fn agy_allows(pol: &Policy, tool: &str, pairs: &[(&str, &str)]) -> bool {
        agy_tool_decision(pol, tool, &agy_params(pairs))
            .unwrap_or_else(|| panic!("{tool} must be checked"))
            .allow
    }

    #[test]
    fn agy_tool_payload_maps_onto_claude_tools() {
        let root = Path::new("/p");
        let p = agy_tool_payload(
            "run_command",
            &agy_params(&[("CommandLine", "git status")]),
            root,
        )
        .unwrap();
        assert_eq!(p["tool_name"], "Bash");
        assert_eq!(p["tool_input"]["command"], "git status");
        assert_eq!(p["cwd"], "/p");
        let p = agy_tool_payload(
            "run_command",
            &agy_params(&[("CommandLine", "ls"), ("Cwd", "/p/sub")]),
            root,
        )
        .unwrap();
        assert_eq!(p["cwd"], "/p/sub");

        for (agy, claude) in [
            ("read_file", "Read"),
            ("write_file", "Write"),
            ("edit_file", "Edit"),
        ] {
            let p = agy_tool_payload(agy, &agy_params(&[("Path", "/p/a.py")]), root).unwrap();
            assert_eq!(p["tool_name"], claude, "{agy}");
            assert_eq!(p["tool_input"]["file_path"], "/p/a.py", "{agy}");
        }
        let p = agy_tool_payload(
            "write_to_file",
            &agy_params(&[("TargetFile", "/p/b.py")]),
            root,
        )
        .unwrap();
        assert_eq!(p["tool_name"], "Write");
        assert_eq!(p["tool_input"]["file_path"], "/p/b.py");
        let p = agy_tool_payload(
            "view_file",
            &agy_params(&[("AbsolutePath", "/p/c.py")]),
            root,
        )
        .unwrap();
        assert_eq!(p["tool_name"], "Read");
        assert_eq!(p["tool_input"]["file_path"], "/p/c.py");
        let p = agy_tool_payload(
            "list_dir",
            &agy_params(&[("DirectoryPath", "/p/src")]),
            root,
        )
        .unwrap();
        assert_eq!(p["tool_name"], "Glob");
        assert_eq!(p["tool_input"]["path"], "/p/src");

        // a path tool without its path stays without one (decide refuses it)
        let p = agy_tool_payload("write_file", &agy_params(&[]), root).unwrap();
        assert!(p["tool_input"].get("file_path").is_none());

        // ask-only tools: nothing to check; any other tool keeps its name
        for t in AGY_ASK_TOOLS {
            assert!(agy_tool_payload(t, &agy_params(&[]), root).is_none(), "{t}");
        }
        let p =
            agy_tool_payload("manage_task", &agy_params(&[("Action", "status")]), root).unwrap();
        assert_eq!(p["tool_name"], "manage_task");
        assert_eq!(p["tool_input"]["Action"], "status");
    }

    #[test]
    fn agy_tool_steps_checked_against_the_plan() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::create_dir_all(root.join("out")).unwrap();
        let pol = grant_policy(&root);
        let r = |rel: &str| root.join(rel).to_string_lossy().to_string();

        // run_command: only a command of `## Lệnh được chạy`, one plain command
        assert!(!agy_allows(
            &pol,
            "run_command",
            &[("CommandLine", "python3 -c 'x'")]
        ));
        assert!(agy_allows(
            &pol,
            "run_command",
            &[("CommandLine", "python scripts/make.py --all")]
        ));
        assert!(!agy_allows(
            &pol,
            "run_command",
            &[("CommandLine", "cat << 'EOF' > /tmp/a.py\nimport os\nEOF")]
        ));
        assert!(agy_allows(
            &pol,
            "run_command",
            &[("CommandLine", "git status")]
        ));
        // ...run from the project root only
        assert!(!agy_allows(
            &pol,
            "run_command",
            &[("CommandLine", "python scripts/make.py a"), ("Cwd", "/tmp")]
        ));
        assert!(agy_allows(
            &pol,
            "run_command",
            &[
                ("CommandLine", "python scripts/make.py a"),
                ("Cwd", &root.to_string_lossy())
            ]
        ));

        // files: writes only to the Thay đổi files / write dirs, reads from the read dirs
        assert!(!agy_allows(&pol, "write_file", &[("Path", &r("src/x.py"))]));
        assert!(!agy_allows(
            &pol,
            "write_to_file",
            &[("TargetFile", &r("src/x.py"))]
        ));
        assert!(agy_allows(
            &pol,
            "write_file",
            &[("Path", &r("out/new.txt"))]
        ));
        assert!(agy_allows(&pol, "edit_file", &[("Path", &r(".plan.md"))]));
        assert!(agy_allows(
            &pol,
            "read_file",
            &[("Path", &r("scripts/make.py"))]
        ));
        assert!(!agy_allows(
            &pol,
            "read_file",
            &[("Path", &r("src/secret.py"))]
        ));
        assert!(!agy_allows(&pol, "write_file", &[]));
        // listing and searching: where the plan lets the Dev read (see the listing test)
        assert!(agy_allows(
            &pol,
            "list_dir",
            &[("DirectoryPath", &r("scripts"))]
        ));
        assert!(agy_allows(
            &pol,
            "grep_search",
            &[("SearchPath", &r("scripts"))]
        ));
        assert!(!agy_allows(
            &pol,
            "list_dir",
            &[("DirectoryPath", &r("src"))]
        ));

        // browser tools are refused, ask-only tools are not checked, other tools pass
        assert!(!agy_allows(
            &pol,
            "browser_navigate",
            &[("Url", "https://x")]
        ));
        assert!(agy_tool_decision(&pol, "ask_question", &agy_params(&[])).is_none());
        assert!(agy_allows(&pol, "manage_task", &[("Action", "status")]));
    }

    #[test]
    fn agy_policy_without_command_list_allows_only_git() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        // the policy of an agy Dev: the plan's grants are claude-only, so no command list
        let pol = Policy {
            bash_allow: None,
            read_dirs: vec![],
            write_dirs: vec![],
            ..grant_policy(&root)
        };
        // the commands of the 2026-11-05 incident (run 56)
        assert!(!agy_allows(
            &pol,
            "run_command",
            &[(
                "CommandLine",
                "python3 -c 'import PIL; print(PIL.__version__)'"
            )]
        ));
        assert!(!agy_allows(
            &pol,
            "run_command",
            &[(
                "CommandLine",
                "cat << 'EOF' > /tmp/generate_assets.py\nimport os\nEOF"
            )]
        ));
        assert!(!agy_allows(
            &pol,
            "run_command",
            &[("CommandLine", "ls -la")]
        ));
        for cmd in [
            "git status --short",
            "git add projects/a",
            "git commit -m \"t: x\"",
        ] {
            assert!(
                agy_allows(&pol, "run_command", &[("CommandLine", cmd)]),
                "{cmd}"
            );
        }
        // the hook decision for claude is unchanged: claude's own allow rules cover other commands
        assert!(decide(&pol, &bash_payload("ls -la")).allow);
    }

    #[test]
    fn agy_listing_inside_and_outside_granted_dirs() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        for d in ["scripts/sub", "out/a", "src/app", "other", ".git"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        // the plan's Thay đổi file `src/app/main.py`, read dir `scripts`, write dir `out`
        let pol = Policy {
            read_allow: Some(vec![root.join(".plan.md"), root.join("src/app/main.py")]),
            write_allow: vec![root.join(".plan.md"), root.join("src/app/main.py")],
            ..grant_policy(&root)
        };
        let r = |rel: &str| root.join(rel).to_string_lossy().to_string();
        let lists = |tool: &str, key: &str, rel: &str| agy_allows(&pol, tool, &[(key, &r(rel))]);

        // inside (or exactly) a granted read / write directory
        for rel in ["scripts", "scripts/sub", "out", "out/a"] {
            assert!(lists("list_dir", "DirectoryPath", rel), "{rel}");
            assert!(lists("find_by_name", "SearchDirectory", rel), "{rel}");
            assert!(lists("grep_search", "SearchPath", rel), "{rel}");
        }
        // a plan file, and (names only) the directory that holds it
        assert!(lists("grep_search", "SearchPath", "src/app/main.py"));
        assert!(lists("list_dir", "DirectoryPath", "src/app"));
        assert!(lists("find_by_name", "SearchDirectory", "src/app"));
        assert!(!lists("grep_search", "SearchPath", "src/app"));
        // outside: other directories, the root (only .plan.md lives there), parents, .git
        for rel in ["other", "src", "", ".git", "scripts/../other"] {
            assert!(!lists("list_dir", "DirectoryPath", rel), "{rel:?}");
        }
        assert!(!agy_allows(&pol, "list_dir", &[("DirectoryPath", "/etc")]));
        assert!(!agy_allows(&pol, "find_by_name", &[]), "no path = the root");
        // relative paths are read from the root
        assert!(agy_allows(
            &pol,
            "list_dir",
            &[("DirectoryPath", "scripts")]
        ));
        assert!(!agy_allows(&pol, "list_dir", &[("DirectoryPath", "other")]));

        // without limited reads the guard's own rule (read-free) is unchanged
        let free = Policy {
            read_allow: None,
            ..pol.clone()
        };
        assert!(agy_allows(
            &free,
            "list_dir",
            &[("DirectoryPath", &r("other"))]
        ));
    }

    #[test]
    fn agy_decisions_are_logged() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        let log = root.join("guard.jsonl");
        let pol = Policy {
            log: Some(log.clone()),
            ..grant_policy(&root)
        };
        assert!(!agy_allows(&pol, "run_command", &[("CommandLine", "id")]));
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("\"decision\":\"block\""), "{text}");
    }

    struct SoftReadEnv {
        _td: tempfile::TempDir,
        _outside_td: tempfile::TempDir,
        root: PathBuf,
        pol: Policy,
    }

    fn setup_soft_read_env() -> SoftReadEnv {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "hello").unwrap();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/b.txt"), "world").unwrap();
        std::fs::write(root.join(".env.x"), "SECRET=1").unwrap();

        let outside_td = tempfile::tempdir().unwrap();
        let outside_file = outside_td.path().canonicalize().unwrap().join("outside.env");
        std::fs::write(&outside_file, "OUTSIDE=1").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside_file, root.join("link.env")).unwrap();

        let pol = grant_policy(&root);
        SoftReadEnv {
            _td: td,
            _outside_td: outside_td,
            root,
            pol,
        }
    }

    fn soft_read(pol: &Policy, tool: &str, pairs: &[(&str, &str)]) -> Option<String> {
        agy_soft_read(pol, tool, &agy_params(pairs))
    }

    #[test]
    fn agy_soft_read_read_file_existing() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "read_file", &[("Path", "a.txt")]).is_some());
        assert!(soft_read(
            &env.pol,
            "read_file",
            &[("Path", &env.root.join("a.txt").to_string_lossy())]
        )
        .is_some());
    }

    #[test]
    fn agy_soft_read_read_file_nonexistent() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "read_file", &[("Path", "khong-ton-tai.json")]).is_some());
    }

    #[test]
    fn agy_soft_read_read_file_env() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "read_file", &[("Path", ".env.x")]).is_none());
    }

    #[test]
    fn agy_soft_read_read_file_outside_etc_passwd() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "read_file", &[("Path", "/etc/passwd")]).is_none());
    }

    #[test]
    fn agy_soft_read_read_file_symlink_outside() {
        let env = setup_soft_read_env();
        #[cfg(unix)]
        assert!(soft_read(&env.pol, "read_file", &[("Path", "link.env")]).is_none());
    }

    #[test]
    fn agy_soft_read_list_dir() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "list_dir", &[("DirectoryPath", "sub")]).is_some());
    }

    #[test]
    fn agy_soft_read_run_command_ls() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "ls -la sub")]).is_some());
    }

    #[test]
    fn agy_soft_read_run_command_cat_env() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "cat .env.x")]).is_none());
    }

    #[test]
    fn agy_soft_read_run_command_semicolon() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "ls; rm -rf x")]).is_none());
    }

    #[test]
    fn agy_soft_read_run_command_find_delete() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "find . -delete")]).is_none());
    }

    #[test]
    fn agy_soft_read_run_command_grep_pattern() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "grep -rn foo sub")]).is_some());
    }

    #[test]
    fn agy_soft_read_run_command_rm() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "rm a.txt")]).is_none());
    }

    #[test]
    fn agy_soft_read_write_file() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "write_file", &[("Path", "a.txt")]).is_none());
    }

    #[test]
    fn agy_soft_read_run_command_glob_blocked() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "cat .gsd/.e*")]).is_none());
    }

    #[test]
    fn agy_soft_read_run_command_symlink_following_flags_blocked() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "grep -Rn foo sub")]).is_none());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "rg --follow foo sub")]).is_none());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "find -L . -name x")]).is_none());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "grep -rn foo sub")]).is_some());
    }

    #[test]
    fn agy_soft_read_all_cases() {
        let env = setup_soft_read_env();
        assert!(soft_read(&env.pol, "read_file", &[("Path", "a.txt")]).is_some());
        assert!(soft_read(&env.pol, "read_file", &[("Path", "khong-ton-tai.json")]).is_some());
        assert!(soft_read(&env.pol, "read_file", &[("Path", ".env.x")]).is_none());
        assert!(soft_read(&env.pol, "read_file", &[("Path", "/etc/passwd")]).is_none());
        #[cfg(unix)]
        assert!(soft_read(&env.pol, "read_file", &[("Path", "link.env")]).is_none());
        assert!(soft_read(&env.pol, "list_dir", &[("DirectoryPath", "sub")]).is_some());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "ls -la sub")]).is_some());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "cat .env.x")]).is_none());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "cat .gsd/.e*")]).is_none());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "ls; rm -rf x")]).is_none());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "find . -delete")]).is_none());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "find -L . -name x")]).is_none());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "grep -rn foo sub")]).is_some());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "grep -Rn foo sub")]).is_none());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "rg --follow foo sub")]).is_none());
        assert!(soft_read(&env.pol, "run_command", &[("CommandLine", "rm a.txt")]).is_none());
        assert!(soft_read(&env.pol, "write_file", &[("Path", "a.txt")]).is_none());
    }

    /// A project repo with `node_modules/.bin/x`, `node_modules/pkg/package.json` and a sibling
    /// `secret/key.txt`, plus a worktree whose `node_modules` links to the repo's. Returns
    /// (repo, worktree), both canonical.
    fn linked_worktree(td: &Path) -> (PathBuf, PathBuf) {
        let repo = td.join("repo");
        let wt = td.join("wt");
        std::fs::create_dir_all(repo.join("node_modules/.bin")).unwrap();
        std::fs::create_dir_all(repo.join("node_modules/pkg")).unwrap();
        std::fs::create_dir_all(repo.join("secret")).unwrap();
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(repo.join("node_modules/.bin/x"), "#!/bin/sh\n").unwrap();
        std::fs::write(repo.join("node_modules/pkg/package.json"), "{}").unwrap();
        std::fs::write(repo.join("secret/key.txt"), "k").unwrap();
        std::fs::write(repo.join("package.json"), "{}").unwrap();
        let repo = repo.canonicalize().unwrap();
        let wt = wt.canonicalize().unwrap();
        std::os::unix::fs::symlink(repo.join("node_modules"), wt.join("node_modules")).unwrap();
        (repo, wt)
    }

    #[test]
    fn linked_node_modules_is_readable_through_the_symlink_only() {
        let td = tempfile::tempdir().unwrap();
        let (repo, wt) = linked_worktree(td.path());
        let pol = grant_policy(&wt);
        // reading through the symlink is allowed, and the reason says why
        let d = decide(&pol, &file_payload("Read", &wt, "node_modules/.bin/x"));
        assert!(d.allow, "{d:?}");
        assert_eq!(d.reason, "in linked node_modules");
        assert!(decide(&pol, &file_payload("Read", &wt, "node_modules/pkg/package.json")).allow);
        // a sibling of the symlink target in the repo is still refused
        assert!(!decide(&pol, &file_payload("Read", &repo, "secret/key.txt")).allow);
        assert!(!decide(&pol, &file_payload("Read", &repo, "package.json")).allow);
        assert!(!decide(&pol, &file_payload("Read", &wt, "node_modules/../secret/key.txt")).allow);
        // a folder beside node_modules in the repo is refused too
        assert!(!decide(&pol, &file_payload("Read", &repo, "secret")).allow);
        // writes into node_modules are refused, through the link or directly
        for tool in ["Write", "Edit"] {
            assert!(!decide(&pol, &file_payload(tool, &wt, "node_modules/.bin/x")).allow);
            assert!(!decide(&pol, &file_payload(tool, &wt, "node_modules/new.js")).allow);
            assert!(!decide(&pol, &file_payload(tool, &repo, "node_modules/new.js")).allow);
        }
        // Grep / Glob are unchanged: refused while read_allow is set
        for tool in ["Grep", "Glob"] {
            let payload = serde_json::json!({
                "tool_name": tool,
                "tool_input": { "path": wt.join("node_modules").to_string_lossy() },
                "cwd": wt.to_string_lossy(),
            });
            assert!(!decide(&pol, &payload).allow, "{tool}");
        }
        // denied components stay denied inside node_modules
        std::fs::create_dir_all(repo.join("node_modules/.git")).unwrap();
        std::fs::write(repo.join("node_modules/.git/config"), "x").unwrap();
        std::fs::write(repo.join("node_modules/pkg/.env"), "x").unwrap();
        assert!(!decide(&pol, &file_payload("Read", &wt, "node_modules/.git/config")).allow);
        assert!(!decide(&pol, &file_payload("Read", &wt, "node_modules/pkg/.env")).allow);
    }

    #[test]
    fn node_modules_grants_nothing_without_a_symlink() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
        std::fs::write(root.join("node_modules/.bin/x"), "x").unwrap();
        let pol = grant_policy(&root);
        // a real node_modules folder is not opened by this rule
        assert!(!decide(&pol, &file_payload("Read", &root, "node_modules/.bin/x")).allow);
    }

    #[test]
    fn symlinked_node_modules_to_a_foreign_folder_grants_nothing() {
        let td = tempfile::tempdir().unwrap();
        let root = td.path().join("wt");
        let other = td.path().join("elsewhere");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("f"), "x").unwrap();
        let root = root.canonicalize().unwrap();
        let other = other.canonicalize().unwrap();
        // the link target is not named node_modules (could even be `/`)
        std::os::unix::fs::symlink(&other, root.join("node_modules")).unwrap();
        let pol = grant_policy(&root);
        assert!(!decide(&pol, &file_payload("Read", &root, "node_modules/f")).allow);
    }

    #[test]
    fn load_policy_reads_a_written_policy() {
        let td = tempfile::tempdir().unwrap();
        let run = td.path().join("run");
        let root = td.path().join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let exe = std::env::current_exe().unwrap();
        let g = GuardGrants {
            bash_allow: Some(vec!["make test".to_string()]),
            ..GuardGrants::default()
        };
        write_hook_settings_grants(&run, &root, Some(&[root.join("a.md")]), &[], &exe, &g, &[]).unwrap();
        let pol = load_policy(&run.join("policy.json")).unwrap();
        assert_eq!(pol.bash_allow, Some(vec!["make test".to_string()]));
        assert_eq!(pol.read_allow.as_ref().map(|v| v.len()), Some(1));
        assert!(pol.log.is_some());
        assert!(load_policy(&run.join("missing.json")).is_err());
    }
}
