use crate::error::{BrainError, BrainResult};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Every git process agent-brain spawns. `GIT_OPTIONAL_LOCKS=0` keeps
/// background reads (`git status` from the dashboard and the Telegram bot)
/// from taking `.git/index.lock`, which would make a concurrent `git add` or
/// `git commit` from an agent run fail. Locks that write commands need are
/// not affected.
fn git_cmd(root: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(root);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
    if !root.join(".git").exists() {
        let shadow = shadow_dir(root);
        if shadow.is_dir() {
            cmd.env("GIT_DIR", shadow);
            cmd.env("GIT_WORK_TREE", root);
        }
    }
    cmd
}

/// 64-bit FNV-1a, written by hand so the hash stays stable across Rust versions.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Hidden git dir of an `idea` project: `<data_dir>/shadow/<16 hex>.git`.
pub fn shadow_dir(root: &Path) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    let canon = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let hash = fnv1a64(canon.as_os_str().as_bytes());
    crate::config::data_dir()
        .join("shadow")
        .join(format!("{hash:016x}.git"))
}

/// Serializes unit tests that point `XDG_DATA_HOME` at a temp dir.
#[cfg(test)]
pub static SHADOW_TEST_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

const SHADOW_MAX_FILES: u64 = 20_000;
const SHADOW_MAX_BYTES: u64 = 500 * 1024 * 1024;

/// Returns true when the folder is within the shadow size limits.
fn within_shadow_limits(root: &Path) -> bool {
    let mut files: u64 = 0;
    let mut bytes: u64 = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                let name = entry.file_name();
                if name == ".git" || name == "node_modules" || name == "target" {
                    continue;
                }
                stack.push(entry.path());
            } else {
                files += 1;
                bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
                if files > SHADOW_MAX_FILES || bytes > SHADOW_MAX_BYTES {
                    return false;
                }
            }
        }
    }
    true
}

/// Creates the hidden shadow repo of an `idea` project folder (no-op when the
/// folder has a real `.git` or the shadow already exists).
pub fn ensure_shadow(root: &Path) -> BrainResult<()> {
    if root.join(".git").exists() {
        return Ok(());
    }
    let shadow = shadow_dir(root);
    if shadow.exists() {
        return Ok(());
    }
    if !within_shadow_limits(root) {
        return Err(BrainError::error(format!(
            "Thư mục ý tưởng '{}' quá lớn để theo dõi (>20000 file hoặc >500 MiB) — tách phần tài liệu nhỏ ra hoặc dùng git thật (git init).",
            root.display()
        )));
    }
    let result = create_shadow(root, &shadow);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&shadow);
    }
    result
}

fn create_shadow(root: &Path, shadow: &Path) -> BrainResult<()> {
    let shadow_s = shadow.to_string_lossy().to_string();
    let run_plain = |args: &[&str]| -> BrainResult<()> {
        let out = std::process::Command::new("git")
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|e| BrainError::error(format!("Lỗi chạy git: {e}")))?;
        if !out.status.success() {
            return Err(BrainError::error(format!(
                "Không tạo được repo ẩn: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(())
    };
    if let Some(parent) = shadow.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| BrainError::error(format!("Không tạo được thư mục shadow: {e}")))?;
    }
    let gd = format!("--git-dir={shadow_s}");
    run_plain(&[&gd, "init", "-q", "-b", "main"])?;
    run_plain(&[&gd, "config", "user.name", "agent-brain"])?;
    run_plain(&[&gd, "config", "user.email", "agent-brain@localhost"])?;
    let info = shadow.join("info");
    std::fs::create_dir_all(&info)
        .map_err(|e| BrainError::error(format!("Không tạo được info/exclude: {e}")))?;
    std::fs::write(
        info.join("exclude"),
        ".env\n.env.*\n*.pem\n*.key\nid_rsa*\nnode_modules/\ntarget/\n",
    )
    .map_err(|e| BrainError::error(format!("Không ghi được info/exclude: {e}")))?;
    git(root, &["add", "-A"])?;
    git(
        root,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "agent-brain: snapshot ban dau",
        ],
    )?;
    Ok(())
}

pub fn git(root: &Path, args: &[&str]) -> BrainResult<String> {
    let mut cmd = git_cmd(root);
    cmd.args(args);

    let output = cmd
        .output()
        .map_err(|e| BrainError::error(format!("Lỗi chạy git: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let msg = if !stderr.is_empty() {
            stderr
        } else {
            let code_str = output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string());
            format!(
                "Lệnh git thất bại (exit {code_str}): git {}",
                args.join(" ")
            )
        };
        return Err(BrainError::error(msg));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn git_bytes(root: &Path, args: &[&str]) -> BrainResult<Vec<u8>> {
    let mut cmd = git_cmd(root);
    cmd.args(args);

    let output = cmd
        .output()
        .map_err(|e| BrainError::error(format!("Lỗi chạy git: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let msg = if !stderr.is_empty() {
            stderr
        } else {
            let code_str = output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string());
            format!(
                "Lệnh git thất bại (exit {code_str}): git {}",
                args.join(" ")
            )
        };
        return Err(BrainError::error(msg));
    }
    Ok(output.stdout)
}

pub fn show_file_bytes(root: &Path, rev: &str, path: &str) -> BrainResult<Vec<u8>> {
    let spec = format!("{rev}:{path}");
    git_bytes(root, &["show", &spec])
}

pub fn is_repo_root(path: &Path) -> bool {
    let p = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => return false,
    };
    if !p.is_dir() {
        return false;
    }
    match git(&p, &["rev-parse", "--show-toplevel"]) {
        Ok(top) => {
            let top_trimmed = top.trim();
            match std::fs::canonicalize(top_trimmed) {
                Ok(top_path) => top_path == p,
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

/// True for a path whose last component is `node_modules` and that is a symlink under `root`: the
/// link the worktree setup makes to the repo's `node_modules`. A real `node_modules` folder, or
/// any other path, is not one.
fn is_node_modules_link(root: &Path, path: &str) -> bool {
    let trimmed = path.trim_end_matches('/');
    Path::new(trimmed).file_name().is_some_and(|n| n == "node_modules")
        && std::fs::symlink_metadata(root.join(trimmed)).is_ok_and(|m| m.file_type().is_symlink())
}

/// Changed and untracked paths of the working tree at `root`. A `node_modules` symlink is left
/// out (a project whose `.gitignore` says `node_modules/` does not ignore a symlink).
pub fn changed_paths(root: &Path) -> BrainResult<BTreeSet<String>> {
    let raw = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    if raw.is_empty() {
        return Ok(BTreeSet::new());
    }
    let mut parts: Vec<&str> = raw.split('\0').collect();
    if let Some(&"") = parts.last() {
        parts.pop();
    }
    let mut paths = BTreeSet::new();
    let mut i = 0;
    while i < parts.len() {
        let entry = parts[i];
        if entry.is_empty() {
            i += 1;
            continue;
        }
        let status = if entry.len() >= 2 { &entry[..2] } else { "" };
        let path = if entry.len() >= 3 { &entry[3..] } else { "" };
        if !path.is_empty() && !is_node_modules_link(root, path) {
            paths.insert(path.to_string());
        }
        if status.contains('R') || status.contains('C') {
            i += 1;
            if i < parts.len() && !parts[i].is_empty() && !is_node_modules_link(root, parts[i]) {
                paths.insert(parts[i].to_string());
            }
        }
        i += 1;
    }
    Ok(paths)
}

pub fn current_branch(root: &Path) -> Option<String> {
    match git(root, &["branch", "--show-current"]) {
        Ok(out) => {
            let s = out.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        }
        Err(_) => None,
    }
}

pub fn head_sha(root: &Path) -> BrainResult<String> {
    let out = git(root, &["rev-parse", "HEAD"])?;
    Ok(out.trim().to_string())
}

pub fn is_hex_sha(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn ancestry(root: &Path, ancestor: &str, descendant: &str) -> Option<bool> {
    if !is_hex_sha(ancestor) || !is_hex_sha(descendant) {
        return None;
    }

    let mut cmd = git_cmd(root);
    cmd.args(["merge-base", "--is-ancestor", ancestor, descendant]);

    let status = cmd.status().ok()?;
    match status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

pub fn is_ancestor(root: &Path, ancestor: &str, descendant: &str) -> bool {
    ancestry(root, ancestor, descendant) == Some(true)
}

#[allow(clippy::result_unit_err)]
pub fn all_plan_commits_on_branch(
    root: &Path,
    branch: &str,
    slug: &str,
    base_commit: Option<&str>,
) -> Result<Vec<String>, ()> {
    if branch.is_empty() || branch.starts_with('-') {
        return Err(());
    }

    let ref_name = format!("refs/heads/{branch}");
    let mut args = vec!["log", "--format=%H %s"];
    let range;
    if let Some(base) = base_commit.filter(|b| is_hex_sha(b)) {
        range = format!("{base}..{ref_name}");
        args.push(&range);
    } else {
        args.push(&ref_name);
    }
    args.push("--");

    let mut cmd = git_cmd(root);
    cmd.args(&args);

    let output = cmd.output().map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_subj = format!("{slug}: plan");
    let mut out = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((sha, subj)) = trimmed.split_once(' ') {
            if subj.trim() == expected_subj {
                out.push(sha.trim().to_string());
            }
        }
    }

    Ok(out)
}

#[allow(clippy::result_unit_err)]
pub fn plan_commit_on_branch(
    root: &Path,
    branch: &str,
    slug: &str,
    base_commit: Option<&str>,
) -> Result<Option<String>, ()> {
    all_plan_commits_on_branch(root, branch, slug, base_commit).map(|v| v.into_iter().next())
}

pub fn commit_time_ms(root: &Path, sha: &str) -> Option<i64> {
    if !is_hex_sha(sha) {
        return None;
    }
    let out = git(root, &["show", "-s", "--format=%ct", sha]).ok()?;
    out.trim().parse::<i64>().ok().map(|s| s * 1000)
}

pub fn branch_exists(root: &Path, name: &str) -> bool {
    let ref_name = format!("refs/heads/{name}");
    git(root, &["rev-parse", "--verify", &ref_name]).is_ok()
}

pub fn create_branch(root: &Path, name: &str) -> BrainResult<()> {
    create_branch_from(root, name, "HEAD")
}

/// `git checkout -b <name> <start>`: a new branch from `start` (a branch name or `HEAD`).
pub fn create_branch_from(root: &Path, name: &str, start: &str) -> BrainResult<()> {
    git(root, &["checkout", "-b", name, start, "--"])?;
    Ok(())
}

/// `git checkout <name>` of an existing local branch. Refused (nothing run) when the branch does
/// not exist; git itself refuses when a local change would be overwritten.
pub fn checkout_branch(root: &Path, name: &str) -> BrainResult<()> {
    if name.starts_with('-') || !branch_exists(root, name) {
        return Err(BrainError::error(format!(
            "Không có nhánh '{name}' để checkout"
        )));
    }
    git(root, &["checkout", name, "--"])?;
    Ok(())
}

/// `git branch <name> <start>`: a new branch from `start` without checking it out (D2.2: the
/// main repo stays where it is; the task works in its own worktree).
pub fn branch_from(root: &Path, name: &str, start: &str) -> BrainResult<()> {
    if name.starts_with('-') || start.starts_with('-') {
        return Err(BrainError::error(format!("Tên nhánh không hợp lệ: '{name}'")));
    }
    git(root, &["branch", "--no-track", name, start])?;
    Ok(())
}

/// `git worktree add <path> <branch>` of an existing local branch (D2.2). The parent folder is
/// created first; an existing `path` is refused by git itself.
pub fn worktree_add(root: &Path, path: &Path, branch: &str) -> BrainResult<()> {
    if branch.starts_with('-') || !branch_exists(root, branch) {
        return Err(BrainError::error(format!(
            "Không có nhánh '{branch}' để tạo worktree"
        )));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let p = path.to_string_lossy();
    git(root, &["worktree", "add", "--", &p, branch])?;
    Ok(())
}

/// `git worktree remove <path>` WITHOUT `--force` (D2.4): git refuses a worktree with local
/// changes or untracked files. The branch is never deleted.
pub fn worktree_remove(root: &Path, path: &Path) -> BrainResult<()> {
    let p = path.to_string_lossy();
    git(root, &["worktree", "remove", "--", &p])?;
    Ok(())
}

/// True when `branch` is fully merged into `base` (every commit of `branch` is reachable from
/// `base`). False when either does not exist.
pub fn is_merged_into(root: &Path, branch: &str, base: &str) -> bool {
    if !branch_exists(root, branch) || !branch_exists(root, base) {
        return false;
    }
    let sha = |b: &str| {
        git(root, &["rev-parse", "--verify", &format!("refs/heads/{b}")])
            .map(|s| s.trim().to_string())
            .ok()
    };
    match (sha(branch), sha(base)) {
        (Some(a), Some(b)) => is_ancestor(root, &a, &b),
        _ => false,
    }
}

/// `git` with a time limit, for commands that talk to a remote (`fetch`, `push`). Never asks for
/// anything: `GIT_TERMINAL_PROMPT=0`, and ssh runs in batch mode with a connect timeout unless
/// the user chose their own ssh already (`GIT_SSH_COMMAND` / `GIT_SSH`, or `core.sshCommand`,
/// which the variable would override). The child runs in its own process group; when `timeout`
/// passes the whole group (git, ssh, hooks) is killed and an error is returned. No shell.
pub fn git_net(root: &Path, args: &[&str], timeout: std::time::Duration) -> BrainResult<String> {
    use std::io::Read;
    use std::os::unix::process::CommandExt;
    let user_ssh = std::env::var_os("GIT_SSH_COMMAND").is_some()
        || std::env::var_os("GIT_SSH").is_some()
        || git(root, &["config", "--get", "core.sshCommand"])
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
    let mut cmd = git_cmd(root);
    cmd.args(args);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    if !user_ssh {
        cmd.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o ConnectTimeout=20");
    }
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.process_group(0);
    let first = args.first().copied().unwrap_or("");
    let mut child = cmd
        .spawn()
        .map_err(|e| BrainError::error(format!("Lỗi chạy git: {e}")))?;
    let pid = child.id() as i32;
    let reader = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    };
    let out_t = reader(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let err_t = reader(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) => {}
            Err(e) => return Err(BrainError::error(format!("Lỗi chờ git: {e}"))),
        }
        if std::time::Instant::now() >= deadline {
            // SAFETY: plain signal to the process group this function created.
            unsafe {
                libc::killpg(pid, libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let stdout = out_t.join().unwrap_or_default();
    let stderr = err_t.join().unwrap_or_default();
    let Some(status) = status else {
        return Err(BrainError::error(format!(
            "git {first} quá {}s",
            timeout.as_secs()
        )));
    };
    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr).trim().to_string();
        let msg = if stderr.is_empty() {
            let code = status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string());
            format!("Lệnh git thất bại (exit {code}): git {first}")
        } else {
            stderr
        };
        return Err(BrainError::error(msg));
    }
    Ok(String::from_utf8_lossy(&stdout).to_string())
}

/// `git remote get-url <name>`; `None` when there is no such remote.
pub fn remote_url(root: &Path, name: &str) -> Option<String> {
    if name.is_empty() || name.starts_with('-') {
        return None;
    }
    git(root, &["remote", "get-url", name])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// The full sha `rev` points to (`git rev-parse --verify --quiet <rev>^{commit}`), or `None`.
pub fn rev_parse(root: &Path, rev: &str) -> Option<String> {
    if rev.is_empty() || rev.starts_with('-') {
        return None;
    }
    let spec = format!("{rev}^{{commit}}");
    git(root, &["rev-parse", "--verify", "--quiet", &spec])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| is_hex_sha(s))
}

/// `(ahead, behind)` of `local` against `upstream`: `git rev-list --left-right --count
/// <local>...<upstream>`.
pub fn ahead_behind(root: &Path, local: &str, upstream: &str) -> BrainResult<(usize, usize)> {
    if local.starts_with('-') || upstream.starts_with('-') {
        return Err(BrainError::error("Tên ref không hợp lệ"));
    }
    let range = format!("{local}...{upstream}");
    let out = git(root, &["rev-list", "--left-right", "--count", &range, "--"])?;
    let mut parts = out.split_whitespace().map(|p| p.parse::<usize>());
    match (parts.next(), parts.next()) {
        (Some(Ok(a)), Some(Ok(b))) => Ok((a, b)),
        _ => Err(BrainError::error(format!(
            "Không đọc được kết quả git rev-list: {}",
            out.trim()
        ))),
    }
}

/// Subjects of the first-parent commits in `range` (newest first), at most `max`. Empty on error.
pub fn log_subjects(root: &Path, range: &str, max: usize) -> Vec<String> {
    if range.is_empty() || range.starts_with('-') {
        return Vec::new();
    }
    let n = format!("--max-count={max}");
    git(root, &["log", "--first-parent", "--format=%s", &n, range, "--"])
        .map(|out| out.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// Pure-Rust syntax check for a branch short name, no process spawned. It applies the rules of
/// `git check-ref-format --branch` that matter here: not empty, not `@`, no leading `-`, no
/// whitespace or control characters (DEL included), none of `~ ^ : ? * [ \`, no `..`, no `@{`,
/// no `//`, not starting or ending with `/`, not ending with `.`, and no path component that
/// starts with `.` or ends with `.lock`. Unicode letters are allowed.
pub fn valid_branch_name_syntax(name: &str) -> bool {
    if name.is_empty()
        || name == "@"
        || name.starts_with('-')
        || name.starts_with('/')
        || name.ends_with('/')
        || name.ends_with('.')
        || name.contains("..")
        || name.contains("@{")
        || name.contains("//")
        || name
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "~^:?*[\\".contains(c))
    {
        return false;
    }
    name.split('/')
        .all(|part| !part.starts_with('.') && !part.ends_with(".lock"))
}

/// A branch name agent-brain may put on a git command line: it passes
/// [`valid_branch_name_syntax`] and is accepted by `git check-ref-format --branch`. The git
/// process is only spawned for names that pass the cheap syntax check; use the syntax-only
/// function where `git` may be absent (startup config validation, dashboard poll).
pub fn valid_branch_name(name: &str) -> bool {
    if !valid_branch_name_syntax(name) {
        return false;
    }
    std::process::Command::new("git")
        .args(["check-ref-format", "--branch", name])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Diff giữa 2 commit cho 1 file duy nhất, giới hạn tối đa `max_bytes`.
pub fn diff_file(
    root: &Path,
    prev: &str,
    cur: &str,
    file: &str,
    max_bytes: usize,
) -> BrainResult<String> {
    let out = git(root, &["diff", prev, cur, "--", file])?;
    if out.len() > max_bytes {
        let mut idx = max_bytes;
        while idx > 0 && !out.is_char_boundary(idx) {
            idx -= 1;
        }
        let mut s = out[..idx].to_string();
        s.push_str("\n... [cắt bớt do vượt quá giới hạn 64 KiB]");
        Ok(s)
    } else {
        Ok(out)
    }
}

/// Trả (số dòng thêm, số dòng bớt) giữa 2 commit cho 1 file duy nhất qua `git diff --numstat`.
pub fn diff_numstat_file(
    root: &Path,
    prev: &str,
    cur: &str,
    file: &str,
) -> BrainResult<Option<(u64, u64)>> {
    let out = git(root, &["diff", "--numstat", prev, cur, "--", file])?;
    for line in out.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let added: u64 = parts[0].parse().unwrap_or(0);
            let deleted: u64 = parts[1].parse().unwrap_or(0);
            return Ok(Some((added, deleted)));
        }
    }
    Ok(None)
}

pub fn commit_paths(root: &Path, paths: &[&str], message: &str) -> BrainResult<String> {
    let mut add_args = vec!["add", "-f", "--"];
    add_args.extend_from_slice(paths);
    git(root, &add_args)?;

    let mut commit_args = vec!["commit", "-m", message, "--"];
    commit_args.extend_from_slice(paths);
    git(root, &commit_args)?;

    head_sha(root)
}

pub fn diff_names(root: &Path, a: &str, b: &str) -> BrainResult<BTreeSet<String>> {
    let range = format!("{a}..{b}");
    let out = git(root, &["diff", "--name-only", &range])?;
    let set = out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(set)
}

/// Tên file đổi giữa `from` (sha hoặc nhánh) và `to` (nhánh hoặc sha) bằng
/// `git diff --name-only --no-renames from..to --`. Chỉ đọc. Từ chối chuỗi rỗng, chuỗi bắt
/// đầu bằng `-` và tên không hợp lệ làm đối số git (không phải sha hex cũng không qua
/// `valid_branch_name_syntax`).
pub fn changed_names_between(root: &Path, from: &str, to: &str) -> BrainResult<BTreeSet<String>> {
    for rev in [from, to] {
        if rev.is_empty()
            || rev.starts_with('-')
            || !(is_hex_sha(rev) || valid_branch_name_syntax(rev))
        {
            return Err(BrainError::error(format!(
                "Tham chiếu git không hợp lệ cho so sánh: '{rev}'"
            )));
        }
    }
    let range = format!("{from}..{to}");
    let out = git(root, &["diff", "--name-only", "--no-renames", &range, "--"])?;
    Ok(out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

pub fn diff_code_names(root: &Path, a: &str, b: &str) -> BrainResult<BTreeSet<String>> {
    let mut names = diff_names(root, a, b)?;
    names.retain(|n| !crate::protocol_paths::is_protocol_file(n));
    Ok(names)
}

pub fn is_tracked(root: &Path, rel: &str) -> bool {
    let mut cmd = git_cmd(root);
    cmd.args(["ls-files", "--error-unmatch", "--", rel]);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    match cmd.status() {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

pub fn find_commit_by_subject(root: &Path, subject: &str) -> BrainResult<Option<String>> {
    let out = git(root, &["log", "--format=%H %s", "HEAD"])?;
    for line in out.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let sha = parts[0];
        let subj = if parts.len() > 1 { parts[1] } else { "" };
        if subj == subject {
            return Ok(Some(sha.to_string()));
        }
    }
    Ok(None)
}

pub fn commit_subjects(root: &Path, a: &str, b: &str) -> BrainResult<Vec<(String, String)>> {
    let range = format!("{a}..{b}");
    let out = git(root, &["log", "--reverse", "--first-parent", "--format=%H %s", &range])?;
    let mut res = Vec::new();
    for line in out.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let sha = parts[0];
        let subj = if parts.len() > 1 { parts[1] } else { "" };
        res.push((sha.to_string(), subj.to_string()));
    }
    Ok(res)
}

/// Computes the set of files modified by Dev on the task branch across regular commits and merge
/// conflict resolutions / manual edits, excluding files brought in from merged branches without conflict.
pub fn dev_changed_names(root: &Path, base: &str) -> BrainResult<BTreeSet<String>> {
    let range = format!("{base}..HEAD");
    let mut changed = BTreeSet::new();

    // 1. Files in regular commits of the task branch (first-parent, no-merges)
    let log_out = git(
        root,
        &[
            "log",
            "--first-parent",
            "--no-merges",
            "--format=",
            "--name-only",
            &range,
        ],
    )?;
    for line in log_out.lines() {
        let p = line.trim();
        if !p.is_empty() {
            changed.insert(p.to_string());
        }
    }

    // 2. Files in merge commits on first-parent that differ from all parents
    let merges_out = git(root, &["rev-list", "--first-parent", "--merges", &range])?;
    for merge_sha in merges_out.lines() {
        let sha = merge_sha.trim();
        if sha.is_empty() {
            continue;
        }
        let diff_out = git(
            root,
            &[
                "diff-tree",
                "-r",
                "--cc",
                "--name-only",
                "--no-commit-id",
                sha,
            ],
        )?;
        for line in diff_out.lines() {
            let p = line.trim();
            if !p.is_empty() {
                changed.insert(p.to_string());
            }
        }
    }

    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(dir.path())
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .unwrap();
        assert!(status.success());
        dir
    }

    fn commit(root: &Path, files: &[(&str, &str)], msg: &str) -> String {
        for (rel, content) in files {
            let p = root.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, content).unwrap();
        }
        let status = std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .unwrap();
        assert!(status.success());

        let output = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                msg,
            ])
            .current_dir(root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        head_sha(root).unwrap()
    }

    #[test]
    fn changed_paths_skips_a_node_modules_symlink_only() {
        let dir = repo();
        let root = dir.path();
        commit(root, &[("a.txt", "1")], "init");
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("node_modules")).unwrap();
        // a symlink named node_modules at any depth is skipped too
        std::fs::create_dir_all(root.join("packages/app")).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("packages/app/node_modules")).unwrap();
        // a symlink with another name is reported
        std::os::unix::fs::symlink(outside.path(), root.join("other-link")).unwrap();
        std::fs::write(root.join("untracked.txt"), "x").unwrap();
        let paths = changed_paths(root).unwrap();
        assert!(!paths.iter().any(|p| p.ends_with("node_modules")), "{paths:?}");
        assert!(paths.contains("other-link"), "{paths:?}");
        assert!(paths.contains("untracked.txt"), "{paths:?}");
    }

    #[test]
    fn changed_paths_reports_a_real_node_modules_folder() {
        let dir = repo();
        let root = dir.path();
        commit(root, &[("a.txt", "1")], "init");
        std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        std::fs::write(root.join("node_modules/pkg/index.js"), "x").unwrap();
        std::fs::write(root.join("untracked.txt"), "x").unwrap();
        let paths = changed_paths(root).unwrap();
        assert!(paths.contains("node_modules/pkg/index.js"), "{paths:?}");
        assert!(paths.contains("untracked.txt"), "{paths:?}");
    }

    #[test]
    fn is_tracked_cases() {
        let dir = repo();
        let root = dir.path();
        commit(root, &[("src/tracked.py", "print(1)\n")], "add tracked");
        std::fs::write(root.join("untracked.txt"), "hello").unwrap();

        assert!(is_tracked(root, "src/tracked.py"));
        assert!(!is_tracked(root, "untracked.txt"));
        assert!(!is_tracked(root, "missing.txt"));
    }

    #[test]
    fn ancestry_tri_state() {
        let dir = repo();
        let root = dir.path();
        let a = commit(root, &[("a.txt", "1")], "commit a");
        let b = commit(root, &[("b.txt", "1")], "commit b");

        assert_eq!(ancestry(root, &a, &b), Some(true));
        assert_eq!(ancestry(root, &b, &a), Some(false));

        let bogus_sha = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(ancestry(root, &a, bogus_sha), None);
        assert_eq!(ancestry(root, "-x", &b), None);
        assert_eq!(ancestry(root, "zz", &b), None);

        assert!(is_ancestor(root, &a, &b));
        assert!(!is_ancestor(root, &b, &a));
        assert!(!is_ancestor(root, "-x", &b));
    }

    #[test]
    fn plan_commit_bounded_to_branch() {
        let dir = repo();
        let root = dir.path();

        let _old_plan = commit(root, &[("old.txt", "1")], "x: plan");
        let base = commit(root, &[("init2.txt", "1")], "init 2");

        let status = std::process::Command::new("git")
            .args(["checkout", "-b", "task/x", &base])
            .current_dir(root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .unwrap();
        assert!(status.success());

        assert_eq!(
            plan_commit_on_branch(root, "task/x", "x", Some(&base)),
            Ok(None)
        );

        let p1 = commit(root, &[("plan.txt", "1")], "x: plan");
        assert_eq!(
            plan_commit_on_branch(root, "task/x", "x", Some(&base)),
            Ok(Some(p1))
        );

        let p2 = commit(root, &[("plan.txt", "2")], "x: plan");
        assert_eq!(
            plan_commit_on_branch(root, "task/x", "x", Some(&base)),
            Ok(Some(p2.clone()))
        );

        commit(root, &[("rev.txt", "1")], "x: plan-review");
        assert_eq!(
            plan_commit_on_branch(root, "task/x", "x", Some(&base)),
            Ok(Some(p2))
        );
    }

    #[test]
    fn plan_commit_ignores_tag_and_paths() {
        let dir = repo();
        let root = dir.path();

        let old_plan = commit(root, &[("old.txt", "1")], "x: plan");
        let base = commit(root, &[("init2.txt", "1")], "init 2");

        let status = std::process::Command::new("git")
            .args(["checkout", "-b", "task/x", &base])
            .current_dir(root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .unwrap();
        assert!(status.success());

        let p = commit(root, &[("plan.txt", "1")], "x: plan");

        // create tag named task/x on old commit
        let tag_status = std::process::Command::new("git")
            .args(["tag", "task/x", &old_plan])
            .current_dir(root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .unwrap();
        assert!(tag_status.success());

        // create file named task
        std::fs::write(root.join("task"), "ambiguous").unwrap();

        assert_eq!(
            plan_commit_on_branch(root, "task/x", "x", Some(&base)),
            Ok(Some(p.clone()))
        );

        // base_commit None searches whole branch
        assert_eq!(
            plan_commit_on_branch(root, "task/x", "x", None),
            Ok(Some(p))
        );
    }

    #[test]
    fn plan_commit_refuses_dash() {
        let dir = repo();
        let root = dir.path();
        assert_eq!(plan_commit_on_branch(root, "-x", "x", None), Err(()));
        assert_eq!(plan_commit_on_branch(root, "", "x", None), Err(()));
        assert_eq!(
            plan_commit_on_branch(root, "nonexistent", "x", None),
            Err(())
        );
    }

    #[test]
    fn diff_code_names_excludes_reports() {
        let dir = repo();
        let root = dir.path();
        let a = commit(root, &[("src/a.py", "x = 1\n")], "init");
        let b = commit(root, &[(".plan-review.md", "# review\n")], "review");
        let diff_ab = diff_code_names(root, &a, &b).unwrap();
        assert!(diff_ab.is_empty(), "diff_ab was: {:?}", diff_ab);

        let c = commit(
            root,
            &[
                ("src/a.py", "x = 2\n"),
                (".code-review.md", "# code review\n"),
            ],
            "code changes",
        );
        let diff_bc = diff_code_names(root, &b, &c).unwrap();
        let mut expected = BTreeSet::new();
        expected.insert("src/a.py".to_string());
        assert_eq!(diff_bc, expected);
    }

    #[test]
    fn git_sets_optional_locks_off() {
        let dir = repo();
        let root = dir.path();
        let out = git(
            root,
            &[
                "-c",
                "alias.optlocks=!printenv GIT_OPTIONAL_LOCKS",
                "optlocks",
            ],
        )
        .expect("git() must pass GIT_OPTIONAL_LOCKS to git");
        assert_eq!(out.trim(), "0");
    }

    #[test]
    fn git_net_timeout_kills_the_process_group() {
        let dir = repo();
        let root = dir.path();
        let pidfile = root.join("slow.pid");
        let alias = format!(
            "alias.slow=!echo $$ > '{}'; exec sleep 30",
            pidfile.display()
        );
        let start = std::time::Instant::now();
        let err = git_net(root, &["-c", &alias, "slow"], std::time::Duration::from_secs(1))
            .expect_err("a command past its time limit fails");
        assert!(err.message.contains("quá 1s"), "{}", err.message);
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        let pid: i32 = std::fs::read_to_string(&pidfile)
            .expect("the slow command started")
            .trim()
            .parse()
            .unwrap();
        // The killed process is gone (or a zombie about to be reaped by init).
        let gone = (0..50).any(|_| {
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
            let dead = stat.is_empty() || stat.contains(") Z ");
            if !dead {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            dead
        });
        assert!(gone, "process {pid} still runs after the timeout");
        // A quick command works and returns its output.
        let out = git_net(root, &["rev-parse", "--is-inside-work-tree"], std::time::Duration::from_secs(30))
            .unwrap();
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn branch_name_check() {
        for bad in ["", "-x", "a b", "a..b", "a\tb", "a\nb", "@{-1}", "x@{1}", "a~1", "a:b"] {
            assert!(!valid_branch_name(bad), "{bad:?}");
        }
        for ok in ["dev", "main", "task/sua-app", "release/1.2"] {
            assert!(valid_branch_name(ok), "{ok:?}");
        }
    }

    const VALID_BRANCHES: [&str; 5] = ["dev", "main", "task/sua-app", "release/1.2", "feature/đăng-nhập"];
    const INVALID_BRANCHES: [&str; 21] = [
        "", "@", "-x", "a b", "a..b", "a~1", "a^b", "a:b", "a?b", "a*b", "a[b", "a\\b", "a//b",
        "/a", "a/", "a.", "a.lock", "x/.hidden", "x/y.lock/z", "a@{1}", "a\tb",
    ];

    #[test]
    fn branch_name_syntax_table() {
        for ok in VALID_BRANCHES {
            assert!(valid_branch_name_syntax(ok), "{ok:?}");
        }
        for bad in INVALID_BRANCHES {
            assert!(!valid_branch_name_syntax(bad), "{bad:?}");
        }
        assert!(!valid_branch_name_syntax("a\u{7f}b"), "DEL");
    }

    #[test]
    fn branch_name_syntax_agrees_with_git() {
        if std::process::Command::new("git").arg("--version").output().is_err() {
            return;
        }
        for name in VALID_BRANCHES.iter().chain(INVALID_BRANCHES.iter()) {
            assert_eq!(
                valid_branch_name_syntax(name),
                valid_branch_name(name),
                "syntax check and git disagree on {name:?}"
            );
        }
    }

    #[test]
    fn rev_and_ahead_behind() {
        let dir = repo();
        let root = dir.path();
        let a = commit(root, &[("a.txt", "1")], "one");
        git(root, &["branch", "other"]).unwrap();
        commit(root, &[("b.txt", "1")], "two");
        commit(root, &[("c.txt", "1")], "three");
        assert_eq!(rev_parse(root, "other").as_deref(), Some(a.as_str()));
        assert_eq!(rev_parse(root, "nope"), None);
        assert_eq!(rev_parse(root, "-x"), None);
        assert_eq!(ahead_behind(root, "main", "other").unwrap(), (2, 0));
        assert_eq!(ahead_behind(root, "other", "main").unwrap(), (0, 2));
        assert_eq!(log_subjects(root, "other..main", 5), vec!["three", "two"]);
        assert_eq!(log_subjects(root, "other..main", 1), vec!["three"]);
        assert_eq!(remote_url(root, "origin"), None);
        git(root, &["remote", "add", "origin", "/srv/x.git"]).unwrap();
        assert_eq!(remote_url(root, "origin").as_deref(), Some("/srv/x.git"));
    }

    #[test]
    fn changed_paths_leaves_foreign_index_lock_alone() {
        let dir = repo();
        let root = dir.path();
        commit(root, &[("a.txt", "1\n")], "init");
        std::fs::write(root.join("a.txt"), "2\n").unwrap();

        let lock = root.join(".git").join("index.lock");
        std::fs::write(&lock, "").unwrap();

        let paths = changed_paths(root).expect("status must not fail on a held index.lock");
        let mut expected = BTreeSet::new();
        expected.insert("a.txt".to_string());
        assert_eq!(paths, expected);
        assert!(
            lock.exists(),
            "status must not touch another process's index.lock"
        );
    }
    // ---- shadow repo (idea projects) ----

    use super::SHADOW_TEST_ENV as SHADOW_ENV;

    struct ShadowEnv {
        _lock: std::sync::MutexGuard<'static, ()>,
        _data: tempfile::TempDir,
        old: Option<std::ffi::OsString>,
    }

    impl ShadowEnv {
        fn new() -> ShadowEnv {
            let lock = SHADOW_ENV.lock().unwrap_or_else(|e| e.into_inner());
            let data = tempfile::tempdir().unwrap();
            let old = std::env::var_os("XDG_DATA_HOME");
            std::env::set_var("XDG_DATA_HOME", data.path());
            ShadowEnv {
                _lock: lock,
                _data: data,
                old,
            }
        }
    }

    impl Drop for ShadowEnv {
        fn drop(&mut self) {
            match &self.old {
                Some(v) => std::env::set_var("XDG_DATA_HOME", v),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
        }
    }

    #[test]
    fn shadow_basic_flow() {
        let _env = ShadowEnv::new();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("idea.md"), "# y tuong\n").unwrap();

        ensure_shadow(&root).unwrap();
        assert!(!root.join(".git").exists());
        assert!(shadow_dir(&root).is_dir());
        assert!(is_repo_root(&root));
        let base = head_sha(&root).unwrap();
        assert_eq!(base.trim().len(), 40);
        assert_eq!(current_branch(&root).as_deref(), Some("main"));

        // (b) commit a new file, diff_names between the two SHAs
        std::fs::write(root.join("plan.md"), "plan\n").unwrap();
        let sha = commit_paths(&root, &["plan.md"], "x: plan").unwrap();
        let names = diff_names(&root, &base, &sha).unwrap();
        assert_eq!(names.into_iter().collect::<Vec<_>>(), vec!["plan.md"]);

        // (c) task branch
        create_branch(&root, "task/x").unwrap();
        assert_eq!(current_branch(&root).as_deref(), Some("task/x"));
        git(&root, &["checkout", "main"]).unwrap();
        assert_eq!(current_branch(&root).as_deref(), Some("main"));
        assert!(!root.join(".git").exists());

        // (d) second call is a no-op
        ensure_shadow(&root).unwrap();
        let count = git(&root, &["rev-list", "--count", "--all"]).unwrap();
        assert_eq!(count.trim(), "2");
        let roots = git(&root, &["rev-list", "--max-parents=0", "HEAD"]).unwrap();
        assert_eq!(roots.lines().count(), 1);
    }

    #[test]
    fn shadow_skipped_for_real_git() {
        let _env = ShadowEnv::new();
        let dir = repo();
        let root = dir.path().canonicalize().unwrap();
        ensure_shadow(&root).unwrap();
        assert!(!shadow_dir(&root).exists());
        assert!(root.join(".git").is_dir());
        assert!(is_repo_root(&root));
    }

    #[test]
    fn shadow_too_large_leaves_nothing() {
        let _env = ShadowEnv::new();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        for i in 0..=SHADOW_MAX_FILES {
            std::fs::write(root.join(format!("f{i}")), "").unwrap();
        }
        let err = ensure_shadow(&root).unwrap_err();
        assert!(
            err.to_string()
                .contains("quá lớn để theo dõi (>20000 file hoặc >500 MiB)"),
            "{err}"
        );
        assert!(!shadow_dir(&root).exists());
    }

    #[test]
    fn shadow_excludes_secrets() {
        let _env = ShadowEnv::new();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        for f in [".env", ".env.local", "a.pem", "b.key", "id_rsa", "ok.md"] {
            std::fs::write(root.join(f), "secret\n").unwrap();
        }
        ensure_shadow(&root).unwrap();
        let files = git(&root, &["ls-tree", "-r", "--name-only", "HEAD"]).unwrap();
        assert_eq!(files.trim(), "ok.md");
    }

    #[test]
    fn shadow_dir_differs_per_path() {
        let _env = ShadowEnv::new();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let ra = a.path().join("docs");
        let rb = b.path().join("docs");
        std::fs::create_dir_all(&ra).unwrap();
        std::fs::create_dir_all(&rb).unwrap();
        assert_ne!(shadow_dir(&ra), shadow_dir(&rb));
        assert_eq!(shadow_dir(&ra), shadow_dir(&ra));
    }

    #[test]
    fn test_dev_changed_names() {
        let r = repo();
        let root = r.path();

        // Base commit on main
        let base_sha = commit(root, &[("init.txt", "v1")], "init");

        // 1. Chỉ có commit thường: bằng diff_names
        commit(root, &[("a.txt", "a1")], "commit a");
        let diff1 = diff_names(root, &base_sha, "HEAD").unwrap();
        let dev1 = dev_changed_names(root, &base_sha).unwrap();
        assert_eq!(dev1, diff1);
        assert_eq!(dev1, ["a.txt".to_string()].into_iter().collect());

        // 2. Nhánh task có commit sửa a.txt, rồi merge --no-ff một nhánh khác sửa b.txt, không xung đột: kết quả chỉ có a.txt
        let r2 = repo();
        let root2 = r2.path();
        let base2 = commit(root2, &[("init.txt", "v1")], "init");

        let run_cmd = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(root2)
                .status()
                .unwrap();
            assert!(status.success());
        };
        run_cmd(&["checkout", "-b", "other"]);
        commit(root2, &[("b.txt", "b1")], "other commit b");

        run_cmd(&["checkout", "-b", "task", &base2]);
        commit(root2, &[("a.txt", "a1")], "task commit a");

        let merge_status = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
                "merge",
                "--no-ff",
                "-m",
                "merge other",
                "other",
            ])
            .current_dir(root2)
            .status()
            .unwrap();
        assert!(merge_status.success());

        let dev2 = dev_changed_names(root2, &base2).unwrap();
        assert_eq!(dev2, ["a.txt".to_string()].into_iter().collect());

        // 3. Merge có xung đột trên c.txt, giải tay rồi commit: kết quả có c.txt
        let r3 = repo();
        let root3 = r3.path();
        let base3 = commit(root3, &[("c.txt", "base")], "init");

        let run_cmd3 = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(root3)
                .status()
                .unwrap();
            assert!(status.success());
        };
        run_cmd3(&["checkout", "-b", "other3"]);
        commit(root3, &[("c.txt", "other branch modification")], "other c");

        run_cmd3(&["checkout", "-b", "task3", &base3]);
        commit(root3, &[("c.txt", "task branch modification")], "task c");

        // Try merge other3 (will conflict)
        let _ = std::process::Command::new("git")
            .args(["merge", "--no-ff", "other3"])
            .current_dir(root3)
            .status();

        // Resolve manually
        std::fs::write(root3.join("c.txt"), "resolved content").unwrap();
        run_cmd3(&["add", "c.txt"]);
        let commit_res = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "resolved conflict",
            ])
            .current_dir(root3)
            .status()
            .unwrap();
        assert!(commit_res.success());

        let dev3 = dev_changed_names(root3, &base3).unwrap();
        assert!(dev3.contains("c.txt"));
    }
}
