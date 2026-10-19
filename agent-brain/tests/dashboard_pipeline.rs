mod common;
#[allow(dead_code)]
#[path = "support/protocol.rs"]
mod protocol;

use common::{wait_until, Sandbox};
use protocol::{lead_scenario, setup_project};
use serde_json::json;
use std::time::Duration;

fn wait_for_job(srv: &common::Serve, job_id: u64) -> serde_json::Value {
    let mut job_view = None;
    let ok = wait_until(Duration::from_secs(30), || {
        let r = srv.get(&format!("/api/jobs/{job_id}"));
        if r.status == 200 {
            let j = r.json();
            if j["state"] == "exited" {
                job_view = Some(j);
                return true;
            }
        }
        false
    });
    assert!(ok, "job {job_id} did not exit within timeout");
    job_view.unwrap()
}

#[test]
fn pipeline_lead_dev_qa_via_api() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let sc_lead = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_lead).unwrap()).unwrap();

    let srv = sb.serve(&[]);

    // 1. Lead via POST /api/tasks
    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 0);

    // Detail after Lead
    let t_resp = srv.get("/api/tasks/1");
    assert_eq!(t_resp.status, 200);
    let task = t_resp.json();

    assert_eq!(task["steps"][0]["state"], "xong");
    assert_eq!(task["steps"][1]["state"], "cho");
    assert_eq!(task["steps"][2]["state"], "cho");
    assert_eq!(task["banner"], "Chờ duyệt plan");

    let actions = task["actions"].as_array().unwrap();
    let run_dev_act = actions.iter().find(|a| a["id"] == "run_dev").unwrap();
    let relead_act = actions.iter().find(|a| a["id"] == "relead").unwrap();
    assert!(run_dev_act["enabled"].as_bool().unwrap());
    assert!(relead_act["enabled"].as_bool().unwrap());

    let plan = &task["plan"];
    assert_eq!(plan["source"], "worktree");
    let head = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    assert_eq!(plan["commit"], head);
    assert_eq!(plan["changes"][0]["file"], "src/app.py");

    let sections = plan["sections"].as_array().unwrap();
    let sec_names: Vec<&str> = sections
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(sections.len(), 5);
    assert!(sec_names.contains(&"Mục tiêu"));
    assert!(sec_names.contains(&"Thay đổi"));
    assert!(sec_names.contains(&"Không được đụng tới"));
    assert!(sec_names.contains(&"Cách kiểm tra"));
    assert!(sec_names.contains(&"Vướng mắc"));

    // Runs count stays 1 for 3 seconds (no auto-start of Dev)
    std::thread::sleep(Duration::from_secs(3));
    let runs_count: i64 = sb
        .db()
        .query_row("SELECT count(*) FROM runs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(runs_count, 1);

    // 2. Dev via POST /api/tasks/1/dev
    let sc_dev = json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
        ]
    });
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_dev).unwrap()).unwrap();

    let resp_dev = srv.post_json("/api/tasks/1/dev", &json!({}));
    assert_eq!(resp_dev.status, 202);
    let job_id_dev = resp_dev.json()["job_id"].as_u64().unwrap();
    let job_dev = wait_for_job(&srv, job_id_dev);
    assert_eq!(job_dev["exit_code"], 0);

    let t_resp2 = srv.get("/api/tasks/1");
    let task2 = t_resp2.json();
    assert_eq!(task2["status"], "dev_done");
    assert_eq!(task2["steps"][0]["state"], "xong");
    assert_eq!(task2["steps"][1]["state"], "xong");
    assert_eq!(task2["steps"][2]["state"], "cho");

    let actions2 = task2["actions"].as_array().unwrap();
    assert_eq!(actions2.len(), 1);
    assert_eq!(actions2[0]["id"], "run_qa");
    assert!(actions2[0]["enabled"].as_bool().unwrap());

    // 3. QA via POST /api/tasks/1/qa
    let qa_report_content = "\
# QA: sửa app

## Đánh giá
Đã kiểm tra kỹ.

## Kết quả kiểm tra
Test pass.

## Kết luận
Đạt

## Đề xuất
Không có.
";
    let sc_qa = json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "write", "path": ".qa-report.md", "content": qa_report_content}
        ]
    });
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_qa).unwrap()).unwrap();

    let resp_qa = srv.post_json("/api/tasks/1/qa", &json!({}));
    assert_eq!(resp_qa.status, 202);
    let job_id_qa = resp_qa.json()["job_id"].as_u64().unwrap();
    let job_qa = wait_for_job(&srv, job_id_qa);
    assert_eq!(job_qa["exit_code"], 0);

    let t_resp3 = srv.get("/api/tasks/1");
    let task3 = t_resp3.json();
    assert_eq!(task3["status"], "qa_pass");
    assert_eq!(task3["group"], "done");
    assert_eq!(task3["qa"]["verdict"], "Đạt");
    assert!(task3["actions"].as_array().unwrap().is_empty());
}

#[test]
fn pipeline_dev_blocked_shows_vuong_mac() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let sc_lead = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_lead).unwrap()).unwrap();

    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id)["exit_code"], 0);

    // Dev appends a blocker to .plan.md as in cli_dev p1_08
    let sc_dev = json!({
        "ops": [
            {
                "op": "write",
                "path": ".plan.md",
                "content": "\nThiếu thư viện XYZ cần xử lý trước\n",
                "append": true
            }
        ]
    });
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_dev).unwrap()).unwrap();

    let resp_dev = srv.post_json("/api/tasks/1/dev", &json!({}));
    assert_eq!(resp_dev.status, 202);
    let job_id_dev = resp_dev.json()["job_id"].as_u64().unwrap();
    let job_dev = wait_for_job(&srv, job_id_dev);
    assert_eq!(job_dev["exit_code"], 4); // EXIT_BLOCKED = 4

    let t_resp = srv.get("/api/tasks/1");
    let task = t_resp.json();
    assert_eq!(task["status"], "blocked");
    assert_eq!(task["steps"][1]["state"], "vuong_mac");
    assert_eq!(task["plan"]["blockers"], true);

    let vm_sec = task["plan"]["sections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Vướng mắc")
        .unwrap();
    assert!(vm_sec["text"]
        .as_str()
        .unwrap()
        .contains("Thiếu thư viện XYZ"));

    let actions = task["actions"].as_array().unwrap();
    let relead = actions.iter().find(|a| a["id"] == "relead").unwrap();
    let run_dev = actions.iter().find(|a| a["id"] == "run_dev").unwrap();
    assert!(relead["enabled"].as_bool().unwrap());
    assert!(!run_dev["enabled"].as_bool().unwrap());
    assert_eq!(
        run_dev["reason"].as_str().unwrap(),
        "Vướng mắc chưa xử lý trong .plan.md — hãy xử lý trước khi chạy dev"
    );
}

#[test]
fn pipeline_qa_fix_offers_relead_and_dev() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let sc_lead = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_lead).unwrap()).unwrap();

    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id)["exit_code"], 0);

    // Dev ok
    let sc_dev = json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
        ]
    });
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_dev).unwrap()).unwrap();
    let resp_dev = srv.post_json("/api/tasks/1/dev", &json!({}));
    let job_id_dev = resp_dev.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id_dev)["exit_code"], 0);

    // QA report: Cần sửa
    let qa_report_content = "\
# QA: sửa app

## Đánh giá
Chưa đạt chuẩn.

## Kết quả kiểm tra
Lỗi logic hàm app.

## Kết luận
Cần sửa

## Đề xuất
Sửa lại hàm app.
";
    let sc_qa = json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "write", "path": ".qa-report.md", "content": qa_report_content}
        ]
    });
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_qa).unwrap()).unwrap();

    let resp_qa = srv.post_json("/api/tasks/1/qa", &json!({}));
    let job_id_qa = resp_qa.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id_qa)["exit_code"], 0);

    let t_resp = srv.get("/api/tasks/1");
    let task = t_resp.json();
    assert_eq!(task["status"], "qa_fix");
    assert_eq!(task["steps"][2]["state"], "can_sua");
    assert_eq!(task["qa"]["verdict"], "Cần sửa");

    let actions = task["actions"].as_array().unwrap();
    let relead = actions.iter().find(|a| a["id"] == "relead").unwrap();
    let run_dev = actions.iter().find(|a| a["id"] == "run_dev").unwrap();
    assert!(relead["enabled"].as_bool().unwrap());
    assert!(run_dev["enabled"].as_bool().unwrap());
}

#[test]
fn plan_read_from_branch_when_not_checked_out() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let sc_lead = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_lead).unwrap()).unwrap();

    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id)["exit_code"], 0);

    // Switch repo to main
    sb.git(&repo, &["checkout", "main"]);

    let t_resp = srv.get("/api/tasks/1");
    let task = t_resp.json();
    assert_eq!(task["plan"]["source"], "branch");
    assert!(!task["plan"]["sections"].as_array().unwrap().is_empty());

    let actions = task["actions"].as_array().unwrap();
    let run_dev = actions.iter().find(|a| a["id"] == "run_dev").unwrap();
    assert!(!run_dev["enabled"].as_bool().unwrap());
    assert_eq!(
        run_dev["reason"].as_str().unwrap(),
        "Không ở nhánh task 'task/sua-app' (nhánh hiện tại: 'main') — không đoán"
    );

    let runs_before: i64 = sb
        .db()
        .query_row("SELECT count(*) FROM runs", [], |r| r.get(0))
        .unwrap();

    // POST /api/tasks/1/dev -> 409
    let resp_dev = srv.post_json("/api/tasks/1/dev", &json!({}));
    assert_eq!(resp_dev.status, 409);
    assert_eq!(resp_dev.json()["kind"], "not_allowed");

    let runs_after: i64 = sb
        .db()
        .query_row("SELECT count(*) FROM runs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(runs_before, runs_after);
}

#[test]
fn d02_new_plan_commit_is_picked_up() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let sc_lead = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_lead).unwrap()).unwrap();

    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id)["exit_code"], 0);

    // Append a line to .plan.md
    let mut plan_txt = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    plan_txt.push_str("\n<!-- user edit -->\n");
    std::fs::write(repo.join(".plan.md"), &plan_txt).unwrap();

    let t_resp = srv.get("/api/tasks/1");
    let task = t_resp.json();
    assert_eq!(task["plan"]["dirty"], true);
    let actions = task["actions"].as_array().unwrap();
    let run_dev = actions.iter().find(|a| a["id"] == "run_dev").unwrap();
    assert!(!run_dev["enabled"].as_bool().unwrap());
    assert!(run_dev["reason"]
        .as_str()
        .unwrap()
        .contains("sua-app: plan"));

    let commit_count_before = sb
        .git(&repo, &["rev-list", "--count", "HEAD"])
        .trim()
        .parse::<i64>()
        .unwrap();

    // Commit it manually
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", "sua-app: plan"]);

    let new_head = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();

    let t_resp2 = srv.get("/api/tasks/1");
    let task2 = t_resp2.json();
    assert_eq!(task2["plan"]["dirty"], false);
    assert_eq!(task2["plan"]["commit"], new_head);

    let actions2 = task2["actions"].as_array().unwrap();
    let run_dev2 = actions2.iter().find(|a| a["id"] == "run_dev").unwrap();
    assert!(run_dev2["enabled"].as_bool().unwrap());

    let commit_count_after = sb
        .git(&repo, &["rev-list", "--count", "HEAD"])
        .trim()
        .parse::<i64>()
        .unwrap();
    assert_eq!(commit_count_after, commit_count_before + 1);
}

#[test]
fn relead_from_api_replans() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let sc_lead = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_lead).unwrap()).unwrap();

    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id)["exit_code"], 0);

    // Relead with modified plan
    let sc_relead = lead_scenario(&[("src/app.py", "1", "sửa app lần 2", "yêu cầu mới")]);
    std::fs::write(
        &sb.scenario_file,
        serde_json::to_string(&sc_relead).unwrap(),
    )
    .unwrap();

    let resp_relead = srv.post_json("/api/tasks/1/relead", &json!({}));
    assert_eq!(resp_relead.status, 202);
    let job_id_relead = resp_relead.json()["job_id"].as_u64().unwrap();
    let job_relead = wait_for_job(&srv, job_id_relead);
    assert_eq!(job_relead["exit_code"], 0);

    let lead_runs: i64 = sb
        .db()
        .query_row("SELECT count(*) FROM runs WHERE role = 'lead'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(lead_runs, 2);

    let t_resp = srv.get("/api/tasks/1");
    assert_eq!(t_resp.json()["status"], "planned");
}

#[test]
fn tasks_list_profile_filter() {
    let sb = Sandbox::new();

    // 1. Work project
    let repo_work = sb.make_repo("proj-work", &[("a.py", "# work")], None, None);
    let mut entry_work = sb.project_entry(&repo_work, "proj-work", "work");
    entry_work["name_vi"] = json!("Dự án công việc");

    // 2. Personal project
    let repo_personal = sb.make_repo("proj-personal", &[("b.py", "# personal")], None, None);
    let mut entry_personal = sb.project_entry(&repo_personal, "proj-personal", "personal");
    entry_personal["name_vi"] = json!("Dự án cá nhân");

    sb.write_map(json!([entry_work, entry_personal]), &[]);

    // Run lead on both via CLI
    let sc1 = lead_scenario(&[("a.py", "1", "sửa work", "y/c")]);
    let out1 = sb.run(&["run", "lead", "proj-work", "việc 1"], Some(sc1));
    assert_eq!(out1.code, 0);

    let sc2 = lead_scenario(&[("b.py", "1", "sửa personal", "y/c")]);
    let out2 = sb.run(&["run", "lead", "proj-personal", "việc 2"], Some(sc2));
    assert_eq!(out2.code, 0);

    let srv = sb.serve(&[]);

    // Filter work
    let r_work = srv.get("/api/tasks?profile=work");
    assert_eq!(r_work.status, 200);
    let j_work = r_work.json();
    let tasks_work = j_work["tasks"].as_array().unwrap();
    assert_eq!(tasks_work.len(), 1);
    assert_eq!(tasks_work[0]["project"], "proj-work");
    assert!(tasks_work[0]["steps"].as_array().is_some());
    assert!(tasks_work[0]["group"].as_str().is_some());
    assert!(tasks_work[0]["banner"].as_str().is_some());

    // Filter personal
    let r_pers = srv.get("/api/tasks?profile=personal");
    assert_eq!(r_pers.status, 200);
    let j_pers = r_pers.json();
    let tasks_pers = j_pers["tasks"].as_array().unwrap();
    assert_eq!(tasks_pers.len(), 1);
    assert_eq!(tasks_pers[0]["project"], "proj-personal");

    // No param -> both
    let r_all = srv.get("/api/tasks");
    assert_eq!(r_all.status, 200);
    let tasks_all = r_all.json()["tasks"].as_array().unwrap().clone();
    assert_eq!(tasks_all.len(), 2);

    // Bogus profile -> 400
    let r_bogus = srv.get("/api/tasks?profile=bogus");
    assert_eq!(r_bogus.status, 400);
}

#[test]
fn unknown_task_is_404() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");
    let srv = sb.serve(&[]);

    let r_get = srv.get("/api/tasks/999");
    assert_eq!(r_get.status, 404);

    let r_dev = srv.post_json("/api/tasks/999/dev", &json!({}));
    assert_eq!(r_dev.status, 404);
}

#[test]
fn pipe05_route_shape_auth_and_404() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let sc_lead = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_lead).unwrap()).unwrap();

    let srv = sb.serve(&[]);

    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    let job = wait_for_job(&srv, job_id);
    assert_eq!(job["exit_code"], 0);

    // Request without cookie -> 401
    assert_eq!(
        srv.request("GET", "/api/tasks/1/pipeline", &[], b"").status,
        401
    );

    // Unknown task -> 404
    let r_404 = srv.get("/api/tasks/999/pipeline");
    assert_eq!(r_404.status, 404);
    assert_eq!(r_404.json()["error"], "Không tìm thấy task 999");

    // 200 for task 1
    let r_200 = srv.get("/api/tasks/1/pipeline");
    assert_eq!(r_200.status, 200);
    let val = r_200.json();
    let obj = val.as_object().unwrap();

    let mut keys: Vec<String> = obj.keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, vec!["cards", "current", "cycle", "ledger", "task_id"]);

    let cards = obj["cards"].as_object().unwrap();
    let mut card_keys: Vec<String> = cards.keys().cloned().collect();
    card_keys.sort();
    assert_eq!(card_keys, vec!["ban", "code", "plan", "qa", "start"]);

    let ledger = obj["ledger"].as_object().unwrap();
    let mut ledger_keys: Vec<String> = ledger.keys().cloned().collect();
    ledger_keys.sort();
    assert_eq!(ledger_keys, vec!["groups", "total"]);

    assert_eq!(cards["start"]["state"], "xong");
    assert_eq!(cards["start"]["result"], "bạn đã giao");
    let created_at: i64 = sb
        .db()
        .query_row("SELECT created_at FROM tasks WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(cards["start"]["at_ms"], created_at);

    assert_eq!(cards["plan"]["state"], "xong");
    assert_eq!(cards["plan"]["rounds"], 1);
    assert_eq!(cards["plan"]["run_id"], 1);

    assert_eq!(cards["ban"]["state"], "cho_ban");
    assert_eq!(cards["ban"]["result"], "chờ duyệt plan");

    assert_eq!(cards["code"]["state"], "cho");
    assert_eq!(cards["code"]["rounds"], 0);
    assert!(cards["code"]["duration_ms"].is_null());
    assert!(cards["code"]["cost_usd"].is_null());

    assert_eq!(val["current"], "ban");
    assert_eq!(val["cycle"], 1);

    let groups = ledger["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0]["header"], "Chu kỳ 1 · plan đầu tiên");
    assert_eq!(groups[0]["right"], "chờ bạn");

    let rows = groups[0]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["label"], "Vòng 1 · plan");
    assert_eq!(rows[0]["verdict"], "xong");
    let num_turns: i64 = sb
        .db()
        .query_row("SELECT num_turns FROM runs WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows[0]["turns"], num_turns);

    assert_eq!(ledger["total"]["runs"], 1);
}

struct SeedRun {
    role: &'static str,
    status: &'static str,
    outcome: Option<&'static str>,
    started_at: i64,
    ended_at: Option<i64>,
    cost_usd: f64,
    usage_source: &'static str,
    num_turns: Option<i64>,
    base_commit: Option<&'static str>,
}

fn seed_pipeline_task(sb: &Sandbox, branch: &str, runs: &[SeedRun]) -> (i64, Vec<i64>) {
    let repo = setup_project(sb, "work");
    let conn = agent_brain::db::open(&sb.db_path).unwrap();

    let project = agent_brain::projects::Project {
        id: "web-ban-hang".to_string(),
        path: repo.to_string_lossy().to_string(),
        name_vi: "Web bán hàng".to_string(),
        aliases: vec![],
        kind: "code".to_string(),
        profile: "work".to_string(),
        repo: None,
        missing: false,
    };
    agent_brain::db::sync_projects(&conn, &[project]).unwrap();
    let pid = agent_brain::db::project_row_id(&conn, "web-ban-hang").unwrap();
    let task_id = agent_brain::db::upsert_task(
        &conn,
        pid,
        "seed-task",
        "Seed task",
        branch,
        Some("main"),
        None,
    )
    .unwrap();

    let mut run_ids = Vec::new();
    for sr in runs {
        let new_run = agent_brain::db::NewRun {
            task_id: Some(task_id),
            project_id: pid,
            role: sr.role.to_string(),
            profile: "work".to_string(),
            model: Some("sonnet".to_string()),
            pid: 1000,
            pgid: 1000,
            pid_start: 1000,
            base_commit: sr.base_commit.map(|s| s.to_string()),
            run_dir: None,
        };
        let r_id = agent_brain::db::insert_run(&conn, &new_run).unwrap();
        let finish = agent_brain::db::RunFinish {
            status: sr.status.to_string(),
            exit_code: Some(0),
            session_id: None,
            usage: agent_brain::stream::Usage::default(),
            usage_source: sr.usage_source.to_string(),
            cost_usd: sr.cost_usd,
            num_turns: sr.num_turns,
            assistant_turns: sr.num_turns,
            turns_before_first_edit: None,
            reads_outside_plan: None,
            permission_denials: 0,
            error: None,
        };
        agent_brain::db::finish_run(&conn, r_id, &finish).unwrap();
        conn.execute(
            "UPDATE runs SET started_at = ?1, ended_at = ?2, outcome = ?3 WHERE id = ?4",
            rusqlite::params![sr.started_at, sr.ended_at, sr.outcome, r_id],
        )
        .unwrap();
        run_ids.push(r_id);
    }

    (task_id, run_ids)
}

#[test]
fn pipe01_cards_rounds_and_cycles_match_db() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    // 1. Lead
    let sc_lead = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_lead).unwrap()).unwrap();
    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id)["exit_code"], 0);

    // 2. Dev
    let sc_dev = json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
        ]
    });
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_dev).unwrap()).unwrap();
    let resp_dev = srv.post_json("/api/tasks/1/dev", &json!({}));
    assert_eq!(resp_dev.status, 202);
    let job_id_dev = resp_dev.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id_dev)["exit_code"], 0);

    // 3. QA with Cần sửa
    let qa_report_content = "\
# QA: sửa app

## Đánh giá
Chưa đạt chuẩn.

## Kết quả kiểm tra
Lỗi logic hàm app.

## Kết luận
Cần sửa

## Đề xuất
Sửa lại hàm app.
";
    let sc_qa = json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "write", "path": ".qa-report.md", "content": qa_report_content}
        ]
    });
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_qa).unwrap()).unwrap();
    let resp_qa = srv.post_json("/api/tasks/1/qa", &json!({}));
    assert_eq!(resp_qa.status, 202);
    let job_id_qa = resp_qa.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id_qa)["exit_code"], 0);

    // 4. Relead
    let sc_relead = lead_scenario(&[("src/app.py", "1", "sửa app lần 2", "yêu cầu mới")]);
    std::fs::write(
        &sb.scenario_file,
        serde_json::to_string(&sc_relead).unwrap(),
    )
    .unwrap();
    let resp_relead = srv.post_json("/api/tasks/1/relead", &json!({}));
    assert_eq!(resp_relead.status, 202);
    let job_id_relead = resp_relead.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id_relead)["exit_code"], 0);

    // Seed costs for runs 1..=4: 1.25, 2.5, 0.75, 1.0
    let db = sb.db();
    db.execute("UPDATE runs SET cost_usd = 1.25 WHERE id = 1", [])
        .unwrap();
    db.execute("UPDATE runs SET cost_usd = 2.5 WHERE id = 2", [])
        .unwrap();
    db.execute("UPDATE runs SET cost_usd = 0.75 WHERE id = 3", [])
        .unwrap();
    db.execute("UPDATE runs SET cost_usd = 1.0 WHERE id = 4", [])
        .unwrap();

    let pipe_resp = srv.get("/api/tasks/1/pipeline");
    assert_eq!(pipe_resp.status, 200);
    let data = pipe_resp.json();

    assert_eq!(data["cycle"], 2);

    let groups = data["ledger"]["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 4);
    let kinds: Vec<&str> = groups.iter().map(|g| g["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, vec!["plan", "build", "qa", "plan"]);

    let headers: Vec<&str> = groups
        .iter()
        .map(|g| g["header"].as_str().unwrap())
        .collect();
    assert_eq!(
        headers,
        vec![
            "Chu kỳ 1 · plan đầu tiên",
            "Chu kỳ 1 · build",
            "Chu kỳ 1 · QA",
            "Chu kỳ 2 · plan lại",
        ]
    );

    assert_eq!(groups[0]["right"], "bạn đã duyệt");
    assert_eq!(groups[3]["right"], "chờ bạn");

    let lead_runs: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE task_id = 1 AND role = 'lead'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(lead_runs, 2);
    assert_eq!(data["cards"]["plan"]["rounds"], lead_runs);
    assert_eq!(data["cards"]["plan"]["state"], "xong");

    let (dev_started, dev_ended): (i64, i64) = db
        .query_row(
            "SELECT started_at, ended_at FROM runs WHERE id = 2",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(data["cards"]["code"]["state"], "cho");
    assert_eq!(data["cards"]["code"]["rounds"], 1);
    assert!((data["cards"]["code"]["cost_usd"].as_f64().unwrap() - 2.5).abs() < 1e-9);
    assert_eq!(
        data["cards"]["code"]["duration_ms"],
        dev_ended - dev_started
    );

    assert_eq!(data["cards"]["qa"]["state"], "cho");
    assert_eq!(data["cards"]["qa"]["rounds"], 1);

    assert_eq!(data["cards"]["ban"]["state"], "cho_ban");
    assert_eq!(data["current"], "ban");

    let all_rows: Vec<&serde_json::Value> = groups
        .iter()
        .flat_map(|g| g["rows"].as_array().unwrap())
        .collect();
    assert_eq!(all_rows.len(), 4);
    for row in &all_rows {
        let r_id = row["run_id"].as_i64().unwrap();
        let db_turns: i64 = db
            .query_row(
                "SELECT num_turns FROM runs WHERE id = ?1",
                rusqlite::params![r_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(row["turns"], db_turns);
    }

    let total_runs: i64 = db
        .query_row("SELECT COUNT(*) FROM runs WHERE task_id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(data["ledger"]["total"]["runs"], total_runs);
    let total_cost = data["ledger"]["total"]["cost_usd"].as_f64().unwrap();
    assert!((total_cost - 5.5).abs() < 1e-9);
}

#[test]
fn pipe02_ledger_totals_match_db() {
    let sb = Sandbox::new();
    let t0 = 1_700_000_000_000i64;
    let runs = vec![
        SeedRun {
            role: "lead",
            status: "succeeded",
            outcome: None,
            started_at: t0,
            ended_at: Some(t0 + 60_000),
            cost_usd: 1.25,
            usage_source: "result",
            num_turns: Some(10),
            base_commit: None,
        },
        SeedRun {
            role: "dev",
            status: "killed",
            outcome: None,
            started_at: t0 + 120_000,
            ended_at: Some(t0 + 150_000),
            cost_usd: 0.40,
            usage_source: "partial",
            num_turns: Some(5),
            base_commit: None,
        },
        SeedRun {
            role: "dev",
            status: "succeeded",
            outcome: None,
            started_at: t0 + 200_000,
            ended_at: Some(t0 + 500_000),
            cost_usd: 2.00,
            usage_source: "result",
            num_turns: Some(20),
            base_commit: None,
        },
        SeedRun {
            role: "qa",
            status: "succeeded",
            outcome: Some("qa:fix"),
            started_at: t0 + 600_000,
            ended_at: Some(t0 + 660_000),
            cost_usd: 0.50,
            usage_source: "result",
            num_turns: Some(8),
            base_commit: None,
        },
        SeedRun {
            role: "inspect",
            status: "succeeded",
            outcome: None,
            started_at: t0 + 700_000,
            ended_at: Some(t0 + 710_000),
            cost_usd: 0.05,
            usage_source: "result",
            num_turns: Some(2),
            base_commit: None,
        },
    ];

    let (task_id, _run_ids) = seed_pipeline_task(&sb, "task/seed-task", &runs);
    let srv = sb.serve(&[]);
    let pipe_resp = srv.get(&format!("/api/tasks/{task_id}/pipeline"));
    assert_eq!(pipe_resp.status, 200);
    let data = pipe_resp.json();

    let db = sb.db();
    let mut stmt = db
        .prepare(
            "SELECT id FROM runs WHERE task_id = ?1 AND role IN ('lead','dev','qa') ORDER BY started_at, id",
        )
        .unwrap();
    let db_ids: Vec<i64> = stmt
        .query_map(rusqlite::params![task_id], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let groups = data["ledger"]["groups"].as_array().unwrap();
    let row_ids: Vec<i64> = groups
        .iter()
        .flat_map(|g| g["rows"].as_array().unwrap())
        .map(|r| r["run_id"].as_i64().unwrap())
        .collect();
    assert_eq!(row_ids, db_ids);

    // Inspect run is absent
    let inspect_id: i64 = db
        .query_row(
            "SELECT id FROM runs WHERE task_id = ?1 AND role = 'inspect'",
            rusqlite::params![task_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!row_ids.contains(&inspect_id));

    // total.runs == count of lead,dev,qa set (4)
    assert_eq!(data["ledger"]["total"]["runs"], db_ids.len() as i64);
    assert_eq!(data["ledger"]["total"]["runs"], 4);

    let sum_cost: f64 = db
        .query_row(
            "SELECT SUM(cost_usd) FROM runs WHERE task_id = ?1 AND role IN ('lead','dev','qa') AND status <> 'killed' AND usage_source = 'result'",
            rusqlite::params![task_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!((sum_cost - 3.75).abs() < 1e-9);
    let total_cost = data["ledger"]["total"]["cost_usd"].as_f64().unwrap();
    assert!((total_cost - sum_cost).abs() < 1e-9);

    assert_eq!(data["ledger"]["total"]["cost_unknown"], 1);

    // Killed row: cost null and verdict "đã dừng"
    let all_rows: Vec<&serde_json::Value> = groups
        .iter()
        .flat_map(|g| g["rows"].as_array().unwrap())
        .collect();
    let killed_row = all_rows.iter().find(|r| r["state"] == "killed").unwrap();
    assert!(killed_row["cost_usd"].is_null());
    assert_eq!(killed_row["verdict"], "đã dừng");

    // QA row verdict "cần sửa"
    let qa_row = all_rows.iter().find(|r| r["role"] == "qa").unwrap();
    assert_eq!(qa_row["verdict"], "cần sửa");

    let total_dur: i64 = 60_000 + 30_000 + 300_000 + 60_000;
    assert_eq!(data["ledger"]["total"]["duration_ms"], total_dur);
}

#[test]
fn pipe05_refetch_is_idempotent() {
    let sb = Sandbox::new();
    let _repo = setup_project(&sb, "work");

    let sc_lead = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_lead).unwrap()).unwrap();

    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id)["exit_code"], 0);

    let r1 = srv.get("/api/tasks/1/pipeline");
    let r2 = srv.get("/api/tasks/1/pipeline");
    assert_eq!(r1.status, 200);
    assert_eq!(r2.status, 200);
    assert_eq!(r1.text(), r2.text());
}

#[test]
fn pipe03_gate_follows_plan_commits() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    let sc_lead = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_lead).unwrap()).unwrap();

    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id)["exit_code"], 0);

    let head = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    let ct = sb
        .git(&repo, &["show", "-s", "--format=%ct", &head])
        .trim()
        .parse::<i64>()
        .unwrap();

    let pipe_resp1 = srv.get("/api/tasks/1/pipeline");
    assert_eq!(pipe_resp1.status, 200);
    let p1 = pipe_resp1.json();
    assert_eq!(p1["cards"]["ban"]["state"], "cho_ban");
    assert_eq!(p1["cards"]["ban"]["result"], "chờ duyệt plan");
    assert_eq!(p1["cards"]["ban"]["at_ms"], ct * 1000);
    assert_eq!(p1["current"], "ban");

    // Run Dev
    let sc_dev = json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
        ]
    });
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_dev).unwrap()).unwrap();

    let resp_dev = srv.post_json("/api/tasks/1/dev", &json!({}));
    assert_eq!(resp_dev.status, 202);
    let job_id_dev = resp_dev.json()["job_id"].as_u64().unwrap();
    let job_dev = wait_for_job(&srv, job_id_dev);
    assert_eq!(job_dev["exit_code"], 0);

    let dev_started_at: i64 = sb
        .db()
        .query_row(
            "SELECT started_at FROM runs WHERE task_id = 1 AND role = 'dev'",
            [],
            |r| r.get(0),
        )
        .unwrap();

    let pipe_resp2 = srv.get("/api/tasks/1/pipeline");
    assert_eq!(pipe_resp2.status, 200);
    let p2 = pipe_resp2.json();
    assert_eq!(p2["cards"]["ban"]["state"], "xong");
    assert_eq!(p2["cards"]["ban"]["result"], "đã duyệt plan");
    assert_eq!(p2["cards"]["ban"]["at_ms"], dev_started_at);

    // Append a line to .plan.md without committing
    let mut plan_txt = std::fs::read_to_string(repo.join(".plan.md")).unwrap();
    plan_txt.push_str("\n<!-- user edit -->\n");
    std::fs::write(repo.join(".plan.md"), &plan_txt).unwrap();

    let pipe_resp3 = srv.get("/api/tasks/1/pipeline");
    assert_eq!(pipe_resp3.status, 200);
    let p3 = pipe_resp3.json();
    assert_eq!(p3["cards"]["ban"]["state"], "xong");

    // Then git add .plan.md and git commit -m "sua-app: plan"
    sb.git(&repo, &["add", ".plan.md"]);
    sb.git(&repo, &["commit", "-m", "sua-app: plan"]);

    let new_head = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    let new_ct = sb
        .git(&repo, &["show", "-s", "--format=%ct", &new_head])
        .trim()
        .parse::<i64>()
        .unwrap();

    let pipe_resp4 = srv.get("/api/tasks/1/pipeline");
    assert_eq!(pipe_resp4.status, 200);
    let p4 = pipe_resp4.json();
    assert_eq!(p4["cards"]["ban"]["state"], "cho_ban");
    assert_eq!(p4["cards"]["ban"]["result"], "chờ duyệt plan");
    assert_eq!(p4["cards"]["ban"]["at_ms"], new_ct * 1000);
}

#[test]
fn pipe03_unknown_git_waits_for_dev() {
    let sb = Sandbox::new();
    let t0 = 1_700_000_000_000i64;
    let runs = vec![SeedRun {
        role: "lead",
        status: "succeeded",
        outcome: None,
        started_at: t0,
        ended_at: Some(t0 + 60_000),
        cost_usd: 1.0,
        usage_source: "result",
        num_turns: Some(5),
        base_commit: None,
    }];
    let (task_id, _) = seed_pipeline_task(&sb, "task/khong-co-nhanh", &runs);
    let srv = sb.serve(&[]);
    let resp = srv.get(&format!("/api/tasks/{task_id}/pipeline"));
    assert_eq!(resp.status, 200);
    let data = resp.json();
    assert_eq!(data["cards"]["ban"]["state"], "cho_ban");
    assert_eq!(data["cards"]["ban"]["result"], "chờ duyệt plan");
    assert_eq!(data["cards"]["ban"]["at_ms"], t0 + 60_000);
}

#[test]
fn pipe03_unknown_git_approved_by_later_dev() {
    let sb = Sandbox::new();
    let t0 = 1_700_000_000_000i64;
    let runs = vec![
        SeedRun {
            role: "lead",
            status: "succeeded",
            outcome: None,
            started_at: t0,
            ended_at: Some(t0 + 60_000),
            cost_usd: 1.0,
            usage_source: "result",
            num_turns: Some(5),
            base_commit: None,
        },
        SeedRun {
            role: "dev",
            status: "succeeded",
            outcome: None,
            started_at: t0 + 120_000,
            ended_at: Some(t0 + 180_000),
            cost_usd: 1.5,
            usage_source: "result",
            num_turns: Some(10),
            base_commit: None,
        },
    ];
    let (task_id, _) = seed_pipeline_task(&sb, "task/khong-co-nhanh", &runs);
    let srv = sb.serve(&[]);
    let resp = srv.get(&format!("/api/tasks/{task_id}/pipeline"));
    assert_eq!(resp.status, 200);
    let data = resp.json();
    assert_eq!(data["cards"]["ban"]["state"], "xong");
    assert_eq!(data["cards"]["ban"]["result"], "đã duyệt plan");
    assert_eq!(data["cards"]["ban"]["at_ms"], t0 + 120_000);
}

#[test]
fn pipe03_no_plan_commit_on_branch() {
    let sb = Sandbox::new();
    let t0 = 1_700_000_000_000i64;
    let runs = vec![SeedRun {
        role: "lead",
        status: "succeeded",
        outcome: None,
        started_at: t0,
        ended_at: Some(t0 + 60_000),
        cost_usd: 1.0,
        usage_source: "result",
        num_turns: Some(5),
        base_commit: None,
    }];
    let (task_id, _) = seed_pipeline_task(&sb, "main", &runs);
    let srv = sb.serve(&[]);
    let resp = srv.get(&format!("/api/tasks/{task_id}/pipeline"));
    assert_eq!(resp.status, 200);
    let data = resp.json();
    assert_eq!(data["cards"]["ban"]["state"], "cho");
    assert_eq!(data["cards"]["ban"]["result"], "chưa có plan");
    assert_eq!(data["cards"]["plan"]["result"], "xong");
}

#[test]
fn pipe03_gate_after_relead_and_second_dev() {
    let sb = Sandbox::new();
    let repo = setup_project(&sb, "work");

    // 1. Lead
    let sc_lead = lead_scenario(&[("src/app.py", "1", "sửa app", "yêu cầu")]);
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_lead).unwrap()).unwrap();

    let srv = sb.serve(&[]);
    let resp = srv.post_json(
        "/api/tasks",
        &json!({
            "project": "web-ban-hang",
            "request": "sửa app",
            "name": "sua-app"
        }),
    );
    assert_eq!(resp.status, 202);
    let job_id = resp.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id)["exit_code"], 0);

    // 2. Dev
    let sc_dev = json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa app"]}
        ]
    });
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_dev).unwrap()).unwrap();
    let resp_dev = srv.post_json("/api/tasks/1/dev", &json!({}));
    assert_eq!(resp_dev.status, 202);
    let job_id_dev = resp_dev.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id_dev)["exit_code"], 0);

    // 3. QA with Cần sửa
    let qa_report_content = "\
# QA: sửa app

## Đánh giá
Chưa đạt chuẩn.

## Kết quả kiểm tra
Lỗi logic hàm app.

## Kết luận
Cần sửa

## Đề xuất
Sửa lại hàm app.
";
    let sc_qa = json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "write", "path": ".qa-report.md", "content": qa_report_content}
        ]
    });
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_qa).unwrap()).unwrap();
    let resp_qa = srv.post_json("/api/tasks/1/qa", &json!({}));
    assert_eq!(resp_qa.status, 202);
    let job_id_qa = resp_qa.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id_qa)["exit_code"], 0);

    // 4. Re-Lead
    let sc_relead = lead_scenario(&[("src/app.py", "1", "sửa app lần 2", "yêu cầu mới")]);
    std::fs::write(
        &sb.scenario_file,
        serde_json::to_string(&sc_relead).unwrap(),
    )
    .unwrap();
    let resp_relead = srv.post_json("/api/tasks/1/relead", &json!({}));
    assert_eq!(resp_relead.status, 202);
    let job_id_relead = resp_relead.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id_relead)["exit_code"], 0);

    let head_relead = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    let ct_relead = sb
        .git(&repo, &["show", "-s", "--format=%ct", &head_relead])
        .trim()
        .parse::<i64>()
        .unwrap();

    let pipe_relead = srv.get("/api/tasks/1/pipeline").json();
    assert_eq!(pipe_relead["cycle"], 2);
    assert_eq!(pipe_relead["cards"]["ban"]["state"], "cho_ban");
    assert_eq!(pipe_relead["cards"]["ban"]["result"], "chờ duyệt plan");
    assert_eq!(pipe_relead["cards"]["ban"]["at_ms"], ct_relead * 1000);

    // 5. Second Dev
    let sc_dev2 = json!({
        "ops": [
            {"op": "read", "path": ".plan.md"},
            {"op": "read", "path": "src/app.py"},
            {"op": "write", "path": "src/app.py", "content": "def app():\n    return 3\n"},
            {"op": "git", "args": ["add", "src/app.py"]},
            {"op": "git", "args": ["commit", "-m", "sua-app: sửa app lần 2"]}
        ]
    });
    std::fs::write(&sb.scenario_file, serde_json::to_string(&sc_dev2).unwrap()).unwrap();
    let resp_dev2 = srv.post_json("/api/tasks/1/dev", &json!({}));
    assert_eq!(
        resp_dev2.status,
        202,
        "POST /api/tasks/1/dev returned {}: {}",
        resp_dev2.status,
        resp_dev2
            .json()
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    );
    let job_id_dev2 = resp_dev2.json()["job_id"].as_u64().unwrap();
    assert_eq!(wait_for_job(&srv, job_id_dev2)["exit_code"], 0);

    let second_dev_started_at: i64 = sb
        .db()
        .query_row(
            "SELECT started_at FROM runs WHERE task_id = 1 AND role = 'dev' ORDER BY started_at DESC, id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();

    let pipe_dev2 = srv.get("/api/tasks/1/pipeline").json();
    assert_eq!(pipe_dev2["cards"]["ban"]["state"], "xong");
    assert_eq!(pipe_dev2["cards"]["ban"]["result"], "đã duyệt plan");
    assert_eq!(pipe_dev2["cards"]["ban"]["at_ms"], second_dev_started_at);
    assert_eq!(pipe_dev2["cards"]["code"]["state"], "xong");
    assert_eq!(pipe_dev2["cards"]["code"]["rounds"], 2);
}

#[test]
fn pipe03_is_ancestor_cases() {
    let sb = Sandbox::new();
    let repo = sb.make_repo("anc", &[("a.txt", "1\n")], None, None);
    let c1 = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();

    std::fs::write(repo.join("a.txt"), "2\n").unwrap();
    sb.git(&repo, &["add", "a.txt"]);
    sb.git(&repo, &["commit", "-m", "second"]);
    let c2 = sb.git(&repo, &["rev-parse", "HEAD"]).trim().to_string();

    assert!(agent_brain::gitops::is_ancestor(&repo, &c1, &c2));
    assert!(agent_brain::gitops::is_ancestor(&repo, &c1, &c1));
    assert!(!agent_brain::gitops::is_ancestor(&repo, &c2, &c1));
    assert!(!agent_brain::gitops::is_ancestor(&repo, "zz", &c1));
    assert!(!agent_brain::gitops::is_ancestor(&repo, "--all", &c2));
    assert!(!agent_brain::gitops::is_ancestor(&repo, "", &c2));

    let ct1 = sb
        .git(&repo, &["show", "-s", "--format=%ct", &c1])
        .trim()
        .parse::<i64>()
        .unwrap();
    assert_eq!(
        agent_brain::gitops::commit_time_ms(&repo, &c1),
        Some(ct1 * 1000)
    );
    assert_eq!(agent_brain::gitops::commit_time_ms(&repo, "-x"), None);
}
