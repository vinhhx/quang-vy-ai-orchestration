# Phase 7: Journal Vault (Obsidian) - Research

**Researched:** 2026-11-10
**Domain:** Rust CLI/daemon (`agent-brain`): SQLite -> deterministic Markdown export -> git sync to a private Obsidian vault; Telegram digest and `/tim` (SQLite FTS5)
**Confidence:** HIGH for in-repo facts and experiments (all read or run this session); MEDIUM for Obsidian/obsidian-git mobile behaviour (docs read, no device test)

**Provenance tags used in this file**
- `[VERIFIED: path:lines]` = I opened the file this session; the quoted text is verbatim.
- `[VERIFIED: ran ...]` = I ran it this session (scratch dir only: `/tmp/claude-1000/-mnt-data-IDE-QuangVy-Orchestration/deb9038a-35af-4aa4-918a-cb8c24f57013/scratchpad/`). Real data was read from a **copy** of the live `brain.db`, never the live file.
- `[CITED: url]` = official docs or plugin README read this session.
- `[ASSUMED]` = not verified; listed in the Assumptions Log.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Đã thống nhất trước (2026-11-10, memory `journal-vault-idea`)**
- **D-01:** Repo vault riêng tư, chỉ chứa dữ liệu; code nằm trong agent-brain. brain.db là nguồn sự thật, vault dựng lại được bất cứ lúc nào.
- **D-02:** Layout: `Today.md` (embed file hôm nay `![[...]]`, link hôm qua và tháng), `Nhật ký/YYYY/MM/YYYY-MM-DD.md`, tháng `YYYY-MM.md`, năm `YYYY.md`, `Dự án/<project>.md`. Chỉ ngày/tháng/năm, không có file tuần. Quy mô 10–30 task/ngày.
- **D-03:** Không đưa vào vault: diff, prompt, path, env, key, endpoint, account. Chỉ field allowlist, có quét secret trước khi push.

**Nguồn chữ trong nhật ký**
- **D-04:** Câu mô tả mỗi task ghép từ **mẫu cố định theo allowlist** (tiêu đề, dự án, trạng thái, vai đã chạy). Không gọi model → không tốn token, không đọc plan/QA/diff nên không thể lộ chúng.
- **D-05:** Tiêu đề/yêu cầu gốc do người dùng gõ được dùng **nguyên văn nhưng cắt gọn (~140 ký tự) và qua bộ quét secret**; chặn path và URL nội bộ.
- **D-06:** **Một dòng mỗi task mỗi ngày**: trạng thái cuối trong ngày kèm số vòng. Task nhiều ngày xuất hiện lại ở ngày có thay đổi/hoàn tất.

**Phân loại 3 nhóm**
- **D-07:** Nhóm (Công việc / Video / Hỗ trợ bộ phận khác) theo **cấu hình dự án** (`journal_group`, khai một lần). Không đoán theo từ khóa, không đánh nhãn theo task.
- **D-08:** Dự án chưa khai nhóm → **Công việc**.

**Chạy, commit, push**
- **D-09:** Exporter chạy **trong `agent-brain serve`** (vòng nền ~30 phút và cuối ngày) và có lệnh tay `agent-brain journal export` để chạy/dựng lại thủ công. Dùng cơ chế pump hiện có của serve.
- **D-10:** Push lỗi (mạng, SSH, xung đột): **commit cục bộ, thử lại lần sau, báo Telegram sau N lần liên tiếp**. Nếu pull xung đột (có người sửa vault) thì dừng và báo, **không ghi đè**.
- **D-11:** Vault clone tại `~/.local/share/agent-brain/journal`, tác giả commit `agent-brain journal` (danh tính riêng).
- **D-12:** Ngày tính theo **giờ địa phương máy (Asia/Ho_Chi_Minh)**, đổi ngày lúc 00:00; brain.db lưu epoch ms nên quy đổi khi xuất.

**Digest Telegram và /tim**
- **D-13:** Digest cuối ngày, **18:00 và cấu hình được**: số task theo nhóm, mục "cần anh xử lý", link file ngày.
- **D-14:** Link là **github.com tới file ngày** (repo private, mở khi đã đăng nhập GitHub), không dùng `obsidian://`.
- **D-15:** `/tim` (SQLite FTS5 trong brain.db) chỉ index **nội dung đã allowlist của nhật ký** (tiêu đề, dự án, nhóm, ngày, trạng thái); trả về ngày + link, không lộ thêm so với vault.
- **D-16:** Mục "cần anh xử lý" ở đầu file ngày gồm: **plan chờ duyệt (cổng)**; **hết quota / môi trường sập**; **QA "Cần sửa" hoặc vướng mắc chưa xử lý**; **run lỗi / timeout / protocol_violation**.

### Claude's Discretion
- Tên bảng/migration (lược đồ DB tiếp theo), cấu trúc module (`journal`), mẫu câu cụ thể, giá trị N cho báo lỗi push, định dạng frontmatter của file ngày, định dạng nhãn trạng thái tiếng Việt, tên trường cấu hình digest.

### Deferred Ideas (OUT OF SCOPE)
- Ghi ngược/giao việc từ vault (Telegram vẫn là đường duy nhất).
- Tóm tắt bằng model cho cuối ngày (đã chọn mẫu cố định; có thể xem lại sau).
- Tìm trong plan/QA report qua `/tim`.
</user_constraints>

## Project Constraints (from CLAUDE.md and the repo's own rules)

- Work split: Claude plans and reviews; an executor (agy, or Claude executors in worktrees when the user chooses) implements. Plans must be executable without this conversation: exact files, steps, acceptance criteria, verify commands. [VERIFIED: /mnt/data/IDE/QuangVy-Orchestration/CLAUDE.md]
- Executor commits per task; never `git add -A`; never `--no-verify`; must not touch `.planning/STATE.md`, `ROADMAP.md`, untracked `.planning/` files, `~/.claude-orca`. Reviewer re-runs verify commands and runs `spikes/lib/scan-secrets.sh` where it applies (it only scans `spikes/`, see section Secret scanning).
- **No hard code** rule: no concrete project/workflow/team/engine names in code (fixtures excepted); every new option is configuration with a default that keeps today's behaviour, lives in `projects_map.json` (read through `src/projects.rs`) or `config.json` (read through `src/config.rs`), has range checks, a test for "option absent = old behaviour", and (rule 2) an endpoint/field in the dashboard to edit it. [VERIFIED: .planning/quick/e2e-findings-fixes.plan.md:11-33]
- Binary is installed only at a `serve` restart, never while tasks run; schema upgrades follow the README convention (stop serve, `cp -p brain.db brain.db.bak-pre-<tag>-<date>` incl. `-wal`/`-shm`, install, restart). [VERIFIED: agent-brain/README.md:36-63; memory index entry `agent-brain-binary-swap`]
- Test command: `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1`. [VERIFIED: agent-brain/README.md:520]
- No `.claude/skills` pattern applies (only `threejs-*` skills exist). `workflow.nyquist_validation` and `security_enforcement` are absent from `.planning/config.json` (only `_auto_chain_active`), so both sections below are required.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| JRN-01 | Day file from brain.db in Asia/Ho_Chi_Minh; one line per task per day; 3 groups; "cần anh xử lý" header | Data map (1a-1c), day boundary (1d), attention rules, status vocabulary, group config |
| JRN-02 | `Today.md`, month, year, `Dự án/<project>.md`; no weekly file; idempotent | Pure render -> path map, writer with ownership marker, deletion of stale files, golden/idempotency tests |
| JRN-03 | Allowlist only; ~140-char cut; block path/URL; secret scan before every commit; on hit no commit/push, notify | Sanitizer pipeline + final gate, injection neutralisation, secrets.rs is value-based (patterns must be added) |
| JRN-04 | Clone at `~/.local/share/agent-brain/journal`, author `agent-brain journal`, serve loop + CLI, push failure policy, pull conflict stop | Git sequence (verified with local bare remotes and the real remote), failure counters in DB, mailbox to Telegram |
| JRN-05 | Digest once per day (18:00 configurable) to the paired chat with counts, attention list, github.com link | DB mailbox + `detect.rs` scan, claim-before-push, GitHub URL encoding |
| JRN-06 | `/tim <keyword>` via FTS5 over allowlisted content, paired chat only | FTS5 verified in the bundled SQLite, fold + quoted-token MATCH builder, `/tim` wiring |
| JRN-07 | Everything configurable; migration with backup; no binary swap while tasks run | `journal` config section, `journal_group` read leniently from the map, migration 12, **FK-check blocker on the live DB**, README upgrade section |
</phase_requirements>

## Summary

Everything the exporter needs is already in `brain.db`, but several facts differ from what the CONTEXT assumes and will change the plans. The most important: (1) the live DB has 645 orphan `events` rows, and `db::open` aborts any pending migration when `PRAGMA foreign_key_check` reports a violation, so **migration 12 would stop every `agent-brain` command on the real DB** unless the check is fixed first; (2) `tasks.title` is not a title but the **full raw request** (average 1,513 chars, 21 of 39 rows multi-line, max 3,926), so the sanitizer is mandatory, not a nicety, and `proposals.title` (<=200 chars, set by triage) is the better source when a proposal exists; (3) the vault repo is **not empty**: it already holds a hand-pushed demo export (2 commits, 23 files, branches `main` and `master` at the same SHA) that shows the format the user likes; (4) `quota_stops` is empty on this machine (it is only filled while the orchestrator is enabled), so quota state must come from `runs.outcome = 'quota'`; (5) `protocol_violation` runs have `status = 'succeeded'`, so category 4 of D-16 must key on `outcome`, not `status`.

The code seams are clear. A `journal` module is a pure "rows -> `BTreeMap<path, bytes>`" renderer plus a thin writer and a thin git sync. The serve loop copies `orchestrator::spawn_loop` (tokio + `spawn_blocking` + `st.shutdown`). The Telegram `BotCtx`/outbox is private to the bot threads, so digest and alert messages must travel through a small DB mailbox that a new scan in `telegram/detect.rs` turns into `Outgoing::Message` (the same pattern as `IntakeFailures`/`ChainWaits`). `/tim` is one more arm in `intake::on_command`. `gitops.rs` cannot be reused for the vault (private `git_cmd`, no timeout, no env, no author, "nothing to commit" indistinguishable from errors), so the journal needs its own small git runner. `secrets.rs` redacts **known secret values** only; pattern detection (key shapes, tokens, paths, URLs) must be new code in the sanitizer. FTS5 is compiled into the bundled SQLite 3.53.2; `unicode61` does not fold `đ`, so fold text in Rust with the existing `routing::normalize` and index with `remove_diacritics 0`.

**Primary recommendation:** Build `agent-brain/src/journal/` as pure functions (collect -> render -> sanitize) with golden tests first, then writer + gitsync against local bare remotes, then migration 12 (after fixing the FK-check blocker), then serve loop + Telegram mailbox + `/tim`. Add no new crates.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Source of truth for tasks/runs/approvals | brain.db (SQLite) | - | D-01: one-way export, vault is derived |
| Day/attention/status derivation | `journal::collect` (pure Rust over DB rows) | SQL for loading only | Must be deterministic and testable without I/O |
| Markdown rendering, sanitising, path layout | `journal::render` + `journal::sanitize` (pure) | - | Golden-file testable; the only place user text can enter the vault |
| Writing files into the clone | `journal::writer` (local FS) | - | Touches only managed paths; atomic temp+rename |
| Git clone/commit/pull/push | `journal::gitsync` (external `git` + `ssh`) | - | Needs timeouts, fixed author, non-interactive SSH |
| Scheduling (30 min + midnight rollover + digest time) | serve tokio loop (`journal::spawn_loop`) | CLI `journal export` | D-09 |
| Delivering digest / alerts | Telegram bot threads (`detect.rs` scan -> outbox) | DB mailbox table as the bus | `BotCtx` is private; chat pairing already enforced |
| `/tim` search | brain.db FTS5 table | Telegram command handler | D-15: index lives in brain.db |
| Presentation on phone | Obsidian + obsidian-git (pull only) | github.com web view (Telegram link) | Two renderers: Obsidian and GitHub (see Pitfall 9) |

## Standard Stack

No new crates. Everything needed is already pinned. [VERIFIED: agent-brain/Cargo.toml:8-23]

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rusqlite (bundled) | =0.40.2 -> libsqlite3-sys 0.38.2 -> SQLite 3.53.2 | DB reads, FTS5, migration | Already the DB layer; FTS5 compiled in (see FTS5 section) |
| chrono | =0.4.45, `default-features=false, features=["clock","std"]` | Day boundaries, weekday, `Local` | Already used by `stats::day_key`; no tz database needed |
| unicode-normalization | =0.1.25 | NFC for emitted names/text; `routing::normalize` folding | Already used in 10 files |
| regex | =1.13.1 | Sanitizer patterns | Already used (`routing`, `secrets` neighbours) |
| libc | =0.2.190 | `flock` for a journal lock; process group | Already used by `dashboard/lock.rs` |
| serde_json | =1.0.154 | Read `journal_group` from the map, config section | Already used everywhere |
| tempfile (dev) | =3.27.0 | Sandbox dirs in tests | Existing dev-dependency |

### External tools (not crates)
| Tool | Version on this machine | Purpose |
|------|------------------------|---------|
| git | 2.43.0 [VERIFIED: ran `git --version`] | clone/add/commit/fetch/rebase/push |
| OpenSSH | 9.6p1 [VERIFIED: ran `ssh -V`] | transport for `git@github.com:...` |
| python3 | 3.12.3 | only for the existing `tests/fakes/fake_telegram.py` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `git` CLI | `git2` / `gix` crate | New heavy dependency, musl static build risk; the CLI is already how `gitops.rs` works |
| `chrono::Local` | `chrono-tz` / `jiff` / `time` | New dependency; Vietnam has no DST so `Local` or a fixed +07:00 offset is enough |
| Own percent-encoder (8 lines) | `percent-encoding` crate | It exists in the lockfile only transitively (axum); do not promote it to a direct dependency for 8 lines |
| FTS5 external-content table | plain FTS5 table rebuilt per export | Rows are derived data, so a plain table with `DELETE` + `INSERT` in one transaction is simplest; external content adds a second table and a `rebuild` step for no gain at a few thousand rows |

**Installation:** none. **Version verification:** versions read from `Cargo.toml`/`Cargo.lock`, and the bundled SQLite version from `libsqlite3-sys-0.38.2/sqlite3/sqlite3.c:470` (`#define SQLITE_VERSION        "3.53.2"`). A scratch crate pinned with the repo's `Cargo.lock` printed `sqlite 3.53.2 ENABLE_FTS5=1`. [VERIFIED: ran scratch crate, `cargo run --offline`]

## Package Legitimacy Audit

No external packages are added by this phase (all crates above are already pinned in `Cargo.toml`/`Cargo.lock`), so the `package-legitimacy` gate has nothing to check.

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

If a planner is tempted to add `percent-encoding`, `chrono-tz`, `insta`, or `git2`: do not (see Alternatives). Any such addition would need `checkpoint:human-verify` and a Cargo.lock review.

## Architecture Patterns

### System Architecture Diagram

```
                      (every ~30 min, on day rollover, and at digest time)
 serve: journal::spawn_loop ──┐                    CLI: agent-brain journal export [--no-push]
                              ▼                                   │
                 ┌──────────────────────────┐                     │
                 │ run_export_once(cfg,db)  │ <───────────────────┘
                 │  flock(journal.lock)     │
                 └────────────┬─────────────┘
                              │
   brain.db (ONE read txn) ───┤ collect: tasks, runs, proposals, intake, projects, approvals
   projects_map.json ─────────┤ journal_group per project (lenient read)
                              ▼
                 pure: build DayModels (as-of rules, attention, rounds, roles)
                              ▼
                 pure: render -> BTreeMap<rel_path, String>   (sanitize every user string here)
                              ▼
                 gate: secret scan on every generated byte  ──hit──► no write, no commit, mailbox "secret"
                              ▼ ok
 gitsync::prepare  (clone if missing | fetch; ff/rebase onto origin/<branch>; dirty/conflict ──► stop + mailbox)
                              ▼
 writer: write changed files (temp+rename), delete stale managed files (marker proves ownership)
                              ▼
 git add --pathspec-from-file (only the changed/removed paths) ; diff --cached --quiet ──no change──► done
                              ▼ changed
 git commit (fixed author, no gpg)  ──► git push HEAD:refs/heads/<branch> (timeout, BatchMode)
                              ▼
 FTS rebuild in one txn (journal_fts) ; journal_state counters ; journal_mail rows (digest/alerts)
                              ▼
 telegram/detect.rs scan (<=60 s) ──► Outgoing::Message ──► outbox ──► paired chat only
 Telegram /tim ──► intake::on_command ──► journal_fts MATCH ──► reply with github.com links
```

### Recommended Project Structure
```
agent-brain/src/journal/
├── mod.rs        # run_export_once(), cmd_export(), re-exports
├── config.rs     # JournalConfig (parsed by config.rs::config_from_value), journal_group_of(map, slug)
├── collect.rs    # DB -> Vec<TaskDay>  (read-only snapshot, grouping in Rust)
├── model.rs      # TaskDay, DayModel, Attention, Group, status labels (pure)
├── render.rs     # DayModels -> BTreeMap<String, String>  (pure, deterministic)
├── sanitize.rs   # sanitize_text(), neutralise_markdown(), scan_generated()
├── writer.rs     # apply map to clone, marker ownership, deletions
├── gitsync.rs    # run_git() with timeout/env, sync(), commit_push(), error classification
├── state.rs      # journal_state kv + journal_mail helpers (streak, claim)
├── fts.rs        # rebuild(), search(), fold(), match_query()
├── digest.rs     # digest_due() (pure, takes now+tz), digest_text()
└── loop_.rs      # spawn_loop(Arc<AppState>)
agent-brain/src/telegram/journal_cmd.rs   # /tim
agent-brain/src/telegram/detect.rs        # + JournalMail scan (next to ChainWaits)
agent-brain/tests/journal_*.rs            # golden, idempotency, gitsync(bare remote), digest, fts, sanitizer
```
`lib.rs` needs `pub mod journal;` (lib.rs lists modules alphabetically: add after `inputs`). [VERIFIED: agent-brain/src/lib.rs]

### 1. Data available in brain.db (item 1)

Schema facts, all from `db.rs` MIGRATIONS (11 entries, `pub const SCHEMA_VERSION: i64 = 11;` at db.rs:195). `[VERIFIED: agent-brain/src/db.rs:8-195]`

| Need | Source (exact) | How to derive |
|------|----------------|---------------|
| Task identity | `tasks(id, project_id, slug, title, branch, status, created_at, updated_at, origin, level, workdir, approved_at, auto_state, auto_json, workflow_json)` | `tasks.workdir` and `branch` are paths/branch names: **never export** |
| Title (a) | **Prefer** `proposals.title` via `proposals.task_id = tasks.id` (latest `id`), else `tasks.title`, else `tasks.slug` | `proposals.title` is validated `<= MAX_TITLE_CHARS = 200` (orchestrator/triage.rs:11). `tasks.title` is overwritten by the Lead/Analyst child with the **full request**: `upsert_task(&ctx.conn, proj_row_id, &task_slug, request, ...)` (roles/lead.rs:591-598; roles/analyst.rs:205-213) and `ON CONFLICT ... DO UPDATE SET title = excluded.title` (db.rs:431). Request cap `MAX_REQUEST_CHARS: usize = 20_000` (dashboard/api_tasks.rs:23). Live data: tasks.title avg 1,513 chars, max 3,926, 21/39 contain newlines; proposals.title avg 43, max 103; 25/39 tasks have a proposal [VERIFIED: ran on copy of live DB] |
| Project | `projects(slug, name_vi)`; `db::list_project_display_map` returns id -> (slug, name_vi or slug) (db.rs:1643-1664) | Use slug as the file name key (validate, see Pitfall 6); `projects.path` is a path: never export |
| Final status of the day | **No status history exists** (`tasks.status` is only the current value). Derive from the **last run of the task belonging to day D** | Map (role, status, outcome) -> label by reusing `dashboard::taskstate::step_state` (taskstate.rs:107-137, `pub fn step_state(run: Option<&RunRow>) -> (&'static str, &'static str)`). Verbatim arms: `Some("dev:blocked") => return ("vuong_mac", "Vướng mắc")`, `Some("qa:fix") => return ("can_sua", "Cần sửa")`, `Some("protocol_violation") => return ("vi_pham", "Vi phạm giao thức")`, `Some("plan_invalid") => return ("sua_lai", "Plan sai, Lead sửa lại")`, `Some(crate::quota::OUTCOME_QUOTA) => return ("het_quota", "hết quota")`, `Some(crate::preflight::OUTCOME_ENV_DOWN) => { return ("moi_truong", "môi trường chưa sẵn sàng") }`, then by status `"killed" => ("killed", "killed")`, `"failed" \| "timed_out" \| "lost" => ("loi", "lỗi")`, `"succeeded" => ("xong", "xong")` |
| Task-level words | `tasks.status` values written in code: `'open'` (db.rs:429), `"planned"` (roles/lead.rs:902,912), `"dev_done"`/`"blocked"` (roles/dev.rs:1268,1274), `"qa_fix"`/`"qa_pass"` (roles/qa.rs:659,664), `"answered"` (roles/analyst.rs:464); `pub const SUCCESS_STATUSES: &[&str] = &["answered", "qa_pass"];` (tasks.rs:7) | Use only as a hint for the **current** day (today); past days use runs |
| Rounds | `runs.round` is NULL for 31 of 199 live runs (manual runs). Pure helper: `decide::effective_rounds(runs: &[RunFact]) -> Vec<i64>` (decide.rs:515) fed by `RunFact::from_row(&RunRow)` (decide.rs:279), rows from `db::runs_for_task` (db.rs:829) | Rounds shown = `max(effective_rounds)` over the task's runs up to end of D (min 1) |
| Roles that ran | `runs.role` CHECK `('lead','dev','qa','inspect','plan-review','code-review','analyst','analysis-review')` (db.rs:100); `runs.engine = 'người'` for human steps (db.rs:547) | Distinct roles of day D's runs in a fixed canonical order; Vietnamese label table in `journal::model` |
| Run outcomes | `set_run_outcome` call sites: `"quota"`, `"env:down"`, `"protocol_violation"`, `"agy:empty"`, `"plan_invalid"`, `"review:<verdict>"`, `"dev:blocked"`, `"qa:fix"`, `"qa:pass"`, `"analyst:invalid"` [VERIFIED: grep of `set_run_outcome` across src/] | Live data: `(None,succeeded) 94, qa:pass 37, protocol_violation(succeeded) 11, qa:fix 11, dev:blocked 9, plan_invalid 8, review:approve 7, quota(failed) 6, review:pass 6, (None,timed_out) 3, review:revise 3, (None,failed) 2, (None,killed) 2` [VERIFIED: ran on copy] |
| Run stages | `runs.stage` live values: analyze, code-review, fix, implement, plan, plan-review, qa, review | `Stage` enum does not list `analyze`/`review`; do not parse `stage` for labels |
| Origin (c) | `tasks.origin` (`pub const ORIGIN_TELEGRAM: &str = "telegram";` db.rs:889; NULL otherwise). Source of intake: `intake.source CHECK (source IN ('ui','telegram','trello','sheet'))` (db.rs:67) via `proposals.intake_id`. Fixbug key: `db::fixbug_key_for_task(conn, task_id)` (db.rs:1977) | Allowlist: source kind only (`telegram`/`ui`/`trello`/`sheet`). Do **not** export `fixbug_refs.title`/`url`/`note` |
| Day boundary (d) | all timestamps are epoch ms (live: all 199 runs and 39 tasks have 13-digit values) | See "Day boundaries" below |

**Which day does a run belong to:** day of `COALESCE(ended_at, started_at)` (a still-running run belongs to its start day). Live data has 0 runs spanning midnight, so this rarely matters. A task is **active on D** if it has a run belonging to D or `tasks.approved_at` falls in D.

**"Cần anh xử lý" (b), derived as-of end of day D, from DB only (the journal must not read plan files).** Let L = the task's last run with `started_at < end(D)`.

| D-16 category | DB rule | Notes |
|---------------|---------|-------|
| Plan waiting at the gate | L.role in (`lead`,`plan-review`), L.status = `succeeded`, L.outcome NULL or `review:approve`/`review:pass`, **and** no approval timestamp in `[L.started_at, end(D))` (`tasks.approved_at`, `plan_approvals.approved_at`) | Gate state is not stored: `autopilot::decide::gate_state(wf, f)` needs git facts (`plan_commit`) (decide.rs:835-850), and `auto_state='gate'` exists only for autopilot tasks (`claim::release(.., "gate", ..)` at autopilot/mod.rs:591). Approval history is partial: `plan_approvals` is written only by `claim.rs:651-652`; `tasks.approved_at` is overwritten on re-approval and cleared by `lead.rs:911` |
| Quota stop | L.outcome = `quota` | **Do not use `quota_stops`**: rows are inserted only by `scheduler::handle_quota_stops` (scheduler.rs:444) which runs after `if !orch.enabled { return Ok(()); }` (scheduler.rs:52-53); live DB: `quota_stops` = 0 rows vs 6 runs with outcome `quota` [VERIFIED: ran] |
| Environment down | L.outcome = `env:down` | Optional extra: `json_extract(t.auto_json, '$.stop.reason') = 'moi_truong_chua_san'` (precedent scheduler.rs:578) covers a refusal before any run exists; `StopReason::MoiTruongChuaSan.as_str() = "moi_truong_chua_san"` (decide.rs:139) |
| QA "Cần sửa" / blockers | L.outcome in (`qa:fix`, `dev:blocked`) | "Vướng mắc" text lives only in the plan file (`planfile::section_text`), not in the DB; `dev:blocked` + `tasks.status='blocked'` (dev.rs:1266-1268) is the DB proxy. QA counts exist in `runs.metrics_json` (`specs`, `failing`, `not_automatable`, report.rs:189-195) if a number is wanted |
| Failed / timeout / protocol_violation | L.outcome = `protocol_violation`, **or** (L.outcome not in quota/env) and L.status in (`failed`,`timed_out`,`lost`) | `protocol_violation` runs have status `succeeded` (11 live rows), and quota runs have status `failed`, so **precedence must be quota > env > protocol/blocked/fix > failed**, as `telegram::detect::current_event` documents ("precedence: quota, stop, gate, QA passed, run failed", detect.rs:76-77). Exclude `killed` (user action) |

Resolved = a later run exists (L changes), so only the latest run matters. Rule for which days list an item: past day D lists it only if the task was **active on D**; **today** also carries over older unresolved items (live), so the digest and `Today.md` show everything still open. This keeps past day files stable (pure function of DB and D) while today is complete. (Recommendation, not a locked decision.)

**Day boundaries.** Vietnam has no DST, but do not hard code +07:00: the repo already has the exact pattern. `stats::day_key<Tz: chrono::TimeZone>(ms: i64, tz: &Tz) -> String` (stats.rs:219) is called with `&chrono::Local` in production (stats.rs:287) and `chrono::FixedOffset::east_opt(7 * 3600)` in tests (tests/dashboard_stats.rs:135,232); `telegram::intake::local_midnight_ms(now)` exists (intake.rs:442). The machine is `Asia/Ho_Chi_Minh` (`/etc/localtime -> /usr/share/zoneinfo/Asia/Ho_Chi_Minh`, `date` prints `+07`) [VERIFIED: ran]. So: write all journal date math generic over `Tz`, use `chrono::Local` in the loop/CLI and `FixedOffset` in tests. Optional config `journal.utc_offset_minutes` (absent = `Local`) protects against a serve started under `TZ=UTC` (an ASSUMED risk, A8). `chrono` is already a dependency with `clock`+`std`; no new dependency is needed. day end for D = next local midnight in ms (use `Tz::from_local_datetime(...).earliest()` like `local_midnight_ms`).

### 2. Where each new thing plugs in (item 2)

| Thing | Location | Pattern to copy |
|-------|----------|-----------------|
| Serve loop | `dashboard/mod.rs:270-271`: `let bot = crate::telegram::start(state.clone());` / `let _orchestrator = crate::orchestrator::spawn_loop(state.clone());` -> add `let _journal = crate::journal::spawn_loop(state.clone());` | `orchestrator::spawn_loop(st: Arc<AppState>) -> tokio::task::JoinHandle<()>` (orchestrator/mod.rs:185-219): `tokio::select!` on `st.shutdown.changed()` and `sleep`, work inside `tokio::task::spawn_blocking`, errors only `term::emit`. Tick every ~60 s and let `run_export_once` decide due-ness (interval, day rollover, digest time) |
| "pump" | `dashboard/pump.rs` is the SSE hint broadcaster (`TICK_MS = 250`, watches `PRAGMA data_version`, sends on `hints`); it is **not** a scheduler. D-09's "pump" = the serve background-loop mechanism above | Optional: subscribe `st.hints` to export soon after changes; not needed for a 30-min cadence |
| `AppState` | `cfg`, `db_path`, `map_path`, `shutdown: watch::Receiver<bool>` (dashboard/mod.rs:33-47) | `cfg` is a startup copy; read `cfg.journal` once per tick |
| Telegram commands | `intake::parse_command` (intake.rs:263): name must match `a-z0-9_` 1-32 chars, so `/tim` (no diacritics) is valid. Dispatch `intake::on_command` match (intake.rs:711-729): add `"tim" => super::journal_cmd::on_tim(ctx, rest),` | `fixbug_cmd.rs` (parse -> `bot::reply(ctx, &text)`; read-only commands run inline, network ones on a thread) |
| Pairing check | `bot::authz(update, paired)` (bot.rs:89) is applied in `handle_update` (bot.rs:395) before `on_message`; every command, `/tim` included, is already paired-chat only | No extra check; add a test that a stranger's `/tim` gets no reply |
| Command menu / help | `setMyCommands` list in `telegram/mod.rs:230-238`; `HELP_TEXT` (bot.rs:42). **`tests/telegram_intake.rs:1953-1971` asserts the exact command list**: update it when adding `{"command": "tim", ...}` | tests already compare `HELP_TEXT` by reference (telegram_intake.rs:266) |
| Send path | `Outgoing::Message { text }` -> `edit_request` -> `sendMessage` with only `chat_id` and `text`, i.e. plain text, **no parse_mode** (outbox.rs:32-60); sender retries with backoff while the network is down (`edit()`, outbox.rs:651-698); `Outbox::push(&self, item)` (outbox.rs:116) | URLs auto-link in plain text; percent-encode spaces or the link breaks (Pitfall 10) |
| Why a mailbox | `BotCtx` is built inside `telegram::start` (mod.rs:98-136); `BotHandle` only exposes `stop()` (mod.rs:108-113), so the tokio journal loop cannot reach the outbox | `detect::run` (detect.rs:527-617) already scans the DB and does `ctx.outbox.push(item)`; scans: `IntakeFailures` (detect.rs:352), `EnvReruns`, `ChainWaits`. Add `JournalMail::scan(conn) -> BrainResult<Vec<Outgoing>>`. The loop does a full scan every `SAFETY_SCAN_SECS = 60` (detect.rs:33) so delivery latency is <= 60 s |
| Config section | `config.rs`: `Config` struct (config.rs:1040-1060), `config_from_value` (config.rs:1203), `orchestrator` parse block (config.rs:1394-1458), `telegram_max_tasks_per_day` (config.rs:1461-1471), the single struct literal `Ok(Config { ... })` at config.rs:1501. Defaults file `assets/config.default.json` (`include_str!` at config.rs:6, parsed by 9 tests) | Add `pub journal: JournalConfig`; parse like `orchestrator`; `config.default.json` may omit the section (absent = off) |
| Per-project `journal_group` | **Do not add a field to `Project`**: 41 struct literals build it (`Project {` in src/ and tests/ incl. db.rs tests and tests/db_stats.rs). Follow `projects::preflight_of(map, project_id)` (projects.rs:551-563): a free function reading the entry object leniently, e.g. `journal::config::journal_group_of(map: &Value, slug: &str) -> GroupId` | `DB projects` table is rebuilt from the map by `sync_projects` (db.rs:307) and has no such column; read the map at export time via `projects::load_map(&st.map_path)` |
| CLI | `main.rs`: add `Journal { #[command(subcommand)] subcommand: JournalCommands }` to `enum Commands` (main.rs:17-...), arm in the final `match cmd` after `open_ctx()` (main.rs:~420-520) with `ctx.cfg`, `ctx.conn`, `ctx.map`; subcommands `Export { --no-push, --rebuild }` | `open_ctx()` also runs `reconcile_stale_runs`; same as every other CLI command |
| Migration | `db.rs`: append entry 12 to `MIGRATIONS`, set `SCHEMA_VERSION = 12`; `open()` applies pending entries in one `BEGIN IMMEDIATE` txn (db.rs:283-300). Update `tests/db_migration.rs:976` (`assert_eq!(agent_brain::db::SCHEMA_VERSION, 11);`) and add `v11_db_upgrades_to_v12_keeping_data` following `build_v10`-style helpers (db_migration.rs:1454-1482) | See Migration SQL below and Pitfall 1 |
| Tests layout | integration tests in `agent-brain/tests/*.rs` using `common::Sandbox` (tests/common/mod.rs:30-98: temp `HOME`, `XDG_*`, `AGENT_BRAIN_DB`, `AGENT_BRAIN_CONFIG`, `GIT_CONFIG_GLOBAL`), `sb.db()` (mod.rs:412), `sb.serve(&[])`; fake Telegram: `tests/support/telegram.rs` (`FakeTelegram::start(&sb)`, `calls("sendMessage")`, `push_update(message_update(..))`, `write_config(&sb, chat, 0o600)`) which launches `<repo>/tests/fakes/fake_telegram.py`; env hooks for tests exist (`AGENT_BRAIN_TELEGRAM_API`, `AGENT_BRAIN_TELEGRAM_BACKOFF_MS`, `AGENT_BRAIN_TELEGRAM_INTAKE_QUIET_MS`) | A test-only env hook for "now" (e.g. `AGENT_BRAIN_JOURNAL_NOW_MS`) is consistent with those |

**Journal lock.** `agent-brain journal export` can run while serve's loop runs. Use an advisory `libc::flock(LOCK_EX|LOCK_NB)` on `<data_dir>/journal.lock`, same technique as `dashboard/lock.rs` (`LOCK_FILE = "serve.lock"`, `acquire(dir)`); `acquire` is hard-wired to its constant, so copy the 20 lines or add `acquire_named`. A second runner that cannot take the lock exits quietly ("đang có lần xuất khác").

### 3. SQLite FTS5 (item 3)

- **FTS5 is compiled in.** `libsqlite3-sys-0.38.2/build.rs:159`: `.flag("-DSQLITE_ENABLE_FTS5")` inside the unconditional bundled `cc::Build` (lines 150-170); Cargo.lock pins `libsqlite3-sys 0.38.2` and `rusqlite 0.40.2` (Cargo.lock:516-517, 670-671). A scratch crate using the repo's lockfile printed `sqlite 3.53.2 ENABLE_FTS5=1`. [VERIFIED: build.rs + ran]
- **Tokenizer semantics (ran on the exact bundled build).** Inserted `Sửa lỗi đăng nhập trang quản trị`, `Tìm kiếm khách hàng theo số điện thoại`:

| Tokenizer | `tim`->`Tìm` | `loi`->`lỗi` | `dang`->`đăng` | `nhap*`->`nhập` |
|-----------|-------------|-------------|---------------|-----------------|
| `unicode61` (default = remove_diacritics 1) | yes | **no** (ỗ has 2 marks) | **no** | no |
| `unicode61 remove_diacritics 0` | no | no | no | no |
| `unicode61 remove_diacritics 2` | yes | yes | **no** | yes |
| `trigram` | no | no | no | no |

  `đ` (U+0111) is never folded by `unicode61`, in any mode, so "dang" can never match "đăng". NFD-stored text matches the same as NFC under `remove_diacritics 2`. [VERIFIED: ran scratch crate]
- **Recommendation: fold in Rust, index folded text with `unicode61 remove_diacritics 0`.** `routing::normalize` (routing.rs:33-45) already does exactly the needed fold: `text.replace(['đ', 'Đ'], "d").nfd().collect()`, strip `\p{Mn}`, lowercase, collapse `[\s_\-]+` to one space. Apply it to every indexed column value and to every query token. Result of the fold test: `tim`, `Tìm`, `dang`, `Đăng`, `dang nhap`, `so dien`, `kiem*` all hit the intended row. [VERIFIED: ran]
- **Table shape:** plain (not contentless, not external-content) FTS5, rebuilt per export:

```sql
CREATE VIRTUAL TABLE journal_fts USING fts5(
  task_ref UNINDEXED, day UNINDEXED,            -- returned, not searched
  title, project, grp, day_text, status,        -- allowlist D-15, all folded
  tokenize='unicode61 remove_diacritics 0');
```
  Why plain: the rows are a pure function of brain.db, so `BEGIN IMMEDIATE; DELETE FROM journal_fts; INSERT ...; COMMIT;` is idempotent (ran: two rebuilds leave 1 row; `INSERT INTO jfts(jfts) VALUES('integrity-check')` and `'optimize'` succeed). External-content also works (`'rebuild'` ran fine) but adds a second table for nothing at this scale. Cross-column AND works with plain quoted tokens (`"apec" "dang"` matched title+project), and the date is searchable as tokens (`"2026" "11" "09"`). [VERIFIED: ran]
- **Safe MATCH from untrusted Telegram text.** Raw input fails: `AND`, `foo AND`, `"unterminated`, `a-b` ("no such column: b"), `col:foo`, `*`, `(`, `tim^`, `NOT x`, `fix bug #12`, `50% off`, `a'b` all raise errors (and `title:sua` is interpreted as a column filter) [VERIFIED: ran]. Safe builder: `fold(query)` -> split on every non-alphanumeric char -> drop empties -> cap token count (e.g. 8) and token length -> wrap each token as `"token"` (double any `"`, though folding already removed them) -> join with a space (implicit AND); optional trailing `*` on the last token as `"tok"*`. Ran: all the hostile strings above produced valid queries and no error; `"dang" "nhap"` -> hit; `"kie"*` prefix works; `"title:sua"` quoted -> 0 hits (no column filter). Empty token list -> reply with usage. Order with `ORDER BY bm25(journal_fts, ...), day DESC LIMIT n`.
- **Migration inside a txn works:** on a copy of the live DB, `CREATE TABLE`s + `CREATE VIRTUAL TABLE journal_fts USING fts5(...)` inside `BEGIN IMMEDIATE` with `foreign_keys=OFF`, then `user_version = 12`, committed; 8 `journal_%` objects appear in `sqlite_master` (virtual table shadow tables). [VERIFIED: ran on copy]

### 4. Git from Rust (item 4)

**What `gitops.rs` gives and why it is not enough.** `fn git_cmd(root: &Path)` is private and sets only `GIT_TERMINAL_PROMPT=0` and `GIT_OPTIONAL_LOCKS=0` plus `current_dir(root)` (gitops.rs:10-24); `pub fn git(root, args)` (gitops.rs:157-182) uses `.output()` with **no timeout** and returns `BrainError::error(stderr)` with no structure; `commit_paths` (gitops.rs:501-511) runs `git add -f -- paths` then `git commit -m msg -- paths` using whatever identity/signing the global config has; "nothing to commit" prints on **stdout** with exit 1 and empty stderr, so it becomes the generic "Lệnh git thất bại (exit 1)". `current_dir(root)` also forbids `git clone` into a not-yet-existing directory. Therefore: **new `journal::gitsync` with its own runner**, reusing only the conventions (`GIT_TERMINAL_PROMPT=0`, `BrainError`). `procs::kill_tree(root: ProcId, grace, force)` (procs.rs:114) exists for killing a stuck git+ssh tree after a wall-clock timeout (spawn, poll `try_wait`, kill).

**Runner contract (recommended):** `run_git(dir: Option<&Path>, args: &[&str], timeout: Duration) -> GitOut { code, stdout, stderr }` with env: `GIT_TERMINAL_PROMPT=0`, `LC_ALL=C` (stable English stderr to classify), `GIT_SSH_COMMAND="ssh -o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout=<n> -o ServerAliveInterval=15 -o ServerAliveCountMax=2"`, `GIT_ALLOW_PROTOCOL=file:ssh:https` (blocks `ext::`/`fd::`), `GIT_AUTHOR_NAME/EMAIL` and `GIT_COMMITTER_NAME/EMAIL` from config (D-11 name `agent-brain journal`), and `-c commit.gpgsign=false -c core.quotepath=false -c core.autocrlf=false` (+ optionally `-c core.hooksPath=/dev/null`, A6). Pass the URL after `--` and refuse a URL starting with `-`. Use `-z` for any porcelain parsing (porcelain v1 quotes names containing spaces even with `core.quotepath=false`: ran).

**SSH on this machine:** `GIT_SSH_COMMAND="ssh -o BatchMode=yes -o ConnectTimeout=15 -o StrictHostKeyChecking=yes" git ls-remote git@github.com:vinhhx/agent-brain-journal.git` exits 0 (host key already trusted, key works non-interactively) [VERIFIED: ran]. Keep `StrictHostKeyChecking=yes` (no trust-on-first-use); a failure surfaces as a push failure with category "ssh".

**The remote is NOT empty** [VERIFIED: ran `git ls-remote --symref` and a scratch clone]:
```
ref: refs/heads/main	HEAD
203c7fed46054fcb3b53c65706ffbc5af313d544	HEAD / refs/heads/main / refs/heads/master
```
Two commits by the user (`dcdb058` "Nhật ký: xuất demo từ brain.db (38 task, 28/10 – 10/11/2026)", `203c7fe` "Nhật ký: task dạng callout gập/mở (thử nghiệm)"), 23 tracked files, and a `.gitignore` with exactly `.obsidian/workspace*.json`, `.obsidian/cache`, `.trash/`. `main` and `master` both exist at the same SHA; HEAD is `main`. So: branch name comes from config `journal.branch` (default `main`), bootstrap = `git clone --branch <branch> -- <url> <dir>` when the remote has that branch; when the remote is truly empty (a fresh bare repo, which is also what tests use) `git clone` succeeds with "warning: You appear to have cloned an empty repository" and the local branch follows the server's unborn HEAD (ran: `master`, `main`, `trunk` for three bare repos), so force the name: `git checkout -b <branch>` on the unborn branch (ran: `master` -> `main`) and first push `git push -u origin HEAD:refs/heads/<branch>` (ran: remote then has `refs/heads/main`, upstream `origin/main`). Pitfall seen in the probe: if the remote HEAD names a different branch than the one pushed, a fresh clone says "remote HEAD refers to nonexistent ref, unable to checkout", so always `fetch` + `checkout -B <branch> origin/<branch>` rather than trusting clone's checkout. The first real export will rewrite the demo files ("lần xuất sau sẽ ghi đè" is printed in the demo's own header) and push a fast-forward commit; never touch `master`.

**Sync sequence per export (all verified with local bare remotes, git 2.43.0):**
1. `lock` -> if `<dir>` missing: clone (as above). If `<dir>` exists but is not a git repo and not empty: refuse + notify, do not delete.
2. Dirty check: `git status --porcelain -z --untracked-files=no`. Tracked paths dirty and all inside the generated path set (left by a crashed run) -> `git checkout -- <those paths>` (derived data, regenerated next); any other dirty path (a human edited) -> **stop, notify, do not overwrite** (D-10). Untracked stray files are left alone (ran: unrelated untracked file stays out of the commit).
3. `git fetch origin <branch>` (timeout). Failure = network/ssh category -> record failure, keep local commits, stop. Then `git rev-list --left-right --count HEAD...origin/<branch>`: `0 0` nothing; `0 N` behind -> `git merge --ff-only origin/<branch>`; `N 0` ahead -> nothing to integrate; `N M` diverged -> `git rebase origin/<branch>` (our unpushed commits only touch generated files, so this normally succeeds; ran: non-conflicting rebase succeeded and the following push exited 0). If the rebase exits non-zero (`CONFLICT (content)`, ran: exit 1, `git diff --name-only --diff-filter=U` lists the file), run `git rebase --abort` (ran: exit 0, tree clean, HEAD back on our commit), record a **blocked** state, notify once, and stop until the remote moves or the user fixes it. Never `reset --hard` to the remote and never force-push.
4. Write files (writer), then `printf '<path>\0...' | git add --pathspec-from-file=- --pathspec-file-nul` with exactly the changed and deleted relative paths (ran: stages modifications and deletions, leaves unrelated untracked files alone). No `-A`, no `-f`.
5. Nothing-to-commit detection: `git diff --cached --quiet` (exit 0 = nothing staged, exit 1 = something staged; ran). Skip commit and push when 0 (idempotency: second run = no commit).
6. `git commit -m "journal: <date> <HH:MM> (<n> file)"` with the fixed author. A timestamp in the **commit message** is fine (metadata); none in file bodies.
7. `git push origin HEAD:refs/heads/<branch>` with a timeout. Non-fast-forward rejection (exit 1, `! [rejected] HEAD -> main (fetch first)`, ran) -> one fetch+rebase retry, then fail. Network/DNS failure: exit **128**, stderr `ssh: Could not resolve hostname ...` / `fatal: Could not read from remote repository.` (ran). Classify from exit code + `LC_ALL=C` stderr into {network, ssh/auth (`Permission denied`, `Host key verification failed`), rejected, other}; put only the category and a sanitised first line in Telegram (raw stderr can contain URLs/paths).
8. Phone safety: **history must stay linear and published history must never be rewritten** (rebase of unpushed local commits is fine). obsidian-git on mobile does not support the rebase merge strategy [CITED: https://publish.obsidian.md/git-doc/Getting+Started].

### 5. Secret scanning and sanitising (item 5)

**What exists.**
- `secrets.rs` is **value-based**, not pattern-based: `pub fn contains_secret(content: &[u8], secrets: &[String]) -> bool`, `pub fn redact_str(text, secrets)`, `pub fn scan_for_secrets(root, paths, secrets)`; `MIN_SECRET_BYTES = 8`, `REDACTED = "[đã ẩn]"`, `SECRET_SCAN_MAX_BYTES = 1_048_576` (files over 1 MiB are skipped), symlinks not followed [VERIFIED: agent-brain/src/secrets.rs:14-165]. It can be called on generated Markdown (use `contains_secret(content.as_bytes(), &secrets)` on the in-memory bytes before writing, avoiding the 1 MiB skip) but only finds **known values**.
- Where the values come from: `inputs::env_secrets()` (every env var whose NAME contains `key`/`token`/`secret`/`password` and whose value is >= 8 bytes, inputs.rs:58-72) and `inputs::collect_secrets(st: &AppState)` (adds dashboard token, Telegram token, engine key envs; inputs.rs:75-101). The CLI path has no `AppState`: build the list from `env_secrets()` + `cfg.engines.all_key_envs()` + `telegram::config::load(...)` token. A false positive (a short English word equal to some env var value >= 8 bytes) blocks the export by design; the report prints paths/names only.
- `spikes/lib/scan-secrets.sh` hard-codes `TARGET="$REPO/spikes"`; it cannot scan the vault. Its useful content is the pattern list: `sk-[A-Za-z0-9_-]{16,}|sk-ant-[A-Za-z0-9_-]{8,}|ghp_[A-Za-z0-9]{20,}`, `Bearer [A-Za-z0-9._-]{16,}`, `eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]+\.`, `/home/$USER`, emails; and the env-value denylist. [VERIFIED: spikes/lib/scan-secrets.sh]

**What they miss for this use:** internal absolute paths other than `/home/$USER` (`/mnt/...`, `~/.config/...`, `C:\...`), relative paths and branch names (`task/foo`), URLs and bare hostnames/IPs/`host:port`, env-var-name shaped tokens (`DEEPSEEK_API_KEY`), `key=value` secrets, Telegram bot tokens (`\d{6,}:[A-Za-z0-9_-]{30,}`), AWS/Google/GitLab/GitHub-fine-grained keys, PEM blocks, long hex/base64 blobs, zero-width/bidi characters.

**Minimal sanitizer (recommended; lists are recommendations, A9).** One function `sanitize_text(&str, max_chars) -> String` applied to **every** string that came from the DB (task/proposal title, project display name; status labels and group labels are constants). Order matters:
1. NFC-normalise; delete control chars, zero-width (U+200B-U+200F), bidi controls (U+202A-U+202E, U+2066-U+2069), BOM; turn `\r`, `\n`, `\t` and runs of whitespace into one space (the text becomes **one line**).
2. Replace with a placeholder (demo style: URLs -> `[link]`, paths -> `[…]`): URLs (`\b[a-z][a-z0-9+.-]*://\S+`, `www\.\S+`), bare hosts (`\b[\w-]+(\.[\w-]+)*\.(com|net|org|vn|io|dev|app|local|internal|lan|corp|test|cloud)\b(:\d+)?`), IPv4/`localhost:port`, absolute/home/relative paths (token containing `/` or `\` with a second segment, token starting `~` or `/`, `[A-Za-z]:\\`), emails, `key|token|secret|password|passwd|pwd|api[_-]?key|authorization|bearer` followed by `[:=]\s*\S+`, PEM `-----BEGIN`, Telegram token shape, the pattern list from scan-secrets.sh, ALL_CAPS_WITH_UNDERSCORE tokens (env var names), any `[A-Za-z0-9+/=_-]{24,}` token, and every known secret value (`redact_str`).
3. **Run steps 1-2 on the full text, then truncate** (to `journal.title_max_chars`, default 140, on a char boundary, preferably at a word, add `…`). Truncating first could cut a key in half so that no pattern matches.
4. Neutralise Markdown/Obsidian syntax by **allow-list**: keep Unicode letters/digits, space and a small punctuation set (`. , ; : ( ) & + - _ ' " ? ! …`); replace everything else (including `[ ] ! # \` < > | * ~ = % $ ^ { } \\ @ /`) with a space and collapse spaces. Because the text is a single line placed **after** a fixed prefix (a bullet or `> ` marker), a leading `#`, `-`, `>` or `---` can never start a block, and with `[`/`]`/`<`/`>`/backtick/`%`/`$`/`{`/`^` gone there is no `[[x]]`, `![[x]]` (embed of another vault file), `#tag`, `%%comment%%`, `$$math$$`, `<html>`, `` `dataview` ``, `{{template}}`, `^block-id` or footnote syntax left. Titles never go into YAML frontmatter (frontmatter carries only date, counts and fixed tags), so `---` in a title cannot break the header.
5. **Final gate** `scan_generated(map: &BTreeMap<String,String>, secrets) -> Vec<(path, rule)>` on every generated byte (pattern set + known values) **before any file is written**. Any hit: write nothing, commit nothing, push nothing, put a `secret` row in `journal_mail` containing only file paths and rule names (never a value), keep the last good vault untouched. This satisfies JRN-03 "khi phát hiện thì không commit, không push, báo người dùng".

Where it sits: `render` takes only already-sanitised strings (`SafeText` newtype with a private constructor in `sanitize.rs`), so unsanitised user text cannot reach a template by type; the gate runs on the render output and also in `writer` as a second check (defence in depth). Fuzz-ish test: feed a corpus (`![[secret]]`, `[[x]]`, `#tag`, backticks, `---`, `<script>`, `%%`, `$$`, `{{`, `\n# Heading`, zero-width, RTL override, `sk-ant-...`, `/home/x/.ssh/id_rsa`, `https://internal.example/path?token=abc`, `DEEPSEEK_API_KEY=abcd1234efgh`, a 4,000-char title) through `sanitize_text` and `render`, then assert: one line, <= max chars, no char outside the allow-list, none of `[[ ]] ![ < > ` %%` present, no pattern from the gate matches.

### 6. Obsidian and obsidian-git facts (item 6)

| Fact | Status |
|------|--------|
| obsidian-git mobile uses isomorphic-git (JS reimplementation); "very unstable", the author does not recommend mobile use; **no SSH authentication** (HTTPS + personal access token for GitHub); **repo size limited by memory**; **no rebase merge strategy**; no submodules; no symlinks/exec bit; Obsidian may crash on clone/pull. | [CITED: https://publish.obsidian.md/git-doc/Getting+Started and https://raw.githubusercontent.com/Vinzent03/obsidian-git/master/README.md] |
| "A filename that works on macOS may fail on Android, preventing a pull or checkout." Avoid `< > : " / \ | ? *`, control chars, trailing dots/spaces, names differing only by case or Unicode normalisation. | first sentence [CITED: Getting+Started]; the character list is from plugin/forum pages [ASSUMED-level, A4] |
| Periodic pull: commit-and-sync "run automatically every X minutes", and "You can also disable the pulling or pushing part from the 'Commit-and-sync' section"; README lists "Auto-pull on Obsidian startup". For a pull-only phone: disable commit and push parts so the phone never creates commits. | [CITED: https://publish.obsidian.md/git-doc/Start+here, README]. Whether an interval pull fires on Android only while the app is open is not documented there (a review says the plugin "only runs while Obsidian is open") [ASSUMED, A3] |
| Folder paths in links start at the vault root with `/`; characters `# \| ^ : %% [[ ]]` "may not work as a link"; embed syntax `![[Internal links]]`; blocks `![[note#^id]]`. | [CITED: https://obsidian.md/help/links, https://obsidian.md/help/embeds] |
| Callouts: `> [!type]` first line, `+`/`-` after the type makes them foldable ("A plus sign expands the callout by default, and a minus sign collapses it instead"); unsupported types render as `note`. | [CITED: https://obsidian.md/help/callouts] |
| Shortest-path wikilinks: with "Shortest path when possible" Obsidian writes just the note name when it is unique in the vault and adds path only when needed. Existing links resolve by unique name regardless of the setting. | forum-level sources only [ASSUMED, A5] |
| GitHub does **not** render Obsidian syntax: alerts are `> [!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, `[!CAUTION]` ("Five types of alerts are available"), so `> [!success]-` shows as a plain blockquote with literal `[!success]-`; wikilinks `[[x]]`/`![[x]]` are not described for repository files (assume literal). | alerts [CITED: https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax]; wikilink literalness [ASSUMED, A5] |

**Consequences for layout.**
- **Normalisation:** git on Linux stores path bytes exactly as written (ran: `Nhật ký/...` stored as `4e68 e1ba ad74 20 6b c3bd`, i.e. precomposed NFC `ậ` U+1EAD and `ý`; `core.precomposeunicode` is unset on Linux, it is a macOS setting). So write NFC and assert it in a test (`path.nfc().eq(path.chars())` for every emitted path). To remove the risk further, only the two folder names carry diacritics; every **file** name is ASCII (`YYYY-MM-DD.md`, `YYYY-MM.md`, `YYYY.md`, slug), and links never need to mention the folders.
- **Embed resolution:** `Today.md` can use `![[2026-11-10]]` (unique name) or the path form `![[Nhật ký/2026/11/2026-11-10]]`; both resolve. Prefer **path-qualified links for project pages** (`[[Dự án/apec|apec]]`) because a project slug can collide with another note name case-insensitively (e.g. a project called `today`), and short names for date files (unique by format). Link only to files that exist: an unresolved wikilink clicked on the phone **creates an empty note** locally. "Yesterday" must be the most recent earlier day that has a file; when today has no file, `Today.md` must say so in text instead of embedding a missing note.
- **Repo policy files:** the repo already has `.gitignore` (3 lines above). Recommend: bootstrap `.gitignore` only when absent (never overwrite), and ignore the whole `.obsidian/` so neither plugin data (which can hold the phone's PAT) nor volatile workspace files can ever be committed or block a pull; the exporter never writes inside `.obsidian/`. No `.gitattributes`: every writer is LF/UTF-8 without BOM (demo file: `UTF-8 Unicode text`, 0 CR); mobile isomorphic-git ignoring attributes is [ASSUMED, A7].
- **Phone setup is manual (document, not code):** HTTPS clone with a fine-grained read-only PAT; pull-only settings above; no push. The agent-brain side must therefore never rewrite published history.
- **GitHub link encoding.** Path `Nhật ký/2026/11/2026-11-10.md` becomes `https://github.com/vinhhx/agent-brain-journal/blob/main/Nh%E1%BA%ADt%20k%C3%BD/2026/11/2026-11-10.md` (UTF-8 bytes percent-encoded, space `%20`, `/` kept) [VERIFIED: ran `urllib.parse.quote(path, safe='/')`]; `Dự án/apec.md` -> `D%E1%BB%B1%20%C3%A1n/apec.md`. Encode with a tiny RFC 3986 function (unreserved = `A-Za-z0-9-._~`, keep `/`) and a unit test against these two strings. An unauthenticated request to the private URL returns 404 (as expected); that GitHub accepts the encoded form when logged in is standard URL handling [ASSUMED, A10]. Derive the web base from `journal.repo_url` (`git@github.com:owner/repo.git` or `https://github.com/owner/repo(.git)`) with an optional `journal.web_base_url` override; do not hard-code the owner/repo.

### 7. Idempotency and churn (item 7)

- **Structure:** `fn render_all(model: &Model, cfg: &RenderCfg) -> BTreeMap<String, String>` (relative path -> full file text). `BTreeMap` fixes iteration order; inside files sort by (group order, project, first-run time, task id). No `now()` inside render; the only clock input is `today` passed in (for `Today.md` and for the "carry-over" rule). No timestamp, hash or run id in a file body unless it is data-derived (e.g. "27 phút" = sum of run durations of that day is data-derived and fine).
- **Writer:** for each (path, text): read the existing bytes, write only if different via temp file in the same directory + `rename`; compute the changed-path list for `git add`. **Stale files:** list files under the managed roots (`Nhật ký/`, `Dự án/`, and `Today.md`); delete those not in the map **only if they begin with the generated marker line** (the demo's marker is `<!-- Tệp sinh tự động từ agent-brain — không sửa tay, lần xuất sau sẽ ghi đè. -->`, placed after the frontmatter in day/month/year/project files and on line 1 in `Today.md`). A managed-path file **without** the marker is a human file: do not overwrite or delete it, report it (D-10 spirit), exclude it from the commit.
- **Second run = no diff:** after the first export, a second run on the same DB writes nothing, `git diff --cached --quiet` is 0, no commit (success criterion 1).
- **Golden tests:** build a fixture DB with `db::open(&tmp/brain.db)` and inserts (the repo's unit tests already do this: `fresh_db()`/`task_run()` in db.rs:2163-2220, and `tests/db_stats.rs:1-120`); freeze `now_ms` (insert explicit `started_at`/`ended_at` values), use `FixedOffset +07:00`; render; compare each file with `tests/fixtures/journal/golden/<path>`; refresh with an env var (e.g. `UPDATE_GOLDEN=1`) written by the test helper (no snapshot crate). Fixture must include: task spanning two days, a run at 23:59:59 and 00:00:01 local, quota/env/blocked/fix/violation/failed/gate cases, a task without a proposal (title = raw multi-line request with path, URL, fake key), a project with `journal_group` valid, absent and invalid, and a project whose slug needs fallback.

### 8. Failure handling (item 8, keep small)

Persist in brain.db (migration 12), not in a file: it is covered by the same backup, atomic, and survives restart.

```sql
CREATE TABLE journal_state (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at INTEGER NOT NULL);
CREATE TABLE journal_mail (id INTEGER PRIMARY KEY, kind TEXT NOT NULL, dedupe_key TEXT NOT NULL UNIQUE,
  text TEXT NOT NULL, created_at INTEGER NOT NULL, sent_at INTEGER);
-- plus journal_fts (see FTS5)
```
Keys in `journal_state` (suggested): `last_export_ms`, `last_export_day`, `last_pushed_sha`, `push_fail_streak` (int), `push_fail_since_ms`, `push_alert_sent` (`0|1`), `blocked` (empty or `conflict`/`dirty`/`secret`/`foreign-file`), `fts_hash`.
- **Counting:** each export that ends in a push/fetch failure increments `push_fail_streak`; a success resets it to 0 and, if `push_alert_sent = 1`, enqueues one `recovered` mail then clears the flag. A cycle with nothing to push and a reachable remote also resets.
- **Notify once, not every cycle:** when `push_fail_streak >= journal.push_fail_notify_after` (suggest default 3, range 1-100) and `push_alert_sent = 0` -> insert `journal_mail(kind='pushfail', dedupe_key='pushfail:<since_ms>')` and set the flag. The UNIQUE `dedupe_key` makes duplicates impossible, including after restart.
- **Retry cadence:** failures do not add extra attempts; the next attempt is the next normal tick (`journal.interval_minutes`, default 30). Local commits accumulate and the first successful push publishes them all (ran: rebase+push after remote moved).
- **Blocked states** (`conflict`, `dirty`, `foreign-file`, `secret`): do nothing destructive, send one mail per distinct cause (`dedupe_key` includes the remote SHA or file list hash), and skip git work until the cause changes (remote SHA moved, tree clean, secret gone). `agent-brain journal export` run by hand re-evaluates immediately and prints the reason.
- **Mail delivery:** `detect.rs` scan `SELECT id, text FROM journal_mail WHERE sent_at IS NULL ORDER BY id`, then `UPDATE journal_mail SET sent_at = ?1 WHERE id = ?2 AND sent_at IS NULL` and push `Outgoing::Message` only when `changes() == 1` (claim-before-push: at-most-once, no duplicate after a restart; a crash between claim and delivery loses one message, acceptable for a digest, A11). If Telegram is not configured, rows stay unsent and harmless.
- **Digest scheduling without double sending:** `digest_due(now_ms, tz, digest_hhmm, last_digest_day) -> Option<Day>` is pure: due when local time >= `digest_time` and `last_digest_day != today`. Create `journal_mail(kind='digest', dedupe_key='digest:YYYY-MM-DD')` only **after the export for that day has pushed successfully** (or the vault is already at HEAD), so the GitHub link is not stale. A serve started at 19:00 still sends today's digest once; a restart afterwards finds the unique key and does nothing.

### Pattern: config (`journal` section of config.json, all optional, absent = feature off)

| Key | Default | Range / validation |
|-----|---------|--------------------|
| `enabled` | `false` | bool; serve starts no loop unless true; `journal export` works whenever `repo_url` is set |
| `repo_url` | none (no default; do not hard-code) | non-empty, no whitespace/NUL, must not start with `-`; scheme allow-list `git@host:`, `ssh://`, `https://`, `file://`, absolute path (tests) |
| `branch` | `main` | `[A-Za-z0-9._/-]{1,100}` |
| `clone_dir` | `<data_dir>/journal` (= `~/.local/share/agent-brain/journal`, D-11) | `~` expanded with `expand_tilde` |
| `interval_minutes` | `30` | 1-1440 |
| `digest_time` | `"18:00"` | `HH:MM`; `digest_enabled` default `true` |
| `push_fail_notify_after` | `3` | 1-100 |
| `title_max_chars` | `140` | 40-400 |
| `git_timeout_seconds` | `60` | 10-600 |
| `author_name` / `author_email` | `agent-brain journal` / a non-personal placeholder address | non-empty |
| `groups` | `[{id:"cong-viec",label:"Công việc"},{id:"video",label:"Video"},{id:"ho-tro",label:"Hỗ trợ bộ phận khác"}]` | ordered list, ids `[a-z0-9-]`, first = default group (D-08) |
| `utc_offset_minutes` | absent (= `chrono::Local`) | -720..=840 |
| `web_base_url` | derived from `repo_url` | https URL |

Per project in `projects_map.json`: `"journal_group": "<group id>"`; absent or unknown id -> first group (default) with a one-time warning (`term::emit`), same leniency as `preflight::read_lenient`. Note: the demo calls its second group "Phân tích & hỗ trợ bộ phận khác" and shows no `Video` group; making the labels configurable removes the conflict (Open Question 1).

### Anti-Patterns to Avoid
- **Reading plan/QA/diff files or `projects.path`/`tasks.workdir`/`branch`/`run_dir` for any purpose.** D-04 says the journal cannot leak what it never reads.
- **Using `tasks.title` raw, or `tasks.status` as the day's status.** One is the full request, the other is only the present.
- **Using `quota_stops` or `status = 'failed'` as the quota/failure signal** (see table).
- **Calling `gitops::commit_paths` / `gitops::git` for the vault** (identity, signing, `-f`, no timeout).
- **Pushing the digest/alert through `BotCtx` directly from the tokio loop** (not reachable); use the mailbox.
- **`git add -A`, `git push --force`, `git reset --hard origin/...`, `rebase` of published commits, `amend`.**
- **Trusting `unicode61 remove_diacritics` for Vietnamese** (does not fold `đ`).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Timezone/day maths | manual `ms % 86_400_000 + 7h` arithmetic | `chrono` generic over `Tz` (`stats::day_key` pattern, `Local` / `FixedOffset`) | Month/year/weekday/leap handling, testable with a fixed offset |
| Vietnamese folding for search | custom diacritic tables | `routing::normalize` (NFD + strip `Mn` + `đ->d`) | Already tested in the repo; covers `đ` which SQLite does not |
| Full-text search | `LIKE '%x%'` over rows or an own inverted index | SQLite FTS5 (compiled in) + quoted-token MATCH builder | Ranking, prefix, NFC/NFD handling, speed |
| Git protocol | own transport, libgit2 binding | `git` CLI via a small timeout-aware runner | Already how the repo works; SSH config/agent reuse |
| "Nothing to commit" | parse `git commit` output | `git diff --cached --quiet` exit code | Message is on stdout and locale-dependent |
| Atomic file write | `fs::write` in place | temp file in same dir + `rename` (as `secrets::redact_file`, `config::save`) | No half-written Markdown on crash |
| Process-tree kill on timeout | `child.kill()` only | `procs::kill_tree` | ssh is a child of git |
| Secret value matching | own substring search | `secrets::contains_secret` / `redact_str` for known values (+ new regex set for shapes) | Already handles JSON-escaped spellings and longest-first replacement |
| Serve-wide singleton | PID file by hand | `libc::flock` pattern of `dashboard/lock.rs` | Releases automatically on crash |
| Percent-encoding | string replace of spaces only | 8-line RFC 3986 encoder + test vs the two strings in this file | Diacritics are multi-byte; spaces alone break Telegram autolinks |

**Key insight:** the dangerous parts here are *what leaves the machine* (user text, secrets) and *what gets rewritten* (history, human files). Both are solved by narrowing inputs (allow-list sanitiser, marker ownership, explicit pathspecs) rather than by adding capability.

## Common Pitfalls

### Pitfall 1: Migration 12 fails on the live database (orphan events)
**What goes wrong:** `db::open` runs pending migrations in a txn and then `SELECT COUNT(*) FROM pragma_foreign_key_check`; if `bad > 0` it returns `"Nâng cấp CSDL thất bại: vi phạm khóa ngoại sau khi dựng lại bảng runs"` and rolls back (db.rs:291-298). The live DB (copy) has **645 violations, all `events` -> `runs`** (orphan events from deleted runs), while the Nov 9 backup `brain.db.bak-20261109-xoa-task-dung` has 0 [VERIFIED: ran `PRAGMA foreign_key_check` on both]. Result: the new binary would refuse to start serve and every CLI command on the real data.
**Why:** `foreign_keys=OFF` during migration plus rows deleted earlier with foreign keys off; the check was written for the table-rebuild migrations and counts pre-existing damage as a new failure.
**How to avoid:** in `db::open`, count violations **before** applying pending migrations (same txn) and fail only if the count **increases**; add a `tests/db_migration.rs` case "pre-existing orphan rows do not block an unrelated migration". Optionally document `DELETE FROM events WHERE run_id NOT IN (SELECT id FROM runs)` in the README upgrade step (after the backup). Plan this as the **first task**, before adding migration 12.
**Warning signs:** `agent-brain` prints the FK error right after installing the new binary.

### Pitfall 2: Raw request used as the title
`tasks.title` averages 1.5 KB and is often multi-line with pasted content, URLs, branch names. Prefer `proposals.title`; always sanitise and truncate.

### Pitfall 3: quota and failure categories misclassified
Quota runs are `failed`; protocol violations are `succeeded`; `quota_stops` is empty when the orchestrator is off. Key on `runs.outcome` with the precedence in the attention table.

### Pitfall 4: Past day files change retroactively
`tasks.approved_at` is the latest approval only; a later re-plan clears it. Document that the gate rule is an approximation for past days, and keep today's carry-over out of past-day files so they stabilise after midnight. One more export after 00:00 finalises yesterday (loop must trigger on day rollover).

### Pitfall 5: Today.md and "yesterday" link to missing notes
Missing targets create empty notes when tapped on the phone. Link only existing files; when today has no file, print text.

### Pitfall 6: Project slug as a file name
`projects_map.json` is hand-editable: a slug with `/`, `..`, leading dot, uppercase collisions, or Windows/Android-hostile characters becomes a path. Validate `^[a-z0-9][a-z0-9._-]{0,63}$`; otherwise `routing::slugify(name_vi)`; if empty or colliding use `du-an-<project_row_id>`. Never use `name_vi` as a path.

### Pitfall 7: The first export overwrites the user's demo
Expected and announced by the demo's own header, but run `journal export --no-push` first in the live check and show the diff; the demo format choices (callouts, group names, "Số bước chạy") are decisions the user already made visually (Open Question 1).

### Pitfall 8: Pulling a dirty or diverged clone
`git rebase`/`pull` refuse or misbehave with a dirty tree; handle in the fixed order in the sync sequence; never `--autostash`.

### Pitfall 9: Two renderers
Telegram sends a github.com link (D-14), but Obsidian callouts and wikilinks are not rendered by GitHub. Keep the day body plain Markdown (headings + bullet list, as D-06's "một dòng mỗi task" suggests); use Obsidian-only syntax (embed, wikilinks) only in navigation lines (`Today.md`, header/footer links) where degradation is harmless. If the user prefers the demo's foldable callouts, the GitHub view shows `> [!success]-` literally (A5).

### Pitfall 10: Telegram link breaks on spaces
`Nhật ký` contains a space and diacritics; un-encoded, plain-text autolinking stops at the space. Always percent-encode (section 6) and keep the URL as the last token on its line.

### Pitfall 11: FTS query injection and DoS
Raw MATCH strings throw errors or select columns (`title:x`). Use the quoted-token builder; cap query length (e.g. 100 chars), tokens (8), results (e.g. 8 lines, clipped with `render::clip(text, MAX_TEXT)`, MAX_TEXT = 3900 at render.rs:13).

### Pitfall 12: Test expectations that change
`tests/db_migration.rs:976` asserts `SCHEMA_VERSION == 11`; `tests/telegram_intake.rs:1953-1971` asserts the exact `setMyCommands` list; `HELP_TEXT` is compared by reference so it may change. `Config` has one literal constructor (config.rs:1501); `Project` has 41 literals (do not add a field).

### Pitfall 13: Git environment leaks into behaviour
User/global git config can enable signing, hooks, `url.insteadOf`, `core.autocrlf`. Pass `-c commit.gpgsign=false -c core.autocrlf=false` and the author via env; keep the user's `~/.ssh/config`. `GIT_CONFIG_GLOBAL` is overridden in `Sandbox` tests only.

### Pitfall 14: Vault growth on the phone
obsidian-git mobile has a memory-bound repo size. Project pages that list every task ever grow without bound (10-30 tasks/day); keep per-project pages to recent months in full and older months as counts with links (config `journal.project_page_months`, suggest 3), keep history small (no binaries, no per-task files).

### Pitfall 15: Export while agents write
Use one read transaction for the whole collect (`BEGIN` deferred, WAL gives a stable snapshot) so a run finishing mid-export cannot produce inconsistent counts; `db::open` sets `busy_timeout` 5000 ms.

### Pitfall 16: Shutdown during a push
`async_serve` drops tasks at exit; an interrupted `git push` is safe (atomic ref update), but do not leave `index.lock`: remove a stale `.git/index.lock` only if no journal lock is held and the file is older than the git timeout.

## Code Examples

Illustrative skeletons for the executor; every value used is quoted in the sections above. Not production code.

### Day bounds generic over `Tz` (pattern of stats.rs:219 and intake.rs:442)
```rust
// Source: agent-brain/src/stats.rs:219 (day_key) and telegram/intake.rs:442 (local_midnight_ms)
use chrono::{DateTime, NaiveDate, TimeZone};
fn day_bounds_ms<Tz: TimeZone>(d: NaiveDate, tz: &Tz) -> (i64, i64) {
    let start = (0..=3).filter_map(|h| d.and_hms_opt(h, 0, 0))
        .find_map(|t| tz.from_local_datetime(&t).earliest()).map(|t| t.timestamp_millis()).unwrap();
    let next = d.succ_opt().unwrap();
    let end = (0..=3).filter_map(|h| next.and_hms_opt(h, 0, 0))
        .find_map(|t| tz.from_local_datetime(&t).earliest()).map(|t| t.timestamp_millis()).unwrap();
    (start, end) // run belongs to d when COALESCE(ended_at, started_at) in [start, end)
}
// tests: chrono::FixedOffset::east_opt(7 * 3600) (tests/dashboard_stats.rs:135)
```

### FTS fold + safe MATCH (ran in scratch; uses routing::normalize semantics)
```rust
fn fold(text: &str) -> String { /* == crate::routing::normalize(text) */ }
fn match_query(user: &str, max_tokens: usize) -> Option<String> {
    let toks: Vec<String> = fold(user)
        .split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()).take(max_tokens)
        .map(|t| format!("\"{}\"", t.replace('"', "\"\""))).collect();
    if toks.is_empty() { None } else { Some(toks.join(" ")) }   // implicit AND; optional "last"* prefix
}
// SELECT day, task_ref FROM journal_fts WHERE journal_fts MATCH ?1
//   ORDER BY bm25(journal_fts), day DESC LIMIT 8
```

### Migration 12 (executed inside BEGIN IMMEDIATE on a copy of the live DB; user_version -> 12)
```sql
CREATE TABLE journal_state (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at INTEGER NOT NULL);
CREATE TABLE journal_mail (id INTEGER PRIMARY KEY, kind TEXT NOT NULL, dedupe_key TEXT NOT NULL UNIQUE,
  text TEXT NOT NULL, created_at INTEGER NOT NULL, sent_at INTEGER);
CREATE VIRTUAL TABLE journal_fts USING fts5(task_ref UNINDEXED, day UNINDEXED,
  title, project, grp, day_text, status, tokenize='unicode61 remove_diacritics 0');
```

### Git sync commands (all ran against local bare remotes; `$G` = `git -c user.name=... -c user.email=... -c commit.gpgsign=false`)
```bash
git clone --branch main -- "$URL" "$DIR"          # or: git clone -- "$URL" "$DIR" && git checkout -b main   (empty remote)
git fetch origin main
git rev-list --left-right --count HEAD...origin/main      # "ahead behind"
git merge --ff-only origin/main                           # behind only
$G rebase origin/main || git rebase --abort               # diverged; conflict => abort + notify
printf 'Nhật ký/2026/11/2026-11-10.md\0Today.md\0' | git add --pathspec-from-file=- --pathspec-file-nul
git diff --cached --quiet || $G commit -m "journal: 2026-11-10 14:30 (2 file)"
git push origin HEAD:refs/heads/main                      # first push of an empty remote adds -u
```

### Digest claim (at-most-once, restart-safe)
```sql
INSERT OR IGNORE INTO journal_mail(kind, dedupe_key, text, created_at) VALUES ('digest', 'digest:2026-11-10', ?1, ?2);
-- detect.rs scan: SELECT id, text FROM journal_mail WHERE sent_at IS NULL ORDER BY id;
UPDATE journal_mail SET sent_at = ?1 WHERE id = ?2 AND sent_at IS NULL;   -- push Outgoing::Message iff changes() == 1
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `unicode61 remove_diacritics 1` | `remove_diacritics 2` handles stacked marks (`ỗ`) | SQLite 3.27+ (bundled is 3.53.2) | Still not enough for `đ`; fold in Rust |
| `git pull` for the exporter | explicit `fetch` + `ff-only` / `rebase` + `abort` | n/a | Deterministic conflict handling, no hidden merge commits (phone cannot rebase) |
| Contentless FTS5 deletes limited | `contentless_delete=1` (3.43+) available | 3.43 | Not needed: plain table rebuilt per export |

**Deprecated/outdated:** `obsidian://` deep links for the digest (D-14 rejects them); SSH from the Android obsidian-git plugin (unsupported, use HTTPS+PAT).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Past-day gate/attention reconstruction from `approved_at` + last run is acceptable (approximation, not exact history) | Data map | Past files may show/omit a gate item that was real; user can accept or ask for stricter rules |
| A2 | Carry-over of unresolved attention items only on *today's* file/digest, not in past-day files | Data map | User may expect every day file to list all open items |
| A3 | obsidian-git interval pull on Android works only while Obsidian is open | Obsidian facts | Phone may lag; docs do not say |
| A4 | Android-hostile character list and NFC/NFD collision claim come from forum/plugin pages | Obsidian facts | Low: file names are ASCII; only two folder names have diacritics |
| A5 | Shortest-name wikilinks resolve when unique; GitHub shows `[[x]]` and `> [!success]-` literally | Obsidian facts, Pitfall 9 | Day file looks cluttered on github.com if callouts are used |
| A6 | `-c core.hooksPath=/dev/null` is safe/needed to neutralise a global hook | Git runner | A user hook could run on vault commits |
| A7 | Mobile isomorphic-git ignores `.gitattributes`; no CRLF conversion | Repo policy | CRLF churn if wrong; mitigated by LF-only writers |
| A8 | A serve started under `TZ=UTC` is a realistic risk worth `utc_offset_minutes` | Day boundaries | Extra optional config key if not needed |
| A9 | Regex/placeholder lists in the sanitizer are a sensible minimum | Sanitizer | Leaks of unusual secret shapes; the final gate and known-value scan reduce risk |
| A10 | GitHub web view accepts the percent-encoded UTF-8 path while logged in | GitHub link | Link 404 for logged-in user (low probability; standard URL encoding) |
| A11 | At-most-once digest/alert delivery (claim before push) is preferable to at-least-once | Failure handling | One lost message on a crash in a tiny window |
| A12 | `journal.enabled` defaults to false and `repo_url` has no default | Config | User must add both to config.json once |

## Open Questions

1. **Which format does the user want: the demo already in the repo, or a stricter reading of D-06?**
   - Known: the demo (hand-pushed 2026-11-10) uses foldable callouts per task, "Số bước chạy" and duration, frontmatter `date/tasks/tags`, a generated-file marker, wikilinks by short name, and groups "Công việc" + "Phân tích & hỗ trợ bộ phận khác" (no `Video`). D-06/D-07 say one line per task and groups Công việc / Video / Hỗ trợ bộ phận khác. GitHub does not render callouts/wikilinks (D-14 link target).
   - Unclear: keep callouts (rich in Obsidian, noisy on github.com) or plain bullets; name of the third group.
   - Recommendation: confirm in `/gsd-discuss-phase` or at plan review; default to plain bullets for task lines + Obsidian syntax only in navigation, labels from `journal.groups`.
2. **How to unblock migration 12 on the live DB (645 orphan events).**
   - Known: pre-check counts as failure (Pitfall 1).
   - Unclear: fix in code (compare before/after) vs one-time manual cleanup, or both.
   - Recommendation: code fix + test as plan task 1; README cleanup note optional. Needs the user's nod only if they prefer a manual cleanup.
3. **Automatic backup before migrate, or README step only (JRN-07 wording "migration có sao lưu trước")?**
   - Known: today's convention is a manual README step per phase (README.md:36-63) plus `scripts/phase6-live-check.sh --schema-dry-run` on a copy; `db::open` does no backup.
   - Recommendation: README section "Nâng cấp lên Phase 7 (lược đồ 12)" + generalise the dry-run script; no automatic copy inside `open()`.
4. **`journal_group` edit surface.** §0 rule 2 of the e2e plan asks for a dashboard field/endpoint per new option; this phase adds no UI. Recommend: hand-edit `projects_map.json` now, record the missing endpoint as a follow-up (or add a tiny `PUT /api/projects/{id}/journal` in `api_projects.rs`, pattern `update_preflight`, if the user wants strict compliance).
5. **Digest timing relative to push** (send only after a successful push so the link is fresh) vs always at 18:00. Recommend the former (Failure handling).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| git | gitsync, tests | yes | 2.43.0 | none needed |
| ssh (OpenSSH) | push/fetch to GitHub | yes | 9.6p1; BatchMode + StrictHostKeyChecking=yes `ls-remote` succeeded | HTTPS remote in config for other machines |
| Access to `git@github.com:vinhhx/agent-brain-journal.git` | live check | yes | read OK (`ls-remote` exit 0); **write not tested** | user confirms at live check |
| cargo / rustc | build, tests | yes | 1.98.1 / 1.98.1 (`rust-version = "1.85"`) | none |
| bundled SQLite with FTS5 | `/tim`, migration | yes | 3.53.2, ENABLE_FTS5=1 | none (system sqlite not used) |
| python3 | existing fake Telegram | yes | 3.12.3 | none |
| sqlite3 CLI | none required | no | - | not needed (probes used scratch Rust crate and python) |
| System time zone | day boundaries (`Local`) | yes | Asia/Ho_Chi_Minh (+07) | `journal.utc_offset_minutes` |
| Telegram live bot, Obsidian on phone | live acceptance | manual | - | covered by fake Telegram + bare remote in automated tests |

**Missing dependencies with no fallback:** none. **Missing with fallback:** none.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `cargo test` (unit `#[cfg(test)]` + `agent-brain/tests/*.rs`), `tempfile` 3.27.0, fake Telegram (`tests/fakes/fake_telegram.py`), local bare git repos |
| Config file | none (`agent-brain/Cargo.toml`); sandbox helper `tests/common/mod.rs` |
| Quick run command | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib journal` |
| Full suite command | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| JRN-01 | Day file content, groups, one line per task per day, status/rounds/roles, attention 4 categories incl. precedence quota>violation>failed, midnight boundary at 00:00 +07 | golden-file (fixture DB, `FixedOffset +07`) | `cargo test ... --test journal_export golden_day` | Wave 0 |
| JRN-01 | `journal_group` valid/absent/invalid -> group / default | unit | `cargo test ... --lib journal::config` | Wave 0 |
| JRN-02 | All 5 file kinds exist, no weekly file, second run = identical bytes and no commit | idempotency (render twice; export twice into bare remote, assert 1 commit) | `cargo test ... --test journal_export idempotent` | Wave 0 |
| JRN-02 | Stale managed files removed only when marker present; human file kept and reported | writer unit/integration | `cargo test ... --test journal_export stale_and_foreign` | Wave 0 |
| JRN-03 | Sanitiser: one line, <=140, no `[[`/`![[`/`#tag`/backtick/HTML/`---`, paths/URLs/keys replaced, truncation after replacement | fuzz-ish corpus unit | `cargo test ... --lib journal::sanitize` | Wave 0 |
| JRN-03 | Planted secret in a task request: nothing in vault, no commit, no push, one `secret` mail with paths only | integration (bare remote + fixture) | `cargo test ... --test journal_export secret_blocks_commit` | Wave 0 |
| JRN-04 | Clone of empty bare remote (branch forced), first push, second export no commit; clone of non-empty remote; push failure keeps local commit, next run publishes; remote moved -> rebase + push; conflict -> abort, blocked, no overwrite | fake-git-remote (local bare repo as `repo_url`) | `cargo test ... --test journal_gitsync` | Wave 0 |
| JRN-04 | Streak counting, alert exactly once after N, recovery mail, persisted across reopen of the DB | unit on `state.rs` | `cargo test ... --lib journal::state` | Wave 0 |
| JRN-04 | Lock prevents concurrent export (CLI vs loop) | integration | `cargo test ... --test journal_gitsync lock` | Wave 0 |
| JRN-05 | `digest_due` with a fake clock: not before 18:00, once after, once per day, late start still sends once; mail claim sends exactly one `sendMessage` to the paired chat, restart does not resend; link percent-encoding | unit (pure) + fake-Telegram serve test | `cargo test ... --lib journal::digest` ; `--test journal_telegram digest_once` | Wave 0 |
| JRN-06 | FTS: migration creates table; `tim`/`dang` match `Tìm`/`đăng`; hostile queries never error; empty query -> usage; stranger gets no reply; results show date+link only (no plan/QA/diff text) | unit + fake-Telegram | `cargo test ... --lib journal::fts` ; `--test journal_telegram tim` | Wave 0 |
| JRN-07 | Config absent = no loop, `journal export` refuses with guidance; ranges rejected; migration 12 on v11 DB keeps data; **v11 DB with pre-existing orphan events still upgrades**; `SCHEMA_VERSION == 12` | unit + `tests/db_migration.rs` | `cargo test ... --test db_migration` ; `--lib config::tests::journal` | Wave 0 (update line 976) |
| JRN-07 | README upgrade section + dry-run script covers schema 12 | manual review + script run on a copy | `bash scripts/phase7-live-check.sh --schema-dry-run` (new, modelled on phase6) | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib journal` (< 30 s once built)
- **Per wave merge:** the targeted `--test journal_*` binaries plus `--test db_migration --test telegram_intake`
- **Phase gate:** full suite green (`-- --test-threads=1`) before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `agent-brain/src/journal/` module skeleton with `#[cfg(test)]` units (config, sanitize, state, digest, fts)
- [ ] `agent-brain/tests/support/journal.rs` fixture builder (temp `brain.db` via `db::open`, explicit epoch-ms runs/tasks/proposals/approvals) and `tests/fixtures/journal/golden/` files
- [ ] `agent-brain/tests/journal_export.rs`, `journal_gitsync.rs` (local bare remote, `git init --bare -b main`), `journal_telegram.rs` (FakeTelegram + `AGENT_BRAIN_JOURNAL_NOW_MS` hook)
- [ ] `db.rs` FK-check fix + test in `tests/db_migration.rs`; update `SCHEMA_VERSION` assertion at db_migration.rs:976 and `setMyCommands` assertion at telegram_intake.rs:1953-1971
- [ ] `scripts/phase7-live-check.sh` (schema dry-run on a copy, as `phase6-live-check.sh --schema-dry-run`)
- Framework install: none

### Manual / live (user)
First real `journal export --no-push` diff review against the demo; a real push to the private repo; digest at the real 18:00 on the paired chat; `/tim` from the phone; Obsidian on Android: HTTPS + PAT clone, pull, open `Today.md`, check embed and links; confirm that tapping a link opens github.com logged in; confirm no write path from the vault back to brain.db (no route exists).

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (no new login) | chat pairing already enforced by `bot::authz`; git auth = user's SSH key, `BatchMode=yes` |
| V3 Session Management | no | - |
| V4 Access Control | yes | `/tim` and digest only to the paired chat; vault repo is private; no inbound route from vault to DB (one-way) |
| V5 Input Validation | **yes** | allow-list sanitiser for every DB string, quoted-token FTS builder, config range checks, project-slug validation, `repo_url` scheme allow-list and `--` before URL |
| V6 Cryptography | no | no hand-rolled crypto; transport by ssh/https via `git` |
| V7 Error handling & logging | yes | Telegram/log messages carry categories and file names, never secret values, raw stderr or full URLs |
| V8 Data protection | yes | allowlist only; D-03 fields never read (`path`, `workdir`, `branch`, `run_dir`, plan/QA/diff, env, keys) |
| V10 Malicious code / supply chain | yes | no new crates; `GIT_ALLOW_PROTOCOL=file:ssh:https` blocks `ext::` remote-command transports |
| V14 Configuration | yes | `StrictHostKeyChecking=yes`, `BatchMode=yes`, fixed author, no signing prompts, `.obsidian/` ignored |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Task text smuggles Obsidian/Markdown syntax (`![[other file]]` embed, `[[x]]`, `#tag`, `%%`, HTML, frontmatter) | Tampering / Info disclosure | One-line allow-list text after a fixed prefix; titles never in YAML; fuzz test |
| Secret or internal path/URL/host in a request reaches GitHub | Information disclosure | Replace patterns, truncate after replacement, final scan on all output, block commit+push on hit, mail with paths only |
| Project slug as path (`../`, absolute) writes outside the vault | Tampering | Slug regex + fallback, path join check (`canonicalize` prefix) in writer |
| Git argument/URL injection (`-oProxyCommand`, `ext::sh -c ...`) | Elevation of privilege | `--` separator, refuse leading `-`, scheme allow-list, `GIT_ALLOW_PROTOCOL` |
| SSH MITM on first connect | Spoofing | `StrictHostKeyChecking=yes` (host key already trusted here, ran) |
| FTS MATCH injection / column filter / huge query | Tampering / DoS | quoted tokens, token and length caps, result cap |
| Overwriting a human edit in the vault or force-rewriting history | Tampering | marker ownership, never force-push/reset, stop on conflict |
| Stranger uses `/tim` | Elevation | existing `authz` drops it; test |
| Phone PAT leaked through a committed `.obsidian/plugins/...` | Information disclosure | `.gitignore` the whole `.obsidian/` |

## Sources

### Primary (HIGH confidence): read or run this session
- Repo: `agent-brain/src/db.rs` (whole), `tasks.rs`, `quota.rs`, `preflight.rs` (head), `gitops.rs` (1-300, 495-515), `secrets.rs`, `inputs.rs`, `routing.rs`, `stats.rs` (210-235), `config.rs` (1-75, 1030-1210, 1455-1530), `projects.rs` (1-300, 540-565, 796-830), `main.rs`, `app.rs`, `lib.rs`, `error.rs`, `dashboard/mod.rs`, `dashboard/pump.rs`, `dashboard/lock.rs`, `dashboard/taskstate.rs` (1-140, 495-545), `orchestrator/mod.rs`, `orchestrator/scheduler.rs`, `autopilot/decide.rs` (1-260, 480-560, 835-850), `autopilot/claim.rs` (469-507), `roles/{lead,dev,qa,mod}.rs` (outcome sites), `telegram/{mod,bot,intake,detect,outbox,config,fixbug_cmd}.rs`, `tests/common/mod.rs`, `tests/support/telegram.rs`, `tests/db_migration.rs`, `tests/telegram_intake.rs`, `spikes/lib/scan-secrets.sh`, `.planning/quick/e2e-findings-fixes.plan.md` §0, `agent-brain/README.md` (upgrade sections).
- Registry sources: `libsqlite3-sys-0.38.2/build.rs`, `sqlite3/sqlite3.c`; `Cargo.lock`.
- Scratch experiments (scratchpad dir): FTS5 tokenizer/query probes (`fts5probe`), migration-in-txn on a copy of the live DB, git probes with local bare remotes (`gitprobe*`), `git ls-remote`/clone of the real vault repo (read-only), read-only queries on a copy of the live `brain.db`.

### Secondary (MEDIUM confidence): official docs read
- https://publish.obsidian.md/git-doc/Getting+Started (mobile limits)
- https://publish.obsidian.md/git-doc/Start+here (commit-and-sync pull/push parts)
- https://raw.githubusercontent.com/Vinzent03/obsidian-git/master/README.md (features, mobile)
- https://obsidian.md/help/links, https://obsidian.md/help/embeds, https://obsidian.md/help/callouts
- https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax (alerts)

### Tertiary (LOW confidence): search results only, marked ASSUMED
- Obsidian forum threads on "Shortest path when possible" and Android file-name restrictions; a review stating obsidian-git "only runs while Obsidian is open".

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH, nothing new, versions read from lockfile and registry source
- Architecture / integration points: HIGH, every seam read with line numbers
- Data derivation (status, attention, rounds): MEDIUM-HIGH, rules verified against code and live data shape, but past-day gate reconstruction is an approximation (A1)
- FTS5: HIGH, run on the exact bundled build
- Git sync: HIGH, run against local bare remotes and read-only against the real remote; real push not tested
- Obsidian/obsidian-git mobile: MEDIUM, docs read, no device
- Pitfalls: HIGH for 1-5, 10-12 (verified); MEDIUM for 8, 14

**Research date:** 2026-11-10
**Valid until:** 2026-12-10 (code facts change with every merged phase; re-check `db.rs` migrations count and the FK state of the live DB immediately before planning; obsidian-git docs are fast-moving, 7-14 days for mobile claims)
