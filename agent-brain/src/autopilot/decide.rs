//! Quyết định thuần (decide) của autopilot (AUTO-05).
//!
//! Hàm thuần không I/O, không gọi tiến trình, không truy vấn database,
//! không đọc đồng hồ hệ thống (thời gian được đưa vào qua `facts.now_ms`).

use crate::config::Price;
use crate::db::{RunRow, TaskAuto};
use crate::pricing::{self, BudgetLimits, BudgetUsage};
use crate::workflows::Workflow;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Giai đoạn thực thi trong quy trình.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    Plan,
    PlanReview,
    Implement,
    CodeReview,
    Qa,
    Fix,
}

impl Stage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Stage::Plan => "plan",
            Stage::PlanReview => "plan-review",
            Stage::Implement => "implement",
            Stage::CodeReview => "code-review",
            Stage::Qa => "qa",
            Stage::Fix => "fix",
        }
    }

    pub fn parse(s: &str) -> Option<Stage> {
        match s {
            "plan" => Some(Stage::Plan),
            "plan-review" => Some(Stage::PlanReview),
            "implement" => Some(Stage::Implement),
            "code-review" => Some(Stage::CodeReview),
            "qa" => Some(Stage::Qa),
            "fix" => Some(Stage::Fix),
            _ => None,
        }
    }

    pub fn role(&self) -> &'static str {
        match self {
            Stage::Plan => "lead",
            Stage::PlanReview => "plan-review",
            Stage::Implement => "dev",
            Stage::CodeReview => "code-review",
            Stage::Qa => "qa",
            Stage::Fix => "dev",
        }
    }
}

/// Vòng lặp quy trình.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loop {
    Plan,
    Build,
    Qa,
}

impl Loop {
    pub fn as_str(&self) -> &'static str {
        match self {
            Loop::Plan => "plan",
            Loop::Build => "build",
            Loop::Qa => "qa",
        }
    }
}

/// Mục tiêu sửa lỗi của Dev fix mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixFor {
    CodeReview,
    Qa,
}

impl FixFor {
    pub fn as_str(&self) -> &'static str {
        match self {
            FixFor::CodeReview => "code_review",
            FixFor::Qa => "qa",
        }
    }

    pub fn cli_arg(&self) -> &'static str {
        match self {
            FixFor::CodeReview => "code-review",
            FixFor::Qa => "qa",
        }
    }

    pub fn parse(s: &str) -> Option<FixFor> {
        match s {
            "code_review" | "code-review" => Some(FixFor::CodeReview),
            "qa" => Some(FixFor::Qa),
            _ => None,
        }
    }
}

/// Lý do dừng autopilot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    HetVong,
    HetNganSach,
    ViPhamGiaoThuc,
    VuongMac,
    DungBoiBan,
    RunLoi,
    /// The engine hit its quota or rate limit (ENG-02, D-15); autopilot never switches engine.
    HetQuota,
    /// C2: the project's target environment failed its preflight check (before a run, or while
    /// it ran: outcome `env:down`). Not a round, not budget.
    MoiTruongChuaSan,
}

impl StopReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            StopReason::HetVong => "het_vong",
            StopReason::HetNganSach => "het_ngan_sach",
            StopReason::ViPhamGiaoThuc => "vi_pham_giao_thuc",
            StopReason::VuongMac => "vuong_mac",
            StopReason::DungBoiBan => "dung_boi_ban",
            StopReason::RunLoi => "run_loi",
            StopReason::HetQuota => "het_quota",
            StopReason::MoiTruongChuaSan => "moi_truong_chua_san",
        }
    }
}

/// Thông tin chi tiết khi autopilot dừng.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StopInfo {
    pub reason: StopReason,
    #[serde(default)]
    pub loop_name: Option<Loop>,
    #[serde(default)]
    pub stage: Option<Stage>,
    #[serde(default)]
    pub round: Option<i64>,
    #[serde(default)]
    pub cap: Option<i64>,
    #[serde(default)]
    pub run_id: Option<i64>,
    #[serde(default)]
    pub role: Option<String>,
    pub at_ms: i64,
    #[serde(default)]
    pub detail_line: Option<String>,
    #[serde(default)]
    pub used_ms: Option<i64>,
    #[serde(default)]
    pub used_usd: Option<f64>,
    #[serde(default)]
    pub estimated: Option<bool>,
    #[serde(default)]
    pub limit_minutes: Option<i64>,
    #[serde(default)]
    pub limit_usd: Option<f64>,
}

/// Bước run tiếp theo do autopilot chỉ định.
#[derive(Debug, Clone, PartialEq)]
pub struct StageRun {
    pub stage: Stage,
    pub round: i64,
    pub fix_for: Option<FixFor>,
    pub review_input: bool,
    pub note: Option<String>,
    pub fresh_plan: bool,
    /// The closing Dev turn after a Dev timeout (`finalize_on_timeout`); not a new round.
    pub finalize: bool,
}

/// `StageRun.note` of the closing Dev turn after a Dev timeout.
pub const FINALIZE_NOTE: &str = "Lượt chốt: không làm việc mới. Ghi kết quả đang có, cập nhật file kết quả theo plan, commit.";

impl StageRun {
    pub fn card_key(&self) -> &'static str {
        match self.stage {
            Stage::Plan => "plan",
            Stage::PlanReview => "plan_review",
            Stage::Implement => "code",
            Stage::CodeReview => "code_review",
            Stage::Qa => "qa",
            Stage::Fix => match self.fix_for {
                Some(FixFor::CodeReview) => "code",
                Some(FixFor::Qa) | None => "fix",
            },
        }
    }
}

/// Quyết định của autopilot.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    Run(StageRun),
    Gate,
    Done,
    Stop(StopInfo),
}

/// Yêu cầu replan (Trả về Lead / Chạy lại Lead).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Replan {
    #[serde(default)]
    pub note: Option<String>,
}

/// Cấu trúc auto_json lưu trữ trong tasks.auto_json.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AutoJson {
    #[serde(default)]
    pub grants: BTreeMap<String, i64>,
    #[serde(default)]
    pub bypass_stages: i64,
    #[serde(default)]
    pub replan: Option<Replan>,
    #[serde(default)]
    pub stop: Option<StopInfo>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub budget_extra_minutes: i64,
    #[serde(default)]
    pub budget_extra_usd: f64,
    #[serde(default)]
    pub skip_build_once: bool,
    /// C2: when the scheduler last restarted the autopilot after the environment came back
    /// (Telegram reports each value once).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_rerun_at: Option<i64>,
    /// Set only by the "Dừng hẳn" control (`auto_end`): together with `auto_state = 'ended'` it
    /// marks the task as cancelled. Ending autopilot at the plan gate leaves it `false`.
    #[serde(default)]
    pub cancelled: bool,
}

impl AutoJson {
    pub fn parse(s: Option<&str>) -> Self {
        s.and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Dữ liệu trích xuất từ một RunRow.
#[derive(Debug, Clone, PartialEq)]
pub struct RunFact {
    pub id: i64,
    pub role: String,
    /// `runs.engine` of the run (`claude`, `deepseek`, `agy`, `người`).
    pub engine: String,
    pub stage: Option<Stage>,
    pub fix_for: Option<FixFor>,
    pub round: Option<i64>,
    pub status: String,
    pub outcome: Option<String>,
    pub kill_requested: bool,
    pub started_at: i64,
    pub error: Option<String>,
    /// The run was a closing Dev turn (`metrics_json.finalize`).
    pub finalize: bool,
}

impl RunFact {
    pub fn from_row(row: &RunRow) -> Self {
        let stage = row.stage.as_deref().and_then(Stage::parse);
        let metrics = row
            .metrics_json
            .as_deref()
            .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok());
        let fix_for = metrics.as_ref().and_then(|v| {
            v.get("fix_for")
                .and_then(|f| f.as_str())
                .and_then(FixFor::parse)
        });
        let finalize = metrics
            .as_ref()
            .and_then(|v| v.get("finalize"))
            .and_then(|f| f.as_bool())
            .unwrap_or(false);
        RunFact {
            id: row.id,
            role: row.role.clone(),
            engine: row.engine.clone(),
            stage,
            fix_for,
            round: row.round,
            status: row.status.clone(),
            outcome: row.outcome.clone(),
            kill_requested: row.kill_requested_at.is_some(),
            started_at: row.started_at,
            error: row.error.clone(),
            finalize,
        }
    }

    /// The run stopped on quota or a rate limit (outcome `quota`).
    pub fn is_quota(&self) -> bool {
        self.outcome.as_deref() == Some(crate::quota::OUTCOME_QUOTA)
    }

    /// The run was stopped because its environment went down (outcome `env:down`, C2).
    pub fn is_env_down(&self) -> bool {
        self.outcome.as_deref() == Some(crate::preflight::OUTCOME_ENV_DOWN)
    }

    /// A stop that is nobody's fault and consumes no round: quota or environment down.
    pub fn is_pause(&self) -> bool {
        self.is_quota() || self.is_env_down()
    }

    pub fn is_ok(&self) -> bool {
        if self.status != "succeeded" {
            return false;
        }
        if let Some(ref oc) = self.outcome {
            if oc == "protocol_violation"
                || oc == "dev:blocked"
                || oc == "agy:empty"
                || oc == "dsh:empty"
                || oc == crate::quota::OUTCOME_QUOTA
                || oc == crate::preflight::OUTCOME_ENV_DOWN
            {
                return false;
            }
        }
        true
    }
}

/// Yêu cầu dừng từ bên ngoài.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopRequest {
    Soft,
    Kill,
}

/// Toàn bộ dữ liệu ngữ cảnh để ra quyết định thuần.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub runs: Vec<RunFact>,
    pub plan_commit: Option<String>,
    pub approved: bool,
    pub stop_request: Option<StopRequest>,
    pub budget_usage: BudgetUsage,
    pub budget_limits: BudgetLimits,
    pub bypass_left: i64,
    pub grants: BTreeMap<String, i64>,
    pub replan: Option<Replan>,
    pub resume: bool,
    pub now_ms: i64,
    pub skip_build_once: bool,
    pub code_after_plan: bool,
    /// The working tree has uncommitted changes to files of the plan (set by the driver).
    pub dev_dirty_in_plan: bool,
}

impl Facts {
    pub fn with_code_after_plan(mut self, code_after_plan: bool) -> Self {
        self.code_after_plan = code_after_plan;
        self
    }
}

/// Trạng thái cổng duyệt plan (Bạn card).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateState {
    NoPlan,
    AwaitingReview,
    Waiting,
    Approved,
}

/// Trạng thái của giai đoạn plan.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanPhase {
    Done,
    Next(StageRun),
    Stopped(StopInfo),
}

/// Xây dựng facts từ các hàng dữ liệu database và tham số runtime.
pub fn build_facts(
    wf: &Workflow,
    runs: &[RunRow],
    auto: &TaskAuto,
    aj: &AutoJson,
    plan_commit: Option<&str>,
    pricing: &BTreeMap<String, Price>,
    now_ms: i64,
    resume: bool,
) -> Facts {
    let mut filtered_runs: Vec<RunFact> = runs
        .iter()
        .filter(|r| r.role != "inspect")
        .map(RunFact::from_row)
        .collect();
    filtered_runs.sort_by_key(|r| (r.started_at, r.id));

    // Pitfall 2: a quota run never consumes a round. It is kept only as the LAST run when not
    // resuming (so D2 can stop on it); on resume, or once a later run supersedes it, it is dropped
    // and the stopped step is decided again with the same round.
    let last_index = filtered_runs.len().saturating_sub(1);
    let mut index = 0usize;
    filtered_runs.retain(|r| {
        let i = index;
        index += 1;
        !r.is_pause() || (!resume && i == last_index)
    });

    // D-28: approved chỉ đúng khi approved_commit bằng với plan_commit hiện tại
    let approved = match (&auto.approved_commit, plan_commit) {
        (Some(ac), Some(pc)) => ac == pc,
        _ => false,
    };

    let stop_request = if auto.auto_stop_at.is_some() {
        if auto.auto_stop_kind.as_deref() == Some("kill") {
            Some(StopRequest::Kill)
        } else {
            Some(StopRequest::Soft)
        }
    } else {
        None
    };

    let budget_usage = crate::pricing::budget_usage(runs, pricing, now_ms);
    let budget_limits = BudgetLimits::from_workflow(wf).with_extra(aj.budget_extra_minutes, aj.budget_extra_usd);

    Facts {
        runs: filtered_runs,
        plan_commit: plan_commit.map(|s| s.to_string()),
        approved,
        stop_request,
        budget_usage,
        budget_limits,
        bypass_left: aj.bypass_stages,
        grants: aj.grants.clone(),
        replan: aj.replan.clone(),
        resume,
        now_ms,
        skip_build_once: aj.skip_build_once,
        code_after_plan: false,
        dev_dirty_in_plan: false,
    }
}

/// L0 loop_of: xác định vòng lặp của một run.
pub fn loop_of(r: &RunFact) -> Option<Loop> {
    if let Some(stage) = r.stage {
        match stage {
            Stage::Plan | Stage::PlanReview => Some(Loop::Plan),
            Stage::Implement | Stage::CodeReview => Some(Loop::Build),
            Stage::Fix => {
                if r.fix_for == Some(FixFor::Qa) {
                    Some(Loop::Qa)
                } else {
                    Some(Loop::Build)
                }
            }
            Stage::Qa => Some(Loop::Qa),
        }
    } else {
        match r.role.as_str() {
            "lead" | "plan-review" => Some(Loop::Plan),
            "dev" | "code-review" => Some(Loop::Build),
            "qa" => Some(Loop::Qa),
            _ => None,
        }
    }
}

/// C1 cycles: tính chu kỳ của từng run trong danh sách.
pub fn cycles(runs: &[RunFact]) -> Vec<i64> {
    let mut res = Vec::with_capacity(runs.len());
    let mut current_cycle = 1i64;
    let mut last_loop: Option<Loop> = None;

    for r in runs {
        let is_author_plan = match r.stage {
            Some(Stage::Plan) => true,
            None => r.role == "lead",
            _ => false,
        };
        if is_author_plan {
            if let Some(Loop::Build) | Some(Loop::Qa) = last_loop {
                current_cycle += 1;
            }
        }
        res.push(current_cycle);

        if let Some(lp) = loop_of(r) {
            last_loop = Some(lp);
        }
    }
    res
}

/// R1 effective_rounds: tính round hiệu lực của từng run.
pub fn effective_rounds(runs: &[RunFact]) -> Vec<i64> {
    let cycs = cycles(runs);
    let mut res = Vec::with_capacity(runs.len());

    for (i, r) in runs.iter().enumerate() {
        if let Some(stored_round) = r.round {
            res.push(stored_round);
            continue;
        }

        let run_cycle = cycs[i];
        // C1: a closing Dev turn keeps the round of the step it closes.
        if r.finalize {
            let closed = (0..i).rev().find(|&j| {
                cycs[j] == run_cycle
                    && runs[j].role == r.role
                    && runs[j].stage == r.stage
                    && runs[j].fix_for == r.fix_for
            });
            if let Some(j) = closed {
                res.push(res[j]);
                continue;
            }
        }
        let is_plan = r.stage == Some(Stage::Plan) || (r.stage.is_none() && r.role == "lead");
        let is_plan_review =
            r.stage == Some(Stage::PlanReview) || (r.stage.is_none() && r.role == "plan-review");
        let is_implement =
            r.stage == Some(Stage::Implement) || (r.stage.is_none() && r.role == "dev");
        let is_code_review =
            r.stage == Some(Stage::CodeReview) || (r.stage.is_none() && r.role == "code-review");
        let is_fix_code_review =
            r.stage == Some(Stage::Fix) && r.fix_for == Some(FixFor::CodeReview);
        let is_qa = r.stage == Some(Stage::Qa) || (r.stage.is_none() && r.role == "qa");
        let is_fix_qa = r.stage == Some(Stage::Fix) && r.fix_for == Some(FixFor::Qa);

        if is_plan {
            let earlier_plan_count = runs[..i]
                .iter()
                .enumerate()
                .filter(|(j, prev)| {
                    cycs[*j] == run_cycle
                        && !prev.is_pause()
                        && (prev.stage == Some(Stage::Plan)
                            || (prev.stage.is_none() && prev.role == "lead"))
                })
                .count();
            res.push(1 + earlier_plan_count as i64);
        } else if is_plan_review {
            let latest_plan_round = runs[..i]
                .iter()
                .enumerate()
                .rev()
                .find(|(j, prev)| {
                    cycs[*j] == run_cycle
                        && (prev.stage == Some(Stage::Plan)
                            || (prev.stage.is_none() && prev.role == "lead"))
                })
                .map(|(j, _)| res[j])
                .unwrap_or(1);
            res.push(latest_plan_round);
        } else if is_implement {
            let earlier_implement_count = runs[..i]
                .iter()
                .enumerate()
                .filter(|(j, prev)| {
                    cycs[*j] == run_cycle
                        && !prev.is_pause()
                        && !prev.finalize
                        && (prev.stage == Some(Stage::Implement)
                            || (prev.stage.is_none() && prev.role == "dev"))
                })
                .count();
            res.push(1 + earlier_implement_count as i64);
        } else if is_code_review {
            let latest_impl_or_fix_round = runs[..i]
                .iter()
                .enumerate()
                .rev()
                .find(|(j, prev)| {
                    cycs[*j] == run_cycle
                        && (prev.stage == Some(Stage::Implement)
                            || (prev.stage.is_none() && prev.role == "dev")
                            || (prev.stage == Some(Stage::Fix)
                                && prev.fix_for == Some(FixFor::CodeReview)))
                })
                .map(|(j, _)| res[j])
                .unwrap_or(1);
            res.push(latest_impl_or_fix_round);
        } else if is_fix_code_review {
            let earlier_count = runs[..i]
                .iter()
                .enumerate()
                .filter(|(j, prev)| {
                    cycs[*j] == run_cycle
                        && !prev.is_pause()
                        && !prev.finalize
                        && (prev.stage == Some(Stage::Implement)
                            || (prev.stage.is_none() && prev.role == "dev")
                            || (prev.stage == Some(Stage::Fix)
                                && prev.fix_for == Some(FixFor::CodeReview)))
                })
                .count();
            res.push(1 + earlier_count as i64);
        } else if is_qa {
            let earlier_qa_count = runs[..i]
                .iter()
                .enumerate()
                .filter(|(j, prev)| {
                    cycs[*j] == run_cycle
                        && !prev.is_pause()
                        && (prev.stage == Some(Stage::Qa)
                            || (prev.stage.is_none() && prev.role == "qa"))
                })
                .count();
            res.push(1 + earlier_qa_count as i64);
        } else if is_fix_qa {
            let latest_qa_round = runs[..i]
                .iter()
                .enumerate()
                .rev()
                .find(|(j, prev)| {
                    cycs[*j] == run_cycle
                        && (prev.stage == Some(Stage::Qa)
                            || (prev.stage.is_none() && prev.role == "qa"))
                })
                .map(|(j, _)| res[j])
                .unwrap_or(1);
            res.push(latest_qa_round);
        } else {
            res.push(1);
        }
    }

    res
}

/// D4 plan_phase: tính toán kết quả giai đoạn lập plan.
pub fn plan_phase(wf: &Workflow, f: &Facts) -> PlanPhase {
    let cycs = cycles(&f.runs);
    let current_cycle = cycs.last().copied().unwrap_or(1);
    let eff_rounds = effective_rounds(&f.runs);

    // Lọc các ok run trong chu kỳ hiện tại
    let ok_runs: Vec<(usize, &RunFact)> = f
        .runs
        .iter()
        .enumerate()
        .filter(|(idx, r)| cycs[*idx] == current_cycle && r.is_ok())
        .collect();

    let highest_ok_plan_round = ok_runs
        .iter()
        .filter(|(_idx, r)| r.stage == Some(Stage::Plan) || (r.stage.is_none() && r.role == "lead"))
        .map(|(idx, _)| eff_rounds[*idx])
        .max()
        .unwrap_or(0);

    // (a) workflow có plan và plan_commit là None
    if wf.has("plan") && f.plan_commit.is_none() {
        // Trần vòng cho Lead khi chưa có commit plan (WR-02): max_rounds + grant,
        // hoặc 1 + grant khi workflow không có vòng plan.
        let grant = f.grants.get("plan").copied().unwrap_or(0);
        let cap_a = if wf.has_loop("plan") {
            wf.max_rounds as i64 + grant
        } else {
            1 + grant
        };
        if highest_ok_plan_round >= cap_a {
            let newest_lead = ok_runs
                .iter()
                .filter(|(_idx, r)| {
                    r.stage == Some(Stage::Plan) || (r.stage.is_none() && r.role == "lead")
                })
                .last()
                .map(|(_idx, r)| *r);
            return PlanPhase::Stopped(StopInfo {
                reason: StopReason::RunLoi,
                loop_name: Some(Loop::Plan),
                stage: Some(Stage::Plan),
                round: Some(highest_ok_plan_round),
                cap: Some(cap_a),
                run_id: newest_lead.map(|r| r.id),
                role: newest_lead.map(|r| r.role.clone()),
                at_ms: f.now_ms,
                detail_line: Some(format!(
                    "Lead đã chạy {highest_ok_plan_round} vòng mà chưa có commit plan"
                )),
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            });
        }
        return PlanPhase::Next(StageRun {
            stage: Stage::Plan,
            round: 1 + highest_ok_plan_round,
            fix_for: None,
            review_input: false,
            note: None,
            fresh_plan: true,
            finalize: false,
        });
    }

    // (b) workflow có plan-review
    if wf.has("plan-review") {
        let last_ok_plan_loop = ok_runs
            .iter()
            .filter(|(_, r)| loop_of(r) == Some(Loop::Plan))
            .last();

        match last_ok_plan_loop {
            None => {
                return PlanPhase::Next(StageRun {
                    stage: Stage::PlanReview,
                    round: 1,
                    fix_for: None,
                    review_input: false,
                    note: None,
                    fresh_plan: false,
                    finalize: false,
                });
            }
            Some(&(idx, x)) => {
                let r = eff_rounds[idx];
                let is_plan =
                    x.stage == Some(Stage::Plan) || (x.stage.is_none() && x.role == "lead");
                if is_plan {
                    return PlanPhase::Next(StageRun {
                        stage: Stage::PlanReview,
                        round: r,
                        fix_for: None,
                        review_input: false,
                        note: None,
                        fresh_plan: false,
                        finalize: false,
                    });
                }
                // X là PlanReview
                if x.outcome.as_deref() == Some("review:approve") {
                    return PlanPhase::Done;
                } else if x.outcome.as_deref() == Some("review:revise") {
                    if wf.has("plan") && wf.has_loop("plan") {
                        let grant = f.grants.get("plan").copied().unwrap_or(0);
                        let cap = wf.max_rounds as i64 + grant;
                        if r >= cap {
                            return PlanPhase::Stopped(StopInfo {
                                reason: StopReason::HetVong,
                                loop_name: Some(Loop::Plan),
                                stage: Some(Stage::PlanReview),
                                round: Some(r),
                                cap: Some(cap),
                                run_id: Some(x.id),
                                role: Some(x.role.clone()),
                                at_ms: f.now_ms,
                                detail_line: None,
                                used_ms: None,
                                used_usd: None,
                                estimated: None,
                                limit_minutes: None,
                                limit_usd: None,
                            });
                        } else {
                            return PlanPhase::Next(StageRun {
                                stage: Stage::Plan,
                                round: r + 1,
                                fix_for: None,
                                review_input: true,
                                note: None,
                                fresh_plan: false,
                                finalize: false,
                            });
                        }
                    } else {
                        return PlanPhase::Stopped(StopInfo {
                            reason: StopReason::HetVong,
                            loop_name: Some(Loop::Plan),
                            stage: Some(Stage::PlanReview),
                            round: Some(1),
                            cap: Some(1),
                            run_id: Some(x.id),
                            role: Some(x.role.clone()),
                            at_ms: f.now_ms,
                            detail_line: None,
                            used_ms: None,
                            used_usd: None,
                            estimated: None,
                            limit_minutes: None,
                            limit_usd: None,
                        });
                    }
                } else {
                    return PlanPhase::Stopped(StopInfo {
                        reason: StopReason::RunLoi,
                        loop_name: Some(Loop::Plan),
                        stage: Some(Stage::PlanReview),
                        round: Some(r),
                        cap: None,
                        run_id: Some(x.id),
                        role: Some(x.role.clone()),
                        at_ms: f.now_ms,
                        detail_line: Some("Review không có kết luận".to_string()),
                        used_ms: None,
                        used_usd: None,
                        estimated: None,
                        limit_minutes: None,
                        limit_usd: None,
                    });
                }
            }
        }
    }

    // (c) trường hợp còn lại
    PlanPhase::Done
}

/// Xác định trạng thái cổng duyệt plan (Bạn card).
pub fn gate_state(wf: &Workflow, f: &Facts) -> GateState {
    if f.approved {
        return GateState::Approved;
    }
    if f.plan_commit.is_none() {
        return GateState::NoPlan;
    }
    if wf.has("plan-review") {
        match plan_phase(wf, f) {
            PlanPhase::Done | PlanPhase::Stopped(_) => GateState::Waiting,
            PlanPhase::Next(_) => GateState::AwaitingReview,
        }
    } else {
        GateState::Waiting
    }
}

// ============================================================================
// Các hàm trợ giúp cho quy tắc D1 .. D9
// ============================================================================

/// Quy tắc D1: Yêu cầu dừng từ bên ngoài (soft hoặc kill stop request).
fn rule_d1_stop_request(f: &Facts, eff_rounds: &[i64]) -> Option<Decision> {
    if f.stop_request.is_some() {
        let last_run = f.runs.last();
        let last_idx = f.runs.len().checked_sub(1);
        let round = last_idx.map(|idx| eff_rounds[idx]);
        Some(Decision::Stop(StopInfo {
            reason: StopReason::DungBoiBan,
            loop_name: last_run.and_then(loop_of),
            stage: last_run.and_then(|r| r.stage),
            round,
            cap: None,
            run_id: last_run.map(|r| r.id),
            role: last_run.map(|r| r.role.clone()),
            at_ms: f.now_ms,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        }))
    } else {
        None
    }
}

/// Quy tắc D2: Kiểm tra lỗi / trạng thái bất thường của run cuối cùng (trừ khi đang resume).
fn rule_d2_last_run_failures(f: &Facts, eff_rounds: &[i64]) -> Option<Decision> {
    if f.resume {
        return None;
    }
    let last_run = f.runs.last()?;
    let last_idx = f.runs.len() - 1;
    let round = Some(eff_rounds[last_idx]);
    let loop_name = loop_of(last_run);
    let stage = last_run.stage;

    if last_run.status == "running" {
        return Some(Decision::Stop(StopInfo {
            reason: StopReason::RunLoi,
            loop_name,
            stage,
            round,
            cap: None,
            run_id: Some(last_run.id),
            role: Some(last_run.role.clone()),
            at_ms: f.now_ms,
            detail_line: Some(format!("Run {} vẫn đang chạy", last_run.id)),
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        }));
    }
    // ENG-02 / D-15: a quota or rate-limit stop is its own reason, checked before the generic
    // failed-run rule. Nothing switches engine or retries (D-16).
    if last_run.is_quota() {
        return Some(Decision::Stop(StopInfo {
            reason: StopReason::HetQuota,
            loop_name,
            stage,
            round,
            cap: None,
            run_id: Some(last_run.id),
            role: Some(last_run.role.clone()),
            at_ms: f.now_ms,
            detail_line: Some(crate::quota::stop_detail(
                last_run.id,
                &last_run.role,
                &last_run.engine,
            )),
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        }));
    }
    // C2: the environment went down during the run; like quota, its own reason and no round.
    if last_run.is_env_down() {
        return Some(Decision::Stop(StopInfo {
            reason: StopReason::MoiTruongChuaSan,
            loop_name,
            stage,
            round,
            cap: None,
            run_id: Some(last_run.id),
            role: Some(last_run.role.clone()),
            at_ms: f.now_ms,
            detail_line: last_run.error.clone(),
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        }));
    }
    if last_run.status == "killed" {
        return Some(Decision::Stop(StopInfo {
            reason: StopReason::DungBoiBan,
            loop_name,
            stage,
            round,
            cap: None,
            run_id: Some(last_run.id),
            role: Some(last_run.role.clone()),
            at_ms: f.now_ms,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        }));
    }
    if last_run.status == "failed" || last_run.status == "timed_out" || last_run.status == "lost" {
        return Some(Decision::Stop(StopInfo {
            reason: StopReason::RunLoi,
            loop_name,
            stage,
            round,
            cap: None,
            run_id: Some(last_run.id),
            role: Some(last_run.role.clone()),
            at_ms: f.now_ms,
            detail_line: last_run.error.clone(),
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        }));
    }
    if last_run.outcome.as_deref() == Some("protocol_violation") {
        return Some(Decision::Stop(StopInfo {
            reason: StopReason::ViPhamGiaoThuc,
            loop_name,
            stage,
            round,
            cap: None,
            run_id: Some(last_run.id),
            role: Some(last_run.role.clone()),
            at_ms: f.now_ms,
            detail_line: last_run.error.clone(),
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        }));
    }
    if last_run.outcome.as_deref() == Some("dev:blocked") {
        return Some(Decision::Stop(StopInfo {
            reason: StopReason::VuongMac,
            loop_name,
            stage,
            round,
            cap: None,
            run_id: Some(last_run.id),
            role: Some(last_run.role.clone()),
            at_ms: f.now_ms,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        }));
    }
    // agy trả SUCCESS nhưng không sửa file nào: nhiều khả năng bị từ chối quyền (D-05).
    if last_run.outcome.as_deref() == Some("agy:empty") {
        let detail_line = if last_run.role == "lead" {
            crate::roles::lead::LEAD_AGY_EMPTY_MESSAGE
        } else {
            crate::roles::dev::AGY_EMPTY_MESSAGE
        };
        return Some(Decision::Stop(StopInfo {
            reason: StopReason::VuongMac,
            loop_name,
            stage,
            round,
            cap: None,
            run_id: Some(last_run.id),
            role: Some(last_run.role.clone()),
            at_ms: f.now_ms,
            detail_line: Some(detail_line.to_string()),
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        }));
    }
    // dsh chạy xong nhưng không sửa file nào: container không ghi được hoặc dsh chỉ trả lời
    // (D-35, D-16); không tự chạy tiếp hay đổi engine.
    if last_run.outcome.as_deref() == Some("dsh:empty") {
        return Some(Decision::Stop(StopInfo {
            reason: StopReason::VuongMac,
            loop_name,
            stage,
            round,
            cap: None,
            run_id: Some(last_run.id),
            role: Some(last_run.role.clone()),
            at_ms: f.now_ms,
            detail_line: Some(crate::roles::dev::DSH_EMPTY_MESSAGE.to_string()),
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        }));
    }

    None
}

/// C1 `finalize_on_timeout`: the last run is a Dev run (implement or fix) that timed out, the
/// workflow enables the option, files of the plan are still changed, and no closing turn ran for
/// this step and round yet → one more Dev run of the same step and round, with `FINALIZE_NOTE`.
/// Off (the default) → `None`, and D2 stops on the timeout as before.
fn rule_finalize_on_timeout(
    wf: &Workflow,
    f: &Facts,
    cycs: &[i64],
    eff_rounds: &[i64],
) -> Option<StageRun> {
    if f.resume || !wf.finalize_on_timeout.enabled || !f.dev_dirty_in_plan {
        return None;
    }
    let last = f.runs.last()?;
    let last_idx = f.runs.len() - 1;
    if last.role != "dev" || last.status != "timed_out" || last.is_pause() || last.kill_requested
    {
        return None;
    }
    let stage = match last.stage {
        Some(s @ (Stage::Implement | Stage::Fix)) => s,
        _ => return None,
    };
    let round = eff_rounds[last_idx];
    let already = f.runs.iter().enumerate().any(|(i, r)| {
        r.finalize
            && cycs[i] == cycs[last_idx]
            && eff_rounds[i] == round
            && r.stage == last.stage
            && r.fix_for == last.fix_for
    });
    if already {
        return None;
    }
    Some(StageRun {
        stage,
        round,
        fix_for: last.fix_for,
        review_input: false,
        note: Some(FINALIZE_NOTE.to_string()),
        fresh_plan: false,
        finalize: true,
    })
}

/// Quy tắc D3: Replan từ người dùng (Trả về Lead / Chạy lại Lead).
fn rule_d3_replan(
    f: &Facts,
    current_cycle: i64,
    cycs: &[i64],
    eff_rounds: &[i64],
) -> Option<StageRun> {
    let rp = f.replan.as_ref()?;
    // Lead sau vòng build/QA mở chu kỳ mới — vòng tính lại từ 1 (WR-03, O7).
    let opens_new_cycle = matches!(
        f.runs.last().and_then(loop_of),
        Some(Loop::Build | Loop::Qa)
    );
    let round = if opens_new_cycle {
        1
    } else {
        let highest_ok_plan_round = f
            .runs
            .iter()
            .enumerate()
            .filter(|(idx, r)| {
                cycs[*idx] == current_cycle
                    && r.is_ok()
                    && (r.stage == Some(Stage::Plan) || (r.stage.is_none() && r.role == "lead"))
            })
            .map(|(idx, _)| eff_rounds[idx])
            .max()
            .unwrap_or(0);
        1 + highest_ok_plan_round
    };

    Some(StageRun {
        stage: Stage::Plan,
        round,
        fix_for: None,
        review_input: false,
        note: rp.note.clone(),
        fresh_plan: rp.note.is_none(),
        finalize: false,
    })
}

/// Quy tắc D4: Giai đoạn lập plan (chỉ chạy khi chưa approved).
fn rule_d4_plan_phase(wf: &Workflow, f: &Facts) -> Option<Decision> {
    if (wf.has("plan") || wf.has("plan-review")) && !f.approved {
        match plan_phase(wf, f) {
            PlanPhase::Next(candidate) => Some(rule_d9_budget(candidate, f)),
            PlanPhase::Stopped(stop_info) => Some(Decision::Stop(stop_info)),
            PlanPhase::Done => None,
        }
    } else {
        None
    }
}

/// Quy tắc D5: Cổng duyệt plan (Gate) của bạn.
fn rule_d5_gate(wf: &Workflow, f: &Facts) -> Option<Decision> {
    let later = wf.has("implement") || wf.has("code-review") || wf.has("qa") || wf.has("fix");
    if later && !f.approved {
        return Some(Decision::Gate);
    }
    if !later {
        if wf.has("gate:you") && !f.approved {
            return Some(Decision::Gate);
        } else {
            return Some(Decision::Done);
        }
    }
    None
}

/// Quy tắc D6: Giai đoạn build (implement và code-review).
pub fn rule_d6_build_phase(
    wf: &Workflow,
    f: &Facts,
    current_cycle: i64,
    cycs: &[i64],
    eff_rounds: &[i64],
) -> Result<Option<Decision>, ()> {
    if !wf.has("implement") && !wf.has("code-review") {
        return Ok(None);
    }

    let ok_runs: Vec<(usize, &RunFact)> = f
        .runs
        .iter()
        .enumerate()
        .filter(|(idx, r)| cycs[*idx] == current_cycle && r.is_ok())
        .collect();

    let last_ok_build_loop = ok_runs
        .iter()
        .filter(|(_, r)| loop_of(r) == Some(Loop::Build))
        .last();

    match last_ok_build_loop {
        None => {
            if f.skip_build_once && f.code_after_plan {
                return Ok(None);
            }
            if wf.has("implement") {
                let candidate = StageRun {
                    stage: Stage::Implement,
                    round: 1,
                    fix_for: None,
                    review_input: false,
                    note: None,
                    fresh_plan: false,
                    finalize: false,
                };
                Ok(Some(rule_d9_budget(candidate, f)))
            } else if wf.has("code-review") {
                let candidate = StageRun {
                    stage: Stage::CodeReview,
                    round: 1,
                    fix_for: None,
                    review_input: false,
                    note: None,
                    fresh_plan: false,
                    finalize: false,
                };
                Ok(Some(rule_d9_budget(candidate, f)))
            } else {
                Ok(None)
            }
        }
        Some(&(idx, x)) => {
            if wf.has("code-review") {
                let r = eff_rounds[idx];
                let is_impl_or_fix = x.stage == Some(Stage::Implement)
                    || (x.stage.is_none() && x.role == "dev")
                    || (x.stage == Some(Stage::Fix) && x.fix_for != Some(FixFor::Qa));
                if is_impl_or_fix {
                    let candidate = StageRun {
                        stage: Stage::CodeReview,
                        round: r,
                        fix_for: None,
                        review_input: false,
                        note: None,
                        fresh_plan: false,
                        finalize: false,
                    };
                    return Ok(Some(rule_d9_budget(candidate, f)));
                }

                // X là CodeReview
                if x.outcome.as_deref() == Some("review:pass") {
                    // Build done, tiếp tục sang QA
                    return Ok(None);
                } else if x.outcome.as_deref() == Some("review:revise") {
                    if wf.has("implement") && wf.has_loop("build") {
                        let grant = f.grants.get("build").copied().unwrap_or(0);
                        let cap = wf.max_rounds as i64 + grant;
                        if r >= cap {
                            return Ok(Some(Decision::Stop(StopInfo {
                                reason: StopReason::HetVong,
                                loop_name: Some(Loop::Build),
                                stage: Some(Stage::CodeReview),
                                round: Some(r),
                                cap: Some(cap),
                                run_id: Some(x.id),
                                role: Some(x.role.clone()),
                                at_ms: f.now_ms,
                                detail_line: None,
                                used_ms: None,
                                used_usd: None,
                                estimated: None,
                                limit_minutes: None,
                                limit_usd: None,
                            })));
                        } else {
                            let candidate = StageRun {
                                stage: Stage::Fix,
                                round: r + 1,
                                fix_for: Some(FixFor::CodeReview),
                                review_input: false,
                                note: None,
                                fresh_plan: false,
                                finalize: false,
                            };
                            return Ok(Some(rule_d9_budget(candidate, f)));
                        }
                    } else {
                        return Ok(Some(Decision::Stop(StopInfo {
                            reason: StopReason::HetVong,
                            loop_name: Some(Loop::Build),
                            stage: Some(Stage::CodeReview),
                            round: Some(1),
                            cap: Some(1),
                            run_id: Some(x.id),
                            role: Some(x.role.clone()),
                            at_ms: f.now_ms,
                            detail_line: None,
                            used_ms: None,
                            used_usd: None,
                            estimated: None,
                            limit_minutes: None,
                            limit_usd: None,
                        })));
                    }
                } else {
                    return Ok(Some(Decision::Stop(StopInfo {
                        reason: StopReason::RunLoi,
                        loop_name: Some(Loop::Build),
                        stage: Some(Stage::CodeReview),
                        round: Some(r),
                        cap: None,
                        run_id: Some(x.id),
                        role: Some(x.role.clone()),
                        at_ms: f.now_ms,
                        detail_line: Some("Review không có kết luận".to_string()),
                        used_ms: None,
                        used_usd: None,
                        estimated: None,
                        limit_minutes: None,
                        limit_usd: None,
                    })));
                }
            } else {
                // Không có code-review -> build done
                Ok(None)
            }
        }
    }
}

/// Quy tắc D7: Giai đoạn QA và fix (QA).
fn rule_d7_qa_phase(
    wf: &Workflow,
    f: &Facts,
    current_cycle: i64,
    cycs: &[i64],
    eff_rounds: &[i64],
) -> Option<Decision> {
    if !wf.has("qa") {
        return None;
    }

    let ok_runs: Vec<(usize, &RunFact)> = f
        .runs
        .iter()
        .enumerate()
        .filter(|(idx, r)| cycs[*idx] == current_cycle && r.is_ok())
        .collect();

    let last_ok_qa_loop = ok_runs
        .iter()
        .filter(|(_, r)| loop_of(r) == Some(Loop::Qa))
        .last();

    match last_ok_qa_loop {
        None => {
            let candidate = StageRun {
                stage: Stage::Qa,
                round: 1,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: false,
                finalize: false,
            };
            Some(rule_d9_budget(candidate, f))
        }
        Some(&(idx, x)) => {
            let r = eff_rounds[idx];
            let is_qa = x.stage == Some(Stage::Qa) || (x.stage.is_none() && x.role == "qa");
            if is_qa {
                if x.outcome.as_deref() == Some("qa:pass") {
                    return Some(Decision::Done);
                } else if x.outcome.as_deref() == Some("qa:fix") {
                    if wf.has("fix") && wf.has_loop("qa") {
                        let grant = f.grants.get("qa").copied().unwrap_or(0);
                        let cap = wf.max_rounds as i64 + grant;
                        if r >= cap {
                            return Some(Decision::Stop(StopInfo {
                                reason: StopReason::HetVong,
                                loop_name: Some(Loop::Qa),
                                stage: Some(Stage::Qa),
                                round: Some(r),
                                cap: Some(cap),
                                run_id: Some(x.id),
                                role: Some(x.role.clone()),
                                at_ms: f.now_ms,
                                detail_line: None,
                                used_ms: None,
                                used_usd: None,
                                estimated: None,
                                limit_minutes: None,
                                limit_usd: None,
                            }));
                        } else {
                            let candidate = StageRun {
                                stage: Stage::Fix,
                                round: r,
                                fix_for: Some(FixFor::Qa),
                                review_input: false,
                                note: None,
                                fresh_plan: false,
                                finalize: false,
                            };
                            return Some(rule_d9_budget(candidate, f));
                        }
                    } else {
                        let detail_line = Some(format!(
                            "QA kết luận Cần sửa. Quy trình {} không có vòng sửa tự động (thiếu bước fix / vòng qa)",
                            wf.name
                        ));
                        return Some(Decision::Stop(StopInfo {
                            reason: StopReason::HetVong,
                            loop_name: Some(Loop::Qa),
                            stage: Some(Stage::Qa),
                            round: Some(1),
                            cap: Some(1),
                            run_id: Some(x.id),
                            role: Some(x.role.clone()),
                            at_ms: f.now_ms,
                            detail_line,
                            used_ms: None,
                            used_usd: None,
                            estimated: None,
                            limit_minutes: None,
                            limit_usd: None,
                        }));
                    }
                } else {
                    return Some(Decision::Stop(StopInfo {
                        reason: StopReason::RunLoi,
                        loop_name: Some(Loop::Qa),
                        stage: Some(Stage::Qa),
                        round: Some(r),
                        cap: None,
                        run_id: Some(x.id),
                        role: Some(x.role.clone()),
                        at_ms: f.now_ms,
                        detail_line: Some("QA không có kết luận".to_string()),
                        used_ms: None,
                        used_usd: None,
                        estimated: None,
                        limit_minutes: None,
                        limit_usd: None,
                    }));
                }
            } else {
                // X là Fix(Qa)
                let candidate = StageRun {
                    stage: Stage::Qa,
                    round: r + 1,
                    fix_for: None,
                    review_input: false,
                    note: None,
                    fresh_plan: false,
                    finalize: false,
                };
                Some(rule_d9_budget(candidate, f))
            }
        }
    }
}

/// Quy tắc D8: Không có QA stage -> Done sau khi build phase hoàn tất.
fn rule_d8_no_qa_phase(wf: &Workflow) -> Option<Decision> {
    if !wf.has("qa") {
        Some(Decision::Done)
    } else {
        None
    }
}

/// Quy tắc D9: Kiểm tra trần ngân sách cho ứng viên StageRun.
fn rule_d9_budget(candidate: StageRun, f: &Facts) -> Decision {
    if pricing::exhausted(&f.budget_usage, &f.budget_limits) && f.bypass_left == 0 {
        let loop_name = match candidate.stage {
            Stage::Plan | Stage::PlanReview => Some(Loop::Plan),
            Stage::Implement | Stage::CodeReview => Some(Loop::Build),
            Stage::Fix => {
                if candidate.fix_for == Some(FixFor::Qa) {
                    Some(Loop::Qa)
                } else {
                    Some(Loop::Build)
                }
            }
            Stage::Qa => Some(Loop::Qa),
        };
        Decision::Stop(StopInfo {
            reason: StopReason::HetNganSach,
            loop_name,
            stage: Some(candidate.stage),
            round: Some(candidate.round),
            cap: None,
            run_id: None,
            role: None,
            at_ms: f.now_ms,
            detail_line: None,
            used_ms: Some(f.budget_usage.used_ms),
            used_usd: Some(f.budget_usage.used_usd),
            estimated: Some(f.budget_usage.estimated_runs > 0),
            limit_minutes: Some(f.budget_limits.minutes),
            limit_usd: Some(f.budget_limits.usd),
        })
    } else {
        Decision::Run(candidate)
    }
}

/// Quyết định hành động tiếp theo của autopilot (D1 .. D9).
pub fn decide(wf: &Workflow, f: &Facts) -> Decision {
    if wf.kind == "analysis" {
        return Decision::Stop(StopInfo {
            reason: StopReason::DungBoiBan,
            loop_name: None,
            stage: None,
            round: None,
            cap: None,
            run_id: None,
            role: None,
            at_ms: f.now_ms,
            detail_line: Some(format!(
                "Quy trình \"{}\" là loại phân tích, không hỗ trợ chạy autopilot.",
                wf.name
            )),
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        });
    }

    let eff_rounds = effective_rounds(&f.runs);
    let cycs = cycles(&f.runs);
    let current_cycle = cycs.last().copied().unwrap_or(1);

    // Quy tắc D1: Stop request từ bên ngoài có độ ưu tiên cao nhất
    if let Some(dec) = rule_d1_stop_request(f, &eff_rounds) {
        return dec;
    }

    // C1: one closing Dev turn after a Dev timeout, when the workflow asks for it.
    if let Some(candidate) = rule_finalize_on_timeout(wf, f, &cycs, &eff_rounds) {
        return rule_d9_budget(candidate, f);
    }

    // Quy tắc D2: Kiểm tra lỗi của run cuối cùng (trừ khi đang resume)
    if let Some(dec) = rule_d2_last_run_failures(f, &eff_rounds) {
        return dec;
    }

    // Quy tắc D3: Replan từ người dùng (nếu có)
    if let Some(candidate) = rule_d3_replan(f, current_cycle, &cycs, &eff_rounds) {
        return rule_d9_budget(candidate, f);
    }

    // Quy tắc D4: Giai đoạn lập plan (khi wf có plan hoặc plan-review VÀ chưa được approved)
    if let Some(dec) = rule_d4_plan_phase(wf, f) {
        return dec;
    }

    // Quy tắc D5: Cổng duyệt plan (Gate)
    if let Some(dec) = rule_d5_gate(wf, f) {
        return dec;
    }

    // Quy tắc D6: Giai đoạn build (implement và code-review)
    match rule_d6_build_phase(wf, f, current_cycle, &cycs, &eff_rounds) {
        Ok(Some(dec)) => return dec,
        Ok(None) => {}
        Err(_) => {}
    }

    // Quy tắc D7: Giai đoạn QA và fix (QA)
    if let Some(dec) = rule_d7_qa_phase(wf, f, current_cycle, &cycs, &eff_rounds) {
        return dec;
    }

    // Quy tắc D8: Không có QA stage -> Done sau build phase
    if let Some(dec) = rule_d8_no_qa_phase(wf) {
        return dec;
    }

    Decision::Done
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::db::RunRow;
    use crate::workflows::load_from;

    fn test_cfg() -> Config {
        crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap()
    }

    fn dummy_run(
        id: i64,
        role: &str,
        stage: Option<&str>,
        round: Option<i64>,
        status: &str,
        outcome: Option<&str>,
        metrics_json: Option<&str>,
    ) -> RunRow {
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
            started_at: 1000 + id * 10,
            ended_at: Some(2000 + id * 10),
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
            stage: stage.map(|s| s.to_string()),
            round,
            metrics_json: metrics_json.map(|s| s.to_string()),
            member_id: None,
        }
    }

    #[test]
    fn empty_task_runs_lead() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();
        let facts = build_facts(wf, &[], &auto, &aj, None, &pricing, 5000, false);

        assert_eq!(
            decide(wf, &facts),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 1,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: true,
                finalize: false,
            })
        );
    }

    /// Plan approved at "p1"; the first Dev run stopped on quota on the deepseek engine.
    fn quota_dev_rows() -> Vec<RunRow> {
        let r1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let r2 = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:approve"),
            None,
        );
        let mut r3 = dummy_run(
            3,
            "dev",
            Some("implement"),
            Some(1),
            "failed",
            Some("quota"),
            None,
        );
        r3.engine = "deepseek".to_string();
        vec![r1, r2, r3]
    }

    fn approved_auto() -> TaskAuto {
        TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn d2_quota_stops_het_quota() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let rows = quota_dev_rows();
        let facts = build_facts(
            wf,
            &rows,
            &approved_auto(),
            &AutoJson::default(),
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert_eq!(facts.runs.len(), 3, "the last quota run is kept");
        assert_eq!(facts.runs[2].engine, "deepseek");

        match decide(wf, &facts) {
            Decision::Stop(info) => {
                assert_eq!(info.reason, StopReason::HetQuota);
                assert_eq!(info.reason.as_str(), "het_quota");
                assert_eq!(info.run_id, Some(3));
                assert_eq!(info.role.as_deref(), Some("dev"));
                assert_eq!(info.stage, Some(Stage::Implement));
                assert_eq!(info.round, Some(1));
                assert_eq!(
                    info.detail_line.as_deref(),
                    Some("Run 3 (dev) trên engine deepseek dừng vì hết quota hoặc bị giới hạn tốc độ. Agent-brain không tự đổi engine.")
                );
            }
            other => panic!("expected Stop(HetQuota), got {other:?}"),
        }

        // The quota branch precedes the generic failed-run rule: the same run without the
        // quota outcome is a plain run_loi.
        let mut plain = quota_dev_rows();
        plain[2].outcome = None;
        let facts = build_facts(
            wf,
            &plain,
            &approved_auto(),
            &AutoJson::default(),
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        match decide(wf, &facts) {
            Decision::Stop(info) => assert_eq!(info.reason, StopReason::RunLoi),
            other => panic!("expected Stop(RunLoi), got {other:?}"),
        }
    }

    #[test]
    fn quota_run_dropped_on_resume() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let rows = quota_dev_rows();
        let facts = build_facts(
            wf,
            &rows,
            &approved_auto(),
            &AutoJson::default(),
            Some("p1"),
            &pricing,
            5000,
            true,
        );
        assert_eq!(
            facts.runs.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![1, 2],
            "resume drops the quota run"
        );
        match decide(wf, &facts) {
            Decision::Run(sr) => {
                assert_eq!(sr.stage, Stage::Implement);
                assert_eq!(sr.round, 1, "the stopped step keeps its round");
            }
            other => panic!("expected Run(Implement r1), got {other:?}"),
        }

        // A quota run that a later run superseded is dropped even without resume.
        let mut rows = quota_dev_rows();
        rows.push(dummy_run(
            4,
            "dev",
            Some("implement"),
            None,
            "succeeded",
            None,
            None,
        ));
        let facts = build_facts(
            wf,
            &rows,
            &approved_auto(),
            &AutoJson::default(),
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert_eq!(
            facts.runs.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![1, 2, 4]
        );
    }

    #[test]
    fn c2_env_down_stops_moi_truong_and_counts_no_round() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let mut rows = quota_dev_rows();
        rows[2].outcome = Some("env:down".to_string());
        rows[2].error = Some("Môi trường không còn sẵn sàng — http://127.0.0.1:1: bị từ chối kết nối".to_string());
        let facts = build_facts(
            wf,
            &rows,
            &approved_auto(),
            &AutoJson::default(),
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert!(!facts.runs[2].is_ok());
        match decide(wf, &facts) {
            Decision::Stop(info) => {
                assert_eq!(info.reason, StopReason::MoiTruongChuaSan);
                assert_eq!(info.reason.as_str(), "moi_truong_chua_san");
                assert_eq!(info.run_id, Some(3));
                assert_eq!(info.round, Some(1));
                assert!(info.detail_line.unwrap().contains("bị từ chối"));
            }
            other => panic!("expected Stop(MoiTruongChuaSan), got {other:?}"),
        }
        // Resumed: the env-down run is dropped and the step runs again with the same round.
        let facts = build_facts(
            wf,
            &rows,
            &approved_auto(),
            &AutoJson::default(),
            Some("p1"),
            &pricing,
            5000,
            true,
        );
        match decide(wf, &facts) {
            Decision::Run(sr) => {
                assert_eq!(sr.stage, Stage::Implement);
                assert_eq!(sr.round, 1);
            }
            other => panic!("expected Run(Implement r1), got {other:?}"),
        }
        // Rounds: an env-down run counts like a quota run (not at all).
        rows.push(dummy_run(4, "dev", Some("implement"), None, "succeeded", None, None));
        let facts: Vec<RunFact> = rows.iter().map(RunFact::from_row).collect();
        assert_eq!(effective_rounds(&facts)[3], 1);
    }

    #[test]
    fn effective_rounds_skip_quota() {
        let mut rows = quota_dev_rows();
        rows[2].round = None;
        rows.push(dummy_run(
            4,
            "dev",
            Some("implement"),
            None,
            "succeeded",
            None,
            None,
        ));
        rows.push(dummy_run(
            5,
            "qa",
            Some("qa"),
            None,
            "failed",
            Some("quota"),
            None,
        ));
        rows.push(dummy_run(
            6,
            "qa",
            Some("qa"),
            None,
            "succeeded",
            None,
            None,
        ));
        let facts: Vec<RunFact> = rows.iter().map(RunFact::from_row).collect();
        let eff = effective_rounds(&facts);
        assert_eq!(
            eff,
            vec![1, 1, 1, 1, 1, 1],
            "quota runs do not count: {eff:?}"
        );

        // Without the quota outcome the earlier failed run is counted as before.
        let mut counted = rows.clone();
        counted[2].outcome = None;
        counted[4].outcome = None;
        let facts: Vec<RunFact> = counted.iter().map(RunFact::from_row).collect();
        assert_eq!(effective_rounds(&facts), vec![1, 1, 1, 2, 1, 2]);
    }

    #[test]
    fn lead_then_review() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();
        let r1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let facts = build_facts(wf, &[r1], &auto, &aj, Some("p1"), &pricing, 5000, false);

        assert_eq!(
            decide(wf, &facts),
            Decision::Run(StageRun {
                stage: Stage::PlanReview,
                round: 1,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );
    }

    #[test]
    fn revise_then_lead_round2() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();
        let r1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let r2 = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:revise"),
            None,
        );
        let facts = build_facts(wf, &[r1, r2], &auto, &aj, Some("p1"), &pricing, 5000, false);

        assert_eq!(
            decide(wf, &facts),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 2,
                fix_for: None,
                review_input: true,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );
    }

    #[test]
    fn approve_then_gate() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();
        let r1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let r2 = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:approve"),
            None,
        );
        let facts = build_facts(wf, &[r1, r2], &auto, &aj, Some("p1"), &pricing, 5000, false);

        assert_eq!(decide(wf, &facts), Decision::Gate);
    }

    #[test]
    fn stale_approval_gates() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson::default();
        let r1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let r2 = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:approve"),
            None,
        );
        // plan_commit is "p2" while approved_commit is "p1"
        let facts = build_facts(wf, &[r1, r2], &auto, &aj, Some("p2"), &pricing, 5000, false);
        assert!(!facts.approved);
        assert_eq!(decide(wf, &facts), Decision::Gate);
    }

    #[test]
    fn build_facts_maps_rows() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();

        let mut r_inspect = dummy_run(1, "inspect", None, None, "succeeded", None, None);
        r_inspect.started_at = 100;
        let mut r_fix = dummy_run(
            2,
            "dev",
            Some("fix"),
            Some(1),
            "succeeded",
            None,
            Some(r#"{"fix_for":"qa"}"#),
        );
        r_fix.started_at = 200;

        let auto = TaskAuto {
            auto_stop_at: Some(3000),
            auto_stop_kind: Some("kill".to_string()),
            ..Default::default()
        };
        let aj = AutoJson {
            bypass_stages: 2,
            ..Default::default()
        };

        let facts = build_facts(
            wf,
            &[r_fix, r_inspect],
            &auto,
            &aj,
            None,
            &pricing,
            4000,
            false,
        );
        // Inspect row dropped
        assert_eq!(facts.runs.len(), 1);
        assert_eq!(facts.runs[0].id, 2);
        assert_eq!(facts.runs[0].fix_for, Some(FixFor::Qa));
        assert_eq!(facts.stop_request, Some(StopRequest::Kill));
        assert_eq!(facts.bypass_left, 2);
    }

    #[test]
    fn auto_json_round_trip() {
        let mut grants = BTreeMap::new();
        grants.insert("plan".to_string(), 1);
        let stop = StopInfo {
            reason: StopReason::HetVong,
            loop_name: Some(Loop::Plan),
            stage: Some(Stage::PlanReview),
            round: Some(3),
            cap: Some(3),
            run_id: Some(5),
            role: Some("plan-review".to_string()),
            at_ms: 12345,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        let aj = AutoJson {
            grants,
            bypass_stages: 2,
            replan: Some(Replan {
                note: Some("x".to_string()),
            }),
            stop: Some(stop),
            last_error: None,
            budget_extra_minutes: 0,
            budget_extra_usd: 0.0,
            skip_build_once: false,
            env_rerun_at: None,
            cancelled: true,
        };

        let json = aj.to_json();
        let parsed = AutoJson::parse(Some(&json));
        assert_eq!(parsed, aj);
        // auto_json written before the flag existed reads as not cancelled.
        assert!(!AutoJson::parse(Some("{\"bypass_stages\":1}")).cancelled);

        assert_eq!(AutoJson::parse(Some("not json")), AutoJson::default());
        assert_eq!(AutoJson::parse(None), AutoJson::default());
    }

    // ------------------------------------------------------------------------
    // Behavior tests (Task 2)
    // ------------------------------------------------------------------------

    #[test]
    fn build_loop() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson::default();

        let p1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let pr1 = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:approve"),
            None,
        );
        let i1 = dummy_run(
            3,
            "dev",
            Some("implement"),
            Some(1),
            "succeeded",
            None,
            None,
        );

        // approved, cycle [P1 ok, PR1 approve, I1 ok] → Run(CodeReview, 1)
        let f1 = build_facts(
            wf,
            &[p1.clone(), pr1.clone(), i1.clone()],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert_eq!(
            decide(wf, &f1),
            Decision::Run(StageRun {
                stage: Stage::CodeReview,
                round: 1,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );

        // + CR1 revise → Run(Fix(CodeReview), 2)
        let cr1 = dummy_run(
            4,
            "code-review",
            Some("code-review"),
            Some(1),
            "succeeded",
            Some("review:revise"),
            None,
        );
        let f2 = build_facts(
            wf,
            &[p1.clone(), pr1.clone(), i1.clone(), cr1.clone()],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            6000,
            false,
        );
        assert_eq!(
            decide(wf, &f2),
            Decision::Run(StageRun {
                stage: Stage::Fix,
                round: 2,
                fix_for: Some(FixFor::CodeReview),
                review_input: false,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );

        // + Fix2 ok (stage fix, fix_for code_review, round 2) → Run(CodeReview, 2)
        let fix2 = dummy_run(
            5,
            "dev",
            Some("fix"),
            Some(2),
            "succeeded",
            None,
            Some(r#"{"fix_for":"code_review"}"#),
        );
        let f3 = build_facts(
            wf,
            &[
                p1.clone(),
                pr1.clone(),
                i1.clone(),
                cr1.clone(),
                fix2.clone(),
            ],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            7000,
            false,
        );
        assert_eq!(
            decide(wf, &f3),
            Decision::Run(StageRun {
                stage: Stage::CodeReview,
                round: 2,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );

        // + CR2 pass → Run(Qa, 1)
        let cr2 = dummy_run(
            6,
            "code-review",
            Some("code-review"),
            Some(2),
            "succeeded",
            Some("review:pass"),
            None,
        );
        let f4 = build_facts(
            wf,
            &[p1, pr1, i1, cr1, fix2, cr2],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            8000,
            false,
        );
        assert_eq!(
            decide(wf, &f4),
            Decision::Run(StageRun {
                stage: Stage::Qa,
                round: 1,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );
    }

    #[test]
    fn qa_loop() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson::default();

        let p1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let pr1 = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:approve"),
            None,
        );
        let i1 = dummy_run(
            3,
            "dev",
            Some("implement"),
            Some(1),
            "succeeded",
            None,
            None,
        );
        let cr1 = dummy_run(
            4,
            "code-review",
            Some("code-review"),
            Some(1),
            "succeeded",
            Some("review:pass"),
            None,
        );

        // + Q1 qa:fix → Run(Fix(Qa), 1)
        let q1 = dummy_run(
            5,
            "qa",
            Some("qa"),
            Some(1),
            "succeeded",
            Some("qa:fix"),
            None,
        );
        let f1 = build_facts(
            wf,
            &[p1.clone(), pr1.clone(), i1.clone(), cr1.clone(), q1.clone()],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            9000,
            false,
        );
        assert_eq!(
            decide(wf, &f1),
            Decision::Run(StageRun {
                stage: Stage::Fix,
                round: 1,
                fix_for: Some(FixFor::Qa),
                review_input: false,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );

        // + FixQ1 ok → Run(Qa, 2)
        let fix_q1 = dummy_run(
            6,
            "dev",
            Some("fix"),
            Some(1),
            "succeeded",
            None,
            Some(r#"{"fix_for":"qa"}"#),
        );
        let f2 = build_facts(
            wf,
            &[
                p1.clone(),
                pr1.clone(),
                i1.clone(),
                cr1.clone(),
                q1.clone(),
                fix_q1.clone(),
            ],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            10000,
            false,
        );
        assert_eq!(
            decide(wf, &f2),
            Decision::Run(StageRun {
                stage: Stage::Qa,
                round: 2,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );

        // + Q2 qa:pass → Done
        let q2 = dummy_run(
            7,
            "qa",
            Some("qa"),
            Some(2),
            "succeeded",
            Some("qa:pass"),
            None,
        );
        let f3 = build_facts(
            wf,
            &[p1, pr1, i1, cr1, q1, fix_q1, q2],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            11000,
            false,
        );
        assert_eq!(decide(wf, &f3), Decision::Done);
    }

    #[test]
    fn caps() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();

        // plan loop with three revises (P1 PR1 P2 PR2 P3 PR3, all revise)
        let p1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let pr1 = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:revise"),
            None,
        );
        let p2 = dummy_run(3, "lead", Some("plan"), Some(2), "succeeded", None, None);
        let pr2 = dummy_run(
            4,
            "plan-review",
            Some("plan-review"),
            Some(2),
            "succeeded",
            Some("review:revise"),
            None,
        );
        let p3 = dummy_run(5, "lead", Some("plan"), Some(3), "succeeded", None, None);
        let pr3 = dummy_run(
            6,
            "plan-review",
            Some("plan-review"),
            Some(3),
            "succeeded",
            Some("review:revise"),
            None,
        );

        let f1 = build_facts(
            wf,
            &[
                p1.clone(),
                pr1.clone(),
                p2.clone(),
                pr2.clone(),
                p3.clone(),
                pr3.clone(),
            ],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            12000,
            false,
        );
        assert_eq!(
            decide(wf, &f1),
            Decision::Stop(StopInfo {
                reason: StopReason::HetVong,
                loop_name: Some(Loop::Plan),
                stage: Some(Stage::PlanReview),
                round: Some(3),
                cap: Some(3),
                run_id: Some(6),
                role: Some("plan-review".to_string()),
                at_ms: 12000,
                detail_line: None,
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );

        // with grants {"plan":1} → Run(Plan, 4, review_input true)
        let mut grants = BTreeMap::new();
        grants.insert("plan".to_string(), 1);
        let aj_grant = AutoJson {
            grants,
            ..Default::default()
        };
        let f2 = build_facts(
            wf,
            &[p1, pr1, p2, pr2, p3, pr3],
            &auto,
            &aj_grant,
            Some("p1"),
            &pricing,
            13000,
            false,
        );
        assert_eq!(
            decide(wf, &f2),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 4,
                fix_for: None,
                review_input: true,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );

        // build loop CR3 revise at cap → Stop{HetVong, Build, round 3, cap 3}
        let auto_app = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let cr3 = dummy_run(
            10,
            "code-review",
            Some("code-review"),
            Some(3),
            "succeeded",
            Some("review:revise"),
            None,
        );
        let f_build_cap = build_facts(
            wf,
            &[cr3],
            &auto_app,
            &aj,
            Some("p1"),
            &pricing,
            14000,
            false,
        );
        assert_eq!(
            decide(wf, &f_build_cap),
            Decision::Stop(StopInfo {
                reason: StopReason::HetVong,
                loop_name: Some(Loop::Build),
                stage: Some(Stage::CodeReview),
                round: Some(3),
                cap: Some(3),
                run_id: Some(10),
                role: Some("code-review".to_string()),
                at_ms: 14000,
                detail_line: None,
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );

        // qa loop Q3 qa:fix at cap → Stop{HetVong, Qa, round 3, cap 3}
        let cr_pass = dummy_run(
            14,
            "code-review",
            Some("code-review"),
            Some(1),
            "succeeded",
            Some("review:pass"),
            None,
        );
        let q3 = dummy_run(
            15,
            "qa",
            Some("qa"),
            Some(3),
            "succeeded",
            Some("qa:fix"),
            None,
        );
        let f_qa_cap = build_facts(
            wf,
            &[cr_pass, q3],
            &auto_app,
            &aj,
            Some("p1"),
            &pricing,
            15000,
            false,
        );
        assert_eq!(
            decide(wf, &f_qa_cap),
            Decision::Stop(StopInfo {
                reason: StopReason::HetVong,
                loop_name: Some(Loop::Qa),
                stage: Some(Stage::Qa),
                round: Some(3),
                cap: Some(3),
                run_id: Some(15),
                role: Some("qa".to_string()),
                at_ms: 15000,
                detail_line: None,
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );
    }

    #[test]
    fn no_fourth_round() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();

        let mut runs = Vec::new();
        for r in 1..=3 {
            runs.push(dummy_run(
                r * 2 - 1,
                "lead",
                Some("plan"),
                Some(r),
                "succeeded",
                None,
                None,
            ));
            runs.push(dummy_run(
                r * 2,
                "plan-review",
                Some("plan-review"),
                Some(r),
                "succeeded",
                Some("review:revise"),
                None,
            ));
        }

        let facts = build_facts(wf, &runs, &auto, &aj, Some("p1"), &pricing, 20000, false);
        let d = decide(wf, &facts);
        // Không bao giờ trả về Run(Plan, 4) khi không có grant
        assert_ne!(
            d,
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 4,
                fix_for: None,
                review_input: true,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );
        assert!(matches!(
            d,
            Decision::Stop(StopInfo {
                reason: StopReason::HetVong,
                ..
            })
        ));
    }

    #[test]
    fn nhanh_and_o4() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let nhanh_wf = loaded.get("nhanh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto_app = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson::default();

        let p1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        // [P1] → Run(Implement, 1)
        let f1 = build_facts(
            nhanh_wf,
            &[p1.clone()],
            &auto_app,
            &aj,
            Some("p1"),
            &pricing,
            1000,
            false,
        );
        assert_eq!(
            decide(nhanh_wf, &f1),
            Decision::Run(StageRun {
                stage: Stage::Implement,
                round: 1,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );

        // [P1, I1] → Run(Qa, 1)
        let i1 = dummy_run(
            2,
            "dev",
            Some("implement"),
            Some(1),
            "succeeded",
            None,
            None,
        );
        let f2 = build_facts(
            nhanh_wf,
            &[p1.clone(), i1.clone()],
            &auto_app,
            &aj,
            Some("p1"),
            &pricing,
            2000,
            false,
        );
        assert_eq!(
            decide(nhanh_wf, &f2),
            Decision::Run(StageRun {
                stage: Stage::Qa,
                round: 1,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );

        // [P1, I1, Q1 qa:fix] → Stop{HetVong, Qa, round 1, cap 1}
        let q1_fix = dummy_run(
            3,
            "qa",
            Some("qa"),
            Some(1),
            "succeeded",
            Some("qa:fix"),
            None,
        );
        let f3 = build_facts(
            nhanh_wf,
            &[p1.clone(), i1.clone(), q1_fix],
            &auto_app,
            &aj,
            Some("p1"),
            &pricing,
            3000,
            false,
        );
        assert_eq!(
            decide(nhanh_wf, &f3),
            Decision::Stop(StopInfo {
                reason: StopReason::HetVong,
                loop_name: Some(Loop::Qa),
                stage: Some(Stage::Qa),
                round: Some(1),
                cap: Some(1),
                run_id: Some(3),
                role: Some("qa".to_string()),
                at_ms: 3000,
                detail_line: Some(
                    "QA kết luận Cần sửa. Quy trình nhanh không có vòng sửa tự động (thiếu bước fix / vòng qa)"
                        .to_string(),
                ),
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );

        // Q1 qa:pass → Done
        let q1_pass = dummy_run(
            3,
            "qa",
            Some("qa"),
            Some(1),
            "succeeded",
            Some("qa:pass"),
            None,
        );
        let f4 = build_facts(
            nhanh_wf,
            &[p1, i1, q1_pass],
            &auto_app,
            &aj,
            Some("p1"),
            &pricing,
            4000,
            false,
        );
        assert_eq!(decide(nhanh_wf, &f4), Decision::Done);

        // Workflow {stages ["code-review"], loops []} approved → Run(CodeReview, 1), revise → Stop{HetVong, Build, 1, 1}
        let o4_wf = Workflow {
            name: "o4-test".to_string(),
            kind: "code".to_string(),
            stages: vec!["code-review".to_string()],
            loops: vec![],
            max_rounds: 3,
            budget_minutes: 60,
            budget_usd: 20.0,
            auto: false,
            review: false,
            source: "test".to_string(),
            timeouts: Default::default(),
            finalize_on_timeout: Default::default(),
        };
        let f_o4_1 = build_facts(
            &o4_wf,
            &[],
            &auto_app,
            &aj,
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert_eq!(
            decide(&o4_wf, &f_o4_1),
            Decision::Run(StageRun {
                stage: Stage::CodeReview,
                round: 1,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );

        let cr_rev = dummy_run(
            1,
            "code-review",
            Some("code-review"),
            Some(1),
            "succeeded",
            Some("review:revise"),
            None,
        );
        let f_o4_2 = build_facts(
            &o4_wf,
            &[cr_rev],
            &auto_app,
            &aj,
            Some("p1"),
            &pricing,
            6000,
            false,
        );
        assert_eq!(
            decide(&o4_wf, &f_o4_2),
            Decision::Stop(StopInfo {
                reason: StopReason::HetVong,
                loop_name: Some(Loop::Build),
                stage: Some(Stage::CodeReview),
                round: Some(1),
                cap: Some(1),
                run_id: Some(1),
                role: Some("code-review".to_string()),
                at_ms: 6000,
                detail_line: None,
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );
    }

    #[test]
    fn gate_has_priority_over_build() {
        let custom_wf = Workflow {
            name: "no-gate-token".to_string(),
            kind: "code".to_string(),
            stages: vec![
                "plan".to_string(),
                "implement".to_string(),
                "qa".to_string(),
            ],
            loops: vec![],
            max_rounds: 3,
            budget_minutes: 60,
            budget_usd: 20.0,
            auto: false,
            review: false,
            source: "test".to_string(),
            timeouts: Default::default(),
            finalize_on_timeout: Default::default(),
        };
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();
        let p1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);

        let facts = build_facts(
            &custom_wf,
            &[p1],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert_eq!(decide(&custom_wf, &facts), Decision::Gate);
    }

    #[test]
    fn approval_overrides_plan_stop() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson::default();

        let mut runs = Vec::new();
        for r in 1..=3 {
            runs.push(dummy_run(
                r * 2 - 1,
                "lead",
                Some("plan"),
                Some(r),
                "succeeded",
                None,
                None,
            ));
            runs.push(dummy_run(
                r * 2,
                "plan-review",
                Some("plan-review"),
                Some(r),
                "succeeded",
                Some("review:revise"),
                None,
            ));
        }

        // approved and PR3 revise at cap → Run(Implement, 1)
        let facts = build_facts(wf, &runs, &auto, &aj, Some("p1"), &pricing, 20000, false);
        assert_eq!(
            decide(wf, &facts),
            Decision::Run(StageRun {
                stage: Stage::Implement,
                round: 1,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );
    }

    #[test]
    fn stop_request_first() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto_soft = TaskAuto {
            auto_stop_at: Some(3000),
            auto_stop_kind: Some("soft".to_string()),
            ..Default::default()
        };
        let aj = AutoJson::default();
        let p1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);

        let facts = build_facts(
            wf,
            &[p1],
            &auto_soft,
            &aj,
            Some("p1"),
            &pricing,
            4000,
            false,
        );
        let d = decide(wf, &facts);
        assert!(matches!(
            d,
            Decision::Stop(StopInfo {
                reason: StopReason::DungBoiBan,
                ..
            })
        ));
    }

    #[test]
    fn agy_empty_stops_with_vuong_mac() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson::default();

        let r = dummy_run(
            6,
            "dev",
            Some("implement"),
            Some(1),
            "succeeded",
            Some("agy:empty"),
            None,
        );
        assert!(!RunFact::from_row(&r).is_ok());
        let f = build_facts(wf, &[r], &auto, &aj, Some("p1"), &pricing, 9500, false);
        assert_eq!(
            decide(wf, &f),
            Decision::Stop(StopInfo {
                reason: StopReason::VuongMac,
                loop_name: Some(Loop::Build),
                stage: Some(Stage::Implement),
                round: Some(1),
                cap: None,
                run_id: Some(6),
                role: Some("dev".to_string()),
                at_ms: 9500,
                detail_line: Some(
                    "agy không sửa file nào — có thể bị từ chối quyền (xem stderr.txt của run)."
                        .to_string()
                ),
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );
    }

    #[test]
    fn agy_empty_lead_stops_with_lead_message() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson::default();

        let r = dummy_run(
            7,
            "lead",
            Some("plan"),
            Some(1),
            "succeeded",
            Some("agy:empty"),
            None,
        );
        assert!(!RunFact::from_row(&r).is_ok());
        let f = build_facts(wf, &[r], &auto, &aj, Some("p1"), &pricing, 9600, false);
        assert_eq!(
            decide(wf, &f),
            Decision::Stop(StopInfo {
                reason: StopReason::VuongMac,
                loop_name: Some(Loop::Plan),
                stage: Some(Stage::Plan),
                round: Some(1),
                cap: None,
                run_id: Some(7),
                role: Some("lead".to_string()),
                at_ms: 9600,
                detail_line: Some(crate::roles::lead::LEAD_AGY_EMPTY_MESSAGE.to_string()),
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );
    }

    #[test]
    fn d2_dsh_empty_stops_vuong_mac() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson::default();

        let r = dummy_run(
            6,
            "dev",
            Some("implement"),
            Some(1),
            "succeeded",
            Some("dsh:empty"),
            None,
        );
        assert!(!RunFact::from_row(&r).is_ok());
        let f = build_facts(wf, &[r], &auto, &aj, Some("p1"), &pricing, 9500, false);
        assert_eq!(
            decide(wf, &f),
            Decision::Stop(StopInfo {
                reason: StopReason::VuongMac,
                loop_name: Some(Loop::Build),
                stage: Some(Stage::Implement),
                round: Some(1),
                cap: None,
                run_id: Some(6),
                role: Some("dev".to_string()),
                at_ms: 9500,
                detail_line: Some(crate::roles::dev::DSH_EMPTY_MESSAGE.to_string()),
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );
    }

    #[test]
    fn last_run_failures() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson::default();

        // killed → DungBoiBan
        let r_killed = dummy_run(1, "dev", Some("implement"), Some(1), "killed", None, None);
        let f1 = build_facts(
            wf,
            &[r_killed],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert_eq!(
            decide(wf, &f1),
            Decision::Stop(StopInfo {
                reason: StopReason::DungBoiBan,
                loop_name: Some(Loop::Build),
                stage: Some(Stage::Implement),
                round: Some(1),
                cap: None,
                run_id: Some(1),
                role: Some("dev".to_string()),
                at_ms: 5000,
                detail_line: None,
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );

        // failed / timed_out / lost → RunLoi
        let r_failed = dummy_run(2, "dev", Some("implement"), Some(1), "failed", None, None);
        let f2 = build_facts(
            wf,
            &[r_failed],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            6000,
            false,
        );
        assert_eq!(
            decide(wf, &f2),
            Decision::Stop(StopInfo {
                reason: StopReason::RunLoi,
                loop_name: Some(Loop::Build),
                stage: Some(Stage::Implement),
                round: Some(1),
                cap: None,
                run_id: Some(2),
                role: Some("dev".to_string()),
                at_ms: 6000,
                detail_line: None,
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );

        // succeeded + protocol_violation → ViPhamGiaoThuc
        let r_proto = dummy_run(
            3,
            "dev",
            Some("implement"),
            Some(1),
            "succeeded",
            Some("protocol_violation"),
            None,
        );
        let f3 = build_facts(
            wf,
            &[r_proto],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            7000,
            false,
        );
        assert_eq!(
            decide(wf, &f3),
            Decision::Stop(StopInfo {
                reason: StopReason::ViPhamGiaoThuc,
                loop_name: Some(Loop::Build),
                stage: Some(Stage::Implement),
                round: Some(1),
                cap: None,
                run_id: Some(3),
                role: Some("dev".to_string()),
                at_ms: 7000,
                detail_line: None,
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );

        // succeeded + dev:blocked → VuongMac
        let r_blocked = dummy_run(
            4,
            "dev",
            Some("implement"),
            Some(1),
            "succeeded",
            Some("dev:blocked"),
            None,
        );
        let f4 = build_facts(
            wf,
            &[r_blocked],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            8000,
            false,
        );
        assert_eq!(
            decide(wf, &f4),
            Decision::Stop(StopInfo {
                reason: StopReason::VuongMac,
                loop_name: Some(Loop::Build),
                stage: Some(Stage::Implement),
                round: Some(1),
                cap: None,
                run_id: Some(4),
                role: Some("dev".to_string()),
                at_ms: 8000,
                detail_line: None,
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );

        // running → RunLoi with detail_line
        let r_running = dummy_run(5, "dev", Some("implement"), Some(1), "running", None, None);
        let f5 = build_facts(
            wf,
            &[r_running],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            9000,
            false,
        );
        assert_eq!(
            decide(wf, &f5),
            Decision::Stop(StopInfo {
                reason: StopReason::RunLoi,
                loop_name: Some(Loop::Build),
                stage: Some(Stage::Implement),
                round: Some(1),
                cap: None,
                run_id: Some(5),
                role: Some("dev".to_string()),
                at_ms: 9000,
                detail_line: Some("Run 5 vẫn đang chạy".to_string()),
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );
    }

    #[test]
    fn resume_retries_same_round() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();

        let p1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let pr1 = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:revise"),
            None,
        );
        let p2_killed = dummy_run(3, "lead", Some("plan"), Some(2), "killed", None, None);

        // with resume false → Stop{DungBoiBan}
        let f_no_resume = build_facts(
            wf,
            &[p1.clone(), pr1.clone(), p2_killed.clone()],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert!(matches!(
            decide(wf, &f_no_resume),
            Decision::Stop(StopInfo {
                reason: StopReason::DungBoiBan,
                ..
            })
        ));

        // with resume true → Run(Plan, 2, review_input true)
        let f_resume = build_facts(
            wf,
            &[p1, pr1, p2_killed],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            5000,
            true,
        );
        assert_eq!(
            decide(wf, &f_resume),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 2,
                fix_for: None,
                review_input: true,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );
    }

    #[test]
    fn budget() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();

        // candidate Run with used >= limit and bypass 0 → Stop{HetNganSach}
        let mut f_over = build_facts(wf, &[], &auto, &aj, None, &pricing, 5000, false);
        f_over.budget_usage.used_ms = 3_600_000; // 60 phút
        f_over.budget_usage.used_usd = 20.0;
        f_over.budget_usage.used_cents = 2000;
        f_over.budget_usage.estimated_runs = 1;

        assert_eq!(
            decide(wf, &f_over),
            Decision::Stop(StopInfo {
                reason: StopReason::HetNganSach,
                loop_name: Some(Loop::Plan),
                stage: Some(Stage::Plan),
                round: Some(1),
                cap: None,
                run_id: None,
                role: None,
                at_ms: 5000,
                detail_line: None,
                used_ms: Some(3_600_000),
                used_usd: Some(20.0),
                estimated: Some(true),
                limit_minutes: Some(60),
                limit_usd: Some(20.0),
            })
        );

        // bypass 2 → the Run
        let mut f_bypass = f_over.clone();
        f_bypass.bypass_left = 2;
        assert_eq!(
            decide(wf, &f_bypass),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 1,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: true,
                finalize: false,
            })
        );

        // at the gate with an exhausted budget → Gate
        let p1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let pr1 = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:approve"),
            None,
        );
        let mut f_gate = build_facts(
            wf,
            &[p1, pr1],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        f_gate.budget_usage.used_cents = 2500;
        assert_eq!(decide(wf, &f_gate), Decision::Gate);
    }

    #[test]
    fn budget_cents_boundary() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();

        let mut f = build_facts(wf, &[], &auto, &aj, None, &pricing, 5000, false);

        // used $19.994 → Run
        f.budget_usage.used_usd = 19.994;
        f.budget_usage.used_cents = (19.994f64 * 100.0).round() as i64; // 1999
        assert!(matches!(decide(wf, &f), Decision::Run(_)));

        // used $19.995 → Stop{HetNganSach}
        f.budget_usage.used_usd = 19.995;
        f.budget_usage.used_cents = (19.995f64 * 100.0).round() as i64; // 2000
        assert!(matches!(
            decide(wf, &f),
            Decision::Stop(StopInfo {
                reason: StopReason::HetNganSach,
                ..
            })
        ));
    }

    #[test]
    fn replan() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();

        let p1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let pr1 = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:approve"),
            None,
        );

        // replan {note Some("tách bước 2")} after [P1, PR1 approve]
        let aj_note = AutoJson {
            replan: Some(Replan {
                note: Some("tách bước 2".to_string()),
            }),
            ..Default::default()
        };
        let f1 = build_facts(
            wf,
            &[p1.clone(), pr1.clone()],
            &auto,
            &aj_note,
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert_eq!(
            decide(wf, &f1),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 2,
                fix_for: None,
                review_input: false,
                note: Some("tách bước 2".to_string()),
                fresh_plan: false,
                finalize: false,
            })
        );

        // note None → fresh_plan true
        let aj_none = AutoJson {
            replan: Some(Replan { note: None }),
            ..Default::default()
        };
        let f2 = build_facts(
            wf,
            &[p1, pr1],
            &auto,
            &aj_none,
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert_eq!(
            decide(wf, &f2),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 2,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: true,
                finalize: false,
            })
        );
    }

    /// P1, PR1 approve, I1, CR1 pass, Q1 fix, F1(qa), Q2 fix, F2(qa), Q3 fix (mac-dinh).
    fn qa_exhausted_runs() -> Vec<RunRow> {
        vec![
            dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None),
            dummy_run(
                2,
                "plan-review",
                Some("plan-review"),
                Some(1),
                "succeeded",
                Some("review:approve"),
                None,
            ),
            dummy_run(
                3,
                "dev",
                Some("implement"),
                Some(1),
                "succeeded",
                None,
                None,
            ),
            dummy_run(
                4,
                "code-review",
                Some("code-review"),
                Some(1),
                "succeeded",
                Some("review:pass"),
                None,
            ),
            dummy_run(
                5,
                "qa",
                Some("qa"),
                Some(1),
                "succeeded",
                Some("qa:fix"),
                None,
            ),
            dummy_run(
                6,
                "dev",
                Some("fix"),
                Some(1),
                "succeeded",
                None,
                Some(r#"{"fix_for":"qa"}"#),
            ),
            dummy_run(
                7,
                "qa",
                Some("qa"),
                Some(2),
                "succeeded",
                Some("qa:fix"),
                None,
            ),
            dummy_run(
                8,
                "dev",
                Some("fix"),
                Some(2),
                "succeeded",
                None,
                Some(r#"{"fix_for":"qa"}"#),
            ),
            dummy_run(
                9,
                "qa",
                Some("qa"),
                Some(3),
                "succeeded",
                Some("qa:fix"),
                None,
            ),
        ]
    }

    #[test]
    fn replan_after_qa_starts_round_1() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson {
            replan: Some(Replan { note: None }),
            ..Default::default()
        };
        let runs = qa_exhausted_runs();
        let facts = build_facts(wf, &runs, &auto, &aj, Some("p1"), &pricing, 9000, false);
        assert!(facts.approved);
        assert_eq!(
            decide(wf, &facts),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 1,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: true,
                finalize: false,
            })
        );
    }

    #[test]
    fn replan_new_cycle_then_revise_continues() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        // approved_commit p1, new plan commit p2 → not approved
        let auto = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson::default();
        let mut runs = qa_exhausted_runs();
        runs.push(dummy_run(
            10,
            "lead",
            Some("plan"),
            Some(1),
            "succeeded",
            None,
            None,
        ));
        runs.push(dummy_run(
            11,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:revise"),
            None,
        ));
        let facts = build_facts(wf, &runs, &auto, &aj, Some("p2"), &pricing, 11000, false);
        assert!(!facts.approved);
        assert_eq!(
            decide(wf, &facts),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 2,
                fix_for: None,
                review_input: true,
                note: None,
                fresh_plan: false,
                finalize: false,
            })
        );
    }

    #[test]
    fn replan_after_build_starts_round_1() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson {
            replan: Some(Replan { note: None }),
            ..Default::default()
        };
        let runs = vec![
            dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None),
            dummy_run(
                2,
                "plan-review",
                Some("plan-review"),
                Some(1),
                "succeeded",
                Some("review:approve"),
                None,
            ),
            dummy_run(
                3,
                "dev",
                Some("implement"),
                Some(1),
                "succeeded",
                None,
                None,
            ),
            dummy_run(
                4,
                "code-review",
                Some("code-review"),
                Some(1),
                "succeeded",
                Some("review:revise"),
                None,
            ),
            dummy_run(
                5,
                "dev",
                Some("fix"),
                Some(2),
                "succeeded",
                None,
                Some(r#"{"fix_for":"code_review"}"#),
            ),
            dummy_run(
                6,
                "code-review",
                Some("code-review"),
                Some(2),
                "succeeded",
                Some("review:revise"),
                None,
            ),
        ];
        let facts = build_facts(wf, &runs, &auto, &aj, Some("p1"), &pricing, 7000, false);
        assert_eq!(
            decide(wf, &facts),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 1,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: true,
                finalize: false,
            })
        );
    }

    #[test]
    fn replan_in_plan_loop_keeps_counting() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson {
            replan: Some(Replan {
                note: Some("x".to_string()),
            }),
            ..Default::default()
        };
        let runs = vec![
            dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None),
            dummy_run(
                2,
                "plan-review",
                Some("plan-review"),
                Some(1),
                "succeeded",
                Some("review:approve"),
                None,
            ),
        ];
        let facts = build_facts(wf, &runs, &auto, &aj, Some("p1"), &pricing, 5000, false);
        assert_eq!(
            decide(wf, &facts),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 2,
                fix_for: None,
                review_input: false,
                note: Some("x".to_string()),
                fresh_plan: false,
                finalize: false,
            })
        );
    }

    fn lead_runs_without_commit(n: i64) -> Vec<RunRow> {
        (1..=n)
            .map(|r| dummy_run(r, "lead", Some("plan"), Some(r), "succeeded", None, None))
            .collect()
    }

    #[test]
    fn rule_a_capped_after_max_rounds() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();
        let runs = lead_runs_without_commit(3);
        let facts = build_facts(wf, &runs, &auto, &aj, None, &pricing, 8000, false);
        assert_eq!(
            decide(wf, &facts),
            Decision::Stop(StopInfo {
                reason: StopReason::RunLoi,
                loop_name: Some(Loop::Plan),
                stage: Some(Stage::Plan),
                round: Some(3),
                cap: Some(3),
                run_id: Some(3),
                role: Some("lead".to_string()),
                at_ms: 8000,
                detail_line: Some("Lead đã chạy 3 vòng mà chưa có commit plan".to_string()),
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );
    }

    #[test]
    fn rule_a_grant_allows_one_more() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let mut grants = BTreeMap::new();
        grants.insert("plan".to_string(), 1);
        let aj = AutoJson {
            grants,
            ..Default::default()
        };
        let runs = lead_runs_without_commit(3);
        let facts = build_facts(wf, &runs, &auto, &aj, None, &pricing, 8000, false);
        assert_eq!(
            decide(wf, &facts),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 4,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: true,
                finalize: false,
            })
        );
    }

    #[test]
    fn rule_a_below_cap_runs() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();
        let runs = lead_runs_without_commit(1);
        let facts = build_facts(wf, &runs, &auto, &aj, None, &pricing, 8000, false);
        assert_eq!(
            decide(wf, &facts),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 2,
                fix_for: None,
                review_input: false,
                note: None,
                fresh_plan: true,
                finalize: false,
            })
        );
    }

    #[test]
    fn rule_a_no_plan_loop_cap_1() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("nhanh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();
        let runs = lead_runs_without_commit(1);
        let facts = build_facts(wf, &runs, &auto, &aj, None, &pricing, 8000, false);
        assert_eq!(
            decide(wf, &facts),
            Decision::Stop(StopInfo {
                reason: StopReason::RunLoi,
                loop_name: Some(Loop::Plan),
                stage: Some(Stage::Plan),
                round: Some(1),
                cap: Some(1),
                run_id: Some(1),
                role: Some("lead".to_string()),
                at_ms: 8000,
                detail_line: Some("Lead đã chạy 1 vòng mà chưa có commit plan".to_string()),
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            })
        );

        // a grant allows one more Lead without a plan loop
        let mut grants = BTreeMap::new();
        grants.insert("plan".to_string(), 1);
        let aj_grant = AutoJson {
            grants,
            ..Default::default()
        };
        let f2 = build_facts(wf, &runs, &auto, &aj_grant, None, &pricing, 8000, false);
        assert!(matches!(
            decide(wf, &f2),
            Decision::Run(StageRun {
                stage: Stage::Plan,
                round: 2,
                ..
            })
        ));
    }

    fn het_vong(
        loop_name: Loop,
        stage: Stage,
        round: i64,
        cap: i64,
        run_id: i64,
        role: &str,
        at_ms: i64,
    ) -> Decision {
        Decision::Stop(StopInfo {
            reason: StopReason::HetVong,
            loop_name: Some(loop_name),
            stage: Some(stage),
            round: Some(round),
            cap: Some(cap),
            run_id: Some(run_id),
            role: Some(role.to_string()),
            at_ms,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        })
    }

    fn one_grant(key: &str) -> AutoJson {
        let mut grants = BTreeMap::new();
        grants.insert(key.to_string(), 1);
        AutoJson {
            grants,
            ..Default::default()
        }
    }

    #[test]
    fn het_vong_cap_includes_grant() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();

        // plan loop: P1..P4 / PR1..PR4 all revise, grants {"plan": 1} → 4 / 4
        let auto = TaskAuto::default();
        let mut plan_runs = Vec::new();
        for r in 1..=4 {
            plan_runs.push(dummy_run(
                r * 2 - 1,
                "lead",
                Some("plan"),
                Some(r),
                "succeeded",
                None,
                None,
            ));
            plan_runs.push(dummy_run(
                r * 2,
                "plan-review",
                Some("plan-review"),
                Some(r),
                "succeeded",
                Some("review:revise"),
                None,
            ));
        }
        let f_plan = build_facts(
            wf,
            &plan_runs,
            &auto,
            &one_grant("plan"),
            Some("p1"),
            &pricing,
            20000,
            false,
        );
        assert_eq!(
            decide(wf, &f_plan),
            het_vong(Loop::Plan, Stage::PlanReview, 4, 4, 8, "plan-review", 20000)
        );

        // build loop: CR4 revise, grants {"build": 1} → 4 / 4
        let auto_app = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let cr4 = dummy_run(
            10,
            "code-review",
            Some("code-review"),
            Some(4),
            "succeeded",
            Some("review:revise"),
            None,
        );
        let f_build = build_facts(
            wf,
            &[cr4],
            &auto_app,
            &one_grant("build"),
            Some("p1"),
            &pricing,
            21000,
            false,
        );
        assert_eq!(
            decide(wf, &f_build),
            het_vong(
                Loop::Build,
                Stage::CodeReview,
                4,
                4,
                10,
                "code-review",
                21000
            )
        );

        // qa loop: Q4 qa:fix, grants {"qa": 1} → 4 / 4
        let cr_pass = dummy_run(
            14,
            "code-review",
            Some("code-review"),
            Some(1),
            "succeeded",
            Some("review:pass"),
            None,
        );
        let q4 = dummy_run(
            15,
            "qa",
            Some("qa"),
            Some(4),
            "succeeded",
            Some("qa:fix"),
            None,
        );
        let f_qa = build_facts(
            wf,
            &[cr_pass, q4],
            &auto_app,
            &one_grant("qa"),
            Some("p1"),
            &pricing,
            22000,
            false,
        );
        assert_eq!(
            decide(wf, &f_qa),
            het_vong(Loop::Qa, Stage::Qa, 4, 4, 15, "qa", 22000)
        );
    }

    #[test]
    fn rounds_and_cycles() {
        // [L, PR, L, PR, D impl, CR, D fix(code_review), CR, Q, D fix(qa), Q, L]
        let r0 = dummy_run(1, "lead", None, None, "succeeded", None, None);
        let r1 = dummy_run(2, "plan-review", None, None, "succeeded", None, None);
        let r2 = dummy_run(3, "lead", None, None, "succeeded", None, None);
        let r3 = dummy_run(4, "plan-review", None, None, "succeeded", None, None);
        let r4 = dummy_run(5, "dev", None, None, "succeeded", None, None);
        let r5 = dummy_run(6, "code-review", None, None, "succeeded", None, None);
        let r6 = dummy_run(
            7,
            "dev",
            Some("fix"),
            None,
            "succeeded",
            None,
            Some(r#"{"fix_for":"code_review"}"#),
        );
        let r7 = dummy_run(8, "code-review", None, None, "succeeded", None, None);
        let r8 = dummy_run(9, "qa", None, None, "succeeded", None, None);
        let r9 = dummy_run(
            10,
            "dev",
            Some("fix"),
            None,
            "succeeded",
            None,
            Some(r#"{"fix_for":"qa"}"#),
        );
        let r10 = dummy_run(11, "qa", None, None, "succeeded", None, None);
        let r11 = dummy_run(12, "lead", None, None, "succeeded", None, None);

        let runs: Vec<RunFact> = vec![r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11]
            .iter()
            .map(RunFact::from_row)
            .collect();

        let cycs = cycles(&runs);
        assert_eq!(cycs, vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2]);

        let eff = effective_rounds(&runs);
        assert_eq!(eff, vec![1, 1, 2, 2, 1, 1, 2, 2, 1, 1, 2, 1]);

        // A stored round wins over the derived one
        let mut r_stored = runs.clone();
        r_stored[0].round = Some(99);
        assert_eq!(effective_rounds(&r_stored)[0], 99);

        // A plan-review that follows a QA run does not start a new cycle
        let mut runs_pr = runs.clone();
        runs_pr.truncate(11); // up to Q
        let pr_after_q = RunFact {
            id: 13,
            role: "plan-review".to_string(),
            engine: "claude".to_string(),
            stage: Some(Stage::PlanReview),
            fix_for: None,
            round: None,
            status: "succeeded".to_string(),
            outcome: None,
            kill_requested: false,
            started_at: 5000,
            error: None,
            finalize: false,
        };
        runs_pr.push(pr_after_q);
        assert_eq!(cycles(&runs_pr).last().copied(), Some(1));
    }

    #[test]
    fn gate_state_cases() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("mac-dinh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto = TaskAuto::default();
        let aj = AutoJson::default();

        // no plan commit → NoPlan
        let f_no_plan = build_facts(wf, &[], &auto, &aj, None, &pricing, 5000, false);
        assert_eq!(gate_state(wf, &f_no_plan), GateState::NoPlan);

        // plan committed and PR1 revise below cap → AwaitingReview
        let p1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let pr1_rev = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:revise"),
            None,
        );
        let f_await = build_facts(
            wf,
            &[p1.clone(), pr1_rev],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert_eq!(gate_state(wf, &f_await), GateState::AwaitingReview);

        // PR approve → Waiting
        let pr1_app = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:approve"),
            None,
        );
        let f_wait = build_facts(
            wf,
            &[p1.clone(), pr1_app],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert_eq!(gate_state(wf, &f_wait), GateState::Waiting);

        // plan loop stopped at cap → Waiting
        let mut cap_runs = Vec::new();
        for r in 1..=3 {
            cap_runs.push(dummy_run(
                r * 2 - 1,
                "lead",
                Some("plan"),
                Some(r),
                "succeeded",
                None,
                None,
            ));
            cap_runs.push(dummy_run(
                r * 2,
                "plan-review",
                Some("plan-review"),
                Some(r),
                "succeeded",
                Some("review:revise"),
                None,
            ));
        }
        let f_cap = build_facts(wf, &cap_runs, &auto, &aj, Some("p1"), &pricing, 5000, false);
        assert_eq!(gate_state(wf, &f_cap), GateState::Waiting);

        // approved → Approved
        let auto_approved = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let f_approved = build_facts(
            wf,
            &[p1.clone()],
            &auto_approved,
            &aj,
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert_eq!(gate_state(wf, &f_approved), GateState::Approved);

        // nhanh with plan committed → Waiting
        let nhanh_wf = loaded.get("nhanh").unwrap();
        let f_nhanh = build_facts(
            nhanh_wf,
            &[p1],
            &auto,
            &aj,
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        assert_eq!(gate_state(nhanh_wf, &f_nhanh), GateState::Waiting);
    }

    /// Duyệt "thẳng QA": cờ skip_build_once giữ tới lần duyệt sau, nên sau khi QA chạy
    /// autopilot không quay lại implement (B2).
    #[test]
    fn skip_build_once_stays_after_qa() {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let wf = loaded.get("nhanh").unwrap();
        let pricing = crate::config::default_pricing();
        let auto_app = TaskAuto {
            approved_commit: Some("p1".to_string()),
            ..Default::default()
        };
        let aj = AutoJson {
            skip_build_once: true,
            ..Default::default()
        };
        let p1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let facts = |runs: &[RunRow], code: bool| {
            build_facts(wf, runs, &auto_app, &aj, Some("p1"), &pricing, 2000, false)
                .with_code_after_plan(code)
        };

        // Có code sau plan: bỏ implement, chạy QA.
        match decide(wf, &facts(std::slice::from_ref(&p1), true)) {
            Decision::Run(sr) => assert_eq!(sr.stage, Stage::Qa),
            other => panic!("expected Run(Qa), got {other:?}"),
        }
        // QA đạt: xong, không quay lại implement.
        let q_pass = dummy_run(2, "qa", Some("qa"), Some(1), "succeeded", Some("qa:pass"), None);
        assert_eq!(decide(wf, &facts(&[p1.clone(), q_pass], true)), Decision::Done);
        // Chưa có code sau plan: cờ không có tác dụng, chạy implement như thường.
        match decide(wf, &facts(&[p1], false)) {
            Decision::Run(sr) => assert_eq!(sr.stage, Stage::Implement),
            other => panic!("expected Run(Implement), got {other:?}"),
        }
    }

    // ---- C1: finalize_on_timeout ----

    fn timed_out_dev_rows(extra_finalize: bool) -> Vec<RunRow> {
        let r1 = dummy_run(1, "lead", Some("plan"), Some(1), "succeeded", None, None);
        let r2 = dummy_run(
            2,
            "plan-review",
            Some("plan-review"),
            Some(1),
            "succeeded",
            Some("review:approve"),
            None,
        );
        let r3 = dummy_run(3, "dev", Some("implement"), Some(1), "timed_out", None, None);
        let mut rows = vec![r1, r2, r3];
        if extra_finalize {
            rows.push(dummy_run(
                4,
                "dev",
                Some("implement"),
                Some(1),
                "timed_out",
                None,
                Some(r#"{"continuation": true, "finalize": true}"#),
            ));
        }
        rows
    }

    fn finalize_wf(enabled: bool) -> Workflow {
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let mut wf = loaded.get("mac-dinh").unwrap().clone();
        wf.finalize_on_timeout = crate::workflows::FinalizeOnTimeout {
            enabled,
            minutes: 10,
        };
        wf
    }

    fn timed_out_facts(wf: &Workflow, rows: &[RunRow], dirty: bool) -> Facts {
        let pricing = crate::config::default_pricing();
        let mut facts = build_facts(
            wf,
            rows,
            &approved_auto(),
            &AutoJson::default(),
            Some("p1"),
            &pricing,
            5000,
            false,
        );
        facts.dev_dirty_in_plan = dirty;
        facts
    }

    #[test]
    fn finalize_on_dev_timeout_runs_one_closing_turn() {
        let wf = finalize_wf(true);
        let facts = timed_out_facts(&wf, &timed_out_dev_rows(false), true);
        assert_eq!(
            decide(&wf, &facts),
            Decision::Run(StageRun {
                stage: Stage::Implement,
                round: 1,
                fix_for: None,
                review_input: false,
                note: Some(FINALIZE_NOTE.to_string()),
                fresh_plan: false,
                finalize: true,
            })
        );
    }

    #[test]
    fn finalize_runs_once_per_round() {
        let wf = finalize_wf(true);
        let facts = timed_out_facts(&wf, &timed_out_dev_rows(true), true);
        assert!(facts.runs[3].finalize);
        match decide(&wf, &facts) {
            Decision::Stop(info) => {
                assert_eq!(info.reason, StopReason::RunLoi);
                assert_eq!(info.run_id, Some(4));
            }
            other => panic!("expected Stop(RunLoi), got {other:?}"),
        }
    }

    #[test]
    fn finalize_needs_changed_plan_files() {
        let wf = finalize_wf(true);
        let facts = timed_out_facts(&wf, &timed_out_dev_rows(false), false);
        match decide(&wf, &facts) {
            Decision::Stop(info) => assert_eq!(info.reason, StopReason::RunLoi),
            other => panic!("expected Stop(RunLoi), got {other:?}"),
        }
    }

    #[test]
    fn finalize_off_keeps_the_timeout_stop() {
        // Option missing (the built-in workflow) and option present but disabled: same stop.
        let cfg = test_cfg();
        let loaded = load_from(None, &cfg);
        let builtin = loaded.get("mac-dinh").unwrap().clone();
        assert!(!builtin.finalize_on_timeout.enabled);
        for wf in [builtin, finalize_wf(false)] {
            let facts = timed_out_facts(&wf, &timed_out_dev_rows(false), true);
            match decide(&wf, &facts) {
                Decision::Stop(info) => {
                    assert_eq!(info.reason, StopReason::RunLoi);
                    assert_eq!(info.run_id, Some(3));
                    assert_eq!(info.stage, Some(Stage::Implement));
                }
                other => panic!("expected Stop(RunLoi), got {other:?}"),
            }
        }
    }

    #[test]
    fn finalize_turn_does_not_count_as_a_round() {
        // Without stored rounds, a closing turn after a timed-out implement keeps round 1.
        let mut rows = timed_out_dev_rows(true);
        for r in rows.iter_mut() {
            r.round = None;
        }
        rows[3].status = "succeeded".to_string();
        let facts: Vec<RunFact> = rows.iter().map(RunFact::from_row).collect();
        assert_eq!(effective_rounds(&facts), vec![1, 1, 1, 1]);
    }
}
