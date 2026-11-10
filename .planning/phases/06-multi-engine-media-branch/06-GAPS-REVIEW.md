---
phase: 06-multi-engine-media-branch
reviewed: 2026-11-10T00:00:00Z
depth: standard
files_reviewed: 15
files_reviewed_list:
  - agent-brain/src/human.rs
  - agent-brain/src/engine.rs
  - agent-brain/src/teams.rs
  - agent-brain/src/departments.rs
  - agent-brain/src/dashboard/api_tasks.rs
  - agent-brain/src/telegram/bot.rs
  - agent-brain/src/telegram/render.rs
  - agent-brain/src/telegram/summary.rs
  - agent-brain/assets/dashboard/js/task.js
  - scripts/dashboard-js-check.mjs
  - agent-brain/tests/human_engine.rs
  - agent-brain/tests/task_team.rs
  - agent-brain/tests/engine_routing.rs
  - agent-brain/tests/departments.rs
  - agent-brain/tests/telegram_quota.rs
findings:
  critical: 0
  warning: 3
  info: 4
  total: 7
status: issues_found
---

# Phase 06 gap closure (06-27..06-30): Code Review Report

**Reviewed:** 2026-11-10
**Depth:** standard (diff `027c05a..HEAD`, rustfmt-only hunks ignored)
**Files Reviewed:** 15
**Status:** issues_found

## Summary

The four gap plans are largely sound. I traced these areas and found them correct:

- `human::done` retry logic (`already_merged`). A fix-stage `start_wait` computes a fresh `base` from the current HEAD, and `try_push` pushes that same commit. So `remote == base` after a Trả lại cycle and the "no new commits" refusal still fires. The local-commit case is also refused online and offline.
- The readiness policy is applied at all three bind points (`TeamReadiness::refusal`, `project_pin`, `member_usable`).
- Key leakage: only engine ids and env var names reach messages and JSON. Tests assert a planted key is absent from the task, job, run, and Telegram payloads.

No blockers. The warnings are consistency and completeness gaps: places where the new "no silent claude" or `chưa kiểm` policy is not applied everywhere it should be.

## Warnings

### WR-01: phong-ban still falls back to claude silently when departments.json is broken or the config will not load (06-30 incomplete)

**File:** `agent-brain/src/engine.rs:~710-760` (the `has_members` computation and the `active_cfg == None` path)
**Issue:** 06-30 stops the step only when `loaded.members_of(dept)` is non-empty. `load_from` returns `members: []` plus `errors` for a JSON syntax error, a non-object root, a file over `MAX_FILE_BYTES`, or an unreadable file. In every one of those cases `has_members` is false. The code then takes the "no members -> team engine (claude)" path, with no warning and with `loaded.errors` ignored. The same happens when the config fails to load: `active_cfg` is `None`, `warn(...)` is emitted, and the run goes ahead on claude. The user's intent was department members, and a typo in departments.json silently turns the task into a claude run. That is exactly the D-16 behaviour 06-30 set out to remove. `TeamReadiness::refusal` has the same blind spot, so the quota buttons offer `phong-ban` in this state.
**Fix:** Treat `!loaded.errors.is_empty() && loaded.members.is_empty()` as "department file unusable" and return a `BrainError` that names the first error. For the config-load failure on `phong-ban`, return an error instead of `warn` plus claude. Mirror the first change in `TeamReadiness::refusal`. For example:
```rust
if dept.is_some() && loaded.members.is_empty() && !loaded.errors.is_empty() {
    return Err(BrainError::error(format!(
        "Đội hình {}: departments.json không đọc được ({}) — không tự chạy bằng engine khác.",
        task_team.name, loaded.errors[0])));
}
```

### WR-02: the `chưa kiểm` mark ignores the department member that phong-ban will actually pick (06-29)

**File:** `agent-brain/src/teams.rs:~395-410` (`unverified_engine`); `agent-brain/src/departments.rs:~571` (`member_usable`, `pick_from` ranking)
**Issue:** 06-28 makes a `chưa kiểm` member usable. `unverified_engine` reads only `team.engine_for(...)`, which is claude for `phong-ban`. So for a `phong-ban` task that stops on quota:
- the Telegram button, the Telegram confirm text, and `quota.unverified_teams` (and so the dashboard re-run dialog) never carry the mark;
- the actual run lands on a member whose endpoint is unverified, and `pick_from` does not prefer a verified member over an unverified one (it ranks only by running count, score, and file order).

The doc comment admits "reads the team's own engine", and the test comment "phong-ban member readiness is wired in Task 2" shows the case was deferred. The user sees "claude" and gets an unverified gateway. The mark still appears in the run header (`plan.unverified`), but only after the run starts.
**Fix:** For `phong-ban` with members, compute the mark from the member that `ready_candidates` + `pick_from` would choose. Alternatively mark the team when any ready candidate is `chưa kiểm`. At minimum, rank verified members first in `pick_from`, so a `chưa kiểm` member is picked only when no `sẵn sàng` member qualifies.

### WR-03: the dashboard Phòng ban page still treats `chưa kiểm` members as not ready

**File:** `agent-brain/src/dashboard/api_departments.rs:184` (`member_readiness`)
**Issue:** `let is_ready = s.readiness == "san_sang";` was not moved to `engine::readiness_allows_run`. After 06-28, `member_usable`, `ready_candidates`, `pick_from` and `TeamReadiness::refusal` accept `chưa kiểm`. The department page shows the same member as not ready ("Chưa sẵn sàng" badge, `ready: false`) while the engine picks that member and runs it. This is the cross-bind-point inconsistency 06-28 was meant to eliminate.
**Fix:**
```rust
let is_ready = crate::engine::readiness_allows_run(s.readiness);
```
Keep the reason text for `chưa kiểm` so the page can show a "chưa kiểm" note instead of a failure.

## Info

### IN-01: project pins override the team engine, so the `chưa kiểm` mark and the readiness check can describe the wrong engine

**File:** `agent-brain/src/teams.rs:~326-410`; `agent-brain/src/engine.rs:~686-700`
**Issue:** `resolve_inner` applies `project_pin` before the team engine. `TeamReadiness::refusal` and `unverified_engine` only look at the team's engine. With a `role_engines` pin on an unverified endpoint, the run is marked but the Telegram and dashboard buttons show no mark. A team whose own engine is unready is hidden even though the pin would run fine. The behaviour predates this gap work, but it limits how much the new mark can be trusted. A short note in the `unverified_engine` doc comment, or passing the pins into `TeamReadiness`, would help.

### IN-02: stale or misleading text

**Files:** `agent-brain/src/teams.rs:~1370-1400`; `agent-brain/src/telegram/render.rs:~437`
**Issue:**
- Test `c3_unverified_engine_is_ready_and_marked` carries a stale comment ("phong-ban member readiness is wired in Task 2"). It also has an unused `pinned` engine and an unused `_phong_ban` binding. The phong-ban path is not asserted there.
- The hidden-teams line now says "(engine thiếu key hoặc thiếu lệnh)". `not_ready = teams.len() - quota_teams.len()` also counts profile refusals (SPK-10), unknown engines, and department teams with no usable member, so the parenthetical is not accurate.

**Fix:** Remove or complete the test items. Reword the hidden-teams line to "chưa sẵn sàng cho bước này" without naming causes, or list only the causes that can actually occur.

### IN-03: `readiness_allows_run(&str)` is stringly typed

**File:** `agent-brain/src/engine.rs:113`
**Issue:** The callers pass `s.readiness`, which is a `&'static str` key. A typo or an unknown value silently returns false, which fails closed, but the type system does not help. The raw `== "san_sang"` comparison at `api_departments.rs:184` (WR-03) shows how easily a call site misses the helper. Consider a `Readiness::allows_run(self)` method plus a key-to-enum parser, or add a clippy/grep guard test that forbids a raw `"san_sang"` comparison outside `engine.rs`.

### IN-04: test hygiene in `tests/departments.rs`

**File:** `agent-brain/tests/departments.rs` (the two tests appended at the end)
**Issue:**
- Both tests call `std::env::set_var` and only `remove_var` at the very end. A failing `unwrap` or `assert` leaves `AB_TEST_DEPT_GW_KEY` and `AGENT_BRAIN_CONFIG_DIR` set and poisons `ENV_LOCK`, so later tests fail with misleading errors. The pre-existing tests do the same.
- The new tests are glued to the previous function without a blank line.

**Fix:** Use a small RAII guard that restores the env on drop, and add the blank lines.

---

_Reviewed: 2026-11-10_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
