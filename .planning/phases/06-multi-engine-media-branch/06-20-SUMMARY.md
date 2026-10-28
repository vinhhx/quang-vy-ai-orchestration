---
phase: 06-multi-engine-media-branch
plan: 20
subsystem: spikes
tags: [e09, s6, coverage, requirements, eng-08, eng-09, eng-10, eng-11]
status: partial — Tasks 1, 2 done; Task 3 (user checkpoint) open
key-files:
  modified:
    - spikes/e09/run.sh
    - spikes/e09/verify.sh
    - spikes/e09/NOTES.md
    - spikes/s6/run.sh
    - spikes/s6/NOTES.md
    - agent-brain/tests/e09_fixtures.rs
    - .planning/phases/06-multi-engine-media-branch/COVERAGE.md
    - .planning/REQUIREMENTS.md
  created:
    - spikes/s6/raw/s6-kill-wrapper.txt
---

# Plan 06-20 summary

Task 1 (E-09 per endpoint and per model) and Task 2 (kill-wrapper and uid probes, COVERAGE,
ENG-08..11) were committed before the quota ran out. Claude re-ran their verify commands.

## Verification (re-run by Claude)
- offline probe, default and `E09_NAME=gw`: exit 0, git tree unchanged; missing key: exit 2
- `--test e09_fixtures`: passes (prints `bỏ qua: chưa có fixture E-09`, as the plan allows)
- `spikes/s6/run.sh --nope`: exit 2 with `--kill-wrapper` and `--uid-probe` in usage
- COVERAGE check and REQUIREMENTS check (126 total, 126 mapped, ENG-08..11) exit 0
- no `pkill`/`killall`, no bare `docker inspect`, `scan-secrets: OK`, Cargo files unchanged

## Open: Task 3 (user)
- `spikes/s6/raw/s6-kill-wrapper.txt` records a kill-wrapper run: `marker_visible=yes`, `sh_parent_kept=yes`, `leftover: none`, `final: none`. This closes assumption A18 for plan 06-19 P6-09.
- Not done: the gateway E-09 probe (needs the user's `VCADM_GATEWAY_KEY`) and the optional official-endpoint probe. `spikes/e09/NOTES.md` stays `not-run` for those; per D-40 that blocks nothing.
- `--uid-probe` (A19) result is not recorded.
