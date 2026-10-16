---
phase: 03-rust-cli-core
plan: 05
subsystem: routing-profile
tags: [rust, routing, profile, projects-map, contract-tests]

requires:
  - phase: 03-rust-cli-core
    provides: "03-03 routing/projects/profile/config modules; 03-04 CLI entry points and Sandbox"
provides:
  - "`agent-brain projects scan` / `projects add`: Phase 2 scan semantics (depth one, hand edits kept, missing/restored, id collisions suffixed, remote credentials stripped, unknown keys preserved, duplicate alias warnings), atomic map save (tmp + fsync + rename)"
  - "Contract tests on the shared fixtures: contract_scan (15), contract_routing (5), contract_profile (3)"
  - "routing::normalize drops exactly Unicode Mn (Python parity); profile::build_env normalises the tilde config dir like os.path.normpath"
affects: [03-10, 03-11, 03-13]

actuals:
  tasks: 2
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Parity proven by the shared tests/contract/*.json fixtures that Phase 2's Python suite also reads"

key-files:
  created:
    - agent-brain/tests/contract_scan.rs
    - agent-brain/tests/contract_routing.rs
    - agent-brain/tests/contract_profile.rs
  modified:
    - agent-brain/src/projects.rs
    - agent-brain/src/routing.rs
    - agent-brain/src/profile.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, gemini-3.8-flash-high); reviewed by Claude"
  - "The two 03-03 review findings were handed to this plan (owner of routing.rs/profile.rs) and fixed test-first: normalize_unicode_mn_only, build_env_tilde_normpath"

patterns-established:
  - "Exact Unicode category checks use the pinned regex crate (\\p{Mn}) instead of approximations"

requirements-completed: [CLI-02, PROF-02]

duration: 15min
completed: 2026-10-16
status: complete
---

# Phase 3 Plan 05: Routing, Profile and Project Catalogue Parity Summary

**`agent-brain projects scan/add` now match Phase 2's map semantics, and routing, profile env and config validation pass the same shared fixtures as the Python reference; the two parity gaps found in 03-03 are fixed.**

## Performance

- **Duration:** 15 min (agy run 908 s)
- **Completed:** 2026-10-16
- **Tasks:** 2
- **Files:** 6

## Task Commits

1. `2eef661` test(03-05): add failing scan contract tests
2. `a64e4d3` feat(03-05): port projects scan and add
3. `2ac41db` test(03-05): add routing and profile contract tests
4. `a67b6eb` feat(03-05): routing and profile parity

## Deviations from Plan

Two additions at the reviewer's request (03-03 review findings), both in this plan's files:

1. `routing::normalize` drops only category Mn, via `\p{Mn}` from the pinned regex crate after NFD (was `is_combining_mark`, which also dropped Mc/Me). Test `normalize_unicode_mn_only`.
2. `profile::build_env` normalises `HOME + ~dir` with a pure-string `posix_normpath` (trailing slashes, `.`, `..`), as Python's `os.path.normpath`. Test `build_env_tilde_normpath`.

## Review (Claude)

Re-run by the reviewer on a `git archive` snapshot of `a67b6eb` (separate target dir, so the next agy run was not disturbed):

- `cargo test --offline --locked`: all suites pass twice (5 unit, 11 cli_e2e, 3 contract_profile, 5 contract_routing, 15 contract_scan, 13 lib_foundation). Acceptance: contract_scan ok count 15 (≥ 15); contract_routing named tests 4/4; contract_profile named tests 2/2; `cargo fmt --check` clean.
- Greps on `a67b6eb`: `pub fn find_duplicates(` 1, `sync_all` 1, `agent-brain projects scan --root` 2.
- Python: full suite OK and `tests.test_projects` OK in the repo (the snapshot lacks `spikes/`, so the stream-contract cases can only run in the repo).
- Code read: both fixes match the Python reference; `save_map` writes a temp file in the map's directory, `sync_all`, then renames, removing the temp file on failure.
- Scope: exactly the plan's six files; `brain/` and the shared fixtures untouched.

## Issues Encountered

None.

## Next Phase Readiness

03-10 and 03-11 use these routing and profile functions through the binary; 03-13's live P2-03 check uses `projects scan` on the real map.
