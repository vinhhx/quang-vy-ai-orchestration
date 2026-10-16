# Fix plan: defects found in the Phase 2 live run

- Repo root: `/mnt/data/IDE/QuangVy-Orchestration`
- Branch: `gsd/phase-02-protocol-mvp-scripts`
- Runs during plan 02-07's live checkpoint (Task 3, real tasks).
- Executor: Antigravity (`agy`). Reviewer: Claude.
- Python stdlib only. No installs, no network. Never run the real `claude` binary or `agy`.
- TDD per task: a test commit that fails for the new tests, then the fix commit.

## Task 1 (most important): write rules break when the agent changes directory

### Why

Lead's tool rules are `Write(./.plan.md)` and `Edit(./.plan.md)` (QA: `./.qa-report.md`; Dev: `Read/Edit/Write(./<file>)` per plan file). Claude Code resolves `./` against the agent's **current** working directory, and that directory persists across Bash calls. Claude Code auto-allows read-only commands such as `cd X && tail ...`, so in live runs Lead did `cd app/Containers/AppSection/User && ...`, did not `cd` back, and its later `Write` of `<root>/.plan.md` was denied ("Claude requested permissions to write to <root>/.plan.md, but you haven't granted it yet"). This happened in 2 of 8 live Lead runs (`apec/khao-sat`, `app-sale-api/them-api-insert-tuoi-cho-user`); every run where the `cd` was undone or never happened wrote fine. The same can happen to Dev and QA.

### Changes

1. `brain/config.py` `build_env`: always set `CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR=1` in the returned env (Claude Code then returns to the project directory after every Bash command). Set it after the profile handling, for both profiles, overriding any inherited value.
2. Absolute-path rules next to the existing relative ones. Claude Code's permission syntax for an absolute filesystem path is `//` followed by the path without its leading `/` (for `/mnt/data/x/.plan.md` the rule is `Write(//mnt/data/x/.plan.md)`). Keep every existing `./` rule unchanged and add the absolute form:
   - `brain/role_lead.py`: keep `LEAD_ALLOWED_TOOLS` as is; where the `RoleSpec` is built, pass `allowed_tools=LEAD_ALLOWED_TOOLS + abs_rules(proj_path, [".plan.md"], ("Write", "Edit"))`.
   - `brain/role_qa.py`: same with `[".qa-report.md"]`.
   - `brain/role_dev.py`: `dev_allowed_tools(files)` gains an optional keyword `root: Path | None = None` (existing callers keep working); when `root` is given, for each file it also adds `Read(//…)`, `Edit(//…)`, `Write(//…)` for `root / file`. `cmd_dev` passes `root=proj_path`.
   - Put the helper `abs_rules(root, rel_files, tools) -> list[str]` in `brain/agent.py`. Callers pass an already resolved `root`; the helper builds `abs_path = str(root / rel)` (no further resolving; the guard handles symlinks) and returns `f"{tool}(/{abs_path})"`, so each rule starts with `//`.
3. `--dry-run` output shows the new rules automatically (it prints the built command).

### Tests (`tests/test_agent.py` or the existing role e2e tests, and `tests/test_profile.py`)

- `build_env` sets `CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR=1` for work and personal, even when the base env has it as `0`.
- `abs_rules(Path("/a/b"), [".plan.md"], ("Write", "Edit")) == ["Write(//a/b/.plan.md)", "Edit(//a/b/.plan.md)"]`.
- Lead, Dev and QA e2e: argv still contains every old `./` rule the existing tests assert, and also the `//<root>/...` forms for `.plan.md` / `.qa-report.md` / each plan file.
- Update `tests/contract/profile_env.json` expectations only if a contract test fails because of the new variable; do not change any other contract.

## Task 2: `brain scan --root <missing>` is accepted silently

### Why

`brain scan --root /does/not/exist` exits 0, saves the bad path into `scan_roots`, and the summary line silently counts it out. Seen live after a typo (`source_code` vs `source_codes`).

### Changes (`brain/registry.py` `cmd_scan`)

- Before changing `scan_roots`: any `--root` value that is not an existing directory → `BrainError(f"Thư mục không tồn tại: {path}", EXIT_ERROR)`, nothing written to the map.
- Roots already saved in the map that no longer exist (the scan's `bad_roots`): keep them (do not delete user data), but print one stderr line per root: `CẢNH BÁO: thư mục gốc không tồn tại, bỏ qua: <path>`.

### Tests (`tests/test_registry.py` or wherever `cmd_scan` is tested)

- `--root` to a missing dir → exit 1, map file unchanged byte for byte (or not created).
- Map with one valid and one missing saved root → exit 0, warning names the missing root, valid root scanned.

## Task 3: a test writes into the user's real home

### Why

`tests/test_tasklog.py::test_record_run_failure_prints_warning` builds a config without `runs_dir`, so every suite run creates `~/.local/share/agent-brain/runs/proj-1/task-fail/<ts>-lead/` in the real home.

### Changes

- Give that test's config (and any other test config that reaches `execute_role` without `runs_dir`) a `runs_dir` inside the test's temp directory. Search all tests for `execute_role(` and config dicts used with it.
- Do not delete anything under `~/.local/share/agent-brain/`.

### Check

```bash
before=$(find ~/.local/share/agent-brain/runs -mindepth 3 -maxdepth 3 2>/dev/null | wc -l)
python3 -m unittest discover -s tests -t . >/dev/null 2>&1
after=$(find ~/.local/share/agent-brain/runs -mindepth 3 -maxdepth 3 2>/dev/null | wc -l)
test "$before" = "$after" && echo "no leak"
```

## Task 4: manual fixes (`tests/protocol-manual.md`, `brain/README.md`)

1. Preparation section: add how to create the throwaway project used in P1-06..P1-09: `git init` a new folder under a scan root, add a small source file and a test, commit, add a `.gitignore` with `__pycache__/` and `*.pyc` (running tests otherwise leaves untracked files and `brain lead` refuses a dirty tree), `brain scan`, then set its `name_vi`, `aliases` and `profile: "work"`. Say that wherever the manual writes `<du-an-nhap>` you use that project's name.
2. P1-08: the plan must really lack the information. Empty the "Dòng / hàm", "Sửa gì" and "Lý do" cells **and** the body of `## Mục tiêu` and `## Cách kiểm tra`; keep the File column and the first two lines. Explain why: in the live run, clearing only two columns left Mục tiêu fully specifying the work, and Dev correctly implemented it.
3. Gate command: always written as `cd <repo root> && python3 -m brain.tasklog check protocol-log --min 5`, with one sentence that `python3 -m brain...` only works from the repo root.
4. Troubleshooting: add "Lead/QA/Dev bị chặn ghi file của chính nó sau khi `cd`" → fixed by Task 1; if it still happens, check that the dry-run shows the `//` rules and send the run dir to the maintainer.
5. `brain/README.md`: in the assumptions table, add a line for the new env variable and the `//` rule form as **unverified until the next live run** (same style as the other A-xx rows).

## Scope

Modify only: `brain/config.py`, `brain/agent.py`, `brain/role_lead.py`, `brain/role_dev.py`, `brain/role_qa.py`, `brain/registry.py`, `brain/README.md`, `tests/protocol-manual.md`, and test files under `tests/` (including `tests/contract/profile_env.json` only if Task 1 requires it). Keep every existing function signature (Dev's new keyword argument is optional). Do not touch `brain/guard.py`, `brain/prompts/`, `protocol-log/`, `.planning/`, `~/.claude`, `~/.claude-orca`, `~/.claude-personal`, `~/.config/agent-brain/`.

## Commits

Stage by name; no `git add -A`, no `git add .`, no `--no-verify`, no amend, no push.

1. `test(02-07): expect absolute write rules and fixed agent cwd`
2. `fix(02-07): keep agent cwd at project root and add absolute path rules`
3. `test(02-07): brain scan rejects a missing root`
4. `fix(02-07): brain scan rejects a missing root and warns on saved ones`
5. `test(02-07): keep tasklog test run dirs out of the real home` (one commit is fine here: test-only change)
6. `docs(02-07): manual and README updates from the live run`

## Acceptance criteria

1. `brain lead <p> "<req>" --dry-run` shows both `Write(./.plan.md)` and `Write(//<root>/.plan.md)` in the printed command.
2. `build_env` sets `CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR=1` for both profiles.
3. Dev's argv has `Read/Edit/Write` in both forms for every plan file and `.plan.md`; QA's has both forms for `.qa-report.md`.
4. `brain scan --root /nonexistent` exits 1 and leaves the map untouched; saved missing roots produce a warning.
5. The full suite passes, and the "no leak" check prints `no leak`.
6. Manual and README contain the Task 4 items.
7. Exactly the commits above, touching only files in Scope.

## Verify commands

Run from the repo root (`B` = HEAD before your first commit). Use a temporary `XDG_CONFIG_HOME` for anything that runs `brain` so the real `~/.config/agent-brain/` is never read or written.

```bash
python3 -m unittest discover -s tests -t . 2>&1 | tail -3
python3 -c "from pathlib import Path; from brain.agent import abs_rules; print(abs_rules(Path('/a/b'), ['.plan.md'], ('Write','Edit')))"
python3 -c "from brain.config import build_env, load_config; c=load_config(); print(build_env('work', c, {'CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR':'0'})['CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR'])"
T=$(mktemp -d); XDG_CONFIG_HOME=$T brain/brain scan --root /nonexistent-xyz; echo "exit=$?"; ls $T/agent-brain 2>&1
# the "no leak" check from Task 3
grep -n "du-an-nhap\|__pycache__\|Mục tiêu\|cd .* && python3 -m brain.tasklog" tests/protocol-manual.md | head
git log --oneline $B..HEAD
git diff --name-only $B..HEAD | sort
```

## Summary to write (final response, not a file)

Commit hashes and messages, files changed, test output after each test commit (failing) and each fix commit (passing), the output of every verify command, and any deviation with its reason.
