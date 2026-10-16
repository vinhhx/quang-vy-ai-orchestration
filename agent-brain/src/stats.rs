use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};
use rusqlite::Connection;

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

pub fn parse_since(_s: &str, _now_ms: i64) -> BrainResult<i64> {
    Err(BrainError::not_available("parse_since"))
}

pub fn query_stats(
    _conn: &Connection,
    _profile: Option<&str>,
    _since_ms: Option<i64>,
) -> BrainResult<serde_json::Value> {
    Err(BrainError::not_available("query_stats"))
}

pub fn cmd_stats(
    _ctx: &mut Ctx,
    _profile: Option<&str>,
    _since: Option<&str>,
    _json: bool,
) -> BrainResult<i32> {
    Err(BrainError::not_available("stats"))
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
