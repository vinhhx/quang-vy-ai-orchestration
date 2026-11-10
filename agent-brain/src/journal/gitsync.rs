//! Bộ chạy git riêng của nhật ký (D-10, D-11; RESEARCH §4): có thời hạn, danh tính cố định,
//! không ký, không hook, không hỏi mật khẩu, chỉ giao thức file/ssh/https. Không dùng `gitops`.

use super::config::JournalConfig;
use crate::error::{BrainError, BrainResult};
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct GitCtx {
    pub timeout: Duration,
    pub kill_grace: Duration,
    pub ssh_connect_timeout_seconds: u64,
    pub author_name: String,
    pub author_email: String,
}

impl GitCtx {
    pub fn from_cfg(cfg: &JournalConfig, kill_grace: Duration) -> GitCtx {
        GitCtx {
            timeout: Duration::from_secs(cfg.git_timeout_seconds),
            kill_grace,
            ssh_connect_timeout_seconds: cfg.ssh_connect_timeout_seconds,
            author_name: cfg.author_name.clone(),
            author_email: cfg.author_email.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOut {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl GitOut {
    pub fn ok(&self) -> bool {
        self.code == 0 && !self.timed_out
    }
}

/// Tùy chọn toàn cục đặt trước lệnh con: cấu hình git của người dùng không đổi được commit.
const GLOBAL_OPTS: &[&str] = &[
    "-c",
    "commit.gpgsign=false",
    "-c",
    "core.quotepath=false",
    "-c",
    "core.autocrlf=false",
    "-c",
    "core.hooksPath=/dev/null",
];

fn command(ctx: &GitCtx, cwd: Option<&Path>, args: &[&str]) -> Command {
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new("git");
    cmd.args(GLOBAL_OPTS).args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let n = ctx.ssh_connect_timeout_seconds;
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .env("GIT_ALLOW_PROTOCOL", "file:ssh:https")
        .env(
            "GIT_SSH_COMMAND",
            format!(
                "ssh -o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout={n} -o ServerAliveInterval={n} -o ServerAliveCountMax=2"
            ),
        )
        .env("GIT_AUTHOR_NAME", &ctx.author_name)
        .env("GIT_COMMITTER_NAME", &ctx.author_name)
        .env("GIT_AUTHOR_EMAIL", &ctx.author_email)
        .env("GIT_COMMITTER_EMAIL", &ctx.author_email);
    cmd.process_group(0);
    cmd
}

fn drain<R: Read + Send + 'static>(r: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut r) = r {
            let _ = r.read_to_end(&mut buf);
        }
        buf
    })
}

/// Chạy `git` với dữ liệu stdin tùy chọn; quá `ctx.timeout` thì dừng cả cây tiến trình.
pub fn run_git_input(
    ctx: &GitCtx,
    cwd: Option<&Path>,
    args: &[&str],
    input: Option<&[u8]>,
) -> BrainResult<GitOut> {
    let mut cmd = command(ctx, cwd, args);
    cmd.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| BrainError::error(format!("Nhật ký: không chạy được git: {e}")))?;
    let out_h = drain(child.stdout.take());
    let err_h = drain(child.stderr.take());
    if let (Some(data), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = stdin.write_all(data);
    }

    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait()? {
            Some(st) => break Some(st),
            None if start.elapsed() >= ctx.timeout => {
                timed_out = true;
                if let Some(pid) = crate::procs::proc_id(child.id() as i32) {
                    crate::procs::kill_tree(pid, ctx.kill_grace, &AtomicBool::new(false));
                }
                let _ = child.kill();
                break child.wait().ok();
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let stdout = String::from_utf8_lossy(&out_h.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&err_h.join().unwrap_or_default()).into_owned();
    Ok(GitOut {
        code: status.and_then(|s| s.code()).unwrap_or(-1),
        stdout,
        stderr,
        timed_out,
    })
}

pub fn run_git(ctx: &GitCtx, cwd: Option<&Path>, args: &[&str]) -> BrainResult<GitOut> {
    run_git_input(ctx, cwd, args, None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailCategory {
    Network,
    SshAuth,
    Rejected,
    Timeout,
    Other,
}

impl FailCategory {
    pub fn label(&self) -> &'static str {
        match self {
            FailCategory::Network => "lỗi mạng",
            FailCategory::SshAuth => "lỗi SSH/xác thực",
            FailCategory::Rejected => "repo vault từ chối (remote đã đổi)",
            FailCategory::Timeout => "git quá thời gian",
            FailCategory::Other => "lỗi git",
        }
    }
}

pub fn classify(code: i32, stderr: &str, timed_out: bool) -> FailCategory {
    let _ = code;
    if timed_out {
        return FailCategory::Timeout;
    }
    if stderr.contains("Permission denied") || stderr.contains("Host key verification failed") {
        return FailCategory::SshAuth;
    }
    if [
        "Could not resolve hostname",
        "Connection timed out",
        "Network is unreachable",
        "Connection refused",
        "Could not read from remote repository",
    ]
    .iter()
    .any(|p| stderr.contains(p))
    {
        return FailCategory::Network;
    }
    if stderr.contains("[rejected]")
        || stderr.contains("non-fast-forward")
        || stderr.contains("fetch first")
    {
        return FailCategory::Rejected;
    }
    FailCategory::Other
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitFailure {
    pub category: FailCategory,
    /// Chỉ nhãn loại lỗi; không bao giờ chứa stderr thô (có thể có URL/đường dẫn).
    pub summary: String,
}

impl GitFailure {
    fn of(category: FailCategory) -> GitFailure {
        GitFailure {
            category,
            summary: category.label().to_string(),
        }
    }

    fn from_out(out: &GitOut) -> GitFailure {
        GitFailure::of(classify(out.code, &out.stderr, out.timed_out))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitStop {
    Conflict { detail: String },
    Dirty { paths: Vec<String> },
    NotARepo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prepared {
    Ready,
    Stopped(GitStop),
    /// Chỉ khi chưa có bản clone dùng được (lần clone đầu thất bại).
    Failed(GitFailure),
}

fn dir_is_empty(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|mut it| it.next().is_none())
        .unwrap_or(false)
}

fn ref_exists(ctx: &GitCtx, dir: &Path, r: &str) -> bool {
    run_git(ctx, Some(dir), &["rev-parse", "--verify", "--quiet", r])
        .map(|o| o.ok())
        .unwrap_or(false)
}

fn rev(ctx: &GitCtx, dir: &Path, r: &str) -> Option<String> {
    run_git(ctx, Some(dir), &["rev-parse", "--verify", "--quiet", r])
        .ok()
        .filter(|o| o.ok())
        .map(|o| o.stdout.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Bảo đảm có bản clone ở `cfg.clone_dir` (clone khi chưa có). Đồng bộ với remote, dọn tệp
/// dở dang và xử lý xung đột là phần của 07-05.
pub fn prepare(ctx: &GitCtx, cfg: &JournalConfig, generated: &BTreeSet<String>) -> Prepared {
    let _ = generated;
    let dir: &Path = &cfg.clone_dir;
    if dir.join(".git").exists() {
        return Prepared::Ready;
    }
    if dir.exists() && !dir_is_empty(dir) {
        return Prepared::Stopped(GitStop::NotARepo);
    }
    let Some(url) = cfg.repo_url.as_deref() else {
        return Prepared::Failed(GitFailure::of(FailCategory::Other));
    };
    if url.starts_with('-') {
        return Prepared::Failed(GitFailure::of(FailCategory::Other));
    }
    if let Some(parent) = dir.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return Prepared::Failed(GitFailure::of(FailCategory::Other));
        }
    }
    let Some(dir_s) = dir.to_str() else {
        return Prepared::Failed(GitFailure::of(FailCategory::Other));
    };
    match run_git(ctx, None, &["clone", "--no-checkout", "--", url, dir_s]) {
        Ok(o) if o.ok() => {}
        Ok(o) => return Prepared::Failed(GitFailure::from_out(&o)),
        Err(_) => return Prepared::Failed(GitFailure::of(FailCategory::Other)),
    }
    let branch = cfg.branch.as_str();
    let remote_ref = format!("refs/remotes/origin/{branch}");
    let res = if ref_exists(ctx, dir, &remote_ref) {
        run_git(
            ctx,
            Some(dir),
            &["checkout", "-B", branch, &format!("origin/{branch}")],
        )
    } else {
        run_git(
            ctx,
            Some(dir),
            &["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")],
        )
    };
    match res {
        Ok(o) if o.ok() => Prepared::Ready,
        Ok(o) => Prepared::Failed(GitFailure::from_out(&o)),
        Err(_) => Prepared::Failed(GitFailure::of(FailCategory::Other)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Published {
    Pushed {
        sha: String,
    },
    UpToDate,
    /// `--no-push`: tệp đã ghi vào bản clone, không stage, không commit, không đẩy.
    LocalOnly,
    CommittedNotPushed(GitFailure),
}

fn local_failure() -> Published {
    Published::CommittedNotPushed(GitFailure::of(FailCategory::Other))
}

/// Stage đúng `paths`, commit khi có thay đổi, rồi đẩy `HEAD` lên `refs/heads/<branch>` khi
/// HEAD khác `origin/<branch>`. Không `-A`, không `-f`, không force-push.
pub fn commit_and_push(
    ctx: &GitCtx,
    cfg: &JournalConfig,
    paths: &[String],
    message: &str,
    push: bool,
) -> Published {
    let dir: PathBuf = cfg.clone_dir.clone();
    if !push {
        return Published::LocalOnly;
    }
    if !paths.is_empty() {
        let mut input = Vec::new();
        for p in paths {
            input.extend_from_slice(p.as_bytes());
            input.push(0);
        }
        match run_git_input(
            ctx,
            Some(&dir),
            &["add", "--pathspec-from-file=-", "--pathspec-file-nul"],
            Some(&input),
        ) {
            Ok(o) if o.ok() => {}
            _ => return local_failure(),
        }
    }
    let staged = match run_git(ctx, Some(&dir), &["diff", "--cached", "--quiet"]) {
        Ok(o) if o.timed_out => return local_failure(),
        Ok(o) if o.code == 0 => false,
        Ok(o) if o.code == 1 => true,
        _ => return local_failure(),
    };
    if staged {
        match run_git(ctx, Some(&dir), &["commit", "-q", "-m", message]) {
            Ok(o) if o.ok() => {}
            _ => return local_failure(),
        }
    }
    let Some(head) = rev(ctx, &dir, "HEAD") else {
        return Published::UpToDate;
    };
    let branch = cfg.branch.as_str();
    let remote = rev(ctx, &dir, &format!("refs/remotes/origin/{branch}"));
    if remote.as_deref() == Some(head.as_str()) {
        return Published::UpToDate;
    }
    match run_git(
        ctx,
        Some(&dir),
        &["push", "origin", &format!("HEAD:refs/heads/{branch}")],
    ) {
        Ok(o) if o.ok() => Published::Pushed { sha: head },
        Ok(o) => Published::CommittedNotPushed(GitFailure::from_out(&o)),
        Err(_) => Published::CommittedNotPushed(GitFailure::of(FailCategory::Other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_categories() {
        assert_eq!(classify(128, "", true), FailCategory::Timeout);
        assert_eq!(
            classify(
                128,
                "ssh: Could not resolve hostname x\nfatal: Could not read from remote repository.",
                false
            ),
            FailCategory::Network
        );
        assert_eq!(
            classify(128, "git@x: Permission denied (publickey).\nfatal: Could not read from remote repository.", false),
            FailCategory::SshAuth
        );
        assert_eq!(
            classify(1, " ! [rejected]        HEAD -> main (fetch first)", false),
            FailCategory::Rejected
        );
        assert_eq!(classify(1, "something", false), FailCategory::Other);
    }
}
