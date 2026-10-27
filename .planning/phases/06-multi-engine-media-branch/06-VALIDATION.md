---
phase: "6"
slug: "multi-engine-media-branch"
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: "2026-10-26"
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Seeded from `06-RESEARCH.md` § Validation Architecture. The per-task map is filled from the PLAN.md files once planning passes the checker.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (lib unit tests and integration tests on `tests/common::Sandbox` with fake claude), `node scripts/dashboard-js-check.mjs` (vm harness), bash UI gates, headless-Chrome smoke with CDP keys |
| **Config file** | `agent-brain/Cargo.toml`. The sandbox config is written by `Sandbox::write_config`, which points `claude_bin` at the fake claude. |
| **Quick run command** | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib -- <module filter>` and `node scripts/dashboard-js-check.mjs` |
| **Full suite command** | `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1`, then `bash scripts/dashboard-ui-lint.sh`, `bash scripts/build-css.sh --check`, `node scripts/dashboard-js-check.mjs`, and `bash scripts/dashboard-smoke.sh overview assign task task-stop stats projects board settings --keys` |
| **Estimated runtime** | about 240 seconds for the full suite; under 30 seconds for a module-filtered lib run |

---

## Sampling Rate

- **After every task commit:** run the quick command filtered to the task's module, plus the JS check for UI tasks.
- **After every plan wave:** run that wave's integration tests, plus the UI gates for UI waves.
- **Before `/gsd-verify-work`:** the full suite must be green, `build-css.sh --check` must report `app.css khớp nguồn`, and the user-run live checks in LIVE-CHECKS § 8 must be done.
- **Max feedback latency:** 300 seconds.

---

## Requirement → Test Map (from research)

| Req ID | Behavior | Test Type | Automated Command | File Exists |
|--------|----------|-----------|-------------------|-------------|
| ENG-01 | Mixed team (claude lead/qa, deepseek dev where the deepseek `command` is the fake claude): `runs.engine` per run, header `engine …`, `metrics.team` | integration | `cargo test --test engine_routing` | ❌ W0 |
| ENG-02 | Quota fixtures (claude, deepseek 402/429, agy) give outcome `quota`, `EXIT_QUOTA` and autopilot `HetQuota`; a transient 429 does not stop the task | unit + integration | `cargo test --lib quota`; `cargo test --test engine_quota` | ❌ W0 |
| ENG-03 | A re-run after quota keeps the partial diff, the prompt mentions the diff and `.handover.md`, and no autopilot round is consumed | integration | `cargo test --test engine_quota eng03` | ❌ W0 |
| ENG-04 | Stats `engines[]` splits tokens and cost; an unpriced engine is flagged | integration | `cargo test --test dashboard_stats`, `--test db_stats` | ✅ extend |
| ENG-05 | A planted key is absent from stream.jsonl, events.raw_json, stderr.txt, job output, `runs.error` and git history | integration | `cargo test --test engine_secrets` | ❌ W0 |
| ENG-06 | The `fake_agy.py` stream is parsed and usage mapped; the Dev permission flag applies to Dev only; an empty diff is suspicious; an out-of-scope diff is a protocol violation | integration | `cargo test --test engine_agy` | ❌ W0 |
| ENG-07 | QA must be claude (refused by the loader and by Settings save); "Cần sửa" SQL by Dev engine, including human | unit + integration | `cargo test --lib teams`; `--test dashboard_stats` | ❌ W0 / extend |
| TEAM-* / SET-* / HUM-* / QUOTA-* | New requirement IDs defined by the planner (research proposal: TEAM-01..03, SET-01..05, HUM-01..03, QUOTA-01) | see research table | `--test task_team`, `--test dashboard_settings`, `--test human_engine`, `--test telegram_actions` | ❌ W0 / extend |

### Amendment rows (D-30…D-44, from `06-RESEARCH.md` § Amendment Research)

| Req ID | Behavior | Test Type | Automated Command | File Exists |
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

ENG-01 and ENG-05 rows above that name `deepseek` are superseded by these rows where they conflict: engines are configured ids (D-30), the Dev engine of the mixed-team gate is any `anthropic` entry.

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (filled after the plans pass the checker) | | | | | | | | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/fakes/fake_agy.py`: emits `init` / `step_update` / `result`, slow writes, error and exit codes, and an optional soft-deny SUCCESS with no edits
- [ ] Extend the `fake_claude.py` recorder: record env var NAMES and boolean matches for `ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` / the key var, never values; add a scenario that echoes a planted env value (ENG-05)
- [ ] `Sandbox` helpers: `write_teams`, `write_staff`, a config `engines` override, and a bare-remote plus staff-clone helper
- [ ] Fixtures: `tests/fixtures/stream/{claude_quota_*,deepseek_*,agy_*}.jsonl`
- [ ] New test files from the map above; update `db_migration.rs` version assertions to 4
- [ ] A `settings` section in `dashboard-js-check.mjs` and a `settings` case in `dashboard-smoke.sh`

- [ ] (amendment) `tests/fakes/fake_docker.py` and `tests/fakes/fake_dsh.py`, plus a `Sandbox` helper that puts the shim directory first on `PATH` and writes the `dsh` engine entry
- [ ] (amendment) `fake_claude.py`: `env_names`, `anthropic_base_url`, `auth_token_matches_expected`, `echo_env`, with per-call `FAKE_CLAUDE_EXPECT_TOKEN`
- [ ] (amendment) `Sandbox::write_engines(list)` and a personal scratch project helper
- [ ] (amendment) fixtures `tests/fixtures/stream/anthropic_e09_<name>_{ok,auth_error}.jsonl` from the user-run E-09 per endpoint (tests skip with a printed line when absent)
---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| E-09 per `anthropic` endpoint and per model (gateway first, D-34): stream-json, usage, tool use, file edit | ENG-01, ENG-11, D-34 | Needs the user's key and a real network call | User-run E-09 plan (`E09_KEY_ENV` / `E09_NAME`), LIVE-CHECKS § 8 |
| dsh black box on a scratch personal repo: kill inside the container, file ownership, first real dsh Dev task | ENG-09, D-35, D-38 | Needs the real container | User-run, S6-style, LIVE-CHECKS § 8 |
| `claude-max` refused on a work project, runs on a personal project | ENG-10, D-36, D-42 | Real profiles | LIVE-CHECKS § 8 |
| agy 1.4.7 probe: event shape, narrow permission mode for Dev (D-26), soft-deny behaviour | ENG-06, D-26 | Needs the user's agy login | User-run probe plan |
| End-to-end `tiet-kiem` task (claude Lead/QA, deepseek Dev) | ENG-01 phase gate | Real engines and quota | LIVE-CHECKS § 8 |
| Settings screen look in light and dark themes, and at 1024 / 768 / 480 px | SET-* | Visual judgement | LIVE-CHECKS § 8 browser steps |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 300s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
