---
phase: 05-embedded-dashboard
reviewed: 2026-10-19T00:00:00Z
depth: standard
files_reviewed: 58
files_reviewed_list:
  - agent-brain/assets/config.default.json
  - agent-brain/assets/dashboard/app.css
  - agent-brain/assets/dashboard/index.html
  - agent-brain/assets/dashboard/js/app.js
  - agent-brain/assets/dashboard/js/assign.js
  - agent-brain/assets/dashboard/js/overview.js
  - agent-brain/assets/dashboard/js/projects.js
  - agent-brain/assets/dashboard/js/stats.js
  - agent-brain/assets/dashboard/js/task.js
  - agent-brain/assets/dashboard/js/theme.js
  - agent-brain/assets/dashboard/screens/assign.html
  - agent-brain/assets/dashboard/screens/overview.html
  - agent-brain/assets/dashboard/screens/projects.html
  - agent-brain/assets/dashboard/screens/stats.html
  - agent-brain/assets/dashboard/screens/task.html
  - agent-brain/assets/dashboard/vendor/alpine.csp.min.js
  - agent-brain/assets/dashboard/VENDOR.md
  - agent-brain/Cargo.lock
  - agent-brain/Cargo.toml
  - agent-brain/dashboard-src/input.css
  - agent-brain/LIVE-CHECKS.md
  - agent-brain/README.md
  - agent-brain/src/config.rs
  - agent-brain/src/dashboard/api_projects.rs
  - agent-brain/src/dashboard/api_runs.rs
  - agent-brain/src/dashboard/api_stats.rs
  - agent-brain/src/dashboard/api_tasks.rs
  - agent-brain/src/dashboard/assets.rs
  - agent-brain/src/dashboard/auth.rs
  - agent-brain/src/dashboard/jobs.rs
  - agent-brain/src/dashboard/lock.rs
  - agent-brain/src/dashboard/mod.rs
  - agent-brain/src/dashboard/pump.rs
  - agent-brain/src/dashboard/sse.rs
  - agent-brain/src/dashboard/taskstate.rs
  - agent-brain/src/lib.rs
  - agent-brain/src/main.rs
  - agent-brain/src/roles/lead.rs
  - agent-brain/src/routing.rs
  - agent-brain/src/stats.rs
  - agent-brain/tests/cli_lead.rs
  - agent-brain/tests/common/mod.rs
  - agent-brain/tests/dashboard_assets.rs
  - agent-brain/tests/dashboard_http.rs
  - agent-brain/tests/dashboard_kill.rs
  - agent-brain/tests/dashboard_pipeline.rs
  - agent-brain/tests/dashboard_projects.rs
  - agent-brain/tests/dashboard_run.rs
  - agent-brain/tests/dashboard_stats.rs
  - agent-brain/tests/dashboard_stream.rs
  - agent-brain/tests/routing_suggest.rs
  - .gitignore
  - protocol-log/nghiem-thu-p4.md
  - scripts/build-css.sh
  - scripts/build-release.sh
  - scripts/dashboard-keys.mjs
  - scripts/dashboard-smoke.sh
  - scripts/dashboard-ui-lint.sh
findings:
  critical: 1
  warning: 13
  info: 12
  total: 26
status: issues_found
---

# Phase 5: Code Review Report

**Reviewed:** 2026-10-19
**Depth:** standard
**Files Reviewed:** 58
**Status:** issues_found

## Summary

I reviewed the Rust dashboard (`auth`, `assets`, `sse`, `pump`, `jobs`, `lock`, `api_*`, `taskstate`), the stats, routing and config changes, the Alpine CSP front end, the shell and Node scripts, and the docs. No structural-findings block was provided, so there is no fallow section.

What holds up:
- **Asset serving** uses an exact-match table, so there is no path traversal.
- **SQL** is fully parameterised.
- **No `x-html` or `eval`** anywhere in the JS. The only `innerHTML` call writes embedded static HTML in `app.js:315`.
- **Vendored Alpine:** the file's sha256 matches `VENDOR.md`, and it contains no `eval` or `new Function`, so it is compatible with `script-src 'self'`.
- **Host and Origin checks:** the Host check runs before auth. The Origin check covers every mutating method, and `Content-Type` is enforced on POST and PUT.
- **Kill path:** the HTTP kill goes only through `runner::cmd_kill`. Signals are PID-start verified by `procs::alive`, and the scripts kill by PID only.
- **Secrets:** `protocol-log/nghiem-thu-p4.md` has no tokens or secrets, and `spikes/lib/scan-secrets.sh` reports OK.

Main concerns:
- **CR-01:** the only credential is a cookie, and cookies are not isolated by port.
- **Races:** several races in invariants the README promises, namely the E-06 single personal-profile run and job-to-task matching.
- **Front end:** stale-response races in the router, the raw-JSON dialog and the combobox.
- **README:** its API catalogue does not match the routes. This is confirmed, and several of its routes do not exist.

All three already-known items are confirmed (WR-02, WR-03, WR-04).

Limits of this review: `app.css` and the minified Alpine were sanity-checked only. The Rust integration tests were skimmed for reliability patterns (sleeps, kill-by-name, fixed ports) rather than read line by line. `overview.html`, `stats.html` and `projects.html` were grepped for dangerous sinks and only partially read.

## Critical Issues

### CR-01: The session cookie is the only credential and is not port-isolated, so any other local service can steal it and drive agents

**File:** `agent-brain/src/dashboard/auth.rs:35-45`, `agent-brain/src/dashboard/auth.rs:137-148`, `agent-brain/src/dashboard/auth.rs:158-165`

**Issue:**
- **Cookie leaks across ports.** The `agent_brain_token` cookie is host-scoped (`127.0.0.1`), and browsers do not scope cookies by port. Every request the browser makes to any other service on `http://127.0.0.1:<other>/` carries `Cookie: agent_brain_token=<64-hex>`. In this product's own workflow those are the dev servers of projects that AI agents build and the user previews on localhost, plus any compromised `npm` dev dependency.
- **Impact.** A hostile local service reads the header and replays it against the dashboard with a forged `Origin: http://127.0.0.1:<port>` header. The Origin check only stops browsers; a non-browser client can set the header freely. That gives it `POST /api/tasks` with an arbitrary prompt, which means agent execution in any registered project, plus Kill and the `projects scan` route.
- **Cookie tossing.** `cookie_value` returns the first `agent_brain_token` match. A neighbour service on the same host can set its own `agent_brain_token` cookie, and the browser may send it first. That locks the real user out with 401.
- **Docs.** `README.md` says "cookie gắn với host cụ thể" (the cookie is bound to a specific host). That is true for host but not for port, so the isolation it implies does not exist.

**Fix:** Do not make a cookie the only credential. Options, strongest first:
1. Keep the token out of cookies. Redeem `?token=` once in JS (`sessionStorage`) and send it as `Authorization: Bearer` / `X-AB-Token` on every `fetch` (browser `fetch` and `EventSource` honour same-origin only, so a neighbour port never receives it). `EventSource` cannot set headers, so replace it with `fetch()` plus a ReadableStream parser, or issue a short-lived single-use stream ticket from `POST /api/runs/{id}/stream-ticket`.
2. If the cookie stays, additionally require a header that a neighbour service cannot learn, for example a per-page `X-AB-CSRF` value returned in the 303 body or a meta tag, and compare it with `ct_eq`.
3. Either way, make `cookie_value` check every `agent_brain_token` pair, not only the first. This removes the tossing DoS:

```rust
raw.split(';').filter_map(|kv| kv.trim().split_once('=')).any(|(k, v)| k == name && ct_eq(v.as_bytes(), st.token.as_bytes()))
```

## Warnings

### WR-01: `stats::parse_since` multiplies and subtracts without overflow checks

**File:** `agent-brain/src/stats.rs:76-83`

**Issue:** The regex `^(\d+)([mhd])$` plus `parse::<i64>()` accepts any value up to `i64::MAX`. `num * 24 * 60 * 60 * 1000` and `now_ms - duration_ms` are unchecked. `Cargo.toml` has no `overflow-checks` in `[profile.release]`, so:
- In release builds, `GET /api/stats?since=99999999999999d` and `agent-brain stats --since 99999999999999d` silently wrap to a garbage window (a "since" far in the future or the past).
- In debug and test builds the same input panics. Inside `spawn_blocking` that surfaces as a 500 "Lỗi tác vụ blocking".
- The existing unit tests only cover small values.

**Fix:**
```rust
let mult: i64 = match unit { "m" => 60_000, "h" => 3_600_000, "d" => 86_400_000, _ => unreachable!() };
let err = || BrainError::error(format!("Khoảng thời gian không hợp lệ: '{s}' (ví dụ 30m, 24h, 7d)"));
let duration_ms = num.checked_mul(mult).ok_or_else(err)?;
now_ms.checked_sub(duration_ms).ok_or_else(err)
```
Add a test: `assert!(parse_since("99999999999999d", now).is_err())`.

### WR-02: README API catalogue lists routes that do not exist and omits real ones (confirmed known item)

**File:** `agent-brain/README.md:274-291` (routes in `agent-brain/src/dashboard/api_tasks.rs:428-438`, `api_runs.rs:16-21`, `api_stats.rs:17-19`)

**Issue:** Confirmed mismatches against the real routers:
- **Documented but absent:**
  - `POST /api/tasks/:id/lead` (the real route is `/relead`).
  - `GET /api/runs/:id/events`.
  - `GET /api/stats/days`.
  - `GET /api/stats/profiles`.
  - The day and profile data are folded into `GET /api/stats` as `by_day` and `by_profile`.
- **Wrong request body:** `POST /api/tasks` is documented with `"slug"`. The struct (`NewTask`, `deny_unknown_fields`) accepts `name`, so a client following the README gets a 422.
- **Real but undocumented:**
  - `GET /api/runs/{id}`.
  - `GET /api/runs/{id}/events/{seq}/raw`.
  - `GET /api/preflight`.
  - `GET /api/jobs/{id}`.
  - Also, `GET /api/slug?text=` and `GET /api/projects/{id}/status` are listed but the others around them are not.
- **Misleading wording:** the README output block calls the token "token truy cập một lần" (one-time token). It is reusable for the whole `serve` lifetime. It also says "không dùng localhost", but `host_allowed` and `origin_allowed` both accept `localhost:PORT`.

**Fix:** Rewrite the section from the real routers, using `{id}` notation. Fix `slug` to `name`, replace `/lead` with `/relead`, replace the two `/stats/*` entries with a note about `by_day` and `by_profile`, and replace `/events` with the two real run routes. Add `/api/preflight` and `/api/jobs/{id}`. Change "một lần" to "dùng trong suốt phiên serve", or implement real single-use semantics. A test that greps the README route list against `mod.rs` would keep the two from drifting again.

### WR-03: Confirmation dialog does not return focus to its trigger on every close path (confirmed known item)

**File:** `agent-brain/assets/dashboard/js/task.js:724-750` (markup `screens/task.html:266-275`)

**Issue:**
- `dialogOk` (743-750) never restores focus, so after "Kill run" or "Chạy lại Lead" focus falls to `<body>`. `runAction` also sets `busyAction`, which disables the original button (`actionDisabled`). The action list then changes to `[kill]` or `[]`, so the trigger usually no longer exists.
- `dialogCancel` restores `lastFocus` but never clears it. The restore is silently skipped if the element was detached or disabled by a 3 s re-render.
- `role="alertdialog" aria-modal="true"` has no focus trap. Tab moves focus into the background page. `@keydown.escape` is on the inner dialog only, so Escape does nothing when focus has left it.
- `scripts/dashboard-keys.mjs` only asserts that focus enters the dialog and that Escape hides it. It never asserts focus return.

**Fix:**
```js
closeDialog: function (restore) {
  this.dialog = null;
  window.AB.lockScroll(false);
  var t = this.lastFocus; this.lastFocus = null;
  if (restore && t && document.contains(t) && !t.disabled) t.focus();
  else window.AB.focusLater('screen');   // stable fallback: <main id="screen" tabindex="-1">
}
```
Call it from both `dialogCancel` and `dialogOk`. Add a Tab and Shift+Tab trap on the dialog, or set `inert` on the sibling content while it is open. Move the Escape handler to the overlay or to `document` while open. Extend `dashboard-keys.mjs` with `activeElement` checks after Cancel and Escape.

### WR-04: Combobox acts on stale suggestions inside the 200 ms debounce, and a late response silently resets keyboard navigation (confirmed known item)

**File:** `agent-brain/assets/dashboard/js/assign.js:75-86`, `97-128`, `130-172`

**Issue:**
- `onQueryInput` resets `active = -1` but leaves `items` populated from the previous query, and starts a 200 ms timer before `suggest()` fires.
- During that window the listbox still shows the old results. `ArrowDown` then `Enter` calls `choose()` on an item that does not match what the user typed. `selected` is set and the preflight runs against the wrong project.
- The reverse also happens. When the `suggest()` response lands (lines 120 and 125) it sets `self.active = -1`, wiping out arrow-key navigation the user just made. The next `Enter` is a silent no-op.
- `scripts/dashboard-keys.mjs:136-137` hides this by sleeping 300 ms before any key press.

**Fix:** Mark results stale on input and ignore navigation and `Enter` until fresh results arrive. Preserve the highlighted item by id across a refresh:
```js
onQueryInput: function () { ...; this.items = []; this._pending = true; ... }
// in suggest().then: this._pending = false; keep active by id
// in onQueryKey: if (this._pending && (key === 'Enter' || key === 'ArrowDown' || key === 'ArrowUp')) { event.preventDefault(); return; }
```
Add a test to `dashboard-keys.mjs` that types and presses `ArrowDown` and `Enter` immediately, asserting no selection of a stale item.

### WR-05: The "at most one personal-profile run" invariant (E-06) is checked and then reserved non-atomically

**File:** `agent-brain/src/dashboard/jobs.rs:121-140`, `agent-brain/src/dashboard/api_tasks.rs:173-199`

**Issue:** `busy_reason()` checks personal-profile exclusivity and runs before `JobManager::start`. `start()` itself only rejects a duplicate project (`reservations.contains_key(&spec.project)`). Two concurrent requests for two different personal projects, for example two tabs or a double POST, both pass the pre-check, because neither reservation exists yet. Both then pass `start()`, and two `personal` runs execute in parallel. That breaks the E-06 rule the README promises ("tối đa 1 tác vụ ... personal") and the shared `~/.claude-orca` config directory it protects.

**Fix:** Enforce it inside the same critical section as the reservation:
```rust
if spec.profile == "personal" && state.reservations.values().any(|p| p == "personal") {
    return Err(BrainError::error("Profile personal đang có việc vừa giao từ dashboard — không chạy song song ... (E-06)"));
}
```
Map the message to the existing 409 `kind: "busy"`. Add a concurrency test that starts two personal projects at once.

### WR-06: Running dashboard jobs are matched to a task by branch name only, ignoring the project

**File:** `agent-brain/src/dashboard/api_tasks.rs:244-246`, `agent-brain/src/dashboard/api_tasks.rs:986-989`

**Issue:** `busy_reason_for_task` and `get_task` both pick the "this task's job" with `j.task_id == Some(task_id) || j.branch == Some(branch)`. The slug is user-derived and the same slug in two projects is plausible (for example "sua-readme"). A running job in project A with branch `task/sua-readme` is then treated as the job of project B's task `task/sua-readme`. Effects:
- The task detail page shows A's job (`job`) for B's task.
- In the personal-profile branch (lines 287-296) the reservation of A is skipped via `tj.project == p`, so the E-06 guard is bypassed for B.

**Fix:** Require the project to match as well: `j.project == project.id && (j.task_id == Some(task_id) || j.branch.as_deref() == Some(branch))`. Apply the same change at line 986, using the task's project slug.

### WR-07: `POST /api/projects/scan` is not serialised with alias writes, has no timeout, and is not tied to the server lifetime

**File:** `agent-brain/src/dashboard/api_projects.rs:363-399` (versus `192-194`, `248`, `299`)

**Issue:**
- **Lost update.** `update_aliases` holds `WRITE_LOCK` across etag check, `load_map`, mutate and `save_map`. The scan child rewrites `projects_map.json` without taking that lock. A scan that completes between `update_aliases`' `load_map` and `save_map` is silently overwritten. Newly discovered projects vanish, and the etag check cannot detect it because the read already happened.
- **Concurrency.** Nothing prevents two scans from running concurrently.
- **No timeout.** `cmd.output()` blocks a blocking-pool thread with no timeout, so a hung scan occupies it indefinitely.
- **Not tied to the server.** The child is not in a process group and has no PDEATHSIG. A Ctrl+C on `serve` neither waits for it nor kills it.

**Fix:** Take `write_mutex()` around the scan and the map reload, or run scan as a `JobManager` job with the project reservation semantics. Add a deadline (spawn, poll `try_wait`, then kill via `procs::kill_tree`). Apply `process_group(0)` and PDEATHSIG as `jobs.rs` does.

### WR-08: Job output reader threads stop at the first non-UTF-8 line, leaving the child writing to a closed pipe

**File:** `agent-brain/src/dashboard/jobs.rs:230`, `agent-brain/src/dashboard/jobs.rs:243`

**Issue:** `reader.lines().map_while(Result::ok)` ends the reader on the first `Err` (invalid UTF-8 from `BufRead::lines`). The pipe read end is then dropped while the child (`agent-brain run`, which relays git and claude output) keeps writing. The child gets EPIPE: Rust ignores SIGPIPE, so `println!`/`write!` returns an error or panics, and in either case the run is degraded or dies. Separately, a single very long line with no newline is buffered without bound.

**Fix:** Read raw bytes and decode lossily, with a cap:
```rust
let mut r = std::io::BufReader::new(stdout);
let mut buf = Vec::new();
loop {
    buf.clear();
    match r.read_until(b'\n', &mut buf) { Ok(0) | Err(_) => break, Ok(_) => {} }
    buf.truncate(8192);
    let line = String::from_utf8_lossy(&buf).trim_end().to_string();
    /* push into ring */
}
```
Keep draining until EOF regardless of decode problems.

### WR-09: Router navigation has no ordering guard, so a slow earlier navigation can overwrite the screen of a later one

**File:** `agent-brain/assets/dashboard/js/app.js:286-338`

**Issue:** `navigate()` is `loadScript(name)` then `fetch(screen html)` then `innerHTML = html`, with no sequence check. Navigate to a not-yet-loaded screen (script fetch plus HTML fetch), then immediately to an already-loaded one (HTML fetch only). The second finishes first, then the first overwrites it. The URL, `AB.route`, nav highlight and title say screen B while screen A is shown. A's `init` also registers its `setInterval` into the already-reset `AB._timers` list. Those intervals update a destroyed component until the next navigation. The same shape exists in the catch branch.

**Fix:**
```js
var navSeq = 0;
function navigate(isHashChange) {
  var mySeq = ++navSeq; ...
  .then(function (html) { if (mySeq !== navSeq) return; ... })
  .catch(function () { if (mySeq !== navSeq) return; ... });
}
```
Also coalesce concurrent `loadScript` calls for the same name by caching the promise rather than a boolean.

### WR-10: Raw-JSON dialog response is applied without checking that the dialog is still the same one

**File:** `agent-brain/assets/dashboard/js/task.js:192-209`

**Issue:** `openRaw` captures `self` and then writes `self.raw.text` and `self.raw.error` in the async callback.
- If the user closes the dialog (`closeRaw` sets `this.raw = null`) before the response arrives, `self.raw.text = ...` throws `TypeError: Cannot set properties of null`, an unhandled promise rejection.
- If the user closes it and opens another row before the first response arrives, the late response writes the first row's JSON into the second row's dialog, showing the wrong data under the right title.

**Fix:** Capture the dialog object and verify identity:
```js
var mine = { seq: r.seq, text: '', error: '' }; this.raw = mine;
...then(function (res) { if (self.raw !== mine) return; mine.text = ...; });
```

### WR-11: `POST /api/runs/{id}/kill` validates run X but `cmd_kill` kills whichever run is currently running for the task

**File:** `agent-brain/src/dashboard/api_runs.rs:61-117`

**Issue:** The handler checks that run `id` is `running` and has a task, then calls `cmd_kill(&mut ctx, task_id)`. `cmd_kill` re-queries `running_run_for_task(task_id)` and kills that. If run X finishes in the window between the check and the kill, and a new run for the same task starts (for example the user starts Dev right after Lead ends), the click on "Kill run X" terminates the new run. The `run_id` in the URL is validated but never used to select the victim. The kill dialog only mitigates this a little, because the 3 s poll can show stale state.

**Fix:** After `cmd_kill` resolves its target, verify identity. The simplest option is to add a `run_id` parameter to `cmd_kill` (or a `cmd_kill_run`) that returns an error if `running_run_for_task(task_id).id != run_id`. Any new kill entry point must still go through `runner`, as the plan requires.

### WR-12: ~100 lines of task-state derivation and two error-mapping blocks are duplicated, so the enabled gate can drift from what the UI shows

**File:** `agent-brain/src/dashboard/api_tasks.rs:762-1035` versus `1037-1293`; job-start error mapping at `592-619` versus `1257-1285`

**Issue:** `get_task` and `post_task_action` repeat the same sequence: project lookup, `current_branch`, dirty set, plan read, plan commit, `code_after_plan`, plan parse, `GitFacts`, `busy_reason_for_task`, latest runs per role and `derive`. The enabled/disabled decision for the POST must be identical to the one the UI rendered. Any future edit to one copy changes behaviour silently. The `jobs.start` error to HTTP mapping is likewise pasted twice. The seven-line `(StatusCode, [(CONTENT_TYPE, ...)], Json(..))` response tuple is repeated about 40 times even though `json_error` and `Refusal` exist.

**Fix:** Extract `fn load_task_view(conn, jobs, st, task) -> BrainResult<(Project, GitFacts, TaskView, Option<PlanObj>, Option<QaObj>)>` and `fn start_job_response(jobs, spec, extra) -> Response`, and add a `json_ok(status, value)` helper. Both handlers then share one derivation.

### WR-13: 500 responses echo raw internal error strings (SQLite, I/O, git, absolute paths) to the client

**File:** `agent-brain/src/dashboard/api_tasks.rs:494`, `626`, `758`, `1033`, `1291`, `1370`; `api_runs.rs:57`, `171`, `219`; `api_projects.rs:137`, `182`, `359`, `403`, `519`; `api_stats.rs:75`

**Issue:** Every handler maps `Err(e)` to `json_error(500, &e.message)`. `BrainError` messages come straight from `rusqlite`, `std::io` and git stderr, so they can contain SQL fragments, schema names and absolute paths. The frontend renders `res.data.error` verbatim via `AB.errorText`. It is authenticated and local, but combined with CR-01 it widens what a stolen cookie reveals. A 4xx `Refusal` carrying user-meaningful Vietnamese text is a different case and is fine.

**Fix:** Log the full error server-side (`term::emit`) and return a stable message with a short correlation id for non-user errors, for example `{"error":"Lỗi nội bộ (mã 7f3a)"}`. Keep the verbatim message only for the validated 4xx paths.

## Info

### IN-01: Dead and duplicate branches in `taskstate::derive`

**File:** `agent-brain/src/dashboard/taskstate.rs:324-341`

**Issue:** All three non-`attention` paths return `"waiting"`. `else if f.task.status == "planned" || f.task.status == "dev_done" { "waiting" } else { "waiting" }` appears twice. This is dead logic, and it suggests an unfinished distinction (for example `open` versus `planned`).

**Fix:** Collapse to a single `else { "waiting" }`, or implement the intended difference.

### IN-02: `x-for` loops use non-unique `:key` values

**File:** `agent-brain/assets/dashboard/screens/task.html:143` (`:key="err"`), `screens/projects.html:59` (`:key="a"`)

**Issue:** Plan validation errors can repeat (identical message for two rows), and hand-edited `projects_map.json` aliases may not be de-duplicated (only the `PUT` path de-duplicates). Duplicate keys make Alpine drop or mis-render items.

**Fix:** Use the index in the key: `:key="idx + ':' + err"`.

### IN-03: Pump logs on every failed tick and re-reads the baseline after any error

**File:** `agent-brain/src/dashboard/pump.rs:20-49`, `66-69`, `108-111`

**Issue:** If the DB cannot be opened or a query fails persistently, the pump emits an error line every 250 ms (about 4 lines per second) with no backoff. After any `break` the outer loop recomputes `max_event_id`, so events written during the failure window never produce hints. SSE streams recover only through the 15 s fallback poll.

**Fix:** Add exponential backoff (cap about 5 s) and log only on the first failure and on recovery. Preserve `max_event_id` across reopen.

### IN-04: `Mutex::lock().unwrap()` makes one panic permanent

**File:** `agent-brain/src/dashboard/api_projects.rs:192`, `agent-brain/src/dashboard/jobs.rs:123`, `164`, `211`, `287`, `301`, `312`, `329`, `338`

**Issue:** A panic while a guard is held poisons the mutex. Every later `unwrap()` panics too, so alias saves return 500 forever, and any API call touching `jobs` fails until restart.

**Fix:** Use `lock().unwrap_or_else(|e| e.into_inner())`, since the guarded data here has no cross-field invariants a panic could leave half-updated.

### IN-05: `cfg.kill_grace_secs + 5` is unchecked

**File:** `agent-brain/src/dashboard/mod.rs:259`

**Issue:** `kill_grace_secs` is read from user config as any `u64`. A huge value overflows, panicking in debug and wrapping to a tiny grace period in release.

**Fix:** `cfg.kill_grace_secs.saturating_add(5)`, and optionally clamp the config value to a sane maximum.

### IN-06: Git commands take a branch or commit as a bare argument and the ref can be ambiguous with a path

**File:** `agent-brain/src/dashboard/api_tasks.rs:820` (`git log --format=%H %s <branch>`), `agent-brain/src/dashboard/api_projects.rs:493` (`git diff --stat <base_commit>`)

**Issue:** Without `--`, git treats an argument matching both a ref and a worktree path as ambiguous and fails. A repo containing a `task/<slug>` path would break the plan-commit lookup silently (the error is swallowed to `None`). The values come from the DB, not directly from the request, so this is not an injection, but it is a robustness gap.

**Fix:** Append `--`: `["log", "--format=%H %s", &task.branch, "--"]` and `["diff", "--stat", &base_commit, "--"]`.

### IN-07: Static route names shadow project ids

**File:** `agent-brain/src/dashboard/api_projects.rs:56-63`

**Issue:** A project with id `scan` or `suggest` can never be edited or inspected: `PUT /api/projects/scan` and `PUT /api/projects/suggest` match the static routes, which only allow POST and GET respectively, so they return 405.

**Fix:** Reject those two ids in `projects add` and scan, or move the static routes under `/api/project-tools/...`.

### IN-08: Smoke and key-check script robustness

**File:** `scripts/dashboard-smoke.sh:57-80` and `~164`, `scripts/dashboard-keys.mjs:16-21`, `80-100`

**Issue:**
- `trap cleanup EXIT INT TERM` with `local exit_code=$?` then `exit "$exit_code"`: on INT or TERM the trap sees the status of the last foreground command (often 0), so an interrupted run can exit 0. Cleanup also runs twice (the signal trap plus the EXIT trap).
- `chmod 755 tests/fakes/fake_claude.py` mutates a tracked file during a test run.
- `dashboard-keys.mjs` has no `ws.onclose` or `onerror` after connect and no overall deadline. If Chrome dies mid-run the pending promises never settle and the script hangs.

**Fix:**
- Use `trap 'exit 130' INT; trap 'exit 143' TERM; trap cleanup EXIT`.
- Drop the `chmod`, or check the mode in the repo.
- In `dashboard-keys.mjs`, add `ws.onclose = () => rejectAll(new Error('CDP closed'))` and wrap `main()` in a 60 s timeout.

### IN-09: Reservation leaks if the spawner thread dies

**File:** `agent-brain/src/dashboard/jobs.rs:157-159`

**Issue:** `reply_rx.recv().map_err(...)?` returns without `state.reservations.remove(&spec.project)`, so the project stays reserved until restart. Only reachable if the `ab-spawner` thread panics, but the error branch at 164 does release correctly, so this one is inconsistent.

**Fix:** Release the reservation before returning on the `recv` error, or use an RAII guard for the reservation.

### IN-10: Dead `cfg(not(unix))` branches and an unused field in `lock.rs`

**File:** `agent-brain/src/dashboard/lock.rs:10-11`, `37-40`, `112`

**Issue:** The crate cannot compile on non-Unix (`mod.rs` uses `tokio::signal::unix` and `libc::prctl` unconditionally, `jobs.rs` uses `libc` unconditionally), so the `cfg(not(unix))` branches are dead. They would also acquire no lock at all. `ServeLock.path` is carried under `#[allow(dead_code)]`.

**Fix:** Remove the non-Unix branches (or `compile_error!` on non-Unix), and delete the unused `path` field.

### IN-11: `GET /api/jobs/{id}` reports `run_id` as the task's latest run, which can be a previous run, and nothing consumes it

**File:** `agent-brain/src/dashboard/api_tasks.rs:1344-1349`

**Issue:** Right after a Dev, QA or re-Lead action the new run row does not exist yet. `latest_run_for_task` then returns the previous run, so the field is wrong for the first moments of every job. `assign.js` and `task.js` ignore `run_id`, so it is currently harmless, but it is a trap for the next consumer.

**Fix:** Drop the field, or only return runs with `started_at >= job.started_at`.

### IN-12: Test `d10_config_port_used_without_flag` has a port TOCTOU

**File:** `agent-brain/tests/dashboard_http.rs:55-58`

**Issue:** It binds `127.0.0.1:0`, reads the port, drops the listener, then asks `serve` to use that port. Another process (or a parallel test) can claim the port in between, so the test can fail intermittently. The suite is documented as `--test-threads=1`, which reduces but does not remove the risk.

**Fix:** Retry with a new port on `AddrInUse`, or assert on the config-to-port plumbing without binding (unit-test `config_from_value` plus the `port_flag.unwrap_or(cfg.dashboard_port)` logic).

---

_Reviewed: 2026-10-19_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
