# Phase 6: Multi-engine Teams - Research

> **Amendment note (2026-10-27):** the final section `## Amendment Research: D-30 … D-36` supersedes every DeepSeek-specific statement in this file that assumes a fixed engine set (D-01 / D-08: `deepseek` as the one built-in claude-with-endpoint engine, `engines.deepseek` / `engines.agy` as the only config keys, `Engine::{Deepseek, Agy}`, the "dsh is not used" rows, and the old 06-11 / 06-19 rows of the decomposition table) wherever the two conflict. Everything else (executed plans 06-01 … 06-10, 06-12 … 06-18) still stands as written.

**Researched:** 2026-10-26
**Domain:** Rust CLI orchestrator (agent-brain): per-role engine routing (claude / deepseek-via-claude / agy / human), quota stop, team storage, dashboard Settings, Telegram actions, per-engine stats
**Confidence:** MEDIUM overall. Code seams HIGH (every file below was opened this session). Claude Code / agy / DeepSeek quota shapes MEDIUM-LOW (no real rejected-quota evidence exists in the repo; shapes come from docs and must be calibrated by user-run probes).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Engines**
- **D-01:** DeepSeek runs as `claude` on the host with `ANTHROPIC_BASE_URL` set to DeepSeek's Anthropic-format endpoint and the key in `ANTHROPIC_AUTH_TOKEN`. This reuses the Phase 3 stream-json parser, token counting, `--max-turns`, `--allowedTools` and the S4 kill. The `dsh` Docker container is **not** used (S6: black box, no tokens, kill must happen inside the container).
- **D-02:** The **first plan is a decision gate that tests E-09**. It runs one real tiny `claude -p` call through the DeepSeek endpoint and records whether stream-json, usage, tool use and file edits work. If it fails, **stop the DeepSeek work and ask the user** whether to use dsh as a black box or drop DeepSeek. The agy, team, quota, Settings and human-engine work goes ahead either way. — **Reversibility:** costly — every later DeepSeek plan assumes the stream-json path.
- **D-03:** The DeepSeek key is read **only from an environment variable**. The config holds just the variable's name (for example `DEEPSEEK_API_KEY`). A missing variable gives a clear Vietnamese error before spawning. The key never goes into a committed file, a log, `raw_json`, a run dir, argv shown in the UI, or a Telegram message (ENG-05). Sensitive variables are filtered before anything is written.
- **D-04:** Antigravity is a **real headless engine** (ENG-06, S7): `agy -p … --output-format stream-json --print-timeout <explicit>`, with stdin from `/dev/null`, stdout to a file, and an outer timeout. There is no turn limit. Its events use the `event` key (`init`, `step_update`, `result`), so the parser needs its own branch that reads `result.status`, `result.response` and `result.usage`. The run is killed with the S4 method.
- **D-05:** agy gets `--dangerously-skip-permissions` **only for the Dev role**. agy has no `--allowedTools`, so after every agy Dev run agent-brain checks `git diff`: touching files outside the plan's scope, protocol files, `.planning/` or secret-looking paths is flagged as a protocol violation (the existing `vi phạm giao thức` stop). agy in Lead or reviewer roles runs without skip, so in practice it is read-only. Because tool approvals are soft-denied with exit 0, a "success" with an empty diff on a Dev run is reported as suspicious, not as success.
- **D-06:** Cheap engines (`deepseek`, `agy`) and `người` are allowed for **every role except QA**. QA must use a strong engine (`claude`). Saving a team whose QA is not `claude` is refused by validation, on the Settings screen and in the file loader.
- **D-07:** The `claude` engine **always uses the project's profile** (work = `~/.claude`, personal = `~/.claude-orca`). Teams name engines (`claude`, `deepseek`, `agy`, `người:<id>`), never accounts. A work project can never borrow the personal account's quota, which keeps the SPK-10 boundary. The source spec's `claude-work` / `claude-personal` engines and its `~/.claude-work` / `~/.claude-max` paths are superseded.
- **D-08:** Engine definitions are **fixed**, built into the binary plus `config.json` (command path, endpoint, key variable name, model per engine). The UI shows each engine's readiness (`sẵn sàng`, `thiếu key`, `thiếu lệnh`, `chưa kiểm`) but **cannot edit engines**. No UI route writes a command or endpoint.

**Teams and the Settings screen**
- **D-09:** Teams live in `~/.config/agent-brain/teams.json`. It can be hand-edited like `workflows.json`, and has a size cap and line/column error messages in the same style. Built-in default teams ship in the binary, at least `mac-dinh` (all `claude`) and `tiet-kiem` (Lead/QA `claude`, Dev `deepseek`). A team has a name (slug), a purpose/description in Vietnamese, and an engine for each role: `lead`, `dev`, `qa`, `plan-review`, `code-review`. No brain.db table is added for teams.
- **D-10:** A new dashboard screen **Cài đặt** (`#/settings`) creates, edits and deletes teams **and workflows** (the user chose to include workflows). Writes are atomic: temp file, fsync, rename. They are validated with the same rules as the file loader, and stay behind the token and header middleware like every POST. Built-in teams and workflows are read-only, but can be copied ("Nhân bản").
- **D-11:** A team **and** a workflow cannot share a name, or the UI must always label which is which. The source's `mac-dinh` is already a workflow name, so the planner picks the rule (for example, separate namespaces with explicit labels) and records it.
- **D-12:** How a task gets a team: the project's default `team` comes from `projects_map.json` (missing means `mac-dinh`). The Giao việc screen and Telegram `/task` get a team picker that overrides it for that task, and the CLI gets `--team`. The chosen team is stored durably with the task, so restarts and autopilot keep it.
- **D-13:** The team of a task that is **not running** can be changed on the task detail screen. The change applies from the next step on, and a history line is recorded (who, when, from → to). While a run is live, changing is refused.
- **D-14:** Every run records its real engine in `runs.engine` (which today is always `claude`) and the team name. The log header line `[agent-brain] … · profile … · model …` gains `engine …` (ENG-01).

**Quota and rate limits**
- **D-15:** A quota or rate-limit error is detected from the stream (error result or subtype) or the exit code, for each engine. The run gets outcome `quota`, and the autopilot stops with a new reason word, for example `hết quota` ("Hết quota engine X"). The task **waits for the user**.
- **D-16:** **Never auto-switch.** There are no configurable fallback rules in this phase (deferred). The "Bạn" gate card shows `Chạy lại bước này bằng đội hình…` (a team picker) and `Thử lại sau` (same engine, when the user chooses).
- **D-17:** Notifications go to the dashboard **and Telegram**. Following the 5.3 pattern, the Telegram message carries buttons to pick a team and re-run the step, through the same shared action function as the dashboard (`run_task_action` style). The Telegram trust rules from 5.3 and 5.4 still apply. Tasks created from Telegram still get their plan approved only on the dashboard. Re-running an implement or QA step is not plan approval.
- **D-18:** When a step is re-run on a new engine, the new engine continues **on the current worktree**: the partial diff is kept, the prompt says "continue from `.plan.md`, look at the existing git diff", and `.handover.md` is included if it exists. The session is never resumed across engines (ENG-03).

**Human engine (`người`)**
- **D-19:** Staff are declared on the Settings screen: id, display name, and optional notes/contact. A team can set any role except QA to `người:<id>`. At that step agent-brain prepares the task branch with the committed `.plan.md` and shows it as "Chờ <tên> làm". It does not spawn anything.
- **D-20:** Staff work on their own machine and **push the task branch to the remote**. Staff have **no dashboard or Telegram access**; the user presses `Xong` on the task screen for them. On `Xong`, agent-brain fetches the branch into the task's worktree and continues the workflow. The **existing workflow** decides who approves: code-review (AI) on the git diff, then QA (strong engine) and/or the "Bạn" gate for a real test. A failure goes back to the same person with the report (`Trả lại`).
- **D-21:** A human run is recorded as a run with engine `người` and no token usage, so statistics and the "Cần sửa" rate (ENG-07) cover it too.

**Statistics**
- **D-23:** The Thống kê screen splits tokens and cost by engine (ENG-04) and shows QA's "Cần sửa" rate by Dev engine (ENG-07). It reuses the existing stats queries, grouped by `runs.engine`. agy usage comes from `result.usage`. Price entries per engine go in `config.json` `pricing`.

**Scope**
- **D-22:** **Media moves to inserted phase 06.1 (Media Branch).** The user's direction for 06.1 is: a generic render job (approved script in the media project → agent writes the render config/script → runner runs FFmpeg → output file); output goes under `<media project>/output/`, media projects are plain folders so no worktree is created; the `media_jobs` table and render kill with no leftovers (S4 method); the whiteboard pipeline does not exist yet, connecting it comes later.

### Claude's Discretion
- Exact JSON shapes of `teams.json`, the engine entries in `config.json`, and the staff list. The planner can keep staff inside `teams.json` or put them in `staff.json`.
- How a task stores its team (a column on `tasks`, which needs a schema v4 migration with backup and a decision gate as in 05.4, versus a run-dir/plan-header field). Pick the durable, smallest option and flag any migration as a decision checkpoint.
- The quota detection patterns for each engine. Research the real stream and exit shapes, and use fixtures.
- The Settings screen layout. It follows the 05-UI-SPEC rules (Tailwind build, Alpine CSP, offline, Vietnamese, light/dark, ui-lint), and a UI-SPEC via `/gsd-ui-phase 6` is recommended before planning. (UI-SPEC now exists and is approved: `06-UI-SPEC.md`.)

### Deferred Ideas (OUT OF SCOPE)
- Configurable auto-fallback rules (for example "Dev out of quota on claude → deepseek"), recorded and notified. Rejected for now: always ask (D-16).
- The dsh container as a black-box engine, only if E-09 fails and the user chooses it (D-02).
- Staff access through Telegram (multi-user bot), or any staff access to the dashboard. Rejected: the user presses `Xong` for them (D-20).
- Editing engine definitions (command, endpoint, key name) on the UI (D-08).
- **Phase 06.1 Media Branch:** MEDIA-01 … 03 with the direction in D-22. Connecting the real whiteboard pipeline comes after that.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ENG-01 | `tiet-kiem` task: Lead/QA claude, Dev DeepSeek, log shows the right engine per run (phase gate) | Seam map rows S1-S6 (engine resolution at `cmd_*` + `execute_role_staged`, `insert_run` hard-codes `'claude'`), deepseek env builder (Engine Facts), E-09 gate plan, log header change |
| ENG-02 | Simulated quota mid-task: error reported, team change proposed, no silent switch | Quota detector spec (claude/deepseek/agy), single seam in `execute_role_staged` returning typed `EXIT_QUOTA`, `StopReason::HetQuota`, fixtures runnable on `tests/fakes/fake_claude.py` |
| ENG-03 | Engine change mid-task continues from `.plan.md` (+ `.handover.md`) | Pitfall 1 (`cmd_dev` refuses a dirty tree), continuation mode, allowed-tools / read-allow additions, rounds must not count quota runs |
| ENG-04 | Stats split tokens per engine | `stats.rs` queries are keyed on `runs.*` columns; add `GROUP BY runs.engine`; pricing must be engine-first (Pitfall 6) |
| ENG-05 | DeepSeek key never in committed files or logs | Redactor at the runner (before the stream file write), env stripping for non-deepseek engines, post-run secret scan before commit, planted-key tests |
| ENG-06 | Antigravity integrated per S7 | agy event schema, `Summary` agy branch, runner stdout-to-file mode, Dev skip-permissions + diff scope + empty-diff check, fake agy |
| ENG-07 | QA "Cần sửa" rate recorded per engine; QA runs on a strong engine | Stats SQL (QA verdict joined to the latest earlier Dev run's engine), QA-must-be-`claude` validator shared by loader and Settings |
| TEAM-01 (proposed) | `teams.json` loader/validator/built-ins (size cap, line/column errors, QA rule, separate namespace from workflows) | Mirror of `workflows.rs` (`MAX_FILE_BYTES`, `key_lines`, `validate_entry`) |
| TEAM-02 (proposed) | Task stores its team + change history durably; change refused while a run is live | Schema v4 `tasks.team_json`, CAS update, `task_rev` + Telegram signature inclusion |
| TEAM-03 (proposed) | Team selection: project `team` key, Giao việc, Telegram `/task`, CLI `--team` | `new_task_args`, `start_new_task`, `cmd_lead`, `cmd_auto --start`, `workflow_of` twin `team_of` |
| SET-01 (proposed) | Settings screen shell + read-only Engine readiness (local checks only) | UI-SPEC section "Cài đặt", engine readiness rules, route/asset registration |
| SET-02 (proposed) | Team create/edit/delete/copy writes (atomic, version token, conflict, refuses invalid file) | Settings write pattern (atomic helper in `projects.rs`) |
| SET-03 (proposed) | Workflow create/edit/delete/copy via Settings | `workflows::validate_entry` must become `pub`; reverse mapping `Workflow` -> file entry |
| SET-04 (proposed) | Staff create/edit/delete (`staff.json`) with usage checks | Separate `staff.json`, same loader/atomic infrastructure |
| SET-05 (proposed) | UI gates for the new screen (ui-lint, css build, assets table, js-check, smoke) | `scripts/dashboard-ui-lint.sh`, `build-css.sh --check`, `assets.rs` table |
| HUM-01 (proposed) | Human step: prepare branch, show `Chờ <tên> làm`, spawn nothing | Marker in `team_json` (no run row), exit code `EXIT_WAITING`, autopilot state `waiting_human` |
| HUM-02 (proposed) | `Xong` fetches branch ff-only and continues; `Trả lại` | `gitops` additions, `ControlOutcome` flow, autopilot resume via `auto {id}` |
| HUM-03 (proposed) | Human run recorded (engine `người`, no tokens) and counted in stats / Cần sửa rate | `insert_human_run`, `usage_source = 'none'` |
| QUOTA-01 (proposed) | Re-run a quota-stopped step with another team from dashboard and Telegram through one shared action | `run_task_action` family, Telegram callback grammar extension |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

Treated with the authority of locked decisions. [VERIFIED: /mnt/data/IDE/QuangVy-Orchestration/CLAUDE.md]

- Claude analyses and plans, Antigravity (agy) executes, Claude reviews. Claude does not write the implementation. Plans must be executable by another agent: exact files, steps, acceptance criteria, verify commands.
- Call agy only through the `agy` CLI: `timeout 45m agy -p "<prompt>" --output-format stream-json --print-timeout 40m </dev/null > "$LOG" 2> "${LOG%.jsonl}.stderr.txt"`. Never the bare `antigravity` binary. Always stdin from `/dev/null`, stdout to a file (agy has pipe-output bugs), explicit `--print-timeout`, outer `timeout`. agy has no turn-limit flag.
- Tool permissions are unverified: after every run check the diff for real changes before trusting `status: SUCCESS`. `--dangerously-skip-permissions` only with the user's approval.
- Kill method = S4 (record descendants, SIGTERM the recorded PID, sweep recorded descendants). Never `pkill` / `killall` / kill by name.
- The prompt handed to agy must carry: absolute plan path, repo root + branch, commit-per-task rule (no `git add -A`, no `--no-verify`), what not to touch (`.planning/STATE.md`, `ROADMAP.md`, untracked `.planning/` files, `~/.claude-orca`), verify commands, summary contents.
- Review checklist: diff inside declared files, re-run verify commands yourself, acceptance criteria met, no secrets (`spikes/lib/scan-secrets.sh`), record `SUMMARY.md` / `VERIFICATION.md`.
- Project-level hard rules relevant here (PROJECT.md Constraints): single binary, offline dashboard, 127.0.0.1 + token, process-group kill, tolerant stream parser that always stores `raw_json`, roles exchange only through files, secrets from env only and filtered before logging. [VERIFIED: .planning/PROJECT.md:86-97]

## Summary

Phase 6 is mostly plumbing around five existing seams, not new subsystems. The engine decision belongs in one place: each `cmd_{lead,dev,qa,review}` already builds an argv with `build_command*` and hands it to `execute_role_staged`, which builds the env, inserts the run row (with `'claude'` hard-coded), supervises the process and finishes the row. If that function takes an engine plan (binary, argv, env additions, redaction secrets, stream format, db engine name) the rest of the phase hangs off it: the log header, `runs.engine`, secret hygiene, quota detection (one typed error after the run row is finished) and per-engine stats. Roles never see a team; the child process resolves the task's team from brain.db (autopilot and dashboard jobs already launch `agent-brain run <role>` children without engine arguments).

The riskiest items are all "unknown shape" items, so the plan order must put them first as user-run probes with scrubbed fixtures: (1) E-09, a tiny real `claude -p` through the DeepSeek Anthropic endpoint (D-02); (2) an agy 1.4.7 probe (the S7 evidence is for 1.3.5; flags are unchanged but event shapes for tool steps, errors and quota were never captured); (3) a real error-result capture using a deliberately invalid DeepSeek token (no billing, yields an authentic Anthropic-format-endpoint error stream for fixtures). No real rejected-quota stream exists anywhere in the repo; the only real rate-limit line is an `allowed_warning` event (spikes/s1). Quota detection therefore must be multi-signal and tested on documented/assumed shapes, with the raw line stored so the first real quota stop recalibrates it.

Three design choices need user-visible flags: (a) a human step is stored as a marker in a new `tasks.team_json` column and a run row is inserted only on `Xong` (a `running` row without a pid would be marked `lost` by `reconcile_stale_runs`, which runs on every `Ctx` open); (b) quota runs must not consume autopilot rounds; (c) the human engine for Lead and the two reviewers has artifact contracts that are materially larger than Dev (see Open Questions). The schema v4 migration is a single `ALTER TABLE tasks ADD COLUMN`, one-way, and lands on a live DB that is still at schema 2 (5.4's v3 is not deployed yet), so the README backup note must cover the 2 -> 4 jump.

**Primary recommendation:** Build one `EnginePlan` seam in `execute_role_staged` (claude first, then deepseek/agy/human variants), add `tasks.team_json` (schema v4, gated by a blocking-human decision as in 05.4-02), detect quota once after the run and surface it as a typed `EXIT_QUOTA` error plus run outcome `quota`, and run the E-09 and agy probes as the first, user-run plans.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Engine selection per role | API / Backend (Rust `run <role>` child) | Database (task team) | Children already resolve the task from the checked-out branch (`tasks::current_task`); no argv carries the team |
| Team definitions (`teams.json`, `staff.json`) | Filesystem config (`~/.config/agent-brain/`) | API / Backend (loader, validator) | D-09: hand-editable file, no table |
| Task team + change history | Database (`tasks.team_json`) | API / Backend (CAS update) | D-12/D-13 durable across restarts/autopilot |
| Secret hygiene (key env, redaction) | API / Backend (runner + env builder) | Filesystem (post-run commit scan) | Redaction must run before the stream file / events / term / job output sinks |
| Quota detection | API / Backend (`stream.rs` summary + `execute_role_staged`) | Database (`runs.outcome = 'quota'`) | One seam shared by all five roles and three process engines |
| Quota/human UI state | API / Backend (task-detail payload) | Browser (banner/callout) | UI-SPEC: banner and callout depend only on task-detail payload |
| Settings writes | API / Backend (atomic file writes behind token+Host middleware) | Browser (Alpine editors) | D-10; server validation authoritative, client mirrors cheap rules |
| Human step wait / `Xong` fetch | API / Backend (git via `gitops`) | Browser (callout buttons), Telegram (none, D-20) | Fetch is a server-side git operation on the project root |
| Stats per engine | Database (SQL group by `runs.engine`) | API / Backend (pricing) | D-23 reuses stats queries |
| Telegram quota/team actions | API / Backend (bot, shared action fn) | Telegram (callbacks) | 05.3 pattern: same function as the dashboard |
| Engine readiness | API / Backend (local checks: PATH lookup, env presence) | Browser (read-only list) | UI-SPEC C8: no model call, no tokens |

## Standard Stack

### Core (no new crates: the build is `--offline --locked`)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde_json (feature `preserve_order`) | =1.0.154 | teams/staff/workflow file read-modify-write keeps key order | already declared; `preserve_order` makes Settings writes minimally invasive [VERIFIED: agent-brain/Cargo.toml:11 `serde_json = { version = "=1.0.154", features = ["preserve_order"] }`] |
| rusqlite (bundled) | =0.40.2 | schema v4 (`ALTER TABLE ... ADD COLUMN`) | existing migration vector `MIGRATIONS` [VERIFIED: Cargo.toml:12] |
| tokio | =1.53.3 | runner supervision (stdout-to-file tail uses `tokio::time` polling) | existing [VERIFIED: Cargo.toml:13] |
| axum | =0.8.9 | new `/api/teams`, `/api/staff`, settings write routes behind existing `auth::gate` layer | existing [VERIFIED: Cargo.toml:18; agent-brain/src/dashboard/mod.rs router layers] |
| regex | =1.13.1 | quota text patterns | existing [VERIFIED: Cargo.toml:16] |

### Supporting (external tools probed on this machine)

| Tool | Version found | Purpose | Note |
|------|---------------|---------|------|
| claude (Claude Code) | 2.1.310 | claude + deepseek engines | spikes ran 2.1.300; re-verify stream shapes [VERIFIED: `claude --version`] |
| agy | 1.4.7 | agy engine | S7 evidence is 1.3.5; `agy --help` flag list is identical (no `--max-turns`) [VERIFIED: `agy --version`, `agy --help` this session] |
| git | 2.43.0 | fetch / ff-only merge for `Xong` | [VERIFIED: `git --version`] |
| node | v24.21.0 | `scripts/dashboard-js-check.mjs` | [VERIFIED] |
| python3 | 3.12.3 | `tests/fakes/fake_claude.py`, ui-lint | [VERIFIED] |
| Chrome | 154.0.8037.92 | `scripts/dashboard-smoke.sh` | [VERIFIED] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `tasks.team_json` column | store team in `auto_json` | no migration, but `auto_json` is rewritten wholesale by the driver (`set_auto_json`), parse failure silently defaults, and manual tasks have no autopilot semantics; rejected |
| `tasks.team_json` column | team in `.plan.md` `brain-meta` | meta is compared for equality and rewritten by Lead (`curr_meta != meta`), editable by agents; rejected |
| marker + run-at-`Xong` for human steps | `running` run row with NULL pid | `reconcile_stale_runs` (called in `open_ctx_with`) marks pid-less running rows `lost`; `cmd_kill`, busy rules and `execute_role_staged` all treat `running` as a live process; rejected |
| separate `staff.json` | staff inside `teams.json` | UI-SPEC already names a separate "file nhân sự"; keeps teams.json hand-editing simple; recommended |
| `--dangerously-skip-permissions` for agy Dev (D-05, locked) | `--mode accept-edits` / `--sandbox` | narrower in principle; unverified; evaluate in the agy probe and raise with the user (Open Question 3) |

**Installation:** none. `cargo build --manifest-path agent-brain/Cargo.toml --offline --locked`.

## Package Legitimacy Audit

No external packages are installed in this phase (Rust dependencies are pinned and built offline; the Settings screen adds no JS dependency, `Tool: none` in UI-SPEC). Audit table: not applicable.

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
 user (dashboard / Telegram / CLI)
        |  create task (team?) / change team / quota re-run / Xong
        v
 +---------------------------+      teams.json, staff.json, workflows.json
 | agent-brain serve         |<---- (~/.config/agent-brain, atomic writes, version token)
 |  api_tasks / api_settings |
 |  telegram bot (shared fn) |
 +------------+--------------+
              | JobManager spawns `agent-brain run <role> [--team T] -- project ...`
              |  (autopilot driver spawns the same children, stage_args())
              v
 +---------------------------+     brain.db (schema v4)
 | agent-brain run <role>    |<--> tasks.team_json  (name, history[], human marker)
 |  cmd_lead/dev/qa/review   |     runs.engine, runs.outcome='quota', metrics_json{team,staff,...}
 |   1. resolve team(task)   |
 |   2. engine = team[role]  |
 |   3. EnginePlan{argv,env, |
 |      redact[],format}     |
 +------------+--------------+
              |
   execute_role_staged (single seam)
              |-- engine claude   -> claude_bin (project profile env)      stream-json  -> Summary(Claude)
              |-- engine deepseek -> claude_bin + ANTHROPIC_BASE_URL/AUTH  stream-json  -> Summary(Claude)
              |-- engine agy      -> agy -p ... (stdout -> file, tail)     NDJSON event -> Summary(Agy)
              `-- engine người    -> no process: marker + exit EXIT_WAITING; run row inserted at `Xong`
              |
   redact(secret values) -> stream.jsonl / events.raw_json / term / job output ring
              |
   finish_run -> quota detector -> outcome 'quota' -> Err(EXIT_QUOTA)  (no auto-switch)
              |
   autopilot: decide() D2 -> StopReason::HetQuota ; dashboard/Telegram: quota banner + team picker
```

### Recommended Project Structure

```
agent-brain/src/
├── engine.rs            # Engine enum, EnginePlan, resolve(team, role), readiness(), env builders, cost_for()
├── teams.rs             # Team, LoadedTeams, load()/load_from(), validate_entry(), summary(), builtins
├── staff.rs             # Staff, LoadedStaff, load(), validate
├── secrets.rs           # Redactor (exact-value + known patterns), scan_paths() for post-run commit scan
├── settings_store.rs    # version token, atomic write (reuse projects.rs pattern), global write mutex
├── quota.rs             # QuotaHit, claude-format and agy-format detectors (pure, fixture-tested)
├── human.rs             # marker, prepare(), done() (fetch ff-only + insert_human_run), return()
├── dashboard/api_settings.rs  # /api/teams /api/staff /api/engines + write routes (workflows write too)
assets/
├── teams.default.json   # mac-dinh, tiet-kiem (QA claude)
├── dashboard/screens/settings.html, js/settings.js
tests/
├── engine_routing.rs, engine_quota.rs, engine_secrets.rs, engine_agy.rs, human_engine.rs,
│   dashboard_settings.rs, task_team.rs      # new
└── fakes/fake_agy.py                          # new; fake_claude.py recorder extended
```

### Pattern 1: Engine decision at one seam
**What:** `execute_role_staged` receives `RunPlan { engine: EnginePlan, ... }` instead of a bare `argv`. `EnginePlan` carries binary, argv, env overrides/removals, secret values to redact, stream format (`Claude` | `Agy`), db engine name, model label and a `pre_spawn_error` (missing key / missing command). Each `cmd_*` builds `EnginePlan` via `engine::plan_for(ctx, task, role, spec)`; `build_command*` stays the claude-family argv builder.
**When to use:** every role run. Claude engine output must be byte-identical to today's (regression gate: existing suites).
**Evidence of the seam:** `execute_role_staged` builds env with `crate::profile::build_env(&project.profile, &ctx.cfg.profiles, &crate::profile::current_env())?` and emits `"[agent-brain] {} · {} · profile {} ({}) · model {}"` [VERIFIED: agent-brain/src/roles/mod.rs:427-449]; `insert_run` hard-codes the engine: `VALUES (?1, ?2, ?3, 'claude', ?4, ?5, ?6, ?7, ?8, 'running', ?9, ?10, ?11)` [VERIFIED: agent-brain/src/db.rs:326].

### Pattern 2: One quota seam, typed error after the run row is finished
**What:** after `finish_run`, `execute_role_staged` runs `quota::detect(engine_format, &summary, &outcome, stderr_tail)`. On a hit and status not `Killed`/`TimedOut`: `db::set_run_outcome(run_id, "quota")`, merge `metrics_json.quota = {engine, signal, reset_at?}`, store a redacted one-line detail in `runs.error`, and return `Err(BrainError::new(msg, EXIT_QUOTA))`. Every `cmd_*` already propagates with `?` (`let role_run = crate::roles::execute_role_staged(...)?;` in lead.rs:377, dev.rs:378, review.rs:314; qa.rs:191 same shape), so no per-role code is needed. [VERIFIED: grep of roles/*.rs this session]
**Exit code:** add `pub const EXIT_QUOTA: i32 = 5;` next to the existing constants `EXIT_OK = 0, EXIT_ERROR = 1, EXIT_ROUTING = 2, EXIT_PROTOCOL = 3, EXIT_BLOCKED = 4, EXIT_INTERRUPTED = 130` [VERIFIED: agent-brain/src/error.rs:1-6]. Autopilot's `child_error_stop` ignores `[EXIT_OK, EXIT_PROTOCOL, EXIT_BLOCKED, EXIT_INTERRUPTED]` and otherwise only acts when the latest run `succeeded` with no outcome [VERIFIED: agent-brain/src/autopilot/mod.rs:340-345], so a `failed` quota run falls through to `decide()` D2, which must test `outcome == "quota"` BEFORE the generic `failed` branch (`if last_run.status == "failed" || ...` at decide.rs:826).

### Pattern 3: Task team in `tasks.team_json` (schema v4)
**What:** one nullable TEXT column holding `{"name": "...", "history": [{"at": ms, "by": "dashboard"|"telegram"|"cli", "from": "...", "to": "..."}], "wait": {...}|null}`. NULL = legacy task = all-claude pseudo-team (UI-SPEC copy "task tạo trước Phase 6 — mọi vai trò chạy bằng claude"). Name reference, not a role snapshot: UI-SPEC defines a "team no longer in teams.json" state, which only makes sense for a name reference. Writes use compare-and-swap (`UPDATE tasks SET team_json = ?new, updated_at = ?now WHERE id = ?id AND team_json IS ?old`) so concurrent dashboard/Telegram/CLI changes cannot interleave history.
**Migration:** append `"ALTER TABLE tasks ADD COLUMN team_json TEXT;"` as `MIGRATIONS[3]` and set `SCHEMA_VERSION = 4`. Existing vector end: `"ALTER TABLE tasks ADD COLUMN origin TEXT;",` and `pub const SCHEMA_VERSION: i64 = 3;` [VERIFIED: agent-brain/src/db.rs:58,61]. `open()` refuses a DB newer than the binary, so rollback = restore the pre-upgrade backup (same as 05.4: README note + `brain.db.bak-pre-<phase>-<date>`).
**Gate:** first task of the schema plan is `checkpoint:decision` (blocking-human), copying 05.4-02 Task 1 (one-way, installed binary then refuses the DB). [CITED: .planning/phases/05.4-telegram-task-intake/05.4-02-PLAN.md:42,123-152]
**Side effects to handle:** include `team_json` in `task_rev` (api_tasks.rs `task_rev` hashes status, latest run, auto state/stop, approved commit, plan commit) and bump `tasks.updated_at` on every team/marker write, because the Telegram detector signature includes `t.updated_at` and `auto_json` [VERIFIED: agent-brain/src/telegram/detect.rs `signatures()`; agent-brain/src/dashboard/api_tasks.rs:955-967].

### Pattern 4: Human step = marker, not a run row
**What:** at a human step the child process (`run dev` etc.) validates preconditions, writes `team_json.wait = {role, stage, round, staff, since_ms, branch, mode:"waiting"|"rejected", report_paths[]}`, optionally pushes the branch (Open Question 2), prints `Chờ <tên> làm`, and exits with a new `EXIT_WAITING = 6`. The autopilot driver treats that exit code like the gate: `claim::release(.., "waiting_human", &aj)` and return OK (add `Some("waiting_human")` to `taskstate::auto_state_of`, which today maps only running/interrupted/gate/stopped/done [VERIFIED: agent-brain/src/dashboard/taskstate.rs:306-333]). `claim()` has no precondition on the previous `auto_state` (except `more_round`), so `auto {id}` after `Xong` just works [VERIFIED: agent-brain/src/autopilot/claim.rs:150-225]. On `Xong`: validate on the right branch + clean tree, `git fetch origin <branch>`, `git merge --ff-only FETCH_HEAD`, run the same post-run checks the AI role uses (Dev: diff scope check as in dev.rs:444-453), insert the human run, clear the marker, and if `autopilot_continues(...)` start `auto {id}`.
**Why:** `runs.status` is CHECK-constrained to `'running','succeeded','failed','timed_out','killed','lost'` [VERIFIED: agent-brain/src/db.rs:34], and `reconcile_stale_runs` marks any running row without a live pid as `lost` and is called on every `open_ctx_with` [VERIFIED: agent-brain/src/runner.rs:32-53; agent-brain/src/app.rs:24].
**Human run row (D-21):** `engine = 'người'`, `model = NULL`, `pid/pgid/pid_start = NULL`, `status = 'succeeded'`, `usage_source = 'none'`, tokens 0, `cost_usd = 0`, `started_at = marker.since_ms`, `ended_at = now`, `metrics_json = {"team": ..., "staff": "<id>", "human": {"waited_ms":..., "branch":..., "from":..., "to":...}}`, `run_dir = NULL`. The pipeline view emits `engine: "người:<id>"` composed from `metrics_json.staff` (UI-SPEC AB.engineText form) while stats group by the single value `'người'`.

### Pattern 5: Loader mirrors `workflows.rs`
`teams.rs` copies the structure of workflows: built-in JSON compiled in (`include_str!("../assets/teams.default.json")`), `MAX_FILE_BYTES` 256 KiB cap, `Dòng {line}, cột {col}: ...` serde errors, per-entry `validate_entry` using `key_lines` for line numbers, source tags `builtin` / `file` / `override`, valid entries load even when others fail and `errors` lists the rest [VERIFIED: agent-brain/src/workflows.rs:17,781-836]. Name rule: reuse `workflows::validate_name` for teams **but note it caps names at 40 bytes** (`if bytes.is_empty() || bytes.len() > 40`) [VERIFIED: agent-brain/src/workflows.rs:140-143] while UI-SPEC says `maxlength="48"`. Pick one cap (recommend 40 everywhere; adjust UI-SPEC C4 `maxlength` to 40).
Team roles keys in the file: `lead`, `dev`, `qa`, `plan-review`, `code-review` (hyphenated, equal to `Role::as_str`); the API payload may use underscores per UI-SPEC. Engine values: `claude`, `deepseek`, `agy`, `người:<staff-id>`. Validator rules: QA must equal `claude` (D-06); `người:<id>` with unknown staff id is a **warning** (UI-SPEC amber line), not an error; a role missing from an entry defaults to `claude`.

### Pattern 6: Settings writes (atomic + version token + conflict + invalid-file refusal)
- Atomic write: copy the `projects.rs` helper (temp file in the same dir with `create_new(true)` mode `0o600`, write, flush, `sync_all`, `rename`, remove temp on failure) [VERIFIED: agent-brain/src/projects.rs:340-395]; add a directory `fsync` after the rename (D-10 says temp, fsync, rename).
- Version token: hash of the raw file bytes (or the constant `"absent"`). `std::hash::DefaultHasher` is acceptable because tokens are compared only inside one `serve` process (same approach as `task_rev`). Reads return `version`; writes take `{entry, version}` and compare under a process-wide `Mutex` immediately before the rename; mismatch -> 409 `{code:"conflict"}`.
- Refuse when the loaded file has `errors` (UI-SPEC: "dashboard không ghi đè file lỗi"): return 409 with the load-error text.
- Never rewrite entries the user did not touch: parse the file as `serde_json::Value` (order preserved), change one key, write back.
- Routes live in `api_settings.rs` merged in `dashboard::router`; they inherit the existing `auth::gate` layer and `DefaultBodyLimit::max(65536)` [VERIFIED: agent-brain/src/dashboard/mod.rs `router()`]. Use POST for save/delete (UI-SPEC data contract; `AB.api` DELETE has no body).

### Pattern 7: Redaction before every sink
The runner writes each stdout line to `stream.jsonl` first and only then calls `obs.on_line(&line)` (which inserts `events.raw_json`, prints to terminal and feeds the job output ring) [VERIFIED: agent-brain/src/runner.rs:322-329, 402-409]. Therefore redaction goes in the runner: `RunRequest { redact: Vec<String>, ... }`, applied to every line before the file write, and to `stderr.txt` after the process exits (the child writes stderr straight into the file: `cmd.stderr(stderr_file)`, runner.rs:233). `RunRequest` env is built explicitly (`cmd.env_clear(); cmd.envs(&req.env);` runner.rs:229-230) so the child sees only what `EnginePlan` provides.

### Pattern 8: Engine readiness is local-only
`engine::readiness(cfg, env)` -> `{name, state: san_sang|thieu_key|thieu_lenh|chua_kiem, reason, command, key_env, model}`: command found on `PATH` or an absolute executable; key env var present and non-empty (value never read into the payload). `người`: staff count. No model call, no tokens (UI-SPEC C8). Readiness reflects the **serve** process env, which is what dashboard-launched jobs inherit (`JobManager` spawns children without `env_clear`: jobs.rs `Command::new(&exe_clone)` with only `args`, `stdin`, `stdout`, `stderr`, `process_group`) [VERIFIED: agent-brain/src/dashboard/jobs.rs:62-99].

### Pattern 9: Stats per engine
Existing queries are plain `SELECT ... FROM runs WHERE (?1 IS NULL OR runs.profile = ?1) AND (?2 IS NULL OR runs.started_at >= ?2) GROUP BY ...` [VERIFIED: agent-brain/src/stats.rs query_report]. Add `by_engine` with `GROUP BY runs.engine` and a cost that sums stored `cost_usd`; add the Cần-sửa rate:

```sql
-- QA verdict joined to the engine of the latest Dev run inserted before that QA run
SELECT d.engine,
       SUM(CASE WHEN q.outcome = 'qa:fix' THEN 1 ELSE 0 END) AS qa_fix,
       COUNT(*) AS qa_total
FROM runs q
JOIN runs d ON d.id = (SELECT d2.id FROM runs d2
                       WHERE d2.task_id = q.task_id AND d2.role = 'dev' AND d2.id < q.id
                       ORDER BY d2.id DESC LIMIT 1)
WHERE q.role = 'qa' AND q.outcome IN ('qa:pass','qa:fix')
  AND (?1 IS NULL OR q.profile = ?1) AND (?2 IS NULL OR q.started_at >= ?2)
GROUP BY d.engine;
```
(outcomes `qa:pass` / `qa:fix` are set in `roles/qa.rs` [VERIFIED: agent-brain/src/roles/qa.rs:281,286]). Ordering by `id` (monotonic insertion) is robust to human runs whose `started_at` is the wait start.
Cost: `SUM(runs.cost_usd)` is correct only if deepseek/agy runs store a computed cost at `finish_run` (Pitfall 7) and "chưa có giá" is derived from the absence of a pricing entry, not from `cost = 0`.

### Anti-Patterns to Avoid
- **Passing the team on every child argv for mid-task steps.** Children resolve the team from the task row; only `run lead`/`auto --start` (task creation) take `--team`.
- **Auto-switching or retrying on another engine after quota.** Out of scope (D-16). Exit and wait.
- **Reading the key into `Config`.** `Config` holds only the variable name; the value is read at spawn time from the process env and handed to the redactor.
- **A `running` run row for a human wait.** See Pattern 4.
- **Branching on exit code alone for quota.** `claude -p` returns the failure as a `result` on stdout and a non-zero exit; agy exits 0 on soft-deny and non-zero on failure; a transient 429 can be retried and then succeed. Decide from the final `result` plus signals.

## Seam Map (answers research question 1)

| # | Seam | Where | Change |
|---|------|-------|--------|
| S1 | argv + binary per role | `roles/mod.rs` `build_command`, `build_command_with_prompt` (builds `cfg.claude_bin -p <input> --output-format stream-json --verbose --model <m> --append-system-prompt <prompt> ...`) lines 131-178 | keep as the claude-family builder; add `build_agy_command(spec, cfg, prompt)` (single `-p` argument = role prompt + `\n\n---\n` + user input, since agy has no system-prompt flag) |
| S2 | forbidden permission flags | `roles/mod.rs:94-107` `check_forbidden_args` refuses `--dangerously-skip-permissions` in `common_args` / `extra_args` | agy Dev flag is added by the agy builder only, with a unit test that non-Dev agy argv never contains it |
| S3 | env | `profile::build_env` (removes `NESTED_SESSION_VARS`, applies profile `unset_env`, sets `CLAUDE_CONFIG_DIR`, sets `CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR=1`) [VERIFIED: profile.rs:67-105]; default profiles `unset_env` = `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL` [VERIFIED: assets/config.default.json:50-54] | new `engine::env_for(engine, base)`: claude -> base + strip `ANTHROPIC_MODEL`, `ANTHROPIC_DEFAULT_{OPUS,SONNET,HAIKU}_MODEL`, `CLAUDE_CODE_SUBAGENT_MODEL`, the deepseek key var; deepseek -> `build_env` (same profile dir) then insert `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN` AFTER the profile unset step, `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, remove the original key var; agy -> base minus claude/ANTHROPIC vars and the deepseek key var |
| S4 | run insert | `db::insert_run` hard-codes `'claude'` (db.rs:326); `NewRun` has no engine | add `engine: String` to `NewRun`; callers: `execute_role_staged` Observer (roles/mod.rs:479) and db tests |
| S5 | stream parse | `stream::classify` -> kinds `system/*`, `assistant`, `user`, `result`, `rate_limit_event`, else `unknown:*` (stream.rs:146-163); `Summary::feed` | `classify`: lines with an `event` key and no `type` -> `agy/<event>`; `Summary` gets a format flag and an agy branch; `term::event_text` and `sse::row_kind` must learn `agy/*` (row_kind maps unknown to `system`; `result` -> `result`) |
| S6 | header line | `roles/mod.rs:446-449` | `"[agent-brain] {role} · {project} · profile {profile} ({desc}) · engine {engine} · model {model}"` |
| S7 | autopilot stop | `decide.rs:113-122` `enum StopReason { HetVong, HetNganSach, ViPhamGiaoThuc, VuongMac, DungBoiBan, RunLoi }` (serde `snake_case`, `as_str` strings) and `view.rs:6-15` `reason_word` | add `HetQuota` (`"het_quota"`, word `"hết quota"`), `stop_detail` text per UI-SPEC; D2 rule before the `failed` branch; `claim()`'s `more_round` match falls into `_ => {}` already |
| S8 | rounds | `build_facts` filters only `inspect` runs (`.filter(|r| r.role != "inspect")`, decide.rs:343) and `effective_rounds` counts earlier runs regardless of status | keep a quota run only as the LAST run when not resuming; drop superseded quota runs (Pitfall 2) |
| S9 | Telegram | `callback.rs` strict grammar `{op}:{task}:{rev}` / `{op}:{task}:{run}:{rev}`, `MAX_LEN = 64`, one-char ops; intake uses `i{op}:{flow}[:{index}]` with `MAX_INTAKE_INDEX = 99` [VERIFIED: callback.rs:1-9,175]; `detect.rs` `EventKind { Gate, Stop, QaPass, RunFailed }`; `render.rs buttons_for` | new `EventKind::Quota` (key `quota:{run_id}`) and exclude `outcome = quota` runs from `RunFailed`; new ops for team pick/confirm carrying `{task}:{idx}:{ver4}:{rev}` (see Code Examples) |
| S10 | workflows load/serve | `workflows::load` / `load_path` / `load_from`; `GET /api/workflows` in `api_workflows.rs` | make `validate_entry` `pub(crate)`, add `Workflow::to_file_entry()` (loops array -> object), extend GET payload with the editable shape + `version` |
| S11 | task creation | `new_task_args(auto, slug, workflow, project_id, request, origin)` builds `run lead`/`auto --start` argv with `--name --workflow [--origin telegram] -- project request` (api_tasks.rs:556-582); `start_new_task` resolves workflow with project default (586-626); `cmd_lead` creates branch + task row, then `set_task_workflow_if_null` | add `team: Option<&str>` through `StartRequest`, `NewTask` (note `#[serde(deny_unknown_fields)]` on `NewTask`, api_tasks.rs:26), `new_task_args` (`--team`), `main.rs` `Run`/`Auto` args, and `cmd_lead` -> `db::set_task_team_if_null` right after the workflow snapshot |
| S12 | schema | see Pattern 3 | v4 migration + `team_json` helpers |
| S13 | actions | `run_task_action(st, task_id, action_id, role, rev, note)` handles `run_dev` / `run_qa` / `relead` / `auto`; controls via `run_control_as(.., ActionSource)` [VERIFIED: api_tasks.rs:1489-1573, 1774-1922] | new functions in the same family: `change_team`, `quota_rerun` (write team CAS then start job; revert on refusal like `revert_approval`), `human_done`, `human_return`; `ActionSource` gains `Cli` only if a CLI entry is added |
| S14 | task state | `load_task_state` -> `TaskState {task, project, root, git, plan_text, ..., origin}` and `taskstate::derive_with` builds `actions`/`controls` | add `team`, `team_history`, `human`, `quota` facts and the four action ids |
| S15 | pipeline | `taskstate::step_state` maps outcomes (`dev:blocked`, `qa:fix`, `protocol_violation`, `review:*`) and `pipeline::state_word` maps state keys to words | add `quota` -> (`het_quota`, `hết quota`); ledger label gets the engine; total row drops the word `claude` (UI-SPEC) |
| S16 | stats | see Pattern 9 | `engines[]` and `qa_by_dev_engine[]` in `GET /api/stats` |
| S17 | projects map | `workflow_of(map, project_id)` reads the raw JSON `Value` per project (unknown keys survive scans) [VERIFIED: projects.rs:93-105] | add `team_of` twin; no struct change |
| S18 | assets | static table `ASSETS` with `include_bytes!` per file (assets.rs) | add `/js/settings.js` and `/screens/settings.html` entries; extend `tests/dashboard_assets.rs` |
| S19 | router | `screenTitles` + `parseHash` + `loadScript(name)` loads `'/js/' + name + '.js'` (app.js:402-440) | add `settings` route; the settings file must be self-contained (UI-SPEC) |

## Engine Facts

### claude engine (profile-bound, D-07)
- No change to argv/env beyond `engine` bookkeeping and env hygiene (strip `ANTHROPIC_MODEL`, `ANTHROPIC_DEFAULT_*_MODEL`, `CLAUDE_CODE_SUBAGENT_MODEL` and the deepseek key var so a DeepSeek-configured shell cannot leak into a claude run). Today only the three `unset_env` variables are stripped. [ASSUMED: that shell-exported `ANTHROPIC_DEFAULT_*_MODEL` would change `--model sonnet` alias resolution; the variables are documented in the DeepSeek integration guide [CITED: api-docs.deepseek.com/guides/coding_agents] but their effect on this binary was not tested.]
- Stream: `rate_limit_event` is informational: a real line from this repo: `{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","resetsAt":1792310400,"rateLimitType":"seven_day","utilization":0.94,"overageStatus":"rejected","overageDisabledReason":"out_of_credits","isUsingOverage":false,"surpassedThreshold":0.75,"unifiedWindows":{"five_hour":{...},"seven_day":{...}},"overageEnabled":true},"uuid":"...","session_id":"..."}` [VERIFIED: spikes/s1/raw/s1.jsonl line 5; also spikes/s4/raw/s4-r1.jsonl line 8].

### Claude Code failure shapes (for quota detection)
- Headless failures print the failure as the `result` on stdout; the process exits non-zero (exit code 0 on success) [CITED: https://code.claude.com/docs/en/headless]. SIGTERM exits 143 and records no result for the interrupted turn [CITED: same page].
- `system/api_retry` events: fields `attempt`, `max_retries`, `retry_delay_ms`, `error_status` (integer or null), `error` category in {`authentication_failed`, `oauth_org_not_allowed`, `account_on_hold`, `billing_error`, `rate_limit`, `overloaded`, `invalid_request`, `model_not_found`, `server_error`, `max_output_tokens`, `cloud_credential_error`, `unknown`} [CITED: https://code.claude.com/docs/en/headless]. A retried-then-successful 429 emits `api_retry` and ends in a successful result: it must NOT stop the task.
- Result type (SDK): `subtype` `"success"` or {`error_max_turns`, `error_during_execution`, `error_max_budget_usd`, `error_max_structured_output_retries`}; `is_error` boolean; `api_error_status?: number | null` described as "the HTTP status code of the API error that terminated the conversation"; `terminal_reason?` among {`completed`, `max_turns`, `tool_deferred`, `aborted_streaming`, `aborted_tools`, `hook_stopped`, `stop_hook_prevented`, `background_requested`, `blocking_limit`, `rapid_refill_breaker`, `prompt_too_long`, `image_error`, `model_error`, `api_error`, ...}; error arm has `errors: string[]` [CITED: https://code.claude.com/docs/en/agent-sdk/typescript]. Mapping `blocking_limit` to "usage limit reached" is [ASSUMED] (name only).
- `rate_limit_event.rate_limit_info.status` values `allowed` | `allowed_warning` | `rejected`; third-party cheatsheet treats `rejected` with `isUsingOverage == false` and a future `resetsAt` as a throttle [CITED: https://littlebearapps.com/help/untether/claude-stream-json-cheatsheet/ (community source, MEDIUM)].
- User-visible message strings in the Claude Code error reference: `You've hit your session limit`, `You've hit your weekly limit`, `You've hit your Opus limit`, `You've hit your Sonnet limit`, `Server is temporarily limiting requests`, `Request rejected (429)`, `Credit balance is too low`, `You've hit your monthly spend limit`, `spend limit reached`; 529: `API Error: Repeated 529 Overloaded errors...` (not a usage limit) [CITED: https://code.claude.com/docs/en/errors (first 100k chars read)].
- **Gap:** the exact `result` JSON of a usage-limit stop, and its exit code, were not found in docs and no fixture exists in this repo (`grep` of spikes/, tests/, protocol-log/ finds only `allowed_warning` events). Treat every shape below as [ASSUMED] until the first real capture; store the raw line (events table already does).

### deepseek engine (claude + DeepSeek endpoint, D-01)
- Endpoint and compatibility: base URL `https://api.deepseek.com/anthropic`; `x-api-key` fully supported, `anthropic-beta` / `anthropic-version` ignored; tools, streaming, `thinking` (budget ignored) supported; **model names not starting with `claude` return 400**; names starting with `claude-opus` map to `deepseek-v4-pro`, `claude-haiku` / `claude-sonnet` map to the flash model [CITED: https://api-docs.deepseek.com/guides/anthropic_api]. The official Claude Code guide lists `ANTHROPIC_BASE_URL=https://api.deepseek.com/anthropic`, `ANTHROPIC_AUTH_TOKEN=<key>`, `ANTHROPIC_MODEL`, `ANTHROPIC_DEFAULT_{OPUS,SONNET,HAIKU}_MODEL`, `CLAUDE_CODE_SUBAGENT_MODEL`, `CLAUDE_CODE_EFFORT_LEVEL` and uses `deepseek-flash[1m]` / `deepseek-flash` names [CITED: https://api-docs.deepseek.com/guides/coding_agents]. The two pages disagree on the flash model's name (`deepseek-flash` vs `deepseek-v4-flash` seen in search snippets): the E-09 probe decides which model strings the config ships.
- Because `--model sonnet` resolves inside Claude Code to a `claude-sonnet-*` id, DeepSeek's own mapping applies; recommended default for the first probe: keep the role aliases (`opus` for lead/review, `sonnet` for dev) so no DeepSeek model name is needed, then test one explicit `deepseek-*` name.
- Credential mechanics: `ANTHROPIC_AUTH_TOKEN` -> `Authorization: Bearer`; a gateway credential variable takes precedence over a saved claude.ai login immediately (the login stays saved, unused); setting only `ANTHROPIC_BASE_URL` does NOT replace the subscription; when both a shell export and a settings-file `env` block set a variable, **the settings-file value wins** [CITED: https://code.claude.com/docs/en/llm-gateway-connect]. Consequence: a `~/.claude/settings.json` `env` block (work profile) or `~/.claude-orca/settings.json` that pins `ANTHROPIC_*` would override our env; E-09 must run under the real profile. Never put the key in a `--settings` file (that would write the secret into the run dir, violating D-03).
- `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` stops background traffic outside the gateway path (telemetry/version checks) and disables auto-update [CITED: llm-gateway-connect]. Recommended for deepseek runs.
- Fallback knobs if E-09 shows 400s on `thinking`/`adaptive` or unknown fields: `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1`, `CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING=1` [CITED: llm-gateway-connect troubleshooting table].
- **Risk flag:** Anthropic's gateway documentation states it "doesn't support routing Claude Code to non-Claude models through any gateway" [CITED: https://code.claude.com/docs/en/llm-gateway]. Works-today is an observation, not a guarantee; this is the reason D-02 is a gate and the engine stays isolated behind `EnginePlan`.
- DeepSeek HTTP errors: 402 "You have run out of balance", 429 "sending requests too quickly, or have exceeded a concurrency or QPS limit", 503 overloaded; the page documents no JSON body shape [CITED: https://api-docs.deepseek.com/quick_start/error_codes]. How Claude Code renders a 402/429 from a non-Anthropic endpoint (retry category, `api_error_status`) is [ASSUMED]; capture with the invalid-key probe (401 shape) and treat 402/429 by `api_error_status`.
- Cost: Claude Code's `total_cost_usd` is a client-side estimate ("can differ from your actual bill") [CITED: headless docs] computed from its own Claude price table; do not trust it for DeepSeek. Recompute from usage x `pricing` (Pitfall 7). [ASSUMED: the DeepSeek stream reports non-zero `usage`; the docs page has no usage notes; E-09 verifies.]

### agy engine (D-04, D-05)
- Verified facts (agy 1.3.5 spike, flags re-confirmed on 1.4.7): `-p/--print/--prompt`, `--output-format text|json|stream-json`, `--print-timeout` ("0 waits until the turn completes (default 0s)" in the installed help, docs say 5m), `--dangerously-skip-permissions`, `--mode accept-edits|auto|plan`, `--model`, `--effort`, `--sandbox`, `--add-dir`, `--conversation`/`--continue`, **no `--max-turns`** (`flags provided but not defined: -max-turns`, exit 2) [VERIFIED: spikes/s7/raw/s7-help.txt, s7-maxturns.txt; `agy --help` this session].
- Stream schema, real lines: `{"event":"init","conversation_id":"...","init":{"cwd":"...","tools":[...],"permission_mode":"request-review", ...}}`, `{"event":"step_update","step_update":{"conversation_id":"...","step_index":1,"state":"DONE","step_type":"agent_response","text_delta":"\n","duration_seconds":2.37714765,"usage":{"input_tokens":5457,"output_tokens":13,"thinking_tokens":0,"cache_read_tokens":10424,"total_tokens":5470}}}`, `{"event":"result","result":{"conversation_id":"...","status":"SUCCESS","response":"OK\n","duration_seconds":2.572732554,"num_turns":1,"usage":{...}}}` [VERIFIED: spikes/s7/raw/s7-stream.jsonl lines 1-5]. `step_type` values documented: `user_input`, `agent_response`, `tool`, `checkpoint`; `state` `ACTIVE`/`DONE`. A `tool` step's fields (tool name, paths) were never captured: **`reads`/`edits`/`turns_before_first_edit`/`reads_outside_plan` cannot be derived**; store NULL.
- `result.status` values: `SUCCESS`, `ERROR`, `CANCELED`, `INTERRUPTED`, `INVALID`, `WAITING`, `RUNNING`; `error` present only on failure; a run that fails to produce a response exits non-zero and writes the reason to stderr; soft-denied tools continue and exit 0 with a stderr notice; `init.permission_mode` is `request-review` by default and `always-proceed` under `--dangerously-skip-permissions` [CITED: https://antigravity.google/docs/cli/headless]. Use the `permission_mode` value as a machine check: assert `always-proceed` for Dev, `request-review` otherwise, record a mismatch as a protocol warning.
- Usage mapping into existing columns: `input_tokens` -> `input_tokens`; `output_tokens` (+ `thinking_tokens`, folded in for cost; raw kept in `metrics_json.thinking_tokens`) -> `output_tokens`; `cache_read_tokens` -> `cache_read_tokens`; `cache_write_tokens` = 0; `usage_source = 'result'` when a `result` event exists, else `'partial'` from summed `step_update` DONE usages. `total_tokens` is input+output only (cache excluded, S7 `cache_read_tokens` 10424 > `total_tokens` 5470). Folding thinking into output is [ASSUMED] billing behaviour.
- `num_turns` is a counter, not a limit. Bound by `--print-timeout` set explicitly to `role_timeout - 30s` plus the runner's hard timeout (`cfg.timeout_for(role)` -> `TimedOut` + `kill_tree`). Timeout status/exit code is undocumented: probe it.
- **Runner change required:** `supervise_async` always spawns with `cmd.stdout(Stdio::piped())` (runner.rs:232), but CLAUDE.md requires agy stdout to a file (pipe bugs). Add `RunRequest.stdout_file: Option<PathBuf>`: spawn with the file as stdout, then tail it from a task (poll every 100-200 ms, hand complete lines to the same path as today, honour the 2-second drain at exit). For agy the raw stream file in the run dir IS the stdout file.
- Quota shape for agy: undocumented. Community reports show Antigravity quota failures as HTTP 429 `RESOURCE_EXHAUSTED` "Resource has been exhausted (e.g. check quota)" [CITED: discuss.ai.google.dev threads found by search, community/IDE evidence, LOW]. Detector (ASSUMED): `result.status == "ERROR"` or (no result and exit != 0) AND (`result.error` or stderr tail) matches `429|RESOURCE_EXHAUSTED|resource has been exhausted|quota|rate limit|usage limit|credits` case-insensitively.
- Process-tree behaviour of `agy` (language server / daemon children, survives parent?) was never tested: add to the agy probe (normal exit leftovers + S4 kill).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Atomic JSON file write | new tmp+rename code | the helper pattern in `projects.rs:340-395` (extract to `settings_store`) plus dir fsync | already tested, 0600 temp, cleans on failure |
| JSON loader with line/col errors | custom parser | `workflows::key_lines` + `serde_json` error `line()/column()` | UI and CLI already show these error styles |
| Slug validation | new regex | `workflows::validate_name` (40-byte cap) | one rule across tasks, workflows, teams |
| Process kill | engine-specific kill | `procs::kill_tree` + `runner::cmd_kill` (S4) | agy and deepseek runs use the same `supervise` |
| Plan scope / protocol violation for agy Dev | new diff checker | the post-run block in `cmd_dev` (`diff_names(dev_base, HEAD)` ∪ `changed_paths`, set difference against plan files, `outcome = protocol_violation`, `EXIT_PROTOCOL`) [VERIFIED: roles/dev.rs:430-455] and `report::sensitive_path` (secret-looking paths) [VERIFIED: report.rs:216] | already encodes NFC normalisation and `.plan.md` allowance |
| Cost estimation | per-engine formulas | `pricing::estimate_usd` with an engine-first price lookup | budgets (`pricing::budget_usage`) already consume `cost_usd` |
| Callback grammar | free-text callback data | extend `callback.rs` strict encode/decode (1-char op, ids, 16-hex rev, <= 64 bytes) | 5.3/5.4 trust rules rely on it |
| Fake CLI for tests | new binary per engine | `tests/fakes/fake_claude.py` scenarios (`events`, `raw_lines`, `exit_code`, `stderr`, `by_role`) for claude+deepseek; one small `fake_agy.py` | no real quota or key ever used |
| Secrets in test evidence | grep ad hoc | `spikes/lib/scan-secrets.sh` / `scrub.py` for fixtures and a Rust planted-key test | existing hard gate (D-03 of phase 1) |

**Key insight:** this phase is about not duplicating the Phase 3/5 machinery (supervise, kill, parse, actions, atomic writes). Anything that "feels new" (quota, human wait, agy) should be a small addition at an existing seam.

## Runtime State Inventory

(Schema/stored-data migration aspects only; this is not a rename phase.)

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | `runs.engine` exists, default `'claude'`; every historical run is claude (insert hard-codes it) [VERIFIED: db.rs:32,326]. `tasks` has no team column. Live `~/.local/share/agent-brain/brain.db` was at user_version 2 on 2026-10-23, 5.4's v3 not yet deployed [CITED: .planning/STATE.md Ops 2026-10-23; not re-read from the live file] | code edit (`insert_run`), schema v4 migration; README note covers a 2 -> 4 jump, backup `brain.db.bak-pre-6-<date>` (+ `-wal`) |
| Live service config | `~/.config/agent-brain/teams.json` / `staff.json` do not exist yet; `projects_map.json` has no `team` key; `config.json` has no `engines` key | built-in defaults make all optional; document keys in README |
| OS-registered state | none (no services/tasks registered by this phase) | none, verified by reading the code paths; nothing registers OS state |
| Secrets/env vars | new variable name `DEEPSEEK_API_KEY` (name only in config; value only in the process env). Not set in the researcher's environment (checked presence only) | user exports it in the shell that runs `agent-brain serve` / CLI |
| Build artifacts | installed binary `~/.local/bin/agent-brain` is older than the repo (Ops note); new binary must be deployed after backup | deploy checklist in README + LIVE-CHECKS §8 |

## Common Pitfalls

### Pitfall 1: Dev re-run is refused on the dirty tree left by a quota stop (breaks ENG-03)
**What goes wrong:** `cmd_dev` returns `Working tree có thay đổi chưa commit — hãy commit thay đổi tay với message '{task}: plan':` when `changed_paths` is non-empty [VERIFIED: roles/dev.rs:86-103]; the partial diff the first engine left is exactly that.
**How to avoid:** continuation mode, derived (no new flag): when the latest Dev run of the task has outcome `quota`, allow dirty paths that are in the plan file set + protocol files; add to `user_input` "Tiếp tục công việc dở: đọc .plan.md, xem `git diff` hiện có và .handover.md nếu có; không làm lại từ đầu"; extend `allowed_tools` with `Bash(git diff *)`, `Bash(git status *)` and `Read(./.handover.md)` (+ absolute-path rules via `abs_rules`) and add `.handover.md` to the guard `read_allow`. `.handover.md` does not exist until Phase 4 (OPT-04 pending): include only when the file exists. The post-run violation check already unions committed and uncommitted changes, so pre-existing partial changes are validated too.
**Warning signs:** second Dev run exits 1 immediately with the dirty-tree message.

### Pitfall 2: Quota runs burn autopilot rounds
**What goes wrong:** `effective_rounds` counts earlier runs of a stage regardless of status, so a quota-failed Implement counts as round 1 and a re-run becomes round 2 of `max_rounds` (default 3).
**How to avoid:** in `build_facts`, drop quota runs except when the quota run is the last run and `resume` is false (so D2 can stop on it). Unit-test `effective_rounds` and D2 with a quota run last / superseded. The manual Lead round (`manual_round` in lead.rs) must skip quota runs too.

### Pitfall 3: `running` row + no pid = `lost`
See Pattern 4. Also `cmd_kill` marks such a row lost with "Run ... không còn tiến trình". Do not model the human wait as a run.

### Pitfall 4: agy stdout via pipe
CLAUDE.md: agy has pipe-output bugs. Runner pipes stdout today. Implement stdout-to-file + tail; test with `fake_agy.py` writing slowly and a final line without trailing newline.

### Pitfall 5: Env ordering for deepseek
Profiles remove `ANTHROPIC_API_KEY`/`ANTHROPIC_AUTH_TOKEN`/`ANTHROPIC_BASE_URL` (`unset_env`), so setting DeepSeek vars before `build_env` silently loses them. Insert after `build_env`. Conversely the claude engine must not inherit DeepSeek model/alias variables or the key variable. Test via the fake recorder: record env var NAMES present and booleans (`auth_token_equals_sentinel`), never values.

### Pitfall 6: Pricing keyed by model family misprices alias models
`price_for(model, table)` matches `model.to_ascii_lowercase().contains(family)`; with no match it returns the **most expensive** entry [VERIFIED: pricing.rs:8-39]. A deepseek run stores model alias `sonnet` and would be priced as Anthropic sonnet; an engine without an entry would be priced as opus. Use an engine-first lookup: exact key `deepseek` / `agy` in `pricing` first, then family substring only for engine `claude`; "no entry" -> `None` -> UI "chưa có giá" (never 0). Ship no default `deepseek`/`agy` prices (user fills `config.json`).

### Pitfall 7: `total_cost_usd` and budgets for non-Claude engines
`run_cost` uses `r.cost_usd` verbatim when `usage_source == "result"` and the run is not killed [VERIFIED: pricing.rs:57-77]. For deepseek/agy store a computed cost at `finish_run` (usage x engine price, 0 when unpriced) so budgets and `SUM(cost_usd)` work; keep `priced` as a separate flag in the stats payload.

### Pitfall 8: Settings-file `env` overrides process env
A work/personal profile `settings.json` `env` block with `ANTHROPIC_*` beats our env (doc: settings-file value applies). E-09 must run with the real profile; the readiness check cannot detect this (do not read `~/.claude*`).

### Pitfall 9: Redaction order
Redact before `stream_file.write_all`; otherwise the key is on disk even if events are clean. Cover: stream file, `events.raw_json`, `term::emit`, job output ring, `stderr.txt` (post-process), `runs.error`, Telegram texts (render uses DB fields only, so DB redaction covers it).

### Pitfall 10: Key exfiltration through agent output into committed files
Lead has an unrestricted `Read` tool (hooks cover Bash only for Lead: `write_bash_hook_settings`), so a prompt-injected deepseek Lead could `Read(/proc/self/environ)` and write the token into `.plan.md`, which `commit_paths` then commits (ENG-05's literal failure mode). Mitigation: `secrets::scan_paths(root, changed_paths, secrets)` before every `commit_paths` in lead/dev/review flows when the run used a secret-bearing engine; on a hit: do not commit, `outcome = protocol_violation`, redact the file content in the message. Optional hardening [ASSUMED syntax]: `--disallowedTools "Read(//proc/**)"` for deepseek runs.

### Pitfall 11: agy soft-deny returns SUCCESS with no edits
D-05 already says "empty diff on Dev = suspicious". Implement: agy Dev `SUCCESS` and `diff_names(dev_base, HEAD)` ∪ `changed_paths` empty -> outcome `agy:empty` (new), treated like `dev:blocked`-class stop (`VuongMac`-style detail "agy không sửa file nào — có thể bị từ chối quyền") rather than success.

### Pitfall 12: agy version drift and undocumented defaults
S7 ran 1.3.5; installed is 1.4.7. `--print-timeout` default differs between docs (5m) and `--help` (0s): always pass explicitly. Re-capture `init`, tool step, error and timeout shapes in the agy probe before writing the parser.

### Pitfall 13: Telegram callback constraints
64-byte limit and strict decoding: team names (<= 40 chars) cannot ride in callback data with `{op}:{task}:{rev}`. Use an index into the server's team list plus a 4-hex version of that list; reject stale index with a re-rendered picker. Add a confirm tap (Kill precedent) because a re-run costs quota/money.

### Pitfall 14: Name length mismatch (UI-SPEC 48 vs loader 40)
Resolve before coding the editors and validators (Pattern 5).

### Pitfall 15: Python contract tests share fixtures
`tests/contract/stream.json` is consumed by both Python (`tests/test_contract_stream.py`) and Rust (`contract_stream.rs`, `expect_rust` block). Do not add agy/quota cases there without checking the Python side; use Rust-only fixtures under `tests/fixtures/stream/`.

### Pitfall 16: `new_task_args` and `NewTask` signature churn
`NewTask` has `deny_unknown_fields`; existing tests post bodies without `team` (must stay valid). `new_task_args` has unit tests asserting exact argv; add the `--team` pair only when a team is chosen so existing assertions hold.

### Pitfall 17: Shared UI files
`js/app.js` (status maps, router, `AB.*` helpers) and `index.html` are touched by Settings and by task/assign/stats deltas; serialize those plans.

### Pitfall 18: Human `Xong` preconditions
Require: current branch == task branch (existing "không đoán" rule), clean tree except protocol files, remote `origin` present, branch matches `task/<slug>` with `--` separators, `merge --ff-only` (never `pull`, never reset). Diverged/force-pushed branch -> refuse with a Vietnamese message; do not rewrite local history. `git_cmd` already sets `GIT_TERMINAL_PROMPT=0` and `GIT_OPTIONAL_LOCKS=0` [VERIFIED: gitops.rs:11-16].

## Code Examples

### teams.json shape (proposed)
```json
{
  "tiet-kiem": {
    "description": "Lead và QA dùng claude, Dev dùng deepseek để tiết kiệm chi phí.",
    "roles": { "lead": "claude", "dev": "deepseek", "qa": "claude",
               "plan-review": "claude", "code-review": "claude" }
  }
}
```
`staff.json`: `{ "ha": { "display_name": "Hà", "notes": "Zalo 09xx, làm phần giao diện" } }`. `config.json` additions:
```json
{
  "engines": {
    "deepseek": { "command": "claude", "base_url": "https://api.deepseek.com/anthropic",
                  "key_env": "DEEPSEEK_API_KEY",
                  "models": { "lead": "opus", "dev": "sonnet", "qa": "sonnet",
                              "plan-review": "opus", "code-review": "opus" } },
    "agy": { "command": "agy", "print_timeout_margin_secs": 30, "model": null }
  },
  "pricing": { "deepseek": {"input": 0.0, "output": 0.0, "cache_read": 0.0, "cache_write": 0.0} }
}
```
(`pricing.deepseek` is an example only; ship none. Model strings are the E-09 outcome.)

### agy argv (dev) and env
```text
agy -p "<role prompt>\n\n---\n<user input>" --output-format stream-json
    --print-timeout 1770s --dangerously-skip-permissions        # Dev only
cwd = project root; stdin = /dev/null; stdout = <run_dir>/stream.jsonl (file, tailed); stderr = <run_dir>/stderr.txt
```

### Quota detector (sketch for quota.rs; pure, fixture-tested)
```rust
pub struct QuotaHit { pub signal: &'static str, pub detail: String, pub reset_at: Option<i64> }

// claude-format (claude + deepseek). Called once after the stream ends.
pub fn detect_claude(s: &Summary, exit_code: Option<i32>, stderr_tail: &str) -> Option<QuotaHit> {
    // A final result that succeeded is never a quota stop (overage may have covered a rejected event).
    if s.result_seen && !s.is_error { return None; }
    if s.rate_limit_rejected && !s.using_overage { return hit("rate_limit_event.rejected", ...); }
    if s.is_error && matches!(s.api_error_status, Some(429) | Some(402)) { return hit("result.api_error_status", ...); }
    if s.terminal_reason.as_deref() == Some("blocking_limit") { return hit("result.terminal_reason", ...); }
    if s.is_error && text_matches(s.result_text.as_deref()) { return hit("result.text", ...); }
    if !s.result_seen && matches!(s.last_retry_error.as_deref(), Some("rate_limit") | Some("billing_error")) { return hit("api_retry", ...); }
    if !s.result_seen && exit_code.unwrap_or(0) != 0 && text_matches(Some(stderr_tail)) { return hit("stderr", ...); }
    None
}
// text_matches: (?i)(hit your (session|weekly|opus|sonnet|monthly spend)? ?limit|usage limit|credit balance is too low|spend limit|insufficient balance|out of balance|rate limit|too many requests|\b429\b)
```
`Summary` additions needed (all optional fields, tolerant parse): `rate_limit_rejected`, `using_overage`, `api_error_status`, `terminal_reason`, `last_retry_error`, plus `result_seen` exposure.

### Fake-engine fixtures (shapes; claude/deepseek run on `fake_claude.py` via scenario `{"events":[...],"exit_code":1}`)
```jsonl
{"type":"system","subtype":"init","session_id":"fake-quota","model":"fake"}
{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1792310400,"rateLimitType":"five_hour","isUsingOverage":false,"overageStatus":"rejected"},"uuid":"u1","session_id":"fake-quota"}
{"type":"result","subtype":"success","is_error":true,"api_error_status":429,"terminal_reason":"blocking_limit","result":"You've hit your session limit","num_turns":1,"session_id":"fake-quota","total_cost_usd":0,"usage":{"input_tokens":0,"output_tokens":0,"cache_read_input_tokens":0,"cache_creation_input_tokens":0},"permission_denials":[]}
```
Negative fixture (must NOT stop): `system/api_retry` with `"error":"rate_limit","error_status":429` followed by a normal success `result`. DeepSeek-402 fixture: `result` with `is_error:true,"api_error_status":402`. agy: `{"event":"result","result":{"conversation_id":"fake","status":"ERROR","error":"HTTP 429 RESOURCE_EXHAUSTED: Resource has been exhausted (e.g. check quota).","duration_seconds":1.2,"num_turns":0,"usage":{"input_tokens":0,"output_tokens":0,"thinking_tokens":0,"cache_read_tokens":0,"total_tokens":0}}}` with exit 1. All field names above are from docs/real spike lines; the *values* of quota messages are [ASSUMED] and flagged in the Assumptions Log.

### Telegram grammar extension (sketch)
```text
t:{task}:{idx}:{ver4}:{rev}   pick team idx from the list rendered in this message (ver4 = 4-hex of the team-name list)
u:{task}:{idx}:{ver4}:{rev}   confirm re-run with that team   (worst case ~46 bytes < MAX_LEN 64)
```
Both decode strictly; a `ver4` mismatch answers "Danh sách đội hình đã đổi" and re-renders. `u` calls the shared `quota_rerun` action with `ActionSource::Telegram` (not plan approval; Telegram-origin tasks stay dashboard-approve-only per `TELEGRAM_APPROVE_REFUSED`).

### E-09 probe (user-run; the researcher did NOT run it)
```bash
# scripts/live/e09-deepseek.sh (planner to write; secrets never echoed)
[ -n "${DEEPSEEK_API_KEY:-}" ] || { echo "Thiếu DEEPSEEK_API_KEY"; exit 2; }
S="$(mktemp -d)"; cd "$S" && git init -q . && echo base > a.txt && git add . && git -c user.email=x@y -c user.name=x commit -qm init
ANTHROPIC_BASE_URL=https://api.deepseek.com/anthropic ANTHROPIC_AUTH_TOKEN="$DEEPSEEK_API_KEY" \
CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 \
timeout 120 claude -p "Tạo file e09.txt chứa đúng một từ: ok. Sau đó trả lời DONE." \
  --output-format stream-json --verbose --model sonnet --max-turns 4 \
  --allowedTools "Write(./e09.txt)" --strict-mcp-config </dev/null > e09.jsonl 2> e09.stderr
# verify.sh prints yes/no only: init, tool_use Write, tool_result, result, usage.input_tokens>0 && output_tokens>0,
# e09.txt exists, grep -qF -- "$DEEPSEEK_API_KEY" over e09.jsonl/e09.stderr (status only), modelUsage keys, total_cost_usd.
# Probe B (no billing): same with ANTHROPIC_AUTH_TOKEN=invalid-key-for-probe -> capture the real auth-error stream shape.
# Pass criteria: stream-json parses with stream::Summary, usage non-zero, tool use + file edit worked, no key leak.
# On fail: retry once with CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1 CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING=1, then stop and ask the user (dsh black box vs drop).
```
Scrub raw output with `spikes/lib/scrub.py`, gate with `spikes/lib/scan-secrets.sh`, then copy to `tests/fixtures/stream/deepseek_e09*.jsonl` and add a Rust test that summarises it. [VERIFIED: scripts exist: spikes/lib/scan-secrets.sh, spikes/lib/scrub.py]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Source spec engines `claude-work` / `claude-personal`, `~/.claude-max` | one `claude` engine bound to the project profile | CONTEXT D-07 (2026-10-24) | teams name engines, never accounts |
| DeepSeek via `dsh` Docker container (text-only, kill inside container) | `claude` + `ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` | D-01; S6 result | reuses parser/kill/max-turns; gated by E-09 |
| agy as manual tool | headless `agy -p --output-format stream-json` | S7 (1.3.5), re-confirmed flags on 1.4.7 | needs agy parser branch, stdout-to-file, diff checks |
| Single `claude` engine, `runs.engine` constant | per-run engine + team in `metrics_json` | this phase | stats per engine |

**Deprecated/outdated:** `dsh` as an engine (kept only as the fallback if E-09 fails and the user chooses it); the SitePoint claim that DeepSeek lacks an Anthropic endpoint (official docs list it).

## Recommended Plan / Wave Decomposition (answers question 10)

Each plan is executed by agy in a worktree and reviewed by Claude (CLAUDE.md). File ownership is disjoint inside a wave. Count: 14 plans, 7 waves.

| Plan | Wave | Depends on | Scope (requirements) | Owns (main files) | Checkpoints |
|------|------|-----------|----------------------|-------------------|-------------|
| 06-01 E-09 DeepSeek gate | 1 | none | ENG-01/05 evidence | `scripts/live/e09-deepseek.sh`, `spikes/e09/**`, `tests/fixtures/stream/deepseek_e09*.jsonl`, result doc | human-action (user runs with own key), then **decision** pass/fail. FAIL => cancel 06-07, ask dsh-vs-drop, set builtin `tiet-kiem` per answer |
| 06-02 agy 1.4.7 probe | 1 | none | ENG-06 evidence | `scripts/live/agy-probe.sh`, `spikes/s7b/**`, fixtures `agy_*.jsonl` | human-action: tool run with and without skip flag, `init.permission_mode`, soft-deny stderr text, `--print-timeout` behaviour (status/exit), leftover processes after normal exit and after S4 kill, error shapes (bad `--model`), `--mode accept-edits` write test |
| 06-03 Teams/staff/config foundations | 1 | none | TEAM-01, SET-04 (store), TEAM-03 (resolve) | `teams.rs`, `staff.rs`, `assets/teams.default.json`, `config.rs` (`engines`, engine-first `pricing`), `projects.rs` (`team_of`), `lib.rs`, REQUIREMENTS.md ids | none |
| 06-04 Schema v4 + run/team helpers | 1 | none | TEAM-02, HUM-03 (rows) | `db.rs` (`team_json`, CAS helpers, `NewRun.engine`, `insert_human_run`), `tests/db_migration.rs`, README upgrade note | **decision** (one-way, as 05.4-02 Task 1) before any code |
| 06-05 Engine plumbing | 2 | 03, 04 | ENG-01, TEAM-03, ENG-05 (hygiene) | `engine.rs`, `secrets.rs`, `roles/mod.rs`, `profile.rs`, `runner.rs` (redact + `stdout_file` capability), `roles/{lead,dev,qa,review}.rs`, `main.rs` (`--team`), `autopilot/mod.rs` (`--team` on `--start`), header line, `runs.engine`/`metrics.team`, set team at creation | none; claude-only behaviour byte-identical, existing suites green |
| 06-06 Quota | 3 | 05 | ENG-02, ENG-03, QUOTA-01 (core) | `quota.rs`, `stream.rs`, `error.rs`, `roles/mod.rs` seam, `roles/dev.rs` continuation, `autopilot/{decide,view,mod}.rs`, `taskstate.rs`/`pipeline.rs` words, fixtures/scenarios | none |
| 06-07 DeepSeek engine | 4 | 06 (+01 pass) | ENG-01, ENG-05, ENG-04 (cost) | `engine.rs` deepseek env/models/readiness/cost, `secrets.rs` commit scan wiring, `tests/engine_secrets.rs` | gated on 06-01 |
| 06-08 agy engine | 4 | 06, 02 | ENG-06 | `engine.rs` agy, `stream.rs` agy branch, `runner.rs` tail consumer, `roles/*` agy wiring, Dev scope/empty-diff/permission_mode checks, `tests/fakes/fake_agy.py`, `tests/engine_agy.rs` | none |
| 06-09 Settings + task backend API | 4 | 03, 04, 05, 06 | SET-01..04, TEAM-02/03, ENG-04/07 | `dashboard/api_settings.rs`, `settings_store.rs`, `api_workflows.rs`, `workflows.rs` (`pub validate_entry`, `to_file_entry`), `api_tasks.rs`, `taskstate.rs`, `pipeline.rs`, `stats.rs`, `api_stats.rs`, `dashboard/mod.rs` | none |
| 06-10 Human engine | 5 | 06, 09 | HUM-01..03 | `human.rs`, `gitops.rs` (fetch/ff), `autopilot/{mod,claim}.rs` (`EXIT_WAITING`, `waiting_human`), actions in `api_tasks.rs`, `tests/human_engine.rs` (local bare-repo "staff" clone) | scope question (Open Question 1) answered before start |
| 06-11 Telegram quota + team pick + `/task` team | 5 | 06, 09 | QUOTA-01, TEAM-03 | `telegram/{callback,detect,render,bot,intake,compose}.rs`, `tests/telegram_*.rs` | none |
| 06-12 Settings UI + shared UI plumbing | 5 | 09 | SET-01..05 | `js/app.js`, `index.html`, `screens/settings.html`, `js/settings.js`, `assets.rs` | none |
| 06-13 Task / Giao việc / Tổng quan / Thống kê UI | 6 | 10, 12 | TEAM-02/03, QUOTA-01, HUM-01/02, ENG-04/07 | `screens/{assign,task,overview,stats}.html`, matching `js/*.js` | UI-SPEC copy checked verbatim |
| 06-14 Gates, docs, live-checks | 7 | all | SET-05, ENG-01 gate | `app.css` (`build-css.sh`), ui-lint, js-check, smoke cases, README, `agent-brain/LIVE-CHECKS.md` §8, `protocol-log/nghiem-thu-p6.md`, REQUIREMENTS traceability | **human-verify**: ENG-01 real `tiet-kiem` task, ENG-02 via fake engine in the real dashboard + fake Telegram, agy Dev small task, human engine with a second clone, key-hygiene sweep (run dirs, brain.db dump, `git log -p`) |

Order rationale: the two probes are independent and user-run so they start in Wave 1 next to pure-code foundations; D-02 is honoured because only 06-07 depends on the E-09 outcome (agy, teams, quota, Settings, human proceed regardless). If E-09 fails, 06-03's built-in `tiet-kiem` must be revisited with the user (the `deepseek` engine name is fixed by D-08).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Exact JSON of a Claude Code usage-limit stop (`is_error:true`, `api_error_status:429`, `terminal_reason:"blocking_limit"`, message text) | Quota detector, fixtures | Detector misses real stops; mitigated by multi-signal rules + raw line stored + recalibration after first real capture |
| A2 | `blocking_limit` terminal reason means usage limit | Engine Facts | Extra or missing quota hits |
| A3 | How Claude Code renders a DeepSeek 402/429 (retry category / `api_error_status`) | Engine Facts | DeepSeek balance exhaustion shows as generic failure (run lỗi) instead of quota |
| A4 | DeepSeek stream reports non-zero `usage`; `modelUsage`/model names; flash model name | Engine Facts | ENG-04 stats empty for deepseek; E-09 settles it |
| A5 | agy quota error text/status pattern (`ERROR` + 429/RESOURCE_EXHAUSTED text) | Quota detector | agy quota stops look like generic failures |
| A6 | agy `thinking_tokens` billed as output; folded into `output_tokens` | Usage mapping | cost under/over-estimates |
| A7 | agy exit code / `result.status` on `--print-timeout` expiry | Engine Facts | timeout misclassified (runner hard timeout still stops it) |
| A8 | agy leaves no daemon children after normal exit / S4 kill | Pitfall list | leftover processes (ENG-06 / process-safety constraint) |
| A9 | `ANTHROPIC_DEFAULT_*_MODEL` in the user shell would alter claude alias resolution (so strip them) | Seam S3 | harmless extra stripping if wrong |
| A10 | `--disallowedTools "Read(//proc/**)"` syntax works | Pitfall 10 | optional hardening ineffective; post-run scan still protects commits |
| A11 | Human step marker approach is acceptable (no run row while waiting) | Pattern 4 | if user wants a ledger row while waiting, UI needs a synthesised row or a status CHECK change (table rebuild) |
| A12 | `agent-brain` pushes the task branch at human-step preparation (best-effort, only if `origin` exists) | Pattern 4 / Open Question 2 | staff cannot fetch the branch if nobody pushes |
| A13 | Live brain.db is still user_version 2 | Runtime State Inventory | upgrade note wording only |
| A14 | UI-SPEC assumptions C1-C20 stand as planner defaults except C1, C6, C11, C12, C13 which the user confirmed | UI | per UI-SPEC |

## Open Questions (RESOLVED)

All eight questions were answered by the user on 2026-10-26 (06-CONTEXT D-24…D-29) or are settled by a plan; each item ends with its resolution.

1. **Human engine for Lead / plan-review / code-review (D-19 says "any role except QA").**
   - What we know: Dev/fix has a clear artifact (commits on the task branch, diff scope check). Lead needs a committed `.plan.md` with subject `{slug}: plan` that passes `validate_plan`, but for a NEW task there is no branch yet (`cmd_lead` creates it) and a committed skeleton would be mistaken for a plan commit by `plan_commit_on_branch`. Reviewers must produce `.plan-review.md` / `.code-review.md` with front matter (`parse_review`).
   - What's unclear: whether v1 must support all four non-QA roles.
   - Recommendation: build the generic marker/`Xong` mechanism with a per-role artifact contract table; ship Dev (implement + fix) fully; allow the other roles in the validator but have the runtime refuse with a clear Vietnamese "chưa hỗ trợ engine người cho vai trò này" unless the user wants the larger scope. Ask the user.
   - **RESOLVED → D-24:** `người` only for the Dev role in this phase; the teams loader and the Settings save refuse it on Lead, plan-review, code-review and QA (plan 06-03; runtime in 06-12).
2. **Who pushes the task branch for staff?** D-19/D-20 say agent-brain "prepares" the branch and staff "push" theirs; staff need the branch from the remote first. Recommend a best-effort `git push -u origin task/<slug>` at preparation (new network side effect; failure non-fatal and shown in the callout). Needs confirmation.
   - **RESOLVED → D-25:** best-effort `git push` of the task branch at the start of a human step (and at Trả lại), never force; a failure keeps the wait and shows the branch and the push command (plans 06-12, 06-18).
3. **agy Dev permissions:** D-05 locks `--dangerously-skip-permissions`; `--mode accept-edits` (and `--sandbox`) may suffice because agent-brain commits the work itself after the run. Test in 06-02; if it works, propose switching (narrower blast radius).
   - **RESOLVED → D-26:** the 06-02 probe tests `--mode accept-edits` / `--sandbox`; Dev uses the narrowest mode that edits, the skip flag only as the fallback; the post-run diff check always applies (plans 06-02 decision task, 06-09).
4. **DeepSeek model strings:** alias-based (`opus`/`sonnet` -> DeepSeek mapping) versus explicit `deepseek-*` names; docs disagree. E-09 decides.
   - **RESOLVED at execution time by the E-09 gate (plan 06-01 Task 3):** `pass-alias` keeps the role aliases, `pass-explicit` ships the observed model ids (applied by plan 06-11), `fail` stops the DeepSeek work (D-02).
5. **Name length:** 40 (loader) vs 48 (UI-SPEC).
   - **RESOLVED → D-27:** 40 characters everywhere (`workflows::validate_name`); the UI-SPEC's 48 is superseded (plans 06-03, 06-08).
6. **Telegram flow:** one confirm tap after picking a team (recommended) or direct execution.
   - **RESOLVED → D-28:** picking a team on Telegram needs a second confirm tap before anything runs (plan 06-14).
7. **Change team while a human step is waiting:** ~~recommend allowed, clears the marker and records history.~~ (superseded)
   - **RESOLVED → D-29:** changing the team is REFUSED while a human step waits, the same as while a run is live (plans 06-05, 06-10, 06-16).
8. **Task with team missing from `teams.json`:** recommend the step refuses with "Đội hình 'x' của task không còn trong teams.json — đổi đội hình rồi chạy lại" (exit 1, no run row), consistent with UI-SPEC's amber hint.
   - **RESOLVED in plans (Claude's discretion, recorded in 06-05):** the next step is refused with `Đội hình '{x}' của task không còn trong teams.json — đổi đội hình rồi chạy lại.` (exit 1, no run row); the task screen shows the amber hint and keeps `Đổi đội hình` available (06-10, 06-17).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| claude CLI | claude/deepseek engines, live checks | yes | 2.1.310 | none (tests use fake) |
| agy CLI | agy engine, probe | yes | 1.4.7 | tests use `fake_agy.py` |
| cargo / Rust | build + tests (`--offline --locked`) | yes | 1.98.1 (rust-version 1.85) | none |
| git | gitops, human `Xong` | yes | 2.43.0 | none |
| node | `scripts/dashboard-js-check.mjs` | yes | v24.21.0 | none |
| python3 | fake_claude.py, ui-lint, scrub | yes | 3.12.3 | none |
| Chrome headless | `scripts/dashboard-smoke.sh` | yes | 154.0.8037.92 | skip smoke locally, run on user machine |
| docker | not used (dsh dropped, D-01) | present | - | - |
| `DEEPSEEK_API_KEY` | E-09, ENG-01 real gate | not set in the researcher's env (presence only checked) | - | none for live gate; automated tests use a planted fake value |
| Remote git for human engine tests | HUM-02 tests | local bare repo via `Sandbox::make_repo(.., remote)` | - | - |

**Missing dependencies with no fallback:** a DeepSeek key (blocks only the live E-09 / ENG-01 gate, supplied by the user).
**Missing dependencies with fallback:** none.

## Validation Architecture

`workflow.nyquist_validation` is not set in `.planning/config.json` (file contains only `workflow._auto_chain_active`), so validation is enabled.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` (lib unit tests + integration tests on `tests/common::Sandbox` with fake claude), `node scripts/dashboard-js-check.mjs` (vm harness), bash UI gates, headless-Chrome smoke + CDP keys [VERIFIED: 05.5-VALIDATION.md:23] |
| Config file | `agent-brain/Cargo.toml`; sandbox config written by `Sandbox::write_config` (fake `claude_bin`) |
| Quick run command | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib -- teams engine quota stream secrets` and `node scripts/dashboard-js-check.mjs` |
| Full suite command | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1`, then `bash scripts/dashboard-ui-lint.sh`, `bash scripts/build-css.sh --check`, `node scripts/dashboard-js-check.mjs`, `bash scripts/dashboard-smoke.sh settings --keys` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ENG-01 | mixed team: claude lead/qa + deepseek dev (deepseek `command` = fake claude); `runs.engine` per run, header `engine …`, `metrics.team` | integration | `cargo test --test engine_routing` | Wave 0 |
| ENG-02 | quota fixtures (claude, deepseek 402/429, agy) -> outcome `quota`, `EXIT_QUOTA`, autopilot `HetQuota`; transient 429 does not stop | unit + integration | `cargo test --lib quota`; `cargo test --test engine_quota` | Wave 0 |
| ENG-03 | re-run after quota keeps partial diff, prompt mentions diff/.handover.md, extended allowed tools, round not consumed | integration | `cargo test --test engine_quota eng03` | Wave 0 |
| ENG-04 | stats `engines[]` split tokens/cost; unpriced engine flagged | integration | `cargo test --test dashboard_stats`, `--test db_stats` | exists; extend |
| ENG-05 | planted key absent from stream.jsonl, events.raw_json, stderr.txt, job output, `runs.error`, git history; fake echoes env into stream and `.plan.md` | integration | `cargo test --test engine_secrets` | Wave 0 |
| ENG-06 | `fake_agy.py` stream parsed, usage mapped, stdout-to-file tail, Dev flag only for Dev, empty-diff suspicious, out-of-scope diff = protocol violation | integration | `cargo test --test engine_agy` | Wave 0 |
| ENG-07 | QA must be claude (loader + Settings save refused); Cần-sửa SQL by Dev engine incl. human | unit + integration | `cargo test --lib teams`; `--test dashboard_stats` | Wave 0 / extend |
| TEAM-01 | loader: size cap, line/col errors, builtins, override, QA rule, unknown staff warning | unit | `cargo test --lib teams` | Wave 0 |
| TEAM-02 | v3 -> v4 upgrade keeps data; CAS history; refuse while run live; reopen no-op | integration | `cargo test --test db_migration`, `--test task_team` | extend / Wave 0 |
| TEAM-03 | project `team` default, body/CLI override, unknown team refused, `NewTask` without `team` unchanged | integration | `cargo test --test task_team --test dashboard_http --test telegram_intake` | extend |
| SET-01..04 | GET/POST teams/workflows/staff: atomic write, version conflict 409, invalid file refuses, token+Host middleware, never returns key values | integration | `cargo test --test dashboard_settings` | Wave 0 |
| SET-05 | ui-lint, css current, assets table, no banned classes, settings smoke | script | gates in Full suite | exists; extend |
| HUM-01..03 | marker, `EXIT_WAITING`, `Xong` ff-only fetch from a local bare repo, dirty/branch/diverged refusals, human run row, stats row | integration | `cargo test --test human_engine` | Wave 0 |
| QUOTA-01 | rerun action shared by dashboard and Telegram (`t`/`u` callbacks, stale ver, Telegram-origin rules) | integration | `cargo test --test telegram_actions --test engine_quota` | extend |

### Sampling Rate
- **Per task commit:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib -- <module filter>`
- **Per wave merge:** the wave's integration tests plus `bash scripts/dashboard-ui-lint.sh` / `node scripts/dashboard-js-check.mjs` for UI waves
- **Phase gate:** full suite green, `scripts/build-css.sh --check` reports `app.css khớp nguồn`, then the user-run live checks (LIVE-CHECKS §8) before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `tests/fakes/fake_agy.py` (init/step_update/result, slow writes, error + exit codes, optional soft-deny SUCCESS with no edits)
- [ ] extend `fake_claude.py` recorder: record env var NAMES and boolean matches for `ANTHROPIC_BASE_URL`/`ANTHROPIC_AUTH_TOKEN`/key var, never values; scenario support for echoing a planted env value (for ENG-05)
- [ ] `Sandbox` helpers: `write_teams`, `write_staff`, config `engines` override, bare-remote + staff clone helper (`make_repo(.., Some(remote))` exists)
- [ ] fixtures: `tests/fixtures/stream/{claude_quota_*,deepseek_*,agy_*}.jsonl` (Rust-only, not `tests/contract/stream.json`)
- [ ] test files listed above; update `db_migration.rs` version assertions to 4
- [ ] `dashboard-js-check.mjs` `settings` section; `dashboard-smoke.sh` `settings` case (UI-SPEC Deltas row 10)

## Security Domain

`security_enforcement` is not disabled in config; section included.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (no new login) | existing per-start token + cookie (`dashboard/auth.rs`) |
| V3 Session Management | no | unchanged |
| V4 Access Control | yes | new routes inherit `auth::gate` (token, Host allow-list, Origin check) because they are merged into the one router; Telegram: strict callback grammar + paired chat only; engines not UI-editable (D-08) |
| V5 Input Validation | yes | server-side validators shared with loaders (slug rule, size caps, enum engines, QA rule), argv arrays (never a shell), branch name pattern `task/<slug>`, `--` before positional args |
| V6 Cryptography | no hand-rolled crypto | constant-time token compare exists (`ct_eq`); no key storage at all |
| V7 Logging / error handling | yes | redactor on every sink; 500 bodies fixed (existing `internal_error`); readiness payload never contains a key value, argv or endpoint query |
| V8 Data protection | yes | key only in process env; run dirs 0700, temp config files 0600 |
| V12 Files / config | yes | atomic writes, refuse to overwrite an invalid hand-edited file, size caps |

### Known Threat Patterns for this stack
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| DeepSeek key written to stream/events/stderr/job output | Information disclosure | redact exact value before the stream-file write; post-process `stderr.txt`; planted-key tests |
| Prompt-injected agent writes the token into `.plan.md` and it is committed | Information disclosure / Tampering | `secrets::scan_paths` before `commit_paths`; deny `Read(/proc/**)` as optional hardening; Dev/Lead allowlists unchanged |
| Key leaks to non-deepseek engines via inherited env | Information disclosure | strip key var for claude/agy; set `ANTHROPIC_*` only for deepseek after profile `unset_env` |
| Work account OAuth token sent to DeepSeek | Information disclosure | a gateway credential variable (`ANTHROPIC_AUTH_TOKEN`) takes precedence and the saved login is not sent with those requests [CITED: llm-gateway docs]; E-09 must confirm the DeepSeek run authenticated with the token (no auth conflict warning on stderr) and the profile dir is used only for config/settings |
| Settings write abuse (CSRF / LAN) | Tampering / Elevation | 127.0.0.1 + token + Host/Origin checks unchanged; writes never accept commands/endpoints |
| agy Dev with skip-permissions edits outside scope / secrets | Tampering / Elevation | post-run diff scope check + `sensitive_path` + `.planning/` rule; `permission_mode` assertion; empty-diff = suspicious; consider `--sandbox` |
| Malicious branch from "staff" remote | Tampering | ff-only merge, validated ref name, plan-scope check, code-review + QA gates (D-20) |
| Stale Telegram button executes against changed list/state | Tampering | `rev` check + `ver4` list token + confirm tap |
| Quota loop / silent switch | Denial of wallet | never auto-switch (D-16); quota runs do not auto-retry |

## Sources

### Primary (HIGH confidence)
- Codebase files read this session (paths cited inline): `agent-brain/src/{roles/mod.rs, roles/dev.rs, roles/lead.rs, roles/qa.rs, roles/review.rs, profile.rs, stream.rs, runner.rs, db.rs, config.rs, pricing.rs, stats.rs, workflows.rs, projects.rs, gitops.rs, tasks.rs, error.rs, app.rs, term.rs, guard.rs, main.rs, lib.rs, autopilot/{mod,decide,view,claim}.rs, dashboard/{mod,auth,jobs,assets,api_tasks,api_workflows,api_stats,taskstate,pipeline,sse}.rs, telegram/{callback,detect,render}.rs}`, `agent-brain/assets/{config.default.json,workflows.default.json}`, `agent-brain/Cargo.toml`, `agent-brain/tests/common/mod.rs`, `tests/fakes/fake_claude.py`, `tests/contract/stream.json`, `scripts/*`, `agent-brain/LIVE-CHECKS.md` headings
- `spikes/spike-results.md`, `spikes/s7/NOTES.md`, `spikes/s7/raw/*`, `spikes/s1/raw/s1.jsonl`, `spikes/lib/scan-secrets.sh`
- `.planning/{REQUIREMENTS,STATE,PROJECT,ROADMAP}.md`, `06-CONTEXT.md`, `06-UI-SPEC.md`, `05.4-02-PLAN.md`, `05.5-VALIDATION.md`, `CLAUDE.md`
- Local tool probes: `claude --version`, `agy --version`, `agy --help`, `git/node/python3/cargo/chrome --version`

### Secondary (MEDIUM confidence)
- https://code.claude.com/docs/en/headless (exit codes, SIGTERM 143, `system/api_retry` fields)
- https://code.claude.com/docs/en/agent-sdk/typescript (`SDKResultMessage`, `api_error_status`, `terminal_reason` values)
- https://code.claude.com/docs/en/llm-gateway and https://code.claude.com/docs/en/llm-gateway-connect (credential variables, precedence, settings-file env precedence, non-Claude-model support statement, troubleshooting knobs)
- https://code.claude.com/docs/en/errors (message strings; first 100k of 583k chars read)
- https://api-docs.deepseek.com/guides/anthropic_api, https://api-docs.deepseek.com/guides/coding_agents, https://api-docs.deepseek.com/quick_start/error_codes
- https://antigravity.google/docs/cli/headless (exit codes, status values, permission modes)

### Tertiary (LOW confidence)
- https://littlebearapps.com/help/untether/claude-stream-json-cheatsheet/ (community: `rate_limit_event` statuses, `api_error_code` examples)
- discuss.ai.google.dev threads on Antigravity 429 `RESOURCE_EXHAUSTED` (IDE-focused, community)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH, no new dependencies, versions read from `Cargo.toml` and local probes.
- Architecture / seams: HIGH, every seam opened and quoted this session.
- Quota shapes: LOW-MEDIUM, documented field names, assumed message values, no real rejected-quota capture in the repo.
- DeepSeek behaviour: MEDIUM-LOW until E-09 (docs only; Anthropic states non-Claude models are unsupported through gateways).
- agy tool/error behaviour: LOW-MEDIUM until the 1.4.7 probe.
- Pitfalls: HIGH for the code-derived ones (1-6, 9, 13-18), MEDIUM for external-behaviour ones.

**Research date:** 2026-10-26
**Valid until:** 2026-11-09 for CLI-behaviour claims (Claude Code and agy move fast: 2.1.300 -> 2.1.310 and 1.3.5 -> 1.4.7 already drifted since the spikes); code-seam claims hold until the next commit touching the cited files.

---

## Amendment Research: D-30 … D-36 (2026-10-27)

**Scope:** a targeted addendum for the re-planned `06-11` (user-defined engine list replacing the fixed DeepSeek engine) and the revised `06-19` (live check + docs). It does not redo research for the 17 executed plans. Every code citation below was read this session in the CURRENT tree (HEAD `20893cc`, after plans 06-01 … 06-10, 06-12 … 06-18 landed). Constraints honoured: no network call to any model endpoint, no `agy` prompt, no `antigravity`, Docker used read-only (`docker ps`, `docker inspect --format …`, `docker exec … dsh … --help`), no source/config/plan file changed.

**Confidence:** HIGH for the code seams, the docker mount facts and the spike scripts (all opened or executed read-only this session); MEDIUM for the gateway behaviour (`https://ai-gateway.vcadm.vn/proxyllm` could not be probed: no key, and network calls to model endpoints are forbidden); LOW for the quota shape of an unknown gateway and for dsh process behaviour not covered by S6.

**Primary recommendation:** model an engine as an `EngineDef { id, kind, … }` list loaded by `config.rs` (accepting both the new array and today's `{deepseek, agy}` objects), keep the argv of `anthropic` engines owned by `build_command_with_prompt` (never by a user `command` template: the example's template drops `--append-system-prompt`, `--settings` and the guard hooks), store the engine **id** in `runs.engine`, enforce `allowed_profiles` / `pin_profile` in ONE pure function called at the run seam (backstop) and at six bind points, run `dsh` as a marker-wrapped `docker exec … sh -c` black box that is killed inside the container, and split the work into `06-11` (catalog + `anthropic`), `06-20` (E-09 per endpoint tooling, user-run), `06-21` (key hygiene, old 06-11 Tasks 2-3), `06-22` (profile guard at bind points), `06-23` (dsh), `06-24` (Settings/Giao việc engine list UI), then the revised `06-19`.

### Key findings (read this first)

1. **No schema change.** `runs.engine` is `TEXT NOT NULL DEFAULT 'claude'` with no CHECK, so engine ids need no migration. [VERIFIED: agent-brain/src/db.rs:32 `engine TEXT NOT NULL DEFAULT 'claude', profile TEXT NOT NULL,`] The v4 dry run in 06-19 is unaffected.
2. **The user's real machine has no `config.json` at all** (`~/.config/agent-brain/` holds only `projects_map.json`), so "existing `engines.deepseek` / `engines.agy` objects must keep loading" is a compatibility requirement for the repo's own defaults (assets/config.default.json:12-33) and the test sandboxes that write the object shape (agent-brain/tests/engine_agy.rs:28 `sb.write_config(json!({"engines": {"agy": {"command": fake_agy().to_str().unwrap()}}}));`), not for a user file. No rewrite of any real config is needed or allowed. [VERIFIED: `ls -la ~/.config/agent-brain/` this session printed only `projects_map.json`]
3. **The example's `command` templates cannot drive argv.** The `anthropic` example template has no `--append-system-prompt`, no `--settings` (the PreToolUse guard hooks) and no `--disallowedTools`. Using it verbatim would silently remove the Phase 3 guard. [VERIFIED: docs/config.engines.example.json:8-22 versus agent-brain/src/roles/mod.rs:145-175, quoted in "Config shape" below]
4. **dsh sees the host tree at the same path.** The container has a read-write bind `/mnt/data` → `/mnt/data`; all 178 projects in `projects_map.json` (176 work, 2 personal) live under `/mnt/data/`. The same container also mounts `/var/run/docker.sock` and runs as root, so a dsh run can write anywhere under `/mnt/data` and files it writes are root-owned (S6). [VERIFIED: `docker inspect dsh-deepseek-harness --format '{{json .Mounts}}'` this session, quoted in "dsh kind"]
5. **Killing the `docker exec` client does not kill dsh.** S6 variant (a) left `node dsh`, its `bash -c` and `sleep` alive; only an in-container TERM of recorded PIDs worked. Therefore `runner::supervise` and `runner::cmd_kill` both need a container-side stop hook; `procs::kill_tree` alone is insufficient. [VERIFIED: spikes/spike-results.md S6 (4a)/(4b), quoted in "dsh kind"]
6. **`is_agy` is hard-coded in the Dev scope check** (`r.engine == crate::engine::ENGINE_AGY`), and `row_kind` / `term::event_text` map unparsable lines to error rows, so plain-text dsh output needs its own stream format, event kind and an `dsh:empty` outcome next to `agy:empty`. [VERIFIED: agent-brain/src/roles/dev.rs:578-580, agent-brain/src/dashboard/sse.rs:20-21]
7. **On a personal project `claude` already means `~/.claude-orca`**, so a pinned `claude-max` only differs by models / label; D-36 stays as decided but the user should know (Open Question Q4).
8. **E-09 tooling is hard-wired to `DEEPSEEK_API_KEY` and to one set of output names** (`e09-*`, `deepseek_e09_*.jsonl`, `rm -f "$RAW"/e09-*`), so probing a second endpoint would overwrite the first one's evidence; a key-variable override and an endpoint label are required. [VERIFIED: spikes/e09/run.sh:43,114,199-203]

### What D-30 … D-36 supersede, and what still holds

| Pre-amendment statement (06-RESEARCH / old 06-11) | Status | Replacement |
|---|---|---|
| D-01 "DeepSeek is `claude` + endpoint; dsh not used" | superseded | D-30/D-31: any number of `anthropic` entries (generalised D-01 mechanism); D-35: `dsh` is a third kind, black box |
| D-08 "engine definitions fixed in the binary" | partly superseded | engines are a user list in `config.json`; the UI is still read-only (no route writes a command, endpoint or key name) |
| `Engine::{Claude, Deepseek, Agy, Human}` and `ENGINE_ORDER` | superseded | `Engine::{Claude, Named(id), Human(staff)}`; order = `claude`, config order, `người` |
| Pitfall 5 (inject `ANTHROPIC_*` AFTER `profile::build_env`) | still holds, per entry | applies to every `anthropic` entry that has a `base_url` |
| Pitfall 6/7 (engine-first pricing, never bill an alias model as Anthropic) | still holds, generalised | keyed by engine id; native (no `base_url`) entries use the family lookup like `claude` |
| Pitfall 8 (a profile `settings.json` `env` block beats our env) | still holds | per endpoint, E-09 must run under the real profile |
| Pitfall 10 + old 06-11 Tasks 2-3 (redaction before every sink, post-run secret scan) | **unchanged** | secrets = the values of EVERY `key_env` variable used by the engine of the run; message wording says the engine id instead of `deepseek` |
| old 06-11 Task 1 (`deepseek` arm) | replaced | `anthropic` arm over `EngineDef` |
| D-02 / 06-01 outcome `fail` = "not run" | superseded by D-34 | evidence per endpoint (and per model), recorded next to the engine; the Rust work no longer waits for E-09 |
| D-06 (QA = `claude`) | holds | see Open Question Q4 for `claude-max` |
| D-07 (claude-family always uses the project profile) | revised by D-36 | only an entry with `pin_profile` deviates, and only where the project's profile equals it |

### Seam Map for D-30 (question 1)

Risk: H = breaks existing tests or safety if missed, M = visible bug, L = cosmetic.

| # | Where (file:lines, current tree) | What assumes the fixed set | Change for an `id` + `kind` list | Risk |
|---|---|---|---|---|
| S1 | agent-brain/src/engine.rs:11-16, 20-26 `pub const ENGINE_ORDER: [&str; 4] = ["claude", "deepseek", "agy", "người"];` and `pub enum Engine { Claude, Deepseek, Agy, Human(String) }` | closed enum | `Engine::{Claude, Named(String), Human(String)}`; `parse` = trim + NFC, `claude` \| `người:<staff id>` \| a valid engine-id slug; `as_db_str` returns `&str`/`String` (an id is dynamic); `ENGINE_ORDER` becomes a function of the catalog | H |
| S2 | engine.rs:293 `pub db_engine: &'static str,` and roles/mod.rs:486 `db_engine: &'static str,` | static engine names | `String` in `EnginePlan` and the Observer; `insert_run_with_engine(conn, r, engine: &str)` already takes `&str` (db.rs:327) | H |
| S3 | engine.rs:169-215 `readiness(cfg, env, staff_count)` hard-codes claude, `cfg.engines.deepseek`, `cfg.engines.agy`, human | four fixed rows | iterate the catalog: claude (built-in), each list entry by kind, người; add `kind`, `label`, `evidence` fields additively (the existing test asserts names `["claude", "deepseek", "agy", "người"]`, agent-brain/tests/dashboard_settings.rs:1018-1028, so the default catalog order must stay) | H |
| S4 | engine.rs:309-340 `plan_for` `match &resolved.engine { Claude, Agy => agy_plan, Deepseek => Err("Engine … chưa được bật …"), Human => Err }` | per-engine arms | dispatch on `EngineDef.kind`: `anthropic` arm (new), `agy` arm (existing body, parameterised by the entry), `dsh` arm (06-23) | H |
| S5 | engine.rs:415-421 `env.retain(|k, _| !(k.starts_with("ANTHROPIC_") \|\| k == "CLAUDE_CONFIG_DIR" \|\| … \|\| k == &cfg.engines.deepseek.key_env))` | strips only ONE key variable from agy's env | strip EVERY configured `key_env` (all entries) from claude, agy and dsh envs; each gateway arm strips the other entries' key variables | H (ENG-05) |
| S6 | config.rs:77-99 `DeepseekCfg`, `AgyCfg`, `EnginesCfg { deepseek, agy }`; :129-251 `parse_engines`; :271 `pub engines: EnginesCfg,`; :154-160 rejects any key but `deepseek`/`agy` with `"Cấu hình engines.{key} không hợp lệ (chỉ deepseek, agy)"` | fixed struct | `EnginesCfg { list: Vec<EngineDef> }` + `get(id)`, `kind_of(id)`, `all_key_envs()`; keep the legacy-shape error texts byte-identical (asserted at config.rs:971-1000) | H |
| S7 | assets/config.default.json:12-33 (`"engines": { "deepseek": {…}, "agy": {…} }`) and config.rs:913-932 (`engines_defaults` asserts `documented.engines == cfg.engines`) | documents the object shape | keep the object shape as the documented default (proves back-compat) OR switch to the array and keep a second legacy test; either way the equality test must stay | M |
| S8 | assets/teams.default.json (`"tiet-kiem"` → `"dev": "deepseek"`) | built-in team names the id `deepseek` | keep it: the default catalog still contains `deepseek`; when the user supplies an array without `deepseek` the built-in shows an "engine chưa khai báo" warning and the user overrides `tiet-kiem` in `teams.json` (already supported: source `override`, teams.rs:309-313) | M |
| S9 | teams.rs:204-233 `Engine::parse` in `validate_entry`; :214 error `"Engine không hợp lệ cho {role}: {shown} (dùng claude, deepseek, agy hoặc người:<mã>)"`; :220 `if role == "qa" && engine != Engine::Claude {`; :224 human only for dev | message names the fixed engines; QA = exact `Engine::Claude` | syntax-only validation at load (see "unknown engine id" below); new message text; QA rule unchanged (Q4) | H |
| S10 | teams.rs:54-87 `Team::summary` sorts `Engine::Claude→0, Engine::Deepseek→1, Engine::Agy→2, Human→3/4`; :131-140 `engine_text` uses `as_db_str` | fixed sort keys | sort claude, then catalog order (unknown ids last, by id), then people; `engine_text` prints the id | M |
| S11 | pricing.rs:38-47 `engine_price` (`claude`/`người` → `None`, else `table.get(engine)`), :60-70 `is_priced`, :77-89 `estimate_usd` (`r.engine == ENGINE_CLAUDE` → family lookup); roles/mod.rs:662-671 cost fallback `if ep.db_engine == crate::engine::ENGINE_CLAUDE { obs.summary.cost_usd } else { 0.0 }`; stats.rs:356 `is_priced(&engine, pricing)`; autopilot/decide.rs:390 and api_tasks.rs:2140 `budget_usage(.., pricing, ..)` | only the literal `claude` is "native" | "native" = `claude` or an `anthropic` entry without `base_url`; price gateway/agy/dsh entries by id; 4 external call sites + 3 in pricing.rs | H |
| S12 | roles/dev.rs:578-580 `.map(|r| r.engine == crate::engine::ENGINE_AGY)` gating the `.planning/` + secret-path checks (:582-593) and the empty-diff stop (:606-612) | only agy gets the diff-only safety net | gate on kind ∈ {agy, dsh} from `ctx.cfg.engines.kind_of(&r.engine)` | H |
| S13 | dashboard/api_settings.rs:811-826 `get_engines` → `engine::readiness(&st.cfg, …)`; teams payload `teams_json` (:154-230) shows each role's `to_config_string()` | fixed four | additive fields; warnings for unknown ids computed here (it has `st.cfg`) | M |
| S14 | assets/dashboard/js/settings.js:32-45 `ENGINE_ORDER`, `ENGINE_DESC`, `FIXED_ENGINES`; :883-920 `roleOptions` loops `FIXED_ENGINES`; :1536-1575 `engineRows` special-cases `name === 'deepseek'` for key/model rows; :618 QA rule `this.tQa !== 'claude'` | hard-coded names and descriptions | build rows and role options from `GET /api/engines` (server sends `description`, `kind`, `label`); keep `người` row synthesised | H (UI) |
| S15 | assets/dashboard/js/assign.js:5 `var ENGINE_ORDER = ['claude', 'deepseek', 'agy'];` and :218-240 readiness lines | fixed order for the "engine not ready" callout | iterate the order from the API; treat `chua_kiem` as a soft hint, not "bước này sẽ dừng với lỗi" (assign.js:757-760 text) | M |
| S16 | assets/dashboard/js/app.js:299-304 `engineText` (generic), js/stats.js:210-310 (generic over `runs.engine` strings), js/task.js:417-430 (prefix `người`) | none | no change | — |
| S17 | stats.rs:330-372 `query_by_engine` (`GROUP BY runs.engine`), :375-410 `query_qa_by_dev_engine` | generic strings | no change; a no-usage engine (dsh) needs a `usage` hint so "chưa có giá" is not shown for "no tokens" (api_stats.rs:43 is the only caller) | L |
| S18 | quota.rs:104 doc "claude and deepseek engines"; roles/mod.rs:621-647 dispatches on `StreamFormat::{Claude, Agy}`; :733 `user_message(&ep.display, &hit)` | two stream formats | `anthropic` reuses `detect_claude` unchanged; dsh adds `StreamFormat::Text` + a text detector (see "dsh kind") | M |
| S19 | autopilot/decide.rs:294, :935-945 and dashboard/taskstate.rs:110, autopilot/view.rs:347 treat outcome `agy:empty` | one empty-diff outcome per engine | add `dsh:empty` in the same four places (or a shared helper) | M |
| S20 | runner.rs:162-172 `RunRequest { argv, cwd, env, stderr_path, stream_path, timeout, kill_grace, stdout_file }`; kill calls at runner.rs:122, 196, 405, 422, 436, 446 (`crate::procs::kill_tree`) and `cmd_kill` (:55-160) | host-process-tree kill only | optional `stop_hook` + container spec persisted in `runs.metrics_json.dsh`; `cmd_kill` reads `RunRow.metrics_json` (db.rs `RunRow` has `metrics_json`) | H (process safety) |
| S21 | Team bind points (all create or change a task's team): teams.rs:377-418 `resolve`; roles/lead.rs:99-110 (CLI `run lead --team`) and :337-350; dashboard/api_tasks.rs:625-735 `start_new_task` (Giao việc + Telegram `/task`, intake.rs:597, 1163, 1172); autopilot/mod.rs:614-625 (`auto --start --team`, resolves with `None` as project default); api_tasks.rs:1403 `change_team`; :1715 `quota_rerun` (dashboard + Telegram `on_team` Confirm, telegram/bot.rs:1224); api_settings.rs:355 `save_team_blocking` | none know the project profile except `start_new_task` and `change_team`/`quota_rerun` via `state.project` | one pure `team_profile_refusal(team, catalog, project)` called at each | H (D-36) |
| S22 | telegram/intake.rs:1033-1037 (`/task` team list = every team name) and render::team_names (quota picker) | lists teams the project cannot use | optionally skip/mark unfit teams to avoid dead buttons; the shared actions already refuse | L |
| S23 | tests: config.rs:913-1012, engine.rs `plan_for_*` / `readiness_states` / `engine_parse`, teams.rs:517-548, stats.rs:612-640, pipeline.rs:4573-4697, decide.rs:1481-1543, tests/{engine_routing,engine_agy,engine_quota,dashboard_settings,telegram_quota}.rs | literal `deepseek` / four-engine order | update where the enum changes; keep legacy-shape tests as the back-compat proof | M |

### Config Shape and Backward Compatibility (question 2)

#### Recommended file shape

`engines` is an array. The built-ins `claude` and `người` are implicit (not entries). Absent `engines` = the two historical defaults (`deepseek`, `agy`), so every existing sandbox, the built-in `tiet-kiem` team and `config.default.json` keep working. A user array is used exactly as written (no merge with the defaults).

```json
"engines": [
  {"id": "deepseek", "kind": "anthropic", "label": "DeepSeek (endpoint chính thức)",
   "command": "claude", "base_url": "https://api.deepseek.com/anthropic", "key_env": "DEEPSEEK_API_KEY",
   "models": {"lead": "opus", "dev": "sonnet", "qa": "sonnet", "plan-review": "opus", "code-review": "opus"}},
  {"id": "vcadm-deepseek", "kind": "anthropic", "label": "DeepSeek qua gateway công ty",
   "base_url": "https://ai-gateway.vcadm.vn/proxyllm", "key_env": "VCADM_GATEWAY_KEY",
   "models": {"lead": "deepseek/deepseek-reasoner", "dev": "deepseek/deepseek-chat", "qa": "deepseek/deepseek-chat"}},
  {"id": "claude-max", "kind": "anthropic", "label": "Claude Max cá nhân", "pin_profile": "personal",
   "models": {"lead": "opus", "dev": "opus", "qa": "sonnet"}},
  {"id": "agy", "kind": "agy", "command": "agy", "print_timeout_margin_secs": 30, "dev_permission": "accept-edits", "model": null},
  {"id": "dsh", "kind": "dsh", "docker": "docker", "container": "dsh-deepseek-harness", "dsh_profile": "headless"}
]
```

Field rules (planner copies these into validators; every error is Vietnamese in the existing `Cấu hình …` style, see the error table):

| Field | Kinds | Rule |
|---|---|---|
| `id` | all | required; `crate::workflows::validate_name` (lowercase ascii, digits, `-`, starts alnum) AND ≤ 32 bytes (`staff::MAX_ID_BYTES`, staff.rs:8); not `claude`, not `người` in any Unicode normalisation (reserved, D-30), cannot contain `:` (guaranteed by the charset); unique across the list |
| `kind` | all | required; `anthropic` \| `agy` \| `dsh` (D-31, D-35). An OpenAI-format kind is deferred |
| `label` | all | optional, one line, ≤ 120 chars, shown on the Settings Engine row; default per kind. Rendered with `x-text` (no HTML) |
| `allowed_profiles` | all | optional non-empty array of names that exist in `profiles`; empty/unknown → config error. The generic form of D-36 |
| `pricing` | anthropic, agy | optional `{input, output, cache_read, cache_write}` non-negative numbers (the `Price` struct, config.rs:30-36); rejected for `dsh` (no usage) |
| `command` | anthropic, agy | executable only: a string or a one-element array, default `claude_bin` (anthropic) / `agy` (agy). Extra array elements are a config error (see divergence D1) |
| `base_url` | anthropic | optional; when present it must start with `https://` (existing rule, config.rs:176-185); absent = native Anthropic via the profile's login |
| `key_env` | anthropic | required iff `base_url` is present, forbidden otherwise; `^[A-Z_][A-Z0-9_]*$` (existing `valid_key_env`, config.rs:120-127); NAME only (D-32) |
| `models` | anthropic | role → non-empty string, roles ∈ `teams::ROLES`; missing role → `models.dev` → config model (native) / config error "thiếu model cho vai trò" at readiness (gateway with no `models`) |
| `pin_profile` | anthropic without `base_url` | optional; must exist in `profiles`; implies `allowed_profiles = [pin_profile]`; the run env is `profile::build_env(pin_profile, …)` |
| `compat` | anthropic with `base_url` | optional array ⊆ {`disable_experimental_betas`, `disable_adaptive_thinking`}: the two knobs of old 06-11 that E-09 may require |
| `verified` | anthropic with `base_url` | optional short string (date + note) the user pastes after E-09 passes for that endpoint; absent → readiness `chưa kiểm` (see "E-09 per endpoint") |
| `print_timeout_margin_secs`, `model` | agy | unchanged rules (config.rs:232-247) |
| `dev_permission` | agy | enum, default `accept-edits` (the 06-02 decision, engine.rs:367 `AGY_DEV_PERMISSION: &[&str] = &["--mode", "accept-edits"]`); no free-form extra args (divergence D2) |
| `docker`, `container`, `dsh_profile` | dsh | `docker` default `docker` (string); `container` required, `^[A-Za-z0-9][A-Za-z0-9_.-]*$`, ≤ 64; `dsh_profile` default `headless`, slug (D-35) |

**Divergences between `docs/config.engines.example.json` and the recommendation (the user's file is "intent"; confirm in Q1-Q3):**

| # | Example | Recommendation | Why |
|---|---|---|---|
| D1 | `command` is a full argv template with `{prompt}`, `{model}`, `{max_turns}`, `{allowed_tools}` | `command` = executable only for `anthropic`/`agy`; argv stays with the role builders | the template omits `--append-system-prompt`, `--settings` (guard hooks), `--disallowedTools`; `--allowedTools` is multi-valued and `--max-turns` is optional. [VERIFIED: agent-brain/src/roles/mod.rs:145-175 builds `claude -p <input> --output-format stream-json --verbose --model <m> --append-system-prompt <prompt>` then `common_args`, optional `--max-turns`, `extra_args` (= `--settings <file>`), `--allowedTools …`, `--disallowedTools …`] |
| D2 | agy `dev_extra_args: ["--mode","accept-edits"]`; agy `["timeout","{outer_timeout}","agy",…]` | enum `dev_permission`; no `timeout` wrapper | D-30 itself says `dev_permission`; free args could add `--dangerously-skip-permissions` (the repo forbids that flag for claude: roles/mod.rs:94-107 `check_forbidden_args`); the runner already enforces the hard timeout and passes `--print-timeout` (engine.rs:345-351) |
| D3 | `env` maps with `"{base_url}"` and `"${VCADM_GATEWAY_KEY}"` | derive env from `base_url` + `key_env`; no user `env` map | the map restates two fields; `${VAR}` expansion would let a config read ANY env variable into the child. Whitelisted tuning goes through `compat` |
| D4 | `claude-max`: `env.CLAUDE_CONFIG_DIR: "~/.claude-orca"` + `allowed_profiles: ["personal"]` | `pin_profile: "personal"` | reuses `profiles.personal.claude_config_dir` and its `unset_env` through `build_env`, one source of truth for the directory |
| D5 | an entry with `id: "claude"` | not an entry (reserved built-in) OR accept it as an override of label/models only | the built-in is the project-profile engine; two definitions of `claude` can disagree (Q1) |
| D6 | dsh `command` template containing the container name, `{profile}` placeholder | discrete `docker`, `container`, `dsh_profile` fields; agent-brain builds the argv | the kill hook needs the container name and a marker it controls; `{profile}` is ambiguous with the Claude profile (work/personal) |
| D7 | `stdout: "file"`/`"text"` | implied by kind | agy is always file-tailed (CLAUDE.md), dsh is always text |

#### Both shapes, defaults and migration

`parse_engines` (config.rs:129-251) becomes a dispatcher on the JSON type: `Value::Array` → new loader; `Value::Object` → today's loader, producing the same `EngineDef`s (`deepseek` → `{id: "deepseek", kind: anthropic, command: claude_bin default, base_url, key_env, models}`; `agy` → `{id: "agy", kind: agy, …}`), with the legacy error strings unchanged; anything else keeps `"Cấu hình engines không hợp lệ (phải là JSON object)"` extended to `… (phải là mảng hoặc JSON object)`. **Accept both; do not migrate or rewrite any file** (reversible, and there is no user config to migrate). [VERIFIED: config.rs:144-160 quoted: `Some(serde_json::Value::Object(o)) => o,` … `if key != "deepseek" && key != "agy" {`] Because `load_config` replaces the whole default when a `config.json` exists (config.rs:593-602, no overlay), the README must show the minimal `config.json` that needs `models` and `profiles` plus `engines` (`config_from_value` requires `models.{lead,dev,qa}` and both profiles, config.rs:354-380).

#### Validation errors (Vietnamese, existing style `Cấu hình engines.<path> …`)

Use `engines[{i}]` until the id is valid, then `engines.{id}`. Proposed texts (planner may reword, tests assert them verbatim once chosen):

| Case | Message |
|---|---|
| not array/object | `Cấu hình engines không hợp lệ (phải là mảng hoặc JSON object)` |
| entry not an object | `Cấu hình engines[{i}] không hợp lệ (phải là JSON object)` |
| bad id | `Cấu hình engines[{i}].id không hợp lệ (chữ thường không dấu, số và -, tối đa 32 ký tự)` |
| reserved id | `Cấu hình engines[{i}].id '{id}' là engine có sẵn, không khai báo lại` |
| duplicate id | `Cấu hình engines.{id} bị khai báo hai lần (engines[{j}] và engines[{i}])` |
| unknown kind | `Cấu hình engines.{id}.kind không hợp lệ (chỉ anthropic, agy, dsh)` |
| base_url scheme | `Cấu hình engines.{id}.base_url phải bắt đầu bằng https://` |
| key_env missing/misplaced | `Cấu hình engines.{id}.key_env bắt buộc khi có base_url` / `Cấu hình engines.{id}.key_env chỉ dùng cùng base_url` |
| key_env charset | `Cấu hình engines.{id}.key_env không hợp lệ (chỉ chữ hoa, số và _)` |
| models role | `Cấu hình engines.{id}.models.{role} không hợp lệ` |
| command extra args | `Cấu hình engines.{id}.command chỉ nhận tên lệnh, không nhận tham số` |
| allowed_profiles / pin | `Cấu hình engines.{id}.allowed_profiles không hợp lệ (danh sách tên profile có trong profiles)` / `Cấu hình engines.{id}.pin_profile '{p}' không có trong profiles` |
| pin with base_url | `Cấu hình engines.{id}.pin_profile chỉ dùng cho engine không có base_url` |
| dsh container | `Cấu hình engines.{id}.container không được rỗng` |
| unknown field | `Cấu hình engines.{id}: khóa lạ {k}` (strict, like teams.json `Khóa lạ`; the legacy object loader keeps ignoring unknown sub-keys, config.rs tests) |

#### A team that names an engine id missing from the list: tolerant at load, strict at save and at run

Teams are hand-edited files that must keep loading when `config.json` changes, and `teams::load*` takes no config (≈12 call sites, listed in S21/S9). Recommended:

- **Load** (`teams::validate_entry`): syntax only (`claude`, `người:<staff id>`, or a valid id). Unknown ids load; `teams_json` (which has `st.cfg`) adds the existing `warnings` line `Engine '{id}' chưa khai báo trong config.json.`
- **Settings save** (`save_team_blocking`): refuse with a 422 field error `Engine '{id}' không có trong config.json (engines).` (strict, so the UI cannot create a broken team).
- **Run seam** (`engine::resolve_for_run` / `plan_for`): refuse before any run row, exit 1, same shape as the missing-team refusal (verified test `missing_team_refused_before_run`, agent-brain/tests/engine_routing.rs:486-508): `Engine '{id}' của đội hình '{team}' không có trong config.json (engines) — thêm engine hoặc đổi đội hình rồi chạy lại.` Autopilot already turns a child with no new run into a `run lỗi` stop with the last stderr line (autopilot/mod.rs:497 `"Bước {} không tạo run nào"`).
- Built-in `tiet-kiem` keeps `deepseek`; a user list without `deepseek` overrides the team in `teams.json` (documented in README, 06-19).

#### `runs.engine` stores the engine id; pricing follows the id

D-23 needs "tokens per engine". Two gateway endpoints of the same kind must not mix, so the stored value is the **id**, not the kind. Historical rows (`claude`, `agy`, `người`) are already valid ids. [VERIFIED: stats.rs:330-372 `GROUP BY runs.engine`, generic over strings] Consequence (one-way-ish): renaming an id in `config.json` orphans its old rows (shown with the old name, unpriced). Pricing: per-entry `pricing` is merged under the entry id; to avoid distorting `price_for`'s "most expensive family" fallback (pricing.rs:8-37 iterates every table value), keep per-engine prices in a separate map (`cfg.engine_pricing`) and thread it to the four external call sites (stats.rs:356, roles/mod.rs:662, decide.rs:390, api_tasks.rs:2140). Native entries (`claude`, `claude-max`) price by model family like `claude` (cost from the result, roles/mod.rs:667 generalised).

### The `anthropic` kind (question 3: D-31, D-32, D-34)

What generalises from old 06-11, per entry that has a `base_url`:

| Aspect | Old (deepseek) | New (any `anthropic` entry with `base_url`) |
|---|---|---|
| argv | `claude_argv` with argv[0] swapped and `--model` replaced | identical: `argv[0] = def.command`; value after `--model` = `def.models[role]` → `models.dev` → error; everything else (`--append-system-prompt`, `--settings`, `--allowedTools`, `--max-turns`) untouched |
| env | `build_env(project profile)` then insert `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`; remove the key variable and `ANTHROPIC_MODEL`, `ANTHROPIC_DEFAULT_{OPUS,SONNET,HAIKU}_MODEL`, `CLAUDE_CODE_SUBAGENT_MODEL` | same, using `def.base_url` and the value of `def.key_env`; ALSO remove every other entry's `key_env`; `compat` adds `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1` / `CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING=1` |
| missing key | `Chưa đặt biến môi trường {KEY_ENV} cho engine deepseek …` | same text with the engine id: `Chưa đặt biến môi trường {KEY_ENV} cho engine {id} — đặt biến này trong shell chạy agent-brain (serve hoặc CLI) rồi chạy lại.`; refused before any run row |
| redaction / post-run scan | `redact = [key]` | `redact = [key]` of THAT run's engine only; unchanged Tasks 2-3 of old 06-11 (runner `RunRequest.redact`, `redact_line`, `redact_file`; `scan_for_secrets` before commit; `metrics_json.secret_leak`) |
| quota | `quota::detect_claude` | unchanged: it keys on the claude stream, not on the endpoint (quota.rs:104-160). A gateway's own budget error is an unknown shape: LiteLLM documents `budget_exceeded` with code 429 in one place and 400 in another, `error.code` possibly a string [CITED: https://docs.litellm.ai/docs/proxy/users and https://docs.litellm.ai/docs/proxy/error_reference.md, MEDIUM; whether `proxyllm` is LiteLLM is [ASSUMED]]. `detect_claude` already stops on HTTP 429/402 and on the phrases in `text_regex` (quota.rs:27-35); consider adding `budget` / `exceeded budget` to that regex, tagged assumed, and let the first real stop recalibrate (raw lines stay in `events`) |
| pricing | `pricing.deepseek` | per entry id; cost 0 and "chưa có giá" when absent; `total_cost_usd` of a non-Claude model is a client-side estimate and is not trusted (Pitfall 7) |
| model per role | five aliases | per entry; roles not listed fall back to `models.dev`; the example gives only lead/dev/qa for the gateway, so plan-review / code-review use the dev model unless the user lists them |

**Native `anthropic` entries (no `base_url`)**: no env injection, no key, no redaction; env = `build_env(pin_profile or project profile)`. This is how `claude-max` is expressed.

**Unchanged from old 06-11, reuse the plan text**: Task 2 (redaction before every sink including kill/timeout and `stderr.txt` rewrite) and Task 3 (post-run scan of changed + committed paths before the role commits; outcome `protocol_violation`, exit 3), the planted-key test idea (`test-planted-…-0123456789`), the fake-claude additions (`env_names`, `anthropic_base_url`, `auth_token_matches_expected`, `echo_env` op) and the acceptance greps, with `deepseek` wording replaced by the engine id and `DEEPSEEK_API_KEY` by per-entry key variables. Two keys in one process need `FAKE_CLAUDE_EXPECT_TOKEN` per call (set it per `Sandbox::run_env` invocation).

**Mechanism caveat that still applies (Pitfall 8):** a profile `settings.json` `env` block pinning `ANTHROPIC_*` overrides what agent-brain injects; E-09 is the only detector. [CITED: https://code.claude.com/docs/en/llm-gateway-connect, via the 2026-10-26 research above]

### The `dsh` kind (question 4: D-35)

#### Facts established read-only this session

- Container: `dsh-deepseek-harness` is up and has not been touched. [VERIFIED: `docker ps -a --format '{{.Names}} | {{.Status}} | {{.Image}}'` printed `dsh-deepseek-harness | Up 8 days | dsh-base:latest`]
- Mounts, **same-path bind of the whole `/mnt/data`**, read-write; plus a data volume and the Docker socket. [VERIFIED: `docker inspect dsh-deepseek-harness --format '{{json .Mounts}}'` printed `[{"Type":"bind","Source":"/mnt/data","Destination":"/mnt/data","Mode":"rw","RW":true,"Propagation":"rprivate"},{"Type":"volume","Name":"docker_dsh_data","Source":"/var/lib/docker/volumes/docker_dsh_data/_data","Destination":"/root/.dsh","Driver":"local","Mode":"rw","RW":true,"Propagation":""},{"Type":"bind","Source":"/var/run/docker.sock","Destination":"/var/run/docker.sock","Mode":"rw","RW":true,"Propagation":"rprivate"}]`] The config part printed `user= workdir=/mnt/data state=running restart=unless-stopped` (empty user = root). `docker inspect` was always called with `--format`: a bare `docker inspect` prints `Config.Env`, which holds the container's secrets (S6 deliberately never ran it, spikes/s6/run.sh:5-6).
- Every project path in the user's map is under `/mnt/data/` (178 of 178; profiles 176 work, 2 personal; no project has a `team` key). [VERIFIED: python read of `~/.config/agent-brain/projects_map.json` this session]
- `dsh` interface. [VERIFIED: `docker exec dsh-deepseek-harness dsh --profile headless --help` printed `Usage: dsh --profile headless [options] [task...]` / `Answer one task, print the final assistant message, and exit.` / `task        the task text; multiple words are joined by spaces` / `-h, --help  show this help`]; `dsh --help` lists `--profile <name>`, `web`, `plugin` and no output-format, turn-limit or session flag for headless.
- Black-box facts from S6 (user-run, earlier): plain text final message only, no tokens, no `tool_use`; files written by dsh are `uid:gid=0:0` while the host user is `1000:1000`; killing the host `docker exec` client leaves `node dsh`, `bash -c` and `sleep` alive; TERM to recorded in-container PIDs leaves `final: none`; the launcher exit code was 0 even when killed (so it cannot signal a kill). [VERIFIED: spikes/spike-results.md S6 (2), (3), (4a), (4b)]

#### Argv and which parts are per-engine fields

D-35's argv with the marker implementation of "marker/PID file per spikes/s6/run.sh" (S6 variant (b) wrote `$$` to a PID file; the marker form below keeps a findable `sh` parent):

```text
{docker} exec -w {cwd} {container} sh -c ': {marker}; dsh --profile "$1" "$2"' sh {dsh_profile} {prompt}
stdin = /dev/null (runner Stdio::null), stdout = pipe (text lines), stderr = <run_dir>/stderr.txt, cwd (host) = project root
marker = ab-dsh-<12 hex> generated per run; the prompt travels as a positional parameter ($2), never interpolated into the shell text
```

Per-engine fields: `docker`, `container`, `dsh_profile`. Derived per run: `{cwd}` (the project root, canonical), `{marker}`, `{prompt}` (built exactly like the agy prompt: role prompt + `\n\n---\n` + user input via `claude_argv_parts`, engine.rs:437-446). Not fields: the wrapper script text. Whether `sh` runs the last command directly (shell exec optimisation) is not assumed: kill never relies on the process shape, it records the marker's subtree first (S6 "design must not rely on the tree shape").

#### Plain-text output, usage and outcome

- New `StreamFormat::Text` (engine.rs:282-286 has `Claude`, `Agy`). `classify` turns a non-JSON line into `bad_json`, which `row_kind` renders as an error row and `term::event_text` prints as `dòng JSON lỗi` (sse.rs:20-21, term.rs:26-30), so the Observer must convert each text line into a JSON object before `insert_event` (e.g. `{"event":"dsh/text","text":"…"}`, kind `dsh/text`) while `stream.jsonl` keeps the raw text. `Summary` in text mode: `usage_source = none` (the column allows it: [VERIFIED: agent-brain/src/db.rs:40 `usage_source TEXT NOT NULL DEFAULT 'none' CHECK (usage_source IN ('result','partial','none')),`]), tokens 0, `result_text` = tail of stdout (cap 8 KiB).
- `runs.model` / header label: `headless` (the dsh profile); header `· engine dsh · model headless`; `runs.cost_usd = 0`; stats row `priced: false` must not read "chưa có giá" for "no tokens": add a `usage: "none"` hint in the engines payload (S17).
- Outcome: runner status from the exit code of `docker exec` (= the in-container command's status; not trusted for kills, S6). Dev success → the existing diff scope check and commit path. **Reuse of the agy Dev path**: `is_agy` becomes `kind ∈ {agy, dsh}` (dev.rs:578-580), so `.planning/` and `report::sensitive_path` paths are violations even when the plan lists them (dev.rs:582-593) and an empty diff after exit 0 stops with a new outcome `dsh:empty` (agy's is `agy:empty`, dev.rs:606-612; add `dsh:empty` wherever `agy:empty` appears: S19).
- Quota: no JSON; `quota::detect_text(exit_code, stdout_tail, stderr_tail)` applies the existing `text_matches` only when the exit code is non-zero (an exit-0 "quota" message is indistinguishable from an answer; the empty-diff stop catches the no-work case). Shapes are [ASSUMED]; fixtures come from the first real stop.

#### Timeout and kill

- Timeout = `cfg.timeout_for(role)` (runner `RunRequest.timeout`); no `timeout` wrapper, and dsh has no turn flag, so time is the only bound (like agy).
- **Stop hook.** `RunRequest` gains an optional stop hook (runner.rs:162-172 today has none); `supervise_async` calls it at the four stop sites that now call `kill_tree` (SIGINT, SIGTERM, SIGHUP, timeout: runner.rs:405, 422, 436, 446) **before** the host `kill_tree` of the `docker exec` client, and `cmd_kill` (runner.rs:55-160, reached from CLI, dashboard `api_runs.rs:120` and Telegram `bot.rs:874`) calls the same function using `RunRow.metrics_json.dsh = {docker, container, marker}` written by the Observer at spawn (the pid recorded in `runs` is the docker client, so the container spec must be persisted: the kill may run in another process). Hook algorithm (S4 + S6): `docker exec C ps -ww -eo pid=,ppid=,pgid=,args=` → pure function `marker_subtree(snapshot, marker, before)` (a port of the `tree` Python in spikes/s6/run.sh:56-90) → record → `docker exec C kill -TERM <pids>` → poll up to `kill_grace` → re-snapshot → `kill -KILL` the survivors by PID → final snapshot → report; then `docker exec C rm -f -- <pidfile>` if one is used. PIDs are signalled only after being re-matched against the marker in a fresh snapshot (guards in-container PID reuse). Never `pkill`/`killall`/by name; never `docker restart|stop|kill|rm`.
- Residual risk to document: SIGKILL of agent-brain itself (`JobManager::kill_all_now`, dashboard/jobs.rs:389-396, and `shutdown`'s final `kill_tree` at :385) skips the hook and leaves the container process; the next dsh run's preflight can list leftover `ab-dsh-*` markers and warn. [ASSUMED: acceptable; a user-run kill probe (see 06-20) must confirm the marker form on the real image before enabling dsh for real tasks.]

#### Readiness (local, read-only) and the run-time preflight

- `docker` found by `find_command` (engine.rs:120-134) → else `thiếu lệnh` / `Không tìm thấy lệnh docker trên máy.` Container up: `docker ps --filter name=^{container}$ --format {{.Status}}` (the exact read-only form S6 uses, spikes/s6/run.sh:32) with a 3 s timeout, injected as a probe closure so unit tests never need Docker → `Up…` = `sẵn sàng`; otherwise `thiếu lệnh` with reason `Container {name} chưa chạy.` (no new vocabulary; a fifth word is the alternative, Q-optional).
- **Run-time preflight (before the run row):** `docker inspect {container} --format '{{json .Mounts}}'`; require a bind mount with `Source == Destination`, `RW == true` and `cwd.starts_with(Destination)`; else refuse with `Dự án '{id}' ở {cwd} không nằm trong thư mục mà container {container} thấy cùng đường dẫn — engine dsh không chạy được trên dự án này.` Today `/mnt/data` covers all 178 projects.

#### Tests without Docker (fake shims)

`engines[].docker` points at `tests/fakes/fake_docker.py`, a stdlib shim that implements only: `ps --filter name=^C$ --format {{.Status}}` (prints `Up 1 minute`), `inspect C --format {{json .Mounts}}` (prints a mount list chosen by `FAKE_DOCKER_MOUNTS`), and `exec [-w DIR] [-u U] C CMD…` which `chdir`s and `exec`s `CMD` locally, so the real marker wrapper, `ps` and `kill -TERM <pid>` run against local processes. A second shim `tests/fakes/fake_dsh.py` (put first on `PATH` as `dsh`) follows the `fake_agy.py` scenario style (`FAKE_DSH_SCENARIO` ops `write`, `output`, `exit_code`, `sleep_s`, `spawn_child`; `FAKE_DSH_RECORD` argv/cwd/env names). The kill test reads the marker from `runs.metrics_json.dsh.marker`, kills through `agent-brain kill`, and asserts no process whose args contain the marker survives; the shim and tests never use `pkill`/`killall` (acceptance grep).

#### Hazards that need a user decision or a user-run probe

1. **Blast radius.** The container is root with rw `/mnt/data` and the Docker socket, so a dsh run can modify every project under `/mnt/data`, not just the task's repo; the post-run diff check only sees the task's repo. Mitigation available now: `allowed_profiles: ["personal"]` on the `dsh` entry (generic field). Q3.
2. **Root-owned files in work repos.** Files dsh edits become `0:0` (S6 (3)); later in-place edits by the host user fail (unlink/rename still works in a user-owned directory). Untested: `docker exec -u 1000:1000` (would the profile in `/root/.dsh` still be readable?) [ASSUMED unknown]; git run by root inside a user-owned repo may be refused by git's ownership check (also unknown). The first real dsh task must run on a scratch/personal repo.
3. **Roles.** dsh has no read-only mode (agy gets `request-review` soft-deny for Lead/reviewers; dsh's headless profile edits files). Recommend `dsh` = Dev only in this phase, mirroring D-24 for `người`; Q2.

### `claude-max` pinned profile guard (question 5: D-36)

**Where a pinned directory enters env construction.** Today `execute_role_staged` builds `base_env = profile::build_env(&project.profile, &ctx.cfg.profiles, &current_env())` BEFORE the engine is known to `plan_for` (roles/mod.rs:431-444, signature `plan_for(cfg, resolved, role, claude_argv, claude_model, base_env, timeout)`, engine.rs:309-317). Recommended: pass `project_profile` and the raw current env into `plan_for`; a native entry with `pin_profile` calls `build_env(pin_profile, …)` itself (so `unset_env` and `CLAUDE_CONFIG_DIR` come from the pinned profile exactly as for a project of that profile), and `profile_refusal(def, project.profile)` runs first. [VERIFIED: profile.rs:67-105 `build_env` removes `NESTED_SESSION_VARS`, the profile's `unset_env`, then sets `CLAUDE_CONFIG_DIR` from `claude_config_dir` or removes it]

**How a project's profile is determined today.** `Project.profile` read from `projects_map.json` (`profile` key; `check_profile` refuses empty or unknown, profile.rs:19-32 `"Dự án chưa đặt profile (work|personal) — sửa projects_map.json rồi chạy lại"`); SPK-10 maps work → `~/.claude` (company Team seat) and personal → `~/.claude-orca`. [VERIFIED: spikes/spike-results.md SPK-10; config default `profiles.personal.claude_config_dir = "~/.claude-orca"`, assets/config.default.json] The user's map has 176 work and 2 personal projects.

**Enforcement points** (one pure function each, all returning the same Vietnamese text):

| # | Path | Where | Notes |
|---|---|---|---|
| 1 | run seam (backstop; covers autopilot children, CLI `run dev|qa|review`, a team edited by hand, an engine whose `allowed_profiles` changed later) | roles/mod.rs `execute_role_staged` after `resolve_for_run`, before `plan_for` | refusal = exit 1, no run row (like the missing-team refusal); autopilot then stops `run lỗi` with the message (autopilot/mod.rs:497) |
| 2 | project default `team` in `projects_map.json` | `teams::resolve` callers: lead.rs:101, api_tasks.rs:670, api_settings.rs:166 and :188 | a work project whose default team is unfit: refuse the new task with the text, and show it as the existing `note`/warning in `project_defaults` (api_settings.rs:165-190) |
| 3 | Giao việc picker (`POST /api/tasks {team}`) | `start_new_task` (api_tasks.rs:625-735), `req.team` branch | `StartOutcome::Invalid(message)` (422), before any job |
| 4 | Telegram `/task` | goes through `start_new_task` (intake.rs:597, 1163, 1172) | free; optionally hide unfit teams in the picker (intake.rs:1033-1037) |
| 5 | CLI `run lead --team` | lead.rs:99-110 | refuse before branch/task row |
| 6 | `auto --start --team` | autopilot/mod.rs:614-625 | currently `resolve(.., Some(requested), None)`; needs the project profile |
| 7 | change team on a task | `api_tasks::change_team` (:1403), after the `state.teams.get(team)` check; `state.project.profile` is available (TaskState.project, :1015) | `TeamOutcome::Invalid`/`Refused` |
| 8 | quota re-run (dashboard + Telegram confirm) | `api_tasks::quota_rerun` (:1715), after `state.teams.get(team)` and BEFORE `write_team_change` | otherwise the team would change for nothing |
| 9 | Settings team save | `save_team_blocking` (api_settings.rs:355) | refuse when any project whose map `team` is this team has a profile the team's engines forbid, or any open task of such a project uses the team (`open_task_count`, api_settings.rs:144 is the model) |
| 10 | Settings engines change | not a UI action (config is file-edited) | seam backstop (1) covers it |

**Text style** (existing precedents: `"Profile personal đang có run {} chạy …"`, roles/mod.rs:402; `"Đội hình '{}' của task không còn trong teams.json — đổi đội hình rồi chạy lại."`, engine.rs:254-262): `Engine '{id}' chỉ dùng cho dự án profile {allowed} — dự án '{project}' dùng profile {profile} (ranh giới work/personal, SPK-10). Đổi engine của vai trò {role} hoặc chọn đội hình khác.` Settings save variant names the project or the number of open tasks.

**Related facts.** E-06 (no two personal runs in parallel) keys on `runs.profile == project.profile` (roles/mod.rs:388-411) and stays correct: a pinned run only happens on a project of that profile. A gateway engine on a personal project also records profile `personal`, which is conservative (it still uses the project's `CLAUDE_CONFIG_DIR`).

**QA + D-06.** The validator is a strict equality (`engine != Engine::Claude`, teams.rs:220). D-36 does not say whether QA on `claude-max` satisfies "QA must use a strong engine (`claude`)". On a personal project `claude` already resolves to the same `~/.claude-orca`, so the strict rule costs nothing; relaxing it to "any native `anthropic` entry" would need the catalog inside `validate_entry`. Recorded as Open Question Q4 instead of decided.

### E-09 per endpoint (question 6: D-34)

**What `spikes/e09/run.sh` needs** (all verified by reading the script):

| Gap | Evidence | Needed change |
|---|---|---|
| key variable hard-wired | run.sh:43 `KEY="${DEEPSEEK_API_KEY:-}"`, :114 `e09_call "$OUT/e09-$name" DEEPSEEK_API_KEY …`; verify.sh:17 `KEY="${DEEPSEEK_API_KEY:-}"` | `E09_KEY_ENV` (default `DEEPSEEK_API_KEY`) read in both scripts; the gateway run is `E09_KEY_ENV=VCADM_GATEWAY_KEY E09_BASE_URL=https://ai-gateway.vcadm.vn/proxyllm E09_MODEL=deepseek/deepseek-chat bash spikes/e09/run.sh` |
| single output namespace | `rm -f "$RAW"/e09-*` and fixtures `deepseek_e09_ok.jsonl` / `deepseek_e09_auth_error.jsonl` (run.sh:199-203) | `E09_NAME` label (default `deepseek`): files `e09-<name>-*`, fixtures `anthropic_e09_<name>_{ok,auth_error}.jsonl`, never delete another endpoint's files |
| one model per run | `E09_MODEL` single; `E09_EXTRA_MODEL` only probe C | `E09_MODELS="m1 m2"` (reasoner vs chat differ in tool/thinking support): one probe A per model; the endpoint is `pass` only if every model the config lists for a used role passed |
| cheap route check absent | none | optional unauthenticated `POST {base}/v1/messages` with an empty body, expecting 400/401/403/405 rather than 404/connection error, as a zero-token "does it serve /v1/messages" pre-check (a network call: user-run only) |
| decision store | `spikes/e09/NOTES.md` holds ONE decision, currently `fail` = "not run" | one section per endpoint name: `pass` / `fail` / `not-run` plus the pasted observation lines |

**Evidence an endpoint must produce before it is trusted.** The offline self-test already names the observation lines that mean "pass": `a.stream_json_parses=yes a.tool_use_write=yes a.result_seen=yes a.result_is_error=no a.usage_input_gt0=yes a.usage_output_gt0=yes a.file_ok=yes a.key_absent=yes b.result_is_error=yes b.api_error_status=401` [VERIFIED: spikes/e09/run.sh:173-175]. Gate on exactly those, per endpoint AND per model, plus the saved fixtures (`tests/fixtures/stream/anthropic_e09_<name>_*.jsonl`) parsing through `stream::summarize_file` with non-zero tokens.

**How an un-probed endpoint appears.** Readiness stays about local ability (command, key); an `anthropic` entry with a `base_url`, `command` and key present, but no `verified` string, reports `chưa kiểm` (the existing word, UI-SPEC line 145 "Engine readiness before the first check") with reason `Endpoint chưa có bằng chứng E-09 — chạy spikes/e09/run.sh rồi ghi "verified" cho engine này.`; `thiếu key` / `thiếu lệnh` win over it. It does not block a run (the user's own first real task is the check); it adds a header warning and `metrics_json.unverified = true`. Q5 asks whether it should block instead. Ripple: the existing test asserts the default `deepseek` reads `san_sang` (dashboard_settings.rs:1026-1027) and `assign.js` words any non-ready engine as "sẽ dừng với lỗi" (assign.js:757-760), so both change. The decision gate stays user-run (it needs the user's key and a real call).

### 06-19 Revision (question 7)

| Part of 06-19 | Change |
|---|---|
| frontmatter `depends_on`, `requirements` | add `06-20 … 06-24`; add the new ENG ids; ENG-01 gate wording ("DeepSeek" → "an `anthropic` engine verified by E-09") |
| Task 1 config of the live check | generate the array shape: `claude` built-in, `gw` (kind anthropic, command = fake claude, `base_url` https fake host, `key_env` e.g. `PHASE6_GW_KEY`), `agy` (fake agy), `dsh` (kind dsh, `docker` = `tests/fakes/fake_docker.py`, container `fake-dsh`), `claude-max` (native, `pin_profile personal`); export two planted keys; the key sweep greps for both |
| Task 1 assertions | tiet-kiem-shaped team (`lead: claude`, `dev: gw`, `qa: claude`) → engines `claude, gw, claude`; a second task with Dev on `dsh` → engine `dsh`, `usage_source none`, `dsh:empty` NOT hit; quota on `gw` then re-run on `mac-dinh` (as before); `claude-max` team on the work project `live-p6` refused at `POST /api/tasks` (422, text above) and accepted on a second **personal** scratch project with the record showing `claude_config_dir` = the personal dir; `/api/stats` rows per id |
| the "E-09 failed → skip deepseek assertions" truth | delete: the automated check runs on fakes and no longer depends on E-09; E-09 matters only for the real P6-02 |
| `--schema-dry-run` | unchanged (no schema change) |
| Task 2 README | § 5 documents the array shape, kinds, fields, `key_env` NAME only, `allowed_profiles` / `pin_profile`, `verified`, both shapes and the built-in `tiet-kiem` / `deepseek` note, a minimal `config.json`; § 11 adds dsh blast radius (root, `/mnt/data`, docker.sock) and root-owned files; § 14 Settings "Engine" lists configured engines with kind/label; exit codes unchanged |
| LIVE-CHECKS § 8 | P6-02 uses the endpoint that passed E-09 (gateway or official); add `P6-09 dsh as a black box` (scratch personal repo, container up, one tiny Dev task, `agent-brain kill` while running, then `docker exec C ps` shows no `ab-dsh-` marker, root-owned file check) and `P6-10 claude-max` (refused on a work project, runs on a personal one); the heading count becomes 11 (`^### P6-(0[0-9]|10)`), and the acceptance grep that expects 9 changes accordingly |
| human-check | add: gateway E-09 pasted lines for each model, dsh kill result, root-ownership observation |

### Pitfalls (amendment)

#### Pitfall A1: a user `command` template silently removes the guard
**What goes wrong:** executing the example's `claude -p {prompt} … --allowedTools {allowed_tools} --strict-mcp-config` drops `--append-system-prompt`, `--settings <guard hooks>` and `--disallowedTools`; Lead/Dev would run without their prompt and without the PreToolUse guard (W-1, ROLE-06).
**How to avoid:** argv for `anthropic` and `agy` comes only from `build_command_with_prompt` / `agy_plan`; `command` supplies argv[0]. Acceptance grep in 06-11: `--settings` and `--append-system-prompt` still present in the gateway argv recorded by the fake.
**Warning signs:** a run whose fake record lacks `--settings`.

#### Pitfall A2: key variables of other engines leak into a run
**What goes wrong:** engine.rs:415-421 strips only the deepseek `key_env` from agy's env; with N entries, a claude, agy or dsh run (or gateway B) inherits gateway A's key from the serve shell.
**How to avoid:** `catalog.all_key_envs()` removed from every env, then each gateway arm injects only its own token; tests with two planted keys assert `env_names` per record.

#### Pitfall A3: `${VAR}` / free `env` maps read arbitrary environment
A config `env` value `${ANY}` would copy any variable of the serve process into a child. Derive env from `base_url` + `key_env` + whitelisted `compat`; reject `env` (strict unknown-key error).

#### Pitfall A4: in-container processes survive a host kill (dsh)
S6 (4a). Every stop path must run the marker-based container kill, including `cmd_kill` from another process (needs the persisted `metrics_json.dsh`) and the runner's timeout; only SIGKILL of agent-brain itself escapes (documented).

#### Pitfall A5: plain text goes through the JSON classifier
`classify` maps non-JSON to `bad_json`; `row_kind` shows it as an error row and `events.raw_json` must stay re-classifiable by `sse::row_json` (sse.rs:36+). Wrap text lines into a JSON event before insert; keep the raw text in `stream.jsonl`.

#### Pitfall A6: `docker inspect` without `--format` prints the container's environment
Always `--format '{{json .Mounts}}'`; never capture or store a full inspect; add an acceptance grep forbidding a bare `inspect` in `dsh.rs` and the shims.

#### Pitfall A7: `plan_for` signature churn
`plan_for(cfg, resolved, role, claude_argv, claude_model, base_env, timeout)` has ~8 unit-test call sites in engine.rs (:475-665) and one production call (roles/mod.rs:436). Adding `project_profile` + raw env (for `pin_profile`) is one coordinated edit in 06-11; later plans must not change it again.

#### Pitfall A8: renamed/removed engine ids
Old `runs.engine` rows keep the old id; stats show them unpriced and the pipeline label falls back to the raw id. Document "do not rename an engine id that has runs"; `kind_of(unknown)` must return `None` and every consumer (dev.rs scope check, pricing) must treat `None` as "no special handling", never panic.

#### Pitfall A9: `chưa kiểm` repurposing
The word currently has no producer (`Readiness::ChuaKiem` exists, engine.rs:75, `readiness()` never returns it). Emitting it changes `dashboard_settings.rs:1026` expectations and the Giao việc callout text; both are listed in the plan, not discovered at execution.

#### Pitfall A10: pricing "most expensive" fallback sees engine prices
`price_for` falls back to the maximum `input` over ALL table values (pricing.rs:23-36). Engine prices must not live in `cfg.pricing`, or an expensive gateway entry would raise the claude fallback.

#### Pitfall A11: personal-profile concurrency rule and `claude-max`
E-06 blocks a second live run on profile `personal` (roles/mod.rs:396-404). A pinned run and a normal personal run share `~/.claude-orca`, which is exactly what E-06 protects; no change needed, but the live check must not run them concurrently.

### Validation Architecture (amendment)

Same framework and commands as the existing section (`cargo test … -- --test-threads=1`, `node scripts/dashboard-js-check.mjs`, ui-lint, `build-css.sh --check`, smoke). `workflow.nyquist_validation` is not set in `.planning/config.json` (only `workflow._auto_chain_active`), so this amendment is in force. [VERIFIED: `cat .planning/config.json` this session]

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ENG-08 | array shape parses to `EngineDef`s: two `anthropic` entries (gateway + official), `agy`, `dsh`, native `claude-max`; absent `engines` = `[deepseek, agy]`; user array not merged with defaults | unit | `cargo test --lib config::tests::engine_list` | ❌ Wave 0 (new tests in config.rs) |
| ENG-08 | legacy object shape unchanged, legacy error strings byte-identical | unit | `cargo test --lib config::tests::engines_defaults config::tests::engines_validation config::tests::engines_custom` (existing, config.rs:913-1012; assertions may move to `get("deepseek")`) | ✅ extend |
| ENG-08 | every validation error of the table (duplicate id, reserved ids `claude`/`người`/NFD, charset/length, unknown kind, `key_env` rules, models role, `command` extra args, `allowed_profiles`/`pin_profile` unknown, dsh container, unknown field) | unit | `cargo test --lib config::tests::engine_list_errors` | ❌ Wave 0 |
| ENG-08 | the shipped example parses: `docs/config.engines.example.json` (amended to the final shape) loads through `config_from_value` | unit | `cargo test --lib config::tests::example_engines_file_loads` (`include_str!` of `../docs/config.engines.example.json`) | ❌ Wave 0 |
| ENG-08 | team naming an unknown engine id: loads with a warning; Settings save → 422; run seam refuses before any run row; summary sort/`engine_text` for ids | unit + integration | `cargo test --lib teams`; `cargo test --test dashboard_settings team_unknown_engine`; `cargo test --test engine_routing unknown_engine_refused_before_run` | ❌ Wave 0 / extend |
| ENG-08 / ENG-01 | two `anthropic` endpoints in one config: each run gets only its own base_url/token (`auth_token_matches_expected`), the other key variable absent from `env_names`; `runs.engine` = the id; stats rows per id | integration | `cargo test --test engine_routing two_anthropic_endpoints` | ❌ Wave 0 |
| ENG-01 | `tiet-kiem`-shaped team on a custom id: lead/qa `claude`, dev `vcadm-deepseek`; header `· engine vcadm-deepseek · model deepseek/deepseek-chat`; argv still has `--settings`, `--append-system-prompt`, `--allowedTools` | integration | `cargo test --test engine_routing tiet_kiem_routes_lead_dev_qa` (re-specified by the new 06-11) | ❌ Wave 0 |
| ENG-05 | planted key of engine A absent from every sink of runs of engines B, claude, agy, dsh; redaction/scan tests of old 06-11 Tasks 2-3 with per-entry keys | integration | `cargo test --test engine_secrets` | ❌ Wave 0 (06-21) |
| ENG-04 | stats rows by engine id; a native `claude-max` run is priced by family, a gateway run by `engine_pricing[id]`, an unpriced one flagged, a dsh run `usage: none` | unit + integration | `cargo test --lib pricing`; `cargo test --test dashboard_stats eng04_engines_split_and_priced` | ✅ extend |
| ENG-09 | argv recorded by the fake docker: `exec -w <root> <container> sh -c ': ab-dsh-…; dsh --profile "$1" "$2"' sh headless <prompt>`; stdin null; prompt not interpolated | integration | `cargo test --test engine_dsh dsh_argv_and_stdin` | ❌ Wave 0 (06-23) |
| ENG-09 | text lines become `dsh/text` events, `usage_source none`, tokens 0, header label; Dev success commits; scope / `.planning/` / secret-path violation → exit 3; empty diff → `dsh:empty`, exit 4 | integration | `cargo test --test engine_dsh dsh_dev_outcomes` | ❌ Wave 0 |
| ENG-09 | marker kill: `agent-brain kill` (cmd_kill path), runner timeout and SIGINT each end with no process whose args carry the marker; no `pkill`/`killall`/name-based kill in sources and shims | integration | `cargo test --test engine_dsh dsh_kill_inside_container` | ❌ Wave 0 |
| ENG-09 | `marker_subtree` pure parsing of a `ps` snapshot (nested, re-parented orphans via the `before` list, no false positives on a similar marker) | unit | `cargo test --lib dsh::` | ❌ Wave 0 |
| ENG-09 | preflight refuses a project root outside every same-path bind mount; accepts `/mnt/data`; refuses `Source != Destination` and read-only mounts | unit + integration | `cargo test --lib dsh::mounts`; `cargo test --test engine_dsh dsh_mount_preflight` | ❌ Wave 0 |
| ENG-09 / SET-01 | readiness for dsh: no `docker` → `thieu_lenh`; container down → `thieu_lenh` with the reason; up → `san_sang`; probe injected, never a real Docker in tests | unit | `cargo test --lib engine::tests::readiness_dsh` | ❌ Wave 0 |
| ENG-10 | `profile_refusal` / `team_profile_refusal` pure rules (`pin_profile`, `allowed_profiles`, third profile) | unit | `cargo test --lib engine::tests::profile_refusal teams::tests::team_profile_refusal` | ❌ Wave 0 |
| ENG-10 | refusal at each bind point with the exact text and NO side effect (no job, no branch/task row, no team write, no run row): Giao việc 422, CLI `run lead --team`, `auto --start --team`, change team, quota re-run (dashboard and Telegram confirm), project-default `team`, Settings save with a work project referencing the team; success on the personal scratch project with `claude_config_dir` = the personal dir in the fake record | integration | `cargo test --test task_team --test dashboard_settings --test telegram_quota --test cli_autopilot --test engine_routing` (new cases named `eng10_*`) | ❌ Wave 0 (06-22) |
| ENG-10 | run seam backstop refuses a hand-edited task team on a work project (exit 1, no run row, autopilot stops `run lỗi`) | integration | `cargo test --test engine_routing eng10_seam_backstop` | ❌ Wave 0 |
| ENG-11 | E-09 offline self-test with `E09_KEY_ENV` and `E09_NAME`: prints `E09 offline OK`, leak gate negative test, two endpoint names do not overwrite each other's files | script | `E09_OFFLINE=1 E09_KEY_ENV=GW_KEY E09_NAME=gw bash spikes/e09/run.sh` then again with another name and `E09_OFFLINE=1 bash spikes/e09/run.sh` | ❌ Wave 0 (06-20) |
| ENG-11 | readiness `chua_kiem` for a command+key-ready gateway without `verified`; `thieu_key` wins; `verified` → `san_sang`; reason text; header warning + `metrics_json.unverified` | unit + integration | `cargo test --lib engine::tests::readiness_chua_kiem`; `cargo test --test dashboard_settings engines_readiness_no_secret` | ✅ extend |
| SET-01 | `GET /api/engines` lists configured engines in order claude → list → người with `kind`, `label`, `description`, no secret value, no write route (`PUT` → 405 as today) | integration | `cargo test --test dashboard_settings engines_readiness_no_secret settings_routes_gated` | ✅ extend |
| SET-05 | Settings Engine rows and role selects built from the API (custom ids, kind badge, `chưa kiểm` soft hint on Giao việc); QA select still only `claude` | js-check + smoke | `node scripts/dashboard-js-check.mjs` (new `engine-list` section); `bash scripts/dashboard-smoke.sh settings assign --keys`; `bash scripts/dashboard-ui-lint.sh`; `bash scripts/build-css.sh --check` | ✅ extend |

**Sampling rate (amendment).** Per task: the module-filtered `--lib` run plus the plan's own integration file. Per wave: that wave's integration files; wave 10 also the UI gates. Phase gate: unchanged (full suite, UI gates, scan-secrets, Cargo files unchanged since `574b6cd`, the live check with the amended script).

**Wave 0 gaps (amendment).**
- [ ] `tests/fakes/fake_docker.py` and `tests/fakes/fake_dsh.py` (+ a `Sandbox` helper that puts the shim directory first on `PATH` and writes the `dsh` engine entry)
- [ ] `tests/fakes/fake_claude.py`: `env_names`, `anthropic_base_url`, `auth_token_matches_expected`, `echo_env` (the old 06-11 additions, still unbuilt) with per-call `FAKE_CLAUDE_EXPECT_TOKEN`
- [ ] `Sandbox::write_engines(list)` and a **personal** scratch project helper (`setup_auto_project(&sb, "personal")` already takes a profile, engine_routing.rs:57)
- [ ] fixtures `tests/fixtures/stream/anthropic_e09_<name>_{ok,auth_error}.jsonl` (user-run output of 06-20; tests skip with a printed line when absent, as the old `e09_fixtures_parse` did)
- [ ] `06-VALIDATION.md`: add the rows above, replace the single E-09 manual row by "E-09 per endpoint and per model", add manual rows "dsh black box on a scratch personal repo (kill, ownership)", "`claude-max` refused on work / runs on personal"

**Manual-only (amendment).** E-09 per endpoint (user key, real call); dsh kill and ownership on the real container (S6-style, user-run); the first real dsh task; Settings engine list look in light/dark at 1024/768/480.

### Security Domain (amendment)

`security_enforcement` is not disabled in config, so the section applies. ASVS categories touched: V5 (config and file input validation), V7 (redaction of N keys, no secret in errors), V12 (config files: strict keys, no env expansion), V4 (profile boundary as an access-control rule), V14 (configuration: Docker socket / bind-mount exposure of the dsh container).

| Pattern | STRIDE | Standard mitigation |
|---|---|---|
| user config injects argv or env (template, `env`, `${VAR}`) | Tampering / Information disclosure | argv owned by role builders; env derived; strict unknown-key errors; config is the user's own trusted file but still never shell-interpreted (argv arrays only) |
| wrong key reaches the wrong endpoint or engine | Information disclosure | strip all `key_env` from every env, inject one; `https://` only; per-engine redaction; planted-key tests for two keys |
| work account used where personal is required (or the reverse) | Elevation of privilege | one pure `profile_refusal`, called at the run seam and six bind points; `pin_profile` resolves the directory through `profiles`, never a literal path |
| dsh container root + `/mnt/data` rw + docker.sock | Elevation of privilege / Tampering | documented blast radius; optional `allowed_profiles`; post-run diff covers only the task repo (cannot see other projects); first real run on scratch; nothing in agent-brain starts, stops or execs anything but `exec … sh -c`, `ps`, `kill -TERM/-KILL <marker pid>`, `ps`, `inspect --format` |
| shell injection through the dsh prompt | Tampering | prompt passed as positional `$2`, wrapper text is a constant plus a hex marker; `container` / `docker` validated by charset |
| leftover container process after a kill | Denial of service / Repudiation | marker subtree + sweep by recorded PID; residual SIGKILL-of-agent-brain case documented and detected by preflight |
| gateway error body echoing the token | Information disclosure | redaction before every sink applies to gateway engines (their stderr/stream can contain the token) |
| `docker inspect` leaking container env | Information disclosure | only `--format '{{json .Mounts}}'`; acceptance grep |

### Environment Availability (amendment)

| Dependency | Required by | Available | Version | Fallback |
|---|---|---|---|---|
| docker CLI | dsh readiness/preflight/kill (real use), live check P6-09 | ✓ | Docker 29.8.1, build 4a63305 [VERIFIED: `docker --version`] | tests use `tests/fakes/fake_docker.py` |
| container `dsh-deepseek-harness` | dsh real runs | ✓ up | `Up 8 days`, image `dsh-base:latest` [VERIFIED] | none (user starts it; agent-brain never does) |
| `dsh` in the container | dsh | ✓ | `dsh --help` works; headless help quoted above [VERIFIED] | — |
| `ps` inside the container | marker kill | ✓ (used by S6) [VERIFIED: spikes/s6/run.sh `snapshot()` runs `docker exec "$C" ps -ww -eo pid=,ppid=,pgid=,args=`, evidence in S6 (4a)/(4b)] | — | — |
| `sh -c` wrapper behaviour in the image | dsh kill design | unknown (not executed: only `dsh … --help` is allowed) | — | S6-style user probe in 06-20 |
| gateway key (`VCADM_GATEWAY_KEY`) and network | E-09 per endpoint | ✗ in this environment (by constraint) | — | user-run only |
| `python3`, `node`, cargo, git, Chrome | tests, gates | ✓ (see the 2026-10-26 table) | — | — |

**Missing with no fallback:** the user's gateway key and a real call (blocks only the E-09 evidence and the ENG-01 real gate). **Missing with fallback:** Docker in CI-like runs (fake shims).

### Runtime State Inventory (amendment)

Not a rename phase, but `runs.engine` changes meaning from a fixed name to an id: **Stored data:** no historical `deepseek` run can exist (`plan_for` refuses it today: engine.rs:332-335 `Engine::Deepseek => Err(BrainError::error(format!("Engine {} chưa được bật trong bản agent-brain này", …`), so no row migration; **Live service config / OS-registered state:** none; **Secrets/env vars:** new variable NAMES only (`VCADM_GATEWAY_KEY` in the example), values only in the process env; **Build artifacts:** none. Nothing needs a data migration; only the README note about not renaming ids.

### Recommended Plan Decomposition (amendment)

All dependencies on executed plans are satisfied (17 of 19 have SUMMARYs). The old `06-11` file is replaced; five plans are new (numbers 06-20 … 06-24); `06-19` is revised and moves last. The Rust work no longer waits for E-09: D-34 moves the gate from "build the engine" to "trust the endpoint", and fakes cover every behaviour (the real evidence gates only 06-19's human checks).

| Plan | Wave (frontmatter) | depends_on | Requirements | Files it owns | Notes |
|---|---|---|---|---|---|
| **06-11** (replaced) "engine list + `anthropic` kind + profile-refusal core" | 9 | executed plans only | ENG-08, ENG-11 (readiness), ENG-01 (partial), ENG-10 (core fn only) | `config.rs`, `assets/config.default.json`, `engine.rs`, `teams.rs` (parse/summary/QA rule only), `pricing.rs`, `roles/mod.rs` (seam: `String` engine, `plan_for` signature, `profile_refusal` call, unknown-id refusal), `roles/dev.rs` (`kind_of` gate only), `dashboard/api_settings.rs` (`get_engines` payload only), new stub `dsh.rs` (arm returns "Engine dsh chưa được bật"), `lib.rs`, `docs/config.engines.example.json`, `tests/fakes/fake_claude.py`, `tests/{engine_routing,dashboard_settings}.rs`, config/engine/teams unit tests | 3 tasks: (1) tracer: catalog + both shapes + `Engine::Named` + `anthropic` arm → `tiet_kiem_routes_lead_dev_qa` on a custom id; (2) readiness/API/teams-with-catalog/pricing per id/unknown-id refusal; (3) `profile_refusal` + `pin_profile` env + seam backstop + example file test. Estimate ~110k tokens, confidence low |
| **06-20** (new) "E-09 per endpoint + dsh kill probe tooling" | 9 | none | ENG-11 | `spikes/e09/{run.sh,verify.sh,NOTES.md}`, `spikes/s6/run.sh` (a `--kill-wrapper` variant for the marker form, optional `--uid` variant), `tests/fixtures/stream/anthropic_e09_*` (user output) | no Rust; disjoint from 06-11, so same wave; `checkpoint:human-action` (user runs the gateway probe with their own key) then `checkpoint:decision` per endpoint; offline self-tests with fakes; ~40k tokens |
| **06-21** (new) "key hygiene for N keys" = old 06-11 Tasks 2-3 | 10 | 06-11 | ENG-05 | `runner.rs` (`redact`), `roles/mod.rs` (pass redact, post-run scan), `engine.rs` (`scan_for_secrets` appended; no `plan_for` change), `tests/fakes/fake_claude.py` (`echo_env`), `tests/engine_secrets.rs` | near-verbatim reuse of the old tasks; secrets = values of the run's engine `key_env` variables; ~70k tokens |
| **06-22** (new) "profile guard at the bind points (D-36)" | 10 | 06-11 | ENG-10 | `teams.rs` (`team_profile_refusal`), `roles/lead.rs`, `autopilot/mod.rs`, `dashboard/api_tasks.rs`, `dashboard/api_settings.rs` (save), `telegram/intake.rs` (optional picker filter), tests `task_team.rs`, `dashboard_settings.rs`, `telegram_quota.rs`, `cli_autopilot.rs`, `engine_routing.rs` (`eng10_*`) | disjoint from 06-21 (no runner/roles/mod.rs); needs 06-11's `profile_refusal`; ~70k tokens |
| **06-24** (new) "engine list in Settings and Giao việc" | 10 | 06-11 | SET-01 (amended), SET-05 | `assets/dashboard/js/{settings,assign}.js`, `screens/{settings,assign}.html`, `scripts/dashboard-js-check.mjs`, `scripts/dashboard-smoke.sh`, `assets/dashboard/app.css` only if `build-css.sh --check` says so | disjoint (assets only); the UI-SPEC copy change needs approval (Q9); ~60k tokens |
| **06-23** (new) "`dsh` black-box engine" | 11 | 06-21, 06-22 (runner.rs and roles/mod.rs ownership; guard tests) | ENG-09 | new `dsh.rs`, `runner.rs` (stop hook + `cmd_kill`), `roles/mod.rs` (Observer text format, `metrics_json.dsh`), `stream.rs`, `term.rs`, `dashboard/sse.rs`, `roles/dev.rs` (`dsh:empty`), `autopilot/{decide,view}.rs`, `dashboard/taskstate.rs`, `quota.rs` (`detect_text`), `tests/fakes/{fake_docker,fake_dsh}.py`, `tests/engine_dsh.rs` | the largest and riskiest plan (process safety); one `coupling_justified` note for the shared files; ~120k tokens, confidence low; no real Docker in any automated test |
| **06-19** (revised) | 12 | 06-01 … 06-18, 06-20 … 06-24 | ENG-01 … ENG-11, TEAM-02, SET-05 | `scripts/phase6-live-check.sh`, `agent-brain/README.md`, `agent-brain/LIVE-CHECKS.md` | see "06-19 Revision"; its `[BLOCKING]` schema dry run is unchanged |

**Shared-file watch list.** `engine.rs`: 06-11 (owner), 06-21 appends one function only. `roles/mod.rs`: 06-11 → 06-21 → 06-23 strictly serial. `runner.rs`: 06-21 → 06-23. `teams.rs` and `api_settings.rs`: 06-11 → 06-22. `fake_claude.py`: 06-11 → 06-21. `tests/engine_routing.rs`: 06-11 then 06-22 (06-21 uses its own file). 06-11 must leave the dispatch stubs (`dsh` arm, `kind_of`, `all_key_envs`, `profile_refusal`) in place so 06-21/06-22/06-23 never edit `plan_for` again (the old plans did the same with deepseek/agy stubs).

**One-way or costly doors (mark each as a decision checkpoint or an explicit note in the plan):**

1. `runs.engine` = engine **id** from the first non-claude run on: renaming an id later orphans rows; reversible only by not renaming. (06-11, note in README)
2. Engine ids are written into `teams.json` and `tasks.team_json`: same orphan effect; mitigated by the refuse-before-run rule.
3. Nothing in these plans rewrites the user's real `config.json` (none exists), `teams.json` or `projects_map.json`; both config shapes are accepted, so the choice of shape is reversible.
4. The first real dsh run on a work repo can leave root-owned files that only `sudo` fixes: not reversible by agent-brain. Gate it behind `checkpoint:human-verify` in 06-19 (P6-09 on a scratch personal repo) and ship `allowed_profiles` for the `dsh` entry.
5. The first real gateway call spends the company's tokens and sends a company key to an internal endpoint: only through the user-run E-09 script (06-20), never from automated tests.
6. The built-in `tiet-kiem` keeps naming `deepseek`: changing a built-in team later changes behaviour for every user of it (Q7).
7. Adding `chưa kiểm` to readiness changes UI behaviour and two existing assertions (A9): do it once, in 06-11 + 06-24.

### Proposed requirement wording (for the planner to register in REQUIREMENTS.md)

Existing style; coverage grows by 4 (122 → 126) and the Traceability table gets four `Phase 6 | Pending` rows.

- **ENG-08**: Danh sách engine do người dùng khai báo trong `config.json` (`engines`: mảng mục có `id`, `kind` = `anthropic` | `agy` | `dsh`, lệnh, `base_url` + tên biến `key_env` + model theo vai trò, giá); đội hình gọi engine theo `id` (cùng `claude` và `người:<mã>`); nhiều mục cùng loại được phép (gateway công ty và endpoint DeepSeek chính thức); dạng object `engines.deepseek` / `engines.agy` cũ vẫn đọc được; `runs.engine` ghi `id`; dashboard chỉ xem, không route nào ghi engine. *(derived: 06-CONTEXT D-30, D-31, D-32, D-33)*
- **ENG-09**: Engine kiểu `dsh` chạy như hộp đen qua `docker exec` (chỉ văn bản, không token, dừng được bên trong container theo marker và quét các tiến trình con đã ghi, không theo tên, không khởi động lại container), kiểm tra git diff sau khi chạy như Dev của agy, và từ chối dự án nằm ngoài thư mục container thấy cùng đường dẫn. *(derived: 06-CONTEXT D-35)*
- **ENG-10**: Engine có `pin_profile` hoặc `allowed_profiles` chỉ chạy trên dự án có profile phù hợp: lưu đội hình dùng engine đó, giao việc, đổi đội hình, chạy lại khi hết quota, `run lead --team`, `auto --start --team` và chạy bước trên dự án khác profile đều bị từ chối bằng thông báo tiếng Việt, không đường nào mượn quota của profile khác (ranh giới SPK-10). *(derived: 06-CONTEXT D-36)*
- **ENG-11**: Mỗi endpoint `anthropic` có `base_url` cần bằng chứng E-09 (stream-json, tool use, usage, sửa file, không lộ key) theo từng model trước khi được tin; chưa có thì hiện `chưa kiểm`; script thăm dò nhận tên biến key và nhãn endpoint; quyết định do người dùng chạy. *(derived: 06-CONTEXT D-34)*
- Amend **ENG-01** ("Dev chạy DeepSeek" → "Dev chạy trên một engine `anthropic` đã có bằng chứng E-09, endpoint DeepSeek chính thức hoặc gateway"), **ENG-05** ("Key DeepSeek" → "Key của mọi engine (mọi biến `key_env`)"), **TEAM-01** (engine names: "`claude`, tên engine khai báo trong `config.json`, `người:<mã>`") and **SET-01** (mục Engine liệt kê engine khai báo trong config.json cùng độ sẵn sàng, `chưa kiểm` khi thiếu bằng chứng E-09).

### Assumptions Log (amendment)

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A15 | `https://ai-gateway.vcadm.vn/proxyllm` serves Anthropic `/v1/messages` and accepts `ANTHROPIC_AUTH_TOKEN` as its key | anthropic kind, E-09 | the endpoint fails E-09; D-34 anticipates this (the user said "if it serves `/v1/messages`") |
| A16 | `claude --model deepseek/deepseek-chat` is passed through unchanged and the gateway maps it | anthropic kind | model-name 400s; per-model E-09 catches it |
| A17 | the gateway's budget/rate errors surface as HTTP 429/402 or a text `detect_claude` matches (LiteLLM documents 429 in one place and 400 in another [CITED: docs.litellm.ai users / error_reference]; that `proxyllm` is LiteLLM is itself assumed) | anthropic kind, quota | gateway budget stops look like `run lỗi` instead of `hết quota`; raw lines stored for recalibration |
| A18 | the shell in `dsh-base:latest` runs `sh -c ': marker; dsh …'` so the marker stays visible in `ps` args until dsh exits | dsh kind | marker kill finds nothing; mitigated by the PID-file fallback and the user-run kill probe (06-20) before real use |
| A19 | `docker exec -u 1000:1000` works with dsh (profile in `/root/.dsh` readable) | dsh hazards | root-owned files cannot be avoided; unknown, probe only |
| A20 | git run as root inside a user-owned repo is refused or harmless | dsh hazards | root-owned `.git` objects break the host's git; detect with a post-run `git status` in P6-09 |
| A21 | agy/dsh/gateway engines on a personal project share the personal profile's E-06 serialisation safely | claude-max | over-restrictive only |
| A22 | per-model E-09 evidence is required (reasoner vs chat differ) | E-09 | extra user-run probe cost; relaxing to per-endpoint is a one-line rule change |
| A23 | `chưa kiểm` should warn, not block | E-09 | if the user wants a hard gate the run seam refuses instead (Q5) |
| A24 | LiteLLM-style budget phrases (`budget`, `exceeded budget`) belong in the quota regex | anthropic kind | false positives on prose; keep it to an error result only |
| A25 | dsh output is plain text on stdout with errors on stderr and a non-zero exit when dsh fails | dsh kind | quota/failed misclassification; S6 observed exit 0 and one stdout line on success only |

Verified this session (not assumed): the code seams and line numbers above, the dsh help text, the same-path `/mnt/data` bind mount, container user/restart policy, the project count and profile split, `runs.engine` having no CHECK, the E-09 script limits, the existing test expectations quoted.

### Open Questions (amendment)

1. **Q1: shape divergences from `docs/config.engines.example.json`.**
   - What we know: the example is the user's intent but marked "CHƯA dùng được"; it contains an `id: "claude"` entry, full argv templates, `env` maps and a dsh template with the container inside (divergences D1-D7 above).
   - What's unclear: whether the user wants templates to drive argv (the research recommends against: Pitfall A1) and whether an entry with `id: "claude"` is meant to override the built-in's models/label or just document it.
   - Recommendation: `command` = executable only; `claude` reserved (a list entry with that id is a config error telling the user to remove it); `pin_profile` instead of `env.CLAUDE_CONFIG_DIR`; discrete dsh fields. Planner amends the example to the final shape and a test loads it.
2. **Q2: which roles may use `dsh`?**
   - Known: D-35 is silent; dsh has no read-only mode (agy gets `request-review`); `người` is Dev-only (D-24).
   - Unclear: whether Lead/reviewers on dsh are wanted.
   - Recommendation: Dev only in this phase, refused by the teams validator and Settings save with a Vietnamese error (same mechanism as D-24).
3. **Q3: restrict `dsh` to personal projects and accept the blast radius?**
   - Known: root, rw `/mnt/data`, docker.sock, root-owned output files; 176 of 178 projects are work projects.
   - Unclear: whether work repos are in scope for dsh at all.
   - Recommendation: ship `allowed_profiles` for every kind; suggest `["personal"]` on the user's `dsh` entry until P6-09 shows the ownership/kill behaviour is acceptable.
4. **Q4: QA on `claude-max`, and is the pinned engine worth it?** 
   - Known: D-06 says QA must be `claude`; teams.rs:220 is strict equality; on a personal project `claude` already runs on `~/.claude-orca` (assets/config.default.json personal profile), so `claude-max` differs only by models/label.
   - Unclear: whether the user wants QA allowed on any native Claude entry, and whether the model difference justifies the pinned kind.
   - Recommendation: keep QA strictly `claude` (costs nothing on personal projects); keep D-36 as decided.
5. **Q5: should an endpoint without E-09 evidence block runs?**
   - Recommendation: no, warn (`chưa kiểm`, header line, `metrics_json.unverified`). The first real task is the evidence collector in practice; a hard block can be one config flag later.
6. **Q6: Settings save of a team that uses a pinned engine, when no work project references it yet.**
   - Recommendation: allow; refuse only when a project default (`projects_map.json` `team`) or an open task of a project with a forbidden profile uses the team; everything else is caught at use (seam + bind points).
7. **Q7: built-in `tiet-kiem` and a user list that lacks `deepseek`.**
   - Recommendation: keep the built-in naming `deepseek`; document the override in `teams.json`; the Settings team row shows "engine chưa khai báo". Alternative (not recommended): resolve `tiet-kiem`'s Dev to "the first verified gateway", which hides which endpoint is spending money.
8. **Q8: UI-SPEC change.** The approved UI-SPEC says the Engine section lists "Engine có sẵn trong agent-brain" with fixed descriptions (06-UI-SPEC.md:280-288) and that `chưa kiểm` is "Engine readiness before the first check" (:145). Both change (configured list, `chưa kiểm` = no E-09 evidence, `usage: none`). Needs the user's OK on the copy before 06-24 (a short UI-SPEC delta).
9. **Q9 (optional): fifth readiness word for "container chưa chạy"** instead of reusing `thiếu lệnh` with a reason line. Recommendation: reuse, no vocabulary change.

### Sources (amendment)

#### Primary (HIGH confidence)
- Codebase files opened this session (paths cited inline): `agent-brain/src/{engine,config,teams,pricing,quota,runner,profile,stream,term,db,projects,staff,workflows,error}.rs`, `src/roles/{mod,dev,lead}.rs`, `src/autopilot/{mod,decide,view}.rs`, `src/dashboard/{api_settings,api_tasks,api_stats,api_runs,jobs,sse,taskstate,pipeline}.rs`, `src/telegram/{bot,intake}.rs`, `assets/config.default.json`, `assets/teams.default.json`, `assets/dashboard/js/{settings,assign,app,stats,task}.js`, `agent-brain/tests/{engine_routing,engine_agy,dashboard_settings}.rs`, `tests/common/mod.rs`, `tests/fakes/{fake_claude,fake_agy}.py`
- `spikes/spike-results.md` (S5, S6, SPK-10), `spikes/s6/run.sh`, `spikes/e09/{run.sh,verify.sh,NOTES.md}`, `docs/config.engines.example.json`
- `06-CONTEXT.md` (D-30 … D-36 and the decisions they revise), `06-11-PLAN.md`, `06-19-PLAN.md`, SUMMARYs of 06-01, 06-02, 06-03, 06-05, 06-06, 06-07, 06-09, 06-10, 06-13, 06-14, 06-15, `06-UI-SPEC.md` (engine section), `06-VALIDATION.md`, `.planning/REQUIREMENTS.md` (ENG/TEAM/SET/HUM/QUOTA rows)
- Read-only tool output: `docker --version`, `docker ps -a --format …`, `docker inspect dsh-deepseek-harness --format '{{json .Mounts}}'` and a second `--format` for user/workdir/state/restart policy, `docker exec dsh-deepseek-harness dsh --profile headless --help` and `dsh --help`, a python read of `~/.config/agent-brain/{config,projects_map}.json` (no config.json exists; 178 projects)

#### Secondary (MEDIUM confidence)
- https://docs.litellm.ai/docs/proxy/users and https://docs.litellm.ai/docs/proxy/error_reference.md (budget-exceeded error codes vary: 400 / 429 / string `error.code`)
- the 2026-10-26 sources already listed above (Claude Code gateway docs for credential precedence)

#### Tertiary (LOW confidence)
- web search on `docker exec` signal forwarding returned only generic container-signal material; the local S6 evidence is used instead
- anything about `ai-gateway.vcadm.vn` (not reachable by constraint)

### Metadata (amendment)

**Confidence breakdown:**
- Seam map and decomposition: HIGH (every cited file opened in this tree).
- Config shape and guard design: HIGH for the code constraints; MEDIUM for the user-intent choices (Q1-Q4).
- `anthropic` gateway behaviour: MEDIUM-LOW until E-09 per endpoint.
- dsh process behaviour: MEDIUM (S6 evidence + read-only docker facts), LOW for the marker wrapper on the real image until the user-run probe.

**Research date:** 2026-10-27
**Valid until:** 2026-11-03 for the dsh/docker facts (the container's mounts and image can change at any time); code-seam claims hold until the next commit touching the cited files.
