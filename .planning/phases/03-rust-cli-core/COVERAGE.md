# Phase 3 — External surface coverage (claude CLI)

Phase 3 has no network API or SDK. Its one external surface is the local `claude` CLI (2.1.30x), driven as a subprocess by `agent-brain`. The rows below list the CLI flags and stream-json events the binary uses (INTEGRATE) and the ones it deliberately leaves out (OPT-OUT, with the reason). Sources: spikes/spike-results.md S1, S2, S4, S5, S8; 03-RESEARCH.md "Claude command lines"; Phase 2 02-07-SUMMARY.md (A-01..A-11).

| capability | decision | reason |
|---|---|---|
| `-p` headless prompt (stdin = /dev/null) | INTEGRATE | plans 03-04, 03-10, 03-11 (roles), 03-08 (inspect) |
| `--output-format stream-json` + `--verbose` | INTEGRATE | real-time log for LOG-01/02 (S1: stream-json needs --verbose) |
| `--model` (opus / sonnet aliases) | INTEGRATE | per-role models from config; inspect uses inspect_model |
| `--append-system-prompt` | INTEGRATE | the three Phase 2 role prompts (CLI-03) |
| `--allowedTools` / `--disallowedTools` rules | INTEGRATE | role tool limits (ROLE-06): `Tool(./path)`, `Tool(//abs)` (02-07 live fix), `Bash(cmd *)` |
| `--settings` with a PreToolUse command hook | INTEGRATE | Dev guard, served by `agent-brain guard` (03-07, 03-11) |
| `--strict-mcp-config` | INTEGRATE | config common_args; keeps the user's MCP servers out of role runs |
| `--max-turns` | INTEGRATE | inspect only (`--max-turns 1`); acceptance on 2.1.301 checked live in 03-13 (research A1) |
| `--tools ""` | INTEGRATE | inspect: no tools, one turn (S8 variant B) |
| `--no-session-persistence` | INTEGRATE | inspect leaves no transcript (D-15, S8) |
| env `CLAUDE_CONFIG_DIR` per profile | INTEGRATE | PROF-02; work unsets it, personal sets ~/.claude-orca (S5, SPK-10) |
| env `CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR=1` | INTEGRATE | keeps `./` rules valid after `cd` (02-07 live fix) |
| stream event `system/init` (session_id, model) | INTEGRATE | runs.session_id, terminal init line |
| stream events `assistant` (text, tool_use, thinking, message.usage) | INTEGRATE | terminal lines, turn count, KPI, partial usage |
| stream event `user` (tool_result) | INTEGRATE | terminal result lines |
| stream event `result` | INTEGRATE | usage, total_cost_usd, num_turns, subtype, permission_denials, result text → runs columns and inspect check |
| stream events `rate_limit_event`, `system/*`, unknown types | INTEGRATE | hook/task subtypes included; stored raw, shown as one line, never rejected (LOG-02) |
| `--resume` / `--fork-session` | OPT-OUT | warm reuse is FAST-03 (v2) and golden-session fork is Phase 4 / SESS-01 |
| `--dangerously-skip-permissions` / `--allow-dangerously-skip-permissions` | OPT-OUT | forbidden by design; refused if present in config (ROLE-06, T-03-06) |
| `--permission-mode` (incl. bypassPermissions) | OPT-OUT | forbidden by design; tool limits come from allow/deny rules only |
| `--output-format json` | OPT-OUT | no real-time events; stream-json covers the same result line (S1) |
| `--input-format stream-json` | OPT-OUT | prompts are single -p arguments; no interactive feeding needed in Phase 3 |
| `--bare` | OPT-OUT | loses the subscription login (S1) |
| `--mcp-config` (custom MCP servers) | OPT-OUT | roles run without MCP; `--strict-mcp-config` keeps them out |
