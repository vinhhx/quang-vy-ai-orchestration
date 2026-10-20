//! Quyết định thuần (decide) của autopilot (AUTO-05).
//!
//! Hàm thuần không I/O, không gọi tiến trình, không truy vấn database,
//! không đọc đồng hồ hệ thống (thời gian được đưa vào qua `facts.now_ms`).

use crate::config::Price;
use crate::db::{RunRow, TaskAuto};
use crate::pricing::{BudgetLimits, BudgetUsage};
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
}

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
    pub stage: Option<Stage>,
    pub fix_for: Option<FixFor>,
    pub round: Option<i64>,
    pub status: String,
    pub outcome: Option<String>,
    pub kill_requested: bool,
    pub started_at: i64,
}

impl RunFact {
    pub fn from_row(row: &RunRow) -> Self {
        let stage = row.stage.as_deref().and_then(Stage::parse);
        let fix_for = row.metrics_json.as_deref().and_then(|m| {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(m) {
                v.get("fix_for")
                    .and_then(|f| f.as_str())
                    .and_then(FixFor::parse)
            } else {
                None
            }
        });
        RunFact {
            id: row.id,
            role: row.role.clone(),
            stage,
            fix_for,
            round: row.round,
            status: row.status.clone(),
            outcome: row.outcome.clone(),
            kill_requested: row.kill_requested_at.is_some(),
            started_at: row.started_at,
        }
    }

    pub fn is_ok(&self) -> bool {
        if self.status != "succeeded" {
            return false;
        }
        if let Some(ref oc) = self.outcome {
            if oc == "protocol_violation" || oc == "dev:blocked" {
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
#[derive(Debug, Clone)]
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
    let budget_limits = BudgetLimits::from_workflow(wf);

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
        return PlanPhase::Next(StageRun {
            stage: Stage::Plan,
            round: 1 + highest_ok_plan_round,
            fix_for: None,
            review_input: false,
            note: None,
            fresh_plan: true,
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
                                cap: Some(wf.max_rounds as i64),
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

/// Quyết định hành động tiếp theo của autopilot (D1 .. D9).
pub fn decide(wf: &Workflow, f: &Facts) -> Decision {
    let eff_rounds = effective_rounds(&f.runs);
    let cycs = cycles(&f.runs);
    let current_cycle = cycs.last().copied().unwrap_or(1);

    // Quy tắc D1: Stop request từ bên ngoài có độ ưu tiên cao nhất
    if f.stop_request.is_some() {
        let last_run = f.runs.last();
        let last_idx = f.runs.len().checked_sub(1);
        let round = last_idx.map(|idx| eff_rounds[idx]);
        return Decision::Stop(StopInfo {
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
        });
    }

    // Quy tắc D3: Replan từ người dùng (nếu có)
    if let Some(ref rp) = f.replan {
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

        let candidate = StageRun {
            stage: Stage::Plan,
            round: 1 + highest_ok_plan_round,
            fix_for: None,
            review_input: false,
            note: rp.note.clone(),
            fresh_plan: rp.note.is_none(),
        };
        return Decision::Run(candidate);
    }

    // Quy tắc D4: Giai đoạn lập plan (khi wf có plan hoặc plan-review VÀ chưa được approved)
    if (wf.has("plan") || wf.has("plan-review")) && !f.approved {
        match plan_phase(wf, f) {
            PlanPhase::Next(candidate) => return Decision::Run(candidate),
            PlanPhase::Stopped(stop_info) => return Decision::Stop(stop_info),
            PlanPhase::Done => {}
        }
    }

    // Quy tắc D5: Cổng duyệt plan (Gate)
    let later = wf.has("implement") || wf.has("code-review") || wf.has("qa") || wf.has("fix");
    if later && !f.approved {
        return Decision::Gate;
    }
    if !later {
        if wf.has("gate:you") && !f.approved {
            return Decision::Gate;
        } else {
            return Decision::Done;
        }
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
            })
        );
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
        };

        let json = aj.to_json();
        let parsed = AutoJson::parse(Some(&json));
        assert_eq!(parsed, aj);

        assert_eq!(AutoJson::parse(Some("not json")), AutoJson::default());
        assert_eq!(AutoJson::parse(None), AutoJson::default());
    }
}
