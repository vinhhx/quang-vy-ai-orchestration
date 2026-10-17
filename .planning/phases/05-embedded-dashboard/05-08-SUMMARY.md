---
phase: 05-embedded-dashboard
plan: 08
subsystem: dashboard-ui
tags: [alpine-csp, tailwind, html, hash-router, headless-chrome, ui-spec]

requires:
  - phase: 05-embedded-dashboard
    provides: "05-02 assets table + CSP gate, 05-05 GET /api/tasks list, 05-01 Tailwind build and vendored Alpine CSP"
provides:
  - "index.html: UI-SPEC shell (lang vi, scheme-light/dark:scheme-dark, color-scheme meta, Body/Header/Nav recipes, profile filter, theme button, session-lost and connection-lost banners, <main id=screen tabindex=-1>)"
  - "js/theme.js: window.ABTheme {mode, set, apply} (system/dark/light, ab.theme)"
  - "js/app.js: window.AB {route, profile, setProfile, profileQuery, api, every, onLeave, onReady, fmtNum, fmtDuration, fmtTime, fmtCost, stepClass, stepGlyph, stepGlyphClass, roleName, profileClass, filterText, tokensKnown, costKnown, errorText, lockScroll, focusLater}; Alpine shell component; hash router with lazy /js/NAME.js + /screens/NAME.html loading, document.title, focus on route change; events ab:profile, ab:net, ab:route"
  - "screens/overview.html + js/overview.js: Tổng quan (4 groups 2×2 from 1024 px, glyph+role pills, empty/loading/error/stale states, Xong 10 + Hiện thêm 10, limit=100, 4 s refresh, profile filter)"
  - "scripts/dashboard-smoke.sh [screen ...]: sandboxed serve + headless Chrome --dump-dom render check (AB_BIN override)"
  - "scripts/dashboard-ui-lint.sh: lang/color-scheme/main checks and FOCUS outline on every interactive tag of index.html and screens/*.html"
affects: [05-09, 05-10, 05-11, 05-12, 05-13]

actuals:
  tasks: 2
  commits: 2

tech-stack:
  added: []
  patterns:
    - "One screen = one HTML fragment + one JS file, loaded on first visit; index.html never changes for a new screen"
    - "Alpine CSP rules: logic in Alpine.data getters/methods, x-text only, :class full strings, object :style"

key-files:
  created:
    - agent-brain/assets/dashboard/js/theme.js
    - agent-brain/assets/dashboard/js/app.js
    - agent-brain/assets/dashboard/js/overview.js
    - agent-brain/assets/dashboard/screens/overview.html
    - scripts/dashboard-smoke.sh
    - scripts/dashboard-ui-lint.sh
  modified:
    - agent-brain/assets/dashboard/index.html
    - agent-brain/src/dashboard/assets.rs
    - agent-brain/assets/dashboard/app.css

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, --dangerously-skip-permissions approved for Phase 5); reviewed by Claude"
  - "Plan revised against 05-UI-SPEC.md before execution (commits 7a5dc65, 45212a4); UI-SPEC deltas 1-14 and 28 are owned here"

patterns-established:
  - "Every later UI plan runs the banned-class grep, dashboard-ui-lint.sh and dashboard-smoke.sh for its screen"

requirements-completed: []

duration: 25min
completed: 2026-10-17
status: complete
---

# Phase 5 Plan 08: Dashboard Shell and Tổng quan Summary

**The dashboard now opens on the Vietnamese UI-SPEC shell:**
- **Navigation and settings:** a hash router, a profile filter, a system/dark/light theme toggle, and session-lost and connection-lost banners.
- **Tổng quan:** tasks grouped Đang chạy / Chờ bạn / Lỗi / cần xử lý / Xong, two per row, with glyph + role step pills, refreshing every 4 s and following the profile filter.
- **Guard scripts:** a headless-Chrome smoke script and a UI lint guard every later screen.

## Performance

- **Duration:** about 25 min (agy run 976 s, plus review)
- **Completed:** 2026-10-17
- **Tasks:** 2
- **Files:** 9

## Accomplishments

- **Shell:** `index.html` was rewritten from scratch, and the 05-02 placeholder is gone (reviewer note R3). It has the exact `<html lang="vi" class="scheme-light dark:scheme-dark">`, the color-scheme meta, the Body recipe, `<nav aria-label="Điều hướng chính">` with `aria-current` links, the labelled profile select (with its `title` explanation), the theme button, the two `role="alert"` banners, and no inline script or style and no external URL.
- **`app.js`:** implements the full `window.AB` contract that later screens import. It includes a step-glyph table with eight entries, each written with the six-character `︎` escape.
- **Router:** clears timers and `onLeave` callbacks on every route change, sets `document.title`, and moves focus to `#screen` after a hash change.
- **Overview screen:** follows the UI-SPEC Tổng quan contract and its 11 UI Considerations truths: empty copy per group, the full and filtered empty states, `Đang tải...`, the load-error card with `Thử lại`, the stale line, title clamping and the count pills.
- **Smoke script:** builds a temp HOME, XDG dirs, a git repo, a project map and a config, and runs a fake Lead so task 1 is planned. It then starts `serve --port 0`, renders each requested screen with headless Chrome, and stops serve only by its recorded PID (SIGINT, then SIGKILL after 10 s).

## Task Commits

1. `9798c71` feat(05-08): dashboard shell, router, theme and smoke script (agy)
2. `878eb11` feat(05-08): Tổng quan screen and UI lint (agy)

## Deviations from Plan

None.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- **Scope:** the 2 commits touch exactly the 9 declared files.
- **Verify commands:**
  - `node --check` passes for theme.js, app.js and overview.js, and `bash -n` passes for both scripts.
  - The banned-class grep over `index.html`, `screens/` and `js/` prints nothing.
  - The glyph gate finds 8 `uFE0E` escapes and 0 invisible U+FE0E characters.
  - `bash scripts/build-css.sh --check` prints "app.css khớp nguồn".
  - `bash scripts/dashboard-ui-lint.sh` prints "UI lint OK: 11 thẻ tương tác".
  - `bash scripts/dashboard-smoke.sh overview` prints "Smoke OK: overview", and no serve or Chrome process remains afterwards.
- **Acceptance greps:**
  - No inline script, style or URL in index.html or overview.html.
  - The html, meta and body recipe strings each appear once, and the placeholder text count is 0.
  - `grid gap-6 lg:grid-cols-2` appears once and `xl:grid-cols-4` 0 times.
  - `:aria-label="pillLabel(t, ` appears 3 times and `hover:bg-indigo-700` 2 times.
  - No `pkill` or `killall` in the smoke script.
- **Visual check:** the reviewer took headless screenshots of Tổng quan at 1280×900 in the light theme and with `--force-dark-mode`.
  - **Layout:** a 2×2 grid, glyph + role pills (`✓ Lead`, `○ Dev`, `○ QA`), the work badge, and the `Chờ duyệt plan` banner with `Cập nhật HH:MM:SS`.
  - **Text:** Vietnamese diacritics are not clipped.
  - **Dark mode:** text is readable on the slate background, and the native select follows the theme.
- **Full suite:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --no-fail-fast -- --test-threads=1` exits 0 with 236 passed and 0 failed. That's the same count as before, since this plan adds no Rust tests. The Python suite is OK and `cargo fmt --check` exits 0.
- **Secrets:** the only pattern hits are the env var names `ANTHROPIC_API_KEY` etc. in the smoke config's `unset_env` lists. These are names with no values, the same as the Phase 3 default config.

## Next Phase Readiness

05-09 (Giao việc screen with the ARIA combobox project picker) can start. The router already knows `#/assign` and the nav link exists.
