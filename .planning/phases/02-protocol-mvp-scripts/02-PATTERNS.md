# Phase 2: Protocol MVP (Scripts) - Pattern Map

**Mapped:** 2026-10-14
**Files analyzed:** 17 (all new; `.gitignore` modified)
**Analogs found:** 4 spec/template analogs, 0 code analogs / 17. Greenfield: no project Python exists.

## Summary

The repo is greenfield. Tracked files: `files/` (Vietnamese specs), `.planning/`, `README.md`, `.gitignore`, and the GSD install under `.claude/`. `spikes/` does not exist yet (Phase 1 not executed). I verified with `git ls-files` that the analogs below are tracked.

The usable analogs are SPEC TEMPLATES in `files/02-phase1-giao-thuc-mvp.md`, not code. Per D-01 these contracts are what Phase 3 ports to Rust, so copy them verbatim:
- Schema of `projects_map.json` (spec 4.1, amended by D-04).
- `.plan.md` template (spec 4.2).
- Three role prompts (spec 4.3).
- Task log template (spec 7).
- Test cases P1-01..P1-10 (spec 6).

Phase 1's planned conventions are the only code-style analogs: bounded, stdin-closed runs (`</dev/null`, `timeout`), a clean-env wrapper, scrub-then-scan before `git add`, and a verify-script-per-unit pattern. They are planned only, not on disk. Mark the CLI flag assumptions (`--output-format stream-json --verbose`, `--allowedTools`, `--max-turns`) as UNVERIFIED until `spikes/spike-results.md` exists (S1, S5, S8).

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `brain/brain` (entry, symlinked to PATH; argparse subcommands scan/add/lead/dev/qa) | CLI entry | request-response | spec 4.4 (`files/02-phase1-giao-thuc-mvp.md` l.102-108) | spec-only |
| `brain/projects.py` (load/save map, scan, add, no-overwrite merge, alias-dup warning) | service | CRUD / file-I/O | spec 4.1 (l.40-57) + D-04..D-07 | spec-only |
| `brain/routing.py` (normalize, exact match, Fallback Ask) | utility | transform | spec 3.2 (l.20-26) + D-08/D-09 | spec-only |
| `brain/roles.py` (build claude argv per role, tool limits, allowlist from plan table) | service | request-response / process | none; Phase 1 `run_claude` (01-RESEARCH Pattern 1, planned) | style-only |
| `brain/plan_parser.py` (parse File column of "Thay đổi" table) | utility | transform | spec 4.2 (l.59-69) | spec-only |
| `brain/streamlog.py` (count Dev turns before first Edit, files read outside plan, tokens) | utility | streaming / transform | none; fixtures from Phase 1 `spikes/s1/raw/` (planned) | none |
| `brain/gitops.py` (branch `task/<slug>`, clean-tree check, commit plan, git status check) | utility | request-response (subprocess) | none | none |
| `brain/config.toml` or `config.json` (models, profile to CLAUDE_CONFIG_DIR map) | config | n/a | Phase 1 D-06 (`01-CONTEXT.md`) | spec-only |
| `brain/prompts/lead.md`, `dev.md`, `qa.md` | prompt/config | n/a | spec 4.3 (l.71-100) | exact (draft) |
| `brain/templates/plan.md` | template | n/a | spec 4.2 | exact |
| `brain/templates/tasklog.md` | template | n/a | spec 7 (l.134-143) | exact |
| `protocol-log/<task>.md` (x5) | doc | batch (script fills, user fills) | spec 7 | exact |
| `tests/test_routing.py` (P1-01..P1-04, P1-05 scan) | test | transform | spec 6 (l.119-132) | spec-only |
| `tests/test_projects.py`, `tests/test_plan_parser.py`, `tests/test_streamlog.py` | test | transform | none | none |
| `tests/protocol-manual.md` (hand script for P1-06..P1-10) | doc | n/a | spec 6 | spec-only |
| `.gitignore` (modify) | config | n/a | existing `.gitignore` | exact |
| `~/.config/agent-brain/projects_map.json` (runtime, NOT in repo) | data | CRUD | spec 4.1 | exact (amended) |

## Pattern Assignments

### `brain/projects.py` + `projects_map.json` schema (service, CRUD)

**Analog:** `files/02-phase1-giao-thuc-mvp.md` l.40-57, amended by D-04.

Spec original (l.41-56):
```json
{ "root": "~/projects",
  "projects": [ { "id": "web-ban-hang", "path": "~/projects/web-ban-hang",
    "name_vi": "Web bán hàng", "aliases": ["shop", "bán hàng", "web shop"],
    "type": "code", "repo": "p.zamba", "profile": "work" } ] }
```
Phase 2 target shape (D-04/D-06/D-07). `repo` comes from `git remote get-url origin` (null if none). `missing` is a flag, never deleted. `profile` is empty for new projects:
```json
{ "scan_roots": ["/abs/dir"],
  "projects": [ { "id": "web-ban-hang", "path": "/abs/dir/web-ban-hang",
    "name_vi": "web-ban-hang", "aliases": [], "type": "code",
    "repo": "git@host:org/web.git", "profile": "", "missing": false } ] }
```
Rules to implement:
- Merge by `id` or `path`. Update only `path`, `repo`, `missing`. Never touch `name_vi`, `aliases`, `profile` (ROUTE-05).
- Scan depth 1 only, requires `.git` (D-06). `brain add <path>` allows media with no git (`type: media`).
- After merge, warn on an alias or normalized name shared by more than one project (ROUTE-06). Warn, do not fail.
- Expand `~`; store absolute paths. File location: `~/.config/agent-brain/projects_map.json` (D-02). Write atomically (temp file + `os.replace`).
- Refuse to run an agent when `profile` is empty (D-07).

### `brain/routing.py` (utility, transform)

**Analog:** spec 3.2 l.24-26 and D-08/D-09. No code. Implementation notes:
```python
import unicodedata, re
def norm(s):
    s = s.replace("đ", "d").replace("Đ", "d")          # not decomposed by NFD
    s = unicodedata.normalize("NFD", s)
    s = "".join(c for c in s if unicodedata.category(c) != "Mn")
    s = re.sub(r"[\s_\-]+", " ", s.lower()).strip()
    return s
```
- Match: exact `norm(query)` equals `norm(id)`, `norm(name_vi)`, or any `norm(alias)`. No substring, no fuzzy.
- 1 match: proceed. 0 or >1: print candidates (all if 0), exit non-zero, never run the agent, never prompt (D-09).
- An exact `id` must resolve a tie (the user re-calls with the exact id). Decide whether a raw exact `id` wins over alias collisions and document it in the plan.
- Test inputs: "web bán hàng" to `web-ban-hang` (P1-01), "shop" (P1-02), ambiguous pair (P1-03), no match (P1-04). Also "Đà Nẵng" to "da nang".

### `brain/roles.py` (service, process)

**Analog:** none in code. Style analog: Phase 1 planned `run_claude` wrapper (`.planning/phases/01-verify-assumptions/01-PATTERNS.md` l.156-159, "Clean-env wrapper"): `env -u CLAUDECODE ... timeout N claude "$@" </dev/null`. Port to Python `subprocess.run(..., stdin=subprocess.DEVNULL, timeout=...)`.

Per-role argv (D-10..D-13; flags UNVERIFIED until S1/S5/S8 results exist):
- Common: `claude -p <input> --output-format stream-json --verbose --append-system-prompt <prompt file> --model <cfg>`; env `CLAUDE_CONFIG_DIR` from profile (PROF-01); save raw stream to the task log dir.
- Lead (Opus): allow `Read, Grep, Glob, Write(.plan.md), Edit(.plan.md), Bash(git log *), Bash(git show *), Bash(git diff *), Bash(git status *)`. Afterwards run `git status --porcelain`; anything other than `.plan.md` is an error (ROLE-01).
- Dev (Sonnet): allow `Read(<each plan file>)`, `Edit(<each plan file>)`, `Read/Edit(.plan.md)`, `Bash(git add *)`, `Bash(git commit *)`. Disallow `Grep, Glob, WebFetch, WebSearch`. No test execution; the script runs or prints "Cách kiểm tra" (ROLE-06).
- QA (Sonnet): Read, `Write(.qa-report.md)`, Bash git read only. Diff base is the plan commit.
- Role config and model names live in the config file, not hardcoded (D-13).

### `brain/plan_parser.py` (utility, transform)

**Analog:** spec 4.2 `.plan.md` template (l.60-69). Parse the markdown table under `## Thay đổi`, take column 2 (`File`), skip the header and separator rows. Return a deduplicated list; strip backticks. An empty list is an error: refuse to start Dev.

### `brain/gitops.py` (utility, subprocess)

**Analog:** none. Per D-14/D-15:
- `git status --porcelain` non-empty before `brain lead`: stop and report, no stash.
- Branch `task/<slug>`: slug from `norm()` of the request (hyphens) or `--name`. Record base branch and base commit in `.plan.md`.
- Commit `.plan.md` right after Lead with message `"<task>: plan"`. Dev commits as `"<task>: <short>"`. QA diffs `<plan-commit>..HEAD`.

### `brain/streamlog.py` (utility, streaming)

**Analog:** none. Fixture source: Phase 1 `spikes/s1/raw/*.jsonl` (planned, not on disk). If absent, build small hand-written fixtures and flag the format as unverified. Compute from stream-json lines: Dev assistant turns before the first `Edit` tool_use; files from `Read` tool_use not in the plan list; token totals from the final `result` usage. Parse tolerantly: skip non-JSON lines, tolerate missing keys.

### `brain/prompts/{lead,dev,qa}.md` and `brain/templates/*.md`

**Analog:** spec 4.3 (l.73-100) verbatim as the base. Additions allowed by "Claude's Discretion": a good-plan example in Lead (spec 8 risk, l.150), and the `.gsd`-or-CLAUDE.md/README fallback. Dev prompt must keep "Vướng mắc then stop". Do not let the Dev prompt reference `.qa-report.md` (D-16).

Task-log template (spec 7, l.136-143), script fills lines 3-4 and the user fills lines 5-6:
```markdown
- Task:
- Dự án:
- Số turn của Dev trước khi bắt đầu sửa:
- Dev có đọc file ngoài plan không:
- QA phát hiện được gì:
- Phải làm lại không, vì sao:
```

### `tests/*` (test, transform)

**Analog:** spec 6 acceptance table (l.121-132) is the test oracle. Automate P1-01..P1-05 (use `tempfile` git repos for scan, including adding a new repo and asserting hand-set aliases survive) and P1-10 (assert the built env contains the profile's `CLAUDE_CONFIG_DIR` without launching claude). Use stdlib `unittest` for D-01 and no deps. P1-06..P1-09 are manual per discretion: write `tests/protocol-manual.md`. Keep test cases ID-named (`test_p1_01_...`) so Phase 3 Rust tests mirror them (CLI-02/CLI-03).

### `.gitignore` (modify)

**Analog:** existing `.gitignore` (currently `.claude/settings.local.json`, `.claude/.gsd-staging/`). Append `__pycache__/`, `*.pyc`, and the stream-json raw log dir if logs are stored in-repo (decide whether raw logs may contain secrets; scan before commit, per Phase 1 scrub-then-scan convention).

## Shared Patterns

### Bounded, stdin-closed agent runs
**Source:** `.planning/phases/01-verify-assumptions/01-PATTERNS.md` l.156-167 (planned).
**Apply to:** every `claude` call in `roles.py`: `stdin=DEVNULL`, a timeout, optional `--max-turns` (S8 unverified).

### Exit codes and messages
No analog. Use: 0 ok, 2 routing ambiguity or none (print list to stdout), 1 other errors. Phase 3 should keep these.

### Secrets
**Source:** `.planning/PROJECT.md` Constraints. `projects_map.json` stays outside the repo (D-02). Never print env values or commit raw logs without a scan.

### Python style
Stdlib only, `#!/usr/bin/env python3`, `argparse` subparsers, functions pure where possible (routing, plan parser, streamlog) so Phase 3 can port them 1:1.

## No Analog Found

| File | Role | Reason |
|---|---|---|
| `brain/roles.py`, `gitops.py`, `streamlog.py` | service/utility | No Python or subprocess code in the repo; Phase 1 conventions only planned |
| `tests/test_*.py` | test | No tests exist; use the spec 6 table as oracle |
| Bash style from `.claude/hooks/*.sh` | n/a | GSD third-party tooling; not applicable to a Python tool |

## Metadata

**Analog search scope:** `git ls-files` for the whole repo (excluding GSD internals), `files/`, `.planning/phases/01-*`.
**Files scanned:** 33 tracked files; 0 project source files.
**Tracked-source gate (#3645):** all named analogs are git-tracked (`files/02-phase1-giao-thuc-mvp.md`, `.planning/...`, `.gitignore`). `spikes/` references are planned and untracked, flagged above.
**Pattern extraction date:** 2026-10-14
