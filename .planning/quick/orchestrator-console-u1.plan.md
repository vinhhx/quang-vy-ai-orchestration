# Plan U1: màn "Điều phối" (một màn hình duy nhất)

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Mục tiêu
Một màn duy nhất `#/queue` (nhãn menu "Điều phối") cho thấy **mọi thứ** cùng lúc: việc trong hàng chờ, việc cần người dùng duyệt (đề xuất và plan), việc chờ slot, việc đang chạy, việc bị dừng. Người dùng duyệt / từ chối / đổi ưu tiên / sửa đội hình và quy trình ngay trên màn này, và giao việc mới kèm mức ưu tiên. Không tách nhiều màn. U2 (sơ đồ 3D three.js) sẽ được đặt vào đầu màn này sau, nên chừa sẵn vùng `<section data-queue-visual>` trống ở trên các cột (không vẽ gì trong U1).

## Bối cảnh (đã kiểm tra trong code)
- API có sẵn: `src/dashboard/api_intake.rs`
  - `GET/POST /api/intake`, `POST /api/intake/{id}/cancel`
  - `GET /api/proposals?status=`, `POST /api/proposals/{id}/priority|approve|reject`
  - `GET /api/orchestrator/status`
- Bảng `intake.status`: `queued|triaging|proposed|cancelled|failed`. Bảng `proposals.status`: `pending|approved|rejected|started|failed`. Bảng `quota_stops(task_id, stopped_at, engine, notified)`. Schema ở `src/db.rs` (migration 5). Helper: `list_intake`, `list_proposals`, `get_proposal`, `set_proposal_status`, `set_proposal_priority`, `list_quota_stops`, `project_row_id`.
- `ProposalRow` chỉ có `project_id` (số), không có tên dự án.
- `cancel_intake` hiện huỷ intake ở **mọi** trạng thái (kể cả `proposed`). Đây là lỗi, sửa trong plan này.
- `approve` hiện chỉ đổi status sang `approved`, không cho sửa đội hình / quy trình.
- `src/orchestrator/triage.rs`: `pub fn validate(draft: &ProposalDraft, projects, teams, workflows, engines) -> Result<(), String>`. `ProposalDraft {project (slug), title, request, priority, team, workflow, reason}`. Xem `triage_with_warnings` để biết cách nạp `projects` / `teams` / `workflows` / `engines` từ `AppState`.
- `src/orchestrator/scheduler.rs`: `fn active_task_count(st, conn) -> BrainResult<u32>` (đang private).
- `GET /api/tasks?limit=100` (`src/dashboard/api_tasks.rs`, `list_tasks`) trả mỗi task:
  - `id`, `project`, `project_name`, `title`, `status`, `banner`, `group` (`running|waiting|attention|done`), `current_step`, `steps`, `auto_state`, `stop_reason`, `quota_detail`, `human_mode`, `updated_at`, …
  - Cách tính `group`: `src/dashboard/taskstate.rs` khoảng dòng 822.
- Dashboard: SPA hash, Alpine CSP, Tailwind v4 dựng sẵn.
  - Router: `assets/dashboard/js/app.js` (`screenTitles`, `parseHash`, khoảng dòng 462–503). Menu: `assets/dashboard/index.html` (link `#/board` ở dòng 19).
  - Asset nhúng: `src/dashboard/assets.rs` (bảng `ASSETS`, `include_bytes!`).
  - Test asset: `tests/dashboard_assets.rs`, gồm `assets_table_matches_tree` và danh sách màn trong `asset_references_resolve`.
  - CSS: `app.css` dựng sẵn. Sau khi thêm class mới phải chạy `bash scripts/build-css.sh` ở gốc repo.
  - Màn tham khảo để bám kiểu code: `js/board.js` + `screens/board.html`, `js/overview.js`, `js/assign.js`. Dùng `window.AB.api`, `window.AB.errorText`, `window.AB.profileQuery`, và timer đăng ký qua `window.AB._timers` (router tự xoá khi rời màn). Xem cách các màn khác đăng ký polling và làm y như vậy.
- Duyệt **plan** của task vẫn nằm ở màn task (`#/task/{id}`, endpoint `POST /api/tasks/{id}/approve` cần `rev`). U1 KHÔNG đưa nút duyệt plan sang màn mới, chỉ hiện thẻ và link "Xem và duyệt plan" tới `#/task/{id}`.

## Phần A: Backend (Rust)

### A1. `GET /api/orchestrator/board` (mới, trong `api_intake.rs`)
Trả một lần đủ dữ liệu hàng chờ (task lấy riêng từ `/api/tasks`):
```json
{
  "status": {"enabled": true, "max_parallel": 3, "active": 2, "auto_resume_quota": false, "resume_after_minutes": 60},
  "intake":    [ {IntakeRow + "project": "slug"|null, "project_name": "..."|null} ],
  "proposals": [ {ProposalRow + "project": "slug", "project_name": "..."} ],
  "quota_stops": [ {"task_id": 5, "stopped_at": 1730000000, "engine": "claude", "resume_at": 1730003600|null} ],
  "recent":    [ đề xuất rejected/failed và intake failed/cancelled trong 24 giờ, tối đa 20, mới nhất trước ]
}
```
- `intake`: chỉ các intake có trạng thái `queued`, `triaging` và `failed` (failed chỉ lấy trong 24 giờ gần nhất). Sắp xếp `queued`/`triaging` theo thứ tự ưu tiên rồi đến `created_at`.
- `proposals`: chỉ `pending` và `approved`. Sắp theo ưu tiên (urgent > normal > low), sau đó `created_at` tăng dần. Đây chính là thứ tự bộ lập lịch dùng (xem `next_approved_proposal` trong db.rs) và phải khớp với nó.
- `project` / `project_name`: lấy từ bảng `projects` của brain.db theo `project_id`, đọc tên cột thật trong db.rs. Nếu không có cột tên tiếng Việt thì để `project_name` bằng slug.
- `active`: dùng `scheduler::active_task_count`. Đổi hàm đó thành `pub(crate)` và không đổi logic.
- `resume_at`: bằng `stopped_at + resume_after_minutes*60` nếu `auto_resume_quota` bật, ngược lại là `null`. Chỉ trả quota_stops của task có proposal ở trạng thái `started`, giống bộ lọc trong scheduler (fix fcc26ea).
- Khi orchestrator tắt, endpoint vẫn trả 200 với `enabled: false`, kèm dữ liệu đang có trong DB.
- Thêm helper đọc DB vào `src/db.rs` nếu cần, chỉ là hàm thêm mới. Không đổi schema và không thêm migration.

### A2. Duyệt kèm chỉnh sửa: `POST /api/proposals/{id}/approve`
- Body tuỳ chọn (không có body vẫn hoạt động như cũ): `{"team"?: string, "workflow"?: string, "priority"?: string, "title"?: string}`.
- Ghép các trường được gửi lên đè vào đề xuất. Sau đó dựng `ProposalDraft` (project là slug của `project_id`) và gọi `triage::validate` với dữ liệu nạp từ `AppState`, theo đúng cách `triage_with_warnings` nạp. Nếu `validate` lỗi thì trả 422 kèm thông báo lỗi và không đổi gì trong DB.
- Hợp lệ: cập nhật `team`, `workflow`, `priority`, `title` cùng `status='approved'` trong **một** câu UPDATE có `WHERE id=? AND status='pending'`. Nếu 0 dòng bị đổi thì trả 409. Thêm helper `approve_proposal_with(conn, id, team, workflow, priority, title)` vào db.rs.
- Giữ nguyên mã lỗi hiện có: 404 khi không tìm thấy, 409 khi không còn ở `pending`.

### A3. Huỷ intake đúng trạng thái
`POST /api/intake/{id}/cancel` chỉ chấp nhận khi trạng thái là `queued` hoặc `failed`. Các trạng thái khác trả 409, thông báo: "Chỉ huỷ được việc đang chờ hoặc bị lỗi".

### A4. Danh sách đội hình và quy trình cho dropdown
Kiểm tra `GET /api/teams` trong `api_settings.rs` (`get_teams`) đã trả danh sách quy trình chưa.
- Nếu đã có: dùng luôn, KHÔNG thêm route.
- Nếu chưa có: trong `GET /api/orchestrator/board`, thêm `"teams": ["tên", …]` và `"workflows": ["tên", …]` (chỉ tên), nạp giống A2.
Ghi rõ trong báo cáo đã chọn cách nào.

## Phần B: Frontend

### B1. Đăng ký màn
- Thêm `assets/dashboard/js/queue.js` và `assets/dashboard/screens/queue.html`.
- Thêm 2 mục vào `ASSETS` trong `assets.rs`.
- `app.js`: `screenTitles.queue = 'Điều phối'`; `parseHash` nhận `#/queue`.
- `index.html`: thêm link menu "Điều phối" (`href="#/queue"`), đặt ngay sau "Giao việc", cùng kiểu class với các link khác (`navClass('queue')`, `navCurrent('queue')`).
- `tests/dashboard_assets.rs`: thêm `"queue"` vào danh sách trong `asset_references_resolve`.

### B2. Bố cục `queue.html` (một màn, từ trên xuống)
1. **Thanh trạng thái**:
   - Chip "Orchestrator: bật/tắt" và "Slot: {active}/{max_parallel}".
   - Nếu `auto_resume_quota` bật: dòng "Tự chạy lại sau hết quota: {resume_after_minutes} phút".
   - Nếu `enabled=false`: hộp cảnh báo vàng "Orchestrator đang tắt: việc giao mới sẽ chạy thẳng, không qua hàng chờ. Bật bằng `"orchestrator": {"enabled": true}` trong config.json rồi khởi động lại serve." Các cột task bên dưới vẫn hiện.
2. **Ô giao nhanh** (chỉ hiện khi `enabled`):
   - textarea yêu cầu, dropdown dự án (tuỳ chọn, mặc định "Để agent tự chọn", lấy từ `/api/projects`), nhóm 3 nút ưu tiên **Gấp / Thường / Thấp** (mặc định Thường), nút "Đưa vào hàng chờ".
   - Gửi tới `POST /api/intake` với `{request, project?, priority}`. Thành công thì xoá ô và tải lại ngay.
3. `<section data-queue-visual></section>` để trống, dành cho U2.
4. **Năm cột** (lưới 5 cột từ `xl`, 2 cột ở `md`, 1 cột trên điện thoại; không cuộn ngang trang; mỗi cột có tiêu đề kèm số lượng):
   | Cột | Nội dung | Hành động |
   |---|---|---|
   | **Hàng chờ** | intake `queued` (nhãn "Chờ phân tích"), `triaging` (nhãn "Đang phân tích…"), `failed` (lỗi rút gọn, xem đầy đủ khi mở) | Huỷ (queued/failed) |
   | **Cần bạn duyệt** | (a) đề xuất `pending`; (b) task có `group==='waiting'` mà KHÔNG thuộc cột Dừng | (a) xem bên dưới; (b) link "Xem và duyệt plan" → `#/task/{id}` |
   | **Chờ slot** | đề xuất `approved`, theo đúng thứ tự từ API; đánh số thứ tự 1, 2, 3… | Đổi ưu tiên |
   | **Đang chạy** | task `group==='running'`: tiêu đề, dự án, `current_step`, thanh tiến độ theo `steps` (bước đã xong / tổng) | Link mở task |
   | **Dừng** | task có `quota_detail` hoặc nằm trong `quota_stops`, hoặc `auto_state==='stopped'`, hoặc `group==='attention'`. Quota: engine, thời điểm, đếm ngược tới `resume_at` nếu có, nếu không thì ghi "Chờ bạn chạy lại". Khác: `stop_reason` / `banner` | Link mở task |
   - Kiểm tra tên trường thật trong JSON của `/api/tasks` trước khi dùng (`quota_detail`, `auto_state`, `stop_reason`, `steps`). Cấu trúc `steps` phải đọc từ code, không đoán. Một task chỉ được nằm ở **một** cột. Thứ tự xét: Dừng > Đang chạy > Cần duyệt.
   - **Thẻ đề xuất pending:**
     - tiêu đề, dự án, chip ưu tiên (Gấp: đỏ, Thường: xám, Thấp: xanh nhạt), đội hình, quy trình, lý do (`reason`, cắt 3 dòng, có nút xem thêm), yêu cầu gốc (thu gọn).
     - Nút **Duyệt**, **Sửa rồi duyệt** (mở form ngay trong thẻ: tiêu đề, dropdown đội hình, dropdown quy trình, ưu tiên → gửi body A2), **Từ chối** (ô lý do tuỳ chọn, cần bấm xác nhận lần hai).
     - Lỗi 422/409 hiện ngay trong thẻ.
   - Thẻ trong cột "Cần bạn duyệt" sắp xếp: đề xuất gấp trước, rồi task chờ duyệt plan, rồi các đề xuất còn lại.
5. **Gần đây** (thu gọn mặc định, `<details>`):
   - danh sách `recent` (đề xuất bị từ chối hoặc lỗi, intake lỗi hoặc đã huỷ)
   - 10 task `group==='done'` mới nhất.

### B3. Hành vi
- Tải `/api/orchestrator/board` và `/api/tasks?limit=100` + `profileQuery` song song. Tự tải lại mỗi 5 giây qua cơ chế timer của dashboard. Không tải lại khi đang mở form "Sửa rồi duyệt" hoặc ô lý do từ chối, để không mất dữ liệu đang nhập. Tải lại xong phải giữ nguyên trạng thái mở/đóng của thẻ.
- Nếu một API lỗi: hiện dòng lỗi ở vùng tương ứng, phần còn lại vẫn hiện.
- Nút bấm có trạng thái đang gửi (disabled) để chống bấm hai lần.
- Sáng/tối (`dark:`) như các màn khác. Class Tailwind phải là chuỗi literal đầy đủ, không ghép động. Chỉ dùng màu đã có trong các màn hiện tại (đỏ / amber / indigo / slate / emerald…).
- Bàn phím:
  - mọi nút và link đều focus được bằng Tab, có `focus-visible:outline`
  - form sửa đóng bằng `Esc`
  - nút chọn ưu tiên có `aria-pressed`.

## Không làm trong U1
- Không làm sơ đồ 3D, không thêm three.js.
- Không thêm nút duyệt trên Telegram.
- Không đổi hành vi bộ lập lịch, triage, hay màn Giao việc / Bảng công việc.
- Không thêm migration.

## Test
- Mở rộng `tests/intake_api.rs`, hoặc tạo file mới `tests/orchestrator_board_api.rs` dùng chung harness với `intake_api.rs`. Các trường hợp cần có:
  1. `GET /api/orchestrator/board` trả đủ khoá `status`, `intake`, `proposals`, `quota_stops`, `recent`. `proposals` có đúng thứ tự urgent > normal > low và có `project`/`project_name`.
  2. Đề xuất `rejected`/`started` không nằm trong `proposals`.
  3. Approve không có body: thành công như cũ (test cũ vẫn pass).
  4. Approve với `team` không tồn tại: trả 422, đề xuất vẫn `pending`, team trong DB không đổi.
  5. Approve với team/workflow/priority hợp lệ: trả 200, DB có giá trị mới và `status='approved'`.
  6. Approve hai lần: lần hai trả 409.
  7. Cancel intake đang `proposed`: trả 409. Cancel intake `queued`: trả 200.
  8. Khi `enabled=false`: board vẫn trả 200 với `status.enabled=false`.
- `tests/dashboard_assets.rs`: phải pass với 2 asset mới.

## Không được đụng tới
- `.planning/STATE.md`, `.planning/ROADMAP.md`, các file `.planning/` khác, `.claude/worktrees/`, `~/.claude-orca`, `~/.config/agent-brain/*`.
- `agent-brain/assets/prompts/lead.md`, `brain/prompts/lead.md`, `tests/assets_parity.rs`: test `prompts_equal_phase2` đang đỏ từ trước, đang chờ người dùng quyết định. KHÔNG sửa và không cố làm nó xanh.
- Không kill hay restart tiến trình `agent-brain serve` đang chạy (cổng 7437). Nếu cần chạy thử thì dùng cổng và thư mục dữ liệu tạm riêng (xem `scripts/dashboard-smoke.sh`).
- Không dùng `git add -A`, không dùng `--no-verify`. Stage theo đường dẫn. Không push.

## Kiểm tra
Chạy trong `agent-brain/`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked"
```
Chạy ở gốc repo:
```bash
bash scripts/build-css.sh --check
bash scripts/dashboard-ui-lint.sh
node scripts/dashboard-js-check.mjs        # nếu script nhận tham số / cách gọi khác, đọc đầu file và gọi đúng
bash scripts/dashboard-smoke.sh queue      # nếu smoke chưa biết màn queue thì báo, không bắt buộc sửa smoke
bash spikes/lib/scan-secrets.sh
```
Kết quả mong đợi: build OK, mọi test pass trừ `prompts_equal_phase2` (đỏ từ trước), CSS đồng bộ, scan-secrets OK.

## Tiêu chí nghiệm thu
1. Menu có "Điều phối". `#/queue` hiện đủ 5 cột và thanh trạng thái trên một màn. Trên điện thoại các cột xếp dọc, không cuộn ngang.
2. Duyệt, sửa rồi duyệt, từ chối và đổi ưu tiên hoạt động. Lỗi hiện trong thẻ. Dữ liệu sai bị chặn ở backend (422) chứ không chỉ ở giao diện.
3. Giao việc trên màn này có chọn được mức ưu tiên, và việc mới vào cột "Hàng chờ" ngay sau khi gửi.
4. Mỗi task chỉ nằm ở một cột. Task hết quota nằm ở cột "Dừng", kèm engine và đếm ngược (nếu tự chạy lại đang bật).
5. Màn tự cập nhật mỗi 5 giây và không làm mất nội dung đang nhập.
6. Khi orchestrator tắt, màn vẫn dùng được để xem task và hiện cảnh báo.
7. Các test mới pass, test cũ không đổi kết quả.

## Commit
- Commit 1, backend A1–A4 + test: `feat(agent-brain): orchestrator board API, approve with edits, safe intake cancel`.
- Commit 2, frontend B1–B3 + app.css + dashboard_assets: `feat(agent-brain): Điều phối screen for queue, approvals, running and stopped tasks`.
- Mỗi commit kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Báo cáo cuối (agy phải ghi)
File đã sửa theo từng commit, hash commit, kết quả nguyên văn các lệnh kiểm tra, cách đã chọn ở A4, cấu trúc `steps` đọc được trong code, chỗ lệch plan, và phần chưa kiểm tra (giao diện thật trên trình duyệt nếu không chạy được).
