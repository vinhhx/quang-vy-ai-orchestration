# Context (DOC notes and PRD-like material preserved verbatim)

Source language is Vietnamese; text kept as in the source.

---

## Project vision and goal
- source: /mnt/data/IDE/QuangVy-Orchestration/files/00-ROADMAP.md
> Mục tiêu: một bộ điều phối (orchestrator) nhẹ, chạy 1 file binary, để giao việc cho nhiều AI agent (Claude, DeepSeek, Antigravity…) trên mọi dự án: code, video, khách hàng. Thay thế Paperclip.

## Guiding principles
- source: /mnt/data/IDE/QuangVy-Orchestration/files/00-ROADMAP.md (section 1)
1. Giao thức trước, giao diện sau. Phần mang lại giá trị là cách các agent phối hợp (Lead → `.plan.md` → Dev → QA). Dashboard chỉ là lớp hiển thị.
2. Mỗi giai đoạn phải dùng được ngay. Không có giai đoạn nào chỉ là "chuẩn bị". Hết mỗi phase là bạn làm việc thật bằng nó.
3. Đo trước khi tối ưu. Không tối ưu token (session pool, golden session) khi chưa có số liệu token thật từ Phase 2.
4. Kiểm chứng giả định sớm. Mọi cờ CLI, endpoint, khả năng headless phải được thử trong Phase 0 trước khi thiết kế dựa vào chúng.
5. Bạn là tester, nên mỗi phase có tiêu chí nghiệm thu dạng test case. Agent viết code, bạn nghiệm thu.

## Phase overview and gates
- source: /mnt/data/IDE/QuangVy-Orchestration/files/00-ROADMAP.md (section 2)
(Problem numbers #N refer to an original architecture table that is NOT in the ingest set.)

| Phase | Name | Solves (original table) | Main output | Gate |
|---|---|---|---|---|
| 0 | Kiểm chứng giả định | Nền cho tất cả | `spike-results.md` | Mọi giả định có kết quả Đạt/Không đạt |
| 1 | Giao thức MVP (script) | #5, #6, #11 | `projects_map.json`, 3 prompt Lead/Dev/QA, script chạy | Hoàn thành 5 task thật bằng giao thức |
| 2 | Lõi Rust (CLI, chưa có UI) | #1, #3 (phần bắt log), #6, #9 | binary `agent-brain` dạng CLI + SQLite | Kill sạch cây tiến trình; log token đầy đủ |
| 3 | Tối ưu session & token | #7, #8 | Chiến lược session có số liệu chứng minh | Giảm token/task đo được so với Phase 2 |
| 4 | Dashboard nhúng | #2, #3 | Web UI + SSE trong cùng binary | Xem log realtime, kill từ UI |
| 5 | Đa đội hình & nhánh Media | #4, #10 | Routing engine linh hoạt, pipeline video tách riêng | Chạy 1 task trộn engine end-to-end |

## Phase dependency order
- source: /mnt/data/IDE/QuangVy-Orchestration/files/00-ROADMAP.md (section 3)
```
Phase 0 ──► Phase 1 ──► Phase 2 ──┬──► Phase 3
                                  ├──► Phase 4
                                  └──► Phase 5
```
Sau Phase 2, ba phase còn lại độc lập nhau. Chọn thứ tự theo nỗi đau lớn nhất lúc đó: hết quota thường xuyên -> Phase 5 trước; token/task cao -> Phase 3 trước; khó theo dõi agent -> Phase 4 trước.

## Time estimates (reference only)
- source: /mnt/data/IDE/QuangVy-Orchestration/files/00-ROADMAP.md (section 4)
Assumption: done outside work hours, agent writes code, user reviews and accepts. Phase 0: 2-4 sessions (mostly running commands and recording). Phase 1: 1-2 weeks. Phase 2: 2-4 weeks (riskiest, Rust async). Phase 3: 1-2 weeks (depends on collected data). Phase 4: 2-3 weeks. Phase 5: 2-4 weeks (depends on Phase 0 result about Antigravity).

## Cross-cutting KPIs
- source: /mnt/data/IDE/QuangVy-Orchestration/files/00-ROADMAP.md (section 5)
- Token trung bình / task: from SQLite (from Phase 2); target: decreasing across phases.
- Số turn trước khi agent bắt đầu sửa code: count in stream-json log; target: <= 2.
- Task chạy nhầm repo: manual/log; target: 0.
- Tiến trình còn sót sau Kill: `ps` after kill; target: 0.
- Task phải làm lại do Dev hiểu sai plan: manual; target: decreasing.

## Open questions
- source: /mnt/data/IDE/QuangVy-Orchestration/files/00-ROADMAP.md (section 7)
- Hai tài khoản Claude đều do công ty cấp: có được phép dùng một tài khoản cho dự án cá nhân và dựng video không? Cần xác nhận trước Phase 2 (profile Work/Personal).
- Antigravity có chế độ chạy headless và xuất log có cấu trúc không? (Phase 0 trả lời)
- Chọn dashboard trước (Phase 4) hay routing trước (Phase 5)? Quyết định sau Phase 2.

## Phase 0: goals and assumptions to verify
- source: /mnt/data/IDE/QuangVy-Orchestration/files/01-phase0-kiem-chung.md (section 1, 2)
Goal: kiểm tra bằng thực tế mọi giả định kỹ thuật mà bảng kiến trúc dựa vào, trước khi viết dòng code Rust nào.

| Giả định trong bảng gốc | Thực tế cần kiểm tra |
|---|---|
| `--output-format stream-json` đủ để bắt log | Ở chế độ `-p` cần thêm `--verbose` |
| `kill_on_drop` hủy cả cây tiến trình | Chỉ kill tiến trình con trực tiếp; MCP server, bash con có thể còn sót |
| Fork session luôn ăn prompt cache | Cache sống khoảng 5 phút; session gốc nguội thì không giảm giá |
| Giảm 90% phí nhờ cache | Đúng với API trả theo token; với gói subscription, mức ảnh hưởng tới quota chưa rõ |
| Antigravity chạy được như CLI | Chưa xác nhận có chế độ headless + log JSON |
| DeepSeek dùng được qua endpoint tương thích Anthropic | Cần thử với chính Claude Code qua `ANTHROPIC_BASE_URL` |

## Phase 0: spike list (S1 to S8)
- source: /mnt/data/IDE/QuangVy-Orchestration/files/01-phase0-kiem-chung.md (section 3)
Results are recorded in `spike-results.md` (an output artifact, not an ingested doc).
- S1 Stream JSON: `claude -p "liệt kê file trong thư mục này" --output-format stream-json --verbose > s1.jsonl`. Check: line with `session_id`; final `type: result` line has usage/tokens; each tool call is its own line.
- S2 Resume and fork: `claude -p "nhớ số 42" --output-format json`, then `--resume <id>`, then `--resume <id> --fork-session`. Check: fork creates new session_id; original session unchanged.
- S3 Prompt cache: run same task twice 1 minute apart, then twice 10 minutes apart; compare cache read tokens in `result` line.
- S4 Kill process tree: long command (`sleep 300` via bash), kill `claude` process, run `ps -ef | grep sleep`; record leftovers.
- S5 Profile separation: `CLAUDE_CONFIG_DIR=~/.claude-work claude -p "hi"` and `CLAUDE_CONFIG_DIR=~/.claude-max claude -p "hi"`. Check: each uses its own account and session history.
- S6 DeepSeek via Claude Code: set `ANTHROPIC_BASE_URL` and DeepSeek key, run small file-editing task. Check: tool calls work; stream-json same format.
- S7 Antigravity headless: check official docs for non-interactive command, JSON output, turn-limit flag. If none, record so Phase 5 designs an alternative.
- S8 Max turns: `claude -p "review git diff" --max-turns 1 --output-format json`. Check: stops at exactly 1 turn; result usable.

## Phase 0: acceptance and handling
- source: /mnt/data/IDE/QuangVy-Orchestration/files/01-phase0-kiem-chung.md (section 4 to 6)
- [ ] Cả 8 spike có kết quả Đạt / Không đạt / Một phần.
- [ ] Mỗi kết quả "Không đạt" có ghi phương án thay thế.
- [ ] Đã quyết định profile Work/Personal dùng tài khoản nào (sau khi xác nhận chính sách công ty).

Result template per spike: Ngày thử, Lệnh, Kết quả (Đạt / Không đạt / Một phần), Quan sát, Ảnh hưởng tới thiết kế.
Handling: run by hand to understand CLI behavior; agent may explain output but must not draw conclusions for the user.

## Phase goals and scope (Phases 1 to 5)
- source: files/02-phase1-giao-thuc-mvp.md, 03-phase2-loi-rust.md, 04-phase3-toi-uu-session.md, 05-phase4-dashboard.md, 06-phase5-da-doi-hinh-media.md (all under /mnt/data/IDE/QuangVy-Orchestration/)
- Phase 1 goal: chạy được quy trình Lead -> Dev -> QA trên dự án thật chỉ bằng script và file cấu hình; solves #5, #6 (env-var level), #11. In scope: `projects_map.json`, directory-scan script, 3 role prompts, `.plan.md`, per-role run scripts. Out of scope: dashboard, SQLite, session optimization, parallel tasks. Task checklist: scan script; hand-edit `name_vi`/aliases; accent-less name match + Fallback Ask; finalize 3 prompts; write `brain lead/dev/qa`; run 5 real tasks with log.
- Phase 2 goal: turn Phase 1 scripts into Rust binary `agent-brain` with safe process management, stream-json capture, SQLite token storage, zero-token fast-path; solves #1, #3 (log capture), #6, #9. In scope: CLI, process runner, stream-json parser, SQLite, fast-path levels 1 and 2, profile. Out of scope: web server, UI, SSE (Phase 4), session optimization (Phase 3). Rationale for CLI first: hardest part is process management and async stream reading; easier to test in terminal. Delivery: use Phase 1 protocol, each module is a task (Lead plans, Dev codes, QA reads diff); user accepts via test table. Task checklist: init Rust project; projects.rs; runner.rs; stream.rs; db.rs; fastpath.rs; profile.rs; move 3 prompts from Phase 1 to binary.
- Phase 3 goal: reduce tokens per task based on real SQLite data, not guesses; solves #7 (bloated context), #8 (tokens spent re-reading `.gsd`). Task checklist: report queries (avg tokens by role, by project); measure first-3-turn tokens per run; implement option A (`.gsd-summary.md` by hash); re-measure after 1 week; add session length tracking + `.handover.md`; decide on option B (record reason).
- Phase 4 goal: add web UI inside `agent-brain` binary: view tasks and logs in real time, assign tasks, kill, token stats; still one executable; solves #2, #3. In scope: Axum server, SSE, embedded HTML + Tailwind + Alpine.js, profile selection in UI. Out of scope: multi-user login, remote internet access. Task checklist: Axum localhost+token server; API endpoints reusing Phase 2 modules; broadcast channel stream.rs -> SSE; build 5 screens (one screen per agent task); embed static files; build offline CSS.
- Phase 5 goal: each role (Lead/Dev/QA) can run on a different engine, swappable between tasks when quota runs out; separate video production branch from code branch while keeping one coordination point; solves #4 (media loads Orca/Git worktree), #10 (dependence on one AI vendor). Task checklist: read `teams.json` and pick engine by role; detect quota errors and propose team change; token stats per engine; Antigravity approach per S7; media project directory and runner; connect whiteboard script pipeline output to build step.

## Acceptance test tables
- source: files/02..06 (see per-row IDs; all under /mnt/data/IDE/QuangVy-Orchestration/files/)
Phase 1 (02-phase1-giao-thuc-mvp.md section 6):
- P1-01 Call by accented name "web bán hàng" -> matches `web-ban-hang`.
- P1-02 Call by alias "shop" -> matches.
- P1-03 Ambiguous name matching 2 projects -> stop, list both, run no agent.
- P1-04 Name matching no project -> stop, list all, run no agent.
- P1-05 Clone new project then scan -> new project appears, old aliases kept.
- P1-06 Lead finished -> no code file modified (`git status` clean except `.plan.md`).
- P1-07 Dev with complete plan -> only files in plan modified.
- P1-08 Dev with incomplete plan -> writes Vướng mắc, no blind edits.
- P1-09 QA run -> `.qa-report.md` exists, no code edited.
- P1-10 Project profile `work` -> runs with work `CLAUDE_CONFIG_DIR`.

Phase 2 (03-phase2-loi-rust.md section 6):
- P2-01 `run lead` on real project -> log shows each step in terminal.
- P2-02 Kill while agent runs `sleep 300` -> after 5 s, `ps` shows no related `claude` or `sleep`.
- P2-03 Close terminal (Ctrl+C) midway -> no leftover processes.
- P2-04 Run ends -> `runs` row has tokens, session_id, times.
- P2-05 Bad JSON line in stream -> stored raw, program continues.
- P2-06 `status` -> git result, no AI call, 0 tokens.
- P2-07 `inspect` -> exactly 1 turn, no saved session.
- P2-08 `stats --profile work` -> only work-profile runs.
- P2-09 Copy binary to another machine/dir -> runs, no extra install (except agent CLI).

Phase 3 (04-phase3-toi-uu-session.md section 5):
- P3-01 Token report by role -> before/after optimization numbers.
- P3-02 Dev run with `.gsd-summary.md` -> starts editing within <= 2 turns.
- P3-03 Edit `.gsd` -> summary regenerated on next run.
- P3-04 Session over threshold -> `.handover.md` created, new session continues correct work.
- P3-05 New session from handover -> does not re-ask settled decisions.
- P3-06 Average tokens/task -> measurable reduction vs Phase 2.

Phase 4 (05-phase4-dashboard.md section 6):
- P4-01 Run binary -> dashboard opens, nothing extra to install.
- P4-02 Access from another LAN machine -> not reachable.
- P4-03 Access without token -> rejected.
- P4-04 Assign task from UI -> realtime log, pipeline steps change status.
- P4-05 Reload mid-run -> old log + continues receiving new log.
- P4-06 Click Kill -> run stops, no leftover processes (as P2-02).
- P4-07 Ambiguous project name in UI -> list shown to choose, not auto-run.
- P4-08 Change profile -> stats and new tasks follow the profile.
- P4-09 Internet off -> UI still styled correctly.
- P4-10 Compare UI with design (if Figma exists) -> matches user's UI checklist.

Phase 5 (06-phase5-da-doi-hinh-media.md section 5):
- P5-01 Task with team `tiet-kiem` -> Lead/QA on Claude, Dev on DeepSeek; log records correct engine.
- P5-02 Simulated quota exhaustion mid-task -> system reports quota error, proposes switch, no silent switch.
- P5-03 Change engine mid-task -> new engine continues from `.plan.md`, no restart.
- P5-04 Stats by engine -> tokens split correctly per engine.
- P5-05 Media project -> no Git worktree created, render files outside repo.
- P5-06 Kill render job -> FFmpeg stops, no leftover processes.
- P5-07 DeepSeek key -> absent from committed files and logs.

## Risk registers (per phase)
- source: files/02..06 (risk sections; all under /mnt/data/IDE/QuangVy-Orchestration/files/)
- Phase 1: agent ignores prohibitions -> restrict tools via `--allowedTools`/`--disallowedTools`; plan too generic -> add good plan example to Lead prompt; duplicate aliases -> scan warns.
- Phase 2: user cannot read Rust -> agent writes tests per module, small modules, review by behavior via test cases; stream-json format changes by CLI version -> store `raw_json`, soft parse; hard-to-understand async errors -> one module per agent task, test before next module.
- Phase 3: summary missing key info -> QA checks, extend template; handover loses context -> keep old session, do not delete; optimizing without re-measuring -> do not close phase before P3-06.
- Phase 4: exposure of command execution to network -> localhost + token (P4-02, P4-03); SSE disconnect -> browser auto-reconnect, back-fill from SQLite; UI bloat -> simple Alpine.js, one file per screen.
- Phase 5: cheaper engine writes worse code -> QA always on strong engine, track "Cần sửa" rate per engine; differing log formats -> soft parser, `raw_json`; key leak in logs -> filter sensitive env vars before logging.

## Phase 4 UI rationale
- source: /mnt/data/IDE/QuangVy-Orchestration/files/05-phase4-dashboard.md (section 3)
Embedding UI: no Node, venv or CORS config; UI and API same server and origin; UI reads data from Phase 2 core, no logic rewrite. Dashboard can run commands on the user's machine, hence minimal security is mandatory (see constraints.md).
