//! Lệnh của dự án chạy trong container docker đang chạy (project-container).
//!
//! Máy có một container đang chạy mount repo dự án (vd `bizsurvey_app`: repo → `/var/www`) thì
//! các lệnh `php` / `composer` agent gọi trong run được chuyển vào container đó qua một shim do
//! agent-brain viết vào `<run_dir>/bin/`. Agent không bao giờ gọi `docker` (nó vẫn nằm trong
//! `FORBIDDEN_PROGRAMS`); chỉ shim gọi, bằng đường dẫn tuyệt đối tìm lúc dò. Máy không có
//! container như vậy thì chạy như cũ.
//!
//! Dò chỉ dùng `docker ps --format {{.ID}}` và `docker inspect --format` lấy tên, trạng thái và
//! danh sách mount — không bao giờ đọc biến môi trường của container.

use crate::projects::{ContainerMode, ContainerSetting, Project};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How long one detection result is reused for the same project root.
const CACHE_TTL: Duration = Duration::from_secs(60);
/// Cap of each docker call made while detecting.
const DOCKER_CALL_CAP: Duration = Duration::from_secs(5);
/// Compose files read per project, and the largest one read.
const MAX_COMPOSE_FILES: usize = 20;
const MAX_COMPOSE_BYTES: u64 = 256 * 1024;
/// Depth of the walk under `setup/docker`, `setup/local` and `docker`.
const MAX_COMPOSE_DEPTH: usize = 6;
/// Exit code of a shim called outside the mounted folder.
pub const SHIM_OUTSIDE_EXIT: i32 = 97;

/// The `docker inspect` form used for detection: one JSON object per container with only the
/// name, the running flag and the mounts. A bare inspect would print the container env (secrets).
const INSPECT_FORMAT: &str =
    r#"{"Name":{{json .Name}},"State":{"Running":{{json .State.Running}}},"Mounts":{{json .Mounts}}}"#;

/// A running container that mounts the project repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerInfo {
    pub name: String,
    /// The project repo root on this machine (realpath).
    pub source: PathBuf,
    /// The same folder inside the container.
    pub dest: String,
    /// The bind mount's host folder (the repo root or one of its parents, realpath).
    pub mount_source: PathBuf,
    /// The bind mount's folder inside the container.
    pub mount_dest: String,
    /// Absolute path of the docker client found in `PATH` while detecting.
    pub docker_bin: PathBuf,
}

/// Result of one detection: the chosen container, or why there is none.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Detection {
    pub info: Option<ContainerInfo>,
    /// Why this container was chosen, or why there is none.
    pub reason: String,
    /// Names of every running container that mounts the repo.
    pub candidates: Vec<String>,
    /// Warnings for the run (an explicit `container.name` that does not qualify).
    pub warnings: Vec<String>,
}

/// The docker calls detection needs; tests inject a fake one.
pub trait Probe {
    /// Absolute path of the docker client, `None` when there is none.
    fn docker_bin(&self) -> Option<PathBuf>;
    /// Ids of the running containers (`docker ps --format {{.ID}}`).
    fn ps(&self, docker: &Path) -> Result<Vec<String>, String>;
    /// `docker inspect` output for `ids`: a JSON array or one JSON object per line.
    fn inspect(&self, docker: &Path, ids: &[String]) -> Result<String, String>;
}

/// The real docker client.
pub struct DockerProbe;

/// `docker` in the process `PATH`, as an absolute path.
pub fn find_docker_in_path() -> Option<PathBuf> {
    find_in_path("docker", std::env::var_os("PATH")?.as_os_str())
}

fn find_in_path(name: &str, path: &std::ffi::OsStr) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(path)
        .filter(|d| d.is_absolute())
        .map(|d| d.join(name))
        .find(|p| {
            std::fs::metadata(p)
                .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        })
}

impl Probe for DockerProbe {
    fn docker_bin(&self) -> Option<PathBuf> {
        find_docker_in_path()
    }

    fn ps(&self, docker: &Path) -> Result<Vec<String>, String> {
        let args = vec!["ps".to_string(), "--format".to_string(), "{{.ID}}".to_string()];
        match crate::dsh::docker_capture(
            &docker.to_string_lossy(),
            &args,
            &crate::dsh::scrubbed_current_env(),
            DOCKER_CALL_CAP,
        ) {
            Some((true, out)) => Ok(out
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()),
            Some((false, _)) => Err("docker ps lỗi (docker không chạy hoặc không có quyền)".into()),
            None => Err("docker ps không chạy được hoặc quá 5 giây".into()),
        }
    }

    fn inspect(&self, docker: &Path, ids: &[String]) -> Result<String, String> {
        let mut args = vec![
            "inspect".to_string(),
            "--format".to_string(),
            INSPECT_FORMAT.to_string(),
        ];
        args.extend(ids.iter().cloned());
        match crate::dsh::docker_capture(
            &docker.to_string_lossy(),
            &args,
            &crate::dsh::scrubbed_current_env(),
            DOCKER_CALL_CAP,
        ) {
            Some((true, out)) => Ok(out),
            Some((false, _)) => Err("docker inspect lỗi".into()),
            None => Err("docker inspect không chạy được hoặc quá 5 giây".into()),
        }
    }
}

/// One container as read from `docker inspect`.
#[derive(Debug, Clone)]
struct Inspected {
    name: String,
    running: bool,
    /// `(Source, Destination)` of each bind mount.
    binds: Vec<(String, String)>,
}

fn inspected_of(v: &serde_json::Value) -> Option<Inspected> {
    let name = v.get("Name")?.as_str()?.trim_start_matches('/').to_string();
    if name.is_empty() {
        return None;
    }
    let state = v.get("State");
    let running = state
        .and_then(|s| s.get("Running"))
        .and_then(|r| r.as_bool())
        .or_else(|| {
            state
                .and_then(|s| s.get("Status"))
                .and_then(|s| s.as_str())
                .map(|s| s == "running")
        })
        .unwrap_or(false);
    let binds = v
        .get("Mounts")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|m| m.get("Type").and_then(|t| t.as_str()) == Some("bind"))
                .filter_map(|m| {
                    Some((
                        m.get("Source")?.as_str()?.to_string(),
                        m.get("Destination")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Inspected {
        name,
        running,
        binds,
    })
}

/// Parses `docker inspect` output: the default JSON array, or one object per line.
fn parse_inspect(text: &str) -> Vec<Inspected> {
    if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str::<serde_json::Value>(text) {
        return arr.iter().filter_map(inspected_of).collect();
    }
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| inspected_of(&v))
        .collect()
}

fn realpath(p: &str) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p))
}

/// `dest` joined with the relative path `rest` (`/`-separated).
fn join_dest(dest: &str, rest: &Path) -> String {
    let base = if dest.len() > 1 {
        dest.trim_end_matches('/')
    } else {
        dest
    };
    let rest = rest.to_string_lossy();
    if rest.is_empty() {
        base.to_string()
    } else if base.ends_with('/') {
        format!("{base}{rest}")
    } else {
        format!("{base}/{rest}")
    }
}

/// The bind mount of `c` whose source is `root` or its direct parent, the longest source first:
/// `(mount_source, mount_dest, dest of root)`. A mount of a farther ancestor (a harness that
/// mounts all of `/mnt/data`) does not count as the project's container.
fn mount_for(c: &Inspected, root: &Path) -> Option<(PathBuf, String, String)> {
    c.binds
        .iter()
        .filter_map(|(src, dst)| {
            let src = realpath(src);
            if src.as_path() != root && root.parent() != Some(src.as_path()) {
                return None;
            }
            let rest = root.strip_prefix(&src).ok()?;
            Some((src.clone(), dst.clone(), join_dest(dst, rest)))
        })
        .max_by_key(|(src, _, _)| src.as_os_str().len())
}

fn is_app_name(name: &str) -> bool {
    name.ends_with("_app") || name.ends_with("-app")
}

fn compose_file_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("compose") && (lower.ends_with(".yml") || lower.ends_with(".yaml"))
}

fn walk_compose(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_COMPOSE_DEPTH || out.len() >= MAX_COMPOSE_FILES {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if out.len() >= MAX_COMPOSE_FILES {
            return;
        }
        let Ok(ft) = e.file_type() else { continue };
        let path = e.path();
        if ft.is_dir() {
            walk_compose(&path, depth + 1, out);
        } else if ft.is_file() && compose_file_name(&e.file_name().to_string_lossy()) {
            out.push(path);
        }
    }
}

/// The compose files of the repo: `setup/docker/**`, `setup/local/**`, `docker/**`, and
/// `docker-compose*.y*ml` / `compose*.y*ml` at the root. At most 20.
pub fn compose_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for sub in ["setup/docker", "setup/local", "docker"] {
        walk_compose(&root.join(sub), 0, &mut out);
    }
    if let Ok(rd) = std::fs::read_dir(root) {
        let mut names: Vec<String> = rd
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| {
                let l = n.to_ascii_lowercase();
                (l.starts_with("docker-compose") || l.starts_with("compose"))
                    && (l.ends_with(".yml") || l.ends_with(".yaml"))
            })
            .collect();
        names.sort();
        for n in names {
            if out.len() >= MAX_COMPOSE_FILES {
                break;
            }
            out.push(root.join(n));
        }
    }
    out.truncate(MAX_COMPOSE_FILES);
    out
}

/// Every `container_name:` value in the repo's compose files (read as text, no YAML parser).
pub fn compose_container_names(root: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for f in compose_files(root) {
        let small = std::fs::metadata(&f)
            .map(|m| m.len() <= MAX_COMPOSE_BYTES)
            .unwrap_or(false);
        if !small {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        for line in text.lines() {
            let t = line.trim_start();
            if let Some(v) = t.strip_prefix("container_name:") {
                let v = v.split(" #").next().unwrap_or("").trim();
                let v = v.trim_matches(|c| c == '"' || c == '\'').trim();
                if !v.is_empty() {
                    names.insert(v.to_string());
                }
            }
        }
    }
    names
}

/// Rank of a candidate (lower wins) and the reason shown for it.
fn rank(name: &str, compose: &BTreeSet<String>, project_id: &str) -> (u8, &'static str) {
    let in_compose = compose.contains(name);
    if in_compose && is_app_name(name) {
        (0, "tên có trong container_name của file compose, kết thúc _app/-app")
    } else if in_compose {
        (1, "tên có trong container_name của file compose")
    } else if is_app_name(name) {
        (2, "tên kết thúc _app/-app")
    } else if !project_id.is_empty() && name.contains(project_id) {
        (3, "tên chứa id dự án")
    } else {
        (4, "tên nhỏ nhất theo thứ tự chữ")
    }
}

/// Detection with an injected probe; no cache. `root` is the project repo root.
pub fn detect_with(
    project_id: &str,
    root: &Path,
    mode: &ContainerMode,
    probe: &dyn Probe,
) -> Detection {
    let none = |reason: String| Detection {
        reason,
        ..Default::default()
    };
    if *mode == ContainerMode::Off {
        return none("container: false — chạy trên máy".to_string());
    }
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let Some(docker) = probe.docker_bin() else {
        return none("không tìm thấy docker trong PATH".to_string());
    };
    let ids = match probe.ps(&docker) {
        Ok(ids) => ids,
        Err(e) => return none(e),
    };
    if ids.is_empty() {
        return none("không có container nào đang chạy".to_string());
    }
    let text = match probe.inspect(&docker, &ids) {
        Ok(t) => t,
        Err(e) => return none(e),
    };
    let mut matches: Vec<(Inspected, PathBuf, String, String)> = parse_inspect(&text)
        .into_iter()
        .filter(|c| c.running)
        .filter_map(|c| {
            let (ms, md, dest) = mount_for(&c, &root)?;
            Some((c, ms, md, dest))
        })
        .collect();
    matches.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    let candidates: Vec<String> = matches.iter().map(|m| m.0.name.clone()).collect();

    let build = |m: &(Inspected, PathBuf, String, String)| ContainerInfo {
        name: m.0.name.clone(),
        source: root.clone(),
        dest: m.3.clone(),
        mount_source: m.1.clone(),
        mount_dest: m.2.clone(),
        docker_bin: docker.clone(),
    };

    if let ContainerMode::Named(want) = mode {
        return match matches.iter().find(|m| &m.0.name == want) {
            Some(m) => Detection {
                info: Some(build(m)),
                reason: format!("chỉ định trong cấu hình dự án (container.name = {want})"),
                candidates,
                warnings: Vec::new(),
            },
            None => {
                let w = format!(
                    "Container '{want}' (cấu hình dự án) không chạy hoặc không mount repo {} — lệnh chạy trên máy",
                    root.display()
                );
                Detection {
                    info: None,
                    reason: w.clone(),
                    candidates,
                    warnings: vec![w],
                }
            }
        };
    }

    match matches.len() {
        0 => none(format!(
            "không có container đang chạy nào mount {}",
            root.display()
        )),
        1 => Detection {
            info: Some(build(&matches[0])),
            reason: "container đang chạy duy nhất mount repo".to_string(),
            candidates,
            warnings: Vec::new(),
        },
        _ => {
            let compose = compose_container_names(&root);
            let best = matches
                .iter()
                .min_by(|a, b| {
                    rank(&a.0.name, &compose, project_id)
                        .0
                        .cmp(&rank(&b.0.name, &compose, project_id).0)
                        .then_with(|| a.0.name.cmp(&b.0.name))
                })
                .expect("non-empty");
            let why = rank(&best.0.name, &compose, project_id).1;
            Detection {
                info: Some(build(best)),
                reason: format!("{} container khớp — chọn theo: {why}", matches.len()),
                candidates,
                warnings: Vec::new(),
            }
        }
    }
}

type CacheKey = (PathBuf, String, String);

fn cache() -> &'static Mutex<HashMap<CacheKey, (Instant, Detection)>> {
    static CACHE: OnceLock<Mutex<HashMap<CacheKey, (Instant, Detection)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Detection for the project with the real docker client, cached 60 s per project root.
pub fn detect_mode(project: &Project, mode: &ContainerMode) -> Detection {
    let root = project.root();
    let key = (root.clone(), project.id.clone(), format!("{mode:?}"));
    if let Ok(c) = cache().lock() {
        if let Some((at, d)) = c.get(&key) {
            if at.elapsed() < CACHE_TTL {
                return d.clone();
            }
        }
    }
    let d = detect_with(&project.id, &root, mode, &DockerProbe);
    if let Ok(mut c) = cache().lock() {
        c.insert(key, (Instant::now(), d.clone()));
    }
    d
}

/// Auto detection for the project (cached); `None` when no running container mounts the repo.
pub fn detect(project: &Project) -> Option<ContainerInfo> {
    detect_mode(project, &ContainerMode::Auto).info
}

/// True when `s` can be written into the shim: not empty, none of `'"$\`` or a glob character,
/// no control character, no leading or trailing whitespace.
pub fn shim_safe(s: &str) -> bool {
    !s.is_empty()
        && s.trim() == s
        && !s
            .chars()
            .any(|c| c.is_control() || "'\"$\\`*?[]".contains(c))
}

/// The shim script for `cmd`. `Err` when a value cannot be written safely.
pub fn shim_script(info: &ContainerInfo, cmd: &str) -> Result<String, String> {
    let source = info.mount_source.to_string_lossy().to_string();
    let docker = info.docker_bin.to_string_lossy().to_string();
    let dest = if info.mount_dest.len() > 1 {
        info.mount_dest.trim_end_matches('/').to_string()
    } else {
        info.mount_dest.clone()
    };
    for (what, v) in [
        ("thư mục mount", source.as_str()),
        ("đường dẫn trong container", dest.as_str()),
        ("docker", docker.as_str()),
        ("tên container", info.name.as_str()),
        ("lệnh", cmd),
    ] {
        if !shim_safe(v) {
            return Err(format!("{what} '{v}' có ký tự không an toàn cho shim"));
        }
    }
    if source == "/" || !info.mount_source.is_absolute() || !info.docker_bin.is_absolute() {
        return Err("thư mục mount hoặc đường dẫn docker không hợp lệ cho shim".to_string());
    }
    if !crate::projects::valid_container_name(&info.name) {
        return Err(format!("tên container '{}' không hợp lệ", info.name));
    }
    crate::projects::validate_container_command(cmd)?;
    let name = &info.name;
    Ok(format!(
        "#!/bin/sh\n\
         # agent-brain: chạy '{cmd}' trong container {name}\n\
         case \"$PWD/\" in\n  \
         \"{source}/\"*) rel=\"${{PWD#{source}}}\" ;;\n  \
         *) echo \"agent-brain: $PWD không nằm trong thư mục được mount vào container {name}\" >&2; exit {SHIM_OUTSIDE_EXIT} ;;\n\
         esac\n\
         exec \"{docker}\" exec -i -w \"{dest}$rel\" \"{name}\" {cmd} \"$@\"\n"
    ))
}

/// Writes one shim per command into `<run_dir>/bin` (0700 folder, 0700 files) and returns the
/// folder. Every script is checked before anything is written.
pub fn write_shims(run_dir: &Path, info: &ContainerInfo, commands: &[String]) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    let scripts: Vec<(String, String)> = commands
        .iter()
        .map(|c| shim_script(info, c).map(|s| (c.clone(), s)))
        .collect::<Result<_, _>>()?;
    let bin = run_dir.join("bin");
    std::fs::create_dir_all(&bin).map_err(|e| format!("không tạo được {}: {e}", bin.display()))?;
    let _ = std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700));
    for (cmd, script) in scripts {
        let p = bin.join(&cmd);
        std::fs::write(&p, script).map_err(|e| format!("không ghi được {}: {e}", p.display()))?;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("không đặt quyền {}: {e}", p.display()))?;
    }
    Ok(bin)
}

/// Puts `bin` first in the env's `PATH`.
pub fn prepend_path(env: &mut BTreeMap<String, String>, bin: &Path) {
    let b = bin.to_string_lossy().to_string();
    let new = match env.get("PATH").filter(|p| !p.is_empty()) {
        Some(p) => format!("{b}:{p}"),
        None => format!("{b}:/usr/local/bin:/usr/bin:/bin"),
    };
    env.insert("PATH".to_string(), new);
}

fn commands_text(commands: &[String]) -> String {
    commands
        .iter()
        .map(|c| format!("`{c}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The block appended to the role's system prompt when the shim is on.
pub fn prompt_block(info: &ContainerInfo, commands: &[String]) -> String {
    format!(
        "Môi trường: lệnh {} của dự án này chạy trong container docker `{}` (thư mục repo = `{}`). Gọi chúng như bình thường từ thư mục repo; đường dẫn trong output là đường dẫn trong container. Không gọi `docker` trực tiếp.",
        commands_text(commands),
        info.name,
        info.dest
    )
}

/// The container of one run, decided before the engine starts.
#[derive(Debug, Clone, Default)]
pub struct RunContainer {
    /// The container and the commands to shim; `None` = commands run on the machine.
    pub active: Option<(ContainerInfo, Vec<String>)>,
    /// Warnings stored in the run's `metrics_json.warnings`.
    pub warnings: Vec<String>,
}

impl RunContainer {
    /// `{"name", "dest", "commands"}` for the run's `metrics_json.container`.
    pub fn metrics(&self) -> Option<serde_json::Value> {
        self.active.as_ref().map(|(info, cmds)| {
            serde_json::json!({"name": info.name, "dest": info.dest, "commands": cmds})
        })
    }
}

/// Decides the container of a run from a setting and a detection: the run's `cwd` must lie
/// inside the mounted folder, and every shim value must be safe.
pub fn decide(setting: &ContainerSetting, det: &Detection, cwd: &Path) -> RunContainer {
    let mut out = RunContainer {
        active: None,
        warnings: det.warnings.clone(),
    };
    let Some(info) = det.info.clone() else {
        return out;
    };
    let cwd = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    if !cwd.starts_with(&info.mount_source) {
        out.warnings.push(format!(
            "Container {} không mount worktree của task — lệnh {} chạy trên máy",
            info.name,
            setting.commands.join(", ")
        ));
        return out;
    }
    if let Some(e) = setting
        .commands
        .iter()
        .find_map(|c| shim_script(&info, c).err())
    {
        out.warnings.push(format!(
            "Không tạo shim container {}: {e} — lệnh {} chạy trên máy",
            info.name,
            setting.commands.join(", ")
        ));
        return out;
    }
    out.active = Some((info, setting.commands.clone()));
    out
}

/// The container of a run of `project` working in `cwd`, from the projects map and a cached
/// detection.
pub fn for_run(map: &serde_json::Value, project: &Project, cwd: &Path) -> RunContainer {
    let setting = match crate::projects::container_of(map, &project.id) {
        Ok(s) => s,
        Err(e) => {
            return RunContainer {
                active: None,
                warnings: vec![format!(
                    "Cấu hình container của dự án không hợp lệ: {e} — lệnh chạy trên máy"
                )],
            }
        }
    };
    if setting.mode == ContainerMode::Off {
        return RunContainer::default();
    }
    let det = detect_mode(project, &setting.mode);
    decide(&setting, &det, cwd)
}

/// `agent-brain container PROJECT`: prints the detection result (never the container env).
pub fn cmd_container(query: &str) -> crate::error::BrainResult<i32> {
    let map = crate::projects::load_map(&crate::config::map_path())?;
    let project = crate::projects::select_project(query, &map)?;
    let root = project.root();
    let mut lines = vec![format!("Dự án {} ({})", project.id, root.display())];
    let setting = match crate::projects::container_of(&map, &project.id) {
        Ok(s) => s,
        Err(e) => {
            lines.push(format!("Cấu hình container không hợp lệ: {e}"));
            lines.push("→ lệnh chạy trên máy".to_string());
            crate::term::emit_out(&lines.join("\n"));
            return Ok(1);
        }
    };
    let mode_text = match &setting.mode {
        ContainerMode::Auto => "auto (tự dò)".to_string(),
        ContainerMode::Off => "false (tắt)".to_string(),
        ContainerMode::Named(n) => format!("chỉ dùng '{n}'"),
    };
    lines.push(format!("Cấu hình: {mode_text}"));
    let det = detect_with(&project.id, &root, &setting.mode, &DockerProbe);
    match &det.info {
        Some(info) => {
            lines.push(format!("Container: {}", info.name));
            lines.push(format!("Thư mục: {} -> {}", info.source.display(), info.dest));
            if info.mount_source != info.source {
                lines.push(format!(
                    "Mount: {} -> {}",
                    info.mount_source.display(),
                    info.mount_dest
                ));
            }
            lines.push(format!("Docker: {}", info.docker_bin.display()));
            lines.push(format!("Lệnh chuyển vào container: {}", setting.commands.join(", ")));
            lines.push(format!("Lý do: {}", det.reason));
            if det.candidates.len() > 1 {
                lines.push(format!("Các container khớp: {}", det.candidates.join(", ")));
            }
            let check = decide(&setting, &det, &root);
            for w in &check.warnings {
                lines.push(format!("Cảnh báo: {w}"));
            }
        }
        None => {
            lines.push(format!("Không có container: {}", det.reason));
            for w in &det.warnings {
                if *w != det.reason {
                    lines.push(format!("Cảnh báo: {w}"));
                }
            }
            lines.push("→ lệnh chạy trên máy".to_string());
        }
    }
    crate::term::emit_out(&lines.join("\n"));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    struct FakeProbe {
        docker: Option<PathBuf>,
        ps: Result<Vec<String>, String>,
        inspect: String,
    }

    impl Probe for FakeProbe {
        fn docker_bin(&self) -> Option<PathBuf> {
            self.docker.clone()
        }
        fn ps(&self, _: &Path) -> Result<Vec<String>, String> {
            self.ps.clone()
        }
        fn inspect(&self, _: &Path, _: &[String]) -> Result<String, String> {
            Ok(self.inspect.clone())
        }
    }

    fn probe(containers: &[(&str, bool, &str, &str)]) -> FakeProbe {
        let arr: Vec<serde_json::Value> = containers
            .iter()
            .map(|(name, running, src, dst)| {
                serde_json::json!({
                    "Name": format!("/{name}"),
                    "State": {"Status": if *running {"running"} else {"exited"}, "Running": running},
                    "Config": {"Env": ["SECRET=x"]},
                    "Mounts": [
                        {"Type": "volume", "Source": "/var/lib/docker/volumes/x", "Destination": "/data"},
                        {"Type": "bind", "Source": src, "Destination": dst}
                    ]
                })
            })
            .collect();
        FakeProbe {
            docker: Some(PathBuf::from("/usr/bin/docker")),
            ps: Ok(containers.iter().map(|c| c.0.to_string()).collect()),
            inspect: serde_json::Value::Array(arr).to_string(),
        }
    }

    fn real(p: &Path) -> PathBuf {
        std::fs::canonicalize(p).unwrap()
    }

    #[test]
    fn single_container_on_repo_root() {
        let t = tempfile::tempdir().unwrap();
        let root = real(t.path());
        let pr = probe(&[("x_app", true, root.to_str().unwrap(), "/var/www")]);
        let d = detect_with("x", &root, &ContainerMode::Auto, &pr);
        let info = d.info.unwrap();
        assert_eq!(info.name, "x_app");
        assert_eq!(info.dest, "/var/www");
        assert_eq!(info.source, root);
        assert_eq!(info.docker_bin, PathBuf::from("/usr/bin/docker"));
    }

    #[test]
    fn parent_mount_adds_the_rest() {
        let t = tempfile::tempdir().unwrap();
        let parent = real(t.path());
        let root = parent.join("laravel/proj");
        std::fs::create_dir_all(&root).unwrap();
        let laravel = parent.join("laravel");
        let pr = probe(&[("p_app", true, laravel.to_str().unwrap(), "/src/")]);
        let info = detect_with("proj", &root, &ContainerMode::Auto, &pr).info.unwrap();
        assert_eq!(info.dest, "/src/proj");
        assert_eq!(info.mount_source, laravel);
        assert_eq!(info.mount_dest, "/src/");
        // A farther ancestor (a harness mounting a whole disk) is not the project's container.
        let pr = probe(&[("harness", true, parent.to_str().unwrap(), "/src")]);
        assert!(detect_with("proj", &root, &ContainerMode::Auto, &pr).info.is_none());
    }

    #[test]
    fn longest_mount_wins_in_one_container() {
        let t = tempfile::tempdir().unwrap();
        let parent = real(t.path());
        let root = parent.join("r");
        std::fs::create_dir_all(&root).unwrap();
        let mut pr = probe(&[("a_app", true, parent.to_str().unwrap(), "/up")]);
        let mut v: serde_json::Value = serde_json::from_str(&pr.inspect).unwrap();
        v[0]["Mounts"].as_array_mut().unwrap().push(serde_json::json!(
            {"Type": "bind", "Source": root.to_str().unwrap(), "Destination": "/var/www"}
        ));
        pr.inspect = v.to_string();
        let info = detect_with("r", &root, &ContainerMode::Auto, &pr).info.unwrap();
        assert_eq!(info.dest, "/var/www");
    }

    #[test]
    fn no_mount_or_stopped_is_none() {
        let t = tempfile::tempdir().unwrap();
        let root = real(t.path());
        let other = tempfile::tempdir().unwrap();
        let pr = probe(&[("x_app", true, real(other.path()).to_str().unwrap(), "/var/www")]);
        assert!(detect_with("x", &root, &ContainerMode::Auto, &pr).info.is_none());
        let pr = probe(&[("x_app", false, root.to_str().unwrap(), "/var/www")]);
        assert!(detect_with("x", &root, &ContainerMode::Auto, &pr).info.is_none());
        let mut pr = probe(&[("x_app", true, root.to_str().unwrap(), "/var/www")]);
        pr.ps = Err("docker ps lỗi".into());
        assert!(detect_with("x", &root, &ContainerMode::Auto, &pr).info.is_none());
        pr.docker = None;
        assert!(detect_with("x", &root, &ContainerMode::Auto, &pr).info.is_none());
    }

    #[test]
    fn reduced_inspect_format_lines_are_read() {
        let t = tempfile::tempdir().unwrap();
        let root = real(t.path());
        let line = serde_json::json!({
            "Name": "/x_app", "State": {"Running": true},
            "Mounts": [{"Type": "bind", "Source": root.to_str().unwrap(), "Destination": "/var/www"}]
        });
        let pr = FakeProbe {
            docker: Some(PathBuf::from("/usr/bin/docker")),
            ps: Ok(vec!["abc".into()]),
            inspect: format!("{line}\n"),
        };
        assert_eq!(
            detect_with("x", &root, &ContainerMode::Auto, &pr).info.unwrap().name,
            "x_app"
        );
    }

    #[test]
    fn compose_name_wins_then_app_suffix() {
        let t = tempfile::tempdir().unwrap();
        let root = real(t.path());
        let r = root.to_str().unwrap();
        // No compose file: `_app` wins over the alphabetically smaller name.
        let pr = probe(&[("a_queue", true, r, "/var/www"), ("x_app", true, r, "/var/www")]);
        let d = detect_with("x", &root, &ContainerMode::Auto, &pr);
        assert_eq!(d.info.unwrap().name, "x_app");
        assert_eq!(d.candidates, vec!["a_queue", "x_app"]);

        // Compose names the worker: it wins over a non-compose `_app`.
        std::fs::create_dir_all(root.join("setup/docker")).unwrap();
        std::fs::write(
            root.join("setup/docker/docker-compose.yml"),
            "services:\n  app:\n    container_name: x_worker\n",
        )
        .unwrap();
        let d = detect_with("x", &root, &ContainerMode::Auto, &pr2(r));
        assert_eq!(d.info.unwrap().name, "x_worker");

        // Two compose names: the `_app` one wins.
        std::fs::write(
            root.join("setup/docker/docker-compose.yml"),
            "services:\n  app:\n    container_name: \"x_app\"\n  w:\n    container_name: x_worker\n",
        )
        .unwrap();
        let d = detect_with("x", &root, &ContainerMode::Auto, &pr2(r));
        assert_eq!(d.info.unwrap().name, "x_app");

        fn pr2(r: &str) -> FakeProbe {
            probe(&[
                ("x_app", true, r, "/var/www"),
                ("x_worker", true, r, "/var/www"),
                ("a_db", true, r, "/var/www"),
            ])
        }
    }

    #[test]
    fn project_id_then_alphabetical() {
        let t = tempfile::tempdir().unwrap();
        let root = real(t.path());
        let r = root.to_str().unwrap();
        let pr = probe(&[("b_proj_web", true, r, "/w"), ("a_web", true, r, "/w")]);
        assert_eq!(
            detect_with("proj", &root, &ContainerMode::Auto, &pr).info.unwrap().name,
            "b_proj_web"
        );
        assert_eq!(
            detect_with("zzz", &root, &ContainerMode::Auto, &pr).info.unwrap().name,
            "a_web"
        );
    }

    #[test]
    fn off_and_named_modes() {
        let t = tempfile::tempdir().unwrap();
        let root = real(t.path());
        let pr = probe(&[("x_app", true, root.to_str().unwrap(), "/var/www")]);
        assert!(detect_with("x", &root, &ContainerMode::Off, &pr).info.is_none());

        let d = detect_with("x", &root, &ContainerMode::Named("khac".into()), &pr);
        assert!(d.info.is_none());
        assert_eq!(d.warnings.len(), 1);
        assert!(d.warnings[0].contains("khac"));

        let d = detect_with("x", &root, &ContainerMode::Named("x_app".into()), &pr);
        assert_eq!(d.info.unwrap().name, "x_app");

        let setting = ContainerSetting {
            mode: ContainerMode::Named("khac".into()),
            ..Default::default()
        };
        let rc = decide(&setting, &detect_with("x", &root, &setting.mode, &pr), &root);
        assert!(rc.active.is_none());
        assert_eq!(rc.warnings.len(), 1);
    }

    #[test]
    fn setting_parse_and_validate() {
        use crate::projects::parse_container_setting as p;
        use serde_json::json as j;
        assert_eq!(p(None).unwrap(), ContainerSetting::default());
        assert_eq!(p(Some(&j!("auto"))).unwrap().mode, ContainerMode::Auto);
        assert_eq!(p(Some(&j!(false))).unwrap().mode, ContainerMode::Off);
        let s = p(Some(&j!({"name": "x_app", "commands": ["php", "artisan-x"]}))).unwrap();
        assert_eq!(s.mode, ContainerMode::Named("x_app".into()));
        assert_eq!(s.commands, vec!["php", "artisan-x"]);
        for bad in [
            j!({"commands": ["docker"]}),
            j!({"commands": ["sh"]}),
            j!({"commands": ["bash"]}),
            j!({"commands": ["curl"]}),
            j!({"commands": ["PHP"]}),
            j!({"commands": ["a b"]}),
            j!({"commands": []}),
            j!({"name": "bad name"}),
            j!({"other": 1}),
            j!(true),
            j!("yes"),
        ] {
            assert!(p(Some(&bad)).is_err(), "{bad}");
        }
        let map = j!({"projects": [{"id": "a", "container": false}, {"id": "b", "container": {"commands": ["docker"]}}]});
        assert_eq!(crate::projects::container_of(&map, "a").unwrap().mode, ContainerMode::Off);
        assert!(crate::projects::container_of(&map, "b").is_err());
        assert_eq!(crate::projects::container_of(&map, "c").unwrap(), ContainerSetting::default());
    }

    fn info(source: &Path, docker: &Path, name: &str) -> ContainerInfo {
        ContainerInfo {
            name: name.to_string(),
            source: source.to_path_buf(),
            dest: "/var/www".to_string(),
            mount_source: source.to_path_buf(),
            mount_dest: "/var/www".to_string(),
            docker_bin: docker.to_path_buf(),
        }
    }

    #[test]
    fn shim_content_mode_and_path() {
        let t = tempfile::tempdir().unwrap();
        let src = real(t.path());
        let run_dir = src.join("run");
        let i = info(&src, Path::new("/usr/bin/docker"), "x_app");
        let cmds = vec!["php".to_string(), "composer".to_string()];
        let bin = write_shims(&run_dir, &i, &cmds).unwrap();
        assert_eq!(bin, run_dir.join("bin"));
        let mode = std::fs::metadata(&bin).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let php = std::fs::read_to_string(bin.join("php")).unwrap();
        assert!(php.starts_with("#!/bin/sh\n"));
        assert!(php.contains("exec \"/usr/bin/docker\" exec -i -w \"/var/www$rel\" \"x_app\" php \"$@\""));
        assert!(php.contains(&format!("\"{}/\"*) rel=\"${{PWD#{}}}\"", src.display(), src.display())));
        let mode = std::fs::metadata(bin.join("php")).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        assert!(bin.join("composer").exists());

        let mut env = BTreeMap::new();
        env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        prepend_path(&mut env, &bin);
        assert_eq!(env["PATH"], format!("{}:/usr/bin:/bin", bin.display()));

        let block = prompt_block(&i, &cmds);
        assert!(block.contains("`php`, `composer`"));
        assert!(block.contains("`x_app`"));
        assert!(block.contains("`/var/www`"));
    }

    #[test]
    fn unsafe_values_make_no_shim() {
        let t = tempfile::tempdir().unwrap();
        let src = real(t.path());
        for (name, docker) in [
            ("x_app", "/usr/bin/doc$ker"),
            ("x_app", "/usr/bin/doc\"ker"),
            ("x`app", "/usr/bin/docker"),
            ("x'app", "/usr/bin/docker"),
            ("x\napp", "/usr/bin/docker"),
            ("x_app ", "/usr/bin/docker"),
        ] {
            let i = info(&src, Path::new(docker), name);
            let run_dir = src.join(format!("run{}", name.len() + docker.len()));
            assert!(write_shims(&run_dir, &i, &["php".to_string()]).is_err(), "{name} {docker}");
            assert!(!run_dir.join("bin").exists());
            let setting = ContainerSetting::default();
            let det = Detection {
                info: Some(i),
                ..Default::default()
            };
            let rc = decide(&setting, &det, &src);
            assert!(rc.active.is_none());
            assert_eq!(rc.warnings.len(), 1);
        }
        let mut i = info(&src, Path::new("/usr/bin/docker"), "x_app");
        i.mount_source = src.join("a\\b");
        assert!(shim_script(&i, "php").is_err());
    }

    #[test]
    fn worktree_outside_mount_warns() {
        let t = tempfile::tempdir().unwrap();
        let src = real(t.path());
        let wt = tempfile::tempdir().unwrap();
        let det = Detection {
            info: Some(info(&src, Path::new("/usr/bin/docker"), "x_app")),
            ..Default::default()
        };
        let rc = decide(&ContainerSetting::default(), &det, &real(wt.path()));
        assert!(rc.active.is_none());
        assert_eq!(
            rc.warnings,
            vec!["Container x_app không mount worktree của task — lệnh php, composer chạy trên máy"]
        );
        assert!(rc.metrics().is_none());

        let rc = decide(&ContainerSetting::default(), &det, &src);
        assert!(rc.warnings.is_empty());
        assert_eq!(
            rc.metrics().unwrap(),
            serde_json::json!({"name": "x_app", "dest": "/var/www", "commands": ["php", "composer"]})
        );
    }

    #[test]
    fn shim_runs_with_fake_docker() {
        let t = tempfile::tempdir().unwrap();
        let base = real(t.path());
        let src = base.join("repo");
        let sub = src.join("app/Models");
        std::fs::create_dir_all(&sub).unwrap();
        let outside = base.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let fake = base.join("fake-docker");
        std::fs::write(&fake, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let i = info(&src, &fake, "x_app");
        let bin = write_shims(&base.join("run"), &i, &["php".to_string()]).unwrap();

        let out = std::process::Command::new(bin.join("php"))
            .arg("-v")
            .current_dir(&sub)
            .env("PWD", &sub)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let args: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            args,
            vec!["exec", "-i", "-w", "/var/www/app/Models", "x_app", "php", "-v"]
        );

        let out = std::process::Command::new(bin.join("php"))
            .arg("-v")
            .current_dir(&src)
            .env("PWD", &src)
            .output()
            .unwrap();
        let args = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(args.starts_with("exec\n-i\n-w\n/var/www\nx_app\n"), "{args}");

        let out = std::process::Command::new(bin.join("php"))
            .arg("-v")
            .current_dir(&outside)
            .env("PWD", &outside)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(SHIM_OUTSIDE_EXIT));
        assert!(out.stdout.is_empty());
    }

    #[test]
    fn find_in_path_returns_absolute_executable() {
        let t = tempfile::tempdir().unwrap();
        let d = real(t.path());
        let f = d.join("docker");
        std::fs::write(&f, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)).unwrap();
        let path = std::ffi::OsString::from(format!("relative:{}", d.display()));
        assert_eq!(find_in_path("docker", &path), None);
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(find_in_path("docker", &path), Some(f));
    }
}
