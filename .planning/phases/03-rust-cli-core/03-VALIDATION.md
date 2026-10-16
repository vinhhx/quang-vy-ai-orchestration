---
phase: "03"
slug: "rust-cli-core"
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: "2026-10-16"
---

# Phase 03 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust 1.98.1) + tempfile; Python unittest stays as regression for the shared fixtures |
| **Config file** | none — Wave 0 creates `agent-brain/Cargo.toml` |
| **Quick run command** | `cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain && cargo test --offline --locked --lib` |
| **Full suite command** | `cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain && cargo test --offline --locked && cd .. && python3 -m unittest discover -s tests -t . -q` |
| **Estimated runtime** | ~90 seconds (after the first offline build) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --offline --locked --lib` plus the touched integration test
- **After every plan wave:** Run the full suite command (114 Python tests must stay green)
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | DIST-01, CLI-03 | T-03-01 | dependencies pinned with `=`; assets byte-equal to brain/ | static | `python3 -c "import tomllib; ..."` + `cmp brain/<f> agent-brain/assets/<f>` (plan Task 1) | ❌ W0 | ⬜ pending |
| 03-01-02 | 01 | 1 | — | T-03-SC | four [ASSUMED]-provenance crates checked on crates.io | manual | checkpoint (user) | — | ⬜ pending |
| 03-01-03 | 01 | 1 | DIST-01 | T-03-01 | lockfile committed, offline build works | manual + build | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked` (after user fetch) | ❌ W0 | ⬜ pending |
| 03-02-01 | 02 | 1 | LOG-01, LOG-02 | T-03-04 | golden values generated from the Python reference | contract | `python3 -m unittest tests.test_contract_stream tests.test_phase2_integration -v` | ❌ W0 | ⬜ pending |
| 03-02-02 | 02 | 1 | CLI-03 | T-03-04 | guard fixture fail-closed cases exit 2 | contract | `python3 -m unittest tests.test_contract_guard tests.test_guard tests.test_phase2_integration -v` | ❌ W0 | ⬜ pending |
| 03-03-01 | 03 | 2 | CLI-01, CLI-02, CLI-03, PROF-02, LOG-01, LOG-02 | T-03-05..T-03-10 | argv arrays only; bypass flags refused; wrong profile refused; DB 0600 | e2e (tracer) | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test cli_e2e` | ❌ W0 | ⬜ pending |
| 03-04-01 | 04 | 3 | CLI-02 | T-03-11, T-03-12 | atomic map write; credentials stripped; bad map untouched | contract (binary) | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test contract_scan` | ❌ W0 | ⬜ pending |
| 03-04-02 | 04 | 3 | CLI-02, PROF-02 | T-03-13 | 0 / 2+ matches stop with exit 2 before side effects | contract | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test contract_routing --test contract_profile` | ❌ W0 | ⬜ pending |
| 03-05-01 | 05 | 3 | CLI-03 | T-03-14 | plan path traversal / symlink escape rejected | contract | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test contract_planfile` | ❌ W0 | ⬜ pending |
| 03-05-02 | 05 | 3 | CLI-03 | T-03-15 | meta comment cannot be closed by request text | contract + parity | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test contract_planfile --test assets_parity` | ❌ W0 | ⬜ pending |
| 03-06-01 | 06 | 3 | LOG-01, LOG-02 | T-03-16 | soft parse; partial usage after SIGKILL | contract | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test contract_stream` | ❌ W0 | ⬜ pending |
| 03-06-02 | 06 | 3 | LOG-01, LOG-02, CLI-01 | T-03-16, T-03-17 | bad lines stored raw; tool output hidden by default; EPIPE-safe | e2e | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test cli_stream` | ❌ W0 | ⬜ pending |
| 03-06-03 | 06 | 3 | CLI-03 | T-03-18, T-03-19 | guard fail-closed; hook command shell-quoted | contract (binary) | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test contract_guard` | ❌ W0 | ⬜ pending |
| 03-07-01 | 07 | 3 | FAST-01 | T-03-24 | status never starts claude, no runs row | e2e | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test fastpath status` | ❌ W0 | ⬜ pending |
| 03-07-02 | 07 | 3 | FAST-02 | T-03-20, T-03-21 | session cleanup only for UUID ids, no symlink follow; diff byte cap | e2e + unit | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test fastpath inspect` | ❌ W0 | ⬜ pending |
| 03-07-03 | 07 | 3 | STAT-01 | T-03-23 | parameterised SQL; profile/since validated | e2e | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test db_stats` | ❌ W0 | ⬜ pending |
| 03-08-01 | 08 | 4 | PROC-01, PROC-02 | T-03-25, T-03-26 | signal only pid+start-time-matched processes; never own group | unit + fake self-test | `python3 -m unittest tests.test_fake_claude_tree -v` and `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib procs -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 03-08-02 | 08 | 4 | PROC-01, PROC-02 | T-03-25, T-03-27, T-03-28 | no survivors after kill/SIGINT×2/SIGHUP/SIGTERM/timeout; reused PID untouched | integration (real process trees) | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test runner_kill -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 03-09-01 | 09 | 4 | CLI-03, PROF-02 | T-03-32 | second personal run refused; bypass flags refused | e2e | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test cli_lead` | ❌ W0 | ⬜ pending |
| 03-09-02 | 09 | 4 | CLI-03 | T-03-29, T-03-30 | Dev confined by rules + guard + post-check; plan commands never run | e2e | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test cli_dev` | ❌ W0 | ⬜ pending |
| 03-09-03 | 09 | 4 | CLI-03, CLI-01 | T-03-31, T-03-33 | QA writes only .qa-report.md | e2e | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test cli_qa` | ❌ W0 | ⬜ pending |
| 03-10-01 | 10 | 5 | DIST-01 | T-03-34 | offline locked static build; runs under env -i | build/system | `bash scripts/build-release.sh` | ❌ W0 | ⬜ pending |
| 03-10-02 | 10 | 5 | DIST-01 | T-03-35, T-03-36 | live steps stop runs only via agent-brain kill / Ctrl+C | unit + docs | `python3 -m unittest tests.test_live_helpers -v` | ❌ W0 | ⬜ pending |
| 03-11-01 | 11 | 6 | CLI-01, PROC-01, PROC-02, FAST-02, DIST-01 | T-03-38 | live kill checks read-only ps on noted PIDs | manual (live) | checkpoint (user) | — | ⬜ pending |
| 03-11-02 | 11 | 6 | CLI-03, PROF-02 | T-03-37 | personal run only with no other ~/.claude-orca session | manual (live) | checkpoint (user) | — | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `agent-brain/Cargo.toml` + committed `Cargo.lock` and fetched crates (plan 03-01; user checkpoint) — cargo test framework
- [ ] `tests/contract/stream.json`, `tests/contract/guard.json` + Python loaders (plan 03-02) — shared fixtures for LOG-01/02 and CLI-03
- [ ] `agent-brain/tests/common/mod.rs` Sandbox (plan 03-03) — every Rust integration test uses it
- [ ] `tests/fakes/fake_claude_tree.py` (plan 03-08) — process-tree fake for PROC-01/02
- [ ] musl target + musl-gcc installed (plan 03-01; user checkpoint) — DIST-01 build

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| P2-01 real Lead run, per-step log | CLI-01 | real claude, tokens | agent-brain/LIVE-CHECKS.md P2-01 (plan 03-11 Task 1) |
| P2-02 real `sleep 307` kill, 5 s | PROC-01 | real claude Bash tool | LIVE-CHECKS P2-02 with scripts/live/claude-allow-sleep.sh |
| P2-03 Ctrl+C ×1/×2, terminal close | PROC-02 | real tty/session semantics | LIVE-CHECKS P2-03 |
| P2-07 no transcript, `--max-turns` accepted on 2.1.301 | FAST-02 | real CLI (research A1) | LIVE-CHECKS P2-07 |
| P2-09 copy to another machine | DIST-01 | another host | LIVE-CHECKS P2-09 (directory case automated in build-release.sh) |
| D-02 one real task via the binary, Python symlink removed | CLI-03 | real project and account | plan 03-11 Task 2 |
| E-06 personal profile transcript under ~/.claude-orca | PROF-02 | shares login with the orchestrating session | plan 03-11 Task 2, only with no other ~/.claude-orca session |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < {N}s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** {pending / approved YYYY-MM-DD}
