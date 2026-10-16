---
phase: 03-rust-cli-core
verified: 2026-10-16T17:00:00Z
status: passed
score: 5/5 success criteria verified
behavior_unverified: 0
re_verification: false
gaps: []
human_verification:
  - test: "P2-03 case C in a real GUI terminal window"
    expected: "Closing the window running `agent-brain run lead` leaves none of the recorded claude/bash/python PIDs alive within 5 s"
    why_human: "Verified through a pty (same kernel SIGHUP to the session); a real window close was not repeated by hand. Optional."
---

# Phase 3 Verification: Rust CLI Core

Goal: the Phase 2 protocol runs through the Rust binary `agent-brain` (CLI, no UI) with safe process management, stream-json log capture, token accounting in SQLite and zero-token fast paths.

Verified by the reviewer against the code (Rust suite 156/156 in 19 binaries, `cargo fmt --check` clean, Python suite OK), the live runs in `~/.local/share/agent-brain/brain.db` (12 runs on claude 2.1.301), the run dirs, the project repos' git history and `protocol-log/nghiem-thu-p2.md` (11/11 rows Đạt).

## Success criteria

| # | Criterion | Evidence | Status |
|---|-----------|----------|--------|
| 1 | `run lead\|dev\|qa` works on a real project, Vietnamese routing with Fallback Ask, profile per project, step-by-step log | P2-01 (du-an-nhap) and D-02 (apec: lead `0dc79a7` → dev `505fa5c` → qa `db03791` Đạt); `"dự án nháp"` / `nhap-ca-nhan` / `apec` routed; ambiguous `"api"` → project list, exit 2, no agent (dry-run); work runs show `CLAUDE_CONFIG_DIR không đặt`, personal shows `~/.claude-orca` | passed |
| 2 | `kill <task_id>` while the agent runs a long command leaves no claude/child process after 5 s; Ctrl+C mid-run leaves nothing | P2-02: claude → bash (own PGID/SID) → python3, `kill` 1382 ms, exit 130, nothing left; P2-03 A/B/C via pty (Ctrl+C, Ctrl+C ×2 → SIGKILL, SIGHUP), all exit 130 and clean; `sleep 300` replaced by a python sleep because claude 2.1.301 blocks foreground `sleep` | passed |
| 3 | `runs` has tokens (input/output/cache), `session_id`, `pid`, times; a bad JSON line is stored raw and the run continues | All 12 live rows have session_id, pid, started_at/ended_at, 4 token counts, cost; killed runs keep partial usage; `p2_04_runs_row_complete`, `p2_05_bad_json_line_stored_raw` pass | passed |
| 4 | `status` returns git info with 0 tokens and no AI; `inspect` runs exactly 1 turn and leaves no session | `status` 0.03 s, no run row added (`p2_06_status_zero_token`); P2-07: inspect run 5 `num_turns` 1, exit 0, 0 new transcripts (`--max-turns` accepted, A1 holds) | passed |
| 5 | `stats --profile work` counts only work runs; the copied binary runs with nothing installed | `--profile work` 11, `--profile personal` 1, all 12 (`p2_08_stats_profile_filter`); P2-09: static-pie, 0 NEEDED, copied binary runs `--version`/`stats`, and under `env -i PATH=/nonexistent` | passed |

## Requirements

CLI-01, CLI-02, CLI-03, PROF-02, PROC-01, PROC-02, LOG-01, LOG-02, FAST-01, FAST-02, STAT-01, DIST-01: all satisfied (plans 03-03..03-13; live evidence above).

## D-02 hand-over

The binary is installed at `~/.local/bin/agent-brain` (sha256 `0662979a…`, same as the release build). `bash brain/install.sh --uninstall` removed the `brain` symlink; `command -v brain` prints nothing. The Python source stays in `brain/` as the reference.

## Defects found live and fixed in this phase

- claude 2.1.301 blocks foreground `sleep N`, so the live kill guide could not produce the grandchild it claims to test. Fixed by the agy fix plan `03-13-live-sleep-method` (`65386f2`, `87f8e71`, `00443c9`).
- LIVE-CHECKS.md P2-07 pointed at `calc.py` instead of `src/calc.py` (`0620a2f`).

## Notes, not gaps

- The Bash tool's shell runs in its own process group and session: killing claude's process group alone would miss it. agent-brain's recorded-descendant sweep is what made P2-02/P2-03 clean.
- Log noise: repeated `system thinking_tokens` lines. claude leaves an empty `~/.claude/session-env/<id>/` per session. Neither breaks a criterion.
- apec has two module tests that failed before this phase's task (key order, missing `LanguageSeeder`).
- The live checks were delegated by the user to Claude, and the table says so where the method differed (python sleep, pty).
