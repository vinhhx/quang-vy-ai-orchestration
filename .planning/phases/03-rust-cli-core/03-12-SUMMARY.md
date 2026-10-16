---
phase: 03-rust-cli-core
plan: 12
subsystem: release-docs
tags: [rust, musl, static-build, docs, live-checks]

requires:
  - phase: 03-rust-cli-core
    provides: "03-08 kill/stop semantics; 03-09 status/inspect/stats; 03-11 dev/qa parity and pipeline"
provides:
  - "`scripts/build-release.sh`: offline, locked x86_64-unknown-linux-musl build (route A musl-gcc, route B gcc + CFLAGS with a warning), static assertions (`file`, `readelf -d` no NEEDED) and the DIST-01 copy test under `env -i HOME=<tmp> PATH=/nonexistent` (--version, stats --json, fail-closed guard exit 2)"
  - "`scripts/live/claude-allow-sleep.sh`: real-claude wrapper inserting `Bash(sleep *)` right after the first `--allowedTools` (P2-02/P2-03 only)"
  - "`agent-brain/README.md`: commands, exit codes 0/1/2/3/4/130, config keys, env vars, data paths, profiles, identity, kill semantics, build/install, D-02 transition"
  - "`agent-brain/LIVE-CHECKS.md` + `protocol-log/nghiem-thu-p2.md`: exact live-check steps and the empty results table (P2-01..P2-09, D-02, E-06) for 03-13"
affects: [03-13]

actuals:
  tasks: 2
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Release build verified by running the copied binary in an empty environment, not just by `file`"

key-files:
  created:
    - scripts/build-release.sh
    - scripts/live/claude-allow-sleep.sh
    - tests/test_live_helpers.py
    - agent-brain/README.md
    - agent-brain/LIVE-CHECKS.md
    - protocol-log/nghiem-thu-p2.md
  modified: []

key-decisions:
  - "Executed by Antigravity (agy, gemini-3.8-flash-high); reviewed by Claude"

patterns-established:
  - "Live-check docs signal only via `agent-brain kill`, Ctrl+C or terminal close; `ps` read-only by recorded PID"

requirements-completed: [DIST-01]

duration: 15min
completed: 2026-10-16
status: complete
---

# Phase 3 Plan 12: Static Release Build, Manual and Live Checks Summary

**One command (`bash scripts/build-release.sh`) builds a fully static musl `agent-brain` and proves it runs from a fresh directory with an empty environment; the user now has a Vietnamese manual, a step-by-step live-check guide with a results table, and the allow-sleep wrapper needed for the real-claude kill test.**

## Performance

- **Duration:** 15 min (agy run 866 s, 13:10–13:25 UTC, exit 0, `result` status success)
- **Completed:** 2026-10-16
- **Tasks:** 2
- **Files:** 6

## Task Commits

1. `b673f52` feat(03-12): static release build script
2. `e0c3695` test(03-12): add failing wrapper test
3. `f68740a` docs(03-12): manual, live checks and wrapper

## Deviations from Plan

None.

## Review (Claude)

Re-run by the reviewer:

- Verify: `build-release.sh` → `P2-09 OK (route A)`, 6 118 976 byte, runs under `env -i`; `tests.test_live_helpers` 2/2 OK; Rust test-name traceability 17/17 prefixes (p2_01..08, p1_01/03..10); full Python suite 123 tests OK.
- Acceptance: script executable and uses `--offline --locked`; `agent-brain/target` untracked and ignored; README has every command, config key/env var and exit-code rows 0/1/2/3/4/130; LIVE-CHECKS covers P2-01/02/03/07/09, D-02, E-06 and references the wrapper; nghiem-thu-p2.md has exactly 11 ID rows; zero `pkill`/`killall`.
- Content spot-check: LIVE-CHECKS uses the throwaway repo `du-an-nhap`, branch cleanup per killed run, `/tmp/ab-live.json` config copy, PID-scoped `ps -p`, the P2-07 transcript stamp, `brain/install.sh --uninstall` + `command -v brain`, and the E-06 session-id lookup; README documents install, XDG paths, DB 0600, `CLAUDE_CONFIG_DIR`, assumption A1.
- Scope: exactly the plan's six files (`git diff 94197b4..f68740a`); `brain/README.md` untouched; no secrets or home paths in the diff (`scan-secrets.sh` covers `spikes/` only, so the diff was grepped manually).

## Issues Encountered

None.

## Next Phase Readiness

03-13 is the user's live acceptance: follow `agent-brain/LIVE-CHECKS.md`, fill `protocol-log/nghiem-thu-p2.md`, install the binary and retire the Python symlink.
