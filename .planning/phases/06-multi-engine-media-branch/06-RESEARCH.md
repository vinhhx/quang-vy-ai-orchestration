# Phase 6: Multi-engine Teams - Research

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
