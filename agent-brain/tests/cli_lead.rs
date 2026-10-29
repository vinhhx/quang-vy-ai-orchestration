mod common;
#[path = "support/protocol.rs"]
mod protocol;

use common::Sandbox;
use protocol::{lead_scenario, plan_text, setup_project};

#[test]
fn p1_06_lead_writes_only_plan() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let scenario = lead_scenario(&[("src/app.py", "1-5", "thêm nút đăng xuất", "yêu cầu")]);
    let out = sb.run(
        &["run", "lead", "web bán hàng", "thêm nút đăng xuất"],
        Some(scenario),
    );
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    // HEAD subject is them-nut-dang-xuat: plan
    let head_subject = sb.git(&repo, &["log", "-1", "--format=%s"]);
    assert_eq!(head_subject.trim(), "them-nut-dang-xuat: plan");

    // HEAD touches only .plan.md
    let diff_tree = sb.git(
        &repo,
        &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
    );
    assert_eq!(diff_tree.trim(), ".plan.md");

    // git status --porcelain is empty
    let status = sb.git(&repo, &["status", "--porcelain"]);
    assert_eq!(status.trim(), "");
}

#[test]
fn lead_extra_file_is_protocol_violation() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    let base_commit = sb.git(&repo, &["rev-parse", "HEAD"]);

    let p_text = plan_text(
        "Sửa app",
        &[("src/app.py", "1", "sửa", "lý do")],
        "python3 -m unittest",
        "",
    );
    let scenario = serde_json::json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "write", "path": ".plan.md", "content": p_text, "keep_meta": true},
            {"op": "write", "path": "src/other.py", "content": "# extra file\n"}
        ]
    });

    let out = sb.run(&["run", "lead", "web bán hàng", "sửa app"], Some(scenario));
    assert_eq!(
        out.code, 3,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stdout
            .contains("Lead đã sửa file ngoài .plan.md — vi phạm giao thức (ROLE-01):"),
        "stdout missing violation header: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("src/other.py"),
        "stdout missing src/other.py: {}",
        out.stdout
    );

    let head = sb.git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(head.trim(), base_commit.trim());

    assert!(repo.join("src/other.py").is_file());

    let conn = sb.db();
    let outcome: String = conn
        .query_row("SELECT outcome FROM runs WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(outcome, "protocol_violation");
}

#[test]
fn lead_invalid_plan_not_committed() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");
    let base_commit = sb.git(&repo, &["rev-parse", "HEAD"]);

    let scenario = serde_json::json!({ "ops": [] });
    let out = sb.run(&["run", "lead", "web bán hàng", "thêm nút"], Some(scenario));
    assert_eq!(
        out.code, 1,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stderr.contains("Kế hoạch trong .plan.md không hợp lệ"),
        "stderr missing invalid plan message: {}",
        out.stderr
    );
    assert!(
        out.stderr.contains("no_changes"),
        "stderr missing 'no_changes': {}",
        out.stderr
    );

    let head = sb.git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(head.trim(), base_commit.trim());
}

#[test]
fn lead_replan_keeps_base_commit() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    // First run
    let sc1 = lead_scenario(&[("src/app.py", "1", "sửa lần 1", "lý do 1")]);
    let out1 = sb.run(&["run", "lead", "web bán hàng", "thêm nút"], Some(sc1));
    assert_eq!(
        out1.code, 0,
        "out1 stderr: {}\nstdout: {}",
        out1.stderr, out1.stdout
    );

    let plan1 = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    let meta1 = agent_brain::planfile::read_meta(&plan1).expect("meta1");
    let base_commit1 = meta1
        .get("base_commit")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();

    let branch = sb.git(&repo, &["branch", "--show-current"]);
    assert_eq!(branch.trim(), "task/them-nut");

    // Second run on the same branch
    let sc2 = lead_scenario(&[("src/app.py", "2", "sửa lần 2", "lý do 2")]);
    let out2 = sb.run(&["run", "lead", "web bán hàng", "thêm nút"], Some(sc2));
    assert_eq!(
        out2.code, 0,
        "out2 stderr: {}\nstdout: {}",
        out2.stderr, out2.stdout
    );

    let plan2 = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    let meta2 = agent_brain::planfile::read_meta(&plan2).expect("meta2");
    let base_commit2 = meta2
        .get("base_commit")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();

    assert_eq!(base_commit1, base_commit2);

    let conn = sb.db();
    let task_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(task_count, 1);
}

#[test]
fn lead_branch_exists_not_checked_out() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    // Create branch task/them-nut-dang-xuat while staying on main
    sb.git(&repo, &["branch", "task/them-nut-dang-xuat"]);

    let out = sb.run(&["run", "lead", "web bán hàng", "thêm nút đăng xuất"], None);
    assert_eq!(
        out.code, 1,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stderr.contains("đã tồn tại nhưng chưa được checkout"),
        "stderr missing 'đã tồn tại nhưng chưa được checkout': {}",
        out.stderr
    );
}

#[test]
fn lead_stacking_warning() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    // First run creates task/a
    let sc1 = lead_scenario(&[("src/app.py", "1", "sửa a", "lý do a")]);
    let out1 = sb.run(
        &["run", "lead", "web bán hàng", "nhiệm vụ a", "--name", "a"],
        Some(sc1),
    );
    assert_eq!(out1.code, 0, "out1 stderr: {}", out1.stderr);

    // Current branch is task/a. Now run lead on b:
    let sc2 = lead_scenario(&[("src/app.py", "2", "sửa b", "lý do b")]);
    let out2 = sb.run(
        &["run", "lead", "web bán hàng", "nhiệm vụ b", "--name", "b"],
        Some(sc2),
    );
    assert_eq!(out2.code, 0, "out2 stderr: {}", out2.stderr);
    assert!(
        out2.stderr.contains("(stacking)"),
        "stderr missing '(stacking)': {}",
        out2.stderr
    );
}

#[test]
fn lead_dry_run_has_no_side_effects() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let out = sb.run(
        &["run", "lead", "web bán hàng", "thêm nút", "--dry-run"],
        None,
    );
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    assert!(
        out.stdout.contains("Project: web-ban-hang"),
        "missing Project: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("Profile: work ("),
        "missing Profile: work (: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("Model: opus"),
        "missing Model: opus: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("Branch: task/them-nut"),
        "missing Branch: task/them-nut: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("Command: "),
        "missing Command: : {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("<assets/prompts/lead.md>"),
        "missing prompt placeholder: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("'Write(./.plan.md)'"),
        "missing quoted Write(./.plan.md): {}",
        out.stdout
    );

    // No branch created
    let branches = sb.git(&repo, &["branch", "--list", "task/*"]);
    assert_eq!(branches.trim(), "");

    // No record file
    assert!(sb.records().is_empty());

    // count_runs 0, no tasks row
    let conn = sb.db();
    let runs_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(runs_count, 0);
    let tasks_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(tasks_count, 0);
}

#[test]
fn forbidden_flag_in_config_refused() {
    let sb = Sandbox::new();
    setup_project(&sb, "work");

    sb.write_config(serde_json::json!({
        "common_args": ["--strict-mcp-config", "--dangerously-skip-permissions"]
    }));

    let out = sb.run(&["run", "lead", "web bán hàng", "thêm nút"], None);
    assert_eq!(
        out.code, 1,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stderr
            .contains("Cờ quyền không được phép: --dangerously-skip-permissions"),
        "stderr missing forbidden flag message: {}",
        out.stderr
    );
    assert!(sb.records().is_empty());
}

struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn personal_profile_refuses_concurrent_run() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "personal");

    let decoy_child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn decoy sleep");
    let mut decoy = ChildGuard(decoy_child);
    let decoy_pid = decoy.0.id() as i32;
    let proc_stat = agent_brain::procs::read_stat(decoy_pid).expect("read decoy stat");
    let decoy_start = proc_stat.start;

    let conn = agent_brain::db::open(&sb.db_path).expect("open db");
    let map = sb.read_map();
    let projects = agent_brain::projects::projects_of(&map);
    agent_brain::db::sync_projects(&conn, &projects).expect("sync projects");
    let proj_row_id = agent_brain::db::project_row_id(&conn, "web-ban-hang").expect("proj id");

    let run_id = agent_brain::db::insert_run(
        &conn,
        &agent_brain::db::NewRun {
            task_id: None,
            project_id: proj_row_id,
            role: "lead".to_string(),
            profile: "personal".to_string(),
            model: Some("opus".to_string()),
            pid: decoy_pid as i64,
            pgid: decoy_pid as i64,
            pid_start: decoy_start as i64,
            base_commit: None,
            run_dir: None,
        },
    )
    .expect("insert run");

    // run lead on personal project exits 1 with message and no record
    let out = sb.run(&["run", "lead", "web bán hàng", "thêm nút 1"], None);
    assert_eq!(
        out.code, 1,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );
    assert!(
        out.stderr
            .contains("không chạy song song trên cùng thư mục cấu hình (E-06)"),
        "stderr missing E-06 message: {}",
        out.stderr
    );
    assert!(sb.records().is_empty());

    // Switch to profile work: update project map and run row
    let _ = setup_project(&sb, "work");
    conn.execute(
        "UPDATE runs SET profile = 'work' WHERE id = ?1",
        rusqlite::params![run_id],
    )
    .expect("update run profile");

    let sc = lead_scenario(&[("src/app.py", "1", "sửa", "lý do")]);
    let out_work = sb.run(&["run", "lead", "web bán hàng", "thêm nút 2"], Some(sc));
    assert_eq!(
        out_work.code, 0,
        "out_work stderr: {}\nstdout: {}",
        out_work.stderr, out_work.stdout
    );
    assert!(
        out_work.stderr.contains("đang có run"),
        "stderr missing 'đang có run': {}",
        out_work.stderr
    );

    // Explicitly kill decoy
    let _ = decoy.0.kill();
    let _ = decoy.0.wait();
}

#[test]
fn idea_project_lead_runs_on_shadow_git() {
    let sb = Sandbox::new();
    let dir = sb.repos.join("y-tuong");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/app.py"), "def app():\n    return 1\n").unwrap();

    let mut entry = sb.project_entry(&dir, "y-tuong", "work");
    entry["type"] = serde_json::json!("idea");
    entry["name_vi"] = serde_json::json!("Ý tưởng");
    sb.write_map(serde_json::json!([entry]), &[]);

    let scenario = lead_scenario(&[("src/app.py", "1-5", "thêm nút đăng xuất", "yêu cầu")]);
    let out = sb.run(
        &["run", "lead", "y-tuong", "thêm nút đăng xuất"],
        Some(scenario),
    );
    assert_eq!(
        out.code, 0,
        "stderr: {}\nstdout: {}",
        out.stderr, out.stdout
    );

    // the folder never gets a .git; the shadow repo lives under XDG_DATA_HOME
    assert!(!dir.join(".git").exists());
    let shadows: Vec<_> = std::fs::read_dir(sb.xdg_data.join("agent-brain").join("shadow"))
        .expect("shadow dir")
        .flatten()
        .collect();
    assert_eq!(shadows.len(), 1);
    let gd = shadows[0].path();

    let git_shadow = |args: &[&str]| -> String {
        let out = std::process::Command::new("git")
            .arg(format!("--git-dir={}", gd.display()))
            .arg(format!("--work-tree={}", dir.display()))
            .args(args)
            .env("GIT_CONFIG_GLOBAL", &sb.git_config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    assert_eq!(
        git_shadow(&["log", "-1", "--format=%s"]).trim(),
        "them-nut-dang-xuat: plan"
    );
    assert_eq!(
        git_shadow(&["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]).trim(),
        ".plan.md"
    );

    let base: String = sb
        .db()
        .query_row(
            "SELECT base_commit FROM tasks ORDER BY rowid DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(base.len(), 40, "base_commit: {base}");
}
