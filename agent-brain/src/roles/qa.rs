use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};

pub fn cmd_qa(_ctx: &mut Ctx, _project: &str, _dry_run: bool, _verbose: bool) -> BrainResult<i32> {
    Err(BrainError::not_available("run qa"))
}
