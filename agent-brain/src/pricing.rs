use crate::config::Price;
use crate::db::RunRow;
use crate::workflows::Workflow;
use std::collections::BTreeMap;

/// Key prefix of an engine's own price in the pricing table (`engine:<id>`, D-23). Family keys
/// (`opus`, `sonnet`, ...) never carry it, so the two kinds of key cannot collide.
pub const ENGINE_PRICE_PREFIX: &str = "engine:";
/// Key prefix of the marker for a native entry (`native:<id>`): a claude-family engine that is
/// priced by model family like `claude`, never by its own price.
pub const NATIVE_ENGINE_PREFIX: &str = "native:";

pub fn engine_key(id: &str) -> String {
    format!("{ENGINE_PRICE_PREFIX}{id}")
}

pub fn native_key(id: &str) -> String {
    format!("{NATIVE_ENGINE_PREFIX}{id}")
}

fn is_engine_key(key: &str) -> bool {
    key.starts_with(ENGINE_PRICE_PREFIX) || key.starts_with(NATIVE_ENGINE_PREFIX)
}

/// Tra cứu giá theo model family (lowercased substring match) hoặc lấy họ đắt nhất trong bảng.
/// Nếu bảng rỗng, fallback về giá mặc định. Các khóa giá của engine (`engine:<id>`, `native:<id>`)
/// không bao giờ tham gia: giá của một gateway không được làm đổi giá Claude (Pitfall A10).
pub fn price_for(model: Option<&str>, table: &BTreeMap<String, Price>) -> Price {
    let families = || table.iter().filter(|(k, _)| !is_engine_key(k));

    if let Some(m) = model {
        let m_lower = m.to_ascii_lowercase();
        for (family, p) in families() {
            if m_lower.contains(family.as_str()) {
                return *p;
            }
        }
    }

    families()
        .map(|(_, p)| p)
        .max_by(|a, b| {
            a.input
                .partial_cmp(&b.input)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
        .unwrap_or(Price {
            input: 4.0,
            output: 20.0,
            cache_read: 0.2,
            cache_write: 8.0,
        })
}

/// Price of an engine's own entry (`engine:<id>`, filed by the config loader from the entry's
/// `pricing` or the legacy `pricing.<id>`). `claude` and a native entry give `None`: the
/// model-family lookup (`price_for`) applies to them instead. A person (`người`) has no price either.
pub fn engine_price(engine: &str, table: &BTreeMap<String, Price>) -> Option<Price> {
    if engine == crate::engine::ENGINE_CLAUDE || engine == crate::engine::ENGINE_HUMAN {
        return None;
    }
    table.get(&engine_key(engine)).copied()
}

/// USD cost of `usage` on a non-claude engine; `None` for claude and for an unpriced engine.
pub fn cost_for_engine(
    engine: &str,
    usage: &crate::stream::Usage,
    table: &BTreeMap<String, Price>,
) -> Option<f64> {
    let p = engine_price(engine, table)?;
    Some(
        (usage.input_tokens as f64 * p.input
            + usage.output_tokens as f64 * p.output
            + usage.cache_read_input_tokens as f64 * p.cache_read
            + usage.cache_creation_input_tokens as f64 * p.cache_write)
            / 1_000_000.0,
    )
}

/// Whether the engine has a price: claude always (family lookup), a person never, a native entry
/// always (family lookup, like claude), else the `engine:<id>` key exists.
pub fn is_priced(engine: &str, table: &BTreeMap<String, Price>) -> bool {
    if engine == crate::engine::ENGINE_CLAUDE {
        return true;
    }
    if engine == crate::engine::ENGINE_HUMAN {
        return false;
    }
    table.contains_key(&native_key(engine)) || table.contains_key(&engine_key(engine))
}

/// Ước tính chi phí USD dựa trên token và bảng giá.
/// Các hệ số nhân lấy từ bảng config.json đã khớp với các run thực tế (RESEARCH §3.8).
/// Một run bị killed hoặc partial luôn được ước tính để trần ngân sách không bị bypass (D-19).
pub fn estimate_usd(r: &RunRow, table: &BTreeMap<String, Price>) -> f64 {
    // D-23 / Pitfall 6: a gateway or agy run is priced only from its entry's price; with none it
    // costs 0 and is never billed at the most expensive Anthropic family. A native entry runs
    // claude, so it is priced by model family like `claude`.
    let p =
        if r.engine == crate::engine::ENGINE_CLAUDE || table.contains_key(&native_key(&r.engine)) {
            price_for(r.model.as_deref(), table)
        } else {
            match engine_price(&r.engine, table) {
                Some(p) => p,
                None => return 0.0,
            }
        };
    (r.input_tokens as f64 * p.input
        + r.output_tokens as f64 * p.output
        + r.cache_read_tokens as f64 * p.cache_read
        + r.cache_write_tokens as f64 * p.cache_write)
        / 1_000_000.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunCost {
    pub usd: f64,
    pub estimated: bool,
    pub counted: bool,
}

pub fn run_cost(r: &RunRow, table: &BTreeMap<String, Price>) -> RunCost {
    if r.status == "running" {
        RunCost {
            usd: 0.0,
            estimated: false,
            counted: false,
        }
    } else if r.usage_source == "result" && r.status != "killed" {
        RunCost {
            usd: r.cost_usd,
            estimated: false,
            counted: true,
        }
    } else {
        RunCost {
            usd: estimate_usd(r, table),
            estimated: true,
            counted: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize)]
pub struct BudgetUsage {
    pub used_ms: i64,
    pub used_usd: f64,
    pub used_cents: i64,
    pub estimated_runs: i64,
}

pub fn budget_usage(runs: &[RunRow], table: &BTreeMap<String, Price>, now_ms: i64) -> BudgetUsage {
    let mut used_ms: i64 = 0;
    let mut used_usd: f64 = 0.0;
    let mut estimated_runs: i64 = 0;

    for r in runs {
        if r.role == "inspect" {
            continue;
        }

        let dur = if r.status == "running" {
            now_ms - r.started_at
        } else if let Some(ended) = r.ended_at {
            ended - r.started_at
        } else {
            0
        };
        if dur > 0 {
            used_ms += dur;
        }

        let cost = run_cost(r, table);
        if cost.counted {
            used_usd += cost.usd;
            if cost.estimated {
                estimated_runs += 1;
            }
        }
    }

    let used_cents = (used_usd * 100.0).round() as i64;
    BudgetUsage {
        used_ms,
        used_usd,
        used_cents,
        estimated_runs,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BudgetLimits {
    pub minutes: i64,
    pub usd: f64,
}

impl BudgetLimits {
    pub fn from_workflow(wf: &Workflow) -> Self {
        Self {
            minutes: wf.budget_minutes as i64,
            usd: wf.budget_usd,
        }
    }

    pub fn limit_ms(&self) -> i64 {
        self.minutes * 60_000
    }

    pub fn limit_cents(&self) -> i64 {
        (self.usd * 100.0).round() as i64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BudgetLevel {
    Ok,
    Near,
    Over,
}

pub fn budget_level(u: &BudgetUsage, l: &BudgetLimits) -> BudgetLevel {
    let limit_ms = l.limit_ms();
    let limit_cents = l.limit_cents();

    if u.used_ms >= limit_ms || u.used_cents >= limit_cents {
        BudgetLevel::Over
    } else if u.used_ms * 5 >= limit_ms * 4 || u.used_cents * 5 >= limit_cents * 4 {
        BudgetLevel::Near
    } else {
        BudgetLevel::Ok
    }
}

pub fn exhausted(u: &BudgetUsage, l: &BudgetLimits) -> bool {
    budget_level(u, l) == BudgetLevel::Over
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_run(
        role: &str,
        status: &str,
        model: Option<&str>,
        usage_source: &str,
        cost_usd: f64,
        started_at: i64,
        ended_at: Option<i64>,
        in_tok: i64,
        out_tok: i64,
        cr_tok: i64,
        cw_tok: i64,
    ) -> RunRow {
        RunRow {
            id: 1,
            task_id: Some(1),
            project_id: 1,
            role: role.to_string(),
            engine: "claude".to_string(),
            profile: "work".to_string(),
            model: model.map(|s| s.to_string()),
            session_id: None,
            pid: None,
            pgid: None,
            pid_start: None,
            status: status.to_string(),
            outcome: None,
            exit_code: None,
            kill_requested_at: None,
            started_at,
            ended_at,
            input_tokens: in_tok,
            output_tokens: out_tok,
            cache_read_tokens: cr_tok,
            cache_write_tokens: cw_tok,
            cost_usd,
            num_turns: None,
            assistant_turns: None,
            turns_before_first_edit: None,
            reads_outside_plan: None,
            permission_denials: 0,
            usage_source: usage_source.to_string(),
            base_commit: None,
            run_dir: None,
            error: None,
            stage: None,
            round: None,
            metrics_json: None,
        }
    }

    #[test]
    fn test_fitted_estimate() {
        let table = crate::config::default_pricing();
        let r = dummy_run(
            "lead",
            "killed",
            Some("claude-opus-5-5"),
            "partial",
            0.0,
            0,
            Some(1000),
            4,
            20,
            33917,
            6536,
        );
        let est = estimate_usd(&r, &table);
        assert!((est - 0.0594874).abs() < 1e-9);

        let cost = run_cost(&r, &table);
        assert!(cost.estimated);
        assert!(cost.counted);
        assert!((cost.usd - 0.0594874).abs() < 1e-9);
    }

    #[test]
    fn test_stored_cost_wins() {
        let table = crate::config::default_pricing();
        let r = dummy_run(
            "dev",
            "succeeded",
            Some("claude-sonnet-5-5"),
            "result",
            1.32,
            0,
            Some(1000),
            100,
            200,
            300,
            400,
        );
        let cost = run_cost(&r, &table);
        assert_eq!(
            cost,
            RunCost {
                usd: 1.32,
                estimated: false,
                counted: true,
            }
        );
    }

    #[test]
    fn test_killed_with_result_is_estimated() {
        let table = crate::config::default_pricing();
        let r = dummy_run(
            "dev",
            "killed",
            Some("claude-sonnet-5-5"),
            "result",
            0.0,
            0,
            Some(1000),
            100,
            200,
            0,
            0,
        );
        let cost = run_cost(&r, &table);
        assert!(cost.estimated);
        assert!(cost.counted);
    }

    #[test]
    fn test_running_not_counted() {
        let table = crate::config::default_pricing();
        let now = 100_000;
        let r = dummy_run(
            "dev",
            "running",
            Some("claude-opus-5-5"),
            "none",
            0.0,
            now - 90_000,
            None,
            0,
            0,
            0,
            0,
        );
        let cost = run_cost(&r, &table);
        assert!(!cost.counted);
        assert!(!cost.estimated);
        assert_eq!(cost.usd, 0.0);

        let u = budget_usage(&[r], &table, now);
        assert_eq!(u.used_ms, 90_000);
        assert_eq!(u.used_usd, 0.0);
        assert_eq!(u.estimated_runs, 0);
    }

    #[test]
    fn test_unknown_model_is_most_expensive() {
        let table = crate::config::default_pricing();
        let opus_price = table.get("opus").copied().unwrap();

        let p1 = price_for(Some("gpt-x"), &table);
        assert_eq!(p1, opus_price);

        let p2 = price_for(None, &table);
        assert_eq!(p2, opus_price);
    }

    #[test]
    fn test_estimate_monotonic() {
        let table = crate::config::default_pricing();
        let base = dummy_run(
            "lead",
            "killed",
            Some("claude-opus-5-5"),
            "partial",
            0.0,
            0,
            Some(1000),
            10,
            10,
            10,
            10,
        );
        let base_est = estimate_usd(&base, &table);

        let mut r_in = base.clone();
        r_in.input_tokens += 10;
        assert!(estimate_usd(&r_in, &table) >= base_est);

        let mut r_out = base.clone();
        r_out.output_tokens += 10;
        assert!(estimate_usd(&r_out, &table) >= base_est);

        let mut r_cr = base.clone();
        r_cr.cache_read_tokens += 10;
        assert!(estimate_usd(&r_cr, &table) >= base_est);

        let mut r_cw = base.clone();
        r_cw.cache_write_tokens += 10;
        assert!(estimate_usd(&r_cw, &table) >= base_est);
    }

    #[test]
    fn test_budget_usage_sums() {
        let table = crate::config::default_pricing();
        let r_lead = dummy_run(
            "lead",
            "succeeded",
            Some("claude-sonnet-5-5"),
            "result",
            1.25,
            0,
            Some(60_000),
            100,
            100,
            0,
            0,
        );
        let r_dev = dummy_run(
            "dev",
            "killed",
            Some("claude-opus-5-5"),
            "partial",
            0.0,
            60_000,
            Some(90_000),
            4,
            20,
            33917,
            6536,
        );
        let dev_est = estimate_usd(&r_dev, &table);
        let r_qa = dummy_run(
            "qa",
            "succeeded",
            Some("claude-sonnet-5-5"),
            "result",
            0.50,
            90_000,
            Some(150_000),
            100,
            100,
            0,
            0,
        );
        let r_inspect = dummy_run(
            "inspect",
            "succeeded",
            Some("claude-opus-5-5"),
            "result",
            9.99,
            0,
            Some(10_000),
            100,
            100,
            0,
            0,
        );

        let runs = vec![r_lead, r_dev, r_qa, r_inspect];
        let usage = budget_usage(&runs, &table, 150_000);

        assert_eq!(usage.used_ms, 150_000);
        let expected_usd = 1.75 + dev_est;
        assert!((usage.used_usd - expected_usd).abs() < 1e-9);
        assert_eq!(usage.used_cents, (expected_usd * 100.0).round() as i64);
        assert_eq!(usage.estimated_runs, 1);
    }

    #[test]
    fn test_levels() {
        let limits = BudgetLimits {
            minutes: 60,
            usd: 20.0,
        };

        // Time levels: 47 min -> Ok, 48 min -> Near, 60 min -> Over, 59 min 59.999 s -> Near
        let u_47m = BudgetUsage {
            used_ms: 47 * 60_000,
            used_usd: 10.0,
            used_cents: 1000,
            estimated_runs: 0,
        };
        assert_eq!(budget_level(&u_47m, &limits), BudgetLevel::Ok);
        assert!(!exhausted(&u_47m, &limits));

        let u_48m = BudgetUsage {
            used_ms: 48 * 60_000,
            used_usd: 10.0,
            used_cents: 1000,
            estimated_runs: 0,
        };
        assert_eq!(budget_level(&u_48m, &limits), BudgetLevel::Near);
        assert!(!exhausted(&u_48m, &limits));

        let u_59m_sub = BudgetUsage {
            used_ms: 60 * 60_000 - 1,
            used_usd: 10.0,
            used_cents: 1000,
            estimated_runs: 0,
        };
        assert_eq!(budget_level(&u_59m_sub, &limits), BudgetLevel::Near);

        let u_60m = BudgetUsage {
            used_ms: 60 * 60_000,
            used_usd: 10.0,
            used_cents: 1000,
            estimated_runs: 0,
        };
        assert_eq!(budget_level(&u_60m, &limits), BudgetLevel::Over);
        assert!(exhausted(&u_60m, &limits));

        // Cost levels: $15.99 -> Ok, $16.00 -> Near, $19.994 -> Near, $19.995 -> Over, $20 -> Over
        let u_15_99 = BudgetUsage {
            used_ms: 0,
            used_usd: 15.99,
            used_cents: (15.99 * 100.0_f64).round() as i64,
            estimated_runs: 0,
        };
        assert_eq!(budget_level(&u_15_99, &limits), BudgetLevel::Ok);

        let u_16_00 = BudgetUsage {
            used_ms: 0,
            used_usd: 16.00,
            used_cents: (16.00 * 100.0_f64).round() as i64,
            estimated_runs: 0,
        };
        assert_eq!(budget_level(&u_16_00, &limits), BudgetLevel::Near);

        let u_19_994 = BudgetUsage {
            used_ms: 0,
            used_usd: 19.994,
            used_cents: (19.994 * 100.0_f64).round() as i64, // 1999 cents
            estimated_runs: 0,
        };
        assert_eq!(u_19_994.used_cents, 1999);
        assert_eq!(budget_level(&u_19_994, &limits), BudgetLevel::Near);

        let u_19_995 = BudgetUsage {
            used_ms: 0,
            used_usd: 19.995,
            used_cents: (19.995 * 100.0_f64).round() as i64, // 2000 cents
            estimated_runs: 0,
        };
        assert_eq!(u_19_995.used_cents, 2000);
        assert_eq!(budget_level(&u_19_995, &limits), BudgetLevel::Over);
        assert!(exhausted(&u_19_995, &limits));

        let u_20_00 = BudgetUsage {
            used_ms: 0,
            used_usd: 20.00,
            used_cents: (20.00 * 100.0_f64).round() as i64,
            estimated_runs: 0,
        };
        assert_eq!(budget_level(&u_20_00, &limits), BudgetLevel::Over);
        assert!(exhausted(&u_20_00, &limits));
    }

    #[test]
    fn test_limits_from_workflow() {
        let mac_dinh = Workflow {
            name: "mac-dinh".to_string(),
            stages: vec![],
            loops: vec![],
            max_rounds: 3,
            budget_minutes: 60,
            budget_usd: 20.0,
            auto: true,
            source: "builtin".to_string(),
        };
        let limits = BudgetLimits::from_workflow(&mac_dinh);
        assert_eq!(
            limits,
            BudgetLimits {
                minutes: 60,
                usd: 20.0
            }
        );
        assert_eq!(limits.limit_ms(), 3_600_000);
        assert_eq!(limits.limit_cents(), 2000);
    }

    fn deepseek_table(input: Option<f64>) -> BTreeMap<String, Price> {
        let mut t = crate::config::default_pricing();
        if let Some(input) = input {
            t.insert(
                "engine:deepseek".to_string(),
                Price {
                    input,
                    output: 1.10,
                    cache_read: 0.07,
                    cache_write: 0.27,
                },
            );
        }
        t
    }

    #[test]
    fn engine_first_pricing() {
        let none = deepseek_table(None);
        assert_eq!(engine_price("claude", &none), None);
        assert_eq!(engine_price("deepseek", &none), None);
        assert!(is_priced("claude", &none));
        assert!(!is_priced("người", &none));
        assert!(!is_priced("agy", &none));
        assert!(!is_priced("deepseek", &none));

        let with = deepseek_table(Some(0.27));
        assert_eq!(engine_price("claude", &with), None);
        assert_eq!(engine_price("deepseek", &with).unwrap().input, 0.27);
        assert!(is_priced("deepseek", &with));
        assert!(!is_priced("người", &with));
        assert!(!is_priced("agy", &with));

        let mut agy = with.clone();
        agy.insert("engine:agy".to_string(), with["engine:deepseek"]);
        assert!(is_priced("agy", &agy));

        let usage = crate::stream::Usage {
            input_tokens: 2_000_000,
            output_tokens: 1_000_000,
            cache_read_input_tokens: 1_000_000,
            cache_creation_input_tokens: 1_000_000,
        };
        assert_eq!(cost_for_engine("claude", &usage, &with), None);
        assert_eq!(cost_for_engine("deepseek", &usage, &none), None);
        let cost = cost_for_engine("deepseek", &usage, &with).unwrap();
        assert!(
            (cost - (2.0 * 0.27 + 1.10 + 0.07 + 0.27)).abs() < 1e-9,
            "{cost}"
        );
    }

    #[test]
    fn estimate_non_claude_unpriced_is_zero() {
        let mut r = dummy_run(
            "dev",
            "killed",
            Some("sonnet"),
            "partial",
            0.0,
            0,
            Some(1000),
            1_000_000,
            0,
            0,
            0,
        );
        r.engine = "deepseek".to_string();

        // Never the sonnet or the opus price.
        assert_eq!(estimate_usd(&r, &deepseek_table(None)), 0.0);
        let est = estimate_usd(&r, &deepseek_table(Some(0.27)));
        assert!((est - 0.27).abs() < 1e-9, "{est}");

        // A person's run is never priced.
        r.engine = "người".to_string();
        assert_eq!(estimate_usd(&r, &deepseek_table(Some(0.27))), 0.0);

        // A claude run keeps the family lookup (sonnet: 1M input tokens = its input price).
        r.engine = "claude".to_string();
        let table = crate::config::default_pricing();
        let expected = price_for(Some("sonnet"), &table).input;
        assert!((estimate_usd(&r, &table) - expected).abs() < 1e-9);
    }

    #[test]
    fn engine_keys() {
        let gw = Price {
            input: 99.0,
            output: 99.0,
            cache_read: 99.0,
            cache_write: 99.0,
        };
        let base = crate::config::default_pricing();
        let mut t = base.clone();
        t.insert("engine:opus-gw".to_string(), gw);
        t.insert(
            "native:claude-max".to_string(),
            Price {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
        );

        // Pitfall A10: engine prices never move the Claude family lookup, neither the
        // substring match nor the most-expensive fallback.
        assert_eq!(
            price_for(Some("claude-opus-4"), &t),
            price_for(Some("claude-opus-4"), &base)
        );
        assert_eq!(
            price_for(Some("mystery-model"), &t),
            price_for(Some("mystery-model"), &base)
        );
        assert_eq!(
            price_for(Some("engine:opus-gw"), &t),
            price_for(Some("mystery-model"), &base)
        );
        assert_eq!(price_for(None, &t), price_for(None, &base));
        t.insert("native:zzz".to_string(), gw);
        assert_eq!(
            price_for(Some("mystery-model"), &t),
            price_for(Some("mystery-model"), &base)
        );

        // Engine prices are read under `engine:<id>`; the key helpers spell that.
        assert_eq!(ENGINE_PRICE_PREFIX, "engine:");
        assert_eq!(NATIVE_ENGINE_PREFIX, "native:");
        assert_eq!(engine_key("gw"), "engine:gw");
        assert_eq!(native_key("claude-max"), "native:claude-max");
        t.insert("engine:gw".to_string(), gw);
        assert_eq!(engine_price("gw", &t), Some(gw));
        assert_eq!(engine_price("claude", &t), None);
        assert_eq!(engine_price("người", &t), None);
        assert_eq!(engine_price("claude-max", &t), None);
        // A plain key is no longer an engine price.
        let mut plain = base.clone();
        plain.insert("gw".to_string(), gw);
        assert_eq!(engine_price("gw", &plain), None);
        assert!(!is_priced("gw", &plain));

        // is_priced: claude yes, người no, native marker yes, engine price yes, else no.
        assert!(is_priced("claude", &t));
        assert!(!is_priced("người", &t));
        assert!(is_priced("claude-max", &t));
        assert!(is_priced("gw", &t));
        assert!(!is_priced("gw2", &t));
        assert!(!is_priced("claude-max", &base));

        // A killed native run is estimated by its model family, like claude.
        let mut r = dummy_run(
            "dev",
            "killed",
            Some("opus"),
            "partial",
            0.0,
            0,
            Some(1000),
            1_000_000,
            0,
            0,
            0,
        );
        r.engine = "claude-max".to_string();
        let family = price_for(Some("opus"), &base).input;
        assert!(family > 0.0);
        assert!((estimate_usd(&r, &t) - family).abs() < 1e-9);
        // Without the native marker it would be an unpriced engine: 0, never the family.
        assert_eq!(estimate_usd(&r, &base), 0.0);
        // A finished native run keeps the cost from its result.
        r.status = "ok".to_string();
        r.usage_source = "result".to_string();
        r.cost_usd = 0.5;
        assert_eq!(run_cost(&r, &t).usd, 0.5);

        // A gateway run is priced from its entry; an unpriced one costs 0.
        r.status = "killed".to_string();
        r.usage_source = "partial".to_string();
        r.engine = "gw".to_string();
        assert!((estimate_usd(&r, &t) - 99.0).abs() < 1e-9);
        r.engine = "gw2".to_string();
        assert_eq!(estimate_usd(&r, &t), 0.0);
    }
}
