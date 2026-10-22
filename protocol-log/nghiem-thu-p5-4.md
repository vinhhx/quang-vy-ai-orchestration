# Biên bản nghiệm thu Phase 5.4 — Giao việc qua Telegram (INTAKE-11)

> Ghi chú: Bảng này được điền sau khi thực hiện các bước nghiệm thu trong [LIVE-CHECKS.md](../agent-brain/LIVE-CHECKS.md), mục `6. Nghiệm thu Phase 5.4 (Telegram giao việc, INTAKE-11)`. Mọi ca chạy với bot thật của người dùng trên `api.telegram.org` và điện thoại của người dùng, trong môi trường cô lập của P5.4-00 (cấu hình, dữ liệu và `brain.db` riêng trong `$LIVE`, dashboard cổng 7439, fake claude nên không tốn tiền). Tin của bot, dòng terminal và kết quả SQL được chép nguyên văn; tin chuyển tiếp thì không (xem quy tắc dưới).
>
> **Bí mật và nội dung của người khác**:
> - Token bot không bao giờ được ghi vào file này (kể cả một phần).
> - Chat đã ghép nối ghi là `đã ghép (ẩn)`; không ghi chat ID nào.
> - Người gửi của tin chuyển tiếp ghi là `người gửi 1`, `người gửi 2`, `người gửi 3` (theo thứ tự trong danh sách), không ghi tên thật hay `@username`.
> - Nội dung tin chuyển tiếp chỉ được tóm tắt (ví dụ `tin chữ ngắn về lịch họp`), không chép nguyên văn, kể cả trong dòng danh sách của bot và trong bản ghi của fake claude.
> - Trước khi commit, file này phải qua kiểm tra dạng token (`git diff -G"[0-9]{8,10}:[A-Za-z0-9_-]{35}"` không nêu file nào) và `bash spikes/lib/scan-secrets.sh`.

## Môi trường (P5.4-00)

- Ngày:
- Commit của repo (binary release):
- sha256 của `agent-brain/target/release/agent-brain`:
- Môi trường: `XDG_CONFIG_HOME=$LIVE/xdg`, `XDG_DATA_HOME=$LIVE/data`, `AGENT_BRAIN_DB=$LIVE/data/agent-brain/brain.db`, `AGENT_BRAIN_CONFIG=$LIVE/config.json` (`claude_bin` = `tests/fakes/fake_claude.py`, `kill_grace_secs` 1), `GIT_CONFIG_GLOBAL=$LIVE/gitconfig`; repo nháp `demo-tele` (`mac-dinh`) và `demo-tele-nhanh` (`nhanh`), profile `work`, cả hai trong `telegram_projects`; `serve --port 7439`; `AGENT_BRAIN_TELEGRAM_INTAKE_QUIET_MS` không đặt (2000 ms)
- Kiểm tra không cần điện thoại (`serve` chưa có dòng `Telegram`, `telegram test` thoát `1` với `Chưa ghép nối Telegram`, `PRAGMA user_version` = `3`, `serve` dừng bằng SIGINT tới PID đã ghi):
- Bot: `@…`; chat: `đã ghép (ẩn)`

## Kết quả

| Bước | Mong đợi | Thực tế | Đạt? | Ghi chú |
| --- | --- | --- | --- | --- |
| P5.4-01 | Ghép nối trong `$LIVE`: `stat -c %a telegram.json` = `600`; `telegram test` tới điện thoại; `serve` in `Telegram: đã kết nối (@…)`; menu lệnh của chat có `/task` và `/huy` |  |  |  |
| P5.4-02 | `/task sửa app` → `demo-tele-nhanh · work` → `Mặc định (nhanh)` → `Tạo task`: `Đã nhận — đang lập plan · …`, `Đã tạo task 1 · task/sua-app. …`; tin chờ duyệt có `Task giao từ Telegram — duyệt plan trên dashboard.`, không có `Duyệt plan`; SQL `origin` = `telegram`, `PRAGMA user_version` = `3`; duyệt trên dashboard 7439 ghi `approved_commit`, Dev chạy được sau đó |  |  |  |
| P5.4-03 | `/task` không kèm chữ hỏi yêu cầu, tin chữ kế tiếp mở màn hình chọn dự án; `/huy` → `Đã huỷ — không tạo task.`; nút của tin gửi trước khi `serve` khởi động lại → `Yêu cầu đã hết hạn — gõ /task để bắt đầu lại`, tin mất nút, không có task mới |  |  |  |
| P5.4-04 | Ba tin chuyển tiếp trong một lần → đúng một danh sách 3 tin (A5); bỏ chọn tin 2, `Tiếp tục (2 tin)`, `demo-tele · work`, `Mặc định (mac-dinh)`, việc cần làm `sửa app theo tin`, `Tạo task` → đúng một task, tiêu đề bắt đầu bằng `[Telegram · 2 tin chuyển tiếp]` (SQL và dashboard); bản ghi fake claude: 1 khối mở / 1 khối đóng cùng mã, 2 dòng `[i] người gửi · thời gian`, lời nhắn hệ thống có quy tắc dữ liệu không tin cậy; tin chờ duyệt không có `Duyệt plan` |  |  |  |
| P5.4-05 | Tin chuyển tiếp bắt đầu bằng `/task` + ảnh không chú thích trong một lần → danh sách 1 tin có dòng `/task …` và `(bỏ qua 1 tin không có chữ)`; không có yêu cầu `/task` nào mở; `Huỷ` → số task không đổi |  |  |  |
| P5.4-06 | `telegram_projects` = `["demo-tele-nhanh"]` → `/task x` chỉ đưa ra `demo-tele-nhanh · work`; bỏ khóa → `Chưa có telegram_projects trong projects_map.json — thêm id dự án được phép để giao việc từ Telegram.`; không cần khởi động lại `serve` |  |  |  |
| P5.4-07 | `telegram_max_tasks_per_day` = số task giao từ Telegram hôm nay, khởi động lại `serve` (SIGINT tới PID đã ghi) → `/task x` nhận `Đã đủ N task giao từ Telegram hôm nay — …`; `serve.out` có `Telegram: giao việc — từ chối (giới hạn ngày)`; số task không đổi |  |  |  |
| UX-01 | (Backstop, người dùng đánh giá) Các màn hình `/task`, danh sách chọn (nhãn `☑` / `☐` kèm người gửi) và các tin kết quả (`Đã nhận …`, `Đã tạo task …`, các câu từ chối) đọc tốt trên điện thoại; nhãn nút hiện đủ, không bị cắt |  |  |  |

## Giả định research (A1–A6) được xác nhận / bác bỏ

> Ghi chú: Mỗi giả định về Bot API và cách dùng ghi trong `05.4-RESEARCH.md` được xác nhận hoặc bác bỏ tại đây. Giả định bị bác bỏ thành một plan sửa (gap closure), không sửa code ngay trong phiên nghiệm thu.

| Giả định | Kết quả | Bước | Ghi chú |
| --- | --- | --- | --- |
| A1 (offset của entity tính theo UTF-16; code chỉ đọc từ đầu tiên nên không phụ thuộc) |  | P5.4-02, P5.4-03 |  |
| A2 (bàn phím inline nhận đủ số nút; tối đa 30 dự án, 20 tin) |  | P5.4-02, P5.4-04 |  |
| A3 (`claude -p` cho đọc file ngoài thư mục làm việc; rủi ro chấp nhận AR-05.4-01) | không kiểm trong phiên (fake claude) | — |  |
| A4 (`setMyCommands` thành công hoặc lỗi không làm dừng bot) |  | P5.4-01 |  |
| A5 (một lần chuyển tiếp nhiều tin tới thành các update cách nhau dưới 2 giây, được gom thành một danh sách) |  | P5.4-04 |  |
| A6 (giá trị mặc định: 10 task / ngày, cửa sổ 2 s / 10 s, 20 tin, hết hạn 10 phút, danh sách mặc định rỗng chấp nhận được) |  | P5.4-04, P5.4-07, UX-01 |  |

## Dọn dẹp

- `serve` cổng 7439 đã dừng (SIGINT tới PID đã ghi, `ps -p` chỉ in dòng tiêu đề; 0 run `running` trước khi dừng):
- `$LIVE` đã xóa (gồm `telegram.json`, `brain.db`, bản ghi fake claude, hai repo nháp, log run):
- `serve` cổng 7437 và cấu hình thật của người dùng không bị đụng tới:
- Token bot thử: giữ / thu hồi bằng `/revoke` ở @BotFather:
