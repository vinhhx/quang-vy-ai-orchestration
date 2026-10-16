# Biên bản nghiệm thu lõi Rust (Phase 3 / P2-01..P2-09)

> Ghi chú: Bảng này do người dùng điền kết quả sau khi thực hiện các bước nghiệm thu trong [LIVE-CHECKS.md](../agent-brain/LIVE-CHECKS.md).

| ID | Kịch bản | Kết quả mong đợi | Kết quả | Ngày | Ghi chú |
| --- | --- | --- | --- | --- | --- |
| P2-01 | `run lead` trên dự án thật | Log hiện từng bước trên terminal | Đạt | 2026-10-16 | claude 2.1.301, opus; task 1 run 1 succeeded, 4 turn, 28.3s, ~0.16 USD; log từng bước (init, tool, kết quả tool, assistant, result) + dòng `Kết thúc run`; plan commit `fd501e6` trên `task/them-ham-tinh-hieu-hai-so-vao-calcpy`; `status` 0.03s. Ghi chú: Lead phát hiện `hieu` đã có nên plan chỉ thêm test; log in nhiều dòng `system thinking_tokens` (nhiễu). |
| P2-02 | Kill khi agent đang chạy `sleep 300` | Sau 5 giây, `ps` không còn `claude` hay `sleep` liên quan | Đạt (lệnh thay thế) | 2026-10-16 | claude 2.1.301 chặn `sleep 307` ở foreground ("Blocked: standalone sleep"), kể cả `sleep 307 && echo xong` → thay bằng `python3 -c 'import time; time.sleep(307)'` qua wrapper tạm cho phép `Bash(python3 *)`. Cây: claude 308926 → bash 309121 (PGID/SID riêng) → python3 309122. `agent-brain kill 2` dừng sau 1382 ms, run exit 130, status `killed`; 5 s sau `ps -p` chỉ in tiêu đề, không còn tiến trình nào. Cần sửa LIVE-CHECKS.md + claude-allow-sleep.sh cho đúng cách mới. |
| P2-03 | Đóng terminal (Ctrl+C) giữa chừng | Không còn tiến trình sót | | | |
| P2-04 | Run kết thúc | Bảng `runs` có đủ token, session_id, thời gian | | | |
| P2-05 | Dòng JSON lỗi trong stream | Lưu thô, chương trình không dừng | | | |
| P2-06 | `status` | Trả kết quả git, không gọi AI, 0 token | | | |
| P2-07 | `inspect` | Đúng 1 turn, không tạo session lưu lại | Đạt | 2026-10-16 | claude 2.1.301, sonnet; run 5: 1 turn, exit 0, 7.7s, ~0.014 USD; review đúng diff chưa commit. 0 transcript `.jsonl` mới dưới `~/.claude/projects` (giả định A1 `--max-turns` đúng). `stats` có dòng role inspect. Ghi chú: claude vẫn tạo thư mục rỗng `~/.claude/session-env/<session-id>/` (không phải transcript). Lệch hướng dẫn: `calc.py` thật ở `src/calc.py` (LIVE-CHECKS ghi `calc.py` ở gốc repo, cần sửa). |
| P2-08 | `stats --profile work` | Chỉ tính các run của profile work | | | |
| P2-09 | Copy binary sang máy/thư mục khác | Chạy được, không cần cài thêm gì (trừ CLI agent) | Đạt | 2026-10-16 | `build-release.sh` → `P2-09 OK (route A)`, 6 118 976 byte, sha256 0662979a…; `file`: static-pie linked, `readelf -d` 0 NEEDED. Copy sang thư mục khác: `--version` và `stats` exit 0; `env -i HOME=<tmp> PATH=/nonexistent ... stats --json` exit 0. Chưa thử trên máy khác. |
| D-02 | Chạy thử một task thật hoàn chỉnh Lead → Dev → QA và gỡ symlink Python | Chạy thành công qua binary Rust, sau `bash brain/install.sh --uninstall` lệnh `command -v brain` không còn | | | |
| E-06 | Chạy tác vụ với profile `personal` | File transcript session chỉ lưu dưới `~/.claude-orca`, không lưu dưới `~/.claude` | | | |
