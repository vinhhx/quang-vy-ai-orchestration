---
phase: "5"
slug: "embedded-dashboard"
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: "2026-10-17"
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Seeded from `05-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust 1.98.1) with `tempfile`; `tests/common/mod.rs` `Sandbox` (temp HOME, XDG dirs, `AGENT_BRAIN_DB`, `AGENT_BRAIN_CONFIG`, fake claude); Python `tests/fakes/fake_claude.py`, `fake_claude_tree.py` |
| **Config file** | none beyond `agent-brain/Cargo.toml`; new tests `agent-brain/tests/dashboard_*.rs` |
| **Quick run command** | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib` plus the touched `--test dashboard_*` |
| **Full suite command** | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked && python3 -m unittest discover -s tests -t . -q` |
| **Estimated runtime** | ~120 seconds (full suite, warm build) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib` + the touched `dashboard_*` test
- **After every plan wave:** Run the full suite command
- **Before `/gsd-verify-work`:** Full suite must be green, plus the manual P4 rows below
- **Max feedback latency:** 180 seconds

---

## Per-Task Verification Map

Task IDs are filled in once PLAN.md files exist; rows below are per requirement.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | TBD | TBD | DASH-01 | — | N/A | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_http dash01` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | DASH-02 | T-5-* | socket bound to `127.0.0.1` only | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_http dash02` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | DASH-03 | T-5-* | 401 without token, 421 bad Host, 403 bad Origin, new token per start | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_http dash03` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | DASH-04 | — | N/A | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_run dash04` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | DASH-05 | — | N/A | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_run dash05` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | DASH-06 | — | no orphan processes after kill / server stop | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_kill dash06` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | DASH-07 | — | ambiguous project never starts a run | integration + unit | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_run dash07` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | DASH-08 | — | UI cannot override project profile | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_api dash08` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | DASH-09 | — | no external loads in embedded assets | unit + manual | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_assets` | ❌ W0 | ⬜ pending |
| — | — | — | DASH-10 | — | N/A (D-18: no Figma design) | — | record "not applicable" in VERIFICATION | — | — |
| TBD | TBD | TBD | DASH-11 | — | N/A | unit + integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_api` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | DASH-12 | — | etag conflict → 409, other map keys preserved | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_projects` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | JS syntax | — | N/A | script | `for f in agent-brain/assets/dashboard/js/*.js; do node --check "$f" \|\| exit 1; done` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Cargo checkpoint (user): `axum =0.8.9`, `futures-util =0.3.34`, tokio features `net` + `rt-multi-thread`; `cargo fetch` and `Cargo.lock` committed so tests compile `--offline --locked`
- [ ] `agent-brain/assets/dashboard/` placeholders so `include_bytes!` compiles from the first server plan
- [ ] `agent-brain/tests/dashboard_http.rs` + shared raw-HTTP helper in `tests/common` — DASH-01/02/03
- [ ] `dashboard_run.rs`, `dashboard_kill.rs`, `dashboard_api.rs`, `dashboard_projects.rs`, `dashboard_assets.rs`

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| LAN machine cannot reach the dashboard | DASH-02 (P4-02) | needs a second machine | From another LAN host, open `http://<host-ip>:<port>/` — connection refused |
| Styles intact with network off | DASH-09 (P4-09) | real browser, network toggle | Disable network, load dashboard, compare to online view |
| Real run from UI, live log, reload mid-run | DASH-04/05 (P4-04/05) | real `claude` | Assign Vietnamese task in UI, watch stepper and log, reload mid-run |
| Kill from UI leaves no processes | DASH-06 (P4-06) | real `claude` | Live run with `python3 -c 'import time; time.sleep(307)'`, press Kill, check no descendants |
| UI smoke under CSP | DASH-01/09 | headless browser | `google-chrome --headless=new --no-sandbox --virtual-time-budget=4000 --dump-dom <url>` shows mounted screen |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 180s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
