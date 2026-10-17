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
