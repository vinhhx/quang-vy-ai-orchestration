# CLAUDE.md

## Work split: Claude plans and reviews, Antigravity executes

Every non-trivial change in this repo follows three steps. Claude does not write the implementation itself.

1. **Analyze → plan (Claude).** Claude reads the code and the `.planning/` artifacts, then writes the plan (GSD `PLAN.md`, or a `.plan.md` for work outside GSD). The plan must be executable by another agent without this conversation: exact files, steps, acceptance criteria and verify commands.
2. **Execute (Antigravity).** Claude hands the plan to Antigravity through the `agy` CLI and waits for it to finish. Claude does not edit the files in the plan's scope while agy runs.
3. **Review (Claude).** Claude reviews agy's result against the plan: diff, acceptance criteria, verify commands, and secrets. Problems go back to agy as a follow-up plan, or are reported to the user. Claude only fixes things directly when the user asks.

Trivial edits (typos, a one-line config change, docs) and the user's explicit requests to do something directly are exempt.

## How to call agy

Based on spike S7 (`spikes/spike-results.md`, agy 1.3.5):

```bash
cd /mnt/data/IDE/QuangVy-Orchestration
timeout 45m agy -p "<prompt: path to the plan + instructions>" \
  --output-format stream-json --print-timeout 40m \
  </dev/null > "$LOG" 2> "${LOG%.jsonl}.stderr.txt"
```

- Use the `agy` CLI only. Never run the bare `antigravity` binary (it launches the IDE).
- Always: stdin from `/dev/null`, stdout to a file (agy has pipe-output bugs), an explicit `--print-timeout` (the docs and the installed version disagree on the default), and an outer `timeout`.
- agy has **no turn-limit flag** (`--max-turns` is rejected). Bound runs by time only.
- Read the outcome from the `result` event: `status`, `response`, `usage`. The stream uses an `event` key (`init`, `step_update`, `result`), not Claude's `type`/`tool_use`.
- **Tool permissions are unverified.** By default, tools that need approval are soft-denied and agy still exits 0. After every run, check the diff for real changes before trusting `status: SUCCESS`. Use `--dangerously-skip-permissions` only with the user's approval.
- To stop a run, follow the tested kill method from S4: record the descendants, send SIGTERM to the recorded PID, then sweep the recorded descendants. Never use `pkill`/`killall` or kill by process name.

## Prompt handed to agy

Keep it self-contained:
- the absolute path of the plan file, and "read it fully first"
- the repo root and the current branch; commit each task (no `git add -A`, no `--no-verify`)
- what not to touch (`.planning/STATE.md`, `ROADMAP.md`, untracked `.planning/` files, `~/.claude-orca`)
- the verify commands to run, and what to write in the summary (files changed, commits, deviations)

## Review checklist (Claude, after agy)

- `git log` / `git diff` since the pre-run HEAD: every change is inside the plan's declared files.
- The plan's verify commands pass. Re-run them yourself; do not trust agy's report.
- Acceptance criteria and must-haves are met, not just "files exist".
- No secrets or credentials in the diff (and run `spikes/lib/scan-secrets.sh` where it applies).
- Record the result: GSD `SUMMARY.md` / `VERIFICATION.md` for GSD work, otherwise a short report to the user.
