# Biên bản nghiệm thu lõi Rust (Phase 3 / P2-01..P2-09)

> Ghi chú: Bảng này do người dùng điền kết quả sau khi thực hiện các bước nghiệm thu trong [LIVE-CHECKS.md](../agent-brain/LIVE-CHECKS.md).

| ID | Kịch bản | Kết quả mong đợi | Kết quả | Ngày | Ghi chú |
| --- | --- | --- | --- | --- | --- |
| P2-01 | `run lead` trên dự án thật | Log hiện từng bước trên terminal | | | |
| P2-02 | Kill khi agent đang chạy `sleep 300` | Sau 5 giây, `ps` không còn `claude` hay `sleep` liên quan | | | |
| P2-03 | Đóng terminal (Ctrl+C) giữa chừng | Không còn tiến trình sót | | | |
| P2-04 | Run kết thúc | Bảng `runs` có đủ token, session_id, thời gian | | | |
| P2-05 | Dòng JSON lỗi trong stream | Lưu thô, chương trình không dừng | | | |
| P2-06 | `status` | Trả kết quả git, không gọi AI, 0 token | | | |
| P2-07 | `inspect` | Đúng 1 turn, không tạo session lưu lại | | | |
| P2-08 | `stats --profile work` | Chỉ tính các run của profile work | | | |
| P2-09 | Copy binary sang máy/thư mục khác | Chạy được, không cần cài thêm gì (trừ CLI agent) | | | |
| D-02 | Chạy thử một task thật hoàn chỉnh Lead → Dev → QA và gỡ symlink Python | Chạy thành công qua binary Rust, sau `bash brain/install.sh --uninstall` lệnh `command -v brain` không còn | | | |
| E-06 | Chạy tác vụ với profile `personal` | File transcript session chỉ lưu dưới `~/.claude-orca`, không lưu dưới `~/.claude` | | | |
