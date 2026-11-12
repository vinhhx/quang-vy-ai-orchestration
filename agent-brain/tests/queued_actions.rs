//! Hàng chờ chạy lại bước của task có sẵn khi dự án bận (migration 13): xếp hàng, chạy ở tick
//! sau khi hết bận, kiểm plan trước khi Dev chạy từ hàng chờ. Mọi thứ trong thư mục tạm, DB tạm
//! và tiến trình giả; không bao giờ đụng DB hay dự án thật.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::dashboard::api_tasks::{
    cancel_queued_action, queue_action, run_control, ActionSource, ControlOutcome, QueueOutcome,
};
use agent_brain::dashboard::jobs::{JobManager, JobSpec, JobView};
use agent_brain::dashboard::AppState;
use agent_brain::orchestrator::scheduler::{handle_queued_actions, schedule_tick};
use common::{wait_until, Sandbox};
use protocol::{lead_scenario, setup_project};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const PROJECT: &str = "web-ban-hang";
const PLAN_SUBJECT: &str = "sua-app: plan";

/// Tiến trình giả cho `JobManager`: `hold` chờ tới khi file cờ bị xoá (giữ chỗ của dự án),
/// mọi lệnh khác ngủ 30 giây (job chạy từ hàng chờ; `Env` dọn bằng `kill_all_now`).
fn fake_exe(sb: &Sandbox, hold_file: &Path) -> PathBuf {
    let script = sb.root.join("fake_job.sh");
    let body = format!(
        "#!/bin/sh\ncase \"$1\" in\n  hold) while [ -e \"{}\" ]; do sleep 0.1; done; exit 0;;\n  *) sleep 30; exit 0;;\nesac\n",
        hold_file.display()
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

fn app_state(sb: &Sandbox, exe: PathBuf) -> Arc<AppState> {
    let text = std::fs::read_to_string(&sb.config_path).expect("read config");
    let value: Value = serde_json::from_str(&text).expect("parse config");
    let cfg = agent_brain::config::config_from_value(&value).expect("config_from_value");
    Arc::new(AppState {
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
    })
}

fn run_lead(sb: &Sandbox, name: &str, title: &str) {
    let res = sb.run(
        &["run", "lead", "--name", name, "--", PROJECT, title],
        Some(lead_scenario(&[("src/app.py", "app()", "trả về 2", "demo")])),
    );
    assert_eq!(
        res.code, 0,
        "run lead failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

#[derive(Debug, Clone, PartialEq)]
struct Row {
    id: i64,
    task_id: i64,
    action: String,
    status: String,
    note: Option<String>,
    reason: Option<String>,
}

/// Một sandbox có task 1 (`sua-app`, nhánh `task/sua-app`, đã có commit plan) trên dự án
/// `web-ban-hang` ở chế độ shared, một `AppState` trong tiến trình test với job giả.
struct Env {
    sb: Sandbox,
    repo: PathBuf,
    st: Arc<AppState>,
    hold: PathBuf,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.hold);
        self.st.jobs.kill_all_now();
    }
}

impl Env {
    fn new() -> Env {
        Env::with_config(json!({}))
    }

    fn with_config(config: Value) -> Env {
        let sb = Sandbox::new();
        sb.write_config(config);
        let repo = setup_project(&sb, "work");
        run_lead(&sb, "sua-app", "sửa app");
        Env::attach(sb, repo)
    }

    fn attach(sb: Sandbox, repo: PathBuf) -> Env {
        let hold = sb.root.join("hold.flag");
        std::fs::write(&hold, "x").unwrap();
        let st = app_state(&sb, fake_exe(&sb, &hold));
        Env { sb, repo, st, hold }
    }

    /// Một job `hold` của task khác giữ chỗ của dự án (dự án bận).
    fn hold_project(&self, task_id: i64, max_parallel: Option<usize>) {
        self.st
            .jobs
            .start(JobSpec {
                role: "dev".into(),
                project: PROJECT.into(),
                profile: "work".into(),
                branch: Some(format!("task/other-{task_id}")),
                task_id: Some(task_id),
                args: vec!["hold".into()],
                max_parallel,
            })
            .expect("start hold job");
    }

    /// Thả mọi job giữ chỗ; chờ tới khi dự án hết bận.
    fn release(&self) {
        let _ = std::fs::remove_file(&self.hold);
        assert!(
            wait_until(Duration::from_secs(15), || self
                .st
                .jobs
                .reserved()
                .is_empty()),
            "hold jobs did not exit"
        );
    }

    fn queue(&self, action: &str, note: Option<&str>) -> QueueOutcome {
        queue_action(&self.st, 1, action, note, ActionSource::Dashboard).expect("queue_action")
    }

    fn tick(&self) -> Vec<i64> {
        let conn = agent_brain::db::open(&self.st.db_path).unwrap();
        handle_queued_actions(&self.st, &conn).expect("handle_queued_actions")
    }

    fn rows(&self) -> Vec<Row> {
        let conn = self.sb.db();
        let mut stmt = conn
            .prepare(
                "SELECT id, task_id, action_id, status, note, reason FROM queued_actions ORDER BY id",
            )
            .unwrap();
        let rows = stmt
            .query_map([], |r| {
                Ok(Row {
                    id: r.get(0)?,
                    task_id: r.get(1)?,
                    action: r.get(2)?,
                    status: r.get(3)?,
                    note: r.get(4)?,
                    reason: r.get(5)?,
                })
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        rows
    }

    /// Job của task `task_id` (không tính job giữ chỗ, vốn là task khác).
    fn jobs_of(&self, task_id: i64) -> Vec<JobView> {
        self.st
            .jobs
            .list()
            .into_iter()
            .filter(|j| j.task_id == Some(task_id))
            .collect()
    }

    fn git(&self, args: &[&str]) -> String {
        self.sb.git(&self.repo, args)
    }

    fn plan_path(&self) -> PathBuf {
        self.repo.join(".plan.md")
    }

    fn only_row(&self) -> Row {
        let rows = self.rows();
        assert_eq!(rows.len(), 1, "rows: {rows:?}");
        rows.into_iter().next().unwrap()
    }
}

fn queued_id(outcome: QueueOutcome) -> i64 {
    match outcome {
        QueueOutcome::Queued { queued_id } => queued_id,
        other => panic!("expected Queued, got {other:?}"),
    }
}

/// Driver autopilot giả còn sống cho task `task_id`: tiến trình python3 ngủ (không dùng
/// `sleep N`) có (pid, start) ghi vào `auto_pid` / `auto_pid_start`, `auto_state = 'running'`.
/// Không có run nào `running`: khoảng giữa hai bước của autopilot.
fn fake_live_driver(sb: &Sandbox, task_id: i64) -> std::process::Child {
    let child = std::process::Command::new("python3")
        .args(["-c", "import time; time.sleep(60)"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn python3 sleeper");
    let p = agent_brain::procs::proc_id(child.id() as i32).expect("sleeper proc_id");
    sb.db()
        .execute(
            "UPDATE tasks SET auto_state = 'running', auto_pid = ?1, auto_pid_start = ?2 WHERE id = ?3",
            rusqlite::params![p.pid as i64, p.start as i64, task_id],
        )
        .unwrap();
    let running: i64 = sb
        .db()
        .query_row("SELECT COUNT(*) FROM runs WHERE status = 'running'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(running, 0, "no run is running");
    child
}

// (a) xếp hàng chỉ khi hành động bị khoá đúng vì dự án bận

#[test]
fn queue_records_waiting_row_only_for_busy_actions() {
    let env = Env::new();

    // Chạy được ngay: không xếp hàng.
    match env.queue("relead", None) {
        QueueOutcome::NotQueueable(msg) => assert!(msg.contains("chạy được ngay"), "{msg}"),
        other => panic!("expected NotQueueable, got {other:?}"),
    }
    assert!(env.rows().is_empty());

    env.hold_project(900, None);

    let id = queued_id(env.queue("relead", Some("  làm lại phần login  ")));
    let row = env.only_row();
    assert_eq!(row.id, id);
    assert_eq!(row.task_id, 1);
    assert_eq!(row.action, "relead");
    assert_eq!(row.status, "waiting");
    assert_eq!(row.note.as_deref(), Some("làm lại phần login"));

    // Mọi lý do khoá khác `busy` không xếp hàng: Bật autopilot đang khoá vì cổng duyệt plan.
    match env.queue("auto_start", None) {
        QueueOutcome::NotQueueable(msg) => assert!(msg.contains("duyệt plan"), "{msg}"),
        other => panic!("expected NotQueueable, got {other:?}"),
    }
    // Hành động không có trong trạng thái task (Chạy QA khi chưa có code).
    match env.queue("run_qa", None) {
        QueueOutcome::NotQueueable(msg) => {
            assert_eq!(msg, "Hành động không áp dụng cho trạng thái hiện tại")
        }
        other => panic!("expected NotQueueable, got {other:?}"),
    }
    // Không nằm trong danh sách cho phép; ghi chú quá dài; task không có.
    assert!(matches!(env.queue("approve", None), QueueOutcome::Invalid(_)));
    assert!(matches!(
        env.queue("relead", Some(&"ă".repeat(501))),
        QueueOutcome::Invalid(_)
    ));
    assert!(matches!(
        queue_action(&env.st, 999, "relead", None, ActionSource::Dashboard).unwrap(),
        QueueOutcome::NotFound(_)
    ));
    assert_eq!(env.rows().len(), 1, "only the busy relead was queued");
    assert!(env.jobs_of(1).is_empty());
}

// (b) hai lần cho một task: hàng sau thay hàng trước

#[test]
fn queueing_twice_keeps_one_waiting_row() {
    let env = Env::new();
    env.hold_project(900, None);

    let first = queued_id(env.queue("relead", Some("một")));
    let second = queued_id(env.queue("run_dev", None));
    assert_ne!(first, second);

    let rows = env.rows();
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].id, first);
    assert_eq!(rows[0].status, "cancelled", "the first row is replaced");
    let row = &rows[1];
    assert_eq!(row.id, second);
    assert_eq!(row.action, "run_dev");
    assert_eq!(row.status, "waiting");
    assert_eq!(row.note, None, "only relead carries a note");
    let waiting: Vec<_> = rows.iter().filter(|r| r.status == "waiting").collect();
    assert_eq!(waiting.len(), 1, "one waiting row per task");
}

// (c) dự án còn bận: giữ waiting

#[test]
fn still_busy_keeps_the_row_waiting() {
    let env = Env::new();
    env.hold_project(900, None);
    queued_id(env.queue("relead", None));

    assert!(env.tick().is_empty());
    assert!(env.tick().is_empty());

    assert_eq!(env.only_row().status, "waiting");
    assert!(env.jobs_of(1).is_empty(), "nothing started while busy");
}

// (d) hết bận: gọi đúng hàm hiện có, hàng started, job được tạo

#[test]
fn free_project_runs_the_action_with_the_existing_function() {
    let env = Env::new();
    env.hold_project(900, None);
    queued_id(env.queue("relead", Some("ghi chú cho Lead")));
    assert!(env.tick().is_empty());

    env.release();
    assert_eq!(env.tick(), vec![1]);

    let row = env.only_row();
    assert_eq!(row.status, "started", "{row:?}");
    let jobs = env.jobs_of(1);
    assert_eq!(jobs.len(), 1, "one job for task 1: {jobs:?}");
    assert!(
        jobs[0].role == "lead" || jobs[0].role == "auto",
        "relead starts a lead or replan job, got {}",
        jobs[0].role
    );
    assert_eq!(jobs[0].project, PROJECT);

    // Tick sau: không còn hàng waiting, không chạy lại.
    assert!(env.tick().is_empty());
    assert_eq!(env.jobs_of(1).len(), 1);
}

// (e) hai hàng cùng dự án (worktree, max_parallel_tasks = 2): mỗi tick chỉ bắt đầu một

#[test]
fn only_one_action_per_project_starts_per_tick() {
    let sb = Sandbox::new();
    let repo = sb.make_repo(
        PROJECT,
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, PROJECT, "work");
    let obj = entry.as_object_mut().unwrap();
    obj.insert("workspace".to_string(), json!("worktree"));
    obj.insert("max_parallel_tasks".to_string(), json!(2));
    sb.write_map(json!([entry]), &[]);
    run_lead(&sb, "sua-app", "sửa app");
    run_lead(&sb, "task-hai", "việc hai");
    let env = Env::attach(sb, repo);

    // Hai slot đều bị task khác giữ: cả hai task đều bị chặn vì dự án bận.
    env.hold_project(901, Some(2));
    env.hold_project(902, Some(2));
    queued_id(env.queue("relead", None));
    queued_id(
        queue_action(&env.st, 2, "relead", None, ActionSource::Dashboard).expect("queue task 2"),
    );
    assert!(env.tick().is_empty());

    env.release();
    // Cả hai đều chạy được (2 slot trống), nhưng mỗi tick chỉ một hành động cho dự án.
    assert_eq!(env.tick(), vec![1], "oldest row first");
    let rows = env.rows();
    assert_eq!(rows[0].status, "started", "{rows:?}");
    assert_eq!(rows[1].status, "waiting", "{rows:?}");
    assert!(env.jobs_of(2).is_empty());

    assert_eq!(env.tick(), vec![2], "the next tick starts the other one");
    assert_eq!(env.rows()[1].status, "started");
    assert_eq!(env.jobs_of(2).len(), 1);
}

// (f) task đã đổi trạng thái: hành động biến mất -> failed

#[test]
fn action_gone_after_state_change_fails_the_row() {
    let env = Env::new();
    env.hold_project(900, None);
    queued_id(env.queue("relead", None));

    env.sb
        .db()
        .execute("UPDATE tasks SET status = 'qa_pass' WHERE id = 1", [])
        .unwrap();
    env.release();

    assert!(env.tick().is_empty());
    let row = env.only_row();
    assert_eq!(row.status, "failed");
    assert_eq!(
        row.reason.as_deref(),
        Some("Task đã đổi trạng thái — không chạy nữa")
    );
    assert!(env.jobs_of(1).is_empty());
}

// (g) Dev từ hàng chờ với plan hợp lệ

#[test]
fn dev_from_queue_runs_when_the_plan_is_still_valid() {
    let env = Env::new();
    env.hold_project(900, None);
    queued_id(env.queue("run_dev", None));
    env.release();

    assert_eq!(env.tick(), vec![1]);
    assert_eq!(env.only_row().status, "started");
    let jobs = env.jobs_of(1);
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].role, "dev");
}

/// Hàng `run_dev` đang chờ, plan lệch: tick này KHÔNG chạy Dev, hàng thành `relead` kèm lý do;
/// tick sau chạy `relead`.
fn assert_dev_goes_back_to_lead(env: &Env, expect_reason: &str) {
    env.release();
    assert!(env.tick().is_empty(), "Dev must not start");
    assert!(env.jobs_of(1).is_empty(), "no job at the mismatch tick");

    let row = env.only_row();
    assert_eq!(row.status, "waiting", "{row:?}");
    assert_eq!(row.action, "relead", "{row:?}");
    let note = row.note.clone().expect("note");
    assert!(note.starts_with("Plan không còn khớp: "), "{note}");
    assert!(note.contains(expect_reason), "{note}");
    assert!(row.reason.clone().unwrap().contains(expect_reason), "{row:?}");

    // Tick sau chạy Lead, không bao giờ Dev.
    assert_eq!(env.tick(), vec![1]);
    assert_eq!(env.only_row().status, "started");
    let jobs = env.jobs_of(1);
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    assert_ne!(jobs[0].role, "dev");
}

// (h1) file plan thiếu mục

#[test]
fn dev_from_queue_with_invalid_plan_goes_back_to_lead() {
    let env = Env::new();
    let text = std::fs::read_to_string(env.plan_path()).unwrap();
    assert!(text.contains("## Cách kiểm tra"));
    std::fs::write(
        env.plan_path(),
        text.replace("## Cách kiểm tra", "## Kiểm tra"),
    )
    .unwrap();
    env.git(&["add", ".plan.md"]);
    env.git(&["commit", "-m", "sua plan"]);

    env.hold_project(900, None);
    queued_id(env.queue("run_dev", None));
    assert_dev_goes_back_to_lead(&env, "missing_section:Cách kiểm tra");
}

// (h2) commit plan khác commit đã duyệt

#[test]
fn dev_from_queue_with_plan_commit_changed_after_approval_goes_back_to_lead() {
    let env = Env::new();
    match run_control(&env.st, 1, "approve", None, None).unwrap() {
        ControlOutcome::Approved { .. } => {}
        other => panic!("expected Approved, got {other:?}"),
    }
    // Lead lập lại plan sau khi duyệt: commit plan mới (cùng tiêu đề), file vẫn hợp lệ.
    let text = std::fs::read_to_string(env.plan_path()).unwrap();
    std::fs::write(
        env.plan_path(),
        format!("{text}\n<!-- sửa sau khi duyệt -->\n"),
    )
    .unwrap();
    env.git(&["add", ".plan.md"]);
    env.git(&["commit", "-m", PLAN_SUBJECT]);

    env.hold_project(900, None);
    queued_id(env.queue("run_dev", None));
    assert_dev_goes_back_to_lead(&env, "khác commit đã duyệt");
}

// (h3) file plan nêu đã đổi trên nhánh gốc kể từ base_commit

#[test]
fn dev_from_queue_with_planned_file_changed_on_base_goes_back_to_lead() {
    let env = Env::new();
    env.git(&["checkout", "main"]);
    std::fs::write(env.repo.join("src/app.py"), "def app():\n    return 99\n").unwrap();
    env.git(&["add", "src/app.py"]);
    env.git(&["commit", "-m", "main sửa app"]);
    env.git(&["checkout", "task/sua-app"]);

    env.hold_project(900, None);
    queued_id(env.queue("run_dev", None));
    assert_dev_goes_back_to_lead(&env, "src/app.py");
}

#[test]
fn dev_from_queue_ignores_base_changes_to_other_files() {
    let env = Env::new();
    env.git(&["checkout", "main"]);
    std::fs::write(env.repo.join("LICENSE"), "MIT\n").unwrap();
    env.git(&["add", "LICENSE"]);
    env.git(&["commit", "-m", "main thêm LICENSE"]);
    env.git(&["checkout", "task/sua-app"]);

    env.hold_project(900, None);
    queued_id(env.queue("run_dev", None));
    env.release();

    assert_eq!(env.tick(), vec![1]);
    assert_eq!(env.jobs_of(1)[0].role, "dev");
}

// (i) lỗi khi kiểm tra plan: giữ waiting, không chạy gì

#[test]
fn plan_check_error_keeps_the_row_waiting_and_runs_nothing() {
    let env = Env::new();
    env.hold_project(900, None);
    queued_id(env.queue("run_dev", None));
    // Nhánh gốc không còn: `git diff base..nhánh` lỗi -> không chắc.
    env.sb
        .db()
        .execute(
            "UPDATE tasks SET base_branch = 'nhanh-da-xoa' WHERE id = 1",
            [],
        )
        .unwrap();
    env.release();

    for _ in 0..2 {
        assert!(env.tick().is_empty());
        let row = env.only_row();
        assert_eq!(row.status, "waiting", "{row:?}");
        assert_eq!(row.action, "run_dev", "not sent back to Lead: {row:?}");
        assert_eq!(row.note, None);
        assert!(env.jobs_of(1).is_empty(), "Dev must not run when unsure");
    }

    // Hết lỗi (nhánh gốc có lại) thì tick sau chạy Dev.
    env.sb
        .db()
        .execute("UPDATE tasks SET base_branch = 'main' WHERE id = 1", [])
        .unwrap();
    assert_eq!(env.tick(), vec![1]);
    assert_eq!(env.jobs_of(1)[0].role, "dev");
}

// (j) huỷ

#[test]
fn cancelled_row_does_not_run() {
    let env = Env::new();
    env.hold_project(900, None);
    queued_id(env.queue("relead", None));

    assert_eq!(cancel_queued_action(&env.st, 1).unwrap(), Some(true));
    assert_eq!(env.only_row().status, "cancelled");
    // Huỷ lần hai: idempotent. Task không có: None.
    assert_eq!(cancel_queued_action(&env.st, 1).unwrap(), Some(false));
    assert_eq!(cancel_queued_action(&env.st, 999).unwrap(), None);

    env.release();
    assert!(env.tick().is_empty());
    assert_eq!(env.only_row().status, "cancelled");
    assert!(env.jobs_of(1).is_empty());
}

// (k) chạy cả khi orchestrator.enabled = false (qua schedule_tick)

#[test]
fn runs_from_schedule_tick_with_orchestrator_disabled() {
    let env = Env::with_config(json!({ "orchestrator": { "enabled": false } }));
    assert!(!env.st.orch().enabled);
    env.hold_project(900, None);
    queued_id(env.queue("relead", None));

    schedule_tick(&env.st).expect("schedule_tick");
    assert_eq!(env.only_row().status, "waiting");
    assert!(env.jobs_of(1).is_empty());

    env.release();
    schedule_tick(&env.st).expect("schedule_tick");
    assert_eq!(env.only_row().status, "started");
    assert_eq!(env.jobs_of(1).len(), 1);
}

// (n) driver autopilot của task khác còn sống nhưng không có run `running`: vẫn chờ

#[test]
fn live_autopilot_driver_without_a_running_run_keeps_the_row_waiting() {
    let env = Env::new();
    run_lead(&env.sb, "task-hai", "việc hai");
    env.git(&["checkout", "task/sua-app"]);
    let mut driver = fake_live_driver(&env.sb, 2);

    let outcome = env.queue("relead", None);
    if !matches!(outcome, QueueOutcome::Queued { .. }) {
        let _ = driver.kill();
        let _ = driver.wait();
        panic!("relead is busy while the driver lives, got {outcome:?}");
    }
    assert!(env.tick().is_empty());
    assert!(env.tick().is_empty());
    assert_eq!(env.only_row().status, "waiting");
    assert!(
        env.jobs_of(1).is_empty(),
        "driver of task 2 still holds the project"
    );

    // Driver chết: dự án rảnh, hàng chạy ở tick sau.
    let _ = driver.kill();
    let _ = driver.wait();
    assert_eq!(env.tick(), vec![1]);
    assert_eq!(env.only_row().status, "started");
}

// (l) + (m) API: mã trả về, `queued_action` trong GET, không rò đường dẫn tuyệt đối

#[test]
fn api_queue_cancel_and_task_json() {
    let env = Env::new();
    run_lead(&env.sb, "task-hai", "việc hai");
    env.git(&["checkout", "task/sua-app"]);
    let mut driver = fake_live_driver(&env.sb, 2);

    let mut srv = env.sb.serve(&[]);
    let root = env.sb.root.to_string_lossy().to_string();

    // Chưa xếp hàng: `queued_action` là null.
    let g = srv.get("/api/tasks/1");
    assert_eq!(g.status, 200);
    assert!(g.json()["queued_action"].is_null());

    let r = srv.post_json("/api/tasks/1/queue-action", &json!({ "action": "relead" }));
    assert_eq!(r.status, 201, "{}", r.text());
    let body = r.json();
    assert_eq!(body["action"], "relead");
    assert_eq!(body["status"], "waiting");
    assert!(body["queued_id"].as_i64().unwrap() > 0);
    assert!(!r.text().contains(&root), "no absolute path in {}", r.text());

    let g = srv.get("/api/tasks/1");
    let qa = g.json()["queued_action"].clone();
    assert_eq!(qa["action"], "relead");
    assert_eq!(qa["id"], body["queued_id"]);
    assert!(qa["note"].is_null());
    assert!(qa["reason"].is_null());
    assert!(qa["created_at"].as_i64().unwrap() > 0);
    assert!(!qa.to_string().contains(&root), "no absolute path in {qa}");

    // Xếp hàng lần hai thay hàng trước và mang ghi chú.
    let r2 = srv.post_json(
        "/api/tasks/1/queue-action",
        &json!({ "action": "relead", "note": "làm lại" }),
    );
    assert_eq!(r2.status, 201, "{}", r2.text());
    assert_ne!(r2.json()["queued_id"], body["queued_id"]);
    let qa = srv.get("/api/tasks/1").json()["queued_action"].clone();
    assert_eq!(qa["id"], r2.json()["queued_id"]);
    assert_eq!(qa["note"], "làm lại");

    // Từ chối: hành động không có trong trạng thái -> 409; hành động lạ -> 400; task lạ -> 404.
    let r = srv.post_json("/api/tasks/1/queue-action", &json!({ "action": "run_qa" }));
    assert_eq!(r.status, 409, "{}", r.text());
    assert_eq!(r.json()["kind"], "not_queueable");
    assert!(!r.text().contains(&root));
    let r = srv.post_json("/api/tasks/1/queue-action", &json!({ "action": "approve" }));
    assert_eq!(r.status, 400, "{}", r.text());
    let r = srv.post_json("/api/tasks/999/queue-action", &json!({ "action": "relead" }));
    assert_eq!(r.status, 404, "{}", r.text());

    // Huỷ: 200, idempotent; task lạ: 404; GET lại là null.
    let c = srv.post_json("/api/tasks/1/queue-action/cancel", &json!({}));
    assert_eq!(c.status, 200, "{}", c.text());
    assert_eq!(c.json()["cancelled"], true);
    let c = srv.post_json("/api/tasks/1/queue-action/cancel", &json!({}));
    assert_eq!(c.status, 200, "{}", c.text());
    assert_eq!(c.json()["cancelled"], false);
    let c = srv.post_json("/api/tasks/999/queue-action/cancel", &json!({}));
    assert_eq!(c.status, 404, "{}", c.text());
    assert!(srv.get("/api/tasks/1").json()["queued_action"].is_null());

    srv.stop(libc::SIGINT, Duration::from_secs(10));
    let _ = driver.kill();
    let _ = driver.wait();
}

// Hàm đọc git cho bước (c): chỉ đọc, từ chối đối số giống cờ.

#[test]
fn changed_names_between_reads_the_diff_and_rejects_flag_like_refs() {
    let env = Env::new();
    env.git(&["checkout", "main"]);
    std::fs::write(env.repo.join("a.txt"), "a\n").unwrap();
    env.git(&["add", "a.txt"]);
    env.git(&["commit", "-m", "thêm a"]);
    env.git(&["checkout", "task/sua-app"]);
    let base = env.git(&["rev-parse", "main~1"]).trim().to_string();

    let names = agent_brain::gitops::changed_names_between(&env.repo, &base, "main").unwrap();
    assert_eq!(
        names.into_iter().collect::<Vec<_>>(),
        vec!["a.txt".to_string()]
    );
    assert!(agent_brain::gitops::changed_names_between(&env.repo, "-p", "main").is_err());
    assert!(agent_brain::gitops::changed_names_between(&env.repo, &base, "--output=x").is_err());
    assert!(agent_brain::gitops::changed_names_between(&env.repo, "", "main").is_err());
    assert!(
        agent_brain::gitops::changed_names_between(&env.repo, &base, "no-such-branch").is_err()
    );
}
