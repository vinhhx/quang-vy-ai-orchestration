//! Engine `dsh` (D-35): DeepSeek Harness trong container, hộp đen chỉ văn bản.
//! Plan 06-11 chỉ đặt chỗ cho nó: `engine::plan_for` đã gọi `plan` ở đây, và `plan` từ chối
//! chạy cho đến khi plan 06-23 viết phần thân. Nhờ vậy các plan 06-21 … 06-26 không phải sửa
//! `plan_for` nữa.

use crate::config::{Config, DshDef};
use crate::engine::EnginePlan;
use crate::error::{BrainError, BrainResult};
use std::collections::BTreeMap;
use std::path::Path;

/// What a dsh run persists so a kill (or a restart of the serve process) can find and remove
/// the container process it started.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DshRun {
    pub docker: String,
    pub container: String,
    /// Unique token of this run, used to find the container-side process again.
    pub marker: String,
}

/// The run plan of a dsh engine. Not enabled yet: plan 06-23 fills this in.
pub fn plan(
    _cfg: &Config,
    _id: &str,
    _def: &DshDef,
    _role: &str,
    _claude_argv: &[String],
    _base_env: BTreeMap<String, String>,
    _cwd: &Path,
) -> BrainResult<EnginePlan> {
    Err(BrainError::error(
        "Engine dsh chưa được bật trong bản agent-brain này",
    ))
}
