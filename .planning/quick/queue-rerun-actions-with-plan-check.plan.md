# Plan: chạy lại bước của task có sẵn khi dự án bận thì xếp hàng chờ slot, và kiểm tra plan trước khi Dev chạy từ hàng chờ

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Vấn đề (người dùng, 2026-11-12)
Chạy lại Lead của task 53 bị chặn vì dự án `automation-test-e2e` đang chạy task 55 (`max_parallel_tasks = 1`): `Dự án … đang chạy 1 task song song … — chờ một task xong`. Chỉ việc MỚI (màn Giao việc, xem plan `assign-busy-goes-to-queue`) có hàng chờ; thao tác chạy lại bước trên task có sẵn thì bị từ chối ngay, người dùng phải canh để bấm lại.

Người dùng chọn (câu hỏi 2026-11-12):
- Phạm vi: mọi thao tác chạy lại bước của task có sẵn khi dự án bận → ghi nhận "đang chờ slot" và tự chạy khi dự án rảnh.
- Khi tới lượt chạy Dev từ hàng chờ: kiểm tra plan còn đúng không (KHÔNG đụng git: không merge, không rebase). Lệch thì không chạy Dev mà đưa về Lead để lập lại plan.

## Hiện trạng (đã đọc code)
- `agent-brain/src/dashboard/api_tasks.rs`:
  - `run_task_action(st, task_id, action_id, role, rev, note)` (~dòng 3229) phục vụ `run_dev`, `run_qa`, `relead`: nạp `load_task_state`, tìm action trong `state.view.actions`; `!action.enabled` → `ControlOutcome::Refused { reason, kind }` với `kind = "busy"` khi `state.busy == Some(reason)`; ngược lại khởi chạy job (`start_job_outcome`).
  - `run_control_as(st, task_id, control, rev, note, source)` (~dòng 3557) phục vụ các control (`approve`, `return_lead`, `auto_start`, …); từ chối `busy` ở ~dòng 3600-3630.
  - Các route: `/api/tasks/{id}/dev`, `/qa`, `/relead` (~dòng 3329-3353).
- `agent-brain/src/dashboard/taskstate.rs`: danh sách action (`run_dev` ~237, `run_qa` ~289, `relead` ~333, …); `state.busy` là lý do bận của dự án.
- `agent-brain/src/orchestrator/scheduler.rs`: `schedule_tick` gọi `handle_env_stops` TRƯỚC khi kiểm `orch.enabled` (~dòng 42-50); `handle_env_stops` (~dòng 568) là tiền lệ "task đang chờ, tick sau tự chạy lại khi `busy_reason_ws` hết".
- `agent-brain/src/db.rs`: `SCHEMA_VERSION = 12`, mảng migration; test `agent-brain/tests/db_migration.rs`. Lần nâng schema trước đều sao lưu `brain.db` trước (`brain.db.bak-pre-N-…`).
- `agent-brain/src/planfile.rs`: `parse_plan`, `validate_plan(&plan, Some(root))` trả danh sách lỗi (`missing_section:…`, …), `unhandled_blocker_lines`.
- `agent-brain/assets/dashboard/js/task.js` (~dòng 2728-2900 xử lý action `run_dev`/`run_qa`/`relead`) và `screens/task.html`.

## Luật mới
1. Một hành động được xếp hàng khi: là một trong `run_dev`, `run_qa`, `relead`, `auto_start`, và nó đang bị từ chối với `kind = "busy"` (đúng lý do bận của dự án). Mọi lý do từ chối khác (không được phép, có run của chính task đó đang chạy, v.v.) KHÔNG xếp hàng.
2. Mỗi task có tối đa MỘT hàng chờ ở trạng thái `waiting`. Xếp hàng lần hai cho cùng task: thay bằng hành động mới (ghi đè), không nhân đôi.
3. Mỗi tick của scheduler (kể cả khi `orchestrator.enabled = false`, giống `handle_env_stops`) duyệt các hàng `waiting` theo `created_at` tăng dần; với mỗi hàng nạp lại trạng thái task và quyết định:
   - hành động không còn trong `state.view.actions` (task đã đổi trạng thái, đã bị xoá…) → `failed`, lý do `Task đã đổi trạng thái — không chạy nữa`;
   - hành động còn nhưng bị từ chối `busy` → giữ `waiting`;
   - hành động còn và chạy được → thực hiện bằng ĐÚNG hàm hiện có (`run_task_action` hoặc `run_control_as`, không `rev`), rồi `started` khi `Started`; từ chối khác `busy` → `failed` kèm lý do.
4. Kiểm tra plan (chỉ cho `run_dev`, trước khi gọi `run_task_action` từ hàng chờ): nếu plan không còn đúng thì KHÔNG chạy Dev; đổi hàng thành `relead` với `note = "Plan không còn khớp: <lý do>"` (chạy ở tick sau theo luật 3) và ghi lý do vào hàng. "Plan còn đúng" = tất cả các điều sau:
   a. file plan của task đọc được trong thư mục làm việc của task (`protocol_paths`, như `plan_facts`/`get_task` đang đọc) và `planfile::validate_plan(plan, Some(task_root))` không có lỗi;
   b. commit plan trên nhánh task (`gitops::plan_commit_on_branch`) tồn tại và, nếu task đã duyệt (`approved_commit` có giá trị), bằng `approved_commit`;
   c. không có file nào mà plan nêu trong phạm vi sửa đã bị sửa trên nhánh gốc kể từ `task.base_commit`: lấy `git diff --name-only <base_commit>..<nhánh gốc>` (chỉ đọc; dùng `gitops` hiện có, thêm hàm đọc nhỏ nếu thiếu) giao với danh sách đường dẫn plan nêu. Nếu `planfile` không cung cấp danh sách đường dẫn, bỏ bước (c) và ghi vào summary; KHÔNG tự đoán bằng regex trên chữ tự do.
   Lỗi khi chạy kiểm tra (git lỗi, đọc file lỗi) → coi là KHÔNG chắc: giữ `waiting`, ghi cảnh báo ra terminal, thử lại ở tick sau (không chạy Dev, không đẩy về Lead).
5. Huỷ: người dùng huỷ được hàng `waiting` (trạng thái `cancelled`).
6. Hiển thị: màn Task cho biết "Đang chờ slot — <hành động> (đặt lúc HH:MM)" kèm nút Huỷ; khi action bị `busy`, hiện nút phụ `Xếp hàng chờ slot` cạnh nút gốc. Hàng đổi sang `relead` hiện lý do lệch plan. Không thêm gì cho Telegram trong plan này.
7. Giao diện và API không được rò đường dẫn tuyệt đối (như các phản hồi khác).

## Thay đổi
| # | File | Hàm / chỗ | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/db.rs` | mảng migration, `SCHEMA_VERSION`, hàm mới | Migration số 13 (SCHEMA_VERSION = 13): `CREATE TABLE queued_actions (id INTEGER PRIMARY KEY, task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE, action_id TEXT NOT NULL CHECK (action_id IN ('run_dev','run_qa','relead','auto_start')), note TEXT, source TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'waiting' CHECK (status IN ('waiting','started','failed','cancelled')), reason TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);` và `CREATE UNIQUE INDEX queued_actions_one_waiting ON queued_actions(task_id) WHERE status = 'waiting';`. Hàm: `enqueue_action(conn, task_id, action_id, note, source) -> BrainResult<i64>` (thay hàng `waiting` cũ của task bằng hàng mới trong một giao dịch), `waiting_actions(conn)` (FIFO), `waiting_action_for_task(conn, task_id)`, `set_queued_status(conn, id, status, reason)`, `retarget_queued(conn, id, action_id, note, reason)`. Theo đúng khuôn các migration trước (`user_version`, trong giao dịch). | Lưu hàng chờ |
| 2 | `agent-brain/tests/db_migration.rs` | test | Cập nhật các test đang khẳng định phiên bản 12 → 13; thêm test: DB phiên bản 12 nâng lên 13 giữ nguyên dữ liệu cũ, có bảng và chỉ mục mới; chỉ mục duy nhất từ chối hai hàng `waiting` cùng task; `CHECK` từ chối `action_id` lạ. | Chốt migration |
| 3 | `agent-brain/src/dashboard/api_tasks.rs` | hàm mới `queue_action`, `cancel_queued_action`; route; `TaskState`/JSON của `get_task` | `queue_action(st, task_id, action_id: &str, note: Option<&str>, source) -> BrainResult<QueueOutcome>`: nạp state, kiểm luật 1 (action nằm trong danh sách cho phép, đang `!enabled` và `kind` sẽ là `busy` theo đúng phép so `state.busy == Some(reason)` mà `run_task_action` dùng; tách phép so đó thành hàm nhỏ dùng chung, không nhân bản), rồi `db::enqueue_action`; nếu action KHÔNG bị bận thì trả `NotQueueable("Hành động chạy được ngay — bấm chạy")`. Route `POST /api/tasks/{id}/queue-action` body `{ "action": "..." , "note": "..."? }` → 201 `{queued_id, action, status: "waiting"}`; 409 với lý do cho các từ chối; `POST /api/tasks/{id}/queue-action/cancel` → 200 (idempotent) hoặc 404. Thêm vào JSON của `GET /api/tasks/{id}` trường `queued_action: null | {id, action, note, reason, created_at}` (chỉ hàng `waiting`). Đọc `note` qua `normalize_note`. | API |
| 4 | `agent-brain/src/orchestrator/scheduler.rs` | hàm mới `handle_queued_actions(st, conn)`, `schedule_tick` | Gọi `handle_queued_actions` ngay cạnh `handle_env_stops`, TRƯỚC khi kiểm `orch.enabled`. Thực hiện luật 3 và 4. Hàm kiểm tra plan `plan_check(st, state) -> PlanCheck { Ok, Mismatch(String), Unsure(String) }` đặt trong `scheduler.rs` hoặc module nhỏ mới `orchestrator/plancheck.rs` (thêm `pub mod` trong `orchestrator/mod.rs`). Mỗi tick chỉ bắt đầu TỐI ĐA một hành động cho mỗi dự án (để hai hàng cùng dự án không cùng lọt slot). Phát `st.hints.send(0)` khi trạng thái hàng đổi để màn Task cập nhật. Ghi `crate::term::emit("Task N: … chạy từ hàng chờ")` cho mỗi lần bắt đầu/hỏng. | Chạy khi hết bận, kiểm plan |
| 5 | `agent-brain/src/gitops.rs` | hàm đọc mới nếu thiếu | `changed_names_between(root, from: &str, to: &str) -> BrainResult<BTreeSet<String>>` bằng `git diff --name-only <from>..<to> --` (kiểm `is_hex_sha`/tên nhánh như các hàm lân cận, từ chối chuỗi bắt đầu bằng `-`). Chỉ đọc. | Bước (c) của kiểm tra plan |
| 6 | `agent-brain/assets/dashboard/js/task.js`, `agent-brain/assets/dashboard/screens/task.html` | action `run_dev`/`run_qa`/`relead`/`auto_start` (~dòng 2728-2900 của task.js) | Khi một action bị khoá với lý do bận (cùng điều kiện giao diện đang dùng để hiện lý do), hiện thêm nút `Xếp hàng chờ slot` gọi `POST /api/tasks/{id}/queue-action`; sau khi xếp hàng hiện dải `Đang chờ slot — <tên hành động> (đặt lúc HH:MM)` với nút `Huỷ` gọi `.../cancel`; đọc `queued_action` từ `GET /api/tasks/{id}`. Dùng lớp Tailwind đã có trong `app.css` (đã build sẵn; KHÔNG chạy tailwind, KHÔNG sửa `app.css`); CSP: không inline script/style, theo cấu trúc Alpine CSP hiện có. Khi `queued_action.note` bắt đầu bằng `Plan không còn khớp`, hiện lý do trong dải. | Giao diện |
| 7 | `agent-brain/tests/queued_actions.rs` (mới) | cả file | Dùng helper của `tests/common` và mẫu của `tests/orchestrator_scheduler.rs`/`tests/dashboard_run.rs` (thư mục tạm, DB tạm, tiến trình giả; không bao giờ DB thật). Ca kiểm: (a) task có action `relead` bị `busy` → `queue_action` ghi hàng `waiting`; action chạy được ngay → `NotQueueable`; (b) xếp hàng hai lần cho một task → một hàng `waiting`, hàng sau thay hàng trước; (c) `handle_queued_actions` khi dự án còn bận → giữ `waiting`; (d) khi hết bận → gọi đúng hàm hiện có, hàng `started`, job được tạo (kiểm bằng cách task có run/job mới); (e) hai hàng cùng dự án, `max_parallel_tasks = 1` → mỗi tick chỉ bắt đầu một; (f) task đã đổi trạng thái (action biến mất) → `failed` với lý do; (g) `run_dev` từ hàng chờ với plan hợp lệ → chạy Dev; (h) `run_dev` với plan lệch (file plan thiếu mục, hoặc commit plan ≠ `approved_commit`, hoặc file plan nêu đã đổi trên nhánh gốc nếu có danh sách đường dẫn) → KHÔNG chạy Dev, hàng thành `relead` với `Plan không còn khớp: …`, tick sau chạy `relead`; (i) lỗi khi kiểm tra (git lỗi) → giữ `waiting`, không chạy gì; (j) huỷ → `cancelled`, tick sau không chạy; (k) chạy khi `orchestrator.enabled = false`; (l) API `queue-action`/`cancel` trả đúng mã và `queued_action` hiện trong `GET /api/tasks/{id}`; (m) phản hồi API không chứa đường dẫn tuyệt đối; (n) dự án có autopilot driver của task khác còn sống nhưng KHÔNG có run nào `running` (khoảng giữa hai bước, đã gặp thật khi task 53 chuyển từ Dev sang QA) → hàng chờ vẫn `waiting`, không bắt đầu (dùng `busy_reason_ws`, vốn đã tính driver; ca này chặn việc chỉ kiểm bảng `runs`). | Chốt hành vi |
| 8 | `agent-brain/tests/dashboard_assets.rs` | thêm test | Kiểm chuỗi trong asset (`task.js` chứa `queue-action`, `queued_action`; `task.html` chứa dải chờ), như cách plan `assign-busy-goes-to-queue` làm (dự án không có khung test JS). | Khoá giao diện |
| 9 | `agent-brain/README.md` | mục task / hàng chờ / nâng cấp | Mô tả: chạy lại bước khi dự án bận thì xếp hàng chờ slot; kiểm tra plan trước khi Dev chạy từ hàng chờ và hậu quả (đưa về Lead); thêm mục `Nâng cấp lên schema 13` theo khuôn các mục nâng cấp trước: SAO LƯU `brain.db` trước khi chạy serve bản mới, và KHÔNG thay binary khi còn task đang chạy. | Tài liệu |

## Không được đụng tới
- Logic autopilot (`autopilot/*`), `claim.rs::worktree_busy_reason` và các kiểm tra bận (chỉ ĐỌC / dùng lại), quota, Telegram (`telegram/*`), phần Giao việc của `assign.js`/`assign.html` (plan khác lo).
- Luồng `run_task_action`/`run_control_as` hiện có: chỉ tái dùng, không đổi hành vi; chỉ được tách hàm so `busy` nhỏ như mục 3 mà không đổi kết quả.
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*` (brain.db thật và worktree thật; serve đang chạy: executor KHÔNG chạy migration lên DB thật, KHÔNG sao lưu/ghi vào đó), `/mnt/data/source_codes/**`, `~/.claude-orca`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/` của việc khác.
- `assets/dashboard/app.css`, `vendor/`, bảng `ASSETS` trong `assets.rs` (không thêm file asset mới).
- KHÔNG thay binary `agent-brain` đang chạy. `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --test db_migration 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test queued_actions 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test dashboard_assets --test assets_parity --test orchestrator_scheduler --test dashboard_run 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(db|dashboard/api_tasks|orchestrator/(scheduler|plancheck)|gitops)\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -2
```
Test flaky đã biết (không phải của việc này; chạy lại riêng và ghi vào summary): `running_run_adds_kill_button` trong `tests/telegram_actions.rs`, `export_error_is_recorded_once_and_backs_off` trong `tests/journal_safety.rs`.

Acceptance:
1. Chạy lại Lead/Dev/QA hoặc `auto_start` của task có sẵn khi dự án bận: có nút `Xếp hàng chờ slot`; xếp hàng ghi nhận, task hiện "Đang chờ slot", việc tự chạy ở tick sau khi dự án hết bận, kể cả khi `orchestrator.enabled = false`.
2. Dev chạy từ hàng chờ chỉ khi plan còn đúng; lệch thì Lead được chạy lại với ghi chú lý do, Dev không chạy.
3. Lỗi khi kiểm tra plan không làm chạy Dev và không đẩy task về Lead.
4. Lý do từ chối khác `busy` vẫn từ chối như cũ, không xếp hàng.
5. Mỗi dự án chỉ có thêm một hành động bắt đầu mỗi tick; huỷ hoạt động.
6. DB phiên bản 12 nâng lên 13 mà không mất dữ liệu (test); mọi test cũ pass; chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Bốn commit: `feat(agent-brain): queued_actions table and migration 13` (mục 1, 2), `feat(agent-brain): queue reruns of existing tasks while the project is busy, with a plan check before Dev` (mục 3, 4, 5, 7), `feat(dashboard): queue-for-slot button and waiting banner on the task screen` (mục 6, 8), và `docs: queued reruns and schema 13 upgrade note` (mục 9). Mỗi commit kèm dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan (nhất là bước (c) có chạy được hay bỏ, vị trí `plan_check`, hàm so `busy` được tách), kết quả từng lệnh verify (số pass/fail), phần chưa kiểm (giao diện thật chưa mở trong trình duyệt, migration chưa chạy trên DB thật, serve chưa restart).
