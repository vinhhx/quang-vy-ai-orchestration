---
phase: 05
review: 05-REVIEW.md
titles: json
findings:
  - id: CR-01
    severity: critical
    disposition: deferred
    title: "The session cookie is the only credential and is not port-isolated, so any other local service can steal it and drive agents"
  - id: WR-01
    severity: warning
    disposition: open
    title: "`stats::parse_since` multiplies and subtracts without overflow checks"
  - id: WR-02
    severity: warning
    disposition: open
    title: "README API catalogue lists routes that do not exist and omits real ones (confirmed known item)"
  - id: WR-03
    severity: warning
    disposition: open
    title: "Confirmation dialog does not return focus to its trigger on every close path (confirmed known item)"
  - id: WR-04
    severity: warning
    disposition: open
    title: "Combobox acts on stale suggestions inside the 200 ms debounce, and a late response silently resets keyboard navigation (confirmed known item)"
  - id: WR-05
    severity: warning
    disposition: open
    title: "The \"at most one personal-profile run\" invariant (E-06) is checked and then reserved non-atomically"
  - id: WR-06
    severity: warning
    disposition: open
    title: "Running dashboard jobs are matched to a task by branch name only, ignoring the project"
  - id: WR-07
    severity: warning
    disposition: open
    title: "`POST /api/projects/scan` is not serialised with alias writes, has no timeout, and is not tied to the server lifetime"
  - id: WR-08
    severity: warning
    disposition: open
    title: "Job output reader threads stop at the first non-UTF-8 line, leaving the child writing to a closed pipe"
  - id: WR-09
    severity: warning
    disposition: open
    title: "Router navigation has no ordering guard, so a slow earlier navigation can overwrite the screen of a later one"
  - id: WR-10
    severity: warning
    disposition: open
    title: "Raw-JSON dialog response is applied without checking that the dialog is still the same one"
  - id: WR-11
    severity: warning
    disposition: open
    title: "`POST /api/runs/{id}/kill` validates run X but `cmd_kill` kills whichever run is currently running for the task"
  - id: WR-12
    severity: warning
    disposition: open
    title: "~100 lines of task-state derivation and two error-mapping blocks are duplicated, so the enabled gate can drift from what the UI shows"
  - id: WR-13
    severity: warning
    disposition: open
    title: "500 responses echo raw internal error strings (SQLite, I/O, git, absolute paths) to the client"
  - id: IN-01
    severity: info
    disposition: open
    title: "Dead and duplicate branches in `taskstate::derive`"
  - id: IN-02
    severity: info
    disposition: open
    title: "`x-for` loops use non-unique `:key` values"
  - id: IN-03
    severity: info
    disposition: open
    title: "Pump logs on every failed tick and re-reads the baseline after any error"
  - id: IN-04
    severity: info
    disposition: open
    title: "`Mutex::lock().unwrap()` makes one panic permanent"
  - id: IN-05
    severity: info
    disposition: open
    title: "`cfg.kill_grace_secs + 5` is unchecked"
  - id: IN-06
    severity: info
    disposition: open
    title: "Git commands take a branch or commit as a bare argument and the ref can be ambiguous with a path"
  - id: IN-07
    severity: info
    disposition: open
    title: "Static route names shadow project ids"
  - id: IN-08
    severity: info
    disposition: open
    title: "Smoke and key-check script robustness"
  - id: IN-09
    severity: info
    disposition: open
    title: "Reservation leaks if the spawner thread dies"
  - id: IN-10
    severity: info
    disposition: open
    title: "Dead `cfg(not(unix))` branches and an unused field in `lock.rs`"
  - id: IN-11
    severity: info
    disposition: open
    title: "`GET /api/jobs/{id}` reports `run_id` as the task's latest run, which can be a previous run, and nothing consumes it"
  - id: IN-12
    severity: info
    disposition: open
    title: "Test `d10_config_port_used_without_flag` has a port TOCTOU"
open: 26
total: 26
recorded: 2026-10-19T04:45:54.888Z
---

# Phase 05: Code Review Disposition

| Finding | Severity | Disposition | Source |
|---------|----------|-------------|--------|
| CR-01 | critical | deferred | Accepted risk AR-5-01 / T-5-11 (05-SECURITY.md, user-approved 05-02): cookies are not port-isolated (RFC 6265 §8.5); a hostile same-user local process already has the user's privileges. Hardening (token in a header, not a cookie) is a follow-up candidate |
| WR-01 | warning | open | - |
| WR-02 | warning | open | - |
| WR-03 | warning | open | - |
| WR-04 | warning | open | - |
| WR-05 | warning | open | - |
| WR-06 | warning | open | - |
| WR-07 | warning | open | - |
| WR-08 | warning | open | - |
| WR-09 | warning | open | - |
| WR-10 | warning | open | - |
| WR-11 | warning | open | - |
| WR-12 | warning | open | - |
| WR-13 | warning | open | - |
| IN-01 | info | open | - |
| IN-02 | info | open | - |
| IN-03 | info | open | - |
| IN-04 | info | open | - |
| IN-05 | info | open | - |
| IN-06 | info | open | - |
| IN-07 | info | open | - |
| IN-08 | info | open | - |
| IN-09 | info | open | - |
| IN-10 | info | open | - |
| IN-11 | info | open | - |
| IN-12 | info | open | - |

Dispositions: `open` (recorded, not yet triaged), `fixed`, `skipped`, `deferred`.
Set `deferred` by hand and put the reason in the Source cell; both are preserved. A `|` in the reason is kept as prose and escaped on the next run.
Re-running the gate keeps every row it can. A row the current review no longer reports is kept and its Source cell flagged, so a finding does not leave this record silently. ONE exception: when a finding id is REUSED by a different finding, the earlier decision cannot keep a row — the id is taken — and it is dropped. A RECORDED decision (anything but `open`) is named on the console when that happens; a row still at `open` is replaced silently, because `open` records no decision to lose.
