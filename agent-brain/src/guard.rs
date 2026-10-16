use crate::error::BrainResult;
use std::path::{Path, PathBuf};

pub const GUARD_MATCHER: &str = "Read|Edit|Write|MultiEdit|NotebookEdit|Grep|Glob";
pub const WRITE_TOOLS: [&str; 4] = ["Write", "Edit", "MultiEdit", "NotebookEdit"];
pub const READ_TOOLS: [&str; 3] = ["Read", "Grep", "Glob"];

#[derive(Debug, Clone)]
pub struct Policy {
    pub root: PathBuf,
    pub read_allow: Option<Vec<PathBuf>>,
    pub write_allow: Vec<PathBuf>,
    pub log: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub allow: bool,
    pub tool: String,
    pub path: String,
    pub reason: String,
}

pub fn decide(_policy: &Policy, _payload: &serde_json::Value) -> Decision {
    unimplemented!()
}

pub fn sh_quote(s: &str) -> String {
    format!("'{s}'")
}

pub fn write_hook_settings(
    _run_dir: &Path,
    _root: &Path,
    _read_allow: Option<&[PathBuf]>,
    _write_allow: &[PathBuf],
    _exe: &Path,
) -> BrainResult<PathBuf> {
    unimplemented!()
}

pub fn cmd_guard(_policy: Option<&Path>) -> i32 {
    crate::term::emit("agent-brain guard: chưa có trong bản này — chặn (fail-closed)");
    2
}
