# Phase 1: Verify Assumptions - Pattern Map

**Mapped:** 2026-10-14
**Files analyzed:** 20 (all new; no source files modified)
**Analogs found:** 2 weak (style-only) / 20. No project-code analog exists (greenfield).

## Summary

The repo has only `files/` (Vietnamese docs), `.planning/`, and the third-party GSD install in `.claude/`. There is no project shell or python code. The only usable analogs are:
- One template in `files/01-phase0-kiem-chung.md` (the `spike-results.md` skeleton).
- Two GSD shell files (`.claude/hooks/gsd-validate-commit.sh`, `.claude/gsd-core/bin/shared/exit-codes.sh`). These are third-party tooling and are useful only for bash style (strict mode, EXIT trap cleanup, sourced exit-code constants). They are not project conventions.

For everything else, the planner should use the code in 01-RESEARCH.md: Pattern 1 (`run_claude`), Pattern 2 (scrub/scan), "S3 schedule skeleton", "S4 verification snapshot", "Secret scan gate", and "Extract S1 fields" jq examples.

## File Classification

| New File | Role | Data Flow | Closest Analog | Match Quality |
|----------|------|-----------|----------------|---------------|
| `spikes/spike-results.md` | doc/deliverable | batch (human review) | `files/01-phase0-kiem-chung.md` lines 72-81 (template) | exact (template), extended with fields |
| `spikes/lib/env.sh` | utility (sourced) | request-response | `.claude/gsd-core/bin/shared/exit-codes.sh` (sourced, `set -u` safe) | style-only |
| `spikes/lib/scrub.py` | utility | transform | none | none (use RESEARCH Pattern 2) |
| `spikes/lib/scan-secrets.sh` | utility/gate | batch | none (use RESEARCH "Secret scan gate") | none |
| `spikes/lib/check-results.sh` | utility/validator | transform | `.claude/hooks/gsd-validate-commit.sh` (validator, exit codes) | style-only |
| `spikes/s1/run.sh` | script | streaming (JSONL capture) | none (RESEARCH S1 + jq examples) | none |
| `spikes/s2/run.sh` | script | request-response | none | none |
| `spikes/s3/run.sh`, `schedule.sh` | script | batch/scheduled, detached | none (RESEARCH S3 skeleton) | none |
| `spikes/s4/run.sh` | script | process/kill | none (RESEARCH S4 snapshot) | none |
| `spikes/s5/run.sh` | script | file-I/O (profile clone) | none | none |
| `spikes/s6/run.sh` | script | request-response (docker exec) | none | none |
| `spikes/s7/NOTES.md` (+ optional `run.sh`) | doc | request-response | none | none |
| `spikes/s8/run.sh` | script | request-response | none | none |
| `spikes/sN/verify.sh` (x8) | test/smoke | transform | none (RESEARCH Validation map) | none |
| `spikes/sN/raw/*` | fixture | file-I/O | none | none |
| `.gitignore` (modify) | config | n/a | existing `.gitignore` | exact |

## Pattern Assignments

### `spikes/spike-results.md` (doc, human-review)

**Analog:** `files/01-phase0-kiem-chung.md` lines 72-81.

Source template (verbatim):
```markdown
## S1 — Stream JSON
- Ngày thử:
- Lệnh:
- Kết quả: Đạt / Không đạt / Một phần
- Quan sát:
- Ảnh hưởng tới thiết kế:
```
Required extensions (from CONTEXT D-02/D-13, RESEARCH Wave 0):
- Add `Phương án thay thế:` and `Phase bị ảnh hưởng:` fields.
- Add the literal marker `(chờ duyệt)` to every proposed `Kết quả`. The agent writes only a proposal; the user removes the label (D-01/D-02).
- Add an SPK-10 section (Work/Personal decision) with a profile property table (org type, seat tier, role; no emails or UUIDs).
- Include `claude --version` in each result.

### `spikes/lib/env.sh` (utility, sourced by every run.sh)

**Analog:** no project analog. Style-only reference: `.claude/gsd-core/bin/shared/exit-codes.sh` lines 12-14 (sourcing convention) and `.claude/hooks/gsd-validate-commit.sh` lines 10, 37-40.

Source pattern for the exit-code header comment (exit-codes.sh lines 12-14):
```sh
# Usage (from a scanner under scripts/):
#   . "$(dirname "$0")/../gsd-core/bin/shared/exit-codes.sh"
```
Copy the form `. "$(dirname "$0")/../lib/env.sh"`; RESEARCH's S3 skeleton already uses it.

Core body: copy RESEARCH.md Pattern 1 (`run_claude()` with `env -u ... timeout ... claude "$@" </dev/null`, RESEARCH lines 190-195). Add the following helpers, which have no analog:
- `ts()` for a wall-clock timestamp.
- `mk_workdir`.
- `probe` (S3).

Note: do not `set -e` in a sourced file. Use `set -u` only, as exit-codes.sh states it is `set -u` safe.

### `spikes/lib/scan-secrets.sh` (gate) and `spikes/lib/scrub.py` (transform)

**Analog:** none. Use RESEARCH.md "Secret scan gate" (lines 409-419) and Pattern 2 (replacement set, line 201). Run python with `python3 -I`, stdlib only.

Style borrow from `gsd-validate-commit.sh`: strict mode and a single EXIT-trap cleanup.
```bash
set -euo pipefail
cleanup() { rm -f "${TMP:-}" 2>/dev/null || true; }
trap cleanup EXIT
```
(`gsd-validate-commit.sh` lines 10, 37-40.) The scan gate must exit non-zero on any hit and never print env VALUES.

### `spikes/lib/check-results.sh` (validator)

**Analog:** `.claude/hooks/gsd-validate-commit.sh` (a validator with named exit codes). Style only. Use `. exit-codes.sh`-style constants only if the planner wants them; otherwise plain `exit 1`.

Behavior spec from RESEARCH lines 511 and 522:
- Assert 8 sections (S1-S8) plus the SPK-10 section.
- Assert fields `Ngày thử`, `Lệnh`, `Kết quả`, `Quan sát`, `Ảnh hưởng`.
- Assert a "Không đạt" result has `Phương án thay thế`.
- `--final` fails if any `(chờ duyệt)` remains.

### `spikes/s1/run.sh` (script, streaming capture)

**Analog:** none. Use RESEARCH S1 (lines 214-220) and the jq extraction examples (lines 380-386). Required elements:
- Positive run: `claude -p ... --output-format stream-json --verbose --allowedTools "Bash(ls *)"` redirected to `raw/s1.jsonl`.
- Negative run: the same without `--verbose`. Capture stderr and exit code to `raw/s1-no-verbose.txt`.
- Print an OBSERVATIONS block (numbers only); never print Đạt/Không đạt (RESEARCH Pattern 1).

### `spikes/s2/run.sh`, `spikes/s8/run.sh`

**Analog:** none. Follow RESEARCH S2 (lines 222-229, sha256sum + wc -l before/after fork) and S8 (lines 310-315, variants A and B, exit code, `--no-session-persistence`). Both use `run_claude` from env.sh and a scratch dir outside the repo (S8 uses a scratch git repo).

### `spikes/s3/schedule.sh` + `run.sh` (detached scheduler)

**Analog:** none. Copy the RESEARCH.md "S3 schedule skeleton" (lines 389-400) and the measurement design (lines 241-248):
```bash
# start with: setsid nohup bash spikes/s3/schedule.sh >spikes/s3/raw/schedule.log 2>&1 &
set -u; . "$(dirname "$0")/../lib/env.sh"
```
Workdirs are outside the repo, each with a unique-nonce `CLAUDE.md` (10-15k tokens). Probes are at ~1, ~10 and ~65 min after each workdir's own t0. Name output files with real timestamps.

### `spikes/s4/run.sh` (process kill)

**Analog:** none. Copy the RESEARCH "S4 verification snapshot" (lines 402-407):
```bash
snap() { ps -eo pid,ppid,pgid,sid,etimes,cmd --forest | grep -E "claude|sleep 317|bash -c" | grep -v grep; }
snap > raw/ps-before.txt; kill -TERM "$CLAUDE_CHILD_PID"; sleep 5; snap > raw/ps-after-term.txt
pgrep -af '^sleep 317$' > raw/leftover-term.txt || echo "none" >> raw/leftover-term.txt
```
Constraints (RESEARCH lines 255-266):
- Signal only the PID recorded from `$!`.
- No `pkill claude`.
- Test SIGTERM, SIGKILL and the `setsid` group-kill.
- Gate behind a `checkpoint:human-action`, because the classifier denied the kill earlier.

### `spikes/s5/run.sh`, `spikes/s6/run.sh`, `spikes/s7/NOTES.md`

**Analog:** none.
- **S5:** use the RESEARCH safe-clone `rsync` command (line 275, excludes credentials, `chmod 700`). Never run `claude` against `~/.claude-orca`. Never set `CLAUDE_CONFIG_DIR=~/.claude`.
- **S6:** use the `docker exec -w ... dsh --profile headless ... </dev/null` invocation (RESEARCH lines 290-295). Use the scratch dir plus `docker exec ... rm -rf` cleanup. Test the kill variants (a)-(c) (line 298). Never print the env or the key.
- **S7:** NOTES.md records docs-only versus docs plus local `agy` run. Use `agy -p ... </dev/null` only; never run the bare `antigravity` binary (it launches the GUI).

### `spikes/sN/verify.sh` (x8, smoke)

**Analog:** none. Per-spike assertions come from the RESEARCH Validation map (lines 503-511). Example (SPK-01):
```bash
jq -e 'select(.type=="result") | .usage and .session_id' spikes/s1/raw/s1.jsonl
```

### `.gitignore` (modify)

**Analog:** existing `.gitignore`, which currently contains only:
```
.claude/settings.local.json
.claude/.gsd-staging/
```
Append `spikes/**/work/` and `spikes/**/scratch/` (RESEARCH line 176).

## Shared Patterns

### Clean-env wrapper
**Source:** RESEARCH.md Pattern 1 (lines 190-195).
**Apply to:** every `claude` invocation in S1, S2, S3, S4, S5, S8.
Prefix `env -u CLAUDECODE -u CLAUDE_CODE_* ... timeout N claude "$@" </dev/null`.

### Scrub then scan gate before any `git add`
**Source:** RESEARCH Pattern 2 and "Secret scan gate".
**Apply to:** all `spikes/sN/raw/*` and `spike-results.md`.

### Bounded, stdin-closed, re-runnable spikes
**Source:** RESEARCH Pattern 1 (lines 178-180) and Pitfall 10.
**Apply to:** all `run.sh`. Use `timeout`, `</dev/null`, write only into `spikes/sN/raw/`, and print numbers only.

### Bash style (weak, third-party analog)
**Source:** `.claude/hooks/gsd-validate-commit.sh` lines 1-3, 10, 37-40: shebang `#!/usr/bin/env bash`, `set -euo pipefail`, EXIT-trap cleanup.
**Apply to:** `lib/scan-secrets.sh`, `lib/check-results.sh`, and the `sN/run.sh` scripts that create temp files.
Caveat: this is GSD tooling, not a project convention. Do not copy its Node or config-reading logic.

### Human checkpoints
**Source:** CONTEXT D-01/D-02/D-13 and RESEARCH Pattern 3.
**Apply to:** the S4/S6 kill permission, S5 login plus SPK-10 policy, and final approval of every `(chờ duyệt)` label.

## No Analog Found

All of these have no project analog; the planner should use the RESEARCH.md code blocks named above:

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `spikes/lib/scrub.py` | utility | transform | No python in the repo |
| `spikes/lib/scan-secrets.sh` | gate | batch | Greenfield |
| `spikes/s1..s8/run.sh`, `s3/schedule.sh` | script | CLI/process | Greenfield |
| `spikes/sN/verify.sh` | smoke test | transform | No tests exist |
| `spikes/s7/NOTES.md` | doc | n/a | No notes format exists |
| `spikes/sN/raw/*` | fixtures | file-I/O | Generated output |

## Metadata

**Analog search scope:** repo root excluding `.git`; `files/`, `.planning/`, `.claude/` (`find` for `*.sh` and `*.py`), `.gitignore`.
**Files scanned:** 8 shell scripts in `.claude/` (all GSD tooling), 0 python files, 0 project source files.
**Tracked-source gate (#3645):** the repo is not a git repository (`git ls-files` is not applicable). All named analogs are real files in the tree: `files/` and `.claude/hooks`/`.claude/gsd-core`, not mirrors of a capability sync.
**Pattern extraction date:** 2026-10-14
