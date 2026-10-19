---
phase: 05-embedded-dashboard
verified: 2026-10-19T05:30:00Z
status: human_needed
score: 5/5 roadmap truths verified (12/12 requirements satisfied; DASH-10 satisfied as not applicable)
covered_files:
  - ".planning/phases/05-embedded-dashboard/05-01-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-01-SUMMARY.md"
  - ".planning/phases/05-embedded-dashboard/05-02-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-02-SUMMARY.md"
  - ".planning/phases/05-embedded-dashboard/05-03-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-03-SUMMARY.md"
  - ".planning/phases/05-embedded-dashboard/05-04-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-04-SUMMARY.md"
  - ".planning/phases/05-embedded-dashboard/05-05-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-05-SUMMARY.md"
  - ".planning/phases/05-embedded-dashboard/05-06-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-06-SUMMARY.md"
  - ".planning/phases/05-embedded-dashboard/05-07-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-07-SUMMARY.md"
  - ".planning/phases/05-embedded-dashboard/05-08-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-08-SUMMARY.md"
  - ".planning/phases/05-embedded-dashboard/05-09-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-09-SUMMARY.md"
  - ".planning/phases/05-embedded-dashboard/05-10-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-10-SUMMARY.md"
  - ".planning/phases/05-embedded-dashboard/05-11-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-11-SUMMARY.md"
  - ".planning/phases/05-embedded-dashboard/05-12-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-12-SUMMARY.md"
  - ".planning/phases/05-embedded-dashboard/05-13-PLAN.md"
  - ".planning/phases/05-embedded-dashboard/05-13-SUMMARY.md"
  - "agent-brain/assets/dashboard/app.css"
  - "agent-brain/assets/dashboard/index.html"
  - "agent-brain/assets/dashboard/js/app.js"
  - "agent-brain/assets/dashboard/js/assign.js"
  - "agent-brain/assets/dashboard/js/task.js"
  - "agent-brain/src/dashboard/api_projects.rs"
  - "agent-brain/src/dashboard/api_runs.rs"
  - "agent-brain/src/dashboard/api_stats.rs"
  - "agent-brain/src/dashboard/api_tasks.rs"
  - "agent-brain/src/dashboard/assets.rs"
  - "agent-brain/src/dashboard/auth.rs"
  - "agent-brain/src/dashboard/jobs.rs"
  - "agent-brain/src/dashboard/mod.rs"
  - "agent-brain/src/dashboard/sse.rs"
  - "agent-brain/src/dashboard/taskstate.rs"
  - "agent-brain/src/stats.rs"
  - "protocol-log/nghiem-thu-p4.md"
covered_digest: "v3:sha256:89ef65843e565c230a15c3f00df03b44fad60cd71db5e75c2a45c41bd12b4f58"
behavior_unverified: 0
overrides_applied: 0
gaps: []
deferred: []
flagged_prohibitions:
  - statement: "05-01: MUST NOT vendor @alpinejs/csp before the user has confirmed the package on npmjs.com and GitHub"
    verification: judgment
    llm_verdict: "held (non-authoritative): 05-01-SUMMARY records the user reply 'approved' at the legitimacy checkpoint; vendored file sha256 matches VENDOR.md"
  - statement: "05-08/05-09: MUST NOT offer a control that chooses or overrides the profile of a new task (the switcher only filters views)"
    verification: judgment
    llm_verdict: "held (non-authoritative): the only profile control is the header filter <select id=ab-profile> (title says it filters only); POST /api/tasks body is { project, request, name }; d15_profile_field_rejected test passes"
  - statement: "05-10: MUST NOT provide any control that edits .plan.md, .qa-report.md or creates a commit"
    verification: judgment
    llm_verdict: "held (non-authoritative): task.js issues only POST /api/runs/{id}/kill and POST /api/tasks/{id}/{dev|qa|relead} with empty bodies; plan/QA are rendered read-only"
  - statement: "05-11: MUST NOT offer a control that edits a project's profile, id, path or type"
    verification: judgment
    llm_verdict: "held (non-authoritative): projects.js PUT body is { aliases, etag } only; d15_profile_not_editable test passes"
  - statement: "05-13: MUST NOT start a personal-profile run during live checks while another Claude session uses ~/.claude-orca"
    verification: judgment
    llm_verdict: "not provable from the codebase: the live checks used profile work (runs 13-15 profile=work per the DB); no personal run is recorded"
  - statement: "05-13: MUST NOT stop live processes with pkill/killall or by process name"
    verification: judgment
    llm_verdict: "held (non-authoritative): no pkill/killall in scripts/dashboard-*.sh or scripts/dashboard-keys.mjs; 05-13-SUMMARY Review section records PID-only signals"
human_verification:
  - test: "Confirm the six judgment-tier prohibitions in flagged_prohibitions above (unverified-prohibition, human review recommended)"
    expected: "Each statement held during execution. The LLM-judge verdicts above are NON-AUTHORITATIVE and rest on grep and SUMMARY evidence. The two 05-13 prohibitions concern the executor's live behaviour, which the code cannot prove."
    why_human: "ADR-550 D4: a judgment-tier prohibition is never a silent pass. A human must resolve each one."
---

# Phase 5: Embedded Dashboard Verification Report

**Phase Goal:** Người dùng xem task và log realtime, giao việc, kill và xem thống kê token qua web UI nằm trong chính binary `agent-brain`, vẫn là một file chạy duy nhất.
**Verified:** 2026-10-19
**Status:** human_needed. Every must-have is verified. The only open items are the flagged judgment-tier prohibitions, which need a human acknowledgement.
**Re-verification:** No, initial verification.

## Verdict

The phase goal is achieved in the codebase. I found no failed truth, missing or stub artifact, unwired link or blocker anti-pattern. SUMMARY claims were checked against the code and against tests I ran myself (below).

The status is `human_needed` for one reason only. Six judgment-tier `must_haves.prohibitions` carry a non-authoritative LLM verdict and are flagged under ADR-550 D4, which forbids a silent pass. This does not reflect a missing capability. Acknowledging the items, or an override, moves the phase to `passed`.

## Goal Achievement

### Observable Truths (ROADMAP success criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Run the binary and the dashboard opens with nothing else installed. The UI keeps its style offline. A LAN machine cannot reach it. Access without a token is refused. | VERIFIED | **Single file:** `assets.rs` embeds all 16 assets with `include_bytes!`. The installed `~/.local/bin/agent-brain` is `static-pie linked, stripped`, 8.2 MB (sha256 `2b1bb2c8…`). `scripts/build-release.sh` serves it under `env -i PATH=/nonexistent` and prints `P4-01 OK`. **Offline:** `app.css` is prebuilt Tailwind. `dashboard_assets` (6 tests, re-run: pass) proves `dash09_no_external_loads`, and the only URL in the assets is a license comment. `build-css.sh --check` reports `app.css khớp nguồn`. **Loopback:** `mod.rs` binds `TcpListener::bind((Ipv4Addr::LOCALHOST, port))`, and `dash02_listens_on_loopback_only` passes (re-run). **Token:** `auth::gate` returns 421 for a bad Host, 401 for a bad cookie or `?token=` (constant-time compare), and 403 for a bad Origin or 415 for a non-JSON POST. The token comes from `/dev/urandom`, one per start. `dash03_*` (3 tests) pass (re-run). Live P4-01, P4-02, P4-03 and P4-09 are recorded as Đạt. |
| 2 | Assign a Vietnamese task from the UI and the log is live, with Lead → Dev → QA changing state. An ambiguous project name lists the matches and does not run. | VERIFIED | `POST /api/tasks` goes to `JobManager::start`, which spawns a child `agent-brain run` in its own process group (PDEATHSIG, via the spawner thread). `taskstate.rs` derives step states and enabled actions. There is no auto-chaining, and dev/qa/relead are separate POSTs. `dash04_*`, `dash07_ambiguous_or_unknown_project_never_runs`, `d15_profile_field_rejected` and the `dashboard_pipeline` tests exist and are green (full suite 243/0 per the orchestrator). `assign.js` sends `{project, request, name}` only, with the project taken from the suggest list. Live: P4-04 (Lead run 13, Dev run 14, QA run 15, `qa:pass`, one button press per step) and P4-07 (alias `nhap` on two projects listed both and the button stayed disabled) are Đạt. |
| 3 | Reload mid-run and the old log is still there and new lines keep arriving. | VERIFIED | `sse.rs` reads `Last-Event-ID` and replays from SQLite `events`. The client uses `EventSource`, and `pump.rs` feeds the hints. `dash05_resume_with_last_event_id` passes (re-run). `d12_external_writer_rows_arrive_live` exists. Live: P4-05 (user, F5 mid-run) is Đạt. |
| 4 | Press Kill in the UI and the run stops with no leftover processes. | VERIFIED (behavior-dependent, test-backed) | `task.js:756` posts `/api/runs/{id}/kill` and `api_runs.rs` calls `runner::cmd_kill`, so signalling is PID plus start-time verified. `dash06_kill_dashboard_started_run` ran and passed (5.2 s, re-run): no `fake`, `bash` or `sleep` survives after 5 s. `dash06_kill_terminal_started_run`, `dash06_kill_escalates_when_sigterm_ignored` and `d08_*` (Ctrl+C, SIGHUP and crash of serve) exist. Live P4-06 (real claude 2.1.303, tree 84689 → 84880 → 84881 and the terminal-started tree) records `pgrep` clean after Kill, for both UI-started and terminal-started runs. See WR-11 below for the one narrow residual. |
| 5 | The profile switch drives stats and new tasks. Thống kê shows day, profile, project and role. Dự án lists projects with alias edit and rescan. A Figma match where a design exists. | VERIFIED (Figma part not applicable) | **Profile:** the header `<select id=ab-profile>` filters views. A new task takes the project's own profile (the client never sends one). `dash08_stats_profile_filter` exists. Live P4-08 is Đạt (runs 13-15 profile=work per the DB). **Stats:** `api_stats.rs` returns `StatsReport` (which has `by_project` and `by_role`) plus `by_day` and `by_profile`. `stats.html` has the headings Theo ngày, Theo profile, Theo dự án and Theo vai trò. `dash11_stats_breakdowns` exists. **Projects:** the `api_projects.rs` routes are list, suggest, scan, `PUT /{id}` (aliases and etag) and status. `dash12_*` (6 tests) exist. `projects.js` wires PUT and scan. **Figma:** see DASH-10. |

**Score:** 5/5 truths verified. `behavior_unverified`: 0. Every behavior-dependent truth (kill with no leftovers, SSE resume) has a named passing test that I re-ran.

### Plan-level must-haves (spot-checked)

| Must-have | Status | Evidence |
|-----------|--------|----------|
| 05-01: `tools/` gitignored, Tailwind never committed or embedded | VERIFIED | `git check-ignore -v agent-brain/tools` matches `.gitignore:8`. |
| 05-01: Alpine CSP vendored with the pinned hash | VERIFIED | `sha256sum` gives `0d18d7f8…4519a`, equal to `VENDOR.md`. |
| 05-02/03: only 127.0.0.1, token gate, single instance, loopback and port errors | VERIFIED | `mod.rs` and `auth.rs` read; `d10_*`, `d14_second_serve_refused` and `dash0x` exist. |
| 05-04: `jobs.rs` spawner thread plus PDEATHSIG, serve stop kills its runs | VERIFIED | Code read; `d08_*` tests exist. |
| 05-05: kill only via `runner::cmd_kill` | VERIFIED | `api_runs.rs` code read. |
| 05-12/13: release smoke from the static binary, acceptance table | VERIFIED | `build-release.sh` lines 66-173; `protocol-log/nghiem-thu-p4.md` read in full. |

### Required Artifacts and Key Links

| Artifact or link | Status | Details |
|------------------|--------|---------|
| `src/dashboard/{mod,auth,assets,sse,pump,jobs,lock,api_*,taskstate}.rs` | VERIFIED | All substantive and wired. `router()` merges `api_tasks`, `api_runs`, `api_projects` and `api_stats` plus the SSE route, with `auth::gate` as the outermost layer. `main.rs` and `lib.rs` expose `serve`. |
| `assets/dashboard/{index.html, app.css, vendor/, js/×7, screens/×5}` | VERIFIED | All 16 are in `ASSETS` via `include_bytes!`. `dashboard_assets::asset_references_resolve` passes. |
| UI → API links | WIRED | `AB.api` calls found for `/api/tasks`, `/api/runs/{id}/kill`, `/api/tasks/{id}/{dev,qa,relead}`, `/api/stats`, `/api/projects`, `/suggest`, `/scan`, the PUT, and `/api/runs/{id}/stream` via `EventSource`. |
| `jobs.start` → child `agent-brain run` → SQLite → SSE | WIRED | The child writes `runs` and `events`. `pump` and `sse` read them. A live end-to-end run is recorded in P4-04. |

### Data-Flow Trace (Level 4)

| Artifact | Data | Source | Real data | Status |
|----------|------|--------|-----------|--------|
| Thống kê | `by_day`, `by_profile`, `by_project`, `by_role` | `stats::query_*` over `brain.db` (parameterised SQL) | Yes | FLOWING |
| Log pane | event rows | SQLite `events` through SSE | Yes (P4-04 and P4-05 live) | FLOWING |
| Tổng quan / Chi tiết task | task and run state | `taskstate::derive` over DB, git and plan facts | Yes | FLOWING |

### Behavioral Spot-Checks (run by this verifier)

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Loopback bind, token refusal, headers | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_http dash0 -- --test-threads=1` | 5 passed | PASS |
| Offline assets and no external loads | `... --test dashboard_assets` | 6 passed | PASS |
| UI Kill, no leftovers (behavior-dependent) | `... --test dashboard_kill dash06_kill_dashboard_started_run` | 1 passed, 5.2 s | PASS |
| SSE resume after reload (behavior-dependent) | `... --test dashboard_stream dash05` | 1 passed | PASS |
| UI lint and CSS freshness | `bash scripts/dashboard-ui-lint.sh` and `bash scripts/build-css.sh --check` | `UI lint OK: 40 thẻ`, `app.css khớp nguồn` | PASS |
| Static binary | `file ~/.local/bin/agent-brain` | `static-pie linked, stripped` | PASS |

I did not re-run the full suite or the live and real-claude checks. I relied on the orchestrator's regression gate for those: 243 passed, 0 failed, fmt clean, smoke with `--keys` OK, and the Python suite OK.

### Probe Execution

Step 7c: SKIPPED. No `scripts/*/tests/probe-*.sh` exists, and no PLAN declares a probe script.

### Requirements Coverage

Every ID in PLAN frontmatter (DASH-01..DASH-12, the union over plans 05-01..05-13) appears in REQUIREMENTS.md, and the Phase 5 traceability rows are the same twelve IDs. There are no orphaned requirements.

| Requirement | Source plans | Status | Evidence |
|-------------|--------------|--------|----------|
| DASH-01 (one binary, nothing to install) | 05-01, 02, 03, 12, 13 | SATISFIED | `include_bytes!` embedding; static-pie binary; `P4-01 OK` in `env -i`; live P4-01 Đạt. |
| DASH-02 (LAN cannot reach it) | 05-02, 03, 13 | SATISFIED | Bind to 127.0.0.1 only, and `dash02` passes. See the P4-02 note below. |
| DASH-03 (no token, refused) | 05-02, 03, 13 | SATISFIED | `auth::gate` plus `dash03_*`; live P4-03 Đạt across two restarts. |
| DASH-04 (assign, live log, step states) | 05-04, 05, 09, 10, 13 | SATISFIED | See truth 2. |
| DASH-05 (reload keeps the log, SSE resume) | 05-02, 03, 10, 13 | SATISFIED | `dash05_resume_with_last_event_id`; live P4-05. |
| DASH-06 (Kill, no leftover) | 05-05, 10, 13 | SATISFIED | See truth 4. |
| DASH-07 (ambiguous name lists, does not run) | 05-04, 07, 09, 13 | SATISFIED | `dash07_*`, the `resolve` gate and the combobox; live P4-07. |
| DASH-08 (profile switch) | 05-04..05-11, 13 | SATISFIED | Profile never client-chosen; `dash08_*`; live P4-08. |
| DASH-09 (offline styles) | 05-01, 11, 12, 13 | SATISFIED | `dash09_*`; prebuilt CSS; live P4-09 (network off). |
| DASH-10 (Figma checklist if a design exists, otherwise not applicable) | 05-12, 05-13 | SATISFIED (not applicable) | See the note below. |
| DASH-11 (stats by day, profile, project, role; Tổng quan counts) | 05-06, 08, 11, 13 | SATISFIED | `api_stats.rs`, `stats.html`, `dash11_stats_breakdowns`; live P4-01 rendered all five screens on real data. |
| DASH-12 (Dự án: list, alias edit, rescan) | 05-07, 11, 13 | SATISFIED | `dash12_*` (6 tests); `projects.js` PUT and scan. |

**DASH-10, explicitly.** REQUIREMENTS.md phrases it as conditional: "Nếu có thiết kế (Figma), UI khớp theo checklist UI của người dùng; nếu chưa có thiết kế thì không áp dụng". The condition is "a design exists", and it is false. 05-CONTEXT D-18 records no Figma design, and the P4-10 row says `Không áp dụng` with the reason. The requirement's own text defines that outcome as compliance, so it is satisfied and does not count as skipped or failed. The UI was instead checked against the approved `05-UI-SPEC.md` (UI-01..UI-06 all Đạt, and the `05-UI-REVIEW.md` audit gave 21/24 with 0 blockers). If a Figma design appears later, DASH-10 becomes applicable and needs a new check.

**P4-02, the cross-machine half.** The LAN machine could not connect (`Couldn't connect`), but it also cannot reach port 8000 on this host, so that test alone is inconclusive. It was recorded Đạt on local control evidence at the user's choice. In my judgment the requirement is still satisfied. The bind address is a deterministic code fact (`Ipv4Addr::LOCALHOST`), asserted by the passing `dash02_listens_on_loopback_only`, and it is backed by `ss -ltn` (only 127.0.0.1:7437) and the host's own LAN IP refusing 7437 while answering on 8000. A packet from another machine cannot reach a loopback-only socket. This is a recorded limitation, not a gap. A cross-machine test from a reachable host is optional.

### Anti-Patterns Found

| Scan | Result |
|------|--------|
| TBD, FIXME, XXX markers in files modified by the phase | None. |
| TODO, HACK, PLACEHOLDER, "not yet implemented" in `src/dashboard` and `js/` | None. |
| Stubs (static returns with no data source) | None. Every API reads SQLite, git or the map file. |
| `x-html`, `eval`, `innerHTML` | Only `app.js:315` (embedded static screen HTML, not agent data). |

### Open code-review findings

05-REVIEW.md has 1 critical, 13 warnings and 12 info items. 05-REVIEW-DISPOSITION.md has CR-01 deferred as accepted risk AR-5-01 and all 25 others `open`. I read each cited code path for the findings that could matter. None defeats a requirement or a plan truth. They are classified below. **None is a gap.**

| Finding | Classification | Reasoning |
|---------|----------------|-----------|
| CR-01 (cookie not port-isolated) | Non-blocking, accepted risk AR-5-01 | A user-approved threat-register item (T-5-11, RFC 6265 §8.5). DASH-03 requires that access without the token be refused, and it is (401 or 421 or 403, test-proven). CR-01 describes a hostile same-user local process stealing a valid token, which is out of DASH-03's scope. The fix (header token plus `fetch` streaming) is a design change, so schedule it as hardening. Cookie-tossing lock-out (`cookie_value` checks the first match only) is the cheap part and can ship alone. |
| WR-05 (E-06 check then reserve, non-atomic) | Non-blocking follow-up (recommended soon) | **Real**: `busy_reason` runs before `JobManager::start`, and `start()` rejects only a duplicate project, so two concurrent POSTs for two different personal projects can both pass. E-06 and D-17 are CONTEXT decisions rather than DASH requirement text: DASH-08 says new tasks follow the right profile, and the profile is attributed correctly. The race needs two POSTs within the spawn window, such as two tabs or a double submit (the UI disables the button), and defence in depth exists. The child `agent-brain run` re-checks E-06 in `roles/mod.rs:execute_role` against live DB runs. That check is itself narrow-window, so the exposure is small but not zero, and the shared `~/.claude-orca` config is what it protects. The fix is a 3-line check under the same mutex plus a concurrency test. |
| WR-11 (kill validates run X, kills the task's current run) | Non-blocking follow-up | **Real**: `post_kill_run` validates run `id` and then calls `cmd_kill(ctx, task_id)`, which re-resolves the running run. A mis-kill needs run X to finish and a new run for the same task to start between the check and the kill. In the pipeline a new step starts only after a user button press, and Kill sits behind a confirm dialog, so the window is milliseconds and needs a user action in it. DASH-06's core outcome (the run stops, no leftover) is test-proven and live-proven. The fix is a `run_id` guard in `cmd_kill`. It targets a wrong run in a rare race, not a failure of the requirement. |
| WR-06 (job to task matched by branch only) | Non-blocking follow-up | Needs the same slug in two projects with a job running in each. It can show the wrong job on a task page and widen the WR-05 window. A one-line `j.project == project.id &&` fix. |
| WR-01 (`parse_since` overflow) | Non-blocking follow-up | Absurd `since` values on a local, authenticated query. Wrong window or a 500, never data loss. It is also in the Phase 3/4 CLI path. |
| WR-02 (README API catalogue wrong) | Non-blocking follow-up (docs) | The routes themselves work. The README lists some that do not exist. Fix it before the README is relied on. |
| WR-03, WR-04, WR-09, WR-10 (focus return, combobox race, router and dialog stale-response guards) | Non-blocking follow-up (UX and a11y) | WR-03 and WR-04 are already known in 05-13-SUMMARY and 05-UI-REVIEW. The server resolve gate (DASH-07) still prevents an ambiguous start, and the chosen project is shown before the start. |
| WR-07, WR-08, WR-12, WR-13 | Non-blocking follow-up | Scan serialisation, non-UTF-8 output readers, duplicated derivation (maintenance), and internal error text in 500s. These are robustness and hardening items with no requirement impact. |
| IN-01..IN-12 | Informational | Dead code, keys, test hygiene. |

No finding contradicts a must-have truth. Because several concern invariants the README promises (WR-05, WR-06, WR-11), I recommend one small agy fix plan covering WR-05, WR-06 and WR-11 (server, with tests), then another for the front-end items, before the next milestone phase builds on the dashboard.

Other audit artifacts: `05-VALIDATION.md` has `nyquist_compliant: true`. `05-SECURITY.md` has 52/52 threats closed, with accepted risks AR-5-01..03. `05-UI-REVIEW.md` scores 21/24 with 0 blockers.

### Human Verification Required

#### 1. Flagged judgment-tier prohibitions

**Test:** Review the six statements in the `flagged_prohibitions` frontmatter list and acknowledge each one.
**Expected:** Each prohibition held. My grep and SUMMARY-based verdicts support the first four (no profile chooser, no edit-plan control, no project-profile edit, Alpine vendored only after the user's "approved"). The last two concern live executor behaviour: no personal run while another session used `~/.claude-orca`, and no pkill or kill-by-name. I could only confirm that the repo scripts contain none and that the 05-13-SUMMARY says so.
**Why human:** A judgment-tier prohibition gets an LLM-judge verdict that is non-authoritative, so under ADR-550 D4 it is flagged and never silently passed. Resolution is acknowledgement, or an override entry in this file's frontmatter.

(The cross-machine P4-02 half is optional, not required, as explained above.)

### Gaps Summary

No gaps. The phase goal is observably true in the code and was confirmed live: one static binary serves a token-gated, loopback-only, offline-styled dashboard that assigns tasks, streams and resumes logs, kills cleanly and reports stats. The status is `human_needed` only because the ADR-550 D4 flag on six judgment prohibitions needs a human acknowledgement. The open review findings are classified above as non-blocking follow-ups, and WR-05, WR-06 and WR-11 are worth fixing soon.

---

_Verified: 2026-10-19_
_Verifier: Claude (gsd-verifier)_
