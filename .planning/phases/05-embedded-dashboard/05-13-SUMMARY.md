---
phase: 05-embedded-dashboard
plan: 13
subsystem: live-acceptance
tags: [live, uat, kill, browser, loopback, offline]

requires:
  - phase: 05-embedded-dashboard
    provides: "05-01..05-12: the full dashboard, release smoke, LIVE-CHECKS P4 steps, empty acceptance tables"
provides:
  - "protocol-log/nghiem-thu-p4.md: P4-01..P4-09 Đạt, P4-10 Không áp dụng (D-18), UI-01..UI-06 Đạt, each with its evidence"
  - "the reviewed static binary installed in ~/.local/bin (sha256 2b1bb2c8…, includes both live-check fixes)"
affects: []

actuals:
  tasks: 2
  commits: 9

tech-stack:
  added: []
  patterns:
    - "Live UI checks can be driven in a real, visible Chrome over the DevTools protocol (Node 24 WebSocket), clicking the real buttons; signals still come only from the UI Kill"

key-files:
  created: []
  modified:
    - protocol-log/nghiem-thu-p4.md

key-decisions:
  - "Task 1 objective rows (P4-01, P4-03, P4-02 local half, UI-04..UI-06) were run by Claude at the user's request; P4-09 and the visual review by the user"
  - "P4-02 recorded Đạt on local-control evidence at the user's choice: the LAN machine cannot reach this host at all (even port 8000 on 0.0.0.0), so the cross-machine test is inconclusive; this host's own LAN IP answers on 8000 but refuses 7437, and ss shows only 127.0.0.1:7437"
  - "P4-06 + UI-01 were run by Claude at the user's request (\"please run test for me\") in a visible Chrome driven over CDP, against the real claude 2.1.303 on the throwaway repo"
  - "Two defects found live were fixed through agy fix plans and re-verified"

patterns-established:
  - "A live check that the user marks done is cross-checked against the DB/repo before it is recorded; rows without evidence stay open"

requirements-completed: [DASH-01, DASH-02, DASH-03, DASH-04, DASH-05, DASH-06, DASH-07, DASH-08, DASH-09, DASH-10, DASH-11, DASH-12]

duration: 2 sessions (2026-10-19)
completed: 2026-10-19
status: complete
---

# Phase 5 Plan 13: Live Acceptance Summary

**On the real machine, every Phase 5 success criterion holds:**
- **One file, no install:** the static binary serves the dashboard with nothing else installed.
- **Access:** it listens on 127.0.0.1 only and refuses every request without the per-start token. The CSS still works with the network off.
- **Giao việc:** assigning a Vietnamese task runs Lead → Dev → QA, one button press per step, with a live log that survives a reload. An ambiguous project name lists the matches instead of running.
- **Kill:** stops UI-started and terminal-started runs with no leftover process.
- **Profile filter:** filters the views while new tasks keep their project's profile, and Thống kê and Dự án work.
- **Fixes:** two UI defects found during the live run were fixed through agy and re-verified.

## Results (protocol-log/nghiem-thu-p4.md)

| Row | Result | Evidence |
|-----|--------|----------|
| P4-01 | Đạt | New static binary installed; all five screens render on real data (Chrome, data-loaded=yes); the release build also passes P4-01 in `env -i` |
| P4-02 | Đạt | `ss -ltn`: only 127.0.0.1:7437. Via this host's LAN IP, 8000 → 200 but 7437 → refused. The LAN machine was refused, but it also can't reach 8000 (firewall or isolation), so that half is inconclusive |
| P4-03 | Đạt | No/wrong cookie or `?token=` → 401 on `/`, `/app.css`, `/api/tasks`, SSE. Valid token → 303 then 200. Foreign Host → 421, foreign Origin → 403. After two restarts each token is new and old cookies get 401 |
| P4-04 | Đạt | Task 8: Lead run 13 → (press) Dev run 14 → (press) QA run 15, `qa:pass`; commits b62e29d, 061358c, d48ad80 in du-an-nhap |
| P4-05 | Đạt | User: reload mid-run keeps the old rows and receives new ones; pause and `Về cuối (live)`; raw JSON on click |
| P4-06 | Đạt | Case 1 (UI), task 9 / run 16: claude 84689 → bash 84880 → python3 84881. Case 2 (terminal), task 10 / run 17: 85055 → 85196 → 85197. After Kill → "Kill run", none alive after 5 s, `pgrep` clean, runs `killed`, terminal exit 130 |
| P4-07 | Đạt | User: alias `nhap` on two projects lists both; Giao việc stays disabled until one is picked; temporary alias removed |
| P4-08 | Đạt | User: the filter changes Tổng quan, Thống kê and the picker; new runs used profile work (DB) |
| P4-09 | Đạt | User: network off, every screen keeps its styles and fonts |
| P4-10 | Không áp dụng | D-18: no Figma design |
| UI-01 | Đạt | Killed runs 16/17 show `Chi phí —` (not `≈ $0.0000`), with the CLI's partial token sums |
| UI-02 | Đạt | User: raw JSON scrolls in its dialog; long branch and 120-char path wrap |
| UI-03 | Đạt | User: task 8's ẾCH ỘP ỮNG HỘ title keeps its diacritics when clamped |
| UI-04 | Đạt | 1024 and 768 px: one day of data so far, no overlap; re-check once there are more than 14 days |
| UI-05 | Đạt | 768 and 480 px: the header wraps, controls usable (after the overflow fix below) |
| UI-06 | Đạt | 480 px: the stepper stacks, with no arrows |

## Defects found live and fixed

1. **Tổng quan overflowed horizontally at 480 px (05-08).** A long monospace slug in a `truncate` line widened the implicit grid column, so the group cards ran past the viewport.
   - **Fix:** `.planning/fixes/05-08-overview-narrow-overflow.plan.md` (agy, `77e94ed`) adds `min-w-0` to the group card.
   - **Re-checked:** a 480 px screenshot shows the cards fit and the slug truncates.
2. **Confirmation and raw-JSON dialogs never got their initial focus (05-08 `AB.focusLater`).** `focusLater` focused the button after `setTimeout(0)`, but Alpine's `x-show` reveals the dialog later, so `focus()` hit a hidden element and was never retried. Focus stayed on `<main>`, and Escape did nothing.
   - **Fix:** `.planning/fixes/05-08-dialog-initial-focus.plan.md` (agy, `4b32bc4`) retries every 25 ms until the element is visible, up to about 1 s.
   - **Re-checked in the real Chrome:** focus lands on `Giữ plan hiện tại` and `Đóng`, Escape closes both dialogs, and no run started.

After both fixes: 243 Rust tests pass, along with the five-screen smoke test, the UI lint, the Python suite and fmt. The reinstalled binary includes both fixes.

## Open follow-ups (non-blocking)

- **Focus return after close:** after a dialog closes, focus goes to `<body>` instead of back to the button that opened it. The UI-SPEC asks for it to return to the trigger. Accessibility only; a small `app.js`/`task.js` fix plan.
- **Combobox Enter race:** pressing Enter within about 200 ms of typing in the project picker chooses from the previous (unfiltered) list. The chosen project is shown in the field and the preflight panel before starting.
- **UI-04 needs more data:** re-check the day-label thinning once Thống kê has more than 14 days of runs.
- **P4-02 cross-machine half:** inconclusive while this host is unreachable from the LAN. Re-test from a machine that can reach port 8000 if needed.

## Task Commits

- `e67c213` test(05-13): record P4-01, P4-03 and UI-04..06 live results; P4-02 local half
- `7a2aa25` test(05-13): P4-03 re-checked after a second restart
- `4840a6f` test(05-13): record P4-02 with local loopback-bind control
- `401aea0` test(05-13): record P4-09 and the visual review (task 1 done)
- `89c7e07` test(05-13): record P4-04, P4-05, P4-07, P4-08, UI-02, UI-03
- `4fdce30` test(05-13): record P4-06 and UI-01 (UI and terminal kill, no leftovers)
- `77e94ed` fix(05-08): overview group cards shrink at narrow widths (agy, fix plan)
- `4b32bc4` fix(05-08): focusLater waits until the dialog button is visible (agy, fix plan)

## Review (Claude)

- **Acceptance commands:** all four of the plan's checks exit 0. Task 1 has 5 P4 rows and 3 UI rows; Task 2 has 5 P4 rows and 3 UI rows. `grep -nE 'token=[0-9a-f]{16,}' protocol-log/nghiem-thu-p4.md` prints nothing.
- **Task 2 verification:**
  - no `task/p406-*` branch, and du-an-nhap is on a clean `main`;
  - projects_map.json has no temporary entry (alias `nhap` only on du-an-nhap, no path of 120+ characters);
  - no agent-brain run, serve or python sleep process is left;
  - P4-04 and P4-06 runs were cross-checked in brain.db.
- **Kill rule:** signals came only from the UI Kill (runner::cmd_kill), plus SIGINT and SIGTERM to the recorded PIDs of the serve and Chrome processes Claude itself started. Before stopping the user's own serve, its PID's cmdline was confirmed to be `agent-brain serve`.
- **Previous binary:** backed up at /tmp/claude-1000/-mnt-data-IDE-QuangVy-Orchestration/10436bb8-7059-473d-a616-dc31d8e09270/scratchpad/agent-brain.phase3-backup.

## Next Phase Readiness

All 13 plans have SUMMARYs. Phase verification (verify_phase_goal) is next.
