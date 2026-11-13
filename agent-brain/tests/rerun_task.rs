//! "Chạy lại (task mới)": `rerun_task` clones a task through `start_new_task` (the Giao việc path)
//! without needing the old branch or worktree. The server runs in process with a fake job exe that
//! records its argv and exits, so each test reads back exactly what the new task's Lead job got.
//!
//! The process environment (HOME, XDG, config path) is pointed at the sandbox for the duration of
//! a test, so every test holds `ENV_LOCK`.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::dashboard::api_tasks::{
    rerun_info, rerun_task, RerunRequest, StartOutcome, MAX_REQUEST_CHARS,
};
use agent_brain::dashboard::jobs::JobManager;
use agent_brain::dashboard::AppState;
use common::{wait_until, Sandbox};
use protocol::{lead_scenario, setup_project};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

const PROJECT: &str = "web-ban-hang";
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// A job exe that appends its argv (one `--ARG--` terminated entry per argument) to `args_file`
/// and exits at once, so the project is free again for the next re-run.
fn fake_exe(sb: &Sandbox, args_file: &Path) -> PathBuf {
    let script = sb.root.join("fake_job.sh");
    let body = format!(
        "#!/bin/sh\n{{ echo '=== JOB'; for a in \"$@\"; do printf '%s\\n--ARG--\\n' \"$a\"; done; }} >> \"{}\"\nexit 0\n",
        args_file.display()
    );
    std::fs::write(&script, body).expect("write fake_job.sh");
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod");
    }
    script
}

struct Env {
    sb: Sandbox,
    st: Arc<AppState>,
    args_file: PathBuf,
    repo: PathBuf,
    _guard: MutexGuard<'static, ()>,
}

impl Drop for Env {
    fn drop(&mut self) {
        self.st.jobs.kill_all_now();
    }
}

impl Env {
    /// Task 1 `sua-app` (title "sửa app") with a committed plan on `task/sua-app`; the repo is
    /// back on `main`. The task is `qa_pass` on the mac-dinh workflow, level `kho`, team mac-dinh,
    /// with an approval, an auto_json, one input and a lead run: everything a clone must not copy.
    fn new() -> Env {
        let guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let sb = Sandbox::new();
        std::env::set_var("HOME", &sb.home);
        std::env::set_var("XDG_CONFIG_HOME", &sb.xdg_config);
        std::env::set_var("XDG_DATA_HOME", &sb.xdg_data);
        std::env::set_var("AGENT_BRAIN_CONFIG", &sb.config_path);
        std::env::set_var("GIT_CONFIG_GLOBAL", &sb.git_config);
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");

        let repo = setup_project(&sb, "work");
        let res = sb.run(
            &["run", "lead", "--name", "sua-app", "--", PROJECT, "sửa app"],
            Some(lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])),
        );
        assert_eq!(
            res.code, 0,
            "run lead failed: stderr: {}\nstdout: {}",
            res.stderr, res.stdout
        );
        sb.git(&repo, &["checkout", "main"]);

        let args_file = sb.root.join("job_args.txt");
        let text = std::fs::read_to_string(&sb.config_path).expect("read config");
        let value: Value = serde_json::from_str(&text).expect("parse config");
        let cfg = agent_brain::config::config_from_value(&value).expect("config_from_value");
        let exe = fake_exe(&sb, &args_file);
        let st = Arc::new(AppState {
            orch: std::sync::RwLock::new(cfg.orchestrator.clone()),
            cfg,
            port: 0,
            token: "0".repeat(64),
            map_path: sb.map_path(),
            db_path: sb.db_path.clone(),
            exe: exe.clone(),
            hints: tokio::sync::broadcast::channel(16).0,
            shutdown: tokio::sync::watch::channel(false).1,
            jobs: JobManager::new(exe),
        });
        let env = Env {
            sb,
            st,
            args_file,
            repo,
            _guard: guard,
        };

        let wfs = agent_brain::workflows::load(&env.st.cfg);
        let snapshot = wfs.get("mac-dinh").expect("mac-dinh workflow").to_snapshot();
        let db = env.sb.db();
        db.execute(
            "UPDATE tasks SET status = 'qa_pass', level = 'kho', workflow_json = ?1, team_json = ?2, \
             auto_json = '{\"bypass_stages\":1}' WHERE id = 1",
            rusqlite::params![
                snapshot,
                agent_brain::teams::TaskTeam::new("mac-dinh").to_json()
            ],
        )
        .unwrap();
        agent_brain::db::set_task_approval(&db, 1, 100, "abc123").unwrap();
        agent_brain::inputs::add_to_task(&db, 1, "spec.md", "ui", "# Đặc tả\nnút xanh", &[]).unwrap();
        env
    }

    fn git(&self, args: &[&str]) -> String {
        self.sb.git(&self.repo, args)
    }

    fn set_status(&self, status: &str) {
        self.sb
            .db()
            .execute("UPDATE tasks SET status = ?1 WHERE id = 1", [status])
            .unwrap();
    }

    /// What `merge_push` leaves behind: the task branch merged with `--no-ff`, the protocol file
    /// dropped from the merge result, and the branch deleted.
    fn merge_and_delete_branch(&self) {
        self.git(&["merge", "--no-ff", "task/sua-app", "-m", "merge sua-app"]);
        self.git(&["rm", "-q", ".plan.md"]);
        self.git(&["commit", "-m", "bỏ file giao thức"]);
        self.git(&["branch", "-d", "task/sua-app"]);
        assert!(!self.git(&["branch", "--list", "task/sua-app"]).contains("task/sua-app"));
    }

    fn rerun(&self, desc: Option<&str>, mode: Option<&str>) -> StartOutcome {
        let out = rerun_task(
            &self.st,
            1,
            &RerunRequest {
                description: desc.map(str::to_string),
                plan_mode: mode.map(str::to_string),
                rev: None,
            },
        )
        .expect("rerun_task");
        self.settle();
        out
    }

    /// Waits for the job just started to exit, so the project is free again.
    fn settle(&self) {
        assert!(
            wait_until(Duration::from_secs(20), || self.st.jobs.reserved().is_empty()),
            "job did not exit"
        );
    }

    /// argv of every job the fake exe ran, in order.
    fn jobs(&self) -> Vec<Vec<String>> {
        let text = std::fs::read_to_string(&self.args_file).unwrap_or_default();
        text.split("=== JOB\n")
            .filter(|c| !c.trim().is_empty())
            .map(|chunk| {
                let mut args: Vec<String> =
                    chunk.split("\n--ARG--\n").map(str::to_string).collect();
                args.pop(); // the empty tail after the last terminator
                args
            })
            .collect()
    }

    fn started(&self, out: StartOutcome) -> (i64, agent_brain::dashboard::api_tasks::StartInfo) {
        match out {
            StartOutcome::Started { task_id, info, .. } => (task_id, info),
            other => panic!("expected Started, got {other:?}"),
        }
    }

    fn q<T: rusqlite::types::FromSql>(&self, sql: &str, id: i64) -> T {
        self.sb.db().query_row(sql, [id], |r| r.get(0)).unwrap()
    }

    fn input_names(&self, task_id: i64) -> Vec<String> {
        agent_brain::inputs::list_for_task(&self.sb.db(), task_id)
            .unwrap()
            .into_iter()
            .map(|r| r.name)
            .collect()
    }

    /// Every column of task 1, to prove a clone leaves the old task alone.
    fn old_task_row(&self) -> Vec<String> {
        let db = self.sb.db();
        let mut stmt = db
            .prepare(
                "SELECT id, project_id, slug, title, branch, base_branch, base_commit, status, \
                 created_at, updated_at, level, workdir, team_json, workflow_json, approved_at, \
                 approved_commit, auto_state, auto_pid, auto_json, request, cloned_from \
                 FROM tasks WHERE id = 1",
            )
            .unwrap();
        stmt.query_row([], |row| {
            (0..21)
                .map(|i| {
                    let v: rusqlite::types::Value = row.get(i)?;
                    Ok(format!("{v:?}"))
                })
                .collect()
        })
        .unwrap()
    }
}

/// The request the new task's Lead job was given (the last argv entry, after `--`).
fn request_of(job: &[String]) -> &str {
    job.last().map(String::as_str).unwrap_or("")
}

fn arg_after<'a>(job: &'a [String], flag: &str) -> Option<&'a str> {
    let pos = job.iter().position(|a| a == flag)?;
    job.get(pos + 1).map(String::as_str)
}

// ---------------------------------------------------------------------------

#[test]
fn clone_of_done_task_after_branch_and_worktree_cleanup() {
    let env = Env::new();
    env.merge_and_delete_branch();
    let old_before = env.old_task_row();

    // Dữ kiện cho hộp thoại: đã merge, nhánh mất, plan vẫn lấy lại được từ lịch sử git.
    let info = rerun_info(&env.st, 1).unwrap().unwrap();
    assert!(info.merged, "{info:?}");
    assert!(!info.branch_exists);
    assert!(info.plan_available, "{info:?}");
    assert_eq!(info.plan_unavailable_reason, None);
    assert_eq!(info.default_mode, "replan", "a merged task defaults to a new plan");
    assert_eq!(info.request, "sửa app");
    assert_eq!(info.cloned_from, None);

    let (new_id, started) = env.started(env.rerun(None, None));
    assert_eq!(new_id, 2);
    assert_eq!(started.slug, "sua-app-r2");
    assert_eq!(started.branch, "task/sua-app-r2");
    assert_eq!(started.workflow, "mac-dinh");
    assert_eq!(started.team, "mac-dinh");

    // Same team, workflow (auto: `auto --start`) and level, through the Giao việc job.
    let jobs = env.jobs();
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    let job = &jobs[0];
    assert_eq!(&job[0..2], ["auto", "--start"], "{job:?}");
    assert_eq!(arg_after(job, "--name"), Some("sua-app-r2"));
    assert_eq!(arg_after(job, "--workflow"), Some("mac-dinh"));
    assert_eq!(arg_after(job, "--team"), Some("mac-dinh"));
    assert_eq!(arg_after(job, "--level"), Some("kho"));
    let request = request_of(job);
    assert!(request.starts_with("sửa app\n\n--- Chạy lại từ task #1 (sua-app) ---\n"), "{request}");
    assert!(request.contains("Lập plan mới."), "{request}");
    assert!(request.contains("đã merge: có"), "{request}");
    assert!(!request.contains("Yêu cầu bổ sung"), "{request}");

    // The new row: its own branch, the source's level, cloned_from and the original request.
    let db = env.sb.db();
    let row: (String, String, String, Option<i64>, Option<String>) = db
        .query_row(
            "SELECT branch, level, title, cloned_from, request FROM tasks WHERE id = 2",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(row.0, "task/sua-app-r2");
    assert_eq!(row.1, "kho");
    assert_eq!(row.3, Some(1));
    assert_eq!(row.4.as_deref(), Some("sửa app"), "original request, without the directive");
    let old_title: String = env.q("SELECT title FROM tasks WHERE id = ?1", 1);
    assert_eq!(row.2, old_title);
    // team_json / workflow_json are written by the job itself (`--team`, `--workflow` above), not copied.
    let team: Option<String> = env.q("SELECT team_json FROM tasks WHERE id = ?1", 2);
    assert_eq!(team, None);

    // Nothing of the old task's progress is copied: the new task meets the plan gate itself.
    let n: i64 = env.q("SELECT COUNT(*) FROM plan_approvals WHERE task_id = ?1", 2);
    assert_eq!(n, 0, "no plan_approvals copied");
    let n: i64 = env.q("SELECT COUNT(*) FROM runs WHERE task_id = ?1", 2);
    assert_eq!(n, 0, "no runs copied");
    let (approved_at, approved_commit): (Option<i64>, Option<String>) = db
        .query_row(
            "SELECT approved_at, approved_commit FROM tasks WHERE id = 2",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((approved_at, approved_commit), (None, None));
    let aj: Option<String> = env.q("SELECT auto_json FROM tasks WHERE id = ?1", 2);
    assert_eq!(aj, None, "auto_json is not copied");
    let n: i64 = env.q("SELECT COUNT(*) FROM runs WHERE task_id = ?1", 1);
    assert!(n >= 1, "the old task keeps its runs");

    // The old plan (from git history) goes first, then the old input documents.
    assert_eq!(env.input_names(2), vec!["plan-cu.md", "spec.md"]);
    let plan = agent_brain::inputs::list_for_task(&db, 2).unwrap()[0].content.clone();
    assert!(plan.contains("# Plan:"), "{plan}");

    // The old task is untouched, and the new one now reports where it came from.
    assert_eq!(env.old_task_row(), old_before);
    assert_eq!(rerun_info(&env.st, 2).unwrap().unwrap().cloned_from, Some(1));
}

#[test]
fn plan_mode_defaults_and_overrides() {
    let env = Env::new();
    // Unmerged and finished with a block: the branch is still there, the plan comes from it.
    env.set_status("blocked");

    let info = rerun_info(&env.st, 1).unwrap().unwrap();
    assert!(!info.merged);
    assert!(info.branch_exists);
    assert!(info.plan_available);
    assert_eq!(info.default_mode, "reuse");

    // Empty description + unmerged -> reuse.
    env.started(env.rerun(None, None));
    let jobs = env.jobs();
    let req = request_of(&jobs[0]);
    assert!(req.contains("Dùng lại plan trong tài liệu đầu vào `plan-cu.md`"), "{req}");
    assert!(req.contains("Không đổi bước/tiêu chí nghiệm thu."), "{req}");
    assert!(!req.contains("Lập plan mới"), "{req}");

    // A description flips the default to replan, and its text rides along.
    env.started(env.rerun(Some("  đổi màu nút sang đỏ  "), None));
    let jobs = env.jobs();
    let req = request_of(&jobs[1]);
    assert!(req.contains("Lập plan mới."), "{req}");
    assert!(req.contains("đã merge: không"), "{req}");
    assert!(req.ends_with("Yêu cầu bổ sung: đổi màu nút sang đỏ"), "{req}");

    // The user's pick wins over the default, both ways.
    env.started(env.rerun(Some("vẫn dùng plan cũ"), Some("reuse")));
    env.started(env.rerun(None, Some("replan")));
    let jobs = env.jobs();
    assert!(request_of(&jobs[2]).contains("Dùng lại plan"), "{}", request_of(&jobs[2]));
    assert!(request_of(&jobs[2]).contains("Yêu cầu bổ sung: vẫn dùng plan cũ"));
    assert!(request_of(&jobs[3]).contains("Lập plan mới."));
    // Names: -r2, -r3, -r4, -r5, each the smallest free one.
    let names: Vec<&str> = jobs.iter().map(|j| arg_after(j, "--name").unwrap()).collect();
    assert_eq!(names, ["sua-app-r2", "sua-app-r3", "sua-app-r4", "sua-app-r5"]);

    // A task already merged into main defaults to replan even with an empty description.
    env.git(&["merge", "--no-ff", "task/sua-app", "-m", "merge sua-app"]);
    let info = rerun_info(&env.st, 1).unwrap().unwrap();
    assert!(info.merged, "{info:?}");
    assert_eq!(info.default_mode, "replan");
    env.started(env.rerun(None, None));
    let jobs = env.jobs();
    assert!(request_of(&jobs[4]).contains("Lập plan mới."), "{}", request_of(&jobs[4]));
    assert!(request_of(&jobs[4]).contains("đã merge: có"));
}

#[test]
fn missing_plan_blocks_reuse_but_not_replan() {
    let env = Env::new();
    env.set_status("blocked");
    // Unmerged branch deleted: no worktree, no branch, no commit reachable from any ref.
    env.git(&["branch", "-D", "task/sua-app"]);
    let old_before = env.old_task_row();

    let info = rerun_info(&env.st, 1).unwrap().unwrap();
    assert!(!info.branch_exists);
    assert!(!info.plan_available, "{info:?}");
    assert!(
        info.plan_unavailable_reason
            .as_deref()
            .is_some_and(|r| r.contains("Không tìm lại được plan cũ")),
        "{info:?}"
    );
    assert!(!info.merged);
    assert_eq!(info.default_mode, "replan");
    // No stored request and no plan meta any more: falls back to the title.
    let title: String = env.q("SELECT title FROM tasks WHERE id = ?1", 1);
    assert_eq!(info.request, title);

    // reuse is refused (422) with the reason; nothing is created.
    match env.rerun(None, Some("reuse")) {
        StartOutcome::Invalid(m) => assert!(m.contains("Không dùng lại plan cũ được"), "{m}"),
        other => panic!("expected Invalid, got {other:?}"),
    }
    assert!(env.jobs().is_empty());
    let n: i64 = env.q("SELECT COUNT(*) FROM tasks WHERE id > ?1", 1);
    assert_eq!(n, 0);

    // replan still runs; there is no plan-cu.md, only the old input document.
    let (new_id, _) = env.started(env.rerun(None, None));
    assert_eq!(env.input_names(new_id), vec!["spec.md"]);
    let jobs = env.jobs();
    let req = request_of(&jobs[0]);
    assert!(req.starts_with(&format!("{title}\n\n--- Chạy lại từ task #1")), "{req}");
    assert!(req.contains("Không lấy lại được plan cũ"), "{req}");
    assert_eq!(env.old_task_row(), old_before);
}

#[test]
fn names_skip_taken_slugs_and_branches() {
    let env = Env::new();
    // -r2 already has a task row; -r3 only has a branch in the repo.
    {
        let db = env.sb.db();
        let pid: i64 = env.q("SELECT project_id FROM tasks WHERE id = ?1", 1);
        agent_brain::db::upsert_task(&db, pid, "sua-app-r2", "x", "task/sua-app-r2", None, None).unwrap();
    }
    env.git(&["branch", "task/sua-app-r3"]);
    let (_, info) = env.started(env.rerun(None, None));
    assert_eq!(info.slug, "sua-app-r4");

    // A re-run of a re-run counts from the original name.
    let db = env.sb.db();
    let pid: i64 = env.q("SELECT project_id FROM tasks WHERE id = ?1", 1);
    let r4: i64 = db
        .query_row(
            "SELECT id FROM tasks WHERE project_id = ?1 AND branch = 'task/sua-app-r4'",
            [pid],
            |r| r.get(0),
        )
        .unwrap();
    db.execute("UPDATE tasks SET status = 'qa_pass' WHERE id = ?1", [r4]).unwrap();
    let out = rerun_task(&env.st, r4, &RerunRequest::default()).unwrap();
    env.settle();
    let (_, info) = env.started(out);
    assert_eq!(info.slug, "sua-app-r5", "{info:?}");
}

#[test]
fn inputs_are_capped_with_the_old_plan_first() {
    let env = Env::new();
    env.set_status("blocked");
    {
        let db = env.sb.db();
        for name in ["b.md", "c.md", "d.md"] {
            agent_brain::inputs::add_to_task(&db, 1, name, "ui", &format!("nội dung {name}"), &[]).ok();
        }
    }
    // spec.md plus two more fit the old limit of 3; the clone has room for the plan and two more.
    assert_eq!(env.input_names(1), vec!["spec.md", "b.md", "c.md"]);
    let (new_id, info) = env.started(env.rerun(None, None));
    assert_eq!(env.input_names(new_id), vec!["plan-cu.md", "spec.md", "b.md"]);
    let warning = info.warning.expect("dropped input is reported");
    assert!(warning.contains("c.md"), "{warning}");
    assert!(!warning.contains("spec.md"), "{warning}");
}

#[test]
fn refusals_leave_the_old_task_alone() {
    let env = Env::new();
    env.set_status("planned");
    let old_before = env.old_task_row();

    // Still waiting for its next step: not finished.
    match env.rerun(None, None) {
        StartOutcome::PreflightRefused { reason, kind } => {
            assert_eq!(kind, "not_allowed");
            assert!(reason.contains("chưa kết thúc"), "{reason}");
        }
        other => panic!("expected PreflightRefused, got {other:?}"),
    }

    // A live autopilot driver: busy.
    env.set_status("blocked");
    let mut child = std::process::Command::new("python3")
        .args(["-c", "import time; time.sleep(60)"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn python3 sleeper");
    let p = agent_brain::procs::proc_id(child.id() as i32).expect("sleeper proc_id");
    env.sb
        .db()
        .execute(
            "UPDATE tasks SET auto_state = 'running', auto_pid = ?1, auto_pid_start = ?2 WHERE id = 1",
            rusqlite::params![p.pid as i64, p.start as i64],
        )
        .unwrap();
    let out = env.rerun(None, None);
    let _ = child.kill();
    let _ = child.wait();
    match out {
        StartOutcome::PreflightRefused { reason, kind } => {
            assert_eq!(kind, "not_allowed");
            assert!(reason.contains("đang có run hoặc autopilot chạy"), "{reason}");
        }
        other => panic!("expected PreflightRefused, got {other:?}"),
    }
    env.sb
        .db()
        .execute(
            "UPDATE tasks SET auto_state = NULL, auto_pid = NULL, auto_pid_start = NULL WHERE id = 1",
            [],
        )
        .unwrap();

    // A stale rev.
    match rerun_task(
        &env.st,
        1,
        &RerunRequest {
            rev: Some("0000000000000000".to_string()),
            ..Default::default()
        },
    )
    .unwrap()
    {
        StartOutcome::PreflightRefused { kind, .. } => assert_eq!(kind, "stale"),
        other => panic!("expected stale, got {other:?}"),
    }

    // Unknown plan_mode and an over-long description are 422, not silently cut.
    assert!(matches!(
        env.rerun(None, Some("clone")),
        StartOutcome::Invalid(m) if m.contains("plan_mode")
    ));
    let long = "ă".repeat(MAX_REQUEST_CHARS);
    match env.rerun(Some(&long), None) {
        StartOutcome::Invalid(m) => assert!(m.contains("quá dài"), "{m}"),
        other => panic!("expected Invalid, got {other:?}"),
    }

    // Unknown task: 404 through ProjectRefused.
    match rerun_task(&env.st, 999, &RerunRequest::default()).unwrap() {
        StartOutcome::ProjectRefused { status, .. } => assert_eq!(status, 404),
        other => panic!("expected 404, got {other:?}"),
    }
    assert!(rerun_info(&env.st, 999).unwrap().is_err());

    assert!(env.jobs().is_empty(), "no job was started");
    let n: i64 = env.q("SELECT COUNT(*) FROM tasks WHERE id > ?1", 1);
    assert_eq!(n, 0, "no task was created");
    // Status, auto_state and auto_pid were set by hand above; every other column is as before.
    let after = env.old_task_row();
    assert_eq!(after[2..7], old_before[2..7]);
    assert_eq!(after[12..], old_before[12..]);
}

#[test]
fn a_team_that_no_longer_exists_keeps_the_start_message() {
    let env = Env::new();
    env.set_status("blocked");
    env.sb
        .db()
        .execute(
            "UPDATE tasks SET team_json = ?1 WHERE id = 1",
            [agent_brain::teams::TaskTeam::new("doi-da-xoa").to_json()],
        )
        .unwrap();
    match env.rerun(None, None) {
        StartOutcome::Invalid(m) => assert!(m.contains("doi-da-xoa"), "{m}"),
        other => panic!("expected Invalid, got {other:?}"),
    }
    assert!(env.jobs().is_empty());
}

#[test]
fn http_routes_info_and_refusal() {
    let env = Env::new();
    env.set_status("planned");
    let srv = env.sb.serve(&[]);

    // Not finished: 409 not_allowed, nothing created.
    let t = srv.get("/api/tasks/1").json();
    assert!(t["cloned_from"].is_null(), "{t}");
    assert!(
        !t["controls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == "rerun"),
        "{}",
        t["controls"]
    );
    let resp = srv.post_json("/api/tasks/1/rerun", &json!({ "rev": t["rev"] }));
    assert_eq!(resp.status, 409, "{}", resp.text());
    assert_eq!(resp.json()["kind"], "not_allowed");

    env.set_status("qa_pass");
    let t = srv.get("/api/tasks/1").json();
    let rerun = t["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "rerun")
        .cloned()
        .expect("rerun control on a done task");
    assert_eq!(rerun["label"], "Chạy lại (task mới)");
    assert_eq!(rerun["enabled"], true, "{rerun}");

    let info = srv.get("/api/tasks/1/rerun-info");
    assert_eq!(info.status, 200, "{}", info.text());
    let info = info.json();
    for key in [
        "request",
        "merged",
        "plan_available",
        "plan_unavailable_reason",
        "default_mode",
        "branch_exists",
        "cloned_from",
    ] {
        assert!(info.get(key).is_some(), "rerun-info has {key}: {info}");
    }
    assert_eq!(info["plan_available"], true);
    assert_eq!(info["branch_exists"], true);
    assert_eq!(info["default_mode"], "reuse");
    assert_eq!(srv.get("/api/tasks/999/rerun-info").status, 404);

    // Unknown fields and a bad mode are 4xx.
    let resp = srv.post_json("/api/tasks/1/rerun", &json!({ "plan_mode": "clone" }));
    assert_eq!(resp.status, 422, "{}", resp.text());
    let resp = srv.post_json("/api/tasks/1/rerun", &json!({ "rev": "0000000000000000" }));
    assert_eq!(resp.status, 409, "{}", resp.text());
    assert_eq!(resp.json()["kind"], "stale");
}
