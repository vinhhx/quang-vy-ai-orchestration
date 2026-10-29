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
        if !path.is_empty() {
            paths.insert(path.to_string());
        }
        if status.contains('R') || status.contains('C') {
            i += 1;
            if i < parts.len() && !parts[i].is_empty() {
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

pub fn plan_commit_on_branch(
    root: &Path,
    branch: &str,
    slug: &str,
    base_commit: Option<&str>,
) -> Result<Option<String>, ()> {
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

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((sha, subj)) = trimmed.split_once(' ') {
            if subj.trim() == expected_subj {
                return Ok(Some(sha.trim().to_string()));
            }
        }
    }

    Ok(None)
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
    git(root, &["checkout", "-b", name, "HEAD"])?;
    Ok(())
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

pub fn diff_code_names(root: &Path, a: &str, b: &str) -> BrainResult<BTreeSet<String>> {
    let mut names = diff_names(root, a, b)?;
    for proto in crate::stream::PROTOCOL_FILES {
        names.remove(*proto);
    }
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
    let out = git(root, &["log", "--reverse", "--format=%H %s", &range])?;
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
}
