use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};
use crate::roles::RunOpts;

pub fn cmd_analyst(
    _ctx: &mut Ctx,
    _project_q: &str,
    _question: &str,
    _opts: &RunOpts,
) -> BrainResult<i32> {
    Err(BrainError::error("analyst chưa triển khai"))
}
