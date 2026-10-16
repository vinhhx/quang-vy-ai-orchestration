use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};
use crate::term;
use rusqlite::{params, Connection};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Totals {
    pub runs: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectStats {
    pub project: String,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoleStats {
    pub role: String,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsFilter {
    pub profile: Option<String>,
    pub since: Option<String>,
    pub since_ms: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatsReport {
    pub filter: StatsFilter,
    pub total: Totals,
    pub by_project: Vec<ProjectStats>,
    pub by_role: Vec<RoleStats>,
}

pub fn parse_since(s: &str, now_ms: i64) -> BrainResult<i64> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"^(\d+)([mhd])$").unwrap());

    let caps = re.captures(s).ok_or_else(|| {
        BrainError::error(format!(
            "Khoảng thời gian không hợp lệ: '{s}' (ví dụ 30m, 24h, 7d)"
        ))
    })?;

    let num: i64 = caps[1].parse().map_err(|_| {
        BrainError::error(format!(
            "Khoảng thời gian không hợp lệ: '{s}' (ví dụ 30m, 24h, 7d)"
        ))
    })?;

    let unit = &caps[2];
    let duration_ms = match unit {
        "m" => num * 60 * 1000,
        "h" => num * 60 * 60 * 1000,
        "d" => num * 24 * 60 * 60 * 1000,
        _ => unreachable!(),
    };

    Ok(now_ms - duration_ms)
}

pub fn query_report(
    conn: &Connection,
    profile: Option<&str>,
    since: Option<&str>,
    since_ms: Option<i64>,
) -> BrainResult<StatsReport> {
    let total: Totals = conn.query_row(
        "SELECT
            COUNT(*),
            COALESCE(SUM(runs.input_tokens), 0),
            COALESCE(SUM(runs.output_tokens), 0),
            COALESCE(SUM(runs.cache_read_tokens), 0),
            COALESCE(SUM(runs.cache_write_tokens), 0),
            COALESCE(SUM(runs.cost_usd), 0.0)
         FROM runs
         WHERE (?1 IS NULL OR runs.profile = ?1)
           AND (?2 IS NULL OR runs.started_at >= ?2)",
        params![profile, since_ms],
        |r| {
            Ok(Totals {
                runs: r.get(0)?,
                input_tokens: r.get(1)?,
                output_tokens: r.get(2)?,
                cache_read_tokens: r.get(3)?,
                cache_write_tokens: r.get(4)?,
                cost_usd: r.get(5)?,
            })
        },
    )?;

    let mut stmt_proj = conn.prepare(
        "SELECT
            projects.slug,
            COUNT(*),
            COALESCE(SUM(runs.input_tokens), 0),
            COALESCE(SUM(runs.output_tokens), 0),
            COALESCE(SUM(runs.cache_read_tokens), 0),
            COALESCE(SUM(runs.cache_write_tokens), 0),
            COALESCE(SUM(runs.cost_usd), 0.0)
         FROM runs
         JOIN projects ON projects.id = runs.project_id
         WHERE (?1 IS NULL OR runs.profile = ?1)
           AND (?2 IS NULL OR runs.started_at >= ?2)
         GROUP BY projects.slug
         ORDER BY projects.slug",
    )?;
    let by_project = stmt_proj
        .query_map(params![profile, since_ms], |r| {
            Ok(ProjectStats {
                project: r.get(0)?,
                totals: Totals {
                    runs: r.get(1)?,
                    input_tokens: r.get(2)?,
                    output_tokens: r.get(3)?,
                    cache_read_tokens: r.get(4)?,
                    cache_write_tokens: r.get(5)?,
                    cost_usd: r.get(6)?,
                },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut stmt_role = conn.prepare(
        "SELECT
            runs.role,
            COUNT(*),
            COALESCE(SUM(runs.input_tokens), 0),
            COALESCE(SUM(runs.output_tokens), 0),
            COALESCE(SUM(runs.cache_read_tokens), 0),
            COALESCE(SUM(runs.cache_write_tokens), 0),
            COALESCE(SUM(runs.cost_usd), 0.0)
         FROM runs
         WHERE (?1 IS NULL OR runs.profile = ?1)
           AND (?2 IS NULL OR runs.started_at >= ?2)
         GROUP BY runs.role
         ORDER BY runs.role",
    )?;
    let by_role = stmt_role
        .query_map(params![profile, since_ms], |r| {
            Ok(RoleStats {
                role: r.get(0)?,
                totals: Totals {
                    runs: r.get(1)?,
                    input_tokens: r.get(2)?,
                    output_tokens: r.get(3)?,
                    cache_read_tokens: r.get(4)?,
                    cache_write_tokens: r.get(5)?,
                    cost_usd: r.get(6)?,
                },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let filter = StatsFilter {
        profile: profile.map(|p| p.to_string()),
        since: since.map(|s| s.to_string()),
        since_ms,
    };

    Ok(StatsReport {
        filter,
        total,
        by_project,
        by_role,
    })
}

pub fn query_stats(
    conn: &Connection,
    profile: Option<&str>,
    since_ms: Option<i64>,
) -> BrainResult<serde_json::Value> {
    let report = query_report(conn, profile, None, since_ms)?;
    serde_json::to_value(&report).map_err(|e| BrainError::error(format!("Lỗi serialize JSON: {e}")))
}

pub fn cmd_stats(
    ctx: &mut Ctx,
    profile: Option<&str>,
    since: Option<&str>,
    json: bool,
) -> BrainResult<i32> {
    if let Some(prof) = profile {
        crate::profile::check_profile(prof, &ctx.cfg.profiles)?;
    }

    let since_ms = match since {
        Some(s) => {
            let now_ms = crate::db::now_ms();
            Some(parse_since(s, now_ms)?)
        }
        None => None,
    };

    let report = query_report(&ctx.conn, profile, since, since_ms)?;

    if json {
        let json_val = serde_json::to_value(&report)
            .map_err(|e| BrainError::error(format!("Lỗi serialize JSON: {e}")))?;
        let json_str = serde_json::to_string_pretty(&json_val)
            .map_err(|e| BrainError::error(format!("Lỗi serialize JSON: {e}")))?;
        term::emit_out(&json_str);
        return Ok(0);
    }

    let prof_label = profile.unwrap_or("tất cả");
    let since_label = since.unwrap_or("toàn bộ");
    term::emit_out(&format!(
        "Thống kê token — profile: {prof_label} — khoảng: {since_label}"
    ));
    term::emit_out(&format!(
        "Tổng cộng: {} run · {} token vào · {} token ra · {} đọc cache · {} ghi cache · {:.4} USD tương đương",
        report.total.runs,
        report.total.input_tokens,
        report.total.output_tokens,
        report.total.cache_read_tokens,
        report.total.cache_write_tokens,
        report.total.cost_usd
    ));

    term::emit_out("");
    term::emit_out("== Theo dự án ==");
    term::emit_out(&format!(
        "{:<20} {:>6} {:>10} {:>10} {:>10} {:>10} {:>24}",
        "Dự án", "Runs", "Vào", "Ra", "Đọc cache", "Ghi cache", "chi phí tương đương (USD)"
    ));
    for p in &report.by_project {
        term::emit_out(&format!(
            "{:<20} {:>6} {:>10} {:>10} {:>10} {:>10} {:>24.4}",
            p.project,
            p.totals.runs,
            p.totals.input_tokens,
            p.totals.output_tokens,
            p.totals.cache_read_tokens,
            p.totals.cache_write_tokens,
            p.totals.cost_usd
        ));
    }

    term::emit_out("");
    term::emit_out("== Theo vai trò ==");
    term::emit_out(&format!(
        "{:<20} {:>6} {:>10} {:>10} {:>10} {:>10} {:>24}",
        "Vai trò", "Runs", "Vào", "Ra", "Đọc cache", "Ghi cache", "chi phí tương đương (USD)"
    ));
    for r in &report.by_role {
        term::emit_out(&format!(
            "{:<20} {:>6} {:>10} {:>10} {:>10} {:>10} {:>24.4}",
            r.role,
            r.totals.runs,
            r.totals.input_tokens,
            r.totals.output_tokens,
            r.totals.cache_read_tokens,
            r.totals.cache_write_tokens,
            r.totals.cost_usd
        ));
    }

    term::emit_out("");
    term::emit_out("Chi phí là giá trị tương đương; gói subscription không tính tiền theo token.");

    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_since_valid() {
        let now = 1_000_000_000_000_i64;
        assert_eq!(parse_since("30m", now).unwrap(), now - 1_800_000);
        assert_eq!(parse_since("24h", now).unwrap(), now - 86_400_000);
        assert_eq!(parse_since("7d", now).unwrap(), now - 604_800_000);
        assert_eq!(parse_since("0d", now).unwrap(), now);
    }

    #[test]
    fn test_parse_since_invalid() {
        let now = 1_000_000_000_000_i64;
        assert!(parse_since("7", now).is_err());
        assert!(parse_since("d", now).is_err());
        assert!(parse_since("-1d", now).is_err());
        assert!(parse_since("7x", now).is_err());
        assert!(parse_since("", now).is_err());
    }
}
