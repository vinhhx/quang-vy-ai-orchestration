---
phase: 03-rust-cli-core
plan: 13
subsystem: live-acceptance
tags: [rust, live, acceptance, kill, profiles, d-02]

requires:
  - phase: 03-rust-cli-core
    provides: "03-12 static binary, README, LIVE-CHECKS.md, nghiem-thu-p2.md, allow-sleep wrapper"
provides:
  - "Live acceptance of agent-brain on claude 2.1.301: P2-01, P2-02, P2-03, P2-07, P2-09, D-02, E-06 all Đạt in protocol-log/nghiem-thu-p2.md"
  - "Binary installed at ~/.local/bin/agent-brain; Python `brain` symlink removed (D-02)"
  - "Live kill method updated for claude 2.1.301 (python sleep instead of `sleep 307`)"
affects: [phase-04, phase-05, phase-06]

actuals:
  tasks: 2
  commits: 14

tech-stack:
  added: []
  patterns:
    - "Live terminal checks driven through a pty: tty layer delivers SIGINT/SIGHUP, the harness never calls kill()"

key-files:
  created:
    - .planning/fixes/03-13-live-sleep-method.plan.md
  modified:
    - protocol-log/nghiem-thu-p2.md
    - agent-brain/LIVE-CHECKS.md
    - scripts/live/claude-allow-sleep.sh
    - tests/test_live_helpers.py

key-decisions:
  - "User delegated every live check to Claude (\"run P2-01 for me\" … \"run and completed phase 3 for me\"); Claude ran them and recorded the results"
  - "claude 2.1.301 blocks a foreground `sleep N` in the Bash tool, so P2-02/P2-03 hold the process with `python3 -c 'import time; time.sleep(307)'` and the wrapper allows `Bash(python3 *)`; fixed via agy (fix plan 03-13-live-sleep-method)"
  - "P2-03 ran in a pseudo-terminal instead of a GUI terminal window; the kernel delivers the same SIGINT/SIGHUP to the foreground process group"
  - "D-02 ran on apec (real Laravel repo, work profile); E-06 ran on a throwaway personal project `du-an-nhap-ca-nhan` after the user had Claude stop the two interactive ~/.claude-orca sessions"

patterns-established:
  - "Before any personal-profile run, check /proc/<pid>/environ of every claude process for CLAUDE_CONFIG_DIR=~/.claude-orca"

requirements-completed: [CLI-01, CLI-03, PROF-02, PROC-01, PROC-02, FAST-02, DIST-01]

duration: 75min
completed: 2026-10-16
status: complete
---

# Phase 3 Plan 13: Live Acceptance Summary

**`agent-brain` passes every live check on the real claude 2.1.301: step-by-step logs, a clean process-tree kill by `kill`, Ctrl+C (once and twice) and terminal close, `inspect` in one turn with no transcript, a static binary that runs from anywhere, one real lead → dev → qa task on apec, and a personal-profile run whose transcript lands only under `~/.claude-orca`. The Python `brain` command is retired.**

## Results (protocol-log/nghiem-thu-p2.md)

| Check | Result | Evidence |
|---|---|---|
| P2-01 | Đạt | run 1 on du-an-nhap: init/tool/result lines + `Kết thúc run`, plan commit only; `status` 0.03 s |
| P2-02 | Đạt (python sleep) | tree claude → bash (own PGID/SID) → python3; `agent-brain kill` 1382 ms, exit 130, `killed`; no PID left after 5 s |
| P2-03 | Đạt (pty) | A Ctrl+C: 0.98 s; B Ctrl+C ×2: "Dừng ngay (SIGKILL)", 0.37 s; C SIGHUP: 1.12 s; all exit 130, `killed`, no PID left |
| P2-07 | Đạt | `inspect` 1 turn, exit 0 (A1 holds: `--max-turns` accepted); 0 new `.jsonl` |
| P2-09 | Đạt | static-pie, 0 NEEDED; copied binary runs `--version`, `stats`, and under `env -i` |
| D-02 | Đạt | apec task 6: lead `0dc79a7` → dev `505fa5c` (2 plan files) → qa `db03791` Đạt; Python symlink removed |
| E-06 | Đạt | run 12 personal; session `77f438eb…` transcript only under `~/.claude-orca/projects/` |
| P2-04/05/06/08 | Đạt | automated tests `p2_04…`, `p2_05…`, `p2_06…`, `p2_08…` plus live DB/status/stats evidence |

## Commits

1. `c89e62e` docs(03-13): record live P2-01 result
2. `a49939e` docs(03-13): record live P2-02 result (python sleep variant)
3. `88c6b9d` docs(03-13): fix plan for live sleep method
4. `65386f2` test(03-13): expect python sleep rule in live wrapper (agy)
5. `87f8e71` fix(03-13): live wrapper allows python sleep instead of sleep (agy)
6. `00443c9` docs(03-13): live checks use python sleep (agy)
7. `1c2b7c9` docs(03-13): record live P2-07 and P2-09 results
8. `0620a2f` docs(03-13): fix calc.py path in live checks
9. `162fede` docs(03-13): update state with live acceptance progress
10. `6e39faa` docs(03-13): record live P2-03 result (pty)
11. `de52d7c` docs(03-13): record live D-02 result on apec
12. `0e44ba4` docs(03-13): record E-06 and automated P2 rows

(apec's own commits `0dc79a7`, `505fa5c`, `db03791` live on its local branch `task/trim-gia-tri-ban-dich`, not pushed.)

## Deviations from Plan

1. **[Live finding] `sleep 307` is blocked by claude 2.1.301.** Two P2-02 attempts never produced a bash → sleep grandchild ("Blocked: standalone sleep"; the agent retried in the background or gave up). P2-02 passed with `python3 -c 'import time; time.sleep(307)'`; the wrapper, its test and LIVE-CHECKS.md were changed by agy under `.planning/fixes/03-13-live-sleep-method.plan.md` and reviewed (3 tests OK, Python suite OK, valid shell in every run-lead line). The first agy run was stopped at the user's request after steps 1–2 and resumed for step 3. agy's one reported deviation (the P2-02 heading contains "sleep 307") was a contradiction in the reviewer's plan and was accepted.
2. **P2-03 through a pty, not a GUI window.** Signals came only from the tty layer (0x03 → SIGINT to the foreground group = agent-brain's group; claude is in its own group; closing the master → SIGHUP). Not repeated by hand in a real window.
3. **LIVE-CHECKS.md P2-07 path.** The guide wrote `calc.py` at the repo root; the file is `src/calc.py`. Fixed directly (`0620a2f`, docs-only).
4. **D-02 tests.** apec forbids running php on the host (AGENT.md); one baseline run did, with no effect (its only write was refused), and all later runs used the `apec_app` container. Two module tests fail before and after the task (key order in `read()`, missing `Database\Seeders\LanguageSeeder`); not caused by this task.
5. **E-06 setup.** The only personal project was this repo, so a throwaway personal project was added to `~/.config/agent-brain/projects_map.json` (`du-an-nhap-ca-nhan`, backup in the session scratchpad). Two interactive claude sessions on `~/.claude-orca` (pts/2 loyalty, pts/6 phu-gia) were stopped by Claude at the user's request with the S4 method; they can be resumed with `claude --resume <id>`.

## Issues Encountered (not fixed, for a later phase)

- The terminal log repeats `system thinking_tokens` lines many times per run (noise).
- claude creates an empty `~/.claude/session-env/<session-id>/` directory for every session, `inspect` included (not a transcript; 127+ accumulated).
- The 03-11 summary counted 170 Rust tests; the suite lists and passes 156, and no Rust file changed since 03-11, so 170 was a miscount.

## Next Phase Readiness

Phase 3 is complete. Phase 4 needs ≥ 2 weeks of real `runs` data from daily use of the binary; Phase 5 and 6 can start now. The order is the user's choice (ROADMAP: biggest pain first).
