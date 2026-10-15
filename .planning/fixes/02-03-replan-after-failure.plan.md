# Fix plan: re-plan after a failed Lead run is blocked by brain's own `.plan.md`

- Repo root: `/mnt/data/IDE/QuangVy-Orchestration`
- Branch: `gsd/phase-02-protocol-mvp-scripts`
- Runs right after plan 02-03, before wave 4.
- Executor: Antigravity (`agy`). Reviewer: Claude.
- Python stdlib only. TDD: test commit first, then the fix.

## Why

Plan 02-03 has this must-have (`.planning/phases/02-protocol-mvp-scripts/02-03-PLAN.md`, `must_haves.truths`, "[edge ROLE-01 concurrency]"):

> If Lead times out or the user presses Ctrl+C, brain … does not commit .plan.md; re-running brain lead on the same task branch regenerates the plan and keeps the original base_commit.

It is not met. `cmd_lead` writes the skeleton `.plan.md` before the run. When the run times out, is interrupted, or the plan fails `validate_plan`, that file stays uncommitted (untracked on a new branch, modified on a re-plan). The next `brain lead` with the same request runs the dirty-tree check first, sees `.plan.md` and refuses with exit 1. Reproduced by the reviewer:

```
run1 (sleep 5, timeout 1 s): exit 1 | branch task/them-nut | status ?? .plan.md
run2 (same request):         exit 1 | Working tree có thay đổi chưa commit — dừng, không tự stash: .plan.md
```

The plan's own action said to keep the 02-01 dirty-tree check unchanged, which contradicts the must-have. The other 02-03 must-have ("[edge ROLE-05 concurrency]") still holds: a second `brain lead` on a tree dirty from an unfinished run refuses with exit 1. Both are satisfied by this rule:

> When the current branch is already this task's branch (`task/<slug>`, i.e. a re-plan), `.plan.md` alone does not count as dirty: it is the protocol file that the re-plan regenerates. Any other changed or untracked path still refuses with exit 1, and a leftover `.plan.md` on any other branch still refuses.

## Scope

Modify exactly these files:

1. `brain/role_lead.py` (`cmd_lead` only)
2. `tests/test_lead_e2e.py` (add tests only; do not change existing tests)

Keep every function signature unchanged. Do not touch anything else, including `.planning/`, `~/.claude`, `~/.claude-orca` and `~/.claude-personal`. Never run the real `claude` binary or `agy`.

## Change: `brain/role_lead.py` `cmd_lead`

1. Compute `task = slugify(args.name or args.request)` (with its existing empty-slug BrainError) and `branch = f"task/{task}"` **before** the dirty-tree check instead of after it. Keep the messages unchanged.
2. The dirty-tree check becomes:
   ```python
   dirty = gitops.changed_paths(proj_path)
   if gitops.current_branch(proj_path) == branch:
       dirty -= {".plan.md"}
   if dirty:
       raise BrainError(  # same message and code as today
           ...
       )
   ```
3. Nothing else changes. The existing re-plan logic (`is_replan`, reading `base_branch` / `base_commit` / `created_at` from the existing `.plan.md` meta, falling back to `head_sha` when there is none) already handles the leftover skeleton, whose meta brain wrote itself.

## Tests: add to `tests/test_lead_e2e.py`

Use the existing `Sandbox`, `lead_scenario` and `write_config` helpers. Each test sets up a project like the existing tests do.

- `test_replan_after_timeout_regenerates_and_keeps_base_commit`: record `base = HEAD sha` of `main`. Write config with `timeout_seconds` lead = 1; run `brain lead <id> "thêm nút"` with scenario `{"sleep": 5}` → exit 1, current branch `task/them-nut`, `.plan.md` untracked. Restore the default config (`write_config()`), run the same command with `lead_scenario([("src/app.py", "app()", "trả về 2", "demo")])` → exit 0; HEAD subject `them-nut: plan`; `git status --porcelain` empty; the committed `.plan.md`'s brain-meta `base_commit` equals `base`.
- `test_replan_after_validation_failure_succeeds`: first run's scenario writes a plan with no change rows (keeping the meta line, `keep_meta: true`) → exit 1 and `.plan.md` uncommitted; second run with `lead_scenario(...)` → exit 0 and HEAD subject `<task>: plan`.
- `test_replan_refuses_when_other_files_dirty`: after a timed-out first run (as above), also modify `src/app.py` by hand; the second run → exit 1, the output lists `src/app.py`, the fake record count does not increase, and `src/app.py` keeps the hand edit.
- `test_leftover_plan_on_other_branch_refuses`: on `main` (no task branch), create an untracked `.plan.md` by hand; `brain lead <id> "việc khác"` → exit 1, output lists `.plan.md`, no new branch `task/viec-khac`, no fake record.

## Commits

Stage by name; no `git add -A`, no `git add .`, no `--no-verify`, no amend, no push.

1. `test(02-03): re-plan after a failed Lead run` — `tests/test_lead_e2e.py` (the first two new tests fail here; expected).
2. `fix(02-03): allow re-plan over brain's own uncommitted .plan.md` — `brain/role_lead.py`.

## Acceptance criteria

1. Re-running `brain lead` on the same task branch after a timeout or a failed validation commits a new plan and keeps the first run's `base_commit`.
2. Any other dirty path, or a leftover `.plan.md` on another branch, still refuses with exit 1 before any branch or agent side effect, and brain changes no user file.
3. All existing tests pass unchanged.
4. Exactly two new commits, touching exactly the two files above.

## Verify commands

Run from the repo root (`B` = HEAD before your first commit).

```bash
# AC1-AC2: the four new tests ok
python3 -m unittest tests.test_lead_e2e -v 2>&1 | grep -E 'test_replan_after_timeout|test_replan_after_validation|test_replan_refuses_when_other|test_leftover_plan_on_other_branch'

# AC3: whole suite passes
python3 -m unittest discover -s tests -t . -v

# AC4
git log --oneline $B..HEAD
git diff --name-only $B..HEAD | sort
```

## Summary to write (final response, not a file)

Commit hashes and messages, files changed, test output after each commit (new tests failing after commit 1, passing after commit 2), the output of every verify command, and any deviation with its reason.
