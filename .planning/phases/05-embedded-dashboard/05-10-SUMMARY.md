---
phase: 05-embedded-dashboard
plan: 10
subsystem: dashboard-ui
tags: [alpine-csp, stepper, sse, log-pane, dialogs, ui-spec]

requires:
  - phase: 05-embedded-dashboard
    provides: "05-08 shell + AB helpers, 05-05 task detail/actions/kill/raw API, 05-02 SSE stream, 05-09 redirect to #/task/ID"
provides:
  - "screens/task.html + js/task.js: Chi tiết task — header card with banner badge, Lead → Dev → QA stepper (<ol>, aria-current=step, current step ring, stacks below 768 px), tokens/time/cost per step, action buttons from the server's actions (primary/secondary/destructive, disabled reasons visible), Kill and Chạy lại Lead alertdialogs, blocked / needs-fix callouts, read-only plan card (path + copy, commit, sections, changes table), QA card, Lịch sử run table"
  - "live log pane: role=log, one <button> row per event (time, glyph, CLI text), auto-follow with pause and `Về cuối (live)`, thinking_tokens hidden by default with a toggle, 5000-row retention marker, raw-JSON dialog (role=dialog, focus management, Escape), SSE with Last-Event-ID reconnect"
  - "taskstate.rs: an open task whose Lead run ended timed_out offers Chạy lại Lead (reviewer note R1), unit test test_actions_open_after_timed_out_lead_offers_relead"
affects: [05-11, 05-12, 05-13]

actuals:
  tasks: 3
  commits: 5

tech-stack:
  added: []
  patterns:
    - "Dialogs are in-page alertdialog/dialog elements with safe default focus, never window.confirm"

key-files:
  created:
    - agent-brain/assets/dashboard/js/task.js
    - agent-brain/assets/dashboard/screens/task.html
  modified:
    - agent-brain/src/dashboard/assets.rs
    - agent-brain/assets/dashboard/app.css
    - agent-brain/src/dashboard/taskstate.rs

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, --dangerously-skip-permissions approved for Phase 5); reviewed by Claude"
  - "Reviewer-found light-theme defect fixed through a follow-up fix plan run by agy (.planning/fixes/05-10-log-glyph-color.plan.md)"

patterns-established:
  - "Every element on the dark log pane sets its own light colour; nothing inherits the body colour"

requirements-completed: [DASH-04, DASH-05]

duration: 35min
completed: 2026-10-17
status: complete
---

# Phase 5 Plan 10: Chi tiết task Summary

**The task screen shows:**
- **Decision first:** the state and the next action, with the Lead → Dev → QA stepper giving each step's state, tokens, time and cost.
- **Actions:** the CLI's next steps as buttons, with Kill and re-plan confirmations.
- **Evidence:** the read-only plan and QA report, then a live log pane whose rows read like the CLI lines and open their raw JSON on click.
- **Re-plan fix:** an open task whose Lead timed out can now be re-planned.

## Performance

- **Duration:** about 35 min (agy run 954 s + fix run 91 s, plus review)
- **Completed:** 2026-10-17
- **Tasks:** 3 (Task 3 TDD)
- **Files:** 5 (+1 fix)

## Accomplishments

- **Stepper:** `flex flex-col gap-4 md:flex-row` inside an `<ol aria-label="Pipeline Lead, Dev, QA">`. The `→` connectors are hidden below 768 px. The current step carries `ring-2 ring-indigo-600 dark:ring-indigo-400` and `aria-current="step"`. Steps without a run show `Chưa chạy`, and unknown figures show `—`.
- **Actions:** follow the server's `actions` order. The first enabled one is primary, the rest are secondary, and Kill is destructive. A disabled action keeps its reason visible under it. While a request is in flight, the clicked button reads `Đang giao...` (`Đang dừng...` for Kill). Kill and Chạy lại Lead open in-page `alertdialog`s with the UI-SPEC copy, focused on the safe button.
- **Log pane:** `role="log" aria-live="off" tabindex="0"`, `h-96 lg:h-[32rem]`. Each row is a `<button>` that opens the raw-JSON `dialog`. The pane auto-follows and pauses 40 px above the bottom, and `system/thinking_tokens` rows are hidden behind a toggle. The stream states are `đang kết nối`, `live`, `mất kết nối — đang thử lại...`, `đã kết thúc` and `lỗi`.
- **taskstate.rs:** `timed_out` added to the `open` arm's re-plan statuses, with a new unit test (RED commit, then GREEN).

## Task Commits

1. `17f2669` feat(05-10): task screen — stepper, actions, dialogs, plan and QA (agy)
2. `d7cb85f` feat(05-10): live log pane with SSE (agy)
3. `eb42133` test(05-10): timed-out Lead should offer Chạy lại Lead (agy)
4. `df0767e` fix(05-10): offer Chạy lại Lead after a timed-out Lead (R1) (agy)
5. `6e3afa1` fix(05-10): log glyphs use the row colour on the dark pane (agy, reviewer fix plan)

## Deviations from Plan

- **R1 (planned deviation, recorded by the plan):** `agent-brain/src/dashboard/taskstate.rs` gains `|| r.status == "timed_out"` in the `open` arm, so a task whose Lead timed out offers Chạy lại Lead. The change is one condition plus its test.
- **Reviewer-found defect, fixed by a follow-up fix plan (`.planning/fixes/05-10-log-glyph-color.plan.md`):** in the light theme, the log-row glyph `<span>` inherited the body's `text-slate-900` on the `bg-slate-950` pane, so ▶ ↩ ⓘ were nearly invisible. agy added `:class="rowTextClass(r)"` to that span, one attribute in task.html. The glyph now takes its row kind's colour in both themes, as the UI-SPEC log-pane colours require.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- **Scope:** commits 1–4 touch exactly the 5 declared files. The fix commit touches only `screens/task.html`.
- **Verify commands:** all 6 `<automated>` commands, extracted from the plan and run verbatim, exit 0:
  - the task smoke test;
  - the UI lint (30 interactive tags) and banned-class grep;
  - the overview/assign/task smoke test;
  - the glyph and body gates;
  - the taskstate unit tests (8 passed).
- **Acceptance:** all 30 command criteria pass under the reviewer's acceptance checker, with 0 failures. (The 7 skipped items are prose references, not commands.)
- **Full suite:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --no-fail-fast -- --test-threads=1` exits 0 with 237 passed and 0 failed (236 plus the new `timed_out` test). The Python suite is OK and `cargo fmt --check` exits 0.
- **Visual:** headless screenshots of `#/task/1` at 1280×1800 in light and dark (sandbox task 1 is "Chờ duyệt plan").
  - **Header:** the card with `Chờ duyệt plan`.
  - **Stepper:** Lead xong (tokens, time, `Chi phí: —`), Dev current with a ring, `Chạy Dev` primary and `Chạy lại Lead` secondary, QA chờ.
  - **Plan card:** path and `Sao chép`, `Commit plan: <sha>`, sections, and the changes table.
  - **Log pane:** CLI-style rows.
  - **Glyph defect:** found in light mode, fixed by the fix plan, and re-checked: the glyphs are now visible.
- **After the fix:** the exact span grep gives 1, `build-css.sh --check` passes (no new classes), the UI lint is OK and `dashboard-smoke.sh task` is OK.

**Left for the live checks in 05-13:** a real browser's keyboard paths (dialog focus and Escape, log rows) and the 768 / 480 px stepper stacking (backstop UI-05).

## Next Phase Readiness

05-11 (Thống kê and Dự án screens, the offline-assets test) can start.
