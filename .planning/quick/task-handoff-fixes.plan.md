# Plan 1b: sửa lỗi review sau task-handoff (2026-11-07)

Tiếp theo `.planning/quick/task-handoff.plan.md`. Đọc plan đó (mục 0–5, §5b) và `.planning/quick/e2e-run-findings.md` mục 1 trước. Code hiện tại ở HEAD `31234c8` (các commit Plan 1: 3c1fac9, 9b65766, 6ad6e3f). Chỉ sửa trong `agent-brain/src/**`, `agent-brain/tests/**`, `agent-brain/assets/dashboard/**`.

## Thay đổi

1. **Độ khó (level) phải có trước khi Lead đầu tiên được chọn thành viên** (`src/dashboard/api_tasks.rs`, `src/orchestrator/scheduler.rs`).
   - Thêm `pub level: Option<&'a str>` vào `StartRequest` (`api_tasks.rs:534`). Cập nhật mọi chỗ khởi tạo `StartRequest`; dùng `None` khi không có level.
   - Trong `start_new_task`, đoạn upsert hàng task trước `jobs.start` (khoảng `:797`): nếu `level` là Some và hợp lệ (cùng tập giá trị `set_task_level` chấp nhận), ghi `tasks.level` ngay lúc đó. Đồng thời truyền `--level <level>` vào args của job Lead/Analyst (`lead.rs:131,473` và `analyst.rs:215` đã nhận cờ này).
   - Scheduler (`scheduler.rs:~162`): đặt `level: Some(&prop.level)` và xóa lời gọi `set_task_level` sau `start_new_task`.
   - Test: thêm vào `tests/orchestrator_scheduler.rs` một test cho đề xuất `level="kho"`: kiểm `tasks.level == "kho"` và args job chứa `--level kho`, cả hai trước khi Lead chạy.
2. **Giới hạn body 64 KiB chặn mất tài liệu đầu vào** (`src/dashboard/mod.rs:106`).
   - Giữ 65536 làm mặc định cho router.
   - Các route nhận tài liệu đầu vào (tạo task/intake có `inputs`, handoff, endpoint thêm input nếu có) phải nhận được 3 tài liệu × 64 KiB sau khi escape JSON. Đặt `DefaultBodyLimit::max(512 * 1024)` riêng cho các route đó (layer trên route con, hoặc router con merge trước layer chung). Kiểm bằng code rằng layer riêng thật sự ghi đè layer chung.
   - Test: gửi body khoảng 200 KiB đến endpoint giao việc có inputs thì không bị 413; body 1 MiB thì bị 413.
3. **Tạo task trực tiếp: tài liệu được thêm sau khi Lead đã chạy** (`api_tasks.rs:~975-995`).
   - Validate mọi input (tên, kích thước, bí mật) **trước** `start_new_task`; lỗi thì trả 400 và không bắt đầu job.
   - Ghi input vào task **trước** `jobs.start`: truyền `inputs` vào `start_new_task` qua `StartRequest` (`inputs: Option<&[InputItem]>`, hoặc kiểu tương đương), ghi ngay sau khi upsert task, trong cùng transaction nếu được.
   - Test trong `tests/task_inputs.rs`: Lead khởi chạy (stub) thấy input trong DB và trong `run_dir/inputs`; input sai thì không có job.
4. **Chỉ số `after` của chuỗi đề xuất bị lệch khi một bản nháp bị loại** (`src/orchestrator/triage.rs:~715`, `src/orchestrator/mod.rs:~76`).
   - Ánh xạ `after` theo chỉ số gốc. Giữ bảng `orig_index → proposal_id` của những bản nháp được giữ lại.
   - `after` trỏ đến bản nháp đã bị loại thì bỏ liên kết và ghi warning (`tracing::warn!`).
   - Test: 3 bản nháp, bản 0 không hợp lệ, bản 2 có `after=1` ⇒ phải nối với bản 1.
5. **Lỗi bị nuốt khi chuyển câu trả lời sang đề xuất kế tiếp** (`scheduler.rs:~174`).
   - Không dùng `let _ =` cho `add_to_proposal` và `move_proposal_inputs_to_task`.
   - Khi lỗi: ghi warning, và vì task đã chạy rồi, thêm một dòng ghi chú vào task (cơ chế cảnh báo hoặc event sẵn có, nếu có) cho biết "không chuyển được tài liệu phan-tich-<id>.md: <lý do>". Tốt hơn nữa: kiểm kích thước và bí mật **trước** `start_new_task`; lỗi thì đánh dấu đề xuất thất bại kèm lý do, không bắt đầu.
6. **Handoff không nằm trong transaction** (`api_tasks.rs` `post_task_handoff`, `~:2470`). Tạo intake, tạo proposal và `add_to_proposal` trong **một transaction**: lỗi thì rollback, trả 422, không để lại proposal `pending` hay intake `proposed` mồ côi. Test cho câu trả lời quá 64 KiB: 422 và số hàng không đổi.
7. **Hàng task mồ côi khi `jobs.start` bị từ chối** (`api_tasks.rs:~797`).
   - Khi start trả Busy hoặc lỗi sau khi đã upsert hàng task mới: xóa hàng đó (chỉ khi nó do lần gọi này tạo ra, chưa có run nào).
   - Upsert không được ghi đè `status`/`base_branch` của task đã tồn tại khác trạng thái `open`: dùng `ON CONFLICT DO NOTHING`, hoặc kiểm trước.
   - Giữ phần dọn của Telegram (`telegram/intake.rs:1562`), không để trùng logic: có thể dùng chung một hàm.
8. **Kiểm bí mật của tài liệu đầu vào** (`src/inputs.rs:52-85`): mọi chỗ gọi đang truyền `&[]`. Truyền danh sách giá trị bí mật thật đã biết, gồm key engine trong config, token Telegram và token dashboard, lấy từ chỗ `secrets.rs` / redact đang dùng cho run. Test: nội dung chứa token Telegram giả lập thì bị từ chối.
9. **Trùng tên tài liệu** (`inputs.rs:~285` `add_to_intake`, và `add_to_task` / `add_to_proposal` nếu cũng thiếu): cùng tên trong cùng đích ⇒ lỗi "Tên tài liệu đã có". Test.
10. **Đề xuất trong chuỗi chờ mãi** (`scheduler.rs`): task trước kết thúc với bất kỳ trạng thái cuối nào khác thành công (`failed`, `killed`, `timed_out`, `stopped`, `rejected`…, theo tập trạng thái thật trong db.rs) ⇒ đánh dấu đề xuất kế tiếp thất bại kèm lý do. Test với `timed_out`.
11. **Đội hình mặc định của hộp thoại handoff** (`assets/dashboard/js/task.js:1512,1537`): lấy từ dữ liệu dự án đích (`default_team_name` hoặc trường tương đương API đang trả về), chỉ dùng `mac-dinh` khi không có.

## Không sửa
`Cargo.toml`, `~/.config/agent-brain/*` và `~/.local/share/agent-brain/*` thật, dashboard thật trên cổng 7437 (serve thật đang chạy, KHÔNG dừng nó), repo ngoài, `.planning/STATE.md`, `ROADMAP.md`, các file `.planning/` khác, `~/.claude-orca`. Không đổi `has_control` (`bot.rs:557`) hay `validate_plan` trên plan đã commit, chỉ ghi nhận trong tóm tắt.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'   # ≥ 1529 + test mới, failed 0
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"     # ≤ 152
cd /mnt/data/IDE/QuangVy-Orchestration
node scripts/dashboard-js-check.mjs
bash scripts/dashboard-ui-lint.sh
bash scripts/build-css.sh --check
bash spikes/lib/scan-secrets.sh
```

## Commit
Hai commit:
1. `fix(agent-brain): apply level before first lead, inputs before start, chain and handoff integrity`: mục 1, 3–10 cùng test.
2. `fix(dashboard): larger body limit for input documents, handoff default team`: mục 2, 11 cùng test.

`git add` đúng file, không `-A`, không `--no-verify`, không push. Mỗi commit kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

Tóm tắt gồm: file sửa, commit, chỗ lệch plan, clippy trước/sau, kết quả kiểm tra.
