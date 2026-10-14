# Constraints (from SPEC-classified docs)

All entries carry source attribution. Content is taken from the source docs; no content added.

---

## Phase 1: projects_map.json schema
- source: /mnt/data/IDE/QuangVy-Orchestration/files/02-phase1-giao-thuc-mvp.md (section 4.1)
- type: schema
- content:
```json
{
  "root": "~/projects",
  "projects": [
    {
      "id": "web-ban-hang",
      "path": "~/projects/web-ban-hang",
      "name_vi": "Web bán hàng",
      "aliases": ["shop", "bán hàng", "web shop"],
      "type": "code",
      "repo": "p.zamba",
      "profile": "work"
    }
  ]
}
```
`type`: `code` or `media` (prepared for Phase 5). `profile`: `work` or `personal`.

## Phase 1: project routing and Fallback Ask rule
- source: /mnt/data/IDE/QuangVy-Orchestration/files/02-phase1-giao-thuc-mvp.md (section 3.2)
- type: protocol
- content:
  - Scan script scans root dir and adds new projects to `projects_map.json`; must not overwrite hand-edited aliases.
  - Name matching: strip diacritics on both sides, compare against `name_vi` and `aliases`.
  - Fallback Ask: match count 0 or more than 1 -> stop, list projects, ask again. Never guess.
  - Scan script warns on duplicate aliases across projects (section 8).

## Phase 1: three-role protocol (Lead, Dev, QA)
- source: /mnt/data/IDE/QuangVy-Orchestration/files/02-phase1-giao-thuc-mvp.md (section 3.3, 4.3)
- type: protocol
- content:
  - Lead: may read code and survey; forbidden to edit code files; input = task request + `.gsd`; output = `.plan.md`.
  - Dev: edits exactly at positions in plan; forbidden to rescan project or edit outside plan; input = `.plan.md`; output = commit on task branch (message "<task>: <short description>").
  - QA: reads diff, flags risks; forbidden to edit code; input = `git diff` + `.plan.md`; output = `.qa-report.md` (per-item plan match, out-of-plan edits, risks, verdict Đạt / Cần sửa).
  - If Dev finds plan wrong or missing: write to "Vướng mắc" section in `.plan.md` and stop.
  - Mitigation for rule violations: restrict tools via `--allowedTools` / `--disallowedTools`, not only prompt.
  - Roles exchange information only via files (.plan.md, git diff, .qa-report.md).

## Phase 1: .plan.md template
- source: /mnt/data/IDE/QuangVy-Orchestration/files/02-phase1-giao-thuc-mvp.md (section 4.2)
- type: schema
- content:
```markdown
# Plan: <tên task>
## Mục tiêu
## Thay đổi
| # | File | Dòng / hàm | Sửa gì | Lý do |
|---|---|---|---|---|
## Không được đụng tới
## Cách kiểm tra
## Vướng mắc (Dev ghi vào đây nếu có)
```

## Phase 1: brain script commands
- source: /mnt/data/IDE/QuangVy-Orchestration/files/02-phase1-giao-thuc-mvp.md (section 4.4)
- type: protocol
- content:
```
brain lead  "<dự án>" "<yêu cầu>"   -> route -> create branch -> run Lead
brain dev   "<dự án>"               -> run Dev reading .plan.md
brain qa    "<dự án>"               -> run QA on git diff
```
Shell script at this phase. Profile chosen via `CLAUDE_CONFIG_DIR` from project `profile` field. Role prompts stored as separate files, may use `--append-system-prompt`. Draft Lead/Dev/QA prompts are in section 4.3 of the source.

## Phase 1: task log template (data for Phase 3)
- source: /mnt/data/IDE/QuangVy-Orchestration/files/02-phase1-giao-thuc-mvp.md (section 7)
- type: schema
- content: per task record: Task, Dự án, number of Dev turns before starting edits, whether Dev read files outside plan, what QA found, whether rework was needed and why.

## Phase 2: agent-brain CLI command contract
- source: /mnt/data/IDE/QuangVy-Orchestration/files/03-phase2-loi-rust.md (section 4.1)
- type: api-contract
- content:
```
agent-brain projects scan
agent-brain run <role> "<dự án>" "<yêu cầu>"   # role: lead | dev | qa
agent-brain status "<dự án>"                   # fast-path level 1
agent-brain inspect "<dự án>"                  # fast-path level 2
agent-brain kill <task_id>
agent-brain stats [--profile work|personal]
```

## Phase 2: SQLite schema (initial)
- source: /mnt/data/IDE/QuangVy-Orchestration/files/03-phase2-loi-rust.md (section 4.2)
- type: schema
- content:
```
projects(id, path, name_vi, aliases_json, type, profile)
tasks(id, project_id, title, branch, status, created_at)
runs(id, task_id, role, engine, profile, session_id, pid, status,
     started_at, ended_at, input_tokens, output_tokens,
     cache_read_tokens, cache_write_tokens, cost_usd)
events(id, run_id, ts, kind, raw_json)
```

## Phase 2: process management (clean kill of process tree)
- source: /mnt/data/IDE/QuangVy-Orchestration/files/03-phase2-loi-rust.md (section 3.2)
- type: protocol
- content: Tokio `kill_on_drop` only kills the direct child; Claude CLI spawns MCP servers and child shells. Required: spawn child in its own process group; on kill send SIGTERM to whole group, wait a few seconds, then SIGKILL whole group; keep `kill_on_drop` as second safety layer.

## Phase 2: stream-json log capture
- source: /mnt/data/IDE/QuangVy-Orchestration/files/03-phase2-loi-rust.md (section 3.3, 7)
- type: protocol
- content: read stdout line by line, parse JSON, classify: init (has `session_id`), assistant message, tool call, tool result, final `result` (has usage). Unparseable lines are stored raw and must not crash the program. Store `raw_json`; parse softly (ignore unknown fields) because format varies with CLI version.

## Phase 2: fast-path levels
- source: /mnt/data/IDE/QuangVy-Orchestration/files/03-phase2-loi-rust.md (section 3.4)
- type: protocol
- content:
  - Level 1 zero-token: Rust runs `git status`, `git log`, `git diff --stat` itself; 0 tokens.
  - Level 2 1-turn inspector: Rust gathers `git diff` into prompt, runs `--max-turns 1`, no saved session; low tokens.
  - Level 3 warm reuse: follow-up into just-run session (`--resume`); deferred to Phase 3.

## Phase 2: module layout and tech
- source: /mnt/data/IDE/QuangVy-Orchestration/files/03-phase2-loi-rust.md (section 4.3, 5)
- type: protocol
- content: `src/main.rs` (command parsing), `projects.rs` (scan + name match, ported from Phase 1), `runner.rs` (spawn, process group, kill), `stream.rs` (read + parse stream-json), `db.rs` (SQLite), `fastpath.rs` (git status/diff, inspector), `profile.rs` (select CLAUDE_CONFIG_DIR / engine env vars). Dependencies: Tokio, rusqlite (bundled), serde_json, clap. Binary must run when copied elsewhere with no extra install (except agent CLI) (P2-09).

## Phase 3: session and token optimization approach
- source: /mnt/data/IDE/QuangVy-Orchestration/files/04-phase3-toi-uu-session.md (section 2)
- type: nfr
- content:
  - Precondition: at least 2 weeks of `runs` data from Phase 2.
  - Prompt cache lives about 5 minutes; fork of a cold base session gets no discount; fork saves turns but not necessarily tokens.
  - Options: A (summarize `.gsd` into CLAUDE.md / `--append-system-prompt`), B (golden session + fork, needs mtime/hash tracking of `.gsd`), C (A default, B when many consecutive tasks).
  - Proposal: do A first, re-measure; do B only if data shows significant wasted tokens and tasks run close together.
  - Session length control: track total context tokens after each run; initial threshold 45,000 (tunable from data); over threshold -> write `.handover.md` then open new session loading handover.
  - Phase must not close without P3-06 (measured token/task reduction).

## Phase 3: .gsd-summary.md and .handover.md templates
- source: /mnt/data/IDE/QuangVy-Orchestration/files/04-phase3-toi-uu-session.md (section 3.1, 3.2)
- type: schema
- content:
```markdown
# <Dự án> — tóm tắt cho agent
## Công nghệ
## Cấu trúc thư mục chính
## Quy ước code
## Lệnh build / test
## Những chỗ nhạy cảm, không tự ý sửa
```
Regenerated when `.gsd` hash changes.
```markdown
# Handover: <task>
## Đã hoàn thành
## Đang dở
## Quyết định đã chốt
## File liên quan
## Bước tiếp theo
```

## Phase 3: sessions table
- source: /mnt/data/IDE/QuangVy-Orchestration/files/04-phase3-toi-uu-session.md (section 3.3)
- type: schema
- content:
```
sessions(id, project_id, task_id, kind, parent_session_id,
         context_tokens, gsd_hash, created_at, last_used_at)
-- kind: base | task | fork
```

## Phase 4: embedded dashboard HTTP API
- source: /mnt/data/IDE/QuangVy-Orchestration/files/05-phase4-dashboard.md (section 4.2)
- type: api-contract
- content:
```
GET  /api/projects
POST /api/projects/scan
GET  /api/tasks
POST /api/tasks                 # assign task
GET  /api/tasks/:id
GET  /api/runs/:id/stream       # SSE
POST /api/runs/:id/kill
GET  /api/stats?profile=work
GET  /api/projects/:id/status   # fast-path level 1
```

## Phase 4: security and embedding constraints
- source: /mnt/data/IDE/QuangVy-Orchestration/files/05-phase4-dashboard.md (section 2, 3.1 to 3.3)
- type: nfr
- content: Axum server must listen only on `127.0.0.1`; simple access token generated at startup and printed to terminal (mandatory). UI is HTML + Tailwind + Alpine.js embedded in the single binary; same origin as API (no Node, venv, CORS). Must work fully offline (build CSS ahead and embed; CDN Tailwind only for experiments). Out of scope: multi-user login, remote internet access. UI keeps Alpine.js simple, one file per screen.

## Phase 4: real-time log data flow
- source: /mnt/data/IDE/QuangVy-Orchestration/files/05-phase4-dashboard.md (section 4.3)
- type: protocol
- content:
```
agent process -stdout-> stream.rs -+-> write SQLite (events)
                                   +-> broadcast channel -> SSE -> browser
```
On browser reload mid-run: read old events from SQLite first, then attach SSE. SSE reconnects automatically and back-fills from SQLite.

## Phase 4: screens
- source: /mnt/data/IDE/QuangVy-Orchestration/files/05-phase4-dashboard.md (section 4.1)
- type: protocol
- content: five screens. Tổng quan (task list running/done/failed, profile selector); Chi tiết task (Lead -> Dev -> QA pipeline, step status, realtime log, Kill button); Giao việc (Vietnamese request input, project pick or ambiguity list); Thống kê (tokens by day, profile, project, role); Dự án (list from `projects_map.json`, edit alias, rescan button).

## Phase 5: teams.json role-routing config
- source: /mnt/data/IDE/QuangVy-Orchestration/files/06-phase5-da-doi-hinh-media.md (section 3.1)
- type: schema
- content:
```json
{
  "teams": {
    "mac-dinh":    { "lead": "claude-work", "dev": "claude-work", "qa": "claude-work" },
    "tiet-kiem":   { "lead": "claude-work", "dev": "deepseek",    "qa": "claude-work" },
    "ca-nhan":     { "lead": "claude-personal", "dev": "deepseek", "qa": "claude-personal" }
  },
  "engines": {
    "claude-work":     { "cmd": "claude", "env": { "CLAUDE_CONFIG_DIR": "~/.claude-work" } },
    "claude-personal": { "cmd": "claude", "env": { "CLAUDE_CONFIG_DIR": "~/.claude-max" } },
    "deepseek":        { "cmd": "claude", "env": { "ANTHROPIC_BASE_URL": "<endpoint>", "ANTHROPIC_AUTH_TOKEN": "<key>" } }
  }
}
```
Do not store keys directly in a file that may be committed; read from environment variables. Filter sensitive env vars before writing logs (P5-07).

## Phase 5: engine routing and quota fallback
- source: /mnt/data/IDE/QuangVy-Orchestration/files/06-phase5-da-doi-hinh-media.md (section 2.1 to 2.3)
- type: protocol
- content: each role can run on a different engine, swappable between tasks, because roles communicate only through files. Engines: Claude (work/personal) via `claude` + `CLAUDE_CONFIG_DIR` (depends on Phase 0 S1, S5); DeepSeek via `claude` + `ANTHROPIC_BASE_URL` + DeepSeek key (S6); Antigravity depends on S7. If S7 shows Antigravity is not headless: use manually (user opens it and hands over `.plan.md`), dashboard tracks via `git diff` instead of log. Quota/rate-limit errors detected from stream or exit code; do NOT auto-switch silently; ask for confirmation (or follow user-configured rule). New engine continues from `.plan.md` / `.handover.md`. QA should always run on the strong engine (risk mitigation); record "Cần sửa" rate per engine.

## Phase 5: media branch
- source: /mnt/data/IDE/QuangVy-Orchestration/files/06-phase5-da-doi-hinh-media.md (section 2.4, 3.2, 3.3)
- type: protocol
- content: multi-GB video files must not go in Orca's Git worktree. Projects with `type: media` run in a separate directory (`/video-studio`) using FFmpeg / Remotion / Python. Agent writes only scripts and render config; render is a separate process managed by the binary (reuses Phase 2 runner, with kill). Approved whiteboard script feeds the build step. Data flow: approved script -> agent writes render config -> render (FFmpeg/Remotion) -> output file.

## Phase 5: SQLite additions
- source: /mnt/data/IDE/QuangVy-Orchestration/files/06-phase5-da-doi-hinh-media.md (section 3.3)
- type: schema
- content:
```
runs.engine        -- already exists, now used for real
media_jobs(id, project_id, script_ref, status, output_path, started_at, ended_at)
```

## Cross-phase: stream/log format tolerance
- source: /mnt/data/IDE/QuangVy-Orchestration/files/03-phase2-loi-rust.md (section 7); /mnt/data/IDE/QuangVy-Orchestration/files/06-phase5-da-doi-hinh-media.md (section 6)
- type: nfr
- content: log formats differ between engines and CLI versions; parser must be soft (ignore unknown fields) and store `raw_json`.
