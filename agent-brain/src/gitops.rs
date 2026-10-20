use crate::error::{BrainError, BrainResult};
use std::collections::BTreeSet;
use std::path::Path;

pub fn git(root: &Path, args: &[&str]) -> BrainResult<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(args);
    cmd.current_dir(root);
    cmd.env("GIT_TERMINAL_PROMPT", "0");

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

fn is_hex_sha(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_ancestor(root: &Path, ancestor: &str, descendant: &str) -> bool {
    if !is_hex_sha(ancestor) || !is_hex_sha(descendant) {
        return false;
    }
    git(root, &["merge-base", "--is-ancestor", ancestor, descendant]).is_ok()
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
    let mut cmd = std::process::Command::new("git");
    cmd.args(["ls-files", "--error-unmatch", "--", rel]);
    cmd.current_dir(root);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
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
}
