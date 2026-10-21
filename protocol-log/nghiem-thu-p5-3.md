# Biên bản nghiệm thu Phase 5.3 — Thông báo và nút Telegram (TELE-07)

> Ghi chú: Bảng này được điền sau khi thực hiện các bước nghiệm thu trong [LIVE-CHECKS.md](../agent-brain/LIVE-CHECKS.md), mục `5. Nghiệm thu Phase 5.3 (Telegram, TELE-07)`. Mọi ca chạy với bot thật của người dùng trên `api.telegram.org` và điện thoại của người dùng, trong môi trường cô lập của P5.3-00 (cấu hình, dữ liệu và `brain.db` riêng trong `$LIVE`, dashboard cổng 7439, fake claude nên không tốn tiền). Tin nhắn, dòng terminal và kết quả SQL được chép nguyên văn.
>
> **Bí mật**: token bot không bao giờ được ghi vào file này (kể cả một phần). Chat đã ghép nối ghi là `đã ghép (ẩn)`, ID nhóm ở P5.3-06 ghi là `nhóm (ẩn)`. Trước khi commit, file này phải qua kiểm tra dạng token (`git diff -G"[0-9]{8,10}:[A-Za-z0-9_-]{35}"` không nêu file nào).

## Môi trường (P5.3-00)

- Ngày: _chưa điền_
- Commit của repo (binary release): _chưa điền_
- sha256 của `agent-brain/target/release/agent-brain`: _chưa điền_
- Môi trường: `XDG_CONFIG_HOME=$LIVE/xdg`, `XDG_DATA_HOME=$LIVE/data`, `AGENT_BRAIN_DB=$LIVE/data/agent-brain/brain.db`, `AGENT_BRAIN_CONFIG=$LIVE/config.json` (`claude_bin` = `tests/fakes/fake_claude.py`, `kill_grace_secs` 1), `GIT_CONFIG_GLOBAL=$LIVE/gitconfig`; repo nháp `demo-tele` (`mac-dinh`) và `demo-tele-nhanh` (`nhanh`), profile `work`; `serve --port 7439`
- Kiểm tra không cần điện thoại (`serve` chưa có dòng `Telegram`, `telegram test` thoát `1` với `Chưa ghép nối Telegram`): _chưa điền_
- Bot: `@…` (_chưa điền_); chat: `đã ghép (ẩn)`

## Kết quả

| Bước | Mong đợi | Thực tế | Đạt? | Ghi chú |
| --- | --- | --- | --- | --- |
| P5.3-01 | Ghép nối: dán token không hiện gì; mã 6 số gửi từ chat riêng; xác nhận ở terminal và Telegram; `stat -c %a telegram.json` = `600`; `telegram test` tới điện thoại; `serve` in `Telegram: đã kết nối (@…)` | | | |
| P5.3-02 | `auto --start` (`gate.json`) dừng ở cổng; điện thoại nhận file `tele-gate.plan.md` (đọc được đủ) và tin `Chờ duyệt plan · task 1` với dự án, profile, nhánh, tiêu đề plan, danh sách `Thay đổi`, `Review plan: duyệt · vòng 1 / 3`; nút `Duyệt plan`, `Trả về Lead` | | | |
| P5.3-03 | `Duyệt plan` từ điện thoại: tin sửa `✓ Đã duyệt plan <sha7> · autopilot chạy tiếp — từ Telegram …`; autopilot chạy tới QA đạt (`qa_pass`, `done`, `approved_commit` = commit plan); tin `QA đạt · task 1` với tóm tắt | | | |
| P5.3-04 | Tin `Autopilot dừng · hết vòng`; `Trả về Lead` + ghi chú gõ tay → run Lead sau có ghi chú đó; `Chạy thêm 1 vòng` chạy thêm một vòng; `Dừng hẳn` đặt `ended` (ghi lại có / không có tin chờ duyệt sau đó) | | | |
| P5.3-05 | Run đang chạy thêm nút `Kill`; `Kill` → `Xác nhận Kill run N?`; `Huỷ` giữ run chạy; xác nhận → run `killed`, mọi PID đã ghi biến mất (`ps -p`) | | | |
| P5.3-06 | Lệnh từ nhóm không được trả lời; đúng một dòng `Telegram: bỏ qua tin từ chat lạ nhóm (ẩn)`; số task / run không đổi | | | |
| P5.3-07 | Khởi động lại `serve` (SIGINT tới PID đã ghi): không tin cũ nào được gửi lại; `ps -eo args` thấy curl với `-K -`, không có token | | | |
| P5.3-08 | (Không bắt buộc) Mất mạng khoảng một phút: một dòng cảnh báo, một dòng `đã kết nối lại`; tin phát sinh lúc mất mạng tới đúng một lần sau khi có mạng | | | |
| UX-01 | (Backstop, người dùng đánh giá) Tin chờ duyệt và file plan đọc tốt trên điện thoại; nhãn nút (`Duyệt plan`, `Trả về Lead`, `Chạy thêm 1 vòng`, `Dừng hẳn`, `Kill`, `Huỷ`, `Không ghi chú`) hiện đủ, không bị cắt | | | |

## Giả định Bot API (ASSUMED-TG) được xác nhận / bác bỏ

> Ghi chú: Mỗi giả định về Bot API ghi trong các plan 05.3-01 … 05.3-07 được xác nhận hoặc bác bỏ tại đây. Giả định bị bác bỏ thành một plan sửa (gap closure), không sửa code ngay trong phiên nghiệm thu.

| Giả định | Kết quả | Bước | Ghi chú |
| --- | --- | --- | --- |
| | | | |

## Dọn dẹp

- `serve` cổng 7439 đã dừng (SIGINT tới PID đã ghi, `ps -p` chỉ in dòng tiêu đề): _chưa điền_
- `$LIVE` đã xóa: _chưa điền_
- `serve` cổng 7437 và cấu hình thật của người dùng không bị đụng tới: _chưa điền_
