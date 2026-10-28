---
phase: 06-multi-engine-media-branch
plan: 25
subsystem: engines
tags: [rust, agent-brain, dsh, quota, autopilot, eng-09, eng-02]
requires:
  - phase: 06-multi-engine-media-branch
    provides: "dsh engine, StreamFormat::Text, Summary.result_text for dsh text, fake_docker.py / fake_dsh.py, engine_dsh.rs helpers (06-23); config EnginesCfg::kind_of (06-11); quota chain and exit 5 (06-07); agy:empty stop (06-09, 06-10)"
provides:
  - "roles::dev::DSH_EMPTY_MESSAGE; outcome dsh:empty; extra Dev diff checks keyed on the engine KIND of runs.engine through cfg.engines.kind_of (Agy -> agy:empty, Dsh -> dsh:empty, other or unknown id -> none)"
  - "autopilot::decide: dsh:empty is not ok and stops with StopReason::VuongMac, detail line DSH_EMPTY_MESSAGE; dashboard::taskstate step state (vuong_mac, dsh không sửa file)"
  - "quota::detect_text(exit_code, stdout_tail, stderr_tail) and the StreamFormat::Text branch of the quota chain; metrics_json.quota.signal = text"
affects: [06-24, 06-26]

plan_head_before: 32c120b2970a3f8f6814b5c4811ebf3aaaa7e251
plan_head_after: 900128d1f614afe23544f341c53e6f689a679dc5

actuals:
  tokens: 8000
  tasks: 2
  commits: 4

tech-stack:
  added: []
  patterns:
    - "post-run Dev checks decided by engine kind, not by engine id"
    - "text-only engines: quota only on a non-zero exit, stderr tail before stdout tail"

key-files:
  created: []
  modified:
    - agent-brain/src/roles/dev.rs
    - agent-brain/src/autopilot/decide.rs
    - agent-brain/src/autopilot/view.rs
    - agent-brain/src/dashboard/taskstate.rs
    - agent-brain/src/quota.rs
    - agent-brain/src/roles/mod.rs
    - agent-brain/tests/engine_dsh.rs

key-decisions:
  - "The extra Dev checks and the empty-diff outcome key on EngineKind of the run's engine id (an unknown id gets none, never a panic); the name equality with ENGINE_AGY is gone, so an agy-kind id such as agy2 now gets agy:empty too"
  - "detect_text returns None for exit 0 and for an unknown exit code, whatever the text says (an answer cannot be told from a quota notice); stderr is tried before stdout"
  - "No gateway budget phrases added to the shared text_matches regex (assumption A24 not adopted)"

patterns-established:
  - "Integration tests that need several independent project states use one Sandbox per scenario block inside one test fn"

requirements-completed: [ENG-09, ENG-02]

coverage:
  - id: D1
    description: "dsh Dev run with an empty diff stops as dsh:empty: exit 4, DSH_EMPTY_MESSAGE on stdout, no commit"
    requirement: ENG-09
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_dev_outcomes"
        status: pass
    human_judgment: false
  - id: D2
    description: ".planning/ and secret-looking paths are protocol violations for dsh even when the plan lists them; a file outside the plan is a violation (exit 3, protocol_violation)"
    requirement: ENG-09
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_dev_outcomes"
        status: pass
    human_judgment: false
  - id: D3
    description: "an agy-kind engine with an id other than agy gets the same checks and agy:empty (kind-based, D-35)"
    requirement: ENG-09
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_agy_kind_other_id_gets_agy_empty"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_agy.rs (13 tests, unchanged)"
        status: pass
    human_judgment: false
  - id: D4
    description: "autopilot stops vuong_mac with the dsh message after dsh:empty; nothing else starts; RunFact::is_ok false; task screen shows dsh không sửa file"
    requirement: ENG-09
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_empty_stops_autopilot"
        status: pass
      - kind: unit
        ref: "agent-brain/src/autopilot/decide.rs#autopilot::decide::tests::d2_dsh_empty_stops_vuong_mac"
        status: pass
      - kind: unit
        ref: "agent-brain/src/dashboard/taskstate.rs#dashboard::taskstate::tests::step_state_dsh_empty"
        status: pass
      - kind: unit
        ref: "agent-brain/src/autopilot/view.rs (stop_detail with the dsh detail line)"
        status: pass
    human_judgment: false
  - id: D5
    description: "dsh run with a non-zero exit and a limit phrase in stderr or stdout stops as quota: exit 5, metrics quota {engine dsh, signal text, reset_at null}, runs.error quota: ..., no retry or engine switch"
    requirement: ENG-02
    verification:
      - kind: unit
        ref: "agent-brain/src/quota.rs#quota::tests::detect_text_rules"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_quota_stops"
        status: pass
    human_judgment: false
  - id: D6
    description: "a dsh run that exits 0 is never quota, whatever its text says (ends dsh:empty)"
    requirement: ENG-02
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_quota_text_exit0_not_quota"
        status: pass
    human_judgment: false
  - id: D7
    description: "dsh's real failure output (stderr text and exit codes) matches the existing phrase rules (assumption A25)"
    verification: []
    human_judgment: true
    rationale: "dsh's real failure output was never captured; the raw lines stay in events for recalibration after the first real stop"

duration: 25min
completed: 2026-10-28
status: complete
---

# Phase 6 Plan 25: dsh outcomes (dsh:empty) and text-only quota stop Summary

**A dsh Dev run that changed nothing now stops as `dsh:empty` in the CLI (exit 4), the autopilot (`vướng mắc` with the dsh message) and the task screen, the agy/dsh diff checks key on the engine kind instead of the id `agy`, and a dsh run that exits non-zero with a limit phrase in its stderr or stdout stops as `quota` (exit 5) without switching engine.**

## Performance

- **Duration:** ~25 min after a warm read of the plan (cargo builds included)
- **Tasks:** 2 (1 tracer, 1 auto; both `tdd="true"`)
- **Files modified:** 7 (none created)

## Accomplishments

- `roles/dev.rs`: `is_agy` (name equality with `ENGINE_AGY`) replaced by `ctx.cfg.engines.kind_of(&run.engine)`. `Agy` gives the `.planning/` and `report::sensitive_path` violations and `agy:empty` with `AGY_EMPTY_MESSAGE`; `Dsh` gives the same checks and `dsh:empty` with the new `DSH_EMPTY_MESSAGE`; any other kind or unknown id gives no extra checks.
- `autopilot/decide.rs`: `dsh:empty` is in the not-ok list and has its own stop branch (`VuongMac`, detail `DSH_EMPTY_MESSAGE`); `view.rs` needed no logic change (a unit test pins that the detail line is printed); `dashboard/taskstate.rs`: `Some("dsh:empty") => ("vuong_mac", "dsh không sửa file")`.
- `quota.rs`: `detect_text`; `roles/mod.rs`: `StreamFormat::Text` branch (not killed or timed out) calls it with `Summary.result_text` and the 4 KiB stderr tail; the existing quota handling does the rest.

## Task Commits

1. **Task 1 (tracer): dsh Dev diff checks and the `dsh:empty` stop**
   - RED `7d71487` (test)
   - GREEN `154995b` (feat)
2. **Task 2: quota stop for text-only engines**
   - RED `86a268b` (test)
   - GREEN `900128d` (feat)

**Plan metadata:** the `docs(06-25)` commit that adds this file.

## TDD Gate Compliance

`test(06-25)` precedes `feat(06-25)` for both tasks.

- **RED, task 1:** `dsh_dev_outcomes` failed on exit 0 instead of 4 (the dsh run was accepted), `dsh_agy_kind_other_id_gets_agy_empty` failed on exit 0 instead of 4, `dsh_empty_stops_autopilot` failed with stop reason `run_loi` instead of `vuong_mac`. The lib unit tests (`d2_dsh_empty_stops_vuong_mac`, `step_state_dsh_empty`, the view test) reference `DSH_EMPTY_MESSAGE` and so did not compile at RED (a compile failure, not a behaviour failure).
- **RED, task 2:** `dsh_quota_stops` failed on exit 1 instead of 5. `dsh_quota_text_exit0_not_quota` already passed at RED: it is a boundary guard of behaviour that must not change, not a new behaviour. `detect_text_rules` did not compile at RED.
- Evidence is cargo's plain-text report (`gsd_run check tdd-red-evidence` does not support it), assessed by inspection.

## Files Created/Modified

- `agent-brain/src/roles/dev.rs` - `DSH_EMPTY_MESSAGE`, kind-based checks
- `agent-brain/src/autopilot/decide.rs` - `dsh:empty` not ok, stop branch, unit test `d2_dsh_empty_stops_vuong_mac`
- `agent-brain/src/autopilot/view.rs` - unit test only (dsh detail line)
- `agent-brain/src/dashboard/taskstate.rs` - `dsh:empty` arm, unit test `step_state_dsh_empty`
- `agent-brain/src/quota.rs` - `detect_text`, unit test `detect_text_rules`
- `agent-brain/src/roles/mod.rs` - Text branch of the quota chain
- `agent-brain/tests/engine_dsh.rs` - `setup_plan` (setup generalised to the Dev engine and the plan files), `dsh_dev_outcomes`, `dsh_agy_kind_other_id_gets_agy_empty`, `dsh_empty_stops_autopilot`, `dsh_quota_stops`, `dsh_quota_text_exit0_not_quota`

## Decisions Made

See `key-decisions` in the frontmatter.

## Deviations from Plan

### Plan scope notes (no Rule 1-4 deviation)

**1. [Plan wording] `dsh_dev_outcomes` has a fourth block**
- Besides (a), (b), (c) of the plan, block (d) checks that a secret-looking path (`.env`, listed in the plan) is a protocol violation for dsh, the second half of the extra checks of D-35. Test-only.

**2. [Plan wording] the `agy2` regression is a separate integration test in `engine_dsh.rs`**
- `dsh_agy_kind_other_id_gets_agy_empty` uses an `agy2` entry of `kind: agy` with the fake agy; the existing `engine_agy` suite is untouched (13 tests pass).

**3. [Test shape] one Sandbox per scenario block in `dsh_dev_outcomes`**
- The plan lists different plan files for (b) and (d), and a violation stop leaves the working tree dirty, so each block builds its own sandbox.

**Total deviations:** 0 against the contract; 3 plan notes. **Impact:** none.

## Issues Encountered

- No real docker, container or model endpoint was used: every cargo run had a trap `docker` first on `PATH` (it logs and exits 97); its log stayed empty through the full suite. The tests themselves use the fake docker and fake dsh.
- The `contract_guard::guard_contract_cases` BrokenPipe flake did not occur; the full suite passed on the first run.
- `cargo fmt --check` is clean for the whole crate (the pre-existing `e09_fixtures.rs` diff of 06-23 was already fixed by `32c120b`).

## Verification Results

| Command | Result |
|---|---|
| `cargo test ... --test engine_dsh --test engine_agy -- --test-threads=1` | engine_agy 13 passed; engine_dsh 12 passed (after Task 1; `dsh_dev_outcomes ... ok`, `dsh_empty_stops_autopilot ... ok`) |
| `cargo test ... --lib -- autopilot:: dashboard::taskstate --test-threads=1` | 86 passed (`d2_dsh_empty_stops_vuong_mac ... ok`, `step_state_dsh_empty ... ok`) |
| `cargo test ... --test engine_dsh --test engine_quota -- --test-threads=1` | engine_dsh 14 passed (`dsh_quota_stops ... ok`, `dsh_quota_text_exit0_not_quota ... ok`), engine_quota 9 passed |
| `cargo test ... -- --test-threads=1` (full suite, after the last code commit) | 63 result lines, all `ok`; 1182 tests passed, 0 failed |
| `cargo fmt --manifest-path agent-brain/Cargo.toml --check` | exit 0 |
| `git diff --exit-code 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock` | exit 0 |
| `bash spikes/lib/scan-secrets.sh` | `scan-secrets: OK` |
| acceptance greps | `pub const DSH_EMPTY_MESSAGE` one line; `kind_of` in dev.rs 1; `dsh:empty` in decide.rs 3 and taskstate.rs 2; `awk '/ENGINE_AGY/ ...' dev.rs` exit 0; `pub fn detect_text` one line; `detect_text` in roles/mod.rs 1 |

## Known Stubs

None.

## Known limits (stated, not hidden)

- A25: dsh's real stderr and exit codes on failure have never been captured; the phrase rules are the shared `quota::text_matches`. A real quota stop whose text does not match shows as an ordinary failed run; the raw lines are in `events` and `stderr.txt` for recalibration.
- A24 not adopted: no gateway budget phrases in `text_matches`.
- S6: a real dsh Dev that only answers in text without editing ends as `dsh:empty`, as expected.

## Threat Flags

None beyond the plan's register. Mitigations: T-06-94 (`dsh_dev_outcomes`, `dsh_empty_stops_autopilot`, `dsh_agy_kind_other_id_gets_agy_empty`), T-06-95 (`detect_text_rules`, `dsh_quota_stops`, `dsh_quota_text_exit0_not_quota`; a stop never switches engine), T-06-SC (no dependency added, Cargo files unchanged since 574b6cd, scan-secrets clean).

## User Setup Required

None.

## Next Phase Readiness

- dsh outcomes and quota are complete; 06-24 (not merged here) and 06-26 do not depend on this plan's code.
- No blockers.

## Self-Check: PASSED

- Files present: the seven modified files and this SUMMARY.
- Commits in history (`32c120b..HEAD`): `7d71487`, `154995b`, `86a268b`, `900128d`.
- Verify commands re-run after the last code commit (`900128d`): full suite all ok, fmt clean, Cargo files identical to `574b6cd`, `scan-secrets: OK`.
- `.planning/STATE.md`, `ROADMAP.md`, `~/.claude-orca`, untracked `.planning/` files untouched; no `pkill`/`killall`; no key value printed or written.
