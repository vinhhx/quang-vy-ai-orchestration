# Plan: trạng thái "Đã hủy" cho Dừng hẳn + "Chạy lại (task mới)" bằng clone

Repo: /mnt/data/IDE/QuangVy-Orchestration, branch `main`. Code nằm trong `agent-brain/` (Rust + dashboard Alpine). Đọc hết file này trước khi sửa.

## Vấn đề

1. **Dừng hẳn không có trạng thái riêng.** Nút `auto_end` (`agent-brain/src/dashboard/api_tasks.rs`, nhánh `"auto_end"` trong `apply_control`, khoảng dòng 4034) gọi `claim::end_autopilot(.., "stopped", ..)` → `auto_state = 'ended'`, giữ nguyên `tasks.status`. `taskstate.rs` (~dòng 519) ánh xạ `ended` thành `"off"`, tức là giống task chưa từng bật autopilot; `queue.js` `classifyTask` thì xếp task đó vào cột "Dừng" nếu `status` là `blocked`/`qa_fix`. Người dùng không phân biệt được "tạm dừng" / "đã hủy" / "chưa bật autopilot". Lưu ý: `ended` cũng được ghi khi bấm "Dừng autopilot" ở cổng duyệt plan (nghĩa là chuyển sang làm tay, KHÔNG phải hủy), nên không thể suy ra "hủy" chỉ từ `ended`.
2. **Không chạy lại được task đã dọn nhánh/worktree.** `reopen` (`api_tasks.rs` ~4071, `taskstate.rs::build_reopen`) từ chối khi nhánh không còn. Cần cách tạo task mới từ cấu hình của task cũ (clone) mà không phụ thuộc nhánh cũ.

## Quyết định thiết kế (đã chốt với người dùng, không bàn lại)

- "Đã hủy" là **trạng thái suy ra**, không thêm giá trị `auto_state` mới: thêm cờ `cancelled: bool` vào `AutoJson` (serde `default`, không đổi schema). Cờ chỉ được bật bởi `auto_end`. Task coi là hủy khi `auto_state == "ended" && aj.cancelled && status chưa thuộc SUCCESS_STATUSES`. Khi autopilot bật lại, `auto_state` rời `ended` nên trạng thái hủy tự mất.
- "Chạy lại" = **tạo task mới** (clone), không reset task cũ, không dựng lại nhánh cũ. Tái dùng `start_new_task` (đường dùng chung của dashboard và Telegram), không viết đường tạo task thứ hai.
- Hộp thoại chạy lại có ô **"Mô tả thay đổi"** (tùy chọn) và hai chế độ plan: `reuse` (dùng lại plan cũ) / `replan` (phân tích lại). Mặc định tự chọn: task cũ chưa merge và ô mô tả trống → `reuse`; ô mô tả có nội dung, hoặc task cũ đã merge vào base, hoặc không lấy lại được plan cũ → `replan`. Người dùng sửa được mặc định. `reuse` bị khóa kèm lý do khi không lấy lại được plan.
- Cả hai chế độ vẫn đi qua cổng duyệt plan; **không** copy `plan_approvals`, `approved_*`, `auto_json`, `runs`, `quota_stops`, `fixbug_refs`, liên kết Trello.
- Plan cũ và mô tả thay đổi được đưa cho Lead qua `task_inputs` + phần văn bản gắn vào request (không sửa engine/prompt của Lead).

## Files in scope

Rust (`agent-brain/src/`):
- `autopilot/decide.rs` — thêm `cancelled` vào `AutoJson`.
- `dashboard/api_tasks.rs` — `auto_end` bật cờ; clone (`rerun`) + route + `rerun-info`.
- `dashboard/taskstate.rs` — suy ra trạng thái hủy, nhóm `cancelled`, nút `rerun`, gợi ý ở `build_reopen`.
- `db.rs` — migration 14 (`cloned_from`, `request`), `SCHEMA_VERSION = 14`, `TaskRow`/`TASK_COLUMNS` nếu cần, hàm ghi/đọc hai cột mới.
- `dashboard/pipeline.rs`, `telegram/render.rs`, `telegram/bot.rs`, `journal/collect.rs` — chỉ khi `group`/trạng thái mới làm hỏng match hiện có hoặc cần nhãn "Đã hủy" (dùng grep để tìm, sửa tối thiểu).

Dashboard (`agent-brain/assets/dashboard/`):
- `js/queue.js`, `js/task.js`, `screens/task.html`, `screens/queue.html`, `js/overview.js`, `js/office-rooms.js`, `js/office3d.js`, `app.css` chỉ khi build-css yêu cầu (xem Verify).

Tests (`agent-brain/tests/` và test trong các file src ở trên): thêm/sửa test; `tests/db_migration.rs` (đổi 13 → 14).

Không sửa: `.planning/STATE.md`, `ROADMAP.md`, các file `.planning/` đang untracked (`milestone.lock`), `~/.claude-orca`, `agent-brain/assets/dashboard/vendor/`, bất kỳ thứ gì ngoài danh sách trên. Nếu thấy cần sửa file ngoài danh sách, dừng và ghi vào summary thay vì sửa.

## Steps

Mỗi task một commit, stage file theo tên (không `git add -A`, không `--no-verify`, không amend). Thông điệp commit tiếng Anh dạng `feat(agent-brain): …`.

### Task 1 — Trạng thái "Đã hủy"

1. `autopilot/decide.rs`: thêm `#[serde(default)] pub cancelled: bool` vào `AutoJson`. `Default` giữ `false`. Kiểm tra mọi chỗ khởi tạo `AutoJson { .. }` bằng literal (grep) vẫn biên dịch.
2. `api_tasks.rs` nhánh `"auto_end"`: trước `claim::end_autopilot`, clone `state.aj`, đặt `cancelled = true`, truyền bản đó vào `end_autopilot`. Ở nhánh "Dừng autopilot ở cổng" (gọi `end_autopilot(&conn, task_id, "gate", ..)` ~dòng 4028) đặt `cancelled = false` trên bản `aj` truyền vào, để cờ cũ không rò sang. Không đổi chữ ký `end_autopilot`.
3. `taskstate.rs`:
   - Suy ra `cancelled = auto_state == "ended" && aj.cancelled && !is_success_status(status)`. Tìm nơi `aj`/`AutoFacts` sẵn có để lấy cờ; nếu `AutoFacts` chưa mang `aj`, thêm trường tối thiểu.
   - Trong tính `group` (~dòng 993): task hủy và không `running`/`driver_alive` → `group = "cancelled"`, đặt TRƯỚC các nhánh `attention`/`waiting` (task hủy với status `blocked`/`qa_fix` không còn rơi vào "attention").
   - `auto.state` của view trả `"cancelled"` cho task hủy (thay vì `"off"`); giữ `"off"` cho `ended` do dừng ở cổng. Thêm `status_word`/nhãn hiển thị "Đã hủy" ở chỗ đang dựng từ khóa trạng thái.
   - Nút: task hủy hiện "Bật autopilot" (đã có), "Chạy lại (task mới)" (Task 2) và "Dọn worktree" (đã cho phép với `ended`). Không hiện "Dừng hẳn" nữa.
4. Dashboard: thêm nhóm `cancelled`, nhãn "Đã hủy", vào `queue.js` (`SIDE_GROUPS`, `classifyTask` trả `'cancelled'` TRƯỚC điều kiện `stopped`, đếm, danh sách, tiêu đề cột) và `screens/queue.html`; `task.js`/`task.html` hiện huy hiệu "Đã hủy" thay banner "đã dừng". Grep mọi chỗ dùng `group ===`, `auto_state ===`, `auto.state` trong `assets/dashboard/js/` để các màn khác (`overview.js`, `office-rooms.js`, `office3d.js`) không coi task hủy là "đang dừng/cần chú ý". Nhãn "Đã hủy" cũng hiện ở Telegram nếu `render.rs` có chỗ in trạng thái task.
5. Test: ở `taskstate.rs` (xem `test_stopped_banner_and_group` làm mẫu) thêm các ca: `ended`+`cancelled` → group `cancelled`, `auto.state == "cancelled"`; `ended` không cờ (dừng ở cổng) → `off`, không phải `cancelled`; hủy rồi bật lại (`auto_state` rời `ended`) → hết `cancelled`; task hủy nhưng đã `qa_pass` → `done`. Ở `api_tasks.rs`/`claim.rs`: `auto_end` ghi `cancelled=true`, dừng-ở-cổng ghi `cancelled=false`. Cập nhật các test cũ bị ảnh hưởng.

### Task 2 — Chạy lại bằng clone

1. `db.rs`: thêm migration (mục thứ 14 của mảng migrations, đặt `SCHEMA_VERSION = 14`):
   `ALTER TABLE tasks ADD COLUMN cloned_from INTEGER REFERENCES tasks(id); ALTER TABLE tasks ADD COLUMN request TEXT;`
   Thêm hàm `set_task_request(conn, id, &str)`, `set_task_cloned_from(conn, id, i64)`, `task_request(conn, id)`, `task_cloned_from(conn, id)`. Task mới tạo qua `start_new_task` lưu `request` = nội dung yêu cầu gốc (`trimmed_req`) ngay sau `upsert_task` (kể cả task không phải clone). Không backfill task cũ. Cập nhật `tests/db_migration.rs` (mọi `13` → `14`) và thêm test migration 13 → 14 giữ nguyên dữ liệu.
2. `api_tasks.rs`: thêm hàm lõi `rerun_task(st, task_id, RerunRequest { description: Option<String>, plan_mode: Option<String> }) -> BrainResult<StartOutcome>` và các route:
   - `GET /api/tasks/{id}/rerun-info` → `{ request, merged, plan_available, plan_unavailable_reason, default_mode, branch_exists, cloned_from }`.
   - `POST /api/tasks/{id}/rerun` body `{ description?, plan_mode? ("reuse"|"replan"), rev? }` → trả như `start_response` (202 kèm `task_id` mới, `job_id`).
   Đăng ký trong `routes()` cạnh `/reopen`.
3. Lõi `rerun_task`:
   - Từ chối (409, `kind: "not_allowed"`) khi task cũ đang có run/driver chạy, hoặc chưa kết thúc (không phải SUCCESS_STATUSES và không phải `cancelled`; task `blocked`/`qa_fix`/failed vẫn cho chạy lại nếu không có gì đang chạy — dùng cùng điều kiện "không busy" với `cleanup_refusal`). Kiểm tra `rev` giống các control khác (`ControlOutcome::Stale`).
   - Request gốc: `tasks.request`, nếu NULL thì `request:` trong meta của plan (`planfile::read_meta`), nếu vẫn không có thì `tasks.title`.
   - Đội hình = `tasks.team_json` (tên đội/đội hình đã lưu; nếu `start_new_task` chỉ nhận tên đội thì lấy tên từ `team_json`), workflow = `workflow_json` (tên workflow), `level` của task cũ. Truyền qua `StartRequest { team, workflow, level, .. }`; nếu một trong số đó không còn tồn tại, `start_new_task` đã trả `Invalid` — giữ nguyên thông báo.
   - Tên mới: `<slug cũ>-r2`, `-r3`, … chọn số nhỏ nhất mà `task/<slug>` và `analysis/<slug>` chưa có hàng trong DB và nhánh chưa tồn tại trong repo.
   - Lấy plan cũ (cho cả hai chế độ, để làm tham chiếu): thử theo thứ tự (a) file plan trong `workdir` của task cũ nếu còn; (b) `git show <branch>:<đường dẫn plan>` nếu nhánh còn; (c) `git log --all -1 --format=%H -- <đường dẫn plan>` rồi `git show <commit>:<đường dẫn plan>`. Đường dẫn plan lấy từ `protocol_paths::paths_in(protocol_dir, slug).plan` với `protocol_dir` của dự án. Không tìm được → `plan_available=false` và lý do; `reuse` bị từ chối (422) khi `plan_available=false`.
   - `merged`: dùng `worktree_merged(project, task)` (api_tasks.rs ~235); nếu nhánh không còn mà task có status SUCCESS thì coi là `merged = true`.
   - Chế độ: `plan_mode` do người dùng gửi; nếu thiếu, áp mặc định đã chốt ở trên.
   - Dựng request cho `start_new_task`: `"<request gốc>\n\n--- Chạy lại từ task #<id> (<slug>) ---\n<chỉ thị>\n"` với chỉ thị: `reuse` → "Dùng lại plan trong tài liệu đầu vào `plan-cu.md`: chép nguyên nội dung vào file plan, chỉ cập nhật phần meta (task, branch, base_commit, request). Không đổi bước/tiêu chí nghiệm thu."; `replan` → "Lập plan mới. `plan-cu.md` chỉ để tham khảo, code nền có thể đã đổi (task cũ đã merge: <có/không>)."; thêm `"Yêu cầu bổ sung: <description>"` khi có mô tả. Tôn trọng `MAX_REQUEST_CHARS`: nếu quá dài trả `Invalid` rõ ràng, không cắt âm thầm.
   - Inputs của task mới (`StartRequest.inputs`): tối đa `inputs::MAX_INPUTS` (=3). Thứ tự ưu tiên: `plan-cu.md` (nội dung plan cũ, nếu có) trước, rồi các input cũ theo `id` tăng dần cho đến đủ. Input bị bỏ vì hết chỗ → ghi vào `warning` của phản hồi. Tên input phải qua `validate_name`; nội dung qua quét secret như `add_to_task`.
   - Sau khi tạo (`StartOutcome::Started { task_id }`): `set_task_cloned_from(task_id, task cũ)`. Không copy cột nào khác của task cũ.
   - Task cũ giữ nguyên hoàn toàn (không đổi status, không dọn gì).
4. `taskstate.rs`: thêm nút `rerun` ("Chạy lại (task mới)") vào danh sách action của task kết thúc/hủy/blocked không busy, bật khi điều kiện của bước 3 thỏa. Ở `build_reopen`, khi nhánh không còn, `reason` thêm câu "Dùng \"Chạy lại (task mới)\"". Mô tả `cloned_from` trong view (`cloned_from: Option<i64>`) để dashboard hiện "chạy lại từ #N".
5. Dashboard (`js/task.js`, `screens/task.html`): bấm "Chạy lại (task mới)" mở hộp thoại dùng đúng kiểu dialog và `AB.focusLater` của các dialog hiện có: ô "Mô tả thay đổi" (textarea, tùy chọn), nhóm radio hai chế độ plan (mặc định theo `rerun-info`, khóa `reuse` kèm `plan_unavailable_reason`), dòng cảnh báo khi `merged`, nút "Chạy lại" / "Hủy". Gửi `POST /rerun`, thành công thì chuyển sang task mới (`#/task/<task_id>`) như luồng Giao việc đang làm. Lỗi 409/422 hiện đúng thông báo của server. Hiện "chạy lại từ #N" (liên kết) ở đầu màn task khi `cloned_from` có giá trị. Chỉ dùng class Tailwind đã có trong `app.css`; không thêm font-size/padding mới (xem lệnh "banned" ở Verify).
6. Telegram: ngoài phạm vi (không thêm nút chạy lại); chỉ đảm bảo không vỡ khi nhóm `cancelled` xuất hiện.
7. Test (`agent-brain/tests/` — làm theo mẫu `dashboard_races.rs` / `human_dashboard.rs`; một file mới `tests/rerun_task.rs` được phép):
   - clone task `qa_pass` đã dọn nhánh: tạo task mới đúng `team`, `workflow`, `level`, `cloned_from`, `request`; không copy `plan_approvals`/`auto_json`/`runs`.
   - `replan` mặc định khi có `description`; `reuse` mặc định khi mô tả trống và chưa merge; `merged` → `replan`.
   - `plan_available=false` → `reuse` bị 422; `replan` vẫn chạy.
   - tên `-r2`, rồi `-r3` khi `-r2` đã có.
   - tối đa 3 input, plan-cu.md đứng đầu, phần dư vào `warning`.
   - task đang chạy → 409; `rev` cũ → stale.
   - task cũ không bị sửa sau khi clone.

## Acceptance criteria

1. Bấm "Dừng hẳn" → task hiện nhãn "Đã hủy" ở danh sách (nhóm riêng, không nằm trong "Dừng") và màn task; `auto.state == "cancelled"`; bật lại autopilot → hết "Đã hủy".
2. "Dừng autopilot" ở cổng duyệt plan KHÔNG tạo trạng thái "Đã hủy".
3. Task đã `qa_pass`/xong rồi mới bị dừng hẳn vẫn là "Đã xong", không phải "Đã hủy".
4. Task đã merge + dọn nhánh + dọn worktree vẫn chạy lại được qua "Chạy lại (task mới)": tạo task mới, cùng đội hình/workflow/level, nhánh mới tách từ base hiện tại, hiện "chạy lại từ #N".
5. Hộp thoại có ô mô tả + chọn chế độ plan; mặc định đúng quy tắc ở "Quyết định thiết kế"; `reuse` khóa kèm lý do khi không lấy được plan cũ.
6. Task mới luôn phải qua cổng duyệt plan; không có approval nào được copy.
7. Schema `user_version` = 14; DB cũ (13) lên 14 giữ nguyên dữ liệu; task cũ không có `request` vẫn chạy lại được (rơi về meta plan rồi `title`).
8. Không có test cũ nào bị tắt/xóa để qua; test hỏng vì hành vi mới được cập nhật có chủ đích và nêu trong summary.

## Verify (chạy từ repo root, báo lại output)

```bash
cargo build --manifest-path agent-brain/Cargo.toml --offline --locked
cargo test  --manifest-path agent-brain/Cargo.toml --offline --locked          # expect all pass; báo số test trước/sau
for f in agent-brain/assets/dashboard/js/*.js; do node --check "$f" || echo "FAIL $f"; done   # expect không có FAIL
bash scripts/build-css.sh --check                                               # expect "app.css khớp nguồn"; nếu lệch, chạy script build rồi commit app.css
{ grep -rnE '(^|[^a-z0-9-])(text-xs|text-3xl|font-medium|font-bold|p-5|px-2\.5|py-0\.5|px-3|py-3|p-3|gap-3|min-h-10|h-3|hover:bg-indigo-500)([^a-z0-9.-]|$)' agent-brain/assets/dashboard/index.html agent-brain/assets/dashboard/screens agent-brain/assets/dashboard/js; test $? -eq 1; } && echo banned-clean   # expect banned-clean
bash scripts/dashboard-smoke.sh overview assign task stats projects             # expect "Smoke OK: ..."
grep -rn 'SCHEMA_VERSION: i64 = 14' agent-brain/src/db.rs                       # expect 1 dòng
git log --oneline <HEAD trước khi chạy>..HEAD ; git diff --stat <HEAD trước khi chạy>..HEAD   # chỉ file trong "Files in scope"
bash spikes/lib/scan-secrets.sh 2>/dev/null || true                             # nếu script áp dụng được, báo kết quả
```

## Rules

- Không mạng, không cài gói (`cargo ... --offline --locked`). Không chạy `claude` thật.
- **Không cài/thay binary `agent-brain` đang chạy và không start/stop/signal `agent-brain serve` (127.0.0.1:7437)**; chỉ build và test (smoke script dùng sandbox và cổng riêng). Có task đang chạy; việc cài binary do Claude làm ở lần restart serve.
- Chỉ dừng tiến trình bằng PID đã ghi; không `pkill`/`killall`.
- Không tự chạy migration lên `brain.db` thật; test dùng DB tạm.
- Không viết SUMMARY/VERIFICATION; Claude sẽ review và ghi kết quả.
- Cuối cùng in summary: file đã đổi, danh sách commit, mọi sai lệch so với plan (đặc biệt: chỗ đã thêm trường vào `AutoFacts`, nơi `team`/`workflow` được lấy lại), kết quả từng lệnh Verify.
