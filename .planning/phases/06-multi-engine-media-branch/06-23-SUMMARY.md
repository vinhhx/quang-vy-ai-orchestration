---
phase: 06-multi-engine-media-branch
plan: 23
subsystem: engines
tags: [rust, agent-brain, dsh, docker, container-kill, eng-09]
requires:
  - phase: 06-multi-engine-media-branch
    provides: "EnginePlan.dsh, StreamFormat::Text, plan_for dispatch to dsh::plan (06-11); EnginePlan.redact and the run seam (06-21); user-run kill-wrapper probe slot in spikes/s6/NOTES.md (06-20)"
provides:
  - "agent_brain::dsh: MARKER_PREFIX, new_marker, is_valid_marker, wrapper_script, build_argv, same_path_bind_covers, preflight, ProcRow, parse_ps, marker_subtree, ps_snapshot, StopReport, stop_in_container, text_event, the filled plan()"
  - "runner::RunRequest.container_stop and one stop helper used by the four stop sites (SIGINT, SIGTERM, SIGHUP, timeout); cmd_kill stops the container first from runs.metrics_json.dsh"
  - "runs.metrics_json.dsh = {docker, container, marker} written at spawn; event kind dsh/text ({\"dsh\":\"text\",\"text\":...}); stream::classify dsh/<value>; term::event_text and sse::row_kind for dsh/text"
  - "tests/fakes/fake_docker.py (ps readiness status, inspect --format, exec detached from the client) and tests/fakes/fake_dsh.py"
affects: [06-24, 06-25, 06-26]

plan_head_before: 6cc211220a4f00836c2be14523ef0a181af8109e
plan_head_after: bc6da6a

actuals:
  tokens: 21000
  tasks: 2
  commits: 4

tech-stack:
  added: []
  patterns:
    - "prompt as positional $2 of a constant sh -c text; the only variable part of the shell text is the hex marker"
    - "kill inside the container by marker token plus recorded pids, re-matched by pid and the first 100 args characters before every signal; host S4 kill only afterwards"
    - "the fake container is honest about S6 (4a): the command runs detached from the docker client, so a host-only kill leaves it alive and only the marker stop removes it"

key-files:
  created:
    - tests/fakes/fake_docker.py
    - tests/fakes/fake_dsh.py
    - agent-brain/tests/engine_dsh.rs
  modified:
    - agent-brain/src/dsh.rs
    - agent-brain/src/runner.rs
    - agent-brain/src/roles/mod.rs
    - agent-brain/src/stream.rs
    - agent-brain/src/term.rs
    - agent-brain/src/dashboard/sse.rs
    - agent-brain/src/engine.rs

key-decisions:
  - "The mount preflight and the leftover check run with the project profile env minus every key_env, ANTHROPIC_*, CLAUDE_CONFIG_DIR, CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR and CLAUDE_CODE_SUBAGENT_MODEL; kill-time docker calls (another process, no run env) use the process env minus ANTHROPIC_*, those names and any *_KEY / *_TOKEN / *_SECRET"
  - "stop_in_container and cmd_kill refuse a marker that is not exactly ab-dsh- plus 12 lowercase hex (an empty marker would match everything)"
  - "zombie rows (args ending <defunct>) and pid 1 are never selected; if the container cannot be listed mid-stop no further signal is sent and the recorded pids are reported as survivors"
  - "blank stdout lines of dsh are not turned into events (stream.jsonl still keeps them raw)"

patterns-established:
  - "Tests spawn the long-running agent-brain child with stdout and stderr in files so cargo's test report lines stay intact"

requirements-completed: [ENG-09]

coverage:
  - id: D1
    description: "a dsh Dev run spawns exactly the wrapper argv, prompt only as positional $2, fresh marker, stdin /dev/null; the shell text is byte-identical to the 06-20 probe's"
    requirement: ENG-09
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_argv_and_stdin"
        status: pass
      - kind: unit
        ref: "agent-brain/src/dsh.rs#dsh::tests::wrapper_exact"
        status: pass
    human_judgment: false
  - id: D2
    description: "text-only black box: dsh/text events, raw stream.jsonl, usage none, tokens 0, cost 0, model and header = the dsh profile, Dev commit"
    requirement: ENG-09
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_dev_tracer"
        status: pass
      - kind: unit
        ref: "agent-brain/src/stream.rs#stream::tests::dsh_text_events"
        status: pass
    human_judgment: false
  - id: D3
    description: "the docker client env has no key_env, ANTHROPIC_*, CLAUDE_CONFIG_DIR, CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR or CLAUDE_CODE_SUBAGENT_MODEL"
    requirement: ENG-09
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_env_has_no_keys"
        status: pass
    human_judgment: false
  - id: D4
    description: "dsh refuses every role but dev before any run row; the same-path read-write bind preflight refuses a project the container cannot see"
    requirement: ENG-09
    verification:
      - kind: unit
        ref: "agent-brain/src/dsh.rs#dsh::tests::plan_refuses_non_dev"
        status: pass
      - kind: unit
        ref: "agent-brain/src/dsh.rs#dsh::tests::mounts_rules"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_mount_preflight"
        status: pass
    human_judgment: false
  - id: D5
    description: "agent-brain kill, the runner timeout and SIGINT each stop the run inside the container by marker and recorded pids; no marker process and no recorded pid survives"
    requirement: ENG-09
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_kill_inside_container"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_timeout_kills_inside_container"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_sigint_kills_inside_container"
        status: pass
      - kind: unit
        ref: "agent-brain/src/dsh.rs#dsh::tests::marker_subtree_rules"
        status: pass
    human_judgment: false
  - id: D6
    description: "a leftover ab-dsh- process of an earlier run is reported by the next run's preflight and never signalled"
    requirement: ENG-09
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#dsh_leftover_warns"
        status: pass
    human_judgment: false
  - id: D7
    description: "the fake docker answers the 06-26 readiness probe (ps --filter --format) with FAKE_DOCKER_STATUS and rejects ps without --format"
    requirement: ENG-09
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_dsh.rs#fake_docker_ps_status"
        status: pass
    human_judgment: false
  - id: D8
    description: "On the REAL image `sh -c` keeps the marker-carrying sh as the parent for the whole run, and files written by the real dsh are root-owned (assumptions A18, S6 ownership)"
    verification: []
    human_judgment: true
    rationale: "Only the user-run kill-wrapper probe of plan 06-20 and live check P6-09 see the real container; spikes/s6/NOTES.md still reads `not-run`, so A18 is unproven"

duration: 30min
completed: 2026-10-28
status: complete
---

# Phase 6 Plan 23: dsh as a black-box Dev engine, stopped inside the container Summary

**A Dev task on a personal project runs on `dsh` through `docker exec` with a constant `sh -c` wrapper (prompt only as positional `$2`, fresh `ab-dsh-<12 hex>` marker), text-only `dsh/text` events and no tokens, only on a project the container sees at the same path; every stop path (`agent-brain kill`, timeout, SIGINT/SIGTERM/SIGHUP) first stops the run's processes inside the container by marker and recorded pids, then does the host S4 kill.**

## Performance

- **Duration:** ~30 min of execution after a warm read of the plan (a cold `cargo` build ran in the background)
- **Started:** 2026-10-28T03:55Z (approximate; first commit 04:09Z)
- **Completed:** 2026-10-28T04:30Z
- **Tasks:** 2 (1 tracer, 1 auto; both `tdd="true"`)
- **Files modified:** 10 (3 created)

## Accomplishments

- `dsh.rs`: `plan` refuses non-dev (D-37 text) before anything else, runs `preflight` (`docker inspect --format {{json .Mounts}} <container>`, 5 s cap, same-path read-write bind covering the project root by path component), warns about leftover `ab-dsh-` processes, then builds `[docker, exec, -w, <root>, <container>, sh, -c, ": <marker>; dsh --profile \"$1\" \"$2\"; rc=$?; exit $rc", sh, <profile>, <prompt>]` with the prompt built like agy's (`{prompt}\n\n---\n{input}`). `EnginePlan`: `StreamFormat::Text`, model label = dsh profile, `redact` empty, `dsh = Some({docker, container, marker})`.
- Text path: `roles/mod.rs` wraps each non-blank stdout line with `dsh::text_event` before the existing feed / `insert_event` / terminal path; `stream::classify` maps `{"dsh": X}` to kind `dsh/X`; `Summary::feed` keeps the last 8 KiB of dsh text in `result_text`; `term::event_text` prints the text; `sse::row_kind("dsh/text")` is `assistant`. `metrics_json.dsh` is merged in `on_spawn`.
- Container stop: `ps_snapshot` (`exec <c> ps -ww -eo pid=,ppid=,pgid=,args=`), `marker_subtree` (token-equal marker, descendants by ppid, `before` rows with same pid and first 100 args characters), `stop_in_container` (TERM the recorded pids, poll every 200 ms up to `kill_grace` re-matching, KILL the re-matched, final snapshot, `StopReport`).
- `runner.rs`: `RunRequest.container_stop`; `spawn_stop` replaces the four duplicated blocks (container stop, then `procs::kill_tree`, inside one `spawn_blocking`); `cmd_kill` parses `metrics_json.dsh` (validated), stops the container first and prints `Đã dừng {n} tiến trình trong container {c}` plus survivors by pid.
- Fakes: `fake_docker.py` runs `exec` commands detached from the client (own session, re-parented) so a host-only kill leaves them alive as S6 (4a) observed; `ps` (readiness probe of 06-26), `inspect --format`, `exec ps` / `exec kill` are inline. `fake_dsh.py` is scenario driven (lines, write ops, stderr, sleep, spawn_child, pidfile, exit_code) and records `stdin_is_null`.

## Task Commits

1. **Task 1 (tracer): a Dev task runs on dsh end to end**
   - RED `0405ed1` (test): fakes plus `dsh_dev_tracer`, `dsh_argv_and_stdin`, `dsh_env_has_no_keys`, `dsh_mount_preflight`, `fake_docker_ps_status`
   - GREEN `3cd994a` (feat)
2. **Task 2: stop inside the container**
   - RED `c7673a1` (test): `dsh_kill_inside_container`, `dsh_timeout_kills_inside_container`, `dsh_sigint_kills_inside_container`, `dsh_leftover_warns`
   - GREEN `bc6da6a` (feat)

**Plan metadata:** the `docs(06-23)` commit that adds this file (and the deferred-items note).

Kill-wrapper result recorded in `spikes/s6/NOTES.md`: `not-run` (Task 1's precondition accepts `ok` or `not-run`; A18 stays unproven until the user-run probe or live check P6-09).

## TDD Gate Compliance

`test(06-23)` precedes `feat(06-23)` for both tasks.

- **RED, task 1:** against the 06-11 stub the first run failed 8 of 9 tests with `Engine dsh chưa được bật trong bản agent-brain này` (run dev exit 1) or `fake dsh wrote no pidfile`; `fake_docker_ps_status` passed because it tests the fake alone. Evidence is cargo's plain-text report (`gsd_run check tdd-red-evidence` does not support it), assessed by inspection.
- **RED, task 2:** with Task 1's code and a no-op leftover check, `dsh_timeout_kills_inside_container` and `dsh_sigint_kills_inside_container` failed on `marker processes [..]; pid .. alive true; pid .. alive true`, the planned assertion (the host kill ended only the docker client; the container side survived, exactly S6 4a), and `dsh_kill_inside_container` failed on the container-report assertion; `dsh_leftover_warns` failed on the missing warning.
- The two RED commits were cut by splitting the test file after the fact (the files were written once, then committed in the order test, feat, test, feat); the unit tests of `dsh.rs` were added with the GREEN commits (a unit test of a missing function cannot compile).

## Files Created/Modified

- `agent-brain/src/dsh.rs` - everything in `<interfaces>` plus unit tests `wrapper_exact`, `mounts_rules`, `plan_refuses_non_dev`, `parse_ps_rows`, `marker_subtree_rules`, `stop_ignores_invalid_marker`, `text_event_is_json_text`
- `agent-brain/src/runner.rs` - `container_stop`, `StopResult`, `spawn_stop`, `dsh_run_of`, container stop in `cmd_kill`
- `agent-brain/src/roles/mod.rs` - Observer `stream` and `dsh`, text wrapping, `metrics_json.dsh`, `container_stop: ep.dsh.clone()`
- `agent-brain/src/stream.rs`, `agent-brain/src/term.rs`, `agent-brain/src/dashboard/sse.rs` - `dsh/text`
- `agent-brain/src/engine.rs` - test only (see deviations)
- `tests/fakes/fake_docker.py`, `tests/fakes/fake_dsh.py` - new, executable (mode 100755)
- `agent-brain/tests/engine_dsh.rs` - 9 integration tests (new)

## Decisions Made

See `key-decisions` in the frontmatter. In short: env hygiene for every docker call, strict marker validation at every signal site, zombies and pid 1 never selected, no signal when the container cannot be listed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The engine.rs unit test that asserted the stub refusal could no longer pass**
- **Found during:** Task 1
- **Issue:** `engine::tests::unknown_id_and_dsh_stub_are_refused` ended by running a dsh `dev` plan and expecting `Engine dsh chưa được bật ...`. With the stub replaced, that call would run the preflight against the default `docker` command of the test config, i.e. a real docker CLI, which the plan forbids.
- **Fix:** the last assertion now plans the `lead` role and expects the D-37 refusal (returned before any docker call); the test was renamed `unknown_id_and_dsh_non_dev_are_refused`. Test-only edit in `engine.rs`, no production line changed there (the plan's "do not edit engine.rs" targets the dispatch, which is untouched).
- **Files modified:** `agent-brain/src/engine.rs`
- **Commit:** `3cd994a`

**2. [Plan wording] "the first `exec` record" is the run exec, which is the third docker call**
- **Issue:** the plan asks the leftover check to take a process snapshot, which is also a `docker exec`, so the run exec is the second `exec` record, not the first. `dsh_argv_and_stdin` asserts the whole order instead: `inspect`, `exec ... ps`, then the run exec with the planned 10-element argv.
- **Files modified:** `agent-brain/tests/engine_dsh.rs`
- **Commit:** `bc6da6a`

**3. [Rule 2 - Missing critical] Extra safety around signals (beyond the plan text)**
- `is_valid_marker` guards `stop_in_container` and `cmd_kill` (an empty or odd marker would match unrelated processes); zombie rows and pid 1 are never selected; if the container cannot be listed during a stop nothing more is signalled; kill-time docker calls use a scrubbed copy of the process env.
- **Files modified:** `agent-brain/src/dsh.rs`, `agent-brain/src/runner.rs`
- **Commit:** `bc6da6a`

**4. [Plan scope] Test and fake details**
- `fake_docker.py` hides its own processes from the relayed `exec ps` (a real container cannot see host-side docker clients; without this the client, whose command line also carries the marker, would be signalled) and reports the exec exit code through a pipe rather than a status file (same contract, no temp file to leak when the client is killed).
- The long-running `run dev` children of the kill and SIGINT tests are spawned with stdout and stderr in files (`spawn_dev`) so the harness lines `test dsh_kill_inside_container ... ok` stay intact for the plan's `fails_when` check.
- `dsh_leftover_warns` starts `sh -c ': ab-dsh-000000000000; sleep 30; exit 0'` (a trailing command so dash does not replace itself with `sleep` and drop the marker from its command line) and ends it by recorded pids.
- Acceptance check of Task 2, `awk '/"(restart|stop|start|rm)"/ ...'`, was refused by this environment's command filter; the equivalent `grep -nE '"(restart|stop|start|rm)"' agent-brain/src/dsh.rs` finds nothing.

**Total deviations:** 4 (1 Rule 3, 1 Rule 2, 2 plan notes). **Impact:** none on the plan's contract.

## Issues Encountered

- **Pre-existing, out of scope:** `cargo fmt --manifest-path agent-brain/Cargo.toml --check` fails on `agent-brain/tests/e09_fixtures.rs` (one `println!` line) at the merge base `6cc2112`. It is the only file reported; every file of this plan is formatted. Logged in `.planning/phases/06-multi-engine-media-branch/deferred-items.md`; a plain `cargo fmt` fixes it.
- No real docker was ever started: every cargo run went through a wrapper that put a trap `docker` first on `PATH` (it logs and exits 97); the log stayed empty for the whole full-suite run.
- The `contract_guard::guard_contract_cases` BrokenPipe flake did not occur.

## Verification Results

| Command | Result |
|---|---|
| `cargo test ... --test engine_dsh -- --test-threads=1` | 9 passed, 0 failed (all of `dsh_dev_tracer`, `dsh_argv_and_stdin`, `fake_docker_ps_status`, `dsh_kill_inside_container`, `dsh_timeout_kills_inside_container`, `dsh_sigint_kills_inside_container` print `... ok`) |
| `cargo test ... --lib -- dsh:: stream:: --test-threads=1` | 18 passed (`wrapper_exact ... ok`) |
| `cargo test ... --test engine_agy --test engine_routing --test cli_stream --test contract_stream --test dashboard_stream` | all ok (13, 22, 4, 2, 4) |
| `cargo test ... -- --test-threads=1` (full suite) | 63 result lines, all `ok`; 1174 tests passed, 0 failed |
| `cargo fmt --check` | only the pre-existing `tests/e09_fixtures.rs` diff (see Issues); this plan's files clean |
| `git diff --exit-code 574b6cd -- agent-brain/Cargo.toml agent-brain/Cargo.lock` | exit 0 |
| `bash spikes/lib/scan-secrets.sh` | `scan-secrets: OK` |
| acceptance greps | `rc=$?; exit $rc` count 2; fakes executable; inspect awk exit 0; `stop_in_container` in runner.rs 2, one `pub container_stop`; no `pkill`/`killall` anywhere; no container lifecycle literal |

## Known Stubs

None.

## Known limits (stated, not hidden)

- A18 (the marker-carrying `sh` stays the parent of `dsh` on the real image) is unproven: the user-run kill-wrapper probe is `not-run`. If it later reads `not-ok`, `marker_subtree` still catches re-parented children through the `before` rows, but the plan's wrapper must go back to the planner (06-20 Task 4).
- SIGKILL of agent-brain itself (dashboard `kill_all_now`) skips the container stop; the next dsh run's preflight warns about the leftover marker processes and never kills them.
- dsh's exit code and stderr on failure are unobserved (A25): the run status comes from the docker client's exit code; text-mode quota detection is plan 06-25.
- Files written by the real dsh are root-owned (S6); documented by 06-19, checked in P6-09.

## Threat Flags

None beyond the plan's register. Mitigations: T-06-85 (`wrapper_exact`, `dsh_argv_and_stdin`), T-06-86 (`dsh_kill_inside_container`, `dsh_timeout_kills_inside_container`, `dsh_sigint_kills_inside_container`), T-06-87 (token-exact `marker_subtree_rules`, `stop_ignores_invalid_marker`, greps), T-06-88 (D-37 `plan_refuses_non_dev`, profile guard from 06-11/06-22), T-06-89 (`INSPECT_MOUNTS_ARGS`, the fake refuses inspect without `--format`), T-06-90 (`dsh_mount_preflight`), T-06-SC (no dependency added, Cargo files unchanged).

## User Setup Required

None for this plan. Live check P6-09 (real container, A18, root-owned files) and the 06-20 `--kill-wrapper` probe stay with the user.

## Next Phase Readiness

- 06-25 can wire `dsh:empty`, the autopilot and task screen, and text-mode quota on top of `StreamFormat::Text`, `dsh/text` and `metrics_json.dsh`.
- 06-19's live check for dsh `san_sang` can use `fake_docker.py`'s `ps` answer.
- No blockers.

## Self-Check: PASSED

- Files present: `agent-brain/src/dsh.rs`, `agent-brain/src/runner.rs`, `agent-brain/tests/engine_dsh.rs`, `tests/fakes/fake_docker.py`, `tests/fakes/fake_dsh.py` (both mode 100755), this SUMMARY.
- Commits in history (`git log 6cc2112..HEAD`): `0405ed1`, `3cd994a`, `c7673a1`, `bc6da6a`.
- Verify commands re-run after the last code commit (`bc6da6a`): `engine_dsh` 9/9, lib `dsh:: stream::` 18/18, regression set ok, full suite all ok, Cargo files identical to `574b6cd`, `scan-secrets: OK`; `cargo fmt --check` clean apart from the pre-existing `e09_fixtures.rs`.
- No stray `ab-dsh-` / `fake_dsh` / `fake_docker` processes left after the runs; Cargo.toml and Cargo.lock untouched; `.planning/STATE.md` and `ROADMAP.md` untouched.
