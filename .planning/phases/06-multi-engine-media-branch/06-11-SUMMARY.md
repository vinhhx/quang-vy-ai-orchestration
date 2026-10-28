---
phase: 06-multi-engine-media-branch
plan: 11
subsystem: engines
tags: [rust, agent-brain, engine-list, anthropic-endpoint, profile-guard, config]

requires:
  - phase: 06-multi-engine-media-branch
    provides: "engine seam (06-05), agy engine (06-09), teams loader (06-03), pricing keyed by engine (06-07)"
provides:
  - "config.json `engines` as an array of {id, kind, ...} (anthropic | agy | dsh), legacy object shape and absent key still load"
  - "Engine::Named(id): teams, runs.engine, pricing keys and readiness rows are keyed by the engine id"
  - "anthropic kind: claude on the host against any Anthropic-format endpoint, own token only, guard argv intact"
  - "engine::profile_refusal + run-seam backstop (SPK-10); D-40 unverified mark; D-43 unknown-id refusal; D-03 missing-key refusal"
  - "dsh dispatch stub (dsh::plan, DshRun) so plans 06-21..06-26 never change plan_for again"
  - "docs/config.engines.example.json in the final, loader-tested shape"
affects: [06-12, 06-13, 06-19, 06-20, 06-21, 06-22, 06-23, 06-24, 06-25, 06-26]

actuals:
  tokens: 34000
  tasks: 2
  commits: 4

plan_head_before: 129b36fad7e36260881b7cafbee5f70e7acfcbee
plan_head_after: 381d62257a562fdcb149791d19917b23b7d36be2

tech-stack:
  added: []
  patterns:
    - "closed engine enum promoted to an id-keyed catalog (Engine::Named) with kind dispatch in one function (plan_for)"
    - "every child env has every engine's key variable NAME removed; only the gateway arm re-injects its own value under ANTHROPIC_AUTH_TOKEN"
    - "config value never becomes argv or env except derived fields: `command` is an executable only, unknown keys per kind are refused"

key-files:
  created:
    - agent-brain/src/dsh.rs
  modified:
    - agent-brain/src/config.rs
    - agent-brain/src/engine.rs
    - agent-brain/src/lib.rs
    - agent-brain/src/roles/mod.rs
    - agent-brain/src/teams.rs
    - docs/config.engines.example.json
    - tests/fakes/fake_claude.py
    - agent-brain/tests/engine_routing.rs

key-decisions:
  - "`Engine::Named(id)` replaces the closed Deepseek/Agy variants; `deepseek` and `agy` survive only as the two default catalog entries (absent key or legacy object shape)"
  - "A user array is used exactly as written, never merged with the defaults; no config file is ever rewritten"
  - "The claude arm and the agy arm also strip every engine's key variable, not just the gateway arm (T-06-45)"
  - "Key value is trimmed and a blank value counts as missing; the refusal happens in plan_for, before any run row or spawn"
  - "`command` accepts a string or a one-element array; a string with spaces is NOT rejected (argv[0] is never shell-parsed), extra array elements are"
  - "Run-seam backstop runs after resolve_for_run and before build_env, so a hand-edited team row cannot reach a spawn"

patterns-established:
  - "Run-seam order: resolve_for_run -> profile_refusal -> build_env -> plan_for(.., cwd) -> run dir -> header -> spawn"
  - "Fake claude records env NAMES, ANTHROPIC_BASE_URL and a token-matches boolean, never the token"

requirements-completed: [ENG-01, ENG-08, ENG-10, ENG-11]

coverage:
  - id: D1
    description: "engines accepts an array of {id, kind, ...}; several entries of one kind load side by side; the legacy object shape and an absent key load into deepseek + agy; every field rule has its exact error text"
    requirement: ENG-08
    verification:
      - kind: unit
        ref: "agent-brain/src/config.rs#config::tests::engine_list"
        status: pass
      - kind: unit
        ref: "agent-brain/src/config.rs#config::tests::engine_list_errors"
        status: pass
      - kind: unit
        ref: "agent-brain/src/config.rs#config::tests::engines_defaults"
        status: pass
      - kind: unit
        ref: "agent-brain/src/config.rs#config::tests::kind_of_unknown_is_none"
        status: pass
    human_judgment: false
  - id: D2
    description: "a tiet-kiem-shaped team runs Lead and QA on claude and Dev on a configured gateway id, each run with its own argv and env; two gateways in one config never see each other's key"
    requirement: ENG-01
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#tiet_kiem_routes_lead_dev_qa"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#two_anthropic_endpoints"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#legacy_object_shape_still_routes"
        status: pass
      - kind: unit
        ref: "agent-brain/src/engine.rs#engine::tests::anthropic_plan_builds_argv_and_env"
        status: pass
    human_judgment: false
  - id: D3
    description: "missing key, unknown engine id and a profile-restricted engine on the wrong project are refused before any run row; QA stays claude"
    requirement: ENG-10
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#anthropic_missing_key_refused"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#unknown_engine_refused_before_run"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#eng10_seam_backstop"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#pinned_engine_runs_on_personal_project"
        status: pass
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#qa_on_list_id_refused"
        status: pass
      - kind: unit
        ref: "agent-brain/src/engine.rs#engine::tests::profile_refusal_rules"
        status: pass
    human_judgment: false
  - id: D4
    description: "an endpoint without E-09 evidence runs but is marked in the header and in metrics_json.unverified; one with `verified` is not"
    requirement: ENG-11
    verification:
      - kind: integration
        ref: "agent-brain/tests/engine_routing.rs#unverified_endpoint_marked"
        status: pass
    human_judgment: false
  - id: D5
    description: "docs/config.engines.example.json is the final shape and loads through config_from_value"
    requirement: ENG-08
    verification:
      - kind: unit
        ref: "agent-brain/src/config.rs#config::tests::example_engines_file_loads"
        status: pass
    human_judgment: false
  - id: D6
    description: "Against a REAL gateway endpoint the claude argv/env built here actually reaches the model (profile settings.json env block can still override ANTHROPIC_*, Pitfall 8)"
    verification: []
    human_judgment: true
    rationale: "Every test runs against the fake claude; only the user-run E-09 of plan 06-20 can see a real endpoint or a profile settings.json env override"

duration: 45min
completed: 2026-10-28
status: complete
---

# Phase 6 Plan 11: Engine list in config.json and the anthropic kind Summary

**`config.json` `engines` becomes a user-defined list keyed by id (kinds anthropic, agy, dsh); a team role naming an `anthropic` entry runs claude against that endpoint with only its own token and the guard argv intact, and the run seam refuses a missing key, an unknown id and a profile-restricted engine on the wrong project before any run row.**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-10-28T01:10Z (approximate; first commit 01:32Z)
- **Completed:** 2026-10-28T01:57Z
- **Tasks:** 2 (1 tracer, 1 auto; both `tdd="true"`)
- **Files modified:** 9 (1 created)

## Accomplishments

- Engine catalog: `EngineKind`, `AnthropicDef`, `AgyDef`, `DshDef`, `EngineSpec`, `EngineDef`, `EnginesCfg {list}` with `defaults/get/kind_of/all_key_envs/ids`; array loader implements every row of the field-rule table with the exact Vietnamese texts; the legacy object loader keeps its rules and texts and now yields the two default entries.
- `Engine::{Claude, Named(id), Human}`; `plan_for(.., cwd)` dispatches on the entry kind. The `anthropic` arm changes only argv[0] and the value after `--model`, injects `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` and the `compat` knobs after `build_env`, and strips every `key_env` plus `ANTHROPIC_API_KEY`, `ANTHROPIC_MODEL`, the three `ANTHROPIC_DEFAULT_*_MODEL` and `CLAUDE_CODE_SUBAGENT_MODEL`. The claude, native, agy and dsh arms strip every engine's key variable.
- Run seam: D-03 missing key, D-43 unknown id, D-36/D-42 profile guard (`engine::profile_refusal`, before `build_env` and any run row), D-40 header suffix ` · chưa kiểm E-09` plus `metrics_json.unverified`.
- `dsh` parses and dispatches; a run on it is refused with `Engine dsh chưa được bật trong bản agent-brain này` until plan 06-23 (`dsh::DshRun`, `dsh::plan` stub exist now).
- `docs/config.engines.example.json` rewritten to the final shape and loaded by a unit test.

## Task Commits

1. **Task 1 (tracer): engine list, `Engine::Named`, the `anthropic` arm**
   - RED `03f0fbe` (test) — five routing tests and the fake-claude record fields
   - GREEN `ad3d93c` (feat)
2. **Task 2: profile guard, unverified mark, example config, error table**
   - RED `e0e854d` (test) — `eng10_seam_backstop`, `unverified_endpoint_marked`, `engine_list_errors`, `example_engines_file_loads`
   - GREEN `381d622` (feat)

**Plan metadata:** the `docs(06-11)` commit that adds this file.

## TDD Gate Compliance

Both gate sequences are present: `test(06-11)` precedes `feat(06-11)` for each task.

- **RED, task 1:** the five new integration tests failed on the planned behaviour: `tiet_kiem_routes_lead_dev_qa` and `two_anthropic_endpoints` on `Cấu hình engines không hợp lệ (phải là JSON object)` (the array shape was refused), `legacy_object_shape_still_routes` on `Engine deepseek chưa được bật trong bản agent-brain này`, `anthropic_missing_key_refused` and `unknown_engine_refused_before_run` because the step ran on claude. Semantic assessment: each target executed and failed on the assertion for the planned behaviour, not on setup, import or fixture. Evidence is cargo's plain-text report; `gsd_run check tdd-red-evidence` supports TAP/JUnit/swift/unittest only, so the machine classifier was not applicable and the assessment is by inspection of the failure output.
- **RED, task 2:** `eng10_seam_backstop` (the Dev run succeeded instead of being refused) and `unverified_endpoint_marked` (no suffix, no metric) failed on their assertions; `example_engines_file_loads` was committed against the old example file and the example was rewritten in the GREEN commit. `engine_list_errors`, `pinned_engine_runs_on_personal_project` and `qa_on_list_id_refused` already held (they pin behaviour implemented in task 1 or the existing loader) and are not claimed as RED.
- **Not split:** the `engine::tests::profile_refusal_rules` unit test was added in the GREEN commit; a unit test of a function that does not exist cannot compile, so it has no RED run.

## Files Created/Modified

- `agent-brain/src/config.rs` - engine catalog types, array + legacy object loaders, accessor-form tests, `engine_list`, `engine_list_errors`, `example_engines_file_loads`, `kind_of_unknown_is_none`
- `agent-brain/src/engine.rs` - `Engine::Named`, `engine_order`, `plan_for(.., cwd)`, `anthropic_plan`, parameterised `agy_plan`, `profile_refusal`, catalog-driven `readiness`
- `agent-brain/src/dsh.rs` - `DshRun` and the `plan` stub (new)
- `agent-brain/src/lib.rs` - `pub mod dsh;`
- `agent-brain/src/roles/mod.rs` - seam order, `cwd` argument, `db_engine: String`, native cost fallback, profile backstop, D-40 header mark and metric
- `agent-brain/src/teams.rs` - invalid-engine text, summary sort keys for `Named`
- `docs/config.engines.example.json` - final engine-list example
- `tests/fakes/fake_claude.py` - record gains `env_names`, `anthropic_base_url`, `auth_token_matches_expected`
- `agent-brain/tests/engine_routing.rs` - nine new integration tests

## Legacy Assertions Changed

| Test | Change | Why |
|---|---|---|
| `config::tests::engines_validation` | `err("x")` expects `Cấu hình engines không hợp lệ (phải là mảng hoặc JSON object)` | the only legacy string the plan changes (top level may now be an array) |
| `config::tests::engines_defaults`, `engines_validation` | read through `cfg.engines.get("deepseek")` / `get("agy")` accessors | `EnginesCfg` is now a list; every legacy text is untouched |
| `teams::tests::entry_errors_keep_others` | invalid-engine text is now `... (dùng claude, người:<mã> hoặc id của một engine trong config.json)` and the sample input `gpt` became `GPT 4` | the text is changed by the plan; `gpt` is now a syntactically valid engine id (existence is checked at the run seam, D-43) |
| `teams::tests::builtins_only`, `file_entries_and_override` | `Engine::Deepseek` / `Engine::Agy` -> `Engine::Named("deepseek"/"agy")` | enum promoted |
| `engine::tests::plan_for_claude_keeps_argv_and_env` | dropped the "deepseek chưa được bật" loop; `plan_for` calls go through a `run_plan` helper that passes `cwd` | `deepseek` is now a routed `anthropic` entry; the dsh stub keeps the "chưa được bật" text (`unknown_id_and_dsh_stub_are_refused`) |
| `engine::tests::engine_parse` | `gpt` removed from the invalid list, `bad id` / `GPT` / 33-byte id added; `ENGINE_ORDER` -> `engine_order(&cfg)` | same reason; `ENGINE_ORDER` is replaced by `engine_order` |

## Decisions Made

- `anthropic` entry model choice: gateway = `models[role]` else `models.dev` (required by validation); native = `models[role]` else the model the role already chose.
- `readiness` keeps today's rules over the catalog. Non-deepseek `anthropic` entries get a generic description (`claude chạy trên máy với một endpoint định dạng Anthropic.`); the default `deepseek` id keeps its old description so existing Settings output is unchanged. The `chưa kiểm` readiness word and the container probe belong to plan 06-26.
- Reserved-id check (`claude`, `người`, any Unicode normalisation) runs before the name rule so the NFD spelling of `người` gets the reserved-id message instead of the generic invalid-id one.

## Deviations from Plan

None - plan executed exactly as written. (The `#[allow(clippy::too_many_arguments)]` on `plan_for` is a consequence of the plan's own eight-parameter signature.)

**Total deviations:** 0.

## Issues Encountered

- A full-suite run (`cargo test --offline --locked -- --test-threads=1`) passed on the first attempt (483 lib tests, every integration binary ok); the `contract_guard::guard_contract_cases` BrokenPipe flake did not occur.
- `cargo fmt --check` was clean for the whole crate, so the `dashboard_assets.rs` drift logged in `deferred-items.md` from 06-10 is not present in this base.
- `spikes/lib/scan-secrets.sh` prints `scan-secrets: OK`; it only scans `spikes/`. The planted values in tests look like `test-planted-gw-key-...` and no real key appears in any committed file.

## Known Stubs

| File | Line | Reason |
|---|---|---|
| `agent-brain/src/dsh.rs` | `plan` | Intentional: returns `Engine dsh chưa được bật trong bản agent-brain này` until plan 06-23 fills `dsh::plan`. The dispatch, `EnginePlan.dsh` and `DshRun` exist now so `plan_for` never changes again. |

Known gap handed to a later plan, not fixed here (outside this plan's files): `agent-brain/src/roles/dev.rs` still decides the agy-only diff checks with `r.engine == ENGINE_AGY`. An `agy` entry whose id is not `agy` would skip the `.planning/` and secret-path checks. Plan 06-25 replaces that equality with `cfg.engines.kind_of(&run.engine)`; until then keep the agy entry id `agy`.

## Threat Flags

None. Mitigations of the plan's threat register: T-06-45 (key of engine A in a child of engine B) covered by `tiet_kiem_routes_lead_dev_qa`, `two_anthropic_endpoints` and `claude_env_loses_every_key_variable`; T-06-73 (config shaping argv/env) by executable-only `command`, per-kind unknown-key refusal and the `--settings` / `--append-system-prompt` assertions; T-06-74 (SPK-10) by `profile_refusal` and `eng10_seam_backstop`; T-06-75 (id silently mapped elsewhere) by `unknown_engine_refused_before_run`.

## User Setup Required

None - no external service configuration required. To use a gateway, export its key variable in the shell that runs `agent-brain serve` or the CLI; no key is ever written to a file.

## Next Phase Readiness

- Plans 06-12 onward can read `cfg.engines.get/kind_of/all_key_envs`, `Engine::Named`, `EnginePlan.{native, unverified, dsh}` and `plan_for(.., cwd)`.
- Residual risk recorded in the plan: a profile `settings.json` `env` block that pins `ANTHROPIC_*` overrides what is injected; only the user-run E-09 of plan 06-20 can see it.
- No blockers.

## Self-Check: PASSED

- Files present: `dsh.rs`, `config.rs`, `engine.rs`, `engine_routing.rs`, `config.engines.example.json`, `fake_claude.py`, this SUMMARY.
- Commits in history: `03f0fbe`, `ad3d93c`, `e0e854d`, `381d622` (task commits), `6ff22f0` (SUMMARY).
- Verify commands re-run after the last code commit: `engine_routing` 22/22, the three named unit tests 3/3, the full suite green (every result line `ok`), `cargo fmt --check` clean, `Cargo.toml` / `Cargo.lock` identical to `574b6cd`, `scan-secrets: OK`.

---
*Phase: 06-multi-engine-media-branch*
*Completed: 2026-10-28*
