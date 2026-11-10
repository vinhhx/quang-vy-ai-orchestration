---
phase: "6"
slug: "multi-engine-media-branch"
status: verified
# threats_open = count of OPEN threats at or above workflow.security_block_on severity (the blocking gate)
threats_open: 0
asvs_level: 1
block_on: high
created: "2026-11-10"
---

# Phase 6 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.
> Register built from the 26 PLAN `<threat_model>` blocks (authored at plan time). Verified 2026-11-10 by gsd-security-auditor (ASVS L1) against the code on `main` at 6276881, including the post-phase quick plans.

---

## Trust Boundaries

| Boundary | Description | First declared in |
|----------|-------------|-------------------|
| user shell (key in env) → probe script → claude child | the key crosses into the claude process environment only | 06-01 |
| DeepSeek endpoint → probe outputs | remote text (results, errors) lands in files that get committed | 06-01 |
| probe script → agy (remote model with tools) | agy may edit or run things in its cwd | 06-02 |
| agy output → committed evidence | remote text and paths land in committed files | 06-02 |
| hand-edited teams.json / staff.json / config.json → loaders | untrusted user files with arbitrary content and size | 06-03 |
| process env → readiness | env may hold secrets; only presence may be observed | 06-03 |
| new binary → user's brain.db | the migration rewrites the file format | 06-04 |
| dashboard / Telegram / CLI → tasks.team_json | concurrent writers of one column | 06-04 |
| teams.json / task row → engine choice | which binary and env a role runs with | 06-05 |
| CLI flags → task creation | `--team` is user input | 06-05 |
| browser → POST settings routes | JSON bodies are untrusted; cookie + Host/Origin gate is the existing auth::gate | 06-06 |
| dashboard ↔ hand-edited files | another editor may change a file between read and write | 06-06 |
| serve env → `/api/engines` | the env may hold the DeepSeek key | 06-06 |
| engine stream / stderr → quota detector | untrusted text from the model provider decides a stop | 06-07 |
| dirty worktree → Dev continuation | files left by a stopped engine are fed to the next engine | 06-07 |
| server payload → DOM | team descriptions, staff notes and file error lines are user-controlled text | 06-08 |
| browser → settings write routes | the token cookie and Host/Origin gate protect POSTs (06-06) | 06-08 |
| agent-brain → agy (remote model with tools) | agy Dev can edit files in the project root with the granted mode | 06-09 |
| agy stdout file → parser / DB / log | untrusted JSON lines and response text | 06-09 |
| agy response → protocol files | for read-only roles agent-brain writes the model's text into `.plan.md` / reports | 06-09 |
| browser / Telegram bot → task action functions | team names and revs are untrusted input | 06-10 |
| concurrent writers → tasks.team_json | dashboard, Telegram and CLI can race | 06-10 |
| config.json (user file) → argv/env of a child | the user's own file, but it must never shape argv beyond argv[0] or inject env | 06-11 |
| serve/CLI process env → child env | holds every engine's key; each child may see at most its own engine's token | 06-11 |
| team (hand-edited teams.json, task row) → run seam | names an engine id that may be missing or profile-restricted | 06-11 |
| staff machine → remote branch → user's worktree | commits from outside the machine enter the task branch | 06-12 |
| agent-brain → remote (push) | a new network side effect (D-25) | 06-12 |
| query parameters → stats SQL | profile / since are untrusted | 06-13 |
| run metrics_json → pipeline text | staff id comes from stored metrics | 06-13 |
| Telegram callback_data → bot | untrusted strings from the Bot API, possibly replayed or stale | 06-14 |
| bot → shared task actions | the bot acts with the user's authority for the paired chat only | 06-14 |
| server payload → Giao việc DOM | team descriptions and file error lines are user-controlled text | 06-15 |
| browser → POST /api/tasks | `team` is validated server-side (06-10) | 06-15 |
| browser → human action routes | rev and action checks guard stale or forged presses | 06-16 |
| staff.json notes → DOM | user-written contact text | 06-16 |
| server payload → task screen | team names, descriptions, history and quota details are rendered | 06-17 |
| browser → team / quota-rerun routes | presses carry `rev`; the server decides | 06-17 |
| live-check script → user machine | the script starts a serve, creates repos and files; it must stay in its temp dir and never reach the real container | 06-19 |
| dry run → user's real brain.db | the only place this phase reads real data | 06-19 |
| docs → git history | committed text must not carry a token, key or personal path | 06-19 |
| LIVE-CHECKS P6-09 → real dsh container | user-run, root container with rw `/mnt/data` | 06-19 |
| user shell (key) → claude child → gateway | the only place a real key exists; the child gets it through env only | 06-20 |
| probe output → repo (raw evidence, fixtures) | model/endpoint output may echo the key | 06-20 |
| probe script → real container | user-run only; root container with rw /mnt/data and docker.sock | 06-20 |
| process env (keys) → gateway child | the only place a key value may live | 06-21 |
| model output → stream / stderr / files | a prompt-injected agent may echo env or write it into files | 06-21 |
| run artefacts → git history, brain.db, dashboard, Telegram | every persisted or displayed sink | 06-21 |
| dashboard / Telegram / CLI request → team binding | a user choice that can select any team | 06-22 |
| hand-edited teams.json / projects_map.json → loader | file content may name restricted, dsh or unknown engines | 06-22 |
| task team → work/personal profile boundary (SPK-10) | the access-control rule this plan enforces | 06-22 |
| agent-brain → docker CLI → root container | `docker exec` into a container that runs as root, mounts `/mnt/data` read-write and `docker.sock` | 06-23 |
| user / Telegram text → dsh prompt | untrusted text that must never become shell syntax | 06-23 |
| run stop paths (CLI, dashboard, Telegram, timeout, signals) → container processes | the host kill does not reach them (S6 4a) | 06-23 |
| config.json labels / ids → DOM | user-written strings rendered in the dashboard | 06-24 |
| serve env (keys) → engines payload → DOM | only variable names may cross | 06-24 |
| dsh text output → outcome classification | untrusted model text decides empty / quota / ok | 06-25 |
| outcome → autopilot decisions | a wrong outcome could let the autopilot continue on bad work | 06-25 |
| serve env (keys) → engines payload | readiness looks at key presence only | 06-26 |
| agent-brain → docker CLI (readiness probe) | a read-only status query on a root container | 06-26 |
| config pricing → budget / stats | user numbers that decide autopilot budget stops | 06-26 |

---

## Threat Register

| Threat ID | Plan | Category | Component | Severity | Disposition | Mitigation | Status |
|-----------|------|----------|-----------|----------|-------------|------------|--------|
| T-06-01 | 06-01 | Information Disclosure | DEEPSEEK_API_KEY in probe outputs / commits | high | mitigate | key only in `ANTHROPIC_AUTH_TOKEN` env of the claude child; `scrub_raw`; `grep -F` leak gate before any copy (exit 3); `scan-secrets.sh` CLEAN before staging; verify prints yes/no only | closed |
| T-06-02 | 06-01 | Information Disclosure | work-profile OAuth token sent to DeepSeek | medium | mitigate | `ANTHROPIC_AUTH_TOKEN` takes precedence over the saved login (research, llm-gateway docs); verify prints `api_key_source` and `init_model` so the user sees which credential answered; the profile dir is not modified | closed |
| T-06-03 | 06-01 | Denial of Service | runaway probe | low | mitigate | `timeout 180`, `--max-turns 4`, one retry at most, stdin /dev/null | closed |
| T-06-SC | all plans | Tampering | npm/pip/cargo installs | high | mitigate | no package installed; scripts use bash, python3 -I and the installed claude CLI only | closed |
| T-06-04 | 06-02 | Tampering / Elevation of Privilege | agy with permission flags editing outside the scratch repo | high | mitigate | cwd is a throwaway repo under `$SPIKE_WORK_ROOT`; skip flag only with `--with-skip` typed by the user; every call bounded by `timeout` and `--print-timeout` | closed |
| T-06-05 | 06-02 | Denial of Service | agy leftover processes | medium | mitigate | P8 records descendants and stops them by recorded PID only (S4), reports survivors; no kill by name | closed |
| T-06-06 | 06-02 | Information Disclosure | home path / account data in raw outputs | medium | mitigate | `scrub_raw` before verify; `scan-secrets.sh` CLEAN before staging | closed |
| T-06-07 | 06-03 | Denial of Service | teams/staff loaders | medium | mitigate | 256 KiB cap before parsing (`MAX_FILE_BYTES`), per-entry validation, builtins always available | closed |
| T-06-08 | 06-03 | Tampering | engine values in teams.json | high | mitigate | engines are an enum (`Engine::parse` exact names); QA forced to claude (D-06); human only on dev (D-24); commands/endpoints only in config.json (D-08) | closed (accepted risk) |
| T-06-09 | 06-03 | Information Disclosure | readiness / config | high | mitigate | Config stores the key variable NAME only; readiness checks presence; test asserts a planted value never appears in the serialized readiness | closed |
| T-06-10 | 06-03 | Tampering | config `engines.deepseek.base_url` | medium | mitigate | must start with `https://`; key_env restricted to `[A-Z_][A-Z0-9_]*` | closed |
| T-06-11 | 06-04 | Denial of Service | schema v4 makes the installed binary refuse brain.db | medium | mitigate | blocking-human decision (Task 1); backup + dry run on a copy and README note in 06-19; no test touches the live DB — drift: `phase6-live-check.sh --schema-dry-run` still expects user_version 4; current SCHEMA_VERSION is 11 | closed |
| T-06-12 | 06-04 | Tampering | concurrent team writes interleaving history | medium | mitigate | `set_task_team_cas` with NULL-safe `IS ?old`; `set_task_team_if_null` for creation; tests `team_cas_rules` | closed |
| T-06-13 | 06-04 | Repudiation | human work without a durable record | low | mitigate | `insert_human_run` stores times, staff and outcome as a finished run (D-21) | closed |
| T-06-14 | 06-05 | Elevation of Privilege | engine selection bypassing the profile boundary | high | mitigate | claude engine keeps `profile::build_env` (D-07); engines are an enum from teams.rs; adjacency test proves unchanged argv/env — drift: D-36 reversed for `claude_config_dir` entries by quick plan engine-claude-config-dir (2026-10-29) | closed |
| T-06-15 | 06-05 | Tampering | `--team` value | medium | mitigate | resolved against loaded teams before any branch/row; unknown → refused; only lead / auto --start accept it | closed |
| T-06-16 | 06-05 | Tampering | silent engine switch (D-16) | high | mitigate | a missing team refuses the step instead of falling back; resolution never picks another engine — drift: department team `phong-ban` picks a member engine with a printed reason (explicit feature, not a silent switch) | closed |
| T-06-17 | 06-05 | Denial of Wallet | non-claude runs priced as opus or not counted | medium | mitigate | engine-first pricing; unpriced engines cost 0 and are flagged by `is_priced` for the stats UI | closed |
| T-06-18 | 06-06 | Spoofing / Elevation of Privilege | new settings routes | high | mitigate | merged before the fallback inside `auth::gate` (token cookie, Host allow-list, Origin check); test `settings_routes_gated` (401) | closed |
| T-06-19 | 06-06 | Tampering | lost update / clobbered hand edit | medium | mitigate | version token compared under a process-wide lock right before rename (409 conflict); invalid files never written (409 invalid_file); order-preserving single-key edits | closed |
| T-06-20 | 06-06 | Tampering | partial or torn file on crash | medium | mitigate | temp file 0600 + fsync + rename + dir fsync; temp removed on failure; test no `*.tmp` left | closed |
| T-06-21 | 06-06 | Elevation of Privilege | writing engine command/endpoint via UI | high | mitigate | no route accepts engine definitions (D-08); `deny_unknown_fields` bodies; engines GET only — drift: mitigation text stale: POST /api/engines/models (3e265b3) now edits label/model/models; command/base_url/key_env still refused | closed |
| T-06-22 | 06-06 | Information Disclosure | key value in `/api/engines` | high | mitigate | readiness checks presence only; test greps the raw body for the planted value | closed |
| T-06-23 | 06-06 | Denial of Service | oversized bodies | low | mitigate | existing `DefaultBodyLimit::max(65536)`; loaders cap files at 256 KiB | closed |
| T-06-24 | 06-07 | Denial of Wallet | silent engine switch or retry loop after quota | high | mitigate | detector only stops (exit 5); no auto retry or switch anywhere (D-16); autopilot D2 stops with het_quota; tests assert exactly one run | closed (accepted risk) |
| T-06-25 | 06-07 | Tampering | partial diff from a stopped engine smuggling out-of-scope files | medium | mitigate | continuation allows only plan files, protocol files and `.handover.md`; the post-run scope check still unions committed and uncommitted changes | closed |
| T-06-26 | 06-07 | Repudiation | quota stop without evidence | low | mitigate | raw lines stay in `events`; `metrics_json.quota` records engine, signal and reset time; `runs.error` keeps the detail | closed |
| T-06-27 | 06-07 | Denial of Service | false-positive quota on a recovered 429 | medium | mitigate | a successful final result always wins; 529 overloaded excluded; negative unit tests | closed |
| T-06-28 | 06-08 | Tampering (XSS) | descriptions, notes, error lines rendered on Cài đặt | high | mitigate | Alpine `x-text` only, no `x-html` (lint grep), strict CSP unchanged, no inline script/style | closed |
| T-06-29 | 06-08 | Information Disclosure | engine section | high | mitigate | only command, variable name and model are rendered; JS check asserts a planted value never appears; staff notes helper warns not to store passwords | closed |
| T-06-30 | 06-08 | Tampering | lost drafts / overwritten edits | low | mitigate | conflict keeps the draft and reloads; invalid file disables writes; one editor per section | closed |
| T-06-31 | 06-09 | Elevation of Privilege | agy permission flags | high | mitigate | flag only for Dev, narrowest mode from the probe (D-26); unit test for non-Dev argv; permission_mode compared with the expected value | closed |
| T-06-32 | 06-09 | Tampering | agy Dev editing outside scope, `.planning/` or secret paths | high | mitigate | post-run union of committed + uncommitted changes vs plan files; `.planning/` and `report::sensitive_path` always violations for agy; no auto-revert, outcome `protocol_violation` | closed |
| T-06-33 | 06-09 | Repudiation / Tampering | soft-denied tools reported as success | medium | mitigate | empty changed set on Dev → `agy:empty`, exit 4, autopilot `vướng mắc` | closed |
| T-06-34 | 06-09 | Tampering | response text written into protocol files | medium | mitigate | only for read-only roles, only on SUCCESS; the existing Lead/review validators (meta, plan validation, front matter, other-files check) decide | closed |
| T-06-35 | 06-09 | Information Disclosure | claude / DeepSeek credentials reaching agy | high | mitigate | agy env strips every `ANTHROPIC_*`, `CLAUDE_CONFIG_DIR` and the key variable; test on env names | closed |
| T-06-36 | 06-09 | Denial of Service | agy hang or leftover processes | medium | mitigate | explicit `--print-timeout`, runner hard timeout, S4 kill by recorded PID/group, conditional post-exit group sweep; no kill by name | closed |
| T-06-37 | 06-10 | Tampering | stale UI acting on a changed task | medium | mitigate | `rev` check (now including team_json) + CAS on team_json; 409 stale | closed |
| T-06-38 | 06-10 | Elevation of Privilege | Telegram re-run treated as plan approval | high | mitigate | quota_rerun never touches approval; `TELEGRAM_APPROVE_REFUSED` path unchanged; shared function keeps one rule set | closed |
| T-06-39 | 06-10 | Tampering | team change during a live run or human wait | medium | mitigate | `build_change_team` refusals (D-13, D-29) checked server-side before the CAS write | closed |
| T-06-40 | 06-10 | Tampering | unknown team names | low | mitigate | validated against loaded teams (422); `deny_unknown_fields` bodies behind the existing auth gate | closed |
| T-06-41 | 06-10 | Denial of Wallet | automatic re-run after quota | high | mitigate | re-run only on an explicit action; no automatic switch (D-16) | closed (accepted risk) |
| T-06-45 | 06-11 | Information Disclosure | key of engine A inherited by a claude / native / agy / dsh / gateway-B child | high | mitigate | every arm removes `all_key_envs()` from its env; only the gateway arm injects its own value as `ANTHROPIC_AUTH_TOKEN`; `tiet_kiem_routes_lead_dev_qa` and `two_anthropic_endpoints` assert `env_names` per record | closed |
| T-06-46 | 06-11 | Information Disclosure | work OAuth sent to a gateway / profile settings env overriding the injected base URL (Pitfall 8) | medium | mitigate | `ANTHROPIC_*` inserted after `build_env`; profile `unset_env` still applied; residual documented and detected only by the user-run E-09 (06-20) | closed |
| T-06-73 | 06-11 | Tampering | config argv template, `env` map or `${VAR}` expansion reaching a child (Pitfall A1/A3) | high | mitigate | `command` is executable-only (extra elements refused); unknown keys refused; argv built only by the role builders; test asserts `--settings` and `--append-system-prompt` in the gateway argv | closed |
| T-06-74 | 06-11 | Elevation of Privilege | pinned / `allowed_profiles` engine running on a project of another profile (SPK-10) | high | mitigate | one pure `profile_refusal` called at the run seam before any run row; tests `eng10_seam_backstop` and `pinned_engine_runs_on_personal_project` — drift: same D-36 note as T-06-14 | closed |
| T-06-75 | 06-11 | Tampering / Repudiation | a missing engine id silently resolved to another endpoint (D-43) | medium | mitigate | `plan_for` refuses an unknown id before any run row with the exact text; test `unknown_engine_refused_before_run` | closed |
| T-06-47 | 06-12 | Tampering | malicious or rewritten staff branch | high | mitigate | fetch + `merge --ff-only` only (no pull/reset/rebase/force); diverged branch refused; validated `task/<slug>` ref; scope, `.planning/` and secret-path checks; code-review/QA gates decide (D-20) | closed |
| T-06-48 | 06-12 | Tampering | local history overwritten by push | medium | mitigate | push never uses `--force`; push failure is non-fatal and shows the command (D-25) | closed |
| T-06-49 | 06-12 | Repudiation | human work without a record | low | mitigate | `insert_human_run` with staff, times, base/head and outcome (D-21) | closed |
| T-06-50 | 06-12 | Denial of Service | phantom running rows / lost runs | medium | mitigate | the wait is a marker, never a `running` row (reconcile_stale_runs untouched) | closed |
| T-06-51 | 06-12 | Spoofing | staff driving the dashboard or bot | low | accept | staff have no access by design (D-20); only the user runs `human done` / `human return` | closed (accepted risk) |
| T-06-52 | 06-13 | Tampering (SQL injection) | stats queries | medium | mitigate | bound parameters only (`?1`, `?2`), existing profile check and since parser unchanged | closed |
| T-06-53 | 06-13 | Repudiation | misattributed costs | low | mitigate | engine-first stored costs, `priced` flag, tests with unpriced engines | closed |
| T-06-54 | 06-13 | Information Disclosure | staff ids in pipeline | low | accept | only the staff id (no notes/contact) leaves the server; staff data is the user's own and shown on their dashboard | closed (accepted risk) |
| T-06-55 | 06-14 | Tampering | forged / stale team buttons | high | mitigate | strict `decode_team` (indexes only, 4-hex list version, 16-hex rev, 64 bytes); ver and rev re-checked on every press; invalid → no action | closed |
| T-06-56 | 06-14 | Denial of Wallet | accidental re-run from one tap | medium | mitigate | second confirm tap required (D-28); cancel restores; `Thử lại sau` writes nothing | closed |
| T-06-57 | 06-14 | Elevation of Privilege | re-run used as plan approval for Telegram tasks | high | mitigate | quota_rerun never approves; `TELEGRAM_APPROVE_REFUSED` path unchanged; test with a Telegram-origin task | closed |
| T-06-58 | 06-14 | Spoofing | callbacks from other chats | high | mitigate | existing `authz` (paired chat and sender only); test `quota_stranger_ignored` | closed |
| T-06-59 | 06-15 | Tampering (XSS) | descriptions / summaries on Giao việc | high | mitigate | `x-text` only, no `x-html`, CSP unchanged; lint grep | closed |
| T-06-60 | 06-15 | Tampering | forged team names in the request | low | mitigate | server resolves and refuses unknown teams (422) before any job (06-10) | closed |
| T-06-61 | 06-16 | Spoofing / Elevation of Privilege | Xong / Trả lại triggered by someone other than the user | medium | mitigate | routes only on the token-gated dashboard; no Telegram path (D-20); `rev` + enabled-action checks | closed |
| T-06-62 | 06-16 | Tampering | actions while a person works | medium | mitigate | run_dev / run_qa / relead / change_team refused while a marker exists; server-side checks | closed |
| T-06-63 | 06-16 | Information Disclosure | staff notes on the dashboard | low | accept | notes are the user's own contact data shown only on their dashboard (UI-SPEC C18); never in prompts, logs or Telegram | closed (accepted risk) |
| T-06-64 | 06-17 | Tampering (XSS) | team text, quota detail, history on the task screen | high | mitigate | `x-text` only, no `x-html`, CSP unchanged; lint grep | closed |
| T-06-65 | 06-17 | Denial of Wallet | accidental re-run | medium | mitigate | dialog with explicit team choice; `Thử lại sau` makes no request (JS check asserts no API call) | closed |
| T-06-66 | 06-17 | Tampering | stale screen acting | low | mitigate | every POST sends `rev`; 409 stale reloads the screen (existing pattern) | closed |
| T-06-67 | 06-18 | Tampering (XSS) | staff notes / push error text in the callout | high | mitigate | `x-text` only, no `x-html`, CSP unchanged; lint grep | closed |
| T-06-68 | 06-18 | Information Disclosure | push command / error text | low | mitigate | the command is the fixed `git push -u origin {branch}`; errors are git's first line (no credentials: git uses the user's own helper; GIT_TERMINAL_PROMPT=0) | closed |
| T-06-69 | 06-18 | Repudiation | unpriced costs shown as zero | low | mitigate | `chưa có giá` instead of `≈ $0`; footnote explains | closed |
| T-06-70 | 06-19 | Tampering / Information Disclosure | live-check environment | high | mitigate | private HOME / XDG / AGENT_BRAIN_DB / AGENT_BRAIN_CONFIG under mktemp; planted keys only; git status compared before/after; stop by recorded PID only | closed |
| T-06-71 | 06-19 | Tampering | user's real brain.db in the dry run | high | mitigate | `cp -p` of the DB and its `-wal` / `-shm` into a temp dir (SQLite never opens the real path); migration runs on the copy; directory listing + sha256 compared before/after (`real_dir_unchanged=yes`) — drift: same stale-check note as T-06-11 | closed |
| T-06-72 | 06-19 | Information Disclosure | session token / key in output or docs | medium | mitigate | cookie via `curl -K -` on stdin; key checks print yes/no and file names only; diff gate rejects home paths, token-like and key-like strings | closed |
| T-06-96 | 06-19 | Tampering / Denial of Service | the automated live check reaching the real docker / container | high | mitigate | dsh entry points at `tests/fakes/fake_docker.py`; awk acceptance forbids real docker subcommands in the script | closed |
| T-06-97 | 06-19 | Tampering | P6-09 real dsh run leaving root-owned files or touching a work project | medium | mitigate | procedure limited to a scratch personal repo with `allowed_profiles: ["personal"]`; cleanup from inside the container as spikes/s6 does; ownership and `git status` checked and recorded | closed |
| T-06-76 | 06-20 | Information Disclosure | gateway / DeepSeek key in raw evidence or fixtures | high | mitigate | key only via `ANTHROPIC_AUTH_TOKEN` in a subshell; `scrub_raw`, `grep -F` leak gate before any copy, `scan-secrets.sh` after; a leak writes nothing (offline self-test proves the refusal) | closed |
| T-06-77 | 06-20 | Tampering / Repudiation | one endpoint's probe overwriting another's evidence | medium | mitigate | per-name raw directories and fixture names; the copy step removes only its own `raw/{name}` (path guard); offline test with two names | closed |
| T-06-78 | 06-20 | Denial of Service / Information Disclosure | kill-wrapper probe leaving processes in the container, or reading the container env | medium | mitigate | user-run only; signals only to PIDs re-matched by marker and args; no container lifecycle command; no inspect; scratch removed from inside the container | closed |
| T-06-42 | 06-21 | Information Disclosure | key in stream.jsonl / events / terminal / job output | high | mitigate | `secrets::redact_line` before the file write and before the observer, main and drain loops; `eng05_key_absent_everywhere`, `eng05_job_output_redacted` | closed |
| T-06-43 | 06-21 | Information Disclosure | key in stderr.txt or runs.error | high | mitigate | `redact_file` on every exit path incl. kill and timeout; error text built from redacted data and re-redacted; `eng05_killed_run_redacted`, `eng05_timeout_run_redacted`, `eng05_quota_detail_redacted` | closed |
| T-06-44 | 06-21 | Information Disclosure / Tampering | agent writes a key into `.plan.md` or code and agent-brain commits it | high | mitigate | `scan_for_secrets` over changed + committed paths before returning to the role; outcome protocol_violation, exit 3, nothing committed | closed |
| T-06-79 | 06-21 | Information Disclosure | key of engine B reaching runs of engine A, claude or agy (N keys) | high | mitigate | per-run redact list from that run's plan + env stripping of every `key_env` (06-11); `eng05_two_keys_isolated` | closed |
| T-06-80 | 06-22 | Elevation of Privilege | a bind path (Giao việc, Telegram, CLI, autopilot, change team, quota re-run, project default) putting a personal-only engine on a work task | high | mitigate | one pure `team_profile_refusal` called at every bind point; seven `eng10_*` tests, one per path; run-seam backstop from 06-11 remains — drift: same D-36 note as T-06-14 | closed |
| T-06-81 | 06-22 | Tampering | a refusal after a side effect (team written, job queued, branch created) | high | mitigate | refusal placed before `lead_preflight`, `write_team_change` and job creation; awk-order acceptance checks; tests assert team_json unchanged and no new run/job — drift: same D-36 note as T-06-14 | closed |
| T-06-82 | 06-22 | Tampering | a dsh engine in a read-only role (no read-only mode for dsh) | medium | mitigate | D-37 in the loader and on save (`catalog_errors`); dsh::plan refuses non-dev at the run seam (06-23) | closed |
| T-06-85 | 06-23 | Tampering | shell injection through the prompt into `sh -c` | high | mitigate | constant wrapper text + hex marker; prompt only as positional `$2`; `wrapper_exact` and `dsh_argv_and_stdin` assert the argv | closed |
| T-06-86 | 06-23 | Denial of Service | dsh processes left running after Kill / timeout / Ctrl+C | high | mitigate | `stop_in_container` at every stop site and in `cmd_kill` via the persisted `metrics_json.dsh`; three kill tests on a fake that re-parents like a real container; residual (SIGKILL of agent-brain) warned by the next preflight | closed |
| T-06-87 | 06-23 | Tampering / Denial of Service | signalling an unrelated container process (name match, PID reuse) | high | mitigate | token-exact marker match, descendants by ppid, re-match pid + args before every signal; no name-based tool, no lifecycle command (acceptance greps) | closed |
| T-06-88 | 06-23 | Elevation of Privilege | root container with rw `/mnt/data` and `docker.sock` editing other projects | high | mitigate | dsh is Dev-only (D-37), the example and docs restrict it to personal projects (`allowed_profiles`, D-38, enforced by 06-11/06-22), same-path mount preflight; blast radius documented in README (06-19) and checked in P6-09 | closed |
| T-06-89 | 06-23 | Information Disclosure | `docker inspect` printing the container environment (secrets) | medium | mitigate | inspect only with `--format '{{json .Mounts}}'`; the fake refuses inspect without `--format`; awk acceptance | closed |
| T-06-90 | 06-23 | Tampering | project outside the container's same-path view → dsh edits a different tree | medium | mitigate | `same_path_bind_covers` preflight refuses before any run row; `dsh_mount_preflight` | closed |
| T-06-91 | 06-24 | Information Disclosure | a key value, argv or endpoint query rendered on Settings / Giao việc | high | mitigate | rows built from a field whitelist; smoke serves with two planted keys and greps both DOM dumps; js-check asserts names only | closed |
| T-06-92 | 06-24 | Tampering | an engine edit control or route appearing on the dashboard (breaks D-08) | medium | mitigate | the section's only control stays `Kiểm tra lại`; ui-lint counts interactive tags; no new fetch other than the existing GETs | closed (accepted risk) |
| T-06-93 | 06-24 | Tampering (XSS) | a malicious `label` from config.json | low | mitigate | `x-text` only, `x-html` and inline style forbidden (acceptance grep), Alpine CSP build | closed |
| T-06-94 | 06-25 | Repudiation / Elevation of Privilege | a dsh run that did nothing, or wrote `.planning/` / secret paths, accepted as done | medium | mitigate | kind-based extra checks for dsh, `dsh:empty` stop in CLI, autopilot and task screen; `dsh_dev_outcomes`, `dsh_empty_stops_autopilot` | closed |
| T-06-95 | 06-25 | Spoofing | model text faking a quota notice to stop work, or a real quota hidden | low | mitigate | text quota only with a non-zero exit; exit-0 text never quota (`dsh_quota_text_exit0_not_quota`); a stop only waits for the user, never switches engine | closed |
| T-06-83 | 06-26 | Information Disclosure | key value or endpoint secret in `GET /api/engines` | high | mitigate | rows carry variable NAMES, labels and the user's `verified` text only; tests grep the raw body for both planted keys | closed |
| T-06-84 | 06-26 | Tampering / Denial of Service | readiness probe acting on the container | medium | mitigate | one argv `ps --filter … --format {{.Status}}`, stdin null, 3 s cap, only its own child handle is killed; awk acceptance forbids lifecycle/inspect words in the probe; tests inject the probe | closed |

*Status: open · closed · open — below high threshold (non-blocking)*
*Severity: critical > high > medium > low — only open threats at or above workflow.security_block_on count toward threats_open*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-06-01 | T-06-51 | Staff have no dashboard or bot access by design (D-20); only the user runs `human done` / `human return`. Declared `accept` at plan time. | user (plan 06-12) | 2026-10-27 |
| AR-06-02 | T-06-54 | Only the staff id leaves the server, shown on the user's own dashboard. Declared `accept` at plan time. | user (plan 06-13) | 2026-10-27 |
| AR-06-03 | T-06-63 | Staff notes are the user's own contact data, shown only on their dashboard, never in prompts, logs or Telegram (UI-SPEC C18). Declared `accept` at plan time. | user (plan 06-16) | 2026-10-27 |
| AR-06-04 | T-06-08 | The "QA must be claude" clause (D-06/D-39) was removed by user decision: quick plan `team-any-engine-any-role` (D-46, 2026-10-29) lets every role use any engine. Still enforced: human/dsh only on dev, engine command and endpoint only in `config.json`, unknown engine refused at the run seam. | user | 2026-11-10 |
| AR-06-05 | T-06-24 | "No automatic retry after quota" is now opt-in-relaxed: `orchestrator.auto_resume_quota` (default `false`, quick plans `intake-queue-orchestrator`, `e2e-findings-fixes` C3) re-runs a quota-stopped task once after the wait, never switching engine (`scheduler.rs` `handle_quota_stops`). | user | 2026-11-10 |
| AR-06-06 | T-06-41 | `orchestrator.restore_team_after_quota` (default `false`, `e2e-findings-fixes` C3 item 4) switches the team back after a quota reset (`autopilot/mod.rs` `restore_team_after_quota`); the explicit re-run path still needs a user action and never approves a plan. | user | 2026-11-10 |
| AR-06-07 | T-06-92 | Commit `3e265b3` (2026-11-04) added `POST /api/engines/models` and a model editor on Settings; it edits `label`, `model`, `models` only, with `deny_unknown_fields` (test `engines_models_save_and_get` rejects `command`). No plan or decision recorded it: D-08 and the `settings.html` line "Engine chỉ xem" are stale and should be updated. | user | 2026-11-10 |

*Accepted risks do not resurface in future audit runs.*

---

## Audit Evidence (gsd-security-auditor, 2026-11-10)

Targeted suites re-run and passing: `engine_guard` 10, `engine_routing` 22, `engine_secrets` 11, `engine_quota` 9, `engine_agy` 33, `engine_dsh` 14, `telegram_quota` 9, `dashboard_settings` 19, `task_team` 17, `db_migration` 30, `human_engine` 13, `human_dashboard` 9, `dashboard_stats` 11, `db_stats` 8, `e09_fixtures` 1, `dashboard_http` 9; `E09_OFFLINE=1 bash spikes/e09/run.sh` → `E09 offline OK`; `spikes/lib/scan-secrets.sh` → OK. Full suite the same day: 83 suites, 1704 passed.

Unregistered post-phase surfaces (no threat entry, both behind `auth::gate`): `POST /api/engines/models`; the opt-in orchestrator auto-resume and team restore.

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-11-10 | 98 | 98 (91 verified, 7 accepted) | 0 | gsd-security-auditor (ASVS L1) + user acceptance |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-11-10

## Security Audit 2026-11-10

| Metric | Count |
|---|---|
| Threats found | 98 |
| Closed | 98 |
| Open | 0 |
