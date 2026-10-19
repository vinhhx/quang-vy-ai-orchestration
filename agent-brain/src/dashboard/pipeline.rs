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
}
