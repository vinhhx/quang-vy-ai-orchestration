---
phase: 01-verify-assumptions
reviewed: 2026-10-15T00:00:00Z
depth: standard
files_reviewed: 24
files_reviewed_list:
  - .gitignore
  - spikes/lib/check-results.sh
  - spikes/lib/env.sh
  - spikes/lib/scan-secrets.sh
  - spikes/lib/scrub.py
  - spikes/s1/run.sh
  - spikes/s1/verify.sh
  - spikes/s2/run.sh
  - spikes/s2/verify.sh
  - spikes/s3/run.sh
  - spikes/s3/schedule.sh
  - spikes/s3/verify.sh
  - spikes/s4/run.sh
  - spikes/s4/verify.sh
  - spikes/s5/run.sh
  - spikes/s5/verify.sh
  - spikes/s6/run.sh
  - spikes/s6/verify.sh
  - spikes/s7/NOTES.md
  - spikes/s7/run.sh
  - spikes/s7/verify.sh
  - spikes/s8/run.sh
  - spikes/s8/verify.sh
  - spikes/spike-results.md
findings:
  critical: 1
  warning: 10
  info: 5
  total: 16
status: issues_found
---

# Phase 1: Code Review Report

**Reviewed:** 2026-10-15
**Depth:** standard
**Files Reviewed:** 24
**Status:** issues_found

## Summary

Process-signal safety is good. I found no name-based kill anywhere. S4 gates every signal on PID plus `/proc` start time. The group kill in S4 checks that the pgid is the launcher's own and is not this script's. The S6 in-container kills use only a recorded PID or a unique marker, and S3 only does `kill -0`. The `~/.claude-orca` and credential-file handling is also fine. `~/.claude-orca/.credentials.json` is only fingerprinted, and `run_claude` refuses `~/.claude-orca` and `~/.claude` as `SPIKE_PROFILE_DIR`.

The weak spots are the secret-hygiene tooling and a few re-run and abort paths:

- `scrub.py` can truncate a fixture to zero bytes and abort the whole run, and its callers do not check the exit status.
- The scrubber and the scan gate both miss username forms. Those forms are already committed, and the gate reports `scan-secrets: OK`.
- `scan-secrets.sh` can pass when `grep` errors or the container path is missing.
- Re-running `s5 clone` overwrites the baseline it is meant to protect.
- S4 can abort before its scrub step and leave unscrubbed stream-json in the tracked tree.

I ran the scrubber and scanner against synthetic input in the scratchpad. `check-results.sh --final` and `scan-secrets.sh` both exit 0 on the current tree.

## Critical Issues

### CR-01: scrub.py truncates a file to zero bytes on an encode error, and aborts the rest of the run

**File:** `spikes/lib/scrub.py:125-127` (callers: `spikes/lib/env.sh:46-48`, every `scrub_raw` call)
**Issue:** `scrub_file` opens the target with `open(path, "wb")` and only then evaluates `out.encode("utf-8", errors="surrogateescape")`.

A JSON string such as `"\ud83d"` (a lone surrogate from an escaped or truncated emoji, plausible in Claude or agy output) parses to a lone surrogate. `json.dumps(ensure_ascii=False)` emits it. `surrogateescape` only round-trips U+DC80..U+DCFF, so `.encode()` raises `UnicodeEncodeError`.

I reproduced this with a line `{"h":"Bearer <24 chars>","msg":"\ud83d"}`:
- The file was left at 0 bytes. This is data loss on an evidence fixture.
- `main()` only catches `OSError`, so the traceback aborted the loop and the remaining files and directories were never scrubbed. The `Bearer ...` file in the same run stayed unscrubbed.
- The exit code is 1 on the crash, but every caller discards it. Examples are `scrub_raw "$RAW" >/dev/null` and `scrub_raw ... > /dev/null`, and `schedule.sh:29` ignores it too. The scripts then carry on and print "done", so a partial scrub is reported as success.

Even when no exception occurs, `main()` returns 0 after `scrub: cannot process ...` (line 152), so unreadable files also fail open.

**Fix:**
```python
def scrub_file(path):
    ...
    if out != text:
        data = out.encode("utf-8", errors="surrogatepass")   # never raises for lone surrogates
        tmp = path + ".scrub.tmp"
        with open(tmp, "wb") as fh:
            fh.write(data)
        os.replace(tmp, path)                                 # atomic; original untouched on failure
    return count

# main(): catch Exception per file, remember failures, return 1 at the end
failed = 0
for path in iter_files(arg):
    try:
        n = scrub_file(path)
    except Exception as exc:
        print("scrub: FAILED %s: %s" % (path, exc.__class__.__name__), file=sys.stderr)
        failed = 1
        continue
...
return 1 if failed else 0
```
Then make `scrub_raw` callers fail loudly, for example `scrub_raw "$RAW" >/dev/null || { echo "scrub failed" >&2; exit 1; }`.

## Warnings

### WR-01: username leaks into committed fixtures through forms that neither scrub.py nor scan-secrets.sh recognise

**File:** `spikes/lib/scrub.py:34,49-61`, `spikes/lib/scan-secrets.sh:59`
**Issue:** Both tools only handle the literal path `/home/<user>`. The tracked files still contain:
- The claude project-dir encoding of the home path, `-home-pc0741--cache-qv-spikes-s4`, in `spikes/s4/raw/s4-r{1,2,3}.jsonl`.
- The bare username `pc0741`, three times on line 6 of `spikes/s1/raw/s1.jsonl` (`ls -l` owner and group).
- The uid-bearing path `/tmp/claude-1000/...`.

`scan-secrets.sh` prints `OK` for the same tree, so the D-03 hard gate gives false assurance. `spike-results.md` states that all raw output passed both tools.

**Fix:**
- In `scrub.py`, also replace `HOME.replace("/", "-")` (and the `-home-<user>-` form) with `-home-USER-`.
- Replace the bare `$USER` as a whole word with `USER`.
- In `scan-secrets.sh`, add `-home-${USER}` and `\b${USER}\b` patterns, or one derived-forms loop over the same fixed strings.
- Re-scrub the committed s1 and s4 raw files.

### WR-02: scrub.py secret coverage is a "32+ character blob" heuristic and misses common secret shapes

**File:** `spikes/lib/scrub.py:19-31,68`
**Issue:** I verified that these survive unchanged:
- `"password": "hunter2"` and any value under a sensitive key shorter than 32 characters or containing a character outside `[A-Za-z0-9+/=_-]`. The JSON-aware branch requires `BLOB.fullmatch`, which is too strict for a key-name-driven rule.
- `Authorization: Basic <base64>`.
- `github_pat_...`, `AKIA...`, `AIza...`, `xox[bp]-...` and PEM `-----BEGIN ... PRIVATE KEY-----` blocks.
- `Bearer` tokens followed by a lowercase `bearer`.

**Fix:**
- In `walk()`, redact any non-empty string value whose key matches `SENSITIVE_KEY`. Exempt only keys that are known to hold counts or names.
- Extend `RULES` with `github_pat_[A-Za-z0-9_]{20,}`, `AKIA[0-9A-Z]{16}`, `AIza[0-9A-Za-z_-]{35}`, `xox[abprs]-[A-Za-z0-9-]{10,}`, `(?i)authorization:\s*\S+\s+\S+` and a PEM block regex.
- Add the same patterns to `scan-secrets.sh` so the gate is not weaker than the scrubber.

### WR-03: the `sk-` rule has no left word boundary, so it mangles ordinary text and false-positives the gate

**File:** `spikes/lib/scrub.py:22-23`, `spikes/lib/scan-secrets.sh:56`
**Issue:** `sk-[A-Za-z0-9_-]{16,}` matches inside words. I reproduced `task-notification-service-handler` becoming `ta<redacted-key>` and `disk-usage-statistics-report` becoming `di<redacted-key>`. S4 already contains `task_notification` events. The rewrite is silent and non-reversible, and it corrupts fixtures. The scan pattern would also false-positive on the same strings.

**Fix:** anchor both rules, for example `(?<![A-Za-z0-9])sk-ant-...` and `(?<![A-Za-z0-9])sk-[A-Za-z0-9_-]{16,}`. In the grep ERE, use `(^|[^A-Za-z0-9])sk-...`.

### WR-04: scan-secrets.sh fails open on grep errors and on a missing container path

**File:** `spikes/lib/scan-secrets.sh:50,62-63,73-85`
**Issue:**
- `grep -rEIn ... | cut ... >"$TMP" && [ -s "$TMP" ]` runs under `pipefail`. If grep exits 2 (an unreadable file or dangling symlink under `spikes/`) while also finding matches, the pipeline status is 2, the `&&` short-circuits, and the hit is silently dropped. `2>/dev/null` hides the error. Line 62 has the same shape.
- In the container block, `grep ... "$1" 2>/dev/null` in the `if` treats "path does not exist in the container" (exit 2), or an unsupported busybox flag such as `-I` or `--exclude-dir`, as "no leak". The script then prints `CLEAN`.
- `--container ""` (an empty name) silently skips the container scan.

**Fix:**
- Capture grep's status separately and treat anything other than 0 or 1 as a scan failure:
  ```bash
  set +e; out="$(grep -rEIn "${EXCL[@]}" -e "$re" "$TARGET" 2>"$ERR")"; rc=$?; set -e
  [ "$rc" -le 1 ] || { echo "scan: grep error ($rc)" >&2; hits=1; }
  ```
- In the container script, start with `[ -d "$1" ] || { echo "NOPATH"; exit 3; }`.
- Make `--container ""` exit 2.

### WR-05: env-name enumeration via `env | cut -d= -f1` breaks on multi-line values, and the container variant uses `eval`

**File:** `spikes/lib/scan-secrets.sh:38,45,73-77`
**Issue:** Any env var whose value contains a newline yields continuation lines that `cut` passes through as "names". When such a line matches `key|token|secret|password`, `${!name-}` hits a bad substitution. I reproduced `line 38: foo bar_token: invalid variable name`, and the shell aborts mid-scan.

Inside the container, `eval "v=\${$n}"` runs on those names. The container env is trusted here, but this is still an injection-shaped construct.

**Fix:** enumerate with `compgen -e | grep -iE ...` on the host. Use `printenv -0` or `env -0`, keeping the name and value together, in the container, and avoid `eval`. For example: `env -0 | while IFS= read -r -d '' kv; do n=${kv%%=*}; v=${kv#*=}; ...`.

### WR-06: `s5 clone` overwrites the `~/.claude-orca` baseline before it checks whether it is allowed to run

**File:** `spikes/s5/run.sh:34-38`
**Issue:** `orca_fingerprint > "$WORK/orca-before.txt"` runs first. If `~/.claude-personal` already exists and is non-empty, the function then refuses with `return 3`, but the baseline has already been replaced. A second `clone` (an accidental re-run, or the retry that the spike-results "Phương án thay thế" recommends) resets the "before" fingerprint. The later `credentials_unchanged=yes` verdict then only covers the interval since the last `clone`, not since the first.

That verdict is the evidence behind E-06 (credential rotation). The credential fingerprint is the single most important signal in S5.

**Fix:** move the fingerprint write after the refusal check, and never overwrite an existing baseline:
```bash
[ -e "$WORK/orca-before.txt" ] || orca_fingerprint > "$WORK/orca-before.txt"
```

### WR-07: S4 can abort before scrubbing and leave raw stream-json in the tracked tree; `set -e` also makes signal races fatal

**File:** `spikes/s4/run.sh:15,134,137-138,140,144,188`
**Issue:**
- S4 is the only script with `set -euo pipefail`. `kill -TERM "$cp"`, `kill -KILL "$cp"`, `kill -TERM -- "-$pgid"` and `pgid="$(ps -o pgid= -p "$cp" | tr -d ' ')"` are all unguarded. If claude exits on its own in the window between the marker check and the signal (for example a fast `sleep` or a crash), `kill` or `ps` fails and the script exits through the EXIT trap.
- `scrub_raw "$RAW"` runs only at the very end (line 188). An abort leaves the unscrubbed `s4-rN.jsonl` and `s4-rN-*.txt` in `spikes/s4/raw/`. That directory is tracked and is not in `.gitignore`.
- The same ordering issue exists in S1, S2, S5, S7 and S8: the scrub is a final step, so any early exit leaves raw data under `raw/`.

**Fix:**
- Add `|| true` (or a recorded `sig_exit=$?`) to each deliberate signal in `run_one`, and make the `ps -o pgid=` lookup tolerant (`|| pgid=""`, which falls into the existing refusal branch).
- Put the scrub in the EXIT trap in every script that writes to `raw/`: `trap 'scrub_raw "$RAW" >/dev/null 2>&1 || true' EXIT`.
- Add `raw/` to `.gitignore` until the scan has passed, or add a pre-commit check.

### WR-08: `CLEAN_ENV` is a fixed unset list, not "all CLAUDE_CODE_*", and the profile guard only compares exact paths

**File:** `spikes/lib/env.sh:7-9,24,32-39`
**Issue:**
- The comment says the child must not inherit `CLAUDECODE, CLAUDE_CODE_*, ...`, but only 11 named variables are unset. These still pass through to the child `claude`, and they change which account or endpoint is used:
  - `ANTHROPIC_API_KEY`
  - `ANTHROPIC_AUTH_TOKEN`
  - `ANTHROPIC_BASE_URL`
  - `CLAUDE_CODE_OAUTH_TOKEN`
  - `CLAUDE_CODE_USE_BEDROCK` and `CLAUDE_CODE_USE_VERTEX`
  - any other `CLAUDE_CODE_*`
- If the launching shell has one of these set, S1 to S4 and S8 silently run on a different account or billing path than the "work profile `~/.claude`" that spike-results asserts.
- The orca guard compares `realpath` for equality only. A `SPIKE_PROFILE_DIR` that is a subdirectory of `~/.claude-orca` or `~/.claude` passes. `~` is never expanded.

**Fix:** build the unset list dynamically, or run claude under a minimal allowlist environment:
```bash
CLEAN_ENV=(env)
while IFS= read -r v; do CLEAN_ENV+=(-u "$v"); done < <(compgen -e | grep -E '^(CLAUDE|ANTHROPIC)')
```
For the guard, use a prefix test (`case "$resolved/" in "$orca"/*|"$work"/*) refuse ;; esac`).

### WR-09: S6 kill harness records "leftover: none" without checking, and signals a PID it has not re-verified

**File:** `spikes/s6/run.sh:134,149-156,161`
**Issue:**
- `marker-not-started` branch: if the marker was never seen (for example `snapshot` failed, or `ps` formats differ), the code TERMs only the host-side `docker exec` client and writes `leftover: none` / `sweep: none` without taking a final snapshot. Per variant (a) of the spike itself, killing that client does not stop the in-container dsh. A real dsh or `sleep 120` can therefore keep running and be reported as clean. In variant b, `/tmp/s6pid-$NONCE` is also left in the container.
- `CLIENT` comes from `pgrep -P "$LAUNCH" | head -n1` after a fixed `sleep 1` with no retry. If docker is slow to start, `CLIENT` is empty and `kill -TERM "$CLIENT"` (line 161) just errors, so variant (a) never sends a signal. The 5 s wait and `wait` then block on the 120 s task.
- There is no ownership check before `kill -TERM "$CLIENT"`, unlike S4's start-time check. Its ancestry is still `LAUNCH`, but nothing re-confirms it after up to 2 minutes of polling.

**Fix:** retry the `pgrep -P` lookup like S4 does, and re-check `[ "$(ps -o ppid= -p "$CLIENT" | tr -d ' ')" = "$LAUNCH" ]` immediately before each `kill`. In the not-seen branch, take a final `snapshot` and run `tree` on it before writing `leftover:`, and always remove `$PIDF`.

### WR-10: `s5/verify.sh` silently downgrades to the weaker partial checks

**File:** `spikes/s5/verify.sh:16`
**Issue:** with no argument, if `s5-hi-personal.json` is absent and `s5-hi-clone.txt` exists, the verifier switches itself to `--partial`. The strict checks are skipped, and the `PASS` is derived from whatever files happen to be present. A stale `s5-hi-clone.txt` from an earlier `partial` run plus a failed or interrupted `test` run (one that never created `s5-hi-personal.json`) passes with no indication.

The reverse also happens: after `test` and then `partial` runs, `s5-hi-personal.json` is present, so the strict branch runs, and it fails on the `not_run` cross-resume rows.

**Fix:** require the mode explicitly (`--partial` or `--full`) and write `s5-mode.txt` from `run.sh` so the verifier can choose deterministically. Print which mode ran.

## Info

### IN-01: the schedule PID file is a tracked runtime artifact

**File:** `spikes/s3/raw/s3-schedule.pid` (tracked), `spikes/s3/run.sh:18`, `spikes/s3/verify.sh:20`
**Issue:** a stale PID from this machine is committed. On another checkout or host, `kill -0 <pid>` in `run.sh` can hit an unrelated live process and refuse to start the measurement (exit 3). `verify.sh --started` can also false-pass. There is also a race: the guard only sees `s3-schedule.pid`, which `schedule.sh` creates only after the t0 probes, so a second `run.sh` started during t0 is not blocked.
**Fix:** `git rm --cached` the file and ignore it. Use a lock file (`flock`) taken at the top of `run.sh`. Compare `/proc/$pid/cmdline` to `schedule.sh` before treating the PID as the schedule.

### IN-02: S7 appends `exit_code=N` to `.json` and `.jsonl` fixtures

**File:** `spikes/s7/run.sh:25-28`, `spikes/s7/raw/s7-json.json`, `s7-stream.jsonl`
**Issue:** the fixtures are no longer valid JSON or JSONL as a whole. Phase 6 parser tests that load them straight in will fail. Also, `scrub.py` falls to per-line plain-text scrubbing for them, which is weaker than the JSON-aware walk (see WR-02).
**Fix:** write the exit code to a sibling `s7-json.exit.txt` and keep the data files pure.

### IN-03: S3 reports "prefix too small" when claude itself failed, and its `rm -rf` guard is a no-op against a user-set root

**File:** `spikes/s3/run.sh:76-86`, also `s1:11`, `s2:9`, `s3:40`, `s4:174`; `spikes/s8/run.sh:14`
**Issue:**
- A failed probe (non-zero exit, no `.usage`) gives `c=0`, so `small=1` and the message blames the prefix size and advises doubling `S3_PREFIX_BYTES`. The timeline exit code is not consulted.
- `case "$WORK" in "$SPIKE_WORK_ROOT"/sN) rm -rf` is tautological, because `WORK` is built from `SPIKE_WORK_ROOT`. If a user sets `SPIKE_WORK_ROOT` to a precious directory, the script deletes `<root>/sN`.
- S8 has no guard at all: `rm -rf "$S8"`.
**Fix:** in the size gate, check the exit code and `is_error` first and report "claude failed" distinctly. Validate once in `env.sh` that `SPIKE_WORK_ROOT` is an absolute path, not `/` or `$HOME`, and that it ends in `qv-spikes`.

### IN-04: the S5 clone is built from a denylist with different semantics in the rsync and tar branches

**File:** `spikes/s5/run.sh:39-50`
**Issue:**
- rsync excludes `projects/` at any depth, so it also drops unrelated nested `projects/` directories. The tar fallback excludes only `./projects` at the top level.
- A denylist is a leaky way to copy a live credential-adjacent profile. `.claude.json` (carries `oauthAccount` identity), `settings.json` (may hold `env` tokens), `paste-cache/`, `todos/` and `plans/` all travel to the "personal" profile.
- `$PERS` is created with the umask default (755) and only `chmod 700` after the copy. There is a short window where the copied files are readable by others.
**Fix:** use `umask 077` or `mkdir -m 700` before copying. Prefer an allowlist (`settings.json`, `CLAUDE.md`, `commands/`, `agents/`, `skills/`). Anchor the rsync excludes with a leading `/`.

### IN-05: `spike-results.md` passes `check-results.sh --final` only because pending labels were removed by hand, and `cleanup_scr` is suffix-guarded only

**File:** `spikes/spike-results.md:1-8`, `spikes/lib/check-results.sh:151-153`, `spikes/s6/run.sh:17-30`
**Issue:**
- The header records a deviation from D-02: the executor removed the pending labels on the user's instruction. The validator can only count the label string, so it cannot distinguish "user cleared it" from "agent cleared it". The status rests on the prose note alone.
- Separately, `cleanup_scr` runs `rm -rf` as root in the container on any path that ends in `/spikes/s6/scratch`. The check is a suffix match, not an exact one. The assumption that the host path is bind-mounted at the same path in the container is never verified. If the mount is absent, the container's `rm` is a no-op and the scratch directory (which may hold root-owned files) remains on the host. The next run then fails in `make_scr`, because `printf > a.txt` is denied on a root-owned file.
**Fix:**
- Have the user-approval record be machine-checkable, for example `Người xác nhận` plus a hash of the S-section text in a sidecar file.
- Verify the container sees the same directory before relying on it: `docker exec "$C" test -d "$SCR"`. Quote exact equality against `"$REPO/spikes/s6/scratch"` in the guard.

---

_Reviewed: 2026-10-15_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
