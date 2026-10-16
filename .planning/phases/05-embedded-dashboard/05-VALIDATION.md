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

Task IDs mapped from the PLAN.md files (05-01..05-13). Test files are created by the plan that owns the task (no separate Wave 0 plan: the Cargo checkpoint is 05-01; the HTTP/SSE helpers and the tracer test are 05-02; `dashboard_http.rs` and the SSE edge tests are 05-03). Plans 05-03 and 05-09 were split out of the former 05-02 and 05-07 at checker revision 1, and later plans renumbered.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 05-01-T1 | 05-01 | 1 | DASH-01, DASH-09 (deps, CSS tooling) | T-5-01..03, T-5-SC | pinned crates, hash-checked tools | script | `python3 -c "import tomllib; ..."` (manifest assertion in 05-01) | ✅ (Cargo.toml) | ⬜ pending |
| 05-02-T1 | 05-02 | 2 | DASH-01, DASH-05 (tracer); DASH-02/03 bind and gate | T-5-04, T-5-05, T-5-09 | loopback bind, token gate on page, assets and SSE | integration + unit | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_stream -- --test-threads=1` and `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib dashboard` | ❌ created by task | ⬜ pending |
| 05-03-T1 | 05-03 | 3 | DASH-01 (D-14 single instance, D-10 port, D-09 stop) | T-5-13, T-5-46, T-5-47 | second serve refused before any DB write; token never in serve.lock or the refusal | integration + unit | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_http -- --test-threads=1` and `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib dashboard::lock` | ❌ created by task | ⬜ pending |
| 05-03-T2 | 05-03 | 3 | DASH-01 | — | N/A | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_http dash01 -- --test-threads=1` | ❌ created by task | ⬜ pending |
| 05-03-T2 | 05-03 | 3 | DASH-02 | T-5-04 | socket bound to `127.0.0.1` only | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_http dash02 -- --test-threads=1` | ❌ created by task | ⬜ pending |
| 05-03-T2 | 05-03 | 3 | DASH-03 | T-5-05..09 | 401 without token, 421 bad Host, 403 bad Origin, 415 non-JSON, security headers, new token per start | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_http dash03 -- --test-threads=1` | ❌ created by task | ⬜ pending |
| 05-03-T2 | 05-03 | 3 | DASH-05 / D-12 | — | N/A | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_stream -- --test-threads=1` (dash05 resume, d12 external writer, unknown run) | ❌ created by task | ⬜ pending |
| 05-04-T1 | 05-04 | 4 | DASH-04 | T-5-14 | argv + `--`, no shell | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_run dash04 -- --test-threads=1` | ❌ created by task | ⬜ pending |
| 05-04-T2 | 05-04 | 4 | DASH-07 | T-5-15 | ambiguous project never starts a run | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_run dash07 -- --test-threads=1` | ❌ created by task | ⬜ pending |
| 05-04-T2 | 05-04 | 4 | DASH-08 (start) / D-08 | T-5-16, T-5-17 | UI cannot set a profile; serve stop/crash leaves no process | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_run --test dashboard_kill -- --test-threads=1` | ❌ created by task | ⬜ pending |
| 05-05-T1 | 05-05 | 5 | DASH-04 (step states) | — | N/A | unit | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib dashboard::taskstate` | ❌ created by task | ⬜ pending |
| 05-05-T2 | 05-05 | 5 | DASH-04, DASH-08 (task list) | T-5-21, T-5-23 | plan read-only, branch from DB | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_pipeline -- --test-threads=1` | ❌ created by task | ⬜ pending |
| 05-05-T3 | 05-05 | 5 | DASH-06 | T-5-20 | no orphan processes after UI kill (UI- and terminal-started) | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_kill dash06 -- --test-threads=1` | ❌ created by task | ⬜ pending |
| 05-06-T1/T2 | 05-06 | 5 | DASH-08 (stats), DASH-11 | T-5-24 | parameterised SQL, validated profile/since | unit + integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_stats -- --test-threads=1` | ❌ created by task | ⬜ pending |
| 05-07-T1 | 05-07 | 5 | DASH-07 (suggest), DASH-08 (picker) | — | N/A | unit + integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test routing_suggest --test dashboard_projects -- --test-threads=1` | ❌ created by task | ⬜ pending |
| 05-07-T2 | 05-07 | 5 | DASH-12 | T-5-26, T-5-27 | etag conflict → 409, other map keys preserved, no profile edit | integration | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_projects -- --test-threads=1` | ❌ created by task | ⬜ pending |
| 05-08-T1/T2 | 05-08 | 6 | DASH-08, DASH-11 (UI shell, Tổng quan) | T-5-30..32 | x-text only, no external URL, token cookie never read by JS | script + headless | `bash scripts/dashboard-smoke.sh overview` | ❌ created by task | ⬜ pending |
| 05-09-T1 | 05-09 | 7 | DASH-04, DASH-07, DASH-08 (Giao việc UI) | T-5-33, T-5-48, T-5-49 | start body never carries a profile; only a listed project id can start a run | script + headless | `bash scripts/dashboard-smoke.sh overview assign` | ❌ created by task | ⬜ pending |
| 05-10-T1/T2 | 05-10 | 8 | DASH-04, DASH-05, DASH-06 (UI) | T-5-34 | agent output never rendered as HTML | script + headless | `bash scripts/dashboard-smoke.sh overview assign task` | ❌ created by task | ⬜ pending |
| 05-11-T1/T2 | 05-11 | 9 | DASH-11, DASH-12, DASH-08 (UI) | T-5-37, T-5-39 | N/A | script + headless | `bash scripts/dashboard-smoke.sh stats projects` | ❌ created by task | ⬜ pending |
| 05-11-T3 | 05-11 | 9 | DASH-09 | T-5-38 | no external loads in embedded assets | unit | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_assets -- --test-threads=1` | ❌ created by task | ⬜ pending |
| 05-08..05-11 | 05-08..05-11 | 6-9 | JS syntax | — | N/A | script | `for f in agent-brain/assets/dashboard/js/*.js; do node --check "$f" \|\| exit 1; done` | ❌ created by task | ⬜ pending |
| 05-12-T1 | 05-12 | 10 | DASH-01 (release), DIST-01 kept | T-5-41 | token never printed | script | `bash scripts/build-release.sh` (prints `P2-09 OK` and `P4-01 OK`) | ✅ (extended) | ⬜ pending |
| 05-12-T2 / 05-13 | 05-12, 05-13 | 10-11 | DASH-10 | — | N/A (D-18: no Figma design) | — | P4-10 row `Không áp dụng` in protocol-log/nghiem-thu-p4.md; record in VERIFICATION | — | — |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Cargo checkpoint (user): `axum =0.8.9`, `futures-util =0.3.34`, tokio features `net` + `rt-multi-thread`; `cargo fetch` and `Cargo.lock` committed so tests compile `--offline --locked` — plan 05-01 Task 3
- [ ] `agent-brain/assets/dashboard/` files exist before `include_bytes!` needs them — `index.html` + `app.css` + vendored Alpine by 05-01/05-02; each screen's files by the UI plan that adds its ASSETS entry (05-08..05-11)
- [ ] `agent-brain/tests/dashboard_stream.rs` (tracer test) + shared raw-HTTP/SSE helpers in `tests/common/mod.rs` — plan 05-02; `agent-brain/tests/dashboard_http.rs` and the DASH-05/D-12 edge tests in `dashboard_stream.rs` — plan 05-03 (DASH-01/02/03/05)
- [ ] `dashboard_run.rs`, `dashboard_kill.rs` (05-04), `dashboard_pipeline.rs` (05-05), `dashboard_stats.rs` (05-06; replaces the seeded name `dashboard_api.rs`), `routing_suggest.rs` + `dashboard_projects.rs` (05-07), `dashboard_assets.rs` (05-11)

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
