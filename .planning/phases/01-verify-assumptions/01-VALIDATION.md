---
phase: "1"
slug: "verify-assumptions"
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: "2026-10-14"
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> This phase produces evidence (spike outputs + `spike-results.md`), not product code — "tests" are mechanical checks on the evidence package. Source: `01-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Plain bash + jq assertions (no framework install) |
| **Config file** | none — Wave 0 creates `spikes/lib/` helpers |
| **Quick run command** | `bash spikes/lib/check-results.sh && bash spikes/lib/scan-secrets.sh` |
| **Full suite command** | `for s in 1 2 3 4 5 6 7 8; do bash spikes/s$s/verify.sh; done && bash spikes/lib/scan-secrets.sh && bash spikes/lib/check-results.sh --final` |
| **Estimated runtime** | ~10 seconds (checks only; spike runs themselves are not part of the suite) |

---

## Sampling Rate

- **After every task commit:** Run `bash spikes/lib/scan-secrets.sh` (must pass before any `git add` of spike output)
- **After every plan wave:** Run the quick run command
- **Before `/gsd-verify-work`:** Full suite must be green AND user approval recorded (no `(chờ duyệt)` labels left)
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

Task IDs are assigned by the planner; this map is keyed by requirement and is refined when PLAN.md files exist.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | TBD | 0 | SPK-09 | — | Results validator rejects missing fields | lint | `bash spikes/lib/check-results.sh` | ❌ W0 | ⬜ pending |
| TBD | TBD | 0 | (D-03) | T-1-secrets | No key/token/email/home path in committed output | lint | `bash spikes/lib/scan-secrets.sh` | ❌ W0 | ⬜ pending |
| TBD | TBD | ≥1 | SPK-01 | — | N/A | smoke | `jq -e 'select(.type=="result") \| .usage and .session_id' spikes/s1/raw/s1.jsonl` | ❌ W0 | ⬜ pending |
| TBD | TBD | ≥1 | SPK-02 | — | N/A | smoke | `bash spikes/s2/verify.sh` | ❌ W0 | ⬜ pending |
| TBD | TBD | ≥1 | SPK-03 | — | N/A | smoke | `bash spikes/s3/verify.sh` | ❌ W0 | ⬜ pending |
| TBD | TBD | ≥1 | SPK-04 | — | Never `pkill claude`; only the spike's own PGID is signalled | smoke | `bash spikes/s4/verify.sh` | ❌ W0 | ⬜ pending |
| TBD | TBD | ≥1 | SPK-05 | T-1-creds | `.credentials.json` never copied/printed; `~/.claude-orca` untouched | smoke | `bash spikes/s5/verify.sh` | ❌ W0 | ⬜ pending |
| TBD | TBD | ≥1 | SPK-06 | T-1-secrets | DeepSeek key never printed (env var names only) | smoke | `bash spikes/s6/verify.sh` | ❌ W0 | ⬜ pending |
| TBD | TBD | ≥1 | SPK-07 | — | Only `agy` CLI used, never bare `antigravity` | smoke | `bash spikes/s7/verify.sh` | ❌ W0 | ⬜ pending |
| TBD | TBD | ≥1 | SPK-08 | — | N/A | smoke | `bash spikes/s8/verify.sh` | ❌ W0 | ⬜ pending |
| TBD | TBD | final | SPK-09 | — | N/A | lint | `bash spikes/lib/check-results.sh --final` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `spikes/lib/env.sh` — nested-env scrub (`env -u CLAUDECODE` and `CLAUDE_CODE_*`), `run_claude` wrapper with `timeout` and `</dev/null`, timestamp helper
- [ ] `spikes/lib/scrub.py` — JSON-aware redaction (stdlib python, keeps fixtures valid JSON)
- [ ] `spikes/lib/scan-secrets.sh` — hard gate: patterns + live env-var value denylist (values never echoed)
- [ ] `spikes/lib/check-results.sh` — template/section validator; `--final` forbids `(chờ duyệt)`
- [ ] `spikes/spike-results.md` skeleton — 8 sections per `files/01-phase0-kiem-chung.md` §5 plus "Phương án thay thế" and "Phase bị ảnh hưởng"
- [ ] `spikes/sN/verify.sh` stubs for S1–S8
- [ ] `.gitignore` additions for `spikes/**/work/`, `spikes/**/scratch/`
- Framework install: none.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Work/Personal account decision | SPK-10 | Company policy cannot be machine-checked | User confirms policy and records decision in `spike-results.md` decision section |
| Spike verdicts (Đạt / Không đạt / Một phần) | SPK-09 | D-02: user is the one who concludes | User reviews each `(chờ duyệt)` proposal, edits if needed, removes label |
| Interactive login for `~/.claude-personal` | SPK-05 | OAuth login is interactive | User runs login with `CLAUDE_CONFIG_DIR=~/.claude-personal` |
| Kill experiments S4 / S6 | SPK-04, SPK-06 | Auto-mode classifier likely denies kill commands from the agent | User pre-authorises or runs `spikes/s4/run.sh` / `spikes/s6/run.sh` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
