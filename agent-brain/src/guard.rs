use std::path::Path;

pub fn cmd_guard(_policy: Option<&Path>) -> i32 {
    crate::term::emit("agent-brain guard: chưa có trong bản này — chặn (fail-closed)");
    2
}
