---
phase: 05-embedded-dashboard
plan: 07
subsystem: dashboard
tags: [rust, axum, projects, routing, etag, autocomplete]

requires:
  - phase: 05-embedded-dashboard
    provides: "05-04 empty api_projects routes, AppState, blocking/json_error"
provides:
  - "routing::{SuggestRank, Suggestion, suggest}: ranked, accent/case-insensitive project suggestions for the autocomplete (D-05)"
  - "GET /api/projects?profile= (projects, etag, scan_roots, duplicate warnings), GET /api/projects/suggest?q=&profile= (≤ 10 ranked, missing flagged)"
  - "PUT /api/projects/{id} {aliases, etag}: etag check under a write lock (409 + fresh list), alias cleanup, only `aliases` changed, projects::save_map, SQLite mirror, duplicate warnings without refusal"
  - "POST /api/projects/scan: child `agent-brain projects scan`, exit code + stdout/stderr lines, then SQLite mirror"
  - "GET /api/projects/{id}/status: branch, clean, status --short, log -5, diff --stat; zero tokens, no run row"
  - "api_projects::map_etag"
affects: [05-09, 05-11, 05-13]

actuals:
  tasks: 2
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Map edits: re-read → etag compare → change one key → atomic save_map (0600) → db::sync_projects"

key-files:
  created:
    - agent-brain/tests/routing_suggest.rs
    - agent-brain/tests/dashboard_projects.rs
  modified:
    - agent-brain/src/routing.rs
    - agent-brain/src/dashboard/api_projects.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, --dangerously-skip-permissions approved for Phase 5); reviewed by Claude"

patterns-established:
  - "The dashboard never edits a project's profile, path, id or type; PUT bodies use deny_unknown_fields"

requirements-completed: [DASH-12]

duration: 25min
completed: 2026-10-17
status: complete
---

# Phase 5 Plan 07: Projects API Summary

**The Projects API (aliases, rescan) and the autocomplete suggestions are in place:**
- **Suggestions:** ranked, accent-insensitive matches for the autocomplete, with missing projects flagged.
- **Project list:** comes from `projects_map.json`, with an etag and duplicate warnings.
- **Alias editing:** never overwrites a hand edit made since the page loaded, and changes nothing but `aliases`.
- **Rescan:** runs the CLI's own `projects scan`.
- **Git status:** a zero-token read.

## Performance

- **Duration:** about 25 min (agy run 929 s, plus review)
- **Completed:** 2026-10-17
- **Tasks:** 2 (both TDD: tests committed first)
- **Files:** 4

## Accomplishments

- **Suggestions:** `routing::suggest` ranks by match quality over `normalize`d id, Vietnamese name and aliases. The existing `resolve` and `normalize` are unchanged (the routing.rs diff only adds lines).
- **Alias editing:** `update_aliases` takes a process-wide write mutex and compares the etag (a SipHash of the file bytes plus its length). On mismatch it returns 409 with the fresh list, scan_roots and duplicates. Otherwise it reloads the map, cleans the aliases (trim, drop empty, cap at 100 chars, de-duplicate by `normalize`), replaces only `projects[i].aliases`, saves through `projects::save_map`, and mirrors to SQLite. Duplicate names or aliases are warned about, never refused, as `projects scan` does.
- **Status:** `project_status` uses `gitops::git` argument vectors only. It creates no run and adopts no task.

## Task Commits

1. `9762bf8` test(05-07): add tests for project suggestions, list and status endpoints (agy)
2. `3ec5ffe` feat(05-07): project suggestions, list and status endpoints (agy)
3. `adfcf73` test(05-07): add tests for alias editing and rescan (agy)
4. `e9f53e0` feat(05-07): alias editing and rescan from the dashboard (agy)

## Deviations from Plan

None.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- **Scope:** the 4 commits touch exactly the 4 declared files. `routing.rs` has no removed lines.
- **Real map untouched:** `~/.config/agent-brain/projects_map.json` still has its 2026-10-16 mtime, so the tests used the sandbox map only.
- **Tests:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --no-fail-fast -- --test-threads=1` exits 0 with 236 passed and 0 failed (223 before plus 13 new). These include `dash12_alias_edit_preserves_other_keys`, `dash12_alias_edit_conflict_409`, `dash12_duplicate_alias_warns_not_refuses`, `dash12_rescan_runs_cli_scan`, `dash12_rescan_without_roots_shows_cli_error`, `d15_profile_not_editable`, `projects_list_and_profile_filter`, `suggest_accent_and_case` and `suggest_empty_query_lists_all`. The existing `contract_scan` and `contract_routing` tests still pass.
- **Other checks:** the Python suite is OK, `cargo fmt --check` exits 0, and no processes were left behind.
- **Secrets:** the pattern scan is clean.

**Non-blocking notes:**

- **Rescan locking and timeout:** rescan runs the CLI child without the alias write lock and without a timeout. A rescan that overlaps an alias save could race on `projects_map.json` within milliseconds (the plan accepts that window), and a hanging scan would hold its HTTP request open.
- **Conflict wording (for the UI-plan revision):** the 409 text, "projects_map.json vừa thay đổi ở nơi khác — đã tải lại danh sách, hãy sửa lại", differs slightly from the UI-SPEC copy, "projects_map.json vừa bị sửa ở nơi khác — đã tải lại, hãy sửa lại". Pick one when 05-11 is revised.

## Next Phase Readiness

Wave 5 is complete, so the whole server-side API for the five screens exists. Per the user's choice, execution pauses here so the UI plans 05-08..05-11 can be revised against the approved UI-SPEC before waves 6–11.
