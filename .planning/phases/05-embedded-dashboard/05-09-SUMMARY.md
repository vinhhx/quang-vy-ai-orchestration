---
phase: 05-embedded-dashboard
plan: 09
subsystem: dashboard-ui
tags: [alpine-csp, aria-combobox, form, giao-viec]

requires:
  - phase: 05-embedded-dashboard
    provides: "05-08 shell + AB helpers, 05-04 POST /api/tasks / preflight / slug / jobs, 05-07 GET /api/projects/suggest"
provides:
  - "screens/assign.html + js/assign.js: Giao việc — request textarea (maxlength 20000), ARIA combobox project picker (ArrowUp/Down, Enter, Escape, aria-activedescendant, missing projects skipped), slug input (maxlength 48, prefilled via /api/slug, editable), preflight panel (server texts verbatim), start button with disabled reasons, 409 candidate pick list, job polling that opens #/task/ID or shows `Lead dừng sớm`"
affects: [05-10, 05-13]

actuals:
  tasks: 1
  commits: 1

tech-stack:
  added: []
  patterns:
    - "Request bodies are inline object literals on one AB.api line, so a grep gate can prove no profile is ever sent (D-15)"

key-files:
  created:
    - agent-brain/assets/dashboard/js/assign.js
    - agent-brain/assets/dashboard/screens/assign.html
  modified:
    - agent-brain/src/dashboard/assets.rs
    - agent-brain/assets/dashboard/app.css

key-decisions:
  - "Executed by Antigravity (agy 1.4.0, --dangerously-skip-permissions approved for Phase 5); reviewed by Claude"

patterns-established:
  - "Async inputs (suggest, slug, preflight) use sequence counters so a late response never overwrites a newer one"

requirements-completed: []

duration: 15min
completed: 2026-10-17
status: complete
---

# Phase 5 Plan 09: Giao việc Screen Summary

**Giao việc is a narrow single-card form:**
- **Request:** the user writes a Vietnamese request.
- **Project:** chosen with a keyboard-usable combobox, so an ambiguous name can never start a run. Typing clears the choice, missing projects can't be chosen, and the 409 candidates become a pick list.
- **Task name:** edited in the slug field.
- **Preflight:** the server's texts are shown verbatim before starting. After the start, the screen follows the job and opens the new task.

## Performance

- **Duration:** about 15 min (agy run 563 s, plus review)
- **Completed:** 2026-10-17
- **Tasks:** 1
- **Files:** 4

## Accomplishments

- **Combobox:** an `<input role="combobox" aria-autocomplete="list" aria-controls="ab-suggest" :aria-activedescendant="activeId">` over `<ul id="ab-suggest" role="listbox">` with `role="option"` rows. The keys are ArrowDown/ArrowUp (missing projects skipped), Enter (choose), and Escape (close). A mousedown on an option calls `preventDefault` so the choice lands before the blur. `Đang tìm...` appears after 200 ms, and the no-match copy carries a filter hint when a profile filter is on.
- **Start button:** the disabled reason follows the UI-SPEC order: project, request, slug, then the preflight reason, then `Đang kiểm tra...`. The button is linked to the reason through `aria-describedby="ab-start-reason"`.
- **After submit:** the button reads `Đang giao...` and then `Đã giao việc — đang mở task...`. The screen polls `GET /api/jobs/ID` every 500 ms and switches to `#/task/<task_id>` once Lead has created the task. When the job exits without a task, it shows `Lead dừng sớm` with the job output.

## Task Commits

1. `3dae4c8` feat(05-09): Giao việc screen (agy)

## Deviations from Plan

None.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report. Each `<automated>` command was extracted from the plan and run verbatim:

- **Verify commands:**
  1. `node --check` + `cargo build` + `bash scripts/dashboard-smoke.sh overview assign` prints "Smoke OK: overview assign".
  2. The banned-class grep prints nothing, and `dashboard-ui-lint.sh` prints "UI lint OK: 16 thẻ tương tác".
  3. The no-profile-in-body gate exits 0. The only POST is `AB.api('POST', '/api/tasks', { project: this.selected.id, request: this.request, name: this.slug })`.
- **Acceptance:**
  - `build-css.sh --check` passes, with no `x-html`, `style="` or URL in assign.html.
  - Each ARIA string appears exactly once, and each key name appears in assign.js.
  - The limits and copy strings (`maxlength="20000"`, `maxlength="48"`, `max-w-3xl`, `Lead dừng sớm`, `Tên khớp nhiều dự án — chọn một:`, `Đang giao...`, `Đã giao việc — đang mở task...`, the suggest error and the no-match and filter-hint copy) are all present.
- **Code read:** `onQueryInput` (clears the selection), `onQueryKey`, `onOptionDown`, `choose`, `chooseCandidate`, `refreshPreflight`, `submit` (the poll is cleared through the id `AB.every` returns), `disabledReason` and `startLabel`.
- **Visual:** headless screenshots of the screen in light and dark mode match the Giao việc screen contract: single column, `max-w-3xl` card, the label and helper texts, and the disabled `Giao việc` with `Chọn đúng một dự án trong danh sách` under it.
- **Full suite:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --no-fail-fast -- --test-threads=1` exits 0 with 236 passed and 0 failed. The Python suite is OK and `cargo fmt --check` exits 0.
- **Secrets:** the pattern scan is clean.

**Not covered here:** keyboard and mouse use of the combobox in a real browser. Headless `--dump-dom` cannot drive key events. This is the 05-13 live check of the assign screen (P4-07 / UI checks).

## Next Phase Readiness

05-10 (Chi tiết task: stepper, actions with Kill/re-plan dialogs, plan view, live log pane, the `timed_out` re-plan fix) can start. Giao việc already redirects to `#/task/ID`.
