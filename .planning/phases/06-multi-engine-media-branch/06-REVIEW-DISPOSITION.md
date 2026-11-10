---
phase: 06
review: 06-REVIEW.md
titles: json
findings:
  - id: CR-01
    severity: critical
    disposition: open
    title: "`human done` fast-forwards git before the DB commit, and a retry can never finish"
  - id: WR-01
    severity: warning
    disposition: open
    title: "Supervisor busy-loops when the child closes stdout before it exits"
  - id: WR-02
    severity: warning
    disposition: open
    title: "Quota detector matches generic words in model output, so ordinary failures become \"quota\" stops"
  - id: WR-03
    severity: warning
    disposition: open
    title: "Department pick that finds no usable member falls back to the team's engine (claude) with no message"
  - id: WR-04
    severity: warning
    disposition: open
    title: "An engine in `chua_kiem` state runs through a team but is refused for pins, department members and quota re-runs"
  - id: WR-05
    severity: warning
    disposition: open
    title: "`parse_since` multiplication overflows on large input"
  - id: WR-06
    severity: warning
    disposition: open
    title: "Settings writes lose concurrency protection and can replace a symlinked `config.json`"
  - id: IN-01
    severity: info
    disposition: open
    title: "`kill` can miss the container-side stop for a dsh run that was only just spawned"
  - id: IN-02
    severity: info
    disposition: open
    title: "`warn_leftover` matches any token that starts with `ab-dsh-`"
  - id: IN-03
    severity: info
    disposition: open
    title: "`key_env` is not checked against critical variable names"
  - id: IN-04
    severity: info
    disposition: open
    title: "Engine readiness checks the process env, but a run reads the profile-filtered env"
  - id: IN-05
    severity: info
    disposition: open
    title: "Orchestrator Settings save writes stale \"live\" values over on-disk edits"
  - id: IN-06
    severity: info
    disposition: open
    title: "Spike script and test fake allow riskier behaviour than production code"
open: 13
total: 13
recorded: 2026-11-10T02:54:59.804Z
---

# Phase 06: Code Review Disposition

| Finding | Severity | Disposition | Source |
|---------|----------|-------------|--------|
| CR-01 | critical | open | - |
| WR-01 | warning | open | - |
| WR-02 | warning | open | - |
| WR-03 | warning | open | - |
| WR-04 | warning | open | - |
| WR-05 | warning | open | - |
| WR-06 | warning | open | - |
| IN-01 | info | open | - |
| IN-02 | info | open | - |
| IN-03 | info | open | - |
| IN-04 | info | open | - |
| IN-05 | info | open | - |
| IN-06 | info | open | - |

Dispositions: `open` (recorded, not yet triaged), `fixed`, `skipped`, `deferred`.
Set `deferred` by hand and put the reason in the Source cell; both are preserved. A `|` in the reason is kept as prose and escaped on the next run.
Re-running the gate keeps every row it can. A row the current review no longer reports is kept and its Source cell flagged, so a finding does not leave this record silently. ONE exception: when a finding id is REUSED by a different finding, the earlier decision cannot keep a row — the id is taken — and it is dropped. A RECORDED decision (anything but `open`) is named on the console when that happens; a row still at `open` is replaced silently, because `open` records no decision to lose.
