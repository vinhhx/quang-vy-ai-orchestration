# Phase 6: Multi-engine Teams - Context

**Gathered:** 2026-10-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Each role in a task (Lead, Dev, QA, plan-review, code-review) can run on a different **engine**, chosen through a named **team** (đội hình). The engines are:
- `claude`, which uses the project's profile as today
- `deepseek`, which is `claude` on the host with `ANTHROPIC_BASE_URL` pointing at DeepSeek
- `agy`, which is Antigravity running headless
- `người`, a human staff member who works on a pushed branch

When a run hits a quota or rate limit, the task stops and asks the user. It never switches engines on its own. The user can re-run the step with another team, and the new engine continues from `.plan.md`, the current diff, and `.handover.md` if one exists.

There is also a new dashboard **Cài đặt** (Settings) screen where the user creates and edits teams **and workflows**.

Requirements in this phase: ENG-01 … ENG-07, plus new requirement IDs for the Settings screen and the human engine. The planner registers those IDs in REQUIREMENTS.md.

**Media is moved out (D-22).** MEDIA-01 … MEDIA-03 go to a new inserted phase **06.1 (Media Branch)**. The ROADMAP Phase 6 `Requirements` and goal lines, plus a new `### Phase 06.1` section, are updated by the orchestrator after this context is committed. Phase 6 still owns success criteria 1, 2, 3 and 5. Criterion 4 (media) moves to 06.1.
</domain>

<decisions>
## Implementation Decisions

### Engines
- **D-01:** DeepSeek runs as `claude` on the host with `ANTHROPIC_BASE_URL` set to DeepSeek's Anthropic-format endpoint and the key in `ANTHROPIC_AUTH_TOKEN`. This reuses the Phase 3 stream-json parser, token counting, `--max-turns`, `--allowedTools` and the S4 kill. The `dsh` Docker container is **not** used (S6: black box, no tokens, kill must happen inside the container).
- **D-02:** The **first plan is a decision gate that tests E-09**. It runs one real tiny `claude -p` call through the DeepSeek endpoint and records whether stream-json, usage, tool use and file edits work. If it fails, **stop the DeepSeek work and ask the user** whether to use dsh as a black box or drop DeepSeek. The agy, team, quota, Settings and human-engine work goes ahead either way. — **Reversibility:** costly — every later DeepSeek plan assumes the stream-json path.
- **D-03:** The DeepSeek key is read **only from an environment variable**. The config holds just the variable's name (for example `DEEPSEEK_API_KEY`). A missing variable gives a clear Vietnamese error before spawning. The key never goes into a committed file, a log, `raw_json`, a run dir, argv shown in the UI, or a Telegram message (ENG-05). Sensitive variables are filtered before anything is written.
- **D-04:** Antigravity is a **real headless engine** (ENG-06, S7): `agy -p … --output-format stream-json --print-timeout <explicit>`, with stdin from `/dev/null`, stdout to a file, and an outer timeout. There is no turn limit. Its events use the `event` key (`init`, `step_update`, `result`), so the parser needs its own branch that reads `result.status`, `result.response` and `result.usage`. The run is killed with the S4 method.
- **D-05:** agy gets `--dangerously-skip-permissions` **only for the Dev role**. agy has no `--allowedTools`, so after every agy Dev run agent-brain checks `git diff`: touching files outside the plan's scope, protocol files, `.planning/` or secret-looking paths is flagged as a protocol violation (the existing `vi phạm giao thức` stop). agy in Lead or reviewer roles runs without skip, so in practice it is read-only. Because tool approvals are soft-denied with exit 0, a "success" with an empty diff on a Dev run is reported as suspicious, not as success.
- **D-06:** Cheap engines (`deepseek`, `agy`) and `người` are allowed for **every role except QA**. QA must use a strong engine (`claude`). Saving a team whose QA is not `claude` is refused by validation, on the Settings screen and in the file loader.
- **D-07:** The `claude` engine **always uses the project's profile** (work = `~/.claude`, personal = `~/.claude-orca`). Teams name engines (`claude`, `deepseek`, `agy`, `người:<id>`), never accounts. A work project can never borrow the personal account's quota, which keeps the SPK-10 boundary. The source spec's `claude-work` / `claude-personal` engines and its `~/.claude-work` / `~/.claude-max` paths are superseded.
- **D-08:** Engine definitions are **fixed**, built into the binary plus `config.json` (command path, endpoint, key variable name, model per engine). The UI shows each engine's readiness (`sẵn sàng`, `thiếu key`, `thiếu lệnh`, `chưa kiểm`) but **cannot edit engines**. No UI route writes a command or endpoint.

### Teams and the Settings screen
- **D-09:** Teams live in `~/.config/agent-brain/teams.json`. It can be hand-edited like `workflows.json`, and has a size cap and line/column error messages in the same style. Built-in default teams ship in the binary, at least `mac-dinh` (all `claude`) and `tiet-kiem` (Lead/QA `claude`, Dev `deepseek`). A team has a name (slug), a purpose/description in Vietnamese, and an engine for each role: `lead`, `dev`, `qa`, `plan-review`, `code-review`. No brain.db table is added for teams.
- **D-10:** A new dashboard screen **Cài đặt** (`#/settings`) creates, edits and deletes teams **and workflows** (the user chose to include workflows). Writes are atomic: temp file, fsync, rename. They are validated with the same rules as the file loader, and stay behind the token and header middleware like every POST. Built-in teams and workflows are read-only, but can be copied ("Nhân bản").
- **D-11:** A team **and** a workflow cannot share a name, or the UI must always label which is which. The source's `mac-dinh` is already a workflow name, so the planner picks the rule (for example, separate namespaces with explicit labels) and records it.
- **D-12:** How a task gets a team: the project's default `team` comes from `projects_map.json` (missing means `mac-dinh`). The Giao việc screen and Telegram `/task` get a team picker that overrides it for that task, and the CLI gets `--team`. The chosen team is stored durably with the task, so restarts and autopilot keep it.
- **D-13:** The team of a task that is **not running** can be changed on the task detail screen. The change applies from the next step on, and a history line is recorded (who, when, from → to). While a run is live, changing is refused.
- **D-14:** Every run records its real engine in `runs.engine` (which today is always `claude`) and the team name. The log header line `[agent-brain] … · profile … · model …` gains `engine …` (ENG-01).

### Quota and rate limits
- **D-15:** A quota or rate-limit error is detected from the stream (error result or subtype) or the exit code, for each engine. The run gets outcome `quota`, and the autopilot stops with a new reason word, for example `hết quota` ("Hết quota engine X"). The task **waits for the user**.
- **D-16:** **Never auto-switch.** There are no configurable fallback rules in this phase (deferred). The "Bạn" gate card shows `Chạy lại bước này bằng đội hình…` (a team picker) and `Thử lại sau` (same engine, when the user chooses).
- **D-17:** Notifications go to the dashboard **and Telegram**. Following the 5.3 pattern, the Telegram message carries buttons to pick a team and re-run the step, through the same shared action function as the dashboard (`run_task_action` style). The Telegram trust rules from 5.3 and 5.4 still apply. Tasks created from Telegram still get their plan approved only on the dashboard. Re-running an implement or QA step is not plan approval.
- **D-18:** When a step is re-run on a new engine, the new engine continues **on the current worktree**: the partial diff is kept, the prompt says "continue from `.plan.md`, look at the existing git diff", and `.handover.md` is included if it exists. The session is never resumed across engines (ENG-03).

### Human engine (`người`)
- **D-19:** Staff are declared on the Settings screen: id, display name, and optional notes/contact. A team can set any role except QA to `người:<id>`. At that step agent-brain prepares the task branch with the committed `.plan.md` and shows it as "Chờ <tên> làm". It does not spawn anything.
- **D-20:** Staff work on their own machine and **push the task branch to the remote**. Staff have **no dashboard or Telegram access**; the user presses `Xong` on the task screen for them. On `Xong`, agent-brain fetches the branch into the task's worktree and continues the workflow. The **existing workflow** decides who approves: code-review (AI) on the git diff, then QA (strong engine) and/or the "Bạn" gate for a real test. A failure goes back to the same person with the report (`Trả lại`).
- **D-21:** A human run is recorded as a run with engine `người` and no token usage, so statistics and the "Cần sửa" rate (ENG-07) cover it too.

### Statistics
- **D-23:** The Thống kê screen splits tokens and cost by engine (ENG-04) and shows QA's "Cần sửa" rate by Dev engine (ENG-07). It reuses the existing stats queries, grouped by `runs.engine`. agy usage comes from `result.usage`. Price entries per engine go in `config.json` `pricing`.

### Scope
- **D-22:** **Media moves to inserted phase 06.1 (Media Branch).** The user's direction for 06.1 is:
  - A generic render job: approved script in the media project → agent writes the render config/script → runner runs FFmpeg → output file.
  - Output goes under `<media project>/output/`. Media projects are plain folders, so no worktree is created.
  - The `media_jobs` table, and render kill with no leftovers (S4 method).
  - The whiteboard pipeline does not exist yet. Connecting it comes later.

### Amendments after research (user answers, 2026-10-26)
- **D-24 (narrows D-19):** In this phase the `người` engine is allowed **only for the Dev role**. Saving a team that sets Lead, plan-review, code-review or QA to `người:<id>` is refused with a clear Vietnamese error. Other roles are deferred.
- **D-25 (extends D-20):** When a human Dev step starts, agent-brain makes a **best-effort `git push`** of the task branch (with the committed `.plan.md`) to the remote. If the push fails, the step still waits, and the UI shows the branch name and the exact push command for the user to run. This is a new network side effect; it never force-pushes.
- **D-26 (refines D-05):** The agy probe plan tests a narrower permission mode (`--mode accept-edits` / `--sandbox`) for Dev. If the probe shows agy can edit files in that mode, Dev uses it. Only if it cannot does Dev fall back to `--dangerously-skip-permissions`. The post-run git diff scope check always applies.
- **D-27:** Team names are capped at **40 characters**, matching the existing `validate_name`. The UI-SPEC's 48 is superseded.
- **D-28:** Picking a team on Telegram to re-run a step needs a **second confirm tap** before anything runs.
- **D-29:** While a human step is waiting (`Chờ <tên> làm`), changing the task's team is **refused**, the same as while a run is live.

### Amendments during execution (user answers, 2026-10-27)
- **D-30 (replaces the fixed DeepSeek engine of D-01/D-08 for 06-11):** Engines become a **user-defined list** in `config.json` `engines`: each entry has `id`, `kind`, `command`, and per kind its connection fields (`base_url`, `key_env`, `models` per role, `pricing`; agy: `dev_permission`, `print_timeout_margin_secs`, `model`). Teams name engines by `id` (plus the built-in `claude` and `người:<id>`). Several entries of the same kind are allowed (for example the user's company gateway `https://ai-gateway.vcadm.vn/proxyllm` and DeepSeek's official endpoint). The UI still only shows engines and their readiness (D-08 stays read-only); the list is edited in the file. Existing `engines.deepseek` / `engines.agy` objects must keep loading (migrate or accept both shapes).
- **D-31:** Supported kinds now: **`anthropic`** (claude on the host pointed at an Anthropic-format endpoint, the D-01 mechanism, generalized) and **`agy`**. An OpenAI-format kind is **deferred** (needs a different agent CLI that edits files).
- **D-32 (keeps D-03):** The file holds only the key's environment-variable **name** (`key_env`) per engine, never the key.
- **D-33 (keeps D-07):** Engine entries **cannot name a Claude account or profile**; claude-family runs always use the project's profile. For gateway engines the "account" is the key alone.
- **D-34:** E-09 evidence is still required per `anthropic` endpoint before it is trusted (tool use, stream-json, usage through that endpoint). The first one to probe is the user's gateway (`E09_BASE_URL`/`E09_MODEL` overrides of `spikes/e09/run.sh`), if it serves `/v1/messages`. Plan 06-11 is re-planned around D-30…D-34; 06-19 adjusts its DeepSeek checks to the engine list.

### Claude's Discretion
- Behaviour when a task's team no longer exists in `teams.json` (the user left this to the planner).
- Exact JSON shapes of `teams.json`, the engine entries in `config.json`, and the staff list. The planner can keep staff inside `teams.json` or put them in `staff.json`.
- How a task stores its team (a column on `tasks`, which needs a schema v4 migration with backup and a decision gate as in 05.4, versus a run-dir/plan-header field). Pick the durable, smallest option and flag any migration as a decision checkpoint.
- The quota detection patterns for each engine. Research the real stream and exit shapes, and use fixtures.
- The Settings screen layout. It follows the 05-UI-SPEC rules (Tailwind build, Alpine CSP, offline, Vietnamese, light/dark, ui-lint), and a UI-SPEC via `/gsd-ui-phase 6` is recommended before planning.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase source and requirements
- `files/06-phase5-da-doi-hinh-media.md`: source spec (§2.2 engines, §2.3 quota, §3.1 `teams.json` shape, superseded by D-07 for engine names and paths; §5 test cases P5-01 … P5-07)
- `.planning/REQUIREMENTS.md`: ENG-01 … ENG-07 (MEDIA-01 … 03 move to 06.1)
- `.planning/PROJECT.md`: Out of Scope (no silent engine switch), Constraints (Secrets, Process safety, Log format tolerance, Protocol), Key Decisions

### Spike evidence
- `spikes/spike-results.md` §S1 (stream-json needs `--verbose`), §S4 (kill method), §S5 (profiles), §S6 (dsh is a black box, so it is not used), §S7 (agy headless: `event` key, no turn limit, explicit `--print-timeout`, permissions soft-deny), §S8 (max turns), §SPK-10 (work/personal mapping)
- `CLAUDE.md` (repo root): how to call agy safely (stdin `/dev/null`, stdout to file, outer timeout, check diff after `SUCCESS`, S4 kill)

### Prior phase patterns
- `.planning/phases/05.5-work-board/05.5-CONTEXT.md`: most recent dashboard screen decisions (UI rules carried forward)
- `agent-brain/LIVE-CHECKS.md`: live-check pattern (§ 6 Phase 5.4, § 7 Phase 5.5)
- `scripts/dashboard-smoke.sh`, `scripts/dashboard-ui-lint.sh`: UI gates the Settings screen must pass
</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `agent-brain/src/roles/mod.rs` (`build_command`, `execute_role_staged`, `display_argv`): the place where the engine decides argv and env per role. Today it always uses `cfg.claude_bin`.
- `agent-brain/src/profile.rs` (`build_env`, `NESTED_SESSION_VARS`, `unset_env`): env construction. The DeepSeek env and the secret filtering hook in here.
- `agent-brain/src/stream.rs` (`classify`, `Summary::feed`, `Usage`, `UsageSource`): the Claude stream parser. agy needs a parallel branch.
- `agent-brain/src/runner.rs` (`supervise`, `cmd_kill`, `reconcile_stale_runs`): process group + S4 kill, reused for agy and the DeepSeek `claude`.
- `agent-brain/src/workflows.rs`: the loader for `workflows.json` (size cap, line/column errors, built-in `assets/workflows.default.json`). It is the pattern for the `teams.json` loader, and the Settings screen writes the same file.
- `agent-brain/src/autopilot/decide.rs` + `view.rs` (`StopReason`, `reason_word`): add the quota stop reason here.
- `agent-brain/src/telegram/` (`render.rs`, `callback.rs`, `compose.rs`): gate messages with buttons and the strict callback grammar. Extend it for the "re-run with team" button.
- `agent-brain/src/dashboard/api_workflows.rs`: currently GET only. The Settings screen adds write routes.
- `agent-brain/src/db.rs`: `runs.engine` already exists (default `'claude'`), and `runs` has `usage_source`.

### Established Patterns
- Config files live in `~/.config/agent-brain/` and are hand-editable, with Vietnamese errors. Secrets are kept out of committed files (`telegram.json` is 0600).
- Every UI action goes through the same Rust function as the CLI and Telegram. There is no separate code path.
- Schema changes are a decision gate with a backup (05.4 schema v3, `tasks.origin`).
- Phases are executed by gsd-executor in worktrees. Claude reviews the diff and re-runs the gates before merging (05.4, 05.5).

### Integration Points
- Giao việc screen and `start_new_task` (05.4): team picker.
- Task detail / pipeline screen (05.1, 05.2): engine shown per run, team change, `Xong` / `Trả lại` for the human engine, and the quota gate card.
- Thống kê screen (`api_stats.rs`, `stats.rs`): per-engine split.
- `projects_map.json`: a new optional `team` key per project.
</code_context>

<specifics>
## Specific Ideas

- Phase gate (ENG-01): one real task with team `tiet-kiem` runs end to end: Lead and QA on `claude`, Dev on `deepseek`, with the log and `runs.engine` correct.
- Simulating quota (ENG-02) uses a fake engine/fixture that emits the real quota error shape. A test never burns real quota.
- The user wants teams that serve **different purposes** (for example saving money, personal work, staff-led work). The description field is shown on the pickers.
- Human staff are for work the user hands to people who will not code with this tool. Their result is still reviewed through the git diff and/or a live test before it passes.
</specifics>

<deferred>
## Deferred Ideas

- Configurable auto-fallback rules (for example "Dev out of quota on claude → deepseek"), recorded and notified. Rejected for now: always ask (D-16).
- The dsh container as a black-box engine, only if E-09 fails and the user chooses it (D-02).
- Staff access through Telegram (multi-user bot), or any staff access to the dashboard. Rejected: the user presses `Xong` for them (D-20).
- Editing engine definitions (command, endpoint, key name) on the UI (D-08).
- **Phase 06.1 Media Branch:** MEDIA-01 … 03 with the direction in D-22. Connecting the real whiteboard pipeline comes after that.
</deferred>

---

*Phase: 06-multi-engine-teams*
*Context gathered: 2026-10-24*
