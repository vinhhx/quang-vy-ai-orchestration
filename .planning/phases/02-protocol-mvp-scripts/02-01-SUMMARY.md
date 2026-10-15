---
phase: 02-protocol-mvp-scripts
plan: 01
subsystem: cli
tags: [python, stdlib, argparse, subprocess, git, stream-json, fake-claude]

requires:
  - phase: 01-verify-assumptions
    provides: "SPK-10 profile mapping (work = ~/.claude with CLAUDE_CONFIG_DIR unset, personal = ~/.claude-orca), S1 stream-json needs --verbose"
provides:
  - "brain/ package: cli dispatch, config/profile env, routing, project map, gitops, shared role runner"
  - "`brain lead` tracer path end to end against the fake claude"
  - "tests/fakes/fake_claude.py and tests/helpers.Sandbox for every later Phase 2 test"
affects: [02-02, 02-03, 02-04, 02-05, 02-06, 02-07, 03-rust-cli-core]

actuals:
  tasks: 1
  commits: 1

tech-stack:
  added: []
  patterns:
    - "argv lists only, no shell (subprocess without shell=True)"
    - "handlers resolved lazily from cli.HANDLERS 'module:function' strings"
    - "raw stream logs under XDG_DATA_HOME runs dir, 0o700, outside the repo"

key-files:
  created:
    - brain/__init__.py
    - brain/brain
    - brain/cli.py
    - brain/config.py
    - brain/config.json
    - brain/routing.py
    - brain/projects.py
    - brain/gitops.py
    - brain/agent.py
    - brain/role_lead.py
    - brain/prompts/lead.md
    - tests/__init__.py
    - tests/helpers.py
    - tests/fakes/fake_claude.py
    - tests/test_lead_e2e.py
  modified:
    - .gitignore

key-decisions:
  - "Executed by Antigravity (agy 1.3.5, model gemini-3.8-flash-high, --dangerously-skip-permissions approved by the user for Phase 2) after agy's Claude-model quota ran out; reviewed by Claude"

patterns-established:
  - "Sandbox test world: temp XDG dirs, BRAIN_CONFIG, GIT_CONFIG_GLOBAL, fake claude recording argv/env"

requirements-completed: []

duration: 11min
completed: 2026-10-15
status: complete
---

# Phase 2 Plan 01: Tracer `brain lead` Summary

**`brain lead "web bán hàng" "thêm nút đăng xuất"` routes the Vietnamese name, builds the work-profile env, branches `task/them-nut-dang-xuat`, runs Lead through the fake claude with the role's tool limits and commits only `.plan.md`.**

## Performance

- **Duration:** 11 min (agy run 667 s)
- **Started:** 2026-10-15 (after a first attempt on agy's default model failed at once: quota 429)
- **Completed:** 2026-10-15
- **Tasks:** 1
- **Files modified:** 16 (15 created, `.gitignore` modified)

## Accomplishments

- The spine every later plan builds on: `cli`, `config`, `routing`, `projects`, `gitops`, `agent`, with the exact interface signatures listed in 02-01-PLAN.md.
- `brain/config.json` already uses the SPK-10 mapping: work `claude_config_dir: null`, personal `~/.claude-orca`.
- Role runner: stdin `/dev/null`, per-role timeout then SIGTERM then SIGKILL after 10 s, raw stream to a 0o700 runs dir outside the repo, permission-bypass flags rejected.
- Fake claude and `Sandbox` helpers, so tests never run the real `claude` or touch a real profile directory.

## Task Commits

1. **Task 1: Tracer — brain lead path end to end** - `ac5001e` (feat)

## Files Created/Modified

- `brain/brain` - executable entry point (mode 100755), symlink-safe repo root
- `brain/cli.py` - argparse subcommands scan/add/lead/dev/qa and `HANDLERS`
- `brain/config.py`, `brain/config.json` - config load, profile check, child env, run/log dirs
- `brain/routing.py` - `normalize`, `slugify`, `resolve` (D-08, D-09)
- `brain/projects.py` - project map load, `select_project` (Fallback Ask, exit 2), `require_runnable`
- `brain/gitops.py` - git helpers, argv lists only
- `brain/agent.py` - `RoleSpec`, `RunResult`, `build_command`, `execute_role`
- `brain/role_lead.py`, `brain/prompts/lead.md` - Lead command and the spec §4.3 prompt (verbatim)
- `tests/helpers.py`, `tests/fakes/fake_claude.py`, `tests/test_lead_e2e.py` - sandbox, fake, e2e tests
- `.gitignore` - adds `__pycache__/` and `*.pyc`; the four existing lines are kept

## Decisions Made

None beyond the plan.

## Deviations from Plan

- agy added a third test, `test_empty_map_exits_2`, which covers the plan's `<verification>` item (empty map → exit 2 with the hint). Additive, in a declared file.
- `.gitignore` had four existing lines rather than the two the plan mentions; all four were kept.

**Total deviations:** 2, both harmless.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- `python3 -m unittest discover -s tests -t . -v`: 3 tests, OK.
- `python3 brain/brain --help` lists scan, add, lead, dev, qa. `brain lead x y --dry-run` with an empty map exits 2 and prints the empty-map hint.
- All six acceptance-criteria commands pass. `brain/brain` and `tests/fakes/fake_claude.py` are committed as 100755.
- Scope: the commit's files equal `files_modified` exactly. No real home or profile paths in `brain/` or `tests/` other than the configured `~/.claude-orca`; no installs, no network code; no secrets.
- Code read against every step of the plan's `<action>`: interfaces and behaviour match.

Findings not covered by any test (to fix before plan 02-04):

1. **`projects.load_map` treats an unreadable map as empty.** `brain/projects.py:32-33` returns an empty map for invalid JSON or a non-object, not only for a missing file as the plan says. A typo in the hand-edited `projects_map.json` then reads as "(trống) — chạy: brain scan --root", and plan 02-04's `brain scan` saves the map, which could overwrite the user's hand-set `name_vi`, `aliases` and `profile`. It should raise `BrainError` (exit 1) naming the file.
2. **`cli.main` hides real import errors.** `brain/cli.py:79` catches every `ImportError`, so a broken import inside an existing handler module (for example a bug in `role_dev.py`) prints "chưa có trong bản cài này" instead of the error. Only a `ModuleNotFoundError` for the handler module itself should get that message.

Noted, no action now: the tracer test's `present_vars == []` check cannot fail, because `Sandbox.env` already strips the nested-session variables. Plan 02-04's profile tests (`extra_env`) are where `build_env`'s stripping is actually proven.

## Issues Encountered

- The first agy attempt failed in 22 s with `RESOURCE_EXHAUSTED (429): Individual quota reached` (resets about 2026-10-20 15:18 +07). agy's Claude models share that quota; the rerun used `--model gemini-3.8-flash-high`. No files were written by the failed attempt.

## User Setup Required

None.

## Next Phase Readiness

- Plan 02-02 (file contract) can start; it does not depend on the two findings.
- Findings 1 and 2: **resolved** in `5a46d0f` (tests) and `3eced24` (fix), via `.planning/fixes/02-01-review-fixes.plan.md`, executed by agy and reviewed by Claude: an unreadable map now exits 1 naming the file and leaves it unchanged; only a missing handler module/function gets the "chưa có" message.
- Requirements ROLE-01, ROLE-05, ROLE-06, ROUTE-01, ROUTE-03, ROUTE-04 and PROF-01 are only proven at tracer level here; they are not marked complete until plans 02-03 and 02-04 test them fully.
