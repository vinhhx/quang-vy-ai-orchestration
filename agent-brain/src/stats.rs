use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};

pub fn cmd_stats(
    _ctx: &mut Ctx,
    _profile: Option<&str>,
    _since: Option<&str>,
    _json: bool,
) -> BrainResult<i32> {
    Err(BrainError::not_available("stats"))
}
