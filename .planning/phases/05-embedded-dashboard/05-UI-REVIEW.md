---
phase: "5"
slug: "embedded-dashboard"
audited: "2026-10-19"
baseline: "05-UI-SPEC.md (approved)"
overall_score: 21
max_score: 24
screenshots: "none (code-only audit; reviewer screenshots from the live check exist in 05-13-SUMMARY evidence)"
blockers: 0
---

# Phase 5 — UI Review

> Retroactive 6-pillar audit by gsd-ui-auditor (2026-10-19), run as the verify:post `ui-review` hook of `/gsd-execute-phase 5`. Code-only: the dashboard is token-gated and embedded, and the auditor did not render it. The reviewer's live screenshots (1024 / 768 / 480 px, light and dark, plus the post-fix 480 px overview) are described in 05-08..05-13 SUMMARY review sections.

## Scores

| Pillar | Score | Key finding |
|--------|-------|-------------|
| Copywriting | 4/4 | Every spot-checked contract string is present (20 strings, including the error, empty, banner and dialog copy); load-error and stale-poll copy on all four polled screens |
| Visuals | 3/4 | Glyph and status vocabulary matches the contract; the stepper `→` connector (`text-slate-400` on white, ~2.6:1) is barely visible |
| Color | 3/4 | Every themed colour class has a `dark:` pair (the exceptions are the always-dark log pane and the connector); no hex or rgb literals; 60/30/10 split not checked visually |
| Typography | 4/4 | Exactly 4 sizes (`text-sm`, `text-base`, `text-xl`, `text-2xl`) and 2 weights (`font-normal`, `font-semibold`); every clipped element carries `leading-normal` |
| Spacing | 4/4 | No 12px or 20px steps; the only arbitrary value is the spec-declared `lg:h-[32rem]`; controls use `min-h-12` |
| Experience Design | 3/4 | ARIA combobox, `role="log"` pane, focus to `<main>` on route change, `aria-describedby` on disabled reasons, `motion-safe:` only. Open: dialog focus does not return to the trigger; combobox Enter race |
| **Overall** | **21/24** | No blockers |

## Top fixes

1. **Restore focus to the trigger when a dialog closes.** Keyboard and screen-reader users lose their place after Kill, Chạy lại Lead or the raw-JSON viewer, because focus lands on `<body>`. UI-SPEC Interaction Contracts say "returns to the trigger on close". Fix: remember `document.activeElement` on open and focus it again on close (`js/task.js` open/close paths; `AB.focusLater` in `js/app.js` is the only helper). Already recorded in 05-13-SUMMARY "Open follow-ups".
2. **Combobox Enter race.** Pressing Enter within about 200 ms of typing chooses from the previous (unfiltered) suggestion list, which breaks "only a listed option can be chosen" for a moment. The chosen project is shown before starting, and the server resolve gate prevents ambiguous starts. Fix: ignore Enter while a suggestion request is debounced or in flight, or pick only from the latest result set. Already recorded in 05-13-SUMMARY.
3. **Stepper connector contrast.** `screens/task.html:61` and `:86` use `text-slate-400` for the `aria-hidden` `→`. Use `text-slate-600 dark:text-slate-400` and amend the UI-SPEC row that prescribes slate-400, which conflicts with the contract's own 4.5:1 rule. Cosmetic (decorative glyph).

## Minor recommendations

- Re-check the Thống kê day-label thinning once there are more than 14 days of runs (UI-04 had one day of real data).
- `task.html` builds its action buttons in JS; confirm one primary per step card in a rendered pass (the live screenshots showed one primary, `Chạy Dev`, on the current step).

## Disposition

All three top fixes are non-blocking UX and accessibility items. They are carried as follow-ups in the phase VERIFICATION and are candidates for one small agy fix plan.
