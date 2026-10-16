# Fix plan: switch the live kill checks from `sleep 307` to a python sleep (03-13)

Repo: /mnt/data/IDE/QuangVy-Orchestration, branch `main`.

## Problem

Live P2-02 (2026-10-16, claude 2.1.301) showed that Claude Code's Bash tool refuses a foreground `sleep 307`:

```
<tool_use_error>Blocked: standalone sleep 307. To wait for a condition, use Monitor with an until-loop ...
To wait for a command you started, use run_in_background: true. Do not chain shorter sleeps to work around this block.</tool_use_error>
```

`sleep 307 && echo xong` is blocked the same way. The agent then retries with `run_in_background: true` or gives up, so the guide's P2-02/P2-03 scenario never produces a `bash → sleep` grandchild and does not test what it claims to test.

What worked live (P2-02 passed, recorded in `protocol-log/nghiem-thu-p2.md`):

- the wrapper inserted `Bash(python3 *)` after the first `--allowedTools` (instead of `Bash(sleep *)`);
- the request was: `Trước khi làm gì khác, chạy đúng lệnh bash này ở foreground (KHÔNG dùng run_in_background) và chờ nó xong: python3 -c 'import time; time.sleep(307)'`
- the log line was `tool Bash python3 -c 'import time; time.sleep(307)'`;
- the tree was `claude` → `/bin/bash -c … eval 'python3 -c …'` (own PGID and own SID) → `python3 -c import time; time.sleep(307)`;
- `pgrep -af 'time.sleep'` also matches the `agent-brain` and `claude` processes, because the request text is in their argv. The python process is found reliably with `pgrep -f '^python3 -c import time; time.sleep\(307\)'`.

## Files in scope

- `scripts/live/claude-allow-sleep.sh`
- `tests/test_live_helpers.py`
- `agent-brain/LIVE-CHECKS.md`

Nothing else. Keep the wrapper's file name (the README and the guide refer to it). Do not edit `protocol-log/nghiem-thu-p2.md` (it holds the user's results), `agent-brain/README.md`, anything under `agent-brain/src/`, `agent-brain/tests/`, `brain/`, or `.planning/`.

## Steps

1. **Test first.** In `tests/test_live_helpers.py`, change the expected argv of `test_wrapper_inserts_sleep_rule` so the inserted argument is `Bash(python3 *)` instead of `Bash(sleep *)`. Leave `test_wrapper_without_allowed_tools_passes_through` as it is. Add a third test, `test_wrapper_inserts_only_once`, that runs the wrapper with two `--allowedTools` flags (`-p x --allowedTools Read --allowedTools Grep`) and expects `-p x --allowedTools Bash(python3 *) Read --allowedTools Grep`. Run it (the first test must now fail) and commit: `test(03-13): expect python sleep rule in live wrapper`.

2. **Wrapper.** In `scripts/live/claude-allow-sleep.sh`, insert `Bash(python3 *)` instead of `Bash(sleep *)`. Rewrite the header comment to say that the script is used only for the live P2-02/P2-03 checks on the throwaway repo, that claude 2.1.301 blocks a foreground `sleep N` in the Bash tool so the checks hold the process with `python3 -c 'import time; time.sleep(307)'`, and that `Bash(python3 *)` is broad and must never be used for real work. Keep the rest of the logic (first `--allowedTools` only, `exec "${AGENT_BRAIN_REAL_CLAUDE:-claude}"`). Run the tests and commit: `fix(03-13): live wrapper allows python sleep instead of sleep`.

3. **Guide.** In `agent-brain/LIVE-CHECKS.md`, update the P2-02 and P2-03 sections (Vietnamese, same style as the file):
   - Change the P2-02 heading to `### P2-02: Kill khi agent đang chạy lệnh dài (python sleep 307)`.
   - Add a short note at the start of P2-02 explaining the change: claude 2.1.301 blocks a foreground `sleep N` ("Blocked: standalone sleep"), so the checks use `python3 -c 'import time; time.sleep(307)'`, and the wrapper allows `Bash(python3 *)` for these checks only.
   - In every P2-02/P2-03 `run lead` command, replace the request with exactly: `"Trước khi làm gì khác, chạy đúng lệnh bash này ở foreground (KHÔNG dùng run_in_background) và chờ nó xong: python3 -c 'import time; time.sleep(307)'"`. Make the shell quoting valid: the request is in double quotes and contains single quotes. Keep the `--name` values (p202-kill, p203-a, p203-b, p203-c), and write the P2-03 B and C commands out in full rather than only "chạy với --name".
   - Replace the trigger `tool Bash sleep 307` with `tool Bash python3 -c 'import time; time.sleep(307)'`.
   - Replace the PID-recording step with a tree view plus a precise lookup:
     `ps -eo pid,ppid,pgid,sid,args --forest | grep -A3 '[a]gent-brain run lead'` and
     `pgrep -f '^python3 -c import time; time.sleep\(307\)'`.
     Explain why: a plain `pgrep -f time.sleep` also matches agent-brain and claude, whose argv contains the request. Say that the bash child runs in its own PGID and SID, so the check must cover every recorded PID, not only the process group.
   - Rename the PID placeholders to `<claude>,<bash>,<python>`, and use `ps -p <claude>,<bash>,<python> -o pid,args` (header only = clean) in P2-02 and all three P2-03 cases. Add `pgrep -af '^python3 -c import time; time.sleep\(307\)' || echo sạch` as a second check.
   - Add one cleanup line to P2-02 and to each P2-03 case: `rm -f /mnt/data/IDE/du-an-nhap/.plan.md`, after the branch delete. A killed Lead leaves an untracked `.plan.md` that blocks the next run.
   - Do not change other sections (P2-01, P2-07, P2-09, D-02, E-06), and do not add `pkill`, `killall` or `kill` commands. Signals still come only from `agent-brain kill`, Ctrl+C or closing the terminal.
   Commit: `docs(03-13): live checks use python sleep`.

## Verify (run all from the repo root, report the output)

```bash
python3 -m unittest tests.test_live_helpers -v
python3 -m unittest discover -s tests -t . -q
grep -c 'Bash(sleep' scripts/live/claude-allow-sleep.sh tests/test_live_helpers.py agent-brain/LIVE-CHECKS.md
grep -cF 'Bash(python3 *)' scripts/live/claude-allow-sleep.sh tests/test_live_helpers.py agent-brain/LIVE-CHECKS.md
grep -n 'sleep 307' agent-brain/LIVE-CHECKS.md
grep -cF "time.sleep(307)" agent-brain/LIVE-CHECKS.md
grep -cE 'pkill|killall' agent-brain/LIVE-CHECKS.md scripts/live/claude-allow-sleep.sh
bash -n scripts/live/claude-allow-sleep.sh && echo syntax-ok
for id in P2-01 P2-02 P2-03 P2-07 P2-09 D-02 E-06; do grep -qF "$id" agent-brain/LIVE-CHECKS.md || echo "missing $id"; done
git diff --stat HEAD~3..HEAD
```

## Acceptance criteria

- `tests.test_live_helpers`: 3 tests, all OK; the full Python suite is OK.
- `Bash(sleep` appears 0 times in the three files; `Bash(python3 *)` appears in each of them.
- `grep -n 'sleep 307' agent-brain/LIVE-CHECKS.md` prints nothing (the only `307` left is inside `time.sleep(307)`); `time.sleep(307)` appears at least 6 times (P2-02 command and trigger, P2-03 A/B/C commands, pgrep pattern).
- Every P2-02/P2-03 `run lead` line is valid shell: copy one into `bash -n -c '<line>'` and it exits 0.
- 0 `pkill`/`killall`; all seven section IDs are still present.
- Only the three files in scope changed, in three commits in the order above.
