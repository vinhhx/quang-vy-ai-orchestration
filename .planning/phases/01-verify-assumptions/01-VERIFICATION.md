---
phase: 01-verify-assumptions
verified: 2026-10-15T04:00:00Z
status: human_needed
score: 10/12 must-haves verified
covered_files:
  - .planning/phases/01-verify-assumptions/01-01-PLAN.md
  - .planning/phases/01-verify-assumptions/01-01-SUMMARY.md
  - spikes/lib/check-results.sh
  - spikes/lib/env.sh
  - spikes/lib/scan-secrets.sh
  - spikes/lib/scrub.py
  - spikes/s1/raw/s1.jsonl
  - spikes/s4/run.sh
  - spikes/spike-results.md
covered_digest: "v3:sha256:3ec75df8518b0fefb141429f9d0fff6b3b07529a5e3b890efe426134b6899f6d"
behavior_unverified: 0
overrides_applied: 1
overrides:
  - must_have: "Work/Personal account decided after confirming company policy permits it"
    reason: "User waived the policy confirmation; mapping recorded in spikes/spike-results.md SPK-10"
    accepted_by: "vinhhx"
    accepted_at: "2026-10-15T00:00:00Z"
re_verification: false
gaps: []
human_verification:
  - test: "Decide whether the SPK-10 / ROADMAP SC4 condition 'after confirming company policy' may stand waived"
    expected: "The user confirms the waiver (recorded in spike-results.md as 'SPK-10 alway accept ignore condition', vinhhx, 2026-10-15). If yes, add the override suggested below so later verifications stop flagging it."
    why_human: "SC4 literally requires the policy to be confirmed. No policy source exists anywhere in the repo. Only the user can accept that the condition is dropped."
  - test: "Confirm that removing the 8 pending-approval labels on the user's behalf is accepted"
    expected: "The user confirms the approval ('approved', then 'Yes, accept as proposed') was theirs and that D-02 ('only the user removes the labels') was relaxed knowingly."
    why_human: "The PLAN truth says only the user removes the labels. The repo shows the executor did it. The chat instruction behind it cannot be checked from the codebase."
  - test: "Decide whether SPK-05 (S5) should stay marked Complete while the live personal-profile test, transcript split, cross-resume, E-06 and E-07 were never run"
    expected: "Either accept 'Một phần + follow-up' as satisfying SPK-05, or reopen it before Phase 2/3 design PROF on profile isolation."
    why_human: "The verdict is honestly recorded as Một phần, so SPK-09 is met. Whether 'tried for real' is satisfied for SPK-05 is a scope judgment."
---

# Phase 1: Verify Assumptions - Verification Report

**Phase Goal:** Every technical assumption the architecture rests on (CLI flags, headless capability, cache/kill/profile behaviour) was tried for real and the result recorded, so no later phase is designed on a wrong assumption.
**Verified:** 2026-10-15
**Status:** human_needed
**Re-verification:** No, initial verification

## Goal Achievement

The evidence package exists, is internally consistent, and matches what `spike-results.md` claims. The deterministic gate commands all pass:

- all 8 `spikes/sN/verify.sh` exit 0
- `scan-secrets.sh` prints `OK`
- `check-results.sh --final` prints `OK`
- `(chờ duyệt)` count is 0

I spot-checked the load-bearing numbers in the raw files against the prose, and they match (table below). No blocker was found. Three decisions the user must confirm remain, plus several tooling weaknesses recorded as warnings.

### Observable Truths

**ROADMAP success criteria (the contract)**

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| SC1 | `spike-results.md` has all of S1-S8, each with a verdict, date, command, observation and design impact | VERIFIED | Sections S1-S8 plus SPK-10 exist (`spikes/spike-results.md` lines 9-292). Every section has Ngày thử, Phiên bản, Lệnh, Kết quả, Quan sát, Ảnh hưởng, Phương án thay thế, Phase bị ảnh hưởng and Bằng chứng. Verdicts: S1-S4 Đạt; S5-S8 Một phần. `check-results.sh --final` exits 0. |
| SC2 | Every "Không đạt" names an alternative, and S7 states whether Antigravity runs headless and emits JSON | VERIFIED | No "Không đạt" verdicts exist. Every "Một phần" (S5-S8) has a non-empty Phương án thay thế and Phase bị ảnh hưởng. S7 says headless: CÓ, JSON: CÓ, turn limit: KHÔNG. It rests on docs plus a local `agy` run, shown by 'Nguồn:' labels in `spikes/s7/NOTES.md`. The local `--max-turns` rejection (exit 2) is in `s7-maxturns.txt`. |
| SC3 | Baseline numbers exist: `--verbose` required (S1), session_id and usage location (S1), cache warm/cold at 1 vs 10 min (S3), processes left after kill (S4), `--max-turns 1` behaviour (S8) | VERIFIED | S1: `s1-no-verbose.txt` shows `exit_code=1` and the 'requires --verbose' error. `s1.jsonl` has init session_id and a final result line with usage. S3: raw jq gives w1 probe cache_read 44901 / creation 0, w10 probe 44974 / 0, w65 probe 11168 / 33773, matching the classification TSV (warm, warm, cold). S4: leftovers r1 none, r2 `pid=1087004`, r3 none; exits 143/137/143; finals all `none`. S8: A is `error_max_turns`, no `result` key, exit 1; B is `success`, num_turns 1, exit 0. |
| SC4 | The Work/Personal account was decided after confirming company policy permits it | UNCERTAIN (WARNING) | The mapping is decided and recorded with the user's verbatim answers (Work = `~/.claude` Team seat, Personal = `~/.claude-orca` Pro). The policy was NOT confirmed. The file says "Chính sách công ty: không xác nhận, người dùng miễn". The SC4 qualifier is therefore not literally met. It is waived by the user, which needs formal acceptance (see Human Verification 1). |

**PLAN `must_haves.truths`**

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| P1 | `spike-results.md` has S1-S8 and SPK-10 with all required fields (SPK-09, D-02) | VERIFIED | Same as SC1. |
| P2 | Every verdict was first a `(chờ duyệt)` proposal, only the user removes the labels, `--final` passes only afterwards | UNCERTAIN (WARNING) | The labels are gone and `--final` passes. But the executor removed them, and the SUMMARY (deviation 6) admits this departs from D-02. The user's chat instruction is cited in the file header. The repo cannot prove it (Human Verification 2). |
| P3 | Every Một phần / Không đạt names an alternative and affected phase (D-13) | VERIFIED | S5, S6, S7 and S8 each carry both fields. `check-results.sh --section` enforces non-empty. |
| P4 | S1 and S4 each record a concrete tested "Cách làm đã kiểm chứng" | VERIFIED | Both present (lines 29 and 158). The S4 method matches the raw runs: record descendants, SIGTERM the claude PID, wait 5 s, sweep. The same files show it left `none` in all 3 runs. |
| P5 | Each spike has a re-runnable `run.sh` and scrubbed raw output; `scan-secrets.sh` exits 0 | VERIFIED, with warnings | All 8 run.sh and 8 verify.sh exist. The scan exits 0 and no key/JWT/Bearer/email patterns appear in `spikes/`. The scrub is weaker than the gate claims: the OS username `pc0741` survives in `s1.jsonl` and `s4-r1..3.jsonl` (ls owner columns) and the gate does not see it (WR-01). All fixture JSON parses (0 bad lines in 15 JSON/JSONL files), so CR-01 truncation did not occur. |
| P6 | S3 reports cache_read and cache_creation at marks 0/1/10/65 with a warm/cold class; quota impact is "chưa đo được" | VERIFIED | Six JSON files exist, the classification TSV has rows 1, 10 and 65, and the quota line is present. Numbers match the raw JSON. |
| P7 | `~/.claude-personal` exists, cloned from `~/.claude-orca` without credentials, and the orca fingerprint is unchanged | VERIFIED, with caveat | `~/.claude-personal` has mode 700 and no `.credentials.json`; `credentials_copied=no`. The orca `.credentials.json` is unchanged. `s5-orca-untouched.txt` says `unchanged=no` because `.claude.json` changed. The PLAN acceptance expected `unchanged=yes`. The cause is plausible: the running GSD session itself uses that profile. It is disclosed, and the file shows `claude_run_against_orca=no`. |
| P8 | SPK-10 records the user's policy answer verbatim and the chosen mapping | VERIFIED | Verbatim quotes and the table are present (lines 283-292). The table has no emails or UUIDs. |

**Score:** 10/12 verified (SC4 and P2 UNCERTAIN). Of the 10, P5 and P7 carry warnings. `behavior_unverified`: 0. The spike truths assert recorded observations, not runtime state transitions.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `spikes/spike-results.md` | S1-S8 + SPK-10 verdicts | VERIFIED | 292 lines; all fields filled; no labels left. |
| `spikes/lib/{env.sh,scrub.py,scan-secrets.sh,check-results.sh}` | helpers | VERIFIED (weaknesses) | Substantive and used by every run.sh. See Anti-Patterns. |
| `spikes/s1/raw/s1.jsonl` | Phase 3 parser fixture | VERIFIED | 9 valid JSON lines; init plus result with usage. |
| `spikes/s4/raw/` | kill evidence and method | VERIFIED | 23 files; all finals `none`; `s4-exit.txt` has 3 lines. |
| `spikes/sN/{run.sh,verify.sh}` N=1..8 | re-runnable | VERIFIED | All present; verify.sh pass. |
| `spikes/s7/NOTES.md` | docs vs local labelling | VERIFIED | 'Nguồn:' labels present on every finding. |

Evidence paths in `spike-results.md` that appear missing under a naive check are glob patterns (`s3-wK-*`) or prose, not real gaps. `check-results.sh` confirms every concrete path.

### Key Link Verification

| From | To | Status | Details |
|------|----|--------|---------|
| `spikes/sN/run.sh` | `env.sh run_claude` | WIRED | The `s4` and `s7` verifiers also assert bounds (no name-based kill; `timeout 10` and `</dev/null` on agy). |
| `raw/` | `scan-secrets.sh` | PARTIAL | `scrub_raw` runs in run.sh and the scan exits 0. But callers discard the scrubber's exit status (CR-01), and the scan misses username forms (WR-01). |
| `schedule.sh` | S3 completion | WIRED | `schedule.log` DONE, 3 probe files, classification TSV. |
| `spike-results.md` | `check-results.sh --final` | WIRED | exits 0. |

### Data-Flow Trace (Level 4)

Not applicable. The phase has no rendered dynamic data. It produces scripts and recorded evidence. I checked that the prose numbers trace to the raw files (SC3 row).

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Evidence completeness for S1-S8 | `for s in 1..8; do bash spikes/s$s/verify.sh; done` | all exit 0 | PASS |
| Secret/path scan | `bash spikes/lib/scan-secrets.sh` | `scan-secrets: OK` | PASS |
| Final results gate | `bash spikes/lib/check-results.sh --final` | `OK` | PASS |
| Fixture JSON validity | python `json.loads` over 15 fixtures | 0 bad lines | PASS |
| No name-based kill | grep `pkill`/`killall` outside comments | only the guard line in s4/verify.sh | PASS |
| `~/.claude-personal` has no credentials | `stat` / `ls` | mode 700, no `.credentials.json` | PASS |

I did not run any `run.sh`, as instructed.

### Probe Execution

The phase declares no `probe-*.sh` scripts. SKIPPED.

### Requirements Coverage

All 10 IDs in PLAN frontmatter (`SPK-01..SPK-10`) match the 10 IDs in REQUIREMENTS.md's Phase 1 traceability. No orphaned requirement.

| Requirement | Description | Status | Evidence |
|-------------|-------------|--------|----------|
| SPK-01 | stream-json needs `--verbose`; session_id; usage in result | SATISFIED | S1 Đạt; `s1.jsonl`, `s1-no-verbose.txt`. |
| SPK-02 | resume keeps id; fork makes a new id; original unchanged | SATISFIED | Hash before and after are identical (65 lines); session_ids a932... / a932... / 6476.... |
| SPK-03 | cache measured at 1 min and 10 min | SATISFIED | Plus a 65-min mark. Quota "chưa đo được" is permitted by the requirement ("nếu quan sát được"). |
| SPK-04 | kill the tree; record survivors | SATISFIED | 3 runs; r2 shows the survivor; method tested. |
| SPK-05 | profiles use the right account and separate history | PARTIAL, see warning | The work profile was tested live and the empty clone fails fast. The personal live test, transcript split and cross-resume are `not_run`. Honest verdict Một phần. REQUIREMENTS.md marks it `[x] Complete` (Human Verification 3). |
| SPK-06 | DeepSeek via Claude Code | SATISFIED (Một phần) | Tested via the dsh container (D-07 changed the route). Format parity: no. File edit: works. Kill: client kill leaves the in-container process. |
| SPK-07 | Antigravity headless / JSON / turn flag | SATISFIED | Docs plus a local run. |
| SPK-08 | `--max-turns 1` | SATISFIED | Variant A and B recorded; the finding contradicts the naive assumption (FAST-02 needs form B). |
| SPK-09 | 8 verdicts in the template; "Không đạt" has an alternative | SATISFIED | See SC1 and SC2. |
| SPK-10 | Decided the profile mapping after confirming company policy | PARTIAL, see SC4 | Mapping decided; policy confirmation waived, not obtained. |

### Anti-Patterns Found

No TBD/FIXME/XXX/TODO markers in `spikes/`. The code review (`01-REVIEW.md`, 16 findings, all open) was assessed against the must-haves:

| Finding | Severity | Effect on a must-have |
|---------|----------|-----------------------|
| WR-01 username in committed fixtures | Warning | P5 "scrubbed raw output" is only partly true. `pc0741` appears in `s1.jsonl` (line 6) and `s4-r1..3.jsonl`. It is an OS username, not a credential, but the scan reports OK while it is present. Fix before the branch is pushed or shared. |
| CR-01 scrub.py truncates on encode error | Critical in tooling, latent here | Not realized. All 15 JSON fixtures parse and no data fixture is 0 bytes. The two empty files are legitimately empty stderr captures (`s8-*.stderr.txt`). It does not invalidate any current must-have. It makes the "scrubbed" guarantee unreliable on re-runs and should be fixed before the scrubber is trusted again. |
| WR-02, WR-03, WR-04, WR-05 | Warning | The scan gate can pass vacuously (grep errors, missing container path). "scan exits 0" is weaker evidence than it sounds, but I confirmed the current tree is clean by independent grep for keys, JWTs, `ghp_` and emails. |
| WR-06 | Warning | `s5 clone` can overwrite the baseline before its guard. The orca fingerprint here was not corrupted (credentials unchanged). |
| WR-07 | Warning | S4 can leave unscrubbed stream-json if aborted. In the committed tree the S4 fixtures are scrubbed (cwd shows `~/.cache/...`). |
| WR-08 to WR-10, IN-01 to IN-05 | Warning / Info | Robustness and tidiness; none contradicts a recorded observation. |

### Human Verification Required

1. **SC4 / SPK-10 policy waiver.** Test: confirm the company-policy condition is knowingly waived. Expected: the user confirms the recorded waiver. Why human: the repo holds no policy source. If confirmed, the override below turns SC4 into PASSED (override).
2. **Label removal on the user's behalf.** Test: confirm "approved" plus "Yes, accept as proposed" were the user's. Expected: the user acknowledges the D-02 deviation. Why human: it cannot be verified from the codebase.
3. **SPK-05 completeness.** Test: decide whether S5 as Một phần satisfies SPK-05, or reopen it. Expected: a decision. Why human: it is a scope call. Phase 3 is already told not to assume transcript isolation, which limits the risk.

Suggested override, only if the user accepts item 1:

```yaml
overrides:
  - must_have: "Work/Personal account decided after confirming company policy permits it"
    reason: "User waived the policy confirmation; mapping recorded in spikes/spike-results.md SPK-10"
    accepted_by: "vinhhx"
    accepted_at: "2026-10-15T00:00:00Z"
```

### Gaps Summary

No must-have FAILED, so there are no blocker gaps and no `gaps:` entries. The phase delivers what it set out to. All 8 spikes were run, the results are recorded with raw evidence, and the numbers match. The honest "Một phần" verdicts correctly keep later phases from building on untested ground (profile isolation, dsh stream format, agy turn cap).

What stops this being `passed` is:

- (a) the SC4 "policy confirmed" clause was waived, not met
- (b) the D-02 "only the user removes labels" rule was bypassed by instruction
- (c) SPK-05 is marked Complete with its live test skipped

Separately, the secret-hygiene tooling (CR-01, WR-01..05) is weaker than its "exit 0" implies. A username is committed in fixtures. Fix this before the branch is shared. It does not invalidate the phase's findings.

---

_Verified: 2026-10-15_
_Verifier: Claude (gsd-verifier)_
