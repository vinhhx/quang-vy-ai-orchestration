# Plan task-split-children: Lead tách việc quá nặng thành task con, duyệt một lần, tổng hợp báo cáo (2026-11-13)

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. **Đọc toàn bộ file này trước khi làm.** Mỗi mục số (1..9) là một commit riêng, `git add` đúng các file đã sửa (không `git add -A`, không `--no-verify`).

## Bối cảnh
Việc quá nặng (ví dụ "test e2e toàn bộ dự án") hiện thành **một** task: Lead viết một plan, Dev chạy không đủ thời gian. Triage (`src/orchestrator/triage.rs:479`) tách được tối đa 5 đề xuất nhưng chỉ đọc câu chữ, không đọc code nên không biết dự án có những chức năng gì. Lead thì có đọc code.

## Quyết định của người dùng (2026-11-13)
1. **Lead tự quyết** khi lập plan: thấy việc quá lớn thì viết *plan tách việc* (danh sách việc con, mỗi chức năng một việc) thay cho plan sửa code.
2. **Duyệt một lần cả danh sách**: người dùng duyệt plan tách việc của task cha (cổng `gate:you` như cũ). Plan của từng task con được **tự duyệt** (không hỏi lại), trừ khi `approve_check` từ chối (ví dụ có vướng mắc), khi đó task con dừng ở cổng như bình thường.
3. Task con chạy **song song**, tối đa `orchestrator.max_parallel` task đang chạy trên toàn hệ thống (dự án không bật worktree thì tự nhiên chạy lần lượt vì dự án bận: đó là hành vi đúng, chỉ cần giữ hàng chờ). Việc con có thể khai `Sau` (phụ thuộc) một việc con trước.
4. Khi mọi task con đã xong, task cha **tổng hợp một báo cáo gộp** (bảng đạt/không đạt theo chức năng, link từng task con, lỗi nổi bật) và **gửi một tin Telegram** kèm file báo cáo.

## Kiến thức nền (đã khảo sát, đường dẫn trong `agent-brain/`)
- Lead: `src/roles/lead.rs` `cmd_lead` (:161). Đầu vào người dùng dựng ở khoảng :340–:430 (biến `user_input`). Prompt gốc `assets/prompts/lead.md`. Parse/validate plan: `src/planfile.rs` `parse_plan` (:321), `validate_plan` (:441) — lỗi `no_changes` khi bảng `## Thay đổi` không có dòng. Mục `## ` lạ được giữ trong `Plan.sections`.
- Duyệt: `src/autopilot/claim.rs` `approve_check` (:523), `approve_record` (:643). CLI `agent-brain approve` → `autopilot::cmd_approve` (`src/autopilot/mod.rs:927`). Dashboard/Telegram → `run_control_as` (`src/dashboard/api_tasks.rs:3779`, nhánh approve :3861–:4000).
- Autopilot: `src/autopilot/decide.rs` (`Decision` :209, `Facts` :358, `build_facts` :402, `decide` :1529, `rule_d5_gate` :1180). Driver `run_loop` (`src/autopilot/mod.rs:449`): `Gate` → `claim::release(..,"gate",..)` (:582), mẫu trạng thái chờ `waiting_human` (:551). `Facts.code_after_plan` được driver điền ở :492–:513 — làm y hệt cho fact mới.
- Tạo task: `StartRequest` (`api_tasks.rs:694`), `start_new_task` (:794), kết quả `StartOutcome` (`Busy`, `PreflightRefused{kind}` ...). Mẫu gần nhất: `rerun_task` (:4837) + `db::set_task_cloned_from` (`db.rs:1015`), `set_task_request` (:993). Hàng chờ có thử lại: `handle_queued_actions` trong `src/orchestrator/scheduler.rs` (chạy cả khi orchestrator tắt). `active_task_count` (scheduler.rs:20).
- Trạng thái: `tasks.status` không có CHECK; giá trị thành công `SUCCESS_STATUSES` (`src/tasks.rs:6`: `qa_pass`, `answered`). `auto_state` lưu: NULL, running, gate, stopped, done, interrupted, waiting_human, ended (+`aj.cancelled` = Đã hủy, `taskstate::is_cancelled` :532). Hiển thị: `src/dashboard/taskstate.rs` `auto_state_of` (:495), `derive_with` (:947), banner (:1003–:1047), group (:1049–:1083). `TaskState` (`api_tasks.rs:1590`), `load_task_state` (:1683).
- Telegram: `src/telegram/detect.rs` (`EventKind` :38, `current_event` :78, `signatures` :139), `render.rs` (`text_for` :371, `plan_lines` :252, `buttons_for` :537), `outbox.rs` (tải file lên: mẫu Answered :448–:470, nhãn :243–:246).
- Lưu file trả lời: `src/answers.rs` `save(project_id, task_id, slug, content)` / `path` / `read`.
- DB: `src/db.rs` `MIGRATIONS`, `SCHEMA_VERSION = 14` (:231). `tests/db_migration.rs:982` assert số schema.
- Test: fixture DB `claim.rs` `fresh_task()` (:671); integration `tests/common/mod.rs` (`Sandbox`, `fake_claude`), `tests/rerun_task.rs` (AppState trong tiến trình + job exe giả), `tests/orchestrator_scheduler.rs`, `tests/autopilot_driver.rs`, `tests/telegram_notify.rs`, `tests/contract_planfile.rs`.

## Phạm vi file (chỉ sửa trong)
- `agent-brain/src/db.rs` (migration 15 + hàm truy vấn mới), `agent-brain/tests/db_migration.rs` (số schema).
- `agent-brain/src/planfile.rs`, `agent-brain/assets/templates/plan.md` (KHÔNG đổi khung; chỉ khi thật cần thêm chú thích — tốt nhất không đụng), `agent-brain/assets/prompts/lead.md`, `agent-brain/src/roles/lead.rs`.
- File mới `agent-brain/src/split.rs` (+ `pub mod split;` trong `agent-brain/src/lib.rs`).
- `agent-brain/src/autopilot/decide.rs`, `agent-brain/src/autopilot/mod.rs`, `agent-brain/src/autopilot/claim.rs` (chỉ nếu cần hàm phụ).
- `agent-brain/src/dashboard/api_tasks.rs`, `agent-brain/src/dashboard/taskstate.rs`, `agent-brain/assets/dashboard/js/task.js`, `agent-brain/assets/dashboard/screens/task.html` (+ CSS dựng sẵn nếu thêm lớp Tailwind mới: `bash scripts/build-css.sh`).
- `agent-brain/src/orchestrator/scheduler.rs`.
- `agent-brain/src/telegram/detect.rs`, `agent-brain/src/telegram/render.rs`, `agent-brain/src/telegram/outbox.rs`.
- `agent-brain/src/main.rs` (chỉ **thêm** lệnh con `split`, diff nhỏ).
- Test mới: `agent-brain/tests/task_split.rs`; thêm test `#[cfg(test)]` trong các file trên; thêm ca vào `agent-brain/tests/contract_planfile.rs` nếu hợp.
- `agent-brain/README.md`: chỉ **thêm** một mục "Tách việc thành task con" (append-only).

## Không đụng
`.planning/STATE.md`, `ROADMAP.md`, `REQUIREMENTS.md`, file `.planning/` chưa track (ví dụ `.planning/milestone.lock`), `~/.config/agent-brain/**`, `~/.local/share/agent-brain/**`, binary đã cài `~/.local/bin/agent-brain`, `~/.claude-orca`. **Không** cài/đổi binary, **không** khởi động lại `agent-brain serve`. Không đổi hành vi plan thường (không có mục `## Tách việc`).

## 0. Trước khi sửa
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
git rev-parse HEAD
cargo test --offline --locked --no-fail-fast -- --test-threads=1 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
```
Ghi HEAD và số nền (passed/failed) vào tóm tắt. Test đỏ sẵn: ghi tên, không sửa.

## 1. Migration 15 (`src/db.rs`)
Thêm phần tử thứ 15 vào `MIGRATIONS`, đặt `SCHEMA_VERSION = 15`, sửa assert ở `tests/db_migration.rs`:
```sql
ALTER TABLE tasks ADD COLUMN parent_id INTEGER REFERENCES tasks(id);
CREATE INDEX tasks_parent ON tasks(parent_id) WHERE parent_id IS NOT NULL;
CREATE TABLE split_items (
  id INTEGER PRIMARY KEY,
  parent_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  idx INTEGER NOT NULL,               -- số thứ tự trong plan (1..N)
  title TEXT NOT NULL,
  request TEXT NOT NULL,
  after_idx INTEGER,                  -- idx của việc con phải xong trước, NULL nếu độc lập
  plan_commit TEXT NOT NULL,          -- commit plan tách việc đã duyệt
  state TEXT NOT NULL DEFAULT 'waiting' CHECK (state IN ('waiting','started','failed','skipped','cancelled')),
  child_task_id INTEGER REFERENCES tasks(id),
  reason TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(parent_id, idx)
);
CREATE INDEX split_items_waiting ON split_items(state) WHERE state = 'waiting';
```
Hàm mới trong `db.rs` (có test đơn vị): `set_task_parent(conn, task_id, parent_id)`, `task_parent(conn, task_id) -> Option<i64>`, `insert_split_items(conn, parent_id, plan_commit, &[NewSplitItem]) -> usize` (dùng `INSERT OR IGNORE` để gọi lại không nhân đôi), `split_items_of(conn, parent_id) -> Vec<SplitItemRow>`, `waiting_split_items(conn) -> Vec<SplitItemRow>` (sắp theo `parent_id, idx`), `set_split_item_started(conn, id, child_task_id)`, `set_split_item_state(conn, id, state, reason)`, `parents_waiting_children(conn) -> Vec<i64>` (task có `auto_state = 'waiting_children'`).
Test migration: DB schema 14 có dữ liệu nâng lên 15 không mất dòng `tasks`; `parent_id` NULL cho task cũ.

## 2. Định dạng plan tách việc (`src/planfile.rs`)
Plan tách việc = plan có mục `## Tách việc` với bảng:
```
## Tách việc
| # | Việc con | Yêu cầu | Sau |
|---|---|---|---|
| 1 | Đăng nhập CMS | Viết và chạy e2e cho luồng đăng nhập/đăng xuất CMS: ... | |
| 2 | Quản lý bài viết | ... | 1 |
```
- Thêm `pub struct SplitItem { idx: usize, title: String, request: String, after: Option<usize> }`, `pub const MAX_SPLIT_ITEMS: usize = 30`, `pub fn split_items(plan: &Plan) -> Vec<SplitItem>`, `pub fn is_split_plan(plan: &Plan) -> bool` (mục tồn tại và có ≥ 1 dòng dữ liệu).
- `validate_plan`:
  - Plan tách việc **được miễn** `no_changes` và bảng `## Thay đổi` phải **rỗng** (có dòng → lỗi `split_with_changes`).
  - Lỗi mới: `split_too_few` (< 2 dòng), `split_too_many` (> 30), `split_bad_row:N` (thiếu cột, tiêu đề hoặc yêu cầu rỗng, `#` không phải số 1..N đúng thứ tự), `split_bad_after:N` (`Sau` không phải số nhỏ hơn `#` của chính dòng đó), `split_title_too_long:N` (> 120 ký tự), `split_request_too_long:N` (> 4000 ký tự).
  - Plan tách việc không được có `## Tạo sản phẩm`, `## Sản phẩm`, `## Gửi báo cáo` (lỗi `split_with_products`). Các mục bắt buộc khác giữ nguyên (`Mục tiêu`, `Không được đụng tới`, `Cách kiểm tra`, `Vướng mắc`).
  - Ô `Yêu cầu` có thể chứa `\|` — dùng cùng bộ tách ô đã có cho bảng `Thay đổi`.
- Hàm mới `pub fn validate_split_allowed(plan: &Plan, ctx: SplitCtx) -> Result<(), String>` với `SplitCtx { is_child: bool, workflow_auto: bool }`: task con không được tách tiếp (`split_nested`); quy trình không `auto` không được tách (`split_needs_auto`). Lead gọi nó sau `validate_plan` (mục 3).
- Thông điệp tiếng Việt cho mỗi mã lỗi theo cách các lỗi hiện có được dịch (tìm nơi map `no_changes` → câu tiếng Việt và thêm cạnh đó).
- Test: plan tách việc hợp lệ; mỗi mã lỗi trên; plan thường không đổi kết quả (chạy lại toàn bộ test planfile cũ).

## 3. Lead (`assets/prompts/lead.md`, `src/roles/lead.rs`)
- `lead.md`: thêm một khối quy tắc **chỉ có hiệu lực khi đầu vào có dòng `Được phép tách việc: có`**:
  - Tách khi việc quá lớn cho một lượt Dev (ví dụ test e2e toàn bộ dự án, kiểm tra/sửa nhiều module độc lập, quá ~10 file hay ~10 kịch bản test). Không tách việc làm được trong một lượt.
  - Khảo sát dự án trước (route, menu, module, test có sẵn) để liệt kê **từng chức năng thật**; mỗi dòng một chức năng, 2–30 dòng.
  - Mỗi ô `Yêu cầu` phải **tự đủ**: nêu chức năng, phạm vi (màn hình/route/module), file/thư mục liên quan, điều kiện xong, và giữ mọi ràng buộc của yêu cầu gốc (dữ liệu test, môi trường, quy ước đặt tên, báo cáo). Task con không thấy yêu cầu gốc ngoài ô này.
  - Chỉ dùng `Sau` khi việc sau thật sự cần kết quả/code của việc trước (ví dụ việc chung "dựng khung test/fixture đăng nhập" làm việc 1, các việc khác `Sau` = 1). Còn lại để trống để chạy song song.
  - Plan tách việc để bảng `## Thay đổi` trống (giữ dòng tiêu đề bảng), không có `## Tạo sản phẩm`/`## Sản phẩm`/`## Gửi báo cáo`; `## Cách kiểm tra` ghi tiêu chí chung để coi cả việc là đạt.
  - Khi không có dòng `Được phép tách việc: có`: tuyệt đối không viết mục `## Tách việc`.
- `lead.rs`, khi dựng `user_input`: thêm dòng `Được phép tách việc: có` khi (a) task không có `parent_id` (`db::task_parent`), (b) workflow đã resolve có `auto == true` và `kind != "analysis"`, (c) workflow có stage `implement`. Task con: thêm dòng `Task này là việc con của task #<parent_id> "<title cha>"; không được tách tiếp.`
- Sau `validate_plan` thành công (:832–:897), gọi `validate_split_allowed`; lỗi thì xử lý như plan không hợp lệ (dùng lại đúng luồng thử lại một lần đã có).
- Test (`tests/task_split.rs`, dùng `fake_claude` kịch bản ghi plan): dry-run/argv của Lead task gốc có dòng "Được phép tách việc: có"; task con thì không, và có dòng "việc con của task #".

## 4. Module `src/split.rs` (lõi, không phụ thuộc dashboard)
- `pub fn plan_split_items(root, protocol, plan_commit) -> BrainResult<Option<Vec<SplitItem>>>`: đọc plan đã commit (`ProtocolPaths::plan_at`), trả `Some` nếu là plan tách việc.
- `pub fn materialize(conn, parent: &TaskRow, plan_commit, items) -> BrainResult<usize>`: `insert_split_items` (idempotent), đặt `auto_state = 'waiting_children'` cho task cha (dùng `db::set_auto_state`, xoá pid). Gọi lại lần hai không tạo thêm dòng.
- `pub fn child_outcome(conn, item) -> ChildOutcome` với `ChildOutcome { Pending, Passed, Failed(String), Cancelled }`:
  - item `failed`/`skipped` → Failed(reason); `cancelled` → Cancelled; `waiting` → Pending.
  - item `started`: task con `status` ∈ `SUCCESS_STATUSES` → Passed; `taskstate::is_cancelled` → Cancelled; `auto_state` = `stopped` **và** không có dòng `quota_stops` cho task con → Failed(`aj.stop.reason` hoặc `last_error`); `auto_state` = `done` mà không thành công → Failed("Xong nhưng không qua QA"); còn lại (running, gate, waiting_human, interrupted, stopped vì quota, NULL) → Pending.
- `pub fn settled(conn, parent_id) -> bool`: mọi item khác Pending.
- `pub fn build_report(conn, parent) -> String`: markdown:
  - `# Báo cáo gộp: <title cha>`; dòng tổng: `Đạt x / Không đạt y / Hủy z / Tổng N`, tổng chi phí `cost_usd` các run của task con.
  - Bảng `| # | Việc con | Task | Kết quả | Ghi chú |`: Task = `#<id> <slug>`; Kết quả = `Đạt`/`Không đạt`/`Hủy`/`Bỏ qua`; Ghi chú = lý do dừng hoặc dòng đầu tóm tắt QA (≤ 200 ký tự, thay `|` bằng `/`).
  - `## Lỗi nổi bật`: với mỗi việc không đạt, tối đa 15 dòng đầu của tóm tắt QA/lỗi cuối của task con.
  - `## File báo cáo của task con`: danh sách file `## Gửi báo cáo`/`## Sản phẩm` của từng task con (chỉ đường dẫn).
  - Nguồn tóm tắt QA: dùng hàm đã có cho tin QA đạt (`summary::document`/`telegram::summary`), không tự parse lại nếu đã có hàm.
- `pub fn aggregate(conn, parent) -> BrainResult<PathBuf>`: `answers::save(project_slug, parent.id, slug, report)`, đặt `tasks.status = 'split_done'`, `auto_state = 'done'`. Chạy lại được (ghi đè báo cáo).
- `pub fn cancel_waiting(conn, parent_id) -> usize`: item `waiting` → `cancelled` (lý do "Task cha đã hủy").
- Test đơn vị trong file cho từng hàm (fixture như `claim.rs::fresh_task`).

## 5. Duyệt và autopilot (`src/autopilot/*`, `src/dashboard/api_tasks.rs`)
- `decide.rs`: thêm `Facts.split_plan: bool` và `Decision::Split`. Trong `decide`, ngay **sau** `rule_d5_gate` và **trước** D6: nếu `split_plan && approved` → `Decision::Split`. Sửa mọi `match` trên `Decision` cho đủ nhánh. Test: plan tách việc chưa duyệt → `Gate`; đã duyệt → `Split`; plan thường không đổi.
- `mod.rs` `run_loop`: điền `split_plan` như `code_after_plan` (:492–:513) bằng `split::plan_split_items`. `Decision::Split` → `split::materialize` rồi `claim::release(.., "waiting_children", ..)` và thoát vòng.
- Tự duyệt plan task con: trong `run_loop`, ở nhánh `Decision::Gate`, nếu task có `parent_id` và có `split_items` của cha trỏ tới task này với `state = 'started'`: gọi `approve_check`; được thì `approve_record(conn, id, sha, "parent", false)` rồi `continue` vòng lặp (không release); bị từ chối thì release `gate` như cũ và ghi lý do vào log. Test trong `tests/task_split.rs` hoặc `tests/autopilot_driver.rs`.
- Đường duyệt tay với plan tách việc:
  - `cmd_approve` (CLI) và nhánh approve của `run_control_as`: sau `approve_record`, nếu plan là plan tách việc → gọi `split::materialize`, **không** khởi động job `auto`/dev/qa, trả kết quả thành công như bình thường (Telegram/dashboard hiển thị đã duyệt).
  - Control `approve_dev`, `approve_qa` với plan tách việc → `ControlOutcome::Invalid("Plan tách việc: chỉ dùng Duyệt")` trước khi ghi duyệt. `approve_none` vẫn cho (ghi duyệt + materialize, không chạy gì khác — scheduler vẫn khởi động task con, vì người dùng đã duyệt danh sách).
- Dừng hẳn (Đã hủy) task cha: ở chỗ control cancel (api_tasks ~:4030–:4051) gọi thêm `split::cancel_waiting`. Task con đang chạy **không** bị dừng (ghi rõ trong banner, mục 7).

## 6. Hàng chờ task con (`src/orchestrator/scheduler.rs`)
Thêm `handle_split_items(st, conn)`, gọi trong `schedule_tick` ngay sau `handle_queued_actions` (chạy **cả khi orchestrator tắt**, vì người dùng đã duyệt):
1. Giới hạn: `limit = st.orch().max_parallel`; `active = active_task_count(..)`; dừng khi `active >= limit`.
2. Với mỗi `waiting_split_items` (theo `parent_id, idx`):
   - Task cha không còn `auto_state = 'waiting_children'` hoặc đã hủy → bỏ qua (item giữ nguyên trừ khi cha bị hủy: `cancelled`).
   - Có `after_idx`: lấy outcome của item đó; Pending → bỏ qua lượt này; Failed/Cancelled → item `skipped`, lý do `Việc #<after_idx> không đạt`; Passed → đi tiếp. Nếu workflow là code, việc sau cần code của việc trước: chỉ đi tiếp khi nhánh task con trước đã merge vào nhánh gốc (dùng đúng kiểm tra mà xử lý `after_proposal_id` của proposal đang dùng ở scheduler.rs :89–:230; tách thành hàm dùng chung, không chép).
   - `StartRequest { project: slug dự án cha, request: "<title>\n\n<request>\n\n(Việc con #<idx> của task #<parent_id> \"<title cha>\")", name: Some("<slug cha>-c<idx>"), workflow: workflow của cha, level: level của cha, team: team của cha, inputs: task_inputs của cha (nếu có), origin: origin của cha, dry_run: false }` → `start_new_task`.
   - `Started{task_id,..}` → `db::set_task_parent(task_id, parent_id)`, `set_task_request(task_id, request)` gốc của item, `set_split_item_started`, `active += 1`.
   - `Busy` hoặc `PreflightRefused{kind:"busy"}` → giữ `waiting`, sang item kế (dự án khác có thể chạy được).
   - Từ chối khác (Invalid, ProjectRefused, PreflightRefused khác) → item `failed` với lý do.
   - Slug trùng (nhánh đã tồn tại) → thử `-c<idx>-r2`, `-r3` như `pick_rerun_slug`.
3. Với mỗi `parents_waiting_children`: nếu `split::settled` → `split::aggregate` (lỗi chỉ ghi `term::emit`, không làm hỏng tick).
Test (`tests/task_split.rs`, mẫu `tests/rerun_task.rs` + `tests/orchestrator_scheduler.rs` với job exe giả ghi argv): 3 item độc lập, `max_parallel = 2` → lượt đầu chỉ khởi động 2; item có `Sau` chờ đến khi item trước Passed; item trước Failed → item sau `skipped`; tất cả settled → cha `status = split_done`, file báo cáo tồn tại và có đủ N dòng bảng; tick lần nữa không khởi động thêm và không ghi đè trạng thái.

## 7. Dashboard (`taskstate.rs`, `api_tasks.rs`, `task.js`, `task.html`)
- `auto_state_of`: thêm `waiting_children` (không tính là running). Banner: `Chờ việc con: x/N xong` (x = số item khác Pending); nếu cha đã hủy mà còn con đang chạy: `Đã hủy — còn k việc con đang chạy`. Group: `waiting`. `status = split_done` → banner `Tách việc: Đạt x/N`, group `done` (thêm `split_done` vào nhóm hiển thị done; **không** thêm vào `SUCCESS_STATUSES` để chuỗi `after` của proposal không coi nhầm).
- `TaskState`: thêm `parent: Option<{id, slug, title}>` và `children: Vec<{idx, title, state, task_id, slug, outcome}>` (outcome: `pending|passed|failed|cancelled|skipped`).
- Control mới `split_aggregate` ("Tổng hợp ngay", có hộp xác nhận): chỉ hiện ở task cha có `split_items`; gọi `split::aggregate` dù chưa settled (báo cáo ghi rõ việc còn đang chờ). Theo mẫu control `cleanup_worktree`.
- `task.js`/`task.html`: ở trang task cha hiện bảng "Việc con" (idx, tiêu đề, link task con, kết quả); trang task con hiện dòng "Việc con của #<id> <title>" có link. Màn danh sách/board: không bắt buộc đổi.
- Chạy `node ../scripts/dashboard-js-check.mjs` (nếu script nhận tham số khác, đọc đầu file) và `bash ../scripts/dashboard-ui-lint.sh` nếu có sửa HTML/JS.
- Test `#[cfg(test)]` trong `taskstate.rs`: banner/group cho waiting_children, split_done, cha hủy còn con chạy.

## 8. Telegram (`detect.rs`, `render.rs`, `outbox.rs`)
- `render.rs` `plan_lines`: plan tách việc → `Tách thành N việc con:` + 10 tiêu đề đầu (`1. Đăng nhập CMS`...), `… và k việc nữa` nếu dài. Nút Duyệt giữ nguyên.
- `EventKind::SplitDone` (khoá `split:<parent_id>:<updated_at>`): `current_event` trả nó khi `status = split_done`. Text: `Tách việc xong: <title> — Đạt x / Không đạt y / Hủy z` + 5 dòng lỗi nổi bật đầu. Outbox: tải lên file báo cáo (`answers::path`) với nhãn `Báo cáo gộp`, theo mẫu Answered :448–:470.
- Task con: **không** gửi tin "Chờ duyệt plan" khi plan task con được tự duyệt (tự nhiên đúng nếu không release `gate`). Tin QA đạt của từng task con giữ như cũ.
- Test: theo mẫu `tests/telegram_notify.rs` — một tin SplitDone, có file đính kèm, không gửi lặp khi quét lại.

## 9. CLI + README (`src/main.rs`, `README.md`)
- `agent-brain split status <task-id>`: in bảng item (idx, tiêu đề, state, task con, outcome).
- `agent-brain split aggregate <task-id>`: gọi `split::aggregate`, in đường dẫn báo cáo.
- README: mục "Tách việc thành task con": khi nào Lead tách, định dạng `## Tách việc`, duyệt một lần, giới hạn 30 việc, `max_parallel`, task con tự duyệt, Dừng hẳn task cha, báo cáo gộp, hai lệnh CLI.

## Tiêu chí nghiệm thu
1. Plan có `## Tách việc` hợp lệ qua `validate_plan` với bảng `Thay đổi` rỗng; mọi mã lỗi mục 2 có test; plan thường không đổi hành vi (toàn bộ test cũ vẫn qua).
2. Lead của task gốc (workflow auto, có implement) nhận dòng "Được phép tách việc: có"; task con và workflow không auto thì không; plan tách việc ở task con bị từ chối `split_nested`.
3. Duyệt plan tách việc (CLI/dashboard/Telegram) tạo đúng N dòng `split_items`, cha sang `waiting_children`, không khởi động Dev; duyệt lần hai không nhân đôi.
4. Scheduler khởi động task con có `parent_id`, tôn trọng `max_parallel`, `Sau`, dự án bận; task con tự duyệt plan và chạy tiếp không cần người.
5. Khi mọi việc con settled: cha `split_done`, báo cáo gộp tồn tại với đủ N dòng, một tin Telegram SplitDone kèm file.
6. Dừng hẳn task cha → item `waiting` thành `cancelled`, không khởi động thêm task con.
7. Migration 14 → 15 giữ dữ liệu; `SCHEMA_VERSION = 15`.

## Lệnh kiểm tra (chạy hết trước khi báo xong)
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo build --offline --locked
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "^(warning|error)" | sort | uniq -c   # không thêm warning mới so với trước
cargo test --offline --locked --test task_split -- --test-threads=1
cargo test --offline --locked --test db_migration -- --test-threads=1
cargo test --offline --locked --no-fail-fast -- --test-threads=1 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
cd .. && python3 -m unittest discover -s tests -t . -q
bash spikes/lib/scan-secrets.sh 2>/dev/null || true
```
Số failed của toàn bộ test phải ≤ số nền ở mục 0 (và không có tên test đỏ mới).

## Tóm tắt phải ghi khi xong
Ghi vào `.planning/quick/task-split-children.summary.md` (file mới, commit riêng): HEAD trước, danh sách commit, file đã sửa theo từng mục, kết quả từng lệnh kiểm tra (số passed/failed so với nền), các chỗ làm khác plan và lý do, việc còn để ngỏ.
