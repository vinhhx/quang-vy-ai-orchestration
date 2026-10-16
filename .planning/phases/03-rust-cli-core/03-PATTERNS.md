# Phase 3: Rust CLI Core - Pattern Map

**Mapped:** 2026-10-16
**Files analyzed:** 27 new files (greenfield crate `agent-brain/`, plus repo-level scripts/fixtures/fakes)
**Analogs found:** 25 / 27 (every analog is a Phase 2 Python file, so these are behaviour specs to port, not Rust idioms to copy)

All analog paths below were checked as git-tracked (`git ls-files brain tests spikes`). There is no Rust code in the repo yet (`agent-brain/` does not exist), so "analog" always means the Python module whose behaviour the Rust module must reproduce. Line numbers refer to the current `brain/*.py` and `tests/*` files. No gitignored mirror paths are used.

CONTEXT.md is stale on one point: `brain/` and `spikes/` exist now. `tests/contract/*.json` already exist (D-03 is already partly met: normalize, routing, scan, plan, qa_report, profile_env).

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `agent-brain/Cargo.toml`, `Cargo.lock` | config | n/a | no analog (RESEARCH.md "Standard Stack" TOML is the spec) | none |
| `agent-brain/src/error.rs` | utility | request-response | `brain/__init__.py` (1-15) | exact |
| `agent-brain/src/config.rs` | config | file-I/O | `brain/config.py` (24-76) + `brain/config.json` | exact |
| `agent-brain/src/profile.rs` | utility | transform | `brain/config.py` (79-124) + `tests/contract/profile_env.json` | exact |
| `agent-brain/src/routing.rs` | utility | transform | `brain/routing.py` (1-68) + `tests/contract/{normalize,routing}.json` | exact |
| `agent-brain/src/projects.rs` | service | CRUD (file-I/O) | `brain/projects.py` + `brain/registry.py` + `tests/contract/scan.json` | exact |
| `agent-brain/src/planfile.rs` | service | transform | `brain/planfile.py` + `tests/contract/{plan,qa_report}.json` | exact |
| `agent-brain/src/gitops.rs` | utility | request-response (subprocess) | `brain/gitops.py` | exact |
| `agent-brain/src/guard.rs` (+ `guard` subcommand) | middleware (hook) | request-response | `brain/guard.py` + `tests/test_guard.py` | exact |
| `agent-brain/src/stream.rs` | service | streaming/transform | `brain/streamlog.py` + `tests/fixtures/stream/*.jsonl`, `spikes/s1/raw/s1.jsonl` | exact (plus new bad_json/partial-usage logic) |
| `agent-brain/src/db.rs` | model/store | CRUD | `brain/tasklog.py` (concept only: per-run record) | partial; schema is in RESEARCH.md "Suggested Data Model" |
| `agent-brain/src/runner.rs` | service | event-driven/streaming | `brain/agent.py` `execute_role` (190-259 of listing) | role-match; kill tree is NEW |
| `agent-brain/src/roles.rs` | controller | request-response | `brain/role_lead.py`, `role_dev.py`, `role_qa.py`, `agent.py` `abs_rules`/`build_command` | exact |
| `agent-brain/src/fastpath.rs` | controller | request-response | none in Phase 2 (git helpers from `brain/gitops.py`; argv shape from `agent.py:build_command`) | partial |
| `agent-brain/src/stats.rs` | service | batch (SQL aggregate) | none (loosely `brain/tasklog.py`) | none |
| `agent-brain/src/term.rs` | utility | streaming sink | `brain/agent.py` stderr lines (`[brain] ...`) | partial |
| `agent-brain/src/main.rs`, `lib.rs` | controller | request-response | `brain/cli.py` (1-100) | exact |
| `agent-brain/assets/prompts/{lead,dev,qa}.md` | config | static | `brain/prompts/*.md` (byte copy) | exact |
| `agent-brain/assets/templates/{plan,qa-report}.md` | config | static | `brain/templates/{plan,qa-report}.md` (byte copy) | exact |
| `agent-brain/assets/config.default.json` | config | static | `brain/config.json` | exact |
| `agent-brain/tests/common/mod.rs` | test | file-I/O | `tests/helpers.py` `Sandbox` | exact |
| `agent-brain/tests/contract_*.rs` | test | transform | `tests/test_routing.py:74-99`, `test_projects.py:17-143`, `test_planfile.py:33,81`, `test_profile.py:96` | exact |
| `agent-brain/tests/{cli_e2e,fastpath,db_stats,runner_kill}.rs` | test | e2e | `tests/test_{lead,dev,qa,pipeline}_e2e.py`, `tests/fakes/fake_claude.py` | role-match |
| `tests/contract/stream.json`, `tests/contract/guard.json` (NEW fixtures) | test fixture | static | `tests/contract/plan.json` (format), `tests/test_streamlog.py`, `tests/test_guard.py` (source cases) | role-match |
| `tests/fakes/fake_claude_tree.py` (NEW) | test fake | event-driven | `tests/fakes/fake_claude.py` | role-match |
| `scripts/build-release.sh` (NEW) | config/script | batch | `brain/install.sh`, `spikes/lib/*.sh` (shell style only) | partial |
| Python test loaders for stream.json/guard.json (NEW, small) | test | transform | `tests/test_routing.py:74-99` | exact |

## Pattern Assignments

### `agent-brain/src/error.rs` (utility)

**Analog:** `brain/__init__.py` lines 3-15. Keep the exit codes and the one-error-type design.

```python
EXIT_OK = 0
EXIT_ERROR = 1
EXIT_ROUTING = 2
EXIT_PROTOCOL = 3
EXIT_BLOCKED = 4
EXIT_INTERRUPTED = 130

class BrainError(Exception):
    def __init__(self, message: str, code: int = EXIT_ERROR):
```

Rust shape: `pub struct BrainError { pub message: String, pub code: i32 }` plus `pub const EXIT_*`. Same values.

**Error-to-output mapping** (`brain/cli.py` 87-98): routing failures (code 2) print the message to **stdout**, every other `BrainError` prints to **stderr**; both append `\n`. KeyboardInterrupt returns 130. RESEARCH Pitfall 3: map clap usage errors to exit 1 via `Cli::try_parse()` (keep `--help`/`--version` at 0), because 2 is the Fallback Ask code.

---

### `agent-brain/src/main.rs` / `lib.rs` (controller, request-response)

**Analog:** `brain/cli.py`

**Command surface to keep** (cli.py 22-61): subcommands `scan --root DIR (repeatable)`, `add PATH --type code|media`, `lead PROJECT REQUEST [--name SLUG] [--dry-run]`, `dev PROJECT [--dry-run]`, `qa PROJECT [--dry-run]`. Phase 3 spec renames these to `projects scan`, `projects add`, `run lead|dev|qa`, and adds `kill <task_id>`, `status`, `inspect`, `stats`, `guard --policy`. RESEARCH Open Question 2: the positional request is required for `lead`, optional and ignored (stderr note) for `dev`/`qa`.

**Dispatch + error mapping pattern** (cli.py 87-99):
```python
try:
    res = func(args)
    return res if isinstance(res, int) else EXIT_OK
except BrainError as e:
    if e.code == EXIT_ROUTING:
        sys.stdout.write(f"{e.message}\n")
    else:
        sys.stderr.write(f"{e.message}\n")
    return e.code
except KeyboardInterrupt:
    return EXIT_INTERRUPTED
```
No args: print help to stderr and return `EXIT_ERROR` (cli.py 74-76).

---

### `agent-brain/src/config.rs` (config, file-I/O)

**Analog:** `brain/config.py`

**Validation errors to keep verbatim** (config.py 38-61): `Không tìm thấy file cấu hình: {path}`, `Lỗi đọc cấu hình {path}: {e}`, `Cấu hình không hợp lệ (phải là JSON object)`, `Cấu hình thiếu trường 'models'`, `Cấu hình thiếu model cho vai trò '{r}'` (roles lead/dev/qa), `Cấu hình thiếu trường 'profiles'`, `Cấu hình thiếu profile '{p}'` (work/personal). All exit 1.

**Paths** (config.py 63-70 and `projects.py` 15-19):
```python
xdg = os.environ.get("XDG_DATA_HOME")
if xdg: return (Path(os.path.expanduser(xdg)) / "agent-brain" / "runs").resolve()
return (Path.home() / ".local" / "share" / "agent-brain" / "runs").resolve()
```
Map: `$XDG_CONFIG_HOME/agent-brain/projects_map.json`, else `~/.config/agent-brain/projects_map.json`. Python's override env var is `BRAIN_CONFIG`; Rust uses `AGENT_BRAIN_CONFIG` and `AGENT_BRAIN_DB` (RESEARCH "Validation Architecture"). DB at `~/.local/share/agent-brain/brain.db` (D-06), created 0600 in a 0700 dir.

**Default config** is `brain/config.json` (claude_bin `claude`; models lead `opus`, dev `sonnet`, qa `sonnet`; profiles work/personal; `common_args ["--strict-mcp-config"]`; `timeout_seconds` 1800/1800/1200). Add `kill_grace_secs: 3`, `inspect_max_diff_bytes`, `inspect_model: "sonnet"` (D-11, D-14). Embed with `include_str!("../assets/config.default.json")`.

---

### `agent-brain/src/profile.rs` (utility, transform)

**Analog:** `brain/config.py` lines 9-21 and 79-124; fixture `tests/contract/profile_env.json` (7 cases, `base_env` in, `expect.absent` / present out).

**Constant list to copy exactly** (config.py 9-21): `CLAUDECODE`, `CLAUDE_CODE_CHILD_SESSION`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_PID`, `CLAUDE_EFFORT`, `CLAUDE_VC_ENV`, `CLAUDE_CODE_MESSAGING_SOCKET`, `CLAUDE_CODE_MESSAGING_TOKEN`, `CLAUDE_CODE_SESSION_ATTENDED`, `CLAUDE_CODE_ENTRYPOINT`, `CLAUDE_CODE_EXECPATH`.

**check_profile + build_env core** (config.py 79-117):
```python
if not profile or not str(profile).strip():
    raise BrainError("Dự án chưa đặt profile (work|personal) — sửa projects_map.json rồi chạy lại", EXIT_ERROR)
if profile not in profiles:
    raise BrainError(f"Profile '{profile}' không hợp lệ. Các profile đã cấu hình: {names}", EXIT_ERROR)  # names = ", ".join(sorted(...))

env = dict(base_env)   # injected base env, not os.environ (testable)
for var in NESTED_SESSION_VARS: env.pop(var, None)
for var in prof_cfg.get("unset_env", []): env.pop(var, None)
if claude_config_dir is None: env.pop("CLAUDE_CONFIG_DIR", None)
else:
    if dir_str.startswith("~"):
        home = env.get("HOME", ...)
        dir_str = os.path.normpath(os.path.join(home, dir_str[1:].lstrip("/\\")))
    env["CLAUDE_CONFIG_DIR"] = dir_str
env["CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR"] = "1"
```
`describe_profile` strings (config.py 120-124): `CLAUDE_CONFIG_DIR không đặt → mặc định ~/.claude` / `CLAUDE_CONFIG_DIR={dir}`. The `[brain] ...` banner line printed before a run (agent.py 587) uses it.

---

### `agent-brain/src/routing.rs` (utility, transform)

**Analog:** `brain/routing.py` (whole file, 68 lines) + `tests/contract/normalize.json` (8 cases: `input`, `normalized`, `slug`) + `tests/contract/routing.json` (`projects` + `cases` with `query` and `expect {kind: match|none|ambiguous, project | candidates}`).

**normalize / slugify** (routing.py 17-41):
```python
s = text.replace("đ", "d").replace("Đ", "d")
s = unicodedata.normalize("NFD", s)
s = "".join(c for c in s if unicodedata.category(c) != "Mn")
return re.sub(r"[\s_\-]+", " ", s.lower()).strip()
# slugify: norm.replace(" ", "-"); re.sub(r"[^a-z0-9\-]", "", slug); re.sub(r"-+", "-", slug).strip("-")
# max_len=48: cut at 48, then at last "-" if index > 0, then strip("-")
```
RESEARCH has a compiled Rust version (`unicode-normalization` + `regex`, build regexes once via `OnceLock`).

**resolve order** (routing.py 44-68): empty/whitespace query -> `RoutingError(kind="none", candidates=all)`; first an exact `id` match on the stripped query returns immediately; then normalized match against `id`, `name_vi`, each alias; 1 match returns, 0 -> `none` (candidates = all projects), >1 -> `ambiguous` (candidates = matches).

**Fallback Ask text** is in `brain/projects.py` 54-77 (belongs in `projects.rs`):
```python
header = f'Tên "{q}" khớp {n} dự án — dừng, không chạy agent. Gọi lại bằng id chính xác:'   # ambiguous
header = f'Không tìm thấy dự án "{q}" — dừng, không chạy agent. Danh sách dự án:'          # none
# empty list: "  (trống) — chạy: brain scan --root <thư mục>"
# else per candidate: f"  {id}\t{name_vi}\t{path}"
# raise BrainError("\n".join(lines), EXIT_ROUTING)   # printed to stdout, exit 2
```

---

### `agent-brain/src/projects.rs` (service, CRUD on `projects_map.json`)

**Analog:** `brain/projects.py` (+ `brain/registry.py` for the scan/add command output). Fixture: `tests/contract/scan.json`, 10 multi-step cases with ops `scan | add | edit | mkrepo | rmdir | mkdir`, executed by `tests/test_projects.py` `_materialize_layout` / `_execute_case` (lines 17-143). The Rust harness must re-implement that materialiser (layout roots, `git`, `remote`, `children`; `{R}`-style path formatting; expectations `exit_codes`, `projects{id: fields}`, `absent`, `warnings[{key, ids}]`).

**load_map** (projects.py 22-51): missing file returns `{"scan_roots": [], "projects": []}`; unreadable or wrong type errors say `Không đọc được {p}: {e} — sửa file rồi chạy lại (brain không ghi đè file này)`, with variants for "phải là JSON object", "'projects' phải là danh sách", "'scan_roots' phải là danh sách". Use `serde_json::Value` with `preserve_order` so unknown keys and key order survive (fixture `R-unknown-keys`).

**save_map** (projects.py 98-117), the exact on-disk format Python writes, which Rust must match:
```python
fd, tmp_path = tempfile.mkstemp(prefix="projects_map_", suffix=".tmp", dir=str(p.parent))
content = json.dumps(data, ensure_ascii=False, indent=2) + "\n"
... f.write(content); f.flush(); os.fsync(f.fileno()); os.replace(tmp_path, p)
# on error: remove tmp
```

**git_remote credential stripping** (projects.py 120-138): for `http`/`https` remote URLs containing username/password, rebuild without userinfo but keep `:port`; otherwise return the raw string; failure or empty -> `None`.

**unique_id / new_entry** (141-173): collision on `base` with a different realpath -> `base-2`, `base-3`, ...; new entry fields in order: `id, path, name_vi (= folder name), aliases [], type, repo (git_remote only if type == "code", else None), profile "", missing False`.

**scan** (176-256), key rules: iterate roots (invalid roots go to `bad_roots`), children sorted by name; a child counts only if it is a dir (symlinks followed) and `.git` exists; known realpath -> refresh `repo`, clear `missing` (counts `restored`) else `unchanged`; unknown -> `new_entry`, record `suffixed` when id != folder name; afterwards projects not seen in this scan: path no longer a dir -> `missing = True`, else restore/unchanged; return `{added, missing (total flagged), restored, unchanged, bad_roots, suffixed}`. Depth is 1 level only (fixture `D-06-depth`).

**add_project** (259-277): existing realpath returns `(entry, False)` and clears `missing`; type default = `"code"` if `.git` exists else `"media"`; CLI only applies `--type` when passed explicitly (registry.py 59-60 inspects argv; in clap use `Option<String>`).

**find_duplicates** (280-308): keys = normalized id, name_vi, aliases; keys shared by more than one distinct id; sorted by key.

**Command output strings** (`brain/registry.py`; keep byte-identical apart from the program name):
```python
f"Thư mục không tồn tại: {r_path}"                               # exit 1
"Chưa có scan_roots — chạy: brain scan --root <thư mục>"          # exit 1
f"CẢNH BÁO: thư mục gốc không tồn tại, bỏ qua: {br}"             # stderr
f"Đã quét {num_roots} thư mục gốc: +{added} mới, {missing} thiếu (missing), {restored} khôi phục, {unchanged} không đổi"  # stdout
f"Trùng id: đặt id là '{new_id}' thay cho '{base}'"               # stdout
f'CẢNH BÁO: tên/alias trùng "{key}": {ids_str}'                   # stderr, ids joined ", "
f"Đã thêm {id}" / f"Đã có {id} — giữ nguyên các trường sửa tay"
```
Roots are deduplicated by realpath and stored as absolute paths in `scan_roots` (registry.py 22-34).

**require_runnable** (projects.py 80-95), in order: `missing` -> `Dự án '{id}' bị đánh dấu missing — không thể chạy agent`; no path -> `... không có đường dẫn hợp lệ`; not a dir -> `Đường dẫn dự án '{id}' không tồn tại: {path}`; not repo root (`git rev-parse --show-toplevel` == path) -> `Thư mục dự án '{id}' không phải repo git: {path}`; then `check_profile`. All exit 1.

SQLite `projects` table is a mirror synced from the JSON on every run (D-05); JSON stays the source of truth.

---

### `agent-brain/src/planfile.rs` (service, transform)

**Analog:** `brain/planfile.py` + `tests/contract/plan.json` (24 cases with `markdown`, `expect {valid, error_codes (ordered list), files, has_blockers}`, optional `root_layout` that needs symlink materialisation) and `tests/contract/qa_report.json` (`markdown` -> `expect.verdict`).

**Constants** (planfile.py 10-12): `PLAN_SECTIONS = ("Mục tiêu", "Thay đổi", "Không được đụng tới", "Cách kiểm tra", "Vướng mắc")`, `QA_SECTIONS = (...)`, `QA_VERDICTS = ("Đạt", "Cần sửa")`.

**Meta line** (31-34, 72-93): written as `<!-- brain-meta: {json, sorted keys, ensure_ascii=False, ">" escaped as \u003e} -->`; read with regex `<!--\s*brain-meta:\s*(.*?)\s*-->` (DOTALL), invalid JSON -> `None`; `set_meta` replaces the first match (use a closure, not a template string) or inserts after line 1. Compare metas as parsed values.

**Path normalisation** (37-45): strip, strip backticks, strip quotes, NFC, `posixpath.normpath`, drop leading `./`. Needs a hand-written normpath (RESEARCH "acceptable small hand-rolls").

**Skeleton** (48-69): template `templates/plan.md` / `qa-report.md`; replace `<tên task>` in line 1 with first line of title (max 100 chars, NFC); insert meta line as line 2; join with `\n` plus trailing newline.

**Section/title parsing** (96-178): headings `## ` / `# `, NFC, `heading.startswith(section_name)` (so `Vướng mắc (Dev ghi vào đây...)` matches); title from `# Plan:` (case-insensitive) or first `# ` that is not `# qa:`. Table rows: cell splitting on `|` not preceded by `\` (Rust `regex` lacks look-behind; hand-split), skip header row (`#`, `file`) and `^:?-+:?$` separator rows, require >= 5 cells, comma-separated file cells expand to one `Change` per file.

**validate_plan error order** (181-267), must stay identical: `missing_title`; `missing_section:<name>` in PLAN_SECTIONS order; all `bad_row:<n>`; all `empty_file_cell:<n>`; `no_changes`; then per change in order the first matching of `absolute_path:`, `home_path:`, `parent_traversal:`, `git_dir:`, `protocol_file:`, `not_a_file:`, `glob_path:`, `outside_root:` (the last is de-duplicated and needs a resolved-path containment check).

**plan_files / has_blockers / parse_qa_verdict** (270-305): files de-duplicated in order; blockers = section "Vướng mắc" after removing `<!--.*?-->` (DOTALL) is non-empty; verdict = per line strip leading `[>\s*+-]+(?:\d+\.)?\s*`, strip `kết luận:` prefix (IGNORECASE), strip `` `*"'_ `` and whitespace, NFC + casefold compare to `đạt` / `cần sửa`; exactly one distinct verdict -> that verdict, else `None`.

---

### `agent-brain/src/gitops.rs` (utility, subprocess request-response)

**Analog:** `brain/gitops.py`

**Core wrapper** (gitops.py 20-33): args as an array (never a shell), `cwd=root`, env copy plus `GIT_TERMINAL_PROMPT=0`; nonzero exit -> `GitError(stderr.strip() or f"Lệnh git thất bại (exit {code}): git {args}")` (code 1).

**Functions to port with identical git invocations:**
```python
is_repo_root: git rev-parse --show-toplevel  == resolved path
changed_paths: git status --porcelain=v1 -z --untracked-files=all   # parse entry[:2] status, entry[3:] path; R/C statuses consume the next NUL field too
current_branch: git branch --show-current        # None on failure/empty
head_sha: git rev-parse HEAD
branch_exists: git rev-parse --verify refs/heads/<name>
create_branch: git checkout -b <name> HEAD
commit_paths: git add -f -- <paths>; git commit -m <msg> -- <paths>; return head_sha
diff_names: git diff --name-only a..b
find_commit_by_subject: git log --format=%H %s HEAD   # first exact subject match
commit_subjects: git log --reverse --format=%H %s a..b
```
New for FAST-01/02 (no Python analog; use the same wrapper): `git status --short`, `git log --oneline -5`, `git diff --stat <base>`, and the inspect diff.

---

### `agent-brain/src/guard.rs` + `guard` subcommand (hook middleware, request-response)

**Analog:** `brain/guard.py` (228 lines) + `tests/test_guard.py` (source of cases for the new `tests/contract/guard.json`).

**Constants** (11-13): `WRITE_TOOLS = (Write, Edit, MultiEdit, NotebookEdit)`, `READ_TOOLS = (Read, Grep, Glob)`.

**Settings shape** (74-83). The only change: the command becomes `'<current_exe>' guard --policy '<policy.json>'` (single-quote shell quoting) instead of `python -I guard.py` (DIST-01):
```python
{"hooks": {"PreToolUse": [{"matcher": "Read|Edit|Write|MultiEdit|NotebookEdit|Grep|Glob",
                           "hooks": [{"type": "command", "command": cmd}]}]}}
```
`policy.json` keys: `root`, `read_allow` (list or null), `write_allow`, `log`; all paths NFC-normalised absolute (45-63). `settings.json`, `policy.json`, `guard.jsonl` live in the run dir.

**Decision logic** (90-224), to port as a pure `decide(policy, payload)`:
- Exit 2 (fail closed) on: missing `--policy`, unreadable/non-object policy, empty/invalid/non-object stdin.
- Tool not in guarded set -> allow (exit 0), log `tool {name} not guarded`.
- Write tools: path from `file_path` (`notebook_path` for NotebookEdit); missing -> block (`missing path for write-class tool`).
- `Read` needs `file_path` (missing -> block); `Grep`/`Glob` use `path` or fall back to `cwd`.
- Relative paths join `cwd` (or `policy.root`); resolve with non-strict realpath, NFC.
- Write: allowed iff resolved path is in realpath'd `write_allow`. Read: `read_allow is None` -> allow ("read-free mode"); else `Grep`/`Glob` always blocked ("prohibited when read_allow is set"); `Read` allowed iff in `read_allow`.
- Block message to stderr: `brain guard: chặn {tool} {raw_path} — ngoài danh sách được phép của vai trò (xem .plan.md)` then exit 2. In Rust change the `brain guard:` prefix only if no fixture reads it.
- Log entry keys (22-28): `ts` (ISO UTC), `tool`, `path`, `decision`, `reason`; appended as one JSON line with `ensure_ascii=False`; logging errors are swallowed.

---

### `agent-brain/src/stream.rs` (service, streaming/transform)

**Analog:** `brain/streamlog.py` (229 lines); fixtures `tests/fixtures/stream/{dev_blocked,dev_two_reads_then_edit,malformed_lines,parallel_reads}.jsonl`, `spikes/s1/raw/s1.jsonl`, `spikes/s4/raw/s4-r{1,2,3}.jsonl`. Golden numbers for the new `tests/contract/stream.json` are in RESEARCH.md line 633.

**Soft line parsing** (streamlog.py 19-36). Phase 3 adds bytes-level reading (`read_until`, lossy UTF-8) and stores the bad line raw as a `bad_json` event (LOG-02), where Python only counted it:
```python
self.total_lines += 1
line_s = line.strip()
if not line_s: continue
try: data = json.loads(line_s)
except Exception: self.bad_lines += 1; continue
if not isinstance(data, dict): self.bad_lines += 1; continue   # "[1, 2]" is a bad line too
```

**Turn counting** (109-132): `assistant_turns` = number of distinct `message.id` (fall back to top-level `id`; no id counts every line). S1 has 3 assistant lines but 2 ids and `num_turns: 2`.

**Result/usage** (170-199): `usage` fields `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens` read from the last `type: result` event (default 0); `permission_denials` = length of the list; `total_cost_usd`, `num_turns`, `subtype`, `is_error`, `duration_ms`. `format_ok = result_event is not None`. New in Rust: partial usage when there is no result (sum last-seen `message.usage` per distinct id, `usage_source = 'partial'`).

**KPI** (133-163): `tool_counts`, `reads`, `edits` (paths from `file_path` / `notebook_path` / `path`); `turns_before_first_edit = assistant_turns - 1` at the first Edit/Write/MultiEdit/NotebookEdit on a non-protocol file (protocol files `.plan.md`, `.qa-report.md`); `reads_outside_plan` is computed against plan files when provided. Keep `_normalize_tool_path` semantics (41-75), but drop or isolate the hard-coded `/work/proj/` test prefix (lines 65-66, 69-70), which is a fixture artefact.

**Terminal summarisation** (new, D-09): one line per event `HH:MM:SS kind summary`; truncate by `chars()`, not byte slices (Vietnamese).

---

### `agent-brain/src/runner.rs` (service, event-driven/streaming)

**Analog (partial):** `brain/agent.py` `execute_role` (the section around lines 190-259 of the combined listing, i.e. the file's `execute_role` body). Port the contract, replace the mechanics.

**What to keep** from Python:
```python
stderr_log = run_dir / "stderr.txt"; raw_log = run_dir / "stream.jsonl"
proc = subprocess.Popen(argv, cwd=project["path"], env=env, stdin=subprocess.DEVNULL, stdout=PIPE, stderr=stderr_f, ...)
except FileNotFoundError: raise BrainError(f"Không tìm thấy lệnh claude: {argv[0]}", EXIT_ERROR)
timeout = cfg.get("timeout_seconds", {}).get(spec.role, 1800)
reader_thread.join(timeout=5)     # bounded drain after wait (Pitfall 8)
```
Banner to stderr before the run: `[brain] {role} · {project_id} · profile {profile} ({desc}) · model {model}`; end line `[brain] {role} xong · exit {code} · log {raw_log}`. Phase 3 replaces these with the D-09 one-line-per-event sink and a final summary line, and prints `task N, run M` at start (D-08).

**What changes (no Python analog, follow RESEARCH Patterns 1-3):** tokio `current_thread`; spawn with `process_group(0)`; reader task + mpsc + `select!` over lines, `child.wait()`, SIGINT/SIGTERM/SIGHUP, timeout; kill = record `/proc` descendants -> SIGTERM group -> grace (`kill_grace_secs`, default 3) -> SIGKILL group plus recorded survivors (matched by pid + start time); second SIGINT = immediate SIGKILL; Drop guard; PID-reuse-safe identity `(pid, pgid, pid_start)`; reconcile at startup marks stale `running` rows `lost`. RESEARCH lines 419-465 contain the compiled `/proc` stat parser, `alive`, `signal_group` and the `select!` skeleton to copy.

**Timeout/interrupt flags** carried into outcome (agent.py `RunResult`): `exit_code`, `timed_out`, `interrupted`, `started_at`, `finished_at`. Role code branches on them (see `roles.rs`). In Rust these become `runs.status` in `succeeded | failed | timed_out | killed | lost`.

---

### `agent-brain/src/roles.rs` (controller, request-response)

**Analogs:** `brain/role_lead.py`, `brain/role_dev.py`, `brain/role_qa.py`, plus `brain/agent.py` for `abs_rules`, `build_command`, `new_run_dir`.

**Shared helpers (agent.py):**
```python
def abs_rules(root, rel_files, tools):          # lines 38-50 of the file
    clean = rel[2:] if rel.startswith("./") else rel; clean = clean.lstrip("/")
    abs_path = str(root / clean)
    rules.append(f"{tool}(/{abs_path})")        # yields Write(//abs/path)

def new_run_dir(cfg, project, task, role):      # base / project["id"] / task / f"{ts}-{role}"
    ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ"); chmod 0o700 on every dir created
```
**build_command argv order** (agent.py `build_command`): `claude_bin, -p, <user_input>, --output-format, stream-json, --verbose, --model, <models[role]>, --append-system-prompt, <prompt text>, *common_args, [--max-turns N], *extra_args, [--allowedTools *allowed], [--disallowedTools *disallowed]`. Reject any arg starting with `--dangerously-skip-permissions`, `--allow-dangerously-skip-permissions`, `--permission-mode`, or containing `bypassPermissions` -> `Cờ quyền không được phép: {arg}`. Missing model -> `Không tìm thấy model cho vai trò: {role}`. The prompt now comes from `include_str!` instead of reading `brain/prompts/*.md` (RESEARCH Open Question 7: keep a byte-equality test against `brain/prompts` and `brain/templates`).

**Lead** (`role_lead.py` 17-34 tool lists; 37-177 flow). Tools: allow `Read, Grep, Glob, Write(./.plan.md), Edit(./.plan.md), Bash(git log *), Bash(git show *), Bash(git diff *), Bash(git status *)` + `abs_rules(root, [".plan.md"], ("Write","Edit"))`; deny `WebFetch, WebSearch, NotebookEdit, Task`. Flow order to preserve:
1. config, select project, `require_runnable`.
2. `task = slugify(name or request)`; empty -> `Không thể tạo tên task từ yêu cầu — hãy truyền cờ --name <slug>`; `branch = f"task/{task}"`.
3. Dirty check (`changed_paths`, minus `.plan.md` when already on that branch) -> `Working tree có thay đổi chưa commit — dừng, không tự stash:\n<sorted paths>`.
4. Replan if already on the branch (reuse meta `base_branch`, `base_commit`, `created_at`); branch exists but not checked out -> `Nhánh '{branch}' đã tồn tại nhưng chưa được checkout — hãy checkout nhánh đó hoặc dùng cờ --name khác`; stacking warning `Cảnh báo: Nhánh mới '{branch}' được tạo từ nhánh task '{curr}' (stacking)`.
5. meta = `{task, branch, base_branch, base_commit, request, created_at}`.
6. `--dry-run` prints `Project:`, `Profile: <p> (<desc>)`, `Model:`, `Branch:`, `Command: <shlex-joined, prompt replaced by <brain/prompts/lead.md>>`, exit 0, with no side effects.
7. create branch (unless replan), write skeleton `.plan.md`, run role.
8. Post-checks: timeout -> `Lead chạy quá thời gian (timeout) trên task '{task}'`; exit != 0 -> `Lead thất bại với mã lỗi {code} trên task '{task}'` (exit 1); extra changed paths besides `.plan.md` -> print `Lead đã sửa file ngoài .plan.md — vi phạm giao thức (ROLE-01):` + `  {path}` lines + hint, return `EXIT_PROTOCOL` (3); missing `.plan.md`; reset meta if changed; `validate_plan` errors -> `Kế hoạch trong .plan.md không hợp lệ:\n  - {e}...\nSửa tay .plan.md rồi: git -C {path} commit -m "{task}: plan" -- .plan.md`; commit `.plan.md` with message `{task}: plan`; print the plan path.
The user_input text (lines 105-112 of role_lead.py) is part of CLI-03 behaviour; copy it byte for byte. Phase 3 adds D-07: also create the `tasks` row and print `task N, run M`.

**Dev** (`role_dev.py` 25-46 tools; 49-219 flow). `dev_allowed_tools(files, root)`: add `.plan.md` to the file list; per file `Read(./f) Edit(./f) Write(./f)` + abs rules for `Read/Edit/Write`; then `Bash(git add *)`, `Bash(git commit *)`. Deny `Grep, Glob, WebFetch, WebSearch, NotebookEdit, Task`. Flow:
1. Needs `.plan.md` (`Không tìm thấy .plan.md ở gốc dự án`), meta with `task` (`File .plan.md không có metadata hợp lệ`), branch == `meta.branch or task/<task>` (`Không ở nhánh task '{expected}' (nhánh hiện tại: '{cur}') — không đoán`), clean tree (`...hãy commit thay đổi tay với message '{task}: plan':`), valid plan with files, no blockers (`Vướng mắc chưa xử lý trong .plan.md — hãy xử lý trước khi chạy dev`).
2. `dev_base = head_sha`; `read_allow = write_allow = plan files + .plan.md` (resolved); `write_hook_settings`; extra args `["--settings", settings_path]`.
3. After run: timeout/interrupt/nonzero exit print the changed files to stderr first, then raise with `Dev chạy quá thời gian (timeout)...` / `Dev bị ngắt bởi người dùng` (130) / `Dev thất bại với mã lỗi {code}...`.
4. Violation check: `diff_names(dev_base, HEAD) | changed_paths`, NFC, minus plan files and `.plan.md`; any -> print `Dev đã sửa file ngoài plan — vi phạm giao thức (ROLE-06):`, list, `brain không tự hoàn tác`, return 3.
5. Auto-commit pending paths with `{task}: vướng mắc` (if blockers) or `{task}: {first Mục tiêu line, bullet prefix stripped, max 72 chars, else "dev"}`; warn on commit subjects not starting `{task}: ` (`CẢNH BÁO: Commit {sha[:7]} có subject không đúng chuẩn '{task}: ...': '{subj}'`).
6. Print `Các file đã sửa:` list, `Cách kiểm tra (brain không tự chạy — hãy chạy tay trong {path}):\n{section}`; if blockers print `Vướng mắc:\n...` and `Bước tiếp theo: sửa .plan.md hoặc chạy lại brain lead`, return `EXIT_BLOCKED` (4).

**QA** (`role_qa.py` 19-36 tools; 39-141 flow). Allow `Read, Grep, Glob, Write(./.qa-report.md), Edit(./.qa-report.md), Bash(git diff|log|show|status *)` + abs rules for `.qa-report.md`; deny `WebFetch, WebSearch, NotebookEdit, Task`. Flow: same plan/branch/meta preconditions as Dev; dirty minus `.qa-report.md`; find commit with subject `{task}: plan` (`Không tìm thấy commit '{task}: plan' trên nhánh hiện tại`); code changes since plan commit excluding `.plan.md`/`.qa-report.md` must exist (`chưa có thay đổi code sau plan`); `diff_cmd = git diff <plan_commit>..HEAD -- . ':(exclude).plan.md' ':(exclude).qa-report.md'` (also the `--dry-run` output: `Diff command: ...`); write QA skeleton with meta `{task, plan_commit}` and title = `meta.request or task`; after run: extra changed besides `.qa-report.md` -> `QA đã sửa file ngoài .qa-report.md — vi phạm giao thức (ROLE-04):` return 3; missing report -> `QA không tạo file .qa-report.md`; no verdict -> `QA chưa ghi Kết luận (Đạt / Cần sửa)`; commit `.qa-report.md` as `{task}: qa`; print `Kết luận QA: {verdict}`; for `Cần sửa` print the three next-step lines (role_qa.py 531-535).

---

### `agent-brain/src/db.rs` (model/store, CRUD)

**Analog:** none in Python for SQL (Phase 2 used Markdown task logs in `brain/tasklog.py`; RESEARCH Open Question 4 says do not port). Schema and pragmas: RESEARCH.md "Suggested Data Model" (migration 0001: `projects`, `tasks`, `runs`, `events`) and "SQLite open + migrations" snippet (WAL, `busy_timeout(5000)`, `synchronous=NORMAL`, `foreign_keys=ON`, `user_version` loop, read `user_version` as `i64`). Create the file 0600 and its dir 0700. Use `params!` for all values.

Stream-to-column mapping reuses the `summarize` result keys from `streamlog.py` 170-199 (see `stream.rs`). `runs.profile` is a snapshot at run time so STAT-01 is a `WHERE`. The reconcile function uses the `(pid, pid_start)` identity from `runner.rs`.

---

### `agent-brain/src/fastpath.rs` and `stats.rs`

**No Phase 2 analog for the behaviour** (new in Phase 3: D-13..D-16). Reuse:
- git commands from `gitops.rs` (`status`, `log`, `diff`, `current_branch`) for `status` (must work with `claude_bin` set to a missing path and must not write a `runs` row).
- argv skeleton from `agent.py build_command` for `inspect`: `claude -p "<prompt+diff>" --output-format stream-json --verbose --model sonnet --max-turns 1 --tools "" --no-session-persistence --strict-mcp-config` (the empty string is a real argv element; RESEARCH lines 497-501). Cap diff at `inspect_max_diff_bytes` on a line boundary and append `[diff cắt: đã gửi N/M byte]`.
- session cleanup fallback: accept only a UUID-shaped `session_id`, glob `<config_dir>/projects/*/<id>.jsonl` and remove the sibling `<id>/` dir (RESEARCH Pitfall 7).
- `stats`: SQL aggregates over `runs` grouped by project and by role, `--profile`, `--since 7d`, `--json`; label cost as "tương đương".

---

### `agent-brain/src/term.rs` (utility, streaming sink)

**Closest analog:** the `sys.stderr.write(f"[brain] ...")` lines in `brain/agent.py`. The Rust sink must never panic on EPIPE/EIO (RESEARCH Pattern 5 and Pitfall 4): every write goes through one `emit(line)` that ignores errors; truncate with `chars()`.

---

### `agent-brain/tests/common/mod.rs` (test sandbox)

**Analog:** `tests/helpers.py` `Sandbox` (lines 1-90+)

```python
self.xdg_config / self.xdg_data / self.runs / self.protocol_log / self.personal_dir / self.repos  # tempdir subfolders
self.fake_claude = repo_root() / "tests" / "fakes" / "fake_claude.py"; os.chmod(..., 0o755)
self.record_file = root / "fake_claude_record.jsonl"
git_config_file contents: [user] name=Sandbox User email=sandbox@example.com; [commit] gpgsign=false; [init] defaultBranch=main
env: drop CLAUDE_CONFIG_DIR + NESTED_SESSION_VARS; set XDG_CONFIG_HOME, XDG_DATA_HOME, GIT_CONFIG_GLOBAL, GIT_CONFIG_NOSYSTEM=1, FAKE_CLAUDE_RECORD
write_config(**overrides): claude_bin = fake, models, work/personal (personal dir = sandbox dir), common_args, max_turns, timeout_seconds, runs_dir ...
```
Rust adds `env_clear()` plus `HOME`, `AGENT_BRAIN_DB`, `AGENT_BRAIN_CONFIG` (RESEARCH anti-pattern: never touch the real home). Also port `make_repo(name, root, remote)`, `run_brain(...)` (-> spawn `env!("CARGO_BIN_EXE_agent-brain")`), and `records()` (read `FAKE_CLAUDE_RECORD`, JSON lines with `argv`, `cwd`, `claude_config_dir`, `present_vars`, `settings`, `blocked`).

---

### `agent-brain/tests/contract_*.rs` (contract tests)

**Analogs and the fixture-reading shape:**
- Routing/normalize: `tests/test_routing.py` 74-99. For each normalize case `normalize(input) == normalized` and `slugify(input) == slug`; for each routing case `kind == "match"` -> `resolve(query, projects).id == expect.project`, otherwise the error `kind` and the ordered candidate ids match.
- Scan/add: `tests/test_projects.py` 17-143 (materialiser described above under `projects.rs`).
- Plan/QA: `tests/test_planfile.py` 33 (plan) and 81 (qa_report).
- Profile: `tests/test_profile.py` 96 (`profile_env.json` cases).
- Fixture lookup: `Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/contract/<file>.json")`.
Fixture `version` field is 1 and `tests/test_phase2_integration.py:64` asserts every contract file has `version` and case ids, so the new `stream.json` and `guard.json` must follow that format (`{"version": 1, "cases": [{"id": ..., ...}]}`).

---

### `agent-brain/tests/{cli_e2e,fastpath,db_stats,runner_kill}.rs` (e2e tests)

**Analogs:** `tests/test_lead_e2e.py`, `test_dev_e2e.py`, `test_qa_e2e.py`, `test_pipeline_e2e.py`, `test_robustness.py`, `test_streamlog.py`, `test_guard.py`; the fake `tests/fakes/fake_claude.py`.

**Fake scenario keys** (fake_claude.py ~72-84, from `FAKE_CLAUDE_SCENARIO` JSON file): `stderr`, `sleep`, `raw_lines`, `events`, `ops` (`write | read | grep | git`), `ignore_hooks`, `exit_code`. The fake runs `--settings` PreToolUse hooks via the shell, so it exercises the Rust `agent-brain guard` subcommand end to end. Default stream: `system/init` (session `fake-session`, model `fake`), tool events, `result` with usage 10/5/0/0 and cost 0. Do not edit this fake; add `tests/fakes/fake_claude_tree.py` (see Shared Patterns below) for kill tests.

---

### `tests/fakes/fake_claude_tree.py` (NEW test fake)

**Analog:** `tests/fakes/fake_claude.py` (record/scenario conventions, shebang, `FAKE_CLAUDE_RECORD`). New behaviour per RESEARCH lines 609-610: print a `system/init` line, start `bash -c 'sleep 317'` with `start_new_session=True`, write `"<fake pid> <grandchild pid>"` to `$PIDFILE`, `MODE=coop` (terminate child on SIGTERM, exit 143) or `MODE=ignore` (ignore SIGTERM), then idle. Mirrors S4 r1/r2 from `spikes/spike-results.md`.

---

### `scripts/build-release.sh` (NEW)

**Closest analog (style only):** `brain/install.sh` and `spikes/lib/*.sh`. Content is specified in RESEARCH "Cargo/musl build" (3 routes, auto-fallback from `musl-gcc` to the CFLAGS hack, static assertions via `file` and `readelf -d | grep NEEDED`, and an `env -i HOME=<tmp> PATH=/nonexistent ./agent-brain stats` check).

## Shared Patterns

### Exit codes and error output
**Source:** `brain/__init__.py` 3-15 and `brain/cli.py` 87-98
**Apply to:** every module that returns errors, and `main.rs`. Routing failures go to stdout, everything else to stderr; messages are Vietnamese and must stay byte-identical where a test or fixture reads them.

### Project entry point preamble (select, runnable, profile)
**Source:** `role_lead.py` 38-42, `role_dev.py` 227-231, `role_qa.py` 436-440
**Apply to:** `run lead|dev|qa`, `status`, `inspect`
```python
cfg = load_config()
project = select_project(args.project)
require_runnable(project, cfg)
proj_path = Path(os.path.expanduser(project["path"])).resolve()
```

### Hand-editable JSON catalogue is the source of truth
**Source:** `brain/projects.py` `load_map` / `save_map`
**Apply to:** `projects.rs`, and the DB mirror sync in `db.rs`. Preserve unknown keys and key order; write `ensure_ascii=False`, 2-space indent, trailing `\n`, atomic temp + fsync + rename in the same directory.

### Dev/QA/Lead tool restriction (3 layers)
**Source:** allow/deny lists in `role_*.py`, `abs_rules` in `agent.py`, hook in `guard.py`, post-run git check in `role_dev.py` 340-351
**Apply to:** `roles.rs`, `guard.rs`. Never pass `--dangerously-skip-permissions` or `--permission-mode` (build_command rejects them).

### Language-neutral fixtures (D-03)
**Source:** `tests/contract/*.json`
**Apply to:** all pure modules. Both the Python tests and the Rust tests read the same files. Add `stream.json` and `guard.json` in the same `{version, cases[{id,...}]}` format plus a small Python loader each; the 114 Python tests must stay green (`python3 -m unittest discover -s tests -t . -q`).

### Test sandbox isolation
**Source:** `tests/helpers.py` Sandbox + RESEARCH "Anti-Patterns"
**Apply to:** every Rust integration test. `env_clear()` then set only `PATH`, `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `AGENT_BRAIN_DB`, `AGENT_BRAIN_CONFIG`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM=1`, `FAKE_CLAUDE_*`. Tests that signal processes must signal only pids they spawned (CLAUDE.md: no `pkill`/`killall`).

### Subprocess hygiene
**Source:** `brain/gitops.py` 20-33
**Apply to:** `gitops.rs`, `fastpath.rs`, `runner.rs`. Always an argument array, never `sh -c`, `GIT_TERMINAL_PROMPT=0`; the one shell string is the hook `command`, built with single-quote quoting and tested with spaces and quotes in paths.

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `agent-brain/Cargo.toml` / `Cargo.lock` | config | n/a | No Rust in the repo; use RESEARCH.md "Standard Stack" (pinned deps, `[profile.release] strip + lto thin`, keep `panic=unwind`) |
| `agent-brain/src/db.rs` (SQL part) | store | CRUD | Phase 2 has no database; use RESEARCH "Suggested Data Model" and "SQLite open + migrations" |
| `agent-brain/src/runner.rs` (kill tree, signals, /proc, reconcile) | service | event-driven | Phase 2 used Python `terminate()/kill()` on the direct child only; the tree kill is new (RESEARCH Patterns 1-3, verified S4 behaviour in `spikes/spike-results.md`) |
| `agent-brain/src/fastpath.rs`, `stats.rs` | controller/service | request-response / batch | New features (D-13..D-16); only the git helpers and argv builder are reusable |
| `scripts/build-release.sh` musl logic | script | batch | No prior build script; RESEARCH "Code Examples" is the spec |

## Notes for the planner

- Python names change in the Rust CLI (`brain scan` -> `agent-brain projects scan`, `brain lead` -> `agent-brain run lead`). Strings that embed the program name (`Chưa có scan_roots — chạy: brain scan --root <thư mục>`, the empty-list hint in the Fallback Ask, the QA next-step lines) are asserted by Python tests but not by any contract fixture. Decide once in plan 03-01 whether Rust keeps `brain ...` (so Python tests can run against the binary) or uses `agent-brain ...` (RESEARCH recommends not chasing byte equality there).
- `brain/streamlog.py` lines 65-66 and 69-70 contain a `/work/proj/` prefix special case that only exists for test fixtures; port it only if `tests/fixtures/stream/*.jsonl` still need it for `stream.json` parity.
- Hook command and Python executable: `guard.py` used `sys.executable -I guard.py`; the Rust port must use `std::env::current_exe()` (DIST-01).
- The `.plan.md` meta written by Rust must remain readable by Python during the D-02 transition (sorted keys, `>` escaped as `\u003e`).

## Metadata

**Analog search scope:** `/mnt/data/IDE/QuangVy-Orchestration/brain/`, `tests/` (contract, fakes, fixtures, helpers, test_*.py), `spikes/` (listing), `.planning/phases/03-rust-cli-core/{03-CONTEXT,03-RESEARCH}.md`
**Files scanned:** about 45 (read in full: every `brain/*.py` except `tasklog.py`; headers and harness code of the contract fixtures and tests)
**Pattern extraction date:** 2026-10-16
