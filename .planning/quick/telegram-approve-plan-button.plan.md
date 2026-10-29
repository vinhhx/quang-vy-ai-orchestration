# Plan: duyệt plan ngay trên Telegram cho task giao từ Telegram (kể cả tin chuyển tiếp) trên dự án đã tick Telegram

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Quyết định của người dùng (2026-10-29)
Người dùng muốn duyệt plan trên Telegram cho các task tạo từ Telegram, hoặc thuộc dự án đã tick chọn Telegram, kể cả task từ tin chuyển tiếp. Việc này nới luật 05.4 INTAKE-07 ("plan của task giao từ Telegram chỉ duyệt trên dashboard") và rủi ro đã chấp nhận AR-05.4-01 (nội dung chuyển tiếp là dữ liệu không tin cậy mà Lead đọc). Đây là quyết định có chủ đích của người dùng; KHÔNG bỏ các rào chắn bù ở dưới.

## Hiện trạng (đã đọc code)
- `telegram/render.rs`: `approve_offered(s)` = `has_control(s,"approve") && !is_telegram_origin(s)` quyết định nút `Duyệt plan` ở `EventKind::Gate` (~dòng 377) và `EventKind::RunFailed` (~dòng 396); `TELEGRAM_ORIGIN_LINE` (~dòng 20) ghi "duyệt plan trên dashboard".
- `dashboard/api_tasks.rs::run_control_as` nhánh `"approve"` (~dòng 2603): nếu `source == ActionSource::Telegram` và `state.origin == ORIGIN_TELEGRAM` thì `Refused { reason: TELEGRAM_APPROVE_REFUSED }` (hằng ~dòng 2554).
- `telegram/bot.rs`: `Op::Approve => act(ctx,&press,&cb,"Duyệt plan","approve")` (~dòng 495) gọi `run_control_as(..., ActionSource::Telegram)`; nút Kill đã có luồng xác nhận hai lần `Op::KillAsk/KillConfirm/KillCancel` (`on_kill`, ~dòng 910-940) làm mẫu.
- `telegram/callback.rs`: `Op` (`Approve` mã `'a'`), `ALL_OPS`, `MAX_LEN` 64, test round-trip/longest.
- `telegram/intake.rs`: `CONFIRM_FOOTER` (~dòng 51) "Plan của task giao từ Telegram chỉ duyệt trên dashboard."
- Dự án cho phép Telegram: khóa `telegram_projects` trong `projects_map.json` (`intake::allowlist_ids`, bot đọc lại file mỗi lần).

## Luật mới
1. Task có `origin = telegram` được duyệt từ Telegram NẾU dự án của task vẫn nằm trong `telegram_projects` (kiểm lại lúc bấm, lúc đọc file; fail closed: không đọc được file hoặc không có id → từ chối).
2. Task không có origin Telegram: hành vi giữ nguyên (nút có sẵn, như cũ).
3. Duyệt task có origin Telegram cần HAI lần bấm (xác nhận), theo mẫu Kill; task không origin Telegram giữ MỘT lần bấm như cũ.
4. Mọi rào chắn cũ giữ: chỉ chat đã ghép nối bấm được; `rev` phải khớp (nút cũ bị `Stale`); `claim::approve_check` (plan phải commit, không sửa chưa commit, không có run chạy…); duyệt chỉ có hiệu lực khi control `approve` đang bật.

## Thay đổi
| # | File | Dòng / hàm | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/dashboard/api_tasks.rs` | `run_control_as`, nhánh `"approve"`; hằng `TELEGRAM_APPROVE_REFUSED` | Thay điều kiện từ chối: khi `source == Telegram` và `state.origin == ORIGIN_TELEGRAM`, chỉ từ chối nếu dự án KHÔNG còn trong `telegram_projects` (đọc `projects_map.json` qua hàm có sẵn, dùng lại `intake::allowlist_ids` — đổi sang `pub(crate)` nếu cần, không nhân đôi). Lý do từ chối mới: `Dự án không còn trong danh sách Telegram — duyệt plan trên dashboard.` (đổi tên/nội dung hằng cho khớp, cập nhật chỗ dùng). Mọi nhánh còn lại (kiểm `claim::approve_check`, ghi duyệt, chạy autopilot, hoàn lại khi lỗi) giữ nguyên. Đảm bảo duyệt từ Telegram ghi nhận nguồn `telegram` ở nơi hiện đã ghi (audit/sự kiện); nếu hiện chưa có chỗ ghi nguồn thì thêm một dòng audit `Telegram: Duyệt plan task N — đã duyệt` như các hành động khác (`audit()` trong bot.rs). | Nới có kiểm soát, fail closed |
| 2 | `agent-brain/src/telegram/callback.rs` | `Op`, `ALL_OPS`, mã, `needs_run`, test | Thêm `Op::ApproveAsk` và `Op::ApproveConfirm`, `Op::ApproveCancel` với mã một ký tự chưa dùng (kiểm các mã hiện có, không trùng); giữ `Op::Approve` cho đường một lần bấm. Cập nhật `ALL_OPS`, `from_code`, test round-trip/`longest_value_fits`/`rejects_malformed` cho các mã mới; mỗi callback ≤ `MAX_LEN` = 64. | Xác nhận hai lần cho task từ Telegram |
| 3 | `agent-brain/src/telegram/render.rs` | `approve_offered`, `buttons_for` (Gate, RunFailed), `TELEGRAM_ORIGIN_LINE`, `text_for` Gate | `approve_offered(s)` = `has_control(s,"approve")` VÀ (không phải origin Telegram HOẶC dự án còn trong `telegram_projects`). Với task origin Telegram, nút `Duyệt plan` dùng `Op::ApproveAsk` (hai bước); task thường giữ `Op::Approve`. KHÔNG làm IO nặng trong `message_for`/`buttons_for` (đã đọc `projects_map.json` ở đây là IO nhẹ nhưng chỉ làm khi `is_telegram_origin`; nếu cấu trúc hàm không cho đọc thì đưa cờ `project_telegram_ok: bool` vào `TaskState`/tham số dựng sẵn ở `outbox`/`bot` thay vì đọc file trong render, và ghi vào summary). Đổi `TELEGRAM_ORIGIN_LINE` thành: `Task giao từ Telegram — xem file plan trước khi duyệt; plan có thể chứa nội dung từ tin nhắn bạn gửi hoặc chuyển tiếp.` | Nút xuất hiện đúng điều kiện |
| 4 | `agent-brain/src/telegram/bot.rs` | `on_callback` (~dòng 495) và hàm mới `on_approve` | `Op::ApproveAsk`: sửa tin đang bấm thành văn bản xác nhận `Duyệt plan task {id}?\n{title}\nPlan ở file đính kèm bên trên. Bấm Xác nhận để cho phép chạy Dev.` với hai nút `Xác nhận duyệt` (`ApproveConfirm`) và `Huỷ` (`ApproveCancel`), theo mẫu `on_kill`/`button_row` (kể cả cách tạm bỏ tin khỏi registry của detector và đặt lại khi huỷ). `ApproveConfirm`: gọi `act(ctx,&press,&cb,"Duyệt plan","approve")` (`run_control_as` kiểm `rev`, dự án còn trong danh sách, `approve_check`); `ApproveCancel`: khôi phục tin gốc với nút ban đầu (như Kill). Nút cũ/`rev` lệch → thông báo `Stale` hiện có. Ghi `audit()` cho yêu cầu duyệt, xác nhận, huỷ. | Luồng bấm hai lần |
| 5 | `agent-brain/src/telegram/intake.rs` | `CONFIRM_FOOTER` | Đổi thành: `Plan sẽ gửi lại để bạn duyệt: trên Telegram (bấm hai lần) hoặc trên dashboard.` Cập nhật test so khớp chuỗi cũ. | Nhất quán nội dung |
| 6 | `agent-brain/tests/telegram_intake.rs`, `agent-brain/tests/telegram_actions.rs` (chọn file đang có test cổng duyệt và Telegram giả; đọc test hiện có `TELEGRAM_APPROVE_REFUSED`/"không có nút Duyệt plan") | cuối file + sửa test cũ | Lật các test cũ khẳng định "không có nút/bị từ chối" cho task origin Telegram sang hành vi mới. Test mới: (a) Gate của task origin Telegram trên dự án đã tick có nút `Duyệt plan`; dự án KHÔNG còn trong `telegram_projects` thì không có nút; (b) bấm `Duyệt plan` → tin đổi sang xác nhận với hai nút, CHƯA duyệt (DB không có `approved_at`); (c) `Xác nhận duyệt` → task được duyệt (`approved_at`, `approved_commit`), tin ghi `✓ Đã duyệt plan — từ Telegram lúc …`, autopilot tiếp tục theo quy trình; (d) `Huỷ` → tin gốc và nút trở lại, chưa duyệt; (e) giữa hai lần bấm, bỏ dự án khỏi `telegram_projects` → `Xác nhận duyệt` bị từ chối bằng lý do mới, chưa duyệt; (f) nút cũ (`rev` lệch) → `Stale`; (g) bấm từ chat/người khác chat đã ghép nối bị bỏ qua như test hiện có; (h) task KHÔNG origin Telegram vẫn duyệt một lần bấm như cũ; (i) duyệt trên dashboard task origin Telegram vẫn được. Test dùng token giả và thư mục tạm. | Chốt hành vi |
| 7 | `agent-brain/README.md` | mục Telegram: giao việc (~dòng 686-723 gồm "Rủi ro chấp nhận AR-05.4-01"), mục kiểm thử, các câu "chỉ duyệt trên dashboard" (~dòng 480, 667) | Cập nhật: duyệt plan task từ Telegram làm được ngay trên Telegram (xem file plan, bấm Duyệt plan, bấm Xác nhận duyệt) khi dự án còn trong `telegram_projects`; rào chắn còn lại (chat ghép nối, `rev`, kiểm lại danh sách lúc bấm, plan phải commit); sửa mục AR-05.4-01: rủi ro nay còn lại là người dùng duyệt plan có thể chứa nội dung chuyển tiếp mà không đọc kỹ — khuyến nghị đọc file plan trước khi xác nhận; dashboard vẫn duyệt được. | Tài liệu trung thực về rủi ro |
| 8 | `.planning/phases/06-multi-engine-media-branch/06-CONTEXT.md` | sau D-47 | Thêm `- **D-48 (revises 05.4 INTAKE-07, user 2026-10-29):** The plan of a task created from Telegram (forwarded messages included) may be approved on Telegram when its project is still in \`telegram_projects\`, through a two-step confirm; the dashboard still works. Residual risk: forwarded content in the plan is only as safe as the user's reading of the plan file.` | Ghi quyết định |

## Không được đụng tới
- `claim::approve_check`, `approve_record`, logic autopilot, `detect.rs`, quota, `transport.rs`, `outbox.rs` (trừ khi mục 3 bắt buộc dựng cờ ở đó; ghi rõ vào summary).
- Luồng Kill, Trả về Lead, Chạy thêm 1 vòng, Dừng hẳn: giữ nguyên.
- `~/.config/agent-brain/*`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`.
- Không đọc, in hay ghi token Telegram; test dùng token giả.
- `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -5
cargo test --offline --locked --lib telegram 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked --test telegram_intake --test telegram_actions --test telegram_notify --test telegram_quota --test telegram_outage 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked"      # toàn bộ: không lỗi
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(telegram/(bot|callback|render|intake)|dashboard/api_tasks)\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -3
```
Acceptance:
1. Task giao từ Telegram (kể cả chuyển tiếp) trên dự án đã tick: Gate có nút `Duyệt plan`; bấm → màn xác nhận; `Xác nhận duyệt` mới duyệt thật; `Huỷ` không duyệt.
2. Dự án bị bỏ tick giữa chừng: không duyệt được trên Telegram (lý do rõ), vẫn duyệt được trên dashboard.
3. Nút cũ (`rev` lệch) và chat lạ không duyệt được.
4. Task không origin Telegram không đổi hành vi.
5. Mọi test cũ pass (các test cũ về "chỉ duyệt trên dashboard" đã được lật có chủ đích); chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Hai commit: `feat(agent-brain): approve Telegram-created task plans from Telegram with a two-step confirm` (code, test), `docs: Telegram plan approval, D-48 and AR-05.4-01 update` (README, CONTEXT). Mỗi commit kèm `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan (nhất là mục 3 về nơi kiểm danh sách), kết quả từng lệnh verify, phần chưa kiểm (chưa thử qua Telegram thật).
