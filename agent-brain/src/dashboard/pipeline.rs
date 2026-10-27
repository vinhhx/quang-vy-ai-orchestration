//! Pipeline của task: thẻ giai đoạn, sổ lượt chạy và dòng tổng, suy ra thuần từ runs.

use crate::autopilot::decide::{self, FixFor, GateState, Loop, RunFact, Stage};
use crate::dashboard::taskstate;
use crate::db::{RunRow, TaskAuto, TaskRow};
use crate::pricing::{self, BudgetLimits, BudgetUsage};
use crate::workflows::Workflow;

#[derive(Clone, Debug, PartialEq)]
pub enum PlanFacts {
    Unknown,
    NoCommit,
    Commit {
        sha: String,
        at_ms: Option<i64>,
        approved_by: Option<i64>,
    },
}

pub struct PipelineFacts<'a> {
    pub task: &'a TaskRow,
    pub runs: &'a [RunRow],
    pub plan: &'a PlanFacts,
    pub current_step: &'a str,
    pub now_ms: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Counts {
    pub blocking: Option<i64>,
    pub advisory: Option<i64>,
    pub specs: Option<i64>,
    pub failing: Option<i64>,
    pub not_automatable: Option<i64>,
}

impl Counts {
    pub fn from_metrics_json(s: Option<&str>) -> Option<Self> {
        let text = s?;
        let v: serde_json::Value = serde_json::from_str(text).ok()?;
        let obj = v.as_object()?;
        let blocking = obj.get("blocking").and_then(|v| v.as_i64());
        let advisory = obj.get("advisory").and_then(|v| v.as_i64());
        let specs = obj.get("specs").and_then(|v| v.as_i64());
        let failing = obj.get("failing").and_then(|v| v.as_i64());
        let not_automatable = obj.get("not_automatable").and_then(|v| v.as_i64());

        if blocking.is_none()
            && advisory.is_none()
            && specs.is_none()
            && failing.is_none()
            && not_automatable.is_none()
        {
            None
        } else {
            Some(Counts {
                blocking,
                advisory,
                specs,
                failing,
                not_automatable,
            })
        }
    }
}

#[derive(Default)]
pub struct FlowFacts<'a> {
    pub workflow: Option<&'a Workflow>,
    pub auto: Option<&'a TaskAuto>,
    pub auto_json: Option<&'a decide::AutoJson>,
    pub gate: Option<GateState>,
    pub plan_commit_at: Option<i64>,
    pub driver_alive: bool,
    pub next_card: Option<String>,
    pub budget: Option<BudgetUsage>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct WorkflowView {
    pub name: String,
    pub summary: String,
    pub auto: bool,
    pub max_rounds: i64,
    pub budget_minutes: i64,
    pub budget_usd: f64,
    pub limits: String,
    pub loops: Vec<String>,
    pub stages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct FlowCard {
    pub key: String,
    pub title: String,
    pub tag: String,
    pub state: String,
    pub state_word: String,
    pub result: String,
    pub counts: Option<Counts>,
    pub duration_ms: Option<i64>,
    pub cost_usd: Option<f64>,
    pub has_run: bool,
    pub run_id: Option<i64>,
    /// Engine of the card's latest run, or (with `engine_planned`) the engine its role will use
    /// under the task's current team. `None` until `set_planned_engines` fills a card without runs.
    pub engine: Option<String>,
    pub engine_planned: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct FlowGroup {
    pub key: String,
    pub cards: Vec<FlowCard>,
    pub round: Option<i64>,
    pub cap: Option<i64>,
    pub stopped: bool,
    pub stop_word: Option<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct GateView {
    pub present: bool,
    pub state: String,
    pub state_word: String,
    pub result: String,
    pub at_ms: Option<i64>,
    pub note_kind: String,
    pub note: Option<String>,
    pub derived: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct BudgetView {
    pub used_ms: i64,
    pub used_minutes: i64,
    pub limit_minutes: i64,
    pub used_usd: f64,
    pub limit_usd: f64,
    pub estimated: bool,
    pub estimated_runs: i64,
    pub level: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct PipelineResponse {
    #[serde(flatten)]
    pub base: PipelineView,
    pub workflow: Option<WorkflowView>,
    pub groups: Vec<FlowGroup>,
    pub gate: GateView,
    pub current_card: Option<String>,
    pub budget: Option<BudgetView>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct PipelineView {
    pub task_id: i64,
    pub cycle: Option<i64>,
    pub current: Option<String>,
    pub cards: Cards,
    pub ledger: Ledger,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Cards {
    pub start: PointCard,
    pub plan: RoleCard,
    pub ban: PointCard,
    pub code: RoleCard,
    pub qa: RoleCard,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct PointCard {
    pub state: String,
    pub state_word: String,
    pub result: String,
    pub at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct RoleCard {
    pub role: String,
    pub state: String,
    pub state_word: String,
    pub result: String,
    pub rounds: i64,
    pub run_id: Option<i64>,
    pub duration_ms: Option<i64>,
    pub cost_usd: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Ledger {
    pub groups: Vec<LedgerGroup>,
    pub total: LedgerTotal,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct LedgerGroup {
    pub key: String,
    pub kind: String,
    pub cycle: i64,
    pub header: String,
    pub right: Option<String>,
    pub rows: Vec<LedgerRow>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct LedgerRow {
    pub run_id: i64,
    pub role: String,
    pub round: i64,
    pub label: String,
    pub state: String,
    pub verdict: String,
    /// `claude`, `deepseek`, `agy` or `người:{staff}` for a person's run.
    pub engine: String,
    pub human: bool,
    pub turns: Option<i64>,
    pub duration_ms: Option<i64>,
    pub cost_usd: Option<f64>,
    pub running: bool,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    #[serde(rename = "loop")]
    pub loop_name: String,
    pub stage: Option<String>,
    pub kind: String,
    pub outcome: Option<String>,
    pub counts: Option<Counts>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct LedgerTotal {
    pub runs: i64,
    pub duration_ms: Option<i64>,
    pub cost_usd: Option<f64>,
    pub cost_unknown: i64,
    /// Runs done by a person; their minutes are left out of `duration_ms`.
    pub human_runs: i64,
}

pub fn state_word(state: &str) -> &'static str {
    match state {
        "cho" => "chưa bắt đầu",
        "dang_chay" => "đang chạy",
        "xong" => "xong",
        "dat" => "đạt",
        "vuong_mac" => "vướng mắc",
        "can_sua" => "cần sửa",
        "vi_pham" => "vi phạm giao thức",
        "killed" => "đã dừng",
        "loi" => "lỗi",
        "cho_ban" => "chờ bạn",
        "duyet" => "duyệt",
        "sua_lai" => "sửa lại",
        "het_quota" => "hết quota",
        "cho_nguoi" => "chờ người",
        _ => "lỗi",
    }
}

/// Text of the engine that did a run: `người:{staff}` for a person's run (the staff id comes from
/// the stored `metrics_json.staff`), else `runs.engine`.
pub fn engine_text_of(run: &RunRow) -> String {
    if run.engine != crate::engine::ENGINE_HUMAN {
        return run.engine.clone();
    }
    let staff = run
        .metrics_json
        .as_deref()
        .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok())
        .and_then(|v| v.get("staff").and_then(|s| s.as_str().map(str::to_string)))
        .filter(|id| crate::staff::validate_id(id));
    match staff {
        Some(id) => format!("{}:{id}", crate::engine::ENGINE_HUMAN),
        None => crate::engine::ENGINE_HUMAN.to_string(),
    }
}

/// Fills `engine` + `engine_planned: true` on every card without a run, from the role → engine
/// map of the task's team (`claude` for every role when the task has no team). Cards with a run
/// keep the engine of their latest run.
pub fn set_planned_engines(
    resp: &mut PipelineResponse,
    planned: &std::collections::BTreeMap<String, String>,
) {
    for group in &mut resp.groups {
        for card in &mut group.cards {
            if card.has_run || card.engine.is_some() {
                continue;
            }
            let role = match card.key.as_str() {
                "plan" => "lead",
                "plan_review" | "plan-review" => "plan-review",
                "code" | "implement" | "fix" => "dev",
                "code_review" | "code-review" => "code-review",
                "qa" => "qa",
                _ => continue,
            };
            card.engine = Some(
                planned
                    .get(role)
                    .cloned()
                    .unwrap_or_else(|| crate::engine::ENGINE_CLAUDE.to_string()),
            );
            card.engine_planned = true;
        }
    }
}

/// A person's pending step (06-12 `team_json.wait` marker) shown in the pipeline without a run:
/// one synthesized ledger row (`run_id: 0`, `human: true`, state `cho_nguoi`, `started_at` = the
/// marker's `since_ms`, `duration_ms` = the time waited so far) appended to the ledger group of
/// the wait's loop, and the matching stage card (`code`, or `fix` for a fix for QA) set to
/// `cho_nguoi` / `Chờ {tên} làm` with the person as its engine. `total` is never touched: no run
/// exists, so the synthesized row is not counted (D-21, HUM-01).
pub fn apply_human_wait(
    resp: &mut PipelineResponse,
    wait: &crate::teams::HumanWait,
    staff_name: &str,
    now_ms: i64,
) {
    let is_fix = wait.stage == "fix";
    let for_review = wait.fix_for.as_deref() == Some("code_review");
    let (kind, loop_name, card_key) = if !is_fix {
        ("code", "build", "code")
    } else if for_review {
        ("sửa theo review", "build", "code")
    } else {
        ("sửa theo QA", "qa", "fix")
    };

    let ledger = &mut resp.base.ledger;
    // The marker carries the autopilot round; a manual step has none, so it follows the Dev
    // runs already on the ledger (the legacy per-role counter).
    let round = wait.round.unwrap_or_else(|| {
        1 + ledger
            .groups
            .iter()
            .flat_map(|g| g.rows.iter())
            .filter(|r| r.role == "dev")
            .count() as i64
    });
    let row = LedgerRow {
        run_id: 0,
        role: "dev".to_string(),
        round,
        label: format!("Vòng {round} · {kind} · người {staff_name}"),
        state: "cho_nguoi".to_string(),
        verdict: state_word("cho_nguoi").to_string(),
        engine: format!("{}:{}", crate::engine::ENGINE_HUMAN, wait.staff),
        human: true,
        turns: None,
        duration_ms: Some((now_ms - wait.since_ms).max(0)),
        cost_usd: None,
        running: false,
        started_at: wait.since_ms,
        ended_at: None,
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        loop_name: loop_name.to_string(),
        stage: Some(wait.stage.clone()),
        kind: kind.to_string(),
        outcome: None,
        counts: None,
    };
    let extends_last = ledger.groups.last().is_some_and(|g| g.kind == loop_name);
    if extends_last {
        if let Some(g) = ledger.groups.last_mut() {
            g.rows.push(row);
        }
    } else {
        // A new group, numbered like the derive does for any other group of the same cycle.
        let cycle = ledger.groups.last().map(|g| g.cycle).unwrap_or(1);
        let n = ledger.groups.len() + 1;
        let header = if loop_name == "qa" {
            format!("Chu kỳ {cycle} · QA")
        } else {
            format!("Chu kỳ {cycle} · build")
        };
        ledger.groups.push(LedgerGroup {
            key: format!("c{cycle}-{loop_name}-{n}"),
            kind: loop_name.to_string(),
            cycle,
            header,
            right: None,
            rows: vec![row],
        });
    }

    let result = format!("Chờ {staff_name} làm");
    let engine = format!("{}:{}", crate::engine::ENGINE_HUMAN, wait.staff);
    for group in &mut resp.groups {
        for card in group.cards.iter_mut().filter(|c| c.key == card_key) {
            card.state = "cho_nguoi".to_string();
            card.state_word = state_word("cho_nguoi").to_string();
            card.result = result.clone();
            card.engine = Some(engine.clone());
            card.engine_planned = false;
        }
    }
    if card_key == "code" {
        let code = &mut resp.base.cards.code;
        code.state = "cho_nguoi".to_string();
        code.state_word = state_word("cho_nguoi").to_string();
        code.result = result;
    }
    resp.current_card = Some(card_key.to_string());
}

pub fn derive(f: &PipelineFacts) -> PipelineView {
    derive_inner(f, false)
}

fn derive_inner(f: &PipelineFacts, staged: bool) -> PipelineView {
    // R1 Input: keep runs whose role is lead, dev, qa, plan-review, code-review (drop inspect); sort by (started_at ASC, id ASC)
    let mut valid_runs: Vec<&RunRow> = f
        .runs
        .iter()
        .filter(|r| {
            r.role == "lead"
                || r.role == "dev"
                || r.role == "qa"
                || r.role == "plan-review"
                || r.role == "code-review"
        })
        .collect();
    valid_runs.sort_by_key(|r| (r.started_at, r.id));

    let run_facts: Vec<RunFact> = valid_runs.iter().map(|r| RunFact::from_row(r)).collect();
    let eff_rounds = if staged {
        decide::effective_rounds(&run_facts)
    } else {
        Vec::new()
    };

    // R2 Round: per role, 1-based counter over the sorted runs (task-wide) for legacy
    let mut lead_round = 0i64;
    let mut dev_round = 0i64;
    let mut qa_round = 0i64;
    let mut plan_review_round = 0i64;
    let mut code_review_round = 0i64;

    let mut ledger_rows: Vec<LedgerRow> = Vec::with_capacity(valid_runs.len());
    for (idx, run) in valid_runs.iter().enumerate() {
        let rf = &run_facts[idx];
        let loop_name = decide::loop_of(rf)
            .map(|l| l.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let kind = match (rf.stage, rf.role.as_str(), rf.fix_for) {
            (Some(Stage::Plan), _, _) => "plan",
            (Some(Stage::PlanReview), _, _) => "review plan",
            (Some(Stage::Implement), _, _) => "code",
            (Some(Stage::CodeReview), _, _) => "review code",
            (Some(Stage::Qa), _, _) => "QA",
            (Some(Stage::Fix), _, Some(FixFor::CodeReview)) => "sửa theo review",
            (Some(Stage::Fix), _, _) => "sửa theo QA",
            (None, "lead", _) => "plan",
            (None, "dev", _) => "code",
            (None, "qa", _) => "QA",
            (None, "plan-review", _) => "review plan",
            (None, "code-review", _) => "review code",
            _ => "unknown",
        };

        let round = if staged {
            eff_rounds[idx]
        } else {
            match run.role.as_str() {
                "lead" => {
                    lead_round += 1;
                    lead_round
                }
                "dev" => {
                    dev_round += 1;
                    dev_round
                }
                "qa" => {
                    qa_round += 1;
                    qa_round
                }
                "plan-review" => {
                    plan_review_round += 1;
                    plan_review_round
                }
                "code-review" => {
                    code_review_round += 1;
                    code_review_round
                }
                _ => 0,
            }
        };

        // R3 Row state: taskstate::step_state(Some(run)).0, then dat when xong and outcome == Some("qa:pass")
        let (raw_state, _) = taskstate::step_state(Some(run));
        let state = if raw_state == "xong" && run.outcome.as_deref() == Some("qa:pass") {
            "dat"
        } else {
            raw_state
        };
        let verdict = state_word(state).to_string();
        let human = run.engine == crate::engine::ENGINE_HUMAN;

        // R4 Unknown fields
        let duration_ms = if run.status == "running" {
            Some((f.now_ms - run.started_at).max(0))
        } else if let Some(ended) = run.ended_at {
            Some((ended - run.started_at).max(0))
        } else {
            None
        };

        let cost_usd = if !human
            && run.status != "killed"
            && run.status != "running"
            && run.usage_source == "result"
        {
            Some(run.cost_usd)
        } else {
            None
        };

        // R5 Label
        let label = format!("Vòng {round} · {kind}");

        let counts = Counts::from_metrics_json(run.metrics_json.as_deref());

        ledger_rows.push(LedgerRow {
            run_id: run.id,
            role: run.role.clone(),
            round,
            label,
            state: state.to_string(),
            verdict,
            engine: engine_text_of(run),
            human,
            turns: if human { None } else { run.num_turns },
            duration_ms,
            cost_usd,
            running: run.status == "running",
            started_at: run.started_at,
            ended_at: run.ended_at,
            input_tokens: run.input_tokens,
            output_tokens: run.output_tokens,
            cache_read_tokens: run.cache_read_tokens,
            cache_write_tokens: run.cache_write_tokens,
            loop_name,
            stage: run.stage.clone(),
            kind: kind.to_string(),
            outcome: run.outcome.clone(),
            counts,
        });
    }

    // R6 Groups: maximal runs of consecutive rows with the same loop_name
    struct GroupIndices {
        kind: String,
        run_indices: Vec<usize>,
    }

    let mut raw_groups: Vec<GroupIndices> = Vec::new();
    for (idx, row) in ledger_rows.iter().enumerate() {
        let loop_name = row.loop_name.clone();
        if let Some(last) = raw_groups.last_mut() {
            if last.kind == loop_name {
                last.run_indices.push(idx);
                continue;
            }
        }
        raw_groups.push(GroupIndices {
            kind: loop_name,
            run_indices: vec![idx],
        });
    }

    // R7 Cycle: starts at 1; plan group whose prev group is build or qa increments it,
    // provided its first row is not a plan-review run
    let mut cycle_counter = 1i64;
    let mut groups: Vec<LedgerGroup> = Vec::with_capacity(raw_groups.len());

    for (i, rg) in raw_groups.iter().enumerate() {
        let n = (i + 1) as i64;
        if i > 0 && rg.kind == "plan" {
            let prev_kind = &raw_groups[i - 1].kind;
            if prev_kind == "build" || prev_kind == "qa" {
                let first_row = &ledger_rows[rg.run_indices[0]];
                let is_plan_review = first_row.role == "plan-review"
                    || first_row.stage.as_deref() == Some("plan-review");
                if !is_plan_review {
                    cycle_counter += 1;
                }
            }
        }
        let cycle = cycle_counter;

        // R8 Header
        let header = match rg.kind.as_str() {
            "plan" => {
                if cycle == 1 {
                    "Chu kỳ 1 · plan đầu tiên".to_string()
                } else {
                    format!("Chu kỳ {cycle} · plan lại")
                }
            }
            "build" => format!("Chu kỳ {cycle} · build"),
            "qa" => format!("Chu kỳ {cycle} · QA"),
            _ => "".to_string(),
        };

        // R9 Header right (plan groups only, else None)
        let right = if rg.kind == "plan" {
            let last_idx = *rg.run_indices.last().unwrap();
            let mut dev_found = false;
            let mut lead_found = false;
            for after_run in &valid_runs[(last_idx + 1)..] {
                if after_run.role == "dev" {
                    if !lead_found {
                        dev_found = true;
                        break;
                    }
                } else if after_run.role == "lead" {
                    lead_found = true;
                }
            }

            if dev_found {
                Some("bạn đã duyệt".to_string())
            } else {
                let is_last_plan_group = raw_groups.iter().skip(i + 1).all(|g| g.kind != "plan");
                let last_run = valid_runs[last_idx];
                let dev_after = valid_runs[(last_idx + 1)..].iter().any(|r| r.role == "dev");
                if is_last_plan_group && lead_ok(last_run) && !dev_after {
                    Some("chờ bạn".to_string())
                } else {
                    None
                }
            }
        } else {
            None
        };

        let group_rows: Vec<LedgerRow> = rg
            .run_indices
            .iter()
            .map(|&idx| ledger_rows[idx].clone())
            .collect();

        groups.push(LedgerGroup {
            key: format!("c{cycle}-{}-{n}", rg.kind),
            kind: rg.kind.clone(),
            cycle,
            header,
            right,
            rows: group_rows,
        });
    }

    let current_cycle = groups.last().map(|g| g.cycle);

    // R10 Total
    let mut total_duration = 0i64;
    let mut has_known_duration = false;
    let mut total_cost = 0.0f64;
    let mut has_known_cost = false;
    let mut cost_unknown = 0i64;
    let mut human_runs = 0i64;

    for row in &ledger_rows {
        // A person's run has no tokens and no price: it is counted, but its minutes are not
        // agent minutes and its missing cost is not an unknown one.
        if row.human {
            human_runs += 1;
            continue;
        }
        if let Some(dur) = row.duration_ms {
            total_duration += dur;
            has_known_duration = true;
        }
        if let Some(cost) = row.cost_usd {
            total_cost += cost;
            has_known_cost = true;
        } else {
            cost_unknown += 1;
        }
    }

    let total = LedgerTotal {
        runs: ledger_rows.len() as i64,
        duration_ms: if has_known_duration {
            Some(total_duration)
        } else {
            None
        },
        cost_usd: if has_known_cost {
            Some(total_cost)
        } else {
            None
        },
        cost_unknown,
        human_runs,
    };

    // R13 Start card
    let start = PointCard {
        state: "xong".to_string(),
        state_word: "xong".to_string(),
        result: "bạn đã giao".to_string(),
        at_ms: Some(f.task.created_at),
    };

    // R14 Bạn card
    let ban = derive_ban_card(f, &valid_runs);

    // R11 & R12 Role cards (plan, code, qa)
    let plan = derive_role_card("lead", f, &valid_runs, &groups, current_cycle);
    let code = derive_role_card("dev", f, &valid_runs, &groups, current_cycle);
    let qa = derive_role_card("qa", f, &valid_runs, &groups, current_cycle);

    // R15 current
    let current = if valid_runs.is_empty() {
        None
    } else {
        match f.current_step {
            "lead" => Some("plan".to_string()),
            "qa" => Some("qa".to_string()),
            "dev" => {
                if ban.state == "cho_ban" {
                    Some("ban".to_string())
                } else {
                    Some("code".to_string())
                }
            }
            _ => Some("plan".to_string()),
        }
    };

    PipelineView {
        task_id: f.task.id,
        cycle: current_cycle,
        current,
        cards: Cards {
            start,
            plan,
            ban,
            code,
            qa,
        },
        ledger: Ledger { groups, total },
    }
}

fn run_matches_card(r: &RunRow, card_key: &str) -> bool {
    let rf = RunFact::from_row(r);
    match card_key {
        "plan" => rf.stage == Some(Stage::Plan) || (rf.stage.is_none() && rf.role == "lead"),
        "plan_review" => {
            rf.stage == Some(Stage::PlanReview) || (rf.stage.is_none() && rf.role == "plan-review")
        }
        "code" => {
            rf.stage == Some(Stage::Implement)
                || (rf.stage.is_none() && rf.role == "dev")
                || (rf.stage == Some(Stage::Fix) && rf.fix_for == Some(FixFor::CodeReview))
        }
        "code_review" => {
            rf.stage == Some(Stage::CodeReview) || (rf.stage.is_none() && rf.role == "code-review")
        }
        "qa" => rf.stage == Some(Stage::Qa) || (rf.stage.is_none() && rf.role == "qa"),
        "fix" => {
            rf.stage == Some(Stage::Fix) && (rf.fix_for == Some(FixFor::Qa) || rf.fix_for.is_none())
        }
        _ => false,
    }
}

fn run_card_key(r: &RunRow) -> &'static str {
    let rf = RunFact::from_row(r);
    match rf.stage {
        Some(Stage::Plan) => "plan",
        Some(Stage::PlanReview) => "plan_review",
        Some(Stage::Implement) => "code",
        Some(Stage::CodeReview) => "code_review",
        Some(Stage::Qa) => "qa",
        Some(Stage::Fix) => {
            if rf.fix_for == Some(FixFor::CodeReview) {
                "code"
            } else {
                "fix"
            }
        }
        None => match rf.role.as_str() {
            "lead" => "plan",
            "plan-review" => "plan_review",
            "dev" => "code",
            "code-review" => "code_review",
            "qa" => "qa",
            _ => "plan",
        },
    }
}

fn card_result(
    key: &str,
    state: &str,
    latest_run: Option<&RunRow>,
    f: &PipelineFacts,
    x: &FlowFacts,
    qa_latest_run: Option<&RunRow>,
    fix_has_run_in_cycle: bool,
) -> String {
    match key {
        "plan" => match state {
            "cho" => "chưa bắt đầu".to_string(),
            "dang_chay" => "đang lập plan".to_string(),
            "xong" => {
                if matches!(x.gate, Some(GateState::NoPlan))
                    || matches!(f.plan, PlanFacts::NoCommit)
                {
                    "xong".to_string()
                } else {
                    "plan đã commit".to_string()
                }
            }
            "killed" => "đã dừng".to_string(),
            "loi" => "Lead lỗi".to_string(),
            "vi_pham" => "vi phạm giao thức".to_string(),
            "vuong_mac" => "vướng mắc".to_string(),
            "can_sua" => "cần sửa".to_string(),
            _ => state_word(state).to_string(),
        },
        "plan_review" => match state {
            "cho" => "chưa bắt đầu".to_string(),
            "dang_chay" => "đang review plan".to_string(),
            "duyet" => "duyệt".to_string(),
            "sua_lai" => "sửa lại".to_string(),
            "killed" => "đã dừng".to_string(),
            "loi" => "Review lỗi".to_string(),
            "vi_pham" => "vi phạm giao thức".to_string(),
            _ => state_word(state).to_string(),
        },
        "code" => match state {
            "cho" => "chưa bắt đầu".to_string(),
            "dang_chay" => {
                let is_cr_fix = latest_run
                    .map(|r| {
                        let rf = RunFact::from_row(r);
                        rf.stage == Some(Stage::Fix) && rf.fix_for == Some(FixFor::CodeReview)
                    })
                    .unwrap_or(false);
                if is_cr_fix {
                    "đang sửa theo review".to_string()
                } else {
                    "đang code".to_string()
                }
            }
            "xong" => {
                if let Some(n) = latest_run.and_then(|r| r.num_turns) {
                    format!("{n} lượt")
                } else {
                    "xong".to_string()
                }
            }
            "vuong_mac" => "vướng mắc".to_string(),
            "can_sua" => "cần sửa".to_string(),
            "killed" => "đã dừng".to_string(),
            "loi" => "Dev lỗi".to_string(),
            "vi_pham" => "vi phạm giao thức".to_string(),
            _ => state_word(state).to_string(),
        },
        "code_review" => match state {
            "cho" => "chưa bắt đầu".to_string(),
            "dang_chay" => "đang review code".to_string(),
            "dat" => "đạt".to_string(),
            "sua_lai" => "sửa lại".to_string(),
            "killed" => "đã dừng".to_string(),
            "loi" => "Review lỗi".to_string(),
            "vi_pham" => "vi phạm giao thức".to_string(),
            _ => state_word(state).to_string(),
        },
        "qa" => match state {
            "cho" => "chưa bắt đầu".to_string(),
            "dang_chay" => "đang kiểm tra".to_string(),
            "xong" => {
                if latest_run.and_then(|r| r.outcome.as_deref()) == Some("qa:pass") {
                    "đạt".to_string()
                } else {
                    "xong".to_string()
                }
            }
            "can_sua" => "cần sửa".to_string(),
            "vuong_mac" => "vướng mắc".to_string(),
            "killed" => "đã dừng".to_string(),
            "loi" => "QA lỗi".to_string(),
            "vi_pham" => "vi phạm giao thức".to_string(),
            _ => state_word(state).to_string(),
        },
        "fix" => match state {
            "cho" => {
                let qa_passed = qa_latest_run
                    .and_then(|r| r.outcome.as_deref())
                    .map(|oc| oc == "qa:pass")
                    .unwrap_or(false);
                if qa_passed && !fix_has_run_in_cycle {
                    "không cần sửa".to_string()
                } else {
                    "chưa bắt đầu".to_string()
                }
            }
            "dang_chay" => "đang sửa".to_string(),
            "xong" => {
                if let Some(n) = latest_run.and_then(|r| r.num_turns) {
                    format!("{n} lượt")
                } else {
                    "xong".to_string()
                }
            }
            "vuong_mac" => "vướng mắc".to_string(),
            "killed" => "đã dừng".to_string(),
            "loi" => "Dev lỗi".to_string(),
            "vi_pham" => "vi phạm giao thức".to_string(),
            _ => state_word(state).to_string(),
        },
        _ => state_word(state).to_string(),
    }
}

fn derive_gate_view(base: &PipelineView, _f: &PipelineFacts, x: &FlowFacts) -> GateView {
    if let Some(wf) = x.workflow {
        let present = wf.has("gate:you");
        if !present {
            return GateView {
                present: false,
                state: "cho".to_string(),
                state_word: "chưa bắt đầu".to_string(),
                result: "chưa có plan".to_string(),
                at_ms: None,
                note_kind: "not_open".to_string(),
                note: None,
                derived: false,
            };
        }
        let auto_state = x.auto.and_then(|a| a.auto_state.as_deref());
        let stopped_loop = x
            .auto_json
            .and_then(|aj| aj.stop.as_ref())
            .and_then(|s| s.loop_name);
        let has_plan_review = wf.has("plan-review");
        let not_open_note = if has_plan_review {
            "Cổng này mở khi plan đã commit và review plan xong."
        } else {
            "Cổng này mở khi plan đã commit."
        };

        match x.gate {
            Some(GateState::NoPlan) => GateView {
                present: true,
                state: "cho".to_string(),
                state_word: "chưa bắt đầu".to_string(),
                result: "chưa có plan".to_string(),
                at_ms: None,
                note_kind: "not_open".to_string(),
                note: Some(not_open_note.to_string()),
                derived: false,
            },
            Some(GateState::AwaitingReview) => GateView {
                present: true,
                state: "cho".to_string(),
                state_word: "chờ review plan".to_string(),
                result: "chờ review plan".to_string(),
                at_ms: None,
                note_kind: "not_open".to_string(),
                note: Some(not_open_note.to_string()),
                derived: false,
            },
            Some(GateState::Waiting) => {
                let note_kind =
                    if auto_state == Some("stopped") && stopped_loop == Some(Loop::Plan) {
                        "loop_stopped"
                    } else if wf.auto && auto_state != Some("ended") {
                        // Autopilot còn bật. Đã tắt cho task (`ended`): duyệt chỉ
                        // ghi nhận, nên dùng ghi chú của đường tay (D-15 sửa đổi).
                        "auto_waiting"
                    } else {
                        "manual_waiting"
                    };
                let money = if (wf.budget_usd.round() - wf.budget_usd).abs() < 1e-9 {
                    format!("${:.0}", wf.budget_usd)
                } else {
                    format!("${:.2}", wf.budget_usd)
                };
                let note = match note_kind {
                    "loop_stopped" => {
                        "Plan chưa qua review. Bạn có thể duyệt plan hiện tại hoặc trả về Lead."
                            .to_string()
                    }
                    "auto_waiting" => format!(
                        "Autopilot dừng ở đây để bạn duyệt plan. Duyệt plan rồi autopilot tự chạy Code, Review code, QA và Sửa, tối đa {} vòng mỗi bước, trong {} phút hoặc {}.",
                        wf.max_rounds, wf.budget_minutes, money
                    ),
                    "manual_waiting" => {
                        "Duyệt plan để ghi nhận bạn đồng ý, rồi bấm Chạy Dev. Bấm Chạy Dev trực tiếp cũng coi là đã duyệt."
                            .to_string()
                    }
                    _ => "".to_string(),
                };
                GateView {
                    present: true,
                    state: "cho_ban".to_string(),
                    state_word: "chờ bạn".to_string(),
                    result: "chờ duyệt plan".to_string(),
                    at_ms: x.plan_commit_at,
                    note_kind: note_kind.to_string(),
                    note: Some(note),
                    derived: false,
                }
            }
            Some(GateState::Approved) => GateView {
                present: true,
                state: "xong".to_string(),
                state_word: "xong".to_string(),
                result: "đã duyệt plan".to_string(),
                at_ms: x.auto.and_then(|a| a.approved_at),
                note_kind: "approved_explicit".to_string(),
                note: None,
                derived: false,
            },
            None => GateView {
                present: true,
                state: base.cards.ban.state.clone(),
                state_word: base.cards.ban.state_word.clone(),
                result: base.cards.ban.result.clone(),
                at_ms: base.cards.ban.at_ms,
                note_kind: "derived".to_string(),
                note: Some(
                    "Cổng duyệt suy ra từ dữ liệu: Dev đã chạy trên bản plan hiện tại thì coi là đã duyệt."
                        .to_string(),
                ),
                derived: true,
            },
        }
    } else {
        GateView {
            present: true,
            state: base.cards.ban.state.clone(),
            state_word: base.cards.ban.state_word.clone(),
            result: base.cards.ban.result.clone(),
            at_ms: base.cards.ban.at_ms,
            note_kind: "derived".to_string(),
            note: Some(
                "Cổng duyệt suy ra từ dữ liệu: Dev đã chạy trên bản plan hiện tại thì coi là đã duyệt."
                    .to_string(),
            ),
            derived: true,
        }
    }
}

fn derive_current_card(valid_runs: &[&RunRow], x: &FlowFacts, gate_state: &str) -> Option<String> {
    if valid_runs.is_empty() {
        return None;
    }
    // 1. the running run's card
    if let Some(running_run) = valid_runs.iter().rev().find(|r| r.status == "running") {
        return Some(run_card_key(running_run).to_string());
    }
    // 2. else the stopped stage's card
    let is_stopped = x.auto.and_then(|a| a.auto_state.as_deref()) == Some("stopped");
    if is_stopped {
        if let Some(stop) = x.auto_json.and_then(|aj| aj.stop.as_ref()) {
            let card = match stop.stage {
                Some(Stage::Plan) => Some("plan"),
                Some(Stage::PlanReview) => Some("plan_review"),
                Some(Stage::Implement) => Some("code"),
                Some(Stage::CodeReview) => Some("code_review"),
                Some(Stage::Qa) => Some("qa"),
                Some(Stage::Fix) => {
                    if stop.loop_name == Some(Loop::Build) {
                        Some("code")
                    } else {
                        Some("fix")
                    }
                }
                None => match stop.loop_name {
                    Some(Loop::Plan) => Some("plan"),
                    Some(Loop::Build) => Some("code"),
                    Some(Loop::Qa) => Some("qa"),
                    None => None,
                },
            };
            if let Some(c) = card {
                return Some(c.to_string());
            }
        }
    }
    // 3. else `ban` at a waiting gate
    if gate_state == "cho_ban" {
        return Some("ban".to_string());
    }
    // 4. else the next stage's card while a driver runs
    if x.driver_alive {
        if let Some(ref next) = x.next_card {
            return Some(next.clone());
        }
    }
    // 5. else the latest run's card
    if let Some(latest) = valid_runs.last() {
        return Some(run_card_key(latest).to_string());
    }
    None
}

pub fn derive_response(f: &PipelineFacts, x: &FlowFacts) -> PipelineResponse {
    let staged = x.workflow.is_some();
    let base = derive_inner(f, staged);

    let workflow = x.workflow.map(|wf| WorkflowView {
        name: wf.name.clone(),
        summary: wf.summary(),
        auto: wf.auto,
        max_rounds: wf.max_rounds as i64,
        budget_minutes: wf.budget_minutes as i64,
        budget_usd: wf.budget_usd,
        limits: wf.limits_text(),
        loops: wf.loops.clone(),
        stages: wf.stages.clone(),
    });

    let mut valid_runs: Vec<&RunRow> = f
        .runs
        .iter()
        .filter(|r| {
            r.role == "lead"
                || r.role == "dev"
                || r.role == "qa"
                || r.role == "plan-review"
                || r.role == "code-review"
        })
        .collect();
    valid_runs.sort_by_key(|r| (r.started_at, r.id));
    let run_facts: Vec<RunFact> = valid_runs.iter().map(|r| RunFact::from_row(r)).collect();
    let cycs = decide::cycles(&run_facts);
    let current_cycle = cycs.last().copied();
    let eff_rounds = decide::effective_rounds(&run_facts);

    let auto_state = x.auto.and_then(|a| a.auto_state.as_deref());
    let auto_stop_loop = x
        .auto_json
        .and_then(|aj| aj.stop.as_ref())
        .and_then(|s| s.loop_name)
        .map(|l| l.as_str().to_string());

    let qa_latest_run_in_cycle = current_cycle.and_then(|cur_cyc| {
        valid_runs
            .iter()
            .enumerate()
            .filter(|(idx, r)| cycs[*idx] == cur_cyc && run_matches_card(r, "qa"))
            .last()
            .map(|(_, r)| *r)
    });
    let fix_has_run_in_cycle = current_cycle
        .map(|cur_cyc| {
            valid_runs
                .iter()
                .enumerate()
                .any(|(idx, r)| cycs[idx] == cur_cyc && run_matches_card(r, "fix"))
        })
        .unwrap_or(false);

    // Group definitions:
    // canonical loop keys in order: plan, build, qa
    struct GroupDef {
        key: &'static str,
        cards: Vec<(&'static str, &'static str, &'static str, &'static str)>, // key, stage, title, tag
    }

    let group_defs = [
        GroupDef {
            key: "plan",
            cards: vec![
                ("plan", "plan", "Lên plan", "lead"),
                ("plan_review", "plan-review", "Review plan", "plan-review"),
            ],
        },
        GroupDef {
            key: "build",
            cards: vec![
                ("code", "implement", "Code", "dev"),
                ("code_review", "code-review", "Review code", "code-review"),
            ],
        },
        GroupDef {
            key: "qa",
            cards: vec![("qa", "qa", "QA", "qa"), ("fix", "fix", "Sửa", "dev · fix")],
        },
    ];

    let mut groups = Vec::new();
    for gdef in &group_defs {
        let (has_group, allowed_card_keys): (bool, Vec<&str>) = match x.workflow {
            Some(wf) => {
                let mut ckeys = Vec::new();
                for (ckey, stage, _, _) in &gdef.cards {
                    if wf.has(stage) {
                        ckeys.push(*ckey);
                    }
                }
                (!ckeys.is_empty(), ckeys)
            }
            None => {
                // Synthesized legacy flow: stages [plan, gate:you, implement, qa], no loops
                match gdef.key {
                    "plan" => (true, vec!["plan"]),
                    "build" => (true, vec!["code"]),
                    "qa" => (true, vec!["qa"]),
                    _ => (false, Vec::new()),
                }
            }
        };

        if !has_group {
            continue;
        }

        let mut flow_cards = Vec::new();
        for (ckey, _stage, title, tag) in &gdef.cards {
            if !allowed_card_keys.contains(ckey) {
                continue;
            }

            let card_runs: Vec<(usize, &RunRow)> = valid_runs
                .iter()
                .enumerate()
                .filter(|(_, r)| run_matches_card(r, ckey))
                .map(|(i, &r)| (i, r))
                .collect();

            let has_run = !card_runs.is_empty();
            let mut total_duration = 0i64;
            let mut has_duration = false;
            let mut total_cost = 0.0f64;
            let mut has_cost = false;
            for (_, r) in &card_runs {
                let dur = if r.status == "running" {
                    Some((f.now_ms - r.started_at).max(0))
                } else if let Some(ended) = r.ended_at {
                    Some((ended - r.started_at).max(0))
                } else {
                    None
                };
                if let Some(d) = dur {
                    total_duration += d;
                    has_duration = true;
                }
                if r.status != "killed" && r.status != "running" && r.usage_source == "result" {
                    total_cost += r.cost_usd;
                    has_cost = true;
                }
            }
            let duration_ms = if has_duration {
                Some(total_duration)
            } else {
                None
            };
            let cost_usd = if has_cost { Some(total_cost) } else { None };

            let latest_run_in_cycle = current_cycle.and_then(|cur_cyc| {
                card_runs
                    .iter()
                    .filter(|(idx, _)| cycs[*idx] == cur_cyc)
                    .last()
                    .map(|(_, r)| *r)
            });

            let run_id = latest_run_in_cycle.map(|r| r.id);
            let counts = latest_run_in_cycle
                .and_then(|r| Counts::from_metrics_json(r.metrics_json.as_deref()));
            let state = latest_run_in_cycle
                .map(|r| taskstate::step_state(Some(r)).0)
                .unwrap_or("cho");
            let state_word_str = state_word(state).to_string();

            let result = card_result(
                ckey,
                state,
                latest_run_in_cycle,
                f,
                x,
                qa_latest_run_in_cycle,
                fix_has_run_in_cycle,
            );

            flow_cards.push(FlowCard {
                key: ckey.to_string(),
                title: title.to_string(),
                tag: tag.to_string(),
                state: state.to_string(),
                state_word: state_word_str,
                result,
                counts,
                duration_ms,
                cost_usd,
                has_run,
                run_id,
                engine: card_runs.last().map(|(_, r)| engine_text_of(r)),
                engine_planned: false,
            });
        }

        // Group round: round of latest run of the group's cards in the current cycle
        let group_round = current_cycle.and_then(|cur_cyc| {
            valid_runs
                .iter()
                .enumerate()
                .filter(|(idx, _)| {
                    cycs[*idx] == cur_cyc
                        && decide::loop_of(&run_facts[*idx])
                            .map(|l| l.as_str() == gdef.key)
                            .unwrap_or(false)
                })
                .last()
                .map(|(idx, _)| eff_rounds[idx])
        });

        let cap = x
            .workflow
            .filter(|wf| wf.has_loop(gdef.key))
            .map(|wf| wf.max_rounds as i64);

        let stopped = auto_state == Some("stopped") && auto_stop_loop.as_deref() == Some(gdef.key);
        let stop_word = if stopped {
            x.auto_json
                .and_then(|aj| aj.stop.as_ref())
                .map(|s| crate::autopilot::view::reason_word(s.reason).to_string())
        } else {
            None
        };

        groups.push(FlowGroup {
            key: gdef.key.to_string(),
            cards: flow_cards,
            round: group_round,
            cap,
            stopped,
            stop_word,
        });
    }

    let gate = derive_gate_view(&base, f, x);
    let current_card = derive_current_card(&valid_runs, x, &gate.state);

    let budget = if let (Some(wf), Some(u)) = (x.workflow, x.budget.as_ref()) {
        let limits = BudgetLimits::from_workflow(wf);
        let level = match pricing::budget_level(u, &limits) {
            pricing::BudgetLevel::Ok => "ok",
            pricing::BudgetLevel::Near => "near",
            pricing::BudgetLevel::Over => "over",
        };
        Some(BudgetView {
            used_ms: u.used_ms,
            used_minutes: (u.used_ms as f64 / 60000.0).round() as i64,
            limit_minutes: limits.minutes,
            used_usd: u.used_usd,
            limit_usd: limits.usd,
            estimated: u.estimated_runs > 0,
            estimated_runs: u.estimated_runs,
            level: level.to_string(),
        })
    } else {
        None
    };

    PipelineResponse {
        base,
        workflow,
        groups,
        gate,
        current_card,
        budget,
    }
}

fn lead_ok(r: &RunRow) -> bool {
    taskstate::step_state(Some(r)).0 == "xong"
}

fn derive_ban_card(f: &PipelineFacts, valid_runs: &[&RunRow]) -> PointCard {
    match f.plan {
        PlanFacts::Commit {
            approved_by: Some(dev_id),
            ..
        } => {
            let at_ms = f
                .runs
                .iter()
                .find(|r| r.id == *dev_id)
                .map(|r| r.started_at);
            PointCard {
                state: "xong".to_string(),
                state_word: "xong".to_string(),
                result: "đã duyệt plan".to_string(),
                at_ms,
            }
        }
        PlanFacts::Commit {
            approved_by: None,
            at_ms,
            ..
        } => {
            let latest_succeeded_lead = valid_runs
                .iter()
                .rev()
                .find(|r| r.role == "lead" && lead_ok(r))
                .copied();
            let card_at_ms = at_ms.or_else(|| latest_succeeded_lead.and_then(|r| r.ended_at));
            PointCard {
                state: "cho_ban".to_string(),
                state_word: "chờ bạn".to_string(),
                result: "chờ duyệt plan".to_string(),
                at_ms: card_at_ms,
            }
        }
        PlanFacts::NoCommit => PointCard {
            state: "cho".to_string(),
            state_word: "chưa bắt đầu".to_string(),
            result: "chưa có plan".to_string(),
            at_ms: None,
        },
        PlanFacts::Unknown => {
            let latest_succeeded_lead = valid_runs
                .iter()
                .rev()
                .find(|r| r.role == "lead" && lead_ok(r))
                .copied();
            if let Some(lead) = latest_succeeded_lead {
                let l_end = lead.ended_at.unwrap_or(lead.started_at);
                let first_dev = valid_runs
                    .iter()
                    .find(|r| r.role == "dev" && r.started_at >= l_end);
                if let Some(dev) = first_dev {
                    PointCard {
                        state: "xong".to_string(),
                        state_word: "xong".to_string(),
                        result: "đã duyệt plan".to_string(),
                        at_ms: Some(dev.started_at),
                    }
                } else {
                    PointCard {
                        state: "cho_ban".to_string(),
                        state_word: "chờ bạn".to_string(),
                        result: "chờ duyệt plan".to_string(),
                        at_ms: lead.ended_at,
                    }
                }
            } else {
                PointCard {
                    state: "cho".to_string(),
                    state_word: "chưa bắt đầu".to_string(),
                    result: "chưa có plan".to_string(),
                    at_ms: None,
                }
            }
        }
    }
}

fn derive_role_card(
    role: &str,
    f: &PipelineFacts,
    valid_runs: &[&RunRow],
    groups: &[LedgerGroup],
    current_cycle: Option<i64>,
) -> RoleCard {
    let role_runs: Vec<&RunRow> = valid_runs
        .iter()
        .filter(|r| r.role == role)
        .copied()
        .collect();
    let rounds = role_runs.len() as i64;

    let mut total_duration = 0i64;
    let mut has_duration = false;
    let mut total_cost = 0.0f64;
    let mut has_cost = false;

    for r in &role_runs {
        let dur = if r.status == "running" {
            Some((f.now_ms - r.started_at).max(0))
        } else if let Some(ended) = r.ended_at {
            Some((ended - r.started_at).max(0))
        } else {
            None
        };
        if let Some(d) = dur {
            total_duration += d;
            has_duration = true;
        }

        if r.status != "killed" && r.status != "running" && r.usage_source == "result" {
            total_cost += r.cost_usd;
            has_cost = true;
        }
    }

    let duration_ms = if has_duration {
        Some(total_duration)
    } else {
        None
    };
    let cost_usd = if has_cost { Some(total_cost) } else { None };

    // Latest run of that role inside the current cycle
    let latest_run_id = if let Some(cyc) = current_cycle {
        groups
            .iter()
            .filter(|g| g.cycle == cyc)
            .flat_map(|g| g.rows.iter())
            .filter(|r| r.role == role)
            .last()
            .map(|r| r.run_id)
    } else {
        None
    };

    let latest_run = latest_run_id.and_then(|id| valid_runs.iter().find(|r| r.id == id).copied());

    let state = latest_run
        .map(|r| taskstate::step_state(Some(r)).0)
        .unwrap_or("cho");
    let state_word_str = state_word(state).to_string();

    let result = match role {
        "lead" => match state {
            "cho" => "chưa bắt đầu",
            "dang_chay" => "đang lập plan",
            "xong" => {
                if matches!(f.plan, PlanFacts::NoCommit) {
                    "xong"
                } else {
                    "plan đã commit"
                }
            }
            "killed" => "đã dừng",
            "loi" => "Lead lỗi",
            "vi_pham" => "vi phạm giao thức",
            "vuong_mac" => "vướng mắc",
            "can_sua" => "cần sửa",
            _ => state_word(state),
        }
        .to_string(),
        "dev" => match state {
            "cho" => "chưa bắt đầu".to_string(),
            "dang_chay" => "đang code".to_string(),
            "xong" => {
                if let Some(n) = latest_run.and_then(|r| r.num_turns) {
                    format!("{n} lượt")
                } else {
                    "xong".to_string()
                }
            }
            "vuong_mac" => "vướng mắc".to_string(),
            "can_sua" => "cần sửa".to_string(),
            "killed" => "đã dừng".to_string(),
            "loi" => "Dev lỗi".to_string(),
            "vi_pham" => "vi phạm giao thức".to_string(),
            _ => state_word(state).to_string(),
        },
        "qa" => match state {
            "cho" => "chưa bắt đầu",
            "dang_chay" => "đang kiểm tra",
            "xong" => {
                if latest_run.and_then(|r| r.outcome.as_deref()) == Some("qa:pass") {
                    "đạt"
                } else {
                    "xong"
                }
            }
            "can_sua" => "cần sửa",
            "vuong_mac" => "vướng mắc",
            "killed" => "đã dừng",
            "loi" => "QA lỗi",
            "vi_pham" => "vi phạm giao thức",
            _ => state_word(state),
        }
        .to_string(),
        _ => state_word(state).to_string(),
    };

    RoleCard {
        role: role.to_string(),
        state: state.to_string(),
        state_word: state_word_str,
        result,
        rounds,
        run_id: latest_run.map(|r| r.id),
        duration_ms,
        cost_usd,
    }
}

pub fn approving_dev_run_checked(
    runs: &[RunRow],
    plan_sha: &str,
    plan_at_ms: Option<i64>,
    ancestry: impl Fn(&str, &str) -> Option<bool>,
) -> Result<Option<i64>, ()> {
    let mut dev_runs: Vec<&RunRow> = runs.iter().filter(|r| r.role == "dev").collect();
    dev_runs.sort_by_key(|r| (r.started_at, r.id));

    for r in dev_runs {
        match r.base_commit.as_deref() {
            Some(b) if !b.is_empty() => match ancestry(plan_sha, b) {
                Some(true) => return Ok(Some(r.id)),
                Some(false) => {}
                None => return Err(()),
            },
            _ => {
                if let Some(t) = plan_at_ms {
                    if r.started_at >= t {
                        return Ok(Some(r.id));
                    }
                }
            }
        }
    }
    Ok(None)
}

pub fn approving_dev_run(
    runs: &[RunRow],
    plan_sha: &str,
    plan_at_ms: Option<i64>,
    is_ancestor: impl Fn(&str, &str) -> bool,
) -> Option<i64> {
    approving_dev_run_checked(runs, plan_sha, plan_at_ms, |a, d| Some(is_ancestor(a, d)))
        .unwrap_or(None)
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
            stage: None,
            round: None,
            metrics_json: None,
        }
    }

    fn run(
        id: i64,
        role: &str,
        status: &str,
        outcome: Option<&str>,
        started_at: i64,
        ended_at: Option<i64>,
    ) -> RunRow {
        let mut r = dummy_run(id, role, status, outcome);
        r.started_at = started_at;
        r.ended_at = ended_at;
        r
    }

    #[test]
    fn test_derive_lead_only() {
        let task = dummy_task("planned");
        let run = dummy_run(1, "lead", "succeeded", None);
        let runs = vec![run];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 3000,
        };
        let view = derive(&facts);
        assert_eq!(view.cards.plan.state, "xong");
        assert_eq!(view.cards.plan.result, "plan đã commit");
        assert_eq!(view.cards.plan.rounds, 1);
        assert_eq!(view.cards.ban.state, "cho_ban");
        assert_eq!(view.current.as_deref(), Some("ban"));
        assert_eq!(view.ledger.groups.len(), 1);
        assert_eq!(view.ledger.groups[0].header, "Chu kỳ 1 · plan đầu tiên");
        assert_eq!(view.ledger.groups[0].right.as_deref(), Some("chờ bạn"));
        assert_eq!(view.ledger.total.runs, 1);
    }

    #[test]
    fn test_round_numbers_per_role_task_wide() {
        let task = dummy_task("planned");
        let runs = vec![
            run(1, "lead", "succeeded", None, 1000, Some(2000)),
            run(2, "dev", "succeeded", None, 2000, Some(3000)),
            run(3, "qa", "succeeded", None, 3000, Some(4000)),
            run(4, "lead", "succeeded", None, 4000, Some(5000)),
            run(5, "dev", "succeeded", None, 5000, Some(6000)),
        ];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 10000,
        };
        let view = derive(&facts);
        let all_rows: Vec<&LedgerRow> = view.ledger.groups.iter().flat_map(|g| &g.rows).collect();
        assert_eq!(all_rows.len(), 5);
        assert_eq!(all_rows[0].round, 1);
        assert_eq!(all_rows[0].label, "Vòng 1 · plan");
        assert_eq!(all_rows[1].round, 1);
        assert_eq!(all_rows[1].label, "Vòng 1 · code");
        assert_eq!(all_rows[2].round, 1);
        assert_eq!(all_rows[2].label, "Vòng 1 · QA");
        assert_eq!(all_rows[3].round, 2);
        assert_eq!(all_rows[3].label, "Vòng 2 · plan");
        assert_eq!(all_rows[4].round, 2);
        assert_eq!(all_rows[4].label, "Vòng 2 · code");
    }

    #[test]
    fn test_ties_ordered_by_id() {
        let task = dummy_task("dev_done");
        let runs = vec![
            run(5, "dev", "succeeded", None, 1000, Some(2000)),
            run(3, "dev", "succeeded", None, 1000, Some(2000)),
        ];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 5000,
        };
        let view = derive(&facts);
        let all_rows: Vec<&LedgerRow> = view.ledger.groups.iter().flat_map(|g| &g.rows).collect();
        assert_eq!(all_rows.len(), 2);
        assert_eq!(all_rows[0].run_id, 3);
        assert_eq!(all_rows[1].run_id, 5);
        assert_eq!(view.cards.code.run_id, Some(5));
    }

    #[test]
    fn test_inspect_runs_excluded() {
        let task = dummy_task("planned");
        let runs = vec![
            run(1, "inspect", "succeeded", None, 1000, Some(2000)),
            run(2, "lead", "succeeded", None, 2000, Some(3000)),
        ];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 5000,
        };
        let view = derive(&facts);
        let all_rows: Vec<&LedgerRow> = view.ledger.groups.iter().flat_map(|g| &g.rows).collect();
        assert_eq!(all_rows.len(), 1);
        assert_eq!(all_rows[0].run_id, 2);
        assert_eq!(view.ledger.total.runs, 1);
        assert_eq!(view.cards.plan.rounds, 1);
    }

    #[test]
    fn test_groups_and_cycles() {
        let task = dummy_task("dev_done");
        let runs = vec![
            run(1, "lead", "succeeded", None, 1000, Some(2000)),
            run(2, "lead", "succeeded", None, 2000, Some(3000)),
            run(3, "dev", "succeeded", None, 3000, Some(4000)),
            run(4, "qa", "succeeded", None, 4000, Some(5000)),
            run(5, "dev", "succeeded", None, 5000, Some(6000)),
            run(6, "qa", "succeeded", None, 6000, Some(7000)),
            run(7, "lead", "succeeded", None, 7000, Some(8000)),
            run(8, "dev", "succeeded", None, 8000, Some(9000)),
        ];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 10000,
        };
        let view = derive(&facts);
        assert_eq!(view.ledger.groups.len(), 7);
        let kinds: Vec<&str> = view.ledger.groups.iter().map(|g| g.kind.as_str()).collect();
        assert_eq!(
            kinds,
            vec!["plan", "build", "qa", "build", "qa", "plan", "build"]
        );
        let cycles: Vec<i64> = view.ledger.groups.iter().map(|g| g.cycle).collect();
        assert_eq!(cycles, vec![1, 1, 1, 1, 1, 2, 2]);
        let headers: Vec<&str> = view
            .ledger
            .groups
            .iter()
            .map(|g| g.header.as_str())
            .collect();
        assert_eq!(
            headers,
            vec![
                "Chu kỳ 1 · plan đầu tiên",
                "Chu kỳ 1 · build",
                "Chu kỳ 1 · QA",
                "Chu kỳ 1 · build",
                "Chu kỳ 1 · QA",
                "Chu kỳ 2 · plan lại",
                "Chu kỳ 2 · build",
            ]
        );
        let keys: Vec<&str> = view.ledger.groups.iter().map(|g| g.key.as_str()).collect();
        assert_eq!(
            keys,
            vec![
                "c1-plan-1",
                "c1-build-2",
                "c1-qa-3",
                "c1-build-4",
                "c1-qa-5",
                "c2-plan-6",
                "c2-build-7",
            ]
        );
    }

    #[test]
    fn test_header_right() {
        let task = dummy_task("planned");
        // Plan followed by dev -> "bạn đã duyệt"
        let runs_followed_by_dev = vec![
            run(1, "lead", "succeeded", None, 1000, Some(2000)),
            run(2, "dev", "succeeded", None, 3000, Some(4000)),
        ];
        let v1 = derive(&PipelineFacts {
            task: &task,
            runs: &runs_followed_by_dev,
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 5000,
        });
        assert_eq!(v1.ledger.groups[0].right.as_deref(), Some("bạn đã duyệt"));
        assert_eq!(v1.ledger.groups[1].right, None);

        // Last plan group whose last lead succeeded with no dev after -> "chờ bạn"
        let runs_succeeded_last = vec![run(1, "lead", "succeeded", None, 1000, Some(2000))];
        let v2 = derive(&PipelineFacts {
            task: &task,
            runs: &runs_succeeded_last,
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 5000,
        });
        assert_eq!(v2.ledger.groups[0].right.as_deref(), Some("chờ bạn"));

        // Last plan group whose last lead failed -> null
        let runs_failed_last = vec![run(1, "lead", "failed", None, 1000, Some(2000))];
        let v3 = derive(&PipelineFacts {
            task: &task,
            runs: &runs_failed_last,
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 5000,
        });
        assert_eq!(v3.ledger.groups[0].right, None);

        // QA group -> null
        let runs_qa = vec![run(1, "qa", "succeeded", None, 1000, Some(2000))];
        let v4 = derive(&PipelineFacts {
            task: &task,
            runs: &runs_qa,
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 5000,
        });
        assert_eq!(v4.ledger.groups[0].right, None);
    }

    #[test]
    fn test_verdict_precedence() {
        let task = dummy_task("open");
        let runs = vec![
            run(1, "dev", "running", Some("dev:blocked"), 1000, None),
            run(2, "qa", "succeeded", Some("qa:pass"), 2000, Some(3000)),
            run(3, "qa", "succeeded", Some("qa:fix"), 3000, Some(4000)),
            run(4, "dev", "succeeded", Some("dev:blocked"), 4000, Some(5000)),
            run(
                5,
                "lead",
                "succeeded",
                Some("protocol_violation"),
                5000,
                Some(6000),
            ),
            run(6, "dev", "killed", None, 6000, Some(7000)),
            run(7, "lead", "failed", None, 7000, Some(8000)),
            run(8, "lead", "timed_out", None, 8000, Some(9000)),
            run(9, "lead", "lost", None, 9000, Some(10000)),
            run(10, "lead", "succeeded", None, 10000, Some(11000)),
        ];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 12000,
        };
        let view = derive(&facts);
        let all_rows: Vec<&LedgerRow> = view.ledger.groups.iter().flat_map(|g| &g.rows).collect();
        assert_eq!(all_rows[0].state, "dang_chay");
        assert_eq!(all_rows[0].verdict, "đang chạy");
        assert_eq!(all_rows[1].state, "dat");
        assert_eq!(all_rows[1].verdict, "đạt");
        assert_eq!(all_rows[2].state, "can_sua");
        assert_eq!(all_rows[2].verdict, "cần sửa");
        assert_eq!(all_rows[3].state, "vuong_mac");
        assert_eq!(all_rows[3].verdict, "vướng mắc");
        assert_eq!(all_rows[4].state, "vi_pham");
        assert_eq!(all_rows[4].verdict, "vi phạm giao thức");
        assert_eq!(all_rows[5].state, "killed");
        assert_eq!(all_rows[5].verdict, "đã dừng");
        assert_eq!(all_rows[6].state, "loi");
        assert_eq!(all_rows[6].verdict, "lỗi");
        assert_eq!(all_rows[7].state, "loi");
        assert_eq!(all_rows[7].verdict, "lỗi");
        assert_eq!(all_rows[8].state, "loi");
        assert_eq!(all_rows[8].verdict, "lỗi");
        assert_eq!(all_rows[9].state, "xong");
        assert_eq!(all_rows[9].verdict, "xong");
    }

    #[test]
    fn test_unknown_fields_are_null() {
        let task = dummy_task("open");
        let r_killed = run(1, "dev", "killed", None, 1000, Some(2000));
        let r_partial = RunRow {
            usage_source: "partial".to_string(),
            ..run(2, "dev", "succeeded", None, 2000, Some(3000))
        };
        let r_no_turns = RunRow {
            num_turns: None,
            ..run(3, "dev", "succeeded", None, 3000, Some(4000))
        };
        let r_lost = run(4, "dev", "lost", None, 4000, None);
        let r_running = run(5, "dev", "running", None, 10_000, None);

        let runs = vec![r_killed, r_partial, r_no_turns, r_lost, r_running];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 100_000,
        };
        let view = derive(&facts);
        let rows: Vec<&LedgerRow> = view.ledger.groups.iter().flat_map(|g| &g.rows).collect();
        assert!(rows[0].cost_usd.is_none());
        assert!(rows[1].cost_usd.is_none());
        assert!(rows[2].turns.is_none());
        assert!(rows[3].duration_ms.is_none());
        assert_eq!(rows[4].duration_ms, Some(90_000));
        assert!(rows[4].running);
        assert!(rows[4].cost_usd.is_none());
    }

    #[test]
    fn test_totals_sum_known_only() {
        let task = dummy_task("open");
        let r1 = RunRow {
            cost_usd: 1.25,
            usage_source: "result".to_string(),
            ..run(1, "lead", "succeeded", None, 1000, Some(2000))
        };
        let r2 = RunRow {
            cost_usd: 0.40,
            ..run(2, "dev", "killed", None, 2000, Some(3000))
        };
        let r3 = RunRow {
            cost_usd: 2.00,
            usage_source: "result".to_string(),
            ..run(3, "qa", "succeeded", None, 3000, Some(4000))
        };
        let runs = vec![r1, r2, r3];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 5000,
        };
        let v1 = derive(&facts);
        assert_eq!(v1.ledger.total.runs, 3);
        assert_eq!(v1.ledger.total.cost_usd, Some(3.25));
        assert_eq!(v1.ledger.total.cost_unknown, 1);

        // all-unknown
        let r_unk = vec![run(1, "lead", "killed", None, 1000, Some(2000))];
        let v2 = derive(&PipelineFacts {
            task: &task,
            runs: &r_unk,
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 5000,
        });
        assert_eq!(v2.ledger.total.cost_usd, None);

        // no runs
        let v3 = derive(&PipelineFacts {
            task: &task,
            runs: &[],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 5000,
        });
        assert_eq!(v3.ledger.total.runs, 0);
        assert_eq!(v3.ledger.total.duration_ms, None);
        assert_eq!(v3.ledger.total.cost_usd, None);
        assert_eq!(v3.ledger.total.cost_unknown, 0);
    }

    #[test]
    fn test_cards_after_relead() {
        let task = dummy_task("planned");
        let r1 = RunRow {
            cost_usd: 0.50,
            usage_source: "result".to_string(),
            ..run(1, "lead", "succeeded", None, 1000, Some(2000))
        };
        let r2 = RunRow {
            cost_usd: 1.00,
            usage_source: "result".to_string(),
            ..run(2, "dev", "succeeded", None, 3000, Some(5000))
        };
        let r3 = RunRow {
            cost_usd: 0.25,
            usage_source: "result".to_string(),
            ..run(3, "qa", "succeeded", Some("qa:fix"), 6000, Some(7000))
        };
        let r4 = RunRow {
            cost_usd: 0.75,
            usage_source: "result".to_string(),
            ..run(4, "lead", "succeeded", None, 8000, Some(9000))
        };
        let runs = vec![r1, r2, r3, r4];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 10000,
        };
        let view = derive(&facts);
        assert_eq!(view.cycle, Some(2));
        assert_eq!(view.cards.plan.state, "xong");
        assert_eq!(view.cards.plan.rounds, 2);
        assert_eq!(view.cards.plan.run_id, Some(4));

        assert_eq!(view.cards.code.state, "cho");
        assert_eq!(view.cards.code.state_word, "chưa bắt đầu");
        assert_eq!(view.cards.code.result, "chưa bắt đầu");
        assert_eq!(view.cards.code.rounds, 1);
        assert_eq!(view.cards.code.duration_ms, Some(2000));
        assert_eq!(view.cards.code.cost_usd, Some(1.00));

        assert_eq!(view.cards.qa.state, "cho");
        assert_eq!(view.cards.qa.rounds, 1);
    }

    #[test]
    fn test_card_result_lines() {
        let task = dummy_task("planned");

        // Plan states
        let v_cho = derive(&PipelineFacts {
            task: &task,
            runs: &[],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 1000,
        });
        assert_eq!(v_cho.cards.plan.result, "chưa bắt đầu");

        let v_pc = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "lead", "running", None, 1000, None)],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 2000,
        });
        assert_eq!(v_pc.cards.plan.result, "đang lập plan");

        let v_px = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "lead", "succeeded", None, 1000, Some(2000))],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 3000,
        });
        assert_eq!(v_px.cards.plan.result, "plan đã commit");

        let v_pnc = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "lead", "succeeded", None, 1000, Some(2000))],
            plan: &PlanFacts::NoCommit,
            current_step: "dev",
            now_ms: 3000,
        });
        assert_eq!(v_pnc.cards.plan.result, "xong");

        let v_pk = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "lead", "killed", None, 1000, Some(2000))],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 3000,
        });
        assert_eq!(v_pk.cards.plan.result, "đã dừng");

        let v_pl = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "lead", "failed", None, 1000, Some(2000))],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 3000,
        });
        assert_eq!(v_pl.cards.plan.result, "Lead lỗi");

        let v_pv = derive(&PipelineFacts {
            task: &task,
            runs: &[run(
                1,
                "lead",
                "succeeded",
                Some("protocol_violation"),
                1000,
                Some(2000),
            )],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 3000,
        });
        assert_eq!(v_pv.cards.plan.result, "vi phạm giao thức");

        let v_pvm = derive(&PipelineFacts {
            task: &task,
            runs: &[run(
                1,
                "lead",
                "succeeded",
                Some("dev:blocked"),
                1000,
                Some(2000),
            )],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 3000,
        });
        assert_eq!(v_pvm.cards.plan.result, "vướng mắc");

        let v_pcs = derive(&PipelineFacts {
            task: &task,
            runs: &[run(
                1,
                "lead",
                "succeeded",
                Some("qa:fix"),
                1000,
                Some(2000),
            )],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 3000,
        });
        assert_eq!(v_pcs.cards.plan.result, "cần sửa");

        // Code states
        assert_eq!(v_cho.cards.code.result, "chưa bắt đầu");

        let v_cd = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "dev", "running", None, 1000, None)],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 2000,
        });
        assert_eq!(v_cd.cards.code.result, "đang code");

        let r_t7 = RunRow {
            num_turns: Some(7),
            ..run(1, "dev", "succeeded", None, 1000, Some(2000))
        };
        let v_ct7 = derive(&PipelineFacts {
            task: &task,
            runs: &[r_t7],
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 3000,
        });
        assert_eq!(v_ct7.cards.code.result, "7 lượt");

        let r_tnone = RunRow {
            num_turns: None,
            ..run(1, "dev", "succeeded", None, 1000, Some(2000))
        };
        let v_ctn = derive(&PipelineFacts {
            task: &task,
            runs: &[r_tnone],
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 3000,
        });
        assert_eq!(v_ctn.cards.code.result, "xong");

        let v_cvm = derive(&PipelineFacts {
            task: &task,
            runs: &[run(
                1,
                "dev",
                "succeeded",
                Some("dev:blocked"),
                1000,
                Some(2000),
            )],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 3000,
        });
        assert_eq!(v_cvm.cards.code.result, "vướng mắc");

        let v_ccs = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "dev", "succeeded", Some("qa:fix"), 1000, Some(2000))],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 3000,
        });
        assert_eq!(v_ccs.cards.code.result, "cần sửa");

        let v_ck = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "dev", "killed", None, 1000, Some(2000))],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 3000,
        });
        assert_eq!(v_ck.cards.code.result, "đã dừng");

        let v_cl = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "dev", "failed", None, 1000, Some(2000))],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 3000,
        });
        assert_eq!(v_cl.cards.code.result, "Dev lỗi");

        let v_cv = derive(&PipelineFacts {
            task: &task,
            runs: &[run(
                1,
                "dev",
                "succeeded",
                Some("protocol_violation"),
                1000,
                Some(2000),
            )],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 3000,
        });
        assert_eq!(v_cv.cards.code.result, "vi phạm giao thức");

        // QA states
        assert_eq!(v_cho.cards.qa.result, "chưa bắt đầu");

        let v_qd = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "qa", "running", None, 1000, None)],
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 2000,
        });
        assert_eq!(v_qd.cards.qa.result, "đang kiểm tra");

        let v_qp = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "qa", "succeeded", Some("qa:pass"), 1000, Some(2000))],
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 3000,
        });
        assert_eq!(v_qp.cards.qa.state, "xong");
        assert_eq!(v_qp.cards.qa.result, "đạt");

        let v_qx = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "qa", "succeeded", None, 1000, Some(2000))],
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 3000,
        });
        assert_eq!(v_qx.cards.qa.result, "xong");

        let v_qcs = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "qa", "succeeded", Some("qa:fix"), 1000, Some(2000))],
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 3000,
        });
        assert_eq!(v_qcs.cards.qa.result, "cần sửa");

        let v_qvm = derive(&PipelineFacts {
            task: &task,
            runs: &[run(
                1,
                "qa",
                "succeeded",
                Some("dev:blocked"),
                1000,
                Some(2000),
            )],
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 3000,
        });
        assert_eq!(v_qvm.cards.qa.result, "vướng mắc");

        let v_qk = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "qa", "killed", None, 1000, Some(2000))],
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 3000,
        });
        assert_eq!(v_qk.cards.qa.result, "đã dừng");

        let v_ql = derive(&PipelineFacts {
            task: &task,
            runs: &[run(1, "qa", "failed", None, 1000, Some(2000))],
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 3000,
        });
        assert_eq!(v_ql.cards.qa.result, "QA lỗi");

        let v_qv = derive(&PipelineFacts {
            task: &task,
            runs: &[run(
                1,
                "qa",
                "succeeded",
                Some("protocol_violation"),
                1000,
                Some(2000),
            )],
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 3000,
        });
        assert_eq!(v_qv.cards.qa.result, "vi phạm giao thức");
    }

    #[test]
    fn test_card_unknown_cost_is_null() {
        let task = dummy_task("planned");
        let r = run(1, "dev", "killed", None, 1000, Some(3000));
        let view = derive(&PipelineFacts {
            task: &task,
            runs: &[r],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 5000,
        });
        assert!(view.cards.code.cost_usd.is_none());
        assert_eq!(view.cards.code.duration_ms, Some(2000));
    }

    #[test]
    fn test_no_runs() {
        let task = dummy_task("open");
        let view = derive(&PipelineFacts {
            task: &task,
            runs: &[],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 5000,
        });
        assert_eq!(view.cards.start.state, "xong");
        assert_eq!(view.cards.start.result, "bạn đã giao");
        assert_eq!(view.cards.start.at_ms, Some(task.created_at));

        assert_eq!(view.cards.plan.state, "cho");
        assert_eq!(view.cards.plan.state_word, "chưa bắt đầu");
        assert_eq!(view.cards.plan.result, "chưa bắt đầu");
        assert_eq!(view.cards.plan.rounds, 0);
        assert!(view.cards.plan.duration_ms.is_none());
        assert!(view.cards.plan.cost_usd.is_none());

        assert_eq!(view.cards.code.state, "cho");
        assert_eq!(view.cards.code.state_word, "chưa bắt đầu");
        assert_eq!(view.cards.code.result, "chưa bắt đầu");
        assert_eq!(view.cards.code.rounds, 0);
        assert!(view.cards.code.duration_ms.is_none());
        assert!(view.cards.code.cost_usd.is_none());

        assert_eq!(view.cards.qa.state, "cho");
        assert_eq!(view.cards.qa.state_word, "chưa bắt đầu");
        assert_eq!(view.cards.qa.result, "chưa bắt đầu");
        assert_eq!(view.cards.qa.rounds, 0);
        assert!(view.cards.qa.duration_ms.is_none());
        assert!(view.cards.qa.cost_usd.is_none());

        assert_eq!(view.cards.ban.state, "cho");
        assert_eq!(view.cards.ban.result, "chưa có plan");
        assert!(view.cards.ban.at_ms.is_none());

        assert!(view.current.is_none());
        assert!(view.cycle.is_none());
        assert!(view.ledger.groups.is_empty());
    }

    #[test]
    fn test_gate_from_plan_facts() {
        let task = dummy_task("planned");
        let l_succ = run(1, "lead", "succeeded", None, 1000, Some(2000));
        let d_run = run(2, "dev", "succeeded", None, 2500, Some(3500));

        // Commit approved_by dev id -> xong đã duyệt plan at that run's started_at
        let v1 = derive(&PipelineFacts {
            task: &task,
            runs: &[l_succ.clone(), d_run.clone()],
            plan: &PlanFacts::Commit {
                sha: "abc".to_string(),
                at_ms: Some(1500),
                approved_by: Some(2),
            },
            current_step: "dev",
            now_ms: 5000,
        });
        assert_eq!(v1.cards.ban.state, "xong");
        assert_eq!(v1.cards.ban.result, "đã duyệt plan");
        assert_eq!(v1.cards.ban.at_ms, Some(2500));

        // Commit approved_by None at_ms Some(t) -> cho_ban at t
        let v2 = derive(&PipelineFacts {
            task: &task,
            runs: &[l_succ.clone()],
            plan: &PlanFacts::Commit {
                sha: "abc".to_string(),
                at_ms: Some(1800),
                approved_by: None,
            },
            current_step: "dev",
            now_ms: 5000,
        });
        assert_eq!(v2.cards.ban.state, "cho_ban");
        assert_eq!(v2.cards.ban.at_ms, Some(1800));

        // Commit approved_by None at_ms None -> cho_ban at latest succeeded lead ended_at
        let v3 = derive(&PipelineFacts {
            task: &task,
            runs: &[l_succ.clone()],
            plan: &PlanFacts::Commit {
                sha: "abc".to_string(),
                at_ms: None,
                approved_by: None,
            },
            current_step: "dev",
            now_ms: 5000,
        });
        assert_eq!(v3.cards.ban.state, "cho_ban");
        assert_eq!(v3.cards.ban.at_ms, Some(2000));

        // NoCommit -> cho chưa có plan
        let v4 = derive(&PipelineFacts {
            task: &task,
            runs: &[l_succ.clone()],
            plan: &PlanFacts::NoCommit,
            current_step: "dev",
            now_ms: 5000,
        });
        assert_eq!(v4.cards.ban.state, "cho");
        assert_eq!(v4.cards.ban.result, "chưa có plan");
        assert!(v4.cards.ban.at_ms.is_none());

        // Unknown + succeeded lead + later dev -> xong at dev started_at
        let v5 = derive(&PipelineFacts {
            task: &task,
            runs: &[l_succ.clone(), d_run.clone()],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 5000,
        });
        assert_eq!(v5.cards.ban.state, "xong");
        assert_eq!(v5.cards.ban.at_ms, Some(2500));

        // Unknown + succeeded lead only -> cho_ban at lead ended_at
        let v6 = derive(&PipelineFacts {
            task: &task,
            runs: &[l_succ.clone()],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 5000,
        });
        assert_eq!(v6.cards.ban.state, "cho_ban");
        assert_eq!(v6.cards.ban.at_ms, Some(2000));

        // Unknown + failed lead only -> cho
        let l_fail = run(1, "lead", "failed", None, 1000, Some(2000));
        let v7 = derive(&PipelineFacts {
            task: &task,
            runs: &[l_fail],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 5000,
        });
        assert_eq!(v7.cards.ban.state, "cho");
        assert_eq!(v7.cards.ban.result, "chưa có plan");
        assert!(v7.cards.ban.at_ms.is_none());
    }

    #[test]
    fn test_current_mapping() {
        let task = dummy_task("planned");
        let l_succ = run(1, "lead", "succeeded", None, 1000, Some(2000));

        // no rows -> null
        let v0 = derive(&PipelineFacts {
            task: &task,
            runs: &[],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 5000,
        });
        assert!(v0.current.is_none());

        // current_step lead -> plan
        let v1 = derive(&PipelineFacts {
            task: &task,
            runs: &[l_succ.clone()],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 5000,
        });
        assert_eq!(v1.current.as_deref(), Some("plan"));

        // current_step qa -> qa
        let v2 = derive(&PipelineFacts {
            task: &task,
            runs: &[l_succ.clone()],
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 5000,
        });
        assert_eq!(v2.current.as_deref(), Some("qa"));

        // current_step dev with ban cho_ban -> ban
        let v3 = derive(&PipelineFacts {
            task: &task,
            runs: &[l_succ.clone()],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 5000,
        });
        assert_eq!(v3.cards.ban.state, "cho_ban");
        assert_eq!(v3.current.as_deref(), Some("ban"));

        // current_step dev with ban xong -> code
        let d_succ = run(2, "dev", "succeeded", None, 2500, Some(3500));
        let v4 = derive(&PipelineFacts {
            task: &task,
            runs: &[l_succ, d_succ],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 5000,
        });
        assert_eq!(v4.cards.ban.state, "xong");
        assert_eq!(v4.current.as_deref(), Some("code"));
    }

    #[test]
    fn test_derive_is_deterministic() {
        let task = dummy_task("planned");
        let runs = vec![
            run(1, "lead", "succeeded", None, 1000, Some(2000)),
            run(2, "dev", "succeeded", None, 3000, Some(4000)),
        ];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 5000,
        };
        let v1 = derive(&facts);
        let v2 = derive(&facts);
        assert_eq!(v1, v2);
        let j1 = serde_json::to_string(&v1).unwrap();
        let j2 = serde_json::to_string(&v2).unwrap();
        assert_eq!(j1, j2);
    }

    #[test]
    fn test_approving_equal_base() {
        let dev = RunRow {
            base_commit: Some("aaa1".to_string()),
            ..run(42, "dev", "succeeded", None, 1000, Some(2000))
        };
        let runs = vec![dev];
        let res = approving_dev_run(&runs, "aaa1", None, |p, b| p == b);
        assert_eq!(res, Some(42));
    }

    #[test]
    fn test_approving_descendant_base() {
        let dev = RunRow {
            base_commit: Some("bbb2".to_string()),
            ..run(42, "dev", "succeeded", None, 1000, Some(2000))
        };
        let runs = vec![dev];
        let res = approving_dev_run(&runs, "aaa1", None, |p, b| p == "aaa1" && b == "bbb2");
        assert_eq!(res, Some(42));
    }

    #[test]
    fn test_approving_unrelated_base() {
        let dev = RunRow {
            base_commit: Some("ccc3".to_string()),
            ..run(42, "dev", "succeeded", None, 1000, Some(2000))
        };
        let runs = vec![dev];
        let res = approving_dev_run(&runs, "aaa1", None, |_p, _b| false);
        assert_eq!(res, None);
    }

    #[test]
    fn test_approving_null_base_uses_time() {
        let dev = RunRow {
            base_commit: None,
            ..run(42, "dev", "succeeded", None, 1000, Some(2000))
        };
        let runs = vec![dev];
        // started_at >= plan_at -> Some
        assert_eq!(
            approving_dev_run(&runs, "aaa1", Some(1000), |_p, _b| false),
            Some(42)
        );
        assert_eq!(
            approving_dev_run(&runs, "aaa1", Some(900), |_p, _b| false),
            Some(42)
        );
        // started_at < plan_at -> None
        assert_eq!(
            approving_dev_run(&runs, "aaa1", Some(1001), |_p, _b| false),
            None
        );
        // plan_at None -> None
        assert_eq!(approving_dev_run(&runs, "aaa1", None, |_p, _b| false), None);
    }

    #[test]
    fn test_approving_first_match_in_start_order() {
        let dev2 = RunRow {
            base_commit: Some("bbb2".to_string()),
            ..run(10, "dev", "succeeded", None, 2000, Some(3000))
        };
        let dev1 = RunRow {
            base_commit: Some("bbb2".to_string()),
            ..run(20, "dev", "succeeded", None, 1000, Some(2000))
        };
        // given out of order [dev2, dev1]
        let runs = vec![dev2, dev1];
        let res = approving_dev_run(&runs, "aaa1", None, |_p, _b| true);
        assert_eq!(res, Some(20));

        // tie on started_at: earlier id wins
        let dev_b = RunRow {
            base_commit: Some("bbb2".to_string()),
            ..run(15, "dev", "succeeded", None, 1000, Some(2000))
        };
        let dev_a = RunRow {
            base_commit: Some("bbb2".to_string()),
            ..run(7, "dev", "succeeded", None, 1000, Some(2000))
        };
        let runs_tie = vec![dev_b, dev_a];
        let res_tie = approving_dev_run(&runs_tie, "aaa1", None, |_p, _b| true);
        assert_eq!(res_tie, Some(7));
    }

    #[test]
    fn test_approving_ignores_lead_and_qa() {
        let lead = RunRow {
            base_commit: Some("aaa1".to_string()),
            ..run(1, "lead", "succeeded", None, 1000, Some(2000))
        };
        let qa = RunRow {
            base_commit: Some("aaa1".to_string()),
            ..run(2, "qa", "succeeded", None, 2000, Some(3000))
        };
        let runs = vec![lead, qa];
        let res = approving_dev_run(&runs, "aaa1", None, |p, b| p == b);
        assert_eq!(res, None);
    }

    fn staged_run(
        id: i64,
        role: &str,
        stage: Option<&str>,
        round: Option<i64>,
        status: &str,
        outcome: Option<&str>,
        metrics: Option<&str>,
        started_at: i64,
    ) -> RunRow {
        let mut r = run(
            id,
            role,
            status,
            outcome,
            started_at,
            Some(started_at + 1000),
        );
        r.stage = stage.map(|s| s.to_string());
        r.round = round;
        r.metrics_json = metrics.map(|s| s.to_string());
        r
    }

    fn test_mac_dinh() -> Workflow {
        Workflow {
            name: "mac-dinh".to_string(),
            stages: vec![
                "plan".to_string(),
                "plan-review".to_string(),
                "gate:you".to_string(),
                "implement".to_string(),
                "code-review".to_string(),
                "qa".to_string(),
                "fix".to_string(),
            ],
            loops: vec!["plan".to_string(), "build".to_string(), "qa".to_string()],
            max_rounds: 3,
            budget_minutes: 60,
            budget_usd: 20.0,
            auto: true,
            source: "builtin".to_string(),
        }
    }

    fn test_nhanh() -> Workflow {
        Workflow {
            name: "nhanh".to_string(),
            stages: vec![
                "plan".to_string(),
                "gate:you".to_string(),
                "implement".to_string(),
                "qa".to_string(),
            ],
            loops: vec![],
            max_rounds: 3,
            budget_minutes: 60,
            budget_usd: 20.0,
            auto: false,
            source: "builtin".to_string(),
        }
    }

    #[test]
    fn test_card_results_new() {
        let task = dummy_task("dev_done");
        let wf = test_mac_dinh();

        // 1. plan_review variations
        for (status, outcome, exp_res) in [
            ("running", None, "đang review plan"),
            ("succeeded", Some("review:approve"), "duyệt"),
            ("succeeded", Some("review:revise"), "sửa lại"),
            ("killed", None, "đã dừng"),
            ("failed", None, "Review lỗi"),
            ("succeeded", Some("protocol_violation"), "vi phạm giao thức"),
        ] {
            let runs = vec![staged_run(
                1,
                "plan-review",
                Some("plan-review"),
                Some(1),
                status,
                outcome,
                None,
                1000,
            )];
            let facts = PipelineFacts {
                task: &task,
                runs: &runs,
                plan: &PlanFacts::Unknown,
                current_step: "qa",
                now_ms: 5000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            let pr_card = resp.groups[0]
                .cards
                .iter()
                .find(|c| c.key == "plan_review")
                .unwrap();
            assert_eq!(pr_card.result, exp_res);
        }

        // 2. code_review variations
        for (status, outcome, exp_res) in [
            ("running", None, "đang review code"),
            ("succeeded", Some("review:pass"), "đạt"),
            ("succeeded", Some("review:revise"), "sửa lại"),
            ("killed", None, "đã dừng"),
            ("failed", None, "Review lỗi"),
            ("succeeded", Some("protocol_violation"), "vi phạm giao thức"),
        ] {
            let runs = vec![staged_run(
                1,
                "code-review",
                Some("code-review"),
                Some(1),
                status,
                outcome,
                None,
                1000,
            )];
            let facts = PipelineFacts {
                task: &task,
                runs: &runs,
                plan: &PlanFacts::Unknown,
                current_step: "qa",
                now_ms: 5000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            let cr_card = resp.groups[1]
                .cards
                .iter()
                .find(|c| c.key == "code_review")
                .unwrap();
            assert_eq!(cr_card.result, exp_res);
        }

        // 3. fix variations
        // 3a. fix cho "chưa bắt đầu"
        {
            let runs = vec![staged_run(
                1,
                "lead",
                Some("plan"),
                Some(1),
                "succeeded",
                None,
                None,
                1000,
            )];
            let facts = PipelineFacts {
                task: &task,
                runs: &runs,
                plan: &PlanFacts::Unknown,
                current_step: "lead",
                now_ms: 5000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            let fix_card = resp.groups[2]
                .cards
                .iter()
                .find(|c| c.key == "fix")
                .unwrap();
            assert_eq!(fix_card.state, "cho");
            assert_eq!(fix_card.result, "chưa bắt đầu");
        }

        // 3b. fix cho "không cần sửa" when qa passed and no fix run in cycle
        {
            let runs = vec![staged_run(
                1,
                "qa",
                Some("qa"),
                Some(1),
                "succeeded",
                Some("qa:pass"),
                None,
                1000,
            )];
            let facts = PipelineFacts {
                task: &task,
                runs: &runs,
                plan: &PlanFacts::Unknown,
                current_step: "qa",
                now_ms: 5000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            let fix_card = resp.groups[2]
                .cards
                .iter()
                .find(|c| c.key == "fix")
                .unwrap();
            assert_eq!(fix_card.state, "cho");
            assert_eq!(fix_card.result, "không cần sửa");
        }

        // 3c. fix active/completed states
        for (status, outcome, turns, exp_res) in [
            ("running", None, None, "đang sửa"),
            ("succeeded", None, Some(5), "5 lượt"),
            ("succeeded", None, None, "xong"),
            ("succeeded", Some("dev:blocked"), None, "vướng mắc"),
            ("killed", None, None, "đã dừng"),
            ("failed", None, None, "Dev lỗi"),
            (
                "succeeded",
                Some("protocol_violation"),
                None,
                "vi phạm giao thức",
            ),
        ] {
            let mut r = staged_run(
                1,
                "dev",
                Some("fix"),
                Some(1),
                status,
                outcome,
                Some(r#"{"fix_for":"qa"}"#),
                1000,
            );
            r.num_turns = turns;
            let runs = vec![r];
            let facts = PipelineFacts {
                task: &task,
                runs: &runs,
                plan: &PlanFacts::Unknown,
                current_step: "dev",
                now_ms: 5000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            let fix_card = resp.groups[2]
                .cards
                .iter()
                .find(|c| c.key == "fix")
                .unwrap();
            assert_eq!(fix_card.result, exp_res);
        }

        // 4. code: "đang sửa theo review" when running code-review fix
        {
            let r = staged_run(
                1,
                "dev",
                Some("fix"),
                Some(2),
                "running",
                None,
                Some(r#"{"fix_for":"code_review"}"#),
                1000,
            );
            let runs = vec![r];
            let facts = PipelineFacts {
                task: &task,
                runs: &runs,
                plan: &PlanFacts::Unknown,
                current_step: "dev",
                now_ms: 5000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            let code_card = resp.groups[1]
                .cards
                .iter()
                .find(|c| c.key == "code")
                .unwrap();
            assert_eq!(code_card.result, "đang sửa theo review");
        }
    }

    #[test]
    fn test_counts_known_zero_and_unknown() {
        // review approve with blocking 0
        let counts0 = Counts::from_metrics_json(Some(r#"{"blocking":0,"advisory":1}"#)).unwrap();
        assert_eq!(counts0.blocking, Some(0));
        assert_eq!(counts0.advisory, Some(1));

        // QA metrics with all null
        let counts_all_null = Counts::from_metrics_json(Some(
            r#"{"specs":null,"failing":null,"not_automatable":null}"#,
        ));
        assert_eq!(counts_all_null, None);

        // run without metrics
        assert_eq!(Counts::from_metrics_json(None), None);
        assert_eq!(Counts::from_metrics_json(Some("")), None);
        assert_eq!(Counts::from_metrics_json(Some("{}")), None);
    }

    #[test]
    fn test_gate_view_cases() {
        let task = dummy_task("dev_done");
        let wf_auto = test_mac_dinh();
        let wf_manual = test_nhanh();

        // 1. NoPlan
        {
            let facts = PipelineFacts {
                task: &task,
                runs: &[],
                plan: &PlanFacts::NoCommit,
                current_step: "lead",
                now_ms: 1000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf_auto),
                gate: Some(GateState::NoPlan),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.gate.state, "cho");
            assert_eq!(resp.gate.state_word, "chưa bắt đầu");
            assert_eq!(resp.gate.result, "chưa có plan");
            assert_eq!(resp.gate.note_kind, "not_open");
            assert_eq!(
                resp.gate.note.as_deref(),
                Some("Cổng này mở khi plan đã commit và review plan xong.")
            );
            assert_eq!(resp.gate.at_ms, None);
            assert!(!resp.gate.derived);
        }

        // 2. AwaitingReview
        {
            let facts = PipelineFacts {
                task: &task,
                runs: &[],
                plan: &PlanFacts::Unknown,
                current_step: "lead",
                now_ms: 1000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf_auto),
                gate: Some(GateState::AwaitingReview),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.gate.state, "cho");
            assert_eq!(resp.gate.state_word, "chờ review plan");
            assert_eq!(resp.gate.result, "chờ review plan");
            assert_eq!(resp.gate.note_kind, "not_open");
            assert_eq!(
                resp.gate.note.as_deref(),
                Some("Cổng này mở khi plan đã commit và review plan xong.")
            );
            assert_eq!(resp.gate.at_ms, None);
            assert!(!resp.gate.derived);
        }

        // 3. Waiting (mac-dinh auto)
        {
            let facts = PipelineFacts {
                task: &task,
                runs: &[],
                plan: &PlanFacts::Unknown,
                current_step: "lead",
                now_ms: 1000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf_auto),
                gate: Some(GateState::Waiting),
                plan_commit_at: Some(5000),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.gate.state, "cho_ban");
            assert_eq!(resp.gate.state_word, "chờ bạn");
            assert_eq!(resp.gate.result, "chờ duyệt plan");
            assert_eq!(resp.gate.at_ms, Some(5000));
            assert_eq!(resp.gate.note_kind, "auto_waiting");
            assert_eq!(
                resp.gate.note.as_deref(),
                Some("Autopilot dừng ở đây để bạn duyệt plan. Duyệt plan rồi autopilot tự chạy Code, Review code, QA và Sửa, tối đa 3 vòng mỗi bước, trong 60 phút hoặc $20.")
            );
            assert!(!resp.gate.derived);
        }

        // 4. Waiting (nhanh manual)
        {
            let facts = PipelineFacts {
                task: &task,
                runs: &[],
                plan: &PlanFacts::Unknown,
                current_step: "lead",
                now_ms: 1000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf_manual),
                gate: Some(GateState::Waiting),
                plan_commit_at: Some(6000),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.gate.state, "cho_ban");
            assert_eq!(resp.gate.state_word, "chờ bạn");
            assert_eq!(resp.gate.result, "chờ duyệt plan");
            assert_eq!(resp.gate.at_ms, Some(6000));
            assert_eq!(resp.gate.note_kind, "manual_waiting");
            assert_eq!(
                resp.gate.note.as_deref(),
                Some("Duyệt plan để ghi nhận bạn đồng ý, rồi bấm Chạy Dev. Bấm Chạy Dev trực tiếp cũng coi là đã duyệt.")
            );
            assert!(!resp.gate.derived);
        }

        // 5. Waiting with auto_state stopped and stop loop plan
        {
            let auto = TaskAuto {
                workflow_json: None,
                auto_state: Some("stopped".to_string()),
                approved_at: None,
                approved_commit: None,
                auto_pid: None,
                auto_pid_start: None,
                auto_stop_at: None,
                auto_stop_kind: None,
                auto_json: None,
            };
            let stop_info = decide::StopInfo {
                reason: decide::StopReason::HetVong,
                loop_name: Some(Loop::Plan),
                stage: Some(Stage::PlanReview),
                round: Some(3),
                cap: Some(3),
                run_id: Some(1),
                role: Some("plan-review".to_string()),
                at_ms: 7000,
                detail_line: None,
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            };
            let aj = decide::AutoJson {
                stop: Some(stop_info),
                ..Default::default()
            };
            let facts = PipelineFacts {
                task: &task,
                runs: &[],
                plan: &PlanFacts::Unknown,
                current_step: "lead",
                now_ms: 1000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf_auto),
                auto: Some(&auto),
                auto_json: Some(&aj),
                gate: Some(GateState::Waiting),
                plan_commit_at: Some(5000),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.gate.state, "cho_ban");
            assert_eq!(resp.gate.result, "chờ duyệt plan");
            assert_eq!(resp.gate.note_kind, "loop_stopped");
            assert_eq!(
                resp.gate.note.as_deref(),
                Some("Plan chưa qua review. Bạn có thể duyệt plan hiện tại hoặc trả về Lead.")
            );
        }

        // 6. Approved (approved_at 1234)
        {
            let auto = TaskAuto {
                workflow_json: None,
                auto_state: Some("running".to_string()),
                approved_at: Some(1234),
                approved_commit: Some("abc".to_string()),
                auto_pid: None,
                auto_pid_start: None,
                auto_stop_at: None,
                auto_stop_kind: None,
                auto_json: None,
            };
            let facts = PipelineFacts {
                task: &task,
                runs: &[],
                plan: &PlanFacts::Unknown,
                current_step: "dev",
                now_ms: 2000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf_auto),
                auto: Some(&auto),
                gate: Some(GateState::Approved),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.gate.state, "xong");
            assert_eq!(resp.gate.state_word, "xong");
            assert_eq!(resp.gate.result, "đã duyệt plan");
            assert_eq!(resp.gate.at_ms, Some(1234));
            assert_eq!(resp.gate.note_kind, "approved_explicit");
            assert_eq!(resp.gate.note, None);
        }

        // 7. Legacy task (workflow None) with base ban `cho_ban`
        {
            let lead_run = run(1, "lead", "succeeded", None, 1000, Some(2000));
            let runs = vec![lead_run];
            let facts = PipelineFacts {
                task: &task,
                runs: &runs,
                plan: &PlanFacts::Commit {
                    sha: "abc".to_string(),
                    at_ms: Some(2000),
                    approved_by: None,
                },
                current_step: "dev",
                now_ms: 3000,
            };
            let flow = FlowFacts {
                workflow: None,
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.gate.state, "cho_ban");
            assert_eq!(resp.gate.note_kind, "derived");
            assert!(resp.gate.derived);
            assert_eq!(
                resp.gate.note.as_deref(),
                Some("Cổng duyệt suy ra từ dữ liệu: Dev đã chạy trên bản plan hiện tại thì coi là đã duyệt.")
            );
        }
    }

    #[test]
    fn gate_note_ended_is_manual() {
        let task = dummy_task("planned");
        let wf_auto = test_mac_dinh();
        let facts = PipelineFacts {
            task: &task,
            runs: &[],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 1000,
        };
        let auto_of = |st: &str| TaskAuto {
            workflow_json: None,
            auto_state: Some(st.to_string()),
            approved_at: None,
            approved_commit: None,
            auto_pid: None,
            auto_pid_start: None,
            auto_stop_at: None,
            auto_stop_kind: None,
            auto_json: None,
        };

        // Autopilot turned off for the task: approval only records.
        let ended = auto_of("ended");
        let flow = FlowFacts {
            workflow: Some(&wf_auto),
            auto: Some(&ended),
            gate: Some(GateState::Waiting),
            plan_commit_at: Some(5000),
            ..Default::default()
        };
        let resp = derive_response(&facts, &flow);
        assert_eq!(resp.gate.state, "cho_ban");
        assert_eq!(resp.gate.note_kind, "manual_waiting");
        assert_eq!(
            resp.gate.note.as_deref(),
            Some("Duyệt plan để ghi nhận bạn đồng ý, rồi bấm Chạy Dev. Bấm Chạy Dev trực tiếp cũng coi là đã duyệt.")
        );

        // Autopilot armed at the gate: unchanged.
        let gate = auto_of("gate");
        let flow = FlowFacts {
            workflow: Some(&wf_auto),
            auto: Some(&gate),
            gate: Some(GateState::Waiting),
            plan_commit_at: Some(5000),
            ..Default::default()
        };
        assert_eq!(
            derive_response(&facts, &flow).gate.note_kind,
            "auto_waiting"
        );
    }

    #[test]
    fn test_gate_note_money() {
        let task = dummy_task("dev_done");
        let facts = PipelineFacts {
            task: &task,
            runs: &[],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 1000,
        };

        // 0.05 budget_usd
        let mut wf005 = test_mac_dinh();
        wf005.budget_usd = 0.05;
        let flow005 = FlowFacts {
            workflow: Some(&wf005),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };
        let resp005 = derive_response(&facts, &flow005);
        assert!(resp005.gate.note.as_deref().unwrap().contains("$0.05"));

        // 20 budget_usd
        let wf20 = test_mac_dinh();
        let flow20 = FlowFacts {
            workflow: Some(&wf20),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };
        let resp20 = derive_response(&facts, &flow20);
        assert!(resp20
            .gate
            .note
            .as_deref()
            .unwrap()
            .contains("trong 60 phút hoặc $20."));
    }

    #[test]
    fn test_footer_after_grant() {
        let task = dummy_task("dev_done");
        let runs = vec![
            staged_run(
                1,
                "lead",
                Some("plan"),
                Some(1),
                "succeeded",
                None,
                None,
                1000,
            ),
            staged_run(
                2,
                "lead",
                Some("plan"),
                Some(2),
                "succeeded",
                None,
                None,
                2000,
            ),
            staged_run(
                3,
                "lead",
                Some("plan"),
                Some(3),
                "succeeded",
                None,
                None,
                3000,
            ),
            staged_run(
                4,
                "lead",
                Some("plan"),
                Some(4),
                "succeeded",
                None,
                None,
                4000,
            ),
        ];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 5000,
        };
        let wf = test_mac_dinh();
        let flow = FlowFacts {
            workflow: Some(&wf),
            ..Default::default()
        };
        let resp = derive_response(&facts, &flow);
        assert_eq!(resp.groups[0].round, Some(4));
        assert_eq!(resp.groups[0].cap, Some(3));
    }

    #[test]
    fn test_stopped_group() {
        let task = dummy_task("dev_done");
        let runs = vec![staged_run(
            1,
            "dev",
            Some("implement"),
            Some(1),
            "succeeded",
            None,
            None,
            1000,
        )];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 2000,
        };
        let wf = test_mac_dinh();
        let auto = TaskAuto {
            workflow_json: None,
            auto_state: Some("stopped".to_string()),
            approved_at: None,
            approved_commit: None,
            auto_pid: None,
            auto_pid_start: None,
            auto_stop_at: None,
            auto_stop_kind: None,
            auto_json: None,
        };
        let stop_info = decide::StopInfo {
            reason: decide::StopReason::HetVong,
            loop_name: Some(Loop::Build),
            stage: Some(Stage::CodeReview),
            round: Some(3),
            cap: Some(3),
            run_id: Some(1),
            role: Some("code-review".to_string()),
            at_ms: 2000,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        let aj = decide::AutoJson {
            stop: Some(stop_info),
            ..Default::default()
        };
        let flow = FlowFacts {
            workflow: Some(&wf),
            auto: Some(&auto),
            auto_json: Some(&aj),
            ..Default::default()
        };
        let resp = derive_response(&facts, &flow);
        // build group stopped true, stop_word "hết vòng"; others false / null
        assert!(!resp.groups[0].stopped);
        assert_eq!(resp.groups[0].stop_word, None);

        assert!(resp.groups[1].stopped);
        assert_eq!(resp.groups[1].stop_word.as_deref(), Some("hết vòng"));

        assert!(!resp.groups[2].stopped);
        assert_eq!(resp.groups[2].stop_word, None);
    }

    #[test]
    fn test_current_card_rules() {
        let task = dummy_task("dev_done");
        let wf = test_mac_dinh();

        // 1. running plan-review -> "plan_review"
        {
            let runs = vec![staged_run(
                1,
                "plan-review",
                Some("plan-review"),
                Some(1),
                "running",
                None,
                None,
                1000,
            )];
            let facts = PipelineFacts {
                task: &task,
                runs: &runs,
                plan: &PlanFacts::Unknown,
                current_step: "plan-review",
                now_ms: 2000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.current_card.as_deref(), Some("plan_review"));
        }

        // 2. auto stopped at stage Fix loop qa -> "fix"
        {
            let auto = TaskAuto {
                workflow_json: None,
                auto_state: Some("stopped".to_string()),
                approved_at: None,
                approved_commit: None,
                auto_pid: None,
                auto_pid_start: None,
                auto_stop_at: None,
                auto_stop_kind: None,
                auto_json: None,
            };
            let stop_info = decide::StopInfo {
                reason: decide::StopReason::HetVong,
                loop_name: Some(Loop::Qa),
                stage: Some(Stage::Fix),
                round: Some(3),
                cap: Some(3),
                run_id: Some(1),
                role: Some("dev".to_string()),
                at_ms: 2000,
                detail_line: None,
                used_ms: None,
                used_usd: None,
                estimated: None,
                limit_minutes: None,
                limit_usd: None,
            };
            let aj = decide::AutoJson {
                stop: Some(stop_info),
                ..Default::default()
            };
            let runs = vec![staged_run(
                1,
                "qa",
                Some("qa"),
                Some(1),
                "succeeded",
                Some("qa:fix"),
                None,
                1000,
            )];
            let facts = PipelineFacts {
                task: &task,
                runs: &runs,
                plan: &PlanFacts::Unknown,
                current_step: "qa",
                now_ms: 2000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf),
                auto: Some(&auto),
                auto_json: Some(&aj),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.current_card.as_deref(), Some("fix"));
        }

        // 3. gate Waiting, no driver -> "ban"
        {
            let runs = vec![staged_run(
                1,
                "lead",
                Some("plan"),
                Some(1),
                "succeeded",
                None,
                None,
                1000,
            )];
            let facts = PipelineFacts {
                task: &task,
                runs: &runs,
                plan: &PlanFacts::Unknown,
                current_step: "dev",
                now_ms: 2000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf),
                gate: Some(GateState::Waiting),
                driver_alive: false,
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.current_card.as_deref(), Some("ban"));
        }

        // 4. driver alive with next_card "code_review" -> "code_review"
        {
            let runs = vec![staged_run(
                1,
                "dev",
                Some("implement"),
                Some(1),
                "succeeded",
                None,
                None,
                1000,
            )];
            let facts = PipelineFacts {
                task: &task,
                runs: &runs,
                plan: &PlanFacts::Unknown,
                current_step: "dev",
                now_ms: 2000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf),
                gate: Some(GateState::Approved),
                driver_alive: true,
                next_card: Some("code_review".to_string()),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.current_card.as_deref(), Some("code_review"));
        }

        // 5. idle after QA pass -> "qa"
        {
            let runs = vec![staged_run(
                1,
                "qa",
                Some("qa"),
                Some(1),
                "succeeded",
                Some("qa:pass"),
                None,
                1000,
            )];
            let facts = PipelineFacts {
                task: &task,
                runs: &runs,
                plan: &PlanFacts::Unknown,
                current_step: "qa",
                now_ms: 2000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf),
                gate: Some(GateState::Approved),
                driver_alive: false,
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.current_card.as_deref(), Some("qa"));
        }

        // 6. no runs -> null
        {
            let facts = PipelineFacts {
                task: &task,
                runs: &[],
                plan: &PlanFacts::Unknown,
                current_step: "lead",
                now_ms: 2000,
            };
            let flow = FlowFacts {
                workflow: Some(&wf),
                ..Default::default()
            };
            let resp = derive_response(&facts, &flow);
            assert_eq!(resp.current_card, None);
        }
    }

    #[test]
    fn test_budget_view() {
        let task = dummy_task("dev_done");
        let wf = test_mac_dinh();
        let facts = PipelineFacts {
            task: &task,
            runs: &[],
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 1000,
        };

        // usage {used_ms 2_880_000, used_usd 16.0, used_cents 1600, estimated_runs 1} with mac-dinh
        // -> used_minutes 48, limit_minutes 60, limit_usd 20.0, estimated true, level "near"
        let usage = BudgetUsage {
            used_ms: 2_880_000,
            used_usd: 16.0,
            used_cents: 1600,
            estimated_runs: 1,
        };
        let flow = FlowFacts {
            workflow: Some(&wf),
            budget: Some(usage),
            ..Default::default()
        };
        let resp = derive_response(&facts, &flow);
        let b = resp.budget.as_ref().unwrap();
        assert_eq!(b.used_minutes, 48);
        assert_eq!(b.limit_minutes, 60);
        assert_eq!(b.used_usd, 16.0);
        assert_eq!(b.limit_usd, 20.0);
        assert!(b.estimated);
        assert_eq!(b.level, "near");

        // over the limit
        let usage_over = BudgetUsage {
            used_ms: 4_000_000,
            used_usd: 25.0,
            used_cents: 2500,
            estimated_runs: 0,
        };
        let flow_over = FlowFacts {
            workflow: Some(&wf),
            budget: Some(usage_over),
            ..Default::default()
        };
        let resp_over = derive_response(&facts, &flow_over);
        assert_eq!(resp_over.budget.as_ref().unwrap().level, "over");

        // legacy task -> budget null
        let flow_legacy = FlowFacts {
            workflow: None,
            budget: Some(usage),
            ..Default::default()
        };
        let resp_legacy = derive_response(&facts, &flow_legacy);
        assert_eq!(resp_legacy.budget, None);
    }

    #[test]
    fn test_legacy_flow() {
        let task = dummy_task("dev_done");
        let facts = PipelineFacts {
            task: &task,
            runs: &[],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 1000,
        };
        let flow = FlowFacts {
            workflow: None,
            ..Default::default()
        };
        let resp = derive_response(&facts, &flow);
        assert_eq!(resp.workflow, None);
        assert_eq!(resp.groups.len(), 3);
        assert_eq!(resp.groups[0].key, "plan");
        assert_eq!(resp.groups[0].cards.len(), 1);
        assert_eq!(resp.groups[0].cards[0].key, "plan");
        assert_eq!(resp.groups[0].cap, None);

        assert_eq!(resp.groups[1].key, "build");
        assert_eq!(resp.groups[1].cards.len(), 1);
        assert_eq!(resp.groups[1].cards[0].key, "code");
        assert_eq!(resp.groups[1].cap, None);

        assert_eq!(resp.groups[2].key, "qa");
        assert_eq!(resp.groups[2].cards.len(), 1);
        assert_eq!(resp.groups[2].cards[0].key, "qa");
        assert_eq!(resp.groups[2].cap, None);

        assert!(resp.gate.present);
        assert!(resp.gate.derived);
    }

    #[test]
    fn test_nhanh_flow() {
        let task = dummy_task("dev_done");
        let facts = PipelineFacts {
            task: &task,
            runs: &[],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 1000,
        };
        let wf = test_nhanh();
        let flow = FlowFacts {
            workflow: Some(&wf),
            gate: Some(GateState::Waiting),
            ..Default::default()
        };
        let resp = derive_response(&facts, &flow);
        assert_eq!(resp.groups.len(), 3);
        assert_eq!(resp.groups[0].cards.len(), 1);
        assert_eq!(resp.groups[0].cap, None);

        assert_eq!(resp.groups[1].cards.len(), 1);
        assert_eq!(resp.groups[1].cap, None);

        assert_eq!(resp.groups[2].cards.len(), 1);
        assert_eq!(resp.groups[2].cap, None);

        assert_eq!(resp.gate.note_kind, "manual_waiting");
        assert_eq!(
            resp.gate.note.as_deref(),
            Some("Duyệt plan để ghi nhận bạn đồng ý, rồi bấm Chạy Dev. Bấm Chạy Dev trực tiếp cũng coi là đã duyệt.")
        );
    }

    #[test]
    fn test_state_words_new() {
        assert_eq!(state_word("duyet"), "duyệt");
        assert_eq!(state_word("sua_lai"), "sửa lại");
    }

    #[test]
    fn test_staged_ledger_rows() {
        let task = dummy_task("dev_done");
        let runs = vec![
            staged_run(
                1,
                "lead",
                Some("plan"),
                Some(1),
                "succeeded",
                None,
                None,
                1000,
            ),
            staged_run(
                2,
                "plan-review",
                Some("plan-review"),
                Some(1),
                "succeeded",
                Some("review:revise"),
                Some(r#"{"blocking":1,"advisory":4}"#),
                2000,
            ),
            staged_run(
                3,
                "lead",
                Some("plan"),
                Some(2),
                "succeeded",
                None,
                None,
                3000,
            ),
            staged_run(
                4,
                "plan-review",
                Some("plan-review"),
                Some(2),
                "succeeded",
                Some("review:approve"),
                Some(r#"{"blocking":0,"advisory":1}"#),
                4000,
            ),
            staged_run(
                5,
                "dev",
                Some("implement"),
                Some(1),
                "succeeded",
                None,
                None,
                5000,
            ),
            staged_run(
                6,
                "code-review",
                Some("code-review"),
                Some(1),
                "succeeded",
                Some("review:revise"),
                Some(r#"{"blocking":2,"advisory":0}"#),
                6000,
            ),
            staged_run(
                7,
                "dev",
                Some("fix"),
                Some(2),
                "succeeded",
                None,
                Some(r#"{"fix_for":"code_review"}"#),
                7000,
            ),
            staged_run(
                8,
                "code-review",
                Some("code-review"),
                Some(2),
                "succeeded",
                Some("review:pass"),
                None,
                8000,
            ),
            staged_run(
                9,
                "qa",
                Some("qa"),
                Some(1),
                "succeeded",
                Some("qa:fix"),
                Some(r#"{"specs":3,"failing":1}"#),
                9000,
            ),
            staged_run(
                10,
                "dev",
                Some("fix"),
                Some(1),
                "succeeded",
                None,
                Some(r#"{"fix_for":"qa"}"#),
                10000,
            ),
            staged_run(
                11,
                "qa",
                Some("qa"),
                Some(2),
                "succeeded",
                Some("qa:pass"),
                None,
                11000,
            ),
        ];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 20000,
        };
        let wf = test_mac_dinh();
        let flow = FlowFacts {
            workflow: Some(&wf),
            ..Default::default()
        };
        let resp = derive_response(&facts, &flow);
        assert_eq!(resp.base.ledger.groups.len(), 3);
        assert_eq!(resp.base.ledger.groups[0].rows.len(), 4);
        assert_eq!(resp.base.ledger.groups[1].rows.len(), 4);
        assert_eq!(resp.base.ledger.groups[2].rows.len(), 3);

        let all_rows: Vec<&LedgerRow> = resp
            .base
            .ledger
            .groups
            .iter()
            .flat_map(|g| &g.rows)
            .collect();
        let labels: Vec<&str> = all_rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "Vòng 1 · plan",
                "Vòng 1 · review plan",
                "Vòng 2 · plan",
                "Vòng 2 · review plan",
                "Vòng 1 · code",
                "Vòng 1 · review code",
                "Vòng 2 · sửa theo review",
                "Vòng 2 · review code",
                "Vòng 1 · QA",
                "Vòng 1 · sửa theo QA",
                "Vòng 2 · QA",
            ]
        );
        // verdicts of the review rows "sửa lại", "duyệt", "sửa lại", "đạt"
        assert_eq!(all_rows[1].verdict, "sửa lại");
        assert_eq!(all_rows[3].verdict, "duyệt");
        assert_eq!(all_rows[5].verdict, "sửa lại");
        assert_eq!(all_rows[7].verdict, "đạt");

        // row 2 counts {blocking 1, advisory 4, others null} (1-based row 2 -> all_rows[1])
        assert_eq!(
            all_rows[1].counts,
            Some(Counts {
                blocking: Some(1),
                advisory: Some(4),
                specs: None,
                failing: None,
                not_automatable: None,
            })
        );
        // row 9 counts {specs 3, failing 1, not_automatable null} (1-based row 9 -> all_rows[8])
        assert_eq!(
            all_rows[8].counts,
            Some(Counts {
                blocking: None,
                advisory: None,
                specs: Some(3),
                failing: Some(1),
                not_automatable: None,
            })
        );
    }

    #[test]
    fn test_flow_groups_mac_dinh() {
        let task = dummy_task("dev_done");
        let runs = vec![
            staged_run(
                1,
                "lead",
                Some("plan"),
                Some(1),
                "succeeded",
                None,
                None,
                1000,
            ),
            staged_run(
                2,
                "plan-review",
                Some("plan-review"),
                Some(1),
                "succeeded",
                Some("review:revise"),
                Some(r#"{"blocking":1,"advisory":4}"#),
                2000,
            ),
            staged_run(
                3,
                "lead",
                Some("plan"),
                Some(2),
                "succeeded",
                None,
                None,
                3000,
            ),
            staged_run(
                4,
                "plan-review",
                Some("plan-review"),
                Some(2),
                "succeeded",
                Some("review:approve"),
                Some(r#"{"blocking":0,"advisory":1}"#),
                4000,
            ),
            staged_run(
                5,
                "dev",
                Some("implement"),
                Some(1),
                "succeeded",
                None,
                None,
                5000,
            ),
            staged_run(
                6,
                "code-review",
                Some("code-review"),
                Some(1),
                "succeeded",
                Some("review:revise"),
                Some(r#"{"blocking":2,"advisory":0}"#),
                6000,
            ),
            staged_run(
                7,
                "dev",
                Some("fix"),
                Some(2),
                "succeeded",
                None,
                Some(r#"{"fix_for":"code_review"}"#),
                7000,
            ),
            staged_run(
                8,
                "code-review",
                Some("code-review"),
                Some(2),
                "succeeded",
                Some("review:pass"),
                None,
                8000,
            ),
            staged_run(
                9,
                "qa",
                Some("qa"),
                Some(1),
                "succeeded",
                Some("qa:fix"),
                Some(r#"{"specs":3,"failing":1}"#),
                9000,
            ),
            staged_run(
                10,
                "dev",
                Some("fix"),
                Some(1),
                "succeeded",
                None,
                Some(r#"{"fix_for":"qa"}"#),
                10000,
            ),
            staged_run(
                11,
                "qa",
                Some("qa"),
                Some(2),
                "succeeded",
                Some("qa:pass"),
                None,
                11000,
            ),
        ];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 20000,
        };
        let wf = test_mac_dinh();
        let flow = FlowFacts {
            workflow: Some(&wf),
            ..Default::default()
        };
        let resp = derive_response(&facts, &flow);

        let group_keys: Vec<&str> = resp.groups.iter().map(|g| g.key.as_str()).collect();
        assert_eq!(group_keys, vec!["plan", "build", "qa"]);

        let card_keys: Vec<Vec<&str>> = resp
            .groups
            .iter()
            .map(|g| g.cards.iter().map(|c| c.key.as_str()).collect())
            .collect();
        assert_eq!(
            card_keys,
            vec![
                vec!["plan", "plan_review"],
                vec!["code", "code_review"],
                vec!["qa", "fix"],
            ]
        );

        let titles: Vec<Vec<&str>> = resp
            .groups
            .iter()
            .map(|g| g.cards.iter().map(|c| c.title.as_str()).collect())
            .collect();
        assert_eq!(
            titles,
            vec![
                vec!["Lên plan", "Review plan"],
                vec!["Code", "Review code"],
                vec!["QA", "Sửa"],
            ]
        );

        let tags: Vec<Vec<&str>> = resp
            .groups
            .iter()
            .map(|g| g.cards.iter().map(|c| c.tag.as_str()).collect())
            .collect();
        assert_eq!(
            tags,
            vec![
                vec!["lead", "plan-review"],
                vec!["dev", "code-review"],
                vec!["qa", "dev · fix"],
            ]
        );

        let group_rounds: Vec<Option<i64>> = resp.groups.iter().map(|g| g.round).collect();
        assert_eq!(group_rounds, vec![Some(2), Some(2), Some(2)]);

        let group_caps: Vec<Option<i64>> = resp.groups.iter().map(|g| g.cap).collect();
        assert_eq!(group_caps, vec![Some(3), Some(3), Some(3)]);
    }

    #[test]
    fn test_response_serializes_flat() {
        let task = dummy_task("dev_done");
        let runs = vec![staged_run(
            1,
            "lead",
            Some("plan"),
            Some(1),
            "succeeded",
            None,
            None,
            1000,
        )];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 5000,
        };
        let wf = test_mac_dinh();
        let flow = FlowFacts {
            workflow: Some(&wf),
            ..Default::default()
        };
        let resp = derive_response(&facts, &flow);
        let val = serde_json::to_value(&resp).unwrap();
        let obj = val.as_object().unwrap();
        let mut keys: Vec<String> = obj.keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "budget",
                "cards",
                "current",
                "current_card",
                "cycle",
                "gate",
                "groups",
                "ledger",
                "task_id",
                "workflow"
            ]
        );
        let wf_view = resp.workflow.as_ref().unwrap();
        assert_eq!(wf_view.summary, wf.summary());
        assert_eq!(wf_view.limits, "3 vòng · 60 phút · $20");
    }

    #[test]
    fn test_legacy_rows_extended_only() {
        let task = dummy_task("dev_done");
        let runs = vec![
            run(1, "lead", "succeeded", None, 1000, Some(2000)),
            run(2, "dev", "succeeded", None, 2000, Some(3000)),
            run(3, "qa", "succeeded", None, 3000, Some(4000)),
        ];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 10000,
        };
        let view = derive(&facts);
        let all_rows: Vec<&LedgerRow> = view.ledger.groups.iter().flat_map(|g| &g.rows).collect();
        assert_eq!(all_rows.len(), 3);

        assert_eq!(all_rows[0].loop_name, "plan");
        assert_eq!(all_rows[0].stage, None);
        assert_eq!(all_rows[0].kind, "plan");
        assert_eq!(all_rows[0].counts, None);
        assert_eq!(all_rows[0].label, "Vòng 1 · plan");

        assert_eq!(all_rows[1].loop_name, "build");
        assert_eq!(all_rows[1].stage, None);
        assert_eq!(all_rows[1].kind, "code");
        assert_eq!(all_rows[1].counts, None);
        assert_eq!(all_rows[1].label, "Vòng 1 · code");

        assert_eq!(all_rows[2].loop_name, "qa");
        assert_eq!(all_rows[2].stage, None);
        assert_eq!(all_rows[2].kind, "QA");
        assert_eq!(all_rows[2].counts, None);
        assert_eq!(all_rows[2].label, "Vòng 1 · QA");
    }

    #[test]
    fn test_wr03_violating_lead_does_not_wait() {
        let task = dummy_task("planned");
        // Violating lead: status succeeded + outcome protocol_violation
        let violating_lead = run(
            1,
            "lead",
            "succeeded",
            Some("protocol_violation"),
            1000,
            Some(2000),
        );
        let facts_violating = PipelineFacts {
            task: &task,
            runs: &[violating_lead],
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 3000,
        };
        let view_violating = derive(&facts_violating);
        assert_eq!(view_violating.ledger.groups[0].right, None);
        assert_eq!(view_violating.cards.ban.state, "cho");
        assert_eq!(view_violating.cards.ban.result, "chưa có plan");

        // Clean lead: status succeeded + outcome None
        let clean_lead = run(1, "lead", "succeeded", None, 1000, Some(2000));
        let facts_clean = PipelineFacts {
            task: &task,
            runs: &[clean_lead],
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 3000,
        };
        let view_clean = derive(&facts_clean);
        assert_eq!(
            view_clean.ledger.groups[0].right.as_deref(),
            Some("chờ bạn")
        );
        assert_eq!(view_clean.cards.ban.state, "cho_ban");
        assert_eq!(view_clean.cards.ban.result, "chờ duyệt plan");
    }

    #[test]
    fn test_wr02_checked() {
        let dev = RunRow {
            base_commit: Some("bbb2".to_string()),
            ..run(42, "dev", "succeeded", None, 1000, Some(2000))
        };
        let runs = vec![dev];

        // ancestry returns Some(true) -> Ok(Some(42))
        assert_eq!(
            approving_dev_run_checked(&runs, "aaa1", None, |_p, _b| Some(true)),
            Ok(Some(42))
        );

        // ancestry returns Some(false) -> Ok(None)
        assert_eq!(
            approving_dev_run_checked(&runs, "aaa1", None, |_p, _b| Some(false)),
            Ok(None)
        );

        // ancestry returns None -> Err(())
        assert_eq!(
            approving_dev_run_checked(&runs, "aaa1", None, |_p, _b| None),
            Err(())
        );

        // approving_dev_run (bool) still returns Some(42) / None
        assert_eq!(
            approving_dev_run(&runs, "aaa1", None, |_p, _b| true),
            Some(42)
        );
        assert_eq!(approving_dev_run(&runs, "aaa1", None, |_p, _b| false), None);
    }

    #[test]
    fn test_review_never_starts_cycle() {
        let task = dummy_task("dev_done");
        let wf = test_mac_dinh();

        // staged runs L, D, Q, PR (plan-review after QA)
        let runs1 = vec![
            staged_run(
                1,
                "lead",
                Some("plan"),
                Some(1),
                "succeeded",
                None,
                None,
                1000,
            ),
            staged_run(
                2,
                "dev",
                Some("implement"),
                Some(1),
                "succeeded",
                None,
                None,
                2000,
            ),
            staged_run(
                3,
                "qa",
                Some("qa"),
                Some(1),
                "succeeded",
                Some("qa:pass"),
                None,
                3000,
            ),
            staged_run(
                4,
                "plan-review",
                Some("plan-review"),
                Some(2),
                "succeeded",
                Some("review:approve"),
                None,
                4000,
            ),
        ];
        let facts1 = PipelineFacts {
            task: &task,
            runs: &runs1,
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 5000,
        };
        let flow1 = FlowFacts {
            workflow: Some(&wf),
            ..Default::default()
        };
        let resp1 = derive_response(&facts1, &flow1);
        let pr_group = resp1
            .base
            .ledger
            .groups
            .iter()
            .find(|g| g.rows.iter().any(|r| r.run_id == 4))
            .unwrap();
        assert_eq!(pr_group.cycle, 1);

        // L, D, Q, L -> the second L starts cycle 2
        let runs2 = vec![
            staged_run(
                1,
                "lead",
                Some("plan"),
                Some(1),
                "succeeded",
                None,
                None,
                1000,
            ),
            staged_run(
                2,
                "dev",
                Some("implement"),
                Some(1),
                "succeeded",
                None,
                None,
                2000,
            ),
            staged_run(
                3,
                "qa",
                Some("qa"),
                Some(1),
                "succeeded",
                Some("qa:pass"),
                None,
                3000,
            ),
            staged_run(
                4,
                "lead",
                Some("plan"),
                Some(2),
                "succeeded",
                None,
                None,
                4000,
            ),
        ];
        let facts2 = PipelineFacts {
            task: &task,
            runs: &runs2,
            plan: &PlanFacts::Unknown,
            current_step: "lead",
            now_ms: 5000,
        };
        let flow2 = FlowFacts {
            workflow: Some(&wf),
            ..Default::default()
        };
        let resp2 = derive_response(&facts2, &flow2);
        let second_lead_group = resp2
            .base
            .ledger
            .groups
            .iter()
            .find(|g| g.rows.iter().any(|r| r.run_id == 4))
            .unwrap();
        assert_eq!(second_lead_group.cycle, 2);
    }

    fn human_run(
        id: i64,
        role: &str,
        staff_metrics: Option<&str>,
        started: i64,
        ended: i64,
    ) -> RunRow {
        let mut r = run(id, role, "succeeded", None, started, Some(ended));
        r.engine = crate::engine::ENGINE_HUMAN.to_string();
        r.model = None;
        r.pid = None;
        r.pgid = None;
        r.pid_start = None;
        r.input_tokens = 0;
        r.output_tokens = 0;
        r.cache_read_tokens = 0;
        r.cache_write_tokens = 0;
        r.cost_usd = 0.0;
        r.num_turns = None;
        r.usage_source = "none".to_string();
        r.metrics_json = staff_metrics.map(|s| s.to_string());
        r
    }

    fn flow_card(resp: &PipelineResponse, key: &str) -> FlowCard {
        resp.groups
            .iter()
            .flat_map(|g| g.cards.iter())
            .find(|c| c.key == key)
            .unwrap()
            .clone()
    }

    #[test]
    fn ledger_engine_and_human_rows() {
        let task = dummy_task("planned");
        let mut ds = run(2, "dev", "succeeded", None, 2000, Some(5000));
        ds.engine = "deepseek".to_string();
        let runs = vec![
            run(1, "lead", "succeeded", None, 1000, Some(2000)),
            ds,
            human_run(3, "dev", Some(r#"{"team":"x","staff":"ha"}"#), 6000, 66_000),
        ];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 100_000,
        };
        let view = derive(&facts);
        let rows: Vec<&LedgerRow> = view
            .ledger
            .groups
            .iter()
            .flat_map(|g| g.rows.iter())
            .collect();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].engine, "claude");
        assert!(!rows[0].human);
        assert_eq!(rows[0].turns, Some(2));
        assert_eq!(rows[1].engine, "deepseek");
        assert!(!rows[1].human);

        assert_eq!(rows[2].engine, "người:ha");
        assert!(rows[2].human);
        assert_eq!(rows[2].turns, None);
        assert_eq!(rows[2].cost_usd, None);
        // The person's own minutes are still shown on the row ...
        assert_eq!(rows[2].duration_ms, Some(60_000));
        // ... but the total sums agent runs only and counts the person's run apart.
        let total = &view.ledger.total;
        assert_eq!(total.runs, 3);
        assert_eq!(total.human_runs, 1);
        assert_eq!(total.duration_ms, Some(1000 + 3000));
        assert_eq!(total.cost_unknown, 0);

        // No staff id (or one that is not an id) reads as the bare engine.
        assert_eq!(engine_text_of(&human_run(9, "dev", None, 0, 1)), "người");
        assert_eq!(
            engine_text_of(&human_run(9, "dev", Some(r#"{"staff":"../x"}"#), 0, 1)),
            "người"
        );
        assert_eq!(
            engine_text_of(&run(9, "dev", "succeeded", None, 0, Some(1))),
            "claude"
        );
    }

    #[test]
    fn planned_engines_fill_empty_cards() {
        let task = dummy_task("planned");
        let wf = test_mac_dinh();
        let runs = vec![staged_run(
            1,
            "lead",
            Some("plan"),
            Some(1),
            "succeeded",
            None,
            None,
            1000,
        )];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 5000,
        };
        let flow = FlowFacts {
            workflow: Some(&wf),
            ..Default::default()
        };
        let mut resp = derive_response(&facts, &flow);
        // Before the fill: the card with a run names its engine, the others none.
        assert_eq!(flow_card(&resp, "plan").engine.as_deref(), Some("claude"));
        assert!(!flow_card(&resp, "plan").engine_planned);
        assert_eq!(flow_card(&resp, "code").engine, None);

        let planned: std::collections::BTreeMap<String, String> = [
            ("lead", "deepseek"),
            ("plan-review", "agy"),
            ("dev", "người:ha"),
            ("code-review", "claude"),
            ("qa", "claude"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        set_planned_engines(&mut resp, &planned);

        // The run's own engine wins over the plan for the lead (deepseek in the map).
        assert_eq!(flow_card(&resp, "plan").engine.as_deref(), Some("claude"));
        assert!(!flow_card(&resp, "plan").engine_planned);
        for (key, engine) in [
            ("plan_review", "agy"),
            ("code", "người:ha"),
            ("code_review", "claude"),
            ("qa", "claude"),
            ("fix", "người:ha"),
        ] {
            let c = flow_card(&resp, key);
            assert_eq!(c.engine.as_deref(), Some(engine), "card {key}");
            assert!(c.engine_planned, "card {key}");
        }

        // An empty map falls back to claude.
        let mut resp = derive_response(&facts, &flow);
        set_planned_engines(&mut resp, &std::collections::BTreeMap::new());
        assert_eq!(flow_card(&resp, "code").engine.as_deref(), Some("claude"));
        assert!(flow_card(&resp, "code").engine_planned);
    }

    #[test]
    fn card_engine_follows_latest_run() {
        let task = dummy_task("planned");
        let mut first = run(1, "dev", "failed", Some("quota"), 1000, Some(2000));
        first.engine = "deepseek".to_string();
        let second = run(2, "dev", "succeeded", None, 3000, Some(4000));
        let runs = vec![first, second];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "qa",
            now_ms: 5000,
        };
        let resp = derive_response(&facts, &FlowFacts::default());
        let code = flow_card(&resp, "code");
        assert_eq!(code.engine.as_deref(), Some("claude"));
        assert!(!code.engine_planned);
    }

    #[test]
    fn quota_state_word() {
        assert_eq!(state_word("het_quota"), "hết quota");
        assert_eq!(state_word("cho_nguoi"), "chờ người");

        let task = dummy_task("planned");
        let runs = vec![run(1, "dev", "failed", Some("quota"), 1000, Some(2000))];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 5000,
        };
        let view = derive(&facts);
        let row = &view.ledger.groups[0].rows[0];
        assert_eq!(row.state, "het_quota");
        assert_eq!(row.verdict, "hết quota");
        let resp = derive_response(&facts, &FlowFacts::default());
        let code = flow_card(&resp, "code");
        assert_eq!(code.state, "het_quota");
        assert_eq!(code.state_word, "hết quota");
        assert_eq!(code.result, "hết quota");
    }

    fn human_wait(mode: &str, stage: &str, fix_for: Option<&str>) -> crate::teams::HumanWait {
        crate::teams::HumanWait {
            mode: mode.to_string(),
            role: "dev".to_string(),
            stage: stage.to_string(),
            fix_for: fix_for.map(str::to_string),
            round: None,
            staff: "ha".to_string(),
            since_ms: 4000,
            branch: "task/sua-app".to_string(),
            base: "abc".to_string(),
            report_paths: vec![],
            push: None,
            autopilot: false,
        }
    }

    #[test]
    fn human_wait_synthesized_row() {
        let task = dummy_task("planned");
        let wf = test_mac_dinh();
        let runs = vec![
            staged_run(
                1,
                "lead",
                Some("plan"),
                Some(1),
                "succeeded",
                None,
                None,
                1000,
            ),
            staged_run(
                2,
                "plan-review",
                Some("plan-review"),
                Some(1),
                "succeeded",
                Some("review:approve"),
                None,
                2000,
            ),
        ];
        let facts = PipelineFacts {
            task: &task,
            runs: &runs,
            plan: &PlanFacts::Unknown,
            current_step: "dev",
            now_ms: 9000,
        };
        let flow = FlowFacts {
            workflow: Some(&wf),
            ..Default::default()
        };
        let plain = derive_response(&facts, &flow);

        // implement stage: a new build group with one synthesized row; `total` is unchanged.
        let mut resp = plain.clone();
        apply_human_wait(
            &mut resp,
            &human_wait("waiting", "implement", None),
            "Hà",
            9000,
        );
        assert_eq!(resp.base.ledger.total, plain.base.ledger.total);
        assert_eq!(
            resp.base.ledger.groups.len(),
            plain.base.ledger.groups.len() + 1
        );
        let g = resp.base.ledger.groups.last().unwrap();
        assert_eq!(g.kind, "build");
        assert_eq!(g.header, "Chu kỳ 1 · build");
        assert_eq!(g.rows.len(), 1);
        let row = &g.rows[0];
        assert_eq!(row.run_id, 0);
        assert!(row.human);
        assert!(!row.running);
        assert_eq!(row.engine, "người:ha");
        assert_eq!(row.state, "cho_nguoi");
        assert_eq!(row.verdict, "chờ người");
        assert_eq!(row.started_at, 4000);
        assert_eq!(row.duration_ms, Some(5000));
        assert_eq!(row.kind, "code");
        assert_eq!(row.stage.as_deref(), Some("implement"));
        assert_eq!(row.label, "Vòng 1 · code · người Hà");
        assert_eq!(row.cost_usd, None);
        let code = flow_card(&resp, "code");
        assert_eq!(code.state, "cho_nguoi");
        assert_eq!(code.state_word, "chờ người");
        assert_eq!(code.result, "Chờ Hà làm");
        assert_eq!(code.engine.as_deref(), Some("người:ha"));
        assert!(!code.engine_planned);
        assert_eq!(resp.base.cards.code.state, "cho_nguoi");
        assert_eq!(resp.current_card.as_deref(), Some("code"));

        // A fix for QA after a QA run: the row joins the QA group on the fix card.
        let mut qa_runs = runs.clone();
        qa_runs.push(staged_run(
            3,
            "qa",
            Some("qa"),
            Some(1),
            "succeeded",
            Some("qa:fix"),
            None,
            3000,
        ));
        let facts = PipelineFacts {
            runs: &qa_runs,
            ..facts
        };
        let plain = derive_response(&facts, &flow);
        let mut wait = human_wait("rejected", "fix", Some("qa"));
        wait.round = Some(2);
        let mut resp = plain.clone();
        apply_human_wait(&mut resp, &wait, "Hà", 9000);
        assert_eq!(resp.base.ledger.total, plain.base.ledger.total);
        assert_eq!(
            resp.base.ledger.groups.len(),
            plain.base.ledger.groups.len()
        );
        let row = resp
            .base
            .ledger
            .groups
            .last()
            .unwrap()
            .rows
            .last()
            .unwrap()
            .clone();
        assert_eq!(row.kind, "sửa theo QA");
        assert_eq!(row.loop_name, "qa");
        assert_eq!(row.label, "Vòng 2 · sửa theo QA · người Hà");
        assert_eq!(row.run_id, 0);
        let fix = flow_card(&resp, "fix");
        assert_eq!(fix.state, "cho_nguoi");
        assert_eq!(fix.result, "Chờ Hà làm");
        assert_eq!(resp.current_card.as_deref(), Some("fix"));
        // The code card is not the one waiting.
        assert_ne!(flow_card(&resp, "code").state, "cho_nguoi");

        // A task without a workflow (legacy flow) updates its code card too.
        let facts = PipelineFacts {
            runs: &runs[..1],
            ..facts
        };
        let mut resp = derive_response(&facts, &FlowFacts::default());
        apply_human_wait(
            &mut resp,
            &human_wait("waiting", "implement", None),
            "Hà",
            9000,
        );
        assert_eq!(flow_card(&resp, "code").state, "cho_nguoi");
        assert_eq!(resp.base.ledger.total.runs, 1);
    }
}
