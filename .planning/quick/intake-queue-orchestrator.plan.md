# Plan: hàng chờ việc (intake) và agent điều phối

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm. Làm TUẦN TỰ theo giai đoạn G1 → G4. Mỗi giai đoạn: làm xong, chạy verify, commit, rồi mới sang giai đoạn sau. Nếu verify của một giai đoạn không đạt thì DỪNG, báo cáo, không làm giai đoạn sau.

## Mục tiêu (đã chốt với người dùng)
1. Việc được giao qua UI (Giao việc) hoặc Telegram vào **hàng chờ** (`intake`).
2. **Agent điều phối** chạy liên tục trong `agent-brain serve`, quét hàng chờ, phân tích yêu cầu, và tạo các **đề xuất** (`proposals`): mỗi đề xuất gồm dự án, tiêu đề, yêu cầu, mức ưu tiên, đội hình và quy trình đề xuất (chọn theo loại dự án và profile).
3. Người dùng **duyệt đề xuất** (việc + đội hình). Sau khi duyệt, việc vào **hàng xử lý** (`approved`), và từ đó chạy theo luồng hiện có: Lead lập plan → người dùng duyệt plan (`approve`, như hiện tại) → Dev/QA.
4. **Ưu tiên** 3 mức: `urgent` (gấp) > `normal` (thường) > `low` (thấp). Gấp chỉ xếp trước trong hàng chờ; KHÔNG dừng việc đang chạy.
5. **Tối đa 3 task chạy song song** (`max_parallel = 3`, cấu hình được, 1–10).
6. **Hết quota:** task dừng tại bước của engine vừa hết quota (hành vi có sẵn: outcome `quota`, autopilot dừng `het_quota`). KHÔNG tự đổi engine. Báo người dùng qua Telegram. Chỉ tự chạy lại khi bật `auto_resume_quota` và đã qua `resume_after_minutes`. Mặc định TẮT. Các task khác không bị chặn.

## Bối cảnh đã kiểm tra trong code
- Migration: `agent-brain/src/db.rs`, mảng `MIGRATIONS` (hiện 4 phần tử, hằng `SCHEMA_VERSION = 4` ở dòng ~62). Thêm migration thứ 5 vào CUỐI mảng, và cập nhật `SCHEMA_VERSION` thành 5. Migration được chạy trong một transaction, theo `PRAGMA user_version`.
- Tạo task: `start_new_task(st, &StartRequest)` trong `agent-brain/src/dashboard/api_tasks.rs` (dòng ~625). Hàm này đã làm: kiểm tra yêu cầu, dự án, quy trình, đội hình, tiền kiểm (`lead_preflight`), tạo nhánh `task/<slug>`, và khởi chạy Lead qua `JobManager`. Đây là ĐIỂM DUY NHẤT được dùng để tạo task; không viết lại logic này.
- Gọi `start_new_task` hiện có ở: `api_tasks.rs:829` (POST /api/tasks), `telegram/intake.rs:666, 1232, 1241` (Telegram).
- Duyệt plan: `post_approve` trong `api_tasks.rs` (dòng ~2785); CLI `cmd_approve` trong `autopilot/mod.rs` (dòng ~772). Giữ nguyên.
- Trình quản lý job: `agent-brain/src/dashboard/jobs.rs` (`JobManager::start`, `list`, `live_pids`).
- Trạng thái đang chạy: bảng `runs` có `status = 'running'` (`db::running_runs`, `db::running_run_for_task`); autopilot có `tasks.auto_state` (`db::set_auto_state`), `auto_stop_kind`, `auto_stop_at`.
- Cấu hình: `agent-brain/src/config.rs`, struct `Config` (dòng ~894). Có `autopilot: AutopilotDefaults`, `telegram_max_tasks_per_day`, `engines`. Thêm khóa mới theo đúng kiểu các khóa hiện có, có kiểm tra khoảng giá trị.
- Ràng buộc role trong `runs` (CHECK): `lead, dev, qa, inspect, plan-review, code-review`. Triage KHÔNG thêm role mới vào `runs` (tránh dựng lại bảng). Xem G2.
- Dữ liệu từ người dùng (yêu cầu UI, tin Telegram) là DỮ LIỆU KHÔNG TIN CẬY. Khi đưa vào prompt cho agent điều phối, đặt trong khối có ranh giới rõ, không coi là chỉ thị (như cách Telegram làm hiện nay).

## G1 — Mô hình dữ liệu và API (chưa đổi hành vi)

### Thay đổi
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/db.rs` | Thêm migration thứ 5 vào CUỐI `MIGRATIONS`: |
|   |   | `CREATE TABLE intake (id INTEGER PRIMARY KEY, source TEXT NOT NULL CHECK (source IN ('ui','telegram')), project_id INTEGER REFERENCES projects(id), request TEXT NOT NULL, priority TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('urgent','normal','low')), status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued','triaging','proposed','cancelled','failed')), error TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL); CREATE INDEX intake_status ON intake(status);` |
|   |   | `CREATE TABLE proposals (id INTEGER PRIMARY KEY, intake_id INTEGER NOT NULL REFERENCES intake(id) ON DELETE CASCADE, project_id INTEGER NOT NULL REFERENCES projects(id), title TEXT NOT NULL, request TEXT NOT NULL, priority TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('urgent','normal','low')), team TEXT NOT NULL, workflow TEXT NOT NULL, reason TEXT, status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','started','failed')), task_id INTEGER REFERENCES tasks(id), error TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL); CREATE INDEX proposals_status ON proposals(status);` |
|   |   | Thêm bảng `quota_stops`: `task_id INTEGER PRIMARY KEY REFERENCES tasks(id), stopped_at INTEGER NOT NULL, engine TEXT NOT NULL, notified INTEGER NOT NULL DEFAULT 0`. |
| 2 | `agent-brain/src/db.rs` | `SCHEMA_VERSION` = 5. Hàm mới: `insert_intake`, `list_intake(status Option)`, `set_intake_status`, `insert_proposal`, `list_proposals(status Option)`, `set_proposal_status`, `set_proposal_priority`, `next_approved_proposal` (sắp theo `urgent`→`normal`→`low`, rồi `created_at` tăng dần, `LIMIT 1`), `count_active_tasks` (đếm `COUNT(DISTINCT task_id)` từ `runs` có `status='running'` và `task_id IS NOT NULL`, cộng số task có `auto_state='running'`, bỏ trùng). Mọi hàm nhận `&Connection`, dùng tham số bind, không ghép SQL. |
| 3 | `agent-brain/src/config.rs` | Thêm khối `orchestrator` vào `Config`: `enabled: bool` (mặc định `false`), `max_parallel: u32` (mặc định 3, khoảng 1–10), `auto_resume_quota: bool` (mặc định `false`), `resume_after_minutes: u32` (mặc định 60, khoảng 1–1440). Giá trị ngoài khoảng → lỗi cấu hình, theo đúng cách các khóa `autopilot` đang làm. Không đổi file `config.json` của người dùng; chỉ thêm giá trị mặc định trong code. |
| 4 | `agent-brain/src/dashboard/api_intake.rs` (MỚI) | `routes()` trả về các route: `GET /api/intake?status=`, `POST /api/intake` (body `{request, project?, priority?}`, source=`ui`, trả 201 `{id}`), `POST /api/intake/{id}/cancel`, `GET /api/proposals?status=`, `POST /api/proposals/{id}/priority` (body `{priority}`), `POST /api/proposals/{id}/approve`, `POST /api/proposals/{id}/reject` (body `{reason?}`). Chỉ cho phép đổi priority/approve/reject khi proposal đang `pending` (approve/priority) hoặc `approved` (priority). Giới hạn độ dài `request` (dùng hằng `MAX_REQUEST_CHARS` có sẵn trong `api_tasks.rs`). |
| 5 | `agent-brain/src/dashboard/mod.rs` | Đăng ký `api_intake::routes()` cùng các route khác (theo cách các module khác đang đăng ký). |
| 6 | `agent-brain/src/dashboard/api_intake.rs` | `approve`: đặt `proposals.status = 'approved'` và KHÔNG gọi `start_new_task` trực tiếp (việc chạy do bộ lập lịch ở G3). |

### Acceptance G1
1. DB mới (v5) tạo đủ `intake`, `proposals`, `quota_stops`. DB cũ (v4) nâng lên v5 không mất dữ liệu.
2. `POST /api/intake` tạo dòng `queued`; `GET` trả đúng; `cancel` chuyển `cancelled`.
3. `approve` chỉ đổi trạng thái sang `approved`, không tạo task, không tạo nhánh git.
4. Không có thay đổi hành vi khi `orchestrator.enabled = false` (mọi luồng cũ giữ nguyên).

### Verify G1
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --test db_migration --test dashboard_actions --test dashboard_http 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked"
```
Thêm test mới trong `agent-brain/tests/intake_api.rs`: tạo intake, đọc danh sách, đổi priority, approve, cancel, và kiểm tra `approve` không tạo dòng trong `tasks`. Thêm test migration v4→v5 vào `tests/db_migration.rs`.

Commit G1: `feat(agent-brain): intake and proposals schema and API`.

## G2 — Agent điều phối: phân tích hàng chờ thành đề xuất

### Thay đổi
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/orchestrator/mod.rs` (MỚI) | Hàm `tick(st: &AppState)`: nếu `orchestrator.enabled` thì lấy MỘT intake `queued` cũ nhất (ưu tiên `urgent` trước), đặt `triaging`, gọi `triage()`, rồi đặt `proposed` hoặc `failed` (kèm `error`). |
| 2 | `agent-brain/src/orchestrator/triage.rs` (MỚI) | `triage(st, intake) -> Result<Vec<ProposalDraft>>`. Dựng prompt gồm: danh sách dự án (id, tên, profile), danh sách đội hình (`teams::load()`), danh sách quy trình (`workflows::load()`), và yêu cầu trong khối có ranh giới rõ: `<yeu_cau_nguoi_dung>…</yeu_cau_nguoi_dung>`, ghi rõ đây là dữ liệu. Yêu cầu agent trả JSON duy nhất: `{"tasks":[{"project","title","request","priority","team","workflow","reason"}]}`, tối đa 5 mục. Gọi engine bằng cùng cơ chế engine mà Lead đang dùng (tìm trong `agent-brain/src/roles/mod.rs` và `agent-brain/src/engine.rs`; KHÔNG thêm role mới vào `runs`). Nếu không tìm được cách gọi engine không ghi `runs`, DỪNG và báo cáo. |
| 3 | `agent-brain/src/orchestrator/triage.rs` | `validate(draft)`: `project` phải là dự án có trong `projects_map.json` và không `missing`; `team` phải có trong `teams::load()`; `workflow` phải có trong `workflows::load()`; `priority` thuộc 3 giá trị; `title` và `request` không rỗng, không quá độ dài; team dùng engine bị khóa profile thì phải khớp profile dự án (dùng `teams::team_profile_refusal`). Mục không hợp lệ bị bỏ, ghi `reason` vào intake `error`. Nếu không mục nào hợp lệ thì intake `failed`. |
| 4 | `agent-brain/src/orchestrator/mod.rs` | Vòng lặp nền: `spawn_loop(st)` gọi `tick` mỗi 5 giây; khởi động từ `dashboard::cmd_serve` (cùng chỗ với các tác vụ nền khác). Lỗi trong `tick` chỉ ghi log, không làm chết server. |
| 5 | `agent-brain/src/dashboard/mod.rs` | Gọi `orchestrator::spawn_loop` khi server khởi động. |

### Quy tắc bắt buộc
- Agent điều phối KHÔNG được gọi `start_new_task`, KHÔNG tạo nhánh, KHÔNG chạy Lead. Chỉ tạo `proposals`.
- Không có đề xuất nào tự được duyệt.
- Output của engine phải được parse và validate như trên; không tin tưởng JSON thô.

### Acceptance G2
1. Intake `queued` → (khi bật `enabled`) thành `proposed` với các proposal `pending` đã validate.
2. Output sai định dạng hoặc đội hình không tồn tại → intake `failed`, không có proposal.
3. Không có dòng nào mới trong `tasks` hay `runs` sau khi triage.
4. `enabled = false` → vòng lặp không làm gì.

### Verify G2
Test với engine GIẢ (stub trait hoặc đường dẫn binary giả qua biến cấu hình test, theo cách `tests/` đang làm với engine khác). Thêm `agent-brain/tests/orchestrator_triage.rs`: các trường hợp (1)–(4) ở trên.
```bash
cargo test --offline --locked --test orchestrator_triage 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked"
```
Commit G2: `feat(agent-brain): orchestrator triage turns intake into proposals`.

## G3 — Bộ lập lịch: hàng xử lý, 3 song song, ưu tiên, hết quota

### Thay đổi
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/orchestrator/scheduler.rs` (MỚI) | `schedule_tick(st)`: nếu `enabled`: lấy `count_active_tasks`; trong khi `active < max_parallel` và còn proposal `approved`: lấy `next_approved_proposal`, gọi `start_new_task` với `StartRequest` (project, request, name = title slug, workflow, team), rồi đặt proposal `started` + `task_id` nếu thành công, hoặc `failed` + `error` nếu bị từ chối. |
| 2 | `agent-brain/src/orchestrator/scheduler.rs` | Quota: quét task có `auto_stop_kind = het_quota` (hoặc outcome `quota` của run cuối). Khi phát hiện lần đầu, ghi `quota_stops` và gửi thông báo Telegram một lần (dùng API outbox có sẵn; tìm trong `agent-brain/src/telegram/outbox.rs` và `telegram/status.rs`; KHÔNG tạo kênh gửi mới). Nếu `auto_resume_quota = true` và đã qua `resume_after_minutes` kể từ `stopped_at`, gọi đường chạy tiếp autopilot hiện có (`auto TASK_ID` qua `JobManager`, cùng cách `api_tasks` làm khi người dùng bấm chạy tiếp). Không bao giờ tự đổi engine. |
| 3 | `agent-brain/src/orchestrator/scheduler.rs` | Một task đang dừng vì quota KHÔNG được tính vào `count_active_tasks`, nên slot được giải phóng cho việc khác. |
| 4 | `agent-brain/src/orchestrator/mod.rs` | `spawn_loop` gọi thêm `schedule_tick` sau `tick`. |

### Quy tắc bắt buộc
- Không dừng hay đẩy lùi job đang chạy khi có việc `urgent`.
- Không chạy quá `max_parallel` task.
- Không tự chạy lại task hết quota khi `auto_resume_quota = false`.
- Nếu `start_new_task` trả về `PreflightRefused` (ví dụ engine bị khóa, đội hình sai), proposal chuyển `failed` với lý do, KHÔNG thử lại vô hạn.

### Acceptance G3
1. Với 5 proposal `approved` (2 `urgent`, 3 `normal`): tối đa 3 được bắt đầu cùng lúc; việc `urgent` được bắt đầu trước.
2. Khi một task xong, việc tiếp theo trong hàng được bắt đầu.
3. Task hết quota: dừng, không chặn các task khác, Telegram nhận đúng 1 thông báo.
4. `auto_resume_quota = true` và đã qua thời gian: task được chạy tiếp đúng một lần; không lặp.
5. `enabled = false`: không có task nào được bắt đầu bởi bộ lập lịch.

### Verify G3
Test dùng `JobManager` giả hoặc đếm lần gọi `start_new_task` qua hook test. Thêm `agent-brain/tests/orchestrator_scheduler.rs` cho các acceptance 1–5.
```bash
cargo test --offline --locked --test orchestrator_scheduler --test runner_kill --test autopilot_driver 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked"
```
Commit G3: `feat(agent-brain): scheduler runs approved proposals with priority, 3 slots and quota pause`.

## G4 — Đưa intake vào đường vào (UI và Telegram), khi bật orchestrator

### Thay đổi
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/telegram/intake.rs` | Tại ba chỗ gọi `start_new_task` (dòng ~666, 1232, 1241): nếu `orchestrator.enabled` thì thay bằng tạo dòng `intake` với `source='telegram'` (giữ nguyên nội dung chuyển tiếp, người gửi, thời gian trong `request`), trả về thông báo "đã vào hàng chờ #id". Khi `enabled = false`, giữ nguyên hành vi cũ. |
| 2 | `agent-brain/src/dashboard/api_tasks.rs` | POST /api/tasks: khi `enabled`, trả 409 với thông báo hướng dẫn dùng Hàng chờ, hoặc chuyển tiếp vào intake (chọn một và ghi rõ trong báo cáo; mặc định: chuyển vào intake). |
| 3 | `agent-brain/assets/dashboard/js/assign.js` và `screens/assign.html` | Khi `enabled`: nút Giao việc gọi `POST /api/intake`. Trạng thái bật/tắt lấy từ một endpoint nhỏ `GET /api/orchestrator/status` (thêm vào `api_intake.rs`). |

### Acceptance G4
1. `enabled = true`: một tin `/task` trên Telegram và một lần Giao việc trên UI đều thành intake `queued`, không tạo task trực tiếp.
2. `enabled = false`: mọi luồng cũ không đổi (test hiện có vẫn pass).

### Verify G4
```bash
cargo test --offline --locked --test telegram_intake --test dashboard_actions --test cli_dev 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked"
```
Commit G4: `feat(agent-brain): route Telegram and UI assignments through intake when orchestrator is on`.

## Ngoài phạm vi plan này (làm riêng sau)
- Giao diện hàng chờ 3D bằng three.js (đã cài skill ở `.claude/skills/`). Plan riêng, sau khi G3 xong.
- Nút duyệt đề xuất trong Telegram (dùng khung callback hiện có). Plan riêng.

## Không được đụng tới
- Các file có thay đổi chưa commit của người dùng: `agent-brain/assets/dashboard/js/settings.js`, `agent-brain/assets/dashboard/screens/settings.html`, `agent-brain/assets/prompts/lead.md`, `agent-brain/src/config.rs` (CHỈ thêm khối `orchestrator`, không sửa phần khác có thay đổi chưa commit — nếu phần đó xung đột thì DỪNG và báo cáo), `agent-brain/src/dashboard/api_settings.rs`, `agent-brain/src/engine.rs`, `agent-brain/src/skills.rs`, `agent-brain/tests/dashboard_settings.rs`.
- `.claude/skills/`, `.claude/worktrees/`, `.planning/STATE.md`, `.planning/ROADMAP.md`, mọi `.planning/` khác, `~/.config/agent-brain/*`, `~/.claude-orca`.
- `git add -A`, `--no-verify`: cấm. Stage theo đường dẫn. Không push.
- Không đổi hành vi khi `orchestrator.enabled = false`.

## Quy trình
- Mỗi giai đoạn một commit, kèm dòng `Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>`.
- Sau mỗi giai đoạn: ghi kết quả verify nguyên văn (số test pass/fail).
- Báo cáo cuối: file đã sửa theo giai đoạn, hash commit, kết quả verify, chỗ lệch plan, phần chưa kiểm tra (đặc biệt: gửi Telegram thật, engine thật, hết quota thật).

## Quyết định đã ghi nhận (từ người dùng)
- Ưu tiên chỉ xếp hàng, không dừng việc đang chạy.
- Hết quota: dừng tại bước đó, báo Telegram, tự chạy lại chỉ khi bật cờ; không tự đổi engine; không chặn task khác.
- 3 task song song.
- Duyệt hai lần: duyệt đề xuất (việc + đội hình) rồi duyệt plan như hiện tại.
- Agent điều phối chạy liên tục trong `serve`.
- Đội hình tự chọn theo loại dự án và profile.
