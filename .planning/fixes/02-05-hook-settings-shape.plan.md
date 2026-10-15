# Fix plan: Dev guard hook settings use a shape the real Claude Code ignores

- Repo root: `/mnt/data/IDE/QuangVy-Orchestration`
- Branch: `gsd/phase-02-protocol-mvp-scripts`
- Runs right after plan 02-05, before plan 02-06.
- Executor: Antigravity (`agy`). Reviewer: Claude.
- Python stdlib only. TDD: test commit first, then the fix.

## Why

`brain/guard.py` `write_hook_settings` writes this `settings.json` for the Dev run (passed to `claude --settings`):

```json
{"hooks": {"PreToolUse": [{"matcher": "Read|Edit|Write|MultiEdit|NotebookEdit|Grep|Glob", "command": "<python> -I <guard.py> --policy <policy.json>"}]}}
```

Plan 02-05 specified that flat shape. Claude Code's settings schema nests the command inside a `hooks` list of typed entries, as every hook in this repo's own `.claude/settings.local.json` does:

```json
{"hooks": {"PreToolUse": [{"matcher": "Read|Edit|Write|MultiEdit|NotebookEdit|Grep|Glob", "hooks": [{"type": "command", "command": "<python> -I <guard.py> --policy <policy.json>"}]}]}}
```

With the flat shape the real `claude` does not run the guard, so in live use Dev's read limit (ROLE-02) and the `guard.jsonl` decision log silently disappear. Only the post-run git check would remain. The automated tests did not catch this because `tests/fakes/fake_claude.py` reads the same flat shape.

## Scope

Modify exactly these files:

1. `brain/guard.py` (`write_hook_settings` only)
2. `tests/fakes/fake_claude.py` (the hook-loading part of the `--settings` simulation only)
3. `tests/test_guard.py` (update the settings-shape assertions; add the tests below)

Keep every function signature unchanged. Do not touch anything else, including `.planning/`, `~/.claude`, `~/.claude-orca` and `~/.claude-personal`. Never run the real `claude` binary or `agy`.

## Changes

### 1. `brain/guard.py` `write_hook_settings`

Write `settings.json` as:

```python
settings_data = {
    "hooks": {
        "PreToolUse": [
            {
                "matcher": "Read|Edit|Write|MultiEdit|NotebookEdit|Grep|Glob",
                "hooks": [{"type": "command", "command": cmd}],
            }
        ]
    }
}
```

`cmd` and `policy.json` stay exactly as today.

### 2. `tests/fakes/fake_claude.py`

The fake must copy how Claude Code reads hooks, so that a wrong shape fails the tests:

- For each entry in `settings["hooks"]["PreToolUse"]` whose `matcher` fully matches the tool name, run every item of `entry["hooks"]` that has `type == "command"`, using its `command`, with the same payload, `subprocess.run(..., shell=True, ...)` call and exit-code-2 handling as today.
- Ignore any entry that has no `hooks` list (for example the old flat `{"matcher", "command"}` shape): run nothing for it, exactly as the real CLI would.
- Everything else in the fake stays as it is.

### 3. `tests/test_guard.py`

- Update the existing `write_hook_settings` test to assert the nested shape: `PreToolUse[0]["matcher"]` as today; `PreToolUse[0]["hooks"] == [{"type": "command", "command": <cmd>}]`; and `"command" not in PreToolUse[0]`. The command assertions (`-I`, absolute guard.py, `--policy <abs policy.json>`, shell quoting) stay the same.
- Add `test_fake_ignores_flat_hook_shape`: write a `settings.json` in the flat shape by hand (a guard command whose policy blocks the read), run the fake through a Dev-style scenario that reads a file outside the allowlist with `--settings` pointing at it, and assert the read is **not** blocked (the record's `blocked` list is empty). This proves the fake no longer accepts a shape the real CLI ignores.
- The existing simulation test (a Dev scenario reading `other.py` is blocked by the real guard through the fake) must keep passing with settings from `write_hook_settings`.

## Commits

Stage by name; no `git add -A`, no `git add .`, no `--no-verify`, no amend, no push.

1. `test(02-05): expect Claude Code's nested hook settings shape` — `tests/test_guard.py`, `tests/fakes/fake_claude.py` (the guard tests and the Dev/pipeline e2e tests that rely on blocking fail here; that is expected).
2. `fix(02-05): write guard hook settings in Claude Code's nested shape` — `brain/guard.py`.

## Acceptance criteria

1. `write_hook_settings` produces `{"hooks": {"PreToolUse": [{"matcher": ..., "hooks": [{"type": "command", "command": ...}]}]}}` and no flat `command` key.
2. The fake runs only nested `type: command` hooks and ignores the flat shape.
3. The whole suite passes after commit 2, including the existing guard simulation and every Dev/QA/pipeline e2e test.
4. Exactly two new commits, touching exactly the three files above.

## Verify commands

Run from the repo root (`B` = HEAD before your first commit).

```bash
# AC1: prints the nested entry
python3 -c "
import json,tempfile,pathlib
from brain.guard import write_hook_settings
d=pathlib.Path(tempfile.mkdtemp()); (d/'r').mkdir()
s=json.loads(write_hook_settings(d/'run', d/'r', [d/'r'/'a.py'], [d/'r'/'a.py']).read_text())
e=s['hooks']['PreToolUse'][0]
assert set(e)=={'matcher','hooks'} and e['hooks'][0]['type']=='command' and '--policy' in e['hooks'][0]['command']
print(json.dumps(e)[:160])"

# AC2-AC3
python3 -m unittest tests.test_guard -v
python3 -m unittest discover -s tests -t . -v

# AC4
git log --oneline $B..HEAD
git diff --name-only $B..HEAD | sort
```

## Summary to write (final response, not a file)

Commit hashes and messages, files changed, test output after each commit (failing after commit 1, passing after commit 2), the output of every verify command, and any deviation with its reason.
