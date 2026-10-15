---
phase: 01
review: 01-REVIEW.md
titles: json
findings:
  - id: CR-01
    severity: critical
    disposition: open
    title: "scrub.py truncates a file to zero bytes on an encode error, and aborts the rest of the run"
  - id: WR-01
    severity: warning
    disposition: open
    title: "username leaks into committed fixtures through forms that neither scrub.py nor scan-secrets.sh recognise"
  - id: WR-02
    severity: warning
    disposition: open
    title: "scrub.py secret coverage is a \"32+ character blob\" heuristic and misses common secret shapes"
  - id: WR-03
    severity: warning
    disposition: open
    title: "the `sk-` rule has no left word boundary, so it mangles ordinary text and false-positives the gate"
  - id: WR-04
    severity: warning
    disposition: open
    title: "scan-secrets.sh fails open on grep errors and on a missing container path"
  - id: WR-05
    severity: warning
    disposition: open
    title: "env-name enumeration via `env | cut -d= -f1` breaks on multi-line values, and the container variant uses `eval`"
  - id: WR-06
    severity: warning
    disposition: open
    title: "`s5 clone` overwrites the `~/.claude-orca` baseline before it checks whether it is allowed to run"
  - id: WR-07
    severity: warning
    disposition: open
    title: "S4 can abort before scrubbing and leave raw stream-json in the tracked tree; `set -e` also makes signal races fatal"
  - id: WR-08
    severity: warning
    disposition: open
    title: "`CLEAN_ENV` is a fixed unset list, not \"all CLAUDE_CODE_*\", and the profile guard only compares exact paths"
  - id: WR-09
    severity: warning
    disposition: open
    title: "S6 kill harness records \"leftover: none\" without checking, and signals a PID it has not re-verified"
  - id: WR-10
    severity: warning
    disposition: open
    title: "`s5/verify.sh` silently downgrades to the weaker partial checks"
  - id: IN-01
    severity: info
    disposition: open
    title: "the schedule PID file is a tracked runtime artifact"
  - id: IN-02
    severity: info
    disposition: open
    title: "S7 appends `exit_code=N` to `.json` and `.jsonl` fixtures"
  - id: IN-03
    severity: info
    disposition: open
    title: "S3 reports \"prefix too small\" when claude itself failed, and its `rm -rf` guard is a no-op against a user-set root"
  - id: IN-04
    severity: info
    disposition: open
    title: "the S5 clone is built from a denylist with different semantics in the rsync and tar branches"
  - id: IN-05
    severity: info
    disposition: open
    title: "`spike-results.md` passes `check-results.sh --final` only because pending labels were removed by hand, and `cleanup_scr` is suffix-guarded only"
open: 16
total: 16
recorded: 2026-10-15T03:24:02.536Z
---

# Phase 01: Code Review Disposition

| Finding | Severity | Disposition | Source |
|---------|----------|-------------|--------|
| CR-01 | critical | open | - |
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
| IN-01 | info | open | - |
| IN-02 | info | open | - |
| IN-03 | info | open | - |
| IN-04 | info | open | - |
| IN-05 | info | open | - |

Dispositions: `open` (recorded, not yet triaged), `fixed`, `skipped`, `deferred`.
Set `deferred` by hand and put the reason in the Source cell; both are preserved. A `|` in the reason is kept as prose and escaped on the next run.
Re-running the gate keeps every row it can. A row the current review no longer reports is kept and its Source cell flagged, so a finding does not leave this record silently. ONE exception: when a finding id is REUSED by a different finding, the earlier decision cannot keep a row — the id is taken — and it is dropped. A RECORDED decision (anything but `open`) is named on the console when that happens; a row still at `open` is replaced silently, because `open` records no decision to lose.
