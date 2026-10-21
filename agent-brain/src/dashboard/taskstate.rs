use crate::autopilot::decide::{AutoJson, GateState, StopReason};
use crate::db::{RunRow, TaskAuto, TaskRow};
use crate::workflows::Workflow;

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

#[derive(Default)]
pub struct AutoFacts<'a> {
    pub workflow: Option<&'a Workflow>,
    pub auto: Option<&'a TaskAuto>,
    pub auto_json: Option<&'a AutoJson>,
    pub driver_alive: bool,
    pub running: Option<&'a RunRow>,
    pub gate: Option<GateState>,
    pub plan_dirty: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct AutoView {
    pub state: String,
    pub legacy: bool,
    pub workflow_auto: bool,
    pub stop_reason: Option<String>,
    pub stop_word: Option<String>,
    pub stop_detail: Option<String>,
    pub stopped_in: Option<String>,
    pub stop_requested: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct TaskView {
    pub steps: Vec<StepView>,
    pub banner: Option<String>,
    pub group: String,
    pub current_step: String,
    pub actions: Vec<ActionView>,
    pub controls: Vec<ActionView>,
    pub auto: AutoView,
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
        Some("review:approve") => return ("duyet", "duyệt"),
        Some("review:revise") => return ("sua_lai", "sửa lại"),
        Some("review:pass") => return ("dat", "đạt"),
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

/// Xác định trạng thái autopilot cho AutoView.
fn auto_state_of(a: &AutoFacts) -> &'static str {
    if a.workflow.is_none() {
        return "off";
    }
    match a.auto.and_then(|auto| auto.auto_state.as_deref()) {
        Some("running") => {
            if a.driver_alive {
                if a.auto.map_or(false, |auto| auto.auto_stop_at.is_some()) {
                    "stopping"
                } else {
                    "running"
                }
            } else {
                "lost"
            }
        }
        Some("interrupted") => "lost",
        Some("gate") => {
            if a.gate == Some(GateState::Approved) {
                "off"
            } else {
                "gate"
            }
        }
        Some("stopped") => "stopped",
        Some("done") => "done",
        _ => "off",
    }
}

/// Autopilot còn bật cho task: quy trình `auto: true` và bạn chưa tắt autopilot
/// (auto_state khác `ended` — Dừng autopilot ở cổng hoặc Dừng hẳn). Quyết định
/// Duyệt plan có chạy tiếp autopilot không, Trả về Lead / Chạy lại Lead chạy
/// `auto N --replan` hay Lead tay (D-14, D-15 sửa đổi, UI-SPEC B3).
pub fn autopilot_continues(workflow: Option<&Workflow>, auto: Option<&TaskAuto>) -> bool {
    workflow.map_or(false, |w| w.auto)
        && auto.and_then(|a| a.auto_state.as_deref()) != Some("ended")
}

/// Dựng nút điều khiển Duyệt plan (gate Waiting).
fn build_approve(f: &TaskFacts, a: &AutoFacts, running_run: Option<&RunRow>) -> ActionView {
    let mut enabled = true;
    let mut reason = None;
    // Duyệt plan chạy tiếp autopilot: cần dự án rảnh và đúng nhánh task (CR-01).
    // Duyệt chỉ ghi nhận (quy trình tay, autopilot đã tắt) thì không cần.
    let continues = autopilot_continues(a.workflow, a.auto);

    if running_run.is_some() {
        enabled = false;
        reason = Some("Đang có run chạy — chờ run xong hoặc bấm Kill".to_string());
    } else if a.driver_alive {
        enabled = false;
        reason = Some("Autopilot đang chạy task này — chờ autopilot dừng".to_string());
    } else if a.plan_dirty {
        enabled = false;
        reason = Some(format!(
            "Plan có thay đổi chưa commit — commit với message '{}: plan' rồi duyệt",
            f.task.slug
        ));
    } else if continues && f.busy.is_some() {
        enabled = false;
        reason = f.busy.clone();
    } else if continues && f.git.current_branch.as_deref() != Some(&f.task.branch) {
        enabled = false;
        let cur = f.git.current_branch.as_deref().unwrap_or("None");
        reason = Some(format!(
            "Không ở nhánh task '{}' (nhánh hiện tại: '{cur}') — không đoán",
            f.task.branch
        ));
    }

    ActionView {
        id: "approve".to_string(),
        label: "Duyệt plan".to_string(),
        enabled,
        reason,
        run_id: None,
    }
}

/// Dựng nút điều khiển Trả về Lead (gate Waiting).
fn build_return_lead(f: &TaskFacts, a: &AutoFacts, running_run: Option<&RunRow>) -> ActionView {
    let mut enabled = true;
    let mut reason = None;

    if running_run.is_some() {
        enabled = false;
        reason = Some("Đang có run chạy — chờ run xong hoặc bấm Kill".to_string());
    } else if a.driver_alive {
        enabled = false;
        reason = Some("Autopilot đang chạy task này — chờ autopilot dừng".to_string());
    } else if let Some(busy) = &f.busy {
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
        id: "return_lead".to_string(),
        label: "Trả về Lead".to_string(),
        enabled,
        reason,
        run_id: None,
    }
}

/// Dựng nút điều khiển Bật autopilot.
fn build_auto_start(f: &TaskFacts, a: &AutoFacts, running_run: Option<&RunRow>) -> ActionView {
    let mut enabled = true;
    let mut reason = None;

    if a.workflow.is_none() {
        enabled = false;
        reason =
            Some("Task tạo trước Phase 5.2 — không có quy trình để chạy autopilot.".to_string());
    } else if running_run.is_some() {
        enabled = false;
        reason = Some("Đang có run chạy — chờ run xong hoặc bấm Kill".to_string());
    } else if f.task.status == "qa_pass" {
        enabled = false;
        reason = Some("Task đã xong (QA đạt).".to_string());
    } else if a.gate == Some(GateState::Waiting)
        && a.auto.and_then(|auto| auto.auto_state.as_deref()) != Some("ended")
    {
        // Cổng chờ duyệt: Duyệt plan chạy tiếp autopilot. Khi bạn đã tắt
        // autopilot (`ended`), Bật autopilot bật lại và dừng lại ở cổng (D-14).
        enabled = false;
        reason =
            Some("Autopilot dừng ở bước duyệt plan — bấm Duyệt plan để chạy tiếp.".to_string());
    } else if let Some(busy) = &f.busy {
        enabled = false;
        reason = Some(busy.clone());
    } else if f.git.current_branch.as_deref() != Some(&f.task.branch) {
        enabled = false;
        let cur = f.git.current_branch.as_deref().unwrap_or("None");
        reason = Some(format!(
            "Không ở nhánh task '{}' (nhánh hiện tại: '{cur}') — không đoán",
            f.task.branch
        ));
    }

    ActionView {
        id: "auto_start".to_string(),
        label: "Bật autopilot".to_string(),
        enabled,
        reason,
        run_id: None,
    }
}

/// Dựng nút điều khiển Dừng autopilot.
fn build_auto_stop(stop_requested: bool) -> ActionView {
    let mut enabled = true;
    let mut reason = None;
    if stop_requested {
        enabled = false;
        reason = Some("Đang chờ run hiện tại chạy xong.".to_string());
    }
    ActionView {
        id: "auto_stop".to_string(),
        label: "Dừng autopilot".to_string(),
        enabled,
        reason,
        run_id: None,
    }
}

/// Dựng nút điều khiển Chạy thêm 1 vòng.
fn build_auto_more_round(f: &TaskFacts, a: &AutoFacts, running_run: Option<&RunRow>) -> ActionView {
    let mut enabled = true;
    let mut reason = None;

    let is_vuong_mac = a
        .auto_json
        .and_then(|aj| aj.stop.as_ref())
        .map_or(false, |s| s.reason == StopReason::VuongMac);

    if is_vuong_mac && f.git.plan_blockers {
        enabled = false;
        reason =
            Some("Vướng mắc chưa xử lý trong .plan.md — hãy xử lý trước khi chạy dev".to_string());
    } else if running_run.is_some() {
        enabled = false;
        reason = Some("Đang có run chạy — chờ run xong hoặc bấm Kill".to_string());
    } else if let Some(busy) = &f.busy {
        enabled = false;
        reason = Some(busy.clone());
    } else if f.git.current_branch.as_deref() != Some(&f.task.branch) {
        enabled = false;
        let cur = f.git.current_branch.as_deref().unwrap_or("None");
        reason = Some(format!(
            "Không ở nhánh task '{}' (nhánh hiện tại: '{cur}') — không đoán",
            f.task.branch
        ));
    }

    ActionView {
        id: "auto_more_round".to_string(),
        label: "Chạy thêm 1 vòng".to_string(),
        enabled,
        reason,
        run_id: None,
    }
}

/// Dựng nút điều khiển Dừng hẳn.
fn build_auto_end() -> ActionView {
    ActionView {
        id: "auto_end".to_string(),
        label: "Dừng hẳn".to_string(),
        enabled: true,
        reason: None,
        run_id: None,
    }
}

/// Xác định bước hiện tại của task (05.1 IN-05).
pub fn current_step_of(f: &TaskFacts, running: Option<&RunRow>) -> String {
    if let Some(r) = running {
        match r.role.as_str() {
            "lead" | "plan-review" => "lead".to_string(),
            "dev" | "code-review" => "dev".to_string(),
            "qa" => "qa".to_string(),
            _ => match f.task.status.as_str() {
                "open" => "lead".to_string(),
                "planned" | "blocked" => "dev".to_string(),
                "dev_done" | "qa_fix" | "qa_pass" => "qa".to_string(),
                _ => "lead".to_string(),
            },
        }
    } else {
        match f.task.status.as_str() {
            "open" => "lead".to_string(),
            "planned" | "blocked" => "dev".to_string(),
            "dev_done" | "qa_fix" | "qa_pass" => "qa".to_string(),
            _ => "lead".to_string(),
        }
    }
}

pub fn derive(f: &TaskFacts) -> TaskView {
    derive_with(f, &AutoFacts::default())
}

pub fn derive_with(f: &TaskFacts, a: &AutoFacts) -> TaskView {
    let steps = vec![
        make_step("lead", f.lead, f.now_ms),
        make_step("dev", f.dev, f.now_ms),
        make_step("qa", f.qa, f.now_ms),
    ];

    let all_runs = [f.lead, f.dev, f.qa];
    let latest_run = all_runs
        .into_iter()
        .flatten()
        .max_by_key(|r| (r.started_at, r.id));

    let running_run = a.running.filter(|r| r.status == "running").or_else(|| {
        all_runs
            .into_iter()
            .flatten()
            .find(|r| r.status == "running")
    });

    let current_step = current_step_of(f, running_run);

    let auto_state = auto_state_of(a);
    let legacy = a.workflow.is_none();
    let workflow_auto = a.workflow.map_or(false, |w| w.auto);
    let stop_requested =
        !legacy && a.auto.map_or(false, |auto| auto.auto_stop_at.is_some()) && a.driver_alive;

    let (stop_reason, stop_word, stop_detail, stopped_in) = if auto_state == "stopped" {
        if let Some(stop) = a.auto_json.and_then(|aj| aj.stop.as_ref()) {
            (
                Some(stop.reason.as_str().to_string()),
                Some(crate::autopilot::view::reason_word(stop.reason).to_string()),
                Some(crate::autopilot::view::stop_detail(stop, &f.task.slug)),
                stop.loop_name.map(|l| l.as_str().to_string()),
            )
        } else {
            (None, None, None, None)
        }
    } else {
        (None, None, None, None)
    };

    let banner = if auto_state == "stopped" {
        if let Some(word) = &stop_word {
            Some(format!("Chờ bạn · {word}"))
        } else {
            Some("Chờ bạn".to_string())
        }
    } else if auto_state == "gate" {
        Some("Chờ duyệt plan".to_string())
    } else {
        match f.task.status.as_str() {
            "planned" => Some("Chờ duyệt plan".to_string()),
            "dev_done" => Some("Dev xong — chờ chạy QA".to_string()),
            "blocked" => Some("Vướng mắc".to_string()),
            "qa_fix" => Some("QA: Cần sửa".to_string()),
            "qa_pass" => Some("QA: Đạt".to_string()),
            _ => None,
        }
    };

    let group = if running_run.is_some() || a.driver_alive {
        "running".to_string()
    } else if auto_state == "stopped" {
        "waiting".to_string()
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

    let actions = if let Some(r) = running_run {
        vec![ActionView {
            id: "kill".to_string(),
            label: "Kill".to_string(),
            enabled: true,
            reason: None,
            run_id: Some(r.id),
        }]
    } else {
        let mut acts = match f.task.status.as_str() {
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
        };
        if a.driver_alive {
            for act in &mut acts {
                if act.id == "run_dev" || act.id == "run_qa" || act.id == "relead" {
                    act.enabled = false;
                    act.reason = Some(
                        "Autopilot đang chạy task này — bấm Dừng autopilot để làm tay".to_string(),
                    );
                }
            }
        }
        acts
    };

    let auto = AutoView {
        state: auto_state.to_string(),
        legacy,
        workflow_auto,
        stop_reason,
        stop_word,
        stop_detail,
        stopped_in,
        stop_requested,
    };

    let mut controls = Vec::new();
    if legacy {
        controls.push(build_auto_start(f, a, running_run));
    } else {
        if a.gate == Some(GateState::Waiting) {
            controls.push(build_approve(f, a, running_run));
            controls.push(build_return_lead(f, a, running_run));
        }
        if auto_state == "gate" {
            // Autopilot đang chờ ở cổng duyệt plan: Dừng autopilot tắt autopilot
            // cho task (không bỏ qua cổng — D-03); Bật autopilot bật lại.
            controls.push(build_auto_stop(false));
        } else if auto_state != "running" && auto_state != "stopping" {
            controls.push(build_auto_start(f, a, running_run));
        }
        if auto_state == "running" || auto_state == "stopping" {
            controls.push(build_auto_stop(stop_requested));
        }
        if auto_state == "stopped" {
            controls.push(build_auto_more_round(f, a, running_run));
            controls.push(build_auto_end());
        }
    }

    TaskView {
        steps,
        banner,
        group,
        current_step,
        actions,
        controls,
        auto,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autopilot::decide::{Loop, StopInfo};

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
            stage: None,
            round: None,
            metrics_json: None,
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

    fn dummy_task_auto(auto_state: Option<&str>, auto_stop_at: Option<i64>) -> TaskAuto {
        TaskAuto {
            workflow_json: None,
            approved_at: None,
            approved_commit: None,
            auto_state: auto_state.map(|s| s.to_string()),
            auto_pid: None,
            auto_pid_start: None,
            auto_stop_at,
            auto_stop_kind: None,
            auto_json: None,
        }
    }

    #[test]
    fn test_review_words() {
        let oc_approve = "review:approve";
        let r1 = dummy_run(1, "plan-review", "succeeded", Some(oc_approve));
        assert_eq!(step_state(Some(&r1)), ("duyet", "duyệt"));

        let oc_revise = "review:revise";
        let r2 = dummy_run(2, "code-review", "succeeded", Some(oc_revise));
        assert_eq!(step_state(Some(&r2)), ("sua_lai", "sửa lại"));

        let oc_pass = "review:pass";
        let r3 = dummy_run(3, "code-review", "succeeded", Some(oc_pass));
        assert_eq!(step_state(Some(&r3)), ("dat", "đạt"));

        let r4 = dummy_run(4, "plan-review", "running", None);
        assert_eq!(step_state(Some(&r4)), ("dang_chay", "đang chạy"));
    }

    #[test]
    fn test_legacy_task_defaults() {
        let task = dummy_task("planned");
        let git = GitFacts::default();
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
        assert_eq!(view.auto.state, "off");
        assert!(view.auto.legacy);
        assert!(!view.auto.workflow_auto);
        assert!(!view.auto.stop_requested);
        assert_eq!(view.controls.len(), 1);
        assert_eq!(view.controls[0].id, "auto_start");
        assert!(!view.controls[0].enabled);
        assert_eq!(
            view.controls[0].reason.as_deref(),
            Some("Task tạo trước Phase 5.2 — không có quy trình để chạy autopilot.")
        );
        // actions equal to what 05.1 returned
        assert_eq!(view.actions.len(), 2);
        assert_eq!(view.actions[0].id, "run_dev");
        assert_eq!(view.actions[1].id, "relead");
    }

    #[test]
    fn test_gate_waiting_controls() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();

        let task = dummy_task("planned");
        let git = GitFacts {
            current_branch: Some("task/sua-app".to_string()),
            dirty: vec![],
            plan_exists: true,
            plan_blockers: false,
            plan_errors: vec![],
            plan_commit: Some("abc".to_string()),
            code_after_plan: false,
        };
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };
        let auto_facts = AutoFacts {
            workflow: Some(mac_dinh),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };

        let view = derive_with(&facts, &auto_facts);
        let control_ids: Vec<&str> = view.controls.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(control_ids, vec!["approve", "return_lead", "auto_start"]);

        // approve and return_lead enabled
        assert!(view.controls[0].enabled);
        assert_eq!(view.controls[0].id, "approve");
        assert!(view.controls[1].enabled);
        assert_eq!(view.controls[1].id, "return_lead");

        // auto_start disabled with gate waiting text
        assert!(!view.controls[2].enabled);
        assert_eq!(view.controls[2].id, "auto_start");
        assert_eq!(
            view.controls[2].reason.as_deref(),
            Some("Autopilot dừng ở bước duyệt plan — bấm Duyệt plan để chạy tiếp.")
        );
    }

    fn gate_git(branch: &str) -> GitFacts {
        GitFacts {
            current_branch: Some(branch.to_string()),
            dirty: vec![],
            plan_exists: true,
            plan_blockers: false,
            plan_errors: vec![],
            plan_commit: Some("abc".to_string()),
            code_after_plan: false,
        }
    }

    fn ids_of(list: &[ActionView]) -> Vec<&str> {
        list.iter().map(|c| c.id.as_str()).collect()
    }

    #[test]
    fn armed_gate_offers_stop() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();

        let task = dummy_task("planned");
        let git = gate_git("task/sua-app");
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };
        let auto = dummy_task_auto(Some("gate"), None);
        let af = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };
        let view = derive_with(&facts, &af);
        assert_eq!(view.auto.state, "gate");
        assert_eq!(
            ids_of(&view.controls),
            vec!["approve", "return_lead", "auto_stop"]
        );
        let stop = view.controls.iter().find(|c| c.id == "auto_stop").unwrap();
        assert!(stop.enabled);
        assert_eq!(stop.label, "Dừng autopilot");
        assert_eq!(stop.reason, None);
        assert!(!view.controls.iter().any(|c| c.id == "auto_start"));
    }

    #[test]
    fn ended_gate_offers_start() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();

        let task = dummy_task("planned");
        let git = gate_git("task/sua-app");
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };

        // 1. autopilot turned off at the gate (`ended`): Bật autopilot enabled.
        let ended = dummy_task_auto(Some("ended"), None);
        let af = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&ended),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };
        let view = derive_with(&facts, &af);
        assert_eq!(view.auto.state, "off");
        assert_eq!(
            ids_of(&view.controls),
            vec!["approve", "return_lead", "auto_start"]
        );
        let start = view.controls.iter().find(|c| c.id == "auto_start").unwrap();
        assert!(start.enabled, "{start:?}");
        assert_eq!(start.reason, None);

        // 2. ended but busy: the busy reason still applies.
        let facts_busy = TaskFacts {
            busy: Some("Dự án x bận".to_string()),
            ..facts
        };
        let v_busy = derive_with(&facts_busy, &af);
        let start_busy = v_busy
            .controls
            .iter()
            .find(|c| c.id == "auto_start")
            .unwrap();
        assert!(!start_busy.enabled);
        assert_eq!(start_busy.reason.as_deref(), Some("Dự án x bận"));

        // 3. auto_state NULL on mac-dinh: approval starts the autopilot there.
        let facts = TaskFacts {
            busy: None,
            ..facts_busy
        };
        let null = dummy_task_auto(None, None);
        let af_null = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&null),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };
        let v_null = derive_with(&facts, &af_null);
        let start_null = v_null
            .controls
            .iter()
            .find(|c| c.id == "auto_start")
            .unwrap();
        assert!(!start_null.enabled);
        assert_eq!(
            start_null.reason.as_deref(),
            Some("Autopilot dừng ở bước duyệt plan — bấm Duyệt plan để chạy tiếp.")
        );
    }

    #[test]
    fn autopilot_continues_rule() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();
        let nhanh = wfs.get("nhanh").unwrap();

        let states = [None, Some("gate"), Some("stopped"), Some("ended")];
        for st in states {
            let a = dummy_task_auto(st, None);
            assert_eq!(
                autopilot_continues(Some(mac_dinh), Some(&a)),
                st != Some("ended"),
                "mac-dinh, auto_state {st:?}"
            );
            assert!(
                !autopilot_continues(Some(nhanh), Some(&a)),
                "nhanh, auto_state {st:?}"
            );
            assert!(
                !autopilot_continues(None, Some(&a)),
                "no workflow, auto_state {st:?}"
            );
        }
        assert!(autopilot_continues(Some(mac_dinh), None));
        assert!(!autopilot_continues(Some(nhanh), None));
        assert!(!autopilot_continues(None, None));
    }

    #[test]
    fn approve_branch_rule() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();
        let nhanh = wfs.get("nhanh").unwrap();

        let task = dummy_task("planned");
        let git = gate_git("main");
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };
        let approve_of = |view: &TaskView| {
            view.controls
                .iter()
                .find(|c| c.id == "approve")
                .cloned()
                .unwrap()
        };

        // 1. mac-dinh, autopilot armed at the gate, on main: approval would
        // start the driver on another branch -> disabled (CR-01).
        let gate = dummy_task_auto(Some("gate"), None);
        let af = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&gate),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };
        let ap = approve_of(&derive_with(&facts, &af));
        assert!(!ap.enabled);
        assert_eq!(
            ap.reason.as_deref(),
            Some("Không ở nhánh task 'task/sua-app' (nhánh hiện tại: 'main') — không đoán")
        );

        // 2. autopilot turned off (`ended`): approval only records -> enabled.
        let ended = dummy_task_auto(Some("ended"), None);
        let af_ended = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&ended),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };
        let ap = approve_of(&derive_with(&facts, &af_ended));
        assert!(ap.enabled, "{ap:?}");

        // 3. nhanh (manual) on main -> enabled.
        let af_nhanh = AutoFacts {
            workflow: Some(nhanh),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };
        let ap = approve_of(&derive_with(&facts, &af_nhanh));
        assert!(ap.enabled, "{ap:?}");

        // 4. mac-dinh on the task branch -> enabled.
        let git_ok = gate_git("task/sua-app");
        let facts_ok = TaskFacts {
            git: &git_ok,
            ..facts
        };
        let ap = approve_of(&derive_with(&facts_ok, &af));
        assert!(ap.enabled, "{ap:?}");
    }

    #[test]
    fn test_approve_reasons() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();
        let nhanh = wfs.get("nhanh").unwrap();

        let task = dummy_task("planned");
        let git = GitFacts {
            current_branch: Some("task/sua-app".to_string()),
            dirty: vec![],
            plan_exists: true,
            plan_blockers: false,
            plan_errors: vec![],
            plan_commit: Some("abc".to_string()),
            code_after_plan: false,
        };

        // 1. plan_dirty -> approve disabled
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };
        let auto_facts_dirty = AutoFacts {
            workflow: Some(mac_dinh),
            gate: Some(GateState::Waiting),
            plan_dirty: true,
            ..Default::default()
        };
        let view_dirty = derive_with(&facts, &auto_facts_dirty);
        assert_eq!(view_dirty.controls[0].id, "approve");
        assert!(!view_dirty.controls[0].enabled);
        assert_eq!(
            view_dirty.controls[0].reason.as_deref(),
            Some("Plan có thay đổi chưa commit — commit với message 'sua-app: plan' rồi duyệt")
        );

        // 2. busy Some("Dự án x bận") with mac-dinh (auto true) -> approve disabled
        let facts_busy = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: Some("Dự án x bận".to_string()),
            now_ms: 2000,
        };
        let auto_facts_macdinh = AutoFacts {
            workflow: Some(mac_dinh),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };
        let view_busy_macdinh = derive_with(&facts_busy, &auto_facts_macdinh);
        assert_eq!(view_busy_macdinh.controls[0].id, "approve");
        assert!(!view_busy_macdinh.controls[0].enabled);
        assert_eq!(
            view_busy_macdinh.controls[0].reason.as_deref(),
            Some("Dự án x bận")
        );

        // 3. same busy with nhanh (auto false) -> approve enabled
        let auto_facts_nhanh = AutoFacts {
            workflow: Some(nhanh),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };
        let view_busy_nhanh = derive_with(&facts_busy, &auto_facts_nhanh);
        assert_eq!(view_busy_nhanh.controls[0].id, "approve");
        assert!(view_busy_nhanh.controls[0].enabled);

        // 4. running lead run -> approve disabled
        let lead_running = dummy_run(1, "lead", "running", None);
        let facts_lead_running = TaskFacts {
            task: &task,
            lead: Some(&lead_running),
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };
        let view_lead_running = derive_with(&facts_lead_running, &auto_facts_macdinh);
        assert_eq!(view_lead_running.controls[0].id, "approve");
        assert!(!view_lead_running.controls[0].enabled);
        assert_eq!(
            view_lead_running.controls[0].reason.as_deref(),
            Some("Đang có run chạy — chờ run xong hoặc bấm Kill")
        );
    }

    #[test]
    fn test_gate_not_open() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();

        let task = dummy_task("planned");
        let git = GitFacts::default();
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };

        for gate in [
            GateState::AwaitingReview,
            GateState::NoPlan,
            GateState::Approved,
        ] {
            let auto_facts = AutoFacts {
                workflow: Some(mac_dinh),
                gate: Some(gate),
                ..Default::default()
            };
            let view = derive_with(&facts, &auto_facts);
            assert!(
                !view.controls.iter().any(|c| c.id == "approve"),
                "gate {:?} should not have approve",
                gate
            );
            assert!(
                !view.controls.iter().any(|c| c.id == "return_lead"),
                "gate {:?} should not have return_lead",
                gate
            );
        }
    }

    #[test]
    fn test_auto_state_mapping() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();

        let task = dummy_task("planned");
        let git = GitFacts::default();
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };

        // 1. auto_state "running" + driver_alive: true -> "running"
        let auto_run = dummy_task_auto(Some("running"), None);
        let af1 = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto_run),
            driver_alive: true,
            ..Default::default()
        };
        let v1 = derive_with(&facts, &af1);
        assert_eq!(v1.auto.state, "running");
        assert!(!v1.auto.stop_requested);

        // 2. auto_state "running" + auto_stop_at Some -> "stopping" and stop_requested true
        let auto_stopping = dummy_task_auto(Some("running"), Some(1500));
        let af2 = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto_stopping),
            driver_alive: true,
            ..Default::default()
        };
        let v2 = derive_with(&facts, &af2);
        assert_eq!(v2.auto.state, "stopping");
        assert!(v2.auto.stop_requested);

        // 3. auto_state "running" + driver dead -> "lost"
        let af3 = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto_run),
            driver_alive: false,
            ..Default::default()
        };
        let v3 = derive_with(&facts, &af3);
        assert_eq!(v3.auto.state, "lost");

        // 4. auto_state "interrupted" -> "lost"
        let auto_interrupted = dummy_task_auto(Some("interrupted"), None);
        let af4 = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto_interrupted),
            driver_alive: false,
            ..Default::default()
        };
        let v4 = derive_with(&facts, &af4);
        assert_eq!(v4.auto.state, "lost");

        // 5. auto_state "gate" with gate Waiting -> "gate"
        let auto_gate = dummy_task_auto(Some("gate"), None);
        let af5 = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto_gate),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };
        let v5 = derive_with(&facts, &af5);
        assert_eq!(v5.auto.state, "gate");

        // 6. auto_state "gate" with gate Approved -> "off"
        let af6 = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto_gate),
            gate: Some(GateState::Approved),
            ..Default::default()
        };
        let v6 = derive_with(&facts, &af6);
        assert_eq!(v6.auto.state, "off");

        // 7. auto_state "done" -> "done"
        let auto_done = dummy_task_auto(Some("done"), None);
        let af7 = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto_done),
            ..Default::default()
        };
        let v7 = derive_with(&facts, &af7);
        assert_eq!(v7.auto.state, "done");

        // 8. auto_state "ended" / NULL -> "off"
        let auto_ended = dummy_task_auto(Some("ended"), None);
        let af8 = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto_ended),
            ..Default::default()
        };
        assert_eq!(derive_with(&facts, &af8).auto.state, "off");

        let auto_null = dummy_task_auto(None, None);
        let af9 = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto_null),
            ..Default::default()
        };
        assert_eq!(derive_with(&facts, &af9).auto.state, "off");
    }

    #[test]
    fn test_running_review_shows_kill() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();

        let task = dummy_task("planned");
        let git = GitFacts::default();
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };

        let review_run = dummy_run(9, "plan-review", "running", None);
        let af1 = AutoFacts {
            workflow: Some(mac_dinh),
            running: Some(&review_run),
            ..Default::default()
        };
        let v1 = derive_with(&facts, &af1);
        assert_eq!(
            v1.actions,
            vec![ActionView {
                id: "kill".to_string(),
                label: "Kill".to_string(),
                enabled: true,
                reason: None,
                run_id: Some(9),
            }]
        );
        assert_eq!(v1.group, "running");
        assert_eq!(v1.current_step, "lead");

        let code_review_run = dummy_run(10, "code-review", "running", None);
        let af2 = AutoFacts {
            workflow: Some(mac_dinh),
            running: Some(&code_review_run),
            ..Default::default()
        };
        let v2 = derive_with(&facts, &af2);
        assert_eq!(v2.actions[0].run_id, Some(10));
        assert_eq!(v2.group, "running");
        assert_eq!(v2.current_step, "dev");
    }

    #[test]
    fn test_driver_between_stages() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();

        let task = dummy_task("planned");
        let git = GitFacts {
            current_branch: Some("task/sua-app".to_string()),
            plan_exists: true,
            plan_commit: Some("abc".to_string()),
            ..Default::default()
        };
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };

        let auto = dummy_task_auto(Some("running"), None);
        let af = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto),
            driver_alive: true,
            running: None,
            ..Default::default()
        };

        let v = derive_with(&facts, &af);
        assert_eq!(v.group, "running");

        // actions run_dev and relead present and disabled
        assert_eq!(v.actions.len(), 2);
        assert_eq!(v.actions[0].id, "run_dev");
        assert!(!v.actions[0].enabled);
        assert_eq!(
            v.actions[0].reason.as_deref(),
            Some("Autopilot đang chạy task này — bấm Dừng autopilot để làm tay")
        );
        assert_eq!(v.actions[1].id, "relead");
        assert!(!v.actions[1].enabled);
        assert_eq!(
            v.actions[1].reason.as_deref(),
            Some("Autopilot đang chạy task này — bấm Dừng autopilot để làm tay")
        );

        // controls contain auto_stop enabled and no auto_start
        assert!(v.controls.iter().any(|c| c.id == "auto_stop" && c.enabled));
        assert!(!v.controls.iter().any(|c| c.id == "auto_start"));
    }

    #[test]
    fn test_stop_requested_disables_stop() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();

        let task = dummy_task("planned");
        let git = GitFacts::default();
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };

        let auto = dummy_task_auto(Some("running"), Some(1500));
        let af = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto),
            driver_alive: true,
            ..Default::default()
        };
        let v = derive_with(&facts, &af);
        let auto_stop = v.controls.iter().find(|c| c.id == "auto_stop").unwrap();
        assert!(!auto_stop.enabled);
        assert_eq!(
            auto_stop.reason.as_deref(),
            Some("Đang chờ run hiện tại chạy xong.")
        );
    }

    #[test]
    fn test_stopped_banner_and_group() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();

        let task = dummy_task("blocked");
        let git = GitFacts {
            current_branch: Some("task/sua-app".to_string()),
            plan_exists: true,
            plan_commit: Some("abc".to_string()),
            ..Default::default()
        };
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };

        let auto = dummy_task_auto(Some("stopped"), None);
        let stop_info = StopInfo {
            reason: StopReason::HetVong,
            loop_name: Some(Loop::Build),
            stage: None,
            round: Some(3),
            cap: Some(3),
            run_id: None,
            role: None,
            at_ms: 1000,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        let aj = AutoJson {
            stop: Some(stop_info),
            ..Default::default()
        };
        let af = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto),
            auto_json: Some(&aj),
            ..Default::default()
        };

        let v = derive_with(&facts, &af);
        assert_eq!(v.auto.state, "stopped");
        assert_eq!(v.auto.stop_reason.as_deref(), Some("het_vong"));
        assert_eq!(v.auto.stop_word.as_deref(), Some("hết vòng"));
        assert_eq!(
            v.auto.stop_detail.as_deref(),
            Some("Review code vẫn yêu cầu sửa lại sau 3 / 3 vòng.")
        );
        assert_eq!(v.auto.stopped_in.as_deref(), Some("build"));
        assert_eq!(v.banner.as_deref(), Some("Chờ bạn · hết vòng"));
        assert_eq!(v.group, "waiting");

        let control_ids: Vec<&str> = v.controls.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            control_ids,
            vec!["auto_start", "auto_more_round", "auto_end"]
        );
    }

    #[test]
    fn test_more_round_reasons() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();

        let task = dummy_task("blocked");
        let git_blockers = GitFacts {
            current_branch: Some("task/sua-app".to_string()),
            plan_blockers: true,
            plan_exists: true,
            plan_commit: Some("abc".to_string()),
            ..Default::default()
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

        let auto = dummy_task_auto(Some("stopped"), None);
        let stop_vuong_mac = StopInfo {
            reason: StopReason::VuongMac,
            loop_name: Some(Loop::Build),
            stage: None,
            round: Some(1),
            cap: None,
            run_id: None,
            role: None,
            at_ms: 1000,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        let aj = AutoJson {
            stop: Some(stop_vuong_mac),
            ..Default::default()
        };
        let af = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto),
            auto_json: Some(&aj),
            ..Default::default()
        };

        // 1. VuongMac + plan_blockers -> auto_more_round disabled
        let v1 = derive_with(&facts, &af);
        let more_round1 = v1
            .controls
            .iter()
            .find(|c| c.id == "auto_more_round")
            .unwrap();
        assert!(!more_round1.enabled);
        assert_eq!(
            more_round1.reason.as_deref(),
            Some("Vướng mắc chưa xử lý trong .plan.md — hãy xử lý trước khi chạy dev")
        );
        let auto_end1 = v1.controls.iter().find(|c| c.id == "auto_end").unwrap();
        assert!(auto_end1.enabled);

        // 2. busy -> disabled with busy text
        let git_clean = GitFacts {
            current_branch: Some("task/sua-app".to_string()),
            plan_blockers: false,
            plan_exists: true,
            plan_commit: Some("abc".to_string()),
            ..Default::default()
        };
        let facts_busy = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git_clean,
            busy: Some("Dự án x đang bận".to_string()),
            now_ms: 2000,
        };
        let v2 = derive_with(&facts_busy, &af);
        let more_round2 = v2
            .controls
            .iter()
            .find(|c| c.id == "auto_more_round")
            .unwrap();
        assert!(!more_round2.enabled);
        assert_eq!(more_round2.reason.as_deref(), Some("Dự án x đang bận"));
        let auto_end2 = v2.controls.iter().find(|c| c.id == "auto_end").unwrap();
        assert!(auto_end2.enabled);
    }

    #[test]
    fn test_auto_start_reasons() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();

        let git = GitFacts {
            current_branch: Some("task/sua-app".to_string()),
            plan_exists: true,
            plan_commit: Some("abc".to_string()),
            ..Default::default()
        };

        // 1. qa_pass -> disabled "Task đã xong (QA đạt)."
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
        let af = AutoFacts {
            workflow: Some(mac_dinh),
            ..Default::default()
        };
        let v_pass = derive_with(&facts_pass, &af);
        let start_pass = v_pass
            .controls
            .iter()
            .find(|c| c.id == "auto_start")
            .unwrap();
        assert!(!start_pass.enabled);
        assert_eq!(start_pass.reason.as_deref(), Some("Task đã xong (QA đạt)."));

        // 2. busy -> busy text
        let task_dev_done = dummy_task("dev_done");
        let facts_busy = TaskFacts {
            task: &task_dev_done,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: Some("Dự án bận".to_string()),
            now_ms: 2000,
        };
        let v_busy = derive_with(&facts_busy, &af);
        let start_busy = v_busy
            .controls
            .iter()
            .find(|c| c.id == "auto_start")
            .unwrap();
        assert!(!start_busy.enabled);
        assert_eq!(start_busy.reason.as_deref(), Some("Dự án bận"));

        // 3. branch not checked out -> 05.1 branch text
        let git_wrong = GitFacts {
            current_branch: Some("main".to_string()),
            ..git.clone()
        };
        let facts_wrong = TaskFacts {
            task: &task_dev_done,
            lead: None,
            dev: None,
            qa: None,
            git: &git_wrong,
            busy: None,
            now_ms: 2000,
        };
        let v_wrong = derive_with(&facts_wrong, &af);
        let start_wrong = v_wrong
            .controls
            .iter()
            .find(|c| c.id == "auto_start")
            .unwrap();
        assert!(!start_wrong.enabled);
        assert_eq!(
            start_wrong.reason.as_deref(),
            Some("Không ở nhánh task 'task/sua-app' (nhánh hiện tại: 'main') — không đoán")
        );

        // 4. clean idle 5.2 task after build -> enabled
        let facts_clean = TaskFacts {
            task: &task_dev_done,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };
        let v_clean = derive_with(&facts_clean, &af);
        let start_clean = v_clean
            .controls
            .iter()
            .find(|c| c.id == "auto_start")
            .unwrap();
        assert!(start_clean.enabled);
    }

    #[test]
    fn test_lost_driver_view() {
        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let wfs = crate::workflows::load_from(None, &cfg);
        let mac_dinh = wfs.get("mac-dinh").unwrap();

        let task = dummy_task("dev_done");
        let git = GitFacts {
            current_branch: Some("task/sua-app".to_string()),
            plan_exists: true,
            plan_commit: Some("abc".to_string()),
            ..Default::default()
        };
        let facts = TaskFacts {
            task: &task,
            lead: None,
            dev: None,
            qa: None,
            git: &git,
            busy: None,
            now_ms: 2000,
        };

        // auto_state "running", driver dead -> auto.state "lost", no banner change, auto_start enabled
        let auto = dummy_task_auto(Some("running"), None);
        let af = AutoFacts {
            workflow: Some(mac_dinh),
            auto: Some(&auto),
            driver_alive: false,
            ..Default::default()
        };
        let v = derive_with(&facts, &af);
        assert_eq!(v.auto.state, "lost");
        assert_eq!(v.banner.as_deref(), Some("Dev xong — chờ chạy QA"));
        let start = v.controls.iter().find(|c| c.id == "auto_start").unwrap();
        assert!(start.enabled);
    }

    #[test]
    fn test_current_step_of_matches_derive() {
        let git = GitFacts::default();
        let statuses = [
            "open", "planned", "dev_done", "blocked", "qa_fix", "qa_pass",
        ];

        for st in statuses {
            let task = dummy_task(st);
            // 1. None running
            let facts_none = TaskFacts {
                task: &task,
                lead: None,
                dev: None,
                qa: None,
                git: &git,
                busy: None,
                now_ms: 2000,
            };
            assert_eq!(
                current_step_of(&facts_none, None),
                derive(&facts_none).current_step,
                "status: {st}"
            );

            // 2. lead running
            let lead_run = dummy_run(1, "lead", "running", None);
            let facts_lead = TaskFacts {
                task: &task,
                lead: Some(&lead_run),
                dev: None,
                qa: None,
                git: &git,
                busy: None,
                now_ms: 2000,
            };
            assert_eq!(
                current_step_of(&facts_lead, Some(&lead_run)),
                derive(&facts_lead).current_step,
                "status: {st} with lead running"
            );

            // 3. dev running
            let dev_run = dummy_run(2, "dev", "running", None);
            let facts_dev = TaskFacts {
                task: &task,
                lead: None,
                dev: Some(&dev_run),
                qa: None,
                git: &git,
                busy: None,
                now_ms: 2000,
            };
            assert_eq!(
                current_step_of(&facts_dev, Some(&dev_run)),
                derive(&facts_dev).current_step,
                "status: {st} with dev running"
            );

            // 4. qa running
            let qa_run = dummy_run(3, "qa", "running", None);
            let facts_qa = TaskFacts {
                task: &task,
                lead: None,
                dev: None,
                qa: Some(&qa_run),
                git: &git,
                busy: None,
                now_ms: 2000,
            };
            assert_eq!(
                current_step_of(&facts_qa, Some(&qa_run)),
                derive(&facts_qa).current_step,
                "status: {st} with qa running"
            );
        }
    }
}
