//! Pipeline của task: thẻ giai đoạn, sổ lượt chạy và dòng tổng, suy ra thuần từ runs.

use crate::dashboard::taskstate;
use crate::db::{RunRow, TaskRow};

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
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct LedgerTotal {
    pub runs: i64,
    pub duration_ms: Option<i64>,
    pub cost_usd: Option<f64>,
    pub cost_unknown: i64,
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
        _ => "lỗi",
    }
}

pub fn derive(f: &PipelineFacts) -> PipelineView {
    // R1 Input: keep runs whose role is lead, dev or qa (drop inspect); sort by (started_at ASC, id ASC)
    let mut valid_runs: Vec<&RunRow> = f
        .runs
        .iter()
        .filter(|r| r.role == "lead" || r.role == "dev" || r.role == "qa")
        .collect();
    valid_runs.sort_by_key(|r| (r.started_at, r.id));

    // R2 Round: per role, 1-based counter over the sorted runs (task-wide)
    let mut lead_round = 0i64;
    let mut dev_round = 0i64;
    let mut qa_round = 0i64;

    let mut ledger_rows: Vec<LedgerRow> = Vec::with_capacity(valid_runs.len());
    for run in &valid_runs {
        let round = match run.role.as_str() {
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
            _ => 0,
        };

        // R3 Row state: taskstate::step_state(Some(run)).0, then dat when xong and outcome == Some("qa:pass")
        let (raw_state, _) = taskstate::step_state(Some(run));
        let state = if raw_state == "xong" && run.outcome.as_deref() == Some("qa:pass") {
            "dat"
        } else {
            raw_state
        };
        let verdict = state_word(state).to_string();

        // R4 Unknown fields
        let duration_ms = if run.status == "running" {
            Some((f.now_ms - run.started_at).max(0))
        } else if let Some(ended) = run.ended_at {
            Some((ended - run.started_at).max(0))
        } else {
            None
        };

        let cost_usd =
            if run.status != "killed" && run.status != "running" && run.usage_source == "result" {
                Some(run.cost_usd)
            } else {
                None
            };

        // R5 Label
        let label = match run.role.as_str() {
            "lead" => format!("Vòng {round} · plan"),
            "dev" => format!("Vòng {round} · code"),
            "qa" => format!("Vòng {round} · QA"),
            _ => format!("Vòng {round} · {}", run.role),
        };

        ledger_rows.push(LedgerRow {
            run_id: run.id,
            role: run.role.clone(),
            round,
            label,
            state: state.to_string(),
            verdict,
            turns: run.num_turns,
            duration_ms,
            cost_usd,
            running: run.status == "running",
            started_at: run.started_at,
            ended_at: run.ended_at,
            input_tokens: run.input_tokens,
            output_tokens: run.output_tokens,
            cache_read_tokens: run.cache_read_tokens,
            cache_write_tokens: run.cache_write_tokens,
        });
    }

    // R6 Groups: maximal runs of consecutive rows with the same role
    struct GroupIndices {
        kind: &'static str,
        role: String,
        run_indices: Vec<usize>,
    }

    let mut raw_groups: Vec<GroupIndices> = Vec::new();
    for (idx, run) in valid_runs.iter().enumerate() {
        let kind = match run.role.as_str() {
            "lead" => "plan",
            "dev" => "build",
            "qa" => "qa",
            _ => "unknown",
        };
        if let Some(last) = raw_groups.last_mut() {
            if last.role == run.role {
                last.run_indices.push(idx);
                continue;
            }
        }
        raw_groups.push(GroupIndices {
            kind,
            role: run.role.clone(),
            run_indices: vec![idx],
        });
    }

    // R7 Cycle: starts at 1; plan group whose prev group is build or qa increments it
    let mut cycle_counter = 1i64;
    let mut groups: Vec<LedgerGroup> = Vec::with_capacity(raw_groups.len());

    for (i, rg) in raw_groups.iter().enumerate() {
        let n = (i + 1) as i64;
        if i > 0 && rg.kind == "plan" {
            let prev_kind = raw_groups[i - 1].kind;
            if prev_kind == "build" || prev_kind == "qa" {
                cycle_counter += 1;
            }
        }
        let cycle = cycle_counter;

        // R8 Header
        let header = match rg.kind {
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
                if is_last_plan_group && last_run.status == "succeeded" && !dev_after {
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
            kind: rg.kind.to_string(),
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

    for row in &ledger_rows {
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
                .filter(|r| r.role == "lead" && r.status == "succeeded")
                .last();
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
                .filter(|r| r.role == "lead" && r.status == "succeeded")
                .last();
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

pub fn approving_dev_run(
    runs: &[RunRow],
    plan_sha: &str,
    plan_at_ms: Option<i64>,
    is_ancestor: impl Fn(&str, &str) -> bool,
) -> Option<i64> {
    let mut dev_runs: Vec<&RunRow> = runs.iter().filter(|r| r.role == "dev").collect();
    dev_runs.sort_by_key(|r| (r.started_at, r.id));

    for r in dev_runs {
        match r.base_commit.as_deref() {
            Some(b) if !b.is_empty() => {
                if is_ancestor(plan_sha, b) {
                    return Some(r.id);
                }
            }
            _ => {
                if let Some(t) = plan_at_ms {
                    if r.started_at >= t {
                        return Some(r.id);
                    }
                }
            }
        }
    }
    None
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
}
