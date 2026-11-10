---
phase: 06-multi-engine-media-branch
reviewed: 2026-11-10T00:00:00Z
depth: standard
files_reviewed: 27
files_reviewed_list:
  - agent-brain/src/engine.rs
  - agent-brain/src/dsh.rs
  - agent-brain/src/quota.rs
  - agent-brain/src/human.rs
  - agent-brain/src/secrets.rs
  - agent-brain/src/runner.rs
  - agent-brain/src/teams.rs
  - agent-brain/src/staff.rs
  - agent-brain/src/config.rs
  - agent-brain/src/pricing.rs
  - agent-brain/src/stats.rs
  - agent-brain/src/db.rs
  - agent-brain/src/roles/mod.rs
  - agent-brain/src/roles/dev.rs
  - agent-brain/src/autopilot/mod.rs
  - agent-brain/src/dashboard/api_settings.rs
  - agent-brain/src/dashboard/settings_store.rs
  - agent-brain/src/dashboard/mod.rs
  - agent-brain/src/dashboard/api_stats.rs
  - agent-brain/src/dashboard/api_tasks.rs
  - agent-brain/src/dashboard/sse.rs
  - agent-brain/src/dashboard/assets.rs
  - agent-brain/src/telegram/callback.rs
  - agent-brain/src/telegram/bot.rs
  - agent-brain/assets/dashboard/js/settings.js
  - scripts/phase6-live-check.sh
  - spikes/e09/run.sh
findings:
  critical: 1
  warning: 6
  info: 6
  total: 13
status: issues_found
---

# Phase 6: Code Review Report

**Reviewed:** 2026-11-10
**Depth:** standard
**Files Reviewed:** 27 (see list; several large files were read only in the engine, quota, human and settings paths)
**Status:** issues_found

## Summary

The engine seam, the secret scrubbing, the dsh kill path (marker token plus recorded PIDs, no kill by name) and the settings write path are well built. I found no secret leak in the code paths I traced, no unsafe kill, and no injection through the dsh wrapper (the prompt travels only as `$2`). The dashboard auth gate (Host, token cookie, Origin, Content-Type) is sound.

The real defects are in these areas:

- The human `done` step mutates git before it commits to the database, so a failure between the two leaves the task permanently stuck.
- The quota detector matches generic words in model output.
- Three places apply different readiness rules to the same engine.
- The department picker falls back to claude without telling the user.
- The runner busy-loops when stdout closes before the child exits.
- A few smaller correctness and hardening items.

Not read in depth: `dashboard/taskstate.rs`, `dashboard/pipeline.rs`, `autopilot/decide.rs`, `roles/lead.rs`, `telegram/{outbox,render,intake,detect}.rs`, `dashboard/{api_workflows,api_projects}.rs`, and `task.js`, `assign.js`, `stats.js`, `overview.js`, `app.js`. The spike raw evidence was not opened beyond a pattern check of the scripts.

## Critical Issues

### CR-01: `human done` fast-forwards git before the DB commit, and a retry can never finish

**File:** `agent-brain/src/human.rs:357-427`

**Issue:** `done()` runs `git merge --ff-only FETCH_HEAD` (line 367) first. It then does the fallible work (`allowed_paths`, `diff_names(...)?` at line 372, `insert_human_run`, and the `set_task_team_cas` compare-and-swap inside the transaction at 416-427). Any error after the merge leaves the local branch advanced, but the `wait` marker is still in `team_json` and no human run exists. Examples:

- `diff_names` fails.
- SQLite returns BUSY past the 5 s `busy_timeout` while `serve` and `auto` write.
- The CAS loses (`team_changed()`).
- The process is killed mid-way, for example by a `serve` restart that stops its jobs.

On the next click, `remote_sha == head` is true. Lines 357-361 return "Nhánh … chưa có commit mới nào — nhờ … đẩy việc đã làm", which is false because the work is already merged. The task can no longer leave the waiting state through the dashboard or the CLI. The only exits are a manual `git reset` (which rewrites local history, something the module promises never to do) or editing `team_json` by hand.

**Fix:** Make the step idempotent. Compare against the recorded base, not against the pre-merge HEAD, and skip the merge when it has already happened:

```rust
let head = crate::gitops::head_sha(root)?;
let already_merged = head != p.wait.base && crate::gitops::is_ancestor(root, &remote_sha, &head);
if !already_merged {
    if remote_sha == head { /* the "no new commit" error */ }
    if !crate::gitops::is_ancestor(root, &head, &remote_sha) { /* the rewritten-history error */ }
    crate::gitops::git(root, &["merge", "--ff-only", "FETCH_HEAD"])?;
}
let new_head = crate::gitops::head_sha(root)?;
```

When `already_merged` is true, skip the `git fetch` failure branch as well, so an offline retry still works. Continue with the diff check and the DB transaction.

## Warnings

### WR-01: Supervisor busy-loops when the child closes stdout before it exits

**File:** `agent-brain/src/runner.rs:586-621`

**Issue:** The `line = rx.recv()` select arm has no guard. After the stdout reader task hits EOF and drops `tx`, `rx.recv()` returns `None` immediately on every poll, and the arm body (`None => {}`) does nothing. The loop spins at 100% of a core until the `child.wait()` arm happens to fire. This happens whenever a pipe-mode child (claude, or the `docker exec` client of dsh) closes stdout and then lingers: slow MCP or hook shutdown, or a descendant that keeps the process group alive. If the child never exits, it spins until the timeout. The timeout, signal and `after_result` arms are still polled, but the CPU burn is real, and it starves the single-threaded runtime's other work.

**Fix:** Disable the arm once the channel is closed:

```rust
let mut rx_open = true;
...
line = rx.recv(), if rx_open => match line {
    Some(line) => { /* existing body */ }
    None => rx_open = false,
}
```

### WR-02: Quota detector matches generic words in model output, so ordinary failures become "quota" stops

**File:** `agent-brain/src/quota.rs:59`, `quota.rs:219-239`; call site `agent-brain/src/roles/mod.rs:1172-1183`

**Issue:** `text_regex` contains the bare tokens `rate limit`, `usage limit`, `spend limit`, `\b429\b` and `\b402\b`. For dsh, `detect_text` applies it to the last 8 KiB of the run's stdout (`obs.summary.result_text` is the model's own text) whenever the exit code is non-zero.

Scenario: a Dev task such as "add rate limiting to the API" or "return HTTP 429" fails on dsh with a non-zero exit. The transcript mentions "rate limit" or "429", so the run is stored with `outcome = quota`. The autopilot stops with `het_quota`, the dashboard offers "Chạy lại bằng đội hình…", and with `orchestrator.auto_resume_quota` on, the step is retried over and over without ever consuming a round. The same applies to agy, whose regex (`agy_regex`) matches the plain words `quota` and `credits` in a failing run's stderr, and to claude when stderr contains a path or line number like `file.rs:402`.

**Fix:**

- For text engines, scan stderr only. Treat stdout as evidence only when it is short, or when the matched line is anchored to a provider-error shape, for example `^(Error|HTTP|API error)[: ].*\b(429|402)\b`.
- Drop the bare `\b402\b` and `\b429\b` from `text_regex`. Match them only next to `HTTP`, `status` or `error`.

### WR-03: Department pick that finds no usable member falls back to the team's engine (claude) with no message

**File:** `agent-brain/src/engine.rs:696-716` (`Ok(None) => {}` at line 709); contrast with `agent-brain/src/teams.rs:342-353`

**Issue:** On the `phong-ban` team, when a department has members but none is usable (engine not ready, or refused by `profile_refusal`), `departments::pick_from` returns `Ok(None)` (departments.rs:640). `resolve_inner` silently keeps `resolved_engine`, which is the team's default `claude`. The run then starts on the user's own Claude account, possibly on the work profile, even though the user configured only non-claude members for that department. `TeamReadiness::refusal` refuses exactly this situation at every other bind point (dashboard re-run, quota buttons, autopilot restore), so the run seam disagrees with the checks. This contradicts the stated rule "the engine is never switched silently (D-16)". The `Err` arm (line 711) at least prints a warning, but it too proceeds on claude.

**Fix:** When `dept_of_role(role)` has members and `pick` returns `Ok(None)` or `Err`, return a `BrainError` that names the department and the role. Keep the fall-through to claude only when the department has no members at all, which is the documented case.

### WR-04: An engine in `chua_kiem` state runs through a team but is refused for pins, department members and quota re-runs

**File:** `agent-brain/src/engine.rs:564-573`; `agent-brain/src/departments.rs:559-563`; `agent-brain/src/teams.rs:367-377`; versus the run seam `engine.rs:991` and `roles/mod.rs:758`

**Issue:** D-40 says an endpoint without `verified` text "runs, but is marked". `plan_for` and the run header implement that (`unverified: true`, "chưa kiểm E-09"). `project_pin` (Engine branch), `member_usable` and `TeamReadiness::refusal` all require `readiness == san_sang`. `entry_status` reports `chua_kiem` for every `base_url` entry without `verified`, including the default `deepseek` entry.

Consequences:

- After a Claude quota stop, the built-in `tiet-kiem` team (Dev on deepseek) is not offered as a ready team, and `quota_rerun` refuses it. Yet assigning the same team at task creation runs fine.
- A project cannot pin deepseek.
- A department member cannot use deepseek.
- The only fix the user sees is hand-editing `verified` in config.json.

**Fix:** Decide the policy once and apply it everywhere. If D-40 stands, accept `SanSang | ChuaKiem` in `project_pin`, `member_usable` and `TeamReadiness::refusal`, and show the "chưa kiểm" mark on the buttons. If unverified endpoints must not run unattended, refuse them in `plan_for` too.

### WR-05: `parse_since` multiplication overflows on large input

**File:** `agent-brain/src/stats.rs:95-109`; reachable from `agent-brain/src/dashboard/api_stats.rs:30`

**Issue:** `^(\d+)([mhd])$` accepts any `i64`. `GET /api/stats?since=99999999999999d` computes `num * 24*60*60*1000` with plain `*`. This panics in debug builds, which `blocking` turns into a 500, and wraps silently in release builds. The wrapped value can be negative, which makes `now_ms - duration` a future timestamp and returns an empty report with HTTP 200. It needs the dashboard cookie, so it is a robustness bug, not a vulnerability.

**Fix:** Use `num.checked_mul(unit_ms).and_then(|d| now_ms.checked_sub(d))` and return the existing "Khoảng thời gian không hợp lệ" error on `None`.

### WR-06: Settings writes lose concurrency protection and can replace a symlinked `config.json`

**File:** `agent-brain/src/dashboard/api_settings.rs:1041-1048`; `agent-brain/src/dashboard/settings_store.rs:106`

**Issue:**

- `save_engine_models_blocking` has no `version` field in `EngineModelsSaveBody`. It reads the file's current version itself and passes that to `write_object`, so the optimistic-concurrency check is a no-op for this route. A save made from a stale Settings page silently overwrites a `models` block edited in the meantime. The teams, staff, workflow and orchestrator routes all do carry a client version.
- `atomic_write` renames a fresh 0600 temp file over `path`. If `config.json` (or `teams.json`) is a symlink, for example managed by dotfiles, the symlink is replaced by a regular file and the real target is no longer updated. The same happens to ownership and mode.

**Fix:**

- Add `version: String` to `EngineModelsSaveBody`, have the JS send the version it loaded, and pass it to `write_object`. Reuse the ORCH_CONFLICT handling.
- In `atomic_write`, resolve the final path first (`std::fs::canonicalize(path)` when the file exists) and write the temp file next to the canonical path. Keep the original mode with `set_permissions` before the rename.

## Info

### IN-01: `kill` can miss the container-side stop for a dsh run that was only just spawned

**File:** `agent-brain/src/roles/mod.rs:920-970`, `agent-brain/src/runner.rs:81-135`

`on_spawn` inserts the run row (status `running`, pid set) in one statement and writes `metrics_json.dsh` in a later `merge_run_metrics`. `cmd_kill` reads `run.metrics_json` once. A kill that lands between the two writes finds no `dsh` object, skips `stop_in_container`, and kills only the host `docker exec` client. The container process then survives (the S6 4a case). The window is a few milliseconds. Fix: write `dsh` in the same `INSERT` (add a `metrics_json` parameter to `insert_run_with_engine`), or have `cmd_kill` re-read the run row right before `dsh_run_of`.

### IN-02: `warn_leftover` matches any token that starts with `ab-dsh-`

**File:** `agent-brain/src/dsh.rs:480`

`args_tokens(...).any(|t| t.starts_with(MARKER_PREFIX))` also matches processes whose command line merely quotes the prefix. That includes any `sh -c` whose prompt contains text such as `ab-dsh-0123…`, which is common when the task is about this repository. The result is a spurious "container còn tiến trình của run dsh trước" warning listing unrelated PIDs. The warning only prints, so nothing is signalled. Fix: require `is_valid_marker(t)` instead of `starts_with`.

### IN-03: `key_env` is not checked against critical variable names

**File:** `agent-brain/src/config.rs:356-363`; `agent-brain/src/engine.rs:775-779`

`valid_key_env` accepts any upper-case name. `strip_key_envs` removes every configured `key_env` from every child's environment, claude and agy included. A mistyped `key_env: "PATH"` or `"HOME"` strips that variable from all runs. No dashboard route writes `key_env`, so this needs a hand-edited config. Fix: reject `PATH`, `HOME`, `USER`, `SHELL`, `PWD`, and names starting with `LD_` or `CLAUDE_`. The same check could reject a name listed in any profile's `unset_env` (see IN-04).

### IN-04: Engine readiness checks the process env, but a run reads the profile-filtered env

**File:** `agent-brain/src/engine.rs:340-343` (readiness) versus `engine.rs:934-947` (`anthropic_plan`)

`entry_status` tests `key_env` against `profile::current_env()`. `anthropic_plan` looks the key up in `base_env`, which has already passed through `build_env`, and `build_env` applies the profile's `unset_env`. A `key_env` that a profile unsets (for example `ANTHROPIC_AUTH_TOKEN`, which the default profiles unset) shows "sẵn sàng" and then fails at run time with "Chưa đặt biến môi trường". Fix: validate in config that no `key_env` is in any profile's `unset_env`, or read the key from the raw process env before `build_env`.

### IN-05: Orchestrator Settings save writes stale "live" values over on-disk edits

**File:** `agent-brain/src/dashboard/api_settings.rs:1216-1222`, `settings.js:548-562`

`GET /api/orchestrator/settings` returns the in-memory values together with the version of the file. The toggle posts all six fields. If the user edited the `orchestrator` block of config.json after `serve` started, flipping the toggle writes the old in-memory values back under the new file version, and the check passes. This is the intended "live" model, but it is worth surfacing in the UI (show when file and live values differ) or posting only the changed field.

### IN-06: Spike script and test fake allow riskier behaviour than production code

**File:** `spikes/e09/run.sh:75` (`E09_BASE_URL` regex `^https?://`), `tests/fakes/fake_docker.py:127-139`

- `e09/run.sh` accepts `http://` for a real key run, so the key would travel in cleartext. `config.rs` requires `https://`. Fix: apply the same rule in the script (allow `http://` only in offline mode).
- The fake `docker exec kill` signals any PID it is given on the real host. Its `ps` lists the whole host table (only the fake client is filtered out). If the marker logic ever regressed, a test run could signal unrelated host processes. Fix: refuse PIDs whose host command line does not carry an `ab-dsh-` marker.

---

_Reviewed: 2026-11-10_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
