# S7b — agy 1.4.7 headless (D-04, D-26)

Ngày: 2026-10-27 (lần chạy thật 09:23:30Z đến 09:24:52Z, thoát mã 0). agy version: `1.4.7` (`raw/s7b-help.txt`, dòng `# agy --version`). Nguồn: chạy local trên tài khoản agy đã đăng nhập của người dùng; người dùng đã duyệt chạy probe, KHÔNG dùng `--with-skip` nên probe P5 (`skip`) chưa chạy. Không có thông báo xác thực nào xuất hiện. Mỗi lệnh chạy dạng `timeout N agy … </dev/null > FILE 2> FILE.stderr.txt` với `--print-timeout` tường minh, trong repo tạm ngoài repo này. Bằng chứng đã `scrub_raw` và `scan-secrets: OK` trước khi commit. Dòng verify gốc ở `raw/s7b-verify.txt`.

## Bảng kết quả

| Probe | permission_mode (init) | file ghi | result.status | exit | stderr (dòng đầu) |
|-------|------------------------|----------|---------------|------|-------------------|
| `default` (không cờ quyền) | `request-review` | no | SUCCESS | 0 | `jetski: no output produced — a tool required the "write_file" permission that headless mode cannot prompt for, so it was auto-denied. …` |
| `accept-edits` (`--mode accept-edits`) | `request-review` | **yes** | SUCCESS | 0 | rỗng |
| `sandbox` (`--sandbox`) | `request-review` | no | SUCCESS | 0 | cùng thông báo soft-deny như `default` |
| `timeout` (`--print-timeout 5s`) | `request-review` | no | SUCCESS | 0 | `[agy] print timeout after 5s with turn in progress; returning partial output` |
| `badmodel` (`--model khong-co-model-nay`) | không có `init` | no | **ERROR** | **1** | `error: invalid model selection (--model "khong-co-model-nay" …` |
| `procs` (chạy bình thường với accept-edits) | `request-review` | yes | SUCCESS | 0 | rỗng |
| `procs_kill` (bị SIGTERM giữa chừng) | không có `init` | no | **ERROR** | **1** | `error: context canceled` |
| `skip` | chưa chạy (cần `--with-skip`) | — | — | — | — |

Đếm tiến trình (`raw/s7b-procs.txt`): `leftover_after_exit=0`, `survivors_after_kill=0`, `survivors_after_sweep=0`.

## Phát hiện chính

- **Soft-deny (default và sandbox).** Không ghi tệp, `result.status` vẫn `SUCCESS`, thoát mã 0. Dấu hiệu duy nhất nằm ở stderr, dòng bắt đầu bằng: `jetski: no output produced — a tool required the "write_file" permission that headless mode cannot prompt for, so it was auto-denied.` rồi gợi ý thêm `permissions.allow` hoặc `--dangerously-skip-permissions`. `--sandbox` KHÔNG cho phép sửa tệp (`sandbox.file=no`), nên `sandbox` không phải lựa chọn cho Dev.
- **accept-edits ghi được tệp** (`accept_edits.file=yes`), status SUCCESS, exit 0, stderr rỗng. NHƯNG `init.permission_mode` vẫn là `request-review` khi chạy với `--mode accept-edits`: cờ không được phản ánh trong `init`. Vì vậy kế hoạch 06-09 phải kỳ vọng `permission_mode` của Dev là `request-review` (không phải `accept-edits`), và không được dùng `permission_mode` để xác nhận cờ đã có hiệu lực; bằng chứng đúng là diff trong cây làm việc sau lượt chạy (D-05).
- **`--print-timeout` hết hạn.** `result.status` là `SUCCESS`, thoát mã 0, `response` rỗng (chỉ có `init`, `step_update` `user_input` và `result`, mọi `usage` bằng 0). Dấu hiệu duy nhất là dòng stderr `[agy] print timeout after 5s with turn in progress; returning partial output`. Kế hoạch 06-09 phải phát hiện timeout từ stderr, không từ status hay exit code.
- **`--model` sai.** `result.status` là `ERROR`, thoát mã 1, không có `init`; `result.error` bắt đầu `invalid model selection (--model "khong-co-model-nay" --effort ""): model khong-co-model-nay is not recognized as a known model or custom model in settings` kèm danh sách model khả dụng. `conversation_id` rỗng, `num_turns` 0. Đây là hình dạng lỗi để dựng fixture `tests/fixtures/stream/agy_error.jsonl`.
- **Bị SIGTERM.** Gửi SIGTERM tới PID đã ghi: `result.status` `ERROR`, `result.error` `context canceled`, thoát mã 1, stderr `error: context canceled`. Không có `init`.
- **Khóa của step `tool`** (`accept_edits.tool_step_keys`): `conversation_id,state,step_index,step_type,tool_info,tool_name`. Tên công cụ nằm ở `tool_name` (ví dụ `write_file`) và tham số ở `tool_info` (`{"name":"write_file","parameters":{"Path":"…"}}`), KHÔNG có đối tượng `tool` như `tests/fakes/fake_agy.py` đang phát (`tool: {"name","path"}`). Đây là khác biệt fake/thật cần xử lý ở 06-09: hoặc sửa fake cho khớp, hoặc parser đọc `tool_name`/`tool_info`. Step `tool` có hai trạng thái `ACTIVE` rồi `DONE` (kèm `duration_seconds`). `step_type` thấy được: `user_input`, `agent_response`, `tool`. `usage` có các khóa `cache_read_tokens,input_tokens,output_tokens,thinking_tokens,total_tokens`; `num_turns` 1 khi thành công, 0 khi lỗi.
- **Tiến trình còn sót.** Khi chạy, agy sinh tiến trình con (`npm exec chrome-devtools-mcp@latest` → `sh -c` → `chrome-devtools-mcp` → watchdog `node`). Sau khi agy thoát bình thường: `leftover_after_exit=0`. Sau SIGTERM tới PID đã ghi: `survivors_after_kill=0` (trước quét), `survivors_after_sweep=0`. Tức là không cần quét sau khi thoát bình thường; vẫn nên giữ kỹ thuật S4 (ghi con cháu, SIGTERM PID đã ghi, quét theo danh sách đã ghi) làm lưới an toàn cho trường hợp treo.

## Quyết định

`accept-edits`

Dev của agy dùng `--mode accept-edits` (D-26: chế độ hẹp nhất mà probe cho thấy sửa được tệp; `sandbox` không sửa được nên loại, `skip` không cần vì đã có chế độ hẹp hơn). Lead và reviewer không nhận cờ quyền nào. Kiểm tra phạm vi bằng git diff sau lượt chạy và kiểm tra diff rỗng (D-05) luôn áp dụng, và là cách duy nhất xác nhận `accept-edits` có hiệu lực vì `init.permission_mode` không đổi.

Ba dòng cho kế hoạch 06-09:

- **`--print-timeout` hết hạn trông như thế nào:** `status=SUCCESS`, exit 0, `response` rỗng, chỉ có stderr `[agy] print timeout after Ns with turn in progress; returning partial output`; phát hiện từ stderr.
- **Tiến trình còn sót:** không thấy (`leftover_after_exit=0`, `survivors_after_kill=0`, `survivors_after_sweep=0`), nên không cần quét sau khi thoát bình thường.
- **Thông báo soft-deny ở stderr:** `jetski: no output produced — a tool required the "write_file" permission that headless mode cannot prompt for, so it was auto-denied.` (exit 0, status SUCCESS, không có tệp).

## Giới hạn của bằng chứng

- Chưa chạy `--dangerously-skip-permissions` (cần `--with-skip` do người dùng quyết định); `permission_mode` `always-proceed` chưa được quan sát trên 1.4.7.
- Chỉ thử lệnh ghi tệp; chưa thử `run_command` dưới `accept-edits` (có thể vẫn bị soft-deny nên agy Dev có thể không tự chạy test được).
- Chưa quan sát văn bản báo hết quota của agy (giả định A5 vẫn mở); 06-09 giữ hình dạng giả định sau fixture.
- Mẫu timeout chỉ ở `--print-timeout 5s`; hành vi ở ngưỡng dài hơn chưa đo.
