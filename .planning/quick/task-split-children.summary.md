# Tóm tắt hoàn thành: Tách việc thành task con (Task Split Children)

- **Kế hoạch**: `.planning/quick/task-split-children.plan.md`
- **Thời gian hoàn thành**: 2026-11-13
- **HEAD trước khi chạy**: `da8f9d5c322ff66e222170db1c6ee1dca5d03488` (`da8f9d5`)

---

## 1. Danh sách commit

Đã thực hiện đúng quy trình từ mục 0 đến mục 9, mỗi mục một commit độc lập:

1. `b3c489c`: `db: migration 15 thêm parent_id và bảng split_items`
2. `06c96b8`: `planfile: định dạng và kiểm tra plan tách việc`
3. `9cb7a76`: `lead: cấp quyền tách việc và chặn tách việc lồng nhau`
4. `9dd3fdc`: `split: thêm module điều phối và tổng hợp báo cáo việc con`
5. `bc5acb0`: `autopilot: quyết định tách việc và tự duyệt việc con`
6. `619a394`: `scheduler: hàng chờ khởi động task con và tổng hợp báo cáo gộp`
7. `c064ff3`: `dashboard: hiển thị quan hệ cha con, tiến độ việc con và báo cáo gộp`
8. `542a233`: `telegram: thông báo duyệt plan tách việc và gửi báo cáo gộp`
9. `8d1133b`: `cli & docs: lệnh split status/aggregate và tài liệu tách việc`

---

## 2. File đã sửa theo từng mục

- **Mục 1 (DB Migration 15)**:
  - `agent-brain/src/db.rs`: Thêm migration 15 (`parent_id` trong `tasks`, bảng `split_items`, index), hằng số `SCHEMA_VERSION = 15`, và các hàm truy vấn helper (`task_parent`, `insert_split_items`, `list_split_items`, `update_split_item_started`, `update_split_item_terminal`, `count_active_children`, `parents_waiting_children`).
  - `agent-brain/tests/db_migration.rs`: Cập nhật schema check lên 15 và bổ sung test `v14_db_upgrades_to_v15_keeping_data`.

- **Mục 2 (Planfile Parser & Validation)**:
  - `agent-brain/src/planfile.rs`: Parse bảng `## Tách việc`, hàm `split_items`, hàm `parse_after_dep`, kiểm tra tính hợp lệ (`validate_plan`, `validate_split_allowed`, các mã lỗi: `split_too_few`, `split_too_many`, `split_header_missing`, `split_item_empty`, `split_dep_invalid`, `split_dep_cycle`, `split_changes_not_empty`, `split_nested`).

- **Mục 3 (Lead Prompt & Role Rules)**:
  - `agent-brain/assets/prompts/lead.md`: Thêm khối quy tắc tách việc khi có cờ `Được phép tách việc: có`.
  - `agent-brain/src/roles/lead.rs`: Cấp cờ `Được phép tách việc: có` cho task gốc hợp lệ, truyền thông báo task con cho task có cha, và kiểm tra `validate_split_allowed`.
  - `agent-brain/tests/assets_parity.rs`: Cập nhật `without_git_rule` bỏ qua khối quy tắc tách việc để giữ tính tương thích parity với phase 2 prompt.

- **Mục 4 (Module Split)**:
  - `agent-brain/src/split.rs`: Thêm mới module `split` gồm `plan_split_items`, `materialize`, `child_outcome`, `settled`, `build_report`, `aggregate`, `cancel_waiting`, `format_report_preview`.
  - `agent-brain/src/lib.rs`: Đăng ký `pub mod split;`.

- **Mục 5 (Autopilot & Decision)**:
  - `agent-brain/src/autopilot/decide.rs`: Bổ sung `split_plan` vào `Facts` và nhánh `Decision::Split`.
  - `agent-brain/src/autopilot/mod.rs`: Xử lý `Decision::Split` gọi `split::materialize` và chuyển sang `waiting_children`; tự duyệt plan task con khi task bắt đầu từ split.
  - `agent-brain/src/autopilot/claim.rs`: Hỗ trợ trạng thái `waiting_children`.
  - `agent-brain/src/dashboard/api_tasks.rs`: Xử lý `cmd_approve` cho plan tách việc; từ chối `approve_dev`/`approve_qa` cho plan tách việc; tích hợp `split::cancel_waiting` khi dừng hẳn/hủy task cha.

- **Mục 6 (Scheduler & Queue)**:
  - `agent-brain/src/orchestrator/scheduler.rs`: Hàng chờ khởi động task con tôn trọng `max_parallel` (`active_task_count < max_parallel`), kiểm tra phụ thuộc `after_idx`, chuyển branch an toàn từ commit plan cha; tự động tổng hợp báo cáo khi tất cả con settled.
  - `agent-brain/tests/task_split.rs`: Thêm mới bộ kiểm thử tích hợp toàn diện.

- **Mục 7 (Dashboard UI & State)**:
  - `agent-brain/src/dashboard/taskstate.rs`: Banner `Chờ việc con: x/N xong`, `Tách việc: Đạt x/N`, xử lý cha hủy còn con chạy; bổ sung `parent` và `children` vào `TaskState`.
  - `agent-brain/src/dashboard/api_tasks.rs`: Thêm action control `split_aggregate` ("Tổng hợp ngay").
  - `agent-brain/assets/dashboard/js/task.js`: Hiển thị bảng việc con ở task cha và liên kết ngược ở task con.
  - `agent-brain/assets/dashboard/screens/task.html`: Cấu trúc HTML cho bảng việc con và thông tin task cha.

- **Mục 8 (Telegram Notification)**:
  - `agent-brain/src/telegram/detect.rs`: Thêm `EventKind::SplitDone`, bỏ qua thông báo chờ duyệt plan cho task con.
  - `agent-brain/src/telegram/render.rs`: Hiển thị danh sách việc con khi duyệt plan tách việc; định dạng thông báo tổng kết `SplitDone` với kết quả Đạt/Không đạt/Hủy và lỗi nổi bật.
  - `agent-brain/src/telegram/outbox.rs`: Tải đính kèm file báo cáo gộp với nhãn `Báo cáo gộp`.

- **Mục 9 (CLI & Documentation)**:
  - `agent-brain/src/main.rs`: Bổ sung lệnh CLI `agent-brain split status <task-id>` và `agent-brain split aggregate <task-id>`.
  - `agent-brain/README.md`: Bổ sung bảng lệnh CLI và mục tài liệu "Tách việc thành task con".

---

## 3. Kết quả các lệnh kiểm tra

| Lệnh kiểm tra | Số nền (Item 0) | Kết quả sau khi làm | Trạng thái |
|---|---|---|---|
| `cargo build --offline --locked` | Thành công | Thành công (0 error) | Đạt |
| `cargo clippy --offline --locked --all-targets` | 0 error | 0 error, không thêm warning mới | Đạt |
| `cargo test --offline --locked --test task_split -- --test-threads=1` | Chưa có test | 9 passed, 0 failed | Đạt |
| `cargo test --offline --locked --test db_migration -- --test-threads=1` | 39 passed | 40 passed, 0 failed (+1 migration 15) | Đạt |
| `cargo test --offline --locked --no-fail-fast -- --test-threads=1` | 2085 passed, 0 failed | **2107 passed, 0 failed** (+22 test mới) | Đạt |
| `python3 -m unittest discover -s tests -t . -q` | 128 passed, 1 pre-existing failed | 128 passed, 1 pre-existing failed (`test_stream_fixture_covers_all_samples` giữ nguyên) | Đạt |
| `bash spikes/lib/scan-secrets.sh` | scan-secrets: OK | scan-secrets: OK | Đạt |

---

## 4. Các chỗ làm khác plan và lý do

1. **`agent-brain/tests/assets_parity.rs`**: Cập nhật hàm lọc `without_git_rule` bỏ qua khối quy tắc tách việc mới thêm trong `agent-brain/assets/prompts/lead.md`. Đúng theo ghi chú tại mục 3 của plan ("nếu `tests/assets_parity.rs` so sánh hai file, cập nhật `assets_parity.rs` để bỏ qua phần tách việc mới").
2. **`parse_after_dep` trong `planfile.rs`**: Hỗ trợ định dạng mở rộng cho cột "Sau" như `"Sau #k"`, `"#k"`, `"k"`, `"—"`, `"-"` để chấp nhận cả văn phong tiếng Việt tự nhiên của LLM/người dùng mà không gây lỗi phân tích cú pháp nghiêm ngặt.
3. **Cơ chế khởi tạo an toàn cho `TaskState`**: Các trường `parent` và `children` được khởi tạo an toàn với `None` và `Vec::new()` để đảm bảo toàn bộ các điểm khởi tạo `TaskState` khác trong crate không bị ảnh hưởng breaking change.

---

## 5. Việc còn để ngỏ (Future work)

1. **Hiển thị cây phân cấp trên Board Kanban**: Hiện tại các task con xuất hiện độc lập trên Kanban board như các task thông thường (dù trong trang chi tiết có bảng liên kết rõ ràng); có thể nhóm theo card cha hoặc gắn nhãn phân cấp trên board ở bản cập nhật giao diện sau.
2. **Hỗ trợ phân rã việc nhiều tầng (nested split)**: Hiện tại hệ thống chủ động chặn phân rã việc lồng nhau (`split_nested`) để đảm bảo kiểm soát độ phức tạp và tài nguyên hệ thống.
