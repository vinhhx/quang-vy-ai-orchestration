---
phase: "6"
slug: "multi-engine-media-branch"
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: "2026-10-26"
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Seeded from `06-RESEARCH.md` § Validation Architecture. The per-task map is filled from the PLAN.md files once planning passes the checker.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (lib unit tests and integration tests on `tests/common::Sandbox` with fake claude), `node scripts/dashboard-js-check.mjs` (vm harness), bash UI gates, headless-Chrome smoke with CDP keys |
| **Config file** | `agent-brain/Cargo.toml`. The sandbox config is written by `Sandbox::write_config`, which points `claude_bin` at the fake claude. |
| **Quick run command** | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib -- <module filter>` and `node scripts/dashboard-js-check.mjs` |
| **Full suite command** | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1`, then `bash scripts/dashboard-ui-lint.sh`, `bash scripts/build-css.sh --check`, `node scripts/dashboard-js-check.mjs`, and `bash scripts/dashboard-smoke.sh overview assign task task-stop stats projects board settings --keys` |
| **Estimated runtime** | about 240 seconds for the full suite; under 30 seconds for a module-filtered lib run |

---

## Sampling Rate

- **After every task commit:** run the quick command filtered to the task's module, plus the JS check for UI tasks.
- **After every plan wave:** run that wave's integration tests, plus the UI gates for UI waves.
- **Before `/gsd-verify-work`:** the full suite must be green, `build-css.sh --check` must report `app.css khớp nguồn`, and the user-run live checks in LIVE-CHECKS § 8 must be done.
- **Max feedback latency:** 300 seconds.

---

## Requirement → Test Map (from research)

| Req ID | Behavior | Test Type | Automated Command | File Exists |
|--------|----------|-----------|-------------------|-------------|
| ENG-01 | Mixed team (claude lead/qa, deepseek dev where the deepseek `command` is the fake claude): `runs.engine` per run, header `engine …`, `metrics.team` | integration | `cargo test --test engine_routing` | ❌ W0 |
| ENG-02 | Quota fixtures (claude, deepseek 402/429, agy) give outcome `quota`, `EXIT_QUOTA` and autopilot `HetQuota`; a transient 429 does not stop the task | unit + integration | `cargo test --lib quota`; `cargo test --test engine_quota` | ❌ W0 |
| ENG-03 | A re-run after quota keeps the partial diff, the prompt mentions the diff and `.handover.md`, and no autopilot round is consumed | integration | `cargo test --test engine_quota eng03` | ❌ W0 |
| ENG-04 | Stats `engines[]` splits tokens and cost; an unpriced engine is flagged | integration | `cargo test --test dashboard_stats`, `--test db_stats` | ✅ extend |
| ENG-05 | A planted key is absent from stream.jsonl, events.raw_json, stderr.txt, job output, `runs.error` and git history | integration | `cargo test --test engine_secrets` | ❌ W0 |
| ENG-06 | The `fake_agy.py` stream is parsed and usage mapped; the Dev permission flag applies to Dev only; an empty diff is suspicious; an out-of-scope diff is a protocol violation | integration | `cargo test --test engine_agy` | ❌ W0 |
| ENG-07 | QA must be claude (refused by the loader and by Settings save); "Cần sửa" SQL by Dev engine, including human | unit + integration | `cargo test --lib teams`; `--test dashboard_stats` | ❌ W0 / extend |
| TEAM-* / SET-* / HUM-* / QUOTA-* | New requirement IDs defined by the planner (research proposal: TEAM-01..03, SET-01..05, HUM-01..03, QUOTA-01) | see research table | `--test task_team`, `--test dashboard_settings`, `--test human_engine`, `--test telegram_actions` | ❌ W0 / extend |

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (filled after the plans pass the checker) | | | | | | | | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/fakes/fake_agy.py`: emits `init` / `step_update` / `result`, slow writes, error and exit codes, and an optional soft-deny SUCCESS with no edits
- [ ] Extend the `fake_claude.py` recorder: record env var NAMES and boolean matches for `ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` / the key var, never values; add a scenario that echoes a planted env value (ENG-05)
- [ ] `Sandbox` helpers: `write_teams`, `write_staff`, a config `engines` override, and a bare-remote plus staff-clone helper
- [ ] Fixtures: `tests/fixtures/stream/{claude_quota_*,deepseek_*,agy_*}.jsonl`
- [ ] New test files from the map above; update `db_migration.rs` version assertions to 4
- [ ] A `settings` section in `dashboard-js-check.mjs` and a `settings` case in `dashboard-smoke.sh`

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| E-09: one tiny real `claude -p` through the DeepSeek Anthropic-format endpoint (stream-json, usage, tool use, file edit) | ENG-01, D-02 | Needs the user's DeepSeek key and a real network call | User-run decision-gate plan (LIVE-CHECKS § 8) |
| agy 1.4.7 probe: event shape, narrow permission mode for Dev (D-26), soft-deny behaviour | ENG-06, D-26 | Needs the user's agy login | User-run probe plan |
| End-to-end `tiet-kiem` task (claude Lead/QA, deepseek Dev) | ENG-01 phase gate | Real engines and quota | LIVE-CHECKS § 8 |
| Settings screen look in light and dark themes, and at 1024 / 768 / 480 px | SET-* | Visual judgement | LIVE-CHECKS § 8 browser steps |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 300s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
