---
phase: 06-multi-engine-media-branch
plan: 27
subsystem: engine
tags: [human-engine, nguoi, retry, fast-forward, git, hum-02, cr-01]

requires:
  - phase: 06-12
    provides: "human done / start_wait, team_json wait marker, insert_human_run"
provides:
  - "human::done (Xong) is idempotent and retry-safe after a failure past the merge"
  - "offline retry from remote-tracking ref refs/remotes/origin/<branch> when staff work was already merged"
  - "merging the exact checked commit sha rather than FETCH_HEAD"
  - "refusal of local-only commits online and offline"
affects: [06-VERIFICATION]

actuals:
  tokens: 3790
  tasks: 2
  commits: 2

tech-stack:
  added: []
  patterns:
    - "already_merged helper detects partial progress across git/DB boundary"
    - "remote-tracking ref used as offline fallback only when already merged"

key-files:
  created: []
  modified:
    - agent-brain/src/human.rs
    - agent-brain/tests/human_engine.rs

key-decisions:
  - "already_merged requires remote != base && head != base && is_ancestor(base, remote) && is_ancestor(remote, head) so local-only commits never count as staff work"
  - "offline retry reads refs/remotes/origin/<branch> without git fetch/merge and proceeds directly to DB transaction"
  - "git merge --ff-only receives remote_sha rather than FETCH_HEAD to avoid race conditions"

patterns-established:
  - "idempotent retry pattern across non-transactional git and DB boundary"

requirements-completed: [HUM-02]

coverage:
  - id: D1
    description: "Retry-safe human done (Xong) after partial failure past fast-forward merge"
    requirement: HUM-02
    verification:
      - kind: integration
        ref: "agent-brain/tests/human_engine.rs#done_retry_after_failure_past_merge"
        status: pass
    human_judgment: false
  - id: D2
    description: "Offline retry of Xong using remote-tracking ref when staff commits already merged"
    requirement: HUM-02
    verification:
      - kind: integration
        ref: "agent-brain/tests/human_engine.rs#done_retry_offline_after_merge"
        status: pass
    human_judgment: false
  - id: D3
    description: "Retry takes newer pushed commits if staff pushed further after failure"
    requirement: HUM-02
    verification:
      - kind: integration
        ref: "agent-brain/tests/human_engine.rs#done_retry_takes_newer_push"
        status: pass
    human_judgment: false
  - id: D4
    description: "Local-only commits on task branch are refused and never mistaken for staff work"
    requirement: HUM-02
    verification:
      - kind: integration
        ref: "agent-brain/tests/human_engine.rs#done_local_commit_not_taken_as_staff_work"
        status: pass
    human_judgment: false

duration: 35min
completed: 2026-11-10
status: complete
---

# Phase 6 Plan 27: Human Engine Done Retry Summary

**`human::done` (the `Xong` button) is idempotent and retry-safe after a post-merge DB or process failure, supporting offline retries from remote-tracking refs and subsequent staff pushes without rewriting local history or accepting local-only commits.**

## Performance

- **Duration:** 35 min
- **Started:** 2026-11-10T14:02:27+07:00
- **Completed:** 2026-11-10T14:38:00+07:00
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Closed Gap 1 of `06-VERIFICATION.md` (`HUM-02` / code review finding `CR-01`): `agent-brain human done` no longer gets permanently stuck when a step fails between the git fast-forward and the SQLite database transaction.
- Implemented `already_merged(root, base, head, remote)`: verifies whether the staff pushed commits after the wait began (`remote != base`) and that local HEAD already incorporates that remote work (`is_ancestor(remote, head)`).
- Online retry path: when `already_merged` is true, skips `remote == head` and ancestor check refusals, skips `git merge`, logs `Nhánh task/sua-app đã được gộp ở lần bấm Xong trước — ghi nhận tiếp.`, and proceeds straight to scope verification and SQLite transaction.
- Offline retry path: if `git fetch` fails due to network/origin outage after an earlier merge, resolves `refs/remotes/origin/<branch>` and continues with `Không lấy được nhánh task/sua-app từ remote (<error>) — dùng các commit đã gộp ở lần bấm Xong trước.` without running `git merge`.
- Hardened merge execution: passes the exact checked SHA (`remote_sha`) to `git merge --ff-only` rather than `FETCH_HEAD`.
- Preserved all refusal guarantees: local-only commits are strictly refused online (`không đi tiếp từ nhánh trong máy`) and offline (`thử lại sau`), unpushed work yields `chưa có commit mới nào`, and history is never reset or forced.

## Task Commits

1. **Task 1: Xong retried after a failure past the merge finishes the step** - `8965fd6` (fix)
2. **Task 2: offline retry, newer pushes, and local-only commits refused** - `8815d08` (fix)

## Files Created/Modified

- `agent-brain/src/human.rs` - Added `already_merged` helper, restructured `done()` fetch handling with offline fallback via `refs/remotes/origin/<branch>`, updated merge command to use `remote_sha`, and added online/offline skip messages.
- `agent-brain/tests/human_engine.rs` - Added test helpers `fail_human_runs` / `allow_human_runs` using temporary SQLite abort triggers; added 4 integration tests (`done_retry_after_failure_past_merge`, `done_retry_offline_after_merge`, `done_retry_takes_newer_push`, `done_local_commit_not_taken_as_staff_work`).

## Decisions Made

- Enforced `remote != base` in `already_merged` so local commits without upstream staff work can never trigger the retry path.
- In `done()`, matched on `git fetch` result so the offline path cleanly uses `refs/remotes/origin/<branch>` without touching `git merge`.
- Restructured `git merge --ff-only` to target `&remote_sha` directly rather than the generic `FETCH_HEAD` ref.

## Deviations from Plan

None - plan executed exactly as specified.
Note on formatting: As documented in `06-10-SUMMARY.md` and `06-23-SUMMARY.md`, `cargo fmt --check` flags pre-existing formatting drift in files outside this plan's scope (`answers.rs`, `claim.rs`, etc.). Both files modified by this plan (`agent-brain/src/human.rs` and `agent-brain/tests/human_engine.rs`) were fully formatted and verified with `rustfmt --check`.

## Issues Encountered

- During the full test suite run, `telegram_actions::running_run_adds_kill_button` exhibited a timing flake due to a race between the initial button markup and the refreshed markup under heavy system load. Re-running the test in isolation passed immediately with 0 errors.

## Next Phase Readiness

- Gap 1 (`HUM-02`) is closed and verified.
- Ready for plan 06-28.

---
## Self-Check: PASSED
- FOUND: agent-brain/src/human.rs
- FOUND: agent-brain/tests/human_engine.rs
- FOUND commit: 8965fd6
- FOUND commit: 8815d08
