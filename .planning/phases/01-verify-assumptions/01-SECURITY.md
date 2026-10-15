---
phase: "01"
slug: "verify-assumptions"
status: verified
# threats_open = count of OPEN threats at or above workflow.security_block_on severity (the blocking gate)
threats_open: 0
asvs_level: 1
created: "2026-10-15"
---

# Phase 01 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| CLI output → committed fixtures | Raw JSON/text from claude, agy and dsh may carry paths, emails, tokens | Session metadata, local paths, possible identifiers (medium) |
| Host → dsh container | docker exec into a root container that holds the DeepSeek key and a mounted docker.sock | Command strings in, final text out; key stays in container (high) |
| Spike scripts → OS processes | Signals sent next to the operator's own live Claude sessions | Signals to recorded PIDs/PGIDs only (high) |
| Spike scripts → profile dirs | `~/.claude`, `~/.claude-orca`, `~/.claude-personal` hold OAuth credentials | Credential files must not be read, copied or changed (high) |

---

## Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation | Status |
|-----------|----------|-----------|----------|-------------|------------|--------|
| T-1-01 | Information disclosure | spikes/sN/raw/* fixtures | high | mitigate | `scrub_raw`/`scrub.py` wired into every run.sh; `spikes/lib/scan-secrets.sh` exits 0; history `fbecad1..HEAD` clean of key/JWT/Bearer/PEM/home/email patterns | closed |
| T-1-02 | Information disclosure | `claude auth status`, oauthAccount | medium | mitigate | Allowlist filters `spikes/s5/run.sh:62-75,125-138`; SPK-10 table has no email/UUID/org | closed |
| T-1-03 | Tampering | ~/.claude-orca, ~/.claude credentials | high | mitigate | Clone excludes `.credentials.json` (`s5/run.sh:40`); `run_claude` guard `spikes/lib/env.sh:29-43`; orca fingerprint before/after: credentials equal | closed |
| T-1-04 | Denial of service | S4/S6 kill steps | high | mitigate | No pkill/killall; signals only to recorded PIDs/PGIDs (`s4/run.sh:59-67,143-144`, `s6/run.sh:161-182`); unique markers; final leftovers `none` | closed |
| T-1-05 | Information disclosure | DeepSeek key in the container env | high | mitigate | No inspect/env/printenv/.env.docker in `s6/run.sh`; in-container value scan prints only `LEAK:<NAME>`/`CLEAN` (`scan-secrets.sh:69-88`); recorded result CLEAN | closed |
| T-1-06 | Tampering / DoS | root writes via /mnt/data bind mount; container lifecycle | medium | mitigate | Fixed git-ignored scratch dir, guarded `docker exec rm`, dir removed, no root-owned files left; no restart/stop/rebuild | closed |
| T-1-07 | Denial of service | Antigravity GUI launcher and agy stdin hangs | low | mitigate | All agy calls via `timeout 10 agy … </dev/null` (`s7/run.sh:25,34`); enforced by `s7/verify.sh:14-17` | closed |
| T-1-08 | Repudiation | agent self-concluding verdicts | medium | mitigate | 8 `(chờ duyệt)` labels until user approval; removed in `dfc575e` on the user's instruction (UAT test 2 passed); `check-results.sh --final` exits 0 | closed |
| T-1-SC | Tampering | npm/pip/cargo installs | low | mitigate | No install commands, manifests or lockfiles; stdlib-only scripts | closed |

*Status: open · closed · open — below high threshold (non-blocking)*
*Severity: critical > high > medium > low — only open threats at or above workflow.security_block_on count toward threats_open*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

### Residual findings (tracked, not blocking at ASVS L1)

Mitigations are present, but the tooling behind T-1-01 has defects. At ASVS L2, T-1-01 would be OPEN. These items come from `01-REVIEW.md` and are fixed through `/gsd-code-review 01 --fix`:

- **T-1-01 / WR-01:** the OS username appears in tracked fixtures `spikes/s1/raw/s1.jsonl` and `spikes/s4/raw/s4-r{1,2,3}.jsonl` (plus `-home-<user>-…` and `/tmp/claude-<uid>/` forms in S4 text fixtures). Neither scrub nor scan recognises these forms. The branch has not been pushed, so re-scrub before any push.
- **T-1-01 / CR-01, WR-02..WR-05, WR-07:** `scrub.py` can truncate a file on an encode error, and callers ignore its exit code. The scan can pass when grep errors or the container is missing. The S4 scrub step can be skipped by `set -e`. None of these was triggered in the committed evidence.
- **T-1-03 / WR-08:** `CLEAN_ENV` does not unset `ANTHROPIC_API_KEY` / `CLAUDE_CODE_OAUTH_TOKEN`, and the profile guard compares exact paths only. This was not active in the run.
- **T-1-04 / WR-09:** S6 `marker-not-started` can record "leftover: none" without a final snapshot. In S4, the direct kill sites do not re-check process start time.
- **T-1-05, T-1-06 / IN-05, WR-05:** the `rm` guard is suffix-only, and the container scan uses `eval` on env-derived names.

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-1-01 | SPK-10 (compliance, unregistered) | The user waived the company-policy confirmation for the Work (`~/.claude`, Team seat) / Personal (`~/.claude-orca`, Pro) account split; recorded in `spikes/spike-results.md` and the `01-VERIFICATION.md` override (`594c0fd`); UAT test 1 passed | vinhhx | 2026-10-15 |

Informational, not risks: S1–S4 and S8 write transcripts to the real work profile `~/.claude` by design (D-05). The S5 clone copies `.claude.json`/`settings.json` with no secrets (key names checked). `claude_run_against_orca=no` in `s5-orca-untouched.txt` is a literal string; the real control is the `env.sh` guard.

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-10-15 | 9 | 9 | 0 | gsd-security-auditor (ASVS L1, block_on high) |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-10-15
