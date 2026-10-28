---
phase: 06-multi-engine-media-branch
plan: 19
subsystem: verification
tags: [live-check, schema-dry-run, docs, engines, dsh, claude-max, eng-01, eng-11]
status: partial — Tasks 1, 2 done; Task 3 (user checkpoint, schema v4 dry run on the real brain.db) open
requires:
  - phase: 06-multi-engine-media-branch
    provides: "engine list, teams, quota stop and re-run, dsh, claude-max guard, readiness (06-11 .. 06-26); fake claude, fake agy, fake docker, fake dsh; E-09 and kill-wrapper probe slots (06-20)"
provides:
  - "scripts/phase6-live-check.sh: automated isolated check (real binary, fake engines, engine-list config), --keep, --schema-dry-run"
  - "agent-brain/README.md: Phase 6 upgrade note, CLI, exit codes 5 and 6, engine list config, teams.json / staff.json, dsh blast radius, seven screens, new routes, Telegram quota, new tests, troubleshooting"
  - "agent-brain/LIVE-CHECKS.md section 8: P6-00 .. P6-10 and cleanup"
affects: [phase-6-uat, verify-work]

plan_head_before: 5ca5a050cae1cb72c29e6c471423cd7126f0d218
plan_head_after: 446d0cb4045f60e4b359992b804c19bb20ce37da

actuals:
  tokens: 22000
  tasks: 2
  commits: 3

tech-stack:
  added: []
  patterns:
    - "one isolated check: private HOME / XDG / AGENT_BRAIN_* under mktemp, a trap `docker` first on the serve PATH, planted keys only in the environment, stop by recorded PID"
    - "dry-run of a one-way migration on a `cp -p` copy, real directory listing and hashes compared before and after"

key-files:
  created:
    - scripts/phase6-live-check.sh
  modified:
    - agent-brain/README.md
    - agent-brain/LIVE-CHECKS.md
    - .planning/phases/06-multi-engine-media-branch/deferred-items.md

key-decisions:
  - "The run engines, `usage_source` and the personal-profile check read the script's OWN temporary brain.db (python sqlite3, mode=ro) and the fake-claude record, because GET /api/tasks/{id} has no per-run engine field"
  - "The dry run keeps the file names of the copied database (`$TMP/copy/<name>`, `-wal`/`-shm` beside it) and ignores the `..` line of the directory listing when comparing before and after (the parent directory is not the data directory)"
  - "--keep adds one more task left on quota (fifth Dev queue entry succeeds) so P6-03 and P6-07 have a real banner to look at and re-run"

requirements-completed: []

coverage:
  - id: D1
    description: "tiet-kiem task with Dev on a configured gateway id (D-43 override): Lead / Dev / QA run on claude / gw / claude, /api/stats has rows claude and gw, Dev gets only its own endpoint and key"
    requirement: ENG-01
    verification:
      - kind: live-script
        ref: "scripts/phase6-live-check.sh (LIVE OK: task 1 tiet-kiem ...; /api/stats; Dev nhận endpoint và key của gw)"
        status: pass
    human_judgment: false
  - id: D2
    description: "quota stop on the gateway Dev step, quota-rerun on mac-dinh continues on claude, one team_history entry, no run in between"
    requirement: ENG-02
    verification:
      - kind: live-script
        ref: "scripts/phase6-live-check.sh (LIVE OK: task 2 ...)"
        status: pass
    human_judgment: false
  - id: D3
    description: "dsh Dev on a personal project through the fake docker: engine dsh, usage_source none, change committed, outcome not dsh:empty; the real docker is never called (trap log empty)"
    requirement: ENG-09
    verification:
      - kind: live-script
        ref: "scripts/phase6-live-check.sh (LIVE OK: task 3 ...)"
        status: pass
    human_judgment: false
  - id: D4
    description: "claude-max team refused with 422 on the work project and running on the personal profile directory on the personal project; readiness and order of GET /api/engines; two planted keys absent from every file under the temp dir"
    requirement: ENG-10
    verification:
      - kind: live-script
        ref: "scripts/phase6-live-check.sh (doi-max ...; /api/engines ...; key_absent[...]=yes)"
        status: pass
    human_judgment: false
  - id: D5
    description: "schema v4 dry run: user_version 4, integrity ok, counts unchanged, real directory unchanged; refuses a bogus v2 file with no tables"
    verification:
      - kind: live-script
        ref: "scripts/phase6-live-check.sh --schema-dry-run on a fresh v4 database, a v3 database with empty -wal/-shm files, and a bogus v2 file"
        status: pass
    human_judgment: true
    rationale: "Task 3: the user runs it on the REAL brain.db; this agent never touched it"
  - id: D6
    description: "real-engine checks P6-02 .. P6-07, P6-09, P6-10 (E-09 endpoint, agy, human engine, key sweep, Settings look, dsh black box, claude-max)"
    human_judgment: true
    rationale: "Need the user's keys, agy login, a second clone, the real container, and a browser; harvested into the end-of-phase UAT"

duration: 35min
completed: 2026-10-28
---

# Phase 6 Plan 19: Phase 6 live check, schema dry run, README and LIVE-CHECKS section 8 Summary

**`scripts/phase6-live-check.sh` runs the real binary against fake engines in a private temp environment and proves the engine list end to end (gateway Dev, quota stop and re-run, dsh via fake docker, claude-max guard, readiness, two-key sweep), its `--schema-dry-run` mode migrates a copy of a brain.db without touching the real directory, and README plus LIVE-CHECKS section 8 document the engine list, the dsh blast radius and the real-engine procedure. Task 3 (the dry run on the user's real database) and the real-engine checks are the user's.**

## Performance

- **Duration:** ~35 min (warm read of the plan and 06-20 .. 06-26 summaries, one cold debug build of 46 s, full suite ~15 min)
- **Completed:** 2026-10-28T06:30Z
- **Tasks:** 2 of 3 (Task 3 is a blocking-human checkpoint, not started by design)
- **Files modified:** 4 (1 created)

## Accomplishments

- `phase6-live-check.sh` automated mode: private `HOME`, `XDG_*`, `AGENT_BRAIN_DB`, `AGENT_BRAIN_CONFIG`; config with an `engines` array (`gw` anthropic with `verified`, `deepseek` without, `claude-max` with `pin_profile`, fake `agy`, `dsh` with `fake_docker.py`); teams.json overriding `tiet-kiem` with Dev `gw`, plus `doi-dsh` and `doi-max`; two scratch repos (`live-p6` work, `live-p6-ca-nhan` personal); cookie via `curl -sS -K -` on stdin; stop by recorded PID; a trap `docker` first on the serve PATH whose log must stay empty.
- Flows and assertions (all pass): task 1 runs `claude, gw, claude` and `/api/stats` has rows `claude` and `gw`; the gw Dev run got the gateway base URL and the planted token (boolean only) while Lead and QA saw neither base URL nor any key variable and Dev on gw saw no `DEEPSEEK_API_KEY`; task 2 stops on quota (`quota.engine == gw`, exit 5), `quota-rerun {team: mac-dinh, rev}` runs Dev on `claude` with consecutive run ids, one `team_history` entry `tiet-kiem -> mac-dinh` by `dashboard`; task 3 Dev on `dsh`, `usage_source none`, commit on the branch, outcome not `dsh:empty`; `doi-max` on the work project answers 422 with `chỉ dùng cho dự án profile personal`, on the personal project 202 and the Lead run record carries the sandbox personal `claude_config_dir` (the work-project Lead record has none); `/api/engines` order `claude, gw, deepseek, claude-max, agy, dsh, người` with `gw`, `claude-max`, `dsh` `san_sang` and `deepseek` `chua_kiem`.
- Key sweep over the whole temp dir plus a SQL dump of its brain.db and `git log -p` of both repos: prints `key_absent[PHASE6_GW_KEY]=yes` and `key_absent[DEEPSEEK_API_KEY]=yes`, file names only on failure.
- `--keep` leaves serve running and one extra task stopped on quota; re-running it on `mac-dinh` over HTTP succeeds (checked).
- `--schema-dry-run`: prints the stop-serve reminder, `cp -p` copy of `brain.db`, `-wal`, `-shm`; reads `user_version` and counts from the copy; opens the copy with the new binary (`stats --json`, private HOME and XDG); requires v4, `integrity_check` ok, equal counts and an identical real-directory listing and sha256 set; prints `Schema v4 dry run OK (từ v{old})` and `real_dir_unchanged=yes`.
- README and LIVE-CHECKS section 8 as listed in `provides`.

## Task Commits

1. **Task 1 (tracer): the live check and the dry run** - `2afb1d6` (test)
   - follow-up `5f14cd5` (test): `--keep` task can be re-run to success (fifth Dev queue entry)
2. **Task 2: README and LIVE-CHECKS section 8** - `446d0cb` (docs)
3. **Task 3 [BLOCKING, user]: not started**

**Plan metadata:** the `docs(06-19)` commit that adds this file and the deferred-items note.

## Files Created/Modified

- `scripts/phase6-live-check.sh` (new, mode 100755, 869 lines) - everything above
- `agent-brain/README.md` - section 2 upgrade note, section 3 (`--team`, `human done|return`), section 4 (codes 5 and 6, `hết quota` stop reason), section 5 "Engine và đội hình (Phase 6)", section 11 (keys, agy, người, dsh blast radius), section 12 (twelve test files and the script), section 14 (`### Bảy màn hình chức năng`, new routes, Telegram quota and `/task` team step, six troubleshooting entries)
- `agent-brain/LIVE-CHECKS.md` - `## 8. Nghiệm thu Phase 6` with P6-00 .. P6-10 and `### Dọn dẹp`
- `.planning/phases/06-multi-engine-media-branch/deferred-items.md` - two notes

## Decisions Made

See `key-decisions`. The docs follow the plan's wording; where the code differs from the plan text the code won (for example `models` and `profiles` are required in a custom `config.json`, `rev` is optional on `team` and `quota-rerun`, at most 8 team buttons on a Telegram quota message).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `build-css.sh --check` could not run in the fresh worktree**
- **Found during:** Task 2 gate
- **Issue:** the git-ignored vendored binary `agent-brain/tools/tailwindcss` is not in a new worktree.
- **Fix:** `cp -p` from the main checkout into the worktree's ignored path (the script then verified its pinned sha256); nothing tracked changed. Logged in `deferred-items.md`.

**2. [Plan wording] per-run engine is read from the temp brain.db, not from `GET /api/tasks/{id}`**
- **Issue:** the plan's interface lists `runs[].engine`; the payload has no such field (and no `usage_source`).
- **Fix:** python `sqlite3` in `mode=ro` on the script's own temporary database. Logged in `deferred-items.md`.
- **Files modified:** `scripts/phase6-live-check.sh`

**3. [Rule 2 - Missing critical] extra isolation and checks in the live check**
- A trap `docker` first on the serve PATH with an empty-log assertion at the end (on top of the plan's `fake_docker.py` and awk acceptance); the key sweep also covers `git log -p` and a SQL dump; `--keep` adds a kept quota task (fifth Dev queue entry, commit `5f14cd5`).

**4. [Plan scope] dry-run details**
- File names of the copy follow the real names (the plan's bogus v2 verify uses `old.db`); the `..` line is excluded from the directory comparison; a missing table in the copy is an error (that is what makes the bogus v2 file refuse).

**Total deviations:** 4 (1 Rule 3, 1 Rule 2, 2 plan notes). **Impact:** none on the plan's contract.

## Issues Encountered

- This environment's command guard refuses `bash -c '...'` strings that mention git, runtime `PATH=...` prefixes and `timeout -s`; the plan's verify bodies were run verbatim from small wrapper scripts in the session scratchpad (outside the repo).
- `cargo fmt --check` is clean now (the drift logged for 06-10 and 06-23 no longer exists after the merges).
- The `contract_guard::guard_contract_cases` BrokenPipe flake did not occur.
- No real docker, no real container and no real model endpoint was touched: every cargo, script and smoke run had a trap `docker` first on `PATH` (it logs and exits 97); its log never came into existence. The user's own `agent-brain serve` (port 7437) and `~/.config/agent-brain`, real `brain.db`, `~/.claude-orca` were not touched.

## Verification Results

| Command | Result |
|---|---|
| `cargo build ... --offline --locked && bash scripts/phase6-live-check.sh && git status unchanged` | exit 0; nine `LIVE OK:` lines, `key_absent[PHASE6_GW_KEY]=yes`, `key_absent[DEEPSEEK_API_KEY]=yes`, `Phase 6 live check OK` (run three times, last after the final script commit) |
| bogus v2 file with no tables, `--schema-dry-run` | refused (`LỖI dry run: không đọc được bảng projects ...`), verify exit 0 |
| fresh v4 database (with its own `-wal`/`-shm`), `--schema-dry-run` | `Schema v4 dry run OK (từ v4)` and `real_dir_unchanged=yes` |
| extra: v4 rolled back to v3 (`DROP COLUMN team_json`, empty `-wal`/`-shm`) | `Schema v4 dry run OK (từ v3)`, `real_dir_unchanged=yes`, source still `user_version 3` |
| `--keep`, then `quota-rerun {team: mac-dinh}` on the kept task over HTTP | quota engine `gw`, rerun exit 0, team `mac-dinh`, one history entry, quota `None`; serve stopped by PID |
| acceptance greps of Task 1 | executable; no `pkill`/`killall`; `curl -sS -K -` 2; `cp -p` 3; `time-style=full-iso` 1; `fake_docker.py` 1; awk (no real docker subcommand) exit 0 |
| `cargo test ... -- --test-threads=1` (full suite) | 63 suites, 1182 passed, 0 failed |
| `dashboard-ui-lint.sh` / `build-css.sh --check` / `dashboard-js-check.mjs` | `UI lint OK: 113 thẻ`; `app.css khớp nguồn`; `JS check OK: 1317 checks` |
| `dashboard-smoke.sh overview assign task task-stop task-quota task-human stats projects board settings --keys` | `Smoke OK`, `Keys OK` |
| `cargo fmt --check`; `git diff --exit-code 574b6cd -- Cargo.toml Cargo.lock`; `scan-secrets.sh` | clean; unchanged; `scan-secrets: OK` |
| Task 2 acceptance | `### Bảy màn hình chức năng` 1, `#/settings` 1, `bak-pre-6` 3, `EXIT_QUOTA\|EXIT_WAITING` 2, `allowed_profiles` 6, `pin_profile` 5, `verified` 6, `docker.sock` 1; `## 8. Nghiệm thu Phase 6` 1; `### P6-(0[0-9]|10)` 11; no `/home/`, token-like or key-like string in the diff |

## Known Stubs

None.

## Task 3 (open, user): schema v4 dry run on the real database

Not started and not to be started by an agent. The user runs, from the repo root, after `cargo build --manifest-path agent-brain/Cargo.toml --offline --locked` and with `agent-brain serve` and all runs stopped:

```bash
bash scripts/phase6-live-check.sh --schema-dry-run
```

and pastes the last two lines. Claude then records them here. Accepted: `Schema v4 dry run OK (từ v2)` (or `từ v3`, `từ v4`) plus `real_dir_unchanged=yes`. On `LỖI dry run: …` the phase cannot pass verification (open a gap for the planner; never fix the database by hand).

**Pasted line** (user, 2026-10-28, serve still running at the time of the copy):
```
Bản sao: user_version=2 integrity=ok projects=179 tasks=10 runs=17 events=252
Schema v4 dry run OK (từ v2)
real_dir_unchanged=yes
```
Result: ok. Backup taken after: `brain.db.bak-pre-6-20261028` (sqlite backup API, integrity ok, v2, 10 tasks). New musl binary sha256 `73944f27…` installed to `~/.local/bin/agent-brain`; old one kept as `agent-brain.bak-20261021`.

## Human checks for the end-of-phase UAT (`<human-check>`, harvested into 06-UAT.md)

Follow `agent-brain/LIVE-CHECKS.md` section 8, steps P6-02 .. P6-07, P6-09 and P6-10 (P6-08 optional). Expected:

1. **P6-02** a real `tiet-kiem` task on the endpoint that passed E-09 (gateway first; none passed yet, so it runs as evidence collector and the header says `chưa kiểm E-09`, ENG-01's real gate stays pending): Lead and QA on `claude`, Dev on the chosen endpoint with the right `engine` in each header and in Thống kê; the E-09 lines for each model the entry uses are pasted.
2. **P6-03** simulated quota in the dashboard (`--keep`): the banner offers `Chạy lại bước này bằng đội hình…` and nothing runs by itself.
3. **P6-04** small agy Dev task: edits only plan files, or reports `agy không sửa file nào…`.
4. **P6-05** human Dev step through a second clone: `Xong` brings the second clone's commits in.
5. **P6-06** key sweep for every `key_env`: prints only `yes`.
6. **P6-07** Cài đặt (Engine list, team editor) plus Giao việc, task and Thống kê in light and dark theme at 1024 / 768 / 480 px: Engine rows match config order with honest readiness.
7. **P6-09** dsh on a scratch personal repo with a Kill while it runs: no `ab-dsh-` process left, changed file root-owned as documented, git still works.
8. **P6-10** claude-max refused on a work project with the SPK-10 text and running on a personal one.
9. **P6-08** (optional) Telegram quota buttons with the second confirm tap.

## Open items carried (not blockers of this plan)

- 06-20 Task 3: the gateway E-09 probe needs the user's `VCADM_GATEWAY_KEY`; `spikes/e09/NOTES.md` stays `not-run`, which blocks nothing (D-40). The kill-wrapper probe is recorded `ok` (A18 closed), so P6-09's precondition holds. `--uid-probe` (A19) is not recorded.
- From 06-24 (`deferred-items.md`): `task.js` `teamDlgReadinessLines` still uses a fixed engine order; the dsh Settings row omits `Model:`.

## Threat Flags

None beyond the plan's register. Mitigations: T-06-70 (private env, trap docker, planted keys, git status compared, stop by PID), T-06-71 (`cp -p` copy, hashes and listing compared, `real_dir_unchanged=yes`), T-06-72 (`curl -K -`, yes/no key lines, diff gate clean), T-06-96 (fake docker, awk acceptance, empty trap log), T-06-97 (P6-09 limited to a fresh scratch personal repo, cleanup inside the container), T-06-SC (no dependency changed, Cargo files identical to `574b6cd`).

## Self-Check: PASSED

- Files present: `scripts/phase6-live-check.sh` (mode 100755), `agent-brain/README.md`, `agent-brain/LIVE-CHECKS.md`, this SUMMARY, `deferred-items.md`.
- Commits in history (`git log 5ca5a05..HEAD`): `2afb1d6`, `5f14cd5`, `446d0cb`.
- Verify commands re-run after the last code commit (`5f14cd5`): live check OK; docs commit touched only the two docs.
- `.planning/STATE.md`, `ROADMAP.md`, `REQUIREMENTS.md` untouched; no stray `agent-brain` / fake processes after the runs.
