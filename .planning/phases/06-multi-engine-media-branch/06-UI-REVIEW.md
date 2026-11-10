---
phase: "06"
slug: "multi-engine-media-branch"
audited: "2026-11-10"
baseline: "06-UI-SPEC.md"
overall_score: 15
max_score: 24
screenshots: none (code-only audit)
auditor: gsd-ui-auditor (written to disk by the orchestrator from the auditor's return)
---

# Phase 6 — UI Review

Retroactive 6-pillar audit of the current dashboard code (main at 737675b) against `06-UI-SPEC.md`. Code-only: no screenshots, no interaction capture. The auditor read the UI-SPEC up to line 423 of 785 (the Copywriting, Assumptions and later Delta sections were not compared) and did not read the PLAN/SUMMARY/CONTEXT files.

## Pillar Scores

| Pillar | Score | Key finding |
|---|---|---|
| 1. Copywriting | 3/4 | Phase 6 strings match the contract; departures come from later quick plans |
| 2. Visuals | 3/4 | Glyphs, engine lines, banner and callout present; confirm dialogs use `role="alertdialog"` |
| 3. Color | 3/4 | Rose limited to destructive / invalid uses; heavy but allowed indigo on Settings |
| 4. Typography | 2/4 | Contract allows 4 sizes and 2 weights; `text-xs` ×44 and `font-medium` ×8 across the four screens |
| 5. Spacing | 2/4 | Banned and off-scale classes present (`gap-3`, `p-3`, `px-3`, `py-3`, `px-2.5`, `py-0.5`, `min-h-10`) |
| 6. Experience Design | 3/4 | Broad state coverage; two small contract departures |
| **Overall** | **15/24** | |

## Top Fixes

1. **"Ghi chú prompt" section of `screens/settings.html` (~lines 561–588)** — a later quick-plan addition using banned `text-xs`, `font-medium`, `min-h-10`, `px-3 py-1.5`, `py-0.5`. Switch to the contract's `text-sm font-semibold`, `min-h-12`, `px-4 py-2` and the Badge recipe.
2. **Extend `scripts/dashboard-ui-lint.sh` to enforce the UI-SPEC banned-class list.** It does not check `text-xs`, `min-h-10`, `font-medium` or `px-3` today, which is why it is green while these findings exist.
3. **Banned classes in `assign.html` (lines 37–54: `text-xs`, `min-h-10`, `px-3`, `font-medium`) and `task.html` (lines 222–233, 343–355, 491–498, 784: `min-h-10`, `text-xs`, `mt-1`, `px-2`).** Probably older Phase 5 code (not confirmed). Fix or record as accepted; the UI-SPEC claim "banned classes unchanged and grep-checked" is false for these files.
4. (Accepted, no action) `leading-[1.2]` on ~10 card titles and the Stats `grid-cols-[8rem_1fr_auto]` / `[60vh]` values are declared exceptions in the spec; a shared title class would be tidier.

## Detailed Findings

### 1. Copywriting (3/4)
- Match the contract: `Hết quota engine …` (`task.js:2370`), `Chờ {tên} làm (lại) bước …` (`task.js:565`), `Đổi đội hình cho task …` (`task.js:937`), `Đang tải đội hình...` (`assign.html:150`), `Theo engine`, `chưa có giá`, `không có token`.
- Departures from later user-decided quick plans (not scored as defects): Settings has six sections (`Hàng đợi (Orchestrator)`, `Ghi chú prompt` added, `settings.js:5,10`) where the contract has four; the Engine section has a model editor (`settings.html:453`, commit 3e265b3 — see 06-SECURITY.md AR-06-07); the text `QA phải dùng engine claude` is gone (D-46).

### 2. Visuals (3/4)
- Three `role="alertdialog"` confirms on Settings plus the change-team and re-run dialogs on the task screen.
- Icon-only buttons labelled: `aria-label` ×23 on Settings, ×17 on the task screen.
- Weakness: `assign.html` has a single `aria-label`; the remove buttons near line 37 are small `text-xs` text buttons, not 48 px controls.
- Not verified without screenshots: focal point per screen, quota banner priority.

### 3. Color (3/4)
- Indigo per screen: settings 87, task 55, assign 17, stats 9 (Settings mostly editor rings and selection markers, allowed).
- Rose (`rose-6*`): assign 3, settings 5, task 2, stats 0 — within the destructive extension. No hardcoded hex. Stats bars share one indigo series as specified.
- 60/30/10 distribution not checked visually.

### 4. Typography (2/4)
- Sizes: `text-sm` 350, `text-base` 154, `text-xl` 33, `text-2xl` 9, `text-xs` 44 (settings 4, assign 11, task 29).
- Weights: `font-normal` 264, `font-semibold` 256, `font-medium` 8 (settings 3, assign 2, task 3).

### 5. Spacing (2/4)
- Off-scale: `py-1.5` 11, `px-3` 11, `gap-3` 5, `p-3` 5, `px-2` 21, `p-1` 5, `p-2` 9, `gap-1.5` 3, `space-y-0.5` 3, `space-y-3` 3, `space-y-1.5` 1, `py-0.5` 2, `px-2.5` 1, `py-3` 1; `min-h-10` on 5 controls (rule: `min-h-12`).
- The core scale still dominates: `gap-2` 140, `px-4` 111, `py-2` 115, `p-4` 60, `p-6` 47.

### 6. Experience Design (3/4)
- `aria-live` ×3 on Settings, ×4 on the task screen; loading, refusal and warning states on Giao việc and the task dialogs; Settings refusal callout uses `role="alert"` with `tabindex="-1"`.
- Departures: team/staff name input `maxlength="40"` (`settings.html:119`) vs 48 in the spec; section choice kept in `sessionStorage` (`settings.js:214`), matching the spec's `[ASSUMED C2]`.
- Not verified without a running instance: focus return after dialogs, quota banner `Thử lại sau` behaviour.

## Files Audited
`agent-brain/assets/dashboard/screens/{settings,assign,task,stats}.html`, `agent-brain/assets/dashboard/js/{settings,task,stats,assign}.js`, `06-UI-SPEC.md` (lines 1–423).

Registry audit: skipped (`shadcn_initialized: false`, `Tool: none`).
