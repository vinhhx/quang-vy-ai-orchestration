//! DIST-01 regression: the binary starts and answers `stats --json` in an empty environment,
//! with no `git` (or anything else) on PATH. `scripts/build-release.sh` runs the same check.

use std::process::Command;

#[test]
fn stats_json_runs_without_git_on_path() {
    let home = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_agent-brain"))
        .env_clear()
        .env("HOME", home.path())
        .env("PATH", "/nonexistent")
        .args(["stats", "--json"])
        .output()
        .expect("run agent-brain");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "exit {:?}\nstdout: {stdout}\nstderr: {stderr}", out.status);
    assert!(stdout.contains("\"total\""), "stdout: {stdout}");
}
