use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};

pub fn cmd_status(_ctx: &mut Ctx, _project: &str) -> BrainResult<i32> {
    Err(BrainError::not_available("status"))
}

pub fn cmd_inspect(_ctx: &mut Ctx, _project: &str, _verbose: bool) -> BrainResult<i32> {
    Err(BrainError::not_available("inspect"))
}
