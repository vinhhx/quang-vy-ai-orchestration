# Phase 1: Verify Assumptions - Research

**Researched:** 2026-10-14
**Domain:** Empirical verification of CLI/agent behaviour (Claude Code 2.1.299 headless mode, prompt cache TTL, Linux process-tree kill, `CLAUDE_CONFIG_DIR` profile separation, DeepSeek harness in Docker, Antigravity `agy` CLI)
**Confidence:** HIGH on what the docs and local probes say; MEDIUM on predicted spike outcomes (the spikes exist precisely to confirm them)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Cách chạy & lưu bằng chứng**
- **D-01:** Agent thực thi các lệnh spike và thu output; người dùng là người kết luận. Đây là điều chỉnh so với tài liệu gốc (§6 "tự chạy tay") — agent được chạy lệnh, nhưng không được tự chốt kết quả.
- **D-02:** Agent ghi vào `spike-results.md` cả Ngày thử, Lệnh, Quan sát (số liệu khách quan) VÀ một kết quả **đề xuất** (Đạt / Không đạt / Một phần) cùng ảnh hưởng tới thiết kế, mỗi mục đánh dấu rõ `(chờ duyệt)`. Người dùng duyệt/sửa rồi bỏ nhãn. Plan phải có checkpoint duyệt của người dùng trước khi phase được coi là xong.
- **D-03:** Output thô (ví dụ `s1.jsonl`, output `--output-format json`, log `ps`) được **commit vào repo** — Phase 3 dùng làm fixture test cho parser stream-json. Trước khi commit phải lọc dữ liệu nhạy cảm (API key, token, email; cân nhắc rút gọn đường dẫn home).
- **D-04:** Vị trí: `spikes/spike-results.md` và `spikes/s1/` … `spikes/s8/` ở **gốc repo** (không đặt trong `.planning/` để không bị archive khi dọn milestone). Mỗi thư mục spike chứa lệnh đã chạy (script `.sh` tái chạy được) và output thô.

**Tài khoản & môi trường thử**
- **D-05:** S1–S4 và S8 chạy bằng tài khoản Claude hiện tại (`~/.claude`, gói subscription).
- **D-06:** Profile cho S5: `~/.claude` = **work**; clone `~/.claude-orca` sang `~/.claude-personal` = **personal**. Thay cho `~/.claude-max` trong tài liệu gốc. Không sửa `~/.claude-orca` gốc. Nếu cần đăng nhập lại thì đó là bước tương tác của người dùng.
- **D-07:** S6 kiểm chứng DeepSeek qua **harness Docker hiện có** — container `dsh-deepseek-harness` (image `dsh-base:latest`, đang chạy) — thay vì Claude Code trên host + `ANTHROPIC_BASE_URL`. Cần trả lời: gọi headless từ host được không (vd. `docker exec`), output có cùng định dạng stream-json không, tool call sửa file trong project được mount có hoạt động không, kill từ ngoài có sạch không. Key DeepSeek nằm trong harness; không được ghi key ra file/log commit.
- **D-08:** Máy chuẩn là **máy Linux này**. `agent-brain` nhắm Linux trước (process group / SIGTERM); macOS/Windows ngoài phạm vi phase này. — **Reversibility:** costly — chuyển sang hỗ trợ Windows sau này cần cơ chế kill khác (Job Object) ở Phase 3.

**Ngưỡng Đạt cho spike mơ hồ**
- **D-09 (S3 — prompt cache):** KHÔNG dùng `cache_read` tổng làm tín hiệu — system prompt và định nghĩa tool của Claude Code giống nhau giữa các session nên luôn được cache (nhất là khi có session khác cùng tài khoản đang chạy), `cache_read` sẽ không về 0. Đo **phần riêng của dự án** (nội dung `.gsd`/ngữ cảnh dự án): cache ấm → phần này nằm trong `cache_read_input_tokens`; cache hết hạn → chuyển sang `cache_creation_input_tokens`.
- **D-10 (S3 — TTL):** KHÔNG giả định TTL 5 phút. Đo ở 3 mốc: ~1 phút, ~10 phút, ~65 phút để xác định TTL thật trên gói subscription (5 phút hay 1 giờ). Kết quả này quyết định thiết kế "golden session" ở Phase 4 (OPT/SESS). Ảnh hưởng tới quota subscription: ghi nếu quan sát được, không chặn phase.
- **D-11 (S7 — Antigravity):** Tra tài liệu chính thức; nếu máy đã cài Antigravity thì chạy thử lệnh headless/JSON. Không cài mới chỉ để làm spike. Nếu chỉ dựa trên docs, ghi rõ điều đó.
- **D-12 (S4 — kill):** Hai bước: (1) kill PID `claude` khi agent đang chạy `sleep 300` qua bash, ghi tiến trình sót; (2) chạy trong process group riêng (`setsid`) và kill cả nhóm (`kill -- -PGID`), kiểm tra sạch. Phase 3 có sẵn phương án đã kiểm chứng.
- **D-13 (Cổng đóng phase):** Phase đóng khi cả 8 spike có kết quả đã được người dùng duyệt, và mọi spike "Một phần"/"Không đạt" ghi rõ phương án thay thế + phase bị ảnh hưởng. Riêng **S1 (stream-json) và S4 (kill)** — nền của Phase 3 — bắt buộc phải có cách làm rõ ràng, đã thử.

### Claude's Discretion
- Cấu trúc chi tiết bên trong `spikes/sN/` (tên file, script) và cách lọc dữ liệu nhạy cảm.
- Prompt/task cụ thể dùng cho S3 để có phần "riêng dự án" đủ lớn, đo được.
- Cách gọi harness Docker cho S6 (sau khi tìm hiểu cấu hình container).

### Deferred Ideas (OUT OF SCOPE)
- Chính sách công ty cho tài khoản Work/Personal (SPK-10) — không thảo luận trong buổi này; vẫn là blocker mở trong STATE.md, cần xác nhận trước Phase 3.
- Kịch bản kill có MCP server con và kill harness Docker từ ngoài ở mức đầy đủ — S4 chỉ làm process group cơ bản; khía cạnh Docker được trả lời một phần trong S6.
- Hỗ trợ macOS/Windows cho `agent-brain` — ngoài phạm vi; Linux trước.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SPK-01 | S1 stream-json: `session_id` line, final `result` usage, one line per tool call | Message schema (SDKSystemMessage / SDKAssistantMessage / SDKResultMessage) documented; per-spike procedure S1; dedup-by-`message.id` pitfall |
| SPK-02 | S2 resume/fork | `--resume` / `--fork-session` semantics; transcript path; hash-before/after procedure S2 |
| SPK-03 | S3 cache hot/cold | Official TTL rules (subscription = 1h for main conversation); `usage.cache_creation.ephemeral_*` fields; independent-prefix measurement design at ~1/~10/~65 min |
| SPK-04 | S4 kill | Bash tool children are in their own session/pgid (observed); SIGTERM semantics (exit 143, tree-kill of running Bash); two-step procedure + leftover verification |
| SPK-05 | S5 profiles | `CLAUDE_CONFIG_DIR` semantics; `.claude.json` lives inside non-default dirs; safe clone procedure; account facts of the two profiles |
| SPK-06 | S6 DeepSeek | Harness is `dsh` (not Claude Code); headless profile prints only the final message; invocation, mounts, kill options |
| SPK-07 | S7 Antigravity | `agy` 1.3.5 installed and has `-p/--output-format json|stream-json`; no max-turns flag documented; `antigravity` binary is a GUI launcher |
| SPK-08 | S8 max-turns | `--max-turns` documented but absent from `--help`; `error_max_turns` result carries no `result` text |
| SPK-09 | All 8 spikes with Đạt/Không đạt/Một phần + alternatives | Results template, `(chờ duyệt)` workflow, `check-results.sh` validator |
| SPK-10 | Work/Personal account decision | Observed account types of both profiles (company Team seat vs personal Pro); policy confirmation is a human checkpoint |
</phase_requirements>

## Summary

Phase 1 is not software construction; it is a set of eight scripted experiments plus an evidence package. Everything needed is already on this machine (`claude` 2.1.299, `agy` 1.3.5, Docker 29.8.1, `jq`, `python3`, `setsid`, `pstree`, cgroup v2). No package has to be installed. The planner's job is to sequence the spikes (S3 takes 65+ minutes of wall clock and must start first and run detached), to make every spike a re-runnable script writing raw output to `spikes/sN/`, to put a scrub-then-scan gate in front of every commit, and to include the human checkpoints (approve results, decide SPK-10, allow kill commands).

Research found several facts that change how the spikes must be designed, and two that contradict assumptions in CONTEXT/REQUIREMENTS: (1) On a Claude subscription within plan usage, the main conversation (`-p` included) is requested with a **1-hour** cache TTL, but the cache is **scoped to machine + directory (+ git-status snapshot)**, so the "shared system prompt is always warm" premise of D-09 holds only within the same directory - S3 must therefore use an isolated directory with a unique nonce per probe, and must not use the repo as workdir. (2) Claude Code starts every Bash-tool command in its **own session and process group**, so `kill -- -PGID` of the `claude` group (D-12 step 2) is expected to leave the `sleep` alive; what does work per the docs is **SIGTERM to the `claude` PID** (exit 143, Claude Code tree-kills its running Bash command itself), which does not work after SIGKILL. (3) The harness for S6 is `dsh` (DeepSeek Harness), a different agent from Claude Code; its headless profile prints only the final assistant message, so "same stream-json format" will almost certainly be Không đạt and the planner must pre-wire an alternative. (4) Profile facts: `~/.claude` is a **company Team seat (VCCorp, `claude_team`)** and `~/.claude-orca` is a **personal Pro account (gmail)**, which contradicts REQUIREMENTS SPK-10's premise that both accounts are company-issued - the policy question narrows to "may the company seat be used for work only, and is the personal account free for personal/video work".

Two hazards for the executing agent: the Claude Code auto-mode permission classifier **denied** a `kill -TERM -- -<pgid>` this session ("Interfere With Workloads"), so S4/S6 kill scripts need explicit user pre-authorisation or must be run by the user; and running the bare `antigravity` binary (even `--version`) **launches the Antigravity IDE GUI** - only `agy` is the CLI.

**Primary recommendation:** Plan as: Wave 0 scaffolding (clean-env wrapper, scrub + secret-scan scripts, results template, validator) -> start S3 detached immediately -> run S7, S1, S2, S8 while S3 waits -> S4/S6 behind an explicit user-permission checkpoint -> S5 + SPK-10 as human-interactive steps -> scrub gate -> assemble `spike-results.md` with `(chờ duyệt)` labels -> user approval checkpoint.

## Architectural Responsibility Map

This phase has no application tiers; the map assigns each verification capability to the runtime that actually owns the behaviour being tested.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Spike orchestration scripts (`spikes/sN/run.sh`) | Host shell (bash + jq + python3) | — | Must be re-runnable by user and by Phase 3 as fixture generators |
| Claude Code headless run (S1,S2,S3,S8) | Host process `claude -p` | Anthropic API (cache is server-side) | Cache TTL/ scope is decided server-side but reported in the `result` line |
| Process-tree kill (S4) | Host OS (Linux kernel: sessions, pgids, signals) | Claude Code's own SIGTERM handler | Whether leftovers exist depends on kernel session/pgid layout plus Claude Code's handler |
| Profile separation (S5) | Host filesystem (`CLAUDE_CONFIG_DIR` dirs) | Anthropic OAuth account | State lives in files; account identity lives in `.credentials.json` / `.claude.json` |
| DeepSeek run (S6) | Docker container `dsh-deepseek-harness` (runs `dsh`) | Corporate LLM gateway (`DEEPSEEK_BASE_URL`) | Host only drives it via `docker exec`; files shared by bind mount |
| Antigravity check (S7) | Docs + host `agy` CLI | — | Docs are authoritative; a tiny local run confirms it |
| Secret scrubbing | Host script (python3 stdlib) run before `git add` | Pre-commit-style scan script | Must be mechanical, not "remember to" |
| Approval workflow | Human (user) | `check-results.sh` validator | D-01/D-02: agent proposes, user decides |

## Standard Stack

No application dependencies. The "stack" is tooling already present.

### Core
| Tool | Version (this machine) | Purpose | Why standard |
|------|------------------------|---------|--------------|
| `claude` CLI | 2.1.299 [VERIFIED: `claude --version` -> `2.1.299 (Claude Code)`] | Subject of S1-S5, S8 | The thing being verified |
| `agy` (Antigravity CLI) | 1.3.5 [VERIFIED: `agy --version` -> `1.3.5`] | Subject of S7 | Only CLI entry point; `antigravity` is a GUI launcher (see Pitfall 9) |
| Docker | 29.8.1 [VERIFIED: `docker --version` -> `Docker version 29.8.1, build 4a63305`] | S6 harness access | Container already running |
| `dsh` (DeepSeek Harness) | 0.1.1-rc.2 inside container [VERIFIED: `docker exec dsh-deepseek-harness dsh --version` -> `0.1.1-rc.2`] | Subject of S6 | The harness D-07 mandates |
| bash, `setsid`, `pgrep`, `ps`, `pstree` | system | S4 process inspection | Standard Linux tooling |
| `jq` | system (`/usr/bin/jq`) | Extract `session_id`, usage, subtype from JSON/JSONL | Docs themselves use jq for these fields |
| `python3` (3.12.3, stdlib only, run with `-I`) | system | Scrub script, JSON-aware redaction, secret scan | No pip installs; `-I` isolates from cwd |

### Supporting
| Tool | Purpose | When to Use |
|------|---------|-------------|
| `timeout` | Bound every spike command | Always (`timeout 600 claude -p ...`) |
| `systemd-run --user --scope` / cgroup v2 `cgroup.kill` | Candidate leftover-proof kill (S4 optional step 3) | Only as an *additional* observation if step 2 leaves orphans; user manager reports `degraded` so test before recommending |
| `sha256sum`, `wc -l` | Prove "original session unchanged" (S2) | Before/after the fork |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Own scrub script | `gitleaks` / `trufflehog` | Neither is installed (`command -v` printed nothing); D-11-style rule "do not install" applies by analogy - use the python scrub + value-denylist scan |
| Claude on host for S6 | `ANTHROPIC_BASE_URL` to DeepSeek's Anthropic-format endpoint | DeepSeek documents `https://api.deepseek.com/anthropic` [CITED: api-docs.deepseek.com/guides/anthropic_api]; but the key on this machine targets a corporate gateway whose format is unverified - keep only as the documented *alternative* if S6 is Không đạt |

**Installation:** none. `# nothing to install`

**Version verification:** versions above were read from the binaries this session. No registry packages are involved.

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| — | — | — | — | — | — | No external packages are installed by this phase |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none
*`@deepseek-ai/dsh` is already baked into the existing image `dsh-base:latest` (README: `npm install -g @deepseek-ai/dsh`); this phase does not install or update it (`restart.sh` would rebuild and pull the latest from npm - do not run it).*

## Architecture Patterns

### System Architecture Diagram

```
                       ┌──────────────────────── user (approves, decides SPK-10, grants kill permission)
                       │
 spikes/lib/env.sh ──► clean-env wrapper ──┐
 (unset CLAUDE_CODE_*,                     │
  CLAUDECODE, CLAUDE_PID …)                ▼
                          ┌──────────────────────────────────────────────┐
  S3 (detached, 65+ min) ─┤  spikes/sN/run.sh  (timeout, </dev/null)     │
  S1 S2 S8 S7             │    │ claude -p … | agy -p … | docker exec …  │
  S4 S6 (need permission) │    ▼                                         │
  S5 (interactive login)  │  raw/  (stdout JSON/JSONL, ps snapshots)     │
                          └───────────────┬──────────────────────────────┘
                                          ▼
                        spikes/lib/scrub.py  (home path, emails, tokens,
                                              env-value denylist; JSON-valid)
                                          ▼
                        spikes/lib/scan-secrets.sh  ── FAIL ► block commit
                                          ▼ PASS
              spikes/sN/{run.sh, raw/*.jsonl|json|txt, NOTES}  ──► git add
                                          ▼
              spikes/spike-results.md  (8 sections, each "(chờ duyệt)")
                                          ▼
              spikes/lib/check-results.sh ─► user review ─► remove labels ─► phase closes
                                          ▼
              Consumers: Phase 3 parser fixtures (S1), kill strategy (S4), --max-turns fast-path (S8);
                         Phase 4 cache/golden session (S2,S3); Phase 6 DeepSeek/Antigravity (S6,S7)
```

### Recommended Project Structure
```
spikes/
├── spike-results.md        # the deliverable (template from 01-phase0 §5, plus "(chờ duyệt)" labels)
├── lib/
│   ├── env.sh              # source me: scrubs nested-session env, sets TZ/LANG, defines run_claude()
│   ├── scrub.py            # JSON-aware redaction (stdlib, run: python3 -I scrub.py in out)
│   ├── scan-secrets.sh     # fails if patterns or denylisted env VALUES remain in spikes/
│   └── check-results.sh    # asserts 8 sections, required fields, no "(chờ duyệt)" at close
├── s1/ run.sh raw/s1.jsonl raw/s1-no-verbose.txt
├── s2/ run.sh raw/…        # session ids, transcript hashes
├── s3/ run.sh schedule.sh raw/…  # workdirs live OUTSIDE the repo (see Pitfall 2)
├── s4/ run.sh raw/ps-*.txt
├── s5/ run.sh raw/…        # account/status outputs, redacted
├── s6/ run.sh raw/…        # scratch dir is git-ignored and removed via docker exec
├── s7/ NOTES.md raw/agy-help.txt raw/agy-run.json
└── s8/ run.sh raw/…
```
Add to `.gitignore`: `spikes/**/work/` and `spikes/**/scratch/` (scratch dirs, S6 root-owned files).

### Pattern 1: Every spike is `run.sh` + `raw/` + observation lines
**What:** Each `spikes/sN/run.sh` is idempotent, bounded by `timeout`, uses `</dev/null`, writes only into `spikes/sN/raw/`, prints a short "OBSERVATIONS" block (numbers only) and never states Đạt/Không đạt. The agent then copies observations into `spike-results.md` as a *proposed* result marked `(chờ duyệt)` (D-02).
**When to use:** All spikes.
**Example:**
```bash
# spikes/lib/env.sh  (source from every run.sh)
# Nested-session hygiene: the executing agent is itself a Claude Code session, so child `claude`
# inherits CLAUDECODE=1 etc. Phase 3 will spawn claude from a normal terminal, so mimic that.
# Variable names observed in this session's environment (names only):
#   CLAUDE_CODE_CHILD_SESSION CLAUDE_CODE_SESSION_ID CLAUDE_PID CLAUDE_EFFORT CLAUDE_VC_ENV
#   CLAUDE_CODE_MESSAGING_SOCKET CLAUDECODE CLAUDE_CODE_SESSION_ATTENDED CLAUDE_CODE_ENTRYPOINT
#   CLAUDE_CODE_EXECPATH CLAUDE_CODE_MESSAGING_TOKEN
run_claude() {
  env -u CLAUDECODE -u CLAUDE_CODE_CHILD_SESSION -u CLAUDE_CODE_SESSION_ID -u CLAUDE_PID \
      -u CLAUDE_EFFORT -u CLAUDE_VC_ENV -u CLAUDE_CODE_MESSAGING_SOCKET -u CLAUDE_CODE_MESSAGING_TOKEN \
      -u CLAUDE_CODE_SESSION_ATTENDED -u CLAUDE_CODE_ENTRYPOINT -u CLAUDE_CODE_EXECPATH \
      timeout "${SPIKE_TIMEOUT:-300}" claude "$@" </dev/null
}
```
Source for the variable list: `env | grep -E '^(CLAUDE|ANTHROPIC)' | sed 's/=.*//'` run this session [VERIFIED: local probe]. Whether leaving them set would change behaviour is [ASSUMED]; scrubbing them is the conservative choice and should be recorded in each spike's NOTES.

### Pattern 2: Scrub is JSON-aware, then scan is a hard gate
**What:** `scrub.py` parses each JSONL line, walks string values, applies replacements, re-serialises (so Phase 3 fixtures stay valid JSON). `scan-secrets.sh` greps the result for patterns and for the literal *values* of sensitive env vars (read from the environment at scan time, never printed) and exits non-zero on any hit.
**Replacements (suggested set):** `$HOME` -> `~` (and `/home/<user>` -> `/home/USER`); emails `[\w.+-]+@[\w-]+\.[\w.]+` -> `<email>`; `sk-[A-Za-z0-9_-]{16,}` and `sk-ant-[A-Za-z0-9_-]+` -> `<redacted-key>`; `Bearer\s+\S+` -> `Bearer <redacted>`; JWT `eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+`; `ghp_[A-Za-z0-9]{20,}`; long hex/base64 blobs (>=32) only inside keys named like `token|key|secret|password`.

### Pattern 3: Human checkpoints are first-class plan tasks
Three required: (a) user pre-authorises or personally runs kill scripts (S4/S6); (b) interactive login / company-policy confirmation (S5, SPK-10); (c) final approval of every `(chờ duyệt)` proposal (D-02, D-13).

### Anti-Patterns to Avoid
- **Using the repo directory as the S3 workdir:** outputs written inside it change `git status`, which is part of the prefix -> artificial cache misses (Pitfall 2).
- **`pkill claude` / `pkill -f claude` / `kill $CLAUDE_PID`:** would kill the very Claude Code session running the plan (this session's `claude` is `282882` and a VS Code extension `claude` also runs). Kill only PIDs the script itself started and recorded.
- **Letting the agent write "Đạt"** into `spike-results.md` without `(chờ duyệt)` (violates D-01/D-02).
- **Committing raw output before the scan gate passes.**

## Per-Spike Procedures (facts the plan needs)

### S1 - stream-json (SPK-01)
Command shape (from source spec, plus flags needed for non-interactive tool use): `claude -p "liệt kê file trong thư mục này" --output-format stream-json --verbose > raw/s1.jsonl` run inside a small scratch dir containing a handful of files. `ls` is in Claude Code's read-only command set so it needs no approval; to be explicit add `--allowedTools "Bash(ls *)"` [CITED: code.claude.com/docs/en/headless "Auto-approve tools" and "read-only command set"].
- **Documented message types:** `system` with `subtype: "init"` (first event unless startup/hook events precede it; carries `session_id`, `model`, `tools`, `cwd`, `claude_code_version`), `assistant` (carries `session_id`, nested `message` with `usage`), `user` (tool results), and a final `result`. [CITED: code.claude.com/docs/en/agent-sdk/typescript SDKSystemMessage/SDKAssistantMessage/SDKResultMessage] Quoted from the docs: "The last line of the stream is a `result` message with the final response text, cost, and session metadata." [CITED: code.claude.com/docs/en/headless]
- **Per-tool-call lines is a nuance to record, not assume:** a `tool_use` is a content block inside an `assistant` line and its output is a separate `user` line; parallel tool calls share one `message.id` with identical usage ("Parallel tool calls share the same ID, only count once") and "Per-step `output_tokens` is a placeholder, so read output tokens from the result message" [CITED: code.claude.com/docs/en/agent-sdk/cost-tracking]. This is the single most important fact for the Phase 3 parser (LOG-01). [ASSUMED] that the exact line-per-block layout matches; S1 observes it.
- **Result fields to record:** `session_id`, `num_turns`, `total_cost_usd`, `usage.input_tokens`, `usage.output_tokens`, `usage.cache_read_input_tokens`, `usage.cache_creation_input_tokens`, `usage.cache_creation.ephemeral_5m_input_tokens`, `usage.cache_creation.ephemeral_1h_input_tokens`, `modelUsage`, `permission_denials`, `terminal_reason`, `subtype` [CITED: agent-sdk/typescript SDKResultMessage + Usage]. `usage` is "main agent loop only"; prefer `modelUsage` for accounting [CITED same].
- **`--verbose` requirement (roadmap success criterion 3):** every documented stream-json example includes `--verbose`, but no page was found stating it is *mandatory* [ASSUMED]. Run the negative case (`... --output-format stream-json` without `--verbose`, expect an error before any API call) and paste the exact stderr + exit code. This costs no tokens if it errors out.
- **Extra line types to expect and keep in fixtures:** `system` hook events for `SessionStart`/`Setup` hooks "are always included" [CITED: cli-reference `--include-hook-events`]; `~/.claude/settings.json` defines 13 hook events (SessionStart ... SessionEnd; Orca agent hooks gated on `ORCA_AGENT_HOOK*` env, probe-visible) [VERIFIED: local probe, hook event names listed]. Keep these lines in the fixture - Phase 3's parser must tolerate unknown/extra `type`s (LOG-02).

### S2 - resume and fork (SPK-02)
1. `claude -p "nhớ số 42" --output-format json` in scratch dir D; capture `session_id` (`jq -r .session_id`).
2. `claude -p "số tôi bảo nhớ là gì?" --resume <id> --output-format json` - from D. Record whether the returned `session_id` equals `<id>` (docs imply reuse: "--fork-session: When resuming, create a new session ID instead of reusing the original" [CITED: cli-reference]).
3. Before step 4 record `sha256sum` and `wc -l` of the original transcript `~/.claude/projects/<cwd with non-alphanumerics -> '-'>/<id>.jsonl` [CITED: code.claude.com/docs/en/sessions "Where transcripts are stored"; path pattern also observed locally: `~/.claude/projects/-mnt-data-IDE-QuangVy-Orchestration/<uuid>.jsonl` (VERIFIED: ls output)].
4. `claude -p "số đó cộng 1?" --resume <id> --fork-session --output-format json`; assert new `session_id` != `<id>`, original transcript hash/line count unchanged, answer = 43.
5. Bonus observation for Phase 4 (OPT-07): record `cache_read_input_tokens` of the fork vs the resume - docs say a *fork* "inherits the parent's system prompt, tools, and conversation history exactly, so its first request reads the parent's cache" [CITED: prompt-caching "Subagents and the cache" - this sentence is about in-session forks; whether CLI `--fork-session` behaves the same is [ASSUMED] until S2 shows it].
- `-p` sessions are hidden from `--continue` and the picker but resumable by ID [CITED: sessions "/loop, -p, Agent SDK"]. "`claude -p --continue` includes -p ... sessions" - use explicit IDs anyway.
- Running `-p` from inside another Claude Code session: `CLAUDE_CODE_CHILD_SESSION` docs say "Non-interactive `claude -p` sessions still persist" [CITED: env-vars]. Good.

### S3 - prompt cache hot/cold and TTL (SPK-03, D-09, D-10)
**What the docs say (official):** [CITED: code.claude.com/docs/en/prompt-caching "Which TTL each request gets"]
> "Unless you choose a TTL yourself, Claude Code requests the one-hour TTL only on a Claude subscription within your plan's included usage."
- Main conversation (interactive, `-p`, SDK) = one hour on subscription; "Everything else" (subagents, workflows, forks, compaction, session titles) = five minutes; API key / usage credits / cloud provider = five minutes. Over the plan's usage limit the main conversation drops to five minutes.
- "Each request that hits the cache resets the timer."
- **Direct TTL readout:** "run `claude -p "hello" --output-format json` and read `usage.cache_creation` ... one-hour cache writes under `ephemeral_1h_input_tokens` and five-minute cache writes under `ephemeral_5m_input_tokens`." So the *first* (creating) run already tells which TTL was requested; the timed probes tell whether the server honoured it.
- **Overrides to confirm are unset before measuring:** `FORCE_PROMPT_CACHING_5M`, `CLAUDE_CODE_PROMPT_CACHE_TTL`, `ENABLE_PROMPT_CACHING_1H`, `promptCacheTtl` setting, `DISABLE_PROMPT_CACHING`. Probe result: none appear in the environment, in `~/.claude/settings.json`, `~/.claude-orca/settings.json` or `.claude/settings.local.json` (grep count 0 each), and `/etc/claude-code` does not exist [VERIFIED: local probe].
- **Cache scope (changes D-09's premise):** "In Claude Code, the cache is effectively scoped to one machine and directory. The system prompt embeds your auto memory paths ... Two sessions in different directories therefore build different prefixes and miss each other's cache. Sessions you run in parallel in the same directory build matching prefixes and read each other's cache. Sequential sessions share the prefix only when the git status snapshot taken at startup matches." [CITED: prompt-caching "Cache scope"]. Consequences: (a) D-09's "shared system prompt always warm" is true only inside one directory, so a *fresh unique directory* may show `cache_read ~ 0` on its first run - measure it, don't assume; (b) the S3 workdir must be dedicated; (c) git status must be identical between runs.
- **Account facts relevant here:** `~/.claude` credentials show `subscriptionType: team` [VERIFIED: local probe of `.credentials.json` key `subscriptionType` -> `team`]. Weekly utilisation cached in `~/.claude.json` was 82% (`"seven_day": {"utilization": 82 ...`) when probed [VERIFIED: local probe] - keep spike token volume small; do not add S3 repeats lightly.

**Recommended measurement design (independent prefixes, because each hit resets the timer):**
- Create three workdirs `w1`, `w10`, `w65` *outside the repo* (e.g. under the session scratch/`/tmp`-class dir), each containing a `CLAUDE.md` of roughly 10-15k tokens (~40-60 KB of non-repeating text) that begins with a unique nonce line (`uuidgen`), plus a trivial, identical prompt such as "Reply with the single word OK" (no tool use, `num_turns` must be 1). Non-git directory (or a committed-clean git repo with all outputs written elsewhere).
- t0: run each once (cold). Record `cache_read_input_tokens`, `cache_creation_input_tokens`, `ephemeral_5m/1h`. Project-specific size P ~ the creation tokens (a unique nonce guarantees nobody else warmed it).
- Probe `w1` at ~1 min, `w10` at ~10 min, `w65` at ~65 min after its own t0 (probes are single and independent, so no probe refreshes another prefix). Optionally a 4th prefix probed at ~6 min separates a 5-min TTL from anything longer.
- Pass thresholds (propose in the plan; user approves): *warm* if `cache_read(probe) >= cache_read(t0) + 0.9*cache_creation(t0)`; *cold* if `cache_creation(probe) >= 0.9*cache_creation(t0)`. Expected under the docs: 1 and 10 min warm, 65 min cold (if the 1h TTL is honoured). If 10 min is cold, TTL is 5 min -> Phase 4 golden-session design differs (this is the user's stated concern).
- Run the schedule as a **detached** script (`setsid nohup ... &` writing a log) because the Bash tool cannot block 65+ minutes; confirm completion by polling the log. Start it first.
- Same flags, same model, same `--effort` on every run; do not start any other `claude` in those dirs.
- **Quota effect (D-10):** the only local observable is `cachedUsageUtilization` in `~/.claude.json` (integers: `five_hour.utilization`, `seven_day.utilization`), 1% granularity - almost certainly too coarse to see six small runs. Record "không quan sát được" rather than inventing a number. `--max-budget-usd` is available but is a client estimate [CITED: cli-reference].

### S4 - kill the process tree (SPK-04, D-12)
**Observed process layout (this session, read-only probe):** [VERIFIED: local `ps -eo pid,ppid,pgid,sid,cmd --forest`]
```
 282882   12405  282882   12405 claude                                   <- PID PPID PGID SID
 333473  282882  333473  333473  ...\_ /bin/bash -c source .../shell-snapshots/snapshot-bash-….sh ...
```
Every Bash-tool command is a child of `claude` running as `bash -c "source <snapshot>... && eval '<cmd>' < /dev/null && pwd -P >| /tmp/claude-XXXX-cwd"` with **its own PGID and SID equal to its PID**. Therefore:
- Step 2 of D-12 (`setsid claude -p ...` then `kill -- -PGID`) will signal only the `claude` process group; the Bash-tool wrapper and its `sleep` are in different sessions/groups. **Predicted: leftovers remain** [prediction derived from the verified layout; the spike confirms].
- **Documented good path:** "If you stop a `claude -p` run with SIGTERM ... Claude Code exits with code 143 ... On SIGTERM, Claude Code terminates the process tree of any Bash command that is still running ... then runs SessionEnd hooks and exits." SIGINT ends the turn gracefully first. [CITED: code.claude.com/docs/en/headless "Stop a run with SIGTERM"]. So test **both signals**: `kill -TERM <claude-pid>` (expected clean) and `kill -KILL <claude-pid>` (expected orphans: the wrapper `bash` and `sleep` reparent to init/subreaper) - D-12 says only "kill PID claude"; the plan must specify the signal.
- **Candidate Phase 3 strategy to try as an optional step 3 (record result, do not assume):** (i) record the descendant set via `/proc` PPID walk before signalling; (ii) SIGTERM `claude`, wait N s; (iii) SIGKILL every surviving recorded descendant and its pgid; (iv) post-check by unique marker. Stronger alternatives to evaluate for Phase 3: cgroup v2 (`stat -fc %T /sys/fs/cgroup` -> `cgroup2fs` [VERIFIED]) via `systemd-run --user --scope` (user manager state printed `degraded`, so confirm it works), or Rust `prctl(PR_SET_CHILD_SUBREAPER)`.
- **How to run it so it is measurable and safe:**
  - Use a *unique* sleep duration as marker (e.g. `sleep 317`), because other `sleep` processes already exist (8 lines matched `sleep` in `ps` at probe time) [VERIFIED: local probe]; verify with `pgrep -af '^sleep 317$'`, not `ps -ef | grep sleep`.
  - Tell the agent the exact command: `claude -p "Run this exact bash command and wait for it: sleep 317" --allowedTools "Bash(sleep *)" --output-format stream-json --verbose` (without an allow rule `-p` denies Bash and nothing runs).
  - Wait until `pgrep` shows the sleep *and* its parent chain leads to your recorded `claude` PID, then signal. Snapshot `ps -eo pid,ppid,pgid,sid,cmd --forest` before and 5 s after (PROC-01 uses a 5 s window).
  - Record the `claude` PID from `$!` (or `setsid`'s child) - never derive it by name.
  - `-p` runs wait for background work for up to 10 minutes by default; irrelevant for a 317 s foreground sleep but note `CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS` [CITED: headless].
- **Permission hazard (observed):** the auto-mode classifier denied `kill -TERM -- -333492` with reason `[Interfere With Workloads]` [VERIFIED: tool denial this session]. Plan a `checkpoint:human-action`: user adds a narrow permission rule (e.g. for `bash spikes/s4/run.sh`) or runs the script personally; the script itself must only ever signal PIDs it started.

### S5 - profile separation (SPK-05, SPK-10, D-06)
**Facts (all local probes, no secrets printed):**
- `~/.claude/` (default profile) uses `~/.claude.json` **outside** the dir; `~/.claude-orca/` contains its **own** `.claude.json` **inside** the dir plus `.credentials.json` [VERIFIED: `ls -A ~/.claude-orca` listed `.claude.json` and `.credentials.json`; `ls -la ~` showed `.claude.json`]. Docs say all of `~/.claude` "lives under that directory instead" when `CLAUDE_CONFIG_DIR` is set and that credentials move too, with Keychain entries keyed per directory on macOS [CITED: claude-directory, authentication "Credential management"]; the docs do not explicitly state where `.claude.json` goes - the on-disk layout above is the evidence.
- Identity of the two profiles (fields from `oauthAccount`): default -> `organizationName: "VCCorp"`, `organizationType: claude_team`, `seatTier: team_standard`, `organizationRole: user`; orca -> `organizationType: claude_pro`, `organizationRole: admin`, email domain `gmail.com` [VERIFIED: local probe; `accountUuid` differs between them, equal between default and `~/.claude.json.company`]. **This means REQUIREMENTS SPK-10's premise ("hai tài khoản đều do công ty cấp") does not match the machine**: work = company Team seat, personal = personal Pro. SPK-10 is still a human policy confirmation.
- `~/.claude.json.company` and `~/.claude/.credentials.json.company` exist (previous manual swap backups). Leave them untouched.
- `~/.claude-orca/.credentials.json` access token expired ~442 min before probing (`expiresAt` in the past) while `refreshTokenExpiresAt` is ~19.6 days ahead [VERIFIED: local probe]. Any Claude process started against `~/.claude-orca` may refresh and **rewrite that file** -> violates "do not modify `~/.claude-orca`". Never point `claude` at `~/.claude-orca`; only at the clone.
- **Whether copying a refresh token into a second directory invalidates the original is unknown** [ASSUMED: OAuth refresh tokens may rotate on use]. Safer clone: copy config/skills/settings but **exclude credentials**, then let the user run `CLAUDE_CONFIG_DIR=~/.claude-personal claude` / `claude auth login` interactively (D-06 explicitly allows interactive re-login). If the user prefers a pure clone, take a `cp -a` backup of `~/.claude-orca` first and record the result.
- **Safe clone procedure (suggested):** `rsync -a --exclude 'projects/' --exclude 'sessions/' --exclude 'history.jsonl' --exclude 'file-history/' --exclude 'session-env/' --exclude 'shell-snapshots/' --exclude 'telemetry/' --exclude 'backups/' --exclude '.credentials.json' ~/.claude-orca/ ~/.claude-personal/` (read-only on source; excluding `projects/`/`history` makes "separate session history" testable). Do not clone `~/.claude`'s `*.company` files. `chmod 700 ~/.claude-personal`.
- **Tests:** (1) `CLAUDE_CONFIG_DIR=~/.claude-personal claude auth status` and unset-variable `claude auth status` -> compare `authMethod`, `configDirectory` (the JSON "includes a `configDirectory` field" on v2.1.268+ [CITED: cli-reference `claude auth status`]) and org/seat type; redact email before saving. (2) `claude -p "hi" --output-format json` under each profile; confirm each writes its transcript only under its own `$CLAUDE_CONFIG_DIR/projects/…`. (3) `--resume <id-from-A>` under profile B must report "No conversation found with session ID" [CITED: sessions "Resume by session ID"]. (4) **Do not set `CLAUDE_CONFIG_DIR=~/.claude` explicitly** on the live work dir: with the variable set Claude Code looks for `.claude.json` inside the dir, which does not exist for the default profile, so it may behave as un-onboarded or create a new file in the work profile [prediction from the verified layout; ASSUMED]. If the plan wants to test explicit-set-to-default, do it on a throwaway copy of config. Recommended Phase 2/3 rule: for profile *work* (default), **remove** `CLAUDE_CONFIG_DIR` from the child environment; set it only for non-default profiles.
- `ANTHROPIC_API_KEY` / `ANTHROPIC_*` variables are not set in this environment (names list above showed only `CLAUDE*`), so subscription OAuth is what is used [VERIFIED: local probe]. `--bare` is **not usable** for subscription spikes: "OAuth and keychain are never read" [CITED: cli-reference `--bare`, headless].

### S6 - DeepSeek via the Docker harness (SPK-06, D-07)
**What the container is [VERIFIED: `docker inspect` / `docker exec` this session]:**
- Name `dsh-deepseek-harness`, image `dsh-base:latest`, `Entrypoint=["docker-entrypoint.sh"] Cmd=["/usr/local/bin/entrypoint.sh"] WorkingDir=/mnt/data User= Tty=false OpenStdin=false`; runs as `uid=0(root)`.
- Mounts: `bind /mnt/data -> /mnt/data rw=true`, `volume /var/lib/docker/volumes/docker_dsh_data/_data -> /root/.dsh rw=true`, `bind /var/run/docker.sock -> /var/run/docker.sock rw=true`. Because the bind uses the **same path on both sides**, a project under `/mnt/data` (including this repo) has identical paths on host and in the container - no path translation needed.
- Container PID 1 tree: `bash /usr/local/bin/entrypoint.sh` -> `node /usr/local/bin/dsh web --host 127.0.0.1 --port 3081 ...` + `socat TCP-LISTEN:3080 ...` (a **web UI server**, all in process group 3129) [VERIFIED: `docker top`]. Headless runs are *separate* processes started by `docker exec`.
- Env var **names** only (values deliberately not printed): `CORPORATE_HTTP_PROXY DEEPSEEK_MODEL DEEPSEEK_BASE_URL DEEPSEEK_API_KEY HTTPS_PROXY DSH_ROUTER_HOST NO_PROXY PROJECT_NAME HTTP_PROXY http_proxy no_proxy CORPORATE_HTTPS_PROXY CORPORATE_NO_PROXY https_proxy PATH NODE_VERSION YARN_VERSION`. The README documents the model default `deepseek/deepseek-v4-flash` and the gateway `DEEPSEEK_BASE_URL` pointing at a corporate AI gateway [CITED: /mnt/data/IDE/Deepseek-harness/README.md]; do not `cat` `.env.docker`.
- **The harness is `dsh`, not Claude Code.** `dsh --help` (verbatim): "`dsh --profile headless "run the tests"     answer one task, print the result, and exit`". `dsh --profile headless --help` (verbatim): "Answer one task, print the final assistant message, and exit." with only `-h, --help` as options [VERIFIED: command output]. So: **no `--output-format`, no stream-json, no `--max-turns`, no `--resume` in the headless profile**. Predicted S6 sub-results: headless from host = yes (`docker exec`); stream-json parity = **no**; file-editing tool = to be observed.
- Session logs exist inside the container: `/root/.dsh/sessions/<cwd-encoded>/session-<uuid>/session.jsonl.zstd` (zstd-compressed JSONL, 29 project dirs, 127 MB) [VERIFIED: `docker exec ls`]. A possible structured-log source for Phase 6, but unexamined and private (do not decompress/print into committed files).
- The web profile has a UI; the container also has ports 3080 behind Traefik. Driving `dsh web`'s HTTP API is an unexplored alternative for streaming - record as Open Question, do not explore in this phase.

**Invocation to test (run from host):**
```bash
mkdir -p "$REPO/spikes/s6/scratch" && printf 'hello\n' > "$REPO/spikes/s6/scratch/a.txt"
timeout 300 docker exec -w "$REPO/spikes/s6/scratch" dsh-deepseek-harness \
  dsh --profile headless "Append the line 'edited-by-dsh' to a.txt, then reply DONE" </dev/null \
  > "$REPO/spikes/s6/raw/s6-stdout.txt" 2> "$REPO/spikes/s6/raw/s6-stderr.txt"
```
- Use `-w` (workdir) and no `-t`/`-i`. Always `</dev/null`.
- **Ownership pitfall:** the container runs as root, so created/edited files are root-owned on the host; `git` may complain and ordinary `rm` fails. Keep the scratch dir git-ignored and clean it with `docker exec dsh-deepseek-harness rm -rf <scratch>`. Running as another uid (`docker exec -u`) would not be able to read `/root/.dsh` (mode `drwx------`) [VERIFIED: `ls -la` output showed `drwx------ ... root root`].
- **Kill from outside - test, do not assume:** killing the `docker exec` *client* may not stop the in-container process; behaviour depends on the Docker version (a moby change titled "daemon: kill exec process on ctx cancel" says it "has been broken since Docker v17.11" and added a TERM on client-context cancel with a 10 s escalation to KILL; which release contains it was not determined) [CITED: moby commit e7f4963 via search results]. Test (a) SIGTERM the host-side `docker exec` client and check `docker exec <c> ps -eo pid,ppid,pgid,args | grep 'dsh --profile headless'`; (b) launch with a recorded PID: `docker exec <c> sh -c 'echo $$ > /tmp/s6.pid; exec dsh --profile headless "<task>"'` then `docker exec <c> kill -TERM $(docker exec <c> cat /tmp/s6.pid)`; (c) `docker exec <c> pkill -TERM -f 'dsh --profile headless'` (pattern is distinct from the web server's `dsh web`). `ps`, `timeout`, `setsid`, `kill` exist in the container [VERIFIED: `which` output]. Never `docker restart`/`stop` the container - it also serves the web UI, and `restart.sh` rebuilds the image.
- Fallback alternative to record if stream-json parity is Không đạt: run Claude Code on the host with `ANTHROPIC_BASE_URL` -> DeepSeek Anthropic-format endpoint (`https://api.deepseek.com/anthropic`, which maps unknown model names to `deepseek-v4-flash`) [CITED: api-docs.deepseek.com/guides/anthropic_api via search]; usable only with a DeepSeek key that targets that endpoint - not verified for the corporate gateway key [ASSUMED]. Alternatively parse `dsh` output text and log via git diff (mirrors the Antigravity-manual fallback in ENG-06).
- **Secrets:** `DEEPSEEK_API_KEY` exists only in the container env; the spike must never run `env`, `docker inspect` without the names-only template, or `cat .env.docker`; scan gate must include the container key *value* if it is ever exported to the host (it should not be).

### S7 - Antigravity headless (SPK-07, D-11)
- **Installed:** `agy` at `~/.local/bin/agy`, version `1.3.5` [VERIFIED: `command -v` and `agy --version`]; `/usr/local/bin/antigravity` is the **IDE launcher** (see Pitfall 9).
- **Local `agy --help` (verbatim excerpts):** "`--print  Run a single prompt non-interactively and print the response`", "`--output-format  Output format for print mode (text, json, stream-json) (default text)`", "`--input-format  Input format for print mode (text, stream-json). stream-json reads one NDJSON message per line from stdin and runs a turn for each; it requires --output-format stream-json`", "`--print-timeout  Optional time limit for print mode; 0 waits until the turn completes (default 0s)`", "`--conversation  Resume a previous conversation by ID`", "`--dangerously-skip-permissions`", "`--mode  ... (accept-edits, auto, plan)`", "`--json-schema`", "`--log-file`", "`--sandbox`" [VERIFIED: command output]. **No `--max-turns` flag is listed** - this is an absence of evidence; the official docs say only that the page "does not document a maximum number of turns or steps" (my reading of the fetched page) [ASSUMED conclusion: no turn-limit flag]. Falsification attempt for the spike: `agy -p x --max-turns 1 </dev/null` and paste the output (an unknown-flag error would confirm; success would refute the claim).
- **Official docs (headless page):** output formats `text|json|stream-json`; `json` envelope fields `conversation_id, status, response, error, duration_seconds, num_turns, usage`; `usage` has `input_tokens, output_tokens, thinking_tokens, cache_read_tokens, total_tokens`; `stream-json` events `init`, `step_update`, `result`; status values `SUCCESS, ERROR, CANCELED, INTERRUPTED, INVALID, WAITING, RUNNING`; default 5-minute run wait (`--print-timeout`); "Headless mode relies on cached credentials, so authenticate with an interactive `agy` session first"; unapproved tools are "soft-denied" (run exits 0) [CITED: antigravity.google/docs/cli/headless].
- **Known history:** `agy -p` produced zero stdout when stdout was a pipe in 1.0.0-1.0.16 (fixed 1.0.15; later swallowed-error silent exit fixed in 1.1.1) [CITED: github.com/google-antigravity/antigravity-cli/issues/76]. Installed 1.3.5 is newer, but the plan must still run with stdout redirected to a file (the real Phase 6 mode) and `</dev/null` (stdin hang history).
- **Proposed local confirmation (allowed by D-11, tiny cost):** `timeout 120 agy -p "Reply with the single word OK" --output-format json </dev/null > raw/agy-json.json` and a second run with `--output-format stream-json` -> record event types and whether `usage` appears. If `authentication required`, record that as the finding (user login is interactive) instead of working around it. State plainly in results whether S7 conclusions come from docs only or docs + a local run.
- Result shape to preview for Phase 6 (ENG-06): headless **yes**, JSON **yes**, turn-limit flag **not documented**; Phase 6 can integrate as an engine but cannot rely on a turn cap (use `--print-timeout` as the bound).

### S8 - `--max-turns` (SPK-08)
- `--max-turns` is **documented but missing from `claude --help`** output [VERIFIED: grep of `claude --help` for `turns` returned nothing; cli-reference explicitly says "`claude --help` does not list every flag"] [CITED: "Limit the number of agentic turns (print mode only). Exits with an error when the limit is reached."]. Do not "verify absence" by `--help`.
- On hitting the cap the final line is a `result` with `subtype: "error_max_turns"` (type has `errors: string[]` and **no `result` text field**) and `terminal_reason: "max_turns"` [CITED: agent-sdk/typescript SDKResultMessage]. Therefore the spec command `claude -p "review git diff" --max-turns 1 --output-format json` is likely to spend its single turn on a tool call (`git diff`) and return **no usable review** [ASSUMED outcome; the spike decides]. Test two variants and record both: (A) literal spec command; (B) the form FAST-02 needs: put the diff in the prompt/stdin (`git diff | claude -p "review this diff" --max-turns 1 --tools "" --output-format json` - `--tools ""` disables all built-in tools [CITED: cli-reference `--tools`]) -> expect `subtype: success`, `num_turns: 1`, non-empty `result`.
- Also record the process exit code on `error_max_turns` (docs say "Exits with an error"; exact code unverified [ASSUMED non-zero]).
- Bonus for FAST-02 ("không tạo session lưu lại"): add `--no-session-persistence` and assert no new transcript file appears under `~/.claude/projects/<proj>/` [CITED: cli-reference].
- Use a scratch git repo with a tiny diff, not this repo (the diff text goes into the committed fixture; keep it non-sensitive).

### SPK-10 - profile/account decision
- Not research-resolvable. Record observed facts (company Team seat vs personal Pro, above), ask the user to confirm the company policy, and write the decision into `spike-results.md` plus update STATE.md blocker. Do not describe the policy as confirmed unless the user says so. Provide a table of the two profiles' observable properties (org type, seat tier, role, rate-limit tier fields) - without emails or UUIDs.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Parsing stream-json for observations | Regex over raw text | `jq` on each JSONL line (`jq -c 'select(.type=="result")'`) | Documented fields are JSON; Phase 3 will use serde, spikes should prove field presence with a real parser |
| Process discovery for S4 | `ps | grep` string matching | `pgrep -af '^sleep 317$'` + PPID/PGID/SID columns (`ps -eo pid,ppid,pgid,sid,cmd --forest`) | Other `sleep`/`claude` processes exist; need ancestry, not names |
| Session-unchanged proof for S2 | Eyeballing | `sha256sum` + `wc -l` of the transcript before/after | Objective, repeatable |
| Secret detection | Ad-hoc grep from memory | `scrub.py` + `scan-secrets.sh` with env-value denylist | One mechanical gate; the same script re-used for each spike |
| Waiting for 65 minutes | Blocking tool call / manual sleeps | detached `setsid nohup` schedule script + log polling | The Bash tool auto-backgrounds long calls (observed: a >120 s command was moved to background) |
| Token accounting | Summing every assistant line | Use `result.usage`/`modelUsage`; dedupe by `message.id` if reading per-step | Parallel tool calls share id+usage; per-step `output_tokens` is a placeholder |

**Key insight:** every observation this phase needs is already emitted as structured JSON by the tools; the work is choosing clean experimental conditions (unique prefixes, own process groups, unique markers), not building measurement code.

## Common Pitfalls

### Pitfall 1: Running `claude -p` from inside Claude Code inherits the parent's env
**What goes wrong:** child sees `CLAUDECODE=1`, `CLAUDE_CODE_CHILD_SESSION=1`, a parent messaging socket/token, etc.; results may not reflect a top-level launch (Phase 3 spawns from a normal terminal).
**How to avoid:** `env -u ...` wrapper (Pattern 1); record in NOTES that runs were from a nested context and which variables were unset. Never `pkill claude`.
**Warning signs:** sessions missing from `claude agents`, odd hook output in stream.

### Pitfall 2: S3 prefix instability (git status, directory, other sessions)
**What goes wrong:** cache reads vanish because `git status` changed, files were written into the workdir, the cwd differs, or another session in the same dir interleaved.
**How to avoid:** dedicated non-repo workdirs, outputs written elsewhere, identical flags, unique nonce per probe. Interpret a miss only after checking those.
**Warning signs:** `cache_creation` on the 1-minute probe.

### Pitfall 3: Assuming `--max-turns 1` gives an answer
**What goes wrong:** the one turn is consumed by a tool call; result is `error_max_turns` with no `result`. **How to avoid:** embed the diff and disable tools for the usable variant (S8-B). **Warning signs:** empty `.result`, `is_error: true`.

### Pitfall 4: Group-kill does not reach Bash-tool children
**What goes wrong:** `kill -- -PGID` of `claude` leaves `sleep` running (children are in their own sessions). **How to avoid:** SIGTERM the `claude` PID (documented tree-kill), then verify by marker; add descendant-walk fallback. SIGKILL of `claude` cannot run its handler -> orphans.
**Warning signs:** PPID 1 (or subreaper) on the leftover `sleep`.

### Pitfall 5: Executing kill commands blocked by the permission classifier
**What goes wrong:** the executing agent's `kill` is denied (observed denial reason `[Interfere With Workloads]`), stalling S4/S6. **How to avoid:** `checkpoint:human-action` early; script signals only its own recorded PIDs; do not try to bypass the denial.

### Pitfall 6: `ps | grep sleep` noise and wrong-process kills
**What goes wrong:** 8 unrelated `sleep` matches existed; VS Code extension `claude` (PID 224336) and this session `claude` (282882) are alive. **How to avoid:** unique marker + recorded PIDs.

### Pitfall 7: Touching `~/.claude-orca` or `~/.claude`
**What goes wrong:** Claude refreshes tokens and rewrites `.credentials.json`/`.claude.json` -> modifies the "do not touch" original; explicit `CLAUDE_CONFIG_DIR=~/.claude` may confuse the work profile. **How to avoid:** only run `claude` against the clone and the unset default; no `claude` against `~/.claude-orca`.

### Pitfall 8: `--bare` kills subscription auth
**What goes wrong:** `--bare` never reads OAuth/keychain; spike returns "Not logged in". **How to avoid:** do not use `--bare` for S1-S5/S8; if a clean-config run is desired use `--safe-mode` (keeps auth, disables hooks/CLAUDE.md/skills/MCP) and note it changes the cache prefix [CITED: cli-reference `--safe-mode`].

### Pitfall 9: `antigravity --version` starts the IDE
**What goes wrong:** the bare `antigravity` binary is the Electron IDE launcher; during this research `antigravity --version` launched the GUI and left a process group (PGID 333492, ~12 processes, DevTools listening on 127.0.0.1) running. **How to avoid:** use only `agy`. **Cleanup note:** the researcher's own `kill` of that group was denied by the classifier; the user should close that Antigravity window/process group.

### Pitfall 10: Bash tool timeouts and stdin
**What goes wrong:** commands >120 s are auto-backgrounded; `claude -p`/`agy -p` may read an open stdin. **How to avoid:** `</dev/null`, `timeout`, detached scripts for long runs; `run_in_background` + polling for anything >2 min.

### Pitfall 11: Root-owned files from the container
**What goes wrong:** S6 edits create root-owned files under the repo mount. **How to avoid:** scratch dir git-ignored, removed through `docker exec ... rm -rf`; never point the harness at `spikes/` itself.

### Pitfall 12: Sensitive data in "innocent" fields
`system/init` carries `cwd`, plugin paths, tool and skill lists (absolute home paths); hook events may echo hook output; tool results echo file contents; `git log/diff` output can contain author emails; `claude auth status` JSON contains the account email and org; `~/.claude.json` holds `oauthAccount` and a usage cache with `accountUuid`. Scrub and scan every file (Pattern 2) and never commit `auth status` raw.

## Code Examples

### Extract the fields the S1 requirement asks for
```bash
# Source: field names from code.claude.com/docs/en/agent-sdk/typescript (SDKSystemMessage, SDKResultMessage, Usage)
jq -c 'select(.type=="system" and .subtype=="init") | {type,subtype,session_id,model,claude_code_version}' raw/s1.jsonl
jq -c 'select(.type=="assistant") | {sid:.session_id, id:.message.id, blocks:[.message.content[]?.type]}' raw/s1.jsonl
jq -c 'select(.type=="result") | {subtype,is_error,num_turns,session_id,total_cost_usd,
        usage:{i:.usage.input_tokens,o:.usage.output_tokens,cr:.usage.cache_read_input_tokens,
               cc:.usage.cache_creation_input_tokens,t5:.usage.cache_creation.ephemeral_5m_input_tokens,
               t1:.usage.cache_creation.ephemeral_1h_input_tokens}}' raw/s1.jsonl
```

### S3 schedule skeleton (detached)
```bash
# spikes/s3/schedule.sh — start with: setsid nohup bash spikes/s3/schedule.sh >spikes/s3/raw/schedule.log 2>&1 &
set -u; . "$(dirname "$0")/../lib/env.sh"
for k in 1 10 65; do mk_workdir "$k"; done            # unique nonce CLAUDE.md in each, outside the repo
for k in 1 10 65; do probe "$k" t0; done              # cold run, save json
sleep $((1*60));  probe 1  probe                      # ~1 min after t0 (account for run durations)
sleep $((9*60));  probe 10 probe                      # ~10 min after t0
sleep $((55*60)); probe 65 probe                      # ~65 min after t0
# probe(): run_claude -p "Reply with the single word OK" --output-format json  (cwd = workdir k)
```
(`mk_workdir`/`probe` are helpers to write in Wave 0; keep wall-clock timestamps in each output name so gaps are computed from real times, not from `sleep` arguments.)

### S4 verification snapshot
```bash
snap() { ps -eo pid,ppid,pgid,sid,etimes,cmd --forest | grep -E "claude|sleep 317|bash -c" | grep -v grep; }
snap > raw/ps-before.txt; kill -TERM "$CLAUDE_CHILD_PID"; sleep 5; snap > raw/ps-after-term.txt
pgrep -af '^sleep 317$' > raw/leftover-term.txt || echo "none" >> raw/leftover-term.txt
```

### Secret scan gate (names/values never printed)
```bash
# Fail if any env var whose NAME looks sensitive has its VALUE present in spikes/
for n in $(env | cut -d= -f1 | grep -iE 'key|token|secret|password'); do
  v="${!n}"; [ ${#v} -ge 8 ] && grep -rqF -- "$v" spikes/ && { echo "LEAK: value of \$$n present"; exit 1; }
done
grep -rEn 'sk-[A-Za-z0-9_-]{16,}|sk-ant-|Bearer [A-Za-z0-9._-]{16,}|eyJ[A-Za-z0-9_-]{10,}\.' spikes/ && exit 1
grep -rEn '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[a-z]{2,}' spikes/ && exit 1   # emails (allow-list in file if needed)
grep -rn "/home/$USER" spikes/ && exit 1
exit 0
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Assume 5-minute cache TTL everywhere | Subscription main conversation requests 1-hour TTL; API key/credits get 5 min; `CLAUDE_CODE_PROMPT_CACHE_TTL` / `promptCacheTtl` (v2.1.242+) | Claude Code 2.1.2xx docs | S3 must test 1h; Phase 4 golden-session window is ~1 h on subscription |
| Assume `kill_on_drop` / group kill reaches all children | Bash-tool children detach into their own session; Claude Code handles SIGTERM with its own tree-kill | observed on 2.1.299 | Phase 3 kill must be SIGTERM-first plus descendant sweep |
| `ANTHROPIC_BASE_URL` for DeepSeek | Docker `dsh` harness (headless profile) | per D-07 | No stream-json from `dsh`; different parser path |
| Antigravity assumed GUI-only | `agy` CLI with `-p`, `json`/`stream-json` output | CLI 1.1.8+ stream-json | Phase 6 can integrate as engine |

**Deprecated/outdated:** `--remote` (alias of `--cloud`); `--enable-auto-mode` (removed v2.1.111) [CITED: cli-reference]. `--bare` is recommended for scripted calls "and will become the default for `-p` in a future release" [CITED: headless] - a future default change would break subscription-auth spikes; pin the behaviour by recording `claude --version` in each result.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | stream-json in `-p` *requires* `--verbose` (docs only show it in examples) | S1 | Phase 3 spawn command wrong; S1 negative test resolves it |
| A2 | Exact stream line layout (tool_use inside `assistant`, tool_result in `user`) matches the SDK types | S1 | Parser design (LOG-01/02); S1 resolves |
| A3 | Env vars like `CLAUDECODE` do not change `-p` behaviour; unsetting is the conservative choice | Pattern 1 | Low; results labelled "nested" |
| A4 | CLI `--fork-session` first request reads the parent's cache | S2 | OPT-07 decision input only |
| A5 | OAuth refresh tokens may rotate, so a pure credential clone could invalidate the original | S5 | Could log the user out of the personal account; mitigated by excluding credentials or backing up |
| A6 | Setting `CLAUDE_CONFIG_DIR=~/.claude` explicitly misbehaves because `.claude.json` is expected inside | S5 | PROF-01/02 design; mitigated by "unset for default profile" |
| A7 | `agy` has no turn-limit flag (absence evidence: help + docs) | S7 | ENG-06 design; falsification test provided |
| A8 | `--max-turns 1` on "review git diff" ends as `error_max_turns` with no usable result; exit code non-zero | S8 | FAST-02 design; S8 variants A/B resolve |
| A9 | Killing the `docker exec` client may leave the in-container process | S6 | Kill design for harness; test (a)-(c) resolves |
| A10 | SIGKILL of `claude` orphans its Bash child (SIGTERM does not) | S4 | PROC-01/02 design; S4 resolves |
| A11 | DeepSeek's Anthropic-format endpoint accepts the corporate key / gateway | S6 fallback | Only affects the documented alternative |
| A12 | Minimum cacheable prefix is small enough that a ~12k-token CLAUDE.md is cached on the active model | S3 | A too-small prefix gives zeros; observe `cache_creation` on t0 and enlarge if 0 |
| A13 | Orca hooks in `~/.claude/settings.json` are inert unless `ORCA_AGENT_HOOK*` env is set | S1 | Extra hook lines/ side effects in runs |

## Open Questions

1. **Does the company policy allow using the VCCorp Team seat for work only, and the personal Pro for personal/video work?**
   - Known: default = `claude_team`/`VCCorp`; orca = `claude_pro`, gmail domain.
   - Unclear: the company's written policy; whether "personal" runs may ever touch the company seat.
   - Recommendation: human checkpoint in the plan; record the user's answer verbatim in SPK-10 and STATE.md.
2. **Does `agy -p` run unauthenticated or is login needed on this machine?**
   - Known: `~/.gemini/antigravity-cli` exists (credentials not inspected).
   - Recommendation: run the tiny check; if "authentication required", record it and leave login to the user.
3. **Can `dsh` be driven for streaming (web API/SSE) instead of headless?**
   - Known: `dsh web` serves a UI on 3081 (proxied to 3080); headless prints only the final message.
   - Recommendation: out of scope for S6; list as Phase 6 research item if S6 stream parity fails.
4. **Is explicit `CLAUDE_CONFIG_DIR=~/.claude` equivalent to unset?** Not safe to test live; decide the Phase 2/3 rule (unset for work) and test only on a throwaway copy if the user wants proof.
5. **Does the research-launched Antigravity IDE process group (PGID 333492) need cleaning up?** The researcher's kill was denied; the user must close it.
6. **Does a weekly utilisation of 82% on the Team seat limit how many spike runs are affordable?** Keep spike prompts tiny; S3 uses ~6 small runs.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `claude` CLI | S1-S5, S8 | ✓ | 2.1.299 | — |
| `agy` | S7 | ✓ | 1.3.5 | docs-only result (state it) |
| `antigravity` (GUI launcher) | — (do NOT run) | ✓ | n/a | use `agy` |
| Docker + container `dsh-deepseek-harness` | S6 | ✓ (Up) | Docker 29.8.1; `dsh` 0.1.1-rc.2 | docs-only result |
| `jq` | S1-S4, S8 | ✓ | system | python3 json |
| `python3` | scrub/scan | ✓ | 3.12.3 | — |
| `node` | (GSD tooling) | ✓ | v24.21.0 | — |
| `setsid`, `pgrep`, `ps`, `pstree`, `timeout`, `script` | S4 | ✓ | system | — |
| `git` | S8 scratch repo | ✓ | system | — |
| cgroup v2, `systemd-run` | S4 optional step 3 | ✓ / `systemd --user` state `degraded` | — | skip optional step |
| `gitleaks` / `trufflehog` | scan gate | ✗ | — | own `scrub.py` + `scan-secrets.sh` (do not install) |
| Network to Anthropic API | S1-S5, S8 | ✓ (this session works) | — | — |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** gitleaks/trufflehog (own scripts).

## Validation Architecture

(`.planning/config.json` does not exist, so `workflow.nyquist_validation` is treated as enabled.)

This phase produces evidence, not product code, so "tests" are mechanical checks on the evidence package.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Plain bash + jq assertions (no framework install) |
| Config file | none — see Wave 0 |
| Quick run command | `bash spikes/lib/check-results.sh && bash spikes/lib/scan-secrets.sh` |
| Full suite command | `for s in 1 2 3 4 5 6 7 8; do bash spikes/s$s/verify.sh; done && bash spikes/lib/scan-secrets.sh && bash spikes/lib/check-results.sh --final` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SPK-01 | `s1.jsonl` has an `init` line with `session_id` and a final `result` line with `usage`; negative `--verbose` error captured | smoke | `jq -e 'select(.type=="result") | .usage and .session_id' spikes/s1/raw/s1.jsonl` | ❌ Wave 0 |
| SPK-02 | resume id equality recorded; fork id != original; original transcript hash unchanged | smoke | `bash spikes/s2/verify.sh` | ❌ Wave 0 |
| SPK-03 | six JSON outputs exist; for each probe warm/cold classification computable | smoke | `bash spikes/s3/verify.sh` (computes thresholds from `raw/*.json`) | ❌ Wave 0 |
| SPK-04 | `ps` snapshots before/after both signals + leftover files exist | smoke | `bash spikes/s4/verify.sh` | ❌ Wave 0 |
| SPK-05 | two `auth status` captures (redacted) differ in org/seat type; transcripts under distinct dirs; cross-resume error captured | smoke | `bash spikes/s5/verify.sh` | ❌ Wave 0 |
| SPK-06 | stdout captured, file edited or explicitly not; leftover check after each kill variant | smoke | `bash spikes/s6/verify.sh` | ❌ Wave 0 |
| SPK-07 | `agy` help + docs notes + (optional) json/stream-json captures present; absence test output pasted | smoke | `bash spikes/s7/verify.sh` | ❌ Wave 0 |
| SPK-08 | variants A and B both have `subtype`, `num_turns`, exit code recorded | smoke | `bash spikes/s8/verify.sh` | ❌ Wave 0 |
| SPK-09 | 8 sections, each with Ngày thử/Lệnh/Kết quả/Quan sát/Ảnh hưởng, "Không đạt" has alternative; no `(chờ duyệt)` after approval | lint | `bash spikes/lib/check-results.sh --final` | ❌ Wave 0 |
| SPK-10 | decision section present and references user confirmation | manual-only | user confirms in checkpoint (justification: company policy cannot be machine-checked) | n/a |

### Sampling Rate
- **Per task commit:** `bash spikes/lib/scan-secrets.sh` (must pass before any `git add`).
- **Per wave merge:** quick run command above.
- **Phase gate:** full suite green + user approval (labels removed) before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `spikes/lib/env.sh` — nested-env scrub, `run_claude`, timestamp helper
- [ ] `spikes/lib/scrub.py` and `spikes/lib/scan-secrets.sh` — redaction + gate
- [ ] `spikes/lib/check-results.sh` — template/section validator (`--final` forbids `(chờ duyệt)`)
- [ ] `spikes/spike-results.md` skeleton with the 8 sections from 01-phase0 §5 plus "Phương án thay thế" and "Phase bị ảnh hưởng" fields
- [ ] `spikes/sN/verify.sh` stubs
- [ ] `.gitignore` additions for `spikes/**/work/`, `spikes/**/scratch/`
- Framework install: none.

## Security Domain

`security_enforcement` is not set in config (absent = enabled).

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes (profile OAuth, harness key) | Never read/print `.credentials.json` values; use `claude auth status` output redacted; no re-use of refresh tokens across dirs without backup |
| V3 Session Management | partial | Session IDs are not secrets but identify transcripts; keep transcripts (`~/.claude/projects/...`) out of the repo |
| V4 Access Control | yes | `chmod 700` the cloned profile dir; container runs as root with `/var/run/docker.sock` mounted (anything the harness runs can control Docker) - do not widen |
| V5 Input Validation | partial | Scrub script must handle malformed lines without crashing; fixtures keep malformed lines raw only if scrubbed |
| V6 Cryptography | no | none implemented; never hand-roll token handling |
| V7/V8 Data protection & logging | yes | Scrub-then-scan gate (D-03) before commit; value-denylist built from live env var values, never echoed |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Secrets committed in fixtures (API keys, OAuth tokens, `DEEPSEEK_API_KEY`) | Information disclosure | `scrub.py` + `scan-secrets.sh` hard gate; never print env values; `docker inspect` with names-only template |
| Emails / org names / account UUIDs in `auth status` and `oauthAccount` | Information disclosure | Redact or omit; commit only derived facts (org type, seat tier) |
| Absolute home paths and usernames in `init` lines | Information disclosure | `$HOME` -> `~` rewrite |
| Credential clobber of `~/.claude-orca` / work profile | Tampering | Only run against clone/default; exclude credentials from clone; backup before any credential copy |
| Root-owned writes through the Docker bind mount | Tampering / DoS | Scratch dir only, git-ignored, removed through `docker exec` |
| Over-broad kill (`pkill`, group of wrong PID) terminating the operator's own Claude session | Denial of service | Kill only recorded PIDs started by the script; unique marker |
| Prompt content from tool results containing untrusted text | Tampering | Spike prompts are fixed strings; scratch dirs contain only generated files |

## Sources

### Primary (HIGH confidence)
- Local probes this session (read-only): `claude --version/--help`, `agy --version/--help`, `docker inspect/top/exec`, `dsh --help`, `ps`, `ls`, JSON key inspection of `~/.claude.json`, `~/.claude-orca/.claude.json`, `.credentials.json` (keys/non-secret fields only)
- /mnt/data/IDE/QuangVy-Orchestration/files/01-phase0-kiem-chung.md, .planning/phases/01-verify-assumptions/01-CONTEXT.md, REQUIREMENTS.md, STATE.md
- /mnt/data/IDE/Deepseek-harness/README.md and setup/docker/{entrypoint.sh,headless.patch.yml} (non-secret files)

### Secondary (MEDIUM confidence — official docs via WebFetch; context7 not available in this runtime)
- https://code.claude.com/docs/en/prompt-caching (TTL buckets, scope, fields)
- https://code.claude.com/docs/en/headless (`-p`, stream-json, SIGTERM, bare)
- https://code.claude.com/docs/en/cli-reference (flags incl. `--max-turns`, `--fork-session`, `--no-session-persistence`, `--tools`)
- https://code.claude.com/docs/en/agent-sdk/typescript (SDKSystemMessage / SDKResultMessage / Usage) and /agent-sdk/cost-tracking (dedupe by message id)
- https://code.claude.com/docs/en/sessions (resume/fork, transcript path)
- https://code.claude.com/docs/en/env-vars, /authentication, /claude-directory (CLAUDE_CONFIG_DIR, credentials, CLAUDECODE vars)
- https://www.antigravity.google/docs/cli/headless and https://github.com/google-antigravity/antigravity-cli/issues/76

### Tertiary (LOW confidence — search results only)
- moby commit "daemon: kill exec process on ctx cancel" (via search result summary; release not identified)
- https://api-docs.deepseek.com/guides/anthropic_api (via search summary; not fetched)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all tools probed locally; no installs.
- Architecture: HIGH — spike layout/gates follow directly from locked decisions.
- Predicted spike outcomes: MEDIUM — grounded in docs + verified process layout, but each is explicitly an [ASSUMED] hypothesis the spike must confirm.
- Pitfalls: HIGH for those observed this session (env inheritance, classifier denial, GUI launcher, session/pgid layout); MEDIUM for docs-derived ones.

**Research date:** 2026-10-14
**Valid until:** ~2026-10-21 for Claude Code flag/doc details (fast-moving, v2.1.2xx cadence); re-run `claude --version` at spike time and record it.
