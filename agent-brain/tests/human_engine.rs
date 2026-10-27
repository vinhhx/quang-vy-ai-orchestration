//! Phase 6 plan 12: a person does the Dev step (D-19..D-25). agent-brain pushes the task branch,
//! waits without any process or `running` row, fetches the pushed branch fast-forward-only on
//! `human done`, checks the diff and records the work as a finished `người` run. The remote is a
//! local bare repository and the "staff" is a second clone; Claude is the fake one.

mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use agent_brain::teams::TaskTeam;
use common::{RunOut, Sandbox};
use serde_json::{json, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn lead_plan_entry() -> Value {
    protocol::lead_scenario(&[("src/a.txt", "a()", "đổi nội dung", "demo")])
}

/// A bare repository under the Sandbox root, used as `origin`.
fn bare_remote(sb: &Sandbox) -> PathBuf {
    let dir = sb.root.join("remote.git");
    std::fs::create_dir_all(&dir).unwrap();
    sb.git(&dir, &["init", "--bare", "-b", "main"]);
    dir
}

/// The staff's machine: a clone of `remote` that checks out `branch`, applies `edits`, commits
/// with `message` and pushes (no force unless `force`). Returns the clone path.
fn staff_clone_push(
    sb: &Sandbox,
    remote: &std::path::Path,
    branch: &str,
    edits: &[(&str, &str)],
    message: &str,
) -> PathBuf {
    let clone = sb.root.join("staff");
    if !clone.join(".git").exists() {
        sb.git(
            &sb.root,
            &["clone", remote.to_str().unwrap(), clone.to_str().unwrap()],
        );
    }
    sb.git(&clone, &["fetch", "origin"]);
    sb.git(
        &clone,
        &["checkout", "-B", branch, &format!("origin/{branch}")],
    );
    for (rel, content) in edits {
        let p = clone.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, content).unwrap();
    }
    sb.git(&clone, &["add", "-A"]);
    sb.git(&clone, &["commit", "-m", message]);
    sb.git(&clone, &["push", "origin", branch]);
    clone
}

struct World {
    repo: PathBuf,
    remote: Option<PathBuf>,
}

/// Repo `web` (optionally with a bare `origin`), staff `ha` and team `doi-nguoi`
/// (Dev = `người:ha`). With `autopilot` the map entry has no `workflow` key (server default:
/// mac-dinh), otherwise it keeps the manual `nhanh` workflow. No task yet.
fn setup_base(sb: &Sandbox, with_origin: bool, autopilot: bool) -> World {
    let remote = with_origin.then(|| bare_remote(sb));
    let repo = sb.make_repo(
        "web",
        &[("src/a.txt", "a\n")],
        None,
        remote.as_ref().map(|r| r.to_str().unwrap()),
    );
    if remote.is_some() {
        sb.git(&repo, &["push", "-u", "origin", "main"]);
    }
    let mut entry = sb.project_entry(&repo, "web", "work");
    if autopilot {
        entry.as_object_mut().unwrap().remove("workflow");
    }
    sb.write_map(json!([entry]), &[]);
    sb.write_staff(json!({"ha": {"display_name": "Hà"}}));
    sb.write_teams(json!({
        "doi-nguoi": {"description": "Dev do Hà làm", "roles": {"dev": "người:ha"}}
    }));
    World { repo, remote }
}

/// `setup_base` plus task 1 (`task/sua-app`) created by `run lead --team doi-nguoi`, plan committed.
fn setup(sb: &Sandbox, with_origin: bool) -> World {
    let world = setup_base(sb, with_origin, false);
    let res = sb.run(
        &[
            "run",
            "lead",
            "--team",
            "doi-nguoi",
            "--name",
            "sua-app",
            "--",
            "web",
            "sửa app",
        ],
        Some(json!({ "by_role": { "lead": [lead_plan_entry()] } })),
    );
    assert_ok(&res, "run lead --team doi-nguoi");
    world
}

fn assert_ok(res: &RunOut, what: &str) {
    assert_eq!(
        res.code, 0,
        "{what} failed: stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
}

fn both(res: &RunOut) -> String {
    format!("{}\n{}", res.stdout, res.stderr)
}

fn run_dev(sb: &Sandbox) -> RunOut {
    sb.run(&["run", "dev", "--", "web"], None)
}

fn human(sb: &Sandbox, action: &str) -> RunOut {
    sb.run(&["human", action, "1"], None)
}

fn team_json(sb: &Sandbox) -> Option<String> {
    sb.db()
        .query_row("SELECT team_json FROM tasks WHERE id = 1", [], |r| {
            r.get::<_, Option<String>>(0)
        })
        .unwrap()
}

fn task_team(sb: &Sandbox) -> TaskTeam {
    TaskTeam::parse(team_json(sb).as_deref()).expect("task 1 has a parsable team_json")
}

fn run_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap()
}

fn running_count(sb: &Sandbox) -> i64 {
    sb.db()
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE status = 'running'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

fn task_status(sb: &Sandbox) -> String {
    sb.db()
        .query_row("SELECT status FROM tasks WHERE id = 1", [], |r| r.get(0))
        .unwrap()
}

fn head(sb: &Sandbox, repo: &std::path::Path) -> String {
    sb.git(repo, &["rev-parse", "HEAD"]).trim().to_string()
}

struct HumanRunRow {
    status: String,
    outcome: Option<String>,
    usage_source: String,
    tokens: i64,
    stage: Option<String>,
    round: Option<i64>,
    started_at: i64,
    base_commit: Option<String>,
    metrics: Value,
}

/// The `người` runs of task 1, oldest first.
fn human_runs(sb: &Sandbox) -> Vec<HumanRunRow> {
    let conn = sb.db();
    let mut stmt = conn
        .prepare(
            "SELECT status, outcome, usage_source, input_tokens + output_tokens, stage, round,
                    started_at, base_commit, metrics_json
             FROM runs WHERE engine = 'người' ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |r| {
        let metrics: Option<String> = r.get(8)?;
        Ok(HumanRunRow {
            status: r.get(0)?,
            outcome: r.get(1)?,
            usage_source: r.get(2)?,
            tokens: r.get(3)?,
            stage: r.get(4)?,
            round: r.get(5)?,
            started_at: r.get(6)?,
            base_commit: r.get(7)?,
            metrics: serde_json::from_str(&metrics.unwrap_or_else(|| "{}".to_string())).unwrap(),
        })
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

// ---------------------------------------------------------------------------
// Task 1 (tracer): wait, push, Xong
// ---------------------------------------------------------------------------

#[test]
fn human_dev_waits_and_done_fetches() {
    let sb = Sandbox::new();
    let w = setup(&sb, true);
    let remote = w.remote.clone().unwrap();
    let base = head(&sb, &w.repo);
    let runs_before = run_count(&sb);
    let records_before = sb.records().len();

    let res = run_dev(&sb);
    assert_eq!(
        res.code, 6,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    let out = both(&res);
    assert!(
        out.contains("Chờ Hà làm bước Code trên nhánh task/sua-app."),
        "{out}"
    );
    assert!(
        out.contains("Đã đẩy nhánh task/sua-app lên remote origin."),
        "{out}"
    );
    assert_eq!(run_count(&sb), runs_before, "a human wait inserts no run");
    assert_eq!(running_count(&sb), 0);
    assert_eq!(
        sb.records().len(),
        records_before,
        "claude must not be called"
    );

    let wait = task_team(&sb).wait.expect("wait marker");
    assert_eq!(wait.mode, "waiting");
    assert_eq!(wait.role, "dev");
    assert_eq!(wait.stage, "implement");
    assert_eq!(wait.fix_for, None);
    assert_eq!(wait.staff, "ha");
    assert_eq!(wait.branch, "task/sua-app");
    assert_eq!(wait.base, base);
    assert!(!wait.autopilot);
    let push = wait.push.expect("push result");
    assert!(push.ok, "{push:?}");
    assert_eq!(push.command, "git push -u origin task/sua-app");
    sb.git(
        &remote,
        &["rev-parse", "--verify", "refs/heads/task/sua-app"],
    );
    let since_ms = wait.since_ms;

    staff_clone_push(
        &sb,
        &remote,
        "task/sua-app",
        &[("src/a.txt", "a của Hà\n")],
        "sua-app: xong phần của Hà",
    );
    let staff_head = sb
        .git(&remote, &["rev-parse", "refs/heads/task/sua-app"])
        .trim()
        .to_string();

    let done = human(&sb, "done");
    assert_ok(&done, "human done");
    assert!(
        both(&done).contains("Đã lấy nhánh task/sua-app của Hà — run "),
        "{}",
        both(&done)
    );
    assert_eq!(head(&sb, &w.repo), staff_head);
    assert_eq!(
        std::fs::read_to_string(w.repo.join("src/a.txt")).unwrap(),
        "a của Hà\n"
    );

    let rows = human_runs(&sb);
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.status, "succeeded");
    assert_eq!(r.outcome, None);
    assert_eq!(r.usage_source, "none");
    assert_eq!(r.tokens, 0);
    assert_eq!(r.stage.as_deref(), Some("implement"));
    assert_eq!(r.round, None);
    assert_eq!(r.started_at, since_ms);
    assert_eq!(r.base_commit.as_deref(), Some(base.as_str()));
    assert_eq!(r.metrics["staff"], json!("ha"));
    assert_eq!(r.metrics["team"], json!("doi-nguoi"));
    assert_eq!(r.metrics["human"]["branch"], json!("task/sua-app"));
    assert_eq!(r.metrics["human"]["base"], json!(base));
    assert_eq!(r.metrics["human"]["head"], json!(staff_head));
    assert_eq!(running_count(&sb), 0);
    assert!(task_team(&sb).wait.is_none());
    assert!(team_json(&sb).unwrap().contains("\"wait\":null"));
    assert_eq!(task_status(&sb), "dev_done");
}

#[test]
fn human_wait_idempotent() {
    let sb = Sandbox::new();
    let w = setup(&sb, true);
    let _ = w;

    let first = run_dev(&sb);
    assert_eq!(first.code, 6, "{}", both(&first));
    let marker = team_json(&sb);
    let since = task_team(&sb).wait.unwrap().since_ms;
    std::thread::sleep(std::time::Duration::from_millis(20));

    let second = run_dev(&sb);
    assert_eq!(second.code, 6, "{}", both(&second));
    assert!(
        both(&second).contains("Chờ Hà làm bước Code trên nhánh task/sua-app."),
        "{}",
        both(&second)
    );
    assert_eq!(team_json(&sb), marker, "the marker must stay unchanged");
    assert_eq!(task_team(&sb).wait.unwrap().since_ms, since);
    assert_eq!(run_count(&sb), 1, "only the lead run exists");
}

#[test]
fn human_wait_without_origin() {
    let sb = Sandbox::new();
    let _w = setup(&sb, false);

    let res = run_dev(&sb);
    assert_eq!(res.code, 6, "{}", both(&res));
    assert!(
        res.stdout.contains("git push -u origin task/sua-app"),
        "stdout: {}",
        res.stdout
    );
    let wait = task_team(&sb).wait.expect("wait marker");
    let push = wait.push.expect("push result");
    assert!(!push.ok);
    assert_eq!(push.command, "git push -u origin task/sua-app");
    assert!(push.error.is_some());
    assert_eq!(wait.mode, "waiting");
}

// ---------------------------------------------------------------------------
// Task 2: Xong refusals, scope check, fix steps in `rejected` mode, Trả lại
// ---------------------------------------------------------------------------

/// What a refused `human done` must leave untouched: marker, run rows and local HEAD.
fn snapshot(sb: &Sandbox, repo: &std::path::Path) -> (Option<String>, i64, String) {
    (team_json(sb), run_count(sb), head(sb, repo))
}

fn assert_refused(res: &RunOut, text: &str) {
    assert_eq!(
        res.code, 1,
        "stderr: {}\nstdout: {}",
        res.stderr, res.stdout
    );
    assert!(
        both(res).contains(text),
        "missing {text:?} in: {}",
        both(res)
    );
}

#[test]
fn done_refusals_keep_marker() {
    let sb = Sandbox::new();
    let w = setup(&sb, true);
    let remote = w.remote.clone().unwrap();

    // No marker yet.
    assert_refused(&human(&sb, "done"), "Task 1 không chờ nhân sự làm");

    assert_eq!(run_dev(&sb).code, 6);
    let before = snapshot(&sb, &w.repo);

    // Not on the task branch.
    sb.git(&w.repo, &["checkout", "main"]);
    let main_head = head(&sb, &w.repo);
    assert_refused(
        &human(&sb, "done"),
        "Không ở nhánh task 'task/sua-app' (nhánh hiện tại: 'main') — không đoán",
    );
    assert_eq!(head(&sb, &w.repo), main_head);
    sb.git(&w.repo, &["checkout", "task/sua-app"]);

    // Dirty file that is not a protocol file.
    std::fs::write(w.repo.join("src/rac.txt"), "dở dang\n").unwrap();
    let dirty = human(&sb, "done");
    assert_refused(&dirty, "Working tree có thay đổi chưa commit");
    assert!(both(&dirty).contains("src/rac.txt"), "{}", both(&dirty));
    std::fs::remove_file(w.repo.join("src/rac.txt")).unwrap();

    // The staff has pushed nothing new: the remote branch equals the local one.
    assert_refused(
        &human(&sb, "done"),
        "Nhánh task/sua-app trên remote chưa có commit mới nào — nhờ Hà đẩy việc đã làm rồi bấm Xong lại.",
    );
    assert_eq!(snapshot(&sb, &w.repo), before);

    // The branch is gone from the remote.
    sb.git(&remote, &["update-ref", "-d", "refs/heads/task/sua-app"]);
    assert_refused(
        &human(&sb, "done"),
        "Chưa thấy nhánh task/sua-app trên remote — nhờ Hà đẩy nhánh lên rồi bấm Xong lại.",
    );
    assert_eq!(snapshot(&sb, &w.repo), before);
    assert_eq!(running_count(&sb), 0);
    assert!(human_runs(&sb).is_empty());
}

#[test]
fn done_without_origin_refused() {
    let sb = Sandbox::new();
    let w = setup(&sb, false);
    assert_eq!(run_dev(&sb).code, 6);
    let before = snapshot(&sb, &w.repo);
    assert_refused(
        &human(&sb, "done"),
        "Dự án chưa có remote origin — không lấy được nhánh của Hà.",
    );
    assert_eq!(snapshot(&sb, &w.repo), before);
}

#[test]
fn done_diverged_branch_refused() {
    let sb = Sandbox::new();
    let w = setup(&sb, true);
    let remote = w.remote.clone().unwrap();
    assert_eq!(run_dev(&sb).code, 6);

    // The local branch gains a commit the remote lacks, then the staff pushes a different one
    // on top of the old tip: the remote no longer continues from the local branch.
    std::fs::write(w.repo.join("src/a.txt"), "a cục bộ\n").unwrap();
    sb.git(&w.repo, &["add", "src/a.txt"]);
    sb.git(&w.repo, &["commit", "-m", "sua-app: commit cục bộ"]);
    staff_clone_push(
        &sb,
        &remote,
        "task/sua-app",
        &[("src/a.txt", "a của Hà\n")],
        "sua-app: của Hà",
    );
    let before = snapshot(&sb, &w.repo);

    let res = human(&sb, "done");
    assert_refused(
        &res,
        "Nhánh task/sua-app trên remote không đi tiếp từ nhánh trong máy (có thể đã bị viết lại lịch sử) — không gộp. Kiểm tra với Hà rồi thử lại.",
    );
    assert_eq!(snapshot(&sb, &w.repo), before, "history must be untouched");
    assert_eq!(
        std::fs::read_to_string(w.repo.join("src/a.txt")).unwrap(),
        "a cục bộ\n"
    );
    assert!(task_team(&sb).wait.is_some());
}

#[test]
fn done_scope_violation() {
    for stray in ["src/khac.txt", ".planning/ghi-chu.md", ".github/ci.yml"] {
        let sb = Sandbox::new();
        let w = setup(&sb, true);
        let remote = w.remote.clone().unwrap();
        assert_eq!(run_dev(&sb).code, 6);
        staff_clone_push(
            &sb,
            &remote,
            "task/sua-app",
            &[("src/a.txt", "a của Hà\n"), (stray, "ngoài plan\n")],
            "sua-app: xong",
        );

        let res = human(&sb, "done");
        assert_eq!(res.code, 3, "{stray}: {}", both(&res));
        assert!(res.stdout.contains(stray), "{stray}: {}", res.stdout);
        assert!(
            res.stdout.contains("agent-brain không tự hoàn tác"),
            "{}",
            res.stdout
        );
        let rows = human_runs(&sb);
        assert_eq!(rows.len(), 1, "{stray}");
        assert_eq!(rows[0].outcome.as_deref(), Some("protocol_violation"));
        assert_eq!(rows[0].status, "succeeded");
        assert!(task_team(&sb).wait.is_none());
        // Nothing is reverted: the stray file is still in the tree.
        assert!(w.repo.join(stray).is_file(), "{stray} must stay");
        assert_ne!(task_status(&sb), "dev_done", "{stray}");
    }
}

fn qa_needs_fix_scenario() -> Value {
    let report = "# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- lỗi logic\n## Sửa ngoài plan (nếu có)\n## Rủi ro\n## Kết luận\nCần sửa\n";
    json!({"ops": [{"op": "write", "path": ".qa-report.md", "content": report}]})
}

#[test]
fn fix_step_rejected_and_return() {
    let sb = Sandbox::new();
    let w = setup(&sb, true);
    let remote = w.remote.clone().unwrap();

    assert_eq!(run_dev(&sb).code, 6);
    staff_clone_push(
        &sb,
        &remote,
        "task/sua-app",
        &[("src/a.txt", "a của Hà\n")],
        "sua-app: xong",
    );
    assert_ok(&human(&sb, "done"), "human done (implement)");

    let qa = sb.run(&["run", "qa", "--", "web"], Some(qa_needs_fix_scenario()));
    assert_ok(&qa, "run qa");
    let runs_before = run_count(&sb);
    let records_before = sb.records().len();

    let fix = sb.run(
        &["run", "dev", "--fix", "qa", "--round", "1", "--", "web"],
        None,
    );
    assert_eq!(fix.code, 6, "{}", both(&fix));
    assert!(
        both(&fix).contains(
            "Kết quả của Hà chưa đạt — gửi báo cáo .qa-report.md cho Hà rồi bấm Trả lại (agent-brain human return 1)."
        ),
        "{}",
        both(&fix)
    );
    assert_eq!(run_count(&sb), runs_before);
    assert_eq!(
        sb.records().len(),
        records_before,
        "claude must not be called"
    );
    let wait = task_team(&sb).wait.expect("marker");
    assert_eq!(wait.mode, "rejected");
    assert_eq!(wait.stage, "fix");
    assert_eq!(wait.fix_for.as_deref(), Some("qa"));
    assert_eq!(wait.round, Some(1));
    assert_eq!(wait.report_paths, vec![".qa-report.md".to_string()]);
    let rejected_since = wait.since_ms;

    // Xong is refused while the marker is `rejected`.
    assert_refused(
        &human(&sb, "done"),
        "Bấm Trả lại trước: kết quả lần trước của Hà chưa đạt.",
    );

    std::thread::sleep(std::time::Duration::from_millis(20));
    let back = human(&sb, "return");
    assert_ok(&back, "human return");
    assert!(
        both(&back).contains("Đã trả việc lại cho Hà."),
        "{}",
        both(&back)
    );
    let wait = task_team(&sb).wait.expect("marker");
    assert_eq!(wait.mode, "waiting");
    assert!(wait.since_ms > rejected_since);
    assert!(wait.push.expect("push").ok);
    assert_eq!(wait.stage, "fix");

    // A second Trả lại is refused.
    assert_refused(
        &human(&sb, "return"),
        "Chỉ trả lại khi kết quả của Hà chưa đạt.",
    );

    staff_clone_push(
        &sb,
        &remote,
        "task/sua-app",
        &[("src/a.txt", "a sửa theo QA\n")],
        "sua-app: sửa theo QA",
    );
    assert_ok(&human(&sb, "done"), "human done (fix)");
    let rows = human_runs(&sb);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].stage.as_deref(), Some("fix"));
    assert_eq!(rows[1].round, Some(1));
    assert_eq!(rows[1].metrics["fix_for"], json!("qa"));
    assert_eq!(rows[1].outcome, None);
    assert!(task_team(&sb).wait.is_none());
    assert_eq!(task_status(&sb), "dev_done");
}

#[test]
fn return_requires_rejected() {
    let sb = Sandbox::new();
    let _w = setup(&sb, true);

    assert_refused(&human(&sb, "return"), "Task 1 không chờ nhân sự làm");
    assert_eq!(run_dev(&sb).code, 6);
    let marker = team_json(&sb);
    assert_refused(
        &human(&sb, "return"),
        "Chỉ trả lại khi kết quả của Hà chưa đạt.",
    );
    assert_eq!(team_json(&sb), marker);
}

// ---------------------------------------------------------------------------
// Task 3: autopilot waits for the person and resumes after Xong
// ---------------------------------------------------------------------------

fn by_role(roles: Value) -> Value {
    json!({ "by_role": roles })
}

fn review_report(verdict: &str, blocking: i64, advisory: i64) -> String {
    let mut body = format!(
        "---\nverdict: {verdict}\nblocking: {blocking}\nadvisory: {advisory}\nfiles: []\n---\n# Review: sửa app\n## Mục chặn (blocking)\n"
    );
    for i in 0..blocking {
        body.push_str(&format!("- mục chặn {}\n", i + 1));
    }
    body.push_str("## Góp ý (advisory)\n");
    for i in 0..advisory {
        body.push_str(&format!("- góp ý {}\n", i + 1));
    }
    body
}

fn plan_review_entry(verdict: &str, blocking: i64, advisory: i64) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".plan-review.md",
        "content": review_report(verdict, blocking, advisory),
    }]})
}

fn code_review_entry(verdict: &str, blocking: i64) -> Value {
    json!({"ops": [{
        "op": "write",
        "path": ".code-review.md",
        "content": review_report(verdict, blocking, 1),
    }]})
}

fn qa_pass_entry() -> Value {
    let report = "# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- src/a.txt: đã xem.\n## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông.\n## Kết luận\nĐạt\n";
    json!({"ops": [{"op": "write", "path": ".qa-report.md", "content": report}]})
}

fn auto_state_of(sb: &Sandbox) -> (Option<String>, Option<i64>, Value) {
    let (state, pid, aj): (Option<String>, Option<i64>, Option<String>) = sb
        .db()
        .query_row(
            "SELECT auto_state, auto_pid, auto_json FROM tasks WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    let aj = serde_json::from_str(&aj.unwrap_or_else(|| "{}".to_string())).unwrap();
    (state, pid, aj)
}

fn run_roles(sb: &Sandbox) -> Vec<(String, String)> {
    let conn = sb.db();
    let mut stmt = conn
        .prepare("SELECT role, engine FROM runs ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

/// mac-dinh through `auto --start --team doi-nguoi`: Lead, plan review approves, stops at the gate.
fn auto_to_gate(sb: &Sandbox) -> World {
    let w = setup_base(sb, true, true);
    let sc = by_role(json!({
        "lead": [lead_plan_entry()],
        "plan-review": [plan_review_entry("approve", 0, 2)],
    }));
    let res = sb.run(
        &[
            "auto",
            "--start",
            "--name",
            "sua-app",
            "--team",
            "doi-nguoi",
            "--",
            "web",
            "sửa app",
        ],
        Some(sc),
    );
    assert_ok(&res, "auto --start");
    assert_eq!(auto_state_of(sb).0.as_deref(), Some("gate"));
    w
}

/// `approve 1`: the driver reaches the Dev step, the child exits 6 and the task waits.
fn approve_to_waiting(sb: &Sandbox) {
    let res = sb.run(&["approve", "1"], None);
    assert_ok(&res, "approve 1");
    assert!(
        both(&res).contains("chờ nhân sự làm — bấm Xong khi họ báo xong."),
        "{}",
        both(&res)
    );
}

#[test]
fn autopilot_waits_for_human() {
    let sb = Sandbox::new();
    let _w = auto_to_gate(&sb);
    let runs_before = run_count(&sb);

    approve_to_waiting(&sb);

    let (state, pid, aj) = auto_state_of(&sb);
    assert_eq!(state.as_deref(), Some("waiting_human"));
    assert_eq!(pid, None, "the driver pid is cleared");
    assert!(
        aj.get("stop").map(|s| s.is_null()).unwrap_or(true),
        "no stop recorded: {aj}"
    );
    assert_eq!(run_count(&sb), runs_before, "no run for the human wait");
    assert_eq!(running_count(&sb), 0);
    let wait = task_team(&sb).wait.expect("marker");
    assert_eq!(wait.mode, "waiting");
    assert_eq!(wait.stage, "implement");
    assert!(wait.autopilot);
    assert_ne!(task_status(&sb), "blocked");
}

#[test]
fn autopilot_resumes_after_done() {
    let sb = Sandbox::new();
    let w = auto_to_gate(&sb);
    let remote = w.remote.clone().unwrap();
    approve_to_waiting(&sb);

    staff_clone_push(
        &sb,
        &remote,
        "task/sua-app",
        &[("src/a.txt", "a của Hà\n")],
        "sua-app: xong phần của Hà",
    );
    let sc = by_role(json!({
        "code-review": [code_review_entry("pass", 0)],
        "qa": [qa_pass_entry()],
    }));
    let done = sb.run(&["human", "done", "1"], Some(sc));
    assert_ok(&done, "human done");

    assert_eq!(
        auto_state_of(&sb).0.as_deref(),
        Some("done"),
        "{}",
        both(&done)
    );
    let roles = run_roles(&sb);
    let expected: Vec<(String, String)> = [
        ("lead", "claude"),
        ("plan-review", "claude"),
        ("dev", "người"),
        ("code-review", "claude"),
        ("qa", "claude"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    assert_eq!(roles, expected, "{}", both(&done));
    assert!(task_team(&sb).wait.is_none());
    assert_eq!(human_runs(&sb).len(), 1);
}

#[test]
fn autopilot_fix_goes_back_rejected() {
    let sb = Sandbox::new();
    let w = auto_to_gate(&sb);
    let remote = w.remote.clone().unwrap();
    approve_to_waiting(&sb);
    staff_clone_push(
        &sb,
        &remote,
        "task/sua-app",
        &[("src/a.txt", "a của Hà\n")],
        "sua-app: xong phần của Hà",
    );

    // Code review asks for a fix: the driver runs the fix stage, which is a person's step again.
    let sc = by_role(json!({ "code-review": [code_review_entry("revise", 1)] }));
    let done = sb.run(&["human", "done", "1"], Some(sc));
    assert_ok(&done, "human done");
    let (state, pid, aj) = auto_state_of(&sb);
    assert_eq!(state.as_deref(), Some("waiting_human"), "{}", both(&done));
    assert_eq!(pid, None);
    assert!(aj.get("stop").map(|s| s.is_null()).unwrap_or(true), "{aj}");
    let wait = task_team(&sb).wait.expect("rejected marker");
    assert_eq!(wait.mode, "rejected");
    assert_eq!(wait.stage, "fix");
    assert_eq!(wait.fix_for.as_deref(), Some("code_review"));
    assert_eq!(wait.report_paths, vec![".code-review.md".to_string()]);
    assert!(wait.autopilot);

    // Trả lại, the person fixes, Xong: the autopilot goes on to code review and QA.
    assert_ok(&human(&sb, "return"), "human return");
    staff_clone_push(
        &sb,
        &remote,
        "task/sua-app",
        &[("src/a.txt", "a của Hà, đã sửa\n")],
        "sua-app: sửa theo review",
    );
    let sc = by_role(json!({
        "code-review": [code_review_entry("pass", 0)],
        "qa": [qa_pass_entry()],
    }));
    let done = sb.run(&["human", "done", "1"], Some(sc));
    assert_ok(&done, "human done (fix)");
    assert_eq!(
        auto_state_of(&sb).0.as_deref(),
        Some("done"),
        "{}",
        both(&done)
    );
    let rows = human_runs(&sb);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].stage.as_deref(), Some("fix"));
    assert_eq!(rows[1].metrics["fix_for"], json!("code_review"));
}

#[test]
fn autopilot_violation_is_recorded_as_stop() {
    let sb = Sandbox::new();
    let w = auto_to_gate(&sb);
    let remote = w.remote.clone().unwrap();
    approve_to_waiting(&sb);
    staff_clone_push(
        &sb,
        &remote,
        "task/sua-app",
        &[
            ("src/a.txt", "a của Hà\n"),
            ("src/khac.txt", "ngoài plan\n"),
        ],
        "sua-app: xong",
    );

    // The same process resumes the autopilot, so decide records the stop.
    let done = sb.run(&["human", "done", "1"], None);
    assert_eq!(done.code, 4, "{}", both(&done));
    let (state, pid, aj) = auto_state_of(&sb);
    assert_eq!(state.as_deref(), Some("stopped"), "{}", both(&done));
    assert_eq!(pid, None);
    assert!(aj["stop"].is_object(), "{aj}");
    assert_eq!(
        human_runs(&sb)[0].outcome.as_deref(),
        Some("protocol_violation")
    );
    assert!(task_team(&sb).wait.is_none());
}
