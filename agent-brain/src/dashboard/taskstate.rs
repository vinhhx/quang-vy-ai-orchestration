use crate::db::{RunRow, TaskRow};

#[derive(Clone, Debug, Default)]
pub struct GitFacts {
    pub current_branch: Option<String>,
    pub dirty: Vec<String>,
    pub plan_exists: bool,
    pub plan_blockers: bool,
    pub plan_errors: Vec<String>,
    pub plan_commit: Option<String>,
    pub code_after_plan: bool,
}

pub struct TaskFacts<'a> {
    pub task: &'a TaskRow,
    pub lead: Option<&'a RunRow>,
    pub dev: Option<&'a RunRow>,
    pub qa: Option<&'a RunRow>,
    pub git: &'a GitFacts,
    pub busy: Option<String>,
    pub now_ms: i64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct StepView {
    pub role: String,
    pub state: String,
    pub label: String,
    pub run_id: Option<i64>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub cost_usd: f64,
    pub duration_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct ActionView {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub reason: Option<String>,
    pub run_id: Option<i64>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct TaskView {
    pub steps: Vec<StepView>,
    pub banner: Option<String>,
    pub group: String,
    pub current_step: String,
    pub actions: Vec<ActionView>,
}

pub fn step_state(run: Option<&RunRow>) -> (&'static str, &'static str) {
    let r = match run {
        None => return ("cho", "chờ"),
        Some(r) => r,
    };
    if r.status == "running" {
        return ("dang_chay", "đang chạy");
    }
    match r.outcome.as_deref() {
        Some("dev:blocked") => return ("vuong_mac", "Vướng mắc"),
        Some("qa:fix") => return ("can_sua", "Cần sửa"),
        Some("protocol_violation") => return ("vi_pham", "Vi phạm giao thức"),
        _ => {}
    }
    match r.status.as_str() {
        "killed" => ("killed", "killed"),
        "failed" | "timed_out" | "lost" => ("loi", "lỗi"),
        "succeeded" => ("xong", "xong"),
        _ => ("loi", "lỗi"),
    }
}

fn make_step(role: &str, run: Option<&RunRow>, now_ms: i64) -> StepView {
    let (state, label) = step_state(run);
    let (
        run_id,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
        cost_usd,
        duration_ms,
    ) = match run {
        Some(r) => {
            let dur = if r.status == "running" {
                Some((now_ms - r.started_at).max(0))
            } else if let Some(ended) = r.ended_at {
                Some((ended - r.started_at).max(0))
            } else {
                None
            };
            (
                Some(r.id),
                r.input_tokens,
                r.output_tokens,
                r.cache_read_tokens,
                r.cache_write_tokens,
                r.cost_usd,
                dur,
            )
        }
        None => (None, 0, 0, 0, 0, 0.0, None),
    };

    StepView {
        role: role.to_string(),
        state: state.to_string(),
        label: label.to_string(),
        run_id,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
        cost_usd,
        duration_ms,
    }
}

fn build_run_dev(f: &TaskFacts) -> ActionView {
    let mut enabled = true;
    let mut reason = None;

    if let Some(busy) = &f.busy {
        enabled = false;
        reason = Some(busy.clone());
    } else if f.git.current_branch.as_deref() != Some(&f.task.branch) {
        enabled = false;
        let cur = f.git.current_branch.as_deref().unwrap_or("None");
        reason = Some(format!(
            "Không ở nhánh task '{}' (nhánh hiện tại: '{cur}') — không đoán",
            f.task.branch
        ));
    } else if !f.git.dirty.is_empty() {
        enabled = false;
        let mut sorted = f.git.dirty.clone();
        sorted.sort();
        let list = sorted
            .iter()
            .map(|p| format!("  {p}"))
            .collect::<Vec<_>>()
            .join("\n");
        reason = Some(format!(
            "Working tree có thay đổi chưa commit — hãy commit thay đổi tay với message '{}: plan':\n{list}",
            f.task.slug
        ));
    } else if !f.git.plan_exists {
        enabled = false;
        reason = Some("Không tìm thấy .plan.md ở gốc dự án".to_string());
    } else if !f.git.plan_errors.is_empty() {
        enabled = false;
        let err_list = f
            .git
            .plan_errors
            .iter()
            .map(|e| format!("  - {e}"))
            .collect::<Vec<_>>()
            .join("\n");
        reason = Some(format!("Kế hoạch trong .plan.md không hợp lệ:\n{err_list}"));
    } else if f.git.plan_blockers {
        enabled = false;
        reason =
            Some("Vướng mắc chưa xử lý trong .plan.md — hãy xử lý trước khi chạy dev".to_string());
    }

    ActionView {
        id: "run_dev".to_string(),
        label: "Chạy Dev".to_string(),
        enabled,
        reason,
        run_id: None,
    }
}

fn build_run_qa(f: &TaskFacts) -> ActionView {
    let mut enabled = true;
    let mut reason = None;

    if let Some(busy) = &f.busy {
        enabled = false;
        reason = Some(busy.clone());
    } else if f.git.current_branch.as_deref() != Some(&f.task.branch) {
        enabled = false;
        let cur = f.git.current_branch.as_deref().unwrap_or("None");
        reason = Some(format!(
            "Không ở nhánh task '{}' (nhánh hiện tại: '{cur}') — không đoán",
            f.task.branch
        ));
    } else {
        let mut dirty: Vec<String> = f
            .git
            .dirty
            .iter()
            .filter(|p| *p != ".qa-report.md")
            .cloned()
            .collect();
        if !dirty.is_empty() {
            enabled = false;
            dirty.sort();
            let list = dirty
                .iter()
                .map(|p| format!("  {p}"))
                .collect::<Vec<_>>()
                .join("\n");
            reason = Some(format!("Working tree có thay đổi chưa commit:\n{list}"));
        } else if f.git.plan_commit.is_none() {
            enabled = false;
            reason = Some(format!(
                "Không tìm thấy commit '{}: plan' trên nhánh hiện tại",
                f.task.slug
            ));
        } else if !f.git.code_after_plan {
            enabled = false;
            reason = Some("chưa có thay đổi code sau plan".to_string());
        }
    }

    ActionView {
        id: "run_qa".to_string(),
        label: "Chạy QA".to_string(),
        enabled,
        reason,
        run_id: None,
    }
}

fn build_relead(f: &TaskFacts) -> ActionView {
    let mut enabled = true;
    let mut reason = None;

    if let Some(busy) = &f.busy {
        enabled = false;
        reason = Some(busy.clone());
    } else if f.git.current_branch.as_deref() != Some(&f.task.branch) {
        enabled = false;
        reason = Some(format!(
            "Nhánh '{}' đã tồn tại nhưng chưa được checkout — hãy checkout nhánh đó hoặc dùng cờ --name khác",
            f.task.branch
        ));
    } else {
        let mut dirty: Vec<String> = f
            .git
            .dirty
            .iter()
            .filter(|p| *p != ".plan.md")
            .cloned()
            .collect();
        if !dirty.is_empty() {
            enabled = false;
            dirty.sort();
            let list = dirty
                .iter()
                .map(|p| format!("  {p}"))
                .collect::<Vec<_>>()
                .join("\n");
            reason = Some(format!(
                "Working tree có thay đổi chưa commit — dừng, không tự stash:\n{list}"
            ));
        }
    }

    ActionView {
        id: "relead".to_string(),
        label: "Chạy lại Lead".to_string(),
        enabled,
        reason,
        run_id: None,
    }
}

pub fn derive(f: &TaskFacts) -> TaskView {
    let steps = vec![
        make_step("lead", f.lead, f.now_ms),
        make_step("dev", f.dev, f.now_ms),
        make_step("qa", f.qa, f.now_ms),
    ];

    let banner = match f.task.status.as_str() {
        "planned" => Some("Chờ duyệt plan".to_string()),
        "dev_done" => Some("Dev xong — chờ chạy QA".to_string()),
        "blocked" => Some("Vướng mắc".to_string()),
        "qa_fix" => Some("QA: Cần sửa".to_string()),
        "qa_pass" => Some("QA: Đạt".to_string()),
        _ => None,
    };

    let running_role = if f.lead.map_or(false, |r| r.status == "running") {
        Some("lead")
    } else if f.dev.map_or(false, |r| r.status == "running") {
        Some("dev")
    } else if f.qa.map_or(false, |r| r.status == "running") {
        Some("qa")
    } else {
        None
    };

    let current_step = if let Some(role) = running_role {
        role.to_string()
    } else {
        match f.task.status.as_str() {
            "open" => "lead".to_string(),
            "planned" | "blocked" => "dev".to_string(),
            "dev_done" | "qa_fix" | "qa_pass" => "qa".to_string(),
            _ => "lead".to_string(),
        }
    };

    let all_runs = [f.lead, f.dev, f.qa];
    let latest_run = all_runs
        .into_iter()
        .flatten()
        .max_by_key(|r| (r.started_at, r.id));

    let group = if running_role.is_some() {
        "running".to_string()
    } else if f.task.status == "qa_pass" {
        "done".to_string()
    } else if f.task.status == "blocked" || f.task.status == "qa_fix" {
        "attention".to_string()
    } else if let Some(lr) = latest_run {
        if lr.status == "killed"
            || lr.status == "failed"
            || lr.status == "timed_out"
            || lr.status == "lost"
            || lr.outcome.as_deref() == Some("protocol_violation")
        {
            "attention".to_string()
        } else if f.task.status == "planned" || f.task.status == "dev_done" {
            "waiting".to_string()
        } else {
            "waiting".to_string()
        }
    } else if f.task.status == "planned" || f.task.status == "dev_done" {
        "waiting".to_string()
    } else {
        "waiting".to_string()
    };

    let running_run = all_runs
        .into_iter()
        .flatten()
        .find(|r| r.status == "running");

    let actions = if let Some(r) = running_run {
        vec![ActionView {
            id: "kill".to_string(),
            label: "Kill".to_string(),
            enabled: true,
            reason: None,
            run_id: Some(r.id),
        }]
    } else {
        match f.task.status.as_str() {
            "planned" => vec![build_run_dev(f), build_relead(f)],
            "dev_done" => vec![build_run_qa(f)],
            "blocked" | "qa_fix" => vec![build_relead(f), build_run_dev(f)],
            "open" => {
                let allow_relead = match f.lead {
                    None => true,
                    Some(r) => {
                        r.status == "failed"
                            || r.status == "killed"
                            || r.status == "timed_out"
                            || r.status == "lost"
                            || r.outcome.as_deref() == Some("protocol_violation")
                    }
                };
                if allow_relead {
                    vec![build_relead(f)]
                } else {
                    vec![]
                }
            }
            "qa_pass" => vec![],
            _ => vec![],
        }
    };

    TaskView {
        steps,
        banner,
        group,
        current_step,
        actions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_task(status: &str) -> TaskRow {
        TaskRow {
            id: 1,
            project_id: 10,
            slug: "sua-app".to_string(),
            title: "Sửa app".to_string(),
            branch: "task/sua-app".to_string(),
            base_branch: Some("main".to_string()),
            base_commit: Some("abcdef1".to_string()),
            status: status.to_string(),
            created_at: 1000,
            updated_at: 1000,
        }
    }

    fn dummy_run(id: i64, role: &str, status: &str, outcome: Option<&str>) -> RunRow {
        RunRow {
            id,
            task_id: Some(1),
            project_id: 10,
            role: role.to_string(),
            engine: "claude".to_string(),
            profile: "work".to_string(),
            model: Some("sonnet".to_string()),
            session_id: None,
            pid: Some(1234),
            pgid: Some(1234),
            pid_start: Some(1234),
            status: status.to_string(),
            outcome: outcome.map(|s| s.to_string()),
            exit_code: Some(0),
            kill_requested_at: None,
            started_at: 1000,
            ended_at: Some(2000),
            input_tokens: 100,
            output_tokens: 50,
            cache_read_tokens: 20,
            cache_write_tokens: 10,
            cost_usd: 0.05,
            num_turns: Some(2),
            assistant_turns: Some(2),
            turns_before_first_edit: None,
            reads_outside_plan: None,
            permission_denials: 0,
            usage_source: "result".to_string(),
            base_commit: None,
            run_dir: None,
            error: None,
        }
    }

    #[test]
    fn test_step_state_mapping() {
        assert_eq!(step_state(None), ("cho", "chờ"));

        let mut r = dummy_run(1, "lead", "running", None);
        assert_eq!(step_state(Some(&r)), ("dang_chay", "đang chạy"));

        r.status = "succeeded".to_string();
        r.outcome = None;
        assert_eq!(step_state(Some(&r)), ("xong", "xong"));

        r.outcome = Some("qa:pass".to_string());
        assert_eq!(step_state(Some(&r)), ("xong", "xong"));

        r.outcome = Some("dev:blocked".to_string());
        assert_eq!(step_state(Some(&r)), ("vuong_mac", "Vướng mắc"));

        r.outcome = Some("qa:fix".to_string());
        assert_eq!(step_state(Some(&r)), ("can_sua", "Cần sửa"));

        r.outcome = Some("protocol_violation".to_string());
        assert_eq!(step_state(Some(&r)), ("vi_pham", "Vi phạm giao thức"));

        r.outcome = None;
        r.status = "killed".to_string();
        assert_eq!(step_state(Some(&r)), ("killed", "killed"));

        r.status = "failed".to_string();
        assert_eq!(step_state(Some(&r)), ("loi", "lỗi"));

        r.status = "timed_out".to_string();
        assert_eq!(step_state(Some(&r)), ("loi", "lỗi"));

        r.status = "lost".to_string();
        assert_eq!(step_state(Some(&r)), ("loi", "lỗi"));
    }

    #[test]
    fn test_steps_tokens_and_duration() {
        let task = dummy_task("open");
        let git = GitFacts::default();
        let mut lead_run = dummy_run(1, "lead", "succeeded", None);
        lead_run.started_at = 1000;
        lead_run.ended_at = Some(2500);

        let mut dev_run = dummy_run(2, "dev", "running", None);
        dev_run.started_at = 2500;
        dev_run.ended_at = None;

        let facts = TaskFacts {
            task: &task,
            lead: Some(&lead_run),
            dev: Some(&dev_run),
            qa: None,
            git: &git,
            busy: None,
            now_ms: 3000,
        };

        let view = derive(&facts);
        assert_eq!(view.steps.len(), 3);
        assert_eq!(view.steps[0].role, "lead");
        assert_eq!(view.steps[0].duration_ms, Some(1500));
        assert_eq!(view.steps[0].input_tokens, 100);

        assert_eq!(view.steps[1].role, "dev");
        assert_eq!(view.steps[1].duration_ms, Some(500));
        assert_eq!(view.steps[1].state, "dang_chay");

        assert_eq!(view.steps[2].role, "qa");
        assert_eq!(view.steps[2].duration_ms, None);
        assert_eq!(view.steps[2].state, "cho");
    }

    #[test]
    fn test_banners() {
        let git = GitFacts::default();
        for (status, expected) in [
            ("planned", Some("Chờ duyệt plan")),
            ("dev_done", Some("Dev xong — chờ chạy QA")),
            ("blocked", Some("Vướng mắc")),
            ("qa_fix", Some("QA: Cần sửa")),
            ("qa_pass", Some("QA: Đạt")),
            ("open", None),
        ] {
            let task = dummy_task(status);
            let facts = TaskFacts {
                task: &task,
                lead: None,
                dev: None,
                qa: None,
                git: &git,
                busy: None,
                now_ms: 2000,
            };
            let view = derive(&facts);
            assert_eq!(view.banner.as_deref(), expected);
        }
    }

    #[test]
    fn test_group_and_current_step() {
        let git = GitFacts::default();

        // 1. running run -> group: running, current_step is running role
        let task = dummy_task("planned");
        let dev_running = dummy_run(2, "dev", "running", None);
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: Some(&dev_running),
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };
        let view = derive(&facts);
        assert_eq!(view.group, "running");
        assert_eq!(view.current_step, "dev");

        // 2. qa_pass -> group: done
        let task_pass = dummy_task("qa_pass");
        let facts_pass = TaskFacts {
            task: &task_pass,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };
        assert_eq!(derive(&facts_pass).group, "done");

        // 3. blocked / qa_fix -> group: attention
        let task_blk = dummy_task("blocked");
        let facts_blk = TaskFacts {
            task: &task_blk,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };
        assert_eq!(derive(&facts_blk).group, "attention");

        // 4. latest run failed / killed / timed_out / lost / protocol_violation -> attention
        let task_open = dummy_task("open");
        let lead_violation = dummy_run(1, "lead", "failed", Some("protocol_violation"));
        let facts_viol = TaskFacts {
            task: &task_open,
            lead: Some(&lead_violation),
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };
        assert_eq!(derive(&facts_viol).group, "attention");

        // 5. planned / dev_done -> waiting
        let task_plan = dummy_task("planned");
        let lead_ok = dummy_run(1, "lead", "succeeded", None);
        let facts_plan = TaskFacts {
            task: &task_plan,
            lead: Some(&lead_ok),
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };
        assert_eq!(derive(&facts_plan).group, "waiting");
        assert_eq!(derive(&facts_plan).current_step, "dev");
    }

    #[test]
    fn test_actions_running_is_kill_only() {
        let task = dummy_task("planned");
        let git = GitFacts::default();
        let dev_running = dummy_run(5, "dev", "running", None);
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: Some(&dev_running),
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };
        let view = derive(&facts);
        assert_eq!(
            view.actions,
            vec![ActionView {
                id: "kill".to_string(),
                label: "Kill".to_string(),
                enabled: true,
                reason: None,
                run_id: Some(5),
            }]
        );
    }

    #[test]
    fn test_actions_by_status() {
        let git_ready = GitFacts {
            current_branch: Some("task/sua-app".to_string()),
            dirty: vec![],
            plan_exists: true,
            plan_blockers: false,
            plan_errors: vec![],
            plan_commit: Some("abc".to_string()),
            code_after_plan: true,
        };

        // planned -> [run_dev, relead]
        let task = dummy_task("planned");
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_ready,
            busy: None,
            now_ms: 2000,
        };
        let view = derive(&facts);
        assert_eq!(view.actions.len(), 2);
        assert_eq!(view.actions[0].id, "run_dev");
        assert!(view.actions[0].enabled);
        assert_eq!(view.actions[1].id, "relead");
        assert!(view.actions[1].enabled);

        // dev_done -> [run_qa]
        let task = dummy_task("dev_done");
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_ready,
            busy: None,
            now_ms: 2000,
        };
        let view = derive(&facts);
        assert_eq!(view.actions.len(), 1);
        assert_eq!(view.actions[0].id, "run_qa");
        assert!(view.actions[0].enabled);

        // blocked -> [relead, run_dev]
        let task = dummy_task("blocked");
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_ready,
            busy: None,
            now_ms: 2000,
        };
        let view = derive(&facts);
        assert_eq!(view.actions.len(), 2);
        assert_eq!(view.actions[0].id, "relead");
        assert_eq!(view.actions[1].id, "run_dev");

        // qa_fix -> [relead, run_dev]
        let task = dummy_task("qa_fix");
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_ready,
            busy: None,
            now_ms: 2000,
        };
        let view = derive(&facts);
        assert_eq!(view.actions.len(), 2);
        assert_eq!(view.actions[0].id, "relead");
        assert_eq!(view.actions[1].id, "run_dev");

        // open with no lead -> [relead]
        let task = dummy_task("open");
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_ready,
            busy: None,
            now_ms: 2000,
        };
        let view = derive(&facts);
        assert_eq!(view.actions.len(), 1);
        assert_eq!(view.actions[0].id, "relead");

        // open with failed lead -> [relead]
        let failed_lead = dummy_run(1, "lead", "failed", None);
        let facts = TaskFacts {
            task: &task,
            lead: Some(&failed_lead),
            dev: None,
            qa: None,
            git: &git_ready,
            busy: None,
            now_ms: 2000,
        };
        let view = derive(&facts);
        assert_eq!(view.actions.len(), 1);
        assert_eq!(view.actions[0].id, "relead");

        // qa_pass -> []
        let task = dummy_task("qa_pass");
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_ready,
            busy: None,
            now_ms: 2000,
        };
        assert!(derive(&facts).actions.is_empty());
    }

    #[test]
    fn test_actions_open_after_timed_out_lead_offers_relead() {
        let git_ready = GitFacts {
            current_branch: Some("task/sua-app".to_string()),
            dirty: vec![],
            plan_exists: true,
            plan_blockers: false,
            plan_errors: vec![],
            plan_commit: Some("abc".to_string()),
            code_after_plan: true,
        };

        let task = dummy_task("open");

        // timed_out lead -> exactly one action [relead], enabled
        let timed_out_lead = dummy_run(1, "lead", "timed_out", None);
        let facts_timeout = TaskFacts {
            task: &task,
            lead: Some(&timed_out_lead),
            dev: None,
            qa: None,
            git: &git_ready,
            busy: None,
            now_ms: 2000,
        };
        let view_timeout = derive(&facts_timeout);
        assert_eq!(view_timeout.actions.len(), 1);
        assert_eq!(view_timeout.actions[0].id, "relead");
        assert!(view_timeout.actions[0].enabled);

        // succeeded lead -> no action (empty)
        let succeeded_lead = dummy_run(1, "lead", "succeeded", None);
        let facts_succeeded = TaskFacts {
            task: &task,
            lead: Some(&succeeded_lead),
            dev: None,
            qa: None,
            git: &git_ready,
            busy: None,
            now_ms: 2000,
        };
        let view_succeeded = derive(&facts_succeeded);
        assert!(view_succeeded.actions.is_empty());
    }

    #[test]
    fn test_action_reasons_and_first_match_wins() {
        let task = dummy_task("planned");

        // 1. Busy reason
        let git = GitFacts {
            current_branch: Some("task/sua-app".to_string()),
            dirty: vec![],
            plan_exists: true,
            plan_blockers: false,
            plan_errors: vec![],
            plan_commit: Some("abc".to_string()),
            code_after_plan: true,
        };
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: Some("Dự án đang bận".to_string()),
            now_ms: 2000,
        };
        let view = derive(&facts);
        assert!(!view.actions[0].enabled);
        assert_eq!(view.actions[0].reason.as_deref(), Some("Dự án đang bận"));

        // 2. Wrong branch
        let git_wrong_branch = GitFacts {
            current_branch: Some("main".to_string()),
            ..git.clone()
        };
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_wrong_branch,
            busy: None,
            now_ms: 2000,
        };
        let view = derive(&facts);
        assert_eq!(
            view.actions[0].reason.as_deref(),
            Some("Không ở nhánh task 'task/sua-app' (nhánh hiện tại: 'main') — không đoán")
        );
        assert_eq!(
            view.actions[1].reason.as_deref(),
            Some("Nhánh 'task/sua-app' đã tồn tại nhưng chưa được checkout — hãy checkout nhánh đó hoặc dùng cờ --name khác")
        );

        // 3. Dirty tree for dev
        let git_dirty = GitFacts {
            dirty: vec!["file2.py".to_string(), "file1.py".to_string()],
            ..git.clone()
        };
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_dirty,
            busy: None,
            now_ms: 2000,
        };
        let view = derive(&facts);
        assert_eq!(
            view.actions[0].reason.as_deref(),
            Some("Working tree có thay đổi chưa commit — hãy commit thay đổi tay với message 'sua-app: plan':\n  file1.py\n  file2.py")
        );
        assert_eq!(
            view.actions[1].reason.as_deref(),
            Some("Working tree có thay đổi chưa commit — dừng, không tự stash:\n  file1.py\n  file2.py")
        );

        // 4. Missing plan
        let git_no_plan = GitFacts {
            plan_exists: false,
            ..git.clone()
        };
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_no_plan,
            busy: None,
            now_ms: 2000,
        };
        assert_eq!(
            derive(&facts).actions[0].reason.as_deref(),
            Some("Không tìm thấy .plan.md ở gốc dự án")
        );

        // 5. Plan errors
        let git_plan_err = GitFacts {
            plan_errors: vec!["Lỗi cú pháp".to_string()],
            ..git.clone()
        };
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_plan_err,
            busy: None,
            now_ms: 2000,
        };
        assert_eq!(
            derive(&facts).actions[0].reason.as_deref(),
            Some("Kế hoạch trong .plan.md không hợp lệ:\n  - Lỗi cú pháp")
        );

        // 6. Plan blockers
        let git_blockers = GitFacts {
            plan_blockers: true,
            ..git.clone()
        };
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_blockers,
            busy: None,
            now_ms: 2000,
        };
        assert_eq!(
            derive(&facts).actions[0].reason.as_deref(),
            Some("Vướng mắc chưa xử lý trong .plan.md — hãy xử lý trước khi chạy dev")
        );

        // 7. run_qa checks
        let task_dev_done = dummy_task("dev_done");
        let git_no_commit = GitFacts {
            plan_commit: None,
            ..git.clone()
        };
        let facts = TaskFacts {
            task: &task_dev_done,
            lead: None,
            dev: None,
            qa: None,
            git: &git_no_commit,
            busy: None,
            now_ms: 2000,
        };
        assert_eq!(
            derive(&facts).actions[0].reason.as_deref(),
            Some("Không tìm thấy commit 'sua-app: plan' trên nhánh hiện tại")
        );

        let git_no_code = GitFacts {
            code_after_plan: false,
            ..git.clone()
        };
        let facts = TaskFacts {
            task: &task_dev_done,
            lead: None,
            dev: None,
            qa: None,
            git: &git_no_code,
            busy: None,
            now_ms: 2000,
        };
        assert_eq!(
            derive(&facts).actions[0].reason.as_deref(),
            Some("chưa có thay đổi code sau plan")
        );

        // 8. run_qa ignores .qa-report.md in dirty check
        let git_qa_dirty = GitFacts {
            dirty: vec![".qa-report.md".to_string()],
            ..git.clone()
        };
        let facts = TaskFacts {
            task: &task_dev_done,
            lead: None,
            dev: None,
            qa: None,
            git: &git_qa_dirty,
            busy: None,
            now_ms: 2000,
        };
        assert!(derive(&facts).actions[0].enabled);

        // 9. relead ignores .plan.md in dirty check
        let git_plan_dirty = GitFacts {
            dirty: vec![".plan.md".to_string()],
            ..git.clone()
        };
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_plan_dirty,
            busy: None,
            now_ms: 2000,
        };
        assert!(derive(&facts).actions[1].enabled);
    }
}
