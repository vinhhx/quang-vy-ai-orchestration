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

/// Tệp trong `.git/` ghi loại lỗi của lần `fetch` gần nhất (xóa khi `fetch`/`push` thành công):
/// khi không có gì để đẩy, remote không với tới vẫn là một lần đẩy thất bại (D-10).
const FETCH_FAILED_FILE: &str = "agent-brain-journal-fetch-failed";

fn category_code(c: FailCategory) -> &'static str {
    match c {
        FailCategory::Network => "network",
        FailCategory::SshAuth => "ssh",
        FailCategory::Rejected => "rejected",
        FailCategory::Timeout => "timeout",
        FailCategory::Other => "other",
    }
}

fn category_of_code(s: &str) -> FailCategory {
    match s.trim() {
        "network" => FailCategory::Network,
        "ssh" => FailCategory::SshAuth,
        "rejected" => FailCategory::Rejected,
        "timeout" => FailCategory::Timeout,
        _ => FailCategory::Other,
    }
}

fn fetch_mark(dir: &Path) -> PathBuf {
    dir.join(".git").join(FETCH_FAILED_FILE)
}

fn set_fetch_failed(dir: &Path, c: Option<FailCategory>) {
    match c {
        Some(c) => {
            let _ = std::fs::write(fetch_mark(dir), category_code(c));
        }
        None => {
            let _ = std::fs::remove_file(fetch_mark(dir));
        }
    }
}

fn fetch_failed(dir: &Path) -> Option<FailCategory> {
    std::fs::read_to_string(fetch_mark(dir))
        .ok()
        .map(|s| category_of_code(&s))
}

fn nul_list<S: AsRef<str>>(paths: &[S]) -> Vec<u8> {
    let mut input = Vec::new();
    for p in paths {
        input.extend_from_slice(p.as_ref().as_bytes());
        input.push(0);
    }
    input
}

/// Tệp thường (không theo symlink) có dòng đánh dấu sinh tự động.
fn file_has_marker(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_file() => {}
        _ => return false,
    }
    std::fs::read(path)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .map(|s| super::writer::has_marker(&s))
        .unwrap_or(false)
}

fn head_has_marker(ctx: &GitCtx, dir: &Path, rel: &str) -> bool {
    run_git(ctx, Some(dir), &["cat-file", "-p", &format!("HEAD:{rel}")])
        .map(|o| o.ok() && super::writer::has_marker(&o.stdout))
        .unwrap_or(false)
}

/// `.git/index.lock` cũ hơn thời hạn git: do một lần chạy bị ngắt để lại. Gọi khi đang giữ
/// `journal.lock`, nên không có lệnh git nào khác của nhật ký đang chạy.
fn remove_stale_index_lock(ctx: &GitCtx, dir: &Path) {
    let lock = dir.join(".git").join("index.lock");
    let old = std::fs::metadata(&lock)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map(|age| age >= ctx.timeout)
        .unwrap_or(false);
    if old {
        let _ = std::fs::remove_file(&lock);
    }
}

/// Bước 1: đường dẫn đã theo dõi có thay đổi chưa commit. Tệp sinh tự động còn dòng đánh dấu
/// (hoặc tệp cũ có dấu đã bị xóa dở) được khôi phục từ `HEAD`; mọi thay đổi khác → dừng.
fn check_dirty(ctx: &GitCtx, dir: &Path, generated: &BTreeSet<String>) -> Option<Prepared> {
    let out = match run_git(
        ctx,
        Some(dir),
        &["status", "--porcelain=v1", "-z", "--untracked-files=no"],
    ) {
        Ok(o) if o.ok() => o,
        Ok(o) if o.timed_out => return Some(Prepared::Failed(GitFailure::from_out(&o))),
        _ => return Some(Prepared::Stopped(GitStop::NotARepo)),
    };
    let mut entries: Vec<(char, String)> = Vec::new();
    let mut records = out.stdout.split('\0').filter(|r| !r.is_empty());
    while let Some(rec) = records.next() {
        if rec.len() < 4 {
            continue;
        }
        let x = rec.chars().next().unwrap_or(' ');
        entries.push((x, rec[3..].to_string()));
        if x == 'R' || x == 'C' {
            if let Some(orig) = records.next() {
                entries.push((x, orig.to_string()));
            }
        }
    }
    let mut dirty = Vec::new();
    let mut restore = Vec::new();
    for (x, rel) in entries {
        let path = dir.join(&rel);
        let ours = (generated.contains(&rel) && file_has_marker(&path))
            || (std::fs::symlink_metadata(&path).is_err()
                && super::writer::is_managed_path(&rel)
                && head_has_marker(ctx, dir, &rel));
        if !ours {
            dirty.push(rel);
        } else if x != 'A' {
            restore.push(rel);
        }
    }
    if !dirty.is_empty() {
        dirty.sort();
        dirty.dedup();
        return Some(Prepared::Stopped(GitStop::Dirty { paths: dirty }));
    }
    if !restore.is_empty() {
        let res = run_git_input(
            ctx,
            Some(dir),
            &[
                "checkout",
                "HEAD",
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ],
            Some(&nul_list(&restore)),
        );
        if !matches!(res, Ok(ref o) if o.ok()) {
            return Some(Prepared::Stopped(GitStop::Dirty { paths: restore }));
        }
    }
    None
}

enum Integrated {
    Done,
    Stop(GitStop),
    Fail(GitFailure),
}

/// Bước 3: đưa `HEAD` lên `origin/<branch>` mà không viết lại lịch sử đã đẩy.
/// - chỉ đứng sau: `merge --ff-only origin/<branch>`;
/// - rẽ nhánh: `rebase origin/<branch>` (chỉ các commit chưa đẩy của mình);
/// - xung đột: `rebase --abort` rồi dừng (không ghi đè thay đổi của người khác).
fn integrate(ctx: &GitCtx, dir: &Path, branch: &str) -> Integrated {
    let upstream = format!("origin/{branch}");
    if !ref_exists(ctx, dir, &format!("refs/remotes/origin/{branch}")) {
        return Integrated::Done;
    }
    let conflict = |detail: String| Integrated::Stop(GitStop::Conflict { detail });
    if rev(ctx, dir, "HEAD").is_none() {
        return match run_git(ctx, Some(dir), &["checkout", "-B", branch, &upstream]) {
            Ok(o) if o.ok() => Integrated::Done,
            Ok(o) if o.timed_out => Integrated::Fail(GitFailure::from_out(&o)),
            Ok(_) => conflict(upstream),
            Err(_) => Integrated::Fail(GitFailure::of(FailCategory::Other)),
        };
    }
    let counts = match run_git(
        ctx,
        Some(dir),
        &[
            "rev-list",
            "--left-right",
            "--count",
            &format!("HEAD...{upstream}"),
        ],
    ) {
        Ok(o) if o.ok() => o.stdout,
        Ok(o) => return Integrated::Fail(GitFailure::from_out(&o)),
        Err(_) => return Integrated::Fail(GitFailure::of(FailCategory::Other)),
    };
    let mut nums = counts
        .split_whitespace()
        .map(|n| n.parse::<u64>().unwrap_or(0));
    let ahead = nums.next().unwrap_or(0);
    let behind = nums.next().unwrap_or(0);
    if behind == 0 {
        return Integrated::Done;
    }
    if ahead == 0 {
        return match run_git(ctx, Some(dir), &["merge", "--ff-only", "-q", &upstream]) {
            Ok(o) if o.ok() => Integrated::Done,
            Ok(o) if o.timed_out => Integrated::Fail(GitFailure::from_out(&o)),
            Ok(_) => conflict(upstream),
            Err(_) => Integrated::Fail(GitFailure::of(FailCategory::Other)),
        };
    }
    match run_git(ctx, Some(dir), &["rebase", "-q", &upstream]) {
        Ok(o) if o.ok() => Integrated::Done,
        _ => {
            let unmerged = run_git(
                ctx,
                Some(dir),
                &["diff", "--name-only", "--diff-filter=U", "-z"],
            )
            .map(|o| o.stdout)
            .unwrap_or_default();
            let _ = run_git(ctx, Some(dir), &["rebase", "--abort"]);
            let mut paths: Vec<&str> = unmerged.split('\0').filter(|p| !p.is_empty()).collect();
            paths.sort();
            paths.dedup();
            if paths.is_empty() {
                conflict(upstream)
            } else {
                conflict(paths.join(", "))
            }
        }
    }
}

/// Bản clone đã có: khóa index cũ, rebase dở → hủy, kiểm tra thay đổi tay, `fetch`, rồi
/// tích hợp. `fetch` lỗi (mất mạng) vẫn trả `Ready`: commit cục bộ, lần đẩy sẽ lỗi và bị đếm.
fn sync_existing(
    ctx: &GitCtx,
    cfg: &JournalConfig,
    dir: &Path,
    generated: &BTreeSet<String>,
) -> Prepared {
    remove_stale_index_lock(ctx, dir);
    let git_dir = dir.join(".git");
    if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
        let _ = run_git(ctx, Some(dir), &["rebase", "--abort"]);
    }
    if let Some(stop) = check_dirty(ctx, dir, generated) {
        return stop;
    }
    let branch = cfg.branch.as_str();
    match run_git(ctx, Some(dir), &["fetch", "-q", "origin", branch]) {
        Ok(o) if o.ok() => set_fetch_failed(dir, None),
        Ok(o) => {
            set_fetch_failed(dir, Some(classify(o.code, &o.stderr, o.timed_out)));
            return Prepared::Ready;
        }
        Err(_) => {
            set_fetch_failed(dir, Some(FailCategory::Other));
            return Prepared::Ready;
        }
    }
    match integrate(ctx, dir, branch) {
        Integrated::Done => Prepared::Ready,
        Integrated::Stop(s) => Prepared::Stopped(s),
        Integrated::Fail(f) => Prepared::Failed(f),
    }
}

/// Bảo đảm có bản clone ở `cfg.clone_dir` (clone khi chưa có) và đồng bộ bản clone đã có với
/// `origin/<branch>` (RESEARCH §4: kiểm tra thay đổi tay, fetch, ff/rebase, hủy khi xung đột).
pub fn prepare(ctx: &GitCtx, cfg: &JournalConfig, generated: &BTreeSet<String>) -> Prepared {
    let dir: &Path = &cfg.clone_dir;
    if dir.join(".git").exists() {
        return sync_existing(ctx, cfg, dir, generated);
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

fn push_head(ctx: &GitCtx, dir: &Path, branch: &str) -> Result<(), GitFailure> {
    match run_git(
        ctx,
        Some(dir),
        &["push", "-q", "origin", &format!("HEAD:refs/heads/{branch}")],
    ) {
        Ok(o) if o.ok() => Ok(()),
        Ok(o) => Err(GitFailure::from_out(&o)),
        Err(_) => Err(GitFailure::of(FailCategory::Other)),
    }
}

/// Stage đúng `paths` (tệp còn trên đĩa: `add`; tệp đã xóa: `rm --cached`), commit khi có thay
/// đổi, rồi đẩy `HEAD` lên `refs/heads/<branch>` khi HEAD khác `origin/<branch>`. Bị từ chối vì
/// remote đã đổi → một lần `fetch` + tích hợp + đẩy lại. Không `-A`, không ép, không viết lại.
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
    let (present, missing): (Vec<&String>, Vec<&String>) = paths
        .iter()
        .partition(|p| std::fs::symlink_metadata(dir.join(p)).is_ok());
    if !present.is_empty() {
        match run_git_input(
            ctx,
            Some(&dir),
            &["add", "--pathspec-from-file=-", "--pathspec-file-nul"],
            Some(&nul_list(&present)),
        ) {
            Ok(o) if o.ok() => {}
            _ => return local_failure(),
        }
    }
    if !missing.is_empty() {
        match run_git_input(
            ctx,
            Some(&dir),
            &[
                "rm",
                "-q",
                "--cached",
                "--ignore-unmatch",
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ],
            Some(&nul_list(&missing)),
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
        // Không có gì để đẩy; remote không với tới ở lần `fetch` này vẫn là một lần thất bại.
        return match fetch_failed(&dir) {
            Some(c) => Published::CommittedNotPushed(GitFailure::of(c)),
            None => Published::UpToDate,
        };
    }
    let first = match push_head(ctx, &dir, branch) {
        Ok(()) => {
            set_fetch_failed(&dir, None);
            return Published::Pushed { sha: head };
        }
        Err(f) => f,
    };
    if first.category != FailCategory::Rejected {
        return Published::CommittedNotPushed(first);
    }
    // Remote đã đổi giữa `fetch` và `push`: lấy lại, tích hợp commit của mình, đẩy lại một lần.
    match run_git(ctx, Some(&dir), &["fetch", "-q", "origin", branch]) {
        Ok(o) if o.ok() => {}
        Ok(o) => return Published::CommittedNotPushed(GitFailure::from_out(&o)),
        Err(_) => return Published::CommittedNotPushed(first),
    }
    match integrate(ctx, &dir, branch) {
        Integrated::Done => {}
        Integrated::Stop(_) => return Published::CommittedNotPushed(first),
        Integrated::Fail(f) => return Published::CommittedNotPushed(f),
    }
    let Some(head) = rev(ctx, &dir, "HEAD") else {
        return Published::CommittedNotPushed(first);
    };
    match push_head(ctx, &dir, branch) {
        Ok(()) => {
            set_fetch_failed(&dir, None);
            Published::Pushed { sha: head }
        }
        Err(f) => Published::CommittedNotPushed(f),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_examples() {
        assert_eq!(classify(128, "", true), FailCategory::Timeout);
        assert_eq!(
            classify(128, "ssh: Could not resolve hostname x", false),
            FailCategory::Network
        );
        assert_eq!(
            classify(128, "Host key verification failed.", false),
            FailCategory::SshAuth
        );
        assert_eq!(
            classify(
                1,
                "error: failed to push some refs\nhint: non-fast-forward",
                false
            ),
            FailCategory::Rejected
        );
        assert_eq!(classify(1, "fatal: something", false), FailCategory::Other);
        for c in [
            FailCategory::Network,
            FailCategory::SshAuth,
            FailCategory::Rejected,
            FailCategory::Timeout,
            FailCategory::Other,
        ] {
            assert_eq!(category_of_code(category_code(c)), c);
        }
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
