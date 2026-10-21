# Biên bản nghiệm thu Phase 5.3 — Thông báo và nút Telegram (TELE-07)

> Ghi chú: Bảng này được điền sau khi thực hiện các bước nghiệm thu trong [LIVE-CHECKS.md](../agent-brain/LIVE-CHECKS.md), mục `5. Nghiệm thu Phase 5.3 (Telegram, TELE-07)`. Mọi ca chạy với bot thật của người dùng trên `api.telegram.org` và điện thoại của người dùng, trong môi trường cô lập của P5.3-00 (cấu hình, dữ liệu và `brain.db` riêng trong `$LIVE`, dashboard cổng 7439, fake claude nên không tốn tiền). Tin nhắn, dòng terminal và kết quả SQL được chép nguyên văn.
>
> **Bí mật**: token bot không bao giờ được ghi vào file này (kể cả một phần). Chat đã ghép nối ghi là `đã ghép (ẩn)`, ID nhóm ở P5.3-06 ghi là `nhóm (ẩn)`. Trước khi commit, file này phải qua kiểm tra dạng token (`git diff -G"[0-9]{8,10}:[A-Za-z0-9_-]{35}"` không nêu file nào).

## Môi trường (P5.3-00)

- Ngày: 2026-10-21 (21:11 – 23:10)
- Commit của repo (binary release): `3e4b4c2`
- sha256 của `agent-brain/target/release/agent-brain`: `68e2f8a279347d7578a65ef81a7361f48f6573ff58e2241e58272906bf1e88e1`
- Môi trường: `XDG_CONFIG_HOME=$LIVE/xdg`, `XDG_DATA_HOME=$LIVE/data`, `AGENT_BRAIN_DB=$LIVE/data/agent-brain/brain.db`, `AGENT_BRAIN_CONFIG=$LIVE/config.json` (`claude_bin` = `tests/fakes/fake_claude.py`, `kill_grace_secs` 1), `GIT_CONFIG_GLOBAL=$LIVE/gitconfig`; repo nháp `demo-tele` (`mac-dinh`) và `demo-tele-nhanh` (`nhanh`), profile `work`; `serve --port 7439`
- Kiểm tra không cần điện thoại (`serve` chưa có dòng `Telegram`, `telegram test` thoát `1` với `Chưa ghép nối Telegram`): **Đạt** — `status demo-tele`: `Nhánh: main`, `(sạch)`; `serve.out` chỉ có dòng URL và dòng `Mở URL trên…`, đếm dòng `Telegram` = `0`; `telegram test` in `Chưa ghép nối Telegram — chạy agent-brain telegram setup`, `exit=1`; sau SIGINT có `Đã dừng dashboard.`, `ps -p` chỉ in dòng tiêu đề
- Bot: `@quang_vy_bot`; chat: `đã ghép (ẩn)`

## Kết quả

| Bước | Mong đợi | Thực tế | Đạt? | Ghi chú |
| --- | --- | --- | --- | --- |
| P5.3-01 | Ghép nối: dán token không hiện gì; mã 6 số gửi từ chat riêng; xác nhận ở terminal và Telegram; `stat -c %a telegram.json` = `600`; `telegram test` tới điện thoại; `serve` in `Telegram: đã kết nối (@…)` | Setup in `Mở Telegram, nhắn cho @quang_vy_bot đúng mã sau (hết hạn sau 10 phút): NNNNNN`, rồi `Đã ghép nối với chat đã ghép (ẩn) (@quang_vy_bot). Đã lưu $LIVE/xdg/agent-brain/telegram.json (quyền 0600). Khởi động lại agent-brain serve để bật thông báo.`, `exit=0`; `stat -c %a` = `600`; `telegram test`: `Đã gửi tin thử tới chat đã ghép (ẩn).`, `exit=0`; điện thoại nhận `Tin thử từ agent-brain trên máy pc0741-MS: kết nối Telegram hoạt động.`; `serve.out`: `Telegram: đã kết nối (@quang_vy_bot)` | Đạt (một phần) | Ca không-hiện-chữ khi dán **không kiểm được**: tab Orca mới gõ sẵn chữ vào PTY trước khi người dùng dán, setup đọc dòng đó làm token (`Token không đúng dạng`, 2 lần). Setup cuối chạy với token qua stdin (nhánh không-tty của `read_secret`). Hai lần ghép trước hết hạn mã vì người dùng gửi muộn. Tin `Đã ghép nối agent-brain với chat này…` setup báo đã gửi, người dùng chưa xác nhận bằng mắt |
| P5.3-02 | `auto --start` (`gate.json`) dừng ở cổng; điện thoại nhận file `tele-gate.plan.md` (đọc được đủ) và tin `Chờ duyệt plan · task 1` với dự án, profile, nhánh, tiêu đề plan, danh sách `Thay đổi`, `Review plan: duyệt · vòng 1 / 3`; nút `Duyệt plan`, `Trả về Lead` | `Autopilot dừng ở bước duyệt plan (task 1) — duyệt: agent-brain approve 1`, `exit=0`; SQL `1 | planned | gate | None`; điện thoại nhận file `tele-gate.plan.md` (chú thích `Plan task 1: Sửa app() trả về 2`) rồi tin chờ duyệt; `serve.out`: đúng một dòng `Telegram: đã gửi tin chờ duyệt plan cho task 1` | Đạt | Người dùng: file plan đọc được đủ, nút vừa |
| P5.3-03 | `Duyệt plan` từ điện thoại: tin sửa `✓ Đã duyệt plan <sha7> · autopilot chạy tiếp — từ Telegram …`; autopilot chạy tới QA đạt (`qa_pass`, `done`, `approved_commit` = commit plan); tin `QA đạt · task 1` với tóm tắt | `serve.out`: `Telegram: Duyệt plan task 1 — Đã duyệt plan cadcd17 · autopilot chạy tiếp`, `Telegram: đã gửi tin QA đạt cho task 1`; SQL task `1 | qa_pass | done | cadcd1780d20284fe585af56e094006b9facc50d`; run 1–5: `lead · plan`, `plan-review · review:approve`, `dev · implement`, `code-review · review:pass`, `qa · qa:pass`, tất cả `succeeded`; điện thoại: `QA đạt · task 1` / `Sửa app() trả về 2` / `Dự án: demo-tele · profile work · nhánh task/tele-gate` / `Tổng: 5 run · 0 phút · $0.00` | Đạt |  |
| P5.3-04 | Tin `Autopilot dừng · hết vòng`; `Trả về Lead` + ghi chú gõ tay → run Lead sau có ghi chú đó; `Chạy thêm 1 vòng` chạy thêm một vòng; `Dừng hẳn` đặt `ended` (ghi lại có / không có tin chờ duyệt sau đó) | Bước 1: `Autopilot dừng · hết vòng: Review plan vẫn yêu cầu sửa lại sau 3 / 3 vòng. (task 2)`, `exit=4`, SQL `2 | stopped | het_vong`. Bước 2: `Telegram: Trả về Lead task 2 — đã giao (ghi chú 19 ký tự)`; run 12 `lead · vòng 4` ghi chú `thêm test cho app()` (run Lead 6, 8, 10: `None`); ghi chú có trong lời nhắn gửi Lead; tin dừng mới. Bước 3: `Telegram: Chạy thêm 1 vòng task 2 — Đã giao thêm 1 vòng` rồi tin dừng mới **ngay**, không có run mới (run cuối vẫn là 13, `auto_json` `grants.plan = 1`, `stop.round 4`, `stop.cap 4`). Bước 4: `Telegram: Dừng hẳn task 2 — Đã dừng hẳn autopilot…`, SQL `2 | planned | ended | het_vong`; sau đó **có** tin `Chờ duyệt plan · task 2` (không bấm) | Không | **G-01**: sau `Trả về Lead` vượt trần (vòng 4 / trần 3), `Chạy thêm 1 vòng` chỉ nâng trần lên `max_rounds + grant = 4` nên `round >= cap` dừng ngay (`decide.rs` nhánh `review:revise`). Lỗi ở lõi autopilot, dashboard cũng vậy; không phải lỗi Telegram. Thủ tục: `grep -c` trên `claude-record.jsonl` luôn ra `0` vì file ghi JSON với `\u` (đã sửa LIVE-CHECKS) |
| P5.3-05 | Run đang chạy thêm nút `Kill`; `Kill` → `Xác nhận Kill run N?`; `Huỷ` giữ run chạy; xác nhận → run `killed`, mọi PID đã ghi biến mất (`ps -p`) | Task 3 dừng `hết vòng`; `dash_post /api/tasks/3/dev` → `{"job_id":4}`; RUN 20, CLAUDE 2527658 (pgid 2527658), CHILD 2527646 (con của serve). Kill → xác nhận → Huỷ: `Telegram: Kill task 3 — Đã huỷ Kill`, `ps -p` vẫn in cả hai PID, run `running`. Kill → Kill: `Telegram: Kill task 3 — Đã dừng run 20`; `ps -p` chỉ dòng tiêu đề, nhóm 2527658 trống, run `killed`, 0 run `running`; tin còn lại 3 nút `Chạy thêm 1 vòng`, `Trả về Lead`, `Dừng hẳn` | Đạt |  |
| P5.3-06 | Lệnh từ nhóm không được trả lời; đúng một dòng `Telegram: bỏ qua tin từ chat lạ nhóm (ẩn)`; số task / run không đổi | Gửi `/start@quang_vy_bot`, `/help@quang_vy_bot` trong nhóm: bot không trả lời; `serve.out` đúng 1 dòng `Telegram: bỏ qua tin từ chat lạ nhóm (ẩn)`; task / run trước `3 / 20`, sau `3 / 20` | Đạt | Bot là thành viên thường, chế độ riêng tư mặc định (Telegram hiện `không có quyền đọc tin nhắn`) |
| P5.3-07 | Khởi động lại `serve` (SIGINT tới PID đã ghi): không tin cũ nào được gửi lại; `ps -eo args` thấy curl với `-K -`, không có token | `curl -q -sS --max-time 35 --proto =https -K -`, PPID = serve; đếm dạng token trên dòng lệnh curl = `0`. SIGINT → `Đã dừng dashboard.`, `ps -p` chỉ dòng tiêu đề; serve mới, sau 90 giây `serve.out` chỉ có `Telegram: đã kết nối (@quang_vy_bot)`; người dùng: không có tin nào 23:06–23:08 | Đạt |  |
| P5.3-08 | (Không bắt buộc) Mất mạng khoảng một phút: một dòng cảnh báo, một dòng `đã kết nối lại`; tin phát sinh lúc mất mạng tới đúng một lần sau khi có mạng | không chạy | không chạy | Người dùng chọn bỏ qua (không bắt buộc) |
| UX-01 | (Backstop, người dùng đánh giá) Tin chờ duyệt và file plan đọc tốt trên điện thoại; nhãn nút (`Duyệt plan`, `Trả về Lead`, `Chạy thêm 1 vòng`, `Dừng hẳn`, `Kill`, `Huỷ`, `Không ghi chú`) hiện đủ, không bị cắt | Người dùng: file plan đọc được đủ; tin dễ đọc; nhãn nút hiện đủ, không bị cắt | Đạt |  |

## Giả định Bot API (ASSUMED-TG) được xác nhận / bác bỏ

> Ghi chú: Mỗi giả định về Bot API ghi trong các plan 05.3-01 … 05.3-07 được xác nhận hoặc bác bỏ tại đây. Giả định bị bác bỏ thành một plan sửa (gap closure), không sửa code ngay trong phiên nghiệm thu.

| Giả định | Kết quả | Bước | Ghi chú |
| --- | --- | --- | --- |
| ASSUMED-TG-1 | Xác nhận | P5.3-01 | Drain `offset -1` trước khi in mã: tin `hello` gửi trước lần setup cuối không làm hỏng ghép nối |
| ASSUMED-TG-2 | Xác nhận | P5.3-03 … P5.3-05 | Mọi lần bấm nút được xử lý và trả lời (`Đã huỷ Kill`…) |
| ASSUMED-TG-3 | Xác nhận | P5.3-03 … P5.3-05 | Không nút nào bị Telegram từ chối vì `callback_data` |
| ASSUMED-TG-4 | Xác nhận | P5.3-02 … P5.3-05 | sendMessage kèm bàn phím inline |
| ASSUMED-TG-5 | Xác nhận | P5.3-02 | sendDocument `tele-gate.plan.md` kèm chú thích |
| ASSUMED-TG-6 | Xác nhận | P5.3-03 … P5.3-05 | editMessageText đổi nội dung và bỏ / làm mới nút |
| ASSUMED-TG-7 | Một phần | — | Không gặp lỗi 401 / 409 / 429 trong phiên; vẫn chỉ được fake kiểm |
| ASSUMED-TG-8 | Xác nhận | P5.3-01 | getMe trả `quang_vy_bot` |
| ASSUMED-TG-9 | Xác nhận | P5.3-01, P5.3-06 | Chat riêng `type = private`, `from.id = chat.id`; nhóm bị bỏ qua |

## Dọn dẹp

- `serve` cổng 7439 đã dừng (SIGINT tới PID đã ghi, `ps -p` chỉ in dòng tiêu đề): **Đạt** — `Đã dừng dashboard.`; `ps -p` với PID serve và các PID của P5.3-05 chỉ in dòng tiêu đề; 0 run `running` trước khi dừng
- `$LIVE` đã xóa: **Đạt** (gồm `telegram.json`, file token tạm dùng cho setup qua stdin, `brain.db`, hai repo nháp, log run)
- `serve` cổng 7437 và cấu hình thật của người dùng không bị đụng tới: **Đạt** — 7437 vẫn lắng nghe, không bị gửi tín hiệu; `~/.config/agent-brain/telegram.json` không tồn tại
- Token bot đã lộ trong phiên chat với Claude (người dùng dán) và thoáng qua trong `ps` của shell Claude: **cần thu hồi** bằng `/revoke` ở @BotFather
