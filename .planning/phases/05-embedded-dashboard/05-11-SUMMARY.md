---
phase: 05-embedded-dashboard
plan: 11
subsystem: dashboard-ui
tags: [alpine-csp, css-chart, stats, projects, offline-assets, dash-09]

requires:
  - phase: 05-embedded-dashboard
    provides: "05-08 shell + AB helpers, 05-06 GET /api/stats, 05-07 projects API (list, suggest, PUT aliases with etag, scan, status)"
provides:
  - "screens/stats.html + js/stats.js: Thống kê — range select (7 ngày / 30 ngày / Tất cả), 5 total cards (unknown cost shows —), Theo ngày CSS bar chart (object :style heights, labels thinned above 14 days, Cao nhất caption, aria-label per column), Theo profile / dự án (10 + Hiện tất cả) / vai trò bars sorted by tokens, empty state replacing the body"
  - "screens/projects.html + js/projects.js: Dự án — project cards (profile badge, thiếu thư mục badge, id/path monospace, aliases or Chưa có alias), alias edit with Lưu alias / Hủy sửa alias (etag PUT, server 409 text shown verbatim, duplicate warnings), Quét lại (child scan output block), Trạng thái git expander; no control for profile/id/path/type"
  - "tests/dashboard_assets.rs: DASH-09 static checks — asset table matches the tree, no external loads in embedded assets, app.css is the prebuilt Tailwind v4.3.3 output, vendored Alpine is the pinned build, assets served only after the handshake"
affects: [05-12, 05-13]

actuals:
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Charts are plain CSS bars driven by object :style methods (CSP style-src 'self' compatible); no chart library"

key-files:
  created:
    - agent-brain/assets/dashboard/screens/stats.html
    - agent-brain/assets/dashboard/js/stats.js
    - agent-brain/assets/dashboard/screens/projects.html
    - agent-brain/assets/dashboard/js/projects.js
    - agent-brain/tests/dashboard_assets.rs
  modified:
    - agent-brain/src/dashboard/assets.rs
    - agent-brain/assets/dashboard/app.css

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, --dangerously-skip-permissions approved for Phase 5); reviewed by Claude"
  - "Reviewer note R2 applied: the alias-save conflict shows the server's error text verbatim; the UI-SPEC copy row for it is superseded"

patterns-established:
  - "DASH-09 is guarded by a Rust test over the embedded asset table, not only by a manual offline check"

requirements-completed: [DASH-09, DASH-11, DASH-12]

duration: 20min
completed: 2026-10-17
status: complete
---

# Phase 5 Plan 11: Thống kê and Dự án Screens Summary

**Thống kê and Dự án screens, plus a DASH-09 test:**
- **Thống kê:** token totals and CSS bar breakdowns by day, profile, project and role, filtered by time range and profile.
- **Dự án:** lists the projects from `projects_map.json` with alias editing, rescan and git status, and never offers a profile control.
- **DASH-09 test:** a Rust test proves the embedded front end loads nothing from outside the binary.

## Performance

- **Duration:** about 20 min (agy run 671 s, plus review)
- **Completed:** 2026-10-17
- **Tasks:** 3
- **Files:** 7

## Accomplishments

- **Stats chart:** bars use object-form `:style` (`colStyle(d)`, `barStyle(row)`). A grep guard forbids string styles. Day labels are thinned to every fifth plus the first and last above 14 days, and every column carries an `aria-label`. Cost uses `AB.costKnown`, so a run with no result shows `—`, not `≈ $0.0000`.
- **Projects screen:** shows the 409 conflict text exactly as the server sends it (R2), with no UI wording of its own. The only PUT body is `{ aliases, etag: this.etag }`, so no profile is ever sent. The rescan output block shows the CLI's stdout and stderr lines with its exit code.
- **Asset test:** `dashboard_assets.rs` has 6 tests:
  - the `ASSETS` table matches the files under `assets/dashboard`;
  - no `http(s)://`, `//cdn`, `@import url` or font URL appears in any embedded asset;
  - `app.css` starts with the Tailwind v4.3.3 banner and `build-css.sh --check` holds;
  - the Alpine file's sha256 is the pinned one;
  - assets answer 401 before the handshake and 200 after it.

## Task Commits

1. `fca04fb` feat(05-11): Thống kê screen (agy)
2. `ee0f962` feat(05-11): Dự án screen (agy)
3. `0480362` test(05-11): DASH-09 offline asset checks (agy)

## Deviations from Plan

None.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- **Scope:** the 3 commits touch exactly the 7 declared files.
- **Verify commands:** all 8 `<automated>` commands, run verbatim from the plan, exit 0:
  - the smoke tests for stats, projects and all five screens;
  - the UI lint ("40 thẻ tương tác") with the banned-class grep;
  - the stats style and projects body gates;
  - `--test dashboard_assets` with 6 passed;
  - `build-css.sh --check`.
- **Acceptance:** all 15 command criteria pass. 13 ran under the acceptance checker; the reviewer ran the two style guards by hand, and both print 0.
- **Visual:** headless screenshots of `#/stats` (light) and `#/projects` (dark) match the screen contracts. The single-day column is limited to `max-w-16`, `Chi phí ≈` shows `—`, the alias chip and buttons are present, and dark-mode text is readable.
- **Full suite:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --no-fail-fast -- --test-threads=1` exits 0 with 243 passed and 0 failed (237 + 6 asset tests). The Python suite is OK and `cargo fmt --check` exits 0.
- **R2 check:** projects.js does not contain the superseded UI-SPEC wording ("vừa bị sửa ở nơi khác"), count 0.
- **Secrets:** the pattern scan is clean.

**Left for 05-13 (live, by the user):** DASH-09 with the network actually off (P4-09), the day-label thinning at 30 days with real data, and the backstop UI rows.

## Next Phase Readiness

All five screens exist. 05-12 (release smoke of `serve` from the static binary in an empty environment, README, live-check steps) can start.
