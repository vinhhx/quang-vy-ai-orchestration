---
phase: 01-verify-assumptions
plan: 01
subsystem: testing
tags: [claude-code, stream-json, session-resume, prompt-cache, process-kill, claude-config-dir, dsh-harness, agy, max-turns, spikes]

requires: []
provides:
  - "spikes/spike-results.md: S1..S8 verdicts and the SPK-10 decision, all approved by the user"
  - "Re-runnable spikes/sN/run.sh + verify.sh and scrubbed raw evidence under spikes/sN/raw/"
  - "spikes/lib: clean-env claude wrapper, scrubber, secret-scan gate, results validator"
  - "Tested kill sequence for Phase 3 (record descendants, SIGTERM claude PID, wait, sweep) and stream-json fixtures (spikes/s1/raw/s1.jsonl, spikes/s4/raw/s4-r*.jsonl)"
affects: [phase-02, phase-03, phase-04, phase-06]

plan_head_before: fbecad1e98750db917d818ac80524594b76fe2ff
plan_head_after: dfc575e63e06b5623b047cd3a7cdb90c5f85445e
actuals:
  tokens: 54948
  tasks: 8
  commits: 15

tech-stack:
  added: []
  patterns:
    - "Clean-env claude wrapper (run_claude): env -u nested-session vars, timeout, </dev/null, no --bare"
    - "scrub -> secret-scan gate before every git add; evidence-completeness verify.sh per spike (never checks the outcome)"
    - "Detached time-marked probes (setsid + nohup schedule.sh) with independent prefixes per mark"
    - "Record descendants before signalling, then sweep survivors (kill design)"

key-files:
  created:
    - spikes/spike-results.md
    - spikes/lib/env.sh
    - spikes/lib/scrub.py
    - spikes/lib/scan-secrets.sh
    - spikes/lib/check-results.sh
    - spikes/s1/run.sh
    - spikes/s2/run.sh
    - spikes/s3/run.sh
    - spikes/s3/schedule.sh
    - spikes/s4/run.sh
    - spikes/s5/run.sh
    - spikes/s6/run.sh
    - spikes/s7/run.sh
    - spikes/s7/NOTES.md
    - spikes/s8/run.sh
  modified:
    - .gitignore

key-decisions:
  - "S1 stream-json: --verbose is mandatory with -p --output-format stream-json; parser must tolerate hook and rate_limit_event lines and dedupe usage by message.id (verdict Dat)"
  - "S2: --resume keeps session_id, --resume --fork-session makes a new id, original transcript unchanged, fork reads the parent's cache (Dat); auto-memory can leak state between headless runs"
  - "S3: subscription cache uses the 1h TTL bucket, warm at 1 and 10 min, cold at 65 min (Dat); expiry lies between 10 and 65 min; quota impact not measurable"
  - "S4: SIGTERM to the claude PID cleans the tree; SIGKILL orphans bash+sleep; kill -TERM -- -PGID also left nothing (contradicts E-05); verified method is record descendants, SIGTERM claude PID, wait 5 s, sweep (Dat)"
  - "S5 (Mot phan): only mapped, live personal-profile test not run; Personal profile is ~/.claude-orca (not ~/.claude-personal); empty profile fails fast with 'Not logged in' instead of falling back to work"
  - "S6 (Mot phan): dsh headless works from the host via docker exec but prints only the final text (no stream-json, session or usage); killing the host docker exec client leaves the in-container process alive"
  - "S7 (Mot phan): agy -p with json/stream-json works headless; no turn-limit flag (--max-turns rejected, exit 2); bound with --print-timeout plus external timeout"
  - "S8 (Mot phan): --max-turns 1 on 'review git diff' ends error_max_turns with no result text and exit 1; FAST-02 must embed the diff and use --tools '' --no-session-persistence"
  - "SPK-10 mapping: Work = ~/.claude (company Team seat), Personal = ~/.claude-orca (personal Pro); company policy unconfirmed and waived by the user ('SPK-10 alway accept ignore condition')"

requirements-completed: [SPK-01, SPK-02, SPK-03, SPK-04, SPK-05, SPK-06, SPK-07, SPK-08, SPK-09, SPK-10]

coverage:
  - id: D1
    description: "Shared spike scaffolding: clean-env wrapper, scrubber, secret-scan gate, results validator"
    requirement: "SPK-09"
    verification:
      - kind: other
        ref: "bash spikes/lib/scan-secrets.sh && bash spikes/lib/check-results.sh --final"
        status: pass
    human_judgment: false
  - id: D2
    description: "Re-runnable scripts and scrubbed raw evidence for S1..S8 (completeness checked per spike)"
    requirement: "SPK-01"
    verification:
      - kind: other
        ref: "for s in 1..8: bash spikes/s$s/verify.sh"
        status: pass
    human_judgment: false
  - id: D3
    description: "S1..S8 verdicts and the SPK-10 Work/Personal decision recorded in spikes/spike-results.md"
    requirement: "SPK-10"
    verification: []
    human_judgment: true
    rationale: "Verdicts and the account/policy decision are judgments; the user approved them in chat on 2026-10-15 (approved, then 'Yes, accept as proposed')."

duration: 1h 48m
completed: 2026-10-15
status: complete
---

# Phase 1 Plan 1: Verify Assumptions Summary

**Eight live spikes (stream-json, resume/fork, prompt-cache TTL, kill-the-tree, profile split, DeepSeek harness, agy headless, max-turns) with re-runnable scripts, scrubbed evidence, a user-approved verdict table and a tested clean-kill method for Phase 3.**

## Performance

- **Duration:** 1h 48m (first task commit 2026-10-15T01:27Z to 03:15Z; includes a 65-minute S3 detached probe wait and user checkpoints)
- **Started:** 2026-10-15T01:21:29Z (phase execution start)
- **Completed:** 2026-10-15T03:15:06Z
- **Tasks:** 8 of 8
- **Files modified:** 97 files outside `.planning/` (24 scripts and docs, 73 raw evidence files)

## Accomplishments

- Verdicts, all approved by the user as proposed: S1 Dat, S2 Dat, S3 Dat, S4 Dat, S5 Mot phan, S6 Mot phan, S7 Mot phan, S8 Mot phan. Every Mot phan names an alternative and the affected phase.
- S1: `--verbose` is mandatory for `-p --output-format stream-json` (confirmed by the negative run, exit 1). The stream includes hook lines before `init`, `rate_limit_event` lines, one assistant line per content block sharing a `message.id`, and a final `result` line with usage. Fixture saved for the Phase 3 parser.
- S2/S3: fork reads the parent's cache; the subscription writes the 1h TTL bucket; the project-specific prefix (~33.7k tokens) was warm at 1 and 10 minutes and cold at 65 minutes. This sets the Phase 4 golden-session window (design on under about 50 minutes, to be fixed in Phase 4).
- S4: the kill sequence that left no process behind in all three runs is: record claude PID and descendants, SIGTERM the claude PID, wait 5 s, sweep recorded survivors (TERM then KILL after 2 s). SIGKILL on claude orphans the Bash child and `sleep`, so the sweep is mandatory there.
- S5/SPK-10: Work = `~/.claude` (company Team seat), Personal = `~/.claude-orca` (personal Pro). Live personal-profile test, transcript split and cross-resume were not run (user chose map-only).
- S6/S7/S8: dsh headless is a text-only black box and needs in-container kill; agy is headless with JSON output but has no turn limit; `--max-turns 1` needs the diff embedded and tools disabled (FAST-02).
- Secret and path hygiene: `scan-secrets.sh` (host and in-container) clean; `~/.claude-personal` created with mode 700 and no credentials.

## Task Commits

1. **Task 1: Shared scaffolding + S1 stream-json** - `4c4020e` (feat)
2. **Task 2: S2 resume and fork** - `95ce06f` (feat)
3. **Task 3: S3 prompt cache** - `a67018a` (feat, mark 0 and detached schedule), `40f7753` (feat, 1/10/65 min results)
4. **Task 4: S4 kill the process tree** - `5e15d25` (feat, scripts), `28dbdf5` (fix, orchestrator), `3f61a5a` (feat, results)
5. **Task 5: S5 profile separation + SPK-10** - `f2f7db5` (feat, scripts and credential-free clone), `33e9a21` (feat, map-only results and SPK-10)
6. **Task 6: S6 DeepSeek harness** - `2f35c0e` (feat, scripts and basic mode), `c959708` (feat, kill-from-outside evidence)
7. **Task 7: S7 agy headless** - `5dd4783` (feat)
8. **Task 8: S8 max-turns, assembly, approval** - `0acfb1a` (feat, S8), `e6b3261` (feat, header and S5 verify), `dfc575e` (docs, approval recorded and labels removed)

**Plan metadata:** recorded in the `docs(01-01)` metadata commit that follows this SUMMARY.

## Files Created/Modified

- `spikes/spike-results.md` - S1..S8 verdicts, SPK-10 decision, approval note
- `spikes/lib/env.sh` - sourced by every run.sh; `run_claude` clean-env wrapper
- `spikes/lib/scrub.py` - in-place scrubber for raw evidence
- `spikes/lib/scan-secrets.sh` - secret/path scan gate (host and `--container`)
- `spikes/lib/check-results.sh` - structure, `--section` and `--final` validator
- `spikes/s1..s8/run.sh`, `verify.sh` - re-runnable spike and evidence-completeness check (`s3/schedule.sh` for the detached probes)
- `spikes/s7/NOTES.md` - docs vs local-run sources for Antigravity headless
- `spikes/sN/raw/` - 73 scrubbed evidence files
- `.gitignore` - ignores `spikes/**/work/` and `spikes/**/scratch/`

## Decisions Made

See `key-decisions` in the frontmatter. Most load-bearing: the Phase 3 kill method (S4), the `--verbose` and tolerant-parser rule (S1), the S8 FAST-02 command shape, and the SPK-10 mapping with the company policy waived by the user.

## Deviations from Plan

### Process and scope deviations

**1. [Rule 3 - Blocking] S4 run.sh failed in bash 5.2 at run 1**
- **Found during:** Task 4 (user-run S4 script)
- **Issue:** the `${!REC_PIDS[@]+...}` expansion was rejected (`invalid variable name`), so r1 aborted.
- **Fix:** replaced with `for i in "${!REC_PIDS[@]}"`; the script cleans the partial output of the failed run and was re-run from scratch.
- **Files modified:** `spikes/s4/run.sh`
- **Committed in:** `28dbdf5` (made by the orchestrator, not the executor)

**2. [Plan deviation - user decision] S5 map-only, live test not run**
- **Found during:** Task 5 login checkpoint
- **Issue:** the user stated the Personal profile is `~/.claude-orca` (the profile this GSD session itself runs on), which the plan forbids running claude against or modifying (E-06). The user chose "Map only, skip live test".
- **Fix:** `spikes/s5/run.sh` got a `partial` mode, and `spikes/s5/verify.sh` was made to accept map-only evidence (`not_run` rows for personal transcripts and cross-resume). The `~/.claude-personal` clone is kept, unused and not deleted, for a later live test. S5 verdict is Mot phan.
- **Files modified:** `spikes/s5/run.sh`, `spikes/s5/verify.sh`
- **Committed in:** `f2f7db5`, `33e9a21`, `e6b3261`

**3. [Plan deviation - user request] S6 kill run executed by the orchestrator**
- **Found during:** Task 6 kill checkpoint
- **Issue:** the plan has the user run `bash spikes/s6/run.sh --kill`. The user's own run produced no result files and no error output, and the user asked the orchestrator to run it ("You support test for me").
- **Fix:** the orchestrator ran it. The script signals only the client PID it started and in-container PIDs carrying the `s6k-` marker. Afterwards no marker or `sleep 120` process remained in the container and the scratch directory was gone. Recorded in S6.
- **Committed in:** `c959708`

**4. [Rule 3 - Blocking] S7 docs fetched with curl instead of WebFetch**
- **Found during:** Task 7
- **Issue:** WebFetch is not available in the execution session.
- **Fix:** `curl` of `antigravity.google/docs/cli/headless`, text extracted by hand; the page is labelled `Nguồn: docs` in NOTES.md and the verdict states it rests on docs plus a local run.
- **Committed in:** `5dd4783`

**5. [Plan deviation - protocol slip] One `agy --version` ran without the 10 s timeout bound**
- **Found during:** Task 7
- **Issue:** a single `agy --version` was run once without `timeout 10 ... </dev/null`, against the plan's bound rule. It returned normally with no side effect. `run.sh` itself keeps the bound on every agy line and `verify.sh` enforces it.
- **Fix:** none needed; disclosed here.

**6. [Plan deviation - D-02, user instruction] Pending labels removed by the executor**
- **Found during:** Task 8 final approval
- **Issue:** D-02 says only the user removes the pending labels. At the final checkpoint the user replied "approved". The orchestrator found all 8 labels still present and asked; the user answered "Yes, accept as proposed", explicitly instructing that every verdict be accepted exactly as proposed and the labels removed on their behalf.
- **Fix:** the executor removed the 8 labels without touching any verdict or evidence text, and added an approval note under the file header. `check-results.sh --final` then passed.
- **Files modified:** `spikes/spike-results.md`
- **Committed in:** `dfc575e`

**7. [Environment deviation] Work done on branch `gsd/phase-01-verify-assumptions`**
- **Issue:** `main` is a protected branch (the executor refuses to commit there).
- **Fix:** all commits are on `gsd/phase-01-verify-assumptions`, based on `fbecad1`. The branch needs merging to `main` through the normal ship flow.

---

**Total deviations:** 7 (2 blocking fixes, 4 user-directed plan changes, 1 branch/environment). No evidence or verdict was changed after the user's approval.
**Impact on plan:** S5 lost its live profile-separation, transcript-split and cross-resume checks (a user decision, recorded as Mot phan with the follow-up path). Everything else ran as designed.

## Issues Encountered

- S1 showed the work account at 0.94 of its 7-day limit; later spikes kept prompts small and S3 showed no sign of a TTL downgrade.
- Auto-memory wrote a note during S2 step 1, mixing the "resume remembers 42" test with long-term memory; recorded in S2 and carried as a Phase 3/4 design question.
- `claude auth status` in this version does not print `organizationType`, `seatTier` or `organizationRole`; those came from `oauthAccount` in `.claude.json` through an allowlist.
- During S5 `~/.claude-orca/.claude.json` changed on its own (`claude_json_unchanged=no`) because the running session uses that profile; `.credentials.json` was unchanged and the spike only read it.

## Known Stubs

None. No placeholder data flows to a UI; this plan has no UI.

## Open Items Carried Forward

- S5 live test (personal login, transcript split, cross-resume, token rotation E-06, explicit `CLAUDE_CONFIG_DIR=~/.claude` E-07) was not run. Follow-up: log in the cloned `~/.claude-personal` and run `bash spikes/s5/run.sh test`, or test `~/.claude-orca` from a terminal outside GSD.
- S6: tool-using quality beyond a one-line edit, foreground-fork kill, `dsh web` API, DeepSeek key against the Anthropic-format endpoint (E-09) are untested; Phase 6 research.
- S7: agy with write/exec tools, `--input-format stream-json`, `--conversation`, not-logged-in case, and `--print-timeout` behaviour are untested.
- SPK-10: company policy remains unconfirmed; the user waived the condition. The STATE.md blocker for it is cleared on that basis.

## Threat Flags

None. The only new surface this plan touched (`docker exec` into `dsh-deepseek-harness`, the `~/.claude-personal` profile directory) was in the plan's threat model; no new endpoint, auth path or schema was introduced.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 2 and Phase 3 can design on observed behaviour: stream-json fixtures, the kill method, `--verbose`, fork cache reuse, the S8 command shape.
- Phase 3 should treat profile directories as configuration and not assume transcript isolation until the S5 live test is done.
- Phase 6 inherits the dsh (text-only, in-container kill) and agy (no turn cap, own event schema) findings.
- The phase branch `gsd/phase-01-verify-assumptions` still needs merging to `main`.

---
*Phase: 01-verify-assumptions*
*Completed: 2026-10-15*

## Self-Check: PASSED

- Files present: `spikes/spike-results.md`, `spikes/lib/{env.sh,scrub.py,scan-secrets.sh,check-results.sh}`, `spikes/s1/raw/s1.jsonl`, `spikes/s3/raw/s3-classification.tsv`, `spikes/s4/raw/s4-r1-final.txt`, `spikes/s7/NOTES.md`, `spikes/s8/raw/s8-a.json`.
- All 15 commits (`4c4020e` through `dfc575e`) are ancestors of HEAD; `git rev-list --count fbecad1..HEAD` = 15.
- Final gate: all eight `verify.sh` pass, `scan-secrets.sh` OK, `check-results.sh --final` OK.
