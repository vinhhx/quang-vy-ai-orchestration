# Fix plan: three review findings from plans 02-01 and 02-02

- Repo root: `/mnt/data/IDE/QuangVy-Orchestration`
- Branch: `gsd/phase-02-protocol-mvp-scripts`
- Runs after plan 02-02 and before plan 02-03 (02-03 uses `validate_plan`; 02-04 adds `brain scan`, which would overwrite the map; 02-05 turns plan paths into Dev's write allowlist).
- Executor: Antigravity (`agy`). Reviewer: Claude.
- Python stdlib only. TDD: write the tests first, see them fail, then fix.

## Why

The review of plan 02-01 (see `.planning/phases/02-protocol-mvp-scripts/02-01-SUMMARY.md`, section "Review (Claude)") found two defects that no test covers.

1. **`brain/projects.py` `load_map` treats an unreadable map as empty.** Today invalid JSON, a read error or a top-level value that is not an object all return `{"scan_roots": [], "projects": []}`. Plan 02-01 says only a *missing* file returns the empty map. With the current code, a typo in the hand-edited `projects_map.json` reads as "(trống) — chạy: brain scan --root", and plan 02-04's `cmd_scan` does `load_map` → `scan` → `save_map`. That would replace the user's hand-set `name_vi`, `aliases` and `profile` with a freshly scanned map.
2. **`brain/cli.py` `main` hides real import errors.** It catches every `ImportError`/`AttributeError` while loading a handler and prints `Lệnh '<name>' chưa có trong bản cài này`. A handler module that exists but has a broken import (a bug) gets the same message as a module that is not installed yet, so the real error is lost.
3. **`brain/planfile.py` `validate_plan` accepts paths that name the whole repo or a glob.** A File cell of `.`, `./` or `src/..` normalizes to `.`, and `src/*`, `**` or `src/[ab].py` pass as-is; `validate_plan` reports no error and `plan_files` returns them. Plan 02-05 turns `plan_files` into Dev's `Write(./<file>)`/`Edit(./<file>)` rules and the guard hook's write allowlist (D-12, ROLE-06), so one such cell in an LLM-written plan would let Dev write anywhere. A File cell must name one concrete file.

## Scope

Modify or create exactly these files:

1. `brain/projects.py` (modify `load_map` only)
2. `brain/cli.py` (modify the handler-loading block in `main` only)
3. `brain/planfile.py` (modify `validate_plan` only)
4. `tests/contract/plan.json` (append cases only; do not change existing cases)
5. `tests/fakes/broken_handler.py` (new)
6. `tests/test_robustness.py` (new)

Do not touch anything else. In particular, keep every function signature in `brain/` unchanged (later plans import them), do not edit any other test, and do not touch `.planning/`, `~/.claude`, `~/.claude-orca` or `~/.claude-personal`. Never run the real `claude` binary or `agy`.

## Changes

### 1. `brain/projects.py` — `load_map(path=None) -> dict`

Keep the signature. New behaviour:

- File does not exist → return `{"scan_roots": [], "projects": []}` (unchanged).
- File exists but cannot be read (`OSError`), is not valid JSON (`json.JSONDecodeError` / `ValueError`, including invalid UTF-8), its top-level value is not an object, or it has a `projects` or `scan_roots` key whose value is not a list → raise
  `BrainError(f"Không đọc được {p}: {reason} — sửa file rồi chạy lại (brain không ghi đè file này)", EXIT_ERROR)`
  where `p` is the resolved map path and `reason` is a short description (the exception message, or e.g. `phải là JSON object`, `'projects' phải là danh sách`).
- Otherwise `setdefault` `scan_roots` and `projects` to `[]` and return the dict (unchanged).
- Remove the bare `except Exception` that returns the empty map. Do not catch broader than the cases above.
- `load_map` must never write, rename or delete the file.

### 2. `brain/cli.py` — handler loading in `main`

Replace the current `try: import_module + getattr / except (ImportError, AttributeError, ModuleNotFoundError)` block with:

```python
    mod_name, func_name = handler_target.split(":")
    try:
        mod = importlib.import_module(mod_name)
    except ModuleNotFoundError as e:
        if e.name != mod_name:
            raise
        sys.stderr.write(f"Lệnh '{args.subcommand}' chưa có trong bản cài này\n")
        return EXIT_ERROR
    func = getattr(mod, func_name, None)
    if not callable(func):
        sys.stderr.write(f"Lệnh '{args.subcommand}' chưa có trong bản cài này\n")
        return EXIT_ERROR
```

So only "the handler module itself is not installed" and "the module has no such function" print the friendly message. Any other import error (a missing dependency inside an existing module, a syntax error, an exception at import time) propagates with its traceback. Leave the rest of `main` (argparse, `BrainError` and `KeyboardInterrupt` handling, `HANDLERS`) unchanged.

### 3. `brain/planfile.py` — `validate_plan(plan, root=None)`

Keep every signature, every existing error code and the order in which existing codes are reported. Add two codes, checked for each non-empty `change.file` after the existing `protocol_file` check and before the `outside_root` check, in the same `if/elif` chain:

- `not_a_file:<f>` when the normalized path is `.` (that is the repo root, from `.`, `./`, `src/..` and the like).
- `glob_path:<f>` when the path contains any of `*`, `?`, `[` or `]`.

`plan_files` and `parse_plan` stay unchanged (they still return what the plan says; `validate_plan` is the gate).

### 4. `tests/contract/plan.json` — append cases

Append these cases to `cases` (same shape as the existing ones: `id`, `markdown`, `expect` with `valid`, `error_codes`, `files`, `has_blockers`). Each `markdown` is a full valid plan, like the existing `plan-valid-full` case, whose only change row has the given File cell. `error_codes` uses full `code:detail` strings, like the existing cases (e.g. `absolute_path:/etc/passwd`).

| id | File cell | expected codes | expected `files` |
|----|-----------|----------------|------------------|
| `plan-dot-root` | `.` | `not_a_file:.` | `["."]` |
| `plan-dot-slash-root` | `./` | `not_a_file:.` | `["."]` |
| `plan-dir-up-to-root` | `src/..` | `not_a_file:.` | `["."]` |
| `plan-glob-star` | `src/*` | `glob_path:src/*` | `["src/*"]` |
| `plan-glob-double-star` | `**` | `glob_path:**` | `["**"]` |
| `plan-glob-brackets` | `src/[ab].py` | `glob_path:src/[ab].py` | `["src/[ab].py"]` |

All six have `valid: false` and `has_blockers: false`. Bump nothing else in the file (`version` stays 1).

### 5. `tests/fakes/broken_handler.py` (new)

A module whose import fails on a dependency that does not exist, standing in for a handler with a bug:

```python
# Test fixture: a handler module whose own import is broken (see tests/test_robustness.py).
import brain_nonexistent_dependency_for_tests  # noqa: F401


def cmd_broken(args):
    return 0
```

### 6. `tests/test_robustness.py` (new, unittest)

Use `tests.helpers.Sandbox` for CLI runs (in `setUp`, with `addCleanup(self.sandbox.cleanup)`). Map path is `self.sandbox.xdg_config / "agent-brain" / "projects_map.json"`. Tests:

- `test_corrupt_map_refused_and_left_unchanged`: write the bytes `{"projects": [ {"id": "a",` (truncated JSON) to the map path; `run_brain("lead", "x", "y", "--dry-run")` exits 1; stderr contains `projects_map.json` and `không ghi đè`; stdout does not contain `(trống)`; the file's bytes are unchanged.
- `test_non_object_map_refused`: map content `[]` → exit 1, stderr contains `JSON object`.
- `test_projects_not_list_refused`: map content `{"scan_roots": [], "projects": {}}` → exit 1.
- `test_missing_map_is_empty`: no map file → exit 2 and stdout contains `(trống) — chạy: brain scan --root <thư mục>` (existing behaviour kept).
- `test_load_map_direct`: call `brain.projects.load_map(path)` in-process on a temp file with invalid JSON and assert it raises `BrainError` with `code == 1`; on a missing path assert it returns `{"scan_roots": [], "projects": []}`.
- `test_missing_handler_module_message`: with `unittest.mock.patch.dict(brain.cli.HANDLERS, {"scan": "brain.no_such_module_xyz:cmd"})`, `brain.cli.main(["scan"])` returns 1 and stderr (capture with `contextlib.redirect_stderr(io.StringIO())`) contains `chưa có trong bản cài này`.
- `test_missing_handler_function_message`: patch `"scan"` to `"brain.config:no_such_function_xyz"` → returns 1 with the same message.
- `test_broken_handler_import_not_hidden`: patch `"scan"` to `"tests.fakes.broken_handler:cmd_broken"`; `brain.cli.main(["scan"])` raises `ModuleNotFoundError` whose `.name == "brain_nonexistent_dependency_for_tests"`.

These tests must not depend on whether `brain/registry.py`, `brain/role_dev.py` or `brain/role_qa.py` exist (later plans add them), so always patch `HANDLERS` rather than relying on a real missing module.

## Commits

Stage files by name; no `git add -A`, no `git add .`, no `--no-verify`, no amend, no push.

1. `test(02-01): cover unreadable project map and handler import errors` — `tests/test_robustness.py`, `tests/fakes/broken_handler.py` (tests fail at this point; that is expected).
2. `fix(02-01): refuse unreadable project map, stop hiding handler import errors` — `brain/projects.py`, `brain/cli.py`.
3. `test(02-02): contract cases for root and glob paths in the File column` — `tests/contract/plan.json` (`tests.test_planfile` fails at this point; expected).
4. `fix(02-02): reject root and glob paths in plan File cells` — `brain/planfile.py`.

## Acceptance criteria

1. An existing but unreadable map makes `brain` exit 1 with a message naming the file, and the file is left byte-identical.
2. A missing map still behaves as an empty map (exit 2, `(trống)` hint).
3. A missing handler module or function still prints `chưa có trong bản cài này` and exits 1; a broken import inside an existing handler module propagates.
4. `validate_plan` reports `not_a_file` for a File cell that normalizes to `.` and `glob_path` for one containing `*`, `?`, `[` or `]`; all existing contract cases still pass unchanged.
5. No signature in `brain/` changed; the full test suite passes.
6. Exactly four new commits, touching exactly the six files above.

## Verify commands

Run from the repo root (`B` = the HEAD before your first commit).

```bash
# AC1-AC3: all tests ok
python3 -m unittest tests.test_robustness -v

# AC4: contract tests ok, and the six new case ids are present (prints 6)
python3 -m unittest tests.test_planfile -v
python3 -c "import json;ids={c['id'] for c in json.load(open('tests/contract/plan.json',encoding='utf-8'))['cases']};print(len(ids & {'plan-dot-root','plan-dot-slash-root','plan-dir-up-to-root','plan-glob-star','plan-glob-double-star','plan-glob-brackets'}))"

# AC4: existing cases untouched; prints "existing cases unchanged"
python3 -c "import json,subprocess,sys;old=json.loads(subprocess.check_output(['git','show',sys.argv[1]+':tests/contract/plan.json']));new=json.load(open('tests/contract/plan.json',encoding='utf-8'));assert new['cases'][:len(old['cases'])]==old['cases'] and new['version']==1;print('existing cases unchanged')" $B

# AC5: whole suite passes (the 3 tests from 02-01, the 02-02 tests and the new ones)
python3 -m unittest discover -s tests -t . -v

# AC5: the bare except is gone from load_map; prints nothing
sed -n '/^def load_map/,/^def /p' brain/projects.py | grep -n 'except Exception'

# AC5: signatures unchanged; prints one line each
grep -n '^def load_map(path' brain/projects.py
grep -n '^def main(argv' brain/cli.py

# AC6: four commits; exactly the six files
git log --oneline $B..HEAD
git diff --name-only $B..HEAD | sort
```

## Summary to write (final response, not a file)

Commit hashes and messages, files changed, the output tail of every verify command, the test output showing the new tests failing after each test commit (1 and 3) and passing after each fix commit (2 and 4), and any deviation with its reason.
