# Plan: gửi đề xuất của orchestrator (`proposed`) về Telegram và duyệt / từ chối ngay trên Telegram

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Vấn đề (đã xác minh trên brain.db, task 51, 2026-11-12)
Người dùng giao việc từ Telegram (hàng chờ `intake` #41, `source = telegram`). Orchestrator chia việc xong, tạo dòng `proposals` trạng thái `pending` và đặt intake sang `proposed` (09:22:24). Telegram không nói gì: người dùng phải mở dashboard, bấm duyệt đề xuất (09:35:16 task 51 mới được tạo) thì việc mới chạy tiếp.

Nguyên nhân trong code: `telegram/detect.rs` chỉ có thông báo theo task (`EventKind`: Gate, Stop, QaPass, RunFailed, Quota, Answered) và `IntakeFailures` (chỉ intake `failed`). Không có gì nhìn bảng `proposals`. Chữ `proposed` chỉ xuất hiện ở `orchestrator/mod.rs:170`.

## Quyết định (đã chốt với người dùng trong hội thoại, 2026-11-12)
- Chỉ gửi đề xuất của intake có `source = 'telegram'`. Intake từ dashboard (`source = 'ui'`) KHÔNG gửi sang Telegram.
- Mỗi đề xuất (`proposals` pending) là một tin nhắn riêng, có hai nút: `Duyệt đề xuất` (một lần bấm) và `Từ chối`. Duyệt đề xuất chỉ cho orchestrator bắt đầu task; cổng duyệt plan về sau vẫn riêng (D-48, hai lần bấm) nên không cần xác nhận hai lần ở bước này.
- Không sửa team / workflow / priority / title từ Telegram: duyệt nguyên văn đề xuất. Muốn sửa thì dùng dashboard.

## Hiện trạng (đã đọc code)
- `agent-brain/src/dashboard/api_intake.rs`: handler duyệt đề xuất (~dòng 430-535: lấy `get_proposal`, kiểm `status == "pending"`, tra slug dự án, dựng `ProposalDraft`, `triage::validate(...)`, rồi `db::approve_proposal_with`) và handler từ chối (~dòng 541-585: `db::set_proposal_status_reason(..., "rejected", reason)`). Logic nằm thẳng trong handler axum, chưa tách hàm dùng chung.
- `agent-brain/src/db.rs`: `ProposalRow`, `get_proposal`, `list_proposals`, `approve_proposal_with` (chỉ cập nhật khi `status = 'pending'`), `set_proposal_status_reason`. Bảng `intake` có cột `source`.
- Scheduler (`orchestrator/scheduler.rs`) tự bắt đầu đề xuất `approved`; không cần đổi.
- `telegram/detect.rs`: `run()` seed các bộ quét (`IntakeFailures::seed`, `EnvReruns::seed`, `ChainWaits::seed`) rồi mỗi lượt quét push `Outgoing` vào `ctx.outbox`. Mẫu để bắt chước: `IntakeFailures` (tập `seen` trong bộ nhớ, seed lúc khởi động để restart không gửi lại).
- `telegram/outbox.rs`: `enum Outgoing { Notify, Edit, Keyboard, Message }`; `edit_request` đổi `Edit/Keyboard/Message` thành lời gọi Bot API; `run()` (~dòng 703) chuyển các biến thể đó cho `edit(ctx, &item)` (retry/backoff sẵn có). `Outgoing::Message` hiện không có nút.
- `telegram/bot.rs::on_callback` (~dòng 440-500) phân luồng theo tiền tố của `data`: bắt đầu bằng `i` → `intake::on_callback`; `t:` hoặc `u:` → nút team; còn lại → `callback::decode` (`Op`). Các mã `Op` đã dùng: `a p y q r m e k K x n c v l d b o 0 Q g G h`.
- `telegram/intake.rs`: `project_in_allowlist(map_path, project_id)` (đọc `projects_map.json` ngay lúc gọi, fail closed).
- Mẫu hàm xử lý nút: `on_approve` / `on_kill` trong `bot.rs` (dùng `answer`, `edit_result`, `audit`, `with_suffix`); `render::no_keyboard()`, `render::Button`, `render::keyboard_json`.

## Thay đổi
| # | File | Hàm | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/dashboard/api_intake.rs` | handler duyệt và từ chối đề xuất | Tách hai hàm `pub(crate)` dùng chung, KHÔNG đổi hành vi hay thông điệp lỗi của dashboard: `approve_pending_proposal(st: &AppState, id: i64, over: ProposalOverrides) -> BrainResult<Result<(), (StatusCode, String)>>` (`ProposalOverrides` = bốn `Option` team/workflow/priority/title, `Default` = không ghi đè) và `reject_pending_proposal(st, id, reason: Option<&str>) -> BrainResult<Result<(), StatusCode>>`. Hai handler axum chỉ còn gọi hai hàm này. | Telegram và dashboard cùng một đường duyệt, cùng `triage::validate` |
| 2 | `agent-brain/src/telegram/callback.rs` | module mới phần cuối file | Ngữ pháp riêng cho nút đề xuất, KHÔNG dùng `Op`: `w:{proposal_id}:{rev}` (duyệt) và `z:{proposal_id}:{rev}` (từ chối); `rev` = 16 ký tự hex (`valid_rev`). Thêm `ProposalCb { approve: bool, proposal_id: i64, rev: String }`, `encode_proposal`, `decode_proposal` (chặt như `decode`: sai dạng → `None`; độ dài ≤ `MAX_LEN`). Không dùng tiền tố `i`, `t:`, `u:` và không trùng mã `Op`. Test round-trip, từ chối dữ liệu sai, độ dài lớn nhất. | Tránh đụng ngữ pháp có sẵn |
| 3 | `agent-brain/src/telegram/detect.rs` | struct mới `ProposalNotices`, `run()` | `ProposalNotices { seen: HashSet<i64> }` giống `IntakeFailures`: `seed(conn)` ghi mọi proposal `pending` hiện có vào `seen` (restart không gửi lại, TELE-04); `scan(conn, project_in_allowlist/map_path...)` truy vấn `proposals p JOIN intake i ON i.id = p.intake_id WHERE i.source = 'telegram' AND p.status = 'pending'`, với mỗi id chưa thấy thì push `Outgoing::Prompt { text, keyboard }`. Quên id không còn `pending`. Thêm vào danh sách seed và mảng `results` trong `run()`. Hàm `proposal_rev(&ProposalRow) -> String` (16 hex, băm `id`, `title`, `team`, `workflow`, `priority`, `status`, `updated_at`; dùng hàm băm đang dùng cho `rev` của task ở `dashboard/api_tasks.rs` quanh dòng 1641-1652 nếu dùng lại được, nếu không dùng `DefaultHasher`) để nút cũ (đề xuất đã sửa trên dashboard) bị coi là cũ. | Phát hiện đề xuất mới |
| 4 | `agent-brain/src/telegram/render.rs` | hàm mới `proposal_text`, `proposal_keyboard` | Văn bản: `Đề xuất #{id} — dự án {slug}`, dòng `Tiêu đề`, `Đội hình`, `Quy trình`, `Ưu tiên`, `Cấp`, `Lý do` (nếu có), `Chạy sau đề xuất #{after}` (nếu `after_proposal_id`), và cuối cùng yêu cầu rút gọn bằng `render::clip` (tổng ≤ 3500 ký tự). Không in đường dẫn tuyệt đối. Bàn phím: hai nút `Duyệt đề xuất` (`w:`) và `Từ chối` (`z:`) trên cùng một hàng. | Nội dung tin nhắn |
| 5 | `agent-brain/src/telegram/outbox.rs` | `enum Outgoing`, `edit_request`, `run()` | Thêm `Outgoing::Prompt { text: String, keyboard: serde_json::Value }` gửi bằng `sendMessage` có `reply_markup`; `edit_request` trả `("sendMessage", {chat_id, text, reply_markup})`; `run()` chuyển nó cho `edit(ctx, &item)` cùng nhóm với `Message`. Cập nhật mọi `match` trên `Outgoing` (kể cả `fn notify_id`/test, `edit` ở ~dòng 651) cho biên dịch. | Tin có nút, dùng lại retry/backoff |
| 6 | `agent-brain/src/telegram/bot.rs` | `on_callback`, hàm mới `on_proposal` | Trước nhánh `callback::decode` thêm: nếu `data` bắt đầu `w:` hoặc `z:` thì `callback::decode_proposal(data)`; sai dạng hoặc thiếu `message_id` → `answer(INVALID_BUTTON)`. `on_proposal`: (a) đọc đề xuất bằng `db::get_proposal`; không có → `answer("Đề xuất không còn")`; (b) nếu `status != "pending"` → `answer("Đề xuất đã được xử lý ({status}) ở nơi khác", false)` và sửa tin: thêm hậu tố `— đã xử lý ngoài Telegram`, bỏ nút; (c) nếu `proposal_rev` khác `rev` → `answer("Nút cũ — đề xuất đã đổi, xem lại trên dashboard", true)` và bỏ nút; (d) duyệt: kiểm lại dự án của đề xuất còn trong `telegram_projects` bằng `intake::project_in_allowlist` (đọc lúc bấm, fail closed), không còn → `answer("Dự án không còn trong danh sách Telegram — duyệt trên dashboard.", true)`, KHÔNG duyệt; (e) gọi `approve_pending_proposal(.., Default)` (hoặc `reject_pending_proposal`); lỗi xác thực (`triage::validate`) → `answer` với thông điệp lỗi (rút gọn, `render::clip`), giữ nguyên nút; thành công → sửa tin bằng `edit_result` với hậu tố `✓ Đã duyệt từ Telegram lúc HH:MM` (hoặc `✗ Đã từ chối`), bỏ nút, `answer("Đã duyệt")`. (f) `audit("Duyệt đề xuất" / "Từ chối đề xuất", id, kết quả)` theo mẫu (không in token). | Hành động trên Telegram, fail closed |
| 7 | `agent-brain/tests/telegram_proposals.rs` (mới) | cả file | Dùng `tests/support/telegram.rs` và mẫu của `tests/telegram_intake.rs` / `tests/telegram_notify.rs` (token giả, thư mục tạm, Telegram giả `tests/fakes/fake_telegram.py`). Ca kiểm: (a) intake `source=telegram` có proposal `pending` → đúng MỘT tin với hai nút; tin chứa tiêu đề/đội hình/quy trình; (b) intake `source=ui` → không có tin; (c) restart serve (seed) không gửi lại đề xuất đã có; (d) bấm `Duyệt đề xuất` → `proposals.status = approved`, tin đổi thành `✓ Đã duyệt…`, không còn nút; scheduler nhận (dùng helper có sẵn nếu test khác đã làm, nếu không chỉ cần kiểm `status`); (e) bấm `Từ chối` → `rejected`; (f) bấm lần hai / đã duyệt trên dashboard trước → báo đã xử lý, không đổi gì; (g) `rev` lệch (đổi `title` trong DB giữa lúc gửi và lúc bấm) → từ chối nút cũ, không duyệt; (h) dự án bị bỏ khỏi `telegram_projects` giữa chừng → không duyệt, lý do rõ; (i) chat lạ bấm bị bỏ qua; (j) `data` sai dạng → `INVALID_BUTTON`; (k) đề xuất có `after_proposal_id` hiển thị dòng `Chạy sau`. Trong `tests/intake_api.rs` hoặc `tests/orchestrator_triage.rs` (file đang test duyệt/từ chối qua HTTP) giữ nguyên test cũ: chúng phải vẫn pass sau khi tách hàm ở mục 1. | Chốt hành vi |
| 8 | `agent-brain/README.md` | mục Telegram | Thêm một đoạn ngắn: đề xuất của việc giao từ Telegram được gửi về Telegram, có nút `Duyệt đề xuất` / `Từ chối`; việc từ dashboard không gửi; muốn sửa đội hình thì dùng dashboard; nút kiểm lại `telegram_projects` lúc bấm. | Tài liệu |

## Không được đụng tới
- Luồng duyệt plan (`on_approve`, `Op::Approve*`, D-48), `claim::approve_check`, autopilot, quota, `transport.rs`, `teams`.
- `orchestrator/` (triage, scheduler): chỉ ĐỌC. Không đổi cách tạo `proposals`.
- Hành vi và thông điệp lỗi HTTP của dashboard: chỉ tách hàm (mục 1), không đổi.
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*` (brain.db thật, serve đang chạy), `~/.claude-orca`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`.
- Không đọc, in hay ghi token Telegram; test dùng token giả.
- KHÔNG thay binary `agent-brain` đang chạy (chỉ build/test trong cây làm việc). `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -5
cargo test --offline --locked --lib telegram 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked --test telegram_proposals 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test telegram_intake --test telegram_actions --test telegram_notify --test telegram_outage --test intake_api --test orchestrator_triage 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked"      # toàn bộ: không lỗi
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(telegram/(bot|callback|render|detect|outbox)|dashboard/api_intake)\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -3
```
Acceptance:
1. Việc giao từ Telegram: khi orchestrator đặt đề xuất `pending`, Telegram nhận đúng một tin cho mỗi đề xuất, có nút `Duyệt đề xuất` và `Từ chối`.
2. `Duyệt đề xuất` chuyển đề xuất sang `approved` bằng cùng đường với dashboard (kể cả `triage::validate`); `Từ chối` chuyển sang `rejected`.
3. Việc giao từ dashboard không gửi tin; restart serve không gửi lại tin cũ.
4. Nút cũ, đề xuất đã xử lý ở nơi khác, dự án bị bỏ khỏi `telegram_projects`, chat lạ: không đổi gì và có thông báo rõ.
5. Mọi test cũ pass; chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Hai commit: `feat(agent-brain): send Telegram-created intake proposals to Telegram with approve/reject buttons` (code, test), `docs: Telegram proposal approval in README` (README). Mỗi commit kèm dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan (nhất là cách tính `rev` và chữ ký hai hàm dùng chung ở mục 1), kết quả từng lệnh verify, phần chưa kiểm (chưa thử qua Telegram thật, chưa restart serve).
