---
phase: 05-embedded-dashboard
plan: 12
subsystem: release-docs
tags: [musl, release, smoke, docs, live-checks]

requires:
  - phase: 05-embedded-dashboard
    provides: "05-02..05-11 serve and all five screens"
provides:
  - "scripts/build-release.sh: P4-01 step — the static musl binary runs `serve --port 0` in an empty environment (env -i, PATH=/nonexistent, temp HOME), serves / and /app.css through the token handshake, refuses without a token (401), and stops by recorded PID"
  - "agent-brain/README.md: dashboard section (serve, port/dashboard_port, token URL and cookie, single instance, what the profile switcher does, screens, stopping, cookie scope note T-5-11)"
  - "agent-brain/LIVE-CHECKS.md: P4-01..P4-10 live steps (python sleep 307 for kill checks, PID-only verification, no pkill/killall)"
  - "protocol-log/nghiem-thu-p4.md: empty P4 acceptance table (P4-10 recorded as not applicable, D-18) + UI-01..UI-06 backstop table"
affects: [05-13]

actuals:
  tasks: 2
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Release-level proof runs the shipped binary in env -i; the build itself stays offline and Node-free"

key-files:
  created:
    - protocol-log/nghiem-thu-p4.md
  modified:
    - scripts/build-release.sh
    - agent-brain/README.md
    - agent-brain/LIVE-CHECKS.md

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, --dangerously-skip-permissions approved for Phase 5); reviewed by Claude"
  - "DASH-10 recorded as not applicable (D-18: no Figma design)"

patterns-established:
  - "Live acceptance records are tables the user fills; executors create them with empty result cells"

requirements-completed: [DASH-01, DASH-10]

duration: 15min
completed: 2026-10-17
status: complete
---

# Phase 5 Plan 12: Release Smoke and Docs Summary

**The release script proves that the static musl binary serves the token-gated dashboard and its CSS in an empty environment. The README documents the dashboard. LIVE-CHECKS gives exact P4 live steps, and the empty acceptance tables are ready for the user's live run in 05-13.**

## Performance

- **Duration:** about 15 min (agy run 586 s, plus review)
- **Completed:** 2026-10-17
- **Tasks:** 2
- **Files:** 4

## Accomplishments

- **Release smoke:** `build-release.sh` adds a P4-01 step after the existing P2-09 checks. In the temp dir it runs `env -i HOME=<tmp> PATH=/nonexistent <bin> serve --port 0`, reads the URL line, performs the `?token=` handshake with `curl` to 127.0.0.1 only, and checks `/` and `/app.css`. It also checks that a request without the cookie gets 401, and stops the server by its recorded PID. The release path still runs no Tailwind and no npm.
- **Live checks:** `LIVE-CHECKS.md` P4-01..P4-10 are in Vietnamese. The kill checks (P4-06) hold the tree with `python3 -c 'import time; time.sleep(307)'` through the existing live wrapper and verify recorded PIDs with `ps -p`. Signals come only from the UI Kill, `agent-brain kill`, or Ctrl+C.
- **Acceptance record:** `protocol-log/nghiem-thu-p4.md` has empty result cells for P4-01..P4-09, P4-10 marked "Không áp dụng (D-18)", and the six UI backstop rows UI-01..UI-06.

## Task Commits

1. `e89fc9b` feat(05-12): release smoke serves the dashboard from the static binary (agy)
2. `9f939bd` docs(05-12): dashboard README, live checks and acceptance table (agy)
3. `b442f6d` docs(05-12): live checks name the real Giao việc button label (Claude, a 2-word docs typo; CLAUDE.md exempts docs typos)

## Deviations from Plan

- Review found that P4-04 and P4-06 told the user to press a button called "Giao việc cho Lead". The real button label is `Giao việc` (UI-SPEC Primary CTAs). Claude fixed the two occurrences directly, a 2-word docs typo exempt under CLAUDE.md, and re-ran the docs check.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- **Scope:** agy's 2 commits touch exactly the 4 declared files.
- **Installed binary:** `~/.local/bin/agent-brain` is untouched (mtime 2026-10-16). The release script copies the binary only into its temp dir.
- **Verify:** both `<automated>` commands exit 0. `bash scripts/build-release.sh` prints "P2-09 OK (route A) … 8195648 byte · sha256 caafefaf…", "P4-01 OK: dashboard chạy trong môi trường rỗng (env -i, PATH=/nonexistent) — / và /app.css qua token, 401 khi thiếu token", and leaves no serve process. The docs check prints "docs ok" and still does after the typo fix.
- **Acceptance:** all 6 command criteria pass under the reviewer's checker.
- **Safety greps:** no `pkill` or `killall` in LIVE-CHECKS, README or build-release.sh. The new P4 kill steps use the python sleep. The remaining `sleep 5` lines are pre-existing P2 steps the user types in their own terminal.
- **Content read:** P4-06 (both the UI-started and terminal-started kill cases), and the acceptance table (empty cells, P4-10 not applicable, UI-01..UI-06).
- **Full suite:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --no-fail-fast -- --test-threads=1` exits 0 with 243 passed and 0 failed. The Python suite is OK.
- **Secrets:** the pattern scan is clean.

## Next Phase Readiness

05-13, the live acceptance with the real claude, a browser, another LAN machine and the network off, is ready. It is the user's checkpoint plan.
